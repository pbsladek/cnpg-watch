# Isolated live validation

Status: Accepted

## Context

The [design](../../design.md) and [implementation guide](../../implementation.md)
require a small, testable CNPG dashboard with bounded incident-time overhead.

## Decision

Use named disposable k3d clusters with a separate kubeconfig; never merge into the user's kubeconfig or run failure injection on its default context. Pin k3d/k3s, CNPG, and PostgreSQL, preserve bounded redacted evidence, and clean up only run-owned resources.

## Consequences

Validate this decision with the corresponding phase gate. Record measured limits
and compatibility changes here instead of silently changing runtime contracts.
