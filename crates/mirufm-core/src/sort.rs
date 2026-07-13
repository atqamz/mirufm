use crate::fs::{Entry, EntryKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

pub fn sort(entries: &mut [Entry], key: SortKey, dirs_first: bool) {
    entries.sort_by(|a, b| {
        if dirs_first {
            let a_dir = a.kind == EntryKind::Dir;
            let b_dir = b.kind == EntryKind::Dir;
            if a_dir != b_dir {
                // Dirs ahead of non-dirs.
                return b_dir.cmp(&a_dir);
            }
        }
        match key {
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            // Largest first.
            SortKey::Size => b.meta.size.cmp(&a.meta.size),
            // Newest first; entries without a timestamp sort last.
            SortKey::Modified => match (b.meta.modified, a.meta.modified) {
                (Some(bt), Some(at)) => bt.cmp(&at),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });
}

pub fn filter(entries: Vec<Entry>, show_hidden: bool) -> Vec<Entry> {
    if show_hidden {
        return entries;
    }
    entries
        .into_iter()
        .filter(|e| !e.name.starts_with('.'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Meta;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn entry(name: &str, kind: EntryKind, size: u64, modified: Option<SystemTime>) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
            meta: Meta {
                size,
                modified,
                readonly: false,
            },
        }
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn name_sort_is_case_insensitive() {
        let mut v = vec![
            entry("banana", EntryKind::File, 0, None),
            entry("Apple", EntryKind::File, 0, None),
            entry("cherry", EntryKind::File, 0, None),
        ];
        sort(&mut v, SortKey::Name, false);
        assert_eq!(names(&v), vec!["Apple", "banana", "cherry"]);
    }

    #[test]
    fn dirs_first_groups_dirs_ahead() {
        let mut v = vec![
            entry("b.txt", EntryKind::File, 0, None),
            entry("zdir", EntryKind::Dir, 0, None),
            entry("a.txt", EntryKind::File, 0, None),
        ];
        sort(&mut v, SortKey::Name, true);
        assert_eq!(names(&v), vec!["zdir", "a.txt", "b.txt"]);
    }

    #[test]
    fn size_sort_descending_largest_first() {
        let mut v = vec![
            entry("small", EntryKind::File, 10, None),
            entry("big", EntryKind::File, 100, None),
            entry("mid", EntryKind::File, 50, None),
        ];
        sort(&mut v, SortKey::Size, false);
        assert_eq!(names(&v), vec!["big", "mid", "small"]);
    }

    #[test]
    fn modified_sort_newest_first() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(3600);
        let mut v = vec![
            entry("old", EntryKind::File, 0, Some(older)),
            entry("new", EntryKind::File, 0, Some(now)),
            entry("undated", EntryKind::File, 0, None),
        ];
        sort(&mut v, SortKey::Modified, false);
        assert_eq!(names(&v)[0], "new");
        assert_eq!(names(&v)[1], "old");
        assert_eq!(names(&v)[2], "undated");
    }

    #[test]
    fn filter_hides_dotfiles_when_requested() {
        let v = vec![
            entry(".hidden", EntryKind::File, 0, None),
            entry("visible", EntryKind::File, 0, None),
        ];
        let shown = filter(v.clone(), false);
        assert_eq!(names(&shown), vec!["visible"]);
        let all = filter(v, true);
        assert_eq!(all.len(), 2);
    }
}
