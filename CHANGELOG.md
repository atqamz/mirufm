# Changelog

## 0.1.0

Initial release.

A Miller-columns file manager in Rust, built on Zed's gpui:

- Column-strip navigation with a virtualized directory view and a pinned file preview.
- File operations: copy, cut, paste, move, inline rename, new folder, trash, and permanent delete.
- Multi-selection, a right-click menu (open, open-with, launch terminal), and per-entry git status badges.
- A pure, gpui-free `mirufm-core` crate for logic; the `mirufm` crate is the GUI shell.
