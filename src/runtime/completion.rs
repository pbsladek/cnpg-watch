//! Owned session work, separated by lifecycle responsibility.
use super::*;
impl Session {
    pub(super) fn complete(&mut self, completion: Completion, now: Tick) {
        match completion {
            Completion::Sql(value) => {
                let SqlCompletion {
                    ticket,
                    result,
                    _permit,
                } = value;
                let outcome = result.as_ref().map(|_| ()).map_err(|e| *e);
                let accepted = match result {
                    Ok(Collected::Fast(sample)) => {
                        let accepted = self.store.finish_probe(ticket.clone(), Ok(*sample), now);
                        if accepted {
                            self.completed += 1;
                            let pressure = self
                                .store
                                .selected_pods()
                                .filter_map(|p| self.store.samples.get(&p.id.key))
                                .filter(|o| {
                                    matches!(
                                        o.freshness(now, self.budgets.stale_after()),
                                        Freshness::Fresh(_)
                                    )
                                })
                                .filter_map(|o| o.last_good.as_ref())
                                .filter(|s| self.store.valid_ticket(&s.ticket))
                                .filter_map(|s| s.value.connections.as_ref()?.pressure_percent())
                                .max();
                            self.scheduler.pressure(pressure);
                        }
                        accepted
                    }
                    Ok(Collected::Diagnostic(sample)) => {
                        let accepted =
                            self.store
                                .finish_diagnostic(ticket.clone(), Ok(*sample), now);
                        if accepted {
                            self.diagnostic_completed += 1;
                        }
                        accepted
                    }
                    Err(error) => {
                        if ticket.class == Class::Fast {
                            self.store.finish_probe(ticket.clone(), Err(error), now)
                        } else {
                            self.store
                                .finish_diagnostic(ticket.clone(), Err(error), now)
                        }
                    }
                };
                if accepted {
                    self.scheduler.finish(&ticket, outcome, now);
                }
                self.probes.remove(&ticket.request);
            }
            Completion::Auxiliary(value) => {
                let AuxiliaryCompletion { ticket, result } = value;
                let outcome = result.as_ref().map(|_| ()).map_err(|e| *e);
                if ticket.scope == self.store.scope && self.auxiliary_scheduler.accepts(&ticket) {
                    let accepted = self.store.finish_auxiliary(ticket.clone(), result, now);
                    let outcome = if !accepted {
                        Err(ErrorClass::Capacity)
                    } else if let Some(Attempt::Failed(error)) =
                        self.store.auxiliary.get(&ticket.key).map(|o| &o.attempt)
                    {
                        Err(*error)
                    } else {
                        outcome
                    };
                    self.auxiliary_scheduler.finish(&ticket, outcome, now);
                    self.auxiliary_stop = None;
                } else {
                    self.store.rejected += 1;
                }
            }
            Completion::Pooler(value) => {
                let PoolerCompletion {
                    ticket,
                    result,
                    _permit,
                } = value;
                let outcome = result.as_ref().map(|_| ()).map_err(|e| *e);
                if self.pooler_scheduler.accepts(&ticket)
                    && crate::pooler::valid(&self.store, &ticket)
                {
                    let accepted = self.store.finish_pooler(ticket.clone(), result, now);
                    let outcome = if !accepted {
                        Err(ErrorClass::Capacity)
                    } else if let Some(Attempt::Failed(e)) = self
                        .store
                        .pooler_samples
                        .get(&ticket.pod.key)
                        .map(|o| &o.attempt)
                    {
                        Err(*e)
                    } else {
                        outcome
                    };
                    self.pooler_scheduler.finish(&ticket, outcome, now);
                } else {
                    self.store.rejected += 1;
                }
                if self
                    .pooler_stops
                    .get(&ticket.pod.key)
                    .is_some_and(|(current, _)| current == &ticket)
                {
                    self.pooler_stops.remove(&ticket.pod.key);
                }
            }
        }
    }
}
