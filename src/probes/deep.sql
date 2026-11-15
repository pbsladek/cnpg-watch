WITH activity AS MATERIALIZED (
  SELECT pid,backend_type,state,wait_event_type,wait_event,xact_start,query_start,NULL::text AS query_text
  FROM pg_stat_activity WHERE pid<>pg_backend_pid()
), oldest AS (
  SELECT pid,backend_type,state,wait_event_type,wait_event,
    CASE WHEN xact_start IS NOT NULL THEN greatest(0,extract(epoch FROM clock_timestamp()-xact_start)*1000)::bigint END AS xact_age_ms,
    CASE WHEN query_start IS NOT NULL THEN greatest(0,extract(epoch FROM clock_timestamp()-query_start)*1000)::bigint END AS query_age_ms,
    query_text
  FROM activity WHERE xact_start IS NOT NULL OR state='active'
  ORDER BY xact_start NULLS LAST,query_start,pid LIMIT 20
)
SELECT json_build_object(
  'schema_version',1,'server_version',current_setting('server_version_num')::int,
  'current_user',current_user,'session_user',session_user,
  'superuser',(SELECT rolsuper FROM pg_roles WHERE rolname=current_user),
  'monitor',pg_has_role(current_user,'pg_monitor','USAGE'),
  'system_id',(SELECT system_identifier::text FROM pg_control_system()),
  'timeline',(SELECT timeline_id FROM pg_control_checkpoint()),
  'in_recovery',pg_is_in_recovery(),'server_time',clock_timestamp(),
  'connections',(SELECT json_build_object('clients',count(*) FILTER (WHERE backend_type='client backend'),
    'active',count(*) FILTER (WHERE backend_type='client backend' AND state='active'),
    'idle_in_transaction',count(*) FILTER (WHERE state='idle in transaction'),
    'max_connections',current_setting('max_connections')::int,
    'reserved',current_setting('reserved_connections')::int,
    'super_reserved',current_setting('superuser_reserved_connections')::int) FROM activity),
  'waits',(SELECT coalesce(json_agg(row_to_json(w)),'[]'::json) FROM
    (SELECT wait_event_type,wait_event,count(*) AS count FROM activity
      WHERE wait_event IS NOT NULL GROUP BY wait_event_type,wait_event
      ORDER BY count(*) DESC,wait_event_type,wait_event LIMIT 64) w),
  'blockers',(SELECT coalesce(json_agg(row_to_json(b)),'[]'::json) FROM
    (SELECT pid,(pg_blocking_pids(pid))[1:16] AS blocking_pids FROM
      (SELECT pid FROM activity WHERE wait_event_type='Lock' ORDER BY pid LIMIT 20) blocked) b),
  'long_transactions',(SELECT coalesce(json_agg(row_to_json(o)),'[]'::json) FROM oldest o),
  'autovacuum_workers',(SELECT count(*) FROM activity WHERE backend_type='autovacuum worker')
);
