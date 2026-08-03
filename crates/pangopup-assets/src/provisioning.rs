//! Whole-product provisioning and status composition.

use crate::{
    AssetError, AssetErrorKind, LocalStatus, RuntimeLocalStatus, RuntimeSyncOutcome, SyncEvent,
    SyncOutcome, inspect_runtime_cache, local_status, runtime_local_status, sync_assets_observed,
    sync_runtime_assets_observed,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComponentError {
    pub status: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SnvSyncObservation {
    Complete(SyncOutcome),
    Error(ComponentError),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RuntimeSyncObservation {
    Complete(RuntimeSyncOutcome),
    Error(ComponentError),
    NotAttempted(RuntimeNotAttempted),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeNotAttempted {
    pub status: &'static str,
    pub reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CombinedSyncOutcome {
    pub status: &'static str,
    pub snv: SyncOutcome,
    pub runtime: RuntimeSyncOutcome,
    pub downloaded_bytes: u64,
    pub resumed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CombinedSyncIncomplete {
    pub snv: SnvSyncObservation,
    pub runtime: RuntimeSyncObservation,
}

#[derive(Debug)]
pub enum CombinedSyncResult {
    Ready(CombinedSyncOutcome),
    Incomplete(CombinedSyncIncomplete),
}

#[cfg(test)]
type SnvSync<'a> = dyn Fn(&Path, Option<&Path>, bool) -> Result<SyncOutcome, AssetError> + 'a;
#[cfg(test)]
type RuntimeSync<'a> =
    dyn Fn(&Path, Option<&Path>, bool) -> Result<RuntimeSyncOutcome, AssetError> + 'a;
#[cfg(test)]
type RuntimeInspect<'a> =
    dyn Fn(Option<&Path>) -> Result<crate::RuntimeCacheInspection, AssetError> + 'a;

pub fn sync_all_assets(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
) -> Result<CombinedSyncResult, AssetError> {
    sync_all_assets_observed(data_root, cache_root, offline, &mut |_| {})
}

pub fn sync_all_assets_observed(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
    observer: &mut dyn FnMut(SyncEvent),
) -> Result<CombinedSyncResult, AssetError> {
    let _lock = crate::local::acquire_provisioning_lock(data_root)?;
    let snv = match sync_assets_observed(data_root, cache_root, offline, observer) {
        Ok(outcome) => outcome,
        Err(error) => {
            let runtime = if offline && error.kind() == AssetErrorKind::AssetsMissing {
                match inspect_runtime_cache(cache_root) {
                    Ok(crate::RuntimeCacheInspection::Complete) => {
                        RuntimeSyncObservation::NotAttempted(RuntimeNotAttempted {
                            status: "not_attempted",
                            reason: "snv_sync_failed",
                            cache: Some("complete"),
                        })
                    }
                    Err(runtime) => RuntimeSyncObservation::Error(component_error(runtime, false)),
                }
            } else {
                RuntimeSyncObservation::NotAttempted(RuntimeNotAttempted {
                    status: "not_attempted",
                    reason: "snv_sync_failed",
                    cache: None,
                })
            };
            return Ok(CombinedSyncResult::Incomplete(CombinedSyncIncomplete {
                snv: SnvSyncObservation::Error(component_error(error, false)),
                runtime,
            }));
        }
    };
    let snv_downloaded = snv.downloaded_bytes;
    let snv_resumed = snv.resumed_bytes;
    let (runtime_result, event_error) = {
        let mut event_error = None;
        let mut translate =
            |event: SyncEvent| match offset_event(event, snv_downloaded, snv_resumed) {
                Ok(event) => observer(event),
                Err(error) => event_error = Some(error),
            };
        let result = sync_runtime_assets_observed(data_root, cache_root, offline, &mut translate);
        (result, event_error)
    };
    if let Some(error) = event_error {
        return Err(error);
    }
    let runtime = match runtime_result {
        Ok(outcome) => outcome,
        Err(error) => {
            return Ok(CombinedSyncResult::Incomplete(CombinedSyncIncomplete {
                snv: SnvSyncObservation::Complete(snv),
                runtime: RuntimeSyncObservation::Error(component_error(error, true)),
            }));
        }
    };
    let downloaded_bytes = snv_downloaded
        .checked_add(runtime.downloaded_bytes)
        .ok_or_else(counter_overflow)?;
    let resumed_bytes = snv_resumed
        .checked_add(runtime.resumed_bytes)
        .ok_or_else(counter_overflow)?;
    observer(SyncEvent::Complete {
        downloaded_bytes,
        resumed_bytes,
    });
    Ok(CombinedSyncResult::Ready(CombinedSyncOutcome {
        status: "ready",
        snv,
        runtime,
        downloaded_bytes,
        resumed_bytes,
    }))
}

fn offset_event(event: SyncEvent, downloaded: u64, resumed: u64) -> Result<SyncEvent, AssetError> {
    Ok(match event {
        SyncEvent::Transfer {
            component,
            asset_name,
            attempt,
            max_attempts,
            mode,
            member_bytes,
            member_total,
            invocation_downloaded_bytes,
            invocation_resumed_bytes,
        } => SyncEvent::Transfer {
            component,
            asset_name,
            attempt,
            max_attempts,
            mode,
            member_bytes,
            member_total,
            invocation_downloaded_bytes: downloaded
                .checked_add(invocation_downloaded_bytes)
                .ok_or_else(counter_overflow)?,
            invocation_resumed_bytes: resumed
                .checked_add(invocation_resumed_bytes)
                .ok_or_else(counter_overflow)?,
        },
        SyncEvent::Complete {
            downloaded_bytes,
            resumed_bytes,
        } => SyncEvent::Complete {
            downloaded_bytes: downloaded
                .checked_add(downloaded_bytes)
                .ok_or_else(counter_overflow)?,
            resumed_bytes: resumed
                .checked_add(resumed_bytes)
                .ok_or_else(counter_overflow)?,
        },
        other => other,
    })
}

#[cfg(test)]
fn sync_all_assets_with(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
    snv_sync: &SnvSync<'_>,
    runtime_sync: &RuntimeSync<'_>,
    runtime_inspect: &RuntimeInspect<'_>,
) -> Result<CombinedSyncResult, AssetError> {
    let _lock = crate::local::acquire_provisioning_lock(data_root)?;
    sync_all_with(
        data_root,
        cache_root,
        offline,
        snv_sync,
        runtime_sync,
        runtime_inspect,
    )
}

#[cfg(test)]
fn sync_all_with(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
    snv_sync: &SnvSync<'_>,
    runtime_sync: &RuntimeSync<'_>,
    runtime_inspect: &RuntimeInspect<'_>,
) -> Result<CombinedSyncResult, AssetError> {
    let snv = match snv_sync(data_root, cache_root, offline) {
        Ok(outcome) => outcome,
        Err(error) => {
            let runtime = if offline && error.kind() == AssetErrorKind::AssetsMissing {
                match runtime_inspect(cache_root) {
                    Ok(crate::RuntimeCacheInspection::Complete) => {
                        RuntimeSyncObservation::NotAttempted(RuntimeNotAttempted {
                            status: "not_attempted",
                            reason: "snv_sync_failed",
                            cache: Some("complete"),
                        })
                    }
                    Err(runtime) => RuntimeSyncObservation::Error(component_error(runtime, false)),
                }
            } else {
                RuntimeSyncObservation::NotAttempted(RuntimeNotAttempted {
                    status: "not_attempted",
                    reason: "snv_sync_failed",
                    cache: None,
                })
            };
            return Ok(CombinedSyncResult::Incomplete(CombinedSyncIncomplete {
                snv: SnvSyncObservation::Error(component_error(error, false)),
                runtime,
            }));
        }
    };
    let runtime = match runtime_sync(data_root, cache_root, offline) {
        Ok(outcome) => outcome,
        Err(error) => {
            return Ok(CombinedSyncResult::Incomplete(CombinedSyncIncomplete {
                snv: SnvSyncObservation::Complete(snv),
                runtime: RuntimeSyncObservation::Error(component_error(error, true)),
            }));
        }
    };
    let downloaded_bytes = snv
        .downloaded_bytes
        .checked_add(runtime.downloaded_bytes)
        .ok_or_else(counter_overflow)?;
    let resumed_bytes = snv
        .resumed_bytes
        .checked_add(runtime.resumed_bytes)
        .ok_or_else(counter_overflow)?;
    Ok(CombinedSyncResult::Ready(CombinedSyncOutcome {
        status: "ready",
        snv,
        runtime,
        downloaded_bytes,
        resumed_bytes,
    }))
}

fn counter_overflow() -> AssetError {
    AssetError::new(
        AssetErrorKind::AssetStateInvalid,
        "asset byte counter overflow",
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SnvReadyStatus {
    pub status: &'static str,
    pub bundle_id: String,
    pub transport_id: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeReadyStatus {
    pub status: &'static str,
    pub profile_id: String,
    pub snv_bundle_id: String,
    pub model_bundle_id: String,
    pub reference_bundle_id: String,
    pub mask_sha256: String,
    pub model_path: PathBuf,
    pub reference_path: PathBuf,
    pub mask_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComponentState {
    pub status: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SnvStatusObservation {
    Ready(SnvReadyStatus),
    State(ComponentState),
    Error(ComponentError),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RuntimeStatusObservation {
    Ready(RuntimeReadyStatus),
    State(ComponentState),
    Error(ComponentError),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CombinedLocalStatus {
    pub status: &'static str,
    pub data_dir: PathBuf,
    pub syncing: bool,
    pub installing: bool,
    pub snv: SnvStatusObservation,
    pub runtime: RuntimeStatusObservation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CombinedStatusInvalid {
    pub snv: SnvStatusObservation,
    pub runtime: RuntimeStatusObservation,
}

#[derive(Debug)]
pub enum CombinedStatusResult {
    Valid(CombinedLocalStatus),
    Invalid(CombinedStatusInvalid),
}

pub fn combined_local_status(data_root: &Path) -> Result<CombinedStatusResult, AssetError> {
    combined_local_status_with(data_root, &|locked| {
        crate::runtime_install::runtime_local_status_locked(locked)
    })
}

fn combined_local_status_with(
    data_root: &Path,
    locked_runtime_status: &dyn Fn(
        &crate::local::LockedRoot,
    ) -> Result<RuntimeLocalStatus, AssetError>,
) -> Result<CombinedStatusResult, AssetError> {
    let syncing = crate::local::probe_provisioning_lock(data_root)?;
    match crate::local::acquire_install_observation(data_root) {
        Ok(crate::local::InstallObservation::Acquired(locked)) => {
            let snv = observe_snv(crate::local::local_status_locked(&locked));
            let runtime = observe_runtime(locked_runtime_status(&locked));
            Ok(finish_status(data_root, syncing, false, true, snv, runtime))
        }
        Ok(crate::local::InstallObservation::MissingRoot) => {
            Ok(CombinedStatusResult::Valid(combined_status(
                data_root,
                syncing,
                false,
                SnvStatusObservation::State(ComponentState { status: "missing" }),
                RuntimeStatusObservation::State(ComponentState { status: "missing" }),
            )))
        }
        Ok(crate::local::InstallObservation::MissingAuthority) => {
            let snv = observe_snv(local_status(data_root));
            let runtime = observe_runtime(runtime_local_status(data_root));
            if matches!(
                (&snv, &runtime),
                (
                    SnvStatusObservation::State(ComponentState { status: "missing" }),
                    RuntimeStatusObservation::State(ComponentState { status: "missing" })
                )
            ) {
                return Ok(CombinedStatusResult::Valid(combined_status(
                    data_root, syncing, false, snv, runtime,
                )));
            }
            Err(AssetError::new(
                AssetErrorKind::AssetStateInvalid,
                "installed asset state requires an existing .install.lock authority",
            ))
        }
        Err(error) if error.kind() == AssetErrorKind::AssetLocked => {
            let snv = observe_snv(local_status(data_root));
            let runtime = observe_runtime(runtime_local_status(data_root));
            Ok(finish_status(data_root, syncing, true, false, snv, runtime))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
pub(crate) fn combined_local_status_with_runtime_profile(
    data_root: &Path,
    profile: &crate::RuntimeProfile,
) -> Result<CombinedStatusResult, AssetError> {
    combined_local_status_with(data_root, &|locked| {
        crate::runtime_install::runtime_local_status_locked_with_profile(locked, profile)
    })
}

fn finish_status(
    data_root: &Path,
    syncing: bool,
    installing: bool,
    coherent: bool,
    snv: SnvStatusObservation,
    mut runtime: RuntimeStatusObservation,
) -> CombinedStatusResult {
    if coherent {
        if let (
            SnvStatusObservation::Ready(snv_ready),
            RuntimeStatusObservation::Ready(runtime_ready),
        ) = (&snv, &runtime)
            && snv_ready.bundle_id != runtime_ready.snv_bundle_id
        {
            runtime = RuntimeStatusObservation::Error(ComponentError {
                status: "error",
                code: "PROFILE_INCOMPATIBLE",
                message: "installed runtime profile is bound to a different SNV bundle".to_owned(),
            });
        }
        if matches!(snv, SnvStatusObservation::Error(_))
            || matches!(runtime, RuntimeStatusObservation::Error(_))
        {
            return CombinedStatusResult::Invalid(CombinedStatusInvalid { snv, runtime });
        }
    }
    CombinedStatusResult::Valid(combined_status(
        data_root, syncing, installing, snv, runtime,
    ))
}

fn combined_status(
    data_root: &Path,
    syncing: bool,
    installing: bool,
    snv: SnvStatusObservation,
    runtime: RuntimeStatusObservation,
) -> CombinedLocalStatus {
    let ready = usize::from(matches!(snv, SnvStatusObservation::Ready(_)))
        + usize::from(matches!(runtime, RuntimeStatusObservation::Ready(_)));
    CombinedLocalStatus {
        status: match ready {
            2 => "ready",
            1 => "partial",
            _ => "missing",
        },
        data_dir: data_root.to_owned(),
        syncing,
        installing,
        snv,
        runtime,
    }
}

fn observe_snv(status: Result<LocalStatus, AssetError>) -> SnvStatusObservation {
    match status {
        Ok(LocalStatus::Ready { active, .. }) => SnvStatusObservation::Ready(SnvReadyStatus {
            status: "ready",
            bundle_id: active.bundle_id,
            transport_id: active.transport_id,
            path: active.path,
        }),
        Ok(LocalStatus::Installing { .. }) => SnvStatusObservation::State(ComponentState {
            status: "installing",
        }),
        Ok(LocalStatus::Missing { .. }) => {
            SnvStatusObservation::State(ComponentState { status: "missing" })
        }
        Err(error) => SnvStatusObservation::Error(component_error(error, false)),
    }
}

fn observe_runtime(status: Result<RuntimeLocalStatus, AssetError>) -> RuntimeStatusObservation {
    match status {
        Ok(RuntimeLocalStatus::Ready {
            profile_id,
            snv_bundle_id,
            model_bundle_id,
            reference_bundle_id,
            mask_sha256,
            model_path,
            reference_path,
            mask_path,
            ..
        }) => RuntimeStatusObservation::Ready(RuntimeReadyStatus {
            status: "ready",
            profile_id,
            snv_bundle_id,
            model_bundle_id,
            reference_bundle_id,
            mask_sha256,
            model_path,
            reference_path,
            mask_path,
        }),
        Ok(RuntimeLocalStatus::Installing { .. }) => {
            RuntimeStatusObservation::State(ComponentState {
                status: "installing",
            })
        }
        Ok(RuntimeLocalStatus::Missing { .. }) => {
            RuntimeStatusObservation::State(ComponentState { status: "missing" })
        }
        Err(error) => RuntimeStatusObservation::Error(component_error(error, true)),
    }
}

fn component_error(error: AssetError, runtime: bool) -> ComponentError {
    let code = if runtime {
        match error.kind() {
            AssetErrorKind::TransportIncompatible => "PROFILE_INCOMPATIBLE",
            AssetErrorKind::StagingInvalid => "PROFILE_UNSAFE",
            AssetErrorKind::BundleInvalid => "PROFILE_CORRUPT",
            _ => error.kind().code(),
        }
    } else {
        match error.kind() {
            AssetErrorKind::InstallConflict => "BUNDLE_INCOMPATIBLE",
            _ => error.kind().code(),
        }
    };
    ComponentError {
        status: "error",
        code,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pangopup_core::{DnaBase, GenomicPosition, Grch38Contig, Grch38Snv, ScoreProvider};
    use std::cell::RefCell;
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::time::SystemTime;

    fn tree_snapshot(root: &Path) -> Vec<(PathBuf, u32, u64, SystemTime)> {
        fn visit(base: &Path, current: &Path, entries: &mut Vec<(PathBuf, u32, u64, SystemTime)>) {
            let mut children = fs::read_dir(current)
                .expect("read installed tree")
                .map(|entry| entry.expect("directory entry").path())
                .collect::<Vec<_>>();
            children.sort();
            for path in children {
                let metadata = fs::symlink_metadata(&path).expect("installed metadata");
                entries.push((
                    path.strip_prefix(base).expect("relative path").to_owned(),
                    metadata.mode(),
                    metadata.len(),
                    metadata.modified().expect("modified time"),
                ));
                if metadata.is_dir() {
                    visit(base, &path, entries);
                }
            }
        }

        let mut entries = Vec::new();
        if root.exists() {
            visit(root, root, &mut entries);
        }
        entries
    }

    fn snv(downloaded: u64, resumed: u64) -> SyncOutcome {
        SyncOutcome {
            status: "installed",
            profile: "snv-grch38-v1".to_owned(),
            bundle_id: "sha256:snv".to_owned(),
            transport_id: "sha256:snv-transport".to_owned(),
            path: PathBuf::from("/data/snv"),
            downloaded_bytes: downloaded,
            resumed_bytes: resumed,
        }
    }

    fn runtime(downloaded: u64, resumed: u64) -> RuntimeSyncOutcome {
        RuntimeSyncOutcome {
            status: "reused",
            transport_id: "sha256:runtime-transport".to_owned(),
            profile_id: "sha256:profile".to_owned(),
            snv_bundle_id: "sha256:snv".to_owned(),
            model_bundle_id: "sha256:model".to_owned(),
            reference_bundle_id: "sha256:reference".to_owned(),
            mask_sha256: "sha256:mask".to_owned(),
            path: PathBuf::from("/data/runtime"),
            downloaded_bytes: downloaded,
            resumed_bytes: resumed,
        }
    }

    fn snv_ready(bundle_id: &str) -> SnvStatusObservation {
        SnvStatusObservation::Ready(SnvReadyStatus {
            status: "ready",
            bundle_id: bundle_id.to_owned(),
            transport_id: "sha256:transport".to_owned(),
            path: PathBuf::from("/data/snv"),
        })
    }

    fn runtime_ready(snv_bundle_id: &str) -> RuntimeStatusObservation {
        RuntimeStatusObservation::Ready(RuntimeReadyStatus {
            status: "ready",
            profile_id: "sha256:profile".to_owned(),
            snv_bundle_id: snv_bundle_id.to_owned(),
            model_bundle_id: "sha256:model".to_owned(),
            reference_bundle_id: "sha256:reference".to_owned(),
            mask_sha256: "sha256:mask".to_owned(),
            model_path: PathBuf::from("/data/model"),
            reference_path: PathBuf::from("/data/reference"),
            mask_path: PathBuf::from("/data/mask"),
        })
    }

    #[test]
    fn component_order_and_checked_totals_are_fixed() {
        let calls = RefCell::new(Vec::new());
        let result = sync_all_with(
            Path::new("/data"),
            Some(Path::new("/cache")),
            false,
            &|_, _, _| {
                calls.borrow_mut().push("snv");
                Ok(snv(10, 3))
            },
            &|_, _, _| {
                calls.borrow_mut().push("runtime");
                Ok(runtime(20, 4))
            },
            &|_| panic!("inspection is not used"),
        )
        .expect("composition");
        assert_eq!(&*calls.borrow(), &["snv", "runtime"]);
        let CombinedSyncResult::Ready(result) = result else {
            panic!("ready")
        };
        assert_eq!((result.downloaded_bytes, result.resumed_bytes), (30, 7));

        let error = sync_all_with(
            Path::new("/data"),
            Some(Path::new("/cache")),
            false,
            &|_, _, _| Ok(snv(u64::MAX, 0)),
            &|_, _, _| Ok(runtime(1, 0)),
            &|_| panic!("inspection is not used"),
        )
        .expect_err("overflow");
        assert_eq!(error.kind(), AssetErrorKind::AssetStateInvalid);
        assert_eq!(error.to_string(), "asset byte counter overflow");
    }

    #[test]
    fn combined_observer_offsets_runtime_and_complete_counters_exactly() {
        let runtime_events = [
            SyncEvent::Transfer {
                component: crate::SyncComponent::Runtime,
                asset_name: "model.onnx".to_owned(),
                attempt: 2,
                max_attempts: 4,
                mode: crate::SyncTransferMode::Resume,
                member_bytes: 80,
                member_total: 100,
                invocation_downloaded_bytes: 20,
                invocation_resumed_bytes: 10,
            },
            SyncEvent::Phase {
                component: crate::SyncComponent::Runtime,
                phase: crate::SyncPhase::Ready,
            },
        ];
        let mut observed = runtime_events
            .into_iter()
            .map(|event| offset_event(event, 30, 7).expect("checked runtime offset"))
            .collect::<Vec<_>>();
        observed.push(SyncEvent::Complete {
            downloaded_bytes: 50,
            resumed_bytes: 17,
        });
        assert_eq!(
            observed,
            vec![
                SyncEvent::Transfer {
                    component: crate::SyncComponent::Runtime,
                    asset_name: "model.onnx".to_owned(),
                    attempt: 2,
                    max_attempts: 4,
                    mode: crate::SyncTransferMode::Resume,
                    member_bytes: 80,
                    member_total: 100,
                    invocation_downloaded_bytes: 50,
                    invocation_resumed_bytes: 17,
                },
                SyncEvent::Phase {
                    component: crate::SyncComponent::Runtime,
                    phase: crate::SyncPhase::Ready,
                },
                SyncEvent::Complete {
                    downloaded_bytes: 50,
                    resumed_bytes: 17,
                },
            ]
        );
        assert_eq!(
            offset_event(
                SyncEvent::Complete {
                    downloaded_bytes: 1,
                    resumed_bytes: 0,
                },
                u64::MAX,
                0,
            )
            .expect_err("overflow is fatal")
            .kind(),
            AssetErrorKind::AssetStateInvalid
        );
    }

    #[test]
    fn online_short_circuits_and_offline_inspects_runtime_cache() {
        let online_calls = RefCell::new(Vec::new());
        let online = sync_all_with(
            Path::new("/data"),
            Some(Path::new("/cache")),
            false,
            &|_, _, _| Err(AssetError::new(AssetErrorKind::AssetDownload, "snv failed")),
            &|_, _, _| {
                online_calls.borrow_mut().push("runtime");
                Ok(runtime(0, 0))
            },
            &|_| {
                online_calls.borrow_mut().push("inspect");
                Ok(crate::RuntimeCacheInspection::Complete)
            },
        )
        .expect("incomplete");
        assert!(online_calls.borrow().is_empty());
        assert!(matches!(online, CombinedSyncResult::Incomplete(_)));

        let offline = sync_all_with(
            Path::new("/data"),
            Some(Path::new("/cache")),
            true,
            &|_, _, _| {
                Err(AssetError::new(
                    AssetErrorKind::AssetsMissing,
                    "snv missing",
                ))
            },
            &|_, _, _| panic!("runtime install is not attempted"),
            &|_| Ok(crate::RuntimeCacheInspection::Complete),
        )
        .expect("offline incomplete");
        let CombinedSyncResult::Incomplete(incomplete) = offline else {
            panic!("incomplete")
        };
        assert!(matches!(
            incomplete.runtime,
            RuntimeSyncObservation::NotAttempted(RuntimeNotAttempted {
                cache: Some("complete"),
                ..
            })
        ));
    }

    #[test]
    fn empty_status_is_exact_and_sync_lock_is_observable_and_nonblocking() {
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let CombinedStatusResult::Valid(missing) =
            combined_local_status(&root).expect("missing status")
        else {
            panic!("valid missing status")
        };
        assert_eq!(missing.status, "missing");
        assert!(!missing.syncing);
        assert!(!missing.installing);
        assert!(matches!(
            missing.snv,
            SnvStatusObservation::State(ComponentState { status: "missing" })
        ));
        assert!(matches!(
            missing.runtime,
            RuntimeStatusObservation::State(ComponentState { status: "missing" })
        ));
        assert!(!root.exists(), "status does not create the data root");

        let owner = crate::local::acquire_provisioning_lock(&root).expect("owner");
        let loser = crate::local::acquire_provisioning_lock(&root).expect_err("loser");
        assert_eq!(loser.kind(), AssetErrorKind::AssetLocked);
        assert_eq!(
            loser.to_string(),
            "another Pangopup synchronization is in progress"
        );
        let CombinedStatusResult::Valid(syncing) =
            combined_local_status(&root).expect("syncing status")
        else {
            panic!("valid syncing status")
        };
        assert!(syncing.syncing);
        drop(owner);
        let replacement = crate::local::acquire_provisioning_lock(&root).expect("released");
        drop(replacement);
    }

    #[test]
    fn shared_install_lock_yields_nonwaiting_installing_observations() {
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let owner = crate::local::acquire_shared_install_lock(&root).expect("install owner");
        let CombinedStatusResult::Valid(status) =
            combined_local_status(&root).expect("installing status")
        else {
            panic!("valid installing status")
        };
        assert!(status.installing);
        assert!(matches!(
            status.snv,
            SnvStatusObservation::State(ComponentState {
                status: "installing"
            })
        ));
        assert!(matches!(
            status.runtime,
            RuntimeStatusObservation::State(ComponentState {
                status: "installing"
            })
        ));
        drop(owner);
    }

    #[test]
    fn installed_partial_status_is_read_only_and_requires_lock_authority() {
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let transport = temp.path().join("transport");
        crate::pack_bundle(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/snv-regression/bundle"),
            &transport,
        )
        .expect("pack miniature bundle");
        crate::install_transport(&transport, &root).expect("install miniature bundle");

        let before = tree_snapshot(&root);
        let CombinedStatusResult::Valid(status) =
            combined_local_status(&root).expect("read-only status")
        else {
            panic!("valid partial status")
        };
        assert_eq!(status.status, "partial");
        assert!(!status.installing);
        assert!(matches!(status.snv, SnvStatusObservation::Ready(_)));
        assert!(matches!(
            status.runtime,
            RuntimeStatusObservation::State(ComponentState { status: "missing" })
        ));
        assert_eq!(
            tree_snapshot(&root),
            before,
            "status changed installed state"
        );

        fs::remove_file(root.join(".install.lock")).expect("remove lock authority");
        let missing_authority = combined_local_status(&root).expect_err("authority is required");
        assert_eq!(missing_authority.kind(), AssetErrorKind::AssetStateInvalid);
        assert_eq!(
            missing_authority.to_string(),
            "installed asset state requires an existing .install.lock authority"
        );
        assert!(
            !root.join(".install.lock").exists(),
            "status repaired missing authority"
        );
    }

    #[test]
    fn offline_empty_pair_reports_both_bounded_missing_inventories() {
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let cache = temp.path().join("cache");
        let result = sync_all_assets(&root, Some(&cache), true).expect("bounded result");
        let CombinedSyncResult::Incomplete(incomplete) = result else {
            panic!("incomplete")
        };
        let SnvSyncObservation::Error(snv) = incomplete.snv else {
            panic!("SNV missing")
        };
        let RuntimeSyncObservation::Error(runtime) = incomplete.runtime else {
            panic!("runtime missing")
        };
        assert_eq!(snv.code, "ASSETS_MISSING");
        assert_eq!(runtime.code, "ASSETS_MISSING");
        assert!(snv.message.contains("snv-grch38-v1 is incomplete"));
        assert!(runtime.message.contains("runtime-grch38-v1 is incomplete"));
    }

    #[test]
    fn status_aggregation_covers_partial_compatible_mismatch_and_error_policy() {
        let missing_snv = SnvStatusObservation::State(ComponentState { status: "missing" });
        let missing_runtime = RuntimeStatusObservation::State(ComponentState { status: "missing" });
        let CombinedStatusResult::Valid(partial_snv) = finish_status(
            Path::new("/data"),
            false,
            false,
            true,
            snv_ready("sha256:snv"),
            missing_runtime.clone(),
        ) else {
            panic!("SNV partial")
        };
        assert_eq!(partial_snv.status, "partial");
        let CombinedStatusResult::Valid(partial_runtime) = finish_status(
            Path::new("/data"),
            false,
            false,
            true,
            missing_snv,
            runtime_ready("sha256:snv"),
        ) else {
            panic!("runtime partial")
        };
        assert_eq!(partial_runtime.status, "partial");

        let CombinedStatusResult::Valid(ready) = finish_status(
            Path::new("/data"),
            false,
            false,
            true,
            snv_ready("sha256:snv"),
            runtime_ready("sha256:snv"),
        ) else {
            panic!("compatible ready")
        };
        assert_eq!(ready.status, "ready");

        let CombinedStatusResult::Invalid(mismatch) = finish_status(
            Path::new("/data"),
            false,
            false,
            true,
            snv_ready("sha256:other"),
            runtime_ready("sha256:snv"),
        ) else {
            panic!("mismatch")
        };
        assert!(matches!(
            mismatch.runtime,
            RuntimeStatusObservation::Error(ComponentError {
                code: "PROFILE_INCOMPATIBLE",
                ..
            })
        ));

        let observed_error = RuntimeStatusObservation::Error(ComponentError {
            status: "error",
            code: "PROFILE_CORRUPT",
            message: "broken".to_owned(),
        });
        assert!(matches!(
            finish_status(
                Path::new("/data"),
                false,
                false,
                true,
                snv_ready("sha256:snv"),
                observed_error.clone(),
            ),
            CombinedStatusResult::Invalid(_)
        ));
        assert!(matches!(
            finish_status(
                Path::new("/data"),
                false,
                true,
                false,
                snv_ready("sha256:snv"),
                observed_error,
            ),
            CombinedStatusResult::Valid(_)
        ));
    }

    #[test]
    fn top_level_lock_loser_calls_no_component_and_active_lookup_stays_usable() {
        let temp = tempfile::TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let transport = temp.path().join("transport");
        crate::pack_bundle(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/snv-regression/bundle"),
            &transport,
        )
        .expect("pack miniature bundle");
        crate::install_transport(&transport, &data).expect("install miniature bundle");
        let owner = crate::local::acquire_provisioning_lock(&data).expect("owner");
        let calls = RefCell::new(0_usize);
        let error = sync_all_assets_with(
            &data,
            Some(&temp.path().join("cache")),
            true,
            &|_, _, _| {
                *calls.borrow_mut() += 1;
                Ok(snv(0, 0))
            },
            &|_, _, _| {
                *calls.borrow_mut() += 1;
                Ok(runtime(0, 0))
            },
            &|_| {
                *calls.borrow_mut() += 1;
                Ok(crate::RuntimeCacheInspection::Complete)
            },
        )
        .expect_err("locked loser");
        assert_eq!(error.kind(), AssetErrorKind::AssetLocked);
        assert_eq!(*calls.borrow(), 0);

        let (_, bundle) = crate::open_active_bundle(&data).expect("active lookup remains openable");
        let snv = Grch38Snv::new(
            Grch38Contig::from_code(12).expect("chr12"),
            GenomicPosition::new(6_801_301).expect("position"),
            DnaBase::G,
            DnaBase::A,
        )
        .expect("SNV");
        bundle.lookup(snv, None).expect("lookup while sync is held");
        drop(owner);
    }

    #[test]
    fn complete_cached_pair_flows_through_locked_top_level_composition() {
        let temp = tempfile::TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let cache = temp.path().join("cache");
        fs::create_dir_all(&cache).expect("cache");
        fs::write(cache.join("snv.complete"), b"complete").expect("SNV cache marker");
        fs::write(cache.join("runtime.complete"), b"complete").expect("runtime cache marker");
        let order = RefCell::new(Vec::new());
        let result = sync_all_assets_with(
            &data,
            Some(&cache),
            true,
            &|_, cache, offline| {
                assert!(offline);
                assert_eq!(
                    fs::read(cache.expect("cache").join("snv.complete")).expect("cached SNV"),
                    b"complete"
                );
                order.borrow_mut().push("snv");
                Ok(snv(0, 0))
            },
            &|_, cache, offline| {
                assert!(offline);
                assert_eq!(
                    fs::read(cache.expect("cache").join("runtime.complete"))
                        .expect("cached runtime"),
                    b"complete"
                );
                order.borrow_mut().push("runtime");
                Ok(runtime(0, 0))
            },
            &|_| panic!("complete SNV does not inspect runtime early"),
        )
        .expect("complete cached pair");
        let CombinedSyncResult::Ready(outcome) = result else {
            panic!("ready")
        };
        assert_eq!(&*order.borrow(), &["snv", "runtime"]);
        assert_eq!((outcome.downloaded_bytes, outcome.resumed_bytes), (0, 0));
    }
}
