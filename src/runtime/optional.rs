//! Owned session work, separated by lifecycle responsibility.
use super::*;
impl Session {
    pub(super) fn admit_auxiliary(&mut self, now: Tick) {
        if self.scope_loading {
            return;
        }
        self.auxiliary_scheduler.reconcile(&self.config, now);
        let Some(ticket) = self.auxiliary_scheduler.admit(self.store.scope, now) else {
            return;
        };
        if !self.store.begin_auxiliary(ticket.clone()) {
            self.auxiliary_scheduler
                .finish(&ticket, Err(ErrorClass::Capacity), now);
            return;
        }
        let cancel = self.root.child_token();
        self.auxiliary_stop = Some(cancel.clone());
        let (loader, config, tx, root) = (
            self.auxiliary_loader.clone(),
            self.config.clone(),
            self.result_tx.clone(),
            self.root.clone(),
        );
        self.tasks.spawn(async move {
            let mut result = loader.execute(&config, &ticket.key, &cancel).await;
            if result
                .as_ref()
                .is_ok_and(|s| s.bytes() + 512 > config.budgets.result_bytes)
            {
                result = Err(ErrorClass::Capacity);
            }
            let bytes = result.as_ref().map_or(512, |s| s.bytes() + 512);
            let _ = tx
                .send(
                    Completion::Auxiliary(AuxiliaryCompletion { ticket, result }),
                    bytes,
                    &root,
                )
                .await;
        });
    }
    pub(super) fn admit_pooler(&mut self, now: Tick) {
        if self.scope_loading
            || !self.config.pooler_stats
            || self.config.profile == Profile::Minimal
            || !matches!(self.view, View::Poolers | View::Connections)
        {
            return;
        }
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return;
        };
        let Some(ticket) = self.pooler_scheduler.admit(now) else {
            return;
        };
        if !self.store.begin_pooler(ticket.clone()) {
            self.pooler_scheduler
                .finish(&ticket, Err(ErrorClass::Capacity), now);
            return;
        }
        let cancel = self.root.child_token();
        self.pooler_stops
            .insert(ticket.pod.key.clone(), (ticket.clone(), cancel.clone()));
        let (client, socket, budgets, tx, root) = (
            self.client.clone(),
            self.config.pooler_socket.clone(),
            self.budgets.clone(),
            self.result_tx.clone(),
            self.root.clone(),
        );
        self.tasks.spawn(async move {
            let mut result =
                crate::pooler::execute(client, &ticket.pod.key, &socket, &budgets, &cancel).await;
            if result
                .as_ref()
                .is_ok_and(|s| s.bytes() + 512 > budgets.result_bytes)
            {
                result = Err(ErrorClass::Capacity);
            }
            let bytes = result.as_ref().map_or(512, |s| s.bytes() + 512);
            let _ = tx
                .send(
                    Completion::Pooler(PoolerCompletion {
                        ticket,
                        result,
                        _permit: permit,
                    }),
                    bytes,
                    &root,
                )
                .await;
        });
    }
}
