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
const RIVEN_OPEN_DELAY: std::time::Duration = std::time::Duration::from_millis(800);

// Delay after SendResult(4) before OCR. Assumes new stats are on screen within 1s.
const RIVEN_REROLL_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);

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
