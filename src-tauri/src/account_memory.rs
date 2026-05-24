// Linux-only; compiled only when the `memory` feature is enabled (see lib.rs).
//
// Architecture: fork() before Tauri initialises. Child stays root forever and
// handles all privileged operations. Parent drops to the real (pre-sudo) UID
// immediately after the initial scan so D-Bus / portal / GTK connect normally.
// Communication is a pre-fork anonymous socketpair — no filesystem path, no
// external process can connect.
//
// See docs/memory.md for full design rationale and IPC security model.

use once_cell::sync::Lazy;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

// ── Protocol ──────────────────────────────────────────────────────────────────
// Request:  [u8 tag] [u8 payload_len] [payload 0..=255 bytes]
// Response: [u8 status] [u16le body_len] [body bytes]

const REQ_SCAN_ACCOUNT: u8 = 0x01;
const REQ_READ_LOG_BUFFER: u8 = 0x02;

const RESP_OK: u8 = 0x00;
const RESP_ERR: u8 = 0x01;

fn write_request(sock: &mut UnixStream, tag: u8, payload: &[u8]) -> std::io::Result<()> {
    debug_assert!(payload.len() <= 255);
    let mut frame = [0u8; 2 + 255];
    frame[0] = tag;
    frame[1] = payload.len() as u8;
    frame[2..2 + payload.len()].copy_from_slice(payload);
    sock.write_all(&frame[..2 + payload.len()])
}

fn read_response(sock: &mut UnixStream) -> std::io::Result<Result<Vec<u8>, String>> {
    let mut hdr = [0u8; 3];
    sock.read_exact(&mut hdr)?;
    let status = hdr[0];
    let body_len = u16::from_le_bytes([hdr[1], hdr[2]]) as usize;
    let mut body = vec![0u8; body_len];
    sock.read_exact(&mut body)?;
    if status == RESP_OK {
        Ok(Ok(body))
    } else {
        Ok(Err(String::from_utf8_lossy(&body).into_owned()))
    }
}

// ── Public types / statics ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountInfo {
    pub username: String,
    pub account_id: String,
    pub nonce: Option<String>,
}

/// Result of the initial scan. Populated by the parent before Tauri starts.
pub static ACCOUNT_INFO: Lazy<Mutex<Option<AccountInfo>>> = Lazy::new(|| Mutex::new(None));

/// Persistent connection to the privileged child. Available for future requests
/// after the initial startup scan.
static CHILD_SOCK: Lazy<Mutex<Option<UnixStream>>> = Lazy::new(|| Mutex::new(None));

// ── Parent: entry point ───────────────────────────────────────────────────────

/// Fork a privileged child, do the initial account scan, then drop back to the
/// real user. Must be called before `tauri::Builder::default()`.
pub fn init() {
    if !is_root() {
        eprintln!("[memory] not root — skipping (rerun as sudo)");
        return;
    }

    let (mut parent_fd, child_fd) = match socketpair_cloexec() {
        Some(p) => p,
        None => {
            eprintln!("[memory] socketpair failed: {}", std::io::Error::last_os_error());
            return;
        }
    };

    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("[memory] fork failed: {}", std::io::Error::last_os_error());
        }
        0 => {
            // ── privileged child ─────────────────────────────────────────────
            drop(parent_fd);
            child_main(child_fd);
            // never returns
        }
        _child_pid => {
            // ── parent ───────────────────────────────────────────────────────
            drop(child_fd);

            // Fetch account info synchronously — all data available before Tauri starts.
            if let Err(e) = write_request(&mut parent_fd, REQ_SCAN_ACCOUNT, &[]) {
                eprintln!("[memory] send REQ_SCAN_ACCOUNT: {e}");
            } else {
                match read_response(&mut parent_fd) {
                    Ok(Ok(body)) => match serde_json::from_slice::<AccountInfo>(&body) {
                        Ok(info) => {
                            eprintln!("[memory] account: {} ({})", info.username, info.account_id);
                            *ACCOUNT_INFO.lock().unwrap() = Some(info);
                        }
                        Err(e) => eprintln!("[memory] deserialize AccountInfo: {e}"),
                    },
                    Ok(Err(e)) => eprintln!("[memory] child error: {e}"),
                    Err(e) => eprintln!("[memory] recv_response: {e}"),
                }
            }

            // Keep socket alive for future requests.
            *CHILD_SOCK.lock().unwrap() = Some(parent_fd);

            drop_privileges();
        }
    }
}

