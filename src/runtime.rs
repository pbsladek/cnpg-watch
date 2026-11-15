mod completion;
#[path = "runtime/logs.rs"]
mod log_streams;
mod optional;
use crate::{
    budget::Budgets,
    cli::Profile,
    collectors,
    config::Config,
    health, kubernetes, logs,
    model::*,
    navigation::View,
    probes::{
        self, PodExec,
        registry::{Class, Diagnostic},
    },
    queue,
    reducer::{Changes, Store, WatchEvent},
    scheduler::Scheduler,
    time::Clock,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
enum Collected {
    Fast(Box<FastSample>),
    Diagnostic(Box<Diagnostic>),
}
struct SqlCompletion {
    ticket: ProbeTicket,
    result: Result<Collected, ErrorClass>,
    _permit: OwnedSemaphorePermit,
}
enum Completion {
    Sql(SqlCompletion),
    Pooler(PoolerCompletion),
    Auxiliary(AuxiliaryCompletion),
}
struct AuxiliaryCompletion {
    ticket: crate::auxiliary::Ticket,
    result: Result<crate::auxiliary::Sample, ErrorClass>,
}
struct PoolerCompletion {
    ticket: crate::pooler::Ticket,
    result: Result<crate::pooler::Sample, ErrorClass>,
    _permit: OwnedSemaphorePermit,
}
struct Prepared {
    client: kube::Client,
    namespace: String,
    config: Config,
    sql: Availability,
    executor: Option<PodExec>,
    operator: Option<crate::doctor::Operator>,
}
struct ScopeReady {
    epoch: ScopeEpoch,
    result: Result<Prepared, ErrorClass>,
}
pub struct Session {
    pub store: Store,
    pub clock: Clock,
    pub budgets: Budgets,
    pub sql: Availability,
    pub namespace: String,
    pub completed: u64,
    pub diagnostic_completed: u64,
    pub max_queue_bytes: usize,
    pub cleanup_errors: u64,
    pub logs: logs::Book,
    pub log_scope: logs::timeline::Scope,
    pub log_sources: logs::timeline::Sources,
    pub log_projection_revision: u64,
    pub log_omitted: usize,
    pub event_logs: logs::events::Events,
    last_event_sync: Option<(ScopeEpoch, u64, u64)>,
    pub health: health::Engine,
    pub operator: Option<crate::doctor::Operator>,
    pub view: View,
    pub scope_error: Option<ErrorClass>,
    auxiliary_scheduler: crate::auxiliary::Scheduler,
    auxiliary_loader: crate::auxiliary::Loader,
    auxiliary_stop: Option<CancellationToken>,
    pooler_scheduler: crate::pooler::Scheduler,
    pooler_stops: BTreeMap<Key, (crate::pooler::Ticket, CancellationToken)>,
    source_rx: queue::Receiver<WatchEvent>,
    source_tx: queue::Sender<WatchEvent>,
    source_stops: BTreeMap<Kind, CancellationToken>,
    result_rx: queue::Receiver<Completion>,
    result_tx: queue::Sender<Completion>,
    scope_rx: queue::Receiver<ScopeReady>,
    scope_tx: queue::Sender<ScopeReady>,
    log_rx: queue::Receiver<logs::Message>,
    log_tx: queue::Sender<logs::Message>,
    log_active: BTreeMap<logs::Source, (u64, CancellationToken)>,
    log_generation: u64,
    log_recheck: Tick,
    log_since_seconds: i64,
    pub log_dropped: Arc<AtomicU64>,
    pub(crate) tasks: JoinSet<()>,
    pub(crate) root: CancellationToken,
    source_cancel: CancellationToken,
    loader_cancel: CancellationToken,
    probes: BTreeMap<RequestId, CancellationToken>,
    pub scheduler: Scheduler,
    executor: Option<PodExec>,
    permits: Arc<Semaphore>,
    client: kube::Client,
    pub(crate) config: Config,
    scope_loading: bool,
    pub(crate) wake: Arc<tokio::sync::Notify>,
    last_second: u64,
    last_reconcile: Option<(u64, u64, View)>,
}
impl Session {
    pub async fn start(config: Config) -> Result<Self, ErrorClass> {
        let budgets = config.budgets.clone();
        budgets.validate().map_err(|_| ErrorClass::Capacity)?;
        let (client, namespace) = tokio::time::timeout(
            Duration::from_secs(10),
            kubernetes::client(&config, &budgets),
        )
        .await
        .map_err(|_| ErrorClass::Timeout)??;
        Key::new(&namespace, config.cluster.as_deref().unwrap_or("scope"))
            .map_err(|_| ErrorClass::InvalidData)?;
        Self::with_client(config, client, namespace).await
    }
    /// The caller supplies the shared rate-limited client; production uses `start`.
    pub async fn with_client(
        mut config: Config,
        client: kube::Client,
        namespace: String,
    ) -> Result<Self, ErrorClass> {
        let budgets = config.budgets.clone();
        budgets.validate().map_err(|_| ErrorClass::Capacity)?;
        let operator = if config.operator_watch {
            Some(crate::doctor::operator(client.clone(), &config).await)
        } else {
            None
        };
        if config.operator_namespace.is_none() {
            config.operator_namespace = operator
                .as_ref()
                .filter(|o| o.state == crate::doctor::State::Available)
                .and_then(|o| o.namespace.clone());
        }
        let wake = Arc::new(tokio::sync::Notify::new());
        let (source_tx, source_rx) = queue::channel_with_notify(
            budgets.topology_messages,
            budgets.topology_bytes,
            wake.clone(),
        );
        let (result_tx, result_rx) =
            queue::channel_with_notify(budgets.result_messages, budgets.result_bytes, wake.clone());
        let (log_tx, log_rx) =
            queue::channel_with_notify(budgets.log_messages, budgets.log_queue_bytes, wake.clone());
        let (scope_tx, scope_rx) = queue::channel_with_notify(2, 64 * 1024, wake.clone());
        let auxiliary_loader = crate::auxiliary::Loader::new(client.clone(), budgets.clone());
        let root = CancellationToken::new();
        let source_cancel = root.child_token();
        let loader_cancel = root.child_token();
        let mut tasks = JoinSet::new();
        let mut store = Store::new(budgets.clone());
        store.enable_sources(
            collectors::plans(&config, &namespace)
                .into_iter()
                .map(|p| p.kind),
        );
        store.describe_sources(collectors::describe(&config, &namespace));
        let source_stops = collectors::spawn_config(
            &mut tasks,
            client.clone(),
            &config,
            &namespace,
            store.scope,
            source_tx.clone(),
            &source_cancel,
        );
        let (sql, executor) = load_executor(&config, &client, &budgets).await;
        let scheduler = Scheduler::with_registry(
            budgets.clone(),
            config.refresh,
            0x243f6a8885a308d3,
            config.profile,
            config.show_query_text,
        );
        let view = if config.cluster.is_some() {
            View::Instances
        } else {
            View::Fleet
        };
        Ok(Self {
            store,
            clock: Clock::default(),
            budgets: budgets.clone(),
            sql,
            namespace,
            completed: 0,
            diagnostic_completed: 0,
            max_queue_bytes: 0,
            cleanup_errors: 0,
            logs: Default::default(),
            log_scope: logs::timeline::Scope::Selected,
            log_sources: Default::default(),
            log_projection_revision: 0,
            log_omitted: 0,
            event_logs: Default::default(),
            last_event_sync: None,
            health: Default::default(),
            operator,
            view,
            scope_error: None,
            source_rx,
            source_tx,
            source_stops,
            auxiliary_scheduler: Default::default(),
            auxiliary_loader,
            auxiliary_stop: None,
            pooler_scheduler: Default::default(),
            pooler_stops: BTreeMap::new(),
            result_rx,
            result_tx,
            scope_rx,
            scope_tx,
            log_rx,
            log_tx,
            log_active: BTreeMap::new(),
            log_generation: 0,
            log_recheck: Tick(0),
            log_since_seconds: 900,
            log_dropped: Arc::new(AtomicU64::new(0)),
            tasks,
            root,
            source_cancel,
            loader_cancel,
            probes: BTreeMap::new(),
            scheduler,
            executor,
            permits: Arc::new(Semaphore::new(budgets.exec_concurrency)),
            client,
            config,
            scope_loading: false,
            wake,
            last_second: 0,
            last_reconcile: None,
        })
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    pub fn client(&self) -> kube::Client {
        self.client.clone()
    }
    fn apply_changes(&mut self, changes: Changes) {
        for request in changes.cancel {
            if let Some(token) = self.probes.get(&request) {
                token.cancel();
            }
        }
    }
    pub fn select_cluster(&mut self, key: Option<Key>) {
        let changes = self.store.select(key);
        self.apply_changes(changes);
        self.close_logs();
    }
    fn reset_scope(&mut self) {
        let changes = self.store.switch_scope();
        self.apply_changes(changes);
        for token in self.probes.values() {
            token.cancel();
        }
        self.source_cancel.cancel();
        self.source_cancel = self.root.child_token();
        for (_, (_, stop)) in std::mem::take(&mut self.pooler_stops) {
            stop.cancel();
        }
        self.pooler_scheduler = Default::default();
        if let Some(stop) = self.auxiliary_stop.take() {
            stop.cancel();
        }
        self.auxiliary_scheduler = Default::default();
        self.close_logs();
        self.logs.clear();
        self.health = Default::default();
        self.scope_error = None;
    }
    pub async fn set_profile(&mut self, profile: Profile) {
        if self.config.profile == profile {
            return;
        }
        if self.scope_loading {
            let mut config = self.config.clone();
            config.profile = profile;
            if !config.refresh_explicit {
                config.refresh = Duration::from_millis(
                    crate::capabilities::envelope(profile).default_refresh_ms,
                );
            }
            self.replace_scope(config);
            self.sql = if profile == Profile::Minimal {
                Availability::DisabledByProfile
            } else {
                Availability::Unknown
            };
            return;
        }
        self.reset_scope();
        self.config.profile = profile;
        if !self.config.refresh_explicit {
            self.config.refresh =
                Duration::from_millis(crate::capabilities::envelope(profile).default_refresh_ms);
        }
        self.scheduler = Scheduler::with_registry(
            self.budgets.clone(),
            self.config.refresh,
            0x243f6a8885a308d3,
            profile,
            self.config.show_query_text,
        );
        self.store.enable_sources(
            collectors::plans(&self.config, &self.namespace)
                .into_iter()
                .map(|p| p.kind),
        );
        self.store
            .describe_sources(collectors::describe(&self.config, &self.namespace));
        self.source_stops = collectors::spawn_config(
            &mut self.tasks,
            self.client.clone(),
            &self.config,
            &self.namespace,
            self.store.scope,
            self.source_tx.clone(),
            &self.source_cancel,
        );
        let (sql, executor) = load_executor(&self.config, &self.client, &self.budgets).await;
        self.sql = sql;
        self.executor = executor;
    }
    /// Client/config resolution runs as owned cancellable work; input/rendering continue.
    pub fn replace_scope(&mut self, config: Config) {
        self.reset_scope();
        self.scope_loading = true;
        self.executor = None;
        self.sql = Availability::Unknown;
        self.config = config.clone();
        self.loader_cancel.cancel();
        self.loader_cancel = self.root.child_token();
        let cancel = self.loader_cancel.clone();
        let epoch = self.store.scope;
        let tx = self.scope_tx.clone();
        let budgets = self.budgets.clone();
        self.tasks.spawn(async move {
            let operation = async {
                let mut config = config;
                let (client, namespace) = kubernetes::client(&config, &budgets).await?;
                let operator = if config.operator_watch {
                    Some(crate::doctor::operator(client.clone(), &config).await)
                } else {
                    None
                };
                if config.operator_namespace.is_none() {
                    config.operator_namespace = operator
                        .as_ref()
                        .filter(|o| o.state == crate::doctor::State::Available)
                        .and_then(|o| o.namespace.clone());
                }
                let (sql, executor) = load_executor(&config, &client, &budgets).await;
                Ok(Prepared {
                    client,
                    namespace,
                    config,
                    sql,
                    executor,
                    operator,
                })
            };
            let result = tokio::select! {
                _ = cancel.cancelled() => return,
                result = tokio::time::timeout(Duration::from_secs(10), operation) => {
                    result.unwrap_or(Err(ErrorClass::Timeout))
                },
            };
            let _ = tx.send(ScopeReady { epoch, result }, 4096, &cancel).await;
        });
    }
    pub async fn refresh(&mut self) {
        let (sql, executor) = load_executor(&self.config, &self.client, &self.budgets).await;
        self.sql = sql;
        self.executor = executor;
        let now = self.clock.now();
        self.scheduler.refresh(now);
        self.pooler_scheduler.refresh(now);
        self.auxiliary_scheduler.refresh(now);
        if now.0.saturating_sub(self.log_recheck.0) >= self.budgets.backoff_floor_ms {
            self.log_recheck = now;
            let sources = self.log_active.keys().cloned().collect();
            self.close_logs();
            self.set_log_sources(sources, self.log_since_seconds);
        }
    }
    pub fn set_view(&mut self, view: View) {
        if self.view == View::Logs && view != View::Logs {
            self.close_logs();
        }
        if matches!(self.view, View::Connections | View::Waits)
            && !matches!(view, View::Connections | View::Waits)
        {
            for ticket in self
                .store
                .active
                .values()
                .filter(|t| t.class == Class::Deep)
            {
                if let Some(token) = self.probes.get(&ticket.request) {
                    token.cancel();
                }
            }
        }
        self.view = view;
        self.scheduler
            .set_diagnostic_view(matches!(view, View::Connections | View::Waits));
    }
    pub fn pending_targets(&self) -> usize {
        self.scheduler.len()
    }
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
    pub fn queue_bytes(&self) -> usize {
        self.source_rx.queued_bytes() + self.result_rx.queued_bytes() + self.log_rx.queued_bytes()
    }
    pub fn tick(&mut self) {
        let now = self.clock.now();
        self.max_queue_bytes = self.max_queue_bytes.max(self.queue_bytes());
        if let Some(packet) = self.scope_rx.try_recv()
            && packet.value.epoch == self.store.scope
        {
            self.scope_loading = false;
            match packet.value.result {
                Ok(prepared) => {
                    self.client = prepared.client;
                    self.auxiliary_loader =
                        crate::auxiliary::Loader::new(self.client.clone(), self.budgets.clone());
                    self.namespace = prepared.namespace;
                    self.config = prepared.config;
                    self.executor = prepared.executor;
                    self.sql = prepared.sql;
                    self.operator = prepared.operator;
                    self.scheduler = Scheduler::with_registry(
                        self.budgets.clone(),
                        self.config.refresh,
                        0x243f6a8885a308d3,
                        self.config.profile,
                        self.config.show_query_text,
                    );
                    self.store.enable_sources(
                        collectors::plans(&self.config, &self.namespace)
                            .into_iter()
                            .map(|p| p.kind),
                    );
                    self.store
                        .describe_sources(collectors::describe(&self.config, &self.namespace));
                    self.source_stops = collectors::spawn_config(
                        &mut self.tasks,
                        self.client.clone(),
                        &self.config,
                        &self.namespace,
                        self.store.scope,
                        self.source_tx.clone(),
                        &self.source_cancel,
                    );
                }
                Err(e) => {
                    self.scope_error = Some(e);
                    self.sql = Availability::Unavailable(e);
                }
            }
        }
        let start = Instant::now();
        for _ in 0..self.budgets.batch_events {
            let mut consumed = false;
            if let Some(packet) = self.source_rx.try_recv() {
                let kind = packet.value.kind;
                let changes = self.store.watch_at(packet.value, now);
                self.apply_changes(changes);
                consumed = true;
                if self
                    .store
                    .watches
                    .get(&kind)
                    .is_some_and(|w| w.status == SyncState::Failed(ErrorClass::Capacity))
                    && let Some(stop) = self.source_stops.get(&kind)
                {
                    stop.cancel();
                }
            }
            if let Some(packet) = self.result_rx.try_recv() {
                self.complete(packet.value, now);
                consumed = true;
            }
            if let Some(packet) = self.log_rx.try_recv() {
                let message = packet.value;
                if message.source.scope == self.store.scope && self.source_valid(&message.source) {
                    self.logs.accept(message, now, &self.budgets);
                } else {
                    self.logs.rejected += 1;
                }
                consumed = true;
            }
            if !consumed || start.elapsed() >= Duration::from_millis(self.budgets.batch_ms) {
                break;
            }
        }
        while let Some(result) = self.tasks.try_join_next() {
            if result.is_err() {
                self.cleanup_errors += 1;
            }
        }
        let invalid: Vec<_> = self
            .log_active
            .keys()
            .filter(|s| !self.source_valid(s))
            .cloned()
            .collect();
        for key in invalid {
            if let Some((_, stop)) = self.log_active.remove(&key) {
                stop.cancel();
            }
        }
        self.logs.enforce(now, &self.budgets);
        if now.0 / 1000 != self.last_second {
            self.last_second = now.0 / 1000;
            self.store.utc_ms = Some(chrono::Utc::now().timestamp_millis());
            self.store.expire_events(
                now,
                chrono::Utc::now().timestamp_millis(),
                self.budgets.event_retention_ms,
            );
        }
        if self.view == View::Logs {
            self.sync_log_events();
        }
        self.store.prune_pooler_samples();
        self.pooler_scheduler
            .reconcile(&self.store, now, self.budgets.pending_probes);
        let poolers_enabled = !self.scope_loading
            && self.config.pooler_stats
            && self.config.profile != Profile::Minimal
            && matches!(self.view, View::Poolers | View::Connections);
        for (ticket, stop) in self.pooler_stops.values() {
            if !poolers_enabled || !crate::pooler::valid(&self.store, ticket) {
                stop.cancel();
            }
        }
        self.health.update(&self.store, now, &self.budgets);
        self.scheduler
            .set_diagnostic_view(matches!(self.view, View::Connections | View::Waits));
        if self.last_reconcile != Some((self.store.revision, now.0 / 1000, self.view)) {
            self.scheduler.reconcile(&self.store, now);
            self.last_reconcile = Some((self.store.revision, now.0 / 1000, self.view));
        }
        if !self.scope_loading
            && self.probes.is_empty()
            && let Some(executor) = &self.executor
            && let Some(ticket) = self.scheduler.admit(&self.store, now)
            && let Some(bundle) = self.scheduler.bundle(&self.store, &ticket, now)
            && let Ok(permit) = self.permits.clone().try_acquire_owned()
            && self.store.begin_probe(ticket.clone())
        {
            let cancel = self.root.child_token();
            self.probes.insert(ticket.request, cancel.clone());
            let executor = executor.clone();
            let tx = self.result_tx.clone();
            let root = self.root.clone();
            let role = self.config.db_role.clone();
            let profile = self.config.profile;
            let result_limit = self.budgets.result_bytes;
            self.tasks.spawn(async move {
                let mut result = executor
                    .execute_bundle(&ticket.pod.key, bundle, &cancel)
                    .await
                    .and_then(|bytes| {
                        if ticket.class == Class::Fast {
                            probes::parse(&bytes, &role).map(|s| Collected::Fast(Box::new(s)))
                        } else {
                            probes::registry::parse(bundle, &bytes, &role, profile)
                                .map(|s| Collected::Diagnostic(Box::new(s)))
                        }
                    });
                if result.as_ref().is_ok_and(|s|match s {Collected::Fast(s)=>s.bytes()+512, Collected::Diagnostic(s)=>s.bytes()+512}>result_limit) { result=Err(ErrorClass::Capacity); }
                let bytes = result.as_ref().map_or(512, |s| match s {
                    Collected::Fast(s) => s.bytes() + 512,
                    Collected::Diagnostic(s) => s.bytes() + 512,
                });
                let _ = tx
                    .send(
                        Completion::Sql(SqlCompletion {
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
        self.admit_pooler(now);
        self.admit_auxiliary(now);
    }
    pub fn log_drop_count(&self) -> u64 {
        self.log_dropped.load(Ordering::Relaxed)
    }
    pub async fn shutdown(mut self) -> u64 {
        self.root.cancel();
        let grace = tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(result) = self.tasks.join_next().await {
                if result.is_err() {
                    self.cleanup_errors += 1;
                }
            }
        })
        .await;
        if grace.is_err() {
            self.tasks.abort_all();
            while self.tasks.join_next().await.is_some() {}
            self.cleanup_errors += 1;
        }
        self.cleanup_errors
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.root.cancel();
        self.tasks.abort_all();
    }
}
async fn load_executor(
    config: &Config,
    client: &kube::Client,
    budgets: &Budgets,
) -> (Availability, Option<PodExec>) {
    if config.profile == Profile::Minimal {
        return (Availability::DisabledByProfile, None);
    }
    let Some(path) = &config.password_file else {
        return (Availability::Unavailable(ErrorClass::Authentication), None);
    };
    match probes::password_file(path).await {
        Ok(password) => (
            Availability::Available,
            Some(PodExec::new(
                client.clone(),
                password,
                config.db_role.clone(),
                budgets.clone(),
            )),
        ),
        Err(e) => (Availability::Unavailable(e), None),
    }
}
