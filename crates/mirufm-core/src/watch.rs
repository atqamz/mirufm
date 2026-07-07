use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

const DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("failed to start filesystem watcher: {0}")]
    Notify(#[from] notify::Error),
}

/// Guard for a single-directory watch. Dropping it stops watching: notify's
/// own `Drop` unregisters the path and shuts down its event thread, which in
/// turn disconnects our coalescing thread's channel and lets it exit.
pub struct Watcher {
    _inner: RecommendedWatcher,
}

pub fn watch(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<Watcher, WatchError> {
    let (tx, rx) = channel::<notify::Result<notify::Event>>();
    let mut inner = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    inner.watch(path, RecursiveMode::NonRecursive)?;

    thread::spawn(move || {
        while let Ok(Ok(_event)) = rx.recv() {
            // Coalesce a burst of events into a single callback.
            while matches!(rx.recv_timeout(DEBOUNCE), Ok(Ok(_))) {}
            on_change();
        }
    });

    Ok(Watcher { _inner: inner })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_on_file_creation() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = channel();
        let _watcher = watch(dir.path(), move || {
            let _ = tx.send(());
        })
        .unwrap();

        std::fs::write(dir.path().join("new_file.txt"), b"hi").unwrap();

        rx.recv_timeout(Duration::from_secs(5))
            .expect("callback should fire on file creation");
    }
}
