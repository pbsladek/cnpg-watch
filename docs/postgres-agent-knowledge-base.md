# Agent Knowledge Base: PostgreSQL Source And Operations

This document is for agents working on `cnpg-watch`. Read it before changing
SQL probes, connection behavior, Postgres RBAC setup, log parsing, replication
views, wait diagnostics, or load-shedding behavior.

The CloudNativePG knowledge base covers Kubernetes and CNPG resource behavior.
This file covers PostgreSQL itself and the source-code surfaces that explain
what the tool should query, trust, and avoid.

## Review Baseline

Reviewed on 2026-07-03.

Primary sources:

- PostgreSQL current documentation: PostgreSQL 18.4.
- PostgreSQL official Git mirror stable source:
  `REL_18_STABLE`, commit `12c519207db01a9d4a6b47b205883928a083758f`.
- PostgreSQL official Git mirror development source:
  `master`, commit `e0ff7fd9aa2e6f77c38825e71200ced742220d55`.
- Stable source `configure.ac` identifies the reviewed stable tree as
  PostgreSQL `18.4`.
- Development source `configure.ac` identifies `master` as `20devel`.

Important version note:

- Treat PostgreSQL 18.4 docs and `REL_18_STABLE` source as the current stable
  baseline.
- Treat `master` source as forward-looking implementation detail only. Do not
  assume a `master` column or wait event exists in a live cluster.
- Probe code must discover server version with
  `current_setting('server_version_num')::int` or `SHOW server_version_num`
  before selecting version-specific columns.

Source links:

- PostgreSQL docs: https://www.postgresql.org/docs/current/
- Source repository docs: https://www.postgresql.org/docs/current/sourcerepo.html
- PostgreSQL GitHub mirror: https://github.com/postgres/postgres
- Monitoring docs: https://www.postgresql.org/docs/current/monitoring-stats.html
- Predefined roles: https://www.postgresql.org/docs/current/predefined-roles.html
- Lock view docs: https://www.postgresql.org/docs/current/view-pg-locks.html
- Replication slots: https://www.postgresql.org/docs/current/view-pg-replication-slots.html
- WAL/PITR docs: https://www.postgresql.org/docs/current/continuous-archiving.html
- Logging docs: https://www.postgresql.org/docs/current/runtime-config-logging.html

## Mental Model

PostgreSQL is a multi-process database server. The postmaster accepts frontend
connections, forks child backends, starts auxiliary processes, and coordinates
startup, shutdown, and crash recovery.

For `cnpg-watch`:

- A Postgres pod is not one process. It contains a postmaster, client backends,
  WAL sender/receiver processes, autovacuum processes, checkpointer, WAL writer,
  background writer, startup process during recovery, logical replication
  workers, and other version-dependent workers.
- `pg_stat_activity` and related views expose process state through shared
  memory, not by inspecting OS process lists.
- Many important health signals are cluster-wide, not database-local. A probe
  connected to one database can see many cluster-wide views, but relation names
  and OID joins can be database-specific.
- A read-only monitoring role can observe most of what this tool needs, but not
  everything. The UI must show reduced capability rather than require
  superuser.

## Source Tree Map

Important PostgreSQL source paths for this tool:

