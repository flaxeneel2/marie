use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    sync::mpsc,
};

use notify::{recommended_watcher, Event, RecursiveMode, Result as NotifyResult, Watcher};
use tauri::{AppHandle, Emitter};

pub fn log_path() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(|v| PathBuf::from(v).join("Warframe").join("EE.log"))
            .unwrap_or_else(|_| PathBuf::from("EE.log"))
    }
    #[cfg(not(windows))]
    {
        // Explicit override takes priority (useful for custom Steam libraries or dev/testing).
        if let Ok(p) = std::env::var("WF_EE_LOG") {
            return PathBuf::from(p);
        }

        // Default Proton path: Steam app ID 230410 (Warframe).
        if let Ok(home) = std::env::var("HOME") {
            let proton = PathBuf::from(home)
                .join(".local/share/Steam/steamapps/compatdata/230410/pfx/drive_c/users/steamuser/AppData/Local/Warframe/EE.log");
            if proton.exists() {
                return proton;
            }
        }

        PathBuf::from("EE.log")
    }
}

// Triggers when Warframe displays the relic reward selection UI.
// "Relic rewards initialized" fires before "Got rewards" and coincides with the
// moment the reward screen becomes visible.
const TRIGGER: &str = "Relic rewards initialized";

// Lua event that fires when the riven cycling screen finishes setting up.
// More reliable than the SWF creation line — fires after the diorama is ready.
const RIVEN_OPEN: &str = "OmegaRerollSelection.lua: Diorama setup";

// Cost confirmation dialog — user initiated a roll.
const RIVEN_COST_CONFIRM: &str = "Are you sure you want to cycle";

// User pressed yes on the cost confirmation — roll is confirmed.
const RIVEN_SEND_RESULT: &str = "Dialog::SendResult(4)";

// Delay after Diorama setup before capturing initial stats.
const RIVEN_OPEN_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

// Delay after SendResult(4) before OCR. Assumes new stats are on screen within 4s.
const RIVEN_REROLL_DELAY: std::time::Duration = std::time::Duration::from_millis(4000);

// HUD returning to ship — riven screen exited.
const RIVEN_CLOSE: &str = "DiegeticArtifactCards.lua: DBG: HudVis";

// Marks the start of a new fissure reward sequence; resets the card counter.
const MISSION_START: &str = "ProjectionsCountdown.swf";

// Network lag between the log line and the reward cards appearing on screen.
const TRIGGER_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);

// Rolling window of recent log lines scanned when the trigger fires.
const RECENT_LINE_BUFFER: usize = 200;

/// Best-effort squad size detection from recent log lines.
///
/// Scans the rolling buffer for known player-count patterns.  Returns `None`
/// if nothing matches; the caller falls back to 4.
///
/// If the patterns here don't fire against your EE.log, run with
/// `RUST_LOG=debug` and add the relevant substring to this function.
fn detect_squad_size(recent: &std::collections::VecDeque<String>) -> Option<u32> {
    for line in recent.iter().rev() {
        // "Net [Info]: relic reward choices: 3"  (observed in fissure sessions)
        if let Some(rest) = line
            .to_lowercase()
            .find("relic reward choices:")
            .map(|i| &line[i + "relic reward choices:".len()..])
        {
            if let Ok(n) = rest.trim().split_whitespace().next().unwrap_or("").parse::<u32>() {
                if (1..=4).contains(&n) {
                    return Some(n);
                }
            }
        }

        // "Script [Info]: PlayerCount = 2"  (seen in some build variants)
        if let Some(rest) = line
            .to_lowercase()
            .find("playercount")
            .map(|i| &line[i + "playercount".len()..])
        {
            let rest = rest.trim_start_matches(|c: char| !c.is_ascii_digit());
            if let Ok(n) = rest.split_whitespace().next().unwrap_or("").parse::<u32>() {
                if (1..=4).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Memory-based EE.log watcher: polls the in-process ring buffer at VA 0x589000
/// every 150 ms via the privileged child, avoiding the ~10 s on-disk flush delay.
/// Same events as `start_watcher`; used when the `memory` feature is enabled.
#[cfg(all(feature = "memory", target_os = "linux"))]
pub fn start_memory_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let poll = std::time::Duration::from_millis(150);

        let mut riven_active = false;
        let mut pending_roll = false;
        let mut trigger_was_in_buf = false;
        let mut cost_confirm_was_in_buf = false;
        let mut send_result_was_in_buf = false;

        loop {
            std::thread::sleep(poll);

            let buf = match crate::account_memory::read_log_buffer() {
                Some(b) => b,
                None => {
                    riven_active = false;
                    pending_roll = false;
                    trigger_was_in_buf = false;
                    cost_confirm_was_in_buf = false;
                    send_result_was_in_buf = false;
                    continue;
                }
            };

            let text = String::from_utf8_lossy(&buf);

            // ── Riven screen open / close ────────────────────────────────────────
            let riven_now = text.contains(RIVEN_OPEN);
            if riven_now && !riven_active {
                riven_active = true;
                pending_roll = false;
                cost_confirm_was_in_buf = false;
                send_result_was_in_buf = false;
                eprintln!("[riven:mem] screen opened — OCR in {RIVEN_OPEN_DELAY:?}");
                let app2 = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(RIVEN_OPEN_DELAY);
                    app2.emit("riven-screen-open", ()).ok();
                });
            }
            if !riven_now && riven_active {
                riven_active = false;
                pending_roll = false;
                cost_confirm_was_in_buf = false;
                send_result_was_in_buf = false;
                eprintln!("[riven:mem] screen closed");
            }

            // ── Riven rolling ────────────────────────────────────────────────────
            if riven_active {
                let confirm_now = text.contains(RIVEN_COST_CONFIRM);
                if confirm_now && !cost_confirm_was_in_buf {
                    eprintln!("[riven:mem] cost confirm seen — awaiting SendResult");
                    pending_roll = true;
                }
                cost_confirm_was_in_buf = confirm_now;

                let result_now = text.contains(RIVEN_SEND_RESULT);
                if result_now && !send_result_was_in_buf && pending_roll {
                    pending_roll = false;
                    eprintln!("[riven:mem] roll confirmed — OCR in {RIVEN_REROLL_DELAY:?}");
                    app.emit("riven-rolling", ()).ok();
                    let app2 = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(RIVEN_REROLL_DELAY);
                        app2.emit("riven-reroll", ()).ok();
                    });
                }
                send_result_was_in_buf = result_now;
            }

            // ── Relic trigger ────────────────────────────────────────────────────
            let trigger_now = text.contains(TRIGGER);
            if trigger_now && !trigger_was_in_buf {
                let player_count = squad_size_from_buf(&buf);
                eprintln!(
                    "[marie:mem] relic reward screen ({player_count}p), waiting {TRIGGER_DELAY:?}"
                );
                let app2 = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(TRIGGER_DELAY);
                    app2.emit("relic-trigger", player_count).ok();
                });
            }
            trigger_was_in_buf = trigger_now;
        }
    });
}

