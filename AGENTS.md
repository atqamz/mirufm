# AGENTS.md

Ground rules for AI agents and humans working on mirufm, a Miller-columns file manager in Rust.
Skim once; project convention beats personal preference.

## Architecture

- Cargo workspace, two crates:
  - `crates/mirufm-core`: pure logic, unit-tested. MUST NEVER depend on gpui - this is a compile-time firewall between logic and UI.
  - `crates/mirufm`: the GUI shell on Zed's gpui (pinned git dependency).
- New logic goes in `mirufm-core` unless it is genuinely UI; keep the shell thin.

## Toolchain

- Nix devshell only. Run cargo via `nix develop --command bash -c '<cmd>'`.
- Never install toolchains or tools globally.

## Workflow

- Read before write. Follow existing patterns; simplest clear solution wins (YAGNI).
- No new dependency without discussion.
- Before done: `cargo test --workspace`, `cargo fmt`, `cargo clippy` - all clean.
- Add or update tests in `mirufm-core` when touching logic.

## Commits and releases

- Conventional Commits: `feat:`, `fix:`, `refactor:`, `chore:`, `ci:`, `perf:`, optional scope like `feat(core):`.
- One logical change per commit; imperative, lowercase, no trailing period.
- Releases are automated with release-please (single whole-repo release, `v0.1.0` tags).
  Never hand-edit `CHANGELOG.md` or `version.txt` - release-please owns them.

## Style

- No em dashes; use plain "-". No emojis in code, commits, or docs.
- Comments explain "why", not "what"; default to no comments.
- Never commit secrets (`.env*`, keys, tokens).
