# mirufm

A fast, GUI file explorer for NixOS with Finder-style Miller columns.

Mouse-first, keyboard-secondary. Built with Rust and gpui.

Status: pre-alpha, under active development.

## Development

```
nix develop
cargo build
cargo test
```

## Install

```
nix profile install .
```

To set mirufm as the default handler for folders:

```
xdg-mime default mirufm.desktop inode/directory
```

With home-manager, the equivalent is setting `xdg.mimeApps.defaultApplications."inode/directory" = "mirufm.desktop"`.

Logs are written to `$XDG_STATE_HOME/mirufm/log` (defaults to `~/.local/state/mirufm/log`).

## License

Apache-2.0.