/// Extract squad size from the raw ring-buffer bytes by searching for "reward choices: N".
#[cfg(all(feature = "memory", target_os = "linux"))]
fn squad_size_from_buf(buf: &[u8]) -> u32 {
    let needle = b"reward choices: ";
    let mut last = 4u32;
    for i in 0..buf.len().saturating_sub(needle.len() + 1) {
        if buf[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            let d = buf[i + needle.len()];
            if (b'1'..=b'4').contains(&d) {
                last = (d - b'0') as u32;
            }
        }
    }
    last
}

pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let path = log_path();

        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[marie] EE.log not found at {path:?}: {e}");
                return;
            }
        };

        let mut reader = BufReader::new(file);
        // Only process lines written after the app starts.
        reader.seek(SeekFrom::End(0)).ok();

        let (tx, rx) = mpsc::channel::<NotifyResult<Event>>();
        let Ok(mut watcher) = recommended_watcher(tx) else {
            eprintln!("[marie] failed to create file watcher");
            return;
        };
        if watcher.watch(&path, RecursiveMode::NonRecursive).is_err() {
            eprintln!("[marie] failed to watch {path:?}");
            return;
        }

        eprintln!("[marie] watching {path:?}");

        let mut recent: std::collections::VecDeque<String> =
            std::collections::VecDeque::with_capacity(RECENT_LINE_BUFFER);

        // Riven session state.
        let mut riven_active = false;
        // Set when cost confirm dialog appears; cleared after SendResult(4) triggers OCR.
        let mut pending_roll = false;

        for event in rx.iter() {
            if event.is_err() {
                continue;
            }
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if line.contains(MISSION_START) {
                            recent.clear();
                        }

                        if recent.len() == RECENT_LINE_BUFFER {
                            recent.pop_front();
                        }
                        recent.push_back(line.clone());

                        if line.contains(TRIGGER) {
                            let player_count = detect_squad_size(&recent).unwrap_or(4);
                            eprintln!(
                                "[marie] relic reward screen detected ({player_count} player(s)), \
                                 waiting {TRIGGER_DELAY:?}"
                            );
                            std::thread::sleep(TRIGGER_DELAY);
                            app.emit("relic-trigger", player_count).ok();
                        }

                        if line.contains(RIVEN_OPEN) {
                            eprintln!("[riven] screen opened — OCR in {RIVEN_OPEN_DELAY:?}");
                            riven_active = true;
                            pending_roll = false;
                            std::thread::sleep(RIVEN_OPEN_DELAY);
                            app.emit("riven-screen-open", ()).ok();
                        }

                        if line.contains(RIVEN_CLOSE) {
                            eprintln!("[riven] screen closed");
                            riven_active = false;
                            pending_roll = false;
                        }

                        if riven_active {
                            eprintln!("[riven:raw] {}", line.trim_end());

                            if line.contains(RIVEN_COST_CONFIRM) {
                                eprintln!("[riven] cost confirm seen — awaiting SendResult");
                                pending_roll = true;
                            }

                            if line.contains(RIVEN_SEND_RESULT) && pending_roll {
                                pending_roll = false;
                                eprintln!("[riven] roll confirmed — OCR in {RIVEN_REROLL_DELAY:?}");
                                app.emit("riven-rolling", ()).ok();
                                std::thread::sleep(RIVEN_REROLL_DELAY);
                                app.emit("riven-reroll", ()).ok();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
}
