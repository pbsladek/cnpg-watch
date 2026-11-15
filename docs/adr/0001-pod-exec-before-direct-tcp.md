# Pod-local SQL transport and monitor authentication

Status: Accepted after the Phase 1 live proof (2026-11-15 UTC)

## Context

The [design](../../design.md#monitoring-authentication-and-transport-proof)
requires a dedicated monitoring role, bounded work, and no superuser fallback.
Local peer authentication does not provide that role automatically.

## Decision

Use Kubernetes WebSocket exec into the `postgres` container and psql over
loopback TCP with `sslmode=require`. Read a protected local password file (at
most 4 KiB; no group/other permissions), then send the password over exec stdin.
A fixed shell wrapper exports it for psql. Passwords never enter command
arguments, tracing fields, UI values, or machine-readable output.

Use `watch_monitor` as the default: CNPG 1.30.0 rejected the design's earlier
`cnpg_watch` name as reserved when provisioning a managed role. Runtime also
verifies the effective/session role, monitoring membership, and no superuser.
No fallback to the container's peer-authenticated administrative identity exists.

The fast instance bundle has a 750 ms client deadline including attachment
cleanup. Its statement timeout derives from half that deadline, capped at
500 ms (375 ms with defaults); lock timeout is capped at 100 ms. A 2 s libpq
connect bound and a minimum 2 s transient backoff protect failed attempts.
Concurrent capped stdout/stderr reads and remote exit status determine success.

## Evidence

The actual monitoring role executed the shipped PostgreSQL 18.4 bundle against
CNPG 1.30.0 / k3s 1.35.5. The proof rejected invalid credentials, nonzero remote
status with JSON stdout, and oversized output. Timeout and cancellation were
followed by a separate test-only admin observer confirming no remaining
`application_name=cnpg-watch` backend. Initial fast bundles took roughly 45–47 ms.

See [the Phase 1 receipt](../validation/phase-1.md) for exact commands and limits.

## Consequences

A protected credential file is required for SQL; topology continues without it.
TLS inside the pod encrypts loopback traffic; Kubernetes exec remains a broader
capability than the SQL role. Port-forward/native SQL is deferred until a
measured need justifies another transport. Production setup commands remain
Phase 2 work; live tests provision their own roles and isolated resources.