| Path | Why `cnpg-watch` cares |
| --- | --- |
| `src/backend/postmaster/postmaster.c` | postmaster lifecycle, child process handling, reserved connection behavior |
| `src/include/miscadmin.h` | `BackendType` enum and helper macros |
| `src/include/postmaster/proctypelist.h` | human-readable backend process type descriptions |
| `src/backend/catalog/system_views.sql` | SQL definitions for views such as `pg_stat_activity`, `pg_stat_replication`, `pg_replication_slots`, `pg_stat_wal`, `pg_stat_io` |
| `src/include/catalog/pg_proc.dat` | built-in functions, volatility, arguments, and default ACLs |
| `src/include/catalog/pg_authid.dat` | predefined roles such as `pg_monitor`, `pg_read_all_stats`, `pg_use_reserved_connections` |
| `src/include/catalog/pg_auth_members.dat` | built-in membership, especially `pg_monitor` membership |
| `src/backend/utils/activity/backend_status.c` | shared-memory backend status reporting used by `pg_stat_activity` |
| `src/backend/utils/activity/wait_event_names.txt` | canonical wait event names and descriptions |
| `src/backend/utils/adt/pgstatfuncs.c` | implementation of stats-returning SQL functions |
| `src/backend/utils/adt/lockfuncs.c` | `pg_locks` and `pg_blocking_pids()` behavior |
| `src/backend/access/transam/xlogfuncs.c` | WAL LSN and recovery helper functions |
| `src/backend/replication/walsender.c` | WAL sender and replication stats behavior |
| `src/backend/replication/walreceiver.c` | WAL receiver stats and connection fields |
| `src/backend/utils/init/postinit.c` | connection slot and replication permission checks |
| `src/backend/utils/misc/guc.c` and `src/backend/utils/misc/guc_tables.c` | configuration visibility and setting behavior |
| `src/backend/utils/error/elog.c` | server logging fields and backend type in logs |
| `src/bin/psql` and `src/interfaces/libpq` | client behavior if implementing direct SQL outside pod exec |
| `src/test/recovery` and `src/test/subscription` | executable examples of replication and recovery behavior |

When adding or updating probes, inspect `system_views.sql` first. It is the
least ambiguous source of column names and joins for built-in views.

## Process Types

`BackendType` in `src/include/miscadmin.h` includes:

- `B_BACKEND`: normal client backend.
- `B_WAL_SENDER`: physical or logical WAL sender.
- `B_WAL_RECEIVER`: standby WAL receiver.
- `B_STARTUP`: startup/recovery process.
- `B_AUTOVAC_LAUNCHER` and `B_AUTOVAC_WORKER`.
- `B_BG_WRITER`, `B_CHECKPOINTER`, `B_WAL_WRITER`.
- `B_BG_WORKER`: extension or internal background worker.
- `B_SLOTSYNC_WORKER`, `B_WAL_SUMMARIZER`, `B_IO_WORKER`, and other
  version-dependent workers.
- `B_LOGGER`: logger process; it is not connected to shared memory and does not
  have a `PGPROC` entry.

For `cnpg-watch`:

- Display `backend_type` from `pg_stat_activity`, not inferred process names.
- Expect background process names and wait events to vary across versions.
- Avoid alerting just because a background process is idle. Many Activity waits
  mean "no work right now".
- On a replica, `startup`, `walreceiver`, and recovery waits are expected.
- On a primary, `walsender` rows are expected when replicas or backups stream.

## Connection Strategy Under Load

The tool must be useful when the database is stressed.

PostgreSQL connection facts:

- `max_connections` caps normal backend slots.
- `superuser_reserved_connections` keeps final slots for superusers.
- `reserved_connections` keeps additional slots for roles with
  `pg_use_reserved_connections`.
- `pg_use_reserved_connections` does not bypass `max_connections`; it only lets
  a role use reserved slots before the final superuser-only slots.
- WAL sender connections require the `REPLICATION` role attribute.

For `cnpg-watch`:

- The setup flow may create a monitor role with `pg_monitor` and optionally
  `pg_use_reserved_connections`.
- Do not make `pg_use_reserved_connections` mandatory. Some operators may not
  want monitoring to consume reserved slots.
- Prefer one short-lived SQL probe session per target at low frequency unless a
  persistent connection has a clear benefit.
- Set `application_name` to identify the tool and cluster view, for example
  `cnpg-watch/<component>`.
- Set conservative session limits:
  `SET statement_timeout`, `SET lock_timeout`, `SET idle_in_transaction_session_timeout`.
- Use autocommit. Do not hold long transactions while reading stats views.
- If connection attempts fail with "too many connections", keep Kubernetes and
  CNPG API observation alive and mark SQL as degraded.
- Back off aggressively on authentication errors, connection refusal, startup
  recovery errors, or repeated timeout.
- Never run blocking diagnostics by default. A slow probe is worse than a
  missing probe during an incident.

Suggested defaults:

