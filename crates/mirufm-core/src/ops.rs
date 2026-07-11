use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    #[error("invalid name {0:?}")]
    InvalidName(String),
    #[error("{0} already exists")]
    Exists(PathBuf),
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("trash failed for {path}: {message}")]
    Trash { path: PathBuf, message: String },
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/')
}

/// First non-colliding path in `dir` for `name`: `name`, then `name copy.ext`,
/// `name copy 2.ext`, and so on. The extension is preserved.
pub fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let as_path = Path::new(name);
    let stem = as_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = as_path
        .extension()
        .map(|e| e.to_string_lossy().into_owned());
    let mut n = 1;
    loop {
        let base = if n == 1 {
            format!("{stem} copy")
        } else {
            format!("{stem} copy {n}")
        };
        let fname = match &ext {
            Some(e) => format!("{base}.{e}"),
            None => base,
        };
        let candidate = dir.join(&fname);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

pub fn mkdir(parent: &Path, name: &str) -> Result<PathBuf, OpsError> {
    if !valid_name(name) {
        return Err(OpsError::InvalidName(name.to_string()));
    }
    let path = parent.join(name);
    if path.exists() {
        return Err(OpsError::Exists(path));
    }
    std::fs::create_dir(&path).map_err(|source| OpsError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, OpsError> {
    if !valid_name(new_name) {
        return Err(OpsError::InvalidName(new_name.to_string()));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    let dest = parent.join(new_name);
    if dest == path {
        return Ok(dest);
    }
    if dest.exists() {
        return Err(OpsError::Exists(dest));
    }
    std::fs::rename(path, &dest).map_err(|source| OpsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_name_rejects_separators_and_dots() {
        assert!(valid_name("photo.png"));
        assert!(!valid_name(""));
        assert!(!valid_name("."));
        assert!(!valid_name(".."));
        assert!(!valid_name("a/b"));
    }

    #[test]
    fn unique_dest_returns_name_when_free() {
        let dir = tempfile::tempdir().unwrap();
        let got = unique_dest(dir.path(), "a.txt");
        assert_eq!(got, dir.path().join("a.txt"));
    }

    #[test]
    fn unique_dest_appends_copy_then_numbers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"").unwrap();
        let first = unique_dest(dir.path(), "a.txt");
        assert_eq!(first, dir.path().join("a copy.txt"));

        std::fs::write(dir.path().join("a copy.txt"), b"").unwrap();
        let second = unique_dest(dir.path(), "a.txt");
        assert_eq!(second, dir.path().join("a copy 2.txt"));
    }

    #[test]
    fn unique_dest_handles_no_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("photos")).unwrap();
        let got = unique_dest(dir.path(), "photos");
        assert_eq!(got, dir.path().join("photos copy"));
    }

    #[test]
    fn mkdir_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let made = mkdir(dir.path(), "new").unwrap();
        assert_eq!(made, dir.path().join("new"));
        assert!(made.is_dir());
    }

    #[test]
    fn mkdir_rejects_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            mkdir(dir.path(), "a/b"),
            Err(OpsError::InvalidName(_))
        ));
    }

    #[test]
    fn mkdir_errors_when_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("new")).unwrap();
        assert!(matches!(
            mkdir(dir.path(), "new"),
            Err(OpsError::Exists(_))
        ));
    }

    #[test]
    fn rename_moves_within_parent() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, b"x").unwrap();
        let renamed = rename(&a, "b.txt").unwrap();
        assert_eq!(renamed, dir.path().join("b.txt"));
        assert!(!a.exists());
        assert!(renamed.exists());
    }

    #[test]
    fn rename_rejects_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, b"x").unwrap();
        assert!(matches!(
            rename(&a, "b/c.txt"),
            Err(OpsError::InvalidName(_))
        ));
    }

    #[test]
    fn rename_errors_when_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"y").unwrap();
        assert!(matches!(rename(&a, "b.txt"), Err(OpsError::Exists(_))));
    }
}
