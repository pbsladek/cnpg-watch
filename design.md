# cnpg-watch initial design

Use the [phased implementation guide](implementation.md) to execute this design
and track tasks, validation evidence, and phase completion.

## Summary

`cnpg-watch` is a real-time terminal dashboard for CloudNativePG clusters. It gives operators a persistent view of PostgreSQL topology and health in Kubernetes: primary and replica LSN positions, replication lag per instance, WAL archiving status, pooler connection counts, and recent Kubernetes events.

The first implementation should feel like a focused, Postgres-native slice of `k9s`: cluster topology is the main navigation model, PostgreSQL runtime state is first-class, and Kubernetes objects are supporting evidence rather than the whole product.

## Problem

`kubectl cnpg status` is useful for point-in-time diagnosis, but it is not a live dashboard. Re-running it in a loop repeatedly initializes CLI state, reads configuration and plugin files, talks to the API server from scratch, and redraws unstructured text. That model is noisy for humans and wasteful for a workflow that needs continuous observation.

`cnpg-watch` replaces repeated invocations with one long-lived process that:

- opens Kubernetes watches once and keeps local state fresh;
- samples Postgres runtime views on a controlled interval;
- renders a stable terminal UI with explicit stale/error states;
- preserves recent history long enough to show changes and regressions.

## Goals

- Show CloudNativePG cluster topology in real time.
- Identify the current primary and each replica.
- Show per-instance LSN positions and replication lag.
- Show WAL archive health and recent archive failures.
- Show CNPG pooler availability and connection counts.
- Show recent Kubernetes warning/normal events related to the selected cluster.
- Avoid shelling out to `kubectl` for normal operation.
- Use `ratatui` for a responsive terminal interface.
- Provide setup commands that can apply required Kubernetes RBAC and create a least-privilege PostgreSQL monitoring role when the operator supplies admin credentials.
- Continue providing useful Kubernetes and cached Postgres state when a database is overloaded, rejecting connections, slow to answer, or partially unavailable.
- Support macOS and Linux first. Windows terminal support is not part of the initial target.
- Keep the initial implementation small enough to ship as a single Rust binary.

## Non-goals

- Replacing `kubectl`, `k9s`, or the full CNPG plugin feature set.
- Managing clusters, promoting replicas, or performing destructive actions.
- Requiring Prometheus as the primary data source.
- Requiring direct local TCP access to PostgreSQL services.
- Implementing a generic Kubernetes dashboard.
- Persisting historical metrics beyond the current process.
- Performing database writes during normal watch mode.
- Supporting Windows terminals in the initial implementation.

## User experience

The default command opens the dashboard for the current kube context:

```sh
cnpg-watch
```

Useful initial flags:

```sh
cnpg-watch --namespace app-db
cnpg-watch --cluster payments
cnpg-watch --namespace app-db --operator-namespace cnpg-system
cnpg-watch --all-namespaces
cnpg-watch --all-namespaces --operator-namespace cnpg-system
cnpg-watch --refresh 2s
cnpg-watch --kube-context prod-us-east
```

Setup commands are explicit and separate from the dashboard:

```sh
cnpg-watch setup kubernetes-rbac --namespace app-db --service-account cnpg-watch
cnpg-watch setup kubernetes-rbac --namespace app-db --operator-namespace cnpg-system --service-account cnpg-watch
cnpg-watch setup postgres-role --namespace app-db --cluster payments
cnpg-watch setup pooler-role --namespace app-db --cluster payments --pooler payments-rw
cnpg-watch doctor --namespace app-db --operator-namespace cnpg-system --cluster payments
```

`setup` commands may create or update Kubernetes objects and PostgreSQL roles. The default `cnpg-watch` command must not mutate Kubernetes or PostgreSQL state.

Following the parts of k9s that fit this tool, `cnpg-watch` should include a command/filter grammar early:

```text
:clusters              show visible CNPG clusters
:fleet                 show fleet-wide CNPG cluster health across namespaces
:operator              focus the CNPG operator install and controller health
:instances             focus instance table
:poolers               focus pooler panel
:events                focus event feed
:health                focus health reason list
:hints                 focus contextual runbook hints
:archive               focus WAL archive panel
:logs                  focus CNPG-aware log timeline
:connections           focus connection pressure
:waits                 focus waits, blockers, and active query diagnostics
:backups               focus Backup and ScheduledBackup state
:storage               focus instance PVC/data/WAL/tablespace volumes
:services              focus CNPG services and endpoint slices
:replicas              focus replica topology, slots, and sync state
:roles                 focus DatabaseRole reconciliation
:databases             focus Database reconciliation
:subscriptions         focus logical replication subscriptions/publications
:certs                 focus certificate expiration and rotation state
:plugins               focus CNPG-I plugin status and capabilities
:profile               show or switch runtime configuration profile
:doctor                run non-mutating preflight and show results
:screendump            save current screen and current model snapshot
/payments              regex filter clusters/instances/events
/! healthy             inverse regex filter
/-l cnpg.io/cluster=x  label selector where Kubernetes labels apply
/-f paym               fuzzy filter
```

When more than one CNPG cluster is visible, `:fleet` should be the natural overview. When exactly one cluster is selected, the main dashboard can remain cluster-focused while still showing fleet/operator summary badges.

Do not expose k9s-style generic resource commands such as `:pod` or `:deploy` in the initial version. `cnpg-watch` is not a generic Kubernetes browser; commands should map to Postgres/CNPG operational views.

The first screen is the operational view, not a landing page. It should use the
mode-based interaction model described in the UI section:

- when more than one cluster is visible, open in `Fleet` mode;
- when exactly one cluster is visible or `--cluster` is set, open in `Cluster`
  mode;
- when a command such as `:logs`, `:doctor`, or `:backups` is selected, open a
  focused `Detail` view;
- every mode should share the same header, bottom command/status bar,
  stale-state language, reduced-mode rendering, and contextual help model.

Core keys:

- `q` / `Ctrl-C`: quit
- `j` / `Down`: next row
- `k` / `Up`: previous row
- `Tab`: move focus between panes
- `l`: open CNPG-aware logs for the selected cluster, instance, pooler, backup, or event context
- `r`: force refresh of sampled SQL state
- `/`: filter clusters or instances
- `:`: command mode
- `Esc`: leave command/filter mode or return to the previous focused pane
- `[`: previous view or command history entry
- `]`: next view or command history entry
- `Ctrl-S`: save screen/model dump
- `?`: show help overlay

The UI must make stale data obvious. If Kubernetes watches are connected but SQL sampling fails, topology should continue updating while LSN cells show the last successful sample age and the current error. The UI should never block on a slow probe; it renders the last known snapshot while collectors work in the background.

Ease-of-use requirements:

- Start with the current kube context and namespace unless flags override them.
- Treat the watched database namespace scope and CNPG operator namespace scope as separate settings. The operator commonly runs outside the database namespace, while CNPG CRDs are cluster-wide and a normal Kubernetes API server has one CNPG operator installation managing many CNPG clusters.
- Support `--operator-namespace`, `--operator-selector`, and `--no-operator-watch`. If the operator namespace is not provided, `doctor` should try safe discovery and otherwise report operator evidence as unavailable, not failed.
- Provide `cnpg-watch info` to print config, logs, dumps, cache, and e2e artifact locations.
- Detect missing permissions at startup and show the exact `cnpg-watch setup ...` command that would grant them.
- Offer `doctor` as a non-mutating preflight that checks Kubernetes permissions, CNPG CRD visibility, operator namespace visibility, pod exec access, PostgreSQL role permissions, pooler access, and terminal support.
- Group `doctor` output by feature area: topology, operator, SQL probes, poolers, backups, managed objects, storage, services, certificates, and plugins.
- Prefer readable defaults over required configuration files.
- Keep copyable error messages short enough for a terminal and include detailed logs only in a debug log file or with `--verbose`.
- Make reduced mode explicit. For example: `watching topology; SQL probes disabled: pods/exec forbidden`.
- Show `syncing` or `warming cache` states while watches have not produced an initial snapshot. Do not show `no resources` until the relevant watch has synced.
- Support screen/model dumps for bug reports. A dump should include the rendered text screen, selected cluster key, watch health, sample ages, reduced-mode reasons, and redacted errors.

## Architecture

The process has four main layers:

1. `kube_client`: builds one configured Kubernetes client from local kube config or in-cluster config.
2. `collectors`: maintain live state from Kubernetes watches and periodic Postgres probes.
3. `store`: owns the normalized in-memory model consumed by the UI.
4. `ui`: renders the model with `ratatui` and sends user actions back to the app loop.

Collectors send typed observations over bounded `tokio::sync::mpsc` channels.
One application loop owns the state and applies a synchronous, deterministic
reducer. The same loop handles input, freshness deadlines, and terminal drawing.
Each iteration has a bounded work budget so a busy collector cannot monopolize
the loop. Collectors perform I/O; reducers and renderers do not.

```text
Kubernetes watches ──> bounded topology channel ─┐
Postgres probes ─────> bounded result channel ──┤
Selected logs ───────> bounded log batches ─────┼──> app loop/reducer ──> view model ──> ratatui
Terminal input ─────> separate control channel ┤           │
Freshness deadlines ───────────────────────────┘           └──> effects: resync, refresh, select
```

This avoids scattered mutable state, gives one place to handle deletes and stale data, and keeps rendering deterministic.

Keep three responsibilities separate within that owner:

- `ObservedState`: accepted resource observations, samples, source health, and
  capabilities, keyed by identity and scope.
- `NavigationState`: selected keys, active view, focus, filters, and bounded
  command/navigation history.
- `DerivedState` and `ViewModel`: health findings, fleet rows, and render data
  computed from observations, navigation, terminal size, and supplied time.

Track dirty clusters and changed dependencies. An input-only change should not
recompute fleet health; an unrelated log line should not sort every instance
table. Build rows only when their inputs change, and format/render only the
visible rows. Keep derived caches discardable and testable against full
recomputation. Do not clone the entire resource store on each frame.

The initial implementation uses one owner because it makes ordering and
testing straightforward. If profiling shows that view preparation or drawing
delays reduction, a separate reducer may publish the latest
`Arc<ViewModel>` through `tokio::sync::watch`. That channel should publish small
immutable render data, with no deep clone of the full store. Intermediate
frames may be skipped; resource lifecycle events may not. A per-resource actor
system or shared `Arc<Mutex<AppState>>` is unnecessary for the first version.

Normal watch mode is read-only from the application's perspective. Mutating operations live under `setup` modules and require explicit subcommands. This prevents an operator from accidentally changing RBAC or database roles while trying to observe an incident.

### Topology behavior matrix

The implementation should treat CNPG topology as a small set of explicit cases
instead of inferring too much from object names.

| Situation | Expected behavior |
| --- | --- |
| Normal Kubernetes API server | One cluster-wide CNPG CRD set, one CNPG operator installation, and many CNPG `Cluster` resources across namespaces. `:fleet` shows database clusters, not operators. |
| Operator namespace configured and readable | Show `:operator`, correlate operator events/logs when available, and associate the operator with visible CNPG clusters unless watch-namespace evidence says otherwise. |
| Operator namespace not configured | `doctor` may try safe discovery. Watch mode should still start from database namespace scope and show operator evidence as unavailable until configured or discovered. |
| Operator namespace unreadable | Keep cluster topology, SQL probes, events, and logs that are allowed. Mark operator details as reduced mode with the exact missing permission. |
| CRDs exist but operator is not visible | Show CRD and cluster evidence, mark operator evidence unavailable, and avoid assuming the operator is down unless deployment or pod evidence supports that finding. |
| More than one plausible operator install is visible in the same Kubernetes API server | Show an ambiguity state with namespace, deployment, selector, image/version, and evidence. Do not create a many-operator mental model unless the user explicitly configures a supported exceptional topology later. |
| `vcluster` or separate control plane | Treat it as a separate Kubernetes API boundary, normally reached through a separate kube context. Do not model it as another operator namespace inside the same API server. |
| Reduced RBAC | Degrade only the affected feature. No feature may silently disappear; it must report a capability state and reduced-mode reason. |

### Core state model

Agents should implement these concepts before broad feature work. Names can
change in code, but the ownership boundaries should remain stable.

- `KubeScope`: resolved kube context, database namespace mode, optional cluster
  selector, operator namespace, operator selector, and all-namespaces flag.
- `OperatorState`: discovery method, namespace, deployment, pods, leases,
  events, log capability, metrics capability, CRD version evidence, ambiguity
  evidence, and reduced-mode reasons.
- `ClusterState`: CNPG cluster identity, status, conditions, instances, pods,
  services, endpoint slices, PVCs, poolers, backups, managed objects, image
  posture, plugin posture, and association to operator evidence.
- `CapabilityState`: per-feature availability and profile gate, with typed
  reasons for unknown, forbidden, unavailable, or not-applicable capabilities.
  Sample freshness is a separate property.
- `Observation<T>`: optional last-good typed value and timestamps, independent
  latest-attempt state, source identity, and freshness deadline. Failed attempts
  retain the last-good value.
- `ProbeState`: scheduling state for an identified target, including request ID,
  in-flight ownership, timeout, backoff, and pending refresh intent.
- `HealthFinding`: stable ID, severity, resource key, evidence references,
  first/last observed time, navigation target, and reduced-mode contribution.
- `ViewModel`: render-only rows, panels, badges, selected keys, scroll state,
  and text already wrapped or truncated for the current terminal size.

Collectors should emit typed events into the reducer. Views should never parse
raw Kubernetes objects, SQL rows, log lines, or command output directly.

### Collector ownership

Each collector should define its inputs, outputs, permissions, backoff policy,
freshness behavior, and reduced-mode behavior.

| Collector | Inputs | Outputs | Permission shape | Failure behavior |
| --- | --- | --- | --- | --- |
| Kubernetes watches | CNPG CRDs, pods, services, EndpointSlices, PVCs, events, leases, webhook resources | normalized object events and cache sync state | list/watch/get by namespace or cluster scope | mark affected watches `forbidden`, `syncing`, `stale`, or `failed` without stopping unrelated watches |
| Operator collector | configured/discovered operator namespace, selector, deployment, pods, events, leases | `OperatorState`, operator capability state, ambiguity evidence | namespace-scoped reads plus optional logs/metrics | reduce only operator views when unreadable |
| SQL probes | selected cluster, instance pods, role credentials, profile settings | primary/replica/archive/connection/wait/role samples | `pods/exec` plus PostgreSQL monitor role | short timeout, bounded concurrency, exponential backoff, stale sample display |
| PgBouncer collector | CNPG `Pooler`, pooler pods, admin credentials or generated role | pooler availability and admin stats | pooler reads, optional `pods/exec`, PgBouncer admin SQL | pooler availability remains visible when admin stats are unavailable |
| Log collector | selected resource context, pod log access, source toggles | bounded log events with source, severity, timestamp, redaction status | `pods/log` for selected namespaces | per-source byte and line caps, stream errors shown inline |
| Optional metrics collector | operator or pod metrics endpoint when configured | scrape freshness and selected controller/process metrics | profile-gated endpoint access | disabled or unavailable by default, never required for watch mode |
| Doctor collector | self-subject access review, CRDs, selected resources, optional live probes | capability report and setup suggestions | non-mutating reads and access checks | report unknown when checks cannot be performed |
| Setup generators | capability matrix, profile, namespace settings, admin PostgreSQL credentials only when applying SQL | RBAC manifests, diffs, and PostgreSQL DDL | explicit setup subcommands only | dry-run by default; apply requires explicit `--apply` |

### Performance budgets

Defaults should be conservative and configurable. The goal is to keep the TUI
useful while the database or Kubernetes API is struggling.

| Budget | Default target |
| --- | --- |
| Kubernetes client QPS/burst | 5 QPS / 10 burst until real usage proves it should change |
| Exec concurrency | 2 per process across PostgreSQL and PgBouncer; 1 PostgreSQL bundle per cluster; 1 exec per pod |
| SQL probe timeout | 750 ms end to end for a fast instance bundle, 2 s for a diagnostic bundle, including exec/connect/read/status handling |
| SQL backoff | exponential from 2 s to a 30 s ceiling for transient failures, with bounded jitter; reset after a successful bundle |
| PgBouncer probe timeout | 750 ms, independent from direct PostgreSQL probes |
| Log retention | 2,000 lines or 1 MiB per source, whichever is reached first; 8 MiB total; at most 8 active follow streams |
| Input framing | 16 KiB per log line and 64 KiB per log batch; discard the remainder of an oversized line before reading another |
| Probe output | 256 KiB stdout and 16 KiB stderr per bundle, enforced while reading, before JSON parsing |
| Event retention | 200 per cluster, 2,000 total, maximum age 1 h; evict on time deadlines as well as insertion |
| Queue capacity | control 128 messages/512 KiB; topology 1,024/8 MiB; probe results 128/4 MiB; log batches 64/4 MiB; stop admission at either limit |
| Pending probes | at most 128 intents total and one per target/class; no spawned task per waiting intent |
| Topology payload | at most 1 MiB per normalized event; oversized observations produce an explicit capacity error |
| Topology cache + relist staging | 64 MiB and 20,000 normalized resource entries combined; include old and staged incarnations until commit/cancellation |
| History retention | 100 entries each for commands, filters, and navigation |
| Reducer work | service input/deadlines each turn; yield after 64 bulk events or 4 ms, whichever comes first |
| Render frequency | at most 10 frames per second; skip unchanged frames but redraw when visible ages, freshness, or layout change |
| View row handling | sort/filter in the model, render only visible rows, and keep selection stable across refreshes |
| Support dump size | cap logs and samples, include truncation markers, and prefer redacted structured state over bulk text |

All budgets should appear in `doctor`, `info`, and support dumps. Profiles may
tighten or loosen them, but no profile should allow unbounded probes, logs, or
render loops.

These are initial configurable limits, not measured performance claims. Keep
them in one validated `BudgetConfig`; tables, command output, and tests use
that configuration. Reject zero intervals, invalid sizes, and inconsistent
per-source/global limits at startup. Bound aggregate queues by both message
count and payload size. Include staging buffers, parsed fields, derived caches,
retained inactive sources, and failure diagnostics in memory accounting.

Use byte-budget permits alongside bounded mpsc capacity and retain both permits
until delivery/discard. Reserve a small independent control budget; shutdown
must remain observable even when every queue is full. Charge parsed payload
sizes, not only their smaller wire representation. Keep cache/staging resource
counts and byte ceilings explicit in `BudgetConfig`; an oversized fleet or
observation produces a visible capacity limit with a suggested narrower scope,
not silent resource loss or an endless relist/retry loop.

The API request limiter applies to list pages, watch establishment/reconnect,
exec establishment, and log requests. It does not count frames on established
streams as new requests. Concurrency limits and request-rate limits are
separate controls. Preserve watch recovery and control responsiveness when
probe work is throttled. Performance and soak tests below define how to
measure CPU, memory, latency, and sampling duration.

### Database-under-load policy

`cnpg-watch` must stay useful when PostgreSQL is overloaded, refusing
connections, or slow to answer.

- Prefer Kubernetes watches, CNPG status, events, pod readiness, services, and
  logs over live SQL when SQL probes are failing.
- Never queue unlimited SQL work. A new probe for the same target should replace
  or skip an older pending probe instead of piling up.
- Set `statement_timeout` and `lock_timeout` where possible, and use short
  client-side timeouts regardless of database settings.
- Use lightweight `pg_stat_*`, replication, archiver, and activity views.
  Avoid heavyweight catalog scans in watch mode.
- Keep SQL panels stale-but-visible. Show last successful sample age, error
  class, and current backoff.
- Classify overload separately from authentication, missing role grants,
  network failure, pod exec failure, and PostgreSQL recovery state.
- Keep logs and Kubernetes topology updating when SQL probes are backing off.
- Run expensive diagnostics only from explicit views or commands, and keep them
  profile-gated.

### Redaction contract

Redaction should be deterministic and visible. The default mode should protect
incident artifacts while still leaving enough evidence to debug.

Profiles should map to a redaction mode:

- `strict`: redact query text, client addresses, usernames, database names,
  application names, Secret names, Secret values, connection strings, and file
  paths that may contain credentials.
- `standard`: redact Secret values, connection strings, passwords, tokens,
  query text, and client addresses; show Kubernetes object names and PostgreSQL
  role/database names unless they appear inside credentials.
- `debug`: keep object names and addresses, but still redact Secret values,
  passwords, tokens, and connection strings.

Query text is redacted by default. Client addresses are redacted by default.
Support dumps must include the redaction mode, the fields redacted, and
truncation markers. Setup SQL may print role names and grants, but never prints
admin passwords or generated password values unless an explicit future command
is designed for secret creation.

### Machine-readable contracts

Machine-readable output should be versioned before CI, agents, or downstream
tools depend on it.

- `doctor --output json`: `schema_version`, context, database namespace scope,
  operator namespace state, profile, capability matrix, reduced-mode reasons,
  findings, setup suggestions, and probe budget settings.
- `setup kubernetes-rbac --dry-run --output yaml`: only the generated manifests,
  with deterministic object order and labels identifying managed objects.
- `setup kubernetes-rbac --diff --output json`: `schema_version`, matching,
  missing, changed, extra managed rules, and whether `--apply` would change
  anything.
- `setup postgres-role --print-sql`: deterministic SQL with stable section
  comments, role profile, PostgreSQL major target, and no connection secrets.
- `info --output json`: binary version, build info, config paths, cache/log/dump
  paths, terminal info, profile, resolved budgets, and enabled feature gates.
- screen/model dump: `schema_version`, selected view, selected resource keys,
  capability state, health findings, probe ages, reduced-mode reasons, redacted
  samples, and a text rendering of the current screen.

Dumps expose current scope/watch/topology epochs, resource UIDs, request IDs,
last-good sample metadata, latest-attempt outcome, computed freshness, and
resolved queue/output/stream budgets. Serialize elapsed durations and UTC times
rather than Rust runtime internals. Include rejection/drop/capacity counters
without preserving arbitrary rejected payloads. Redaction applies to identities
and error details according to the selected mode.

Text output can change for readability. JSON and YAML contracts should change
only with a schema version bump or a clearly additive field.

### Implementation dependency graph

The phases below should follow this dependency order:

```text
CLI/config + bounded runtime + terminal lifecycle
  -> core identities, observations, minimal capabilities, replay fixtures
  -> Cluster/Pod adapters -> reducer -> instance view model -> TUI
  -> early k3d proof: monitor authentication, one SQL bundle, timeout cleanup
  -> tested vertical slice: watches -> state -> SQL -> instance table
  -> replay/property tests + performance baseline
  -> doctor/setup contracts + remaining MVP collectors/views
  -> broader capability/profile/query registries as features are added
  -> real PTY smoke + release packaging
```

Implement only the capability entries and query metadata needed by the current
slice. That subset must exist before its doctor output, setup generation, UI
badges, and health findings. Version-tolerant adapters precede their collectors;
view models precede screen snapshots. Expand contracts with each working
feature instead of implementing every future registry up front. Real-terminal
tests validate the assembled app and complement deterministic state tests.

### Release packaging

Initial support is macOS and Linux only.

- Target binaries: `darwin-arm64`, `darwin-amd64`, `linux-amd64`, and
  `linux-arm64`.
- Release artifacts: compressed binaries, checksums, generated shell
  completions, and a minimal container image only if it helps CI or in-cluster
  debugging.
- Install paths: GitHub release binaries first; Homebrew formula after the CLI
  and config paths stabilize.
- Terminal support: document minimum terminal size, color assumptions, and
  fallback behavior for limited color or missing alternate-screen support.
- Config paths: use platform-appropriate config/cache/data directories on
  macOS and Linux, and show them in `info`.
- Windows is out of scope for the initial implementation. Avoid choices that
  make later Windows support impossible, but do not test or package for it yet.

## Lessons from k9s v0.51.0

This design intentionally borrows from k9s where the ideas fit, while staying narrower than k9s. The review covered k9s v0.51.0 docs, README, release notes, changelogs, and source packages including `internal/view`, `internal/watch`, `internal/model`, `internal/model1`, `internal/render`, `internal/client`, and `internal/dao`.

### Features worth adapting

k9s has a mature operator interaction model:

- command mode with `:` commands and aliases;
- regex, inverse regex, label-selector, and fuzzy filtering;
- help overlays that list active mnemonics;
- command and filter history;
- previous/next navigation;
- context and namespace selection;
- readonly mode that disables mutations;
- screen dumps and `info` output that points to logs/config/state;
- wide columns, sort controls, and column navigation;
- skins and context-specific presentation settings;
- plugins, hotkeys, custom views, logs, exec/shell, port-forwarding, benchmarks, XRay topology, Pulses, and Popeye integration.

For `cnpg-watch`, the initial fit is:

- command mode, but only for CNPG/Postgres views;
- filtering, history, help, screen dumps, and info output;
- readonly-by-default watch mode, with setup isolated under explicit commands;
- a topology view inspired by XRay, but scoped to CNPG clusters, instances, poolers, services, pods, replication, and events;
- a health summary inspired by Pulses, but showing Postgres/CNPG signals instead of generic cluster resource counts;
- conservative theme support later, after the default UI is stable.

Initial non-fits:

- arbitrary plugins;
- generic resource browsing;
- Kubernetes mutation shortcuts;
- benchmarking/load generation from the TUI;
- node shell, pod shell, edit, delete, and kill workflows.

These are powerful k9s features, but they would dilute the first version and increase the safety/RBAC surface for an incident-focused Postgres dashboard.

### TUI architecture lessons

k9s v0.51.0 uses a Go TUI stack based on `tview` and `tcell`; `cnpg-watch` will use Rust with `ratatui` and `crossterm`. The useful architectural pattern is not the library choice, but the separation:

- an app shell owns input, pages, command mode, help, status, and flash messages;
- a command interpreter maps text commands to views;
- browser/table views bind resource-specific keys;
- models watch or refresh data and notify listeners;
- renderers turn resource objects into rows;
- a registry maps resource identifiers to data access and render behavior;
- a watch factory owns Kubernetes informer/cache lifetimes.

For `cnpg-watch`, keep the equivalent pieces smaller:

- `app`: input, focus, command mode, history, actions, and reducer snapshots;
- `commands`: parse `:` commands into typed app actions;
- `views`: CNPG-specific view descriptors rather than arbitrary Kubernetes GVRs;
- `collectors`: Kubernetes watches and SQL/PgBouncer probes;
- `renderers` or `view_model`: convert app state to rows/panels;
- `registry`: map `clusters`, `instances`, `poolers`, `archive`, `events`, and `doctor` views to render and command behavior.

### Kubernetes interaction lessons

k9s uses dynamic shared informers keyed by namespace, checks access with self-subject access review before using informers, waits briefly for cache sync, and models `List/Get` operations over informer stores rather than repeatedly calling the API server. It also had to harden v0.51.0 around namespace-specific RBAC checks and cluster-scoped resources.

Apply these lessons:

- keep one watch/cache owner per namespace scope and resource type;
- track cache state as `starting`, `syncing`, `synced`, `restarting`, `forbidden`, or `failed`;
- show `syncing` instead of `no clusters found` until the relevant cache has actually synced;
- do access checks with the correct namespace/resource semantics, especially for cluster-scoped CNPG CRDs and namespace-scoped pods/events;
- use configurable Kubernetes client QPS/burst limits and expose them in `doctor`;
- page list-style API calls where pagination is available or where a metrics-like API can time out;
- skip reducer/render work when incoming watch data is semantically unchanged;
- batch expensive row/probe hydration work rather than spawning unbounded tasks.

### k9s pain points to avoid

The v0.51.0 release notes and historical changelogs show recurring pain points that are directly relevant:

- RBAC checks are easy to get subtly wrong for namespace and cluster-scoped resources.
- Showing `no resources` while caches are still syncing confuses users.
- Large clusters expose API QPS, metrics pagination, and per-item goroutine overhead problems.
- Rendering must respect wide columns and avoid panics on missing or unusual data.
- Context switching can cause redundant API calls and stale styles/state.
- Owner/topology jumps can fail on cluster-scoped or unusual resources.
- Pod status can be wrong when sidecar init containers or unusual container states are present.
- Customization is valuable, but config churn and breaking changes create support load.

For `cnpg-watch`, the response is:

- model sync state explicitly;
- prove RBAC checks with tests and live reduced-RBAC e2e cases;
- cap and batch probe work;
- normalize optional/missing CNPG fields before rendering;
- make context/namespace switching cancel old watches and probes before starting new ones;
- treat topology edges as fallible evidence, not guaranteed links;
- delay broad customization until the core model is stable.

### Evolution takeaways

k9s evolved from a live resource table and logs tool into a full Kubernetes workbench: early changelogs added config discovery, log handling, filtering, and fail-early diagnostics; later releases added traversal, port-forwarding, benchmarks, custom columns, command/filter improvements, per-cluster configuration, plugins/hotkeys, skins, XRay/Pulses/Popeye, and then performance hardening as cluster scale and customization grew.

The lesson for `cnpg-watch` is to sequence the product:

- start with reliable observation, not mutation;
- make logs/config/dumps discoverable from day one;
- build filtering and command history early because operators use them constantly;
- add topology/health summaries before adding extension systems;
- design for scale before users hit it: cache sync state, pagination, QPS, bounded probes, and semantic diffing.

## cnpg-watch feature implications

The first public version should include these k9s-inspired features:

- Command mode with a small command registry.
- Filter mode with regex, inverse regex, label selector where relevant, and fuzzy match.
- Help overlay generated from active actions.
- Command/filter history.
- Previous/next view navigation.
- Screen/model dump and `info` output for support.
- Health summary pane showing watch health, SQL probe health, archive health, replication lag state, and pooler saturation.
- Topology pane showing cluster -> instances -> poolers/events, with fallible edges and missing-data states.
- Explicit `syncing`, `stale`, `forbidden`, `partial`, and `degraded` states.
- Configurable but bounded Kubernetes QPS/burst and SQL probe concurrency.

Features to postpone:

- user-defined plugins and hotkeys;
- arbitrary custom columns;
- Kubernetes edit/delete/kill/restart actions;
- port-forwarding and HTTP benchmarking;
- generic shell workflows and generic log browsing outside CNPG/Postgres contexts.

### MVP boundary

The first usable build should be deliberately smaller than the full design.
The goal is a dependable CNPG/PostgreSQL operational dashboard, not every view
that the product will eventually grow into.

MVP includes:

- namespace or all-namespace discovery of CNPG `Cluster` resources;
- pods, PVCs, services, EndpointSlices, and events associated with visible
  clusters;
- basic operator namespace support, including reduced mode when operator access
  is unavailable;
- `:fleet`, `:instances`, `:health`, `:events`, `:doctor`, and bounded
  selected-resource `:logs`;
- exec-based fast PostgreSQL probes for role, LSN, replay lag, and archive
  status;
- explicit stale, syncing, forbidden, degraded, and reduced-mode states;
- setup dry-runs for Kubernetes RBAC and PostgreSQL monitor-role DDL.

MVP excludes:

- merged cluster/operator log timelines;
- optional operator metrics;
- deep lock/blocker diagnostics;
- certificate reads;
- mutation or action flows from the TUI;
- custom plugin systems or user-authored views.

Alpha can add pooler stats, backup resources, and stronger fleet/operator
health. Beta can add merged logs, managed objects, service/storage drill-downs,
and real PTY smoke coverage. GA requires the reduced-mode matrix, k3d e2e
suite, screen/model dumps, and release packaging to be stable.

## CNPG-specific feature improvements

The first design is intentionally narrow, but it should be more CNPG-native than a `Cluster`/pod watch plus SQL probes. CloudNativePG exposes operational resources and status surfaces that map directly to incidents operators care about. The tool should add them in phases without becoming a generic Kubernetes browser.

### Cluster status and readiness

Improve the cluster header and instance table with a CNPG status adapter that normalizes:

- cluster phase, conditions, observed generation, current primary, target primary when present, ready instances, and online update state when available;
- failover/switchover indicators, including demotion or promotion evidence exposed by the CRD status;
- failover quorum state when the `FailoverQuorum` resource is present;
- synchronous replication mode and current synchronous standbys when this can be derived from CNPG status or SQL;
- pending restart/reload, rolling update, fenced instance, and hibernation indicators;
- operator/plugin-reported degraded states as first-class warnings rather than raw JSON details.

The first release should remain observe-only for failover and switchover. Showing readiness and risk is in scope; initiating promotion, switchover, or failover is not.

### Storage and PVC topology

CloudNativePG manages PostgreSQL instance PVCs directly rather than through StatefulSets, so storage needs its own panel instead of being hidden behind pod readiness.

Add a `:storage` view that shows:

- data, WAL, and tablespace PVCs associated with each instance;
- PVC phase, capacity, storage class, selected node/zone when known, and resize-pending conditions;
- bound PV and pod scheduling mismatches that explain why an instance is pending;
- persistent volume or filesystem pressure events related to the selected cluster.

The topology view should treat PVC readiness as a separate signal from pod readiness. A pod can be missing because storage is stuck, and the UI should say that plainly.

### Backup, WAL, and recovery resources

Add a `:backups` view backed by CNPG `Backup` and `ScheduledBackup` resources, plus the existing `pg_stat_archiver` probe.

The view should show:

- last successful backup, active backup, failed backup reason, and backup age;
- scheduled backup cron/spec summary, suspended schedules, last schedule time, and next inferred schedule time where safe;
- backup target such as primary or prefer-standby when the CRD exposes it;
- backup method: CNPG-I plugin, Barman object store, or volume snapshot;
- retention policy and whether WAL archiving appears consistent with backup expectations;
- restore/PITR context when the selected cluster was created from recovery metadata.

The archive panel should link WAL failures to backup configuration evidence when possible. For example, a broken Barman object store target should appear as backup/WAL context, not only as a SQL archiver failure.

### CNPG-I plugin visibility

CloudNativePG exposes plugin status and plugin capabilities in cluster status. Add a compact plugin panel or `:plugins` view that shows:

- plugin name, version, and reported capabilities;
- WAL archive/restore capability, backup capability, and restore hook capability when reported;
- missing or incompatible plugin status for clusters that depend on plugin-backed backup or WAL behavior;
- plugin status freshness and whether the operator has observed the current cluster generation.

This matters because CNPG-I plugins can be the reason backup, WAL, or restore behavior is degraded even when PostgreSQL itself is healthy.

### Pooler deep integration

The existing PgBouncer plan is good, but it should use the CNPG `Pooler` resource more fully.

Improve the pooler panel with:

- Pooler phase/status, desired instances, ready instances, unavailable instances, and reason/message fields when available;
- pooler type, service name, generated secret/config references, image/version, and config age;
- EndpointSlice readiness for the pooler service;
- PgBouncer `SHOW POOLS`, `SHOW STATS`, and optionally `SHOW DATABASES` when the configured stats user permits it;
- separate reduced-mode messages for missing Pooler watch access, missing pod access, missing exec access, and missing PgBouncer stats credentials.

Pooler health should be independent from direct PostgreSQL health. A bad pooler can explain application outage while the database remains healthy.

### Managed PostgreSQL objects

CNPG now includes declarative resources for managed PostgreSQL objects. Add `:databases`, `:roles`, `:publications`, and `:subscriptions` views after core topology is stable.

These views should watch:

- `Database` resources and their reconciliation status;
- `DatabaseRole` resources, including statuses such as reconciled, pending reconciliation, not managed, and reserved;
- `Publication` and `Subscription` resources for logical replication topology;
- related Kubernetes events and SQL evidence where a reconciled object still has runtime lag or errors.

The dashboard should not edit these objects initially. It should show desired-vs-reconciled state and make permission gaps obvious.

### Replica and distributed topology

CNPG supports replica clusters, delayed replicas, external clusters, standby-style topologies, HA replication slots, and logical decoding failover. Add a `:replicas` view that combines CNPG status with SQL runtime probes.

The view should show:

- upstream/downstream cluster relationship when exposed by status/spec;
- delayed replica configuration versus observed replay delay;
- current timeline and promotion history evidence where available;
- physical replication slot state, inactive slots, restart LSN, confirmed flush LSN for logical slots, and slot lag;
- synchronous replication quorum or priority state and whether expected standbys are actually synchronous.

This should reuse the existing SQL backoff rules because replication-slot queries can still hit a stressed primary.

### Services, endpoints, and app connectivity

Add a `:services` view for CNPG-generated services and endpoint slices:

- read-write, read-only, and replica service names;
- endpoint count, ready endpoint count, and target pod names;
- mismatches where the read-write service does not point at the current primary;
- missing or stale endpoints after failover;
- service selector and label evidence used to associate services with a cluster.

This gives operators a path from "database is healthy" to "applications still cannot connect" without turning the product into a generic service browser.

### CNPG-aware log viewer

k9s has an effective log viewer, but `cnpg-watch` should not copy it as a single-pod tail. Logs should be grouped around CNPG operating questions and correlated with the dashboard model.

Add a `:logs` view and `l` shortcut with these scopes:

- selected cluster: merged timeline from the primary, replicas, poolers, matching CNPG events, and optionally CNPG operator logs from the configured operator namespace;
- selected instance: PostgreSQL container logs for that pod, with role, timeline, LSN sample age, and restart count in the header;
- selected pooler: PgBouncer logs plus pooler pod readiness, endpoint readiness, and connection-count sample state;
- selected backup/scheduled backup: related backup pod/job logs when discoverable, CNPG events, WAL/archive status, and plugin status;
- selected event: logs from the involved pod or CNPG resource neighborhood around the event timestamp.

The log viewer should support:

- follow mode, pause/resume, jump to newest/oldest, and time-window selection such as last 5m/15m/1h;
- regex, inverse regex, and severity filters;
- source toggles for primary, replicas, poolers, backup jobs, operator, and events;
- stable merged ordering by timestamp with a visible fallback when timestamps are missing or malformed;
- parsing CNPG/PostgreSQL JSON logs when present, while preserving raw lines for non-JSON output;
- highlighting PostgreSQL severities such as FATAL, PANIC, ERROR, WARNING, checkpoint, archive, replication, connection, and authentication messages;
- redaction of passwords, connection strings, tokens, and secret-looking values before rendering, dumps, or test artifacts;
- bounded memory and byte limits per source so a noisy pod cannot freeze the TUI.

The log view should be evidence-driven. If the current primary changed, the merged cluster log should annotate that transition. If the read-write service points at a stale endpoint, the log timeline should let the operator include service/endpoint events next to PostgreSQL and pooler logs. If the CNPG operator runs in another namespace, operator log source labels should show that namespace explicitly. If a database is overloaded and SQL probes are backing off, the logs should keep streaming when `pods/log` is allowed.

Initial implementation should read Kubernetes pod logs through the API, not by shelling out to `kubectl logs`. It should request `tailLines` and `sinceSeconds` by default, then follow only the selected scope. Do not start unbounded follow streams for every visible pod.

### Certificates and security posture

Add a `:certs` view when Kubernetes secret permissions are granted, and a reduced-mode summary when they are not.

The view should show:

- server, client, and CA certificate expiration windows;
- secret age and rotation status when inferable;
- warnings for soon-to-expire certificates;
- redacted secret names and no private key material in dumps.

Secret reads should be optional. The main dashboard must remain useful without secret access.

### Images and version posture

Add version posture to `doctor` and the cluster detail view:

- CNPG CRD versions seen and whether the project has fixtures for them;
- cluster PostgreSQL image, major version, `ImageCatalog` or `ClusterImageCatalog` usage, and image/catalog mismatch warnings;
- deprecated or unusual image configuration surfaced as a warning;
- operator version evidence when it can be read from deployment labels, pod labels, container images, or CRD metadata.

This is especially useful for support dumps because many subtle issues are version-specific.

### Operator namespace support

CloudNativePG operator pods usually run in a control namespace such as
`cnpg-system`, while `Cluster` resources and PostgreSQL pods live in
application namespaces. CNPG CRDs are cluster-wide, so the normal model for one
Kubernetes API server is one CNPG CRD set, one CNPG operator installation, and
many CNPG `Cluster` resources spread across namespaces.

The design must model the operator namespace separately from database
namespaces without implying that multiple operators are the common case.
Multiple operator evidence should be treated as exceptional: migration,
misconfiguration, overlapping installs, or separate virtual clusters. A
`vcluster` should be treated as a separate Kubernetes API boundary, normally
accessed through a separate kube context, not as another operator namespace
inside the same cluster.

Add an operator namespace model with:

- explicit `--operator-namespace` support for the normal single operator
  namespace;
- optional `--operator-selector` for installs that use non-default labels;
- `--no-operator-watch` for locked-down environments;
- safe discovery in `doctor` by checking common namespaces and label selectors when RBAC allows it;
- reduced-mode output when operator namespace, deployment, pods, or logs cannot be read;
- defensive ambiguity reporting when evidence suggests more than one operator
  install in the same Kubernetes API server.

Operator evidence should include:

- operator deployment/pod readiness and restart counts;
- operator image/version when visible;
- operator log stream availability;
- CNPG CRD version evidence;
- recent operator events if available.

Operator failures should not block normal cluster observation. They should enrich `:health`, `:hints`, `:logs`, `:plugins`, and `doctor` when available.

### Operator dashboard

Add a dedicated `:operator` view. It should optimize for one visible operator
installation and show:

- operator namespace, deployment name, selector, and discovery method;
- deployment desired/ready/unavailable replicas;
- operator pod readiness, restarts, node, image, age, and last termination reason;
- leader or active controller pod when detectable from leases, logs, or labels;
- operator image/version and CRD version evidence;
- operator watch/log/metrics capability state;
- recent operator events and warning log snippets;
- reduced-mode reasons when the operator namespace is forbidden, missing, ambiguous, or intentionally disabled.

If multiple operator installs appear in the same Kubernetes API server, the view
should show an `ambiguous` banner and list the evidence. It should not assume
that each CNPG cluster has a different operator unless the association evidence
is explicit. In normal operation, `:fleet` is about many CNPG database clusters,
not many operators.

### Multi-cluster fleet view

Add a `:fleet` view for environments with many CNPG clusters in one Kubernetes cluster. This should be the default overview when `--all-namespaces` is used or more than one cluster is visible.

The fleet view should group by namespace and show one row per CNPG cluster:

- namespace/name, phase, primary, instance count, ready count, and restart summary;
- cluster health severity and top reason;
- lag summary and stale SQL probe indicator;
- backup age/failure state;
- pooler availability and waiting-client summary;
- storage/service warning badges;
- Postgres image/version and CNPG image/catalog posture;
- operator evidence state and whether operator details are available.

Fleet rows should be navigable. Entering a row selects that cluster and opens the normal cluster dashboard. Filters should work across namespace, cluster name, health reason, image, and operator evidence state.

### Operator-to-cluster association

Add an association model that links visible clusters to the configured or
discovered operator without pretending uncertain evidence is certain. In the
normal case, a single visible operator is associated with all visible CNPG
clusters in the same Kubernetes API server unless scope restrictions indicate
otherwise.

Evidence can include:

- explicit `--operator-namespace` or configured operator namespace;
- cluster namespace allowed by an operator deployment's watch namespace configuration when visible;
- operator logs referencing a cluster namespace/name;
- operator events involving a CNPG resource;
- fresh `observedGeneration` and cluster conditions;
- CRD versions and controller labels.

Association confidence should be `explicit`, `likely`, `weak`, or `unknown`. The UI should show `operator: unknown` when evidence is insufficient. Health reasons and hints should not blame an operator unless association confidence is explicit or likely.

Multiple visible operator installs should usually lower confidence and produce
an ambiguity reason rather than creating a many-operator mental model.

### Reconciliation health

Add reconciliation health to `:operator`, `:fleet`, `:health`, and cluster detail views.

Signals:

- `generation` versus `observedGeneration` age for `Cluster`, `Pooler`, `Backup`, `ScheduledBackup`, and managed-object resources;
- stale or unchanged conditions after spec changes;
- pending `Database`, `DatabaseRole`, `Publication`, or `Subscription` reconciliation;
- pooler desired/ready drift;
- backup/scheduled-backup reconcile failures;
- operator log lines around reconcile errors, matched by namespace/name when possible.

This should answer "is the operator currently reconciling this desired state?" better than pod readiness alone.

### Operator log correlation

Extend `:logs` so operator logs are correlated instead of dumped wholesale.

For a selected cluster, include operator log lines only when they likely relate to that cluster, based on:

- namespace/name in structured fields or message text;
- involved object from a nearby Kubernetes event;
- reconcile ID or controller request key if present;
- timestamp proximity to cluster condition changes, failover, backup, pooler, service, or managed-object events.

The operator log source should still allow a full operator view from `:operator`, but cluster-scoped log mode should default to relevant lines. Unmatched operator lines should be available through a source toggle, not mixed into the cluster timeline by default.

### Admission and webhook health

Add webhook/admission health to `:operator` and `doctor` when the installed CNPG operator exposes admission webhooks.

Show:

- validating and mutating webhook configuration presence;
- webhook service name, namespace, and EndpointSlice readiness;
- backing pod readiness;
- webhook certificate expiration when readable;
- recent admission failures from events or API errors;
- reduced-mode reasons when webhook configuration or secret access is forbidden.

This helps diagnose failures when users cannot create or update CNPG resources even though existing clusters continue running.

### Optional operator metrics

If operator metrics are available directly or through a configured endpoint, add optional metrics support without making Prometheus a requirement.

Useful metrics:

- reconcile error count and recent rate;
- reconcile duration summaries;
- workqueue depth, retries, and longest-running items;
- controller-runtime API request errors;
- operator process restarts and scrape freshness.

Metrics should be profile-gated and reduced-mode aware. They should enrich `:operator`, `:fleet`, `:health`, and `:hints`, but the product must remain useful without them.

### Fleet-level health reasons

Add fleet-level health reasons alongside per-cluster reasons.

Examples:

- `critical: 2 clusters have read-write endpoint mismatches`
- `critical: operator cnpg-system has no ready pods`
- `warning: 5 clusters have stale observedGeneration`
- `warning: 3 namespaces lack pods/exec; SQL probes unavailable`
- `warning: 4 clusters have failed scheduled backups`
- `info: operator metrics unavailable for cnpg-prod`

Fleet health should deduplicate noisy per-cluster issues and show the affected cluster count, namespaces, and a drill-down target. In `--all-namespaces` mode, fleet health should be visible even before a specific cluster is selected.

### CNPG doctor checks

Extend `doctor` beyond generic RBAC:

- CRDs present for `clusters`, `poolers`, `backups`, `scheduledbackups`, `databases`, `databaseroles`, `publications`, `subscriptions`, `failoverquorums`, `imagecatalogs`, and `clusterimagecatalogs` when supported by the installed CNPG version;
- operator namespace, operator deployment/pod visibility, and operator log access when configured or discovered;
- access to pods, events, PVCs, services, endpoint slices, and optional secrets;
- access to `pods/log` for CNPG-aware log views;
- access to `pods/exec` for PostgreSQL and PgBouncer probes;
- whether CNPG labels needed for ownership matching exist on live objects;
- whether the selected cluster has poolers, scheduled backups, plugin status, and managed-object resources;
- whether a reduced runtime role can still produce a meaningful dashboard.

The output should group missing checks by feature. For example, "backup resources unavailable" is more useful than a flat list of missing verbs.

### Health scoring with reasons

Add a `:health` view and a compact health reason strip on the main screen. This should not be a vague score. It should be a ranked list of concrete reasons derived from CNPG, Kubernetes, SQL probes, PgBouncer, services, storage, backup state, logs, and permissions.

Examples:

- `critical: read-write service endpoint does not match current primary`
- `critical: primary SQL probes backing off: too many connections`
- `warning: replica payments-2 replay lag 1.4 GiB`
- `warning: latest scheduled backup failed 3h ago`
- `info: pods/log forbidden; log timeline unavailable`

The health model should:

- calculate severity as `critical`, `warning`, `info`, or `ok`;
- keep source evidence with each reason, such as object key, probe name, timestamp, and linkable view target;
- deduplicate related symptoms so one bad pooler does not create ten equivalent warnings;
- rank user-impacting connectivity and data-safety issues above cosmetic or optional-feature issues;
- expose stale evidence separately from current failures;
- require compatible, time-bounded WAL evidence for lag and distinguish the
  last-replayed-transaction age from replication delay on an idle primary;
- make reduced-mode gaps visible without treating every missing optional permission as a cluster outage.

Health reasons should drive navigation. Pressing enter on a reason should jump to the most relevant view, such as `:services`, `:connections`, `:backups`, `:logs`, or `:doctor`.

### Contextual runbook hints

Add a `:hints` view backed by deterministic rules. Hints should be short, contextual, and non-mutating. They should explain the next useful observation or setup step without pretending to automate incident response.

Examples:

- missing `pods/exec`: show the exact `cnpg-watch setup kubernetes-rbac ...` command or the minimal verb/resource needed;
- SQL probes backing off due to `too many connections`: suggest `:connections`, pooler checks, and the role limits involved;
- archiver failures plus backup plugin warning: suggest checking `:backups`, `:plugins`, and recent archive log lines;
- read-write endpoint mismatch after failover: suggest `:services`, relevant events, and CNPG operator logs;
- certificate expiration warning: suggest `:certs` and the secret name, without exposing secret data;
- managed role pending reconciliation: suggest `:roles`, related events, and PostgreSQL permission checks.