- `connect_timeout`: 2-5 seconds.
- `statement_timeout`: 1-3 seconds for dashboard probes.
- `lock_timeout`: 250-500 ms.
- Probe jitter: required across clusters to avoid synchronized load.
- Minimum probe interval under stress: increase automatically when timeouts,
  pool saturation, or high wait pressure is detected.

## Monitoring RBAC

Useful predefined roles from source and docs:

| Role | Use for this tool |
| --- | --- |
| `pg_monitor` | preferred broad monitoring role; member of `pg_read_all_settings`, `pg_read_all_stats`, and `pg_stat_scan_tables` |
| `pg_read_all_stats` | read all `pg_stat_*` views and statistics normally hidden from ordinary users |
| `pg_read_all_settings` | read settings normally hidden from ordinary users |
| `pg_stat_scan_tables` | execute monitoring functions that may scan tables or take `ACCESS SHARE` locks |
| `pg_use_reserved_connections` | optional incident-mode access to reserved connection slots |
| `pg_signal_backend` | optional explicit action capability for cancel/terminate; should not be in the default watch role |
| `pg_checkpoint` | action capability; not needed for normal watch mode |
| `pg_read_server_files`, `pg_write_server_files`, `pg_execute_server_program` | dangerous; do not grant to this tool |

Implementation correction (2026-11-15 UTC): CNPG 1.30.0's managed-role
validation rejects names with the reserved `cnpg_` prefix. Use `watch_monitor`
and `watch_actions` for this project's defaults. This was verified in the
isolated live fixture; PostgreSQL's own role naming rules are separate.

Recommended monitoring role shape:

```sql
CREATE ROLE watch_monitor LOGIN;
GRANT pg_monitor TO watch_monitor;
-- Optional only when the operator chooses incident reserved-slot access:
GRANT pg_use_reserved_connections TO watch_monitor;
```

Optional action role shape:

```sql
CREATE ROLE watch_actions NOLOGIN;
GRANT pg_signal_backend TO watch_actions;
```

For `cnpg-watch`:

- Separate observation and action roles.
- Do not require superuser for normal operation.
- Do not grant file or program roles.
- Do not grant `pg_signal_backend` unless the design explicitly includes query
  cancel/session terminate actions with confirmation and audit.
- Use `pg_has_role(current_user, 'pg_monitor', 'member')` and related checks to
  show capability in `doctor`.

## Statistics Snapshot Semantics

PostgreSQL's cumulative statistics are not instant telemetry.

Important behavior:

- Many cumulative stats are flushed periodically, not on every event.
- Current activity from `track_activities` is more immediate.
- Statistics accessed inside a transaction can be cached until the transaction
  ends, depending on `stats_fetch_consistency`.
- `pg_stat_clear_snapshot()` can discard the current session's stats snapshot.
- A clean shutdown can preserve stats; crash recovery, immediate shutdown,
  base backup start, and PITR can reset counters.

For `cnpg-watch`:

- Run each stats scrape in autocommit or explicitly call
  `pg_stat_clear_snapshot()` before a multi-query scrape.
- Treat counter resets as normal after failover, pod restart, crash, PITR, or
  immediate shutdown.
- Use `stats_reset` columns when computing rates.
- Do not compare raw cumulative counters across pod identity changes without
  reset detection.
- Label values as "sampled" or "since reset" in internal models.

## Core Views To Probe

Use version-gated SQL. Query `to_regclass()` or server version before assuming
newer views/columns.

