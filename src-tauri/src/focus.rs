/// Returns true if Warframe is the currently active/focused window.
/// Fails open (returns true) when the check cannot be performed so that
/// environments where active-window info is unavailable don't silently
/// suppress all captures.
pub fn is_warframe_focused() -> bool {
    check().unwrap_or(true)
}

fn check() -> Option<bool> {
    platform::active_window_title().map(|t| t.to_lowercase().contains("warframe"))
}

// ── Platform implementations ──────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use windows::core::PWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

    pub fn active_window_title() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 == 0 {
                return None;
            }
            let mut buf = vec![0u16; 512];
            let len = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
            if len <= 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }
}

#[cfg(not(windows))]
mod platform {
    /// Queries `_NET_ACTIVE_WINDOW` and `_NET_WM_NAME` via X11.
    /// Works for Warframe running under XWayland.
    /// Returns None if X11 is unreachable (e.g. pure Wayland session without XWayland).
    pub fn active_window_title() -> Option<String> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::*;

        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;

        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;

        let prop = conn
            .get_property(false, root, net_active_window, 0u32, 0, 1)
            .ok()?
            .reply()
            .ok()?;

        if prop.value.len() < 4 {
            return None;
        }
        let window = u32::from_ne_bytes(prop.value[..4].try_into().ok()?);
        if window == 0 {
            return None;
        }

        let net_wm_name = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .ok()?
            .reply()
            .ok()?
            .atom;

        let utf8_string = conn
            .intern_atom(false, b"UTF8_STRING")
            .ok()?
            .reply()
            .ok()?
            .atom;

        let title_prop = conn
            .get_property(false, window, net_wm_name, utf8_string, 0, 512)
            .ok()?
            .reply()
            .ok()?;

        std::str::from_utf8(&title_prop.value).ok().map(str::to_owned)
    }
}
