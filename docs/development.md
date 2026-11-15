# Develop and qualify changes

Use the pinned Rust 1.94.1 toolchain and Python 3.11+. `make check` runs
formatting, clippy on all targets and a locked build. `make test` needs no
cluster or credentials: it runs
Rust tests, reviewed terminal snapshots, lifecycle checks and deterministic
package tests. It never accepts changed snapshots automatically.

Rendering lives in `src/screens/render.rs`. It consumes prepared values and
performs no I/O. Navigation histories and selection reconciliation are pure;
the app dispatcher owns key handling and session transitions. Keep external
work in collectors and the session, with typed results, bounded queues and
generation checks before reduction. Read the [design](../design.md) before
changing budgets, permissions or evidence semantics.

Review buffer changes with `cargo insta review` if installed, or inspect the
`.snap.new` files and promote only the intended differences. Generate candidates
with `INSTA_UPDATE=new cargo test --test terminal_layout`. CI uses
`INSTA_UPDATE=no INSTA_FORCE_PASS=0`. The property suite covers zero-sized
rectangles, Unicode, changing counts and removed selections. For longer runs:

```sh
PROPTEST_CASES=4096 cargo test --locked --test replay --test terminal_layout
```

`make test-terminal` builds the lifecycle example and runs portable-pty/vt100
checks for normal exit, Ctrl-C, error and panic restoration. `make test-tui`
creates a fresh disposable Kubernetes fixture and runs the full serial live
suite, including the real app, semantic screen waits, resize/navigation cycles
and an 8,000-line Unicode log flood. Install Docker and the checksum-verified
tools with `scripts/install-e2e-tools.sh` first. Select `E2E_MATRIX_CELL=k8s134`,
`k8s135` or `k8s136`; the default is `k8s135`.

Fixtures use separate kubeconfigs and names beginning `cnpg-watch-e2e-`. Cleanup
only touches that fixture. `make test-cleanup` deliberately fails a fresh fixture
with exit 42 and verifies diagnostics and deletion. `E2E_KEEP_CLUSTER=1` is an
explicit debugging option; delete that named fixture when finished. Raw PTY
captures and working credentials are private and excluded from CI uploads.
Public receipts and redacted diagnostics live under `artifacts/e2e/`.

The disposable node uses an absolute 1 GiB disk eviction floor and 99/98% image
GC thresholds because a shared Docker VM can cross Kubernetes' percentage
defaults without the fixture consuming significant disk. This applies only to
new test nodes and is recorded in `run.json`. Do not prune unrelated images or
change existing workloads to make a gate pass.

`make test-load` and `make test-soak` run the production preparation/renderer
pipeline in release mode for 60 seconds and 30 minutes. They measure a defined
synthetic fleet, not external API capacity. `make bench` measures individual
paths. Inspect counters, task cleanup and memory samples as well as latency.
Record noisy shared-host measurements as characterization. See the
[Phase 7 receipt](validation/phase-7.md) for the measured support boundary.

`scripts/check-linux-native.sh` runs the offline gates in a native Docker Linux
container with read-only sources and registry, task-owned output directories,
two build jobs and no runtime network. It needs Docker and a populated Cargo
registry. It does not substitute for Intel or hosted CI execution.

Regenerate checked-in completions and command documentation with
`make cli-artifacts`. Package and qualify changes with the
[release workflow](releasing.md). Keep validation receipts explicit about source,
platform, native/emulated execution, failures and unrun gates.
