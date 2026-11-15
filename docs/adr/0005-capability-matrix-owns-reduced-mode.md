# Shared capability state

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Implement only topology and fast-SQL capability entries in Phase 1. Availability, last-good evidence, and latest-attempt outcome are independent. Phase 2 expands this into a scope-aware profile catalog consumed by doctor, setup, and the TUI. Future features remain unfinished; no renderer performs API permission checks. See [ADR 0012](0012-explicit-bootstrap-and-permission-proof.md).

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
