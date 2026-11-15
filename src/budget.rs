use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budgets {
    pub api_qps: u32,
    pub api_burst: u32,
    pub exec_concurrency: usize,
    pub pending_probes: usize,
    pub probe_timeout_ms: u64,
    pub stale_ms: u64,
    pub backoff_floor_ms: u64,
    pub backoff_ceiling_ms: u64,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub control_messages: usize,
    pub control_bytes: usize,
    pub topology_messages: usize,
    pub topology_bytes: usize,
    pub result_messages: usize,
    pub result_bytes: usize,
    pub log_messages: usize,
    pub log_queue_bytes: usize,
    pub log_line_bytes: usize,
    pub log_batch_bytes: usize,
    pub log_source_bytes: usize,
    pub log_total_bytes: usize,
    pub log_streams: usize,
    pub object_bytes: usize,
    pub cache_bytes: usize,
    pub cache_entries: usize,
    pub batch_events: usize,
    pub batch_ms: u64,
    pub render_ms: u64,
    pub watch_sync_ms: u64,
    pub event_retention_ms: u64,
    pub service_convergence_ms: u64,
    pub log_idle_ms: u64,
    pub dump_bytes: usize,
    pub medium_interval_ms: u64,
    pub deep_interval_ms: u64,
    pub diagnostic_timeout_ms: u64,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            api_qps: 5,
            api_burst: 10,
            exec_concurrency: 2,
            pending_probes: 128,
            probe_timeout_ms: 750,
            stale_ms: 10_000,
            backoff_floor_ms: 2000,
            backoff_ceiling_ms: 30_000,
            stdout_bytes: 256 * 1024,
            stderr_bytes: 16 * 1024,
            control_messages: 128,
            control_bytes: 512 * 1024,
            topology_messages: 1024,
            topology_bytes: 8 * 1024 * 1024,
            result_messages: 128,
            result_bytes: 4 * 1024 * 1024,
            log_messages: 64,
            log_queue_bytes: 4 * 1024 * 1024,
            log_line_bytes: 16 * 1024,
            log_batch_bytes: 64 * 1024,
            log_source_bytes: 1024 * 1024,
            log_total_bytes: 8 * 1024 * 1024,
            log_streams: 8,
            object_bytes: 1024 * 1024,
            cache_bytes: 64 * 1024 * 1024,
            cache_entries: 20_000,
            batch_events: 64,
            batch_ms: 4,
            render_ms: 100,
            watch_sync_ms: 60_000,
            event_retention_ms: 900_000,
            service_convergence_ms: 5000,
            log_idle_ms: 60_000,
            dump_bytes: 2 * 1024 * 1024,
            medium_interval_ms: 30_000,
            deep_interval_ms: 10_000,
            diagnostic_timeout_ms: 2000,
        }
    }
}
impl Budgets {
    pub fn validate(&self) -> anyhow::Result<()> {
        let value = serde_json::to_value(self)?;
        anyhow::ensure!(
            value.as_object().unwrap().values().all(|v| v
                .as_u64()
                .is_some_and(|n| n > 0 && n <= u64::from(u32::MAX))),
            "budgets must be positive and fit the supported 32-bit counters/permits"
        );
        anyhow::ensure!(
            self.api_qps <= self.api_burst
                && self.backoff_floor_ms >= 2000
                && self.backoff_floor_ms <= self.backoff_ceiling_ms,
            "invalid rate/backoff budget"
        );
        anyhow::ensure!(
            self.object_bytes
                .checked_add(512)
                .is_some_and(|n| n <= self.topology_bytes)
                && self.object_bytes <= self.cache_bytes
                && self
                    .stdout_bytes
                    .checked_add(self.stderr_bytes)
                    .and_then(|n| n.checked_add(512))
                    .is_some_and(|n| n <= self.result_bytes)
                && self.control_bytes >= 64,
            "payload exceeds queue/cache budget"
        );
        anyhow::ensure!(
            self.log_line_bytes <= self.log_batch_bytes
                && self.log_batch_bytes <= self.log_source_bytes
                && self.log_source_bytes <= self.log_total_bytes
                && self.log_batch_bytes <= self.log_queue_bytes,
            "inconsistent log budgets"
        );
        anyhow::ensure!(
            self.render_ms >= 100
                && self.batch_ms <= self.render_ms
                && self.probe_timeout_ms < self.stale_ms
                && self.medium_interval_ms >= self.probe_timeout_ms
                && self.deep_interval_ms >= self.probe_timeout_ms
                && self.diagnostic_timeout_ms <= 5000
                && self.diagnostic_timeout_ms >= self.probe_timeout_ms,
            "invalid timing budgets"
        );
        anyhow::ensure!(
            [
                self.control_bytes,
                self.topology_bytes,
                self.result_bytes,
                self.log_queue_bytes
            ]
            .into_iter()
            .all(|n| n <= u32::MAX as usize),
            "queue byte budget exceeds semaphore range"
        );
        Ok(())
    }
    pub fn statement_timeout_ms(&self) -> u64 {
        (self.probe_timeout_ms / 2).clamp(1, 500)
    }
    pub fn lock_timeout_ms(&self) -> u64 {
        (self.statement_timeout_ms() / 2).clamp(1, 100)
    }
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.probe_timeout_ms)
    }
    pub fn stale_after(&self) -> Duration {
        Duration::from_millis(self.stale_ms)
    }
}