fn socketpair_cloexec() -> Option<(UnixStream, UnixStream)> {
    let mut fds = [-1i32; 2];
    let ret = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if ret != 0 {
        return None;
    }
    unsafe { Some((UnixStream::from_raw_fd(fds[0]), UnixStream::from_raw_fd(fds[1]))) }
}

// ── Privilege helpers ─────────────────────────────────────────────────────────

fn effective_uid() -> u32 {
    let Ok(status) = fs::read_to_string("/proc/self/status") else {
        return 1;
    };
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:\t") {
            return rest
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
        }
    }
    1
}

fn is_root() -> bool {
    effective_uid() == 0
}

fn real_ids() -> (u32, u32) {
    // sudo sets all three UIDs (real/effective/saved) to 0, so /proc/self/status
    // shows "Uid: 0 0 0 0" — the real-uid field is useless. Use $SUDO_UID/$SUDO_GID
    // which sudo always injects, then /proc/self/loginuid as a kernel-level fallback
    // (unchanged by setuid/sudo, requires CONFIG_AUDIT which is standard on most distros).
    let uid: u32 = std::env::var("SUDO_UID")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            fs::read_to_string("/proc/self/loginuid").ok().and_then(|s| {
                let v: u32 = s.trim().parse().ok()?;
                if v == u32::MAX { None } else { Some(v) } // MAX = loginuid unset
            })
        })
        .unwrap_or(0);
    let gid: u32 = std::env::var("SUDO_GID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (uid, gid)
}

pub fn drop_privileges() {
    let (real_uid, real_gid) = real_ids();
    if real_uid == 0 {
        return; // genuinely started as root — nothing to drop
    }
    unsafe {
        libc::setresgid(real_gid, real_gid, real_gid);
        libc::setresuid(real_uid, real_uid, real_uid);
        // setresuid clears the dumpable flag when credentials change, which makes
        // /proc/<pid>/ entries root-owned. The XDG portal verifies callers by
        // opening /proc/<pid>/root — restore dumpable so it can.
        libc::prctl(libc::PR_SET_DUMPABLE, 1, 0, 0, 0);
    }
    eprintln!("[memory] dropped to UID={real_uid} GID={real_gid}");
}

// ── Shared scan logic ─────────────────────────────────────────────────────────

/// Last `Logged in` line in EE.log → (username, account_id).
pub fn parse_account_from_log() -> Option<(String, String)> {
    let content = fs::read_to_string(crate::ee_log::log_path()).ok()?;
    let mut last = None;
    for line in content.lines() {
        let Some(idx) = line.find("Logged in ") else { continue };
        let rest = &line[idx + "Logged in ".len()..];
        let (Some(sp), Some(open), Some(close)) =
            (rest.find(' '), rest.find('('), rest.find(')'))
        else {
            continue;
        };
        if sp < open && open < close {
            last = Some((rest[..sp].to_string(), rest[open + 1..close].to_string()));
        }
    }
    last
}

/// Find the PID of a running `Warframe.x64` process via /proc.
fn find_warframe_pid() -> Option<u32> {
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        // Re-read cmdline for every candidate — also used for PID re-verification
        // in request handlers, so keep this as a standalone function.
        if pid_is_warframe(pid) {
            return Some(pid);
        }
    }
    None
}