Hints should have stable IDs, severity, source evidence, and a dismiss-until-change mechanism for the current process. They should not run writes, restarts, failovers, shell commands, or `kubectl` commands from the TUI.

### Connection pressure view

Add a `:connections` view for PostgreSQL and PgBouncer pressure. This directly supports the goal of doing the best possible job when a database is under load or refusing connections.

The view should show:

- per-instance connection usage: total, active, idle, idle in transaction, waiting, and percent of `max_connections`;
- reserved connection headroom where PostgreSQL exposes it, including superuser/reserved connection context by version;
- top database, user, client address, and application-name buckets with configurable row limits;
- blocked probe state, including when `cnpg-watch` itself is backing off due to connection failures;
- pooler client active/waiting and server active/idle/used counts beside direct PostgreSQL counts;
- warnings for connection storms, idle-in-transaction buildup, pooler waiters, and direct-to-primary traffic bypassing poolers when inferable.

SQL must be lightweight and bounded:

- use `pg_stat_activity`, `pg_settings`, and version-safe columns only;
- aggregate counts in SQL instead of returning full session lists;
- do not show full query text by default;
- redact user, database, client, or application values in dumps when configured;
- stop or slow this probe before lower-cost topology probes if the database is under pressure.

### Wait and query diagnostics

Add a `:waits` view for safe wait-event, blocker, and active-query diagnostics. This is intentionally a summary view, not a full query monitor.

The view should show:

- wait-event counts by `wait_event_type` and `wait_event`;
- blocker/blockee summaries for lock waits;
- longest active transactions and longest active queries, with query text disabled or truncated/redacted by default;
- autovacuum and vacuum activity summaries;
- replication-related waits when visible;
- whether this diagnostic probe is disabled, stale, or backing off due to load.

Safety rules:

- use short statement timeouts and no explicit transactions;
- cap returned rows tightly;
- avoid `pg_locks` joins that can become expensive unless the cluster is small or the profile enables deeper diagnostics;
- make query text opt-in with `--show-query-text` or a profile flag, and redact it from dumps by default;
- never cancel queries or terminate sessions from the first release.

### Configuration profiles

Add runtime profiles so operators can choose the safety/capability envelope without building a custom config file first.

Initial profiles:

- `minimal`: Kubernetes topology, CNPG resources, services, storage, events, no SQL exec, no logs, no secrets;
- `standard`: topology, SQL health probes, poolers, backups, services, storage, events, health reasons, and runbook hints;
- `incident`: standard plus CNPG-aware logs, connection pressure, wait diagnostics, richer support dumps, and shorter refresh intervals within safe concurrency limits;
- `secure`: standard with no secret reads, no query text, stricter redaction, and conservative dump contents.

Phase 2 profile defaults are 5 s for minimal, 2 s for standard, 1 s for
incident, and 3 s for secure. An explicit refresh overrides these defaults and
survives profile changes. Queue/concurrency limits remain shared; pending
collectors are reported as unfinished rather than started speculatively.

Profiles should be selectable with `--profile`, shown in the top bar and `doctor`, and inspectable with `:profile`. A profile controls which collectors are enabled, probe intervals, log time windows, redaction strictness, and optional features. RBAC setup should be able to render manifests for a target profile:

```sh
cnpg-watch setup kubernetes-rbac --profile standard --namespace app-db --service-account cnpg-watch --dry-run
cnpg-watch setup kubernetes-rbac --profile incident --namespace app-db --service-account cnpg-watch --dry-run
```

Profiles should not hide reduced-mode state. If the user selects `incident` but lacks `pods/log`, the dashboard should clearly say that incident logging is unavailable while the rest of the profile continues.

## Design hardening contracts

The feature list above describes what the tool should show. The contracts below
describe how the implementation stays buildable, safe under load, and
consistent across views.

### Rust types and module boundaries

Use the type system for operational invariants:

- Newtypes distinguish `Lsn`, `TimelineId`, `ScopeEpoch`, `WatchEpoch`,
  `RequestId`, resource UID, and PostgreSQL system identifier. Keep namespace
  and cluster-scoped keys distinct; include resource kind where keys are mixed.
- Implement parsing and validation with `TryFrom`/`FromStr`. Store LSNs as
  `Lsn(u64)` after parsing and format them at output boundaries. Use checked
  arithmetic; unsupported comparisons return a typed reason instead of zero.
- External DTOs tolerate missing and additional fields, then validate into
  internal observations. `DynamicObject` and `serde_json::Value` stay inside
  adapters/transport parsing. Domain code consumes typed results and errors.
- Enums represent meaningful alternatives and carry the required data. Do not
  combine availability, freshness, and last-attempt outcome into a single enum,
  or use independent booleans that admit contradictory capability states.
- Keep the reducer, health rules, scheduling decisions, and view conversion
  pure. Pass time explicitly and return typed effects for I/O work.
- Use a generic executor or a small transport enum for the initial SQL
  implementations. Native async trait methods do not make a trait usable as
  `dyn SqlExecutor`; introduce boxed futures or an async-trait adapter only if
  runtime dispatch is actually needed. Require `Send` futures at spawned-task
  boundaries where the runtime requires them.

Start with one package exposing a library for domain code and a thin binary
entry point. Add modules when a working feature needs them. The module outline
is a responsibility map, not a requirement to create empty files or public
traits. Keep test helpers and fixtures out of the release binary.

### Observation identity and acceptance

Namespace/name identifies a lookup location; UID identifies its current
incarnation. Every asynchronous event carries a `ScopeEpoch`. Increment it
when context, namespace scope, or selectors change, before cancelling old work.
Each watch group also has a `WatchEpoch`; each SQL attempt carries a monotonically
assigned `RequestId` and the target's resource identity.

The reducer accepts a result only when:

1. Its scope and collector/watch epoch are still active.
2. Its cluster and pod UIDs match the current target incarnation.
3. Its request ID matches the active attempt for that probe target; a completed
   or cancelled attempt cannot overwrite a newer value.
4. Its topology epoch still matches when interpreting role/timeline-dependent
   data. A primary change, observed promotion, or incompatible timeline change
   invalidates pending derived lag work and triggers role rediscovery.

Deletes apply only to the matching UID. A late delete for a previous pod must
not delete its replacement. UID changes clear incarnation-specific samples,
counter baselines, and capability caches. Cancellation reduces wasted work;
the acceptance rules remain necessary for events already queued at cancellation.
Resource versions are opaque Kubernetes cursors, not numeric clocks to compare
across unrelated objects.

### Watch initialization and relist reconciliation

Adapt the pinned kube-runtime watch API into an explicit transaction:

1. `Init` starts a fresh staging set for one resource/scope/watch epoch and
   marks that watch syncing or restarting.
2. `InitApply` records every listed object's key and UID in that set. Compare
   normalized contents for changed data, but do not deduplicate away membership.
3. `InitDone` atomically replaces that watch's accepted set. Remove objects
   absent from the completed list, even if no delete was received.
4. An interrupted initialization discards staging and preserves the previous
   complete set with degraded source health. Never publish a partial list as
   synced or remove objects based on an incomplete initialization.

Initial empty lists become `empty` only after completion. Ordinary `Apply` and
`Delete` events update the accepted set after initialization. Watch errors are
stream errors with separate health events; they are not resource deletes.
Adapt these semantics if a pinned kube-runtime version uses different names.

Do not coalesce initialization boundaries or deletes in a generic latest-value
queue. Backpressure the watch reader, or explicitly invalidate and resync the
watch if delivery integrity is lost. Bound staging memory and report capacity
errors; preserve the last complete set rather than silently truncating it.

### Queue, framing, and task lifecycle

Use separate bounded channels for control, topology, probe results, and log
batches. The app loop services control and due deadlines between bounded bulk
batches, and gives each ready data class a turn. Avoid draining a channel until
empty or using a biased select that can starve another class indefinitely.

| Traffic | Full-queue behavior |
| --- | --- |
| Input/control | Coalesce repeated resize/refresh intents; keep shutdown observable through a cancellation token outside the queue |
| Topology/relist | Backpressure the reader; if an event is lost, mark that watch unsynced and request a complete resync |
| Probe results | Await bounded delivery with cancellation; keep scheduling permits until the result is delivered or discarded |
| Pending probes | At most one pending intent per target/class; merge repeated refreshes and reject excess new targets with a visible budget reason |
| Logs | Drop bounded batches or oldest retained entries, record counts, and show a truncation marker |

Enforce byte caps while framing log lines and reading stdout/stderr. A retained
line limit does not bound an unbounded `read_line` or `read_to_end`. Drain or
close oversized input without accumulating it, and avoid parsing arbitrarily
large JSON. Bound parsed fields and error text as well as raw payloads. Retain
one canonical log entry with bounded indexes rather than copying it into every
cluster/operator/merged view. Evict inactive source metadata and buffers under
the global budget as the user visits new scopes.

Own every collector task in a task group (`JoinSet` or an equivalent owner),
with parent and per-scope cancellation tokens. On scope changes, advance the
epoch, cancel children, and reject late events. On shutdown, stop admission,
cancel producers, and join tasks within a bounded grace period, then abort and
await remaining local tasks. Restore terminal state through a guard on normal
return, error, and panic. No detached collector or exec task may survive its
owner unnoticed.

Exec timeout handling must cover process status, stdout, stderr, and transport
cleanup. Read output streams concurrently with byte limits to avoid pipe
deadlocks. Check remote exit status even when stdout contains valid JSON. Use
`psql -X -A -t -w -v ON_ERROR_STOP=1` with no interactive TTY, and configure
session timeouts before executing the bundle. Use server-side statement limits
as well as client deadlines: dropping a local future alone does not establish
that remote SQL stopped. Abort/close the attachment and await local cleanup;
prove bounded remote-query termination in the live timeout test. Never put
passwords in command arguments or error artifacts.

### Capability matrix

Capabilities should be modeled as data, not one-off checks in UI code. Each
feature should declare:

- feature ID and display name;
- required Kubernetes verbs, API groups, resources, and namespace scope;
- required PostgreSQL grants or predefined roles;
- required PgBouncer access when applicable;
- required CNPG CRD presence and minimum known version when applicable;
- profile availability: `minimal`, `standard`, `incident`, `secure`;
- whether it works without operator access, `pods/exec`, `pods/log`, or Secret
  reads;
- reduced-mode message and suggested `setup` or manual remediation command.

The first checked-in matrix should cover:

| Feature | Main requirements | Reduced mode |
| --- | --- | --- |
| topology | CNPG `clusters`, pods, events | dashboard cannot start usefully without `clusters`; pod/event gaps degrade rows |
| SQL fast probes | `pods/exec`, monitor role | LSN/archive fields unavailable, Kubernetes state continues |
| logs | `pods/log` for selected scopes | `:logs` disabled only |
| operator | operator namespace deployments/pods/events | operator evidence unavailable, cluster observation continues |
| poolers | CNPG `poolers`, pods, optional PgBouncer stats | availability without connection counts |
| backups | CNPG `backups` and `scheduledbackups` | backup view unavailable only |
| services | services and EndpointSlices | connectivity checks unavailable only |
| storage | PVCs | storage warnings unavailable only |
| certs | selected Secret reads | certificate view unavailable only |
| managed objects | Database/DatabaseRole/Publication/Subscription CRDs | managed-object views unavailable only |

`doctor`, `setup`, `:profile`, top-bar badges, and health reasons must all read
from the same capability model.

### Freshness and sample state

Keep three independent dimensions:

- Availability: enabled and available, disabled by profile, unknown, forbidden,
  unavailable, or not applicable, with typed reasons and evidence.
- Observation: an optional last-good value with source identity, received time,
  source timestamp when present, and freshness policy.
- Latest attempt: idle, in flight, succeeded, or failed, including request ID,
  timing, redacted error, and next eligible attempt when backing off.

A forbidden or timed-out latest attempt may coexist with a stale last-good
value. Mark retained SQL evidence stale immediately after a failed refresh,
with its actual age and failure reason, even if the normal age threshold has
not elapsed. A never-observed target has no last-good value. Do not overwrite
a valid sample with an empty failure or invent zero values for missing evidence.

Pass `Now` into reducers, health rules, and schedulers. Use monotonic time for
deadlines and elapsed durations; use wall-clock/source time for display and
correlation. Serialize ages and UTC timestamps in dumps, not process-local
`Instant` values. Tests control the same clocks across the application, not
only the UI. Avoid unsynchronized `std::time::Instant::now()` calls in domain
code when testing with Tokio's paused clock.

Maintain a next-deadline schedule for sample expiry, evidence expiry, event
eviction, and backoff eligibility. Time/deadline events invalidate affected
health and view data even if collectors produce no new observations. Visible
age labels may redraw at a coarser cadence, within the render cap. Skipping an
unchanged frame must not freeze freshness or the status bar's age labels.

Watch connection health, completed cache synchronization, and object last-change
time are different facts. An object that has not changed for an hour is still
current evidence when its watch is healthy and synchronized. Use connection
errors, watch progress/timeout handling, and relist completion to assess source
health; do not expire Kubernetes objects solely because no update arrived.
Derived findings inherit all required evidence freshness and compatibility.

### Probe scheduler

The probe scheduler should run independent probe classes with explicit
priority, timeout, interval, and backoff rules.

| Class | Purpose | Default behavior |
| --- | --- | --- |
| `core_kube` | watches and cache state | always on when authorized |
| `fast_sql` | role, LSN, replay, archive | short timeout, high priority |
| `medium_sql` | replication slots, settings, sync state | lower frequency |
| `deep_sql` | waits, blockers, long-running transactions | profile-gated, backs off first |
| `pooler` | PgBouncer admin stats | independent from direct PostgreSQL health |
| `logs` | selected-scope pod logs | lazy, bounded, selected scope only |
| `operator_metrics` | optional controller metrics | disabled by default |

Each class needs:

- default interval and timeout;
- jitter range;
- max concurrency and per-target concurrency;
- stale threshold;
- backoff floor and ceiling;
- overload behavior;
- profile gate;
- reduced-mode reason when disabled by permissions.

Under database pressure, disable or slow `deep_sql` before `medium_sql`, and
slow `medium_sql` before `fast_sql`. Kubernetes watches and bounded selected
logs should continue when their permissions are available.

A scheduled unit is one **instance bundle**: one connection/exec invocation,
one bounded JSON result, and one end-to-end deadline. A fast bundle includes
role detection and the compatible LSN, replication, and archive observations
for that role. Separate query IDs may contribute to a bundle, but the interval
does not promise a complete cluster sweep every two seconds. A primary with
multiple queries should not spawn an exec per cell.

| Target/class | Normal interval | Stale after | End-to-end deadline |
| --- | --- | --- | --- |
| Selected cluster, fast instance bundle | 2 s | 10 s | 750 ms |
| Unselected cluster, fast instance bundle | 30 s | 90 s | 750 ms |
| Medium SQL bundle | 30 s | 90 s | 2 s |
| Deep SQL bundle | 10 s while its diagnostic view/profile is active | 30 s | 2 s |
| Selected pooler stats bundle | 5 s | 15 s | 750 ms |

Unselected SQL sampling is off in MVP: fleet rows show Kubernetes health plus
explicit cached/never-sampled SQL state. Background fleet SQL may be enabled
later under the 30 s policy and the same process-wide admission limit; it must
not create a probe task for every visible pod at each tick. Other unselected
diagnostic and pooler probes remain off. Log streams stay lazy and selected.

The normal interval is a minimum spacing between starts for a target, not a
catch-up obligation. Eligibility also requires the prior attempt and cleanup
to finish, backoff to expire, and global/per-cluster/per-pod permits plus API
rate budget. Limit PostgreSQL to one active bundle per cluster and all exec to
two per process, with one per pod. PgBouncer has independent failure/backoff
state but shares the process exec budget. Use fair admission across clusters
and classes; fast probes have priority, with bounded opportunities for other
eligible work so it cannot starve indefinitely under normal load.

Apply up to 10% positive interval/backoff jitter through an injected seeded
random source, capped at the 30 s backoff ceiling. Use completion/deadline
scheduling or `MissedTickBehavior::Skip`, never a burst of missed probes after
a runtime stall. Keep one coalesced pending intent per target/class, subject
to the global pending limit. A forced refresh coalesces with that intent and
retains backoff, rate, and concurrency limits. Manual refresh cannot create a
connection storm.

Classify transient overload/network failures separately from authentication
and permission failures. The latter should suspend the affected capability
until credentials/configuration change or an explicitly rate-limited capability
recheck is requested. Do not repeatedly retry a known-invalid password every
two seconds. Publish throttled/disabled reasons and actual sweep duration, so
operators can distinguish a configured interval from an achieved sample rate.

Measure the admission model before increasing concurrency. For example, five
serialized 700 ms calls take at least 3.5 s, even with a 2 s interval setting.
Bundle inexpensive queries, accept older fleet samples, or reduce frequency
before increasing load on an already struggling database.

### SQL transport abstraction

SQL probes should not be coupled to Kubernetes exec. The initial candidate is
pod-local exec with an explicit monitoring-role authentication mechanism. The
early live proof below must establish that mechanism before the transport is
accepted. The probe layer depends on a small executor interface:

```rust
trait SqlExecutor {
    async fn execute(
        &self,
        target: &SqlTarget,
        bundle: &ProbeBundle,
    ) -> Result<BoundedProbeOutput, ProbeError>;
}
```

Initial transports:

- `PodExecSqlExecutor`: execs into CNPG PostgreSQL pods and runs `psql`.
- `PgbouncerAdminExecutor`: execs into pooler pods and runs PgBouncer admin
  commands.

`BoundedProbeOutput` carries capped output plus successful remote process
status; the parser produces the registered typed result. Raw bytes/JSON never
reach the reducer or UI. SQLSTATE, Kubernetes status, timeout phase, and exit
status should feed typed errors when available. For psql, request stable error
verbosity/locale and test classification fixtures; use an explicit unknown
class when stderr does not support a reliable diagnosis.

### Monitoring authentication and transport proof

Pod-local execution removes the need for a locally reachable PostgreSQL
service. It does not establish authentication as a dedicated monitor role.
CNPG's default local HBA rules use peer authentication; a process running as
the container's PostgreSQL operating-system user cannot assume that selecting
`-U cnpg_watch` will authenticate as the monitoring role.

Evaluate these options in the first live k3d experiment:

| Option | Benefit | Requirement/cost |
| --- | --- | --- |
| Pod exec + loopback TCP as monitoring role | Keeps pod locality and the proposed transport | Explicit credential source/delivery, matching HBA/TLS rules, psql startup and connection cost |
| Pod exec + configured peer mapping | Can avoid a password | Installation-specific HBA/identity mapping with explicit setup and validation |
| Port-forward + native PostgreSQL client | Structured protocol errors and possible connection reuse | Port-forward lifecycle, credentials/TLS, and a native client dependency |

Prefer the first option if the proof succeeds with a practical credential
delivery path. Record the selected mode, credential source and delivery
mechanism, socket/host/database/role settings, required grants, and exec RBAC
in the transport ADR. Credentials may come from a user-supplied protected
source or an explicitly authorized Secret; watch mode must not create mounts,
Secrets, HBA rules, or roles. Secret reads remain optional and unavailable SQL
must degrade cleanly when no authorized credential path exists. Do not fall
back silently to the container's superuser connection. A low-privilege SQL role
limits the tool's queries; Kubernetes pod-exec access remains a broader
capability and must be described accurately.

The proof must execute the shipped bundle with `current_user` and
`session_user` confirming the intended role, enforce its timeouts, reject
invalid credentials without prompting, and demonstrate cleanup after timeout
and cancellation. Observe `pg_stat_activity` from the test's separate admin
connection to confirm that timed-out probe work terminates within its stated
server-side bound. Also test replica role changes, remote nonzero exit with
valid-looking stdout, and output caps.

If exec cannot meet the authentication or cleanup contract, evaluate the
port-forward/native-client option before expanding the scheduler. Keep a
concrete transport choice behind the interface; do not build a generic plugin
system. Persistent connection reuse is a measured follow-up, with bounded
connection lifetime and statistics-snapshot handling if adopted.

### Version-gated query registry

PostgreSQL probe SQL should live in a registry instead of ad hoc strings. Each
registered query should declare:

- query ID;
- probe class;
- minimum PostgreSQL major version;
- optional maximum version when a query is superseded;
- required views/functions/columns;
- required grants or predefined roles;
- timeout class;
- expected JSON schema;
- redaction policy;
- fallback query or reduced-mode reason.

Startup capability detection should record `server_version_num`, readable views,
optional columns, and role membership before enabling version-specific probes.
Shipped probes should use explicit column lists and emit exactly one JSON
document. `SELECT *` should not appear in production probe SQL.

### Health rules

Health should be produced by deterministic rules over normalized state, not by
renderers. A `HealthReason` should include:

- stable ID;
- severity: `critical`, `warning`, `info`, or `ok`;
- concise message;
- source evidence;
- confidence;
- freshness;
- target view and selected object when the user presses enter;
- deduplication key;
- related runbook hint IDs.

Health ranking should prefer user-impacting connectivity and data-safety
signals over optional feature gaps. Missing optional permissions should usually
be `info`, unless they hide a feature the selected profile explicitly requested
and the user is relying on it during incident mode.

Evaluate rules on accepted observation changes and relevant time deadlines.
Invalidate only affected clusters/rules, with full recomputation available as
a test oracle. Conditions from different watches are not an atomic snapshot:
service/primary mismatch rules should require fresh compatible evidence and a
short, configured persistence window before declaring a critical mismatch
during failover. Show pending/uncertain evidence while sources converge. Rule
tests must cover recovery as well as detection, so cleared evidence removes a
finding rather than leaving a sticky warning.

### Setup safety contract

Setup commands are the only normal place where the tool mutates Kubernetes,
PostgreSQL, or PgBouncer state. Each setup command should support:

- `--dry-run`: print generated objects or SQL without applying;
- `--diff`: compare intended state with readable existing state where possible;
- `--apply`: perform the mutation;
- `--output yaml|json|sql|text` where applicable;
- deterministic output suitable for tests;
- explicit subject/role/profile flags rather than guessing privileged targets;
- redacted handling of supplied credentials;
- idempotent apply semantics;
- no privilege pruning unless `--prune` is explicitly set.

`setup postgres-role` should also support `--print-sql`, password preservation
versus rotation flags, and separate monitor/reserved/action role profiles.
Watch mode should refuse to create or update roles, RBAC, secrets, or pooler
configuration.

### Fixture and artifact contract

The repository should define fixture formats before implementation spreads
parsers through the codebase.

Fixture classes:

- CNPG CR JSON for every supported resource and important version difference;
- Kubernetes watch event streams for apply/delete/restart/error/sync cases;
- timestamped event replays containing scope/watch epochs, UIDs, request IDs,
  initialization boundaries, late completions, and explicit clock advances;
- SQL probe stdout/stderr fixtures for success, timeout, permission denied,
  malformed JSON, unsupported version, and overloaded database;
- PgBouncer `SHOW POOLS`, `SHOW STATS`, and failure fixtures;
- PostgreSQL, PgBouncer, CNPG operator, and Kubernetes event log fixtures;
- reduced-RBAC capability fixtures;
- terminal screen/model fixtures for snapshot tests.

Failure artifacts should include redacted debug logs, rendered screen text,
model dump, capability matrix result, kube context/scope, selected cluster,
probe ages, and reduced-mode reasons.

### TUI safety line

The TUI is observe-only for the first release. It must not expose shortcuts or
command-mode verbs for:

- failover, switchover, promotion, fencing, or hibernation changes;
- query cancel or backend termination;
- backup triggering;
- PostgreSQL config reloads or checkpoints;
- Kubernetes edit/delete/restart/scale actions;
- role, RBAC, Secret, or Pooler configuration changes.

Those operations can be reconsidered later only as explicit action subcommands
with confirmation, audit output, and separate RBAC/PostgreSQL action roles.

## Platform support

Initial support targets:

- macOS on Apple Silicon and Intel.
- Linux on common `x86_64` and `aarch64` distributions.

The binary should use portable Rust libraries and avoid OS-specific shell tools. Terminal handling should rely on `crossterm` through `ratatui`. The first release does not target Windows because terminal behavior, kube config paths, and exec handling should be validated separately before claiming support.

## Data sources

### Kubernetes watches

Use `kube` runtime watchers rather than invoking `kubectl`.

Initial watched resources:

- CNPG `Cluster` custom resources.
- CNPG `Pooler` custom resources.
- CNPG `Backup` and `ScheduledBackup` custom resources.
- CNPG `Database`, `DatabaseRole`, `Publication`, and `Subscription` custom resources when the CRDs exist.
- CNPG `FailoverQuorum`, `ImageCatalog`, and `ClusterImageCatalog` resources when present.
- `Pod` resources for CNPG instances and poolers.
- `Pod` log streams for selected CNPG instances, poolers, backup jobs, and optionally CNPG operator pods.
- `Event` resources for involved objects related to the selected cluster.
- CNPG operator `Deployment`, `ReplicaSet`, `Pod`, `Event`, and optional `Pod` log resources in the configured operator namespace.
- CNPG operator `Lease`, `Service`, `EndpointSlice`, `ValidatingWebhookConfiguration`, and `MutatingWebhookConfiguration` resources when operator/webhook health checks are enabled.
- Optional CNPG operator metrics endpoints when enabled by profile/configuration.
- `PersistentVolumeClaim` resources for data, WAL, and tablespace volume state.
- `Service` and `EndpointSlice` resources for read-write, read-only, replica, and pooler connectivity.
- `Secret` resources only when certificate expiration or generated credential validation is explicitly enabled and RBAC allows it. Secret data must be redacted in logs, dumps, and UI details.

CNPG custom resources should be read through version-tolerant dynamic adapters.
Use `kube::api::DynamicObject` at the Kubernetes boundary, then immediately
convert the object into internal `cnpg-watch` observation models such as
`ClusterObservation`, `PoolerObservation`, `BackupObservation`, or
`ManagedObjectObservation`.

Raw CNPG JSON access is allowed only inside the `cnpg` adapter modules. The
reducer, health engine, probes, and UI must never read CNPG JSON paths
directly. Adapters should return optional fields, source metadata,
missing-field reasons, and reduced-mode/capability evidence instead of
panicking or assuming every field exists.

The data flow should be:

```text
Kubernetes DynamicObject
  -> cnpg::adapter
  -> cnpg-watch internal typed observation
  -> reducer/store
  -> view model
  -> TUI
```

Every adapter path must be covered by fixtures for CNPG 1.30.x and for any
newer CNPG version the project claims to support. Generated CNPG Rust structs
can be reconsidered later inside the adapter layer only, if dynamic adapters
become too large or field handling stabilizes enough to justify generation.

Field names for CNPG status must be verified against the CRD version used by the project before coding. The design assumes the CRDs expose enough status to identify the current primary, readiness, and pooler state, but the SQL probes below are the source of truth for LSN and WAL archive details.

CNPG resource watches should be capability-driven. If a cluster has no `ScheduledBackup` CRD or the runtime identity cannot watch `DatabaseRole`, the affected view should show a reduced-mode message while the rest of the dashboard continues.

Pod log access should be lazy and scoped. The app may watch pods continuously, but it should open log streams only for the active log scope and close them when the user leaves that scope, changes cluster, or narrows source toggles. Historical log reads should default to bounded `tailLines`/`sinceSeconds` requests.

Operator watches should use their own namespace scope and watch status. A
database namespace watch can be healthy while the operator namespace is
forbidden, missing, or intentionally disabled. Context and namespace switching
must advance the scope epoch and cancel both database-scope and operator-scope
watches. Replacements use a new epoch; old queued events cannot update them
even if cancellation races with delivery.

### Postgres SQL probes

Some required facts are PostgreSQL runtime facts, not Kubernetes facts. The first implementation should collect them by opening Kubernetes exec sessions into CNPG instance pods and running `psql` locally inside the container.

This avoids a locally reachable database service and spawning `kubectl`.
Authentication must follow the monitoring transport proof above; pod locality
does not remove the dedicated role's authentication requirement. The transport
also relies on the image having `psql`, as normal CNPG PostgreSQL images do.
If exec, authentication, or psql is unavailable, degrade only the affected SQL
capability and preserve last-good samples.

Probes must be safe under database stress:

- Use a dedicated low-privilege monitoring role created by `cnpg-watch setup postgres-role`.
- Set `application_name='cnpg-watch'`.
- Set short `connect_timeout`, `statement_timeout`, and `lock_timeout` values.
- Run read-only catalog/statistics queries only.
- Avoid explicit transactions and avoid queries that wait on locks.
- Limit one in-flight SQL probe per PostgreSQL pod by default.
- Back off aggressively after connection refused, too many connections, timeout, or admin shutdown errors.
- Keep showing the last successful sample with its age instead of replacing it with empty values.
- Stop probing a pod temporarily when repeated failures suggest the database is under load, while Kubernetes watches and events continue normally.

The probe scheduler must treat connection failures as data. A struggling database is one of the main reasons the operator is using the tool, so failure states should be visible as health signals rather than hidden in logs.

Primary probe:

```sql
select json_build_object(
  'server_time', now(),
  'timeline_id', timeline_id,
  'current_wal_lsn', pg_current_wal_lsn(),
  'current_wal_insert_lsn', pg_current_wal_insert_lsn()
)
from pg_control_checkpoint();
```

Primary replication probe:

```sql
select coalesce(json_agg(row_to_json(t)), '[]'::json)
from (
  select
    application_name,
    client_addr::text,
    state,
    sync_state,
    sent_lsn,
    write_lsn,
    flush_lsn,
    replay_lsn,
    pg_wal_lsn_diff(pg_current_wal_lsn(), replay_lsn) as replay_lag_bytes
  from pg_stat_replication
  order by application_name
) t;
```

Replica probe:

```sql
select json_build_object(
  'server_time', now(),
  'receive_lsn', pg_last_wal_receive_lsn(),
  'replay_lsn', pg_last_wal_replay_lsn(),
  'last_replayed_transaction_age_ms',
    extract(epoch from now() - pg_last_xact_replay_timestamp()) * 1000,
  'in_recovery', pg_is_in_recovery()
);
```

WAL archiver probe:

```sql
select json_build_object(
  'archived_count', archived_count,
  'last_archived_wal', last_archived_wal,
  'last_archived_time', last_archived_time,
  'failed_count', failed_count,
  'last_failed_wal', last_failed_wal,
  'last_failed_time', last_failed_time,
  'stats_reset', stats_reset
)
from pg_stat_archiver;
```

Connection pressure probe:

```sql
select json_build_object(
  'server_time', now(),
  'max_connections', current_setting('max_connections')::int,
  'superuser_reserved_connections', current_setting('superuser_reserved_connections')::int,
  'by_state', (
    select coalesce(json_agg(row_to_json(s)), '[]'::json)
    from (
      select state, count(*) as count
      from pg_stat_activity
      group by state
      order by count(*) desc
    ) s
  ),
  'by_database', (
    select coalesce(json_agg(row_to_json(d)), '[]'::json)
    from (
      select datname, count(*) as count
      from pg_stat_activity
      group by datname
      order by count(*) desc
      limit 10
    ) d
  ),
  'waiting', (
    select count(*)
    from pg_stat_activity
    where wait_event is not null
  )
);
```

Wait summary probe:

```sql
select json_build_object(
  'server_time', now(),
  'waits', (
    select coalesce(json_agg(row_to_json(w)), '[]'::json)
    from (
      select wait_event_type, wait_event, count(*) as count
      from pg_stat_activity
      where wait_event is not null
      group by wait_event_type, wait_event
      order by count(*) desc
      limit 20
    ) w
  ),
  'long_running', (
    select coalesce(json_agg(row_to_json(q)), '[]'::json)
    from (
      select pid, state, wait_event_type, wait_event,
             extract(epoch from now() - xact_start) as xact_age_seconds,
             extract(epoch from now() - query_start) as query_age_seconds,
             md5(query) as query_hash
      from pg_stat_activity
      where state <> 'idle'
      order by greatest(
        coalesce(extract(epoch from now() - xact_start), 0),
        coalesce(extract(epoch from now() - query_start), 0)
      ) desc
      limit 10
    ) q
  )
);
```

These are query components, not a requirement for separate exec invocations.
The registry assembles compatible components into a role-aware instance bundle
that emits one JSON document. Run psql without prompts or a TTY using
`-X -A -t -w -v ON_ERROR_STOP=1`, with session timeouts already set. Parse into
typed DTOs and validate the result; do not parse aligned table output or accept
JSON from a failed remote command as success. Connection pressure and wait
diagnostics have lower priority than core LSN/archive bundles.

`last_replayed_transaction_age_ms` is the age of the last replayed transaction,
not replication delay. It can grow on an idle primary while the replica is
fully caught up, and it is null before replaying any transaction. Do not drive
lag severity from this age alone. Retain source timestamps and receive times
and derive byte lag only from observations with compatible system identity,
role/topology epoch, timeline evidence, and bounded sampling skew. Use a
configurable maximum skew of 2 s initially; return an explicit unknown reason
when compatibility or timing cannot be established. Do not clamp an invalid
or negative cross-sample comparison to zero. Primary-side peer replay positions
can reduce cross-instance timing error when reliably associated with a replica.

### Pooler connection counts

CloudNativePG Pooler resources use PgBouncer, so the design should treat PgBouncer as a first-class integration rather than a best-effort extra. Pooler connection counts are more environment-dependent than instance readiness, so the first implementation should support a layered strategy:

1. Use CNPG `Pooler` status and pooler pod readiness for availability.
2. If PgBouncer admin access is available through the pod, exec `psql` against the PgBouncer admin database and run `SHOW POOLS` / `SHOW STATS`.
3. If admin access is not available, show availability and an explicit `connections unavailable` state instead of guessing.

The UI should separate `desired/ready pooler pods` from `active/client/server connection counts` so partial data is not misleading.

Good pooler integration includes:

- Discover CNPG Pooler objects associated with the selected cluster.
- Display pooler type when available, such as read-write or read-only.
- Show desired, ready, and unavailable pooler pods.
- Show client active/waiting and server active/idle/used/tested counts from PgBouncer when admin or stats access is configured.
- Show pool saturation symptoms clearly, including waiting clients and exhausted server pools.
- Keep PostgreSQL instance probes independent from pooler probes so a PgBouncer outage does not hide direct instance health.
- Let `setup pooler-role` create or configure a PgBouncer stats/admin user when the operator supplies admin PgBouncer credentials or a CNPG-compatible secret target.

The setup command should prefer a stats-only PgBouncer user for watch mode. Admin-level PgBouncer credentials may be used during setup, but they should not be required for normal dashboard operation.

## Permission and bootstrap model

There are two separate permission domains:

- Kubernetes permissions for watching CNPG resources, pods, events, and using pod exec.
- PostgreSQL/PgBouncer permissions for reading runtime statistics.
- Profile-dependent permissions for optional logs, secrets, connection pressure, and wait diagnostics.

The normal dashboard should run with the smallest practical permission set. Bootstrap commands can be run by a human or automation identity with elevated privileges to create the lower-privilege runtime access.

### Kubernetes RBAC setup

`cnpg-watch setup kubernetes-rbac` should be able to print or apply the Kubernetes RBAC needed for a runtime identity.

Example commands:

```sh
cnpg-watch setup kubernetes-rbac --namespace app-db --service-account cnpg-watch --dry-run
cnpg-watch setup kubernetes-rbac --namespace app-db --operator-namespace cnpg-system --service-account cnpg-watch --dry-run
cnpg-watch setup kubernetes-rbac --namespace app-db --operator-namespace cnpg-system --service-account cnpg-watch --diff
cnpg-watch setup kubernetes-rbac --namespace app-db --service-account cnpg-watch --apply
cnpg-watch setup kubernetes-rbac --all-namespaces --service-account cnpg-watch --apply
```

The command should support at least these subjects:

- a namespaced `ServiceAccount` for in-cluster or impersonated use;
- a named Kubernetes `User` or `Group` subject for local kubeconfig users when the operator supplies the subject explicitly.

Namespace-scoped mode creates a `Role` and `RoleBinding` in the database
namespace. If `--operator-namespace` differs, create a read-only operator `Role`
and `RoleBinding` there. These bindings grant only namespaced resources.
`ClusterImageCatalog`, CRD object reads, and validating/mutating webhook
configurations are cluster-scoped even when the operator is namespaced. Keep
those optional requirements in a separate scope-aware capability group.

For namespace-scoped setup, generate an additional narrowly scoped
`ClusterRole` and `ClusterRoleBinding` only with explicit
`--include-cluster-resources`. Without it, keep the namespaced bindings useful
and show those optional capabilities as unavailable. A namespaced RoleBinding
to a ClusterRole does not grant cluster-scoped resource access. All-namespaces
mode generates a `ClusterRole` and `ClusterRoleBinding` for the selected
profile, still excluding disabled features and Secret reads by default.

`--dry-run` output should be deterministic and match checked-in examples.
`--diff` should compare intended RBAC with the readable live objects and report
missing, changed, and extra managed rules without applying changes. `--apply`
should be idempotent and should not remove rules it did not create unless
`--prune` is explicitly provided.

Minimum runtime permissions:

- `get`, `list`, `watch` on CNPG `clusters`.
- `get`, `list`, `watch` on CNPG `poolers`.
- `get`, `list`, `watch` on CNPG `backups` and `scheduledbackups`.
- `get`, `list`, `watch` on CNPG `databases`, `databaseroles`, `publications`, and `subscriptions` when those features are enabled.
- `get`, `list`, `watch` on namespaced CNPG `failoverquorums` and `imagecatalogs` when those feature checks are enabled and the CRDs exist.
- `get`, `list`, `watch` on `pods`.
- `get` on `pods/log` for CNPG-aware log views.
- `get`, `list`, `watch` on `persistentvolumeclaims`.
- `get`, `list`, `watch` on `services`.
- `get`, `list`, `watch` on `endpointslices` in `discovery.k8s.io`.
- `get`, `list`, `watch` on `events`.
- transport/version-specific `pods/exec` permissions for Postgres and PgBouncer probes, validated below.
- optional `get`, `list`, `watch` on selected `secrets` for certificate expiration checks and generated credential validation.

Additional operator namespace permissions when operator support is enabled:

- `get`, `list`, `watch` on `deployments` in `apps`.
- `get`, `list`, `watch` on `replicasets` in `apps`.
- `get`, `list`, `watch` on `pods`.
- `get`, `list`, `watch` on `events`.
- `get`, `list`, `watch` on `leases` in `coordination.k8s.io` when leader detection is enabled.
- `get`, `list`, `watch` on `services` and `endpointslices` when webhook or metrics endpoint checks are enabled.
- optional `get` on `pods/log` for operator log timeline support.

Optional cluster-scoped permissions, separate from operator namespace access:

- `get`, `list`, `watch` on CNPG `clusterimagecatalogs` for image posture;
- `get`, `list`, `watch` on `validatingwebhookconfigurations` and
  `mutatingwebhookconfigurations` in `admissionregistration.k8s.io` for webhook
  posture;
- explicitly requested CRD metadata reads in `apiextensions.k8s.io` when needed;
  API discovery and normal resource list/watch must not require CRD object
  reads merely to observe CNPG clusters.

`DatabaseRoleList` is the Kubernetes list kind for `DatabaseRole`, not a CRD
endpoint. Do not create a watcher, capability requirement, or RBAC rule for
`databaserolelists`. Reading the ordinary resource list/watch includes its
status; do not add `/status` permissions unless an actual separate request
requires them.

Pin Kubernetes and kube versions before finalizing exec RBAC. Kube's WebSocket
exec handshake uses HTTP GET; authorization can require `get` on `pods/exec`,
with an additional `create` check on Kubernetes versions/configurations that
enable `AuthorizePodWebsocketUpgradeCreatePermission`. Generate only the
requirements established for the supported combinations, and use `get` plus
`create` when needed across the supported range. SSAR and doctor must test the
same permission set as the real transport, including both verbs when required.
Live tests cover get-only, create-only, both, and neither to expose mismatches
between generated RBAC, preflight, and actual exec.
Phase 2's pinned Kubernetes 1.35.5/kube 5.0.0 live matrix requires both verbs;
get-only, create-only, and neither are denied. See the
[qualification receipt](docs/validation/phase-2.md).

`doctor` should use Kubernetes self-subject access reviews when available, then fall back to live API checks. The UI should report missing permissions as reduced-mode capabilities, not as a full startup failure unless no useful Kubernetes access exists.

### PostgreSQL role setup

`cnpg-watch setup postgres-role` assumes the operator can provide admin PostgreSQL credentials or an admin secret reference. It creates a dedicated role for normal monitoring.

CNPG 1.30.0 rejects managed roles with the reserved `cnpg_` prefix. The live
Phase 1 proof therefore uses `watch_monitor`; keep generated role names outside
that prefix. Direct PostgreSQL role creation has a separate naming policy, but
must not make the default incompatible with CNPG-managed roles.

Target role properties:

- login role named `watch_monitor` by default;
- no superuser, no createdb, no createrole, no replication, no bypassrls;
- granted built-in monitoring privileges such as `pg_monitor` or the narrowest version-supported grants needed for statistics views;
- optional `pg_use_reserved_connections` grant only when the user chooses the
  reserved-connection profile;
- optional action role grants such as `pg_signal_backend` only when the user
  explicitly creates a separate action role;
- includes the grants required for profile-enabled connection pressure and wait diagnostics, while keeping query text disabled by default;
- optional generated password stored in a Kubernetes secret when requested;
- `statement_timeout`, `lock_timeout`, and `idle_in_transaction_session_timeout` set low for the role where supported;
- role comment explaining that the role is managed by `cnpg-watch`.

Example commands:

```sh
cnpg-watch setup postgres-role --namespace app-db --cluster payments --dry-run
cnpg-watch setup postgres-role --namespace app-db --cluster payments --print-sql
cnpg-watch setup postgres-role --namespace app-db --cluster payments --role-profile monitor --apply
cnpg-watch setup postgres-role --namespace app-db --cluster payments --role-profile monitor-reserved --apply
cnpg-watch setup postgres-role --namespace app-db --cluster payments --role-profile actions --dry-run
```

The setup command should be idempotent:

- create the role if missing;
- rotate or preserve the password according to an explicit flag;
- add missing grants;
- avoid removing privileges it did not create unless `--prune` is explicitly set;
- emit deterministic SQL for tests and review;
- validate the resulting role with the same capability checks used by `doctor`
  when validation is requested.

Normal watch mode should use the monitoring role, not admin credentials. If only admin credentials are provided at runtime, warn and continue only when `--allow-admin-db-credentials` is set.

### PgBouncer role setup

`cnpg-watch setup pooler-role` configures PgBouncer access for pooler statistics when possible.

The preferred runtime permission is stats visibility, not administrative control. The setup flow should support:

- creating or updating a PgBouncer stats user;
- storing generated credentials in a Kubernetes secret;
- validating that `SHOW POOLS` and `SHOW STATS` work through the selected pooler pod;
- falling back to documentation output when the Pooler configuration is not safely mutable by the tool.

The design should not assume every CNPG Pooler installation exposes the same auth path. If the tool cannot safely configure PgBouncer, it should still display pooler pod readiness and explain exactly what is missing for connection counts.

## In-memory model

The reducer maintains normalized state by scoped lookup key and verifies the
current resource incarnation before accepting asynchronous observations. The
following types show ownership and invariants; omitted supporting types and
derives are implementation details. Runtime timestamps are converted into
explicit UTC timestamps/ages for serialized output.

```rust
enum ObjectKey {
    Namespaced {
        kind: ResourceKind,
        namespace: NamespaceName,
        name: ResourceName,
    },
    ClusterScoped {
        kind: ResourceKind,
        name: ResourceName,
    },
}

struct ObjectIdentity {
    key: ObjectKey,
    uid: ResourceUid,
}

struct ClusterId(ObjectIdentity);
struct PodId(ObjectIdentity);
struct OperatorKey(ObjectKey);
struct ScopeEpoch(u64);
struct WatchEpoch(u64);
struct TopologyEpoch(u64);
struct RequestId(u64);
struct Lsn(u64);
struct TimelineId(u32);
struct SystemIdentifier(u64);
struct MonoTime(tokio::time::Instant);

struct Now {
    monotonic: MonoTime,
    wall: DateTime<Utc>,
}

struct ProbeTarget {
    cluster: ClusterId,
    pod: PodId,
    bundle: ProbeBundleId,
}

struct ProbeTicket {
    scope_epoch: ScopeEpoch,
    topology_epoch: TopologyEpoch,
    request_id: RequestId,
    target: ProbeTarget,
}

struct WatchEnvelope<T> {
    scope_epoch: ScopeEpoch,
    watch_epoch: WatchEpoch,
    event: WatchEvent<T>, // Init, InitApply, InitDone, Apply, Delete, health
}

struct Stamped<T> {
    value: T,
    received_at: MonoTime,
    source_time: Option<DateTime<Utc>>,
    source: ObservationSource, // includes watch identity or ProbeTicket
}

struct Observation<T> {
    last_good: Option<Stamped<T>>,
    latest_attempt: AttemptState,
    freshness_policy: FreshnessPolicy,
}

enum AttemptState {
    Idle,
    InFlight { request: RequestId, started_at: MonoTime },
    Succeeded { request: RequestId, finished_at: MonoTime },
    Failed {
        request: RequestId,
        finished_at: MonoTime,
        error: ProbeErrorSummary,
        next_eligible: Option<MonoTime>,
    },
}

enum Freshness {
    NeverObserved,
    Fresh { age: Duration },
    Stale { age: Duration, reason: StaleReason },
}

enum FreshnessPolicy {
    Sampled { stale_after: Duration },
    WatchBacked { watch: WatchId },
}

struct SourceStatus {
    source: DataSource,
    sync: SyncState,
    connection: ConnectionState,
    last_sync: Option<MonoTime>,
    last_progress: Option<MonoTime>,
}

enum Availability {
    DisabledByProfile,
    Unknown { reason: CapabilityReason },
    Available,
    Forbidden { missing: Vec<PermissionRequirement> },
    Unavailable { reason: CapabilityReason },
    NotApplicable,
}

struct CapabilityState {
    feature: FeatureId,
    availability: Availability,
    evidence: Vec<EvidenceRef>,
}

struct AppState {
    observed: ObservedState,
    navigation: NavigationState,
    derived: DerivedState,
    diagnostics: DiagnosticsState,
}

struct ObservedState {
    scope: Scope,
    scope_epoch: ScopeEpoch,
    operator_scope: Option<OperatorScope>,
    operators: BTreeMap<OperatorKey, OperatorState>,
    clusters: BTreeMap<ObjectKey, ClusterState>,
    events: VecDeque<KubeEvent>,
    logs: LogStore,
    profile: RuntimeProfile,
    capabilities: BTreeMap<FeatureId, CapabilityState>,
    sources: BTreeMap<WatchId, SourceStatus>,
}

struct NavigationState {
    selected_cluster: Option<ObjectKey>,
    active_view: ViewId,
    focus: FocusTarget,
    command_history: VecDeque<String>,
    filter_history: VecDeque<String>,
    active_filter: Option<FilterExpr>,
    logs: LogNavigation,
}

struct DerivedState {
    fleet: FleetState,
    health: BTreeMap<ObjectKey, HealthState>,
    hints: BTreeMap<ObjectKey, Vec<RunbookHint>>,
    dirty: DirtyDependencies,
}

struct ClusterState {
    id: ClusterId,
    topology_epoch: TopologyEpoch,
    generation: Option<i64>,
    phase: Option<String>,
    primary: Option<String>,
    target_primary: Option<String>,
    instances: BTreeMap<String, InstanceState>,
    poolers: BTreeMap<ObjectKey, PoolerState>,
    archive: Observation<ArchiveState>,
    connections: ConnectionPressureState,
    waits: WaitDiagnosticsState,
    backups: BackupState,
    storage: StorageState,
    services: ServiceState,
    managed_objects: ManagedObjectState,
    replica_topology: ReplicaTopologyState,
    plugin_status: Vec<CnpgPluginState>,
    certificates: CertificateState,
    version: VersionPosture,
    operator_association: OperatorAssociation,
    reconciliation: ReconciliationState,
    conditions: Vec<Condition>,
    last_kube_change: MonoTime,
}

struct InstanceState {
    name: String,
    pod: Option<PodId>,
    role: InstanceRole,
    pod_ready: bool,
    restart_count: i32,
    node_name: Option<String>,
    lsn: Observation<LsnState>,
    replication: Observation<ReplicationPeerState>,
}

struct LsnState {
    system_id: Option<SystemIdentifier>,
    timeline: Option<TimelineId>,
    role: InstanceRole,
    current_wal_lsn: Option<Lsn>,
    receive_lsn: Option<Lsn>,
    replay_lsn: Option<Lsn>,
    last_replayed_transaction_age: Option<Duration>,
    // Derived lag is evaluated separately from compatible stamped observations.
}

struct PoolerState {
    key: ObjectKey,
    pooler_type: Option<String>,
    phase: Option<String>,
    service: Option<ObjectKey>,
    desired: Option<i32>,
    ready: Option<i32>,
    unavailable: Option<i32>,
    connections: Observation<PoolerConnections>,
}

struct BackupState {
    last_success: Option<BackupSummary>,
    active: Vec<BackupSummary>,
    failed: Vec<BackupSummary>,
    schedules: BTreeMap<ObjectKey, ScheduledBackupSummary>,
    retention_policy: Option<String>,
    method: Option<BackupMethod>,
}

struct StorageState {
    volumes: BTreeMap<String, VolumeSummary>,
    warnings: Vec<StorageWarning>,
}

struct ServiceState {
    rw: Option<ServiceEndpointState>,
    ro: Option<ServiceEndpointState>,
    replicas: Vec<ServiceEndpointState>,
    poolers: Vec<ServiceEndpointState>,
}

struct ManagedObjectState {
    databases: BTreeMap<ObjectKey, ReconcileStatus>,
    roles: BTreeMap<ObjectKey, ReconcileStatus>,
    publications: BTreeMap<ObjectKey, ReconcileStatus>,
    subscriptions: BTreeMap<ObjectKey, ReconcileStatus>,
}

struct ReplicaTopologyState {
    upstream: Option<String>,
    downstreams: Vec<String>,
    delayed_replicas: Vec<DelayedReplicaState>,
    slots: Vec<ReplicationSlotState>,
    sync_state: Option<SyncReplicationState>,
}

struct CnpgPluginState {
    name: String,
    version: Option<String>,
    capabilities: Vec<String>,
    warnings: Vec<String>,
}

struct CertificateState {
    items: Vec<CertificateSummary>,
    reduced_mode: Option<String>,
}

struct VersionPosture {
    cnpg_crd_version: Option<String>,
    operator_version: Option<String>,
    postgres_image: Option<String>,
    image_catalog: Option<String>,
    warnings: Vec<String>,
}

struct OperatorScope {
    namespace: Option<String>,
    selector: LabelSelector,
    discovery: OperatorDiscoveryMode,
    enabled: bool,
}

struct OperatorState {
    namespace: Option<String>,
    deployment: Option<ObjectKey>,
    pods: BTreeMap<ObjectKey, OperatorPodState>,
    leader: Option<ObjectKey>,
    version: Option<String>,
    image: Option<String>,
    conditions: Vec<Condition>,
    webhooks: WebhookHealthState,
    metrics: OperatorMetricsState,
    watches: WatchStatus,
    logs: Option<LogSourceKey>, // canonical source state lives in LogStore
    reduced_mode: Option<String>,
}

struct FleetState {
    rows: Vec<FleetClusterRow>,
    health: HealthState,
    operator_rows: Vec<OperatorFleetRow>,
    last_evaluated: MonoTime,
}

struct OperatorAssociation {
    operator: Option<OperatorKey>,
    confidence: AssociationConfidence,
    evidence: Vec<EvidenceRef>,
}

struct ReconciliationState {
    observed_generation: Option<i64>,
    generation_lag_age: Option<Duration>,
    pending_resources: Vec<ObjectKey>,
    stale_conditions: Vec<Condition>,
    related_operator_errors: Vec<LogEntryId>, // bounded references into LogStore
}

struct LogNavigation {
    active_scope: Option<LogScope>,
    follow: bool,
    paused: bool,
    filter: Option<FilterExpr>,
    since: LogSince,
    reduced_mode: Option<String>,
}

struct LogStore {
    // Source keys include scope epoch, pod UID, container, and stream identity.
    sources: BTreeMap<LogSourceKey, LogSourceState>,
    entries: VecDeque<LogEntry>,
    retained_bytes: usize,
}

enum LogScope {
    Cluster(ObjectKey),
    Instance { cluster: ObjectKey, pod: ObjectKey },
    Pooler { cluster: ObjectKey, pooler: ObjectKey },
    Backup { cluster: ObjectKey, backup: ObjectKey },
    Event { cluster: ObjectKey, involved: ObjectKey },
}

struct LogEntry {
    id: LogEntryId,
    ts: Option<DateTime<Utc>>,
    source: LogSourceKey,
    severity: Option<String>,
    message: String,
    parsed_fields: BTreeMap<String, String>,
    redacted: bool,
}

struct HealthState {
    overall: HealthSeverity,
    reasons: Vec<HealthReason>,
    last_evaluated: MonoTime,
}

struct HealthReason {
    id: String,
    severity: HealthSeverity,
    message: String,
    evidence: Vec<EvidenceRef>,
    target_view: Option<ViewId>,
    stale: bool,
}

struct RunbookHint {
    id: String,
    severity: HealthSeverity,
    title: String,
    detail: String,
    evidence: Vec<EvidenceRef>,
    target_view: Option<ViewId>,
    dismissed_until_change: bool,
}

struct ConnectionPressureState {
    by_instance: BTreeMap<PodId, Observation<InstanceConnectionPressure>>,
    // Pooler connection samples live in PoolerState; views join by key.
}

struct WaitDiagnosticsState {
    by_instance: BTreeMap<PodId, Observation<InstanceWaitSummary>>,
    // Each instance summary includes its bounded blockers/query summaries.
}

struct RuntimeProfile {
    name: ProfileName,
    sql_probes: ProbeProfile,
    logs: LogProfile,
    redaction: RedactionProfile,
    features: BTreeSet<FeatureFlag>,
}
```

