# Phase 7 validation

Status: implementation and local candidate qualification complete on 2026-11-15
UTC. Phase 7 code, tests, packaging and workflows are implemented. Hosted CI has not run: this
workspace has no Git remote. GA remains open until those jobs pass. Packages
are local candidates; no release has been published.

The final renderer has 120×40, 100×30 and smaller table-first layouts. It shares
headers and footers with scrollable row/log snapshots, doctor and profile views.
Navigation uses stable row identities and bounded back/forward history.
The app dispatcher has an injected-key regression without terminal/network I/O.
Twenty-two reviewed buffers cover healthy/degraded/reduced states, optional
views, full/compressed/narrow dashboards, long Unicode, empty/filtered views
and details. Properties exercise dimensions down to 0×0 and removed selections.

`make check` and `make test` passed on macOS arm64 and native Linux arm64 in
Docker. The Mac default suite passed 98 Rust cases, plus Python projection,
lifecycle and archive reproducibility checks. The 15 ignored Rust selections
are explicitly invoked live/load suites. Replay/layout properties were repeated
with 4,096 cases; snapshots were checked with updates and force-pass disabled.
The final expanded all-five-shell completion contract and current generated
reference passed separately on both platforms.

Real terminal tests use portable-pty 0.9.0 and vt100 0.18.1. The live workflow
starts the real app against CNPG, checks fresh SQL and operational views,
performs eight doctor/profile/resize cycles, filters, inspects snapshots,
traverses history, resizes down to 1×1, and injects 8,000 Unicode log lines while
checking key feedback. It exits through q and a second Ctrl-C child. The guard
suite checks normal/error/panic/Ctrl-C restoration and child/thread cleanup.
Captures and queues are bounded. Public screenshots are redacted; raw terminal
bytes and working kubeconfigs/passwords remain private and are not uploaded.

The live matrix pins k3d 5.9.0, kubectl 1.35.5, CNPG 1.30.0, PostgreSQL 18.4,
PgBouncer 1.25.1 and Kubernetes 1.34.8/1.35.5/1.36.1. Each cell runs the full
serial operations suite: setup/idempotence, actual RBAC identities, both
primaries and replica with all six enabled SQL variants, overload, promotion,
watch reconnect/replacement, optional evidence and independent reduced modes.
The receipt records each cell rather than infer compatibility from compilation.
No other PostgreSQL major, Kubernetes distribution or custom Pooler auth is
qualified. Backup/restore, TLS trust and promotion safety claims remain outside
the evidence described in the earlier operational receipts.

The 1.34 fixture allows get-only WebSocket exec; newer pinned cells require both
get and create. The catalog keeps both verbs for portable generated RBAC.
The actual get-only/create-only/both/neither outcomes are stored per cell.

Failures found during qualification were corrected and affected gates repeated:

- portable-pty starts commands from the home directory and resolves relative
  executable names through PATH. The driver now sets the project cwd and uses
  an absolute lifecycle executable path.
- The new Criterion draw closure initially returned a borrowed completed frame.
  It now discards that return value inside the iteration; all-target clippy
  checks cover the benchmark.
- A shared Docker VM crossed kubelet's percentage disk floor and evicted the
  fixture. Disposable nodes now have an explicit 1 GiB floor and 99/98% image
  GC policy, recorded in each run receipt. Existing workloads/images were not
  changed or pruned.
- The installer initially expected bare k3d checksum filenames; upstream uses
  `_dist/`. It now compares basenames. Replacing a macOS executable in place
  interrupted a fixture helper, so installation now verifies hashes and swaps
  in a new inode atomically, then executes both installed tools. The interrupted
  1.35 run does not qualify that cell.
- Local HTTP/RSS checks require host permissions unavailable in the sandbox;
  the relevant checks were repeated with those permissions. Missing RSS is
  now an explicit load-test failure, so CI cannot silently skip the memory gate.
- Debian's Python 3.11 predates tar extraction filters. Package smoke now
  accepts only regular files within its normalized temporary root, with
  traversal/link regression cases, rather than requiring a newer Python.

Release load uses 100 clusters, 300 instances, two Pooler pods, 200 operational
resources, 1,000 topology updates/s and 5,120 log lines/s across eight sources.
It exercises production framing, redaction, retention, timeline projection,
health, queued/admitted/reduced Pooler samples and the responsive renderer in a
120×40 TestBackend. Scope churn repeats every ten seconds. These are synthetic
pipeline measurements; they do not establish Kubernetes API or terminal-device
throughput, or simultaneous SQL collection from all 300 instances.

| Measurement | Result |
| --- | --- |
| macOS final 60 seconds | Input p95/p99 96.650/100.504 ms; prepare/draw p95 2.973 ms; RSS peak 21,984 KiB |
| Native Linux final 60 seconds | Input p95/p99 97.249/100.421 ms; prepare/draw p95 3.613 ms; RSS peak 14,872 KiB |
| macOS 30-minute soak | Input p95/p99 96.892/100.013 ms; prepare/draw p95 3.035 ms; RSS peak 23,344 KiB |
| Actual minimal release TUI idle | 0.298% of one core; 21,280 KiB RSS |
| Actual standard release TUI idle | Fresh SQL and native Pooler sample verified; 0.498% of one core; 22,432 KiB RSS |
| Bounds in all runs | Log high-water 8,388,120 B; cache 362,714 B; queue 541,184 B; zero queue drops or remaining tasks |

