# Phase 5 validation

Status: complete, locally validated on 2026-11-15 UTC. Phases 5/6 are complete;
next task: P7.1.

P5.1–P5.5 implement independent PgBouncer statistics inside shared exec admission.
The qualified combination is CNPG 1.30.0 / PgBouncer 1.25.1 / PostgreSQL 18.4 on
the pinned disposable k3d fixture. Broader version support remains unqualified.

The actual pooler console uses peer identity `pgbouncer` in the `pgbouncer`
container at `/controller/run`, port 5432. It does not use the PostgreSQL monitor
password. Runtime executes fixed SHOW VERSION/POOLS/STATS commands only. The
stock identity has console privileges; no stats-only identity is claimed.
`setup pooler-role` produces deterministic dry-run/diff guidance and has no apply
mode, because PostgreSQL DDL alone cannot change console authentication.
[ADR 0009](../adr/0009-optional-operational-evidence.md) records that decision.

`tests/pooler.rs` validates captured CSV, reordered/unknown columns, units,
unsupported versions, malformed values, row/output caps, fair per-pod admission,
request/scope/UID fences, stale retained samples and review-only setup.
`tests/pooler_live.rs` uses the actual peer console, holds controlled application
clients behind a one-server pool, observes waiting clients, and checks native
failure isolation, direct PostgreSQL samples, SQL-credential independence and
bounded shutdown. `tests/operational_live.rs` also denies pooler exec through
resourceNames while allowing direct instance exec, proving independent reduced
mode with actual RBAC. Runtime and doctor use the same canonical UID ownership
predicate before selecting a pooler target.

V01–V11 pass within the pinned local qualification boundary. The final
reproducible gate is:

```sh
make test-live E2E_SUITE=operations
```

It includes the earlier permission/setup, fleet, SQL/transport/promotion suites,
the pooler and operational tests, and three real TUI workflows. It creates and
deletes a unique fixture with a separate kubeconfig. CI selects the same suite;
remote CI and other platforms were not executed locally. The focused retained
fixture also passed ownership-verified doctor preflight and two identical
read-only setup diffs. Neither setup review changed authentication or roles.

Shared performance evidence and the Alpha/Beta checkpoints are recorded in the
[Phase 6 receipt](phase-6.md). The release workload uses production Pooler
scheduling, completion queue, reducer, typed parser and cache, with synthetic
responses; live authentication/saturation is a separate test. Optional stats
remain explicitly enabled and only run in Poolers/Connections. Readiness remains
visible if stats are disabled, unsupported, denied or stale.
