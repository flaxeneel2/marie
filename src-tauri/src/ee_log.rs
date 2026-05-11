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

// The SWF creation line is the earliest reliable signal that the reroll screen
// is open and the current stats are visible.
const RIVEN_OPEN: &str = "Created /Lotus/Interface/OmegaRerollSelection.swf";

// Player confirmed spending Kuva — a roll is now in flight on the server.
const RIVEN_SEND_RESULT: &str = "Dialog::SendResult(4)";

// Appears in the very next Dialog line after SendResult(4) when spending Kuva.
// Distinguishes "confirm roll" from "confirm keep/reject" (which has no PleaseWait).
const RIVEN_LOADING: &str = "NavBar_QuickMatchPleaseWait";

// New stats have arrived and the accept/reject comparison is on screen.
const RIVEN_ROLLED: &str = "Cycle Riven into current selection?";

// Extra delay before OCR on the rolled event — the stats panel needs a moment
// to finish its transition animation after the dialog appears.
const RIVEN_OCR_DELAY: std::time::Duration = std::time::Duration::from_millis(3000);

// Delay before capturing the initial riven stats — the SWF creation line fires
// before the stat panel finishes rendering its transition animation.
const RIVEN_OPEN_DELAY: std::time::Duration = std::time::Duration::from_millis(2000);

// Maximum gap between SendResult(4) and QuickMatchPleaseWait for them to be
// treated as a single "confirm roll" event. Dialogs within the riven screen
// appear on consecutive lines so they arrive well within this window.
const SEND_RESULT_WINDOW: std::time::Duration = std::time::Duration::from_millis(500);

// Log line that fires when the riven reroll SWF is destroyed (screen closed).
const RIVEN_CLOSE: &str = "Destroyed /Lotus/Interface/OmegaRerollSelection.swf";

// Marks the start of a new fissure reward sequence; resets the card counter.
const MISSION_START: &str = "ProjectionsCountdown.swf";

// Network lag between the log line and the reward cards appearing on screen.
const TRIGGER_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

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

        // Riven session tracking (local to this thread — no shared state needed).
        let mut riven_active = false;
        // Timestamp of the last SendResult(4) line seen while riven_active.
        // QuickMatchPleaseWait is only treated as a roll confirmation if it
        // arrives within SEND_RESULT_WINDOW of this timestamp.
        let mut last_send_result: Option<std::time::Instant> = None;

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
                            eprintln!("[marie] riven reroll screen opened — capturing current stats");
                            riven_active = true;
                            last_send_result = None;
                            std::thread::sleep(RIVEN_OPEN_DELAY);
                            app.emit("riven-screen-open", ()).ok();
                        }

                        if line.contains(RIVEN_CLOSE) {
                            eprintln!("[marie] riven reroll screen closed");
                            riven_active = false;
                            last_send_result = None;
                        }

                        if riven_active {
                            if line.contains(RIVEN_SEND_RESULT) {
                                last_send_result = Some(std::time::Instant::now());
                            } else if line.contains(RIVEN_LOADING) {
                                // Only treat as roll confirmation if SendResult(4) was very recent.
                                let is_roll = last_send_result
                                    .map(|t| t.elapsed() < SEND_RESULT_WINDOW)
                                    .unwrap_or(false);
                                if is_roll {
                                    eprintln!("[marie] riven rolling — waiting for new stats");
                                    last_send_result = None;
                                    app.emit("riven-rolling", ()).ok();
                                }
                            } else if line.contains(RIVEN_ROLLED) {
                                last_send_result = None;
                                eprintln!("[marie] riven new stats on screen — OCR in {RIVEN_OCR_DELAY:?}");
                                std::thread::sleep(RIVEN_OCR_DELAY);
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