Each short run accepted 307,200 log lines, completed 24 Pooler samples and
cycled scope five times. The soak accepted 9,216,000 lines, completed 720 Pooler
samples and cycled scope 179 times. Its 360 RSS samples end in a 20,448–20,704 KiB
range over the last 60 samples. Retention evicts old records, so zero queue drops
does not mean every log line stays resident. The soak predates final doctor
scrolling, dispatcher-test additions and the stricter missing-RSS assertion;
these do not change its workload, and all soak RSS observations are present.
The final short runs repeat the production pipeline after cleanup.

The host is an Apple M5 Pro, 18 CPUs, 64 GiB RAM, macOS 27.2.1; Linux runs
natively as arm64 inside Docker's VM. Linux fast/load gates use the GNU target;
musl archives have separate runtime smoke results. Rust is pinned to 1.94.1. Short Criterion
runs (10 samples, 0.2 s warm-up, 0.5 s measurement) characterize the shared host:
full/compressed dashboard draw about 59.5/42.2 µs, Pooler CSV 12.8 µs, full
operational health at 100 clusters 138.5 µs, timeline projection 546.5 µs and
Cluster adaptation 1.009 µs. Earlier unchanged paths are broadly comparable;
the short shared-host measurements do not justify small regression claims.
The supported synthetic scale remains 100 clusters; 1,000-cluster benchmarks
remain characterization.

The Phase 5/6 final short run measured input p95 96.710 ms, frame work p95
3.057 ms and peak RSS 22,384 KiB. Current latency and retention remain within
the same budgets after adding the responsive renderer. The rendering path and
host contention changed, so these runs do not prove a precise speedup.

Four candidate archives contain checksums, versioned manifests, five generated
completions, a generated command reference, license and operator documentation.
The package smoke executes CLI output and actual startup/resize/help/q/Ctrl-C/
error restoration against a local API stub, independently of live compatibility.
Manifest hashes, modes and the binary's reported OS/architecture must agree.
Deterministic packaging tests compare complete archive bytes and metadata.

| Target | Local runtime execution |
| --- | --- |
| macOS arm64 | Native |
| macOS Intel | Rosetta on Apple Silicon |
| Linux arm64 musl | Native Docker Linux VM |
| Linux x86_64 musl | Docker/QEMU emulation |

The CI definitions select four matching native runner types for fast gates and
package execution, a serial three-cell live matrix, scheduled 4,096-case
properties, bounded load and a 30-minute soak. Public success receipts and
redacted failure diagnostics have separate upload allowlists. Injected failure
cleanup deliberately exits 42, retains diagnostics and proves its own fixture
was deleted. Local evidence does not claim hosted or native Intel CI execution.

Qualification commands:

```sh
make check
make test
PROPTEST_CASES=4096 INSTA_UPDATE=no INSTA_FORCE_PASS=0 \
  cargo test --locked --test replay --test terminal_layout
scripts/check-linux-native.sh
scripts/install-e2e-tools.sh
python3 scripts/check-cleanup.py
E2E_MATRIX_CELL=k8s134 make test-live E2E_SUITE=operations
E2E_MATRIX_CELL=k8s135 E2E_KEEP_CLUSTER=1 E2E_MEASURE_IDLE=1 \
  CNPG_WATCH_IDLE_PROFILE=standard CNPG_WATCH_IDLE_POOLER_STATS=1 \
  make test-live E2E_SUITE=operations
E2E_MATRIX_CELL=k8s136 make test-live E2E_SUITE=operations
CNPG_WATCH_LOAD_SECONDS=60 CNPG_WATCH_LOAD_REPORT=artifacts/phase7/load.json \
  cargo test --release --locked --test load -- --ignored --nocapture
CNPG_WATCH_LOAD_SECONDS=1800 CNPG_WATCH_LOAD_REPORT=artifacts/phase7/soak.json \
  cargo test --release --locked --test load -- --ignored --nocapture
cargo bench --locked --bench pipeline -- --sample-size 10 \
  --warm-up-time 0.2 --measurement-time 0.5
make package
```

The 1.35 cell was explicitly retained after passing its tests to run
`scripts/measure-idle.py` with standard/Pooler and minimal settings, then deleted.
The other final cells use automatic cleanup. Cross-target builds use
`scripts/build-release.sh` followed by the package smoke command described in
the [release procedure](../releasing.md). Working output remains under ignored
`artifacts/phase7/`, `artifacts/linux-native/` and `artifacts/e2e/`; candidate
archives and `SHA256SUMS` remain under ignored `dist/`.

Public machine-readable evidence:

- [macOS final load](phase-7-load-macos.json), [native Linux final load](phase-7-load-linux.json), and [macOS soak](phase-7-soak.json).
- [Minimal idle](phase-7-idle-minimal.json) and [standard/Pooler idle](phase-7-idle-pooler.json).
- [Criterion means in nanoseconds](phase-7-benchmark-nanoseconds.json).
- [Four executed packages and archive hashes](phase-7-packages.json).
- [Three complete live cells, real exec/PTY outcomes and cleanup](phase-7-live.json).

Each live cell passed 14 Rust cases, including the optional capture hook and
two terminal selections. All six measured flood feedback responses in every
cell stayed below 101 ms. All owned clusters were removed, and the original
eight Docker workloads remained running. The stricter RSS gate passed on both
platforms. [Final source hashes and toolchain](phase-7-source-sha256.json) identify
the candidate inputs; the soak provenance above records its earlier source
boundary. See the [development guide](../development.md) for artifact and
failure-handling details.
