//! Foreground HTTP adapter with one fixed FIFO model queue.

use super::{
    BatchDecision, CacheOptions, Failure, PendingModel, data_root, map_cache_error,
    map_lookup_error, map_model_fallback_error, resolve_model_cache_options, routed_from_cached,
};
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::{Next, from_fn},
    response::Response,
    routing::{MethodFilter, on},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use pangopup_assets::{AssetError, open_active_bundle, open_installed_runtime_profile};
#[cfg(feature = "service-test-fixtures")]
use pangopup_assets::{open_test_runtime_profile, parse_runtime_profile};
use pangopup_cache::{CacheIdentity, CacheKey, EntryLimit, ModelResultCache};
use pangopup_cli::render_result_raw;
use pangopup_core::{EnsemblGeneId, Grch38Variant, ModelGeneScoreRecord};
use pangopup_engine::{
    ExplicitModelRequest, LookupFirstRouter, ModelFallback, ModelFallbackError, ModelProvenance,
    RouteDecision, RouteRequest, RoutedResult,
};
use pangopup_model::{CpuExecutionMode, CpuPolicy, IntraOpThreads};
use serde::{Deserialize, Serialize};
use serde_json::{json, value::RawValue};
use std::{
    ffi::OsString,
    net::SocketAddr,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, oneshot};

const MAX_BODY: usize = 64 * 1024;
const MAX_VARIANTS: usize = 100;
const MAX_MODEL_MISSES: usize = 10;
const SLOWEST_RETAINED_P50_MILLIS: usize = 10_241;
const READY: u8 = 0;
const DRAINING: u8 = 1;
const FAILED: u8 = 2;

#[derive(Clone, Debug)]
struct ServeOptions {
    listen: SocketAddr,
    data_dir: Option<OsString>,
    workers: usize,
    threads: usize,
    queue_capacity: usize,
    cache_path: Option<PathBuf>,
    cache_limit: Option<EntryLimit>,
}

trait LookupBackend: Send + Sync {
    fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, Failure>;
}

impl LookupBackend for LookupFirstRouter<pangopup_index::BundleOpen> {
    fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, Failure> {
        LookupFirstRouter::inspect(self, request).map_err(map_lookup_error)
    }
}

trait CacheReader: Send {
    fn get(
        &mut self,
        key: &CacheKey,
    ) -> Result<Option<Vec<ModelGeneScoreRecord>>, pangopup_cache::CacheError>;
}

impl CacheReader for ModelResultCache {
    fn get(
        &mut self,
        key: &CacheKey,
    ) -> Result<Option<Vec<ModelGeneScoreRecord>>, pangopup_cache::CacheError> {
        ModelResultCache::get(self, key)
    }
}

trait WorkerBackend: Send {
    fn complete(
        &mut self,
        pending: &PendingModel,
        key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure>;
}

trait ModelCompletion: Send {
    fn complete_unfiltered(
        &mut self,
        pending: PendingModel,
    ) -> Result<RoutedResult, ModelFallbackError>;
    fn provenance(&self) -> &ModelProvenance;
}

impl ModelCompletion for ModelFallback {
    fn complete_unfiltered(
        &mut self,
        pending: PendingModel,
    ) -> Result<RoutedResult, ModelFallbackError> {
        match pending {
            PendingModel::Lookup(request) => ModelFallback::complete_unfiltered(self, request),
            PendingModel::Explicit(request) => {
                ModelFallback::complete_unfiltered_explicit(self, request)
            }
        }
    }

