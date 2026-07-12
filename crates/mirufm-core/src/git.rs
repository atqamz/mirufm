use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use gix::bstr::ByteSlice;

use crate::fs::Cancel;

/// Per-entry git status. Clean entries are absent from the status map.
/// The declaration order is the roll-up precedence: `Modified > Added > Untracked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitState {
    Untracked,
    Added,
    Modified,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("failed to open git repository at {path}: {source}")]
    Open {
        path: PathBuf,
        source: Box<gix::open::Error>,
    },
    #[error("failed to compute git status for {path}: {source}")]
    Status {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("git status cancelled")]
    Cancelled,
}

/// The working-tree root of the repo containing `path`, or `None` when `path`
/// is not inside a git repository. Never errors: a discovery failure is simply
/// "no repo", so the listing renders unbadged.
pub fn discover(path: &Path) -> Option<PathBuf> {
    let repo = gix::discover(path).ok()?;
    Some(repo.workdir()?.to_path_buf())
}

/// Whole-repo status as a map from absolute path to `GitState`. Only non-clean
/// paths are present; clean and ignored paths are absent (gix prunes ignored
/// directories from the walk, so large ignored trees stay cheap). Every
/// directory ancestor of a changed path, up to but excluding `repo_root`, is
/// rolled up at the highest-precedence state of its descendants.
pub fn status(repo_root: &Path, cancel: &Cancel) -> Result<HashMap<PathBuf, GitState>, GitError> {
    let repo = gix::open(repo_root).map_err(|source| GitError::Open {
        path: repo_root.to_path_buf(),
        source: Box::new(source),
    })?;
    let workdir = repo
        .workdir()
        .map(|w| w.to_path_buf())
        .unwrap_or_else(|| repo_root.to_path_buf());

    let platform = repo
        .status(gix::progress::Discard)
        .map_err(|source| GitError::Status {
            path: repo_root.to_path_buf(),
            source: Box::new(source),
        })?
        .untracked_files(gix::status::UntrackedFiles::Files);
    let iter = platform
        .into_iter(None)
        .map_err(|source| GitError::Status {
            path: repo_root.to_path_buf(),
            source: Box::new(source),
        })?;

    let mut map: HashMap<PathBuf, GitState> = HashMap::new();
    for (i, item) in iter.enumerate() {
        if i % 256 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(GitError::Cancelled);
        }
        let item = item.map_err(|source| GitError::Status {
            path: repo_root.to_path_buf(),
            source: Box::new(source),
        })?;
        let (rela, state) = classify(&item);
        let abs = workdir.join(gix::path::from_bstr(rela));
        insert_with_rollup(&mut map, &workdir, abs, state);
    }
    Ok(map)
}

/// Extract the repo-relative path and `GitState` from one status item.
fn classify(item: &gix::status::Item) -> (&gix::bstr::BStr, GitState) {
    match item {
        gix::status::Item::TreeIndex(change) => {
            use gix::diff::index::Change;
            match change {
                Change::Addition { location, .. } => (location.as_ref(), GitState::Added),
                Change::Deletion { location, .. }
                | Change::Modification { location, .. }
                | Change::Rewrite { location, .. } => (location.as_ref(), GitState::Modified),
            }
        }
        gix::status::Item::IndexWorktree(iw) => {
            use gix::status::index_worktree::Item as Iw;
            match iw {
                Iw::Modification { rela_path, .. } => (rela_path.as_bstr(), GitState::Modified),
                Iw::DirectoryContents { entry, .. } => {
                    (entry.rela_path.as_bstr(), GitState::Untracked)
                }
                Iw::Rewrite { dirwalk_entry, .. } => {
                    (dirwalk_entry.rela_path.as_bstr(), GitState::Modified)
                }
            }
        }
    }
}

