SELECT json_build_object(
  'schema_version',1,'server_version',current_setting('server_version_num')::int,
  'current_user',current_user,'session_user',session_user,
  'superuser',(SELECT rolsuper FROM pg_roles WHERE rolname=current_user),
  'monitor',pg_has_role(current_user,'pg_monitor','USAGE'),
  'system_id',(SELECT system_identifier::text FROM pg_control_system()),
  'timeline',(SELECT timeline_id FROM pg_control_checkpoint()),
  'in_recovery',pg_is_in_recovery(),'server_time',clock_timestamp(),
  'views',(SELECT json_agg(json_build_object('name',v.name,
    'readable',coalesce(has_table_privilege(current_user,to_regclass('pg_catalog.'||v.name),'SELECT'),false),
    'columns',(SELECT coalesce(json_agg(a.attname ORDER BY a.attname),'[]'::json)
      FROM pg_attribute a WHERE a.attrelid=to_regclass('pg_catalog.'||v.name)
      AND a.attnum>0 AND NOT a.attisdropped)))
    FROM (VALUES ('pg_stat_activity'),('pg_stat_replication'),('pg_stat_archiver'),
      ('pg_replication_slots'),('pg_settings'),('pg_roles')) v(name)),
  'functions',(SELECT json_agg(json_build_object('name',f.name,
    'allowed',coalesce(has_function_privilege(current_user,to_regprocedure('pg_catalog.'||f.name),'EXECUTE'),false)))
    FROM (VALUES ('pg_control_system()'),('pg_control_checkpoint()'),('pg_is_in_recovery()'),
      ('pg_current_wal_lsn()'),('pg_last_wal_receive_lsn()'),('pg_last_wal_replay_lsn()'),
      ('pg_last_xact_replay_timestamp()'),('pg_wal_lsn_diff(pg_lsn,pg_lsn)'),('pg_blocking_pids(integer)')) f(name))
);
