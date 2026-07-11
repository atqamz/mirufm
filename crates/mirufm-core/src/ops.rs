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
}
