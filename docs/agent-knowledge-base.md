# Agent Knowledge Base: CloudNativePG and PostgreSQL

This document is for agents working on `cnpg-watch`. Read it before changing
the design or implementing collectors, probes, RBAC, setup flows, or TUI views.

The purpose is not to duplicate the CloudNativePG or PostgreSQL manuals. It is
to capture the operational facts that matter for this tool.

For PostgreSQL internals, SQL probe design, source-code maps, monitoring RBAC,
wait events, replication views, and load-aware query rules, also read
[`postgres-agent-knowledge-base.md`](postgres-agent-knowledge-base.md).

## Review Baseline

Reviewed on 2026-07-03.

Primary sources:

- CloudNativePG latest stable release checked through GitHub API:
  `v1.30.0`, published 2026-06-29.
- CloudNativePG source reviewed from `cloudnative-pg/cloudnative-pg` `main`,
  shallow clone commit `66039df3fd3f5e6329cebbd21691adc93c923320`.
- CloudNativePG docs in that source tree under `docs/src/`, including API
  reference, architecture, controller, instance manager, backup, service
  management, connection pooling, monitoring, logging, operator configuration,
  labels/annotations, release notes, and failure/failover docs.
- PostgreSQL current documentation is PostgreSQL 18 at review time.

Important version note:

- The `main` docs mention preview version `1.31.0-rc1`, while GitHub latest
  stable is `v1.30.0`. Treat `main` as current development knowledge and
  `v1.30.0` as the latest stable baseline unless the project pins otherwise.
- Field names must still be verified against the installed CRDs in live
  clusters. CNPG fields evolve.

Source links:

- CloudNativePG docs: https://cloudnative-pg.io/docs/
- CloudNativePG GitHub: https://github.com/cloudnative-pg/cloudnative-pg
- CloudNativePG API reference in docs: https://cloudnative-pg.io/docs/1.30/cloudnative-pg.v1/
- PostgreSQL docs: https://www.postgresql.org/docs/current/

## CNPG Mental Model

CloudNativePG is a Kubernetes operator for vanilla PostgreSQL. The core custom
resource is `Cluster`, representing one PostgreSQL primary plus optional
replicas in one Kubernetes namespace.

Key facts for `cnpg-watch`:

- CNPG is PostgreSQL-only and Kubernetes-only.
- CNPG uses a custom pod controller, not StatefulSets.
- CNPG directly manages pods, PVCs, services, secrets, config maps, jobs, PDBs,
  pooler deployments, and related objects.
- CNPG uses PostgreSQL native physical replication and WAL behavior rather than
  Patroni, repmgr, Stolon, or another external HA manager.
- The Kubernetes API and `Cluster.status` are first-class operational sources.
- Runtime PostgreSQL views are still necessary for LSN, replication, connection
  pressure, waits, locks, archive state, and query/session diagnostics.

## Operator Scope

The operator normally installs in namespace `cnpg-system` as a deployment named
`cnpg-controller-manager`, but agents must not hard-code that as the only shape.

Useful defaults and caveats:

- Operator pod selector commonly includes `app.kubernetes.io/name=cloudnative-pg`.
- The manager container exposes metrics on port `8080` named `metrics`.
- The webhook port is `9443`.
- The deployment uses `--leader-elect`.
- `OPERATOR_NAMESPACE` is populated from the pod namespace.
- Operator behavior can be configured by a ConfigMap/Secret named
  `cnpg-controller-manager-config` in the operator namespace.
- `WATCH_NAMESPACE` can restrict the namespaces the operator watches. It can be
  a comma-separated list. If unset, the operator watches all namespaces.
- CNPG CRDs are cluster-wide. The normal model for one Kubernetes API server is
  one CNPG CRD set, one CNPG operator installation, and many CNPG `Cluster`
  resources across namespaces.
- CNPG docs warn that running multiple operators in one Kubernetes cluster is
  not fully supported as a simple general case because operators share CRDs.
  Multiple namespaced operators can be attempted with
  `ENABLE_WEBHOOK_NAMESPACE_SUFFIX` and `WATCH_NAMESPACE`, but backwards
  compatible upgrades are not guaranteed. Treat this as exceptional evidence,
  not the common design path.
- A `vcluster` should be treated as a separate Kubernetes API boundary,
  normally reached through a separate kube context, not as another operator
  namespace inside the same cluster.

