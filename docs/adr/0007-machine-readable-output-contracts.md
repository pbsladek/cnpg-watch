# Versioned output

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Use schema_version 1 for info and subsequent doctor/dump contracts. Unknown detected values serialize as null. Machine output stdout contains only the requested document; diagnostics go to stderr. Do not serialize credential content or arbitrary external error text.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
