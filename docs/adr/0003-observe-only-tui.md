# Observe-only runtime

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Watch and TUI code have no mutation actions. SQL executes catalog/statistics queries with a dedicated monitor role. Doctor/setup stay explicitly unfinished until Phase 2; no placeholder command reports a successful apply.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
