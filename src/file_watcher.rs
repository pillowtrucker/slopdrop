/// File watcher for hot-reloading TCL modules and configuration
///
/// This module watches the tcl/ directory and config.toml for changes
/// and triggers reloads when files are modified.

use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Receiver, Arc, Mutex};
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

/// Compute SHA1 hash of file contents
/// Returns None if file cannot be read
fn compute_file_hash(path: &Path) -> Option<String> {
    match fs::read(path) {
        Ok(contents) => {
            let mut hasher = Sha1::new();
            hasher.update(&contents);
            let result = hasher.finalize();
            Some(format!("{:x}", result))
        }
        Err(e) => {
            debug!("Failed to read file for hashing {:?}: {}", path, e);
            None
        }
    }
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
    /// The watcher uses a 2-second debounce to avoid triggering on every write during editing
    /// and to filter out spurious initial events on startup.
    ///
    /// Uses content-hash based change detection to avoid spurious reloads from metadata-only changes.
    pub fn start_watching(&self) -> Result<Receiver<FileChangeEvent>> {
        let (tx, rx) = std::sync::mpsc::channel();

        let tcl_dir = self.tcl_dir.clone();
        let config_path = self.config_path.clone();

        // Track file content hashes to detect actual content changes vs metadata changes
        let file_hashes: Arc<Mutex<HashMap<PathBuf, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let file_hashes_clone = file_hashes.clone();

        // Create debounced file watcher with 2-second debounce
        let mut debouncer = new_debouncer(
            Duration::from_secs(2),
            move |res: Result<Vec<DebouncedEvent>, _>| {
                match res {
                    Ok(events) => {
                        for event in events {
                            let path = &event.path;
                            debug!("File system event detected: {:?}", path);

                            // Determine change type
                            let change_type = if path.starts_with(&tcl_dir) && path.extension().and_then(|s| s.to_str()) == Some("tcl") {
                                Some(ChangeType::TclModule)
                            } else if path == &config_path {
                                Some(ChangeType::Config)
                            } else {
                                None
                            };

                            if let Some(change_type) = change_type {
                                // Use content hash to detect actual changes vs spurious events
                                // This filters out read-only access, metadata changes, etc.
                                if let Some(new_hash) = compute_file_hash(path) {
                                    let mut hashes = file_hashes_clone.lock().unwrap();
                                    let hash_changed = match hashes.get(path) {
                                        Some(old_hash) => old_hash != &new_hash,
                                        None => true, // First time seeing this file
                                    };

                                    if hash_changed {
                                        info!("File content changed: {:?} ({:?})", path, change_type);
                                        hashes.insert(path.clone(), new_hash);
                                        let _ = tx.send(FileChangeEvent {
                                            path: path.clone(),
                                            change_type,
                                        });
                                    } else {
                                        debug!("Spurious event ignored (content unchanged): {:?}", path);
                                    }
                                }
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
                .watch(&self.tcl_dir, RecursiveMode::NonRecursive)?;
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
                    .watch(parent, RecursiveMode::NonRecursive)?;
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