    fn provenance(&self) -> &ModelProvenance {
        ModelFallback::provenance(self)
    }
}

struct ProductionWorker {
    fallback: Box<dyn ModelCompletion>,
    cache: ModelResultCache,
}

impl WorkerBackend for ProductionWorker {
    fn complete(
        &mut self,
        pending: &PendingModel,
        key: &CacheKey,
    ) -> Result<RoutedResult, WorkerFailure> {
        match self.cache.get(key) {
            Ok(Some(records)) => {
                return Ok(routed_from_cached(
                    pending,
                    records,
                    self.fallback.provenance().clone(),
                ));
            }
            Ok(None) | Err(pangopup_cache::CacheError::Busy) => {}
            Err(error) => return Err(WorkerFailure::Operational(map_cache_error(error))),
        }
        let filter = pending.gene();
        let modeled = self
            .fallback
            .complete_unfiltered(pending.clone())
            .map_err(|error| match error {
                ModelFallbackError::Rejected(_) => WorkerFailure::Rejected,
                error @ ModelFallbackError::Scoring(_) => {
                    WorkerFailure::Operational(map_model_fallback_error(error))
                }
            })?;
        let RoutedResult::Modeled {
            variant,
            mut records,
            provenance,
        } = modeled
        else {
            unreachable!("model fallback always returns modeled output")
        };
        let _cache_write = self.cache.put(key, &records);
        if let Some(filter) = filter {
            records.retain(|record| record.gene().stable() == filter);
        }
        Ok(RoutedResult::Modeled {
            variant,
            records,
            provenance,
        })
    }
}

struct JobItem {
    output_index: usize,
    pending: PendingModel,
    key: CacheKey,
}

struct ModelJob {
    items: Vec<JobItem>,
    response: oneshot::Sender<Result<Vec<(usize, ScoreOutcome)>, WorkerReply>>,
    slot: JobSlot,
}

impl ModelJob {
    fn weight(&self) -> usize {
        self.items.len()
    }
}

// Keep ordinary completed values inline. Boxing this variant would add an
// allocation for every successful item. The uncommon rejection does not
// justify that cost.
#[allow(clippy::large_enum_variant)]
enum ScoreOutcome {
    Complete(RoutedResult),
    Rejected(Grch38Variant),
}

#[derive(Debug)]
enum WorkerFailure {
    Rejected,
    Operational(Failure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobSlot {
    Unassigned,
    Running,
    Queued,
}

enum WorkerReply {
    BackendFailure(Failure),
    Unavailable,
}

struct DispatchState {
    sender: Option<Sender<ModelJob>>,
    readiness: u8,
    running_jobs: usize,
    running: usize,
    queued: usize,
}

#[derive(Clone, Copy)]
struct DispatchSnapshot {
    readiness: u8,
    running: usize,
    queued: usize,
}

#[derive(Clone)]
struct Dispatcher {
    state: Arc<Mutex<DispatchState>>,
    joins: Arc<Mutex<Vec<JoinHandle<()>>>>,
    workers: usize,
    threads: usize,
    queue_capacity: usize,
}

impl Dispatcher {
    fn snapshot(&self) -> DispatchSnapshot {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        DispatchSnapshot {
            readiness: state.readiness,
            running: state.running,
            queued: state.queued,
        }
    }

    fn admit(&self, mut job: ModelJob) -> Result<(), AdmissionError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match state.readiness {
            READY => {}
            FAILED => return Err(AdmissionError::Unavailable),
            _ => return Err(AdmissionError::Draining),
        }
        let weight = job.weight();
        let admitted = state.running + state.queued;
        if admitted + weight > self.queue_capacity {
            let retry_after_seconds =
                (weight <= self.queue_capacity).then(|| retry_after_seconds(admitted));
            return Err(AdmissionError::Full {
                retry_after_seconds,
            });
        }
        let slot = if state.running_jobs < self.workers && state.queued == 0 {
            state.running_jobs += 1;
            state.running += weight;
            JobSlot::Running
        } else {
            state.queued += weight;
            JobSlot::Queued
        };
        job.slot = slot;
        let send = state
            .sender
            .as_ref()
            .expect("ready dispatcher retains its sender")
            .try_send(job);
        if send.is_err() {
            match slot {
                JobSlot::Running => {
                    state.running_jobs -= 1;
                    state.running -= weight;
                }
                JobSlot::Queued => state.queued -= weight,
                JobSlot::Unassigned => unreachable!(),
            }
            state.readiness = FAILED;
            state.sender.take();
            return Err(AdmissionError::Unavailable);
        }
        Ok(())
    }

    fn stop_admission(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.readiness == READY {
            state.readiness = DRAINING;
            state.sender.take();
        }
    }

    fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sender
            .take();
    }

    fn join_workers(&self) {
        let joins =
            std::mem::take(&mut *self.joins.lock().unwrap_or_else(|error| error.into_inner()));
        for join in joins {
            let _ = join.join();
        }
    }
}

enum AdmissionError {
    Full { retry_after_seconds: Option<usize> },
    Draining,
    Unavailable,
}

fn retry_after_seconds(admitted: usize) -> usize {
    (admitted * SLOWEST_RETAINED_P50_MILLIS)
        .div_ceil(1_000)
        .max(1)
}

#[derive(Clone)]
struct AssetStatus {
    snv_bundle_id: String,
    model_bundle_id: String,
    reference_bundle_id: String,
    mask_sha256: String,
}

#[derive(Clone)]
struct AppState {
    lookup: Arc<dyn LookupBackend>,
    handler_cache: Arc<Mutex<Box<dyn CacheReader>>>,
    cache_gate: Arc<Semaphore>,
    cache_identity: CacheIdentity,
    provenance: ModelProvenance,
    dispatcher: Dispatcher,
    assets: AssetStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreInput {
    variants: Vec<String>,
    #[serde(default, deserialize_with = "present_string")]
    gene: Option<String>,
    #[serde(default, deserialize_with = "present_bool")]
    model_only: Option<bool>,
}

fn present_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn present_bool<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}

#[derive(Serialize)]
struct ScoreOutput {
    results: Vec<Box<RawValue>>,
}

#[derive(Serialize)]
struct Listening<'a> {
    event: &'static str,
    address: &'a str,
}

