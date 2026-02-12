pub mod types;

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub use types::Config;

pub fn default_config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });
    config_dir.join("ferritebar").join("config.toml")
}

pub fn load_config(path: &Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => {
                info!("Loaded config from {}", path.display());
                config
            }
            Err(e) => {
                error!("Failed to parse config: {e}");
                error!("Using default config");
                toml::from_str("").unwrap()
            }
        },
        Err(e) => {
            error!("Failed to read config at {}: {e}", path.display());
            error!("Using default config");
            toml::from_str("").unwrap()
        }
    }
}

/// Watch the config file for changes using inotify.
/// Sends a signal on the returned receiver when the config changes.
/// Debounces rapid changes (e.g. editor save = truncate + write).
pub fn watch_config(path: PathBuf) -> mpsc::Receiver<()> {
    use notify::{Event, EventKind, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel::<()>(4);

    crate::spawn(async move {
        let (notify_tx, mut notify_rx) = mpsc::channel::<()>(16);

        // Create watcher on a blocking thread (notify uses inotify internally)
        let watch_path = path.clone();
        let _watcher_handle = tokio::task::spawn_blocking(move || {
            let tx = notify_tx;
            let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            let _ = tx.blocking_send(());
                        }
                        _ => {}
                    }
                }
            });

            match watcher {
                Ok(ref mut w) => {
                    // Watch the parent directory (handles editor atomic saves)
                    let parent = watch_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."));
                    if let Err(e) = w.watch(parent, RecursiveMode::NonRecursive) {
                        error!("Failed to watch config directory: {e}");
                        return;
                    }
                    info!("Watching config at {}", watch_path.display());

                    // Keep the watcher alive forever
                    loop {
                        std::thread::sleep(Duration::from_secs(3600));
                    }
                }
                Err(e) => {
                    error!("Failed to create file watcher: {e}");
                }
            }
        });

        // Debounce: wait 200ms after last event before signaling reload
        loop {
            // Wait for first event
            if notify_rx.recv().await.is_none() {
                break;
            }

            // Drain rapid follow-up events (debounce 200ms)
            loop {
                match tokio::time::timeout(Duration::from_millis(200), notify_rx.recv()).await {
                    Ok(Some(())) => continue,  // More events, keep waiting
                    Ok(None) => return,         // Channel closed
                    Err(_) => break,            // Timeout - debounce complete
                }
            }

            debug!("Config change detected, signaling reload");
            if tx.send(()).await.is_err() {
                break; // Receiver dropped
            }
        }
    });

    rx
}
