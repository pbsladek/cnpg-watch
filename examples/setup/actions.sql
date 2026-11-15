-- cnpg-watch setup schema 1; PostgreSQL 18; profile Actions
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
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'watch_actions') THEN
    CREATE ROLE "watch_actions" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS INHERIT;
  END IF;
  IF EXISTS (SELECT FROM pg_roles WHERE rolname='watch_actions' AND (rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls)) THEN
    RAISE EXCEPTION 'existing role has elevated attributes; review before --prune';
  END IF;
END
$setup$;
ALTER ROLE "watch_actions" NOLOGIN INHERIT;
GRANT pg_signal_backend TO "watch_actions" WITH INHERIT TRUE;
ALTER ROLE "watch_actions" SET statement_timeout = '375ms';
ALTER ROLE "watch_actions" SET lock_timeout = '100ms';
ALTER ROLE "watch_actions" SET idle_in_transaction_session_timeout = '1000ms';
COMMENT ON ROLE "watch_actions" IS 'Managed by cnpg-watch; Actions; bootstrap only';
COMMIT;
