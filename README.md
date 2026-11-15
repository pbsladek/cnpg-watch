# cnpg-watch

An observe-only terminal dashboard for CloudNativePG on macOS and Linux.
It combines fleet health, bounded logs, PostgreSQL diagnostics, PgBouncer
statistics and CNPG operational evidence. The dashboard observes; explicit setup
commands handle RBAC and monitoring roles.
See [installation](docs/installation.md), [compatibility](docs/compatibility.md),
and the [Phase 7 qualification receipt](docs/validation/phase-7.md).
The [implementation guide](implementation.md) tracks delivery; the
[design](design.md) owns contracts and budgets.

## Run

Rust 1.94.1 is pinned in `rust-toolchain.toml`.

```sh
cargo build --locked
cargo run -- --namespace app-db --cluster payments
cargo run -- info --output json
```

The dashboard uses the current kubeconfig context/namespace unless flags
select another scope. Without SQL credentials it still watches topology and
shows SQL as unavailable. `--profile minimal` explicitly disables SQL.

For SQL, configure a dedicated `watch_monitor` login with `pg_monitor` through
CNPG-managed roles or your administrative workflow. CNPG reserves the `cnpg_`
role prefix, so the project's default avoids it. Store the matching password in
an owner-only local file and select it explicitly:

```sh
chmod 600 /path/to/monitor-password
cargo run -- --namespace app-db --cluster payments \
  --password-file /path/to/monitor-password
```

The password travels through Kubernetes exec stdin, then psql authenticates to
loopback PostgreSQL as the monitoring role with TLS required. It is never a
command argument. Normal operation uses the Kubernetes client directly and
never shells out to kubectl. Runtime refuses a superuser result and does not
create roles, Secrets, or RBAC.

Keys: `q`/Ctrl-C quit, `j`/`k` or arrows select an instance, `Tab` selects the next
cluster, `r` requests refresh within admission/backoff limits, `d` shows capabilities, `p` opens profiles, and `?` opens help.
Enter opens related views; `i` inspects a frozen, scrollable row snapshot.
Esc/Left returns and Right goes forward. Use `:fleet`, `:health`, `:logs`,
`:replication`, `:connections`, and `:waits` to change views; `/` filters rows.
See the [fleet and diagnostics guide](docs/fleet-and-probes.md) for scope changes,
log controls, query-text opt-in, and support dumps.
Doctor checks the actual scoped permissions and monitoring role. Setup renders
offline plans, compares live state, or applies only with explicit `--apply`. See
[doctor and setup](docs/setup.md) for credentials, profiles, subject validation,
password policy, and reviewed examples. Operator flags select a separate discovery/watch/setup scope.

## Configuration and diagnostics

Precedence is CLI flags, supported `CNPG_WATCH_*` environment variables, the
optional TOML config file, then profile defaults (minimal 5 s, standard 2 s,
incident 1 s, secure 3 s). `info` prints paths, build/platform, requested scope,
query/capability metadata, and resolved budgets without
contacting Kubernetes. Undetected live values are null. `--config` selects a
file; config files are capped at 64 KiB and unknown keys are errors.

```toml
namespace = "app-db"
cluster = "payments"
profile = "standard"
refresh = "2s"
db_role = "watch_monitor"
password_file = "/path/to/monitor-password"

[budgets]
api_qps = 5
api_burst = 10
```

Environment overrides include `CNPG_WATCH_CONFIG`, `CNPG_WATCH_CONTEXT`,
`CNPG_WATCH_NAMESPACE`, `CNPG_WATCH_ALL_NAMESPACES`, `CNPG_WATCH_PROFILE`,
`CNPG_WATCH_REFRESH`, `CNPG_WATCH_PASSWORD_FILE`, `CNPG_WATCH_DB_ROLE`, and
`CNPG_WATCH_LOG_FILE`. No environment variable accepts a plaintext password.

Watch diagnostics go to the bounded private log printed by `info`, keeping
stderr out of the terminal display. `--log-file` chooses another path and
`--verbose` enables debug events. External error contents are suppressed; typed
error classes remain visible. Monitor files are capped at 4 KiB and must have
no group/other permissions. Fast SQL uses an end-to-end 750 ms default deadline,
a shorter server statement limit, one active bundle per cluster/pod, and bounded
retry backoff. Last-good samples survive failed refreshes. Unselected clusters
are not sampled in the background.

## Validate

```sh
make check
make test
make test-replay
make test-scheduler
make bench
make test-load
make test-soak
make cli-artifacts
make package
```

Fast tests need no cluster or credentials. `make test` includes reviewed buffer
snapshots, property tests and portable-pty/vt100 terminal restoration checks.
Live/load/soak tests are explicitly selected;
`test-load` runs for 60 seconds and `test-soak` for 30 minutes in release mode.
Synthetic workload results characterize the local pipeline, separately from the
live SQL/TUI and idle measurements. Initial receipts:
[Phase 0](docs/validation/phase-0.md), [Phase 1](docs/validation/phase-1.md),
[Phase 2](docs/validation/phase-2.md), [Phase 3](docs/validation/phase-3.md),
[Phase 4](docs/validation/phase-4.md), [Phase 5](docs/validation/phase-5.md),
[Phase 6](docs/validation/phase-6.md), [Phase 7](docs/validation/phase-7.md).

For live validation, install Docker, curl, openssl and Python 3.11+. Install the
checksum-verified fixture tools, then select a pinned Kubernetes cell:

```sh
./scripts/install-e2e-tools.sh
E2E_MATRIX_CELL=k8s135 make test-live E2E_SUITE=operations
```

The wrapper creates a unique disposable cluster with a separate kubeconfig,
CNPG 1.30.0, PostgreSQL 18.4, two database namespaces, a primary/replica,
a PgBouncer 1.25.1 pooler, operational resources, and a monitoring role. It tests
actual setup/idempotence, least-privilege identities and exec verbs,
authentication/grants, output/status/timeout/cancellation, pod
replacement, promotion, overload shedding, fleet/log scope changes, watch
reconnect, SQL-disabled topology, PgBouncer saturation/denial, named certificates,
optional metrics, managed resources, webhook routing, correlated timelines, and the real TUI, then
cleans up its own cluster. `E2E_KEEP_CLUSTER=1` retains it for investigation.
Artifacts are under `artifacts/e2e/`; working kubeconfigs/passwords stay private
and are excluded from CI artifact uploads. The wrapper never targets the user's
default context for mutation or failure injection.

CI definitions cover Linux/macOS fast checks, the isolated live slice, bounded
load, and a scheduled soak/property run. Local validation receipts distinguish
checks actually run from jobs that still need execution on another platform.

Pooler statistics are explicitly enabled with `--pooler-stats`. Operational views
include `:backups`, `:roles`, `:databases`, `:subscriptions`, `:replicas`,
`:plugins`, and `:certs`. Cluster resources, named certificate reads and operator
metrics each require explicit configuration. See the [operational guide](docs/operational.md)
for permissions, limits, native console identity and timeline commands.
