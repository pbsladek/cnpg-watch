//! Owned session work, separated by lifecycle responsibility.
use super::*;
impl Session {
    pub fn set_log_scope(&mut self, scope: logs::timeline::Scope) {
        self.close_logs();
        self.log_scope = scope;
        self.log_projection_revision += 1;
    }
    pub fn sync_log_events(&mut self) {
        let stamp = (
            self.store.scope,
            self.clock.now().0 / 100,
            self.log_projection_revision,
        );
        if self.last_event_sync == Some(stamp) {
            return;
        }
        self.last_event_sync = Some(stamp);
        let enabled = self.view == View::Logs
            && self.config.profile == Profile::Incident
            && self.log_sources.events;
        let cluster = enabled
            && matches!(
                self.log_scope,
                logs::timeline::Scope::Cluster | logs::timeline::Scope::Backup(_)
            );
        let operator = enabled
            && self.log_sources.operator
            && self.config.operator_watch
            && !matches!(self.log_scope, logs::timeline::Scope::Selected);
        self.event_logs.sync(
            &self.store,
            &mut self.logs,
            &self.budgets,
            logs::events::Selection { cluster, operator },
            self.config.profile,
            self.clock.now(),
        );
    }
    pub fn close_logs(&mut self) {
        self.event_logs.close();
        self.last_event_sync = None;
        for (_, (_, token)) in std::mem::take(&mut self.log_active) {
            token.cancel();
        }
        self.logs.retain_active(&BTreeSet::new(), self.clock.now());
    }
    pub fn set_log_sources(&mut self, sources: Vec<logs::Source>, since_seconds: i64) {
        self.log_since_seconds = since_seconds;
        if self.view != View::Logs || self.config.profile != Profile::Incident {
            self.close_logs();
            return;
        }
        let sources: BTreeSet<_> = sources
            .into_iter()
            .filter(|s| s.channel.stream() && s.scope == self.store.scope && self.source_valid(s))
            .take(self.budgets.log_streams)
            .collect();
        let removed: Vec<_> = self
            .log_active
            .keys()
            .filter(|s| !sources.contains(*s))
            .cloned()
            .collect();
        for source in removed {
            if let Some((_, token)) = self.log_active.remove(&source) {
                token.cancel();
            }
        }
        let mut active = sources.clone();
        active.extend(self.event_logs.active().cloned());
        self.logs.retain_active(&active, self.clock.now());
        for source in sources {
            if self.log_active.contains_key(&source) {
                continue;
            }
            self.log_generation = self
                .log_generation
                .checked_add(1)
                .expect("log generation exhausted");
            let generation = self.log_generation;
            let token = self.root.child_token();
            self.logs
                .activate(source.clone(), generation, self.clock.now());
            let request = logs::StreamRequest {
                source: source.clone(),
                generation,
                since_seconds: since_seconds.clamp(60, 3600),
                profile: self.config.profile,
                budgets: self.budgets.clone(),
            };
            self.tasks.spawn(logs::follow(
                self.client.clone(),
                request,
                self.log_tx.clone(),
                self.log_dropped.clone(),
                token.clone(),
            ));
            self.log_active.insert(source, (generation, token));
        }
    }
    pub(super) fn source_valid(&self, s: &logs::Source) -> bool {
        logs::timeline::source_current(&self.store, s)
    }
    pub(crate) fn has_pending_work(&self) -> bool {
        !self.source_rx.is_empty() || !self.result_rx.is_empty() || !self.log_rx.is_empty()
    }
    pub fn active_log_streams(&self) -> usize {
        self.log_active.len()
    }
}
