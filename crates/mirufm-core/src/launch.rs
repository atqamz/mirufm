use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A parsed `.desktop` application entry. `icon` is parsed but not rendered in v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub path: PathBuf,
}

const TERMINAL_FALLBACKS: [&str; 6] = [
    "alacritty",
    "kitty",
    "foot",
    "wezterm",
    "gnome-terminal",
    "xterm",
];

/// Resolve the terminal to spawn. `$TERMINAL` wins if set and launchable;
/// otherwise the first fallback found. `exists` reports launchability
/// (injected for tests; the shell passes a real PATH scan).
// ponytail: treats $TERMINAL as a bare command name, no embedded args. If a
// user ever needs `flatpak run ...` as their terminal, split on whitespace here.
pub fn terminal_command(
    env_terminal: Option<&str>,
    exists: impl Fn(&str) -> bool,
) -> Option<Vec<String>> {
    if let Some(t) = env_terminal {
        let t = t.trim();
        if !t.is_empty() && exists(t) {
            return Some(vec![t.to_string()]);
        }
    }
    TERMINAL_FALLBACKS
        .iter()
        .find(|c| exists(c))
        .map(|c| vec![c.to_string()])
}

/// Installed apps whose `MimeType=` lists `mime`, scanned from `search_dirs`
/// in priority order. The first `.desktop` file of a given stem wins and
/// shadows later ones (freedesktop override semantics). Malformed,
/// non-matching, NoDisplay, and Hidden entries are skipped.
pub fn apps_for_mime(mime: &str, search_dirs: &[PathBuf]) -> Vec<DesktopApp> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for dir in search_dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(stem) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            // First occurrence of a stem is authoritative; mark it seen even
            // when it does not match, so a shadowing entry hides later ones.
            if !seen.insert(stem) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse_desktop_entry(&content, mime, &path) {
                out.push(app);
            }
        }
    }
    out
}

fn parse_desktop_entry(content: &str, mime: &str, path: &Path) -> Option<DesktopApp> {
    let mut in_group = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = None;
    let mut mimes = String::new();
    let mut nodisplay = false;
    let mut hidden = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        // Plain keys only; localized keys like `Name[de]` are ignored.
        match k.trim() {
            "Name" if name.is_none() => name = Some(v.to_string()),
            "Exec" => exec = Some(v.to_string()),
            "Icon" => icon = Some(v.to_string()),
            "MimeType" => mimes = v.to_string(),
            "NoDisplay" => nodisplay = v == "true",
            "Hidden" => hidden = v == "true",
            _ => {}
        }
    }
    if nodisplay || hidden {
        return None;
    }
    if !mimes.split(';').any(|m| m == mime) {
        return None;
    }
    Some(DesktopApp {
        name: name?,
        exec: exec?,
        icon,
        path: path.to_path_buf(),
    })
}