For `cnpg-watch`:

- Model database namespace scope and operator namespace scope separately.
- Support zero or one configured operator namespace for normal operation.
- If more than one plausible operator install is discovered, show an ambiguity
  state instead of assuming a many-operator topology.
- Association between a cluster and an operator must be evidence based:
  `explicit`, `likely`, `weak`, or `unknown`.
- If operator namespace access is unreadable, the dashboard should still
  observe clusters.
- Operator status should enrich `:operator`, `:fleet`, `:health`, `:hints`,
  `:logs`, `:plugins`, and `doctor`; it must not be a startup dependency.

## CNPG Resource Map

Watch these CNPG resources when supported by the installed CRDs and profile:

| Resource | Why `cnpg-watch` cares |
| --- | --- |
| `Cluster` | primary, replicas, phase, conditions, PVC status, service role, image, plugins, backup summaries |
| `Pooler` | PgBouncer desired/ready state, phase, type, image, config, pooler services |
| `Backup` | active/failed/succeeded backups, method, source instance, WAL range, errors |
| `ScheduledBackup` | schedule, next run, last scheduled time, validation errors |
| `Database` | declarative DB reconciliation state and managed schema/extension/FDW/server status |
| `DatabaseRole` | standalone declarative role reconciliation and client certificate status |
| `DatabaseRoleList` | Kubernetes list kind for `DatabaseRole`; not a separate CRD endpoint, but relevant when modeling list/watch payloads |
| `Publication` | logical replication publication desired/reconciled state |
| `Subscription` | logical replication subscription desired/reconciled state |
| `FailoverQuorum` | quorum-based failover/readiness context when present |
| `ImageCatalog` | namespace-scoped image catalog for Postgres/PgBouncer image posture |
| `ClusterImageCatalog` | cluster-scoped image catalog for image posture |

The API group is `postgresql.cnpg.io/v1`.

## Kubernetes Object Map

Watch related Kubernetes resources with CNPG labels and owner/reference
evidence:

- `Pod`: PostgreSQL instances, PgBouncer poolers, backup/import/join jobs, and
  operator pods.
- `PersistentVolumeClaim`: PGDATA, WAL, and tablespace volumes.
- `Service`: `-rw`, `-ro`, `-r`, optional `-any`, additional managed services,
  pooler services, webhook services.
- `EndpointSlice`: service endpoint readiness and role/primary mismatches.
- `Event`: cluster, pod, pooler, backup, managed-object, service, and operator
  events.
- `Lease`: per-cluster primary lease and operator leader-election leases when
  accessible.
- `Deployment`/`ReplicaSet`: operator and pooler posture where relevant.
- `ValidatingWebhookConfiguration` and `MutatingWebhookConfiguration`: CNPG
  admission health.
- `Secret`: optional and redacted; needed for certificate expiration and some
  setup validations.
- `ConfigMap`: operator config and monitoring query config where readable.

Do not build a generic Kubernetes browser. These objects are supporting
evidence for PostgreSQL/CNPG operation.

## Labels and Annotations

Prefer stable CNPG labels over name parsing. Important labels:

- `cnpg.io/cluster`: owning cluster name.
- `cnpg.io/instanceName`: PostgreSQL instance name.
- `cnpg.io/instanceRole`: `primary`, `replica`, or transient `unhealthy`.
- `cnpg.io/podRole`: distinguishes database instance pods from pooler pods.
- `cnpg.io/poolerName`: PgBouncer pooler name.
- `cnpg.io/pvcRole`: PVC purpose such as `PG_DATA`, `PG_WAL`, or tablespace.
- `cnpg.io/scheduled-backup`: `ScheduledBackup` that created a `Backup`.
- `app.kubernetes.io/managed-by`: usually `cloudnative-pg`.
- `app.kubernetes.io/name`: usually `postgresql` for cluster-generated DB
  resources and `cloudnative-pg` for the operator.
- `app.kubernetes.io/component`: `database`, `pooler`, etc.
- `app.kubernetes.io/instance`: owning cluster name.
- `app.kubernetes.io/version`: PostgreSQL major version on some generated
  resources.

Important annotations:

- `cnpg.io/operatorVersion`: operator version that created/reconciled a
  resource.
