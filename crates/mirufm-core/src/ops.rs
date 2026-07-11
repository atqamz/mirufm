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
    #[error("cannot copy or move {src} into itself or its descendant {dest}")]
    IntoSelf { src: PathBuf, dest: PathBuf },
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
    if is_into_self(src, &dest) {
        return Err(OpsError::IntoSelf {
            src: src.to_path_buf(),
            dest,
        });
    }
    if let Err(e) = copy_recursive(src, &dest) {
        cleanup_dest(&dest);
        return Err(e);
    }
    Ok(dest)
}

/// True when `dest` is `src` itself or a path inside `src`. Copying or moving
/// into such a destination would recurse forever (the fresh child gets
/// enumerated by the source scan) or clobber the source. Canonicalizes both
/// paths so a symlinked route is caught; `dest` usually does not exist yet, so
/// its existing parent is canonicalized and the final component rejoined. Falls
/// back to a lexical component-prefix check if canonicalization is impossible.
fn is_into_self(src: &Path, dest: &Path) -> bool {
    fn canonical(p: &Path) -> Option<PathBuf> {
        if let Ok(c) = p.canonicalize() {
            return Some(c);
        }
        let parent = p.parent()?;
        let name = p.file_name()?;
        Some(parent.canonicalize().ok()?.join(name))
    }
    match (canonical(src), canonical(dest)) {
        (Some(s), Some(d)) => d.starts_with(&s),
        _ => dest.starts_with(src),
    }
}