#[derive(Serialize)]
struct StatusOutput<'a> {
    version: &'static str,
    readiness: &'static str,
    assets: StatusAssets<'a>,
    routes: StatusRoutes,
    model: StatusModel,
}

#[derive(Serialize)]
struct StatusAssets<'a> {
    snv_bundle_id: &'a str,
    model_bundle_id: &'a str,
    reference_bundle_id: &'a str,
    mask_sha256: &'a str,
}

#[derive(Serialize)]
struct StatusRoutes {
    lookup: bool,
    model: bool,
    model_only: bool,
}

#[derive(Serialize)]
struct StatusModel {
    effective_cpu_policy: String,
    workers: usize,
    threads_per_worker: usize,
    running: usize,
    queued: usize,
    queue_capacity: usize,
    work_unit: &'static str,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

pub(super) fn run(raw: &[OsString]) -> ExitCode {
    let options = match parse_options(raw) {
        Ok(options) => options,
        Err(error) => return super::fail(&error),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return super::fail(&Failure {
                code: "SERVICE_START_FAILED",
                message: "HTTP runtime could not start".to_owned(),
                exit: 1,
                details: None,
            });
        }
    };
    match runtime.block_on(serve(options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => super::fail(&error),
    }
}

fn parse_options(raw: &[OsString]) -> Result<ServeOptions, Failure> {
    let mut listen = "127.0.0.1:8080".parse().expect("literal address");
    let mut data_dir = None;
    let mut workers = 1_usize;
    let mut threads = 1_usize;
    let mut queue_capacity = 20_usize;
    let mut cache_path = None;
    let mut cache_limit = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 0;
    while index < raw.len() {
        let option = raw[index]
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        index += 1;
        if !seen.insert(option.to_owned()) {
            return Err(Failure::usage(format!("{option} may be supplied once")));
        }
        let value = raw
            .get(index)
            .ok_or_else(|| Failure::usage(format!("{option} requires a value")))?;
        index += 1;
        match option {
            "--listen" => {
                listen = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| Failure::usage("--listen requires an IP socket address"))?;
            }
            "--data-dir" => data_dir = Some(value.clone()),
            "--model-workers" => workers = bounded_integer(value, option, 1, 8)?,
            "--model-threads" => threads = bounded_integer(value, option, 1, 8)?,
            "--model-queue-capacity" => queue_capacity = bounded_integer(value, option, 1, 1024)?,
            "--model-cache" => {
                let path = PathBuf::from(value);
                super::validate_model_cache_path(&path)?;
                cache_path = Some(path);
            }
            "--model-cache-max-entries" => {
                cache_limit = Some(
                    value
                        .to_str()
                        .ok_or_else(|| Failure::usage("cache limit must be UTF-8"))?
                        .parse()
                        .map_err(|error: pangopup_cache::CacheError| {
                            Failure::usage(error.to_string())
                        })?,
                );
            }
            _ => return Err(Failure::usage(format!("unknown serve option {option}"))),
        }
    }
    Ok(ServeOptions {
        listen,
        data_dir,
        workers,
        threads,
        queue_capacity,
        cache_path,
        cache_limit,
    })
}

fn bounded_integer(
    value: &OsString,
    option: &str,
    minimum: usize,
    maximum: usize,
) -> Result<usize, Failure> {
    value
        .to_str()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(|| Failure::usage(format!("{option} must be in {minimum}..={maximum}")))
}

