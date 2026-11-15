# Phase 0 validation

Status: complete on the local macOS arm64 worktree, 2026-11-15 UTC.
Next task: Phase 1 was completed in the same implementation; next work is P2.1.

## Delivered

Rust 1.94.1 / edition 2024 package and lockfile; thin binary/library boundary;
CLI/config/environment precedence; versioned local info; guarded terminal;
private bounded watch logging; and Linux/macOS CI definitions. Doctor/setup
return unfinished errors and never perform a mutation.

## Checks run

- `make check`: formatting, all-target clippy with warnings denied, and locked
  build passed.
- `make test`: fast tests and the real terminal ownership check passed.
- `cargo build --release --locked`: passed.
- CLI contracts prove clean JSON stdout, null undetected live values,
  help/invalid flags, file/environment/CLI precedence, explicit all-namespace
  overrides, extreme/contradictory budget rejection, and unfinished setup.
- The PTY helper entered actual raw mode/alternate screen, then verified cooked
  termios and alternate-screen restoration after q, Ctrl-C, error, and panic.
  The real assembled application also passed q/Ctrl-C/help smoke in Phase 1.
- Config parsing suppresses source contents in errors; output never includes
  the password. The private diagnostic writer is capped at 1 MiB.

## Limits

These commands ran on macOS 27.2.1 arm64. Linux/macOS GitHub Actions jobs are
provided but were not executed from this local session. No package publishing
or Git commit was performed; source, docs, and lockfile are reviewable worktree
changes. Runtime target qualification beyond this host remains a later gate.
