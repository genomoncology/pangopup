use super::*;
use axum::http::StatusCode;
use pangopup_core::{
    DnaBase, EnsemblGeneId, GencodeGeneId, GeneScoreRecord, GenomicPosition, Grch38Contig,
    LookupProvenance, LookupResult, PangolinScore, PrecomputedProvenance, ReferenceError,
    ReferenceProvenance, ReferenceProvider, RelativePosition, ScoreMagnitude,
};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, mpsc};
use tower::ServiceExt;

struct FakeLookup;
impl LookupBackend for FakeLookup {
    fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, Failure> {
        if request.variant().position().get() == 1 {
            let score = PangolinScore::new(
                ScoreMagnitude::new(1).expect("score"),
                RelativePosition::new(0).expect("position"),
                ScoreMagnitude::new(2).expect("score"),
                RelativePosition::new(0).expect("position"),
            );
            let gene = EnsemblGeneId::from_str("ENSG00000000001").expect("gene");
            return Ok(RouteDecision::Authoritative(RoutedResult::Precomputed {
                variant: request.variant().clone(),
                result: LookupResult::new(
                    vec![GeneScoreRecord::new(gene, score)],
                    Vec::new(),
                    LookupProvenance::Precomputed(PrecomputedProvenance::new(
                        "sha256:test".to_owned(),
                        "doi:test".to_owned(),
                        "md5".to_owned(),
                        true,
                        50,
                    )),
                ),
            }));
        }
        Ok(RouteDecision::ModelRequired(
            pangopup_engine::LookupFirstRouter::new(EmptyProvider)
                .inspect(request)
                .expect("empty lookup")
                .model_required(),
        ))
    }
}

struct PrecomputedShapeLookup;

impl LookupBackend for PrecomputedShapeLookup {
    fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, Failure> {
        let gene = EnsemblGeneId::from_str("ENSG00000000001").expect("gene");
        let records = (request.variant().position().get() == 12)
            .then(|| {
                GeneScoreRecord::new(
                    gene,
                    PangolinScore::new(
                        ScoreMagnitude::new(1).expect("score"),
                        RelativePosition::new(0).expect("position"),
                        ScoreMagnitude::new(2).expect("score"),
                        RelativePosition::new(0).expect("position"),
                    ),
                )
            })
            .into_iter()
            .collect();
        let ambiguities = (request.variant().position().get() >= 11)
            .then(|| pangopup_core::SourceReferenceAmbiguity::new(gene, DnaBase::A))
            .into_iter()
            .collect();
        Ok(RouteDecision::Authoritative(RoutedResult::Precomputed {
            variant: request.variant().clone(),
            result: LookupResult::new(
                records,
                ambiguities,
                LookupProvenance::Precomputed(PrecomputedProvenance::new(
                    "sha256:test".to_owned(),
                    "doi:test".to_owned(),
                    "md5".to_owned(),
                    true,
                    50,
                )),
            ),
        }))
    }
}

struct SignalingLookup {
    inspected: mpsc::Sender<u32>,
}

impl LookupBackend for SignalingLookup {
    fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, Failure> {
        self.inspected
            .send(request.variant().position().get())
            .expect("inspection observer");
        FakeLookup.inspect(request)
    }
}

struct EmptyProvider;
impl pangopup_core::ScoreProvider for EmptyProvider {
    fn lookup(
        &self,
        _snv: pangopup_core::Grch38Snv,
        _gene: Option<EnsemblGeneId>,
    ) -> Result<LookupResult, pangopup_core::LookupError> {
        Ok(LookupResult::new(
            Vec::new(),
            Vec::new(),
            LookupProvenance::Precomputed(PrecomputedProvenance::new(
                "sha256:test".to_owned(),
                "doi:test".to_owned(),
                "md5".to_owned(),
                true,
                50,
            )),
        ))
    }
}

trait DecisionTestExt {
    fn model_required(self) -> pangopup_engine::ModelRequired;
}
impl DecisionTestExt for RouteDecision {
    fn model_required(self) -> pangopup_engine::ModelRequired {
        match self {
            RouteDecision::ModelRequired(value) => value,
            _ => panic!("expected model"),
        }
    }
}

struct EmptyCache;
impl CacheReader for EmptyCache {
    fn get(
        &mut self,
        _key: &CacheKey,
    ) -> Result<Option<Vec<ModelGeneScoreRecord>>, pangopup_cache::CacheError> {
        Ok(None)
    }
}

struct CountingCache {
    gets: Arc<AtomicUsize>,
}

impl CacheReader for CountingCache {
    fn get(
        &mut self,
        _key: &CacheKey,
    ) -> Result<Option<Vec<ModelGeneScoreRecord>>, pangopup_cache::CacheError> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}

struct FakeWorker {
    calls: Arc<AtomicUsize>,
}

struct RecordWorker;

impl WorkerBackend for RecordWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: model_records(),
            provenance: provenance(),
        })
    }
}
impl WorkerBackend for FakeWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: Vec::new(),
            provenance: provenance(),
        })
    }
}

struct BlockingWorker {
    calls: Arc<AtomicUsize>,
    entered: mpsc::Sender<u32>,
    release: Arc<(Mutex<bool>, Condvar)>,
    order: Arc<Mutex<Vec<u32>>>,
}

impl WorkerBackend for BlockingWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        let position = pending.variant().position().get();
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.order.lock().expect("order").push(position);
        let _sent = self.entered.send(position);
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().expect("release");
        while !*released {
            released = ready.wait(released).expect("release wait");
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: Vec::new(),
            provenance: provenance(),
        })
    }
}

struct ControlledPanicWorker {
    entered: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

impl WorkerBackend for ControlledPanicWorker {
    fn complete(
        &mut self,
        _pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        let _ = self.entered.send(());
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().expect("release");
        while !*released {
            released = ready.wait(released).expect("release wait");
        }
        panic!("deterministic controlled worker loss")
    }
}

struct FailSecondWorker {
    calls: Arc<AtomicUsize>,
}

struct FailingWorker {
    failure: WorkerFailure,
}

impl WorkerBackend for FailingWorker {
    fn complete(
        &mut self,
        _pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        Err(match &self.failure {
            WorkerFailure::Rejected => WorkerFailure::Rejected,
            WorkerFailure::Operational(failure) => WorkerFailure::Operational(Failure {
                code: failure.code,
                message: failure.message.clone(),
                exit: failure.exit,
                details: failure.details.clone(),
            }),
        })
    }
}

struct RejectAtWorker {
    position: u32,
    calls: Arc<AtomicUsize>,
}

impl WorkerBackend for RejectAtWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if pending.variant().position().get() == self.position {
            return Err(WorkerFailure::Rejected);
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: Vec::new(),
            provenance: provenance(),
        })
    }
}

struct FakeCompletion {
    calls: Arc<AtomicUsize>,
    provenance: ModelProvenance,
    lock_probe: Option<PathBuf>,
}

struct BlockingCompletion {
    entered: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
    provenance: ModelProvenance,
}

struct ProductionBlockingCompletion {
    calls: Arc<AtomicUsize>,
    entered: mpsc::Sender<u32>,
    release: Arc<(Mutex<bool>, Condvar)>,
    order: Arc<Mutex<Vec<u32>>>,
    provenance: ModelProvenance,
}

impl ModelCompletion for ProductionBlockingCompletion {
    fn complete_unfiltered(
        &mut self,
        pending: PendingModel,
    ) -> Result<RoutedResult, ModelFallbackError> {
        let position = pending.variant().position().get();
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.order.lock().expect("order").push(position);
        let _ = self.entered.send(position);
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().expect("release");
        while !*released {
            released = ready.wait(released).expect("release wait");
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: model_records(),
            provenance: self.provenance.clone(),
        })
    }

    fn provenance(&self) -> &ModelProvenance {
        &self.provenance
    }
}

impl ModelCompletion for BlockingCompletion {
    fn complete_unfiltered(
        &mut self,
        pending: PendingModel,
    ) -> Result<RoutedResult, ModelFallbackError> {
        let _ = self.entered.send(());
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().expect("release");
        while !*released {
            released = ready.wait(released).expect("release wait");
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: model_records(),
            provenance: self.provenance.clone(),
        })
    }

    fn provenance(&self) -> &ModelProvenance {
        &self.provenance
    }
}

impl ModelCompletion for FakeCompletion {
    fn complete_unfiltered(
        &mut self,
        pending: PendingModel,
    ) -> Result<RoutedResult, ModelFallbackError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(path) = &self.lock_probe {
            let connection = rusqlite::Connection::open(path).expect("open lock probe");
            connection
                .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
                .expect("cache has no transaction across inference");
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: model_records(),
            provenance: self.provenance.clone(),
        })
    }

    fn provenance(&self) -> &ModelProvenance {
        &self.provenance
    }
}