- `cnpg.io/pvcStatus`: PVC status such as `initializing`, `ready`, `detached`.
- `cnpg.io/fencedInstances`: JSON list of fenced instances or `*`.
- `cnpg.io/hibernation`: `on` or `off`.
- `cnpg.io/clusterManifest`: hibernated cluster manifest snapshot.
- `cnpg.io/pgControldata`: hibernation/control-data information.
- `cnpg.io/passwordPassthrough`: opt-out for operator-side SCRAM encoding.
- `cnpg.io/reconcilePodSpec`: can disable reconcile-triggered restarts.

Treat missing labels as degraded evidence, not proof that an object is
unrelated. Older versions and custom installs can vary.

## Cluster Status Fields To Normalize

From `api/v1/cluster_types.go`, important `Cluster.status` fields include:

- `instances`, `readyInstances`, `selector`.
- `instancesStatus`, `instancesReportedState`.
- `managedRolesStatus`.
- `tablespacesStatus`.
- `podSelectorRefs`.
- `timelineID`.
- `topology`.
- `currentPrimary`, `targetPrimary`.
- `lastPromotionToken`, `demotionToken`, `systemID`.
- `pvcCount`, `jobCount`.
- `danglingPVC`, `resizingPVC`, `initializingPVC`, `healthyPVC`,
  `unusablePVC`.
- `writeService`, `readService`.
- `phase`, `phaseReason`.
- `secretsResourceVersion`, `configMapResourceVersion`.
- `certificates`.
- `firstRecoverabilityPoint`, `firstRecoverabilityPointByMethod`.
- `lastSuccessfulBackup`, `lastSuccessfulBackupByMethod`.
- `lastFailedBackup`.
- `cloudNativePGCommitHash`, `cloudNativePGOperatorHash`.
- `currentPrimaryTimestamp`, `currentPrimaryFailingSinceTimestamp`,
  `targetPrimaryTimestamp`.
- `poolerIntegrations`.
- `operatorCertificateFingerprint`.
- `availableArchitectures`.
- `conditions`.
- `instanceNames`.
- `onlineUpdateEnabled`.
- `image`, `pgDataImageInfo`, `targetPgDataImageInfo`.
- `pluginStatus`.
- `switchReplicaClusterStatus`.

Do not expose raw JSON paths in UI code. Put field access behind CNPG adapters.

## Plugin Status

`Cluster.status.pluginStatus` reports plugin evidence:

- `name`
- `version`
- generic `capabilities`
- `operatorCapabilities`
- `walCapabilities`
- `backupCapabilities`
- `restoreJobHookCapabilities`
- plugin-provided `status`

This is critical for CNPG-I backup/WAL/restore behavior. Missing plugin
capability can explain WAL archival, backup, or restore failures while
PostgreSQL itself is healthy.

## Pooler / PgBouncer

CNPG Poolers are independent CRs. They are not automatically created for every
cluster.

Important facts:

- `Pooler.spec.cluster` is immutable and references a `Cluster` in the same
  namespace.
- Pooler type can be `rw`, `ro`, or `r`.
- Pooler lifecycle is independent from Cluster lifecycle. Deleting one does not
  delete the other.
- Pooler phase enum in current API: `active`, `paused`, `inactive`, `failed`.
- PgBouncer pool modes include `session` and `transaction`.
- CNPG requires PgBouncer `auth_dbname`; docs say use PgBouncer image 1.19+.
- Default PgBouncer integration creates a PostgreSQL role named
  `cnpg_pooler_pgbouncer`, a `public.user_search(text)` function, grants
  connect/execute permissions, issues a TLS certificate, and configures
  `auth_user`/`auth_query`.
- Pooler pods can have TLS metrics enabled.
- Admin/stats access to PgBouncer is environment-dependent. `cnpg-watch` must
  degrade to readiness/status if `SHOW POOLS`/`SHOW STATS` are unavailable.

For `cnpg-watch`, keep direct PostgreSQL health and Pooler health independent.
A bad pooler can break applications while the DB is healthy.

## Backup And Recovery

CNPG supports physical backups, not logical `pg_dump` backups, for business
continuity.

Important facts:

- Since 1.26, native backup/recovery is being phased out of core in favor of
  CNPG-I plugins.
- The latest stable 1.30 release notes say native Barman Cloud removal is now
  planned for 1.31.0; users are encouraged to migrate to the Barman Cloud
  Plugin.
