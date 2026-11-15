# Native peer console captures

Captured from CNPG 1.30.0 / PgBouncer 1.25.1 in the disposable fixture on
2026-11-15. The `pgbouncer` container executes psql as OS/database identity
`pgbouncer` against `/controller/run`, port 5432, with `--csv` and fixed
SHOW VERSION/POOLS/STATS commands. Empty application pools are valid.
`saturated.csv` was captured while isolated fixture clients waited behind a
one-server transaction pool. Names are fixture identities; no passwords,
connection strings or client addresses are included. Parsed microseconds and
counter values are samples, not contractual constants. Only this version was
live-qualified; column/version variations are synthetic unit inputs.