Deserialize PostgreSQL's hexadecimal `X/Y` LSN representation at the boundary
into `Lsn(u64)` with `FromStr`/`TryFrom`, and implement canonical display.
Unit/property tests cover round trips, malformed halves, overflow, checked
deltas, and timeline incompatibility. A lag result is either a validated byte
delta with evidence or an explicit unknown/not-applicable reason.

`WatchStatus` should distinguish `starting`, `syncing`, `synced`, `restarting`, `forbidden`, and `failed`. This avoids the k9s pain point of showing no data before a cache is actually warm.

`DiagnosticsState` should back `cnpg-watch info`, screen dumps, and e2e artifact capture. It should store log path, config path, dump path, current kube context, CNPG CRD versions seen, active permissions, cache ages, probe ages, and reduced-mode reasons.

## Collector behavior

Kubernetes collectors:

- start one watcher per resource type and namespace scope;
- start operator namespace watchers separately from database namespace watchers when operator support is enabled;
- start one operator watcher group for the configured or discovered operator
  scope;
- report an ambiguity state if discovery finds more than one plausible CNPG
  operator install in the same Kubernetes API server;
- adapt `Apply`, `Delete`, `Init`, `InitApply`, and `InitDone` into typed
  identity-bearing events, with stream errors tracked separately;
- stage initial/relist membership and commit atomically on completion; preserve
  the previous complete set on interruption and remove absent objects only
  after a complete relist;
- reconnect with backoff on watch errors;
- record watch health separately from object health;
- skip unchanged field updates without suppressing UID changes, deletes,
  relist membership, or source-health progress;
- cap Kubernetes client QPS/burst with conservative defaults and expose the effective values in `doctor`;
- handle namespace-scoped and cluster-scoped RBAC checks separately and test both;
- handle database namespace and operator namespace RBAC separately and test same-namespace and different-namespace installations;
- evaluate operator-to-cluster association from explicit config, watch namespace evidence, events, logs, and reconciliation freshness.

Log collector:

- opens pod log streams only for the active `LogScope`;
- uses bounded historical reads before follow mode, with defaults such as last `15m` or last `500` lines per source;
- merges entries from primary, replicas, poolers, backup jobs, operator pods, and events only when the active scope asks for those sources;
- parses JSON log lines into structured fields when possible and preserves raw text otherwise;
- classifies PostgreSQL/CNPG/PgBouncer severities without dropping unknown lines;
- applies redaction before entries reach the UI, dumps, snapshots, or failure artifacts;
- enforces line/batch framing caps before parsing, global and per-source byte
  limits, maximum follow-stream count, and eviction of inactive sources;
- drops log batches/oldest entries under pressure, records counts, and makes
  truncation visible without blocking input or topology behind logs;
- closes streams on scope changes, namespace/context changes, source toggle changes, and application shutdown;
- treats operator log streams as a separate source keyed by operator namespace and pod name.

Health and hint evaluator:

- runs after relevant accepted observation changes and time deadlines, using
  observed state and supplied time; updates only affected dependencies;
- emits deterministic health reasons with stable IDs and source evidence;
- emits fleet-level health reasons when more than one cluster is visible;
- ranks service connectivity, primary availability, data safety, and overload above optional-feature gaps;
- emits runbook hints from the same evidence without mutating Kubernetes or PostgreSQL;
- suppresses duplicate hints until the underlying evidence changes.

SQL probe scheduler:

- derives target pods from the current cluster state;
- probes the primary for current WAL, replication peers, and archiver status;
- probes replicas for receive/replay LSN;
- probes connection pressure and wait diagnostics only when the selected profile enables them;
- limits concurrent exec sessions with a semaphore;
- enforces per-pod probe concurrency of `1`;
- applies jitter so all pods are not sampled at once;
- cancels in-flight probes when a pod is deleted or the selected cluster changes;
- rejects completions with obsolete scope/topology epochs, UIDs, or request IDs
  even if they were queued before cancellation;
- uses adaptive backoff after timeouts, refused connections, too many connections, and admin shutdown errors;
- batches expensive probe result hydration and view-model updates;
- assigns lower priority to connection pressure and wait diagnostics than topology, LSN, archiver, and pooler probes;
- records `last_success`, `last_error`, and `in_flight` per probe target.

Scheduling defaults come from the scheduler contract and `BudgetConfig`:

- Kubernetes watches: streaming, no polling interval.
- Selected fast instance bundle: minimum 2 s start spacing, 750 ms end-to-end
  deadline, 10 s freshness window. A full cluster sweep may take longer.
- Transient failure backoff: exponential from 2 s to the single 30 s ceiling,
  with bounded jitter; authentication/permission failures suspend capability.
- Forced refresh: coalesce one pending intent; preserve backoff, request-rate,
  and concurrency limits. Skip missed intervals rather than catching up.
- Event retention: 200 per cluster, 2,000 total, with a 1 h age cap.

## TUI interaction model

The TUI should be operationally structured. It should not try to show every
possible CNPG/PostgreSQL signal on one screen.

### Primary modes

Use three top-level modes:

- `Fleet`: many clusters, cross-namespace health, operator association, and
  drill-down into a selected cluster.
- `Cluster`: one selected CNPG cluster, with topology, health, instances,
  compact badges, poolers, and events.
- `Detail`: focused views such as logs, backups, poolers, waits, services,
  storage, doctor, operator, certs, roles, databases, subscriptions, plugins,
  and profile.

Mode selection:

- If more than one CNPG cluster is visible, default to `Fleet`.
- If exactly one cluster is visible, default to `Cluster`.
- If `--cluster` is provided, default to that cluster's `Cluster` view.
- Commands such as `:logs`, `:doctor`, `:backups`, `:services`, and `:waits`
  enter `Detail`.

### Shared header

Every screen should use the same top bar so operators always know the active
context:

```text
ctx prod-us-east | ns app-db | cluster payments | profile standard | sql stale 12s | operator cnpg-system weak
```

Header fields:

- kube context;
- namespace scope;
- selected cluster or `fleet`;
- active profile;
- Kubernetes watch state;
- SQL sample state;
- operator namespace and association confidence when known;
- reduced-mode count when any feature is degraded.

### Bottom command/status bar

Every screen should use a bottom bar for current mode, active filter, selected
row actions, reduced-mode reason, and command input.

Examples:

```text
:fleet  12 clusters  filter=/pay  Enter=open  l=logs  ?=help
```

```text
:logs  follow:on  since:15m  sources:primary,replicas  /=filter  s=sources  Esc=back
```

When command mode is active, the bottom bar becomes the command prompt. When a
collector or probe has a recent error, the bottom bar can show one concise
redacted message and point to `:doctor` or `:health` for details.

### Breadcrumb

Detail views should show a breadcrumb beneath the header or at the left of the
status bar:

```text
fleet > app-db/payments > logs
```

This is especially important when logs, backups, services, and operator views
can be opened from several source contexts.

### Health as navigation

Health is the navigation spine. A health reason is not only text; it is a row
with evidence and a target.

Example:

```text
critical  rw service endpoint points at old primary       Enter -> :services
warning   payments-2 replay lag 1.4 GiB                   Enter -> :replicas
info      pods/log forbidden                              Enter -> :doctor
```

Pressing `Enter` on a health row should jump to the target view and select the
object or evidence that caused the reason. If a health reason has no target,
that is a design gap.

### Cluster dashboard layout

For one selected cluster, prioritize health and instances over secondary
panels:

```text
┌ ctx prod-us-east | ns app-db | cluster payments | profile standard | sql fresh 2s ┐
│ health: critical rw endpoint mismatch | warning replica lag | info pods/log off │
├ instances ──────────────────────────────────────────────────────────────────────┤
│ name          role     ready timeline recv_lsn    replay_lsn  lag      state   │
│ payments-1    primary  yes   7        0/7A001230  -           -        fresh   │
│ payments-2    replica  yes   7        0/79FFF000  0/79000000  16 MiB   stale   │
├ badges ─────────────────────────────────────────────────────────────────────────┤
│ backup ok 2h | storage warning pvc resize | services critical rw stale | pooler ok│
├ poolers ┬ events ───────────────────────────────────────────────────────────────┤
│ rw 2/2  │ 12:04 Warning EndpointMismatch rw service points at payments-2       │
└ :cluster  Enter=detail  l=logs  h=health  r=refresh  Tab=focus  ?=help ────────┘
```

Avoid over-pane layouts. Use compact badges for secondary signals and let
operators drill into `:storage`, `:services`, `:backups`, `:poolers`, or
`:replicas`.

### Fleet view mockup

```text
┌ ctx prod-us-east | ns all | fleet | profile standard | watches synced ┐
│ fleet health: critical 2 endpoint mismatches | warning 3 stale SQL    │
├ clusters ──────────────────────────────────────────────────────────────┤
│ ns       cluster    phase   primary    inst ready lag     backup pool operator │
│ app-db   payments   Ready   payments-1 3    3/3   16 MiB  ok 2h  ok   explicit │
│ app-db   ledger     Ready   ledger-2   3    2/3   stale   fail   warn weak     │
│ ops-db   audit      Init    -          2    0/2   unknown -      -    unknown  │
└ :fleet  Enter=open cluster  /=filter  o=operator  d=doctor  ?=help ───────────┘
```

### Health view mockup

```text
┌ ctx prod-us-east | ns app-db | cluster payments | :health ┐
│ sev       reason                                      target      age │
│ critical  rw service endpoint points at old primary   :services   8s │
│ warning   payments-2 replay lag 1.4 GiB               :replicas  12s │
│ warning   latest scheduled backup failed 3h ago       :backups    1m │
│ info      pods/log forbidden                          :doctor    10m │
└ Enter=open target  /=filter  Esc=cluster  ?=help ────────────────────┘
```

### Logs view mockup

```text
┌ ctx prod-us-east | ns app-db | cluster payments | :logs | follow:on since:15m ┐
│ sources: [primary*] [replicas] [poolers] [operator] [events]  filter: ERROR   │
├ timeline ─────────────────────────────────────────────────────────────────────┤
│ 12:04:11 primary/payments-1 ERROR archive command failed: redacted            │
│ 12:04:13 operator/cnpg-system WARN reconcile Cluster app-db/payments failed   │
│ 12:04:17 event/services Warning EndpointMismatch rw points at payments-2      │
└ f=follow  p=pause  s=sources  t=time  /=filter  Esc=back ────────────────────┘
```

The log viewer should keep source labels visible on every line. Cluster-scoped
operator logs should default to correlated lines only, with a source toggle for
full operator logs.

### Doctor view mockup

```text
┌ ctx prod-us-east | ns app-db | :doctor | profile incident ┐
│ feature       state      detail                                  setup        │
│ topology      ok         clusters/pods/events readable            -           │
│ sql probes    reduced    pods/exec forbidden                      setup rbac  │
│ logs          reduced    pods/log forbidden                       setup rbac  │
│ operator      ok         cnpg-system readable, logs unavailable   setup rbac  │
│ certs         disabled   secure profile disables Secret reads     -           │
└ Enter=details  y=copy setup command  /=filter  Esc=back ─────────────────────┘
```

### Reduced-mode rendering

Reduced mode should be persistent, compact, and specific:

```text
reduced: pods/exec forbidden; SQL probes unavailable
```

Rules:

- show a top-bar reduced count when any active feature is degraded;
- show a banner in the affected view;
- keep unaffected views usable;
- include the exact missing verb/resource when known;
- link or jump to `:doctor`;
- do not treat every optional permission gap as cluster failure.

### Stale data language

Use consistent cell states everywhere:

- `fresh`
- `stale 18s`
- `backoff 24s`
- `forbidden`
- `disabled`
- `syncing`
- `unknown`
- `not-applicable`

Avoid blank cells. A blank cell means a rendering bug, not missing data.

### Row actions

The view registry should define row actions for each view. The help overlay and
bottom bar should be generated from the active view's actions.

Initial actions:

| View | Row action |
| --- | --- |
| `:fleet` | `Enter` opens the selected cluster |
| `:instances` | `Enter` opens instance detail; `l` opens instance logs |
| `:health` | `Enter` jumps to the health reason target |
| `:poolers` | `Enter` opens pooler detail; `l` opens pooler logs |
| `:backups` | `Enter` opens backup detail/logs |
| `:services` | `Enter` opens endpoints for the selected service |
| `:operator` | `Enter` opens operator detail/logs |
| `:logs` | `Enter` expands the selected log entry |
| `:doctor` | `Enter` opens check detail; copy setup command when available |

### Layout breakpoints

Define explicit breakpoints:

- `>=120x40`: full cluster dashboard with health strip, instances, badges,
  poolers, and events.
- `100x30` to `119x39`: compressed dashboard with health strip, instances,
  badges, and one secondary panel selected by focus.
- `<100x30`: table-first fallback; show header, reduced-mode banner, primary
  table, and status bar. Secondary panels become detail views.

Do not resize columns based on every refresh. Use fixed or bounded widths and
truncate long pod names, event messages, and errors. Long text must not overlap
adjacent panels.

### Color and style rules

Use color sparingly and never as the only state indicator:

- red: current critical failure;
- yellow: degraded, stale, pending, or risk;
- blue/cyan: selected row, focused pane, or command prompt;
- green: healthy/current;
- dim: unavailable, disabled, not applicable, or old evidence;
- default: neutral operational text.

All colored states must also have text labels such as `critical`, `stale`,
`disabled`, or `forbidden`.

### Contextual help

`?` should open contextual help generated from global actions and the active
view registry.

Example:

```text
Global: q quit | ? help | : command | / filter | Ctrl-S dump
Fleet:  Enter open cluster | o operator | d doctor
Cluster: Tab focus | l logs | r refresh | h health
Logs:   f follow | p pause | s sources | t time window
```

The help overlay should not list actions that are disabled by profile or
missing permissions unless it marks them as unavailable and points to `:doctor`.

## Error handling

Errors should be visible but non-fatal unless the terminal cannot initialize or Kubernetes config cannot be loaded.

Important states:

- kube config load failure: exit with a clear message;
- API authorization failure: show the forbidden resource and continue with other resources if possible;
- missing CNPG API resources: show `CNPG API unavailable` with the active
  context/scope; distinguish this from a synchronized empty cluster list;
- exec not allowed: mark SQL samples unavailable and show the RBAC verb/resource needed;
- pod missing `psql`: mark SQL samples unavailable for that pod;
- SQL timeout: keep the previous successful sample and mark it stale;
- database overloaded or rejecting connections: preserve last sample, increase probe backoff, and display the failure class;
- pooler stats unavailable: continue showing pooler pod readiness and direct instance state;
- watch restart: preserve state, mark watch reconnecting, and let relist correct the model.

## RBAC

The Kubernetes RBAC setup section is the authoritative permission contract.
Watchers, doctor, generated manifests, and tests use the same scope-aware
capability requirements; do not maintain a second handwritten rule list.
Runtime manifests grant only profile-enabled capabilities. Bootstrap identities
need permission to create/update the roles and bindings they explicitly apply.

Generate these examples as the corresponding profiles become implemented:

```text
deploy/rbac/namespace.yaml
deploy/rbac/namespace-with-operator.yaml
deploy/rbac/namespace-with-cluster-resources.yaml
deploy/rbac/cluster.yaml
```

Namespace/operator examples contain only namespaced permissions. The optional
cluster-resources example includes separate ClusterRole/ClusterRoleBinding
objects and makes that scope visible in dry-run/diff output. Examples must match
`setup kubernetes-rbac --dry-run` byte-for-byte after deterministic formatting.
Use live least-privilege identities to validate list/watch, log, and actual
WebSocket exec behavior; SSAR fixtures alone cannot prove transport access.

Missing optional access degrades only the dependent features. Without exec,
SQL and exec-based PgBouncer stats are unavailable; without logs, only log views
are unavailable; without operator access, database observation continues. Missing
cluster-scoped webhook/catalog reads must not block namespaced observation.

## Crate choices

Initial dependencies:

- `tokio` for async runtime.
- `kube` and `kube-runtime` for Kubernetes clients and watchers.
- `k8s-openapi` for core Kubernetes types.
- `ratatui` for terminal rendering.
- `crossterm` for terminal input and raw mode.
- `clap` for CLI flags.
- `serde`, `serde_json`, and `schemars` for typed data and dynamic CRDs.
- `tracing` and `tracing-subscriber` for logs.
- `anyhow` for top-level errors and `thiserror` for library errors.
- `humantime` or `humantime-serde` for duration flags.
- `secrecy` for handling PostgreSQL and PgBouncer setup credentials in memory.

Initial dev dependencies:

- `insta` for reviewed text and serialized snapshots.
- `pretty_assertions` for readable buffer/model diffs.
- `assert_cmd` and `predicates` for non-interactive CLI behavior.
- `rstest` for table-driven parser, reducer, and layout cases.
- `proptest` for generated event sequences, parser properties, and shrinking
  state-machine failures into reproducible regression cases.
- `criterion` for adapter, reducer, health, filtering, view-model, and render
  benchmarks as the corresponding slice is implemented.
- `portable-pty` for spawning the real binary inside a pseudo-terminal during TUI smoke/e2e tests.
- `vt100` for parsing real terminal output into an inspectable screen model.

Start the transport proof with exec-based JSON bundles because they match CNPG
pod locality. Add a native PostgreSQL client if the proof selects the
port-forward option to meet authentication, error, or cleanup requirements.
Do not exclude it solely to preserve the initial dependency list. Add each
dependency when a working slice uses it; avoid linking future features or
test-only tooling into the release binary.

## Module outline

```text
src/
  lib.rs                  domain modules exposed to tests; no terminal startup
  main.rs                 CLI dispatch and process entry point
  cli.rs                  clap definitions for watch, setup, doctor
  config.rs               config file/env/default resolution
  terminal.rs             terminal setup, panic restore, platform checks
  app.rs                  single state owner, bounded app loop, typed effects
  model.rs                identities, observations, availability, accepted state
  reducer.rs              pure state transitions and event acceptance
  time.rs                 injected monotonic/wall time and deadline decisions
  commands.rs             command/filter parser and history support
  capabilities.rs         feature capability matrix and reduced-mode messages
  profiles.rs             built-in runtime profiles and feature gates
  registry.rs             CNPG view registry and action metadata
  kube_client.rs          kube config and shared client construction
  collectors/
    mod.rs
    operator.rs           CNPG operator deployment/pod/event watch adapter
    webhooks.rs           CNPG admission webhook and endpoint health adapter
    metrics.rs            optional operator metrics collector
    clusters.rs           CNPG Cluster watch adapter
    poolers.rs            CNPG Pooler watch adapter
    backups.rs            CNPG Backup and ScheduledBackup watch adapters
    managed.rs            Database, DatabaseRole, Publication, Subscription adapters
    posture.rs            FailoverQuorum, ImageCatalog, ClusterImageCatalog adapters
    pods.rs               Pod watch adapter and CNPG label extraction
    storage.rs            PVC watch adapter for data/WAL/tablespace volumes
    services.rs           Service and EndpointSlice watch adapters
    events.rs             Event watch adapter and filtering
    logs.rs               CNPG-aware pod log streaming, parsing, and redaction
    sql.rs                source task calling probes; owns no scheduling policy
    pgbouncer.rs          source task calling pooler probes
  cnpg/
    mod.rs
    dynamic.rs            version-tolerant DynamicObject adapters
    labels.rs             CNPG label constants and object matching
    observations.rs       internal typed CNPG observation models
    status.rs             Cluster, backup, pooler, plugin, and managed-object status normalization
    association.rs        operator-to-cluster association evidence and confidence
    reconciliation.rs     observedGeneration and reconcile-staleness helpers
  fleet/
    mod.rs
    rows.rs               fleet row aggregation and grouping
    health.rs             fleet-level health reason aggregation
  permissions/
    mod.rs
    kubernetes.rs         RBAC manifests, apply/dry-run, access checks
    postgres.rs           monitoring role creation and validation
    pgbouncer.rs          PgBouncer stats user setup and validation
  setup/
    mod.rs
    diff.rs               deterministic setup diff rendering
    output.rs             yaml/json/sql/text setup output formatting
  probes/
    mod.rs
    scheduler.rs          probe classes, priority, jitter, backoff, freshness
    transport.rs          SqlExecutor trait and target model
    exec.rs               Kubernetes exec transport and timeouts
    registry.rs           version-gated PostgreSQL query registry
    postgres.rs           SQL text and JSON result structs
    diagnostics.rs        connection pressure and wait diagnostics probes
    pgbouncer.rs          SHOW POOLS/STATS parsing
  health/
    mod.rs                pure health and runbook hint rules
    freshness.rs          deadline-based evidence freshness and compatibility
  postgres/
    lsn.rs                LSN parser and lag math
    errors.rs             classify timeout/load/connection failures
    redact.rs             query/user/database/client redaction helpers
  ui/
    mod.rs
    actions.rs             active key bindings and help metadata
    layout.rs
    view_model.rs          map AppState into render-only rows/panels
    widgets.rs
    theme.rs
    testing.rs             TestBackend helpers and stable buffer serialization
  support/
    mod.rs
    dumps.rs               screen/model dump writer with redaction
    info.rs                config/log/cache path and runtime info output
    redact.rs              shared redaction for logs, dumps, and errors
  doctor.rs               non-mutating environment and permission checks
  error.rs
tests/
  support/
    replay.rs             deterministic event/time replay and failure artifacts
    fixtures.rs           test-only fixture loaders
    tui_harness.rs        portable-pty/vt100 real terminal smoke
  contracts.rs            CLI, schema, and least-privilege live contracts
benches/
  pipeline.rs             adapters, reducer, health, view model, and render
fixtures/                 captured external inputs and deterministic replays
```

