use std::path::{Path, PathBuf};
use std::process::Command;

use mirufm_core::launch::{apps_for_mime, exec_argv, terminal_command, DesktopApp};
use mirufm_core::ops::{self, OpsError};

/// XDG application directories, user dir first (it shadows system dirs).
pub fn app_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("XDG_DATA_HOME") {
        if !home.is_empty() {
            dirs.push(PathBuf::from(home).join("applications"));
        }
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    dirs
}

/// Whether `cmd` is launchable: an absolute path that exists, or a bare name
/// found on PATH.
fn on_path(cmd: &str) -> bool {
    let p = Path::new(cmd);
    if p.is_absolute() {
        return p.exists();
    }
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    path.split(':')
        .any(|dir| !dir.is_empty() && Path::new(dir).join(cmd).exists())
}

/// Launch `path` in its default app.
pub fn open_default(path: &Path) -> std::io::Result<()> {
    Command::new("xdg-open").arg(path).spawn().map(|_| ())
}

/// Spawn a terminal with its working directory set to `dir`. Returns
/// `Ok(false)` when no terminal was found (caller shows a notice).
pub fn open_terminal(dir: &Path) -> std::io::Result<bool> {
    let Some(argv) = terminal_command(std::env::var("TERMINAL").ok().as_deref(), on_path) else {
        return Ok(false);
    };
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.current_dir(dir);
    cmd.spawn().map(|_| true)
}

/// The file's MIME type via `xdg-mime`, or `None` on failure.
pub fn mime_of(path: &Path) -> Option<String> {
    let out = Command::new("xdg-mime")
        .args(["query", "filetype"])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mime = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if mime.is_empty() {
        None
    } else {
        Some(mime)
    }
}

/// Installed apps for `path`'s MIME type.
pub fn apps_for_path(path: &Path) -> Vec<DesktopApp> {
    match mime_of(path) {
        Some(mime) => apps_for_mime(&mime, &app_search_dirs()),
        None => Vec::new(),
    }
}

/// Launch `path` with a chosen app.
pub fn open_with(app: &DesktopApp, path: &Path) -> std::io::Result<()> {
    let argv = exec_argv(&app.exec, path);
    if argv.is_empty() {
        return Ok(());
    }
    Command::new(&argv[0]).args(&argv[1..]).spawn().map(|_| ())
}

/// Copy `srcs` into `dest_dir`, returning per-item results.
pub fn run_copy(srcs: &[PathBuf], dest_dir: &Path) -> Vec<(PathBuf, Result<PathBuf, OpsError>)> {
    ops::copy(srcs, dest_dir)
}

/// Move `srcs` into `dest_dir`, returning per-item results.
pub fn run_move(srcs: &[PathBuf], dest_dir: &Path) -> Vec<(PathBuf, Result<PathBuf, OpsError>)> {
    ops::move_items(srcs, dest_dir)
}

/// Count the failures in a batch result and format a notice, or `None` if all
/// items succeeded.
pub fn batch_notice<T>(verb: &str, results: &[(PathBuf, Result<T, OpsError>)]) -> Option<String> {
    let failed = results.iter().filter(|(_, r)| r.is_err()).count();
    if failed == 0 {
        None
    } else {
        Some(format!("{verb}: {failed} of {} failed", results.len()))
    }
}