async fn serve(options: ServeOptions) -> Result<(), Failure> {
    std::panic::set_hook(Box::new(|_| eprintln!("model worker failed")));
    let data = data_root(options.data_dir.clone())?;
    let (active, bundle) = open_active_bundle(&data)
        .map_err(|error| map_startup_asset_error(error, super::map_lookup_asset_error))?;
    let installed = open_service_runtime(&data, &active.bundle_id)?;
    let profile = installed.profile().clone();
    drop(installed);
    let policy = CpuPolicy::new(
        CpuExecutionMode::Sequential,
        IntraOpThreads::Fixed(NonZeroUsize::new(options.threads).expect("positive threads")),
        NonZeroUsize::MIN,
    )
    .map_err(|_| Failure::usage("invalid model thread policy"))?;
    let effective_policy = policy.to_string();
    let provenance = ModelProvenance::new(
        profile.model.bundle_id.clone(),
        profile.model.profile.clone(),
        pangopup_core::ReferenceProvenance::new(
            profile.reference.bundle_id.clone(),
            profile.reference.profile.clone(),
            profile.reference.format.clone(),
            profile.reference.assembly.clone(),
            profile.reference.assembly_accession.clone(),
            profile.reference.sequence_set_sha256.clone(),
        ),
        profile.mask.member_bytes,
        profile.mask.member_sha256.clone(),
    )
    .with_effective_cpu_policy(effective_policy.clone());
    let identity = CacheIdentity::new(
        &profile.model.bundle_id,
        &profile.model.profile,
        &profile.model.representation,
        &effective_policy,
        &profile.reference.bundle_id,
        &profile.reference.profile,
        &profile.reference.sequence_set_sha256,
        profile.mask.member_bytes,
        &profile.mask.member_sha256,
    )
    .map_err(map_cache_error)?;
    let cache_options = resolve_model_cache_options(options.cache_path, options.cache_limit)?;
    let handler_cache = open_cache(&cache_options)?;
    let mut backends: Vec<Box<dyn WorkerBackend>> = Vec::with_capacity(options.workers);
    for _ in 0..options.workers {
        let installed = open_service_runtime(&data, &active.bundle_id)?;
        let (_, model, reference, mask) = installed.into_parts();
        let mask = mask.open().map_err(|_| Failure::profile_corrupt())?;
        let model = model
            .open_with_cpu_policy(policy)
            .map_err(super::map_runtime_error)?;
        let fallback = ModelFallback::new(reference, mask, model);
        if fallback.provenance() != &provenance {
            return Err(Failure {
                code: "PROFILE_CORRUPT",
                message: "installed runtime profile is invalid".to_owned(),
                exit: 1,
                details: None,
            });
        }
        backends.push(Box::new(ProductionWorker {
            fallback: Box::new(fallback),
            cache: open_cache(&cache_options)?,
        }));
    }
    let assets = AssetStatus {
        snv_bundle_id: active.bundle_id,
        model_bundle_id: profile.model.bundle_id,
        reference_bundle_id: profile.reference.bundle_id,
        mask_sha256: profile.mask.member_sha256,
    };
    let state = build_state(
        Arc::new(LookupFirstRouter::new(bundle)),
        Box::new(handler_cache),
        identity,
        provenance,
        backends,
        options.threads,
        options.queue_capacity,
        assets,
    );
    let listener = tokio::net::TcpListener::bind(options.listen)
        .await
        .map_err(|_| Failure {
            code: "SERVICE_START_FAILED",
            message: "HTTP listener could not bind".to_owned(),
            exit: 1,
            details: None,
        })?;
    let address = listener.local_addr().map_err(|_| Failure {
        code: "SERVICE_START_FAILED",
        message: "HTTP listener address is unavailable".to_owned(),
        exit: 1,
        details: None,
    })?;
    let address = address.to_string();
    println!(
        "{}",
        serde_json::to_string(&Listening {
            event: "listening",
            address: &address,
        })
        .expect("listening event is serializable")
    );
    axum::serve(listener, app(state.clone()))
        .with_graceful_shutdown(shutdown(state.dispatcher.clone()))
        .await
        .map_err(|_| Failure {
            code: "SERVICE_FAILED",
            message: "HTTP service failed".to_owned(),
            exit: 1,
            details: None,
        })?;
    state.dispatcher.close();
    tokio::task::block_in_place(|| state.dispatcher.join_workers());
    Ok(())
}

fn map_startup_asset_error(
    error: AssetError,
    mapper: impl FnOnce(AssetError) -> Failure,
) -> Failure {
    let mut failure = mapper(error);
    if matches!(failure.code, "ASSETS_MISSING" | "PROFILE_MISSING") {
        failure.message = "required assets are missing; run pangopup sync".to_owned();
    }
    failure
}

fn open_service_runtime(
    data: &std::path::Path,
    snv_bundle_id: &str,
) -> Result<pangopup_assets::InstalledRuntimeProfile, Failure> {
    #[cfg(feature = "service-test-fixtures")]
    if let Some(path) = std::env::var_os("PANGOPUP_SERVICE_TEST_PROFILE") {
        let bytes = std::fs::read(path).map_err(|_| Failure::profile_corrupt())?;
        let profile = parse_runtime_profile(&bytes).map_err(|_| Failure::profile_corrupt())?;
        return open_test_runtime_profile(data, snv_bundle_id, &profile)
            .map_err(|error| map_startup_asset_error(error, super::map_runtime_error));
    }
    open_installed_runtime_profile(data, snv_bundle_id)
        .map_err(|error| map_startup_asset_error(error, super::map_runtime_error))
}

fn open_cache(options: &CacheOptions) -> Result<ModelResultCache, Failure> {
    let result = if options.disposable_default {
        ModelResultCache::open_default(&options.path, options.limit)
    } else {
        ModelResultCache::open_explicit(&options.path, options.limit)
    };
    result.map_err(map_cache_error)
}