/// Insert `abs` at `state`, then walk every ancestor strictly below `repo_root`
/// and raise each to at least `state`. The root itself is never badged.
fn insert_with_rollup(
    map: &mut HashMap<PathBuf, GitState>,
    repo_root: &Path,
    abs: PathBuf,
    state: GitState,
) {
    upgrade(map, &abs, state);
    let mut cur: &Path = &abs;
    while let Some(parent) = cur.parent() {
        if parent == repo_root || !parent.starts_with(repo_root) {
            break;
        }
        upgrade(map, parent, state);
        cur = parent;
    }
}

fn upgrade(map: &mut HashMap<PathBuf, GitState>, key: &Path, state: GitState) {
    map.entry(key.to_path_buf())
        .and_modify(|s| {
            if state > *s {
                *s = state;
            }
        })
        .or_insert(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn init_repo(dir: &Path) {
        git(dir, &["init", "-q"]);
        git(dir, &["config", "user.email", "t@t"]);
        git(dir, &["config", "user.name", "t"]);
    }

    fn no_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn discover_outside_repo_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(discover(tmp.path()), None);
    }

    #[test]
    fn discover_from_subdir_returns_workdir() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();

        let found = discover(&tmp.path().join("a/b")).unwrap();
        // gix returns the canonical workdir; compare canonically.
        assert_eq!(found, tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn clean_repo_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_repo(root);
        std::fs::write(root.join("f.txt"), b"x").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "c"]);

        let map = status(&discover(root).unwrap(), &no_cancel()).unwrap();
        assert!(map.is_empty(), "clean repo yields empty map, got {map:?}");
    }

    #[test]
    fn reports_modified_added_untracked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_repo(root);
        std::fs::write(root.join("tracked.txt"), b"one").unwrap();
        git(root, &["add", "tracked.txt"]);
        git(root, &["commit", "-qm", "c"]);

        std::fs::write(root.join("tracked.txt"), b"two").unwrap();
        std::fs::write(root.join("staged.txt"), b"s").unwrap();
        git(root, &["add", "staged.txt"]);
        std::fs::write(root.join("untracked.txt"), b"u").unwrap();

        let wd = discover(root).unwrap();
        let map = status(&wd, &no_cancel()).unwrap();
        assert_eq!(map.get(&wd.join("tracked.txt")), Some(&GitState::Modified));
        assert_eq!(map.get(&wd.join("staged.txt")), Some(&GitState::Added));
        assert_eq!(
            map.get(&wd.join("untracked.txt")),
            Some(&GitState::Untracked)
        );
    }

    #[test]
    fn rolls_up_nested_change_to_ancestor_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_repo(root);
        std::fs::write(root.join("seed"), b"x").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "c"]);

        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        std::fs::write(root.join("sub/deep/nested.txt"), b"n").unwrap();

        let wd = discover(root).unwrap();
        let map = status(&wd, &no_cancel()).unwrap();
        // Leaf and both ancestor dirs are present; root itself is not.
        assert_eq!(
            map.get(&wd.join("sub/deep/nested.txt")),
            Some(&GitState::Untracked)
        );
        assert_eq!(map.get(&wd.join("sub/deep")), Some(&GitState::Untracked));
        assert_eq!(map.get(&wd.join("sub")), Some(&GitState::Untracked));
        assert_eq!(map.get(&wd), None);
    }

    #[test]
    fn rollup_takes_highest_precedence() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_repo(root);
        std::fs::create_dir_all(root.join("d")).unwrap();
        std::fs::write(root.join("d/tracked.txt"), b"one").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "c"]);

        // Under d/: one modified (tracked) and one untracked. Dir must read Modified.
        std::fs::write(root.join("d/tracked.txt"), b"two").unwrap();
        std::fs::write(root.join("d/new.txt"), b"n").unwrap();

        let wd = discover(root).unwrap();
        let map = status(&wd, &no_cancel()).unwrap();
        assert_eq!(map.get(&wd.join("d")), Some(&GitState::Modified));
    }
}