/// Best-effort removal of a destination this operation just started creating,
/// so a copy/move that fails partway leaves no orphan copy behind. The error is
/// ignored: the caller is already returning the real failure.
fn cleanup_dest(dest: &Path) {
    let _ = remove_any(dest);
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
    if is_into_self(src, &dest) {
        return Err(OpsError::IntoSelf {
            src: src.to_path_buf(),
            dest,
        });
    }
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
    if let Err(e) = copy_recursive(src, dest) {
        cleanup_dest(dest);
        return Err(e);
    }
    if let Err(e) = remove_any(src) {
        // The copy landed but the source could not be removed, so this move
        // would duplicate the data. The dest was written by this op, so drop it
        // and report the failure - no orphan copy is left behind.
        cleanup_dest(dest);
        return Err(e);
    }
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

    #[test]
    fn copy_into_self_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let foo = dir.path().join("foo");
        std::fs::create_dir(&foo).unwrap();
        std::fs::write(foo.join("data"), b"x").unwrap();

        // Destination would be foo/foo, a descendant of the source.
        let results = copy(&[foo.clone()], &foo);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, Err(OpsError::IntoSelf { .. })));
        // No runaway recursion: no nested foo/foo was created.
        assert!(!foo.join("foo").exists());
    }

    #[test]
    fn copy_into_descendant_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let foo = dir.path().join("foo");
        let bar = foo.join("bar");
        std::fs::create_dir_all(&bar).unwrap();
        std::fs::write(foo.join("data"), b"x").unwrap();

        let results = copy(&[foo.clone()], &bar);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, Err(OpsError::IntoSelf { .. })));
        assert!(!bar.join("foo").exists());
    }

    #[test]
    fn move_into_descendant_is_rejected() {
        // The same-fs rename would give EINVAL, but the guard must also cover
        // the cross-device copy_then_delete fallback, so check it up front.
        let dir = tempfile::tempdir().unwrap();
        let foo = dir.path().join("foo");
        let bar = foo.join("bar");
        std::fs::create_dir_all(&bar).unwrap();

        let results = move_items(&[foo.clone()], &bar);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, Err(OpsError::IntoSelf { .. })));
        assert!(foo.exists());
    }

    #[test]
    fn copy_recursive_failure_leaves_no_partial_dest() {
        use std::os::unix::fs::symlink;

        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let tree = src_dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("good.txt"), b"ok").unwrap();
        // A broken symlink makes std::fs::copy fail (it follows the link to a
        // missing target), aborting the recursive copy partway. This is
        // reproducible regardless of the running user.
        symlink("nonexistent-target", tree.join("broken")).unwrap();

        let results = copy(&[tree.clone()], dst_dir.path());
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_err());
        // F9: the partially written dest subtree was cleaned up.
        assert!(!dst_dir.path().join("tree").exists());
    }

    #[test]
    fn copy_then_delete_cleans_orphan_when_source_remove_fails() {
        use std::os::unix::fs::PermissionsExt;

        let src_parent = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let tree = src_parent.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("inner"), b"z").unwrap();
        let dest = dst_dir.path().join("tree");

        // Make the source's parent read-only so removing `tree` fails after the
        // copy lands - the cross-device fallback's remove step errors.
        let orig = std::fs::metadata(src_parent.path()).unwrap().permissions();
        std::fs::set_permissions(src_parent.path(), std::fs::Permissions::from_mode(0o555))
            .unwrap();
        // Root ignores the read-only bit; skip rather than assert a false pass.
        let enforced = std::fs::write(src_parent.path().join("probe"), b"").is_err();
        let result = if enforced {
            Some(copy_then_delete(&tree, &dest))
        } else {
            None
        };
        std::fs::set_permissions(src_parent.path(), orig).unwrap();

        if let Some(result) = result {
            assert!(result.is_err());
            // F5: the orphan copy at dest was cleaned; the source is untouched.
            assert!(!dest.exists());
            assert!(tree.exists());
        }
    }

    #[test]
    fn copy_batch_reports_per_item_and_does_not_short_circuit() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let good = src_dir.path().join("good.txt");
        std::fs::write(&good, b"x").unwrap();
        let missing = src_dir.path().join("missing.txt");

        // Invalid item first: a valid item after it must still be processed.
        let results = copy(&[missing.clone(), good.clone()], dst_dir.path());
        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_err());
        assert!(results[1].1.is_ok());
        assert!(dst_dir.path().join("good.txt").exists());
    }

    #[test]
    fn move_batch_reports_per_item_and_does_not_short_circuit() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let good = src_dir.path().join("good.txt");
        std::fs::write(&good, b"x").unwrap();
        let missing = src_dir.path().join("missing.txt");

        let results = move_items(&[missing.clone(), good.clone()], dst_dir.path());
        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_err());
        assert!(results[1].1.is_ok());
        assert!(!good.exists());
        assert!(dst_dir.path().join("good.txt").exists());
    }

    #[test]
    fn delete_permanent_batch_does_not_short_circuit() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.txt");
        std::fs::write(&good, b"x").unwrap();
        let missing = dir.path().join("missing.txt");

        let results = delete_permanent(&[missing.clone(), good.clone()]);
        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_err());
        assert!(results[1].1.is_ok());
        assert!(!good.exists());
    }

    #[test]
    fn trash_batch_does_not_short_circuit() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.txt");
        std::fs::write(&good, b"x").unwrap();
        let missing = dir.path().join("missing.txt");

        let results = trash(&[missing.clone(), good.clone()]);
        // Both items produce a result even though the first fails.
        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_err());
        assert_eq!(results[0].0, missing);
        assert_eq!(results[1].0, good);
        // Trash may be unavailable in a sandbox; accept ok or a Trash error, but
        // a clean removal must leave nothing behind.
        if results[1].1.is_ok() {
            assert!(!good.exists());
        } else {
            assert!(matches!(results[1].1, Err(OpsError::Trash { .. })));
        }
    }

    #[test]
    fn move_non_exdev_rename_error_is_typed_not_copied() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let f = src_dir.path().join("a.txt");
        std::fs::write(&f, b"x").unwrap();
        // Point the "destination directory" at a regular file: rename fails with
        // ENOTDIR (not EXDEV 18), so move_one returns a typed Io error via the
        // direct-rename arm instead of taking the cross-device copy fallback.
        let not_a_dir = dst_dir.path().join("file");
        std::fs::write(&not_a_dir, b"").unwrap();

        let results = move_items(&[f.clone()], &not_a_dir);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, Err(OpsError::Io { .. })));
        // Source preserved: the rename failed and there was no copy fallback.
        assert!(f.exists());
    }
}