#[allow(clippy::too_many_arguments)]
fn build_state(
    lookup: Arc<dyn LookupBackend>,
    handler_cache: Box<dyn CacheReader>,
    identity: CacheIdentity,
    provenance: ModelProvenance,
    backends: Vec<Box<dyn WorkerBackend>>,
    threads: usize,
    queue_capacity: usize,
    assets: AssetStatus,
) -> AppState {
    let worker_count = backends.len();
    let (sender, receiver) = bounded(queue_capacity);
    let dispatcher = Dispatcher {
        state: Arc::new(Mutex::new(DispatchState {
            sender: Some(sender),
            readiness: READY,
            running_jobs: 0,
            running: 0,
            queued: 0,
        })),
        joins: Arc::new(Mutex::new(Vec::new())),
        workers: worker_count,
        threads,
        queue_capacity,
    };
    for backend in backends {
        let join = spawn_worker(backend, receiver.clone(), Arc::clone(&dispatcher.state));
        dispatcher
            .joins
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(join);
    }
    AppState {
        lookup,
        handler_cache: Arc::new(Mutex::new(handler_cache)),
        cache_gate: Arc::new(Semaphore::new(1)),
        cache_identity: identity,
        provenance,
        dispatcher,
        assets,
    }
}

fn spawn_worker(
    mut backend: Box<dyn WorkerBackend>,
    receiver: Receiver<ModelJob>,
    state: Arc<Mutex<DispatchState>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        loop {
            let job = match receiver.recv() {
                Ok(job) => job,
                Err(_) => return,
            };
            {
                let mut status = state.lock().unwrap_or_else(|error| error.into_inner());
                let weight = job.weight();
                match job.slot {
                    JobSlot::Running => {}
                    JobSlot::Queued => {
                        status.queued -= weight;
                        status.running += weight;
                        status.running_jobs += 1;
                    }
                    JobSlot::Unassigned => unreachable!("only admitted jobs reach workers"),
                }
            }
            if job.response.is_closed() {
                let mut status = state.lock().unwrap_or_else(|error| error.into_inner());
                status.running_jobs -= 1;
                status.running -= job.weight();
                continue;
            }
            #[cfg(feature = "service-test-fixtures")]
            if let Some(delay) = std::env::var("PANGOPUP_SERVICE_TEST_JOB_DELAY_MS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| (1..=1_000).contains(value))
            {
                thread::sleep(Duration::from_millis(delay));
            }
            let result = catch_unwind(AssertUnwindSafe(|| process_job(&mut *backend, &job)));
            {
                let mut status = state.lock().unwrap_or_else(|error| error.into_inner());
                status.running_jobs -= 1;
                status.running -= job.weight();
            }
            match result {
                Ok(reply) => {
                    let _sent = job.response.send(reply);
                }
                Err(_) => {
                    let _sent = job.response.send(Err(WorkerReply::Unavailable));
                    fail_workers(&receiver, &state);
                    return;
                }
            }
        }
    })
}

fn process_job(
    backend: &mut dyn WorkerBackend,
    job: &ModelJob,
) -> Result<Vec<(usize, ScoreOutcome)>, WorkerReply> {
    let mut results = Vec::with_capacity(job.items.len());
    for item in &job.items {
        if job.response.is_closed() {
            return Ok(results);
        }
        match backend.complete(&item.pending, &item.key) {
            Ok(result) => results.push((item.output_index, ScoreOutcome::Complete(result))),
            Err(WorkerFailure::Rejected) => results.push((
                item.output_index,
                ScoreOutcome::Rejected(item.pending.variant().clone()),
            )),
            Err(WorkerFailure::Operational(failure)) => {
                return Err(WorkerReply::BackendFailure(failure));
            }
        }
    }
    Ok(results)
}

fn fail_workers(receiver: &Receiver<ModelJob>, state: &Arc<Mutex<DispatchState>>) {
    let mut status = state.lock().unwrap_or_else(|error| error.into_inner());
    status.readiness = FAILED;
    status.sender.take();
    while let Ok(job) = receiver.try_recv() {
        let weight = job.weight();
        match job.slot {
            JobSlot::Running => {
                status.running_jobs -= 1;
                status.running -= weight;
            }
            JobSlot::Queued => status.queued -= weight,
            JobSlot::Unassigned => unreachable!("only admitted jobs reach workers"),
        }
        let _sent = job.response.send(Err(WorkerReply::Unavailable));
    }
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/livez", on(MethodFilter::GET, livez))
        .route("/readyz", on(MethodFilter::GET, readyz))
        .route("/v1/status", on(MethodFilter::GET, status))
        .route("/v1/score", on(MethodFilter::POST, score))
        .fallback(not_found)
        .layer(from_fn(reject_known_wrong_method))
        .with_state(state)
}

async fn reject_known_wrong_method(request: Request<Body>, next: Next) -> Response {
    let allowed = match request.uri().path() {
        "/livez" | "/readyz" | "/v1/status" => Some((Method::GET, "GET")),
        "/v1/score" => Some((Method::POST, "POST")),
        _ => None,
    };
    if let Some((method, allow)) = allowed
        && request.method() != method
    {
        let mut response = service_error(
            StatusCode::METHOD_NOT_ALLOWED,
            "METHOD_NOT_ALLOWED",
            "method not allowed",
        );
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static(allow));
        return response;
    }
    next.run(request).await
}

