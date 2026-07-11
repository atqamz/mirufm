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

pub fn trash(paths: &[PathBuf]) -> Vec<(PathBuf, Result<(), OpsError>)> {
    paths
        .iter()
        .map(|p| {
            let r = trash::delete(p).map_err(|e| OpsError::Trash {
                path: p.clone(),
                message: e.to_string(),
            });
            (p.clone(), r)
        })
        .collect()
}

pub fn delete_permanent(paths: &[PathBuf]) -> Vec<(PathBuf, Result<(), OpsError>)> {
    paths.iter().map(|p| (p.clone(), remove_any(p))).collect()
}

fn remove_any(path: &Path) -> Result<(), OpsError> {
    // symlink_metadata does not follow the final symlink, so a link to a
    // directory is removed as a link, not its target.
    let meta = std::fs::symlink_metadata(path).map_err(|source| OpsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let result = if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|source| OpsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn copy(srcs: &[PathBuf], dest_dir: &Path) -> Vec<(PathBuf, Result<PathBuf, OpsError>)> {
    srcs.iter()
        .map(|src| (src.clone(), copy_one(src, dest_dir)))
        .collect()
}

fn copy_one(src: &Path, dest_dir: &Path) -> Result<PathBuf, OpsError> {
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| OpsError::InvalidName(src.display().to_string()))?;
    let dest = unique_dest(dest_dir, &name);
    copy_recursive(src, &dest)?;
    Ok(dest)
}

fn copy_recursive(src: &Path, dest: &Path) -> Result<(), OpsError> {
    let meta = std::fs::symlink_metadata(src).map_err(|source| OpsError::Io {
        path: src.to_path_buf(),
        source,
    })?;
    if meta.is_dir() {
        std::fs::create_dir(dest).map_err(|source| OpsError::Io {
            path: dest.to_path_buf(),
            source,
        })?;
        let rd = std::fs::read_dir(src).map_err(|source| OpsError::Io {
            path: src.to_path_buf(),
            source,
        })?;
        for ent in rd {
            let ent = ent.map_err(|source| OpsError::Io {
                path: src.to_path_buf(),
                source,
            })?;
            copy_recursive(&ent.path(), &dest.join(ent.file_name()))?;
        }
        Ok(())
    } else {
        // ponytail: a symlink here is copied as its target's content, not as a
        // link. Acceptable for v1; revisit if link-preserving copy is needed.
        std::fs::copy(src, dest)
            .map(|_| ())
            .map_err(|source| OpsError::Io {
                path: src.to_path_buf(),
                source,
            })
    }
}

pub fn move_items(srcs: &[PathBuf], dest_dir: &Path) -> Vec<(PathBuf, Result<PathBuf, OpsError>)> {
    srcs.iter()
        .map(|src| (src.clone(), move_one(src, dest_dir)))
        .collect()
}

fn move_one(src: &Path, dest_dir: &Path) -> Result<PathBuf, OpsError> {
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| OpsError::InvalidName(src.display().to_string()))?;
    let dest = unique_dest(dest_dir, &name);
    match std::fs::rename(src, &dest) {
        Ok(()) => Ok(dest),
        // EXDEV (18 on Linux): rename across filesystems is not allowed; fall
        // back to a recursive copy followed by deleting the source.
        Err(e) if e.raw_os_error() == Some(18) => copy_then_delete(src, &dest),
        Err(source) => Err(OpsError::Io {
            path: src.to_path_buf(),
            source,
        }),
    }
}

fn copy_then_delete(src: &Path, dest: &Path) -> Result<PathBuf, OpsError> {
    copy_recursive(src, dest)?;
    remove_any(src)?;
    Ok(dest.to_path_buf())
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
        assert!(matches!(mkdir(dir.path(), "new"), Err(OpsError::Exists(_))));
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

    #[test]
    fn delete_permanent_removes_file_and_dir() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        let d = dir.path().join("d");
        std::fs::write(&f, b"x").unwrap();
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("inner"), b"y").unwrap();

        let results = delete_permanent(&[f.clone(), d.clone()]);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
        assert!(!f.exists());
        assert!(!d.exists());
    }

    #[test]
    fn delete_permanent_reports_per_item_failure() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let results = delete_permanent(&[missing.clone()]);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_err());
    }

    #[test]
    fn trash_removes_from_origin() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("t.txt");
        std::fs::write(&f, b"x").unwrap();
        let results = trash(&[f.clone()]);
        assert_eq!(results.len(), 1);
        // Trash may be unavailable in a sandboxed CI; accept either a clean
        // removal or a reported Trash error, but never a panic or a left-behind
        // file on success.
        if results[0].1.is_ok() {
            assert!(!f.exists());
        } else {
            assert!(matches!(results[0].1, Err(OpsError::Trash { .. })));
        }
    }

    #[test]
    fn copy_file_into_dir() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let f = src_dir.path().join("a.txt");
        std::fs::write(&f, b"hello").unwrap();

        let results = copy(&[f.clone()], dst_dir.path());
        let dest = results[0].1.as_ref().unwrap();
        assert_eq!(dest, &dst_dir.path().join("a.txt"));
        assert_eq!(std::fs::read(dest).unwrap(), b"hello");
        assert!(f.exists()); // copy leaves the source
    }

    #[test]
    fn copy_directory_recursively() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let tree = src_dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("inner.txt"), b"deep").unwrap();

        let results = copy(&[tree.clone()], dst_dir.path());
        let dest = results[0].1.as_ref().unwrap();
        assert_eq!(dest, &dst_dir.path().join("tree"));
        assert_eq!(std::fs::read(dest.join("inner.txt")).unwrap(), b"deep");
    }

    #[test]
    fn copy_auto_renames_on_collision() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let f = src_dir.path().join("a.txt");
        std::fs::write(&f, b"new").unwrap();
        std::fs::write(dst_dir.path().join("a.txt"), b"old").unwrap();

        let results = copy(&[f.clone()], dst_dir.path());
        let dest = results[0].1.as_ref().unwrap();
        assert_eq!(dest, &dst_dir.path().join("a copy.txt"));
        assert_eq!(std::fs::read(dst_dir.path().join("a.txt")).unwrap(), b"old");
        assert_eq!(std::fs::read(dest).unwrap(), b"new");
    }

    #[test]
    fn move_within_filesystem_uses_rename() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let f = src_dir.path().join("a.txt");
        std::fs::write(&f, b"x").unwrap();

        let results = move_items(&[f.clone()], dst_dir.path());
        let dest = results[0].1.as_ref().unwrap();
        assert_eq!(dest, &dst_dir.path().join("a.txt"));
        assert!(!f.exists()); // move removes the source
        assert!(dest.exists());
    }

    #[test]
    fn move_auto_renames_on_collision() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let f = src_dir.path().join("a.txt");
        std::fs::write(&f, b"new").unwrap();
        std::fs::write(dst_dir.path().join("a.txt"), b"old").unwrap();

        let results = move_items(&[f.clone()], dst_dir.path());
        let dest = results[0].1.as_ref().unwrap();
        assert_eq!(dest, &dst_dir.path().join("a copy.txt"));
        assert!(!f.exists());
    }

    #[test]
    fn copy_then_delete_moves_and_removes_source() {
        // Exercises the cross-filesystem fallback path directly, without
        // needing a real second mount.
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let tree = src_dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("inner"), b"z").unwrap();

        let dest = dst_dir.path().join("tree");
        let got = copy_then_delete(&tree, &dest).unwrap();
        assert_eq!(got, dest);
        assert!(!tree.exists());
        assert_eq!(std::fs::read(dest.join("inner")).unwrap(), b"z");
    }
}
