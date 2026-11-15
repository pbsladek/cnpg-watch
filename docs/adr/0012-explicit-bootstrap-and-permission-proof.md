# Explicit bootstrap and permission proof

Status: Accepted

## Decision

Use one scope-aware feature catalog for doctor, profile inspection, and RBAC
planning. Profile envelopes include requested future features, which remain
marked unfinished until their collectors exist. Runtime source/probe evidence
feeds the same catalog; renderers perform no permission I/O.

Doctor uses SSAR and bounded non-mutating live checks. A direct CNPG list does
not require discovery or CRD-object reads. Operator discovery reports zero,
multiple, or forbidden candidates without choosing an install implicitly.

Offline setup needs no client. Online diff reads only; explicit apply is the
only bootstrap write entry point. RBAC uses managed labels, complete read-before-
write planning, resource versions, semantic rule comparison, and normalization
of ServiceAccount subject defaulting. Extra rules/subjects survive unless prune
is explicit. Namespace and optional cluster-scoped access remain separate.

PostgreSQL setup uses an explicit administrative login and protected password
file over exec stdin/TLS loopback. DDL is transactional and serialized, manages
only its named role, preserves passwords/extra memberships by default, and
validates effective inherited grants. Administrative credentials never enter
watch configuration. Requested monitor authentication runs the shipped bundle.
Actions are a separate NOLOGIN role and do not add a runtime action path.

## Evidence and boundaries

The live fixture establishes kube 5.0.0 / Kubernetes 1.35.5 exec requirements:
get-only, create-only, and neither fail; both succeed. Keep both verbs in the
catalog for this pinned combination. CNPG 1.30.0/PostgreSQL 18.4 remain the live
reference. Other distributions/feature-gate combinations need qualification.

See [the Phase 2 receipt](../validation/phase-2.md) and
[command documentation](../setup.md) for validation, output contracts, and
multi-object apply/password/ownership limits.