- Supported backup methods in API: `barmanObjectStore`, `volumeSnapshot`,
  `plugin`.
- Object-store backups require WAL archiving and support PITR.
- Volume snapshots do not require WAL archiving, though WAL archiving is still
  strongly recommended for production and PITR.
- `ScheduledBackup.spec.schedule` uses a six-field cron expression including
  seconds. This is not traditional five-field Unix cron.
- `Backup.spec.cluster` and `ScheduledBackup.spec.cluster` are immutable in
  current release notes.

Important `Backup.status` fields:

- `phase`, `error`, `commandError`.
- `startedAt`, `stoppedAt`.
- `reconciliationStartedAt`, `reconciliationTerminatedAt`.
- `beginWal`, `endWal`, `beginLSN`, `endLSN`.
- `backupId`, `backupName`, `destinationPath`, `serverName`.
- `majorVersion`.
- `instanceID`.
- volume snapshot status.

Important `ScheduledBackup.status` fields:

- `lastCheckTime`
- `lastScheduleTime`
- `nextScheduleTime`
- `error`

For `cnpg-watch`, backup health must combine CNPG resources, `Cluster.status`
backup summary fields, plugin status, WAL archive SQL, and logs.

## Failover, Switchover, And Primary Safety

CNPG failover is operator/instance-manager driven:

- If the primary is unhealthy longer than `.spec.failoverDelay`, failover starts.
- First, `targetPrimary` can become `pending`; the failing primary is shut down
  so WAL receivers on replicas stop.
- Then a new primary is elected and promoted.
- Fast shutdown tries to archive pending WALs; immediate shutdown can lose data.
- `.spec.switchoverDelay` affects RPO/RTO tradeoffs.
- CNPG uses a Kubernetes `Lease` named after the `Cluster` in the cluster
  namespace for safe primary election.
- The primary lease prevents premature promotion; it does not fence a running
  isolated primary.
- The primary isolation check is separate and enabled by default.

For `cnpg-watch`:

- Watch the per-cluster `Lease` when permitted.
- Display `currentPrimary`, `targetPrimary`, timestamps, phase, phase reason,
  lease holder, and lease age together.
- Detect service endpoint mismatch against `currentPrimary`.
- Never initiate failover/switchover in the first release.

## Instance Manager And Probes

Every CNPG PostgreSQL pod starts the instance manager as PID 1. It manages the
PostgreSQL postmaster and backs Kubernetes startup, liveness, and readiness
probes.

Probe facts:

- Startup probe defaults to `pg_isready`, with customizable strategies:
  `pg_isready`, `query`, or `streaming`.
- Replica startup can enforce maximum lag with `maximumLag`.
- Liveness verifies the instance manager and includes primary isolation
  behavior.
- Readiness controls whether the instance should receive service traffic.
- Custom probe settings can cause restarts or misleading readiness if too
  aggressive.

For `cnpg-watch`:

- Pod readiness is not the same as PostgreSQL health, but it is essential
  service-routing evidence.
- Show instance-manager and PostgreSQL log sources distinctly.
- Under load, preserve cached SQL samples while Kubernetes watches continue.

## Storage

CNPG manages PVCs directly instead of relying on StatefulSets.

Important storage facts:

- CNPG can resize PVCs when the storage class supports it.
- PGDATA and WAL can use separate PVCs.
- Tablespaces can have their own PVCs.
- PGDATA and WAL PVCs for one instance must remain coherent. Deleting only the
  WAL PVC can make the remaining PGDATA PVC unusable from PostgreSQL's point of
  view.
- `Cluster.status` reports healthy, initializing, resizing, dangling, and
  unusable PVC lists.
- `cnpg.io/pvcRole` labels identify PGDATA/WAL/tablespace roles.

For `cnpg-watch`, storage state is a first-class cluster signal. A pending pod
can be caused by storage, scheduling, or PVC coherence problems.

## Services And Connectivity

CNPG creates standard Kubernetes services for each `Cluster`:

- `<cluster>-rw`: read/write service to the primary. This is essential and
  cannot be disabled.
- `<cluster>-ro`: read-only service to replicas, if enabled and replicas exist.
- `<cluster>-r`: read service to any ready instance.
- Optional `-any` service can be enabled by operator config.
- Additional managed services can be declared in the `Cluster` spec.