| View/function | Use |
| --- | --- |
| `pg_stat_activity` | sessions, state, waits, transactions, query age, backend type |
| `pg_stat_database` | database-level transactions, temp files, deadlocks, checksum failures, session deaths |
| `pg_stat_database_conflicts` | standby query conflicts due to recovery |
| `pg_stat_replication` | primary-side physical replication sender state and LSNs |
| `pg_stat_wal_receiver` | standby-side receiver state, source, slot, last message timing |
| `pg_replication_slots` | slot type, activity, retained WAL, invalidation, failover/sync fields where available |
| `pg_stat_replication_slots` | logical slot spill/stream counters |
| `pg_stat_subscription` and `pg_stat_subscription_stats` | logical subscription worker state and errors |
| `pg_stat_archiver` | archive success/failure counts and last WAL names |
| `pg_stat_wal` | WAL records, full-page images, WAL bytes, buffer pressure |
| `pg_stat_bgwriter` and `pg_stat_checkpointer` | checkpoint and buffer write behavior |
| `pg_stat_io` | backend/object/context I/O, available in modern versions |
| `pg_stat_all_tables` / `pg_stat_user_tables` | vacuum/analyze health, dead tuples, HOT updates |
| `pg_stat_all_indexes` / `pg_statio_*` | index use and relation I/O |
| `pg_locks` | lock graph inputs |
| `pg_blocking_pids(pid)` | preferred blocker discovery |
| `pg_prepared_xacts` | prepared transactions that can hold locks |
| `pg_settings` | effective config, pending restart, source |
| `pg_file_settings` / `pg_show_all_file_settings()` | config file parse/apply diagnostics when permitted |
| `pg_hba_file_rules` | HBA parse diagnostics when permitted |
| `pg_is_in_recovery()` | primary/standby role from SQL perspective |
| `pg_current_wal_lsn()` | primary current WAL write LSN |
| `pg_last_wal_receive_lsn()` | standby last received WAL LSN |
| `pg_last_wal_replay_lsn()` | standby last replayed WAL LSN |
| `pg_last_xact_replay_timestamp()` | standby replay freshness, with caveats |
| `pg_wal_lsn_diff(a,b)` | byte lag between LSNs |

Do not query expensive catalog joins every refresh. Split probes into fast,
medium, and deep profiles.

## `pg_stat_activity`

Stable fields include:

- identity: `datid`, `datname`, `pid`, `leader_pid`, `usesysid`, `usename`.
- connection: `application_name`, `client_addr`, `client_hostname`,
  `client_port`, `backend_start`.
- timing: `xact_start`, `query_start`, `state_change`.
- waits: `wait_event_type`, `wait_event`.
- state: `state`, `backend_xid`, `backend_xmin`.
- query: `query_id`, `query`.
- process: `backend_type`.

State values from source include:

- `starting`
- `idle`
- `active`
- `idle in transaction`
- `fastpath function call`
- `idle in transaction (aborted)`
- `disabled`

For `cnpg-watch`:

- Redact or truncate `query` by default. Provide raw query only with an
  explicit safe mode.
- Long `idle in transaction` with an old `backend_xmin` is a vacuum risk.
- Long `active` with wait events is a contention clue, not automatically a bad
  query.
- `leader_pid` matters for parallel query. Group workers under leaders in UI.
- `backend_type` distinguishes client sessions from background workers.
- Without `pg_read_all_stats`, many columns for other users can be null. Show a
  capability warning rather than treating null as healthy.

## Wait Events

Wait event names are generated from
`src/backend/utils/activity/wait_event_names.txt`.

Stable wait categories include:

- `Activity`: a process is waiting because it has no work. Often normal for
  background workers.
- `Client`: waiting on client I/O or replication peer I/O.
- `IPC`: waiting for another process.
- `Timeout`: sleeping until a timeout expires.
- `IO`: waiting on storage or filesystem I/O.
- `Lock`: heavyweight lock waits.
- `LWLock`: lightweight internal lock waits.
- `BufferPin`: waiting for another backend to release a buffer pin.
- `Extension`: extension-defined waits.

For `cnpg-watch`:

- Do not create generic severity from category alone.
- Treat `Lock`, `BufferPin`, repeated `IO`, `WAL`, and recovery conflict waits
  as higher diagnostic value.
- Treat `Activity` waits as normal unless the process type says it should be
  doing work.
- Show a short explanation for common waits, but keep raw event names visible.
- Unknown wait names are normal across versions and extensions.

## Locks And Blockers

`pg_locks` is a global view of held and awaited locks. It is useful but easy to
misread.

Important facts:

- `pg_locks` has one row per active lockable object, lock mode, and process.
- `granted=false` means the process is waiting.
- A backend waits for at most one heavyweight lock at a time.
- Row-level locks usually do not appear directly; waiters often show waits on
  transaction IDs.
- `pg_locks` is cluster-wide, but joining `relation` to `pg_class` only works
  correctly for relations in the current database.
