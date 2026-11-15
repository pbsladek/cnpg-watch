# Fleet captures

Captured from the isolated CNPG 1.30.0 / PostgreSQL 18.4 / Kubernetes
1.35.5 fixture on 2026-11-15 UTC. Each file is a list of actual API responses
trimmed to fields consumed by the production adapters. UIDs are consistently
hashed and pod IPs use documentation addresses. Event messages are omitted.
No Secret objects, annotations, credential references, or unused pod specs remain.

The fixture has two database namespaces and a separate operator namespace.
Its Pooler uses PgBouncer 1.25.1. CNPG's Pooler status has `instances` and
`phase`, but no `readyInstances`: the tool retains unknown CR readiness and
shows observed pooler pod readiness separately. Empty EndpointSlices can have
null `endpoints`; the adapter treats that as an empty list.
