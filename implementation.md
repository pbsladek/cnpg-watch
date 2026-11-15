# cnpg-watch implementation guide

Use this document to run implementation from an empty repository through the
first supported release. It turns [the design](design.md) into ordered work,
with task IDs, deliverables, validation, and phase gates.

The design owns product behavior, architecture, budgets, and support boundaries.
This guide owns execution order and progress. If implementation reveals a
conflict, record the evidence and update the relevant design section and ADR
before proceeding with a different contract. Keep both documents consistent.

## Current state and next task

Phases 0–7 are implemented and the release candidate is locally qualified.
The repository includes fleet topology/evidence,
health and hints, bounded correlated logs, capability-gated fast/medium/deep SQL,
PgBouncer statistics, operational views, explicit certificate/metrics collection,
permission preflight, RBAC/role setup, and the validation pipeline. Alpha and Beta
are locally qualified within the pinned fixture boundary.
Phase 7 adds responsive rendering, bounded navigation, real-terminal tests,
the three-cell Kubernetes matrix and four platform packages.

**Next gate: run the configured hosted CI jobs against the release source.**
No implementation phases remain. GA is pending hosted qualification; this
workspace has no Git remote. Package publication is a separate requested action.

| Phase | Outcome | Status | Gate evidence |
| --- | --- | --- | --- |
| 0 | Buildable crate and safe terminal shell | Complete | [Gate P0](docs/validation/phase-0.md) |
| 1 | Tested live watch → state → SQL → instance-table slice | Complete | [Gate P1](docs/validation/phase-1.md) |
| 2 | Permission preflight and explicit setup | Complete | [Gate P2](docs/validation/phase-2.md) |
| 3 | Fleet topology, basic health, and bounded selected logs | Complete | [Gate P3](docs/validation/phase-3.md) |
| 4 | Broader SQL registry and controlled diagnostics | Complete | [Gate P4](docs/validation/phase-4.md) |
| 5 | Pooler availability and PgBouncer statistics | Complete | [Gate P5](docs/validation/phase-5.md) |
| 6 | CNPG operational views and correlated evidence | Complete; Alpha/Beta locally qualified | [Gate P6](docs/validation/phase-6.md) |
| 7 | Terminal polish, compatibility validation, and release packages | Complete: implementation and local candidate; GA pending hosted CI | [Phase 7](docs/validation/phase-7.md) |

Update this table, the next task, and the relevant checkboxes as work lands.
Use `In progress`, `Blocked: <reason>`, or `Complete`; retain links to gate
evidence rather than treating a checkbox as proof.

## How to execute the guide

1. Read the current phase, its design references, and its entry conditions.
   Before collector, probe, RBAC, or setup work, read the relevant portions of
   [the CNPG knowledge base](docs/agent-knowledge-base.md) and
   [the PostgreSQL knowledge base](docs/postgres-agent-knowledge-base.md).
2. Work through tasks in order. Each task should produce a reviewable change
   with the appropriate fixtures and validation. Add modules and dependencies
   when that task uses them; do not scaffold every future feature.
3. Run affected checks, then the phase gate. Record exact commands, results,
   failures, and remaining limitations. A missing prerequisite or skipped live
   test is an unmet gate, not a passing result.
4. Mark a task complete only when its code, behavior, and required evidence
   exist. Mark the phase complete only after all its exit criteria pass.
5. Update the next task and handoff record before changing phases or stopping.
   Retest after changes that invalidate recorded evidence.

Tasks assume the previous phase gate passed. Within a phase, use the listed
order unless a task explicitly names an independent prerequisite. If a live
gate is blocked, continue unaffected pure/fixture work in that phase, record
the blocker, and leave dependent tasks and the phase gate open.

The knowledge bases are dated research notes. Verify external field names,
grants, and transport behavior against the exact pinned fixtures when adding
their implementation. Do not expand support claims merely because current
upstream documentation describes a feature.

### Required evidence for each task

- Changed files and the behavior they implement.
- Relevant unit, fixture, replay, snapshot, live, or performance results.
- Reduced-mode behavior for each optional capability introduced.
- Identity, cancellation, deadline, and cleanup coverage for asynchronous work.
- Redaction review for credentials, SQL, logs, Secrets, and support output.
- Benchmark/load comparison when changing a hot path, queue, cache, or schedule.