For `cnpg-watch`:

- Watch Services and EndpointSlices.
- Compare `-rw` endpoints to `currentPrimary`.
- Show stale/missing endpoints after failover.
- Include pooler services separately from direct Postgres services.

## Logs

CNPG emits logs as JSON to stdout. It does not persist logs to storage.

Top-level fields include:

- `level`
- `ts`
- `logger`
- `msg`
- `record`
- `logging_pod`

PostgreSQL logs use `logger=postgres` and usually `msg=record`; PostgreSQL CSV
log fields are parsed into `record`, including:

- `log_time`
- `user_name`
- `database_name`
- `process_id`
- `connection_from`
- `session_id`
- `command_tag`
- `error_severity`
- `sql_state_code`
- `message`
- `detail`
- `hint`
- `query`
- `application_name`
- `backend_type`

Other `logger` values include:

- `barman-cloud-wal-archive`
- `barman-cloud-wal-restore`
- `initdb`
- `pg_basebackup`
- `pg_controldata`
- `pg_ctl`
- `pg_rewind`
- `pgaudit`
- `postgres`
- `wal-archive`
- `wal-restore`
- `instance-manager`

For `cnpg-watch`:

- Parse JSON logs but preserve raw lines if parsing fails.
- Redact secrets, connection strings, query text, tokens, passwords, and
  secret-looking values before UI, dumps, or artifacts.
- Correlate operator logs by namespace/name, reconcile key, event, and time
  window. Do not dump all operator logs into cluster scope by default.
- Use Kubernetes `pods/log`, not `kubectl logs`.
- Log streams must be lazy, bounded, and closed on scope changes.

## Monitoring And Metrics

Per PostgreSQL instance:

- CNPG exposes a Prometheus-compatible exporter on port `9187`, named
  `metrics`.
- Monitoring queries run as `cnpg_metrics_exporter`, a member of `pg_monitor`,
  over pod-local Unix sockets.
- Default monitoring query output is cached for 30 seconds.
- Default metrics include WAL file counts/size, archive status, sync replica
  expected/observed values, node distribution, last backup timestamps,
  recoverability point, replica mode, manual switchover required, and fencing.
- Automatic `PodMonitor` creation is deprecated; manual `PodMonitor` management
  is recommended.

Operator:

- The operator deployment exposes port `8080` named `metrics`.
- Optional operator metrics can enrich `:operator` and `:fleet`, but do not
  require Prometheus as a dependency.

For `cnpg-watch`:

- Direct SQL/Kubernetes watches are the primary data path.
- Metrics are optional enrichment and should be profile-gated.
- Do not assume Prometheus exists.

## Declarative Objects

CNPG declarative management includes:

- Inline managed roles in `Cluster.spec.managed.roles`, surfaced through
  `Cluster.status.managedRolesStatus`.
- Standalone `DatabaseRole` CRs with `status.observedGeneration`,
  `status.applied`, `status.message`, optional client certificate status, and
  conditions.
- `Database` CRs with `status.observedGeneration`, `status.applied`,
  `status.message`, and object status for schemas, extensions, FDWs, and
  servers.
- `Publication` and `Subscription` CRs for logical replication.

Managed role status values from the cluster API:

- `reconciled`
- `not-managed`
- `pending-reconciliation`
- `reserved`

For `cnpg-watch`:

- Treat `observedGeneration < metadata.generation` as reconciliation lag.
- Treat `applied=false` or non-empty error/message as a visible health reason.
- Do not mutate managed objects in watch mode.

## Image And Version Posture

Current docs say:

- Default Postgres operand image in current docs is
  `ghcr.io/cloudnative-pg/postgresql:18.4-system-trixie`.
- `system` Postgres images are deprecated and tied to in-core Barman Cloud
  support.
- Image catalogs are supported through `ImageCatalog` and
  `ClusterImageCatalog`.
- Pooler image catalogs are supported in current release notes; `Pooler.status`
  reports resolved image and phase.

For `cnpg-watch`:

- Surface operator version, CRD version, cluster image, PG major, image catalog
  reference, deprecated image flavor warnings, and mismatches.

## Webhook Health

CNPG normally uses admission webhooks for defaulting and validation. Current
docs say reconciliation performs fallback defaulting/validation if webhooks are
unavailable or configured to ignore failures, surfacing validation errors in
resource status and logging full validation detail in operator logs.

