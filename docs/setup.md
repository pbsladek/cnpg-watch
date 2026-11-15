# Doctor, profiles, and explicit setup

`watch` observes resources. `doctor` checks access and runs the read-only monitor
bundle when credentials and a single primary are available. Only a setup command
with `--apply` changes RBAC or PostgreSQL roles. The TUI has no bootstrap actions.

## Profiles

| Profile | Default refresh | SQL | Logs requested | Redaction |
| --- | --- | --- | --- | --- |
| minimal | 5 s | disabled | no | standard |
| standard | 2 s | selected cluster only | no | standard |
| incident | 1 s | selected cluster only | yes | standard |
| secure | 3 s | selected cluster only | no | strict |

An explicit CLI, environment, or config refresh overrides the default. All
profiles retain the same validated queue, concurrency, deadline, and retry
budgets. No profile reads Secrets by default or samples fleet SQL in the background.
Explicit named certificate reads are available outside minimal profile. Query text
is omitted unless explicitly enabled in incident profile. Secure `info` also
hides local file paths.

The shared catalog describes the full profile envelope for RBAC planning.
Fleet topology, operational evidence, logs, and fast/medium/deep SQL are implemented.
Incident RBAC includes selected pod logs. PgBouncer statistics, cluster-scoped
resources, named certificate GETs and metrics have separate explicit configuration
gates; granting RBAC alone does not enable them. See the [operational guide](operational.md). `--no-operator-watch` disables operator
requirements. Operator namespace
access and optional cluster resource access are separate choices.

Press `d` in the TUI for accepted capability/source evidence. Press `p` for
profile details; `1`/`2`/`3`/`4` switch profiles from that panel. A switch advances
the scope epoch before cancelling old watches/probes, clears old evidence, and
restarts under the new envelope. Explicit refresh overrides survive the switch.
Esc closes the panel. These panels perform no permission review or setup I/O;
use the doctor command for an active preflight.

## Doctor

```sh
cnpg-watch --namespace app-db --cluster payments \
  --operator-namespace cnpg-system --password-file /private/monitor-password \
  doctor --output json
cnpg-watch --namespace app-db doctor --include-cluster-resources
```

JSON schema 1 includes scope, profile/envelope, capabilities, individual access
checks and their evidence, operator candidates, findings, setup suggestions,
terminal/transport/role state, typed probe errors, and resolved budgets. Text
output groups findings by feature. A completed report exits successfully even
when it describes reduced access; automation should inspect its states. Invalid
arguments or failures to serialize output still fail the command.

Doctor uses SelfSubjectAccessReviews, then safe live list/watch checks when
reviews are unavailable. A review POST does not persist or bootstrap resources.
Named get, log, or exec access that cannot be tested safely remains `unknown`.
When a single primary and protected credentials are available, the shipped
read-only SQL bundle checks actual transport, identity, grants, and PostgreSQL
18 compatibility. Doctor validates the primary's CNPG ownership label first.
Remote errors are classified; arbitrary stderr, connection details, and passwords
are suppressed. Requests have deadlines and share the client's API rate budget.

`forbidden`, `unknown`, `absent`, `syncing`, and `empty` describe different facts.
An empty successful list is not an authorization failure. API resource lists do
not depend on discovery or permission to read CRD objects. Operator discovery
uses the configured namespace/selector, or a bounded all-namespace candidate
search. Zero, multiple, and inaccessible candidates remain explicit; doctor
never guesses among installs. Optional operator failures do not block database
observation. `--include-cluster-resources` adds catalog and webhook
checks; these reads are unnecessary for namespaced observation.

## Kubernetes RBAC

Choose the database scope and subject explicitly. ServiceAccount subjects default
to the supplied database namespace; all-namespace mode requires
`--service-account-namespace`. The account must exist before apply. User/Group
subjects are supplied names, not inferred identities or newly created accounts.

```sh
cnpg-watch --namespace app-db --operator-namespace cnpg-system \
  setup kubernetes-rbac --service-account cnpg-watch --dry-run --output yaml
cnpg-watch --namespace app-db setup kubernetes-rbac \
  --user reader@example.test --diff --output json
cnpg-watch --namespace app-db setup kubernetes-rbac \
  --user reader@example.test --apply --output json
cnpg-watch --all-namespaces setup kubernetes-rbac \
  --service-account cnpg-watch --service-account-namespace tools --dry-run
```

No mode flag means an offline dry-run. Dry-runs never create a client or read
live objects. Namespace mode produces a Role/RoleBinding; a separate configured
operator namespace gets its own pair. All-namespace observation uses a
ClusterRole/ClusterRoleBinding. Namespaced bindings never confer cluster-wide
object access. Add `--include-cluster-resources` explicitly for the separate
catalog/webhook pair. Secret rules are absent by default; explicit named
certificate references add only get grants for those resourceNames.
`databaserolelists` and `/status` rules are never generated. Pooler ownership
requires namespaced Deployment/ReplicaSet reads, independently of operator scope.

`--name` selects a managed object family (default `cnpg-watch`). Diff schema 1
reports matching/missing objects, missing rules/subjects, extra managed rules,
and `would_change`. Ordering/grouping differences and Kubernetes' ServiceAccount
subject defaulting do not produce false diffs. Apply reads and validates every
target before writing, refuses unmanaged name collisions or changed immutable
role references, and uses resource versions for updates. A repeated matching
apply sends no writes.

