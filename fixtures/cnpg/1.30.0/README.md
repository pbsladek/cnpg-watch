# Phase 1 schema fixtures

Provisional minimal fixtures use the Cluster schema from upstream tag `v1.30.0`:
https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/v1.30.0/config/crd/bases/postgresql.cnpg.io_clusters.yaml

Only metadata, `status.currentPrimary`, and `status.phase` are consumed here.
Pod fields use the Kubernetes v1 Pod API; CNPG labels follow the 1.30 docs.
Live captured fixtures belong in `captured/` after the isolated proof, with
sensitive annotations/spec fields removed. Provisional fixtures are not claimed
as captures from a running cluster.

Operational fixtures were captured from the isolated CNPG 1.30.0 / PostgreSQL
18.4 fixture on 2026-11-15. Metadata is reduced to identity, generation and owner
references. Database/DatabaseRole/Publication status is controller-produced;
Backup records a real unavailable-plugin failure and ScheduledBackup is suspended.
Subscription and FailoverQuorum lists are genuinely empty; they do not claim a
successful controller reconciliation. `operational-discovery.json` records the
served kinds/plurals/scopes. Synthetic status transitions in tests are separate
from these captures. `server-public.crt` contains only the disposable fixture's
public certificate, with no private key; tests inject clock/rotation metadata.