For `cnpg-watch`:

- Watch validating/mutating webhook configs when permitted.
- Watch webhook service and EndpointSlices.
- Check backing operator pods.
- Check webhook cert expiration only when secret access is allowed.
- Show reduced mode when webhook resources are forbidden.

## RBAC Guidance

Runtime observation should use the narrowest permissions that support the
selected profile. Setup can run with admin credentials, but watch mode must be
read-only except for `pods/exec` and `pods/log` subresources.

Core CNPG resources:

- `get`, `list`, `watch` on CNPG resources and their `/status` subresources
  where available.

Core Kubernetes resources:

- `get`, `list`, `watch` on pods, services, EndpointSlices, PVCs, events.
- `get`, `list`, `watch` on leases for primary lease and operator leader
  evidence.
- `create` on `pods/exec` for SQL/PgBouncer probes.
- `get` on `pods/log` for log views.
- Optional `get`, `list`, `watch` on secrets for cert expiration and generated
  credential validation.
- Operator namespace: deployments, replicasets, pods, events, leases, services,
  EndpointSlices, webhook configs, optional pod logs, optional secrets.

Reduced mode rule:

- Missing optional RBAC degrades only the relevant feature view.
- Missing `pods/exec` disables SQL probes but not Kubernetes topology.
- Missing `pods/log` disables log timelines but not topology/probes.
- Missing operator namespace access disables operator evidence only.

## PostgreSQL Monitoring Knowledge

Useful PostgreSQL views/functions:

- `pg_stat_activity`: sessions, state, wait events, query start, xact start,
  backend type, query text.
- `pg_stat_replication`: WAL sender/replica streaming state from primary.
- `pg_stat_wal_receiver`: WAL receiver state on standby.
- `pg_stat_archiver`: archived/failed WAL counts and last WAL/times.
- `pg_replication_slots`: slot type, active state, restart LSN,
  confirmed flush LSN, WAL status, failover/synced columns in current versions.
- `pg_stat_subscription` and `pg_stat_subscription_stats`: logical
  subscription workers and errors.
- `pg_locks`: global lock table. Can be joined to `pg_stat_activity`, but lock
  joins are easy to get wrong and can have performance impact if queried too
  frequently.
- `pg_blocking_pids(pid)`: preferred way to identify blockers for waiting
  sessions.
- WAL/LSN functions: `pg_current_wal_lsn()`, `pg_current_wal_insert_lsn()`,
  `pg_wal_lsn_diff()`, `pg_last_wal_receive_lsn()`,
  `pg_last_wal_replay_lsn()`, `pg_last_xact_replay_timestamp()`,
  `pg_is_in_recovery()`.
- `current_setting(...)` for `max_connections`,
  `superuser_reserved_connections`, `reserved_connections` when available.

Privileges:

- `pg_monitor` includes `pg_read_all_settings`, `pg_read_all_stats`, and
  `pg_stat_scan_tables`.
- `pg_read_all_stats` enables visibility into normally restricted statistics.
- Avoid granting broad file/server-program roles such as `pg_read_server_files`,
  `pg_write_server_files`, or `pg_execute_server_program`.
- Avoid `pg_signal_backend` in the first release; `cnpg-watch` should not
  cancel or terminate sessions.

Safety:

- Always set `application_name='cnpg-watch'`.
- Use short `connect_timeout`, `statement_timeout`, and `lock_timeout`.
- Avoid explicit transactions for stats probes, or ensure one query per
  connection invocation.
- Aggregate in SQL; do not pull full session lists by default.
- Query text is sensitive. Default to query hash or redacted/truncated text.
- Under load, connection pressure probes are useful, but deeper wait/lock
  diagnostics must back off before core topology/LSN probes.
- PostgreSQL stats views can lag and are cached within a transaction. Design
  probes as point-in-time evidence, not exact truth.

Connections:

- `max_connections` controls concurrent backends and affects resource sizing.
- `reserved_connections` and `superuser_reserved_connections` protect slots for
  privileged/emergency access.
- Standby `max_connections` must be same or higher than primary.

## Suggested Probe Priority

When a database is under pressure, prefer:

