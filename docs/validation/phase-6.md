# Phase 6 validation

Status: complete, locally validated on 2026-11-15 UTC. Alpha and Beta checkpoints
are locally qualified. Next task: P7.1.

P6.1–P6.9 add operational resources, generation-aware health, optional public
certificate/metrics collection and correlated timelines. The qualified live
combination is CNPG 1.30.0 / PostgreSQL 18.4 / PgBouncer 1.25.1, Kubernetes
v1.35.5+k3s1 with k3d v5.9.0. This is local feature qualification; broader
compatibility, packages and release artifacts remain Phase 7.

`tests/operational_live.rs` observes controller-produced applied generations for
Database, DatabaseRole and Publication; a real failed unavailable-plugin Backup;
a suspended ScheduledBackup; namespaced/global catalogs; CNPG webhook
configuration and current operator service/endpoints; an explicitly named public
certificate; and the operator's explicitly forwarded metrics endpoint. It checks
actual `databaseroles`, `subscriptions` and `failoverquorums` discovery endpoints.
The latter two lists are empty in this fixture. No real subscription, quorum
transition, successful physical backup, restore or PITR validation is claimed.
Compact public resource/discovery captures are under `fixtures/cnpg/1.30.0/`.

The second live case uses a generated runtime ServiceAccount with selected grants
removed. It proves denied named Secret, DatabaseRole, global webhook, operator
logs and pooler exec remain local failures while direct instance SQL, topology
and permitted Backup/Pooler watches remain useful. Scope/profile replacement
closes streams and clears optional samples. The earlier suites retain real
reconnect, pod replacement, promotion, timeout/status, credential and setup proof.

Fast coverage is split by boundary:

- Operational tests cover captured schemas, malformed optional Cluster fields,
  current/pending generation, historical backup failure/recovery ordering,
  webhook scope/readiness, stale event UIDs and incremental/full health agreement.
- Auxiliary tests cover exact named GETs, default/minimal gating, discarded
  private fields, expiry/rotation metadata and quiet-clock health invalidation;
  metrics test finite values, label omission, malformed/capped input, isolated
  HTTP headers, redirects, denial, deadlines, cancellation and scheduler cadence.
- Timeline tests cover structured context, namespace/UID mismatch, reconcile
  windows, timestamp disorder, unmatched/full operator access, source toggles,
  global pause checkpoints, immutable event updates, eviction, old generations,
  bounded indexes and malformed-JSON redaction before retention.
- Existing replay/watch/scheduler tests continue to cover complete/interrupted
  relists, stale results, deadlines and bounded queues/tasks. Global webhook
  normalization additionally tests cluster-scoped deletes and irrelevant rules.

Final commands:

```sh
make check
make test
make test-live E2E_SUITE=operations
CNPG_WATCH_LOAD_SECONDS=60 CNPG_WATCH_LOAD_REPORT=artifacts/phase56/load.json \
  cargo test --release --locked --test load -- --ignored --nocapture
CNPG_WATCH_LOAD_SECONDS=1800 CNPG_WATCH_LOAD_REPORT=artifacts/phase56/soak.json \
  cargo test --release --locked --test load -- --ignored --nocapture
cargo bench --locked --bench pipeline -- --sample-size 10 \
  --warm-up-time 0.2 --measurement-time 0.5
```

The fresh live wrapper selects twelve Rust cases (eleven live behaviors and an
optional capture hook) and three actual TUI workflows
and removes its fixture. Earlier exploratory fixture failures exposed an obsolete
RBAC expectation and a ten-second profile startup deadline that did not cover
the expanded watch set sharing five API requests/second. The bounded profile
startup test now allows 45 seconds and reports source states on failure. Gates
were repeated after implementation changes. No production API rate or probe
deadline was increased to pass those tests. A later PTY failure exposed a real
input-reader issue: navigation tracing showed a missing key beside resize
readiness, before the actor received it. Selecting Crossterm's supported
`use-dev-tty` reader fixed the repeated real-app regression. Temporary tracing
was removed; the final gate includes sixteen doctor/profile/escape cycles.
See ADR 0009.

The corrected release workload contains 100 clusters, 300 instances, two pooler
pods, 200 backup/managed observations, 1,000 topology updates/second and 5,120
log lines/second. It uses production framing/redaction/retention, timeline
projection, operational health, Pooler admission, a bounded completion queue and
sample reduction, plus an in-memory 120×40 terminal. Scope churn repeats every
ten seconds and restores every source; an assertion requires continuing Pooler
completions. A superseded run was stopped after detecting a harness omission in
source restoration. Only the corrected complete runs qualify this gate.

The soak started while final doctor/health validation continued; its source
predates those small additions and the Unix input-reader feature selection.
The synthetic workload does not use an OS terminal input reader. The final 60-second run repeats the workload
after cleanup. These synthetic measurements do not establish live Kubernetes
throughput, a real terminal frame rate, or database query cost. Retention evicts
old records; zero queue drops does not mean every log line remains resident.

Short Criterion runs characterize costs rather than prove small regressions on
this shared Mac: Pooler CSV parsing is approximately 12.1 µs; full operational
health for 100 clusters is 130 µs; projecting eight sources × 512 retained entries
is 515 µs. Cluster adaptation increased from the earlier core-only path to about
0.97 µs as it now decodes operational context. Unchanged health remains cached.
The machine-readable means retain all measured paths, in nanoseconds.

Actual quiet release TUI measurements cover minimal profile and standard profile
with Pooler statistics enabled. Optional certificate/metrics paths remain lazy
and disabled unless configured. Results, high-water marks and source hashes are
stored in the accompanying `phase-56-*.json` artifacts. All final checks passed:
91 fast Rust tests, three terminal projection tests, four restoration cases,
the complete fresh-cluster suite and all three real TUI workflows. Both the fresh
fixture and retained investigation fixture were deleted; the original eight
Docker workloads remained running. Remote CI was not run here.

Alpha covers phase 5 plus backups, storage/services and stronger operator health.
Beta additionally covers the managed resources, topology/plugins, optional
webhook/certificate/metrics evidence, correlated logs and reduced-mode/dump
contracts. V01–V11 pass within this local qualification boundary. These completed checkpoints
do not substitute for Phase 7 release qualification.

| Measurement | Local result |
| --- | --- |
| Final 60-second workload | 307,200 lines; 24 Pooler completions; 5 scope changes; input p95/p99 96.710/100.323 ms; frame work p95 3.057 ms; RSS peak 22,384 KiB |
| Corrected 30-minute soak | 9,216,000 lines; 720 Pooler completions; 179 scope changes; input p95/p99 96.623/99.768 ms; frame work p95 2.982 ms; RSS peak 24,064 KiB across 359 samples |
| Bounds in both runs | Log high-water 8,388,120 B; normalized cache 362,714 B; queued high-water 541,184 B; zero queue drops or remaining tasks |
| Actual minimal TUI idle | 0.298% of one core; 18,832 KiB RSS |
| Actual standard TUI idle | Fresh SQL and native Pooler sample verified; 0.497% of one core; 22,880 KiB RSS |

Machine-readable evidence:

- [Final load](phase-56-load.json) and [corrected soak](phase-56-soak.json).
- [Minimal idle](phase-56-idle-minimal.json) and [Pooler-enabled idle](phase-56-idle-pooler.json).
- [Criterion means in nanoseconds](phase-56-benchmark-nanoseconds.json).
- [Live qualification summary](phase-56-live.json) and [final source hashes](phase-56-source-sha256.json).