async fn livez() -> Response {
    json_response(StatusCode::OK, &json!({"status":"live"}))
}

async fn readyz(State(state): State<AppState>) -> Response {
    if state.dispatcher.snapshot().readiness == READY {
        json_response(StatusCode::OK, &json!({"status":"ready"}))
    } else {
        json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &json!({"status":"not_ready"}),
        )
    }
}

async fn status(State(state): State<AppState>) -> Response {
    let snapshot = state.dispatcher.snapshot();
    let readiness = match snapshot.readiness {
        READY => "ready",
        DRAINING => "draining",
        _ => "failed",
    };
    json_response(
        StatusCode::OK,
        &StatusOutput {
            version: env!("CARGO_PKG_VERSION"),
            readiness,
            assets: StatusAssets {
                snv_bundle_id: &state.assets.snv_bundle_id,
                model_bundle_id: &state.assets.model_bundle_id,
                reference_bundle_id: &state.assets.reference_bundle_id,
                mask_sha256: &state.assets.mask_sha256,
            },
            routes: StatusRoutes {
                lookup: true,
                model: true,
                model_only: true,
            },
            model: StatusModel {
                effective_cpu_policy: format!("sequential:{}/1", state.dispatcher.threads),
                workers: state.dispatcher.workers,
                threads_per_worker: state.dispatcher.threads,
                running: snapshot.running,
                queued: snapshot.queued,
                queue_capacity: state.dispatcher.queue_capacity,
                work_unit: "uncached_model_variant",
            },
        },
    )
}

async fn score(State(state): State<AppState>, request: Request<Body>) -> Response {
    if !has_json_content_type(request.headers()) {
        return service_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "UNSUPPORTED_MEDIA_TYPE",
            "content-type must be application/json",
        );
    }
    match state.dispatcher.snapshot().readiness {
        READY => {}
        FAILED => {
            return service_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MODEL_WORKER_UNAVAILABLE",
                "model worker failed",
            );
        }
        _ => {
            return service_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "SHUTTING_DOWN",
                "service is shutting down",
            );
        }
    }
    let bytes = match to_bytes(request.into_body(), MAX_BODY).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return service_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "REQUEST_TOO_LARGE",
                "request body is too large",
            );
        }
    };
    score_bytes(&state, &bytes).await
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    is_application_json(value.as_bytes())
}

fn is_application_json(value: &[u8]) -> bool {
    // Apply RFC 9110's media-type, token, quoted-string, quoted-pair, and optional-whitespace productions. This route's stricter contract requires every semicolon to introduce a parameter.
    let value = trim_optional_whitespace(value);
    let Some(type_end) = token_end(value, 0) else {
        return false;
    };
    if value.get(type_end) != Some(&b'/') {
        return false;
    }
    let subtype_start = type_end + 1;
    let Some(subtype_end) = token_end(value, subtype_start) else {
        return false;
    };
    if !value[..type_end].eq_ignore_ascii_case(b"application")
        || !value[subtype_start..subtype_end].eq_ignore_ascii_case(b"json")
    {
        return false;
    }
    has_valid_media_type_parameters(value, subtype_end)
}

fn has_valid_media_type_parameters(value: &[u8], mut offset: usize) -> bool {
    loop {
        offset = skip_optional_whitespace(value, offset);
        if offset == value.len() {
            return true;
        }
        if value.get(offset) != Some(&b';') {
            return false;
        }
        offset = skip_optional_whitespace(value, offset + 1);
        let Some(name_end) = token_end(value, offset) else {
            return false;
        };
        if value.get(name_end) != Some(&b'=') {
            return false;
        }
        offset = name_end + 1;
        if value.get(offset) == Some(&b'"') {
            let Some(end) = quoted_string_end(value, offset + 1) else {
                return false;
            };
            offset = end;
        } else {
            let Some(end) = token_end(value, offset) else {
                return false;
            };
            offset = end;
        }
    }
}

fn quoted_string_end(value: &[u8], mut offset: usize) -> Option<usize> {
    while let Some(&byte) = value.get(offset) {
        match byte {
            b'"' => return Some(offset + 1),
            b'\\' => {
                let &quoted = value.get(offset + 1)?;
                if !is_quoted_pair_byte(quoted) {
                    return None;
                }
                offset += 2;
            }
            _ if is_quoted_text_byte(byte) => offset += 1,
            _ => return None,
        }
    }
    None
}

