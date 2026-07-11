use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fs::{Entry, EntryKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Column {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: BTreeSet<usize>,
    pub anchor: Option<usize>,
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
                selected: BTreeSet::new(),
                anchor: None,
                stage: Stage::Loading,
            }],
            cache: HashMap::new(),
        }
    }

    pub fn select(&mut self, col: usize, i: usize) {
        if let Some(c) = self.columns.get_mut(col) {
            if i < c.entries.len() {
                c.selected.clear();
                c.selected.insert(i);
                c.anchor = Some(i);
            }
        }
    }

    pub fn toggle(&mut self, col: usize, i: usize) {
        if let Some(c) = self.columns.get_mut(col) {
            if i < c.entries.len() {
                if !c.selected.remove(&i) {
                    c.selected.insert(i);
                }
                c.anchor = Some(i);
            }
        }
    }

    pub fn select_range(&mut self, col: usize, i: usize) {
        if let Some(c) = self.columns.get_mut(col) {
            if i >= c.entries.len() {
                return;
            }
            let anchor = c.anchor.unwrap_or(i);
            let (lo, hi) = if anchor <= i {
                (anchor, i)
            } else {
                (i, anchor)
            };
            c.selected.clear();
            c.selected.extend(lo..=hi.min(c.entries.len() - 1));
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
            selected: BTreeSet::new(),
            anchor: None,
            stage: Stage::Loading,
        });
        Some(entry.path)
    }

    pub fn set_loaded(&mut self, path: &Path, entries: Vec<Entry>, now: SystemTime) {
        self.cache.insert(
            path.to_path_buf(),
            CachedFolder {
                entries: entries.clone(),
                loaded_at: now,
            },
        );
        if let Some(c) = self.columns.iter_mut().find(|c| c.path == path) {
            // Reconcile selection by path, not index: a reload can reorder or
            // shrink the listing, so a stale index would silently address a
            // different file and a following delete would hit the wrong entry.
            // Surviving paths keep their selection at the new index; vanished
            // paths drop out. The anchor is remapped or cleared the same way.
            let selected_paths: Vec<PathBuf> = c
                .selected
                .iter()
                .filter_map(|&i| c.entries.get(i).map(|e| e.path.clone()))
                .collect();
            let anchor_path = c
                .anchor
                .and_then(|i| c.entries.get(i).map(|e| e.path.clone()));

            c.entries = entries;
            c.stage = Stage::Loaded;

            c.selected = c
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| selected_paths.contains(&e.path))
                .map(|(i, _)| i)
                .collect();
            c.anchor = anchor_path.and_then(|p| c.entries.iter().position(|e| e.path == p));
        }
    }

    pub fn set_error(&mut self, path: &Path, message: String) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.path == path) {
            c.entries.clear();
            c.selected.clear();
            c.anchor = None;
            c.stage = Stage::Error(message);
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
            meta: Meta {
                size: 0,
                modified: None,
                readonly: false,
            },
        }
    }
    fn file_entry(name: &str, parent: &Path) -> Entry {
        Entry {
            name: name.to_string(),
            path: parent.join(name),
            kind: EntryKind::File,
            meta: Meta {
                size: 0,
                modified: None,
                readonly: false,
            },
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
        s.set_loaded(
            Path::new("/root"),
            vec![sub.clone()],
            SystemTime::UNIX_EPOCH,
        );

        let to_load = s.descend(0, 0);
        assert_eq!(to_load, Some(PathBuf::from("/root/sub")));
        assert_eq!(s.columns.len(), 2);
        assert_eq!(s.columns[1].stage, Stage::Loading);
        assert!(s.columns[0].selected.contains(&0));
    }

    #[test]
    fn descend_into_file_selects_but_adds_no_column() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let f = file_entry("a.txt", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![f], SystemTime::UNIX_EPOCH);

        let to_load = s.descend(0, 0);
        assert_eq!(to_load, None);
        assert_eq!(s.columns.len(), 1);
        assert!(s.columns[0].selected.contains(&0));
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

    #[test]
    fn set_error_marks_column_and_clears_entries() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let e = file_entry("x", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![e], SystemTime::UNIX_EPOCH);

        s.set_error(Path::new("/root"), "permission denied".to_string());

        assert_eq!(
            s.columns[0].stage,
            Stage::Error("permission denied".to_string())
        );
        assert!(s.columns[0].entries.is_empty());
    }

    #[test]
    fn select_replaces_and_sets_anchor() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let a = file_entry("a", Path::new("/root"));
        let b = file_entry("b", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![a, b], SystemTime::UNIX_EPOCH);
        s.select(0, 0);
        s.select(0, 1);
        assert_eq!(s.columns[0].selected.len(), 1);
        assert!(s.columns[0].selected.contains(&1));
        assert_eq!(s.columns[0].anchor, Some(1));
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let a = file_entry("a", Path::new("/root"));
        let b = file_entry("b", Path::new("/root"));
        s.set_loaded(Path::new("/root"), vec![a, b], SystemTime::UNIX_EPOCH);
        s.select(0, 0);
        s.toggle(0, 1);
        assert!(s.columns[0].selected.contains(&0));
        assert!(s.columns[0].selected.contains(&1));
        s.toggle(0, 1);
        assert!(!s.columns[0].selected.contains(&1));
    }

    #[test]
    fn select_range_covers_anchor_to_index() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries: Vec<_> = (0..5)
            .map(|i| file_entry(&format!("f{i}"), Path::new("/root")))
            .collect();
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);
        s.select(0, 1); // anchor = 1
        s.select_range(0, 3);
        assert_eq!(
            s.columns[0].selected.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn select_range_reversed_anchor() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries: Vec<_> = (0..5)
            .map(|i| file_entry(&format!("f{i}"), Path::new("/root")))
            .collect();
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);
        s.select(0, 3); // anchor = 3
        s.select_range(0, 1); // clicked index below the anchor
        assert_eq!(
            s.columns[0].selected.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn select_range_without_anchor_selects_single() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries: Vec<_> = (0..5)
            .map(|i| file_entry(&format!("f{i}"), Path::new("/root")))
            .collect();
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);
        // No prior selection: anchor is None, so the range collapses to i.
        s.select_range(0, 2);
        assert_eq!(
            s.columns[0].selected.iter().copied().collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(s.columns[0].anchor, None);
    }

    #[test]
    fn out_of_range_indices_are_noops() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries = vec![
            file_entry("a", Path::new("/root")),
            file_entry("b", Path::new("/root")),
        ];
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);

        // Out-of-range entry index: no panic, no selection change.
        s.select(0, 99);
        s.toggle(0, 99);
        s.select_range(0, 99);
        assert!(s.columns[0].selected.is_empty());

        // Out-of-range column index: no panic, no state change.
        s.select(5, 0);
        s.toggle(5, 0);
        s.select_range(5, 0);
        assert!(s.columns[0].selected.is_empty());
        assert_eq!(s.columns.len(), 1);
    }

    #[test]
    fn set_loaded_reconciles_selection_by_path() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries: Vec<_> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|&n| file_entry(n, Path::new("/root")))
            .collect();
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);

        // Select b (idx 1) and d (idx 3), anchor on d.
        s.select(0, 1);
        s.toggle(0, 3);
        assert_eq!(s.columns[0].anchor, Some(3));

        // Reload: a and c vanish, order becomes [d, e, b].
        let reloaded = vec![
            file_entry("d", Path::new("/root")),
            file_entry("e", Path::new("/root")),
            file_entry("b", Path::new("/root")),
        ];
        s.set_loaded(Path::new("/root"), reloaded, SystemTime::UNIX_EPOCH);

        // Selection follows by path: d -> idx 0, b -> idx 2.
        assert_eq!(
            s.columns[0].selected.iter().copied().collect::<Vec<_>>(),
            vec![0, 2]
        );
        // Anchor d moved to idx 0.
        assert_eq!(s.columns[0].anchor, Some(0));
    }

    #[test]
    fn set_loaded_drops_vanished_selection() {
        let mut s = AppState::new(PathBuf::from("/root"));
        let entries = vec![
            file_entry("a", Path::new("/root")),
            file_entry("b", Path::new("/root")),
            file_entry("c", Path::new("/root")),
        ];
        s.set_loaded(Path::new("/root"), entries, SystemTime::UNIX_EPOCH);
        // Select a (idx 0) and c (idx 2), anchor on a.
        s.select(0, 2);
        s.toggle(0, 0);
        assert_eq!(s.columns[0].anchor, Some(0));

        // Reload without a: [b, c]. a vanishes, c survives at idx 1.
        let reloaded = vec![
            file_entry("b", Path::new("/root")),
            file_entry("c", Path::new("/root")),
        ];
        s.set_loaded(Path::new("/root"), reloaded, SystemTime::UNIX_EPOCH);

        assert_eq!(
            s.columns[0].selected.iter().copied().collect::<Vec<_>>(),
            vec![1]
        );
        // Anchor a vanished, so it is cleared.
        assert_eq!(s.columns[0].anchor, None);
    }
}
