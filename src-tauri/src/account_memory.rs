// Linux-only; compiled only when the `memory` feature is enabled (see lib.rs).
//
// Scans /proc/<warframe_pid>/mem (as root, no ptrace) for the authentication
// nonce pattern, and parses the account ID from EE.log.
//
// Why /proc/<pid>/mem without ptrace: as root, pread() on this file triggers
// the kernel's ptrace_may_access(PTRACE_MODE_READ_FSCREDS) check which passes
// on euid=0 without requiring an actual ptrace-attach. The ptrace() syscall is
// never invoked, so EAC's ptrace-hook is not triggered.

use once_cell::sync::Lazy;
use std::fs;
use std::os::unix::fs::FileExt;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub username: String,
    pub account_id: String,
    pub nonce: Option<String>,
}

pub static ACCOUNT_INFO: Lazy<Mutex<Option<AccountInfo>>> = Lazy::new(|| Mutex::new(None));

fn is_root() -> bool {
    let Ok(status) = fs::read_to_string("/proc/self/status") else {
        return false;
    };
    for line in status.lines() {
        // "Uid:\t<real> <effective> <saved> <filesystem>"
        if let Some(rest) = line.strip_prefix("Uid:\t") {
            // effective UID is the second field
            return rest
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                .map_or(false, |uid| uid == 0);
        }
    }
    false
}

/// Scan EE.log for the last login line and return `(username, account_id)`.
/// Log format: `12.928 Sys [Info]: Logged in flaxeneel2 (5b911d7cf2f2ebf3da0de6ab)`
pub fn parse_account_from_log() -> Option<(String, String)> {
    let content = fs::read_to_string(crate::ee_log::log_path()).ok()?;
    let mut last = None;
    for line in content.lines() {
        let Some(idx) = line.find("Logged in ") else {
            continue;
        };
        let rest = &line[idx + "Logged in ".len()..];
        let (Some(sp), Some(open), Some(close)) =
            (rest.find(' '), rest.find('('), rest.find(')'))
        else {
            continue;
        };
        if sp < open && open < close {
            last = Some((
                rest[..sp].to_string(),
                rest[open + 1..close].to_string(),
            ));
        }
    }
    last
}

fn find_warframe_pid() -> Option<u32> {
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        if cmdline.windows(12).any(|w| w == b"Warframe.x64") {
            return Some(pid);
        }
    }
    None
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
        // skip offset, dev, inode
        let _ = (cols.next(), cols.next(), cols.next());
        let path = cols.next().unwrap_or("").trim();
        // only anonymous / heap / stack — skip file-backed mappings
        if !path.is_empty() && !path.starts_with('[') {
            continue;
        }
        let Some((s, e)) = range.split_once('-') else { continue };
        let (Ok(start), Ok(end)) = (
            u64::from_str_radix(s, 16),
            u64::from_str_radix(e, 16),
        ) else {
            continue;
        };
        // skip regions larger than 64 MiB to bound scan time
        if end.saturating_sub(start) > 64 * 1024 * 1024 {
            continue;
        }
        out.push(MapRegion { start, end });
    }
    out
}

const CHUNK: usize = 65536;
// Must be > needle.len() + max_nonce_len so cross-boundary patterns are caught.
const OVERLAP: usize = 256;

/// Search process memory for `?accountId=<account_id>&nonce=` and return the
/// nonce value that follows it. Uses pread() — no ptrace.
fn scan_nonce(pid: u32, account_id: &str) -> Option<String> {
    let mem_file = match fs::File::open(format!("/proc/{pid}/mem")) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[memory] open /proc/{pid}/mem: {e}");
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
            // Keep last OVERLAP bytes as prefix for cross-boundary matches.
            if valid > OVERLAP {
                buf.copy_within(valid - OVERLAP..valid, 0);
                valid = OVERLAP;
            }

            if consumed >= total {
                break;
            }
            let want = (total - consumed).min(CHUNK);
            let file_off = region.start + consumed as u64;

            match mem_file.read_at(&mut buf[valid..valid + want], file_off) {
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

pub async fn init() {
    if !is_root() {
        eprintln!("[memory] not root — skipping memory scan (rerun as sudo for nonce extraction)");
        return;
    }

    let Some((username, account_id)) = parse_account_from_log() else {
        eprintln!("[memory] no login line found in EE.log — is Warframe running?");
        return;
    };
    eprintln!("[memory] account: {username} ({account_id})");

    let nonce = tokio::task::spawn_blocking({
        let account_id = account_id.clone();
        move || match find_warframe_pid() {
            Some(pid) => {
                eprintln!("[memory] Warframe PID={pid}, scanning memory…");
                let n = scan_nonce(pid, &account_id);
                eprintln!("[memory] nonce: {}", n.as_deref().unwrap_or("<not found>"));
                n
            }
            None => {
                eprintln!("[memory] Warframe not found in /proc");
                None
            }
        }
    })
    .await
    .unwrap_or(None);

    *ACCOUNT_INFO.lock().await = Some(AccountInfo {
        username,
        account_id,
        nonce,
    });
}
