use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct Meta {
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub meta: Meta,
}

pub type Cancel = Arc<AtomicBool>;

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("failed to read directory {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read entry in {path}: {source}")]
    Entry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("read cancelled")]
    Cancelled,
}

pub fn read_dir(path: &Path, cancel: &Cancel) -> Result<Vec<Entry>, FsError> {
    let rd = std::fs::read_dir(path).map_err(|source| FsError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let mut out = Vec::new();
    for (i, ent) in rd.enumerate() {
        // Check cancellation periodically rather than every iteration.
        if i % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(FsError::Cancelled);
        }
        let ent = ent.map_err(|source| FsError::Entry {
            path: path.to_path_buf(),
            source,
        })?;

        // file_type() does not traverse symlinks on the final component.
        let ft = ent.file_type().map_err(|source| FsError::Entry {
            path: ent.path(),
            source,
        })?;
        let kind = if ft.is_symlink() {
            EntryKind::Symlink
        } else if ft.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };

        // metadata() on the DirEntry does not follow symlinks either.
        let (size, modified, readonly) = match ent.metadata() {
            Ok(m) => (m.len(), m.modified().ok(), m.permissions().readonly()),
            Err(_) => (0, None, false),
        };

        out.push(Entry {
            name: ent.file_name().to_string_lossy().into_owned(),
            path: ent.path(),
            kind,
            meta: Meta {
                size,
                modified,
                readonly,
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn reads_files_and_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let mut entries = read_dir(dir.path(), &no_cancel()).unwrap();
        entries.sort_by(|x, y| x.name.cmp(&y.name));

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[0].kind, EntryKind::File);
        assert_eq!(entries[0].meta.size, 5);
        assert_eq!(entries[1].name, "sub");
        assert_eq!(entries[1].kind, EntryKind::Dir);
    }

    #[test]
    fn detects_symlink_without_following() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("target.txt"), b"x").unwrap();
        std::os::unix::fs::symlink(dir.path().join("target.txt"), dir.path().join("link.txt"))
            .unwrap();

        let entries = read_dir(dir.path(), &no_cancel()).unwrap();
        let link = entries.iter().find(|e| e.name == "link.txt").unwrap();
        assert_eq!(link.kind, EntryKind::Symlink);
    }

    #[test]
    fn nonexistent_dir_is_read_error() {
        let err = read_dir(Path::new("/no/such/mirufm/path"), &no_cancel()).unwrap_err();
        assert!(matches!(err, FsError::Read { .. }));
    }

    #[test]
    fn cancelled_flag_aborts() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..2000 {
            std::fs::write(dir.path().join(format!("f{i}")), b"").unwrap();
        }
        let cancel = Arc::new(AtomicBool::new(true));
        let err = read_dir(dir.path(), &cancel).unwrap_err();
        assert!(matches!(err, FsError::Cancelled));
    }
}
