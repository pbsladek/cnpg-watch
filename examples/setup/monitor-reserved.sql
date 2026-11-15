-- cnpg-watch setup schema 1; PostgreSQL 18; profile MonitorReserved
-- Password policy: preserve. No credentials are printed.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL search_path = pg_catalog;
SET LOCAL statement_timeout = '3s';
SET LOCAL lock_timeout = '500ms';
SELECT pg_advisory_xact_lock(178219, 2);
DO $setup$
BEGIN
  IF current_setting('server_version_num')::int NOT BETWEEN 180000 AND 189999 THEN
    RAISE EXCEPTION 'setup requires PostgreSQL 18';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'watch_monitor') THEN
    CREATE ROLE "watch_monitor" LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS INHERIT;
  END IF;
  IF EXISTS (SELECT FROM pg_roles WHERE rolname='watch_monitor' AND (rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls)) THEN
    RAISE EXCEPTION 'existing role has elevated attributes; review before --prune';
  END IF;
END
$setup$;
ALTER ROLE "watch_monitor" LOGIN INHERIT;
GRANT pg_monitor TO "watch_monitor" WITH INHERIT TRUE;
GRANT pg_use_reserved_connections TO "watch_monitor" WITH INHERIT TRUE;
ALTER ROLE "watch_monitor" SET statement_timeout = '375ms';
ALTER ROLE "watch_monitor" SET lock_timeout = '100ms';
ALTER ROLE "watch_monitor" SET idle_in_transaction_session_timeout = '1000ms';
COMMENT ON ROLE "watch_monitor" IS 'Managed by cnpg-watch; MonitorReserved; bootstrap only';
COMMIT;
