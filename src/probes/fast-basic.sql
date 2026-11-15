SELECT json_build_object(
  'server_version', current_setting('server_version_num')::int,
  'current_user', current_user, 'session_user', session_user,
  'superuser', (SELECT rolsuper FROM pg_roles WHERE rolname = current_user),
  'monitor', pg_has_role(current_user, 'pg_monitor', 'USAGE'),
  'server_time', clock_timestamp(),
  'in_recovery', pg_is_in_recovery(),
  'system_id', (SELECT system_identifier::text FROM pg_control_system()),
  'timeline', (SELECT timeline_id FROM pg_control_checkpoint()),
  'current', CASE WHEN NOT pg_is_in_recovery() THEN pg_current_wal_lsn() END,
  'received', CASE WHEN pg_is_in_recovery() THEN pg_last_wal_receive_lsn() END,
  'replay', CASE WHEN pg_is_in_recovery() THEN pg_last_wal_replay_lsn() END,
  'last_replayed_transaction_age_ms', CASE WHEN pg_is_in_recovery() THEN
    extract(epoch FROM clock_timestamp() - pg_last_xact_replay_timestamp()) * 1000 END,
  'peers', (SELECT coalesce(json_agg(row_to_json(p)), '[]'::json) FROM
    (SELECT application_name AS application, host(client_addr) AS address, replay_lsn AS replay
       FROM pg_stat_replication ORDER BY application_name, client_addr LIMIT 128) p),
  'archive', (SELECT json_build_object('archived_count',archived_count,'failed_count',failed_count,
    'stats_reset',stats_reset,'last_archived_time',last_archived_time,'last_failed_time',last_failed_time)
    FROM pg_stat_archiver)
);
