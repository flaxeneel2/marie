use serde::Serialize;

#[derive(Serialize, Clone, Copy)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

/// Find Warframe's window and return its screen geometry in physical pixels.
/// Returns None if Warframe is not running or cannot be found.
pub fn find_warframe_geometry() -> Option<WindowGeometry> {
    platform::find()
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::WindowGeometry;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};

    pub fn find() -> Option<WindowGeometry> {
        unsafe {
            let title: Vec<u16> = "Warframe\0".encode_utf16().collect();
            let hwnd =
                FindWindowW(windows::core::PCWSTR::null(), windows::core::PCWSTR(title.as_ptr()));
            if hwnd.0 == 0 {
                return None;
            }
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect).ok()?;
            Some(WindowGeometry {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
            })
        }
    }
}

// ── Linux (XWayland via X11) ──────────────────────────────────────────────────

#[cfg(not(windows))]
mod platform {
    use super::WindowGeometry;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    pub fn find() -> Option<WindowGeometry> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;

        // _NET_CLIENT_LIST gives all top-level windows managed by the WM.
        let net_client_list = conn
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .ok()?
            .reply()
            .ok()?
            .atom;

        let prop = conn
            .get_property(false, root, net_client_list, 0u32, 0, 4096)
            .ok()?
            .reply()
            .ok()?;

        if prop.value.is_empty() {
            return None;
        }

        let windows: Vec<u32> = prop
            .value
            .chunks_exact(4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
            .collect();

        let net_wm_name =
            conn.intern_atom(false, b"_NET_WM_NAME").ok()?.reply().ok()?.atom;
        let utf8_string =
            conn.intern_atom(false, b"UTF8_STRING").ok()?.reply().ok()?.atom;

        windows.into_iter().find_map(|win| {
            let title_prop = conn
                .get_property(false, win, net_wm_name, utf8_string, 0, 512)
                .ok()?
                .reply()
                .ok()?;

            let title = String::from_utf8_lossy(&title_prop.value);
            if !title.to_lowercase().contains("warframe") {
                return None;
            }

            let geom = conn.get_geometry(win).ok()?.reply().ok()?;

            // translate_coordinates converts the window-local origin (0,0) to
            // root-window (screen) coordinates, giving the absolute screen position.
            let pos = conn
                .translate_coordinates(win, root, 0, 0)
                .ok()?
                .reply()
                .ok()?;

            Some(WindowGeometry {
                x: pos.dst_x as i32,
                y: pos.dst_y as i32,
                width: geom.width as u32,
                height: geom.height as u32,
            })
        })
    }
}