1. Kubernetes watches and cached CNPG status.
2. Pod readiness, service endpoints, events, and leases.
3. Lightweight primary/replica LSN probes.
4. WAL archiver and replication status.
5. Pooler readiness and PgBouncer stats.
6. Connection pressure aggregates.
7. Wait summaries.
8. Lock/blocker diagnostics.
9. Query text, only when explicitly enabled.

## Agent Rules For `cnpg-watch`

- Never assume one CNPG cluster per Kubernetes cluster.
- Never assume the operator is in the same namespace as database clusters.
- Treat one operator installation per Kubernetes API server as the normal CNPG
  model, but do not hardcode it. If discovery finds multiple plausible
  operators, report ambiguity and cite evidence.
- Never assume Prometheus exists.
- Never assume direct local TCP access to PostgreSQL.
- Prefer Kubernetes API watches and pod-local exec probes over shelling out to
  `kubectl`.
- Treat field access as versioned adapter code.
- Keep normal watch mode read-only.
- Do not implement failover/switchover, session termination, query cancel,
  backup trigger, role/database mutation, or Kubernetes mutations in the TUI
  unless the design explicitly moves them out of non-goals.
- Preserve stale last-good samples under load.
- Redact before rendering dumps or logs.
- Make reduced-mode reasons feature-specific and actionable.

## Source Files Worth Checking When Updating This KB

CloudNativePG source:

- `api/v1/cluster_types.go`
- `api/v1/pooler_types.go`
- `api/v1/backup_types.go`
- `api/v1/scheduledbackup_types.go`
- `api/v1/database_types.go`
- `api/v1/databaserole_types.go`
- `api/v1/publication_types.go`
- `api/v1/subscription_types.go`
- `api/v1/failoverquorum_types.go`
- `api/v1/imagecatalog_types.go`
- `api/v1/clusterimagecatalog_types.go`
- `pkg/utils/labels_annotations.go`
- `config/manager/manager.yaml`
- `config/rbac/`
- `config/prometheus/monitor.yaml`
- `docs/src/index.md`
- `docs/src/cloudnative-pg.v1.md`
- `docs/src/controller.md`
- `docs/src/operator_conf.md`
- `docs/src/instance_manager.md`
- `docs/src/failover.md`
- `docs/src/backup.md`
- `docs/src/connection_pooling.md`
- `docs/src/service_management.md`
- `docs/src/monitoring.md`
- `docs/src/logging.md`
- `docs/src/labels_annotations.md`
- `docs/src/release_notes/`

PostgreSQL docs:

- Monitoring statistics:
  https://www.postgresql.org/docs/current/monitoring-stats.html
- `pg_stat_activity`:
  https://www.postgresql.org/docs/current/monitoring-stats.html#MONITORING-PG-STAT-ACTIVITY-VIEW
- `pg_locks`:
  https://www.postgresql.org/docs/current/view-pg-locks.html
- Replication slots:
  https://www.postgresql.org/docs/current/view-pg-replication-slots.html
- Continuous archiving/PITR:
  https://www.postgresql.org/docs/current/continuous-archiving.html
- Streaming replication:
  https://www.postgresql.org/docs/current/warm-standby.html
- Predefined roles:
  https://www.postgresql.org/docs/current/predefined-roles.html
- Connection settings:
  https://www.postgresql.org/docs/current/runtime-config-connection.html


## Observed Phase 3/4 fixture behavior (2026-11-15)

The pinned CNPG 1.30.0 fixture has two database namespaces plus cnpg-system.
Pooler pods carry `cnpg.io/poolerName`; they must not become SQL instance targets.
Pooler status exposes instances/phase without readyInstances. Treat missing CR
readiness as unknown and display pod readiness separately. Pooler image overrides
belong in `spec.template.spec.containers[].image`, not `spec.imageName`.
EndpointSlice endpoints can be null. Read-write endpoint checks use target UIDs
and readiness, allow five seconds of convergence, and never treat missing
permission or unsynchronized sources as an asserted outage. Checked-in trimmed
[captures](../fixtures/cnpg/1.30.0/fleet/README.md) exercise these exact shapes.

Cluster-owner UIDs fence label associations after recreation. Label-only evidence
remains explicitly weaker; conflicting labels/owners are not silently reassigned.
Scope replacement must fence both old results and asynchronously prepared clients.
Selected log streams have the same UID/scope plus generation protection.
