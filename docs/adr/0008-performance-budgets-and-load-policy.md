# Bounded work

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Use one validated BudgetConfig for rates, byte/count queues, retention, output and framing limits, concurrency, and deadlines. Failed SQL preserves stale evidence and backs off; shutdown does not depend on a free queue slot. Performance claims require a measured release baseline.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
