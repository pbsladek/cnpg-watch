# Poolers and operational evidence

These views use accepted Kubernetes observations and bounded samples. A failed
optional source leaves other evidence available; `:doctor` shows which source
is disabled, denied, absent, restarting, or stale.

| Command | Evidence |
| --- | --- |
| `:poolers` | Desired/ready instances, UID-owned pods, type, mode, image and optional PgBouncer pressure |
| `:connections` | PostgreSQL pressure and separately identified PgBouncer pools/statistics |
| `:backups` | Backup attempts, suspended schedules, controller timestamps, method and reported recovery context |
| `:storage`, `:services` | PVC state/role, selectors, endpoint identities and primary routing convergence |
| `:roles`, `:databases` | Managed object application and desired/observed generations |
| `:publications`, `:subscriptions` | Publication and Subscription reconciliation; both use the subscriptions view |
| `:replicas` | Compatible SQL replication/slot evidence plus delayed/external/synchronous configuration and optional FailoverQuorum |
| `:plugins` | Configured/reported CNPG-I capabilities, images, operator versions and catalogs |
| `:operator` | Deployment generation/readiness, scoped services/endpoints and optional selected metrics |
| `:certs` | Public certificate metadata for explicitly named Secrets |

A current-generation application failure differs from a pending generation.
Older failed backups stop raising a current-failure finding when a later success
is reported. Missing ordering timestamps remain uncertain. Backup success,
archiver counters and recovery-point timestamps do not prove a usable restore or
PITR window. Intentional apply delay suppresses the ordinary replica lag alarm;
byte lag and compatibility checks remain visible. Unknown plugin capabilities or
quorum status do not establish failure or safe promotion.

## PgBouncer statistics

```sh
cnpg-watch --namespace app-db --cluster payments --pooler-stats
cnpg-watch --namespace app-db setup pooler-role --pooler payments-pool --dry-run
cnpg-watch --namespace app-db setup pooler-role --pooler payments-pool --diff
```

Statistics are off by default. In the qualified CNPG 1.30.0 / PgBouncer 1.25.1
layout, the `pgbouncer` container runs the `pgbouncer` OS identity and exposes a
peer-authenticated console at `/controller/run`, port 5432. The probe uses that
identity and executes only fixed `SHOW VERSION`, `SHOW POOLS`, and `SHOW STATS`
commands. It needs `pods/exec`, an installed `psql`, and the current
Pooler → Deployment → ReplicaSet → Pod UID chain. It does not use the PostgreSQL
monitor password. `--pooler-socket` explicitly selects another absolute socket
directory; unsupported layouts remain reduced mode.

The stock console identity has administrative console privileges. Runtime sends
only the three read-only commands above. Creating a PostgreSQL role cannot turn
this peer console into a stats-only login. Accordingly, `setup pooler-role`
provides deterministic review/diff guidance and **has no apply mode**. A custom
layout should prefer a dedicated `stats_users` identity, after validating its
CNPG-managed configuration and authentication. Custom TCP/password layouts are
not implemented or live-qualified.

Statistics run only while Poolers or Connections is open, with a 750 ms deadline,
a five-second requested interval plus jitter, independent failure/backoff state,
and the shared two-exec process limit. Output is capped at 256 KiB and 128 rows
per table. Missing columns are unknown or unsupported, rather than zero; counters
and microseconds retain their units. The parser accepts the known major-1 console
shapes from 1.19–1.25, but live qualification covers 1.25.1 only. Pooler readiness
remains useful when statistics are disabled or denied.

## Explicit optional sources

```sh
cnpg-watch --namespace app-db --cluster-resources
cnpg-watch --namespace app-db --certificate-secret app-db/payments-server
cnpg-watch --namespace app-db --operator-namespace cnpg-system \
  --operator-metrics-url http://127.0.0.1:9090/metrics
```

`--cluster-resources` adds ClusterImageCatalog and CNPG-relevant validating and
mutating webhook configuration watches. It does not grant permissions. Operator
service and EndpointSlice watches use the configured operator namespace;
unobserved scope or a filtered database watch cannot establish that a webhook
service is missing. A route warning needs persistent synchronized endpoint
facts. Endpoint evidence does not verify TLS or the admission application.

Repeat `--certificate-secret namespace/name` for up to eight names. The loader
performs only named GET requests: no Secret list/watch or wildcard discovery.
The RBAC generator emits namespaced, `resourceNames`-restricted get grants for
those exact names. Minimal profile disables these reads. Only `tls.crt` or
`ca.crt` is parsed, with a 64 KiB certificate limit and at most 16 certificates.
All other Secret fields are discarded before parsing. Stored metadata contains
UID, validity times, chain count and SHA256 fingerprint; it contains no subject,
PEM or private key. Refresh is requested every 60 seconds. Expiry findings use
the current clock and fresh referenced samples; no TLS trust verification is
claimed. Rotation replaces the sampled public metadata.

Metrics require an explicit HTTP(S) URL and operator watches. Minimal profile
and `--no-operator-watch` disable collection. Scrapes use an isolated HTTP client:
no Kubernetes credentials, proxy inheritance or redirects. The two-second
request limit, 256 KiB body cap, 4,096 lines and finite nonnegative values bound
the input. Five selected controller/workqueue metrics are summed across labels;
label values and unknown metrics are discarded. Refresh is requested every 30
seconds. Cumulative error counters are not current-outage alarms, and failed
scrapes cannot change PostgreSQL availability.

## Correlated timelines

```text
:logscope selected
:logscope cluster
:logscope backup BACKUP_NAME
:logscope operator
:logscope unmatched
:logsource instances|poolers|backups|operator|events on|off
```

Logs require incident profile. Cluster and backup scopes combine current sources
and canonical Kubernetes event records. Full operator and unmatched operator
records require their explicit scopes; operator namespace labels stay visible.
`l` on a backup selects its timeline. Leaving Logs closes streams.

Correlation uses current resource UIDs, structured namespace/name context, or a
hashed reconcile ID with a 30-second window. A namespace/name match without UID
is labelled weaker evidence. Explicit mismatched UIDs are rejected; message
substrings never associate a record. Unknown timestamps and unmatched records
stay explicit. Redaction precedes retention and indexing, including malformed
JSON fallback; opaque reconcile IDs are retained only as hashes.

Space freezes the global arrival checkpoint across sources. `g` shows the oldest
retained window and `G` follows newest. Updated Kubernetes events append immutable
records, so later count changes cannot alter paused rows. Display sorts known
timestamps and exposes unknown times. Retention may evict paused records.

Eight live streams, at most two virtual event sources, 1 MiB per-source retention,
8 MiB aggregate retention, 512 displayed rows, and separate 512 KiB correlation
and event-deduplication indexes bound the work. Counters report stream omissions,
projection/index omissions, eviction, truncation and queue drops. Reopening a
source replaces its old tail; scope/UID/generation fences reject obsolete input.

Support dumps retain schema 1 with additive operational/timeline schema metadata,
source scope/API version/plural, identity, freshness and allowlisted summaries.
Raw API objects, plugin/controller payloads, certificate bytes, metric labels,
URLs, logs, event messages and PostgreSQL user/database names are omitted. The
owner-only artifact remains capped at 2 MiB.
