# Capability-gated PostgreSQL probe classes

Status: Accepted

## Decision

Ship six explicit PostgreSQL 18 variants with schemas, required view columns and
function privileges, role/version gates, row/output/deadline bounds, and fallback
metadata. Detect actual capabilities on each selected server before admitting
dependent work. Preserve the fast-basic fallback when pressure columns are
unavailable. Deep query text requires explicit incident opt-in; dumps never
include it.

Schedule capability, fast, medium, and active-view deep intents independently,
using bounded coalesced targets, weighted class priority, fair pod rotation,
completion-based cadence, seeded jitter, and monotonic backoff. Reuse the proven
exec transport and global permit budget. Authentication suspends until explicit
recheck; unsupported/forbidden work stays local. Overload sheds deep and slows
medium, retaining last-good evidence and independent Kubernetes/log observation.

Only accepted, fresh, identity-compatible fast results affect pressure policy.
Use the maximum selected-pod pressure so a quiet replica cannot erase pressure
on the primary. Capability recapture compares a prior fast role only within the
same topology epoch, allowing recovery after promotion.

Validate system/timeline/role/UID/scope/request identity before accepting results
or calculating WAL/archive deltas. Peer association must be unique. Reset or
incompatible counters do not yield deltas. Production SQL reads bounded system
monitoring views; it does not scan user relations or perform runtime mutations.
Pin search_path to pg_catalog in runtime and setup SQL sessions.

## Evidence and consequences

All six variants run as watch_monitor on PostgreSQL 18.4 primary and replica.
The live fixture holds real monitor connections to provoke reserved-slot errors,
then performs a CNPG switchover and checks recovery. Startup FATAL reserved-slot
messages may lack SQLSTATE; the classifier handles that observed form explicitly.
Test-only EXPLAIN separates database execution/planning from full bundle time and
an exec-only baseline. PostgreSQL 18.4 is the qualified point release; major-18
metadata does not claim live coverage for every point release.
