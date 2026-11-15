# Typed observations at the API boundary

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Read CNPG DynamicObject values only inside adapters, normalize into validated small domain values, and preserve missing/unknown evidence. Keep core Kubernetes pods typed. Pin the CNPG schema reference and keep fixture provenance.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
