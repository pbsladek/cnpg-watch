# Phase 2 validation

Status: complete on the local macOS arm64 worktree, 2026-11-15 UTC.
Next task: P3.1. This qualifies the implemented preflight/bootstrap slice on the
pinned fixture; it does not expand GA or compatibility claims.

## Delivered

- Shared scope-aware feature catalog, four profile envelopes, conservative
  refresh defaults, and explicit unfinished/disabled states for later features.
- Doctor text/JSON schema 1: SSAR plus bounded safe live fallback, direct CNPG
  visibility, operator candidates, actual monitor authentication/SQL validation,
  grouped findings, typed probe failures, budgets, and copyable review commands.
- Deterministic offline RBAC/SQL with checked-in examples; live non-mutating
  diffs; explicit apply; opt-in pruning and password rotation/preservation.
- Managed RBAC ownership and resource-version checks, semantic rule comparison,
  ServiceAccount default normalization, and independent operator/cluster scopes.
- Transactional PostgreSQL role setup with explicit administrative credentials,
  effective inherited grant validation, repeat-apply DDL suppression, and optional
  production monitor authentication after apply.
- TUI capability/profile panels and reduced-mode counts, with epoch fencing and
  owned cancellation when profiles change. No runtime bootstrap/action path.
- Updated [command guide](../setup.md), ADR 0012, design, phase tracker, and live
  CI entry point. Remote CI itself was not run from this session.

## Commands and evidence

| Check | Result |
| --- | --- |
| `make check` | Format, all-target clippy with warnings denied, locked build passed |
| `make test` | 48 fast Rust tests passed; four PTY terminal-restoration cases passed; four live tests and one load test remain explicitly ignored in the fast run |
| Offline contracts | Golden namespace/operator/optional-cluster YAML and monitor/reserved/actions SQL; no kubeconfig needed; explicit subjects/scopes/modes; no credential echo; profile defaults and overrides; secure path redaction |
| Mock API contracts | Diff sends GET only; repeat matching apply sends no writes; extras preserved unless pruned; unmanaged collisions fail before writes; empty apiGroup defaulting is equivalent; SSAR failure uses safe reads and preserves unprovable access as unknown |
| UI contracts | 120x40 and 100x30 capability buffers plus narrow sizes; an unavailable SQL attempt preserves last-good evidence; config-disabled operator state stays explicit |
| Fresh live wrapper | `CNPG_WATCH_PROFILE=minimal CNPG_WATCH_NAMESPACE=should-not-be-used E2E_MEASURE_IDLE=1 make test-live E2E_SUITE=permissions` passed; fixture configuration was isolated from those user overrides; its cluster was deleted |
| Release load | 60 s synthetic pipeline passed; compact [JSON report](phase-2-load.json) |
| Criterion | 10/100/1,000-cluster characterization including the new capability calculation; compact [mean nanoseconds](phase-2-benchmark-nanoseconds.json) |
| Release TUI idle | Actual TUI passed CPU/RSS bounds; compact [JSON report](phase-2-idle.json) |

The final fresh fixture used k3d 5.9.0, k3s 1.35.5-k3s1, CNPG 1.30.0,
PostgreSQL 18.4, and kube 5.0.0 under Rust 1.94.1. The operator and database
namespaces were separate. Test observers/provisioning retained their own admin
identity; runtime permission tests used real bearer-token ServiceAccounts.

Actual requests proved:

- Generated database/operator Roles and bindings work; a second apply reports
  no changes. Namespaced cluster/pod list and watch, separate operator deployment
  reads, and production monitor exec succeed.
- Runtime identities cannot list Secrets, CRD objects, webhook configurations,
  or cluster image catalogs without optional global grants. A database-only
  identity retains useful topology and SQL without operator access.
- Temporarily denying discovery in the disposable fixture makes the discovery
  request fail while direct CNPG/SQL preflight still succeeds. The original
  discovery binding is restored before asserting the result.
- Get-only exec fails; create-only exec fails; both succeed; neither fails.
  These are real WebSocket exec requests on the pinned Kubernetes combination.
- Standard lacks pod-log access. Incident generation adds it and a bounded live
  log read succeeds. This proves permission planning, not a production collector.
- Separate/same operator namespaces, empty operator scope, forbidden discovery,
  and multiple candidate deployments are represented without guessing.