- The docs and source recommend `pg_blocking_pids()` over hand-written
  self-joins for blockers.
- Querying `pg_locks` briefly locks lock-manager data structures and can have
  impact if run too often.

For `cnpg-watch`:

- Use `pg_blocking_pids(pid)` to build blocker trees.
- Sample lock detail on demand or at a lower rate than session counts.
- Avoid resolving every relation name across every database in a tight loop.
- Include prepared transactions from `pg_prepared_xacts` in blocker analysis.
- De-duplicate blocker PIDs; parallel query can produce duplicates.

## WAL, Replication, And Recovery

Core concepts:

- WAL is the source of crash recovery, physical replication, PITR, archiving,
  and timelines.
- PostgreSQL normally stores WAL under `pg_wal/`.
- Default WAL segment size is 16 MB unless changed at `initdb`.
- `wal_level` must be `replica` or higher for archiving and physical
  replication, and `logical` for logical decoding.
- A timeline changes after recovery/promotion. LSN comparisons across timelines
  need care.

Primary-side probes:

- `pg_is_in_recovery()` should be `false`.
- `pg_current_wal_lsn()` gives current WAL write location.
- `pg_stat_replication` shows WAL sender state and sent/write/flush/replay LSNs.
- `pg_replication_slots` shows retained WAL risk via `restart_lsn`,
  `wal_status`, and `safe_wal_size` where available.

Standby-side probes:

- `pg_is_in_recovery()` should be `true`.
- `pg_stat_wal_receiver` shows receiver status, source, slot, message times, and
  conninfo.
- `pg_last_wal_receive_lsn()` and `pg_last_wal_replay_lsn()` show receive/replay
  progress.
- `pg_last_xact_replay_timestamp()` can estimate replay staleness but is null
  before replaying a commit and can be quiet on idle systems.

For `cnpg-watch`:

- Cross-check SQL role with CNPG role labels and `Cluster.status`.
- Show separate receive lag and replay lag when possible.
- Do not compare LSNs from unrelated timelines as simple byte lag.
- Treat a stalled archiver, inactive slot, or growing `restart_lsn` distance as
  storage-risk evidence.
- On CNPG clusters, prefer CNPG-provided backup/WAL plugin status for backup
  truth, and use PostgreSQL WAL views as runtime evidence.

## Replication Slots

`pg_replication_slots` reports both physical and logical slots.

Important fields:

- `slot_name`, `slot_type`, `plugin`, `database`.
- `active`, `active_pid`.
- `restart_lsn`: oldest WAL still needed by the slot.
- `confirmed_flush_lsn`: logical slot consumption progress.
- `wal_status` and `safe_wal_size`: retention pressure indicators where
  available.
- `inactive_since`, `conflicting`, `invalidation_reason`.
- `failover`, `synced`: relevant to failover slot behavior in modern versions.

Version caution:

- Columns differ across major versions. For example, development `master` has
  fields not present in 18.4 stable source.

For `cnpg-watch`:

- Discover columns before selecting optional slot fields.
- Group slots by cluster instance and database.
- Highlight inactive slots retaining WAL.
- Do not drop, advance, or create slots from watch mode.

## Checkpoints, WAL Pressure, And I/O

Useful views:

- `pg_stat_wal`: WAL records, full-page images, WAL bytes, WAL buffers full.
- `pg_stat_checkpointer`: timed/requested checkpoints, restartpoints, write and
  sync time, buffers written.
- `pg_stat_bgwriter`: clean buffers and allocations.
- `pg_stat_io`: backend type/object/context I/O counters and timings.

For `cnpg-watch`:

- Compute rates from deltas and respect `stats_reset`.
- High requested checkpoints or WAL buffers full can indicate checkpoint/WAL
  pressure.
- High temp file growth from `pg_stat_database` can indicate bad plans, memory
  pressure, or insufficient `work_mem`.
- I/O timing values depend on tracking settings and may be zero if disabled.
- Use `pg_stat_io` when available, but gate by version/view existence.

## Vacuum, Bloat, And Wraparound Signals

Important views and settings:

- `pg_stat_user_tables`: dead tuples, vacuum/analyze counts and timestamps,
  autovacuum/analyze counts.
- `pg_stat_progress_vacuum`: active vacuum phase and progress.
- `pg_stat_progress_analyze`: analyze progress.
- `pg_class.relfrozenxid`, `pg_class.relminmxid`, `pg_database.datfrozenxid`.
- Settings such as `autovacuum`, `autovacuum_freeze_max_age`,
  `vacuum_failsafe_age`, and cost delay/limit settings.

For `cnpg-watch`:

- A long-running transaction with old `backend_xmin` can prevent cleanup.
- Standby conflicts can cancel queries during recovery; use
  `pg_stat_database_conflicts`.
- Do not run relation-wide bloat estimates by default. They can be expensive.
- Deep table health belongs behind an explicit profile or on-demand view.
- Show approximate risk categories rather than exact bloat claims unless the
  probe uses a vetted estimate.

## Configuration Surfaces

Useful SQL surfaces:

- `pg_settings`: effective setting, source, context, unit, pending restart.
- `pg_file_settings` or `pg_show_all_file_settings()`: config-file parse and
  apply diagnostics.
- `pg_hba_file_rules`: parsed HBA rules and errors.
- `SHOW` commands for specific values.

For `cnpg-watch`:

- Prefer `pg_settings` over parsing files.
- Highlight `pending_restart=true`.
- Treat `source` as important: default, configuration file, command line,
  environment, database, user, or session.
- Do not edit Postgres config from watch mode.
- In CNPG, desired config is owned by CNPG resources; report drift as evidence,
  not as something to fix directly.

Settings worth normalizing:

- Connections: `max_connections`, `reserved_connections`,
  `superuser_reserved_connections`.
- Activity: `track_activities`, `track_activity_query_size`,
  `compute_query_id`.
- Stats/timing: `track_counts`, `track_io_timing`, `track_wal_io_timing`,
  `stats_fetch_consistency`.
- WAL/replication: `wal_level`, `max_wal_senders`, `max_replication_slots`,
  `hot_standby`, `hot_standby_feedback`, `synchronous_commit`,
  `synchronous_standby_names`.
- Archive/recovery: `archive_mode`, `archive_command`, `archive_library`,
  `archive_timeout`, `restore_command`.
- Vacuum: `autovacuum`, freeze and multixact thresholds.
- Logging: `log_min_duration_statement`, `log_lock_waits`,
  `deadlock_timeout`, `log_line_prefix`, `log_destination`.

## Logging

PostgreSQL logging can go to stderr, csvlog, jsonlog, syslog, eventlog, or
other destinations depending on config and environment. CNPG captures server
logs from container stdout and commonly presents them as structured JSON.

For source-level behavior:

- `elog.c` is the central server error/logging implementation.
- Logs can include backend type, SQLSTATE, severity, detail, hint, statement,
  context, file/line/function, database, user, application, and remote host
  depending on settings and destination.

For `cnpg-watch`:

- Prefer Kubernetes pod logs for CNPG-managed pods.
- Parse JSON when present, but preserve raw log lines.
- Redact SQL, parameters, user data, credentials, and connection strings.
- Correlate logs with SQL state using PID, backend type, pod, timeline, and
  timestamps.
- Build CNPG-tailored views: primary/replica logs, WAL/backup logs, failover
  timeline, pooler auth errors, operator messages, and PostgreSQL errors.
- Do not assume a single pod log is enough. A useful view needs all instances,
  role changes, and time-window correlation.

## Backup And PITR

PostgreSQL physical backup depends on base backups plus WAL continuity.

Important facts:

- `pg_dump`/`pg_dumpall` are logical backups and are not sufficient for
  continuous archiving/PITR.
- WAL archive failure can fill `pg_wal/`; if the filesystem fills, PostgreSQL
  can panic and remain offline until space is freed.
- `archive_command` or `archive_library` must succeed only when the WAL segment
  is safely archived.
- `archive_timeout` can force segment switches but should not be too short.
- `pg_backup_start` and `pg_backup_stop` are low-level APIs and require careful
  ordering.

For `cnpg-watch`:

- Use CNPG `Backup` and plugin status as the orchestrated backup truth.
- Use `pg_stat_archiver`, WAL views, logs, and disk pressure as runtime
  evidence.
- Show archive lag/failure separately from backup object phase.
- Do not trigger backup APIs unless a future action design explicitly allows
  it.

## Logical Replication

Useful surfaces:

- `pg_publication`, `pg_subscription`, `pg_subscription_rel`.
- `pg_stat_subscription`.
- `pg_stat_subscription_stats`.
- logical rows in `pg_replication_slots`.

For `cnpg-watch`:

- CNPG has declarative `Publication` and `Subscription` resources. Correlate
  them with PostgreSQL runtime views.
- Logical slots can retain WAL. Treat inactive logical slots as storage risk.
- Subscription workers may appear as background workers in `pg_stat_activity`.
- Initial table sync has its own worker states and can look different from
  steady apply.

## Query Design Rules

Every SQL probe must be safe under load.

Rules:

- Use explicit column lists.
- Gate version-specific columns.
- Avoid `SELECT *` in shipped probes.
- Keep probes read-only.
- Set timeouts per session.
- Avoid long transactions.
- Avoid relation-wide scans unless the view name clearly says deep inspection.
- Prefer one aggregate query over N per relation/session when possible.
- Use `pg_stat_clear_snapshot()` before collecting a multi-query stats bundle
  in one session.
- Use `application_name`.
- Include server version and pod identity in probe results.

Avoid by default:

- `pg_database_size()` across many databases at high frequency.
- `pg_relation_size()` across many relations at high frequency.
- Bloat estimation queries that scan catalogs or relations deeply.
- `pgstattuple`, `pgrowlocks`, and extension functions that scan table data.
- `EXPLAIN ANALYZE`.
- `pg_cancel_backend`, `pg_terminate_backend`, `pg_reload_conf`,
  `pg_promote`, `pg_switch_wal`, `CHECKPOINT`, `VACUUM`, `ANALYZE`, or DDL.

## Capability Detection

At startup and per cluster, detect:

- `server_version_num`.
- `current_setting('server_version')`.
- `pg_is_in_recovery()`.
- monitoring role membership:
  `pg_has_role(current_user, 'pg_monitor', 'member')`.
- optional role membership:
  `pg_has_role(current_user, 'pg_use_reserved_connections', 'member')`.
- readable views through `to_regclass('pg_catalog.<view>')`.
- optional columns through `information_schema.columns` or `pg_attribute`.
- key settings through `pg_settings`.

Capability state should drive UI affordances and probe selection. Missing
permission or missing columns are not failures by themselves.

## CNPG-Specific Interpretation

When PostgreSQL evidence conflicts with Kubernetes/CNPG evidence, show both and
mark the confidence.

Examples:

- CNPG label says a pod is primary, but `pg_is_in_recovery()` returns true:
  possible stale label, role transition, probe targeting mistake, or split
  evidence during failover.
- `Cluster.status.currentPrimary` changed, but SQL still sees old role:
  failover is in progress or probe is stale.
- Standby has WAL receiver but no progress: inspect network, source, slot,
  archive/restore, and operator logs.
- Primary has inactive physical slot retaining WAL: inspect replica pod,
  `Cluster.status.instancesStatus`, PVC free space, and CNPG events.
- PgBouncer is healthy but SQL probes fail: distinguish pooler access from
  direct pod SQL access.

For the first implementation, prefer pod-local SQL probes through Kubernetes
exec when direct TCP access is uncertain. Direct TCP can be added when service,
secret, network, TLS, and pooler behavior are modeled.

## Suggested Probe Profiles

Fast profile:

- `SELECT current_setting('server_version_num')::int`
- `SELECT pg_is_in_recovery()`
- primary: `pg_current_wal_lsn()`
- standby: `pg_last_wal_receive_lsn()`, `pg_last_wal_replay_lsn()`
- bounded `pg_stat_activity` aggregate by state, wait type, backend type.
- connection/settings basics.

Medium profile:

