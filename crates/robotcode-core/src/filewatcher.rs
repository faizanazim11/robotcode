//! File system watching using the [`notify`] crate.
//!
//! Port of the Python `robotcode.core.filewatcher` module.

use std::path::Path;
use std::sync::{Arc, Mutex};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::event::Event as RobotEvent;

/// A handle to a file watcher.  Dropping this stops the watcher.
pub struct FileWatcherHandle {
    _watcher: RecommendedWatcher,
}

/// Callback type for file change events.
pub type FileEventCallback = Arc<dyn Fn(Vec<notify::Event>) + Send + Sync + 'static>;

/// A simple file watcher that forwards [`notify::Event`]s to registered listeners.
pub struct FileWatcher {
    event: Arc<Mutex<RobotEvent<Vec<notify::Event>>>>,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            event: Arc::new(Mutex::new(RobotEvent::new())),
        }
    }

    /// Subscribe to file change notifications.
    ///
    /// Returns a [`crate::event::Subscription`] handle.
    pub fn subscribe<F>(&self, f: F) -> crate::event::Subscription<Vec<notify::Event>>
    where
        F: Fn(&Vec<notify::Event>) + Send + Sync + 'static,
    {
        self.event.lock().unwrap().subscribe(f)
    }

    /// Start watching `path` (non-recursively by default).
    ///
    /// Returns a [`FileWatcherHandle`] — drop it to stop watching.
    pub fn watch(
        &self,
        path: impl AsRef<Path>,
        recursive: bool,
    ) -> Result<FileWatcherHandle, notify::Error> {
        let event_ref = Arc::clone(&self.event);

        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                event_ref.lock().unwrap().fire(&vec![ev]);
            }
        })?;

        let mut watcher = watcher;
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher.watch(path.as_ref(), mode)?;

        Ok(FileWatcherHandle { _watcher: watcher })
    }
}

impl std::fmt::Debug for FileWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWatcher").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_watcher_creation() {
        let watcher = FileWatcher::new();
        // Verify it can be created without panicking
        let _ = format!("{:?}", watcher);
    }
}
