# Lifecycle acceptance

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Scope/watch/topology epochs, object UIDs, and request IDs protect against obsolete completions. InitApply membership stages separately; InitDone commits atomically. Failed initialization retains the last complete set. Deletes match UIDs, not only names.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
