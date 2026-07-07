use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fs::{Entry, EntryKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Loaded,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub selection: Option<usize>,
    pub stage: Stage,
}

#[derive(Debug, Clone)]
pub struct CachedFolder {
    pub entries: Vec<Entry>,
    pub loaded_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub columns: Vec<Column>,
    pub cache: HashMap<PathBuf, CachedFolder>,
}

impl AppState {
    pub fn new(root: PathBuf) -> AppState {
        AppState {
            columns: vec![Column {
                path: root,
                entries: Vec::new(),
                selection: None,
                stage: Stage::Loading,
            }],
            cache: HashMap::new(),
        }
    }

    pub fn select(&mut self, col: usize, entry_index: usize) {
        if let Some(c) = self.columns.get_mut(col) {
            if entry_index < c.entries.len() {
                c.selection = Some(entry_index);
            }
        }
    }

    pub fn descend(&mut self, col: usize, entry_index: usize) -> Option<PathBuf> {
        let entry = self.columns.get(col)?.entries.get(entry_index)?.clone();
        self.select(col, entry_index);
        // Drop any columns deeper than the one we acted in.
        self.columns.truncate(col + 1);
        if entry.kind != EntryKind::Dir {
            return None;
        }
        self.columns.push(Column {
            path: entry.path.clone(),
            entries: Vec::new(),
            selection: None,
            stage: Stage::Loading,
        });
        Some(entry.path)
    }

    pub fn set_loaded(&mut self, path: &Path, entries: Vec<Entry>, now: SystemTime) {
        self.cache.insert(
            path.to_path_buf(),
            CachedFolder { entries: entries.clone(), loaded_at: now },
        );
        if let Some(c) = self.columns.iter_mut().find(|c| c.path == path) {
            c.entries = entries;
            c.stage = Stage::Loaded;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Meta;

    fn dir_entry(name: &str, parent: &Path) -> Entry {
        Entry {
            name: name.to_string(),
            path: parent.join(name),
            kind: EntryKind::Dir,
            meta: Meta { size: 0, modified: None, readonly: false },
        }
    }
    fn file_entry(name: &str, parent: &Path) -> Entry {
        Entry {
            name: name.to_string(),
            path: parent.join(name),
            kind: EntryKind::File,
            meta: Meta { size: 0, modified: None, readonly: false },
        }
    }

    #[test]
    fn new_has_one_loading_root_column() {
        let s = AppState::new(PathBuf::from("/root"));
        assert_eq!(s.columns.len(), 1);
        assert_eq!(s.columns[0].path, PathBuf::from("/root"));
        assert_eq!(s.columns[0].stage, Stage::Loading);
    }

    #[test]
    fn descend_into_dir_pushes_loading_column() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let sub = dir_entry("sub", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![sub.clone()], SystemTime::UNIX_EPOCH);

        let to_load = s.descend(0, 0);
        assert_eq!(to_load, Some(PathBuf::from("/root/sub")));
        assert_eq!(s.columns.len(), 2);
        assert_eq!(s.columns[1].stage, Stage::Loading);
        assert_eq!(s.columns[0].selection, Some(0));
    }

    #[test]
    fn descend_into_file_selects_but_adds_no_column() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let f = file_entry("a.txt", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![f], SystemTime::UNIX_EPOCH);

        let to_load = s.descend(0, 0);
        assert_eq!(to_load, None);
        assert_eq!(s.columns.len(), 1);
        assert_eq!(s.columns[0].selection, Some(0));
    }

    #[test]
    fn descending_truncates_deeper_columns() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let a = dir_entry("a", Path::new("/root"));
        let b = dir_entry("b", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![a, b], SystemTime::UNIX_EPOCH);
        s.descend(0, 0); // opens /root/a as column 1
        assert_eq!(s.columns.len(), 2);
        // Now descend into a different entry in column 0; column 1 must be replaced.
        let to_load = s.descend(0, 1);
        assert_eq!(to_load, Some(PathBuf::from("/root/b")));
        assert_eq!(s.columns.len(), 2);
        assert_eq!(s.columns[1].path, PathBuf::from("/root/b"));
    }

    #[test]
    fn set_loaded_fills_column_and_caches() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let e = file_entry("x", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![e], SystemTime::UNIX_EPOCH);
        assert_eq!(s.columns[0].stage, Stage::Loaded);
        assert_eq!(s.columns[0].entries.len(), 1);
        assert!(s.cache.contains_key(Path::new("/root")));
    }
}