fn model_records() -> Vec<ModelGeneScoreRecord> {
    vec![ModelGeneScoreRecord::new(
        GencodeGeneId::from_str("ENSG00000000001.1").expect("GENCODE gene"),
        PangolinScore::new(
            ScoreMagnitude::new(12).expect("gain"),
            RelativePosition::new(3).expect("gain position"),
            ScoreMagnitude::new(34).expect("loss"),
            RelativePosition::new(-4).expect("loss position"),
        ),
        Vec::new(),
    )]
}

fn private_cache(temp: &tempfile::TempDir) -> (PathBuf, ModelResultCache) {
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let path = temp.path().join("cache.sqlite3");
    let cache = ModelResultCache::open_explicit(&path, EntryLimit::Unlimited).expect("cache");
    (path, cache)
}

fn pending_at(position: u32) -> PendingModel {
    PendingModel::Explicit(ExplicitModelRequest::new(RouteRequest::new(
        super::super::parse_variant(&format!("GRCh38:chr1:{position}:A:C")).expect("variant"),
        None,
    )))
}
impl WorkerBackend for FailSecondWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == 2 {
            return Err(WorkerFailure::Operational(Failure {
                code: "MODEL_SCORING",
                message: "test failure".to_owned(),
                exit: 1,
                details: None,
            }));
        }
        Ok(RoutedResult::Modeled {
            variant: pending.variant().clone(),
            records: Vec::new(),
            provenance: provenance(),
        })
    }
}

fn provenance() -> ModelProvenance {
    ModelProvenance::new(
        format!("sha256:{}", "1".repeat(64)),
        "model-v1".to_owned(),
        pangopup_core::ReferenceProvenance::new(
            format!("sha256:{}", "2".repeat(64)),
            "reference-v1".to_owned(),
            "format-v1".to_owned(),
            "GRCh38.p14".to_owned(),
            "GCF_000001405.40".to_owned(),
            format!("sha256:{}", "3".repeat(64)),
        ),
        1,
        format!("sha256:{}", "4".repeat(64)),
    )
}

fn identity() -> CacheIdentity {
    identity_with_policy("sequential:1/1")
}

fn scoring_identity() -> ActiveScoringIdentity {
    let profile = pangopup_assets::production_runtime_profile();
    let bytes = pangopup_assets::canonical_runtime_profile_bytes(&profile).expect("profile bytes");
    let runtime_id = pangopup_assets::runtime_profile_id(&bytes).expect("runtime identity");
    ActiveScoringIdentityPreimage::new(
        env!("CARGO_PKG_VERSION"),
        &runtime_id,
        CpuPolicy::SEQUENTIAL_1_1,
    )
    .identity()
}

fn identity_with_policy(policy: &str) -> CacheIdentity {
    CacheIdentity::new(
        &format!("sha256:{}", "1".repeat(64)),
        "model-v1",
        "singleton",
        policy,
        &format!("sha256:{}", "2".repeat(64)),
        "reference-v1",
        &format!("sha256:{}", "3".repeat(64)),
        1,
        &format!("sha256:{}", "4".repeat(64)),
    )
    .expect("identity")
}

fn model_job(positions: &[u32]) -> ModelJob {
    let (response, _waiting) = oneshot::channel();
    ModelJob {
        items: positions
            .iter()
            .enumerate()
            .map(|(output_index, position)| {
                let pending = pending_at(*position);
                JobItem {
                    output_index,
                    key: CacheKey::new(pending.variant(), identity()),
                    pending,
                }
            })
            .collect(),
        response,
        slot: JobSlot::Unassigned,
    }
}

fn idle_dispatcher(workers: usize, capacity: usize) -> (Dispatcher, Receiver<ModelJob>) {
    let (sender, receiver) = bounded(capacity);
    let dispatcher = Dispatcher {
        state: Arc::new(Mutex::new(DispatchState {
            sender: Some(sender),
            readiness: READY,
            running_jobs: 0,
            running: 0,
            queued: 0,
        })),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers,
        threads: 1,
        queue_capacity: capacity,
    };
    (dispatcher, receiver)
}

#[test]
fn admission_counts_running_and_queued_variant_units_at_the_exact_boundary() {
    let (dispatcher, receiver) = idle_dispatcher(2, 10);
    assert!(dispatcher.admit(model_job(&[2, 3, 4, 5])).is_ok());
    assert!(dispatcher.admit(model_job(&[6, 7, 8])).is_ok());
    assert!(dispatcher.admit(model_job(&[9, 10, 11])).is_ok());
    assert!(matches!(
        dispatcher.admit(model_job(&[12])),
        Err(AdmissionError::Full { .. })
    ));
    let snapshot = dispatcher.snapshot();
    assert_eq!(snapshot.running, 7);
    assert_eq!(snapshot.queued, 3);
    assert_eq!(
        snapshot.running + snapshot.queued,
        dispatcher.queue_capacity
    );
    assert_eq!(dispatcher.state.lock().expect("state").running_jobs, 2);
    drop(receiver);
}

#[test]
fn equal_model_work_has_equal_admission_across_request_groupings() {
    let (grouped, grouped_receiver) = idle_dispatcher(1, 10);
    assert!(
        grouped
            .admit(model_job(&[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]))
            .is_ok()
    );
    assert!(matches!(
        grouped.admit(model_job(&[12])),
        Err(AdmissionError::Full { .. })
    ));

    let (split, split_receiver) = idle_dispatcher(1, 10);
    assert!(split.admit(model_job(&[2, 3])).is_ok());
    assert!(split.admit(model_job(&[4, 5, 6])).is_ok());
    assert!(split.admit(model_job(&[7, 8, 9, 10, 11])).is_ok());
    assert!(matches!(
        split.admit(model_job(&[12])),
        Err(AdmissionError::Full { .. })
    ));
    assert_eq!(grouped.snapshot().running + grouped.snapshot().queued, 10);
    assert_eq!(split.snapshot().running + split.snapshot().queued, 10);
    drop(grouped_receiver);
    drop(split_receiver);
}

#[test]
fn default_model_capacity_is_twenty_uncached_variants() {
    let options = parse_options(&[]).expect("default options");
    assert_eq!(options.queue_capacity, 20);

    let (dispatcher, receiver) = idle_dispatcher(1, options.queue_capacity);
    assert!(
        dispatcher
            .admit(model_job(&[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]))
            .is_ok()
    );
    assert!(
        dispatcher
            .admit(model_job(&[12, 13, 14, 15, 16, 17, 18, 19, 20, 21]))
            .is_ok()
    );
    assert!(matches!(
        dispatcher.admit(model_job(&[22])),
        Err(AdmissionError::Full { .. })
    ));
    drop(receiver);
}

#[test]
fn retry_delay_uses_admitted_units_and_rounds_up_without_worker_scaling() {
    assert_eq!(retry_after_seconds(0), 1);
    assert_eq!(retry_after_seconds(1), 11);
    assert_eq!(retry_after_seconds(5), 52);
    assert_eq!(retry_after_seconds(20), 205);
}

#[test]
fn ordinary_errors_do_not_claim_queue_retry_guidance() {
    for (status, code, message) in [
        (
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid request",
        ),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "MODEL_REJECTED",
            "scoring failed",
        ),
        (StatusCode::NOT_FOUND, "NOT_FOUND", "route not found"),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "SCORING_FAILED",
            "scoring failed",
        ),
    ] {
        let response = service_error(status, code, message);
        assert!(!response.headers().contains_key(header::RETRY_AFTER));
    }
}

#[test]
fn failed_send_releases_the_exact_job_weight() {
    let (dispatcher, receiver) = idle_dispatcher(1, 3);
    drop(receiver);
    assert!(matches!(
        dispatcher.admit(model_job(&[2, 3, 4])),
        Err(AdmissionError::Unavailable)
    ));
    let snapshot = dispatcher.snapshot();
    assert_eq!(snapshot.readiness, FAILED);
    assert_eq!(snapshot.running, 0);
    assert_eq!(snapshot.queued, 0);
    assert_eq!(dispatcher.state.lock().expect("state").running_jobs, 0);
}

#[test]
fn worker_loss_releases_every_drained_job_weight() {
    let (sender, receiver) = bounded(5);
    let mut running = model_job(&[2, 3, 4]);
    running.slot = JobSlot::Running;
    let mut queued = model_job(&[5, 6]);
    queued.slot = JobSlot::Queued;
    sender.send(running).expect("running slot");
    sender.send(queued).expect("queued slot");
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender),
        readiness: READY,
        running_jobs: 1,
        running: 3,
        queued: 2,
    }));

    fail_workers(&receiver, &state);

    let state = state.lock().expect("state");
    assert_eq!(state.readiness, FAILED);
    assert_eq!(state.running_jobs, 0);
    assert_eq!(state.running, 0);
    assert_eq!(state.queued, 0);
}