- `pg_stat_database`
- `pg_stat_replication`
- `pg_stat_wal_receiver`
- `pg_replication_slots`
- `pg_stat_archiver`
- `pg_stat_wal`
- `pg_stat_checkpointer`
- selected `pg_settings`

Deep/on-demand profile:

- lock blocker tree with `pg_locks` and `pg_blocking_pids()`.
- table vacuum posture for top-risk relations.
- `pg_stat_io`.
- logical replication detail.
- progress views for vacuum, create index, base backup, copy.
- config file parse views.

Incident/degraded profile:

- Stop deep probes.
- Keep Kubernetes/CNPG watches active.
- Try one low-frequency SQL probe with tight timeouts.
- Prefer reserved connection role only if configured.
- Surface "SQL degraded" explicitly, with last successful sample time.

## Agent Rules

- Do not assume a PostgreSQL major version.
- Do not assume a query column exists because it exists in current docs.
- Do not require superuser for watch mode.
- Do not mutate PostgreSQL from watch mode.
- Do not hold transactions open for stats scrapes.
- Do not hide permission degradation.
- Do not treat missing stats as healthy.
- Do not treat a single pod's SQL result as cluster truth.
- Do not compare LSNs without role and timeline context.
- Do not parse source code when a stable SQL view gives the same answer.
- Do use source code when a view's semantics, cost, privileges, or version
  behavior are unclear.

## Source Files Worth Checking When Updating This KB

Stable source:

- `/tmp/postgres-18-src/configure.ac`
- `/tmp/postgres-18-src/src/backend/catalog/system_views.sql`
- `/tmp/postgres-18-src/src/include/catalog/pg_proc.dat`
- `/tmp/postgres-18-src/src/include/catalog/pg_authid.dat`
- `/tmp/postgres-18-src/src/include/catalog/pg_auth_members.dat`
- `/tmp/postgres-18-src/src/include/miscadmin.h`
- `/tmp/postgres-18-src/src/include/postmaster/proctypelist.h`
- `/tmp/postgres-18-src/src/backend/postmaster/postmaster.c`
- `/tmp/postgres-18-src/src/backend/utils/activity/backend_status.c`
- `/tmp/postgres-18-src/src/backend/utils/activity/wait_event_names.txt`
- `/tmp/postgres-18-src/src/backend/utils/adt/pgstatfuncs.c`
- `/tmp/postgres-18-src/src/backend/utils/adt/lockfuncs.c`
- `/tmp/postgres-18-src/src/backend/access/transam/xlogfuncs.c`
- `/tmp/postgres-18-src/src/backend/replication/walsender.c`
- `/tmp/postgres-18-src/src/backend/replication/walreceiver.c`
- `/tmp/postgres-18-src/src/backend/utils/init/postinit.c`

Development source:

- `/tmp/postgres-src/configure.ac`
- `/tmp/postgres-src/src/backend/catalog/system_views.sql`
- `/tmp/postgres-src/src/backend/utils/activity/wait_event_names.txt`

The `/tmp` source paths are local review artifacts, not repository
dependencies. Refresh them before updating this document.


## Observed Phase 4 probe behavior (2026-11-15)

Live qualification remains PostgreSQL 18.4 with CNPG 1.30.0. All six registered
variants run through production exec as watch_monitor on a primary and replica.
The capability bundle checks actual view columns, SELECT access, and EXECUTE
access to monitored functions. Registry version ranges alone do not establish
point-release compatibility. Capability results carry system/timeline/role
identity and require current topology before dependent admission.

Startup FATAL errors about remaining/reserved connection slots can omit SQLSTATE;
classify the observed reserved-slot text as overload. Hold actual connections
before asserting shedding in tests. A quiet replica's pressure must not erase a
busy primary's pressure. Keep only accepted fresh selected-pod results in this
policy. Promotion invalidates topology: capability recapture must not compare its
role against a fast sample from the previous topology epoch indefinitely.

Pin search_path to pg_catalog. Keep statement/deadline/output bounds across every
variant. Separate EXPLAIN database execution/planning, exec-only baseline, full
bundle round trip, observed sweep duration, and cancellation cleanup in evidence.
The test observer may perform isolated admin fault injection; runtime remains
read-only and never inherits that observer identity.
