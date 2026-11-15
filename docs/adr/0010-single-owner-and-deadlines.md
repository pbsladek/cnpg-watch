# Single state owner

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

One app loop owns state and renders immutable view data. Pure transitions receive monotonic time; deadline events advance freshness even without traffic. Bound bulk processing and service control/deadlines fairly. Avoid full-store frame clones and shared mutable app state.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