#[test]
fn production_worker_rechecks_sqlite_uses_exact_policy_and_holds_no_lock_during_model() {
    let temp = tempfile::tempdir().expect("temp");
    let (path, mut cache) = private_cache(&temp);
    let cached = pending_at(2);
    let cached_key = CacheKey::new(cached.variant(), identity());
    cache
        .put(&cached_key, &model_records())
        .expect("seed cache");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut worker = ProductionWorker {
        fallback: Box::new(FakeCompletion {
            calls: Arc::clone(&calls),
            provenance: provenance(),
            lock_probe: Some(path.clone()),
        }),
        cache,
    };
    let cached_result = worker.complete(&cached, &cached_key).expect("cache hit");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "worker rechecks SQLite");
    assert!(matches!(cached_result, RoutedResult::Modeled { .. }));

    let policy_miss = pending_at(3);
    let wrong_policy_key = CacheKey::new(
        policy_miss.variant(),
        identity_with_policy("sequential:2/1"),
    );
    worker
        .cache
        .put(&wrong_policy_key, &model_records())
        .expect("seed other policy");
    let actual_key = CacheKey::new(policy_miss.variant(), identity());
    worker
        .complete(&policy_miss, &actual_key)
        .expect("model under actual policy");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "other CPU policy must miss"
    );
    worker
        .complete(&policy_miss, &actual_key)
        .expect("new exact-policy hit");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn queued_production_job_rechecks_sqlite_after_admission_before_inference() {
    let temp = tempfile::tempdir().expect("temp");
    let (_path, worker_cache) = private_cache(&temp);
    let mut seeder =
        ModelResultCache::open_explicit(&temp.path().join("cache.sqlite3"), EntryLimit::Unlimited)
            .expect("seeder");
    let pending = pending_at(6);
    let key = CacheKey::new(pending.variant(), identity());
    let (reply, waiting) = oneshot::channel();
    let (sender, receiver) = bounded(1);
    sender
        .send(ModelJob {
            items: vec![JobItem {
                output_index: 0,
                pending,
                key: key.clone(),
            }],
            response: reply,
            slot: JobSlot::Queued,
        })
        .expect("admitted queue");
    seeder.put(&key, &model_records()).expect("earlier fill");
    let calls = Arc::new(AtomicUsize::new(0));
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running_jobs: 0,
        running: 0,
        queued: 1,
    }));
    let join = spawn_worker(
        Box::new(ProductionWorker {
            fallback: Box::new(FakeCompletion {
                calls: Arc::clone(&calls),
                provenance: provenance(),
                lock_probe: None,
            }),
            cache: worker_cache,
        }),
        receiver,
        Arc::clone(&state),
    );
    assert!(waiting.blocking_recv().expect("reply").is_ok());
    state.lock().expect("state").sender.take();
    drop(sender);
    join.join().expect("worker joins");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn production_worker_returns_valid_result_when_sqlite_write_is_busy() {
    let temp = tempfile::tempdir().expect("temp");
    let (path, cache) = private_cache(&temp);
    let blocker = rusqlite::Connection::open(&path).expect("blocker");
    blocker
        .execute_batch("BEGIN IMMEDIATE")
        .expect("write lock");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut worker = ProductionWorker {
        fallback: Box::new(FakeCompletion {
            calls: Arc::clone(&calls),
            provenance: provenance(),
            lock_probe: None,
        }),
        cache,
    };
    let pending = pending_at(7);
    let key = CacheKey::new(pending.variant(), identity());
    let result = worker.complete(&pending, &key).expect("valid model result");
    assert!(matches!(result, RoutedResult::Modeled { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    blocker.execute_batch("ROLLBACK").expect("unlock");
}

#[test]
fn production_worker_running_disconnect_still_writes_through_sqlite() {
    let temp = tempfile::tempdir().expect("temp");
    let (_path, cache) = private_cache(&temp);
    let pending = pending_at(8);
    let key = CacheKey::new(pending.variant(), identity());
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (reply, waiting) = oneshot::channel();
    let (sender, receiver) = bounded(1);
    sender
        .send(ModelJob {
            items: vec![JobItem {
                output_index: 0,
                pending,
                key: key.clone(),
            }],
            response: reply,
            slot: JobSlot::Queued,
        })
        .expect("queue");
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running_jobs: 0,
        running: 0,
        queued: 1,
    }));
    let join = spawn_worker(
        Box::new(ProductionWorker {
            fallback: Box::new(BlockingCompletion {
                entered: entered_tx,
                release: Arc::clone(&release),
                provenance: provenance(),
            }),
            cache,
        }),
        receiver,
        Arc::clone(&state),
    );
    entered_rx.recv().expect("model started");
    drop(waiting);
    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    while state.lock().expect("state").running != 0 {
        thread::yield_now();
    }
    state.lock().expect("state").sender.take();
    drop(sender);
    join.join().expect("worker joins");
    let mut cache =
        ModelResultCache::open_explicit(&temp.path().join("cache.sqlite3"), EntryLimit::Unlimited)
            .expect("reopen cache");
    assert_eq!(cache.get(&key).expect("cache read"), Some(model_records()));
}

fn state() -> (AppState, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![Box::new(FakeWorker {
            calls: Arc::clone(&calls),
        })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    (state, calls)
}

async fn body(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body")
        .to_vec()
}

#[tokio::test]
async fn health_status_and_route_errors_are_exact_json_lines() {
    let (state, _) = state();
    let router = app(state);
    let response = router
        .clone()
        .oneshot(Request::get("/livez").body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body(response).await, b"{\"status\":\"live\"}\n");
    let response = router
        .clone()
        .oneshot(
            Request::post("/livez")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"METHOD_NOT_ALLOWED\",\"message\":\"method not allowed\"}}\n"
    );
    for uri in ["/livez", "/readyz", "/v1/status", "/v1/score"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri(uri)
                    .body(Body::empty())
                    .expect("HEAD request"),
            )
            .await
            .expect("HEAD response");
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{uri}");
        assert_eq!(
            response.headers()[header::ALLOW],
            if uri == "/v1/score" { "POST" } else { "GET" }
        );
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "71");
        assert!(
            body(response).await.is_empty(),
            "HEAD has no wire body: {uri}"
        );
    }
    let response = router
        .clone()
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let mut status: Value = serde_json::from_slice(&body(response).await).expect("status JSON");
    assert!(status.get("request_contract").is_some());
    status
        .as_object_mut()
        .expect("status object")
        .remove("request_contract");
    assert_eq!(
        status,
        json!({
            "version": "0.3.0",
            "readiness": "ready",
            "scoring_identity": "sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f",
            "assets": {"snv_bundle_id":"snv","model_bundle_id":"model","reference_bundle_id":"reference","mask_sha256":"mask"},
            "routes": {"lookup":true,"model":true,"model_only":true},
            "model": {"effective_cpu_policy":"sequential:1/1","workers":1,"threads_per_worker":1,"running":0,"queued":0,"queue_capacity":2,"work_unit":"uncached_model_variant"}
        })
    );
    let response = router
        .clone()
        .oneshot(
            Request::get("/missing")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"NOT_FOUND\",\"message\":\"route not found\"}}\n"
    );
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/unknown")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(body(response).await.is_empty());
}

#[tokio::test]
async fn status_and_every_returned_score_item_share_one_scoring_identity() {
    let (state, _) = state();
    let router = app(state);
    let status = router
        .clone()
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status response");
    let status: Value = serde_json::from_slice(&body(status).await).expect("status JSON");
    let identity = status["scoring_identity"]
        .as_str()
        .expect("status scoring identity");

    let scored = router
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:1:A:C","GRCh38:chr1:2:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("score response");
    let scored: Value = serde_json::from_slice(&body(scored).await).expect("score JSON");
    assert_eq!(scored["results"][0]["scoring_identity"], identity);
    assert_eq!(scored["results"][1]["scoring_identity"], identity);
}

