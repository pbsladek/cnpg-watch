# Build and qualify a release candidate

Package creation is local. Publishing a release, signing/notarizing an archive
or uploading binaries is a separate requested operation.

Run `make check`, `make test`, the pinned live matrix, release load/soak and quiet
TUI measurements before claiming a supported candidate. Run each live cell
serially with `E2E_MATRIX_CELL=CELL make test-tui`. Verify injected failure
cleanup with `make test-cleanup`. Preserve public receipts and redact failure
artifacts; working kubeconfigs, passwords and raw PTY output stay private.

`make package` builds the local OS/CPU package and executes its CLI and terminal
lifecycle smoke test. Linux packages require Zig 0.15.2 and cargo-zigbuild 0.24.1:

```sh
cargo install cargo-zigbuild --version 0.24.1 --locked
make cli-artifacts
make package
```

To build another target, install its Rust standard library and call
`scripts/build-release.sh TARGET`. Supported targets are listed in
[installation](installation.md). Each archive includes a binary, license,
generated completions/reference and operator guides. `manifest.json` versions
the inventory, file hashes/modes and Rust source hashes. `dist/SHA256SUMS`
covers every archive. Archive metadata and gzip timestamps are deterministic;
this does not promise reproducible compiler output across different hosts.

Execute each archive with `scripts/smoke-package.py ARCHIVE --report REPORT
--execution LABEL` on its matching platform, or use `--rosetta` for Intel macOS
on Apple Silicon. The smoke verifies the manifest, CLI output and actual binary
startup, resize/help, q, Ctrl-C and error restoration against a local API stub.
It does not establish live cluster compatibility. Record emulator details for
Linux cross-target execution. Building an archive without executing it leaves
that target unqualified.

The `checks` workflow runs fast gates on four native GitHub runner types.
`live-and-load` runs three pinned Kubernetes cells plus bounded load, scheduled
properties and a 30-minute soak. `package-candidates` builds and executes each
archive on a matching native runner and uploads workflow artifacts. These jobs
have no release publication step. A workflow file is not a completed CI run.

Before GA, require successful hosted checks for the source being released,
review generated examples and manifests, and close V01–V12 in the
[implementation guide](../implementation.md). The [Phase 7 receipt](validation/phase-7.md)
records the local candidate and any remaining qualification gate. This repository
currently has no remote, so local evidence does not claim hosted CI success.
