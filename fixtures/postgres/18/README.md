# PostgreSQL 18 fast-bundle fixture

`primary-fast.json` was captured through the real `watch_monitor` role and
Kubernetes exec transport against PostgreSQL 18.4 / CNPG 1.30.0 in the isolated
k3d fixture on 2026-11-15 UTC. System identity, timestamps, and peer addresses
were anonymized; schema, data types, role/grant checks, WAL fields, and archive
shape remain the production query output. It contains no password or query text.

`capabilities.json`, `medium.json`, and `deep.json` are actual outputs from the
same isolated pinned combination, captured on 2026-11-15. System identity, timeline epoch (normalized to one), and
server time were anonymized; query text is null. The six production variants
(capabilities, fast pressure, fast basic fallback, medium, deep, and explicit
query-text deep) run as the monitor role on a primary and replica in live tests.
Missing-column, function/grant, unsupported-version, malformed, and oversized
variants are derived explicitly in `tests/probes_registry.rs` rather than
presented as successful live outputs. Only PostgreSQL 18.4 is live-qualified.
