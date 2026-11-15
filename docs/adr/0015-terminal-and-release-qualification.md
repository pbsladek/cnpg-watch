# Terminal and release qualification

Status: Accepted

## Decision

Keep terminal layout pure with three fixed breakpoints: 120×40, 100×30 and a
table-first smaller layout. Share header/footer rendering with scrollable
snapshots, doctor and profile panels. Bound navigation history to 32 entries,
reconcile rows by stable identity and discard frozen details on scope changes.

Use reviewed TestBackend buffers and properties for detailed layout coverage.
Use portable-pty/vt100 for the small real-process suite: navigation, resizing,
flood feedback and terminal restoration. Child processes, reader threads and
capture buffers have bounded cleanup/retention. CI cannot accept snapshots.

Pin Kubernetes 1.34/1.35/1.36 fixtures and test actual exec identities separately
from permission preflight. The 1.34 fixture accepts get-only exec, while newer
cells require both get and create with this transport. Generated RBAC retains
both for portability. Record actual outcomes per cell rather than infer
transport behavior from a successful authorization review.

Shared Docker VM disk pressure evicted otherwise healthy disposable fixtures.
New fixture nodes use a 1 GiB absolute eviction floor with 99/98% image GC
thresholds. This adjustment is explicit in fixture receipts and does not alter
existing clusters or application runtime budgets. Never prune unrelated state.

Create deterministic archives with versioned inventories and checksums. Execute
all four platform binaries; distinguish native, Rosetta and QEMU results.
Packages contain generated completions and command documentation from clap.
Local package/compatibility evidence qualifies a candidate. GA still requires
the configured hosted jobs to run successfully. Packaging does not authorize
publication.

## Evidence

See the [Phase 7 receipt](../validation/phase-7.md),
[development guide](../development.md) and [release procedure](../releasing.md).
