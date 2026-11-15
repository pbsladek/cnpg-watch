# Optional operational evidence and pooler console identity

Status: Accepted

CNPG 1.30.0 with PgBouncer 1.25.1 exposes a Unix peer console in the pooler
container at `/controller/run`, port 5432. The effective identity is `pgbouncer`;
it has console privileges. The PostgreSQL monitoring role does not authenticate
this connection. The observed Deployment/ReplicaSet ownership chain determines
which current pooler pod may receive the fixed SHOW-only probe.

Keep statistics explicitly enabled, independently scheduled and bounded inside
the shared exec budget. Limit console commands to SHOW VERSION/POOLS/STATS.
Pooler setup remains review-only until a safe stats-only CNPG configuration and
authentication path is proven. Do not generate a PostgreSQL role and claim that
it changes peer-console privileges. Other socket layouts need explicit selection;
TCP/password console transport remains unsupported.

Decode operational resources at the API boundary into finite enums and compact
allowlisted fields. Discover the served CNPG plural; DatabaseRoleList is a list
kind, not a separate endpoint. Keep optional malformed Cluster context separate
from core topology. Reconciliation findings need current observed generation;
backup ordering, webhook routing scope and resource UIDs remain explicit evidence.

Cluster-scoped watches, named certificate GETs, and metrics scrapes each require
explicit configuration. Secret access uses exact resourceNames grants, with no
list/watch. Retain only public certificate metadata. Metrics use a separate lazy
HTTP client with no Kubernetes authorization or redirects. Failure of either
source preserves core observation and marks retained evidence stale.

Canonical redacted log records carry typed origin, source identity, bounded
structured context and a global arrival sequence. Immutable Kubernetes event
records share the retention budget. Correlation indexes borrow canonical records,
use metadata rather than text matching, and expose omissions. Full/unmatched
operator access is explicit. Payload-bearing screens remain excluded from dumps.

Validate with the [operational guide](../operational.md) and phase 5/6 receipts.
These choices qualify the pinned fixture and do not establish compatibility with
all CNPG, Kubernetes, PostgreSQL or PgBouncer versions.

The final live PTY gate also exposed lost keystrokes when resize and keyboard
readiness arrived together in Crossterm's default Unix Mio reader. Temporary
key/view tracing showed that the actor handled every delivered key correctly;
the missing key never reached it. Select Crossterm 0.29's supported `use-dev-tty`
feature, which uses its poll-based Unix TTY source, while keeping EventStream and
the bounded input channel. No temporary input tracing remains. The actual-app
smoke now repeats sixteen doctor/profile/escape cycles beside resize events and
still checks profile changes, q/Ctrl-C and terminal restoration. This validates
the fix without implementing a second terminal parser or changing frame cadence.