Keep Kubernetes JSON path handling out of UI and reducer code. Keep terminal drawing code free of API calls. Keep setup/mutation code out of collectors so normal watch mode remains read-only. Keep most UI behavior in pure functions from `AppState` to view models and from view models to `ratatui::buffer::Buffer`; the terminal event loop should be a thin shell.

## Implementation controls

These controls make the design executable as a sequence of small, reviewable
changes instead of a large open-ended build.

### Compatibility targets

Initial compatibility target:

- PostgreSQL 18 is the primary SQL probe and fixture target.
- CloudNativePG 1.30.x is the primary CNPG CRD, operator, and e2e target.
- CloudNativePG versions beyond 1.30.x should be supported by capability
  detection, dynamic adapters, and version-gated fixtures when fields remain
  compatible.
- Kubernetes target should be the version bundled by the pinned k3d/k3s e2e
  version, plus the oldest Kubernetes version that CNPG 1.30.x supports in its
  own compatibility policy.
- macOS Apple Silicon, macOS Intel, Linux x86_64, and Linux aarch64 are the
  first binary targets.

The first implementation should not claim PostgreSQL 14-17 support until query
registry fixtures and live probe tests exist for those versions. It can keep
the query registry version-aware from day one so older PostgreSQL versions can
be added later without changing the architecture.

Normal CNPG topology:

- one Kubernetes API server has one cluster-wide CNPG CRD set;
- that API server normally has one CNPG operator installation;
- that operator manages many CNPG `Cluster` resources across namespaces;
- `:fleet` is a many-database-cluster view, not a many-operator view;
- `vcluster` should be treated as a separate Kubernetes API boundary and
  normally a separate kube context.

### Architecture decision records

Create `docs/adr/` early. Each ADR should be short: context, decision,
consequences, and status.

Initial ADRs:

- `0001-pod-exec-before-direct-tcp.md`: pod-local exec is the first SQL
  candidate because it matches CNPG pod locality. Start as proposed; accept it
  only after the early monitoring-role authentication/timeout proof records a
  usable credential path and cleanup behavior. Record why loopback exec,
  configured peer mapping, or port-forward/native SQL was chosen.
- `0002-k3d-default-e2e.md`: k3d is the default local and CI e2e substrate
  because it runs k3s in Docker and works on GitHub-hosted Ubuntu runners and
  local macOS/Linux developer machines.
- `0003-observe-only-tui.md`: the TUI is observe-only in the first release;
  setup and future action subcommands are separate from watch mode.
- `0004-version-tolerant-cnpg-adapters.md`: use dynamic CNPG adapters at the
  Kubernetes boundary, convert immediately into internal typed observations,
  and keep generated/typed CRDs as a possible future adapter-layer
  implementation detail only.
- `0005-capability-matrix-owns-reduced-mode.md`: `doctor`, setup, profiles,
  health reasons, and UI badges all read from one capability matrix.
- `0006-postgresql-18-cnpg-130-targets.md`: PostgreSQL 18 and CNPG 1.30.x are
  the first supported implementation targets.
- `0007-machine-readable-output-contracts.md`: JSON/YAML command outputs are
  versioned contracts; text output can change for readability.
- `0008-performance-budgets-and-load-policy.md`: probes, logs, watches, and
  render loops are bounded so the tool remains useful during overload.
- `0009-observation-identity-and-relist.md`: UID/epoch/request acceptance and
  atomic initialization preserve correctness across replacement and reconnect.
- `0010-single-owner-and-deadlines.md`: one app loop owns state, navigation,
  and derived views; time deadlines and bounded traffic classes preserve
  responsiveness. Split view publication only if profiling justifies it.
- `0011-validation-and-performance-baseline.md`: replay/property tests,
  least-privilege live contracts, and reproducible benchmark/soak results gate
  feature expansion.

### Step definition of done

Every numbered implementation step should be considered done only when the
step has:

- code or documentation committed in the expected module or doc path;
- unit tests, snapshot tests, integration tests, or fixture coverage appropriate
  to the risk of the step;
- fixtures added or updated when the step parses external CNPG, Kubernetes,
  PostgreSQL, PgBouncer, log, RBAC, or terminal data;
- command output contracts updated when the step changes `doctor`, `setup`,
  `info`, or CLI help;
- reduced-mode behavior defined when the step depends on optional permissions;
- redaction reviewed when the step touches logs, SQL, users, databases, client
  addresses, credentials, Secrets, or dumps;
- lifecycle invariants covered when the step adds asynchronous work, with
  identity, cancellation, late-result, deadline, and cleanup cases as relevant;
- measured benchmark or bounded-load evidence when the step changes a hot path,
  queue, cache, framing limit, or scheduling policy. Reuse established fixtures
  and baselines instead of adding tests that mirror implementation details.

### Milestone labels

Each implementation step should be tagged with one or more labels when it is
converted into an issue:

- `contract`: data contracts, capability matrix, registry, ADRs, fixtures.
- `cli`: command-line flags, output, help, exit behavior.
- `model`: normalized state, reducer, evidence, health and hints.
- `collector`: Kubernetes watches, logs, operator metrics.
- `probe`: PostgreSQL and PgBouncer probes.
- `ui`: TUI rendering, command mode, layout, snapshots.
- `setup`: RBAC, PostgreSQL role, PgBouncer setup.
- `test`: unit, integration, snapshot, or harness work.
- `e2e`: k3d, live CNPG fixtures, reduced-RBAC, PTY smoke.
- `docs`: user docs, ADRs, implementation notes.

### Minimal issue backlog

First implementation issue sequence:

1. Scaffold the library/binary, CLI, logging, and terminal lifecycle.
2. Add core identities, independent sample/availability/attempt types, injected
   time, and the minimal topology/fast-SQL capability subset.
3. Add Cluster/Pod adapters and replay fixtures; implement acceptance, relists,
   deletes, time deadlines, bounded channels, and task cleanup.
4. Add the instance view model, table, and deterministic buffer tests.
5. Add the early k3d monitoring-role authentication/timeout proof and transport
   ADR; choose a working credential path before broad scheduler work.
6. Connect one role-aware SQL bundle to the live instance table.
7. Establish replay/property tests, paused-time scheduling tests, and initial
   performance/flood/soak measurements for the slice.
8. Add doctor and setup dry-run/diff contracts from the same capability subset,
   then explicit apply after live validation.
9. Expand namespace/operator permissions, profiles, command/filter parsing, and
   the view registry only as the corresponding MVP behavior is implemented.
10. Add events, services/PVCs/EndpointSlices, basic fleet health, and bounded
    selected-resource logs with aggregate memory limits.
11. Expand live fixtures to multiple namespaces/clusters and reduced RBAC.
12. Extend the proven query registry/scheduler to medium/deep diagnostics,
    poolers, and later operational views in their delivery phases.

### Risk gates

Do not move past the relevant phase until these risks are proven:

- terminal state is restored after normal exit, Ctrl-C, and panic;
- stale scope/watch/topology epochs, pod UIDs, and request IDs cannot overwrite
  accepted evidence, including results queued before cancellation;
- relist completion removes absent objects atomically, and interrupted relists
  preserve the previous complete set;
- sample/evidence expiry and age-based eviction work without incoming data,
  while a quiet synchronized watch keeps unchanged objects current;
- the monitoring transport proves actual role authentication, remote status,
  bounded output, timeout termination, and cleanup before scheduler expansion;
- replay/property tests, paused-time scheduling, and a release-build performance
  baseline cover the first vertical slice before broad registries/views;
- capability matrix drives `doctor`, setup generation, profile state, and UI
  reduced-mode badges;
- query registry gates PostgreSQL 18 probes by server version, required views,
  required columns, and role capability;
- SQL probe timeout preserves the last successful sample and reports stale
  state instead of blank data;
- reduced RBAC cannot crash the app and degrades only the affected feature;
- no TUI view renders directly from raw Kubernetes JSON;
- no production probe SQL lacks a timeout, version gate, and explicit output
  schema;
- screen snapshots prove no overlapping text at 120x40 and 100x30;
- k3d e2e proves operator namespace separate from database namespace;
- support dumps prove redaction for Secrets, credentials, query text, users,
  databases, client addresses, and connection strings.

### Non-regression rules

These rules should guide reviews and future automated checks:

- No `kubectl` shell-out in normal watch mode.
- No Kubernetes, PostgreSQL, PgBouncer, or Secret mutation from the TUI.
- No raw query text in dumps by default.
- No blocking SQL probe without `connect_timeout`, `statement_timeout`, and
  scheduler cancellation.
- No UI renderer reads raw Kubernetes JSON paths.
- No feature-specific permission checks outside the capability matrix except
  low-level adapters feeding the matrix.
- No unbounded pod log follow streams.
- No unbounded framing, output reads, queues, source metadata, or pending probe
  work; enforce global limits as well as per-source limits.
- No detached collector tasks or reliance on cancellation alone to reject
  obsolete results.
- No replay-lag alarm based solely on last-replayed-transaction age.
- No full-store clone or full-fleet recomputation on every input/log event.
- No feature silently disappears because of missing RBAC; it must report a
  reduced-mode reason.
- No setup command applies changes without an explicit `--apply`.
- No new external data parser without fixtures for success, missing field,
  malformed input, and permission failure.

## Delivery phases

The implementation should be phase-based. Each phase has a concrete deliverable and a test gate. Later phases can broaden behavior, but they should not weaken earlier gates.

Release scope gates:

- MVP: topology, basic fleet/instance/health/doctor views, bounded selected
  logs, fast SQL probes, reduced-mode capability reporting, and setup dry-runs.
- Alpha: pooler stats, backups, storage/services drill-down, stronger operator
  view, and local k3d e2e coverage.
- Beta: merged CNPG-aware logs, managed objects, deeper PostgreSQL diagnostics,
  real PTY smoke suite, and reduced-RBAC e2e cases.
- GA: stable capability matrix, profile-specific setup manifests, screen/model
  dumps, macOS/Linux packages, CI k3d e2e, and documented support boundaries.

How to work through the phases:

- Complete the numbered implementation steps in order inside a phase.
- Keep each phase shippable on its own branch; do not start broad feature work
  from later phases until the current phase exit criteria pass.
- When a step introduces a public command or data contract, add fixtures or
  tests in the same step.
- Keep the TUI thin. Most steps should produce testable state, rows, manifests,
  or JSON before terminal rendering consumes them.
- Treat e2e work as a gate for the phase that first needs it, not as cleanup at
  the end.
- Phase 1 proves a narrow live vertical slice before Phase 2 expands setup and
  Phase 3 expands topology. Phase 4 extends the proven SQL transport and query
  registry; it does not defer authentication or timeout proof until then.

### Phase 0: project scaffold

Implementation steps:

1. Initialize the Rust binary crate with `Cargo.toml`, `src/main.rs`, and a
   minimal `README` or command help that identifies the project.
2. Create `docs/adr/` and add the initial ADRs for pod exec, k3d, observe-only
   TUI, version-tolerant CNPG adapters, capability matrix ownership, and
   PostgreSQL 18/CNPG 1.30.x targets.
3. Add dependency groups for runtime, terminal, serialization, tracing, and test
   tooling without wiring every future module yet.
4. Implement `clap` CLI dispatch for `watch`, `doctor`, and `setup`, with
   placeholder handlers that return deterministic output.
5. Add shared flag parsing for kube context, namespace scope, all-namespaces,
   operator namespace, operator selector, profile, refresh interval, and verbose
   logging.
6. Add tracing/log initialization and a `cnpg-watch info` placeholder that
   prints intended config, log, cache, and dump paths.
7. Add terminal initialization, raw-mode/alternate-screen handling, panic
   restore, `q`, and `Ctrl-C`.
8. Render a static empty `ratatui` frame with top bar, empty body, and status
   line.
9. Add schema-version constants for `doctor`, `info`, RBAC diff, and
   screen/model dumps, even if the first handlers are placeholders.
10. Add CI-local commands or `make` targets for format, clippy, tests, and build.
11. Add the first smoke tests for CLI help, command dispatch, and terminal
   teardown.

Deliverables:

- Create Rust binary crate.
- Add CLI dispatch for `watch`, `doctor`, and `setup`.
- Add CLI flags for database namespace scope and separate operator namespace scope.
- Add logging, terminal lifecycle handling, panic restore, and macOS/Linux platform checks.
- Add placeholder schema versions for machine-readable command output.
- Render an empty `ratatui` frame with clean startup and shutdown.

Exit criteria:

- `cargo fmt --check`, `cargo clippy`, and `cargo test` pass.
- The binary exits cleanly on `q` and `Ctrl-C`.
- Terminal state is restored after normal exit and panic in a test harness.
- `cnpg-watch info --output json` placeholder includes schema version, platform,
  config/cache/log paths, and redaction mode.

### Phase 1: first tested vertical slice

Build the smallest live path that proves collection, identity, state, SQL, and
rendering work together. Keep unit tests independent of Docker/network access;
the separate live proof uses pinned k3d/CNPG/PostgreSQL fixtures. The live gate
belongs here because later transport and scheduler design depends on it.

Implementation steps:

1. Define scoped keys, UIDs, epochs, request IDs, `Observation<T>`, availability,
   typed LSNs, and injected time. Implement checked parsing and core invariants.
2. Add only the topology and fast-SQL capability entries and query metadata
   needed by this slice, with disabled/unknown/forbidden behavior. Defer broad
   profile, view, and feature registries to the phases that use them.
3. Add `Cluster` and `Pod` DTOs/adapters, including missing-field fixtures and
   incarnation metadata. Keep raw JSON out of the reducer.
4. Build a synchronous reducer with typed effects and a deterministic replay
   harness for initialization, apply/delete, scope switches, and time advances.
5. Implement bounded channels, cooperative work budgets, task ownership,
   cancellation, deadlines, and complete relist commit semantics.
6. Build the instance view model and table from accepted observations. Test
   syncing, empty-after-sync, replacement, forbidden, and never-sampled states
   with `TestBackend` before connecting a real terminal.
7. Create a small pinned k3d fixture with one CNPG cluster, a primary and a
   replica, and a separate operator namespace. Use test provisioning to create
   the monitoring role; do not depend on a production setup command yet.
8. Run the monitoring authentication/transport proof: select and document the
   credential path, verify effective role/grants, execute a shipped fast bundle,
   and verify server-side timeout and attachment cleanup from a separate admin
   observation connection. Do not accept a silent superuser fallback.
9. Connect the proven executor and one role-aware fast bundle to the reducer
   and table. Record tickets and reject late or obsolete completions.
10. Add paused-time scheduler tests, property-based event sequences, idle-primary
    lag tests, and an initial adapter/reducer/view/render benchmark baseline.
11. Exercise real watch reconnect and pod replacement, SQL failure with retained
    samples, freshness expiry without new input, bounded queues, and shutdown.
    Cover context/selection races deterministically even if interactive context
    switching is added later.
12. Record the chosen transport, limits, measurements, and unresolved constraints
    in the ADRs. Add a small CI live-proof job using the same local script.

Deliverables:

- One working path: Cluster/Pod watches -> typed state -> fast SQL bundle ->
  instance table, with topology useful when SQL is unavailable.
- Minimal capability and query metadata for that path.
- Identity-aware reducer and atomic relists, with replay/property coverage.
- Tested monitoring-role authentication and bounded exec lifecycle.
- Initial performance measurements and a small local/CI live fixture.

Exit criteria:

- Unit, replay, and property tests run without a cluster and persist failing
  seeds/sequences for regression coverage.
- A recreated pod cannot inherit samples, counter baselines, or a late delete
  from its previous UID. Old-scope/watch/probe events cannot overwrite current
  evidence; incomplete initialization cannot remove accepted resources.
- The table becomes stale on time alone and keeps last-good SQL values after
  failure. A quiet healthy watch does not expire unchanged Kubernetes objects.
- The actual monitoring role runs the production bundle. Invalid authentication,
  timeout, output overflow, cancellation, and remote failure terminate cleanly
  and produce typed reduced-mode/error evidence.
- An idle primary with a caught-up replica does not trigger replay-lag health.
- The bounded queue/flood smoke and initial benchmarks record input latency,
  memory accounting, reducer/view cost, and achieved sampling duration.
- The first slice is usable before adding backups, managed objects, webhook
  adapters, optional metrics, deep diagnostics, or every planned view.

### Phase 2: permission preflight and setup

Implementation steps:

1. Extend non-mutating `doctor` around the proven slice's capability matrix;
   report feature groups before performing live Kubernetes checks.
2. Add `doctor --output json` with a versioned schema before relying on text
   output in tests.
3. Add Kubernetes self-subject access review checks, then fallback live API
   checks where SSAR is unavailable or forbidden.
4. Add namespace-scope checks separately from cluster-scope checks.
5. Add operator namespace discovery checks separately from database namespace
   checks, including `--operator-namespace`, `--operator-selector`, and
   `--no-operator-watch`.
6. Add ambiguity checks for the exceptional case where discovery sees more than
   one CNPG operator install in the same Kubernetes API server.
7. Implement `setup kubernetes-rbac --dry-run` from the same capability model
   used by `doctor`.
8. Add checked-in generated RBAC examples for namespace, namespace-with-operator,
   and cluster scope.
9. Implement `setup kubernetes-rbac --diff` against readable live RBAC objects.
10. Add `setup kubernetes-rbac --diff --output json` using the versioned schema
    as the setup feature is introduced.
11. Implement `setup kubernetes-rbac --apply` only after dry-run and diff tests
   are deterministic.
12. Implement `setup postgres-role --print-sql` without requiring a live
   database.
13. Add monitor, monitor-reserved, and action role profiles with deterministic
    DDL and validation expectations.
14. Add optional live validation for PostgreSQL role capabilities through the
    selected SQL transport when credentials are provided.
15. Wire profile selection into `watch`, `doctor`, and setup dry-runs so the
    same feature set drives all three.
16. Include resolved budgets, profile gates, redaction mode, and reduced-mode
    reasons in `doctor --output json`.

Deliverables:

- Add `doctor`.
- Add Kubernetes access checks using self-subject access reviews when available.
- Add operator namespace discovery and access checks without requiring operator access for normal dashboard startup.
- Add ambiguity detection for the exceptional case where more than one operator
  install is discovered in the same Kubernetes API server.
- Add `setup kubernetes-rbac --dry-run` and `--apply`.
- Add `setup kubernetes-rbac --diff` with text and JSON output.
- Add `setup postgres-role` with idempotent SQL generation and validation.
- Add `setup postgres-role --print-sql` and monitor/reserved/action role
  profiles.
- Add `--profile` to `watch`, `doctor`, and `setup kubernetes-rbac`.
- Add generated example manifests under `deploy/rbac/`.
- Add feature-grouped capability reporting for topology, SQL probes, poolers, backups, managed objects, storage, services, certificates, and plugins.
- Add machine-readable `doctor` and RBAC diff contracts with schema versions.

Exit criteria:

- `doctor` reports missing capabilities without mutating the cluster.
- RBAC dry-run output matches checked-in manifests.
- RBAC diff output is deterministic and distinguishes missing, changed, and
  extra managed rules.
- `doctor --output json` validates against the checked-in schema fixture and
  includes budgets, profile, redaction mode, and reduced-mode reasons.
- `setup kubernetes-rbac --diff --output json` validates against the checked-in
  schema fixture.
- Namespace-with-operator RBAC dry-run output creates only namespaced bindings
  by default. Optional cluster-scoped webhook/catalog requirements degrade
  cleanly; `--include-cluster-resources` emits separate explicit cluster bindings.
- Live least-privilege tests validate actual list/watch, logs, and WebSocket
  exec against the permissions reported by doctor and generated by setup.
- Namespace-scoped and cluster-scoped access checks are tested separately.
- Same-namespace and different-namespace operator access checks are tested separately.
- Ambiguous operator discovery is tested with one normal readable operator and
  one extra discovered/forbidden candidate.
- PostgreSQL role setup can run in dry-run mode and produces deterministic DDL.
- PostgreSQL role setup can print SQL without connecting to a database.
- PostgreSQL monitor, monitor-reserved, and action role profiles produce
  separate deterministic DDL and validation expectations.
- Profile-specific RBAC dry-run output matches checked-in manifests for `minimal`, `standard`, `incident`, and `secure`.
- Watch mode can run in reduced mode when `pods/exec` is absent.

### Phase 3: Kubernetes topology

Implementation steps:

1. Extend the first slice's kube config resolution with database namespace,
   all-namespace, and separate operator namespace scopes.
2. Expand the existing shared client and watch manager without duplicating
   watch/cache ownership for the same resource and scope.
3. Apply Kubernetes QPS/burst budgets, watch retry policy, cache sync
   deadlines, and collector health reporting.
4. Expand CNPG `Cluster` watches and model cache states as `starting`,
   `syncing`, `synced`, `forbidden`, or `failed`.
5. Extend pod watches and add event watches scoped to visible CNPG clusters.
6. Expand the live instance table into `:fleet` and `:instances` navigation
   using Kubernetes state, retaining cached/never-sampled SQL badges.
7. Add PVC, service, and EndpointSlice watches and surface compact storage and
   connectivity badges.
8. Add CNPG `Pooler` watch for availability only, before PgBouncer stats.
9. Add operator deployment, pod, event, and optional lease watches in separate
   operator state.
10. Implement operator-to-cluster association confidence from explicit config,
   namespace watch evidence, events, and labels.
11. Add Kubernetes-only health reasons and hints.
12. Add bounded selected-resource log reads for instance and pooler contexts,
    but not the full merged timeline yet.
13. Add collector-level counters for events received, last sync time, last
    error, retry count, dropped log lines, and stale caches.
14. Add local k3d e2e fixture for one operator namespace, one database
    namespace, one CNPG cluster, and one reduced-RBAC run.

Deliverables:

- Load kube config.
- Apply Kubernetes watch budgets and expose collector health.
- Watch CNPG clusters, pods, poolers, and events.
- Watch CNPG operator deployment, pods, and events from the configured or
  discovered operator namespace when enabled.
- Watch the configured or discovered operator namespace and render `:operator`.
  If multiple operator installs are discovered, render an ambiguity state.
- Render `:fleet` by default when multiple clusters are visible.
- Watch PVCs, services, and endpoint slices associated with visible clusters.
- Render cluster list and instance table with primary/replica/readiness from Kubernetes state.
- Render compact service/storage warnings in the main dashboard.
- Render `:health` and `:hints` from Kubernetes-only evidence before SQL probes exist.
- Add a read-only `:logs` skeleton that can show recent bounded logs for the selected instance or pooler when `pods/log` is allowed.
- Preserve watch health independently from object health.
- Keep Kubernetes topology updating when SQL probes are disabled, forbidden, or
  backing off.

