use super::*;
use axum::http::StatusCode;
use pangopup_core::{
    GencodeGeneId, GeneScoreRecord, LookupProvenance, LookupResult, PangolinScore,
    PrecomputedProvenance, RelativePosition, ScoreMagnitude,
};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
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

struct FakeWorker {
    calls: Arc<AtomicUsize>,
}
impl WorkerBackend for FakeWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        _key: &CacheKey,
    ) -> Result<RoutedResult, Failure> {
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
    ) -> Result<RoutedResult, Failure> {
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
    ) -> Result<RoutedResult, Failure> {
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
    fn complete_unfiltered(&mut self, pending: PendingModel) -> Result<RoutedResult, Failure> {
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
    fn complete_unfiltered(&mut self, pending: PendingModel) -> Result<RoutedResult, Failure> {
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
    fn complete_unfiltered(&mut self, pending: PendingModel) -> Result<RoutedResult, Failure> {
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
    ) -> Result<RoutedResult, Failure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == 2 {
            return Err(Failure {
                code: "TEST_FAILURE",
                message: "test failure".to_owned(),
                exit: 1,
                details: None,
            });
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

#[test]
fn admission_capacity_is_workers_plus_waiting_before_any_worker_receives() {
    let (sender, receiver) = bounded(3);
    let dispatcher = Dispatcher {
        state: Arc::new(Mutex::new(DispatchState {
            sender: Some(sender),
            readiness: READY,
            running: 0,
            queued: 0,
        })),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 2,
        threads: 1,
        queue_capacity: 1,
    };
    let job = |position| {
        let (response, _waiting) = oneshot::channel();
        let pending = pending_at(position);
        ModelJob {
            items: vec![JobItem {
                output_index: 0,
                key: CacheKey::new(pending.variant(), identity()),
                pending,
            }],
            response,
            slot: JobSlot::Unassigned,
        }
    };
    assert!(dispatcher.admit(job(2)).is_ok());
    assert!(dispatcher.admit(job(3)).is_ok());
    assert!(dispatcher.admit(job(4)).is_ok());
    assert!(matches!(
        dispatcher.admit(job(5)),
        Err(AdmissionError::Full)
    ));
    let snapshot = dispatcher.snapshot();
    assert_eq!(snapshot.running, 2);
    assert_eq!(snapshot.queued, 1);
    assert!(snapshot.running <= dispatcher.workers);
    assert!(snapshot.queued <= dispatcher.queue_capacity);
    drop(receiver);
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
        1,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
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
    assert_eq!(
            body(response).await,
            b"{\"version\":\"0.1.0\",\"readiness\":\"ready\",\"assets\":{\"snv_bundle_id\":\"snv\",\"model_bundle_id\":\"model\",\"reference_bundle_id\":\"reference\",\"mask_sha256\":\"mask\"},\"routes\":{\"lookup\":true,\"model\":true,\"model_only\":true},\"model\":{\"effective_cpu_policy\":\"sequential:1/1\",\"workers\":1,\"threads_per_worker\":1,\"running\":0,\"queued\":0,\"queue_capacity\":1}}\n"
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
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
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
}

#[tokio::test]
async fn modeled_http_success_is_pinned_as_one_complete_byte_fixture() {
    let (state, _) = state();
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
                .body(Body::from(
                    r#"{"variants":["GRCh38:chr1:2:A:C"],"model_only":true}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let expected = format!(
        "{{\"results\":[{{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":2,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"not_found\",\"records\":[],\"source_reference_ambiguities\":[],\"provenance\":{{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:{}\",\"model_profile\":\"model-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:{}\",\"reference_profile\":\"reference-v1\",\"reference_sequence_set_sha256\":\"sha256:{}\",\"mask_bytes\":1,\"mask_sha256\":\"sha256:{}\",\"masked\":true,\"window\":50}}}}]}}\n",
        "1".repeat(64),
        "2".repeat(64),
        "3".repeat(64),
        "4".repeat(64),
    );
    assert_eq!(body(response).await, expected.as_bytes());
}

#[tokio::test]
async fn malformed_closed_input_and_limits_fail_before_scoring() {
    let (state, calls) = state();
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
    let variants = (2..=12)
        .map(|position| format!("GRCh38:chr1:{position}:A:C"))
        .collect::<Vec<_>>();
    let response = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": variants})).expect("JSON")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

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
                .body(Body::from(vec![b' '; MAX_BODY + 1]))
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
    let valid = br#"{"variants":["GRCh38:chr1:1:A:C"]}"#;
    let mut at_limit = valid.to_vec();
    at_limit.extend(std::iter::repeat_n(b' ', MAX_BODY - valid.len()));
    let accepted = app(state.clone())
        .oneshot(
            Request::post("/v1/score")
                .body(Body::from(at_limit))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(accepted.status(), StatusCode::OK);

    let one_over = app(state.clone())
        .oneshot(
            Request::post("/v1/score")
                .body(Body::from(vec![b' '; MAX_BODY + 1]))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(one_over.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let hundred = std::iter::repeat_n("GRCh38:chr1:1:A:C", MAX_VARIANTS).collect::<Vec<_>>();
    let accepted = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": hundred})).expect("JSON")),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let hundred_one =
        std::iter::repeat_n("GRCh38:chr1:1:A:C", MAX_VARIANTS + 1).collect::<Vec<_>>();
    let rejected = score_bytes(
        &state,
        &Bytes::from(serde_json::to_vec(&json!({"variants": hundred_one})).expect("JSON")),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body(rejected).await,
        b"{\"error\":{\"code\":\"INVALID_REQUEST\",\"message\":\"variants must contain between 1 and 100 values\"}}\n"
    );
}

fn score_request(position: u32) -> Request<Body> {
    Request::post("/v1/score")
        .body(Body::from(format!(
            "{{\"variants\":[\"GRCh38:chr1:{position}:A:C\"]}}"
        )))
        .expect("request")
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
        1,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
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
        2,
        AssetStatus {
            snv_bundle_id: "snv".to_owned(),
            model_bundle_id: "model".to_owned(),
            reference_bundle_id: "reference".to_owned(),
            mask_sha256: "mask".to_owned(),
        },
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
    );
    let response = app(state)
        .oneshot(
            Request::post("/v1/score")
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
        b"{\"error\":{\"code\":\"SCORING_FAILED\",\"message\":\"scoring failed\"}}\n"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn queued_disconnect_is_discarded_before_inference() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = bounded(1);
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running: 0,
        queued: 1,
    }));
    let dispatcher = Dispatcher {
        state: Arc::clone(&state),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 1,
        threads: 1,
        queue_capacity: 1,
    };
    let (reply, waiting) = oneshot::channel();
    drop(waiting);
    sender
        .send(ModelJob {
            items: vec![JobItem {
                output_index: 0,
                pending: PendingModel::Explicit(ExplicitModelRequest::new(RouteRequest::new(
                    super::super::parse_variant("GRCh38:chr1:2:A:C").expect("variant"),
                    None,
                ))),
                key: CacheKey::new(
                    &super::super::parse_variant("GRCh38:chr1:2:A:C").expect("variant"),
                    identity(),
                ),
            }],
            response: reply,
            slot: JobSlot::Queued,
        })
        .expect("queue");
    let join = spawn_worker(
        Box::new(FakeWorker {
            calls: Arc::clone(&calls),
        }),
        receiver,
        state,
    );
    while dispatcher.snapshot().queued != 0 {
        thread::yield_now();
    }
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
    let (sender, receiver) = bounded(1);
    let state = Arc::new(Mutex::new(DispatchState {
        sender: Some(sender.clone()),
        readiness: READY,
        running: 0,
        queued: 1,
    }));
    let dispatcher = Dispatcher {
        state: Arc::clone(&state),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: 1,
        threads: 1,
        queue_capacity: 1,
    };
    let variant = super::super::parse_variant("GRCh38:chr1:2:A:C").expect("variant");
    let (reply, waiting) = oneshot::channel();
    sender
        .send(ModelJob {
            items: vec![JobItem {
                output_index: 0,
                pending: PendingModel::Explicit(ExplicitModelRequest::new(RouteRequest::new(
                    variant.clone(),
                    None,
                ))),
                key: CacheKey::new(&variant, identity()),
            }],
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
