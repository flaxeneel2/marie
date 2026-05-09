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
// moment the 4-choice screen becomes visible.
const TRIGGER: &str = "Relic rewards initialized";

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
                        if line.contains(TRIGGER) {
                            eprintln!("[marie] relic reward screen detected");
                            app.emit("relic-trigger", ()).ok();
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
}
