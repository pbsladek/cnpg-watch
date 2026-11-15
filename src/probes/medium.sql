SELECT json_build_object(
  'schema_version',1,'server_version',current_setting('server_version_num')::int,
  'current_user',current_user,'session_user',session_user,
  'superuser',(SELECT rolsuper FROM pg_roles WHERE rolname=current_user),
  'monitor',pg_has_role(current_user,'pg_monitor','USAGE'),
  'system_id',(SELECT system_identifier::text FROM pg_control_system()),
  'timeline',(SELECT timeline_id FROM pg_control_checkpoint()),
  'in_recovery',pg_is_in_recovery(),'server_time',clock_timestamp(),
  'slots',(SELECT coalesce(json_agg(row_to_json(s)),'[]'::json) FROM
    (SELECT slot_name,slot_type,active,restart_lsn::text,confirmed_flush_lsn::text,wal_status,safe_wal_size,
      CASE WHEN NOT pg_is_in_recovery() AND restart_lsn IS NOT NULL
        THEN pg_wal_lsn_diff(pg_current_wal_lsn(),restart_lsn)::text END AS retained_bytes
      FROM pg_replication_slots ORDER BY slot_name LIMIT 64) s),
  'slot_count',(SELECT count(*) FROM pg_replication_slots),
  'replication',(SELECT coalesce(json_agg(row_to_json(r)),'[]'::json) FROM
    (SELECT application_name,state,sync_state,write_lsn::text,flush_lsn::text,replay_lsn::text
      FROM pg_stat_replication ORDER BY application_name LIMIT 128) r),
  'settings',(SELECT json_object_agg(name,setting) FROM pg_settings WHERE name IN
    ('max_connections','reserved_connections','superuser_reserved_connections','synchronous_commit',
     'synchronous_standby_names','wal_level','wal_keep_size','max_slot_wal_keep_size','hot_standby'))
);
