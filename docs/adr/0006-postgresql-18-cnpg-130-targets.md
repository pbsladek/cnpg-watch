# Initial compatibility

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Build with Rust 1.94.1, edition 2024. Initial live SQL/schema targets are PostgreSQL 18 and CNPG 1.30.0. Pin the exact Kubernetes fixture before live tests; claim no additional server/OS runtime support without recorded tests.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