Use the [validation catalog](#validation-catalog) to choose checks. Validate
behavior and invariants rather than duplicating implementation logic in tests.

## Execution order and release checkpoints

```text
Phase 0: crate + terminal lifecycle
  -> Phase 1: identities + reducer + watches + proven SQL transport + table
  -> Phase 2: doctor + setup + profiles
  -> Phase 3: fleet + topology evidence + selected logs
       -> MVP checkpoint
  -> Phase 4: extend proven SQL registry/scheduler and diagnostics
  -> Phase 5: PgBouncer
  -> Phase 6: operational resources, delivered in small feature slices
       -> Alpha checkpoint after P6.1/P6.2 and stronger operator health
       -> Beta checkpoint after Phase 6's remaining features
  -> Phase 7: full PTY smoke + packages + release qualification
       -> GA checkpoint
```

These checkpoints describe minimum release scope, not a requirement to enable
every implemented feature by default. Medium/deep diagnostics from Phase 4
remain profile-gated. At the MVP checkpoint, fast SQL already works from
Phase 1; it does not wait for the expanded SQL phase. Alpha includes backups
from Phase 6's first slice. Beta features may already have landed behind gates.

The [design's MVP boundary](design.md#mvp-boundary) remains authoritative.
Certificates, merged log correlation, optional metrics, managed objects, and
deep diagnostics must not delay the first usable slice or MVP.

## Shared implementation contracts

Keep these contracts visible during reviews. Their detailed definitions live
in the linked design sections.

| Area | Contract |
| --- | --- |
| Ownership | One application loop owns observed state, navigation, and discardable derived views; pure reducers return typed effects. [Architecture](design.md#architecture) |
| External data | Dynamic/JSON DTOs stay in adapters and transport parsing; domain code consumes validated types. [Rust boundaries](design.md#rust-types-and-module-boundaries) |
| Identity | Scope/watch/topology epochs, UIDs, and request IDs reject obsolete events; cancellation alone is insufficient. [Acceptance](design.md#observation-identity-and-acceptance) |
| Relists | Stage membership and atomically commit complete initialization; interruption preserves the previous complete set. [Relists](design.md#watch-initialization-and-relist-reconciliation) |
| Freshness | Last-good evidence, availability, and latest-attempt outcome are independent; deadlines advance state without incoming data. [Freshness](design.md#freshness-and-sample-state) |
| Bounded work | Separate bounded traffic classes, byte/count admission, capped framing/output, owned tasks, and bounded cleanup. [Lifecycle](design.md#queue-framing-and-task-lifecycle) |
| SQL | Prove actual monitoring-role authentication and remote timeout termination before broad scheduler work. [Transport proof](design.md#monitoring-authentication-and-transport-proof) |
| Scheduling | One coalesced intent per target/class; fair admission, backoff, jitter, skipped missed ticks, and constrained manual refresh. [Scheduler](design.md#probe-scheduler) |
| Lag | Compatible stamped WAL evidence drives byte lag; last-replayed-transaction age alone cannot drive a lag alarm. [SQL semantics](design.md#postgres-sql-probes) |
| Permissions | One scope-aware capability model feeds doctor, setup, profiles, health, and badges. [RBAC setup](design.md#kubernetes-rbac-setup) |
| Mutation | Watch/TUI code observes only; setup applies only with explicit `--apply`; dry-run/diff never mutate. [Setup](design.md#setup-safety-contract) |
| Redaction | Redact before UI/log/artifact output and apply profile policy consistently. [Redaction](design.md#redaction-contract) |

Resolve Rust structure with newtypes, enums carrying required data, checked
parsing/arithmetic, and small concrete interfaces. Use a library plus a thin
binary in one package. Start with generic/enum SQL transport dispatch; introduce
runtime trait-object machinery only for an actual need. Keep full-store clones
and full-fleet recomputation out of input/render hot paths.

Read defaults from one validated `BudgetConfig`, following the
[budget table](design.md#performance-budgets). Do not copy numbers into
collectors, doctor, tests, and docs independently. Performance targets are
provisional until measured; changes need recorded evidence and a design/ADR
update.

## Phase 0 — Project scaffold and terminal safety

**Entry:** documentation-only repository. **Result:** a buildable binary with
honest command dispatch and a terminal that restores correctly.

Design references: [Phase 0](design.md#phase-0-project-scaffold),
[crate choices](design.md#crate-choices),
[machine output](design.md#machine-readable-contracts).

- [x] **P0.1 — Create the package and pin the toolchain.** Add `Cargo.toml`,
  committed `Cargo.lock`, `rust-toolchain.toml`, `src/lib.rs`, `src/main.rs`,
  and a short README. Select the Rust edition/MSRV deliberately. Add only
  dependencies used by the scaffold; ensure the library does not initialize a
  terminal or client on import. Verify a locked build and a library smoke test.
- [x] **P0.2 — Define CLI/config entry points.** Add default watch mode,
  `doctor`, `setup`, and `info`, shared scope/profile/refresh flags, and validated
  config precedence. Unsupported handlers must report that they are unfinished
  and cannot apply changes. Test help, invalid/conflicting flags, and dispatch.
- [x] **P0.3 — Add paths, logging, and output versions.** Use platform-appropriate
  config/cache/log/dump locations; keep diagnostics off machine-readable stdout.
  Add build/schema metadata and `info --output json`, marking undetected values
  unknown. Establish redaction helpers before outputting sensitive input.
- [x] **P0.4 — Implement terminal ownership.** Add the terminal guard, raw mode,
  alternate-screen handling, panic/error restoration, and `q`/Ctrl-C exit. Draw
  only a static header/body/status frame. Use a small PTY lifecycle test now;
  the full product smoke harness comes in Phase 7. Test normal exit, error, and
  panic cleanup with bounded waits.
- [x] **P0.5 — Add repeatable fast checks.** Add `make check`, `make test`, and
  Linux/macOS CI for format, clippy, tests, and build. Keep live/long tests out
  of default `cargo test`. Use deterministic reviewed snapshots where useful;
  CI cannot auto-accept snapshot changes.
- [x] **P0.6 — Record initial architecture decisions.** Create the first ADRs
  listed in the design as their decisions are made. The SQL transport ADR stays
  `Proposed` until P1.9 proves authentication and cleanup. Record declared
  PostgreSQL 18/CNPG 1.30.x targets without guessing exact Kubernetes versions.

**Gate P0:** V01 and V02 pass. The binary builds with its lockfile, help/config
errors are deterministic, JSON stdout is clean, and terminal state restores on
normal exit, error, Ctrl-C, and panic. Record the result before P1.1.

**Defer:** live clients, broad feature/view registries, setup application, and
all operational collectors.

## Phase 1 — First tested live vertical slice

**Entry:** Gate P0. **Result:** Cluster/Pod watches → accepted typed state → one
role-aware SQL bundle → instance table, with useful topology when SQL fails.

Design references: [Phase 1](design.md#phase-1-first-tested-vertical-slice),
[identity](design.md#observation-identity-and-acceptance),
[transport proof](design.md#monitoring-authentication-and-transport-proof),
[performance validation](design.md#performance-benchmarks-and-bounded-load-tests).

- [x] **P1.1 — Implement core values and observations.** Add scoped resource
  keys, UID wrappers, scope/watch/topology epochs, request IDs, typed `Lsn` and
  timeline/system identity. Separate last-good observations, latest attempts,
  and availability. Test parsing, LSN round trips/overflow, and checked deltas.
- [x] **P1.2 — Establish time and minimal metadata.** Add injected monotonic/wall
  time, deadline decisions, topology/fast-SQL capability entries, minimal query
  metadata, and `BudgetConfig` validation. Test invalid budgets and expiry with
  no new data. Keep future feature registries out of this task.
- [x] **P1.3 — Add Cluster/Pod adapters.** Normalize dynamic CNPG and typed core
  objects into small observations. Preserve keys, UIDs, source metadata, missing
  evidence, and semantic equality. Choose and record the exact CNPG 1.30.x
  schema/source reference for these fixtures before coding field paths; reuse
  it in P1.8's live installation. Capture healthy, missing-field, malformed,
  and replacement fixtures; replace provisional samples with captured live
  evidence when available. Never guess unsupported CRD fields.
- [x] **P1.4 — Implement acceptance and relists in the reducer.** Add typed events
  and effects, UID-aware apply/delete, epoch/request rejection, staged
  initialization, and atomic commit. Test late old-UID deletes, late probe
  results, missed deletes, interrupted relists, and unchanged listed membership.
- [x] **P1.5 — Build the deterministic replay/property harness.** Add explicit
  event/time traces, fixed seeds, minimized failure persistence, and a full
  recomputation oracle for the derived data that exists. Prove V03/V04 before
  relying on a live UI to diagnose state correctness.
- [x] **P1.6 — Wire the bounded runtime and watches.** Own tasks and cancellation;
  implement separate control/topology/result/log traffic classes and byte/count
  permits. Add fair bounded loop turns, watch backoff/sync health, rate admission,
  and freshness deadlines. Connect one kube client and Cluster/Pod watches.
  Test full queues, shutdown, quiet healthy watches, and scope replacement.
  Synthetic log traffic may exercise the queue before a log collector exists.
- [x] **P1.7 — Render the accepted instance model.** Add stable row selection,
  primary/replica/readiness, source health, and never-sampled/stale/error cells.
  Test view conversion and buffers at 120x40 and 100x30 before live rendering.
  Incremental view preparation must agree with full recomputation.
- [x] **P1.8 — Provision the isolated live fixture.** Add pinned k3d/k3s, CNPG
  1.30.x, PostgreSQL 18, separate operator/database namespaces, one primary,
  and at least one replica. Record exact versions/image references in
  `tests/e2e/versions.env` or an equivalent checked-in manifest. Add bounded
  readiness waits, unique run names, failure artifacts, and cleanup. Test-only
  provisioning creates the monitoring role; production setup is not required.
- [x] **P1.9 — Implement, prove, and select SQL authentication/transport.** Build
  the minimal executor and candidate fast bundle independently of the scheduler.
  Test exec with loopback credentials first; evaluate configured peer mapping or
  port-forward/native SQL if needed. Use the actual monitoring role and verify
  effective identity/grants. Capture nonzero exit with JSON stdout, invalid
  credentials, stalled connections, output caps, timeout, and cancellation.
  A separate test-only admin observer must confirm remote probe termination.
  Record timeout/cleanup bounds and credential delivery in ADR 0001. Keep the
  gate open if no transport satisfies the contract; never fall back silently
  to superuser.
- [x] **P1.10 — Connect one fast bundle and its minimal scheduler.** Use one
  identified instance bundle with role-appropriate WAL/replication/archive
  components and one bounded result. Gate queries by the proven version/grants.
  Enforce end-to-end deadlines, process/cluster/pod concurrency, one pending
  intent, backoff, jitter, and constrained manual refresh. Validate using paused
  time and production DTOs, including idle-primary lag semantics.
- [x] **P1.11 — Exercise the assembled slice.** Verify real reconnect, same-name
  pod replacement, SQL failure with retained evidence, time-only stale updates,
  and cleanup. Replay scope/selection races even before interactive context
  switching exists. Add `make test-live E2E_SUITE=slice` and a small CI live job.
- [x] **P1.12 — Record the first performance baseline.** Add Criterion workloads
  and release-build bounded-load measurements for the implemented pipeline.
  Use 10/100/1,000-cluster synthetic fixtures and controlled input/log traffic.
  Record achieved scale, latency, idle CPU, RSS, retained/queued bytes, task
  counts, and sampling duration. Establish the scheduled 30-minute soak and
  record a first run; keep it separate from fast PR checks.

**Gate P1:** V01–V09 pass for the slice. The proven transport's live contracts
must run, not just parser fixtures. Late results cannot overwrite replacement
state; partial relists cannot erase accepted data; failed SQL retains stale
evidence; idle/caught-up replication does not alarm. A recorded baseline and
soak show bounded retention/task counts. Investigate missed performance targets
and document measured limits before expanding features or claiming scale.

**Defer:** broad profile/query/view registries, poolers, backups, deep SQL,
merged logs, and optional metrics. Add their contracts with their working slice.

## Phase 2 — Permission preflight and explicit setup

**Entry:** Gate P1, including an accepted transport ADR. **Result:** operators
can diagnose missing capabilities and review/apply explicit bootstrap changes.

Design references: [Phase 2](design.md#phase-2-permission-preflight-and-setup),
[capabilities](design.md#capability-matrix),
[bootstrap permissions](design.md#permission-and-bootstrap-model),
[setup safety](design.md#setup-safety-contract).

- [x] **P2.1 — Expand the capability matrix and profiles.** Define shared
  requirements for implemented features and the `minimal`, `standard`,
  `incident`, and `secure` envelopes. Future feature requests report unfinished
  capability honestly; they do not start fake collectors. Test feature gates,
  budgets, redaction, and profile transitions with cancellation/epoch rules.
- [x] **P2.2 — Implement doctor checks and outputs.** Add namespace/operator/
  cluster-scoped access checks, SSAR plus appropriate non-mutating live fallback,
  CNPG API visibility, terminal/transport/role validation, and grouped remediation.
  Represent forbidden/unknown/absent/syncing/empty distinctly. Add versioned text/
  JSON contracts and checks against the same capability state used by the TUI.
- [x] **P2.3 — Implement RBAC generation and dry-run.** Generate deterministic
  Roles/Bindings for database and separate operator namespaces. Keep optional
  cluster resources explicit with `--include-cluster-resources`. Exclude
  `DatabaseRoleList` endpoints and unnecessary status rules. Add reviewed
  namespace/operator/cluster examples and assert generated output matches them.
- [x] **P2.4 — Implement RBAC diff, then apply.** Compare managed desired/live
  rules with deterministic versioned output. Add explicit apply, idempotence,
  subject validation, and no pruning without `--prune`. Test dry-run/diff paths
  issue no mutating requests and apply twice makes no unintended changes.
- [x] **P2.5 — Implement PostgreSQL role DDL, then application.** Add deterministic
  `--print-sql`, monitor/reserved/separate action-role profiles, explicit password
  preservation/rotation, and credential redaction. Apply twice in the fixture,
  inspect real grants, and run production bundles as the resulting monitor role.
  An action-role setup profile does not introduce TUI actions.
- [x] **P2.6 — Validate actual least-privilege identities.** Compare doctor/setup
  requirements with real list/watch/log/exec requests. Test get-only/create-only/
  both/neither exec verbs for pinned Kubernetes combinations. Verify namespaced
  observation without CRD object, webhook, catalog, Secret, or operator access.
  Add same/different operator namespace, forbidden discovery, and ambiguity cases.
- [x] **P2.7 — Surface results in the app and docs.** Add doctor/profile detail
  navigation, persistent reduced-mode badges, copyable setup suggestions, and
  user docs for the proven credential path. Test command JSON/schema/help and
  ensure normal watch/TUI behavior has no bootstrap mutation path.

**Gate P2:** V01–V08 plus V10 pass for implemented capabilities. Dry-runs and
diffs are deterministic and non-mutating; only explicit apply writes. Live
identities establish that generated permissions and monitoring grants actually
work. Optional missing access degrades the dependent feature only.

**Defer:** automatic privilege pruning, guessing bootstrap subjects, and all
mutation shortcuts in the TUI.

## Phase 3 — Fleet topology and bounded selected logs

**Entry:** Gate P2. **Result:** the MVP can observe multiple namespaces/clusters,
show basic operator/evidence health, and inspect selected-resource logs.

Design references: [Phase 3](design.md#phase-3-kubernetes-topology),
[topology behavior](design.md#topology-behavior-matrix),
[TUI modes](design.md#tui-interaction-model).

- [x] **P3.1 — Expand scope/watch management.** Add all-namespace and separate
  operator scope handling, selectors, and namespace/context replacement. Share
  watchers for identical resource/scope inputs. Extend live fixtures to at least
  two CNPG clusters in different database namespaces. Test late old-scope events,
  disconnected/restarting watches, complete empty lists, and cache capacity errors.
- [x] **P3.2 — Add events, PVCs, services, and EndpointSlices.** Add adapters,
  ownership association, reduced modes, retention/deadline eviction, and compact
  storage/connectivity evidence. Test independent watch ordering, UID changes,
  missing fields, ambiguous ownership, and service/primary convergence windows.
- [x] **P3.3 — Add basic operator and pooler availability.** Observe configured/
  discovered operator deployment/pods/events/optional leases and Pooler resources/
  pods. Separate database availability from operator discovery confidence. Test
  missing operator access, zero/multiple candidates, readiness, and unknown stats.
- [x] **P3.4 — Implement MVP navigation and filters.** Add Fleet/Cluster/Detail
  transitions, `:fleet`, `:instances`, `:health`, `:events`, `:doctor`, `:logs`,
  contextual help, focus, and stable-key selection. Add bounded history and the
  design's regex/inverse/label/fuzzy filters with validation and bounded input.
  Test mode choice, filtered-empty, removed selection, and long/wide text.
- [x] **P3.5 — Add deterministic health and hint rules.** Rank evidence with
  stable IDs/navigation targets and explicit confidence/freshness. Test detection,
  deduplication, expiry, and clearing. Compare incremental results with full
  recomputation. Describe Kubernetes-only health accurately while retaining
  independently sampled SQL evidence from Phase 1.
- [x] **P3.6 — Add lazy selected-resource logs.** Implement bounded historical
  reads and follow streams, capped framing/parsing, redaction, source/aggregate
  retention, truncation counters, filtering, pause/follow, and inactive-source
  eviction. Close streams on view/scope/source changes. Keep canonical entries;
  do not build merged operator timelines yet. Verify floods cannot block input,
  topology, deadlines, or shutdown.
- [x] **P3.7 — Add MVP diagnostics and support output.** Expose source health,
  budgets, rejection/drop counters, sample ages, and queue/task counts in info/
  doctor/debug output. Implement a minimal redacted bounded screen/model dump;
  later phases add their evidence. Validate schema versions and profile redaction.
- [x] **P3.8 — Qualify the MVP checkpoint.** Run fleet/reduced-RBAC/log suites,
  reconnect/replacement/freshness replays, rendering checks, and affected
  benchmarks/load cases. Confirm unselected fleet SQL is off and cached/never-
  sampled values are labelled. Record a usable CLI walkthrough and limitations.

**Gate P3 / MVP:** Complete; [validation receipt](docs/validation/phase-3.md). V01–V11 pass for MVP behavior. Two database namespaces and a
separate operator namespace work; permission gaps stay local; selected logs are
bounded and cancel cleanly; source sync/absence/error states are distinguishable.
The instance table and fast SQL remain usable while optional sources fail.

**Defer:** merged cluster/operator logs, deep diagnostics, certificate reads,
plugin systems, and custom user-authored views.

## Phase 4 — Extend PostgreSQL probes and diagnostics

**Entry:** Gate P3 and P1.9's proven transport. **Result:** a version/capability-
gated query registry with controlled medium/deep diagnostics.

Design references: [Phase 4](design.md#phase-4-expand-postgresql-runtime-probes),
[query registry](design.md#version-gated-query-registry),
[under-load policy](design.md#database-under-load-policy).

- [x] **P4.1 — Expand query capabilities and typed results.** Add actual view/
  column/function/grant detection and version metadata for each shipped bundle.
  Capture successful and unsupported/forbidden/malformed fixtures. Scope
  capability caches to server/pod identity and invalidate after replacement or
  relevant role/version changes. Run every enabled variant on its supported server.
- [x] **P4.2 — Complete fast WAL/archive evidence.** Extend stamped system/
  timeline/role observations, primary-peer association, byte lag, archiver state,
  and reset-aware counters. Show unknown for incompatible/skewed evidence.
  Test promotion, idle primary, missing peer identity, null replay state, counter
  reset, and recovery after failure using production SQL and DTOs.
- [x] **P4.3 — Extend fair scheduling and overload policy.** Add independent
  fast/medium/deep classes, bounded intents, class priority/fairness, capability
  suspension/rechecks, and actual sample/sweep diagnostics. Keep unselected SQL
  off by default; any later background mode follows the design's slower policy.
  Prove exact eligibility, permit cleanup, manual-refresh coalescing, and no
  catch-up bursts with paused time and seeded jitter.
- [x] **P4.4 — Add medium replication/settings bundles.** Implement bounded slot,
  synchronous-replication, and selected settings evidence at the medium cadence.
  Add typed parsers, reduced modes, and health targets without full catalog scans.
  Test live grants, version gates, counter/reset/identity behavior, and output caps.
- [x] **P4.5 — Add gated connection/wait diagnostics.** Implement aggregate
  connection pressure, wait summaries, bounded blocker/long-transaction evidence,
  and `:connections`/`:waits` views. Keep query text opt-in and redacted from
  dumps by default. Test overload slows deep work before medium/fast work and
  never disables independent topology/log observation.
- [x] **P4.6 — Qualify probe behavior and cost.** Extend live overload/timeout/
  authentication/promotion suites and SQL fixtures. Benchmark changed admission,
  parsing, health, and view paths. Record DB-side execution time, process startup,
  full bundle/sweep duration, and cleanup separately; do not infer efficiency
  from a low local render time.

**Gate P4:** Complete; [validation receipt](docs/validation/phase-4.md). V01–V11 pass for all enabled bundles. Every production bundle has
version/grant gates, an output schema, timeout, bounded output, and actual live
coverage as the monitor role. Overload and rejected connections preserve stale
evidence and degrade diagnostic work first.

**Defer:** broader PostgreSQL major support without fixtures/live contracts,
heavy relation scans, and query cancel/terminate or configuration actions.

## Phase 5 — Poolers and PgBouncer

**Entry:** Gate P4. **Result:** pooler readiness and admin statistics remain
distinct, with independent error/backoff state inside the shared exec budget.

Design references: [Phase 5](design.md#phase-5-poolers-and-pgbouncer),
[pooler counts](design.md#pooler-connection-counts),
[pooler setup](design.md#pgbouncer-role-setup).

- [x] **P5.1 — Expand Pooler evidence and fixtures.** Normalize desired/ready/
  unavailable counts, type, phase, service, image, and endpoints. Test partial
  status, missing permissions, replacement, and pooler-to-cluster association.
- [x] **P5.2 — Implement the bounded statistics transport/parser.** Prove the
  pooler's actual stats authentication path and parse captured `SHOW POOLS`/
  `SHOW STATS` responses into typed models. Preserve output/remote-status/
  timeout limits. Test column variation, malformed output, stats denial, and caps.
- [x] **P5.3 — Schedule and render pooler pressure.** Use independent capability/
  failure/backoff state with process-wide/per-pod admission limits. Join canonical
  samples into the pooler/connection views. Test waiting clients/saturation and
  that pooler failure never hides direct PostgreSQL or Kubernetes availability.
- [x] **P5.4 — Add explicit pooler-role setup.** Prefer a stats-only user;
  generate dry-run/diff guidance before safe apply support. Require a proven
  CNPG-compatible auth/configuration path before mutation. Unsupported layouts
  produce documentation and reduced mode. Validate idempotence and credential
  redaction in the isolated fixture.
- [x] **P5.5 — Qualify poolers live and under load.** Add a real CNPG Pooler and
  stats success/denial/saturation cases to local/CI suites. Repeat affected
  scheduling, queue, dump, rendering, and performance checks with pooler traffic.

**Gate P5:** V01–V11 pass with poolers enabled. Readiness survives unavailable
statistics, the runtime uses the intended stats role, and shared admission/
cleanup remains bounded. Alpha still needs backups/stronger operational health
from the first Phase 6 slices.

## Phase 6 — CNPG operational resources

**Entry:** Gate P5. **Result:** focused operational views built as complete
adapter → capability → reducer → evidence → view → validation slices.

Design references: [Phase 6](design.md#phase-6-cnpg-operational-resources),
[operational features](design.md#cnpg-specific-feature-improvements),
[health rules](design.md#health-rules).

For each task, add resource/version fixtures and permissions first, then the
collector, rules/view, reduced mode, live coverage, and relevant performance
checks. Do not batch all adapters before testing any view.

- [x] **P6.1 — Backups and schedules.** Add Backup/ScheduledBackup reconciliation,
  active/latest-success/failure/method/retention evidence, archive correlation,
  and `:backups`. Test missing resources, denied access, pending/failed/recovered
  status, and version-dependent fields. Report unavailable next-run/recovery
  evidence rather than deriving unsupported guarantees.
- [x] **P6.2 — Storage/services drill-down and stronger operator health.** Build
  `:storage`/`:services` from existing canonical observations. Add observed-
  generation and reconciliation freshness, association confidence, and fleet
  summaries. Test failover convergence, resource replacement, stale conditions,
  pending PVCs/endpoints, and clearing findings. **Record the Alpha checkpoint**
  once these tasks, Phase 5, and their validation evidence pass.
- [x] **P6.3 — Managed PostgreSQL objects.** Add Database, DatabaseRole,
  Publication, and Subscription adapters/reconciliation views. Discover actual
  endpoints; `DatabaseRoleList` is a list kind. Test supported/absent CRDs,
  partial status, per-feature denial, and database/role/subscription navigation.
- [x] **P6.4 — Replica topology and data-safety context.** Add delayed/external
  topology, slots/sync evidence, optional FailoverQuorum, and `:replicas`.
  Preserve system/timeline/skew compatibility. Test unsupported fields and
  uncertain evidence; do not turn missing evidence into healthy/failing claims.
- [x] **P6.5 — Plugins and version posture.** Add CNPG-I capability/status,
  operator/image evidence, namespaced ImageCatalog, optional cluster-scoped
  ClusterImageCatalog, and `:plugins`. Test catalog/cluster-scope denial and
  fixture/version gaps independently of ordinary namespaced observation.
- [x] **P6.6 — Webhooks and optional certificates.** Add cluster-scoped webhook
  configuration evidence joined to services/endpoints/pods. Add `:certs` only
  with explicit authorized Secret reads. Test expiry/rotation, unavailable
  cluster scope, absent Secrets, and strict redaction; private keys never enter
  domain models, logs, screens, or dumps.
- [x] **P6.7 — Optional operator metrics.** Add configured/profile-gated bounded
  scrapes and typed selected metrics. Test default-disabled, stale, malformed,
  timeout, unknown metric, and unavailable endpoint behavior. Core watch mode
  and operator availability cannot depend on successful scrapes.
- [x] **P6.8 — Correlated log timelines.** Extend the canonical log store with
  cluster/backup/operator sources, bounded indexes, correlation evidence, source
  toggles, and explicit unmatched/full-operator access. Preserve operator namespace
  labels. Test timestamp disorder, unknown lines, redaction before correlation
  output, source churn, eviction, and total memory/stream caps.
- [x] **P6.9 — Expand dumps and reduced-mode qualification.** Include all new
  capability, version, identity, evidence, and freshness metadata in bounded
  versioned dumps. Extend denied-secret/managed-object/webhook/operator-log and
  malformed-input suites, incremental/full health comparisons, and source-churn
  soak coverage. Record the Beta checkpoint after all Phase 6 tasks pass.

**Gate P6 / Beta:** V01–V11 pass for operational features. Each added source has
fixtures, identity/lifecycle handling, reduced-mode behavior, a focused view,
and validated health navigation. Missing optional evidence does not crash or
silently remove other features; logs and dumps remain bounded/redacted.

**Defer:** arbitrary plugins, generic Kubernetes browsing, unsupported CRD
assumptions, and administrative TUI actions.

## Phase 7 — Terminal polish and release qualification

**Entry:** Gate P6. **Result:** documented macOS/Linux packages with evidence
for compatibility, terminal behavior, reduced modes, and supported fleet scale.

Design references: [Phase 7](design.md#phase-7-terminal-polish-and-release-readiness),
[TUI testing](design.md#tui-test-strategy),
[packaging](design.md#release-packaging).

- [x] **P7.1 — Finalize navigation and responsive rendering.** Complete commands,
  filters, bounded history, breadcrumbs, contextual actions/help, profile state,
  and screen dumps. Review Fleet/Cluster/Detail buffers at 120x40, 100x30, and
  narrow fallback sizes. Property-test arbitrary dimensions, Unicode/wide text,
  empty/removed selections, and changing row counts without panics or overflow.
- [x] **P7.2 — Build the full real-terminal smoke suite.** Add portable-pty/vt100
  startup, semantic screen waits, keys, resize, flood responsiveness, q/Ctrl-C,
  error/panic restoration, and clean teardown against the live fixture. Use
  bounded predicate waits and fixed names; retain raw/redacted failure evidence
  per policy. Run the shared fixture suite serially and keep expensive scenarios
  few; buffer/replay tests retain detailed coverage.
- [x] **P7.3 — Complete compatibility and nightly jobs.** Expand `e2e-k3d` across
  pinned supported Kubernetes/exec combinations and every claimed PostgreSQL
  bundle variant. Run longer property cases, 60-second bounded-load checks,
  and the 30-minute soak in suitable jobs. Verify cleanup and artifact capture
  on failure, not only on success.
- [x] **P7.4 — Build and test release artifacts.** Package macOS arm64/x86_64 and
  Linux x86_64/aarch64 binaries with checksums, generated completions, and command
  reference. Smoke-test `--help`, `info --output json`, and terminal lifecycle on
  a matching runner or documented emulator. Cross-compilation alone is not a
  runtime smoke result; report any untested target explicitly.
- [x] **P7.5 — Qualify performance and support claims.** Run recorded release-
  build workloads, compare affected baselines, inspect RSS/counters/task/source
  plateaus, and publish the scale that meets agreed targets. Resolve missed
  targets or amend the design/ADR with evidence before claiming support.
  Timing reports from noisy runners must not masquerade as precise regressions.
- [x] **P7.6 — Finalize operator/developer documentation.** Document installation,
  credential/setup flows, least-privilege profiles, doctor output, terminal
  limits, data freshness, diagnostics/artifact locations, and known constraints.
  Verify examples against the release binary and qualify the release candidate.

**Gate P7 / GA:** V01–V12 pass for every claimed target/feature. Required output
schemas are versioned, examples match generated output, real PTY/reduced-RBAC
suites pass, and performance/soak reports support documented limits. Package
creation completes this guide's release work; publishing is a separate explicit
operation when requested.

**Local result:** P7.1–P7.6 are complete; see the
[qualification receipt](docs/validation/phase-7.md) for three successful live
cells, four native/emulated package executions, terminal/property coverage and
performance evidence. Hosted CI definitions are present but have not executed
here. The GA gate remains open; no release was published.

## Validation catalog

Every check starts when its feature is introduced and remains required for
relevant changes. A later phase extends existing checks rather than rebuilding
their foundations. Phase references to V01–V11 apply to implemented behavior;
they do not require tests for future features that do not exist yet.

| ID | Check | Required evidence |
| --- | --- | --- |
| V01 | Fast Rust/build checks | Locked build, formatting, clippy, offline unit/integration tests, deterministic CLI/config errors |
| V02 | Terminal/output contracts | Restore on normal/error/panic/Ctrl-C; clean machine stdout; reviewed schemas/help/buffers; no secrets in outputs |
| V03 | Identity and replay | Same-name/new-UID replacement; late delete/result; old scope/watch/topology/request rejection; counter baseline reset |
| V04 | Relist and derived-state properties | Exact membership on complete list; old set survives interrupted list; unchanged members retained; incremental/full recomputation agree |
| V05 | Time and scheduling | Time-only expiry/eviction; quiet healthy watches current; exact backoff/admission; fair priority; no tick bursts; coalesced refresh |
| V06 | Queue/framing/task bounds | Message/byte/line/output/retention/pending caps; flood input responsiveness; full-queue cancellation; permits/tasks reclaimed |
| V07 | Live SQL/transport | Actual intended role/grants, shipped SQL and production DTOs, remote status/output failures, server-side timeout termination, local cleanup |
| V08 | Real watch/reduced-mode integration | Reconnect/replacement/promotion; denied/unknown/absent evidence; independent scopes/sources remain useful |
| V09 | Performance and soak | Seeded workloads and release metadata; latency/CPU/RSS/sample age/queue counters; 60-second load and scheduled 30-minute churn soak |
| V10 | Setup/least-privilege contracts | Pure dry-run/diff, explicit/idempotent apply, preserved privileges/password policy, actual RBAC verbs/grants, no mutation from watch |
| V11 | Feature/view/artifact coverage | Resource/version/error fixtures; stable selection and health targets; clearing/expiry; redaction and bounded dumps/logs |
| V12 | Release qualification | Real PTY suite, compatibility matrix, native/emulated package smoke, checksums/completions, verified support docs |

Keep success, missing field, malformed input, forbidden access, stale value,
replacement, and recovery cases for each external parser/collector. A fixture
that deserializes does not prove a live query or permission contract. Keep the
real test observer's admin identity separate from the application under test.

### Workloads and proposed performance targets

Use the [design's workloads and limits](design.md#performance-benchmarks-and-bounded-load-tests)
without changing their meaning in test helpers:

- Fixed-seed 10/100/1,000-cluster fixtures, normally three instances each.
  Record related resource counts and payload sizes; large scale begins as
  characterization, not a support promise.
- At 100 clusters, target input-to-visible p95 <=100 ms/p99 <=250 ms, view
  preparation plus draw p95 <=16 ms, at most 10 frames/s, idle CPU <=1% of one
  core, and warmed RSS <=128 MiB for the defined capped-log workload.
- Flood with eight selected log sources totaling at least 5,000 lines/s,
  topology bursts, oversized unterminated lines/output, and slow consumers.
- Run 60-second load checks and a separate scheduled 30-minute scope/source/
  reconnect soak. Require retained bytes and task/source counts to plateau.
- Record OS/hardware, toolchain/dependency/image versions, release profile,
  fixture seed, active probes/view, source/update rates, queue high-water marks,
  drops/coalescing, sample/sweep ages, and cleanup/staging peaks.

Use synthetic transport/log producers for early pipeline tests; introduce real
source costs as their collectors arrive. Maintain allocation/retention counters
alongside RSS. Assert deterministic bounds in fast CI and evaluate sustained
timing regressions on controlled runners. Investigate target misses rather than
loosening limits silently or hiding dropped work.

## Planned command entry points

These entry points are implemented. Live and long-running checks remain
explicitly selected; the default suite needs no cluster or credentials.

| Command | Purpose | Introduced |
| --- | --- | --- |
| `make check` | Format, clippy, and locked build | P0.5 |
| `make test` | Default fast unit/integration/CLI/buffer suite; no Docker or live credentials | P0.5 |
| `make test-replay` | Named lifecycle/property regressions | P1.5 |
| `make test-scheduler` | Paused-time timing, admission, cancellation | P1.10 |
| `make test-live E2E_SUITE=slice` | Provision isolated cluster and prove first slice | P1.11 |
| `make bench` | Release-build Criterion comparisons with saved workload metadata | P1.12 |
| `make test-load` | 60-second bounded-load scenario | P1.12 |
| `make test-soak` | 30-minute scheduled/manual lifecycle soak | P1.12 |
| `make test-live E2E_SUITE=permissions` | Real generated RBAC and SQL role contracts | P2.6 |
| `make test-live E2E_SUITE=fleet` | Multi-namespace topology, health, and bounded logs | P3.8 |
| `make test-live E2E_SUITE=probes` | Expanded SQL registry, role changes, and overload | P4.6 |
| `make test-live E2E_SUITE=operations` | PgBouncer availability/statistics/failure | P5.5 |
| `make test-live E2E_SUITE=operations` | Operational resources and optional access matrix | P6.9 |
| `make test-tui` | Full PTY smoke against live fixture | P7.2 |
| `make package` | Build checksummed artifacts and run package smoke | P7.4 |

Implement the fast targets around these Cargo commands after the crate exists:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
cargo test --locked --lib --bins --tests
```

Live, load/soak, and full PTY tests must be explicitly selected (for example,
ignored integration tests called by wrappers). Do not require a cluster to
compile or run the default fast suite. Keep real-world credentials and local
kubeconfig out of default test discovery. Missing Docker/tools/fixture access
must fail the requested live target clearly, not silently skip its assertions.

Provide `scripts/e2e-local-k3d.sh --suite <name>` as the shared local/CI live
entry point, initially accepting `slice` and expanding with implemented suites.
It must provision/target only the named isolated fixture, use explicit fixture
kubeconfig/context and run-owned resources, bound readiness/test waits, and
collect artifacts before cleanup. Keep the existing design's
`E2E_KEEP_CLUSTER=1` escape hatch for investigation. Never point mutation or
failure-injection tests at the user's default kube context.

## Evidence and handoff records

Add short phase receipts under `docs/validation/` as phases are completed.
Keep large generated logs, dumps, and benchmark output in ignored local
artifact directories or CI artifacts. Commit concise redacted results and
reviewed fixtures/snapshots, not credentials or bulk terminal recordings.
Add artifact paths to `info` when those outputs become implemented.

Use this receipt shape and link it from the phase status table:

```text
Phase/task IDs:
Status and date:
Source revision or tested worktree description:
Changed behavior/files:
Toolchain, OS, and fixture versions:
Commands and exit results:
Scenario IDs, seeds, and fixture paths:
Performance comparison and achieved limits, when applicable:
Redaction/reduced-mode/lifecycle evidence:
Artifact locations:
Open issues and unmet gates:
Next task:
```

For a new implementation session, use a brief such as:

```text
Read implementation.md and the design sections linked by the current phase.
Resume the first unfinished task whose prerequisites pass. Implement and
validate the current phase's tasks in order, keeping evidence for its gate.
Update checkboxes, status, the next task, and the handoff record to reflect
actual results. Keep blocked gates open and continue independent work.
```

When handing off unfinished work, include the smallest reproduction and the
expected/actual result. Record whether a blocker needs credentials, tools,
an upstream compatibility decision, or a code fix. Keep the phase incomplete
and identify which tasks remain safe to continue. Do not accept a transport,
declare a release checkpoint, or mark a validation gate complete on the basis
of a planned test.

## Completion checklist

- [x] All phase/task checkboxes are backed by code and linked validation evidence.
- [x] Design, ADRs, compatibility claims, defaults, and generated examples agree.
- [x] Every implemented source has typed normalization, lifecycle handling,
  explicit reduced mode, and applicable fixture/live coverage.
- [x] Performance/soak results support the documented fleet scale and limits.
- [x] The runtime remains observe-only; setup mutations require explicit apply.
- [ ] Release artifacts, real PTY behavior, and support documentation pass Gate P7.
  Local candidate qualification passed; hosted CI is the remaining GA condition.