/// Build the argv to launch `app_exec` (a `.desktop` `Exec=` value) on `file`.
/// `%f`/`%u`/`%F`/`%U` become `file`; `%%` becomes `%`; other `%` codes are
/// dropped; if no file code appears, `file` is appended last.
// ponytail: whole-token field-code handling, no full .desktop Exec quoting.
// If an Exec with quoted args or embedded codes (`--file=%f`) shows up broken,
// upgrade to a proper Exec tokenizer here.
pub fn exec_argv(app_exec: &str, file: &Path) -> Vec<String> {
    let file_str = file.to_string_lossy().into_owned();
    let mut out = Vec::new();
    let mut had_file_code = false;
    for token in app_exec.split_whitespace() {
        match token {
            "%f" | "%u" | "%F" | "%U" => {
                out.push(file_str.clone());
                had_file_code = true;
            }
            "%%" => out.push("%".to_string()),
            "%i" | "%c" | "%k" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            other => out.push(other.to_string()),
        }
    }
    if !had_file_code {
        out.push(file_str);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn exists_set(names: &[&str]) -> impl Fn(&str) -> bool {
        let set: HashSet<String> = names.iter().map(|s| s.to_string()).collect();
        move |n: &str| set.contains(n)
    }

    #[test]
    fn terminal_command_prefers_env_when_present() {
        let cmd = terminal_command(Some("kitty"), exists_set(&["kitty", "xterm"]));
        assert_eq!(cmd, Some(vec!["kitty".to_string()]));
    }

    #[test]
    fn terminal_command_falls_back_through_list() {
        // env unset, alacritty absent, foot present -> foot (first found in order).
        let cmd = terminal_command(None, exists_set(&["foot", "xterm"]));
        assert_eq!(cmd, Some(vec!["foot".to_string()]));
    }

    #[test]
    fn terminal_command_env_missing_falls_back() {
        // env set but not installed -> fall through to the fallback list.
        let cmd = terminal_command(Some("myterm"), exists_set(&["xterm"]));
        assert_eq!(cmd, Some(vec!["xterm".to_string()]));
    }

    #[test]
    fn terminal_command_returns_none_when_nothing_found() {
        let cmd = terminal_command(None, exists_set(&[]));
        assert_eq!(cmd, None);
    }

    fn write_desktop(dir: &std::path::Path, stem: &str, body: &str) {
        std::fs::write(dir.join(format!("{stem}.desktop")), body).unwrap();
    }

    #[test]
    fn apps_for_mime_matches_declared_type() {
        let dir = tempfile::tempdir().unwrap();
        write_desktop(
            dir.path(),
            "reader",
            "[Desktop Entry]\nName=Reader\nExec=reader %f\nMimeType=application/pdf;text/plain;\n",
        );
        let apps = apps_for_mime("application/pdf", &[dir.path().to_path_buf()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Reader");
        assert_eq!(apps[0].exec, "reader %f");
    }

    #[test]
    fn apps_for_mime_skips_nonmatching_and_malformed() {
        let dir = tempfile::tempdir().unwrap();
        write_desktop(
            dir.path(),
            "other",
            "[Desktop Entry]\nName=Other\nExec=other %f\nMimeType=image/png;\n",
        );
        write_desktop(dir.path(), "junk", "not a desktop file at all");
        std::fs::write(
            dir.path().join("notdesktop.txt"),
            "[Desktop Entry]\nMimeType=application/pdf;\n",
        )
        .unwrap();
        let apps = apps_for_mime("application/pdf", &[dir.path().to_path_buf()]);
        assert!(apps.is_empty());
    }

    #[test]
    fn apps_for_mime_skips_nodisplay_and_hidden() {
        let dir = tempfile::tempdir().unwrap();
        write_desktop(
            dir.path(),
            "hiddenone",
            "[Desktop Entry]\nName=Hidden\nExec=h %f\nMimeType=application/pdf;\nNoDisplay=true\n",
        );
        let apps = apps_for_mime("application/pdf", &[dir.path().to_path_buf()]);
        assert!(apps.is_empty());
    }

    #[test]
    fn apps_for_mime_dedupes_by_desktop_stem() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        write_desktop(
            user.path(),
            "reader",
            "[Desktop Entry]\nName=UserReader\nExec=ureader %f\nMimeType=application/pdf;\n",
        );
        write_desktop(
            system.path(),
            "reader",
            "[Desktop Entry]\nName=SystemReader\nExec=sreader %f\nMimeType=application/pdf;\n",
        );
        // User dir first: it shadows the system entry of the same stem.
        let apps = apps_for_mime(
            "application/pdf",
            &[user.path().to_path_buf(), system.path().to_path_buf()],
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "UserReader");
    }

    #[test]
    fn exec_argv_substitutes_single_file_codes() {
        let f = std::path::Path::new("/tmp/a.pdf");
        assert_eq!(exec_argv("reader %f", f), vec!["reader", "/tmp/a.pdf"]);
        assert_eq!(exec_argv("reader %u", f), vec!["reader", "/tmp/a.pdf"]);
    }

    #[test]
    fn exec_argv_substitutes_multi_file_codes() {
        let f = std::path::Path::new("/tmp/a.pdf");
        assert_eq!(exec_argv("v %F", f), vec!["v", "/tmp/a.pdf"]);
        assert_eq!(exec_argv("v %U", f), vec!["v", "/tmp/a.pdf"]);
    }

    #[test]
    fn exec_argv_strips_unknown_field_codes() {
        let f = std::path::Path::new("/tmp/a.pdf");
        // %i %c %k dropped; %% -> % ; file appended because no file code present.
        assert_eq!(exec_argv("app %i %c %k", f), vec!["app", "/tmp/a.pdf"]);
        assert_eq!(exec_argv("app %%", f), vec!["app", "%", "/tmp/a.pdf"]);
    }

    #[test]
    fn exec_argv_appends_file_when_no_code() {
        let f = std::path::Path::new("/tmp/a.pdf");
        assert_eq!(exec_argv("editor", f), vec!["editor", "/tmp/a.pdf"]);
    }
}
