# Phase 4 validation

Status: complete, locally validated on 2026-11-15 UTC. Next task: P5.1.

The probe implementation covers P4.1–P4.6. Live qualification is PostgreSQL
18.4 / CNPG 1.30.0 on the isolated Kubernetes fixture described in the
[Phase 3 receipt](phase-3.md). Registry major-version metadata does not claim
live coverage of other PostgreSQL 18 point releases or other majors.

The registry ships capabilities, fast pressure, fast-basic fallback, medium,
deep, and explicit-query-text deep variants. Each has a schema, version bounds,
required columns/functions/grants, row bounds, timeout, output cap, and redaction
policy. Actual view/column/function detection gates dependent work. Server/pod,
system/timeline/role, scope/topology and request stamps fence stale results.

Medium samples provide bounded slots, synchronous replication peers, and nine
selected settings. Incident Connections/Waits views activate bounded deep
pressure, waits, blockers and long transactions. Query text requires explicit
incident opt-in; it is always excluded from dumps. Fast byte lag requires fresh,
compatible, uniquely associated peers. Archive deltas reject resets and
incompatible identity/timeline epochs. Transaction age alone is never a lag alarm.

The scheduler coalesces bounded intents, rotates pods fairly within weighted
classes, uses completion-based cadence and monotonic backoff, and cannot burst
after a runtime stall. All classes use the existing shared exec permit budget.
Authentication suspends until explicit recheck. Overload sheds deep first and
slows medium, preserving fast work and last-good evidence. Independent topology
and selected logs remain running. Only accepted fresh selected-pod pressure
samples affect policy; a quiet replica cannot erase primary pressure.

`make check` and `make test` cover parsers, gates/fallback, malformed and oversized
outputs, identity/reset behavior, class admission, no catch-up bursts, and actual
resource shapes. Paused-time tests use seeded jitter and no wall-time sleeps.
`tests/probes_live.rs` executes all six variants as the intended monitor role on
payments primary/replica and inventory primary. The overload test holds at least
20 real monitor connections, checks shedding and retained samples, then requests
an actual CNPG switchover and accepts fresh samples in the new topology epoch.
Prior live transport tests retain real wrong-password, remote-status, capped
output, statement deadline, cancellation/backend cleanup and replica replacement
contracts. Setup and runtime identities remain separate.

The fresh reproducible gate is:

```sh
E2E_MEASURE_IDLE=1 make test-live E2E_SUITE=fleet
```

The completed run used `E2E_KEEP_CLUSTER=1` temporarily for terminal diagnosis;
the fixture was explicitly deleted after success. All nine selected Rust live
cases, both app PTY workflows, and release idle passed. The earlier investigation
fixture was also deleted; unrelated Docker workloads were preserved.

It provisions its own context, runs permission/setup, fleet, all SQL variants,
overload/promotion, prior live transport, both real TUI workflows, release idle,
and deletes its own cluster. `E2E_SUITE=probes` runs the same complete fixture gate.
GitHub Actions now selects this fleet gate; remote CI was not run here.

Measurements are kept separately:

- `phase-34-probe-costs.json`: actual full bundle round trips, test-only database
  EXPLAIN execution/planning, exec-only transport baseline, and a full
  psql-startup/connect/SELECT-1 round trip. The last two are separate baselines;
  their difference is not a precise isolated process-startup measurement.
- `phase-34-scheduler-live.json`: observed class duration/sweep counters from the
  saturation/promotion session and shutdown duration/errors. Cadence includes
  jitter, admission, class gating and failures; it is not a promised sample age.
- `phase-34-load.json`: final-source 60-second release workload with process RSS.
- `phase-34-soak.json` and `phase-34-soak-rss-observer.json`: 30-minute production
  log/health workload and separate process RSS observations.
- `phase-34-idle.json`: actual release TUI with quiet live watches.
- [Criterion means in nanoseconds](phase-34-benchmark-nanoseconds.json): short
  10/100/1,000-cluster characterization, ten samples, 0.2 s warm-up and 0.5 s
  measurement. These runs shared a Mac with fixture/soak activity and do not
  establish small percentage regression claims.

| Measurement | Local result |
| --- | --- |
| All six variants on three servers | 18 successful full production bundle executions; full round trips 32.36–244.14 ms, including API admission/transport and psql |
| DB-side EXPLAIN | Maximum execution 0.621 ms in the pinned small fixture; planning and individual samples are recorded separately |
| Saturation/promotion | 26 held monitor clients; retained evidence and independent logs/topology; promotion recovery; shutdown 1.75 ms, zero cleanup errors |
| 60 s final workload | 307,200 accepted log lines; input p95/p99 96.200/99.074 ms; view/render p95 0.755 ms; RSS peak 23,664 KiB; zero queue drops/remaining tasks |
| 30 min soak | 9,216,000 accepted log lines; 179 scope changes; input p95/p99 96.222/100.135 ms; view/render p95 0.765 ms; RSS peak 25,888 KiB across 360 samples; zero queue drops/remaining tasks |
| Actual release TUI idle | 0.200% of one CPU core over 10.01 s after warm-up; 18,464 KiB RSS; quiet live watches in minimal profile |
| Retention bounds under load/soak | Log high-water 8,386,560 bytes; normalized cache 203,600 bytes; queued high-water 335,872 bytes |


At 100 clusters, full health recomputation is about 94 µs; at 1,000 it is about
1.17 ms. Unchanged health requests reuse the memoized result. Capability parsing
is about 5.4 µs, class admission about 0.58 µs, and a representative short
frame/parse/redact path about 0.59 µs. Rendering benchmarks use TestBackend and
the instance table; real multi-view behavior is covered by PTY rather than a
claim of live fleet rendering throughput.

The 30-minute soak executable started while implementation continued. The final
60-second run repeats the workload after subsequent code cleanup. The soak uses
production framing/redaction/retention and health with synthetic producers and
an in-memory terminal; it does not establish live Kubernetes log throughput.
Internal RSS sampling covers the run; a separate observer also cross-checks
the latter part. The final 60-second run records RSS independently. Retention evicts old records: no queued drops does not mean every
line remains resident.

See [ADR 0014](../adr/0014-capability-gated-probe-classes.md). Phase 5 begins with
PgBouncer capabilities/admin statistics; no runtime mutating actions, user table
scans, broader major-version support, or release compatibility claims were added.


The [source SHA-256 manifest](phase-34-source-sha256.json) identifies the final
implementation, fixtures, scripts, and documentation. Private working captures,
credentials and full logs remain excluded from these compact receipts.