#[tokio::test]
async fn status_reports_the_request_contract_from_enforced_boundaries_and_parsers() {
    let (state, _) = state();
    let response = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status response");
    let status: Value = serde_json::from_slice(&body(response).await).expect("status JSON");
    let contract = &status["request_contract"];
    assert_eq!(contract["api_version"], "v1");
    assert_eq!(contract["route"], "/v1/score");
    assert_eq!(contract["content_type"], "application/json");
    assert_eq!(contract["max_body_bytes"], REQUEST_LIMITS.max_body_bytes);
    assert_eq!(contract["variants"]["min_items"], 1);
    assert_eq!(
        contract["variants"]["max_items"],
        REQUEST_LIMITS.max_variants
    );
    assert_eq!(
        contract["variants"]["max_uncached_model_items"],
        REQUEST_LIMITS.max_uncached_model_variants
    );
    assert_eq!(
        contract["variants"]["max_model_allele_bases"],
        pangopup_engine::MAX_MODEL_ALLELE_BASES
    );
    assert_eq!(
        contract["variants"]["max_exact_edit_sequence_bases"],
        pangopup_engine::MAX_EXACT_EDIT_SEQUENCE_BASES
    );
    assert_eq!(
        contract["variants"]["forms"],
        json!([
            "GRCh38:CONTIG:POS:REF:ALT",
            "GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE",
            "GRCh38:CONTIG:DEL:START:END:SEQUENCE"
        ])
    );
    let contigs = contract["variants"]["contigs"]
        .as_array()
        .expect("contig descriptors");
    assert_eq!(contigs.len(), 25);
    for descriptor in contigs {
        let canonical = descriptor["canonical"].as_str().expect("canonical contig");
        let expected = crate::parse_contig(canonical).expect("canonical parses");
        for accepted in descriptor["accepted"]
            .as_array()
            .expect("accepted spellings")
        {
            assert_eq!(
                crate::parse_contig(accepted.as_str().expect("contig spelling")),
                Some(expected)
            );
        }
    }
    assert_eq!(
        contigs.last().expect("mitochondrial descriptor"),
        &json!({
            "canonical": "chrM",
            "accepted": ["M", "MT", "chrM", "chrMT", "NC_012920.1"]
        })
    );
    assert_eq!(
        contract["gene_filter"],
        json!({
            "accepted_forms": [
                "ENSG###########",
                "ENSG###########.VERSION",
                "ENSG###########.VERSION_PAR_Y"
            ],
            "version_minimum": 1,
            "version_maximum": u32::MAX,
            "version_allows_leading_zero": false
        })
    );
    assert_eq!(
        contract["model_only"],
        json!({"type": "boolean", "optional": true})
    );

    let accessions = [
        "NC_000001.11",
        "NC_000002.12",
        "NC_000003.12",
        "NC_000004.12",
        "NC_000005.10",
        "NC_000006.12",
        "NC_000007.14",
        "NC_000008.11",
        "NC_000009.12",
        "NC_000010.11",
        "NC_000011.10",
        "NC_000012.12",
        "NC_000013.11",
        "NC_000014.9",
        "NC_000015.10",
        "NC_000016.10",
        "NC_000017.11",
        "NC_000018.10",
        "NC_000019.10",
        "NC_000020.11",
        "NC_000021.9",
        "NC_000022.11",
        "NC_000023.11",
        "NC_000024.10",
    ];
    let mut expected_contigs = accessions
        .iter()
        .enumerate()
        .map(|(index, accession)| {
            let bare = if index < 22 {
                (index + 1).to_string()
            } else if index == 22 {
                "X".to_owned()
            } else {
                "Y".to_owned()
            };
            json!({
                "canonical": format!("chr{bare}"),
                "accepted": [bare.clone(), format!("chr{bare}"), accession]
            })
        })
        .collect::<Vec<_>>();
    expected_contigs.push(json!({
        "canonical": "chrM",
        "accepted": ["M", "MT", "chrM", "chrMT", "NC_012920.1"]
    }));
    assert_eq!(
        contract,
        &json!({
            "api_version": "v1",
            "route": "/v1/score",
            "content_type": "application/json",
            "max_body_bytes": 65536,
            "variants": {
                "min_items": 1,
                "max_items": 100,
                "max_uncached_model_items": 10,
                "model_work_unit": "uncached_model_variant",
                "assembly": "GRCh38",
                "max_model_allele_bases": 100,
                "max_exact_edit_sequence_bases": 99,
                "forms": [
                    "GRCh38:CONTIG:POS:REF:ALT",
                    "GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE",
                    "GRCh38:CONTIG:DEL:START:END:SEQUENCE"
                ],
                "contigs": expected_contigs
            },
            "gene_filter": {
                "accepted_forms": [
                    "ENSG###########",
                    "ENSG###########.VERSION",
                    "ENSG###########.VERSION_PAR_Y"
                ],
                "version_minimum": 1,
                "version_maximum": 4294967295_u64,
                "version_allows_leading_zero": false
            },
            "model_only": {"type": "boolean", "optional": true}
        })
    );

    for value in [
        "GRCh38:chr1:5051:A:C",
        "GRCh38:chr1:INS:5051:5052:A",
        "GRCh38:chr1:DEL:5051:5051:A",
    ] {
        assert!(crate::parse_variant_input(value).is_ok(), "{value}");
    }
    for value in [
        "ENSG00000000001",
        "ENSG00000000001.1",
        "ENSG00000000001.4294967295_PAR_Y",
    ] {
        assert!(crate::parse_gene_filter(value).is_ok(), "{value}");
    }

    let contig = Grch38Contig::autosome(1).expect("contig");
    let position = GenomicPosition::new(5_051).expect("position");
    let maximum_model = Grch38Variant::new(
        contig,
        position,
        "A",
        format!(
            "A{}",
            "C".repeat(
                contract["variants"]["max_model_allele_bases"]
                    .as_u64()
                    .expect("model allele limit") as usize
                    - 1
            )
        ),
    )
    .expect("maximum model allele");
    assert!(pangopup_engine::validate_model_request(&maximum_model).is_ok());
    let over_model = Grch38Variant::new(
        contig,
        position,
        "A",
        format!("A{}", "C".repeat(MAX_MODEL_ALLELE_BASES)),
    )
    .expect("over-limit literal remains parseable");
    assert!(pangopup_engine::validate_model_request(&over_model).is_err());
    let exact_limit = contract["variants"]["max_exact_edit_sequence_bases"]
        .as_u64()
        .expect("exact edit limit") as usize;
    assert!(
        pangopup_engine::Grch38ExactEdit::insertion(
            contig,
            position,
            GenomicPosition::new(5_052).expect("right"),
            "A".repeat(exact_limit),
        )
        .is_ok()
    );
    assert!(
        pangopup_engine::Grch38ExactEdit::insertion(
            contig,
            position,
            GenomicPosition::new(5_052).expect("right"),
            "A".repeat(exact_limit + 1),
        )
        .is_err()
    );
}

#[tokio::test]
async fn status_reports_the_effective_policy_bound_to_scoring_provenance() {
    let (mut state, _) = state();
    state.provenance = provenance().with_effective_cpu_policy("sequential:2/1");
    let response = app(state)
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status response");
    let status: Value = serde_json::from_slice(&body(response).await).expect("status JSON");
    assert_eq!(status["model"]["effective_cpu_policy"], "sequential:2/1");
}

#[test]
fn request_limit_refusals_are_rendered_from_the_enforced_values() {
    let limits = RequestLimits {
        max_body_bytes: 256,
        min_variants: 2,
        max_variants: 7,
        max_uncached_model_variants: 3,
    };
    assert_eq!(
        limits.variant_count_refusal(),
        "variants must contain between 2 and 7 values"
    );
    assert_eq!(
        limits.model_batch_refusal(),
        "request requires more than 3 uncached model variants"
    );
}

#[tokio::test]
async fn request_contract_is_stable_across_service_state_and_queue_occupancy() {
    let (state, _) = state();
    let ready = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("ready status");
    let ready: Value = serde_json::from_slice(&body(ready).await).expect("ready JSON");
    let expected = ready["request_contract"].clone();

    {
        let mut dispatch = state
            .dispatcher
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        dispatch.running = 1;
        dispatch.queued = 1;
    }
    let occupied = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("occupied status");
    let occupied: Value = serde_json::from_slice(&body(occupied).await).expect("occupied JSON");
    assert_eq!(occupied["request_contract"], expected);

    state.dispatcher.stop_admission();
    let draining = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("draining status");
    let draining: Value = serde_json::from_slice(&body(draining).await).expect("draining JSON");
    assert_eq!(draining["readiness"], "draining");
    assert_eq!(draining["request_contract"], expected);

    state
        .dispatcher
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .readiness = FAILED;
    let failed = app(state)
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("failed status");
    let failed: Value = serde_json::from_slice(&body(failed).await).expect("failed JSON");
    assert_eq!(failed["readiness"], "failed");
    assert_eq!(failed["request_contract"], expected);
}

#[tokio::test]
async fn all_precomputed_status_shapes_carry_the_service_identity() {
    let (mut state, _) = state();
    state.lookup = Arc::new(PrecomputedShapeLookup);
    let expected = state.scoring_identity.as_str().to_owned();
    let scored = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:10:A:C","GRCh38:chr1:11:A:C","GRCh38:chr1:12:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("score response");
    let scored: Value = serde_json::from_slice(&body(scored).await).expect("score JSON");
    assert_eq!(scored["results"][0]["status"], "not_found");
    assert_eq!(scored["results"][1]["status"], "ambiguous_source_reference");
    assert_eq!(scored["results"][2]["status"], "mixed");
    assert!(
        scored["results"]
            .as_array()
            .expect("results")
            .iter()
            .all(|result| result["scoring_identity"] == expected)
    );
}

