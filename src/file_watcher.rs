/// File watcher for hot-reloading TCL modules and configuration
///
/// This module watches the tcl/ directory and config.toml for changes
/// and triggers reloads when files are modified.

use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Type of file change detected
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// TCL module file changed
    TclModule,
    /// Config file changed
    Config,
}

/// File change event
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub change_type: ChangeType,
}

/// File watcher that monitors for file changes
pub struct FileWatcher {
    /// Path to the tcl directory
    tcl_dir: PathBuf,
    /// Path to the config file
    config_path: PathBuf,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(tcl_dir: PathBuf, config_path: PathBuf) -> Self {
        Self {
            tcl_dir,
            config_path,
        }
    }

    /// Start watching for file changes
    ///
    /// This returns a channel receiver that will receive FileChangeEvent when files change.
    /// The watcher uses a 1-second debounce to avoid triggering on every write during editing.
    pub fn start_watching(&self) -> Result<Receiver<FileChangeEvent>> {
        let (tx, rx) = std::sync::mpsc::channel();

        let tcl_dir = self.tcl_dir.clone();
        let config_path = self.config_path.clone();

        // Create debounced file watcher with 1-second debounce
        let mut debouncer = new_debouncer(
            Duration::from_secs(1),
            move |res: Result<Vec<DebouncedEvent>, _>| {
                match res {
                    Ok(events) => {
                        for event in events {
                            let path = &event.path;
                            debug!("File change detected: {:?}", path);

                            // Check if it's a TCL module file
                            if path.starts_with(&tcl_dir) && path.extension().and_then(|s| s.to_str()) == Some("tcl") {
                                info!("TCL module changed: {:?}", path);
                                let _ = tx.send(FileChangeEvent {
                                    path: path.clone(),
                                    change_type: ChangeType::TclModule,
                                });
                            }
                            // Check if it's the config file
                            else if path == &config_path {
                                info!("Config file changed: {:?}", path);
                                let _ = tx.send(FileChangeEvent {
                                    path: path.clone(),
                                    change_type: ChangeType::Config,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        error!("File watcher error: {:?}", e);
                    }
                }
            },
        )?;

        // Watch the TCL directory if it exists
        if self.tcl_dir.exists() {
            debouncer
                .watcher()
                .watch(&self.tcl_dir, notify::RecursiveMode::NonRecursive)?;
            info!("Watching TCL directory: {:?}", self.tcl_dir);
        } else {
            warn!("TCL directory does not exist: {:?}", self.tcl_dir);
        }

        // Watch the config file if it exists
        if self.config_path.exists() {
            if let Some(parent) = self.config_path.parent() {
                // Watch the parent directory since we can't watch a single file directly
                debouncer
                    .watcher()
                    .watch(parent, notify::RecursiveMode::NonRecursive)?;
                info!("Watching config file: {:?}", self.config_path);
            }
        } else {
            warn!("Config file does not exist: {:?}", self.config_path);
        }

        // Keep the debouncer alive by leaking it (it will live for the lifetime of the program)
        // This is intentional - we want the file watcher to stay active until shutdown
        Box::leak(Box::new(debouncer));

        Ok(rx)
    }

    /// Get the TCL directory path
    pub fn tcl_dir(&self) -> &Path {
        &self.tcl_dir
    }

    /// Get the config path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let tcl_dir = temp_dir.path().join("tcl");
        let config_path = temp_dir.path().join("config.toml");

        fs::create_dir(&tcl_dir).unwrap();
        fs::write(&config_path, "test config").unwrap();

        let watcher = FileWatcher::new(tcl_dir.clone(), config_path.clone());
        assert_eq!(watcher.tcl_dir(), tcl_dir.as_path());
        assert_eq!(watcher.config_path(), config_path.as_path());
    }
}
