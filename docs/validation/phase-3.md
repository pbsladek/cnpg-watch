# Phase 3 validation

Status: complete, locally validated on 2026-11-15 UTC.

The fleet implementation covers P3.1–P3.8. Local qualification uses macOS arm64,
Rust 1.94.1, kube 5.0.0, k3d 5.9.0, Kubernetes 1.35.5-k3s1, CNPG 1.30.0,
PostgreSQL 18.4, and PgBouncer 1.25.1. The Phase 4 receipt shares the performance
and fresh-fixture gate. Remote CI and other OS/server combinations were not run
in this session.

Delivered behavior includes all-namespace fleet membership, separate operator
scope, label selection, cancellable context/namespace replacement, typed topology
evidence, operator and Pooler availability, health/hints, navigation/filters,
lazy selected logs, and bounded private support dumps. Runtime ownership lives
in `runtime.rs`; input/navigation and row rendering remain separate. The
[command guide](../fleet-and-probes.md) provides a usable walkthrough and limits.

| Gate | Evidence |
| --- | --- |
| V01 | `make check`: locked build, formatting, all-target clippy with warnings denied; `make test`: 69 Rust tests and terminal restoration cases |
| V02 | Real app PTY checks across views, command/filter validation, profile changes, resize redraws, private dumps, q/Ctrl-C; normal/error/panic terminal restoration |
| V03–V05 | UID/scope/watch/request fencing, interrupted/complete relists, indexed owner association, time-only event expiry, persistent service mismatch/recovery, health cache/full recomputation comparison, paused-time deadlines and scheduling |
| V06 | Capped unterminated log framing, canonical redaction, source/aggregate retention, old-generation rejection, inactive expiry, independent queue bytes, flood responsiveness and task reclamation |
| V07–V08 | Real monitor SQL, separate operator namespace, two database namespaces, lazy primary/pooler logs including empty-success state, scope/context error and recovery, exact empty selector, unselected SQL off |
| V09 | Release load/soak, production log parsing/redaction/retention and health; separate actual release TUI idle and SQL cost measurements in the shared Phase 4 record |
| V10 | Generated Roles with actual restricted bearer-token ServiceAccount; revoked logs/exec/services remain local; prior bootstrap/idempotence and transport tests retained |
| V11 | Actual trimmed resource captures, malformed/unknown/conflicting association tests, stable navigation/filter history, canonical log bounds, redacted schema-1 dumps |

`tests/fleet_live.rs` verifies actual selected-only collection and permission
revocation; `tests/fleet_smoke.py` uses the real binary and PTY. A bounded projection reconstructs the cursor/erase operations emitted by the
TUI, with regression checks for unchanged letters, split sequences, Unicode,
resize, and incomplete-sequence bounds. Actual resize redraws are also exercised;
the checks do not assume unchanged letters reappear in the byte stream. Full release terminal and
compatibility qualification remains Phase 7.

The live fixture exposed two valid API shapes: EndpointSlice `endpoints: null`
and Pooler status without `readyInstances`. Both have regression coverage in
[actual resource captures](../../fixtures/cnpg/1.30.0/fleet/README.md). Pooler pod
readiness is independent; admin statistics remain unknown. A label-only owner
association is weaker than an exact UID association, and conflicts stay unknown.

Default log budgets are eight streams, 16 KiB framed lines, 1 MiB per source,
8 MiB aggregate retention, and one-minute inactive expiry. Oversized lines are
omitted before parsing. Dropped queue entries, truncation, eviction and rejected
generations are visible separately. Reopening replaces a source tail; the tool
does not claim exactly-once log delivery or a merged chronological timeline.

Dumps use allowlisted summaries, bounded lists, and a 2 MiB serialized cap before
owner-only create-new writes. They omit raw objects, log/event messages, query
text, client addresses and credentials. Payload-bearing views are omitted even
when query text is explicitly displayed. Working artifacts and kubeconfigs are
ignored and excluded from CI uploads.

See [ADR 0013](../adr/0013-fleet-evidence-and-selected-streams.md) and the
[Phase 4 measurement record](phase-4.md). Phases 5–7 retain PgBouncer statistics,
broader operational evidence, correlated logs, and release qualification.