Extras are preserved by default. `--prune` removes extra rules/subjects from the
chosen managed objects only; it can be previewed with `--diff`. It does not
delete old object families, unrelated bindings, accounts, or namespaces. Changes
to several Kubernetes objects are not one transaction: after a failed apply,
rerun diff to review remaining work. Inspect the checked-in examples:
[namespace](../examples/setup/namespace.yaml),
[separate operator](../examples/setup/operator.yaml),
[optional cluster objects](../examples/setup/cluster.yaml).

The pinned kube 5.0.0 / Kubernetes 1.35.5 WebSocket transport needs both `get` and
`create` on `pods/exec`. Real service-account tests denied get-only, create-only,
and neither; both succeeded. This requirement is qualified for that combination,
not every Kubernetes distribution or feature-gate configuration.

## PostgreSQL roles

```sh
cnpg-watch setup postgres-role --print-sql
cnpg-watch setup postgres-role --role-profile monitor-reserved --print-sql
cnpg-watch setup postgres-role --role-profile actions --print-sql
cnpg-watch --namespace app-db --cluster payments \
  setup postgres-role --admin-role postgres \
  --admin-password-file /private/admin-password --diff --output json
cnpg-watch --namespace app-db --cluster payments \
  --password-file /private/monitor-password setup postgres-role \
  --admin-role postgres --admin-password-file /private/admin-password \
  --apply --rotate-password --validate --output json
```

Use owner-only UTF-8 password files, at most 4 KiB, with no embedded newline or
NUL. A final newline is accepted. The tool sends admin credentials and SQL over
exec stdin to TLS loopback psql; neither credential is a Kubernetes command
argument. It authenticates as the supplied administrative login. This credential
path requires exec access to the selected primary and valid administrative
PostgreSQL authentication. No Secret is read or created by setup.

The default role is `watch_monitor`; `--role-name` changes the setup target.
CNPG reserves the `cnpg_` prefix. The monitor profile grants `pg_monitor`;
monitor-reserved additionally grants `pg_use_reserved_connections`. Actions
creates a separate `watch_actions` NOLOGIN role with `pg_signal_backend` and no
monitor grant. Creating it does not enable actions in the TUI.

Generated SQL targets PostgreSQL 18, takes a transaction advisory lock, creates
missing roles, sets bounded role timeouts, and adds a management comment. Grant validation checks
effective inherited privileges; an existing membership with INHERIT FALSE is
reported by diff and repaired by apply. Existing
elevated role attributes cause an error unless explicit pruning is requested.
Extra memberships are reported and preserved; `--prune` removes extra direct
memberships and elevated attributes for this target role. It leaves unrelated
object grants/roles alone. Preview that SQL with `--prune --print-sql` first.
Keep CNPG-managed role specs/password Secrets consistent with any chosen setup;
this command does not change the operator's desired configuration.

Passwords are preserved by default (`--preserve-password` makes that explicit).
A new role in preservation mode has no password. `--rotate-password` sets the
value from `--password-file`, even for an existing role. Printed rotation SQL
references `CNPG_WATCH_NEW_PASSWORD` through psql's environment/quoted-variable
support; it never embeds a supplied or generated credential. Offline JSON
prints plan metadata; online diff/apply prints a versioned JSON report. Matching
repeat applies skip DDL. After apply, grant/attribute checks always run;
`--validate` also authenticates as the monitor and runs the production bundle.
Admin credentials are confined to this command and never enter the watch config.

Reviewed SQL: [monitor](../examples/setup/monitor.sql),
[reserved monitor](../examples/setup/monitor-reserved.sql),
[separate actions](../examples/setup/actions.sql).

## Qualification

```sh
make check
make test
make test-live E2E_SUITE=permissions
```

The live wrapper creates its own uniquely named fixture and kubeconfig, enables
admin TCP authentication only in that fixture, runs actual bearer-token RBAC,
SQL setup, discovery/operator/profile tests, then the Phase 1 SQL/watch and PTY
smoke tests. Cleanup deletes its cluster. Private credential files are excluded
from uploaded artifacts. Add `E2E_MEASURE_IDLE=1` for the release TUI idle check.
See [the Phase 2 receipt](validation/phase-2.md) for results and support limits.

Authorization reference: [Kubernetes authorization](https://kubernetes.io/docs/reference/access-authn-authz/authorization/).
Role/password syntax: [PostgreSQL 18 CREATE ROLE](https://www.postgresql.org/docs/18/sql-createrole.html)
and [psql](https://www.postgresql.org/docs/18/app-psql.html).

## Pooler and named certificate planning

`setup pooler-role --pooler NAME --dry-run` and `--diff` produce deterministic
review guidance for the native peer console. Apply is unsupported: changing a
PostgreSQL role does not configure PgBouncer console authentication. Runtime
`--pooler-stats` uses the stock console identity and fixed SHOW commands.

`--certificate-secret namespace/name` opts into exact named public-certificate
reads. Include the same flags when generating RBAC; the generated Roles contain
get-only Secret rules restricted to those resourceNames. No Secret list/watch is
granted. `--cluster-resources` adds runtime global watches and corresponding RBAC
planning. The setup/doctor `--include-cluster-resources` flag remains available.
