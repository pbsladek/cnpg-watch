# Fleet navigation and PostgreSQL diagnostics

Start with the scope and profile needed for your work:

```sh
cnpg-watch --all-namespaces --operator-namespace cnpg-system \
  --profile incident --password-file /path/to/monitor-password
```

Multiple visible clusters open Fleet. One cluster or `--cluster` opens Instances.
SQL runs only for the selected cluster; other rows show cached or never-sampled
values. A label selector (`--selector team=payments`) filters Cluster resources.
The shared Pod/evidence watches keep the information needed to associate related
objects. Enter drills into a row, Esc returns, j/k changes rows, and Tab changes
the selected cluster. The header always shows context, namespace, profile, and
scope epoch. Esc/Left and Right traverse a bounded history of 32 views. Row
selection follows stable IDs across updates and picks a remaining row when the
selected object disappears.

Press `i` to inspect a frozen row snapshot. Enter on a log row opens its full
retained redacted message. Use j/k or PageUp/PageDown to scroll, `i` to capture
again, and Esc to close. The capture time and scope distinguish snapshots from
live evidence; scope changes discard them. Doctor and profile panels also scroll.

At 120×40 the dashboard shows health, instances and both secondary panes.
At 100×30 it keeps one secondary pane; `:pane events` and `:pane poolers` choose
it. Smaller sizes retain the table and freshness first. Text remains meaningful
without color; selected health rows keep their severity color.

Use `:fleet`, `:instances`, `:health`, `:hints`, `:events`, `:services`, `:storage`,
`:poolers`, and `:operator` to inspect accepted evidence. Health reasons include
navigation targets and inspection hints. A read-write service mismatch must
persist for five seconds with synchronized, known endpoint identities before it
becomes critical. Permission gaps produce unknown or unavailable evidence and
leave unrelated sources running. Pooler statistics require explicit `--pooler-stats`; see the [operational guide](operational.md).

Change scope with `:namespace app-db`, `:namespace all`, `:context CONTEXT`, or
`:selector team=payments`. Replacement cancels old collectors and fences old
results before preparing the next client. A failed replacement remains visible;
select a valid context to recover. Command/filter history uses Up/Down and keeps
32 entries. Filters are limited to 512 bytes and invalid filters retain the last
valid filter:

| Input | Meaning |
| --- | --- |
| `/payments.*` | Regex over displayed row fields |
| `/!payments` | Inverse regex |
| `/-l team=payments,environment!=dev` | Label equality and inequality |
| `/-f paymnt` | Ordered fuzzy character match |
| `:clear` | Clear filter |

Logs open with `l` or `:logs` in incident profile. They are lazy, selected-source
streams; leaving the view cancels them. Press `s` to cycle primary, replica,
pooler, and operator source groups. Space pauses display; g/G selects oldest or
follows newest. `:since 5m`, `:since 15m`, and `:since 1h` restart with a bounded
historical tail. `:severity warning`, `error`, `fatal`, or `all` filters display.
A successful stream with no records is distinct from forbidden or restarting.
Known permission failures require `r` or reopening the view to recheck.

Each line is capped before parsing. Oversized lines are omitted; structured
records retain allowlisted fields, and credentials, connection URLs, addresses,
and statement/detail fields are redacted. Source retention defaults to 1 MiB,
aggregate retention to 8 MiB, and active streams to eight. Inactive sources expire
after one minute. Counters expose eviction, truncation, queue drops, and rejected
old generations. Reopening a source replaces its previous tail.

Fast WAL/archive/connection samples use the profile refresh interval. Capabilities
refresh every 60 seconds, medium replication/settings every 30 seconds, and deep
connection/wait samples every ten seconds while Connections or Waits is open in
incident profile. Actual cadence includes completion, jitter, backoff, and fair
admission; these are requested intervals, not freshness promises. All classes
share the exec budget and only one bundle is active per selected cluster.

`:replication` shows bounded slots, replication peers, and selected settings.
`:connections` and `:waits` show aggregate pressure, wait groups, blockers, and
long transactions. Query text is omitted unless `--show-query-text` is explicitly
selected in incident profile. Text remains bounded/redacted and is excluded from
support dumps. Below-capability fallback retains fast WAL without pressure;
unknown version/column/grant evidence suspends dependent work.

Connection pressure at 85% or rejected/overloaded connections sheds deep work
and slows medium work. At 70%, deep cadence slows. Timeouts also trigger bounded
shedding/backoff. Last-good samples remain visible with stale/error status;
Kubernetes watches and selected log streams continue independently. Promotion,
UID replacement, system identity, timeline, scope, and request checks fence old
results. Archive deltas compare compatible samples from the same reset epoch;
transaction age alone is never interpreted as byte lag.

`d`/`:doctor` shows accepted capabilities and source state without launching
extra probes. The `doctor --output json` command performs bounded active preflight
and reports detected SQL variants. `p` changes profile. `info --output json`
reports configured budgets and registry metadata without Kubernetes access.
`:dump` or F12 writes an owner-only, schema-versioned JSON artifact under the
configured dump directory (`--dump-dir`). The 2 MiB cap is enforced before
writing; payload-bearing screens are omitted. Raw API objects, credentials,
query text, log/event messages, and client addresses are excluded.

See [operational views and timelines](operational.md), the
[compatibility boundary](compatibility.md) and the
[Phase 7 receipt](validation/phase-7.md) for qualified combinations and limits.
