# Compatibility and support boundary

cnpg-watch 0.1.0 targets macOS arm64/Intel and Linux arm64/x86_64. An architecture
name in an archive is a build target; runtime qualification is recorded separately
in the [Phase 7 receipt](validation/phase-7.md). Native, Rosetta and Docker/QEMU
runs are labelled explicitly. Hosted CI is configured in this repository and is
not implied by a local run.

The pinned live matrix uses CNPG 1.30.0, PostgreSQL 18.4, PgBouncer 1.25.1 and
k3d 5.9.0. Kubernetes cells are defined in `tests/e2e/matrix.json`:

| Cell | Kubernetes fixture |
| --- | --- |
| `k8s134` | K3s v1.34.8+k3s1 |
| `k8s135` | K3s v1.35.5+k3s1 |
| `k8s136` | K3s v1.36.1+k3s1 |

These minors match the upstream CNPG 1.30 compatibility range. The pinned patch
versions are reproducible test fixtures, rather than deployment upgrade advice.
[CNPG release notes](https://cloudnative-pg.io/docs/current/release_notes/v1.30/)
describe the upstream range. Successful cells and actual exec-verb results are
recorded in the receipt. Other patches, distributions, PostgreSQL majors, custom
Pooler authentication, and plugin implementations have no live support claim.

Actual exec outcomes with the shipped native WebSocket transport are:

| Cell | Get only | Create only | Both | Neither |
| --- | --- | --- | --- | --- |
| `k8s134` | Allowed | Denied | Allowed | Denied |
| `k8s135` | Denied | Denied | Allowed | Denied |
| `k8s136` | Denied | Denied | Allowed | Denied |

Generated RBAC and preflight conservatively require both verbs across this
range. An authorization review alone does not prove an exec attachment works.
Local macOS execution uses 27.2.1; older OS versions have no local runtime
qualification. Matching hosted runner results remain a separate gate.

The Kubernetes client uses bounded native API watches, exec and log requests.
Actual get-only/create-only/both/neither exec tests run in every compatibility
cell. Every enabled PostgreSQL bundle variant runs as the intended monitoring
role on both primaries and the replica. A missing column, grant, CRD, optional
scope or unrecognized version reports reduced mode; it cannot silently invent a
healthy value. DatabaseRoleList is a list kind, not an API endpoint.

Pooler statistics require explicit opt-in and the verified local peer console.
Pooler setup is review-only. Certificate reads require exact configured Secret
names, and metrics require an explicit endpoint. No backup status, certificate
metadata, quorum record or archiver counter proves a successful restore, TLS
trust, or safe promotion. See the [operational guide](operational.md) for limits.

Release performance qualification uses 100 clusters, 300 instances, two Pooler
pods, 200 operational resources, 1,000 synthetic topology updates/second and
5,120 synthetic log lines/second. It exercises the production responsive
renderer, health, timelines, admission, queues and retention. The receipt lists
which systems meet the latency/RSS budgets. This does not claim that the app
collects SQL from 300 instances simultaneously: SQL is selected-cluster only.
Larger fleets remain characterization cases until they pass the same gates.