- Monitor role creation/rotation plus `--validate` authenticates as that new
  non-superuser and runs the production bundle. Repeat preservation leaves the
  stored verifier unchanged and reports no DDL changes. Explicit rotation changes
  it. Diffs do not alter the role. No credential appears in command output.
- Monitor, reserved-slot monitor, and separate NOLOGIN actions grants remain
  distinct. Extra direct membership survives ordinary apply and is removed with
  explicit prune. A pg_monitor membership changed to INHERIT FALSE is detected
  by diff, repaired by apply, and then passes actual monitor authentication.
- Switching an assembled session to minimal advances the epoch and stops SQL;
  switching back resumes SQL; shutdown returns zero cleanup errors. The real TUI
  opens both panels, switches profiles, opens help, and restores on q/Ctrl-C.

The existing Phase 1 transport/session tests also passed in this fresh run:
remote status/output failures, invalid credentials, timeout/cancel backend
termination, replica replacement, watch reconnect, and SQL-disabled topology.
Its initial fast bundle took about 47 ms; the assembled run recorded 38 samples,
2,778 cache bytes, 2,864 queued high-water bytes, and two watch tasks before
clean shutdown. The operator was not the runtime authentication identity.

## Performance and interpretation

Final 60 s release synthetic workload: 100 clusters/300 instances, 1,000 topology
updates/s, 5,120 accepted log lines/s across eight retained sources, 120x40
TestBackend. Input-to-render p95 was 97.371 ms; p99 100.280 ms; view/render p95
0.340 ms. Peak RSS was 14,032 KiB; cache/queue high-water marks were
130,000/233,472 bytes. It accepted 307,200 lines, made five scope changes,
dropped no queued lines, and left zero producer tasks. Retention still evicts
old lines; acceptance is not an assertion that every line remains in memory.

The actual release TUI, minimal profile with one cluster/two instances and quiet
watches, used 0.199% of one core over 10.04 s after warm-up, with 15,088 KiB RSS.
This measures the application separately from the synthetic in-memory terminal.

Criterion used 0.2 s warm-up, 0.5 s measurement, ten samples on this shared Mac.
At 100 clusters, mean unchanged reduction was 87 ns, view preparation 2.97 µs,
capability calculation 2.99 µs, and 120x40 rendering 49.33 µs. Capability
calculation at 1,000 clusters was 29.19 µs. Some short comparisons flagged small
percentage regressions, including roughly 2% reduction and a few percent
rendering/view timing shifts. The host also provisioned the live fixture during
this characterization. These are not controlled attribution measurements; the
absolute costs and the 60 s conformance run remain within the provisional
budgets. Preserve these numbers for a quiet, longer comparison before assigning
small changes to code or claiming improved throughput.

The 30-minute soak was not repeated in Phase 2; the previous Phase 1 result is
retained in its receipt and the scheduled job remains configured. New profile
cancellation is covered live, while the release load rechecks existing queue,
retention, scope churn, and cleanup bounds. No full live-fleet scale claim follows.

## Findings fixed and limits

Live Kubernetes defaulting removed ServiceAccount apiGroup="", which originally
caused false repeat diffs. The normalization now has a mock regression and real
repeat-apply proof. Effective PostgreSQL role membership also needs inheritance;
setup checks usable grants rather than treating membership alone as sufficient.
All-namespace database RBAC keeps a configured operator grant namespaced, with a
pure regression test. Multi-namespace fleet observation remains Phase 3.

Only the pinned Kubernetes/CNPG/PostgreSQL combination and local macOS arm64
were exercised here. Linux/macOS CI definitions are ready; broader permission,
platform, feature-gate, and release matrices remain later qualification.

Bootstrap is limited to explicit subjects and the chosen managed RBAC objects
or PostgreSQL role. Kubernetes multi-object apply is not atomic. Prune does not
delete old RBAC families/accounts/namespaces or unrelated SQL object grants.
Credential files are the proven path; automatic Secret creation, native SQL
transport, and coordination of CNPG-managed desired role state are outside this
slice. Runtime still refuses superuser monitoring and never uses setup's admin
credential implicitly. Later collectors remain honestly unfinished.

Private artifacts/kubeconfigs/passwords are ignored and excluded from uploaded
diagnostics. Test clusters were cleaned up. [Source hashes](phase-2-source-sha256.json)
identify delivered source/build inputs. No Git commit, publication, or deployment
to a user's cluster was performed. No unresolved Phase 2 blocker remains.