#[tokio::test]
async fn score_requires_one_parsed_application_json_content_type_before_service_state() {
    let valid_body = r#"{"variants":["GRCh38:chr1:1:A:C"]}"#;
    for content_type in [
        "application/json",
        "Application/JSON",
        "application/json; charset=utf-8",
        "application/json; p=\"\"",
    ] {
        let (state, _) = state();
        let response = app(state)
            .oneshot(
                Request::post("/v1/score")
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(valid_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "{content_type}");
    }

    for content_type in [
        None,
        Some("application"),
        Some("application/json;"),
        Some("application/json;   "),
        Some("application/json; charset="),
        Some("text/plain"),
        Some("application/json-patch+json"),
    ] {
        let (state, _) = state();
        let mut request = Request::post("/v1/score");
        if let Some(value) = content_type {
            request = request.header(header::CONTENT_TYPE, value);
        }
        let response = app(state)
            .oneshot(request.body(Body::from(valid_body)).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            body(response).await,
            b"{\"error\":{\"code\":\"UNSUPPORTED_MEDIA_TYPE\",\"message\":\"content-type must be application/json\"}}\n"
        );
    }

    for second in ["application/json", "text/plain"] {
        let (state, _) = state();
        let mut request = Request::post("/v1/score")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(valid_body))
            .expect("request");
        request.headers_mut().append(
            header::CONTENT_TYPE,
            HeaderValue::from_str(second).expect("header"),
        );
        let response = app(state).oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    let (route_state, _) = state();
    let response = app(route_state.clone())
        .oneshot(
            Request::get("/v1/score")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let response = app(route_state)
        .oneshot(
            Request::post("/missing")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let (draining_state, _) = state();
    draining_state.dispatcher.stop_admission();
    let response = app(draining_state)
        .oneshot(
            Request::post("/v1/score")
                .body(Body::from(vec![b' '; REQUEST_LIMITS.max_body_bytes + 1]))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let (failed_state, _) = state();
    failed_state
        .dispatcher
        .state
        .lock()
        .expect("dispatcher state")
        .readiness = FAILED;
    let response = app(failed_state)
        .oneshot(
            Request::post("/v1/score")
                .body(Body::from(valid_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let (health_state, _) = state();
    for uri in ["/livez", "/readyz", "/v1/status"] {
        let response = app(health_state.clone())
            .oneshot(Request::get(uri).body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn score_accepts_mitochondrial_aliases_and_reports_canonical_contigs() {
    let (state, calls) = state();
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:MT:1:A:C","GRCh38:chrMT:1:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = serde_json::from_slice(&body(response).await).expect("JSON");
    assert_eq!(value["results"].as_array().expect("results").len(), 2);
    assert_eq!(value["results"][0]["contig"], "chrM");
    assert_eq!(value["results"][1]["contig"], "chrM");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn strict_http_media_type_grammar_handles_quoted_and_invalid_bytes() {
    for value in [
        br#"application/json; p="a\"b\\c""#.as_slice(),
        b"application/json; p=\"\x80\"".as_slice(),
    ] {
        assert!(is_application_json(value), "{value:?}");
    }
    for value in [
        b"application/json; p=\"a\\".as_slice(),
        b"application/json; p=\"\0\"".as_slice(),
        b"application/json; p=\"\x1f\"".as_slice(),
        b"application/json; p=\"\x7f\"".as_slice(),
        b"application/\x80json".as_slice(),
    ] {
        assert!(!is_application_json(value), "{value:?}");
    }
}

#[tokio::test]
async fn draining_keeps_liveness_and_status_but_stops_score_admission() {
    let (state, calls) = state();
    state.dispatcher.stop_admission();
    let router = app(state);
    let live = router
        .clone()
        .oneshot(Request::get("/livez").body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(live.status(), StatusCode::OK);
    let ready = router
        .clone()
        .oneshot(
            Request::get("/readyz")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
    let status_response = router
        .clone()
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status_bytes = body(status_response).await;
    assert!(
        String::from_utf8(status_bytes)
            .expect("UTF-8")
            .contains("\"readiness\":\"draining\"")
    );
    let scoring = router.oneshot(score_request(1)).await.expect("response");
    assert_eq!(scoring.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body(scoring).await,
        b"{\"error\":{\"code\":\"SHUTTING_DOWN\",\"message\":\"service is shutting down\"}}\n"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn score_preserves_order_and_model_only_bypasses_lookup() {
    let (state, calls) = state();
    let response = app(state.clone())
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:1:A:C","GRCh38:chr1:2:A:C"],"model_only":true}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = body(response).await;
    assert!(
        bytes.starts_with(
            b"{\"results\":[{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":1"
        )
    );
    assert!(bytes.ends_with(b"]}\n"));
    assert!(
        bytes
            .windows(b"\"effective_cpu_policy\":\"sequential:1/1\"".len())
            .any(|window| window == b"\"effective_cpu_policy\":\"sequential:1/1\"")
    );
    let value: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(value["results"][0]["position"], 1);
    assert_eq!(value["results"][1]["position"], 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(state.dispatcher.snapshot().running, 0);
    assert_eq!(state.dispatcher.snapshot().queued, 0);
}

#[tokio::test]
async fn modeled_http_success_is_pinned_as_one_complete_byte_fixture() {
    let (state, _) = state();
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C"],"model_only":true}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let expected = format!(
        "{{\"results\":[{{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":2,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"not_found\",\"records\":[],\"source_reference_ambiguities\":[],\"provenance\":{{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:{}\",\"model_profile\":\"model-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:{}\",\"reference_profile\":\"reference-v1\",\"reference_sequence_set_sha256\":\"sha256:{}\",\"mask_bytes\":1,\"mask_sha256\":\"sha256:{}\",\"masked\":true,\"window\":50}},\"scoring_identity\":\"sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f\"}}]}}\n",
        "1".repeat(64),
        "2".repeat(64),
        "3".repeat(64),
        "4".repeat(64),
    );
    assert_eq!(body(response).await, expected.as_bytes());
}

#[tokio::test]
async fn http_gene_filter_accepts_reported_identity_and_matches_its_stable_gene() {
    let (state, calls) = state();
    for gene in ["ENSG00000000001.1", "ENSG00000000001.1_PAR_Y"] {
        let response = score_bytes(
            &state,
            &Bytes::from(
                serde_json::to_vec(&json!({
                    "variants": ["GRCh38:chr1:1:A:C"],
                    "gene": gene,
                }))
                .expect("JSON"),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{gene}");
        let bytes = body(response).await;
        assert!(
            bytes
                .windows(b"\"gene\":\"ENSG00000000001\"".len())
                .any(|window| window == b"\"gene\":\"ENSG00000000001\""),
            "{gene}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn malformed_http_gene_filters_keep_the_stable_error_contract() {
    let (state, calls) = state();
    for gene in [
        "ENSG00000000001.",
        "ENSG00000000001.0",
        "ENSG00000000001.01",
        "ENSG00000000001.4294967296",
        "ENSG00000000001.1_PAR_X",
        "ENSG00000000001.1_PAR_Y_PAR_Y",
        "ENSG00000000001..1",
        "ENSG00000000001.1.extra",
    ] {
        let response = score_bytes(
            &state,
            &Bytes::from(
                serde_json::to_vec(&json!({
                    "variants": ["GRCh38:chr1:1:A:C"],
                    "gene": gene,
                }))
                .expect("JSON"),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{gene}");
        assert_eq!(
            body(response).await,
            b"{\"error\":{\"code\":\"INVALID_REQUEST\",\"message\":\"gene must be ENSG followed by 11 digits, optionally followed by .VERSION or .VERSION_PAR_Y; VERSION must be a nonzero decimal u32 without a leading zero\"}}\n",
            "{gene}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn malformed_closed_input_and_limits_fail_before_scoring() {
    let (boundary_state, _) = state();
    let (state, calls) = state();
    let status = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status response");
    let status: Value = serde_json::from_slice(&body(status).await).expect("status JSON");
    let max_model_items = status["request_contract"]["variants"]["max_uncached_model_items"]
        .as_u64()
        .expect("model item limit") as u32;
    for bytes in [
        br#"{"variants":[],"extra":true}"#.as_slice(),
        br#"{"variants":["bad"]}"#.as_slice(),
        br#"{"variants":["GRCh38:chr1:1:A:C"],"variants":[]}"#.as_slice(),
        br#"{"variants":["GRCh38:chr1:1:A:C"],"gene":null}"#.as_slice(),
        br#"{"variants":["GRCh38:chr1:1:A:C"],"model_only":null}"#.as_slice(),
    ] {
        let response = score_bytes(&state, &Bytes::copy_from_slice(bytes)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let at_model_limit = (2..(2 + max_model_items))
        .map(|position| format!("GRCh38:chr1:{position}:A:C"))
        .collect::<Vec<_>>();
    let response = score_bytes(
        &boundary_state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": at_model_limit})).expect("JSON")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let variants = (2..=(2 + max_model_items))
        .map(|position| format!("GRCh38:chr1:{position}:A:C"))
        .collect::<Vec<_>>();
    let response = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": variants})).expect("JSON")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body(response).await,
        format!(
            "{{\"error\":{{\"code\":\"MODEL_BATCH_TOO_LARGE\",\"message\":\"request requires more than {max_model_items} uncached model variants\"}}}}\n"
        )
        .as_bytes()
    );

    let variants = (0..101).map(|_| "GRCh38:chr1:1:A:C").collect::<Vec<_>>();
    let response = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": variants})).expect("JSON")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn request_body_is_bounded_before_json_parsing() {
    let (state, calls) = state();
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(vec![b' '; REQUEST_LIMITS.max_body_bytes + 1]))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"REQUEST_TOO_LARGE\",\"message\":\"request body is too large\"}}\n"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn exact_body_and_batch_boundaries_are_pinned() {
    let (state, _) = state();
    let status = app(state.clone())
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status response");
    let status: Value = serde_json::from_slice(&body(status).await).expect("status JSON");
    let max_body = status["request_contract"]["max_body_bytes"]
        .as_u64()
        .expect("body limit") as usize;
    let max_variants = status["request_contract"]["variants"]["max_items"]
        .as_u64()
        .expect("variant limit") as usize;
    let valid = br#"{"variants":["GRCh38:chr1:1:A:C"]}"#;
    let mut at_limit = valid.to_vec();
    at_limit.extend(std::iter::repeat_n(b' ', max_body - valid.len()));
    let accepted = app(state.clone())
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(at_limit))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(accepted.status(), StatusCode::OK);

    let one_over = app(state.clone())
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(vec![b' '; max_body + 1]))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(one_over.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let hundred = std::iter::repeat_n("GRCh38:chr1:1:A:C", max_variants).collect::<Vec<_>>();
    let accepted = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": hundred})).expect("JSON")),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let hundred_one =
        std::iter::repeat_n("GRCh38:chr1:1:A:C", max_variants + 1).collect::<Vec<_>>();
    let rejected = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": hundred_one})).expect("JSON")),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body(rejected).await,
        format!(
            "{{\"error\":{{\"code\":\"INVALID_REQUEST\",\"message\":\"variants must contain between 1 and {max_variants} values\"}}}}\n"
        )
        .as_bytes()
    );
}

fn score_request(position: u32) -> Request<Body> {
    Request::post("/v1/score")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            "{{\"variants\":[\"GRCh38:chr1:{position}:A:C\"]}}"
        )))
        .expect("request")
}

fn score_request_for(positions: &[u32]) -> Request<Body> {
    let variants = positions
        .iter()
        .map(|position| format!("GRCh38:chr1:{position}:A:C"))
        .collect::<Vec<_>>();
    Request::post("/v1/score")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"variants": variants})).expect("JSON"),
        ))
        .expect("request")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_workers_report_running_variant_units() {
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let backends = (0..2)
        .map(|_| {
            Box::new(BlockingWorker {
                calls: Arc::clone(&calls),
                entered: entered_tx.clone(),
                release: Arc::clone(&release),
                order: Arc::clone(&order),
            }) as Box<dyn WorkerBackend>
        })
        .collect();
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        backends,
        1,
        5,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let router = app(state.clone());
    let first = tokio::spawn(router.clone().oneshot(score_request_for(&[2, 3])));
    let second = tokio::spawn(router.clone().oneshot(score_request_for(&[4, 5, 6])));
    tokio::task::spawn_blocking(move || {
        entered_rx.recv().expect("first worker entered");
        entered_rx.recv().expect("second worker entered");
    })
    .await
    .expect("entered join");

    let status = router
        .clone()
        .oneshot(
            Request::get("/v1/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status");
    let status: Value = serde_json::from_slice(&body(status).await).expect("status JSON");
    assert_eq!(status["model"]["workers"], 2);
    assert_eq!(status["model"]["running"], 5);
    assert_eq!(status["model"]["queued"], 0);
    assert_eq!(status["model"]["queue_capacity"], 5);
    assert_eq!(status["model"]["work_unit"], "uncached_model_variant");
    let refused = router
        .oneshot(score_request(7))
        .await
        .expect("refused response");
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(refused.headers()[header::RETRY_AFTER], "52");
    assert_eq!(
        refused
            .headers()
            .get_all(header::RETRY_AFTER)
            .iter()
            .count(),
        1
    );
    assert_eq!(
        body(refused).await,
        b"{\"error\":{\"code\":\"MODEL_QUEUE_FULL\",\"message\":\"model queue is full\"}}\n"
    );

    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    assert_eq!(
        first.await.expect("join").expect("response").status(),
        StatusCode::OK
    );
    assert_eq!(
        second.await.expect("join").expect("response").status(),
        StatusCode::OK
    );
    assert_eq!(calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn request_heavier_than_capacity_has_no_retry_guidance() {
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![Box::new(FakeWorker {
            calls: Arc::new(AtomicUsize::new(0)),
        })],
        1,
        1,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let response = app(state)
        .oneshot(score_request_for(&[2, 3]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(!response.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"MODEL_QUEUE_FULL\",\"message\":\"model queue is full\"}}\n"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contended_completed_cache_hit_waits_outside_full_model_capacity() {
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (inspected_tx, inspected_rx) = mpsc::channel();
    let temp = tempfile::tempdir().expect("temp");
    let (_path, mut handler_cache) = private_cache(&temp);
    let cached = pending_at(3);
    handler_cache
        .put(
            &CacheKey::new(cached.variant(), identity()),
            &model_records(),
        )
        .expect("seed completed cache hit");
    let state = build_state(
        Arc::new(SignalingLookup {
            inspected: inspected_tx,
        }),
        Box::new(handler_cache),
        identity(),
        provenance(),
        vec![Box::new(BlockingWorker {
            calls: Arc::clone(&calls),
            entered: entered_tx,
            release: Arc::clone(&release),
            order,
        })],
        1,
        1,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let cache_gate = Arc::clone(&state.cache_gate)
        .acquire_owned()
        .await
        .expect("cache gate");
    let (reply, waiting) = oneshot::channel();
    let pending = pending_at(2);
    assert!(
        state
            .dispatcher
            .admit(ModelJob {
                items: vec![JobItem {
                    output_index: 0,
                    key: CacheKey::new(pending.variant(), identity()),
                    pending,
                }],
                response: reply,
                slot: JobSlot::Unassigned,
            })
            .is_ok(),
        "fill capacity"
    );
    tokio::task::spawn_blocking(move || entered_rx.recv().expect("worker entered"))
        .await
        .expect("entered join");
    assert_eq!(state.dispatcher.snapshot().running, 1);

    let cached_response = tokio::spawn(app(state.clone()).oneshot(score_request(3)));
    tokio::task::spawn_blocking(move || inspected_rx.recv().expect("request inspected"))
        .await
        .expect("inspection join");
    assert_eq!(state.dispatcher.snapshot().running, 1);
    assert!(
        !cached_response.is_finished(),
        "cache lookup waits for its gate"
    );
    drop(cache_gate);
    let cached_response = cached_response
        .await
        .expect("cached join")
        .expect("cached response");
    assert_eq!(cached_response.status(), StatusCode::OK);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "completed cache hit does not run inference"
    );
    assert_eq!(state.dispatcher.snapshot().running, 1);

    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    assert!(waiting.await.expect("worker response").is_ok());
    state.dispatcher.close();
    tokio::task::block_in_place(|| state.dispatcher.join_workers());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fixed_worker_and_waiting_capacity_are_fifo_while_lookup_bypasses_them() {
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let cache_path = temp.path().join("cache.sqlite3");
    let mut handler_cache =
        ModelResultCache::open_explicit(&cache_path, EntryLimit::Unlimited).expect("handler cache");
    let cached = pending_at(5);
    handler_cache
        .put(
            &CacheKey::new(cached.variant(), identity()),
            &model_records(),
        )
        .expect("seed completed cache hit");
    let worker_cache =
        ModelResultCache::open_explicit(&cache_path, EntryLimit::Unlimited).expect("worker cache");
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(handler_cache),
        identity(),
        provenance(),
        vec![Box::new(ProductionWorker {
            fallback: Box::new(ProductionBlockingCompletion {
                calls: Arc::clone(&calls),
                entered: entered_tx,
                release: Arc::clone(&release),
                order: Arc::clone(&order),
                provenance: provenance(),
            }),
            cache: worker_cache,
        })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let router = app(state.clone());
    let first = tokio::spawn(router.clone().oneshot(score_request(2)));
    tokio::task::spawn_blocking(move || entered_rx.recv().expect("first entered"))
        .await
        .expect("join");
    let second = tokio::spawn(router.clone().oneshot(score_request(3)));
    while state.dispatcher.snapshot().queued != 1 {
        tokio::task::yield_now().await;
    }
    let full = router
        .clone()
        .oneshot(score_request(4))
        .await
        .expect("full response");
    assert_eq!(full.status(), StatusCode::TOO_MANY_REQUESTS);

    let lookup = router
        .clone()
        .oneshot(score_request(1))
        .await
        .expect("lookup response");
    assert_eq!(lookup.status(), StatusCode::OK);
    let cached = router
        .oneshot(score_request(5))
        .await
        .expect("cache response");
    assert_eq!(cached.status(), StatusCode::OK);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "lookup and completed cache hit bypass model admission"
    );

    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    assert_eq!(
        first.await.expect("join").expect("response").status(),
        StatusCode::OK
    );
    assert_eq!(
        second.await.expect("join").expect("response").status(),
        StatusCode::OK
    );
    assert_eq!(*order.lock().expect("order"), vec![2, 3]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_panic_fans_out_to_running_and_queued_callers_then_closes_cleanly() {
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![Box::new(ControlledPanicWorker {
            entered: entered_tx,
            release: Arc::clone(&release),
        })],
        1,
        3,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let router = app(state.clone());
    let running = tokio::spawn(router.clone().oneshot(score_request(2)));
    tokio::task::spawn_blocking(move || entered_rx.recv().expect("running entered"))
        .await
        .expect("join");
    let queued_one = tokio::spawn(router.clone().oneshot(score_request(3)));
    let queued_two = tokio::spawn(router.clone().oneshot(score_request(4)));
    while state.dispatcher.snapshot().queued != 2 {
        tokio::task::yield_now().await;
    }
    let failure_dispatcher = state.dispatcher.clone();
    let shutdown_observer =
        tokio::spawn(async move { wait_worker_failure(&failure_dispatcher).await });
    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    for response in [running, queued_one, queued_two] {
        let response = response.await.expect("join").expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body(response).await,
            b"{\"error\":{\"code\":\"MODEL_WORKER_UNAVAILABLE\",\"message\":\"model worker failed\"}}\n"
        );
    }
    shutdown_observer.await.expect("failure starts shutdown");
    assert_eq!(state.dispatcher.snapshot().readiness, FAILED);
    assert_eq!(state.dispatcher.snapshot().queued, 0);
    let response = router.oneshot(score_request(5)).await.expect("response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let response = readyz(State(state.clone())).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    state.dispatcher.close();
    tokio::task::block_in_place(|| state.dispatcher.join_workers());
}

async fn worker_failure_response(failure: WorkerFailure) -> (StatusCode, Vec<u8>) {
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![Box::new(FailingWorker { failure })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let response = app(state)
        .oneshot(score_request(2))
        .await
        .expect("response");
    let status = response.status();
    (status, body(response).await)
}

#[tokio::test]
async fn http_reports_model_rejected_worker_failure() {
    let (status, body) = worker_failure_response(WorkerFailure::Rejected).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        b"{\"error\":{\"code\":\"MODEL_REJECTED\",\"message\":\"scoring failed\"}}\n"
    );
}

#[tokio::test]
async fn http_reports_model_scoring_worker_failure() {
    let (status, body) = worker_failure_response(WorkerFailure::Operational(Failure {
        code: "MODEL_SCORING",
        message: "model failed".to_owned(),
        exit: 1,
        details: None,
    }))
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        b"{\"error\":{\"code\":\"MODEL_SCORING\",\"message\":\"scoring failed\"}}\n"
    );
}

#[tokio::test]
async fn http_reports_model_cache_invalid_worker_failure() {
    let (status, body) = worker_failure_response(WorkerFailure::Operational(Failure {
        code: "MODEL_CACHE_INVALID",
        message: "cache invalid".to_owned(),
        exit: 1,
        details: None,
    }))
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        b"{\"error\":{\"code\":\"MODEL_CACHE_INVALID\",\"message\":\"scoring failed\"}}\n"
    );
}

#[tokio::test]
async fn operational_failure_is_not_reclassified_by_its_public_code() {
    let (status, body) = worker_failure_response(WorkerFailure::Operational(Failure {
        code: "MODEL_REJECTED",
        message: "test operational failure".to_owned(),
        exit: 1,
        details: None,
    }))
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        b"{\"error\":{\"code\":\"MODEL_REJECTED\",\"message\":\"scoring failed\"}}\n"
    );
}

fn state_with_worker(worker: Box<dyn WorkerBackend>) -> AppState {
    build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![worker],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    )
}

struct ExactEditReference {
    bases: Vec<u8>,
    provenance: ReferenceProvenance,
}

impl ReferenceProvider for ExactEditReference {
    fn copy_window(
        &self,
        _contig: Grch38Contig,
        start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError> {
        let offset = usize::try_from(start.get() - 1).expect("offset");
        let source = self
            .bases
            .get(offset..offset + destination.len())
            .ok_or(ReferenceError::OutOfBounds)?;
        destination.copy_from_slice(source);
        Ok(())
    }

    fn provenance(&self) -> &ReferenceProvenance {
        &self.provenance
    }
}

#[tokio::test]
async fn exact_deletion_mismatch_is_an_ordered_item_rejection() {
    let calls = Arc::new(AtomicUsize::new(0));
    let cache_gets = Arc::new(AtomicUsize::new(0));
    let mut state = build_state(
        Arc::new(FakeLookup),
        Box::new(CountingCache {
            gets: Arc::clone(&cache_gets),
        }),
        identity(),
        provenance(),
        vec![Box::new(FakeWorker {
            calls: Arc::clone(&calls),
        })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    state.reference = Some(Arc::new(ExactEditReference {
        bases: b"AAGT".to_vec(),
        provenance: provenance().reference().clone(),
    }));
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:1:A:C","GRCh38:chr1:DEL:3:3:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = body(response).await;
    let output: RawScoreOutput = serde_json::from_slice(&bytes).expect("response JSON");
    assert_eq!(output.results.len(), 2);
    assert_eq!(
        output.results[1].get(),
        r#"{"assembly":"GRCh38","contig":"chr1","position":2,"ref":"AC","alt":"A","status":"rejected","records":[],"source_reference_ambiguities":[],"error":{"code":"MODEL_REJECTED","message":"scoring failed"},"scoring_identity":"sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"}"#
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "conversion rejection must not reach inference"
    );
    assert_eq!(
        cache_gets.load(Ordering::SeqCst),
        0,
        "conversion rejection must not reach cache lookup"
    );
}

#[tokio::test]
async fn exact_edit_anchor_failures_stop_before_lookup_cache_and_inference() {
    for variant in ["GRCh38:chr1:INS:1:2:A", "GRCh38:chr1:INS:4:5:A"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut state = state_with_worker(Box::new(FakeWorker {
            calls: Arc::clone(&calls),
        }));
        state.reference = Some(Arc::new(ExactEditReference {
            bases: b"NAA".to_vec(),
            provenance: provenance().reference().clone(),
        }));
        let response = app(state)
            .oneshot(
                Request::post("/v1/score")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(r#"{{"variants":["{variant}"]}}"#)))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn successful_exact_deletion_matches_literal_records_and_provenance() {
    let mut state = state_with_worker(Box::new(RecordWorker));
    state.reference = Some(Arc::new(ExactEditReference {
        bases: b"AAGT".to_vec(),
        provenance: provenance().reference().clone(),
    }));
    let router = app(state);
    let request_for = |variant: &str| {
        Request::post("/v1/score")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(format!(r#"{{"variants":["{variant}"]}}"#)))
            .expect("request")
    };
    let exact = router
        .clone()
        .oneshot(request_for("GRCh38:chr1:DEL:3:3:G"))
        .await
        .expect("exact response");
    let literal = router
        .oneshot(request_for("GRCh38:chr1:2:AG:A"))
        .await
        .expect("literal response");
    assert_eq!(exact.status(), StatusCode::OK);
    assert_eq!(body(exact).await, body(literal).await);
}

#[derive(serde::Deserialize)]
struct RawScoreOutput {
    results: Vec<Box<RawValue>>,
}

#[tokio::test]
async fn mixed_batch_keeps_precomputed_result_and_orders_typed_rejection() {
    let state = state_with_worker(Box::new(RejectAtWorker {
        position: 2,
        calls: Arc::new(AtomicUsize::new(0)),
    }));
    let router = app(state);
    let normal = router
        .clone()
        .oneshot(score_request(1))
        .await
        .expect("normal response");
    let normal_bytes = body(normal).await;
    let normal: RawScoreOutput = serde_json::from_slice(&normal_bytes).expect("normal JSON");
    let response = router
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:1:A:C","GRCh38:chr1:2:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = body(response).await;
    let raw: RawScoreOutput = serde_json::from_slice(&bytes).expect("raw JSON");
    assert_eq!(raw.results[0].get(), normal.results[0].get());
    assert_eq!(
        raw.results[1].get(),
        r#"{"assembly":"GRCh38","contig":"chr1","position":2,"ref":"A","alt":"C","status":"rejected","records":[],"source_reference_ambiguities":[],"error":{"code":"MODEL_REJECTED","message":"scoring failed"},"scoring_identity":"sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"}"#
    );
    let value: Value = serde_json::from_slice(&bytes).expect("JSON");
    assert!(value["results"][1].get("provenance").is_none());
}

#[tokio::test]
async fn mixed_batch_keeps_modeled_not_found_beside_rejection() {
    let state = state_with_worker(Box::new(RejectAtWorker {
        position: 3,
        calls: Arc::new(AtomicUsize::new(0)),
    }));
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C","GRCh38:chr1:3:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = serde_json::from_slice(&body(response).await).expect("JSON");
    assert_eq!(value["results"][0]["status"], "not_found");
    assert_eq!(value["results"][1]["status"], "rejected");
}

#[tokio::test]
async fn batch_with_only_rejections_keeps_request_level_422() {
    let state = state_with_worker(Box::new(RejectAtWorker {
        position: 2,
        calls: Arc::new(AtomicUsize::new(0)),
    }));
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C","GRCh38:chr1:2:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"MODEL_REJECTED\",\"message\":\"scoring failed\"}}\n"
    );
}

#[tokio::test]
async fn mixed_batch_keeps_exact_cache_hit_beside_rejection_without_rescoring() {
    let temp = tempfile::tempdir().expect("temp");
    let (_path, mut cache) = private_cache(&temp);
    let cached = pending_at(2);
    cache
        .put(
            &CacheKey::new(cached.variant(), identity()),
            &model_records(),
        )
        .expect("seed cache");
    let calls = Arc::new(AtomicUsize::new(0));
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(cache),
        identity(),
        provenance(),
        vec![Box::new(RejectAtWorker {
            position: 3,
            calls: Arc::clone(&calls),
        })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let router = app(state);
    let cached_only = router
        .clone()
        .oneshot(score_request(2))
        .await
        .expect("cached response");
    let cached_only = body(cached_only).await;
    let cached_only: RawScoreOutput = serde_json::from_slice(&cached_only).expect("cached JSON");
    let mixed = router
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C","GRCh38:chr1:3:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("mixed response");
    assert_eq!(mixed.status(), StatusCode::OK);
    let mixed = body(mixed).await;
    let mixed: RawScoreOutput = serde_json::from_slice(&mixed).expect("mixed JSON");
    assert_eq!(mixed.results[0].get(), cached_only.results[0].get());
    assert_eq!(
        mixed.results[1].get(),
        r#"{"assembly":"GRCh38","contig":"chr1","position":3,"ref":"A","alt":"C","status":"rejected","records":[],"source_reference_ambiguities":[],"error":{"code":"MODEL_REJECTED","message":"scoring failed"},"scoring_identity":"sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"}"#
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn model_failure_stops_batch_without_partial_http_result() {
    let calls = Arc::new(AtomicUsize::new(0));
    let state = build_state(
        Arc::new(FakeLookup),
        Box::new(EmptyCache),
        identity(),
        provenance(),
        vec![Box::new(FailSecondWorker {
            calls: Arc::clone(&calls),
        })],
        1,
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
        scoring_identity(),
    );
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C","GRCh38:chr1:3:A:C"]}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body(response).await,
        b"{\"error\":{\"code\":\"MODEL_SCORING\",\"message\":\"scoring failed\"}}\n"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn queued_disconnect_is_discarded_before_inference() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = bounded(2);
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running_jobs: 0,
        running: 0,
        queued: 2,
    }));
    let dispatcher = Dispatcher {
        state: Arc::clone(&state),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 1,
        threads: 1,
        queue_capacity: 2,
    };
    let mut job = model_job(&[2, 3]);
    job.slot = JobSlot::Queued;
    sender.send(job).expect("queue");
    let join = spawn_worker(
        Box::new(FakeWorker {
            calls: Arc::clone(&calls),
        }),
        receiver,
        state,
    );
    while {
        let snapshot = dispatcher.snapshot();
        snapshot.running + snapshot.queued != 0
    } {
        thread::yield_now();
    }
    assert_eq!(dispatcher.snapshot().running, 0);
    assert_eq!(dispatcher.state.lock().expect("state").running_jobs, 0);
    dispatcher.close();
    drop(sender);
    join.join().expect("worker joins");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn running_disconnect_does_not_interrupt_started_inference() {
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (sender, receiver) = bounded(2);
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running_jobs: 0,
        running: 0,
        queued: 2,
    }));
    let dispatcher = Dispatcher {
        state: Arc::clone(&state),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 1,
        threads: 1,
        queue_capacity: 2,
    };
    let (reply, waiting) = oneshot::channel();
    let first = pending_at(2);
    let second = pending_at(3);
    sender
        .send(ModelJob {
            items: vec![
                JobItem {
                    output_index: 0,
                    key: CacheKey::new(first.variant(), identity()),
                    pending: first,
                },
                JobItem {
                    output_index: 1,
                    key: CacheKey::new(second.variant(), identity()),
                    pending: second,
                },
            ],
            response: reply,
            slot: JobSlot::Queued,
        })
        .expect("queue");
    let join = spawn_worker(
        Box::new(BlockingWorker {
            calls: Arc::clone(&calls),
            entered: entered_tx,
            release: Arc::clone(&release),
            order,
        }),
        receiver,
        state,
    );
    assert_eq!(entered_rx.recv().expect("started"), 2);
    drop(waiting);
    let (lock, ready) = &*release;
    *lock.lock().expect("release") = true;
    ready.notify_all();
    while dispatcher.snapshot().running != 0 {
        thread::yield_now();
    }
    assert_eq!(dispatcher.snapshot().queued, 0);
    assert_eq!(dispatcher.state.lock().expect("state").running_jobs, 0);
    dispatcher.close();
    drop(sender);
    join.join().expect("worker joins");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn configuration_bounds_are_rejected_before_startup() {
    for args in [
        vec![OsString::from("--model-workers"), OsString::from("0")],
        vec![OsString::from("--model-threads"), OsString::from("9")],
        vec![
            OsString::from("--model-queue-capacity"),
            OsString::from("0"),
        ],
    ] {
        assert_eq!(parse_options(&args).expect_err("invalid").code, "CLI_USAGE");
    }
}

#[tokio::test]
async fn persistent_signal_stream_forces_running_drain_on_actual_second_signal() {
    let (sender, _receiver) = bounded(1);
    let dispatcher = Dispatcher {
        state: Arc::new(Mutex::new(DispatchState {
            sender: Some(sender),
            readiness: READY,
            running_jobs: 1,
            running: 1,
            queued: 0,
        })),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 1,
        threads: 1,
        queue_capacity: 1,
    };
    let mut signals = signal_receiver();
    tokio::task::yield_now().await;
    let observed = dispatcher.clone();
    let shutdown =
        tokio::spawn(async move { shutdown_with_signals(&observed, &mut signals).await });
    assert_eq!(
        unsafe { libc::kill(std::process::id() as i32, libc::SIGINT) },
        0
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while dispatcher.snapshot().readiness != DRAINING {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first signal begins drain");
    assert_eq!(
        unsafe { libc::kill(std::process::id() as i32, libc::SIGTERM) },
        0
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), shutdown)
            .await
            .expect("second signal forces promptly")
            .expect("shutdown joins"),
        ShutdownOutcome::Forced
    );
    assert_eq!(dispatcher.snapshot().readiness, DRAINING);
}

async fn network_request(address: SocketAddr, request: &[u8]) -> Vec<u8> {
    let request = request.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        stream.write_all(&request).expect("write request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        response
    })
    .await
    .expect("network task")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_listener_pins_head_allow_representation_and_unknown_route() {
    let (state, _) = state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let (stop, stopped) = oneshot::channel::<()>();
    let server_state = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app(server_state))
            .with_graceful_shutdown(async move {
                let _ = stopped.await;
            })
            .await
    });
    let live = network_request(
        address,
        b"GET /livez HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(live.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(live.ends_with(b"\r\n\r\n{\"status\":\"live\"}\n"));

    for (path, allow) in [
        ("/livez", "GET"),
        ("/readyz", "GET"),
        ("/v1/status", "GET"),
        ("/v1/score", "POST"),
    ] {
        let request = format!("HEAD {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n");
        let response = network_request(address, request.as_bytes()).await;
        let text = String::from_utf8(response).expect("HTTP text");
        assert!(
            text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"),
            "{text}"
        );
        assert!(text.contains(&format!("allow: {allow}\r\n")), "{text}");
        assert!(
            text.contains("content-type: application/json\r\n"),
            "{text}"
        );
        assert!(text.contains("content-length: 71\r\n"), "{text}");
        assert!(
            text.ends_with("\r\n\r\n"),
            "HEAD wire body must be empty: {text}"
        );
    }
    let unknown = network_request(
        address,
        b"HEAD /unknown HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n",
    )
    .await;
    let unknown = String::from_utf8(unknown).expect("HTTP text");
    assert!(
        unknown.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{unknown}"
    );
    assert!(unknown.ends_with("\r\n\r\n"), "{unknown}");

    stop.send(()).expect("stop server");
    server.await.expect("server join").expect("server result");
    state.dispatcher.close();
    tokio::task::block_in_place(|| state.dispatcher.join_workers());
}