fn token_end(value: &[u8], start: usize) -> Option<usize> {
    let mut end = start;
    while value.get(end).is_some_and(|byte| is_token_byte(*byte)) {
        end += 1;
    }
    (end > start).then_some(end)
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn is_quoted_text_byte(byte: u8) -> bool {
    matches!(byte, b'\t' | b' ' | b'!' | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
}

fn is_quoted_pair_byte(byte: u8) -> bool {
    matches!(byte, b'\t' | b' ' | 0x21..=0x7e | 0x80..=0xff)
}

fn trim_optional_whitespace(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

fn skip_optional_whitespace(value: &[u8], mut offset: usize) -> usize {
    while value
        .get(offset)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        offset += 1;
    }
    offset
}

async fn score_bytes(state: &AppState, bytes: &Bytes) -> Response {
    let input: ScoreInput = match serde_json::from_slice(bytes) {
        Ok(input) => input,
        Err(_) => return invalid_request("request JSON is invalid"),
    };
    if input.variants.is_empty() || input.variants.len() > MAX_VARIANTS {
        return invalid_request("variants must contain between 1 and 100 values");
    }
    let gene = match input.gene {
        Some(gene) => match EnsemblGeneId::from_str(&gene) {
            Ok(gene) => Some(gene),
            Err(_) => return invalid_request("gene is not a stable Ensembl gene ID"),
        },
        None => None,
    };
    let mut variants = Vec::with_capacity(input.variants.len());
    for value in input.variants {
        match super::parse_variant(&value) {
            Ok(variant) => variants.push(variant),
            Err(_) => return invalid_request("variant is invalid"),
        }
    }
    let mut outputs: Vec<Option<ScoreOutcome>> = (0..variants.len()).map(|_| None).collect();
    let mut pending = Vec::new();
    for (index, variant) in variants.into_iter().enumerate() {
        let decision = if input.model_only.unwrap_or(false) {
            BatchDecision::Model(PendingModel::Explicit(ExplicitModelRequest::new(
                RouteRequest::new(variant, gene),
            )))
        } else {
            match state.lookup.inspect(RouteRequest::new(variant, gene)) {
                Ok(RouteDecision::Authoritative(result)) => BatchDecision::Authoritative(result),
                Ok(RouteDecision::ModelRequired(required)) => {
                    BatchDecision::Model(PendingModel::Lookup(required))
                }
                Err(_) => {
                    return service_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "SCORING_FAILED",
                        "scoring failed",
                    );
                }
            }
        };
        match decision {
            BatchDecision::Authoritative(result) => {
                outputs[index] = Some(ScoreOutcome::Complete(result));
            }
            BatchDecision::Model(required) => {
                let key = CacheKey::new(required.variant(), state.cache_identity.clone());
                pending.push(JobItem {
                    output_index: index,
                    pending: required,
                    key,
                });
            }
        }
    }
    let pending = match handler_cache_hits(state, pending).await {
        Ok(pending) => pending,
        Err(_) => {
            return service_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SCORING_FAILED",
                "scoring failed",
            );
        }
    };
    let mut misses = Vec::new();
    for (item, cached) in pending {
        if let Some(records) = cached {
            outputs[item.output_index] = Some(ScoreOutcome::Complete(routed_from_cached(
                &item.pending,
                records,
                state.provenance.clone(),
            )));
        } else {
            misses.push(item);
        }
    }
    if misses.len() > MAX_MODEL_MISSES {
        return service_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "MODEL_BATCH_TOO_LARGE",
            "request requires more than 10 uncached model variants",
        );
    }
    if !misses.is_empty() {
        let (sender, receiver) = oneshot::channel();
        match state.dispatcher.admit(ModelJob {
            items: misses,
            response: sender,
            slot: JobSlot::Unassigned,
        }) {
            Ok(()) => {}
            Err(AdmissionError::Full {
                retry_after_seconds,
            }) => {
                let mut response = service_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "MODEL_QUEUE_FULL",
                    "model queue is full",
                );
                if let Some(seconds) = retry_after_seconds {
                    response.headers_mut().insert(
                        header::RETRY_AFTER,
                        HeaderValue::from_str(&seconds.to_string())
                            .expect("bounded retry delay is a valid header"),
                    );
                }
                return response;
            }
            Err(AdmissionError::Draining) => {
                return service_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "SHUTTING_DOWN",
                    "service is shutting down",
                );
            }
            Err(AdmissionError::Unavailable) => {
                return service_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "MODEL_WORKER_UNAVAILABLE",
                    "model worker failed",
                );
            }
        }
        match receiver.await {
            Ok(Ok(results)) => {
                if state.dispatcher.snapshot().readiness == FAILED {
                    return service_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "MODEL_WORKER_UNAVAILABLE",
                        "model worker failed",
                    );
                }
                for (index, result) in results {
                    outputs[index] = Some(result);
                }
            }
            Ok(Err(WorkerReply::BackendFailure(failure))) => {
                return service_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    failure.code,
                    "scoring failed",
                );
            }
            Ok(Err(WorkerReply::Unavailable)) | Err(_) => {
                return service_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "MODEL_WORKER_UNAVAILABLE",
                    "model worker failed",
                );
            }
        }
    }
    if outputs
        .iter()
        .all(|output| matches!(output, Some(ScoreOutcome::Rejected(_))))
    {
        return service_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "MODEL_REJECTED",
            "scoring failed",
        );
    }
    let mut results = Vec::with_capacity(outputs.len());
    for output in outputs {
        let Some(output) = output else {
            return service_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SCORING_FAILED",
                "scoring failed",
            );
        };
        let rendered: Result<Box<RawValue>, ()> = match output {
            ScoreOutcome::Complete(result) => render_result_raw(result).map_err(|_| ()),
            ScoreOutcome::Rejected(variant) => render_rejection_raw(variant),
        };
        match rendered {
            Ok(value) => results.push(value),
            Err(_) => {
                return service_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SCORING_FAILED",
                    "scoring failed",
                );
            }
        }
    }
    json_response(StatusCode::OK, &ScoreOutput { results })
}