Exit criteria:

- Fake watch integration tests prove reducer behavior.
- Watch-manager tests prove retry/backoff, cache-sync timeout, and forbidden
  watch behavior without a live Kubernetes API.
- Local e2e test can observe a CNPG cluster in k3d.
- Local e2e test installs the CNPG operator in a different namespace than the test cluster and validates operator reduced/full modes.
- Local e2e deploys at least two CNPG clusters across namespaces and validates `:fleet` grouping and drill-down.
- Deleting a pod updates topology without restarting the dashboard.
- While initial caches are warming, the UI shows `syncing` rather than `no clusters found`.
- Switching namespace or context cancels old watches/probes and does not reuse stale rows.
- Leaving the log view closes active log streams.

### Phase 4: expand PostgreSQL runtime probes

Implementation steps:

1. Extend the executor proven in Phase 1; keep its authentication and cleanup
   contracts intact. Add transport alternatives only when a concrete need is
   demonstrated.
2. Extend bounded bundle execution using `psql -X -A -t -w -v ON_ERROR_STOP=1`
   when exec is the selected transport, with remote-status validation, timeout,
   stderr capture, redaction, and structured error classification.
3. Add capability detection for `server_version_num`, `pg_is_in_recovery()`,
   monitor role membership, readable views, and optional columns.
4. Expand primary, replica, and archiver bundle metadata in the version-gated
   query registry, retaining typed DTOs and live least-privilege contracts.
5. Extend fast bundles and compatible stamped evidence for primary WAL LSN,
   replica receive/replay LSN, validated byte lag, and archiver status.
6. Extend admission with process/per-cluster/per-pod concurrency, bounded pending
   intents, fair priority, jitter, freshness deadlines, and coalesced refresh.
7. Add adaptive backoff for timeout, too many connections, refused connection,
   authentication failure, admin shutdown, and pod exec failures.
8. Enforce the database-under-load policy: no unbounded probe queues, no
   heavyweight watch-mode SQL, visible stale sample ages, and independent
   Kubernetes/log updates while SQL backs off.
9. Add replication-slot and synchronous-replication probes as medium-priority
   probes with version and permission gates.
10. Add connection-pressure and wait-summary probes as profile-gated lower
   priority probes.
11. Update `:health`, `:hints`, `:connections`, and `:waits` from probe results
    without returning full session lists or raw query text by default.
12. Extend k3d e2e to validate LSN/archive fields and forced SQL failure/stale
    states.

Deliverables:

- Implement exec-based JSON SQL probes.
- Implement the SQL executor abstraction with pod-exec as the first transport.
- Implement the version-gated query registry for PostgreSQL probes.
- Show primary WAL LSN, replica receive/replay LSN, and lag.
- Add `pg_stat_archiver` status.
- Add replication slot and synchronous replication probes where permissions and server version allow it.
- Add profile-gated connection pressure and wait diagnostics probes.
- Add stale/error states, adaptive backoff, and forced refresh.
- Apply database-under-load behavior consistently across SQL panels and health
  findings.

Exit criteria:

- Probe parser tests use captured stdout/stderr fixtures.
- Query registry tests cover supported, unsupported, and missing-column
  fallbacks for the oldest and newest supported PostgreSQL versions.
- Transport tests prove probe parsing does not depend on pod-exec internals.
- Local e2e test validates LSN and archive fields against a real CNPG PostgreSQL pod.
- Unit tests cover replication-slot and sync-rep query parsing with unsupported-version fallbacks.
- Unit tests cover connection pressure, wait diagnostics, redacted query summaries, and disabled-by-profile behavior.
- Forced timeouts or connection saturation produce backoff and stale-sample UI state instead of blank data.
- Connection pressure and wait diagnostics slow down or disable before core topology probes under load.
- Kubernetes topology, events, and allowed logs continue updating while SQL
  probes are timing out, forbidden, or backing off.

### Phase 5: Poolers and PgBouncer

Implementation steps:

1. Expand the `Pooler` adapter to normalize desired, ready, unavailable, phase,
   type, service, image, and config evidence.
2. Add pooler service and EndpointSlice association to the pooler view model.
3. Implement `PgbouncerAdminExecutor` over pod exec.
4. Add PgBouncer `SHOW POOLS` and `SHOW STATS` parsers from fixtures before
   running them live.
5. Add pooler probe scheduling independent of direct PostgreSQL instance probes.
6. Render pooler availability separately from pooler connection counts.
7. Implement `setup pooler-role --dry-run` and documentation output for
   installations the tool cannot safely mutate.
8. Add live validation for PgBouncer stats only when credentials and safe
   transport are configured.
9. Extend k3d e2e with a CNPG Pooler fixture and a PgBouncer stats success and
   failure case.

Deliverables:

- Watch CNPG Pooler resources and pooler pods.
- Render desired, ready, unavailable, and type when available.
- Add PgBouncer stats probe for `SHOW POOLS` and `SHOW STATS`.
- Add `setup pooler-role` where the installation can be safely configured.

Exit criteria:

- Unit tests parse PgBouncer output and classify pooler failures.
- Local e2e test validates pooler readiness and PgBouncer connection counts.
- Pooler outage does not hide direct PostgreSQL instance health.

### Phase 6: CNPG operational resources

Implementation steps:

1. Add backup and scheduled-backup adapters, fixtures, views, and health rules.
2. Add storage and service detail views using PVC, service, and EndpointSlice
   state already collected in Phase 3.
3. Add managed-object adapters and views for `Database`, `DatabaseRole`,
   `Publication`, and `Subscription`.
4. Add replica topology, replication-slot, delayed replica, and synchronous
   replication view-models from CNPG status plus SQL evidence.
5. Add plugin, image catalog, operator version, and CRD version posture.
6. Add admission webhook health checks for webhook configs, backing services,
   endpoints, pods, and optional certificate expiry.
7. Add optional operator metrics as a profile-gated collector, with disabled and
   unavailable states tested first.
8. Expand logs from selected-resource logs into CNPG-aware merged timelines for
   cluster, instance, pooler, backup, event, and operator log sources.
9. Add operator log correlation by namespace/name, event context, reconcile key,
   timestamp proximity, and source toggles.
10. Add optional certificate view with strict Secret redaction.
11. Expand screen/model dumps to include capability state, version posture,
    probe ages, reduced-mode reasons, and redacted log/sample context.
12. Add reduced-RBAC e2e cases for missing secrets, managed-object permissions,
    webhook access, operator logs, and `pods/log`.

Deliverables:

- Watch CNPG `Backup` and `ScheduledBackup` resources and render `:backups`.
- Watch managed `Database`, `DatabaseRole`, `Publication`, and `Subscription` resources when available.
- Watch `FailoverQuorum`, `ImageCatalog`, and `ClusterImageCatalog` resources when available for failover readiness and version posture.
- Watch webhook configuration, webhook service endpoints, leader leases, and optional operator metrics when enabled.
- Render `:roles`, `:databases`, and `:subscriptions` reconciliation views.
- Render reconciliation health in `:operator`, `:fleet`, `:health`, and cluster detail views.
- Render `:storage`, `:services`, `:replicas`, `:plugins`, and optional `:certs` detail views.
- Expand `:logs` into a CNPG-aware merged timeline for cluster, instance, pooler, backup, and event scopes.
- Include operator namespace status, events, and logs in `:logs`, `:health`, `:hints`, `:plugins`, and `doctor` when permitted.
- Correlate operator logs by namespace/name, event context, reconcile key, and timestamp before adding them to cluster-scoped log timelines.
- Add admission/webhook health to `:operator`, `:health`, `:hints`, and `doctor`.
- Add optional operator metrics to `:operator`, `:fleet`, `:health`, and `:hints`.
- Expand `:health` and `:hints` with backup, service, storage, log, connection, wait, plugin, and managed-object evidence.
- Add version posture to `doctor`, `info`, and screen/model dumps.
- Add optional certificate expiration checks with strict redaction.

Exit criteria:

- Fixture tests cover each supported CNPG CRD status adapter, including missing CRD and forbidden-resource reduced modes.
- Local e2e validates backup/scheduled-backup visibility and service endpoint readiness against the k3d fixture.
- Local e2e validates CNPG-aware logs for primary/replica/pooler scopes and verifies bounded follow behavior.
- Local e2e validates operator logs from a separate operator namespace and reduced mode when only app namespace access is granted.
- Local e2e validates multiple visible clusters, fleet-level health findings, and operator-to-cluster association confidence.
- Local e2e validates webhook health reduced/full modes and optional operator metrics disabled-by-default behavior.
- Reduced RBAC e2e proves missing Secret or managed-object access degrades only the affected feature view.
- Reduced RBAC e2e proves missing `pods/log` disables only log views.
- Screen/model dumps include CNPG version, resource capability, and redacted certificate state without leaking secret data.

### Phase 7: terminal polish and release readiness

Implementation steps:

1. Finalize command mode, filter mode, history, previous/next navigation, help,
   `info`, and screen/model dumps.
2. Stabilize responsive layouts for 120x40, 100x30, and narrow degraded
   terminals.
3. Add deterministic widget buffer tests and full-screen snapshots for `Fleet`,
   `Cluster`, and `Detail` modes, including shared header/footer, breadcrumbs,
   row actions, reduced-mode banners, and contextual help.
4. Add deterministic widget buffer tests and full-screen snapshots for healthy,
   degraded, no-permission, overloaded, no-cluster, pooler saturation, backup
   failure, endpoint mismatch, storage pending, and long-text states.
5. Implement the real PTY smoke harness with `portable-pty` and `vt100`, adding
   arbitrary narrow sizes, Unicode/wide names, input under log flood, and clean
   teardown cases alongside fixed-size snapshots.
6. Run the real TUI smoke suite against the k3d fixture and keep it small,
   serial, and artifact-rich.
7. Add release packaging for macOS Apple Silicon, macOS Intel, Linux x86_64,
   and Linux aarch64.
8. Generate shell completions and manpage or markdown command reference from
   the CLI definition.
9. Add checksums and package smoke tests that run `--help`, `info --output
   json`, and terminal teardown for each built artifact.
10. Add CI matrix for format, clippy, tests, build, snapshot checks, and package
   smoke.
11. Expand the early Linux live-proof job into `e2e-k3d` after fast checks, with
    pinned k3d/CNPG versions and the declared Kubernetes permission matrix.
12. Document support boundaries, required RBAC profiles, known reduced modes,
   and first-release non-goals.

Deliverables:

- Add filtering, help overlay, responsive layout, and concise error display.
- Add command mode, command/filter history, previous/next navigation, `info`, and screen/model dumps.
- Add deterministic TUI buffer tests for the core layouts at supported terminal sizes.
- Add an involved PTY-driven TUI smoke suite against the k3d e2e cluster.
- Add release packaging for macOS and Linux.
- Add release checksums, shell completions, and generated command reference.
- Add CI matrix for Linux and macOS unit/build checks.
- Add CI e2e job for Kubernetes behavior.

Exit criteria:

- UI remains readable at 120x40 and degrades to 100x30.
- `Fleet`, `Cluster`, and `Detail` mode snapshots include the shared header,
  bottom command/status bar, breadcrumb where applicable, row actions, and
  reduced-mode language.
- Snapshot tests cover healthy, degraded, no-permission, overloaded, no-cluster, and pooler-saturation screens.
- Snapshot tests cover backup failure, storage pending, service endpoint mismatch, managed-object pending reconciliation, plugin warning, and certificate-expiring states.
- Snapshot tests cover syncing, empty-after-sync, filtered-empty, and long/wide-column states.
- Real TUI smoke validates startup, rendering, wrapping, resizing, navigation, help, filtering, refresh, doctor/setup command paths, live Kubernetes watches, SQL probe display, and PgBouncer/pooler display.
- macOS and Linux builds pass in CI.
- Release artifact smoke tests pass for `darwin-arm64`, `darwin-amd64`,
  `linux-amd64`, and `linux-arm64`.
- Checksums and shell completions are generated deterministically.
- Pipeline e2e passes on a clean GitHub-hosted Ubuntu runner.
- Release-build latency, idle CPU, memory accounting/RSS, and scheduled-soak
  results establish the supported fleet scale and show no task/source leak.

## Testing strategy

Testing is split into fast unit tests, integration tests that do not require a live cluster, local e2e tests, and pipeline e2e tests.

Add coverage as each working feature arrives. The broader lists below define
eventual coverage, not a requirement to build dormant parsers, view registries,
or fake production APIs before the first slice. Test externally meaningful
behavior and invariants; avoid tests that merely repeat implementation logic.

### Unit tests

Run on every change with `cargo test`.

- View-model conversion from app state to render-only UI data.
- Runtime profile feature gates, interval settings, redaction settings, and profile-specific RBAC requirements.
- Operator namespace parsing, discovery fallback, selector handling, and reduced-mode messages.
- Ambiguous operator discovery when more than one plausible install is visible in a single Kubernetes API server.
- Operator-to-cluster association confidence and evidence ranking.
- Fleet row grouping, sorting, filtering, top-reason selection, and drill-down targets.
- Reconciliation health for generation/observedGeneration drift, pending managed objects, pooler drift, and stale conditions.
- Admission webhook health adapters for webhook configs, services, endpoints, certificates, and reduced-mode states.
- Optional operator metrics parser and disabled/unavailable/stale states.
- Health reason ranking, deduplication, stale evidence handling, navigation targets, and reduced-mode severity.
- Runbook hint rule IDs, evidence references, dismissal-until-change, and command text generation.
- PostgreSQL LSN parsing and byte-delta calculations.
- CNPG dynamic object adapters for supported CRD samples.
- Pod label extraction and cluster ownership matching.
- PVC, Service, EndpointSlice, Backup, ScheduledBackup, managed-object, plugin-status, and certificate-summary adapters.
- SQL JSON parsing for primary, replica, archiver, and pooler probes.
- SQL JSON parsing for replication slots and synchronous replication state where supported.
- SQL JSON parsing for connection pressure and wait diagnostics probes.
- Log parsing for PostgreSQL JSON logs, plain PostgreSQL logs, PgBouncer logs, CNPG operator logs, malformed lines, and missing timestamps.
- Log source merge ordering, source toggles, pause/follow state, bounded retention, truncation markers, and redaction.
- Query hash/text redaction and dump redaction for users, databases, client addresses, and application names when profile settings require it.
- PostgreSQL and PgBouncer error classification for overload, timeout, authentication, and unavailable states.
- Kubernetes RBAC manifest generation.
- PostgreSQL role DDL generation is idempotent and least-privilege.
- Reducer behavior for apply/delete/restart/stale events.

### Integration tests

Run on every pull request when they do not require Docker or a live Kubernetes API.

- Fake Kubernetes watch streams feeding the reducer.
- Exec probe parser using captured stdout/stderr fixtures.
- Permission preflight with fake self-subject access review responses.
- Setup dry-run output compared to checked-in RBAC manifests.
- Profile-specific setup dry-run output compared to checked-in RBAC manifests.
- Namespace-with-operator setup dry-run output compared to checked-in RBAC manifests.
- Terminal snapshot tests for key UI states where practical.
- CRD fixture tests for multiple CNPG resource versions where fields differ.
- Fake operator deployment/pod/event streams in the same namespace and in a separate operator namespace.
- Fake ambiguous operator discovery with one normal operator install and one extra plausible install in the same Kubernetes API server.
- Fake webhook configuration/service/endpoint streams and metrics endpoint responses.
- Fake pod-log streams feeding the log reducer, including stream errors and permission failures.
- Fake SQL probe streams feeding connection pressure and wait diagnostic reducers under overload/backoff conditions.

### Deterministic replay and state-machine properties

Build this harness with the first slice. A replay contains an initial scope,
typed external events, identities/epochs/request IDs, terminal/input actions
when relevant, and explicit monotonic/wall-clock changes. Apply each step to
the same pure reducer and scheduler decisions used in production. Record a
bounded redacted event trace on failure; fixtures must never contain credentials.

Required sequences:

- Start a probe, replace its pod with the same name/new UID, accept a new
  sample, then deliver the old probe result and old UID's delete.
- Switch context/scope or selected target with pending events; deliver results
  from old scope/watch/topology epochs after the replacement is active.
- Miss a delete during disconnection; complete a relist without that object.
  Also interrupt a partial relist and verify the prior complete set survives.
- Repeat unchanged initialization objects and verify they remain members of
  the committed set even when content deduplication skips field updates.
- Advance time without any new collector events; verify stale cells, evidence
  expiry, backoff eligibility, and age-based eviction. Shift wall time without
  changing monotonic time and verify deadlines are unaffected.
- Converge independent primary/service watches during failover; verify pending
  evidence, configured persistence, detection, and clearing of mismatch health.

Use `proptest` to generate valid lifecycle sequences and legal interleavings
across independent sources. Do not assume arbitrary reordering within one
ordered watch stream. Assert properties rather than snapshots of internal
struct layout:

- Duplicate accepted observations are idempotent.
- Late results/deletes cannot change a newer incarnation or active scope.
- Failed attempts preserve last-good evidence; absence never becomes zero lag.
- Completed relists contain exactly their listed keys; incomplete relists do
  not remove accepted objects or claim synchronization.
- Retained bytes, queued bytes/messages, active streams/tasks, and pending
  probe intents stay within configured bounds.
- Incremental derived state matches full recomputation from the same accepted
  observations, navigation, terminal dimensions, and supplied time.

Persist failing seeds and minimized event traces as regression fixtures. Run
bounded property cases in the fast PR suite and longer runs in the scheduled
suite. Exact replay tests remain the first place to diagnose a lifecycle race.

### Paused-time scheduling and cancellation tests

Extend time injection through the entire app, collectors, health rules, and
scheduler. Use Tokio's paused/advanced time for async scheduling tests, with an
injected seeded jitter source. Pure scheduling tests can supply `Now` directly.
No test should wait seconds of wall time to verify backoff.

Assert exact eligibility around timeout, freshness, interval, and backoff
boundaries; global/per-cluster/per-pod admission; priority and fairness;
coalesced forced refresh; and suspension/recheck after authentication failure.
Simulate a long runtime stall and verify there is no catch-up probe burst.
Verify permits and pending intents are reclaimed on queue cancellation, source
replacement, parser failure, timeout, and shutdown. Test full channels while
shutting down so cleanup cannot deadlock waiting for an undrained result queue.

### Live SQL and transport contracts

Captured JSON proves parsing, but does not prove that a query exists, has the
claimed grants, or works through the actual transport. Run the production
bundles on the pinned live PostgreSQL/CNPG fixture as the intended monitoring
role, and deserialize with production DTOs. Cover primary, replica, idle
primary/caught-up replica, and promotion/role-change cases.

Validate the authentication and timeout proof before expanding the query
registry. Test insufficient grants, invalid credentials, unavailable exec,
remote nonzero status with JSON stdout, oversized stdout/stderr, and stalled
or cancelled attachments. A separate test-only admin connection observes probe
termination; it is never the application's fallback identity. Verify real
transport RBAC with get-only/create-only/both/neither identities for supported
Kubernetes versions. Also validate namespaced operation without cluster-scoped
webhook/catalog/CRD object reads.

Every supported server version must run every bundle variant enabled for that
version. Add an older-version support claim only with fixtures and live query
coverage. Credential/setup tests apply DDL twice, compare resulting privileges,
and run actual probes; a deterministic SQL snapshot alone cannot prove
idempotence or least privilege.

### Performance benchmarks and bounded-load tests

Establish a baseline in Phase 1 and extend it with each hot path. Use Criterion
for adapters, event acceptance/relist commit, health rules, sorting/filtering,
view conversion, and buffer rendering. Benchmark unchanged updates and changed
updates separately. Measure the full application loop as well as isolated
functions; fast parsing alone does not prove responsive input.

Use fixed seeds and workloads of 10, 100, and 1,000 clusters, normally three
instances each. Record total related resources/payload bytes, terminal size,
selected view, filter, update rate, log rate, source count, and enabled probes.
Use 120x40 and 100x30 for render comparisons. Large-fleet cases may initially
be characterization runs; publish which scale meets the supported targets.

Initial acceptance targets for a release build at the 100-cluster workload:

| Measure | Initial target/condition |
| --- | --- |
| Input-to-visible response | p95 <=100 ms and p99 <=250 ms during topology/log flood, excluding external API latency |
| View preparation + frame rendering | p95 <=16 ms for the visible view; maximum 10 draws/s |
| Idle CPU | <=1% of one core after synchronization, with no source traffic except normal deadlines |
| Memory after warm-up | <=128 MiB RSS for the defined fixture with logs at their configured caps; retained byte counters within hard limits |
| Lifecycle cleanup | no growth in live collector/exec task or stream counts after repeated scope/view changes and shutdown |

These are proposed engineering targets, not observed results. Record the first
baseline's machine/OS, Rust/toolchain and dependency versions, release profile,
fixture sizes, and whether allocation/RSS includes staging and transport
buffers. If the initial baseline cannot meet a target, investigate and record
the measured limit and any justified target change in the performance ADR
before claiming supported scale. Memory counters bound application payloads;
RSS measurement also captures allocator/runtime/transport overhead.

Flood tests should include a sustained topology burst, eight selected log
streams totaling at least 5,000 lines/s, oversized unterminated lines, large
probe output, and slow UI consumption. Verify input, watch recovery, time
deadlines, visible drop counters, and bounded queues while producers are busy.
Run a 60 s bounded-load case in the dedicated validation job and a 30 min
scheduled soak that cycles scopes/filters/log sources and repeated reconnects.
After warm-up, retained memory, source metadata, and live tasks must plateau
instead of growing with every visited cluster. Account for temporary relist
staging peaks and verify they are released after commit or cancellation.

Track complete sampling duration and age distributions, API establishment rate,
queue occupancy/queued bytes, reducer batch duration, and time spent preparing
views. Report skipped/coalesced work alongside throughput. Use relative
benchmark baselines on controlled runners; do not enforce tiny wall-time
regression thresholds on noisy shared CI machines. Fast checks enforce
deterministic bounds/invariants, while a dedicated job reports timing/RSS and
investigates sustained regressions. Re-run affected benchmarks after hot-path
changes rather than repeating every benchmark after documentation edits.

### TUI test strategy

Testing a terminal UI by launching an interactive terminal in CI is fragile. The design should make the TUI testable without a real terminal for nearly all behavior.

Use this testing pyramid:

1. View-model unit tests.
   - Convert `AppState` into plain structs such as `InstanceRow`, `ArchivePanel`, `PoolerPanel`, `EventRow`, and `StatusBar`.
   - Assert sorting, filtering, focus, stale markers, colors, warning levels, and reduced-mode messages directly.
   - These tests provide most behavioral coverage and do not depend on terminal dimensions.

2. Widget buffer tests.
   - Render individual widgets into `ratatui::buffer::Buffer` with fixed `Rect` sizes.
   - Assert important cells, styles, truncation, and alignment.
   - Prefer direct `Buffer` assertions for small widgets where failures should point to exact cells.

3. Screen snapshot tests.
   - Use `ratatui::backend::TestBackend` for whole-screen rendering at stable sizes such as `120x40`, `100x30`, and one narrow failure case.
   - Serialize the buffer into a deterministic plain-text form and compare it with `insta::assert_snapshot!`.
   - Keep snapshots focused on representative states, not every possible data permutation.
   - Run CI with `INSTA_UPDATE=no` so snapshots cannot change in the pipeline.

4. Event-loop tests.
   - Abstract terminal input behind an `InputSource` trait. Use the shared
     injected clock/deadline model across UI, reducers, collectors, and probes.
   - Feed synthetic key events and ticks into the app loop without crossterm.
   - Assert resulting actions and selected state rather than terminal bytes.

5. Real PTY smoke/e2e.
   - Spawn the actual `cnpg-watch` binary in a pseudo-terminal connected to the live k3d test cluster.
   - Parse terminal output with a terminal emulator/parser and assert the visible screen.
   - Send real key input, resize the PTY, trigger cluster changes, and verify the UI updates.
   - Use this as a broad product smoke test, while still keeping fine-grained rendering assertions in view-model, buffer, and snapshot tests.

Recommended framework:

- Use ratatui's `TestBackend` for full-screen integration rendering. Ratatui documents it as an in-memory backend for integration testing and exposes buffer, cursor, resize, and assertion helpers.
- Use direct `Buffer` tests for individual widgets. Ratatui's own guidance notes that widget unit tests are better written against the buffer directly, with `TestBackend` reserved for whole-terminal UI tests.
- Use `insta` for reviewed snapshots of serialized buffers and selected view models. Snapshot tests are a good fit for large terminal screens because they provide reviewable diffs instead of unreadable string literals.
- Use `portable-pty` for real TUI smoke tests. It provides a pseudo-terminal API that can spawn commands, set terminal size, write input, and read output from the PTY.
- Use `vt100` to turn the real terminal byte stream into an in-memory screen. This lets tests inspect visible text, wrapping, colors, cursor state, and post-resize layout without scraping raw escape sequences.
- Use `assert_cmd` only for non-interactive CLI paths such as `--help`, `doctor --output json`, and setup dry-runs.

