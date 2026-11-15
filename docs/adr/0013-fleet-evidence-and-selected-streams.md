# Fleet evidence and selected streams

Status: Accepted

## Decision

Keep a single runtime owner for normalized watch state, probe admission, health,
selected log retention, and task lifetimes. Separate terminal navigation and
render-only row projection from the runtime. Scope replacement prepares its
client asynchronously after advancing the scope epoch and cancelling old work.
Identical watch scopes share collectors; database and operator permissions stay
independent. Cluster selectors select membership without removing Pod/evidence
needed for association.

Use typed compact evidence, strong owner UID association when available, and
explicit label/conflict/unknown association otherwise. Index cluster pods and
compare health caching with a full recomputation reference. Service/primary
mismatches require known synchronized identity and a convergence window.

Open capped log streams only for selected incident-profile views. Frame before
parsing, retain canonical redacted records, enforce both per-source and aggregate
bytes, and drop queued log work rather than block control/topology. Cancellation
and generation fencing apply across view/source/profile/scope replacement.
Reopening a source replaces its old tail. Logs are grouped by source; correlated
operator timelines remain Phase 6.

Support artifacts use explicit allowlists, bounded lists and serialized size,
private create-new files, and omit payload-bearing screens. This avoids trusting
redaction alone to remove arbitrary query, event, or structured log contents.

## Evidence and consequences

The live CNPG 1.30.0 fixture showed null EndpointSlice endpoints and Pooler status
without readyInstances. These are valid unknown/empty shapes, covered by actual
captures. Pooler pod readiness is displayed independently; no statistics are
invented. Resource adapters do not retain unused specs or annotations. Tests
cover independent source failures, old identities, complete/partial relists,
log floods, scope replacement, redaction, and bounded task cleanup.
