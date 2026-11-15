# Phase 1 validation

Status: complete for the first live slice on the local macOS arm64 worktree,
2026-11-15 UTC. Next task: P2.1. This receipt describes the implemented slice,
not GA or additional compatibility claims.

## Environment and delivered path

- Rust/cargo 1.94.1, edition 2024; locked dependency graph.
- kube 5.0.0 with Rustls/ring; Kubernetes 1.35 typed API; ratatui 0.30.2.
- k3d 5.9.0 / k3s v1.35.5-k3s1, CNPG 1.30.0, PostgreSQL 18.4.
- Separate `cnpg-system` and `cnpg-watch-e2e` namespaces; one primary/replica.
- A dedicated non-superuser `watch_monitor` role with `pg_monitor`, an explicit
  protected local password file, and no administrative runtime fallback.

Cluster/Pod watches normalize into typed identities and feed the bounded
single-writer reducer, complete relists, selected-instance scheduler, and TUI.
Scope/watch/topology epochs, UIDs, request IDs, independent last-good/attempt
state, monotonic deadlines, and owned cancellation protect lifecycle behavior.

## Validation run

| Check | Evidence |
| --- | --- |
| Fast checks | `make check` and `make test` passed: 32 Rust tests plus the POSIX PTY lifecycle check; live/load tests are explicitly ignored in the fast run |
| Replays/properties | Same-name replacement, late result/delete, old scope/watch epochs, interrupted/complete relists, membership deduplication, failed last-good preservation, freshness without input, LSN round trips/overflow, and incremental/full view agreement |
| Scheduler | Paused-time clock, exact eligibility, bounded exponential backoff, auth suspension/rechecks, per-cluster/pod admission, manual refresh spacing/coalescing, and no catch-up burst |
| Boundaries | Byte/count queues, full-queue cancellation/permit reclamation, capped output, malformed/oversized watch normalization, and explicit capacity errors |
| Live transport | Shipped bundle ran as the monitor role; actual role flags were inspected; bad credentials, nonzero remote exit with JSON stdout, output caps, timeout, and cancellation were exercised |
| Remote cleanup | A separate test-only admin observer verified no remaining `application_name=cnpg-watch` backend after timeout/cancellation |
| Assembled session | Both instances sampled; deleting a replica produced a new UID without inheriting old samples; 30 s watch timeouts reconnected; SQL-disabled topology remained useful; task cleanup returned zero errors |
| Real TUI | `tests/app_smoke.py` saw instance names/fresh SQL, opened help, and verified q/Ctrl-C and terminal restoration |
| Reproducibility | `make test-live` provisioned a fresh uniquely named fixture, ran both live tests and real TUI smoke, and deleted its own cluster |
| Fixtures | Pinned schema reference, anonymized live Cluster/Pod captures, real monitor JSON, and a stepwise protocol replay are checked in |

Final live transport/session commands used the isolated kubeconfig and context
with `CNPG_WATCH_E2E=1`; they never used the default context. Initial monitor
bundles took approximately 45–47 ms. The final assembled run recorded 39 samples,
a 2,778-byte normalized/sample cache, a 2,864-byte observed queue high-water mark,
and two watch tasks before clean shutdown. Those sizes describe the small live
fixture, not an upper bound for larger clusters.

## Performance baseline

Release builds, macOS 27.2.1 arm64. Workload metadata and local JSON reports
are retained under ignored `artifacts/phase1/`; Criterion output is under
`target/criterion/`. These characterize this machine and scenario.

| Scenario | Measured result |
| --- | --- |
| Final 60 s synthetic load | 100 clusters/300 instances; 1,000 topology updates/s; 5,120 log lines/s across eight retained sources; TestBackend 120x40 |
| Input-to-render latency | p95 97.012 ms; p99 100.372 ms |
| View preparation/render | p95 0.342 ms |
| Peak RSS | 13,936 KiB; normalized cache high-water 130,000 bytes; queued high-water 233,472 bytes |
| Load work/cleanup | 307,200 accepted log lines, five scope churns, no queue drops, no remaining producer tasks |
| 30 min synthetic soak | 9,216,000 accepted log lines, 179 scope churns, no queue drops, no remaining producer tasks |
| Soak latency/render | p95 input 96.719 ms; p99 100.522 ms; p95 view/render 0.348 ms |
| Soak memory | Peak RSS 14,672 KiB; normalized cache and queue high-water stayed 130,000 / 233,472 bytes |
| Actual release TUI, quiescent | Minimal profile, one cluster/two instances: 0.10% of one CPU core over 10 s after warm-up; 14,576 KiB RSS |

The soak ran a release test executable while implementation continued. The
final-source 60 s run repeated the workload after subsequent validation/UI
changes. Synthetic load uses bounded fake log/transport traffic and an in-memory
terminal; the separate live/PTY and idle checks cover the assembled application.
The fixture does not establish full live-fleet throughput or a GA scale claim.

The soak's RSS grew from about 13.6 to 14.3 MiB after log warm-up, including its
per-run latency/frame sample vectors. Source retention and normalized/queued
byte counters remained bounded rather than increasing with visited scopes.
Older retained log entries are evicted; “no queue drops” does not mean that all
nine million lines remain in memory. This is an initial baseline, not proof of
zero growth under every future collector.

Criterion's first short baseline used 10/100/1,000-cluster fixtures (three
instances each), 0.2 s warm-up, 0.5 s measurement, and ten samples. Approximate
100-cluster means: unchanged reduction 85 ns, selected view preparation 2.86 µs,
120x40 buffer rendering 47.4 µs. At 1,000 clusters, view preparation was about
25.4 µs. This is characterization; repeat controlled comparisons before making
small regression claims.

## Decisions and limits

- The live fixture rejected the proposed `cnpg_watch` managed role as reserved.
  Default role is now `watch_monitor`; design, fixture, and ADR 0001 agree.
- The accepted transport sends credentials on exec stdin, uses required TLS on
  loopback, and validates remote status. Default 750 ms client deadline includes
  cleanup; statement timeout is 375 ms with the final defaults, capped at 500 ms;
  lock timeout is at most 100 ms; connect bound is 2 s.
- Only PostgreSQL 18.4/CNPG 1.30.0/k3s 1.35.5 were validated live. Extra server,
  OS, Kubernetes permission matrices, doctor/setup, fleet views, actual log
  collectors, poolers, and deeper SQL remain their planned phases.
- GitHub Actions definitions are ready but no remote CI run or publish was
  performed. Large private working artifacts/kubeconfigs/passwords are ignored
  and excluded from uploaded diagnostics. Checked-in fixtures are anonymized.

No unresolved blocker remains for the implemented Phase 1 slice. Broader fleet,
operational, and release qualification remain later gates.