#[derive(Serialize)]
struct RejectedScoreOutput {
    assembly: &'static str,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "alt")]
    alternate: String,
    status: &'static str,
    records: [(); 0],
    source_reference_ambiguities: [(); 0],
    error: ErrorBody<'static>,
}

fn render_rejection_raw(variant: Grch38Variant) -> Result<Box<RawValue>, ()> {
    let value = RejectedScoreOutput {
        assembly: "GRCh38",
        contig: variant.contig().to_string(),
        position: variant.position().get(),
        reference: variant.reference().to_owned(),
        alternate: variant.alternate().to_owned(),
        status: "rejected",
        records: [],
        source_reference_ambiguities: [],
        error: ErrorBody {
            code: "MODEL_REJECTED",
            message: "scoring failed",
        },
    };
    serde_json::value::to_raw_value(&value).map_err(|_| ())
}

async fn handler_cache_hits(
    state: &AppState,
    items: Vec<JobItem>,
) -> Result<Vec<(JobItem, Option<Vec<ModelGeneScoreRecord>>)>, Failure> {
    let permit = Arc::clone(&state.cache_gate)
        .acquire_owned()
        .await
        .map_err(|_| cache_task_failure())?;
    let cache = Arc::clone(&state.handler_cache);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let value = match cache.get(&item.key) {
                Ok(value) => value,
                Err(pangopup_cache::CacheError::Busy) => None,
                Err(error) => return Err(map_cache_error(error)),
            };
            results.push((item, value));
        }
        Ok(results)
    })
    .await
    .map_err(|_| cache_task_failure())?
}

fn cache_task_failure() -> Failure {
    Failure {
        code: "MODEL_CACHE_INVALID",
        message: "model cache task failed".to_owned(),
        exit: 1,
        details: None,
    }
}

async fn not_found() -> Response {
    service_error(StatusCode::NOT_FOUND, "NOT_FOUND", "route not found")
}

fn invalid_request(message: &'static str) -> Response {
    service_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", message)
}

fn service_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    debug_assert!(message.len() <= 256 && !message.contains('\n'));
    json_response(
        status,
        &ErrorEnvelope {
            error: ErrorBody { code, message },
        },
    )
}

fn json_response(status: StatusCode, value: &impl Serialize) -> Response {
    let mut bytes = serde_json::to_vec(value).expect("service response is serializable");
    bytes.push(b'\n');
    let length = bytes.len();
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).expect("bounded response length"),
    );
    response
}

async fn shutdown(dispatcher: Dispatcher) {
    let mut signals = signal_receiver();
    if shutdown_with_signals(&dispatcher, &mut signals).await == ShutdownOutcome::Forced {
        std::process::exit(130);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownOutcome {
    Graceful,
    Forced,
}

async fn shutdown_with_signals(
    dispatcher: &Dispatcher,
    signals: &mut mpsc::UnboundedReceiver<()>,
) -> ShutdownOutcome {
    tokio::select! {
        signal = signals.recv() => {
            if signal.is_some() {
                stop_admission(dispatcher);
            }
        },
        () = wait_worker_failure(dispatcher) => {}
    }
    loop {
        let snapshot = dispatcher.snapshot();
        if snapshot.running == 0 && snapshot.queued == 0 {
            dispatcher.close();
            return ShutdownOutcome::Graceful;
        }
        tokio::select! {
            signal = signals.recv() => {
                if signal.is_some() {
                    return ShutdownOutcome::Forced;
                }
            },
            () = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
    }
}

fn signal_receiver() -> mpsc::UnboundedReceiver<()> {
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(signal_pump(sender));
    receiver
}

fn stop_admission(dispatcher: &Dispatcher) {
    dispatcher.stop_admission();
}

async fn wait_worker_failure(dispatcher: &Dispatcher) {
    while dispatcher.snapshot().readiness != FAILED {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(unix)]
async fn signal_pump(sender: mpsc::UnboundedSender<()>) {
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("install SIGINT handler");
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    loop {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
        if sender.send(()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
