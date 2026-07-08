use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use crate::fs::{read_dir, Cancel, Entry, EntryKind};
use crate::sort::{sort, SortKey};

pub const MAX_PREVIEW_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub enum PreviewModel {
    Text { content: String, truncated: bool },
    Dir { entries: Vec<Entry> },
    Metadata(MetaView),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct MetaView {
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub mode: u32,
    pub modified: Option<SystemTime>,
    pub uid: u32,
    pub gid: u32,
    pub symlink_target: Option<PathBuf>,
}

pub fn preview(entry: &Entry, cancel: &Cancel) -> PreviewModel {
    match entry.kind {
        EntryKind::Dir => match read_dir(&entry.path, cancel) {
            Ok(mut entries) => {
                sort(&mut entries, SortKey::Name, true);
                PreviewModel::Dir { entries }
            }
            Err(e) => PreviewModel::Error(e.to_string()),
        },
        EntryKind::File => preview_file(entry, cancel),
        // Symlinks are not followed; show what the link is and where it points.
        EntryKind::Symlink => PreviewModel::Metadata(meta_view(entry)),
    }
}

fn preview_file(entry: &Entry, cancel: &Cancel) -> PreviewModel {
    if cancel.load(Ordering::Relaxed) {
        return PreviewModel::Metadata(meta_view(entry));
    }
    let file = match std::fs::File::open(&entry.path) {
        Ok(f) => f,
        Err(e) => return PreviewModel::Error(e.to_string()),
    };
    // Read one byte past the cap so we can tell whether the file was truncated
    // without a second stat (entry.meta.size may be stale).
    let mut buf = Vec::new();
    if let Err(e) = file
        .take(MAX_PREVIEW_BYTES as u64 + 1)
        .read_to_end(&mut buf)
    {
        return PreviewModel::Error(e.to_string());
    }
    let truncated = buf.len() > MAX_PREVIEW_BYTES;
    buf.truncate(MAX_PREVIEW_BYTES);
    if buf.contains(&0) {
        return PreviewModel::Metadata(meta_view(entry));
    }
    PreviewModel::Text {
        content: String::from_utf8_lossy(&buf).into_owned(),
        truncated,
    }
}

fn meta_view(entry: &Entry) -> MetaView {
    let (mode, uid, gid) = match std::fs::symlink_metadata(&entry.path) {
        Ok(m) => (m.mode(), m.uid(), m.gid()),
        Err(_) => (0, 0, 0),
    };
    let symlink_target = if entry.kind == EntryKind::Symlink {
        std::fs::read_link(&entry.path).ok()
    } else {
        None
    };
    MetaView {
        name: entry.name.clone(),
        kind: entry.kind,
        size: entry.meta.size,
        mode,
        modified: entry.meta.modified,
        uid,
        gid,
        symlink_target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{Entry, EntryKind, Meta};
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn no_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
    }

    fn entry_for(path: &Path, kind: EntryKind) -> Entry {
        Entry {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: path.to_path_buf(),
            kind,
            meta: Meta {
                size: std::fs::symlink_metadata(path)
                    .map(|m| m.len())
                    .unwrap_or(0),
                modified: None,
                readonly: false,
            },
        }
    }

    #[test]
    fn dir_path_previews_as_sorted_listing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.txt"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("a_sub")).unwrap();
        let e = entry_for(dir.path(), EntryKind::Dir);

        match preview(&e, &no_cancel()) {
            PreviewModel::Dir { entries } => {
                assert_eq!(entries.len(), 2);
                // dirs_first: directory sorts ahead of the file
                assert_eq!(entries[0].name, "a_sub");
                assert_eq!(entries[0].kind, EntryKind::Dir);
                assert_eq!(entries[1].name, "b.txt");
            }
            other => panic!("expected Dir, got {other:?}"),
        }
    }

    #[test]
    fn text_file_reads_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hi.txt");
        std::fs::write(&p, b"hello world").unwrap();
        let e = entry_for(&p, EntryKind::File);

        match preview(&e, &no_cancel()) {
            PreviewModel::Text { content, truncated } => {
                assert_eq!(content, "hello world");
                assert!(!truncated);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn large_file_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.txt");
        let big = vec![b'a'; MAX_PREVIEW_BYTES + 100];
        std::fs::write(&p, &big).unwrap();
        let e = entry_for(&p, EntryKind::File);

        match preview(&e, &no_cancel()) {
            PreviewModel::Text { content, truncated } => {
                assert_eq!(content.len(), MAX_PREVIEW_BYTES);
                assert!(truncated);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn binary_file_falls_back_to_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob.bin");
        std::fs::write(&p, [0x00, 0x01, 0x02, 0x00]).unwrap();
        let e = entry_for(&p, EntryKind::File);

        match preview(&e, &no_cancel()) {
            PreviewModel::Metadata(view) => {
                assert_eq!(view.name, "blob.bin");
                assert_eq!(view.kind, EntryKind::File);
            }
            other => panic!("expected Metadata, got {other:?}"),
        }
    }

    #[test]
    fn nonexistent_path_previews_as_error() {
        let e = Entry {
            name: "gone".to_string(),
            path: PathBuf::from("/no/such/mirufm/file"),
            kind: EntryKind::File,
            meta: Meta {
                size: 0,
                modified: None,
                readonly: false,
            },
        };
        assert!(matches!(preview(&e, &no_cancel()), PreviewModel::Error(_)));
    }

    #[test]
    fn metadata_view_reports_mode_and_size() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("perm.dat");
        std::fs::write(&p, b"1234567890").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
        // Force the Metadata branch with a null byte so it is not read as text.
        std::fs::write(&p, [0x00u8; 10]).unwrap();
        let e = entry_for(&p, EntryKind::File);

        match preview(&e, &no_cancel()) {
            PreviewModel::Metadata(view) => {
                assert_eq!(view.size, 10);
                assert_eq!(view.mode & 0o777, 0o640);
            }
            other => panic!("expected Metadata, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_before_read_aborts() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.txt");
        std::fs::write(&p, b"hello").unwrap();
        let e = entry_for(&p, EntryKind::File);
        let cancel = Arc::new(AtomicBool::new(true));

        // A cancelled preview must not return file text; it falls back to metadata.
        assert!(matches!(preview(&e, &cancel), PreviewModel::Metadata(_)));
    }
}