Snapshot policy:

- Commit approved `.snap` files.
- Do not auto-update snapshots in CI.
- Use stable fixture data with fixed timestamps, widths, namespaces, pod names, event ordering, and color mode.
- Strip ANSI escape codes from snapshots unless a specific test is asserting terminal escape output.
- Snapshot semantic screen text and styles separately when style churn would make text snapshots noisy.
- Keep one snapshot per user-visible state, not per internal implementation detail.
- Add property-based layout checks over terminal dimensions, long/wide Unicode
  text, empty selections, and changing row counts. Check clipping, valid
  selection, and absence of panics; use reviewed buffers for actual layout
  intent rather than treating a successful draw as proof of readability.

The TUI code should expose a test helper like:

```rust
fn render_to_lines(state: &AppState, width: u16, height: u16) -> Vec<String>
```

That helper should build the view model, render through `TestBackend`, and return normalized lines. CI can then assert snapshots without launching a real terminal.

### Fixture naming conventions

Use stable, descriptive paths so fixtures can be found from test failures:

```text
fixtures/
  cnpg/1.30/cluster/healthy.json
  cnpg/1.30/cluster/failover-pending.json
  cnpg/1.30/pooler/ready-rw.json
  cnpg/1.30/backup/failed-plugin.json
  cnpg/1.30/operator/separate-namespace-ready.json
  postgres/18/probes/primary-fast.json
  postgres/18/probes/replica-fast.json
  postgres/18/probes/archiver-failed.json
  postgres/18/probes/connection-pressure.json
  postgres/18/errors/too-many-connections.stderr
  pgbouncer/show-pools/healthy.txt
  pgbouncer/show-stats/saturated.txt
  logs/postgres/json-error.log
  logs/operator/reconcile-error.log
  rbac/standard/missing-pods-exec.json
  rbac/incident/missing-pods-log.json
  replay/pod-recreated-late-probe.json
  replay/relist-interrupted.json
  replay/old-context-result.json
  replay/freshness-without-events.json
  workloads/fleet-100.json
  terminal/120x40/healthy.snap
```

Fixture filenames should describe the operator-facing state, not the internal
test name. Tests should prefer loading fixtures by explicit path so failures are
easy to trace.

### Command output contracts

Non-interactive commands should have stable text output for humans and JSON
output for tests and automation.

Required contracts:

- `cnpg-watch doctor --output json`: reports context, namespace scope, operator
  scopes, profile, feature capability states, reduced-mode reasons, and
  suggested setup commands.
- `cnpg-watch setup kubernetes-rbac --dry-run --output yaml`: emits only the
  intended Kubernetes objects, sorted deterministically.
- `cnpg-watch setup kubernetes-rbac --diff --output json`: reports missing,
  changed, matching, and extra managed RBAC rules without applying changes.
- `cnpg-watch setup postgres-role --print-sql`: emits deterministic SQL for the
  selected role profile and PostgreSQL target version.
- `cnpg-watch info --output json`: reports config path, log path, dump path,
  cache path, binary version, build target, active profile, kube context, and
  known support boundaries.
- `cnpg-watch --help`: lists watch-safe commands separately from setup commands.

JSON outputs should be versioned with a top-level `schema_version` field before
the first public release.

### Self-observability

`cnpg-watch` should expose its own runtime state in `info`, dumps, debug logs,
and selected UI badges.

Track:

- collector status by resource and namespace;
- watch restarts, last sync time, and last error;
- object counts by resource type;
- SQL/PgBouncer probe in-flight count, last success, last error, backoff state,
  and dropped/disabled reason;
- log source state, bytes retained, lines retained, dropped line counts, and
  active follow streams;
- reducer event counts and skipped unchanged-object counts;
- rejected obsolete events by scope/watch/topology epoch, UID, and request ID;
- current scope/watch epochs and bounded initialization staging bytes;
- queue depth, queued bytes, high-water marks, and backpressure/drop counts by
  traffic class;
- pending/coalesced/throttled probe intents, admission wait, actual sweep
  duration, sample age distribution, and remote/local cleanup duration;
- live task/exec/stream counts, retained inactive sources, parsed/retained bytes,
  and bounded cache/staging totals;
- reducer batch duration, dirty/full-recompute counts, view-preparation time,
  frame time/rate, and input-to-visible latency;
- capability matrix result for the active profile;
- redaction mode and dump safety mode.

This data should make support dumps useful even when the visible TUI only shows
a compact reduced-mode banner.

### Failure-injection plan

The e2e suite should include controlled failure scenarios because degraded
behavior is a core product requirement.

MVP/Alpha failure cases:

- remove `pods/exec` and verify SQL probes disable without breaking topology;
- remove `pods/log` and verify only log views disable;
- run with no operator namespace access and verify operator evidence degrades
  without blocking cluster observation;
- delete a replica pod and verify watches update without restarting;
- recreate a pod under the same name and verify old samples/deletes cannot
  affect its replacement;
- reconnect after a missed delete, then interrupt another relist, and verify
  exact membership only changes after complete initialization;
- advance freshness deadlines while collectors are quiet and verify stale
  display without expiring unchanged objects on healthy watches;
- force a SQL probe timeout and verify stale last sample plus backoff;
- verify the test observer sees timed-out SQL terminate and task/exec counts
  return to baseline;
- flood selected logs and oversize framing/output while input, topology,
  freshness, queue bounds, and visible truncation remain correct;
- change scope with pending work and shut down with full result queues; verify
  cleanup completes and old-scope results cannot repopulate the model;
- leave a primary idle with its replica caught up and verify no false lag alarm;
- create connection pressure or simulate `too many connections` and verify
  `:connections` and health reasons;
- patch a service selector or EndpointSlice fixture and verify endpoint mismatch
  health;
- block PgBouncer stats and verify pooler readiness remains visible.

Beta/GA failure cases:

- break archive or backup configuration and verify `:backups`, `:archive`, and
  log hints correlate the evidence;
- deny managed-object permissions and verify only managed-object views degrade;
- deny Secret access and verify certificate checks degrade without leaking
  secret names beyond configured redaction policy;
- deny webhook permissions and verify admission health degrades only;
- inject malformed PostgreSQL JSON logs, PgBouncer output, SQL JSON, and CNPG
  objects to prove parsers fail closed.

### Real TUI smoke/e2e tests

The project should include a more involved real-terminal smoke suite because `cnpg-watch` is primarily a TUI and many important failures only appear when the real event loop, terminal backend, Kubernetes watches, and SQL probes are all running together.

Use a harness roughly shaped like:

```rust
struct TuiHarness {
    pty: portable_pty::PtyPair,
    parser: vt100::Parser,
    child: Box<dyn portable_pty::Child>,
}
```

The harness should provide operations such as:

- `spawn_cnpg_watch(args, env, size)` with a real kubeconfig pointing at the k3d cluster;
- `send_key("j")`, `send_key("/")`, `send_key("r")`, `send_key("?")`, and `send_ctrl_c()`;
- `resize(cols, rows)`;
- `wait_for_text("payments")` and `wait_until_screen(|screen| ...)`;
- `screen_text()` and `screen_cells()` for assertions;
- `snapshot_screen(name)` for optional reviewed smoke snapshots;
- `terminate_and_assert_clean_exit()`.

This suite should validate:

- Startup on the e2e kube context and selected namespace.
- The top bar shows context, namespace, operator namespace when configured, selected cluster, watch health, and sample age.
- `:fleet` groups clusters by namespace, shows top health reasons, operator association, backup/lag/pooler summaries, and drills into a selected cluster.
- `:operator` shows the configured/discovered operator install,
  leader/readiness/restarts/version/log capability/webhook/metrics state,
  reduced-mode reasons, and an ambiguity banner if an extra operator install is
  detected.
- Instance rows render primary/replica roles, readiness, LSNs, lag, and stale/error state.
- WAL/archive and pooler panels render real values from the k3d CNPG fixture.
- Backup, storage, service endpoint, replica topology, plugin, and managed-object views render deterministic fixture state when available.
- `:logs` opens the selected cluster timeline, toggles primary/replica/pooler/operator sources, filters by severity, pauses/resumes follow mode, and preserves layout under long log lines.
- Operator log source labels include the operator namespace when it differs from the database namespace.
- Cluster-scoped operator logs include only correlated lines by default, while `:operator` can show full operator logs.
- `:health` ranks endpoint mismatch, primary probe failure, lag, backup failure, permission gaps, and optional-feature reductions correctly.
- `:hints` shows contextual next-step hints and jumps to the related view.
- `:connections` shows Postgres and PgBouncer pressure without full session lists.
- `:waits` shows wait summaries and redacted long-running query hashes.
- `:profile` shows the active profile and feature gates.
- Event feed updates after deleting or restarting a replica pod.
- `/` filtering changes visible clusters or instances and can be cleared.
- `Tab`, arrow keys, `j`, `k`, `r`, and `?` perform the expected actions.
- Long pod names, event messages, and errors truncate or wrap without overlapping adjacent panels.
- Resizing from `120x40` to `100x30` keeps the instance table usable and collapses lower-priority panels as designed.
- Running with intentionally reduced RBAC shows reduced-mode banners instead of crashing.
- Removing optional access to backups, managed objects, endpoint slices, PVCs, or secrets degrades only the related views.
- Removing operator namespace access degrades only operator status/log/version evidence.
- Removing webhook or metrics permissions degrades only admission/metrics sections.
- Removing `pods/log` access disables only log views and keeps topology, SQL probes, and events usable.
- Simulated SQL probe timeout or connection saturation shows stale samples and backoff state.
- Under connection saturation, `:connections` shows pressure while deeper `:waits` diagnostics back off before core probes.
- Pooler saturation or PgBouncer stats failure is visible while direct instance health remains visible.

Run this suite in CI as part of `e2e-k3d`, not as a separate fake terminal job. It should use the same live CNPG cluster that the Kubernetes e2e tests create, so it validates commands, queries, watches, and formatting together.

To keep it reliable:

- Use fixed terminal sizes and fixture object names.
- Use bounded waits on semantic text or screen predicates, never arbitrary sleeps as the only synchronization.
- Freeze or normalize clocks where practical; otherwise assert age formats with ranges.
- Disable animations and nonessential blinking in test mode.
- Save raw PTY logs, parsed screen snapshots, `kubectl get events`, pod logs, and `cnpg-watch` debug logs on failure.
- Run the suite serially because it owns the terminal, kube namespace, and CNPG fixture.
- Keep the number of real-terminal scenarios small but meaningful; broaden coverage with view-model and buffer tests.

### E2e substrate choice

Use `k3d` as the default local and GitHub Actions e2e substrate.

Rationale:

- `k3d` runs k3s in Docker and is intended for local Kubernetes development.
- GitHub-hosted Ubuntu runners include Docker and kubectl, which makes k3d a natural fit for pipeline e2e.
- macOS developers can run the same k3d workflow with Docker Desktop, Colima, or another supported Docker-compatible runtime.
- Native k3s is closer to some production installs, but it brings more host-level assumptions around systemd, cgroups, networking, and firewall behavior. Keep native k3s as an optional Linux parity job or manual smoke target, not the default.

This recommendation is based on the current k3d docs describing k3d as k3s-in-Docker for local development, the k3d Docker/kubectl requirements, GitHub's hosted-runner specs for Ubuntu runners, the current Ubuntu runner image software list showing Docker and kubectl, and the native k3s requirements around host resources, networking, cgroups, and firewall configuration.

Local prerequisites:

- macOS or Linux.
- Docker-compatible runtime.
- `kubectl`.
- `k3d`.
- `helm` or plain manifests for installing the CNPG operator.

Pipeline prerequisites:

- GitHub Actions `ubuntu-24.04` runner.
- Docker service available on the runner.
- Install pinned `k3d` version during the job.
- Install pinned CNPG operator version.
- Build the local `cnpg-watch` binary and make it available to the e2e scripts.

### Local e2e tests

Provide a single script, for example:

```sh
scripts/e2e-local-k3d.sh
```

The script should:

- create a named k3d cluster with a local registry when needed;
- install the CNPG operator;
- create a namespace for tests;
- keep the CNPG operator in a separate namespace from the test database namespace by default;
- create at least two CNPG clusters in different namespaces for fleet tests;
- deploy a small CNPG cluster with one primary and at least one replica;
- optionally deploy a CNPG Pooler;
- create deterministic CNPG Backup/ScheduledBackup, DatabaseRole, Database, Publication, and Subscription fixtures when the installed CNPG version supports them;
- run `cnpg-watch doctor`;
- run `cnpg-watch doctor --operator-namespace <operator-ns>`;
- run setup dry-runs and, where safe, setup apply commands;
- run the dashboard through the PTY-driven TUI smoke harness;
- delete or restart a replica pod and verify watch updates;
- inject SQL probe failure or saturation and verify stale/backoff state;
- clean up the k3d cluster unless `E2E_KEEP_CLUSTER=1`.

### Pipeline e2e tests

Add a GitHub Actions job that runs after unit and integration tests:

```text
check -> unit -> integration -> e2e-k3d
```

The pipeline e2e job should:

- run only on Linux GitHub-hosted runners initially;
- install a pinned k3d release;
- create an isolated cluster name per workflow run;
- install a pinned CNPG operator release;
- deploy the minimal CNPG test fixture;
- keep the operator namespace distinct from the database namespace;
- deploy at least two CNPG test clusters across namespaces for fleet assertions;
- run the same e2e assertions used by `scripts/e2e-local-k3d.sh`;
- run the PTY-driven TUI smoke suite against the live fixture;
- always collect logs and events on failure;
- always delete the k3d cluster in a cleanup step.

Introduce the small transport/live-slice CI job in Phase 1 and expand it into
this suite. Run live query contracts on each supported PostgreSQL major and
exec/RBAC contracts on the pinned Kubernetes compatibility combinations. Keep
release-build benchmarks and bounded-load checks in a separate validation job
with recorded hardware/workload metadata. Run longer property cases and the
30 min soak on the scheduled pipeline; do not make every fast unit-test run
wait for a soak or full compatibility matrix.

Keep the e2e job separate from the fast checks so day-to-day unit failures are not hidden behind cluster setup time. Run e2e on pull requests that touch collectors, permissions, probes, UI state, workflow files, scripts, or manifests. Also run it nightly on the default branch.

### Manual smoke tests

- Run against a namespace with no CNPG clusters.
- Run against a healthy single-primary/two-replica cluster.
- Run with the CNPG operator in a different namespace and verify operator status/log discovery.
- Delete a replica pod and verify watch updates, stale SQL state, and events.
- Break archive configuration and verify archiver failure display.
- Create or patch backup and managed-object fixtures into pending/failing states and verify the related views.
- Patch a service selector or endpoint fixture in a controlled namespace and verify service endpoint mismatch display.
- Open `:logs` for cluster, primary, replica, pooler, and event scopes; verify source labels, filtering, wrapping, follow/pause, and redaction.
- Switch between `minimal`, `standard`, `incident`, and `secure` profiles and verify feature availability, redaction, and reduced-mode output.
- Validate `:health`, `:hints`, `:connections`, and `:waits` during a controlled connection-pressure scenario.
- Saturate database connections or force probe timeouts and verify adaptive backoff plus stale sample display.
- Validate `setup postgres-role` with admin credentials and then run watch mode with the generated monitoring role.
- Validate PgBouncer `SHOW POOLS` / `SHOW STATS` through a CNPG Pooler.
- Run with RBAC that lacks `pods/exec` and verify reduced mode.

## Research notes

Primary references for lifecycle, transport, and validation contracts added in
the design review:

- [kube-runtime watch initialization and deletion semantics](https://docs.rs/kube-runtime/latest/kube_runtime/watcher/enum.Event.html).
- [Kube exec attachment, output, status, and cleanup](https://docs.rs/kube/latest/kube/api/struct.AttachedProcess.html),
  [WebSocket exec request construction](https://docs.rs/kube-core/latest/src/kube_core/subresource.rs.html),
  and [Kubernetes WebSocket create authorization](https://kubernetes.io/docs/reference/command-line-tools-reference/feature-gates/).
- [CNPG PostgreSQL/HBA configuration](https://cloudnative-pg.io/docs/1.30/postgresql_conf/)
  and [PostgreSQL peer authentication](https://www.postgresql.org/docs/18/auth-peer.html).
- [PostgreSQL recovery information functions](https://www.postgresql.org/docs/18/functions-admin.html#FUNCTIONS-RECOVERY-INFO)
  and [statistics semantics](https://www.postgresql.org/docs/18/monitoring-stats.html).
- [Tokio bounded mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html),
  [latest-value watch channels](https://docs.rs/tokio/latest/tokio/sync/watch/index.html),
  [missed ticks](https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html),
  [graceful shutdown](https://tokio.rs/tokio/topics/shutdown), and
  [paused-time testing](https://tokio.rs/tokio/topics/testing).
- [Rust trait dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility),
  [Proptest](https://proptest-rs.github.io/proptest/proptest/index.html), and
  [Criterion](https://bheisler.github.io/criterion.rs/book/).

Pin implementation dependencies and adapt their exact API names in code. The
contracts above describe required behavior; links to latest crate docs do not
expand the declared PostgreSQL/CNPG/Kubernetes support range. Performance
numbers in this design are initial targets pending the first measured baseline.

Sources reviewed for CNPG-specific design:

- CloudNativePG 1.30 documentation overview and feature list.
- CloudNativePG 1.30 API reference for `Cluster`, `Pooler`, `Backup`, `ScheduledBackup`, `Database`, `DatabaseRole`, `Publication`, `Subscription`, `FailoverQuorum`, `ImageCatalog`, `ClusterImageCatalog`, and plugin status fields.
- CloudNativePG 1.30 backup/recovery, pooler, managed roles/databases, declarative hibernation, fencing, replica cluster, and monitoring documentation sections linked from the main docs.

CNPG observations incorporated above:

- CNPG represents a PostgreSQL cluster as a `Cluster` custom resource with one primary and optional replicas, rather than a generic StatefulSet abstraction.
- CNPG directly manages PVCs, including data, WAL, and tablespace storage, so storage state is operationally important.
- CNPG includes Pooler resources backed by PgBouncer, making pooler status and PgBouncer stats first-class dashboard data.
- CNPG exposes backup and scheduled-backup resources, including multiple backup methods such as Barman/object-store style backup, volume snapshots, and CNPG-I plugin integration.
- CNPG exposes declarative managed-object resources for databases, roles, publications, and subscriptions.
- CNPG status includes plugin capability information that can explain backup, WAL, or restore degradation.
- CNPG supports advanced replication patterns such as replica clusters, delayed replicas, synchronous replication, and HA replication slots, which should shape the replica topology view.

Sources reviewed for the k9s comparison:

- k9s v0.51.0 release notes and `change_logs/release_v0.51.0.md`.
- k9s README at the v0.51.0 source tag.
- k9s command, RBAC, skins, and plugins documentation.
- k9s v0.51.0 source packages: `internal/view`, `internal/watch`, `internal/model`, `internal/model1`, `internal/render`, `internal/client`, `internal/dao`, and `go.mod`.
- Historical k9s changelogs for v0.1.1, v0.7.0, v0.17.0, v0.30.0, v0.40.0, and v0.50.0.

Important concrete observations:

- k9s v0.51.0 uses `github.com/derailed/tview`, `github.com/derailed/tcell/v2`, Kubernetes `client-go`, `kubectl`, `metrics`, Helm, `gojq`, `grype`, and `syft`.
- k9s `watch.Factory` owns dynamic shared informer factories keyed by namespace and performs access checks before returning informers.
- k9s table models reconcile informer-backed data into table data, compute row deltas, and notify listeners.
- k9s has a central registry that maps GVRs to DAO, renderer, and topology renderer behavior.
- k9s v0.51.0 specifically fixed or improved readonly behavior in XRay, owner jumps, sidecar init container pod status, namespace-aware RBAC checks, syncing-state UX, unchanged-data reconcile skipping, client QPS, metrics pagination, worker batching, wide columns, context-switch API calls, and style reset on context switch.

Design implications already incorporated above:

- model cache sync state explicitly;
- test namespace and cluster-scoped RBAC separately;
- cap and expose API QPS/burst;
- batch hydration/probe work;
- skip unchanged reducer/render cycles;
- keep command/filter/help/history, but scope commands to CNPG/Postgres;
- include support-oriented `info` and screen/model dumps from the start;
- postpone broad plugin/custom-view systems until the core dashboard stabilizes.

## Open implementation questions

- Which explicit monitoring credential source/delivery mechanism meets the
  first live slice's authentication and cleanup contract? Resolve this in
  Phase 1, recording the selected transport and measured timeout behavior.
- Which pinned Kubernetes/kube combinations require get, create, or both for
  actual WebSocket exec, and do generated RBAC and doctor match those results?
- What fleet scale meets the initial latency, CPU, and RSS targets on recorded
  macOS/Linux baselines, and which cache/staging or queue budgets need tuning?
- Which CNPG labels are stable enough to use for pod and pooler ownership matching across supported versions?
- Which CNPG resource views should be enabled by default when the matching CRDs are present but the operator installation is older than the fixtures?
- What operator namespace discovery strategy is reliable enough across Helm, OLM, plain manifests, and renamed deployments?
- Which operator pod labels should be treated as default selectors, and when should `--operator-selector` be required?
- How should ambiguity be reported if multiple CNPG operator installs are discovered in one Kubernetes API server despite the normal single-operator model?
- Which controller-runtime metric names should be treated as stable enough for optional operator metrics?
- Which webhook names/selectors are stable enough to identify CNPG admission health across install methods?
- Which Backup/ScheduledBackup status fields are stable enough to drive "last success" and "next schedule" without version-specific false positives?
- Which certificate secrets can be discovered reliably without broad secret access?
- Should PgBouncer connection counts come from admin SQL, metrics endpoints, or both?
- What default lag thresholds should drive yellow/red UI states?
- Which PostgreSQL 18 predefined-role grants are sufficient for the monitor,
  monitor-reserved, and action role profiles?
- Which PostgreSQL versions before 18 should be added after MVP, and what query
  registry fallbacks are needed for them?
- Which CNPG Pooler authentication configurations can be safely updated by `setup pooler-role`?
- How much service/endpoint mismatch detection should depend on CNPG labels versus owner references?
- Should `cnpg-watch` ever expose a plugin system, or should integrations stay built-in and Postgres-specific?
- Which screen/model dump fields can be safely included by default without leaking secrets?
- Should custom columns be limited to named layout presets instead of user-authored expressions?

Resolve each question with fixtures or measurements before implementing the
feature that depends on it. Authentication and cleanup gate the first live SQL
slice; broad operational views, discovery variants, and future integrations
do not block the scaffold or the narrow Cluster/Pod path. Record decisions in
the relevant ADR instead of expanding all future interfaces in advance.


## Implemented fleet and probe decisions

Phases 3 and 4 use the existing single-owner reducer and exec transport. See
[ADR 0013](docs/adr/0013-fleet-evidence-and-selected-streams.md),
[ADR 0014](docs/adr/0014-capability-gated-probe-classes.md), and the
[fleet/diagnostics command guide](docs/fleet-and-probes.md) for the concrete
interfaces and observed resource shapes. PgBouncer admin statistics, correlated
operator timelines, broader operational collectors, and release qualification
remain phases 5–7. PostgreSQL 18.4 with CNPG 1.30.0 is the live-qualified probe
combination; registry major-version metadata does not expand that claim.

## Qualified phase 5/6 implementation details

The pinned CNPG 1.30.0 / PgBouncer 1.25.1 fixture uses Unix peer console identity
`pgbouncer` at `/controller/run`, port 5432. That identity has console privileges;
the fixed runtime probe sends SHOW VERSION/POOLS/STATS only. Statistics are
explicitly enabled and setup remains dry-run/diff guidance with no apply mode.
A PostgreSQL role alone cannot establish stats-only peer-console authentication.
See [ADR 0009](docs/adr/0009-optional-operational-evidence.md).

Operational resources use served CNPG API discovery, current-generation status
and UID-aware association. Cluster-scoped resources, exact named certificate GETs
and isolated metrics scrapes are independently opt-in. Canonical redacted logs
and immutable Kubernetes event records share bounded retention; typed source
context and bounded indexes drive correlation. The [operational guide](docs/operational.md)
documents implemented commands, evidence limits and reduced modes. The phase 5/6
validation receipts describe local qualification; Phase 7 still owns broader
release and compatibility qualification.