/// Verify `pid` still maps to Warframe. Re-checked before every privileged read
/// to defeat PID reuse attacks (Warframe exits → attacker claims same PID).
fn pid_is_warframe(pid: u32) -> bool {
    let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    cmdline.windows(12).any(|w| w == b"Warframe.x64")
}

struct MapRegion {
    start: u64,
    end: u64,
}

fn readable_anonymous_regions(pid: u32) -> Vec<MapRegion> {
    let Ok(maps) = fs::read_to_string(format!("/proc/{pid}/maps")) else {
        return vec![];
    };
    let mut out = Vec::new();
    for line in maps.lines() {
        let mut cols = line.split_whitespace();
        let Some(range) = cols.next() else { continue };
        let Some(perms) = cols.next() else { continue };
        if !perms.starts_with('r') {
            continue;
        }
        let _ = (cols.next(), cols.next(), cols.next()); // offset, dev, inode
        let path = cols.next().unwrap_or("").trim();
        if !path.is_empty() && !path.starts_with('[') {
            continue; // skip file-backed mappings
        }
        let Some((s, e)) = range.split_once('-') else { continue };
        let (Ok(start), Ok(end)) =
            (u64::from_str_radix(s, 16), u64::from_str_radix(e, 16))
        else {
            continue;
        };
        if end.saturating_sub(start) > 64 * 1024 * 1024 {
            continue; // skip regions > 64 MiB
        }
        out.push(MapRegion { start, end });
    }
    out
}

const CHUNK: usize = 65536;
const OVERLAP: usize = 256; // > needle.len() + max_nonce_len; catches cross-boundary patterns

fn scan_nonce(pid: u32, account_id: &str) -> Option<String> {
    let mem_file = match fs::File::open(format!("/proc/{pid}/mem")) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[memory:child] open /proc/{pid}/mem: {e}");
            return None;
        }
    };

    let needle_str = format!("?accountId={account_id}&nonce=");
    let needle = needle_str.as_bytes();
    let mut buf = vec![0u8; CHUNK + OVERLAP];

    for region in readable_anonymous_regions(pid) {
        let total = (region.end - region.start) as usize;
        let mut consumed = 0usize;
        let mut valid = 0usize;

        loop {
            if valid > OVERLAP {
                buf.copy_within(valid - OVERLAP..valid, 0);
                valid = OVERLAP;
            }
            if consumed >= total {
                break;
            }
            let want = (total - consumed).min(CHUNK);
            match mem_file.read_at(&mut buf[valid..valid + want], region.start + consumed as u64) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    consumed += n;
                    valid += n;
                }
            }
            if let Some(pos) = buf[..valid].windows(needle.len()).position(|w| w == needle) {
                let ns = pos + needle.len();
                let nonce: String = buf[ns..valid]
                    .iter()
                    .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
                    .map(|&b| b as char)
                    .collect();
                if !nonce.is_empty() {
                    return Some(nonce);
                }
            }
        }
    }
    None
}

// ── Child: raw fd I/O ─────────────────────────────────────────────────────────
//
// UnixStream::{read,write} route through libc::recv/libc::send which map to
// SYS_recvfrom/SYS_sendto. SYS_sendto (44) is in the seccomp denylist, so any
// write after the filter is applied would be killed. Use libc::read/libc::write
// (SYS_read=0, SYS_write=1) directly — both are allowed.

fn fd_read_exact(fd: i32, buf: &mut [u8]) -> bool {
    let mut done = 0;
    while done < buf.len() {
        let n = unsafe {
            libc::read(fd, buf.as_mut_ptr().add(done) as *mut libc::c_void, buf.len() - done)
        };
        if n <= 0 {
            return false;
        }
        done += n as usize;
    }
    true
}

fn fd_write_all(fd: i32, buf: &[u8]) -> bool {
    let mut done = 0;
    while done < buf.len() {
        let n = unsafe {
            libc::write(fd, buf.as_ptr().add(done) as *const libc::c_void, buf.len() - done)
        };
        if n <= 0 {
            return false;
        }
        done += n as usize;
    }
    true
}

