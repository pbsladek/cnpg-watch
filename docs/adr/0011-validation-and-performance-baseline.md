# Validation evidence

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Use CLI/PTY checks, lifecycle replays and properties, paused-time scheduling, actual least-privilege SQL transport contracts, Criterion baselines, bounded load, and a scheduled 30-minute soak. Keep unmet live/performance gates open and record evidence under docs/validation.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