fn child_send_response(fd: i32, status: u8, body: &[u8]) -> bool {
    let len = (body.len() as u16).to_le_bytes();
    let hdr = [status, len[0], len[1]];
    fd_write_all(fd, &hdr) && fd_write_all(fd, body)
}

// ── Child: main loop ──────────────────────────────────────────────────────────

fn child_main(sock: UnixStream) -> ! {
    use std::os::unix::io::IntoRawFd;
    // Take raw fd ownership so UnixStream doesn't close it on drop.
    let fd = sock.into_raw_fd();

    apply_seccomp_denylist();

    loop {
        // Read request header [tag, payload_len].
        let mut hdr = [0u8; 2];
        if !fd_read_exact(fd, &mut hdr) {
            break; // parent closed socket or died
        }
        let tag = hdr[0];
        let payload_len = hdr[1] as usize;
        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 && !fd_read_exact(fd, &mut payload) {
            break;
        }

        let (status, body): (u8, Vec<u8>) = match tag {
            REQ_READ_LOG_BUFFER => match handle_read_log_buffer() {
                Ok(buf) => (RESP_OK, buf),
                Err(e) => (RESP_ERR, e.into_bytes()),
            },
            REQ_SCAN_ACCOUNT => {
                eprintln!("[memory:child] handling ScanAccount");
                match handle_scan_account() {
                    Ok(info) => {
                        eprintln!("[memory:child] scan ok, serialising");
                        match serde_json::to_vec(&info) {
                            Ok(j) => {
                                eprintln!("[memory:child] serialised {} bytes", j.len());
                                (RESP_OK, j)
                            }
                            Err(e) => {
                                eprintln!("[memory:child] serialise error: {e}");
                                (RESP_ERR, e.to_string().into_bytes())
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[memory:child] scan error: {e}");
                        (RESP_ERR, e.into_bytes())
                    }
                }
            }
            other => {
                eprintln!("[memory:child] unknown tag 0x{other:02x}");
                (RESP_ERR, b"unknown command".to_vec())
            }
        };

        eprintln!("[memory:child] sending response: status={status} body_len={}", body.len());
        if !child_send_response(fd, status, &body) {
            eprintln!("[memory:child] write failed, exiting");
            break;
        }
        eprintln!("[memory:child] response sent ok");
    }

    unsafe { libc::close(fd) };
    std::process::exit(0);
}

/// Read the EE.log ring buffer from Warframe's memory via the privileged child.
/// Returns None if the child is unavailable or Warframe is not running.
pub fn read_log_buffer() -> Option<Vec<u8>> {
    let mut guard = CHILD_SOCK.lock().unwrap();
    let sock = guard.as_mut()?;
    write_request(sock, REQ_READ_LOG_BUFFER, &[]).ok()?;
    match read_response(sock) {
        Ok(Ok(body)) => Some(body),
        _ => None,
    }
}

fn handle_read_log_buffer() -> Result<Vec<u8>, String> {
    let pid = find_warframe_pid().ok_or_else(|| "Warframe not running".to_string())?;
    if !pid_is_warframe(pid) {
        return Err("PID stale".into());
    }
    let mem_file = fs::File::open(format!("/proc/{pid}/mem"))
        .map_err(|e| format!("open /proc/{pid}/mem: {e}"))?;
    const BUFFER_VA: u64 = 0x589000;
    const BUFFER_SIZE: usize = 0x4000; // 16 KiB — covers full ~8 KiB ring buffer
    let mut buf = vec![0u8; BUFFER_SIZE];
    mem_file
        .read_at(&mut buf, BUFFER_VA)
        .map_err(|e| format!("pread 0x{BUFFER_VA:x}: {e}"))?;
    Ok(buf)
}

fn handle_scan_account() -> Result<AccountInfo, String> {
    let (username, account_id) =
        parse_account_from_log().ok_or("no login line in EE.log")?;

    let nonce = find_warframe_pid().and_then(|pid| {
        // Re-verify PID identity before reading memory (PID reuse mitigation).
        if !pid_is_warframe(pid) {
            eprintln!("[memory:child] PID {pid} no longer Warframe — skipping");
            return None;
        }
        eprintln!("[memory:child] Warframe PID={pid}, scanning…");
        let n = scan_nonce(pid, &account_id);
        eprintln!("[memory:child] nonce: {}found", if n.is_some() { "" } else { "not " });
        n
    });

    Ok(AccountInfo { username, account_id, nonce })
}

// ── Child: seccomp denylist ───────────────────────────────────────────────────
//
// Denylist (not allowlist) so Rust's stdlib and allocator work unmodified.
// The explicitly denied syscalls receive SECCOMP_RET_KILL_PROCESS — the child
// process is immediately terminated by the kernel with no chance to recover.
//
// BPF layout (n = number of denied syscalls):
//   [0]     LOAD nr
//   [1..n]  JEQ denied[i], jt=(n-i), jf=0   → match: jump to KILL; no match: next
//   [n+1]   RET ALLOW
//   [n+2]   RET KILL_PROCESS

fn apply_seccomp_denylist() {
    // Classic BPF opcodes (not eBPF).
    const BPF_LOAD_NR: u16 = 0x20; // BPF_LD | BPF_W | BPF_ABS
    const BPF_JEQ_K: u16 = 0x15;   // BPF_JMP | BPF_JEQ | BPF_K
    const BPF_RET_K: u16 = 0x06;   // BPF_RET | BPF_K
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;

    let mut denied: Vec<libc::c_long> = vec![
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_clone,
        libc::SYS_ptrace,
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_bind,
        libc::SYS_listen,
        libc::SYS_sendmsg,
        libc::SYS_sendto,
    ];
    // clone3 (kernel 5.3+) — hardcode number since older libc may not define it.
    #[cfg(target_arch = "x86_64")]
    denied.push(435);
    #[cfg(target_arch = "aarch64")]
    denied.push(220); // same number as clone on aarch64 but clone3 is 435 on x86_64

    let n = denied.len();
    let mut insns: Vec<libc::sock_filter> = Vec::with_capacity(n + 3);

    // [0] Load syscall number (offset 0 in struct seccomp_data).
    insns.push(libc::sock_filter { code: BPF_LOAD_NR, jt: 0, jf: 0, k: 0 });

    // [1..=n] One JEQ per denied syscall.
    for (i, &nr) in denied.iter().enumerate() {
        insns.push(libc::sock_filter {
            code: BPF_JEQ_K,
            jt: (n - i) as u8, // jump distance to KILL instruction
            jf: 0,
            k: nr as u32,
        });
    }

    // [n+1] Allow — reached by fallthrough if no JEQ matched.
    insns.push(libc::sock_filter { code: BPF_RET_K, jt: 0, jf: 0, k: SECCOMP_RET_ALLOW });
    // [n+2] Kill — jumped to by any matching JEQ.
    insns.push(libc::sock_filter { code: BPF_RET_K, jt: 0, jf: 0, k: SECCOMP_RET_KILL_PROCESS });

    let mut prog = libc::sock_fprog {
        len: insns.len() as u16,
        filter: insns.as_mut_ptr(),
    };

    unsafe {
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
        let ret = libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER as libc::c_ulong,
            &mut prog as *mut _ as libc::c_ulong,
            0,
            0,
        );
        if ret != 0 {
            eprintln!(
                "[memory:child] seccomp failed ({}): continuing without filter",
                std::io::Error::last_os_error()
            );
        } else {
            eprintln!("[memory:child] seccomp denylist active ({n} rules)");
        }
    }
}
