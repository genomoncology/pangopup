//! Linux local-user installation and active-bundle discovery.

use super::{
    AssetError, AssetErrorKind, MAX_FIXED11_BYTES, MAX_JSON_BYTES, MAX_NOTICE_BYTES,
    VerifiedTransport, decode_parts, inspect_transport, parse_bundle_manifest_bytes,
    reject_duplicate_json, sha256, valid_sha256,
};
use pangopup_index::{BundleOpen, IndexError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    ffi::{CString, OsStr, OsString},
    fs::{self, File},
    io::{self, ErrorKind, Read, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt, fs::PermissionsExt},
    },
    path::{Path, PathBuf},
};

const RECEIPT_SCHEMA: &str = "pangopup.install-receipt.v1";
const ACTIVE_SCHEMA: &str = "pangopup.active-profile.v1";
const STAGE_SCHEMA: &str = "pangopup.install-stage.v1";
const ROOT_MODE: u32 = 0o700;
const PRIVATE_DIR_MODE: u32 = 0o700;
const METADATA_MODE: u32 = 0o600;
const STAGE_MARKER_MODE: u32 = 0o400;
const MEMBER_MODE: u32 = 0o444;
const BUNDLE_MODE: u32 = 0o555;

#[cfg(test)]
macro_rules! crash_at {
    ($point:ident) => {
        super::install_audit::hit(super::install_audit::FaultPoint::$point)
    };
}

#[cfg(not(test))]
macro_rules! crash_at {
    ($point:ident) => {};
}

#[derive(Clone, Debug, Default)]
pub struct DataPathInputs {
    pub explicit: Option<OsString>,
    pub pangopup_data_dir: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    pub home: Option<OsString>,
}

impl DataPathInputs {
    pub fn from_environment(explicit: Option<OsString>) -> Self {
        Self {
            explicit,
            pangopup_data_dir: std::env::var_os("PANGOPUP_DATA_DIR"),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
            home: std::env::var_os("HOME"),
        }
    }
}

pub fn resolve_data_root(inputs: &DataPathInputs) -> Result<PathBuf, AssetError> {
    if let Some(value) = &inputs.explicit {
        return absolute_utf8(value, "--data-dir");
    }
    if let Some(value) = &inputs.pangopup_data_dir {
        return absolute_utf8(value, "PANGOPUP_DATA_DIR");
    }
    if let Some(value) = &inputs.xdg_data_home {
        return Ok(absolute_utf8(value, "XDG_DATA_HOME")?.join("pangopup"));
    }
    if let Some(value) = &inputs.home {
        return Ok(absolute_utf8(value, "HOME")?
            .join(".local")
            .join("share")
            .join("pangopup"));
    }
    Err(AssetError::new(
        AssetErrorKind::PathUnavailable,
        "no Linux data directory is available",
    ))
}

fn absolute_utf8(value: &OsStr, source: &str) -> Result<PathBuf, AssetError> {
    let Some(text) = value.to_str() else {
        return Err(path_invalid(source));
    };
    let path = PathBuf::from(text);
    if text.is_empty() || !path.is_absolute() {
        return Err(path_invalid(source));
    }
    Ok(path)
}

fn path_invalid(source: &str) -> AssetError {
    AssetError::new(
        AssetErrorKind::PathInvalid,
        format!("{source} must be a nonempty absolute UTF-8 path"),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveBundle {
    pub bundle_id: String,
    pub transport_id: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstallOutcome {
    pub status: &'static str,
    pub bundle_id: String,
    pub transport_id: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalStatus {
    Missing {
        data_dir: PathBuf,
    },
    Installing {
        data_dir: PathBuf,
    },
    Ready {
        active: ActiveBundle,
        installing: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    bundle_id: String,
    transport_id: String,
    members: Vec<InstalledMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledMember {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveProfile {
    schema: String,
    bundle_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageMarker {
    schema: String,
    nonce: String,
    euid: u64,
    bundle_id: String,
    transport_id: String,
}

struct Root {
    path: PathBuf,
    dir: Dir,
    euid: u32,
}

struct Dir {
    file: File,
    dev: u64,
}

struct InstallLock(File);

struct InstallScoreWriter<'a>(&'a mut File);

impl Write for InstallScoreWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        #[cfg(test)]
        if super::install_audit::fail(super::install_audit::FaultPoint::ScoreWrite) {
            return Err(io::Error::other("injected local score write failure"));
        }
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

struct ValidatedInstalled {
    active: ActiveBundle,
    opened: BundleOpen,
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        // SAFETY: flock acts on this live owned descriptor and does not retain it.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub fn install_transport(transport: &Path, data_root: &Path) -> Result<InstallOutcome, AssetError> {
    require_linux()?;
    let root = open_root(data_root, true)?.ok_or_else(|| asset_io("create data root"))?;
    let _lock = acquire_install_lock(&root)?;
    let active_before = read_active_optional_for_install(&root)?;
    let verified = inspect_transport(transport)?;
    let bundle_id = verified.manifest.bundle.bundle_id.clone();
    let transport_id = verified.manifest.transport_id.clone();
    let bundles = ensure_private_dir(&root.dir, "bundles", &root)?;
    let staging = ensure_private_dir(&root.dir, ".staging", &root)?;

    let recovered =
        reconcile_staging(&root, &bundles, &staging, Some((&bundle_id, &transport_id)))?;
    if recovered {
        let active = read_active_optional_for_install(&root)?.ok_or_else(|| {
            AssetError::new(
                AssetErrorKind::AssetStateInvalid,
                "recovery did not publish active state",
            )
        })?;
        return Ok(outcome("reused", active));
    }

    if let Some(active) = active_before {
        // Reading it before transport inspection guarantees invalid installed state is
        // reported before any attempt to replace it. Keep the value live for that proof.
        drop(active);
    }

    let suffix = identity_suffix(&bundle_id)?;
    if let Some(bundle_dir) = open_dir_optional(&bundles, suffix, &root)? {
        if file_mode(&bundle_dir.file)? != BUNDLE_MODE {
            return Err(AssetError::new(
                AssetErrorKind::InstallConflict,
                "published bundle wrapper is not immutable",
            ));
        }
        let installed = validate_installed(&root, suffix, &bundle_dir, None)?;
        if installed.active.bundle_id != bundle_id || installed.active.transport_id != transport_id
        {
            return Err(AssetError::new(
                AssetErrorKind::InstallConflict,
                "published bundle identity conflicts with the requested transport",
            ));
        }
        activate_existing(&root, &staging, &installed.active)?;
        return Ok(outcome("reused", installed.active));
    }

    install_new(&root, &bundles, &staging, transport, verified)
}

pub fn local_status(data_root: &Path) -> Result<LocalStatus, AssetError> {
    require_linux()?;
    let Some(root) = open_root(data_root, false)? else {
        return Ok(LocalStatus::Missing {
            data_dir: data_root.to_owned(),
        });
    };
    let installing = probe_install_lock(&root)?;
    match read_active_optional(&root)? {
        Some(active) => Ok(LocalStatus::Ready { active, installing }),
        None if installing => Ok(LocalStatus::Installing {
            data_dir: data_root.to_owned(),
        }),
        None => Ok(LocalStatus::Missing {
            data_dir: data_root.to_owned(),
        }),
    }
}

pub fn active_bundle(data_root: &Path) -> Result<ActiveBundle, AssetError> {
    require_linux()?;
    let Some(root) = open_root(data_root, false)? else {
        return Err(AssetError::new(
            AssetErrorKind::AssetsMissing,
            "no active Pangopup bundle is installed",
        ));
    };
    read_active_optional(&root)?.ok_or_else(|| {
        AssetError::new(
            AssetErrorKind::AssetsMissing,
            "no active Pangopup bundle is installed",
        )
    })
}

/// Open the active local bundle entirely through verified directory handles.
pub fn open_active_bundle(data_root: &Path) -> Result<(ActiveBundle, BundleOpen), AssetError> {
    require_linux()?;
    let Some(root) = open_root(data_root, false)? else {
        return Err(AssetError::new(
            AssetErrorKind::AssetsMissing,
            "no active Pangopup bundle is installed",
        ));
    };
    let validated = read_active_open_optional(&root)?.ok_or_else(|| {
        AssetError::new(
            AssetErrorKind::AssetsMissing,
            "no active Pangopup bundle is installed",
        )
    })?;
    Ok((validated.active, validated.opened))
}

fn install_new(
    root: &Root,
    bundles: &Dir,
    staging: &Dir,
    transport: &Path,
    verified: VerifiedTransport,
) -> Result<InstallOutcome, AssetError> {
    let bundle_id = verified.manifest.bundle.bundle_id.clone();
    let transport_id = verified.manifest.transport_id.clone();
    let suffix = identity_suffix(&bundle_id)?.to_owned();
    let (nonce, stage) = create_stage(root, staging, &bundle_id, &transport_id)?;
    let result = (|| {
        let payload = create_dir(&stage, "payload", PRIVATE_DIR_MODE, root)?;
        let staged_bundle = create_dir(&payload, &suffix, PRIVATE_DIR_MODE, root)?;
        let bundle = create_dir(&staged_bundle, "bundle", PRIVATE_DIR_MODE, root)?;
        write_candidate(&stage, &bundle_id, root)?;

        let mut score = create_file(&bundle, "scores.pgi", MEMBER_MODE, root)?;
        let mut score_writer = InstallScoreWriter(&mut score);
        decode_parts(transport, &verified.manifest, Some(&mut score_writer)).map_err(|error| {
            if error.kind() == AssetErrorKind::OutputIo {
                AssetError::new(AssetErrorKind::AssetIo, error.to_string())
            } else {
                error
            }
        })?;
        crash_at!(ScoreChmod);
        set_mode(&score, MEMBER_MODE)?;
        crash_at!(ScoreSync);
        score
            .sync_all()
            .map_err(|_| asset_io("sync installed scores.pgi"))?;
        #[cfg(test)]
        super::install_audit::record(super::install_audit::Event::ScoreWriteComplete(
            pangopup_index::test_score_read_bytes(),
        ));

        let mut notice = create_file(&bundle, "NOTICE", MEMBER_MODE, root)?;
        notice
            .write_all(&verified.notice)
            .map_err(|_| asset_io("write installed NOTICE"))?;
        crash_at!(NoticeChmod);
        set_mode(&notice, MEMBER_MODE)?;
        crash_at!(NoticeSync);
        notice
            .sync_all()
            .map_err(|_| asset_io("sync installed NOTICE"))?;

        let mut manifest = create_file(&bundle, "manifest.json", MEMBER_MODE, root)?;
        manifest
            .write_all(&verified.bundle_manifest_bytes)
            .map_err(|_| asset_io("write installed manifest"))?;
        crash_at!(ManifestChmod);
        set_mode(&manifest, MEMBER_MODE)?;
        crash_at!(ManifestSync);
        manifest
            .sync_all()
            .map_err(|_| asset_io("sync installed manifest"))?;

        let receipt = receipt_for(&verified);
        let receipt_bytes = canonical(&receipt, AssetErrorKind::AssetIo, "serialize receipt")?;
        let receipt_file = write_new_file(
            &staged_bundle,
            "receipt.json",
            &receipt_bytes,
            MEMBER_MODE,
            root,
        )?;
        crash_at!(ReceiptChmod);
        set_mode(&receipt_file, MEMBER_MODE)?;
        crash_at!(ReceiptSync);
        receipt_file
            .sync_all()
            .map_err(|_| asset_io("sync install receipt"))?;

        crash_at!(BundleChmod);
        set_mode(&bundle.file, BUNDLE_MODE)?;
        crash_at!(BundleSync);
        bundle
            .file
            .sync_all()
            .map_err(|_| asset_io("sync staged bundle"))?;
        crash_at!(WrapperSync);
        staged_bundle
            .file
            .sync_all()
            .map_err(|_| asset_io("sync staged bundle wrapper"))?;
        crash_at!(PayloadSync);
        payload
            .file
            .sync_all()
            .map_err(|_| asset_io("sync staged payload"))?;
        crash_at!(PrepublishStageSync);
        stage
            .file
            .sync_all()
            .map_err(|_| asset_io("sync staged active candidate"))?;

        let notice_read = open_required_file(
            &bundle,
            "NOTICE",
            MEMBER_MODE,
            root,
            AssetErrorKind::InstallConflict,
        )?;
        let scores_read = open_required_file(
            &bundle,
            "scores.pgi",
            MEMBER_MODE,
            root,
            AssetErrorKind::InstallConflict,
        )?;
        let _opened = cheap_open_members(
            &verified.bundle_manifest_bytes,
            &notice_read,
            &scores_read,
            &bundle_id,
            AssetErrorKind::InstallConflict,
        )?;

        crash_at!(BundleRename);
        rename_noreplace(&payload, &suffix, bundles, &suffix)?;
        crash_at!(PublishedWrapperChmod);
        set_mode(&staged_bundle.file, BUNDLE_MODE)?;
        crash_at!(PublishedWrapperSync);
        staged_bundle
            .file
            .sync_all()
            .map_err(|_| asset_io("sync published bundle wrapper"))?;
        crash_at!(BundlesSync);
        bundles
            .file
            .sync_all()
            .map_err(|_| asset_io("sync bundles directory"))?;

        let active = ActiveBundle {
            bundle_id: bundle_id.clone(),
            transport_id: transport_id.clone(),
            path: installed_path(root, &suffix),
        };
        crash_at!(ActiveRename);
        rename_replace(&stage, "active.candidate.json", &root.dir, "active.json")?;
        crash_at!(RootSync);
        root.dir
            .file
            .sync_all()
            .map_err(|_| asset_io("sync data root"))?;

        // The active rename plus root fsync is the durable commit point. Cleanup
        // failures after it are intentionally deferred to reconciliation.
        let _ = cleanup_committed_stage(staging, &nonce, root);
        Ok(outcome("installed", active))
    })();

    if result.is_err() {
        // Before the commit point cleanup is mandatory. Preserve malformed stages
        // rather than following unexpected entries.
        cleanup_failed_stage(staging, &nonce, root)?;
    }
    result
}

fn activate_existing(root: &Root, staging: &Dir, active: &ActiveBundle) -> Result<(), AssetError> {
    let (nonce, stage) = create_stage(root, staging, &active.bundle_id, &active.transport_id)?;
    create_dir(&stage, "payload", PRIVATE_DIR_MODE, root)?;
    let result = (|| {
        write_candidate(&stage, &active.bundle_id, root)?;
        crash_at!(ActiveRename);
        rename_replace(&stage, "active.candidate.json", &root.dir, "active.json")?;
        crash_at!(RootSync);
        root.dir
            .file
            .sync_all()
            .map_err(|_| asset_io("sync data root"))?;
        let _ = cleanup_committed_stage(staging, &nonce, root);
        Ok(())
    })();
    if result.is_err() {
        cleanup_failed_stage(staging, &nonce, root)?;
    }
    result
}

fn outcome(status: &'static str, active: ActiveBundle) -> InstallOutcome {
    InstallOutcome {
        status,
        bundle_id: active.bundle_id,
        transport_id: active.transport_id,
        path: active.path,
    }
}

fn receipt_for(verified: &VerifiedTransport) -> Receipt {
    Receipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        bundle_id: verified.manifest.bundle.bundle_id.clone(),
        transport_id: verified.manifest.transport_id.clone(),
        members: vec![
            InstalledMember {
                path: "bundle/NOTICE".to_owned(),
                size: verified.notice.len() as u64,
                sha256: sha256(&verified.notice),
            },
            InstalledMember {
                path: "bundle/manifest.json".to_owned(),
                size: verified.bundle_manifest_bytes.len() as u64,
                sha256: sha256(&verified.bundle_manifest_bytes),
            },
            InstalledMember {
                path: "bundle/scores.pgi".to_owned(),
                size: verified.manifest.bundle.scores.size,
                sha256: verified.manifest.bundle.scores.sha256.clone(),
            },
        ],
    }
}

fn read_active_optional(root: &Root) -> Result<Option<ActiveBundle>, AssetError> {
    read_active_open_optional_kind(root, AssetErrorKind::AssetStateInvalid)
        .map(|value| value.map(|validated| validated.active))
}

fn read_active_optional_for_install(root: &Root) -> Result<Option<ActiveBundle>, AssetError> {
    read_active_open_optional_kind(root, AssetErrorKind::InstallConflict)
        .map(|value| value.map(|validated| validated.active))
}

fn read_active_open_optional(root: &Root) -> Result<Option<ValidatedInstalled>, AssetError> {
    read_active_open_optional_kind(root, AssetErrorKind::AssetStateInvalid)
}

fn read_active_open_optional_kind(
    root: &Root,
    wrapper_mode_error: AssetErrorKind,
) -> Result<Option<ValidatedInstalled>, AssetError> {
    let Some(bytes) = read_optional_bounded_file(
        &root.dir,
        "active.json",
        MAX_JSON_BYTES,
        METADATA_MODE,
        root,
        AssetErrorKind::AssetStateInvalid,
    )?
    else {
        return Ok(None);
    };
    let profile: ActiveProfile =
        parse_canonical(&bytes, AssetErrorKind::AssetStateInvalid, "active profile")?;
    if profile.schema != ACTIVE_SCHEMA || !valid_sha256(&profile.bundle_id) {
        return Err(state_invalid("active profile fields are invalid"));
    }
    let suffix = identity_suffix(&profile.bundle_id)?;
    let bundles = open_required_dir(&root.dir, "bundles", root, None)?;
    let bundle_dir = open_required_dir(&bundles, suffix, root, None)?;
    if file_mode(&bundle_dir.file)? != BUNDLE_MODE {
        return Err(AssetError::new(
            wrapper_mode_error,
            "active bundle wrapper is not immutable",
        ));
    }
    validate_installed(root, suffix, &bundle_dir, Some(&profile.bundle_id)).map(Some)
}

fn validate_installed(
    root: &Root,
    suffix: &str,
    bundle_dir: &Dir,
    selected_id: Option<&str>,
) -> Result<ValidatedInstalled, AssetError> {
    let mut wrapper_names =
        read_names(bundle_dir).map_err(|error| state_invalid(error.to_string()))?;
    wrapper_names.sort();
    if wrapper_names != ["bundle", "receipt.json"] {
        return Err(state_invalid("installed wrapper member set is invalid"));
    }
    let receipt_bytes = read_required_bounded_file(
        bundle_dir,
        "receipt.json",
        MAX_JSON_BYTES,
        MEMBER_MODE,
        root,
        AssetErrorKind::AssetStateInvalid,
    )?;
    let receipt: Receipt = parse_canonical(
        &receipt_bytes,
        AssetErrorKind::AssetStateInvalid,
        "install receipt",
    )?;
    let expected_paths = ["bundle/NOTICE", "bundle/manifest.json", "bundle/scores.pgi"];
    if receipt.schema != RECEIPT_SCHEMA
        || !valid_sha256(&receipt.bundle_id)
        || !valid_sha256(&receipt.transport_id)
        || receipt.members.len() != expected_paths.len()
        || receipt
            .members
            .iter()
            .zip(expected_paths)
            .any(|(member, path)| {
                member.path != path
                    || member.size > super::MAX_SAFE_JSON_U64
                    || !valid_sha256(&member.sha256)
            })
        || identity_suffix(&receipt.bundle_id)? != suffix
        || selected_id.is_some_and(|selected| selected != receipt.bundle_id)
    {
        return Err(state_invalid("install receipt fields are invalid"));
    }
    let bundle = open_required_dir(bundle_dir, "bundle", root, Some(BUNDLE_MODE))?;
    let mut member_names = read_names(&bundle).map_err(|error| state_invalid(error.to_string()))?;
    member_names.sort();
    if member_names != ["NOTICE", "manifest.json", "scores.pgi"] {
        return Err(state_invalid("installed bundle member set is invalid"));
    }
    let (notice_file, notice) = read_member(
        &bundle,
        "NOTICE",
        MAX_NOTICE_BYTES,
        &receipt.members[0],
        root,
    )?;
    let (manifest_file, manifest_bytes) = read_member(
        &bundle,
        "manifest.json",
        MAX_JSON_BYTES,
        &receipt.members[1],
        root,
    )?;
    let scores = open_required_file(
        &bundle,
        "scores.pgi",
        MEMBER_MODE,
        root,
        AssetErrorKind::AssetStateInvalid,
    )?;
    let score_metadata = scores
        .metadata()
        .map_err(|_| asset_io("inspect scores.pgi"))?;
    if score_metadata.len() != receipt.members[2].size
        || score_metadata.len() > MAX_FIXED11_BYTES
        || sha256(&notice) != receipt.members[0].sha256
        || sha256(&manifest_bytes) != receipt.members[1].sha256
        || sha256(&manifest_bytes) != receipt.bundle_id
    {
        return Err(state_invalid("installed member identity mismatch"));
    }
    let inner = parse_bundle_manifest_bytes(&manifest_bytes)
        .map_err(|_| state_invalid("installed bundle manifest is invalid"))?;
    let notice_inner = inner
        .members
        .iter()
        .find(|member| member.path == "NOTICE")
        .ok_or_else(|| state_invalid("bundle manifest lacks NOTICE"))?;
    let scores_inner = inner
        .members
        .iter()
        .find(|member| member.path == "scores.pgi")
        .ok_or_else(|| state_invalid("bundle manifest lacks scores.pgi"))?;
    if notice_inner.size != receipt.members[0].size
        || notice_inner.sha256 != receipt.members[0].sha256
        || scores_inner.size != receipt.members[2].size
        || scores_inner.sha256 != receipt.members[2].sha256
    {
        return Err(state_invalid("receipt does not match the bundle manifest"));
    }
    drop(manifest_file);
    let opened = cheap_open_members(
        &manifest_bytes,
        &notice_file,
        &scores,
        &receipt.bundle_id,
        AssetErrorKind::InstallConflict,
    )?;
    Ok(ValidatedInstalled {
        active: ActiveBundle {
            bundle_id: receipt.bundle_id,
            transport_id: receipt.transport_id,
            path: installed_path(root, suffix),
        },
        opened,
    })
}

fn read_member(
    bundle: &Dir,
    name: &str,
    cap: u64,
    descriptor: &InstalledMember,
    root: &Root,
) -> Result<(File, Vec<u8>), AssetError> {
    let file = open_required_file(
        bundle,
        name,
        MEMBER_MODE,
        root,
        AssetErrorKind::AssetStateInvalid,
    )?;
    let bytes = read_bounded_handle_ref(&file, cap, AssetErrorKind::AssetStateInvalid)?;
    if bytes.len() as u64 != descriptor.size {
        return Err(state_invalid("installed member size mismatch"));
    }
    Ok((file, bytes))
}

fn cheap_open_members(
    manifest_bytes: &[u8],
    notice: &File,
    scores: &File,
    bundle_id: &str,
    incompatible: AssetErrorKind,
) -> Result<BundleOpen, AssetError> {
    #[cfg(test)]
    super::install_audit::record(super::install_audit::Event::CheapOpenStart(
        pangopup_index::test_score_read_bytes(),
    ));
    let result = BundleOpen::open_members(manifest_bytes, notice, scores);
    #[cfg(test)]
    super::install_audit::record(super::install_audit::Event::CheapOpenComplete(
        pangopup_index::test_score_read_bytes(),
    ));
    match result {
        Ok(opened) if opened.bundle_id() == bundle_id => Ok(opened),
        Ok(_) => Err(state_invalid("opened bundle identity mismatch")),
        Err(IndexError::Io(error)) => Err(state_invalid(error.to_string())),
        Err(error) => Err(AssetError::new(incompatible, error.to_string())),
    }
}

fn installed_path(root: &Root, suffix: &str) -> PathBuf {
    root.path.join("bundles").join(suffix).join("bundle")
}

fn write_candidate(stage: &Dir, bundle_id: &str, root: &Root) -> Result<(), AssetError> {
    let bytes = canonical(
        &ActiveProfile {
            schema: ACTIVE_SCHEMA.to_owned(),
            bundle_id: bundle_id.to_owned(),
        },
        AssetErrorKind::AssetIo,
        "serialize active profile",
    )?;
    let mut file = create_file(stage, "active.candidate.json", METADATA_MODE, root)?;
    file.write_all(&bytes)
        .map_err(|_| asset_io("write active candidate"))?;
    crash_at!(CandidateChmod);
    set_mode(&file, METADATA_MODE)?;
    crash_at!(CandidateSync);
    file.sync_all()
        .map_err(|_| asset_io("sync active candidate"))?;
    crash_at!(CandidateStageSync);
    stage
        .file
        .sync_all()
        .map_err(|_| asset_io("sync active candidate directory"))?;
    Ok(())
}

fn create_stage(
    root: &Root,
    staging: &Dir,
    bundle_id: &str,
    transport_id: &str,
) -> Result<(String, Dir), AssetError> {
    for _ in 0..32 {
        let nonce = random_nonce()?;
        match mkdir_at(staging, &nonce, PRIVATE_DIR_MODE) {
            Ok(()) => {
                let stage = open_required_dir(staging, &nonce, root, Some(PRIVATE_DIR_MODE))?;
                let marker = StageMarker {
                    schema: STAGE_SCHEMA.to_owned(),
                    nonce: nonce.clone(),
                    euid: u64::from(root.euid),
                    bundle_id: bundle_id.to_owned(),
                    transport_id: transport_id.to_owned(),
                };
                let bytes = canonical(&marker, AssetErrorKind::AssetIo, "serialize stage marker")?;
                let mut marker_file = create_file(&stage, "marker.json", STAGE_MARKER_MODE, root)?;
                marker_file
                    .write_all(&bytes)
                    .map_err(|_| asset_io("write stage marker"))?;
                crash_at!(MarkerChmod);
                set_mode(&marker_file, STAGE_MARKER_MODE)?;
                crash_at!(MarkerSync);
                marker_file
                    .sync_all()
                    .map_err(|_| asset_io("sync stage marker file"))?;
                crash_at!(StageSync);
                stage
                    .file
                    .sync_all()
                    .map_err(|_| asset_io("sync stage marker"))?;
                crash_at!(StagingSync);
                staging
                    .file
                    .sync_all()
                    .map_err(|_| asset_io("sync staging directory"))?;
                return Ok((nonce, stage));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(asset_io("create install stage")),
        }
    }
    Err(asset_io("create unique install stage"))
}

fn random_nonce() -> Result<String, AssetError> {
    let mut random = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|_| asset_io("obtain staging randomness"))?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

struct StagePlan {
    name: String,
    stage: Dir,
    marker: Option<StageMarker>,
    has_candidate: bool,
    published: Option<PublishedStage>,
}

struct PublishedStage {
    wrapper: Dir,
    needs_finalize: bool,
}

fn reconcile_staging(
    root: &Root,
    bundles: &Dir,
    staging: &Dir,
    current: Option<(&str, &str)>,
) -> Result<bool, AssetError> {
    let mut names = read_names(staging)?;
    names.sort();
    let mut plans = Vec::with_capacity(names.len());

    // Phase one is deliberately read-only. A malformed later child must not
    // observe any earlier cleanup or active-profile mutation.
    for name in names {
        if !valid_nonce(&name) {
            return Err(staging_invalid("staging contains a non-nonce child"));
        }
        let stage = open_required_dir(staging, &name, root, Some(PRIVATE_DIR_MODE))
            .map_err(|_| staging_invalid("staging child is not a safe directory"))?;
        validate_tree(&stage, root)?;
        let marker_bytes = read_optional_bounded_file(
            &stage,
            "marker.json",
            MAX_JSON_BYTES,
            STAGE_MARKER_MODE,
            root,
            AssetErrorKind::StagingInvalid,
        )?;
        let Some(marker_bytes) = marker_bytes else {
            if read_names(&stage)?.is_empty() {
                plans.push(StagePlan {
                    name,
                    stage,
                    marker: None,
                    has_candidate: false,
                    published: None,
                });
                continue;
            }
            return Err(staging_invalid("markerless staging wrapper is not empty"));
        };
        let marker: StageMarker = parse_canonical(
            &marker_bytes,
            AssetErrorKind::StagingInvalid,
            "stage marker",
        )?;
        if marker.schema != STAGE_SCHEMA
            || marker.nonce != name
            || marker.euid != u64::from(root.euid)
            || !valid_sha256(&marker.bundle_id)
            || !valid_sha256(&marker.transport_id)
        {
            return Err(staging_invalid("stage marker fields are invalid"));
        }
        let has_candidate = file_exists(&stage, "active.candidate.json")?;
        if has_candidate {
            validate_candidate(&stage, &marker.bundle_id, root)?;
        }
        let suffix = identity_suffix(&marker.bundle_id)?;
        let published = if let Some(bundle_dir) = open_dir_optional(bundles, suffix, root)? {
            let mode = file_mode(&bundle_dir.file)?;
            if mode != BUNDLE_MODE && mode != PRIVATE_DIR_MODE {
                return Err(staging_invalid("published bundle wrapper mode is invalid"));
            }
            if mode == PRIVATE_DIR_MODE && !has_candidate {
                return Err(staging_invalid(
                    "mutable published wrapper lacks its marker-bound candidate",
                ));
            }
            let installed = validate_installed(root, suffix, &bundle_dir, None)?;
            if installed.active.transport_id != marker.transport_id {
                return Err(staging_invalid(
                    "stage marker conflicts with published receipt",
                ));
            }
            Some(PublishedStage {
                wrapper: bundle_dir,
                needs_finalize: mode == PRIVATE_DIR_MODE,
            })
        } else {
            None
        };
        plans.push(StagePlan {
            name,
            stage,
            marker: Some(marker),
            has_candidate,
            published,
        });
    }

    let recovery_indices: Vec<usize> = plans
        .iter()
        .enumerate()
        .filter_map(|(index, plan)| {
            let marker = plan.marker.as_ref()?;
            (plan.has_candidate
                && plan.published.is_some()
                && current
                    .is_some_and(|ids| ids.0 == marker.bundle_id && ids.1 == marker.transport_id))
            .then_some(index)
        })
        .collect();
    if recovery_indices.len() > 1 {
        return Err(staging_invalid(
            "multiple marker-bound candidates claim the requested activation",
        ));
    }

    // Any published 0700 wrapper is accepted only because this complete,
    // marker-bound plan validated it. Freeze and durably publish that metadata
    // before an active profile can name it.
    for plan in &plans {
        if let Some(published) = &plan.published
            && published.needs_finalize
        {
            crash_at!(PublishedWrapperChmod);
            set_mode(&published.wrapper.file, BUNDLE_MODE)?;
            crash_at!(PublishedWrapperSync);
            published
                .wrapper
                .file
                .sync_all()
                .map_err(|_| asset_io("sync recovered bundle wrapper"))?;
            crash_at!(BundlesSync);
            bundles
                .file
                .sync_all()
                .map_err(|_| asset_io("sync recovered bundles directory"))?;
        }
    }

    if let Some(index) = recovery_indices.first().copied() {
        let plan = &plans[index];
        crash_at!(ActiveRename);
        rename_replace(
            &plan.stage,
            "active.candidate.json",
            &root.dir,
            "active.json",
        )?;
        crash_at!(RootSync);
        root.dir
            .file
            .sync_all()
            .map_err(|_| asset_io("sync recovered active profile"))?;

        // The root fsync above is the sole recovered commit point. Every later
        // cleanup is best effort and cannot turn a committed reuse into error.
        for plan in &plans {
            let _ = cleanup_failed_stage(staging, &plan.name, root);
        }
        let _ = staging.file.sync_all();
        return Ok(true);
    }

    for plan in &plans {
        cleanup_failed_stage(staging, &plan.name, root)?;
    }
    staging
        .file
        .sync_all()
        .map_err(|_| asset_io("sync staging directory"))?;
    Ok(false)
}

fn validate_candidate(stage: &Dir, bundle_id: &str, root: &Root) -> Result<(), AssetError> {
    let bytes = read_required_bounded_file(
        stage,
        "active.candidate.json",
        MAX_JSON_BYTES,
        METADATA_MODE,
        root,
        AssetErrorKind::StagingInvalid,
    )?;
    let candidate: ActiveProfile =
        parse_canonical(&bytes, AssetErrorKind::StagingInvalid, "active candidate")?;
    if candidate.schema != ACTIVE_SCHEMA || candidate.bundle_id != bundle_id {
        return Err(staging_invalid("active candidate fields are invalid"));
    }
    Ok(())
}

fn cleanup_committed_stage(staging: &Dir, nonce: &str, root: &Root) -> Result<(), AssetError> {
    #[cfg(test)]
    if super::install_audit::fail(super::install_audit::FaultPoint::CleanupAfterCommit) {
        return Err(asset_io("injected committed-stage cleanup failure"));
    }
    let stage = open_required_dir(staging, nonce, root, Some(PRIVATE_DIR_MODE))?;
    if file_exists(&stage, "payload")? {
        let payload = open_required_dir(&stage, "payload", root, Some(PRIVATE_DIR_MODE))?;
        if !read_names(&payload)?.is_empty() {
            return Err(staging_invalid("committed stage payload is not empty"));
        }
        remove_dir_at(&stage, "payload").map_err(|_| asset_io("remove empty stage payload"))?;
    }
    unlink_file(&stage, "marker.json")?;
    remove_dir_at(staging, nonce).map_err(|_| asset_io("remove stage wrapper"))?;
    staging
        .file
        .sync_all()
        .map_err(|_| asset_io("sync staging cleanup"))
}

fn cleanup_failed_stage(staging: &Dir, nonce: &str, root: &Root) -> Result<(), AssetError> {
    let Some(stage) = open_dir_optional(staging, nonce, root)? else {
        return Ok(());
    };
    validate_tree(&stage, root)?;
    let mut names = read_names(&stage)?;
    names.retain(|name| name != "marker.json");
    for name in names {
        remove_tree_entry(&stage, &name, root)?;
    }
    if file_exists(&stage, "marker.json")? {
        unlink_file(&stage, "marker.json")?;
    }
    remove_dir_at(staging, nonce).map_err(|_| asset_io("remove staging wrapper"))?;
    staging
        .file
        .sync_all()
        .map_err(|_| asset_io("sync staging cleanup"))
}

fn validate_tree(dir: &Dir, root: &Root) -> Result<(), AssetError> {
    for name in read_names(dir)? {
        match open_any_nofollow(dir, &name) {
            Ok(file) => {
                let metadata = file
                    .metadata()
                    .map_err(|_| asset_io("inspect staging entry"))?;
                validate_owned_metadata(&metadata, root, AssetErrorKind::StagingInvalid)?;
                if metadata.file_type().is_dir() {
                    validate_tree(&dir_from_file(file)?, root)?;
                } else if !metadata.file_type().is_file() {
                    return Err(staging_invalid(
                        "staging entry is not a regular file or directory",
                    ));
                }
            }
            Err(_) => {
                return Err(staging_invalid(
                    "staging entry cannot be opened without following links",
                ));
            }
        }
    }
    Ok(())
}

fn remove_tree_entry(dir: &Dir, name: &str, root: &Root) -> Result<(), AssetError> {
    let file = open_any_nofollow(dir, name).map_err(|_| staging_invalid("unsafe staging entry"))?;
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect staging cleanup entry"))?;
    validate_owned_metadata(&metadata, root, AssetErrorKind::StagingInvalid)?;
    if metadata.file_type().is_dir() {
        let child = dir_from_file(file)?;
        set_mode(&child.file, PRIVATE_DIR_MODE)?;
        for nested in read_names(&child)? {
            remove_tree_entry(&child, &nested, root)?;
        }
        remove_dir_at(dir, name).map_err(|_| asset_io("remove staging directory"))
    } else if metadata.file_type().is_file() {
        unlink_file(dir, name)
    } else {
        Err(staging_invalid("unsafe staging entry type"))
    }
}

fn valid_nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn identity_suffix(value: &str) -> Result<&str, AssetError> {
    if !valid_sha256(value) {
        return Err(state_invalid("invalid SHA-256 identity"));
    }
    Ok(&value[7..])
}

fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    kind: AssetErrorKind,
    label: &str,
) -> Result<T, AssetError> {
    reject_duplicate_json(bytes).map_err(|_| {
        AssetError::new(kind, format!("{label} contains invalid or duplicate JSON"))
    })?;
    let value: T = serde_json::from_slice(bytes)
        .map_err(|_| AssetError::new(kind, format!("{label} is not closed v1 JSON")))?;
    if serde_jcs::to_vec(&value)
        .map_err(|_| AssetError::new(kind, format!("cannot canonicalize {label}")))?
        != bytes
    {
        return Err(AssetError::new(
            kind,
            format!("{label} is not canonical RFC 8785 JSON"),
        ));
    }
    Ok(value)
}

fn canonical<T: Serialize>(
    value: &T,
    kind: AssetErrorKind,
    label: &str,
) -> Result<Vec<u8>, AssetError> {
    serde_jcs::to_vec(value).map_err(|_| AssetError::new(kind, label))
}

fn open_root(path: &Path, create: bool) -> Result<Option<Root>, AssetError> {
    let existed = match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(_) => return Err(asset_io("inspect data root")),
    };
    if !existed {
        if !create {
            return Ok(None);
        }
        let mut builder = fs::DirBuilder::new();
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(ROOT_MODE).recursive(true);
        builder
            .create(path)
            .map_err(|_| asset_io("create data root"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(ROOT_MODE))
            .map_err(|_| asset_io("set data root mode"))?;
    }
    let file = open_path_directory(path)
        .map_err(|_| state_invalid("data root is not a real directory"))?;
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect opened data root"))?;
    let euid = effective_uid();
    if metadata.uid() != euid || metadata.mode() & 0o022 != 0 {
        return Err(state_invalid(
            "data root must be owned by the effective uid and not group/world writable",
        ));
    }
    let dir = Dir {
        dev: metadata.dev(),
        file,
    };
    Ok(Some(Root {
        path: path.to_owned(),
        dir,
        euid,
    }))
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and no failure mode.
    unsafe { libc::geteuid() }
}

fn require_linux() -> Result<(), AssetError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(AssetError::new(
            AssetErrorKind::UnsupportedPlatform,
            "local asset installation requires Linux",
        ))
    }
}

fn acquire_install_lock(root: &Root) -> Result<InstallLock, AssetError> {
    let file = open_or_create_file(&root.dir, ".install.lock", METADATA_MODE, root)?;
    set_mode(&file, METADATA_MODE)?;
    // SAFETY: flock acts on the live descriptor and does not retain pointers.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(InstallLock(file))
    } else if io::Error::last_os_error().kind() == ErrorKind::WouldBlock {
        Err(AssetError::new(
            AssetErrorKind::AssetLocked,
            "another asset installation is in progress",
        ))
    } else {
        Err(asset_io("lock asset store"))
    }
}

fn probe_install_lock(root: &Root) -> Result<bool, AssetError> {
    let Some(file) = open_optional_file(
        &root.dir,
        ".install.lock",
        METADATA_MODE,
        root,
        AssetErrorKind::AssetStateInvalid,
    )?
    else {
        return Ok(false);
    };
    // SAFETY: flock acts on this live descriptor only.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        // SAFETY: descriptor is live and owned here.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        Ok(false)
    } else if io::Error::last_os_error().kind() == ErrorKind::WouldBlock {
        Ok(true)
    } else {
        Err(asset_io("probe install lock"))
    }
}

fn ensure_private_dir(parent: &Dir, name: &str, root: &Root) -> Result<Dir, AssetError> {
    match mkdir_at(parent, name, PRIVATE_DIR_MODE) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return Err(asset_io("create asset-store directory")),
    }
    open_required_dir(parent, name, root, Some(PRIVATE_DIR_MODE))
}

fn create_dir(parent: &Dir, name: &str, mode: u32, root: &Root) -> Result<Dir, AssetError> {
    mkdir_at(parent, name, mode).map_err(|_| asset_io("create staged directory"))?;
    open_required_dir(parent, name, root, Some(mode))
}

fn open_required_dir(
    parent: &Dir,
    name: &str,
    root: &Root,
    mode: Option<u32>,
) -> Result<Dir, AssetError> {
    open_dir_optional(parent, name, root)?
        .ok_or_else(|| state_invalid("required directory is missing"))
        .and_then(|dir| {
            if mode.is_some_and(|expected| file_mode(&dir.file).ok() != Some(expected)) {
                Err(state_invalid("directory mode is invalid"))
            } else {
                Ok(dir)
            }
        })
}

fn open_dir_optional(parent: &Dir, name: &str, root: &Root) -> Result<Option<Dir>, AssetError> {
    match open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
    ) {
        Ok(file) => {
            let metadata = file.metadata().map_err(|_| asset_io("inspect directory"))?;
            validate_owned_metadata(&metadata, root, AssetErrorKind::AssetStateInvalid)?;
            if !metadata.file_type().is_dir() {
                return Err(state_invalid("asset-store entry is not a directory"));
            }
            Ok(Some(Dir {
                file,
                dev: metadata.dev(),
            }))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(state_invalid(
            "asset-store directory cannot be opened without following links",
        )),
    }
}

fn dir_from_file(file: File) -> Result<Dir, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect directory handle"))?;
    Ok(Dir {
        file,
        dev: metadata.dev(),
    })
}

fn validate_owned_metadata(
    metadata: &fs::Metadata,
    root: &Root,
    kind: AssetErrorKind,
) -> Result<(), AssetError> {
    if metadata.dev() != root.dir.dev || metadata.uid() != root.euid {
        return Err(AssetError::new(
            kind,
            "asset-store entry has the wrong filesystem or owner",
        ));
    }
    Ok(())
}

fn read_optional_bounded_file(
    parent: &Dir,
    name: &str,
    cap: u64,
    mode: u32,
    root: &Root,
    kind: AssetErrorKind,
) -> Result<Option<Vec<u8>>, AssetError> {
    let Some(file) = open_optional_file(parent, name, mode, root, kind)? else {
        return Ok(None);
    };
    read_bounded_handle(file, cap, kind).map(Some)
}

fn read_required_bounded_file(
    parent: &Dir,
    name: &str,
    cap: u64,
    mode: u32,
    root: &Root,
    kind: AssetErrorKind,
) -> Result<Vec<u8>, AssetError> {
    read_optional_bounded_file(parent, name, cap, mode, root, kind)?
        .ok_or_else(|| AssetError::new(kind, format!("required {name} is missing")))
}

fn read_bounded_handle(file: File, cap: u64, kind: AssetErrorKind) -> Result<Vec<u8>, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect metadata file"))?;
    if metadata.len() > cap {
        return Err(AssetError::new(
            kind,
            "metadata file exceeds its size limit",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| asset_io("read metadata file"))?;
    if bytes.len() as u64 > cap {
        return Err(AssetError::new(
            kind,
            "metadata file grew beyond its size limit",
        ));
    }
    Ok(bytes)
}

fn read_bounded_handle_ref(
    file: &File,
    cap: u64,
    kind: AssetErrorKind,
) -> Result<Vec<u8>, AssetError> {
    let duplicate = file
        .try_clone()
        .map_err(|_| asset_io("duplicate metadata file handle"))?;
    read_bounded_handle(duplicate, cap, kind)
}

fn open_required_file(
    parent: &Dir,
    name: &str,
    mode: u32,
    root: &Root,
    kind: AssetErrorKind,
) -> Result<File, AssetError> {
    open_optional_file(parent, name, mode, root, kind)?
        .ok_or_else(|| AssetError::new(kind, format!("required {name} is missing")))
}

fn open_optional_file(
    parent: &Dir,
    name: &str,
    mode: u32,
    root: &Root,
    kind: AssetErrorKind,
) -> Result<Option<File>, AssetError> {
    match open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
    ) {
        Ok(file) => {
            let metadata = file
                .metadata()
                .map_err(|_| asset_io("inspect regular file"))?;
            validate_owned_metadata(&metadata, root, kind)?;
            if !metadata.file_type().is_file() || metadata.mode() & 0o777 != mode {
                return Err(AssetError::new(
                    kind,
                    "asset-store entry shape or mode is invalid",
                ));
            }
            Ok(Some(file))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(AssetError::new(
            kind,
            "asset-store file cannot be opened without following links",
        )),
    }
}

fn open_or_create_file(
    parent: &Dir,
    name: &str,
    mode: u32,
    root: &Root,
) -> Result<File, AssetError> {
    match open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        mode,
    ) {
        Ok(file) => {
            let metadata = file.metadata().map_err(|_| asset_io("inspect lock file"))?;
            validate_owned_metadata(&metadata, root, AssetErrorKind::AssetStateInvalid)?;
            if !metadata.file_type().is_file() {
                return Err(state_invalid("install lock is not a regular file"));
            }
            Ok(file)
        }
        Err(_) => Err(state_invalid(
            "install lock cannot be opened without following links",
        )),
    }
}

fn create_file(parent: &Dir, name: &str, mode: u32, root: &Root) -> Result<File, AssetError> {
    let file = open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        mode,
    )
    .map_err(|_| asset_io("create staged file"))?;
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect staged file"))?;
    validate_owned_metadata(&metadata, root, AssetErrorKind::AssetIo)?;
    Ok(file)
}

fn write_new_file(
    parent: &Dir,
    name: &str,
    bytes: &[u8],
    mode: u32,
    root: &Root,
) -> Result<File, AssetError> {
    let mut file = create_file(parent, name, mode, root)?;
    file.write_all(bytes)
        .map_err(|_| asset_io("write staged file"))?;
    set_mode(&file, mode)?;
    Ok(file)
}

fn set_mode(file: &File, mode: u32) -> Result<(), AssetError> {
    // SAFETY: fchmod acts on this live file descriptor.
    if unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) } == 0 {
        Ok(())
    } else {
        Err(asset_io("set asset-store mode"))
    }
}

fn file_mode(file: &File) -> Result<u32, AssetError> {
    file.metadata()
        .map(|metadata| metadata.mode() & 0o777)
        .map_err(|_| asset_io("inspect file mode"))
}

fn file_exists(parent: &Dir, name: &str) -> Result<bool, AssetError> {
    match open_any_nofollow(parent, name) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(staging_invalid("staging path is not safe")),
    }
}

fn open_path_directory(path: &Path) -> io::Result<File> {
    let bytes = path.as_os_str().as_bytes();
    let c_path = CString::new(bytes).map_err(|_| io::Error::other("NUL in path"))?;
    // SAFETY: c_path is NUL terminated and flags require no variadic mode argument.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(fd)
}

fn open_any_nofollow(parent: &Dir, name: &str) -> io::Result<File> {
    open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
    )
}

fn open_at(dirfd: RawFd, name: &str, flags: i32, mode: u32) -> io::Result<File> {
    let name = component(name)?;
    openat2_beneath(dirfd, &name, flags, mode)
}

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

const RESOLVE_NO_XDEV: u64 = 0x01;
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;

fn openat2_beneath(dirfd: RawFd, name: &CString, flags: i32, mode: u32) -> io::Result<File> {
    let how = OpenHow {
        flags: flags as u64,
        mode: u64::from(mode),
        resolve: RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV,
    };
    // SAFETY: `name` and `how` remain live for the syscall, `dirfd` is held by
    // the caller, and the kernel receives the exact structure size.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        ) as i32
    };
    file_from_fd(fd)
}

fn file_from_fd(fd: i32) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: a nonnegative successful open/openat result is a new owned fd.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn mkdir_at(parent: &Dir, name: &str, mode: u32) -> io::Result<()> {
    let name = component(name)?;
    // SAFETY: name and descriptor are valid for this call.
    if unsafe { libc::mkdirat(parent.file.as_raw_fd(), name.as_ptr(), mode as libc::mode_t) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn remove_dir_at(parent: &Dir, name: &str) -> io::Result<()> {
    let name = component(name)?;
    // SAFETY: name and descriptor are valid for this call.
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn unlink_file(parent: &Dir, name: &str) -> Result<(), AssetError> {
    let name = component(name).map_err(|_| asset_io("invalid asset-store component"))?;
    // SAFETY: name and descriptor are valid for this call.
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), name.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(asset_io("unlink asset-store file"))
    }
}

fn rename_noreplace(from: &Dir, old: &str, to: &Dir, new: &str) -> Result<(), AssetError> {
    rustix::fs::renameat_with(
        &from.file,
        old,
        &to.file,
        new,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|error| {
        if matches!(
            error.kind(),
            ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
        ) {
            AssetError::new(
                AssetErrorKind::InstallConflict,
                "bundle publication race lost",
            )
        } else {
            AssetError::new(
                AssetErrorKind::AssetIo,
                format!("publish installed bundle: {error}"),
            )
        }
    })
}

fn rename_replace(from: &Dir, old: &str, to: &Dir, new: &str) -> Result<(), AssetError> {
    rustix::fs::renameat(&from.file, old, &to.file, new)
        .map_err(io::Error::from)
        .map_err(|error| {
            AssetError::new(
                AssetErrorKind::AssetIo,
                format!("publish active profile: {error}"),
            )
        })
}

fn read_names(dir: &Dir) -> Result<Vec<String>, AssetError> {
    let dot = CString::new(".").expect("static component");
    let cursor = openat2_beneath(
        dir.file.as_raw_fd(),
        &dot,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        0,
    )
    .map_err(|_| asset_io("open directory cursor"))?;
    let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(cursor, &mut buffer);
    let mut names = Vec::new();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|_| asset_io("read asset-store directory entry"))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = String::from_utf8(bytes.to_vec())
            .map_err(|_| staging_invalid("asset-store child name is not UTF-8"))?;
        names.push(name);
    }
    Ok(names)
}

fn component(name: &str) -> io::Result<CString> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(io::Error::other("invalid path component"));
    }
    CString::new(name).map_err(|_| io::Error::other("NUL in path component"))
}

fn state_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::AssetStateInvalid, message)
}

fn staging_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::StagingInvalid, message)
}

fn asset_io(action: &str) -> AssetError {
    AssetError::new(
        AssetErrorKind::AssetIo,
        format!("{action}: {}", io::Error::last_os_error()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_audit::{self, Event, FaultPoint};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn path_resolution_is_strict_and_precedence_ordered() {
        let base = DataPathInputs {
            explicit: Some("/explicit".into()),
            pangopup_data_dir: Some("/pangopup".into()),
            xdg_data_home: Some("/xdg".into()),
            home: Some("/home/test".into()),
        };
        assert_eq!(
            resolve_data_root(&base).expect("explicit"),
            Path::new("/explicit")
        );
        assert_eq!(
            resolve_data_root(&DataPathInputs {
                explicit: None,
                ..base.clone()
            })
            .expect("environment"),
            Path::new("/pangopup")
        );
        assert_eq!(
            resolve_data_root(&DataPathInputs {
                explicit: None,
                pangopup_data_dir: None,
                ..base.clone()
            })
            .expect("XDG"),
            Path::new("/xdg/pangopup")
        );
        assert_eq!(
            resolve_data_root(&DataPathInputs {
                explicit: None,
                pangopup_data_dir: None,
                xdg_data_home: None,
                ..base
            })
            .expect("HOME"),
            Path::new("/home/test/.local/share/pangopup")
        );
    }

    #[test]
    fn present_invalid_path_never_falls_through() {
        for invalid in [OsString::new(), OsString::from("relative")] {
            let error = resolve_data_root(&DataPathInputs {
                pangopup_data_dir: Some(invalid),
                xdg_data_home: Some("/valid".into()),
                ..DataPathInputs::default()
            })
            .expect_err("present invalid value");
            assert_eq!(error.kind(), AssetErrorKind::PathInvalid);
        }
        assert_eq!(
            resolve_data_root(&DataPathInputs::default())
                .expect_err("unavailable")
                .kind(),
            AssetErrorKind::PathUnavailable
        );
    }

    #[test]
    fn local_json_schemas_are_canonical_closed_and_ordered() {
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.to_owned(),
            bundle_id: format!("sha256:{}", "a".repeat(64)),
            transport_id: format!("sha256:{}", "b".repeat(64)),
            members: ["NOTICE", "manifest.json", "scores.pgi"]
                .into_iter()
                .map(|name| InstalledMember {
                    path: format!("bundle/{name}"),
                    size: 1,
                    sha256: format!("sha256:{}", "c".repeat(64)),
                })
                .collect(),
        };
        let bytes = canonical(&receipt, AssetErrorKind::AssetIo, "receipt").expect("canonical");
        assert_eq!(
            parse_canonical::<Receipt>(&bytes, AssetErrorKind::AssetStateInvalid, "receipt")
                .expect("parse"),
            receipt
        );
        let mut trailing = bytes;
        trailing.push(b'\n');
        assert_eq!(
            parse_canonical::<Receipt>(&trailing, AssetErrorKind::AssetStateInvalid, "receipt")
                .expect_err("trailing")
                .kind(),
            AssetErrorKind::AssetStateInvalid
        );
    }

    #[test]
    fn install_audit_proves_one_compressed_pass_and_direct_cheap_open() {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pangopup-install-audit-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("audit root");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/snv-regression/bundle");
        let transport = root.join("transport");
        crate::pack_bundle(&fixture, &transport).expect("pack audit transport");
        install_audit::take();
        pangopup_index::test_reset_score_read_bytes();
        let data = root.join("data");
        install_transport(&transport, &data).expect("audited install");
        let events = install_audit::take();
        let expected_compressed: usize = fs::read_dir(&transport)
            .expect("transport")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("payload.pgi.zst.part"))
            })
            .map(|entry| entry.metadata().expect("part metadata").len() as usize)
            .sum();
        let score_bytes_at_write = events.iter().find_map(|event| match event {
            Event::ScoreWriteComplete(bytes) => Some(*bytes),
            _ => None,
        });
        let score_bytes_at_open_start = events.iter().find_map(|event| match event {
            Event::CheapOpenStart(bytes) => Some(*bytes),
            _ => None,
        });
        let score_bytes_after_open = events.iter().find_map(|event| match event {
            Event::CheapOpenComplete(bytes) => Some(*bytes),
            _ => None,
        });
        assert_eq!(
            events
                .iter()
                .map(|event| match event {
                    Event::CompressedRead(bytes) => *bytes,
                    _ => 0,
                })
                .sum::<usize>(),
            expected_compressed
        );
        assert_eq!(score_bytes_at_write, Some(0));
        assert_eq!(
            score_bytes_at_open_start,
            Some(0),
            "zero score bytes are read between completed write and cheap-open entry"
        );
        assert!(
            score_bytes_after_open.is_some_and(|bytes| bytes > 0),
            "cheap structural reads are counted only after CheapOpenStart"
        );

        install_transport(&transport, &data).expect("audited reuse");
        assert!(
            install_audit::take()
                .iter()
                .all(|event| !matches!(event, Event::CompressedRead(_))),
            "reuse must not open a transport part"
        );

        make_tree_writable(&root);
        fs::remove_dir_all(root).expect("audit cleanup");
    }

    #[test]
    fn every_durable_install_window_recovers_or_discards_deterministically() {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pangopup-install-faults-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("fault root");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/snv-regression/bundle");
        let transport = root.join("transport");
        crate::pack_bundle(&fixture, &transport).expect("pack fault transport");

        let write_failure_data = root.join("data-score-write-failure");
        install_audit::set_fault(FaultPoint::ScoreWrite);
        assert_eq!(
            install_transport(&transport, &write_failure_data)
                .expect_err("local score write failure")
                .kind(),
            AssetErrorKind::AssetIo,
            "local destination failures are not generic unpack OUTPUT_IO"
        );

        let before_publication = [
            FaultPoint::MarkerChmod,
            FaultPoint::MarkerSync,
            FaultPoint::StageSync,
            FaultPoint::StagingSync,
            FaultPoint::CandidateChmod,
            FaultPoint::CandidateSync,
            FaultPoint::CandidateStageSync,
            FaultPoint::ScoreChmod,
            FaultPoint::ScoreSync,
            FaultPoint::NoticeChmod,
            FaultPoint::NoticeSync,
            FaultPoint::ManifestChmod,
            FaultPoint::ManifestSync,
            FaultPoint::ReceiptChmod,
            FaultPoint::ReceiptSync,
            FaultPoint::BundleChmod,
            FaultPoint::BundleSync,
            FaultPoint::WrapperSync,
            FaultPoint::PayloadSync,
            FaultPoint::PrepublishStageSync,
            FaultPoint::BundleRename,
        ];
        let after_publication = [
            FaultPoint::PublishedWrapperChmod,
            FaultPoint::PublishedWrapperSync,
            FaultPoint::BundlesSync,
            FaultPoint::ActiveRename,
            FaultPoint::RootSync,
        ];
        for (point, expected) in before_publication
            .into_iter()
            .map(|point| (point, "installed"))
            .chain(after_publication.into_iter().map(|point| (point, "reused")))
        {
            let data = root.join(format!("data-{point:?}"));
            install_audit::set_fault(point);
            assert!(
                catch_unwind(AssertUnwindSafe(|| install_transport(&transport, &data))).is_err(),
                "{point:?} must simulate a process crash"
            );
            let recovered = install_transport(&transport, &data).expect("recover fault window");
            assert_eq!(recovered.status, expected, "{point:?}");
            assert_eq!(
                fs::symlink_metadata(recovered.path.parent().expect("wrapper"))
                    .expect("wrapper metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                BUNDLE_MODE,
                "{point:?} leaves an immutable wrapper"
            );
            assert!(
                fs::read_dir(data.join(".staging"))
                    .expect("staging")
                    .next()
                    .is_none(),
                "{point:?} reconciliation finishes cleanup"
            );
        }

        let data = root.join("data-cleanup-failure");
        install_audit::set_fault(FaultPoint::CleanupAfterCommit);
        let committed =
            install_transport(&transport, &data).expect("postcommit cleanup is best effort");
        assert_eq!(committed.status, "installed");
        assert!(data.join("active.json").is_file());
        assert!(
            fs::read_dir(data.join(".staging"))
                .expect("staging")
                .next()
                .is_some(),
            "injected cleanup failure leaves recoverable staging state"
        );
        assert_eq!(
            install_transport(&transport, &data)
                .expect("reuse after cleanup failure")
                .status,
            "reused"
        );

        make_tree_writable(&root);
        fs::remove_dir_all(root).expect("fault cleanup");
    }

    #[test]
    fn wrapper_modes_and_two_phase_reconciliation_fail_closed() {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pangopup-install-reconcile-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("reconcile root");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/snv-regression/bundle");
        let transport = root.join("transport");
        crate::pack_bundle(&fixture, &transport).expect("pack reconcile transport");

        let conflict_data = root.join("conflict-data");
        let installed =
            install_transport(&transport, &conflict_data).expect("install conflict fixture");
        let wrapper = installed.path.parent().expect("wrapper");
        fs::set_permissions(wrapper, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .expect("make wrapper mutable");
        assert_eq!(
            install_transport(&transport, &conflict_data)
                .expect_err("unmarked mutable wrapper")
                .kind(),
            AssetErrorKind::InstallConflict
        );

        let recovery_data = root.join("recovery-data");
        let installed =
            install_transport(&transport, &recovery_data).expect("install recovery fixture");
        fs::remove_file(recovery_data.join("active.json")).expect("remove active profile");
        fs::set_permissions(
            installed.path.parent().expect("recovery wrapper"),
            fs::Permissions::from_mode(PRIVATE_DIR_MODE),
        )
        .expect("make recovery wrapper mutable");
        let nonce = "11111111111111111111111111111111";
        write_test_stage(
            &recovery_data,
            nonce,
            &installed.bundle_id,
            &installed.transport_id,
            true,
        );
        let recovered = install_transport(&transport, &recovery_data).expect("marked recovery");
        assert_eq!(recovered.status, "reused");
        assert_eq!(
            fs::symlink_metadata(installed.path.parent().expect("recovered wrapper"))
                .expect("recovered wrapper metadata")
                .permissions()
                .mode()
                & 0o777,
            BUNDLE_MODE
        );

        let preflight_data = root.join("preflight-data");
        let installed =
            install_transport(&transport, &preflight_data).expect("install preflight fixture");
        let first = "22222222222222222222222222222222";
        write_test_stage(
            &preflight_data,
            first,
            &installed.bundle_id,
            &installed.transport_id,
            false,
        );
        let later = preflight_data
            .join(".staging")
            .join("33333333333333333333333333333333");
        fs::create_dir(&later).expect("later stage");
        fs::set_permissions(&later, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .expect("later stage mode");
        fs::write(later.join("marker.json"), b"malformed").expect("later marker");
        fs::set_permissions(
            later.join("marker.json"),
            fs::Permissions::from_mode(STAGE_MARKER_MODE),
        )
        .expect("later marker mode");
        assert_eq!(
            install_transport(&transport, &preflight_data)
                .expect_err("later malformed stage")
                .kind(),
            AssetErrorKind::StagingInvalid
        );
        assert!(
            preflight_data.join(".staging").join(first).exists(),
            "read-only preflight must not clean an earlier valid stage"
        );

        make_tree_writable(&root);
        fs::remove_dir_all(root).expect("reconcile cleanup");
    }

    #[test]
    fn recovery_preserves_installed_state_and_conflict_error_kinds() {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pangopup-install-recovery-errors-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("recovery error root");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/snv-regression/bundle");
        let transport = root.join("transport");
        crate::pack_bundle(&fixture, &transport).expect("pack recovery error transport");

        let receipt_data = root.join("receipt-data");
        let receipt_install =
            install_transport(&transport, &receipt_data).expect("install receipt fixture");
        fs::remove_file(receipt_data.join("active.json")).expect("remove receipt active");
        write_test_stage(
            &receipt_data,
            "44444444444444444444444444444444",
            &receipt_install.bundle_id,
            &receipt_install.transport_id,
            true,
        );
        let receipt_path = receipt_install
            .path
            .parent()
            .expect("receipt wrapper")
            .join("receipt.json");
        let mut receipt: Receipt = parse_canonical(
            &fs::read(&receipt_path).expect("receipt bytes"),
            AssetErrorKind::AssetStateInvalid,
            "test receipt",
        )
        .expect("receipt parse");
        receipt.members[0].sha256 = format!("sha256:{}", "0".repeat(64));
        fs::set_permissions(&receipt_path, fs::Permissions::from_mode(METADATA_MODE))
            .expect("receipt writable");
        fs::write(
            &receipt_path,
            canonical(&receipt, AssetErrorKind::AssetIo, "test receipt")
                .expect("canonical receipt"),
        )
        .expect("corrupt receipt identity");
        fs::set_permissions(&receipt_path, fs::Permissions::from_mode(MEMBER_MODE))
            .expect("receipt immutable");
        assert_eq!(
            install_transport(&transport, &receipt_data)
                .expect_err("recovery receipt/member mismatch")
                .kind(),
            AssetErrorKind::AssetStateInvalid
        );

        let magic_data = root.join("magic-data");
        let magic_install =
            install_transport(&transport, &magic_data).expect("install magic fixture");
        fs::remove_file(magic_data.join("active.json")).expect("remove magic active");
        write_test_stage(
            &magic_data,
            "55555555555555555555555555555555",
            &magic_install.bundle_id,
            &magic_install.transport_id,
            true,
        );
        let scores_path = magic_install.path.join("scores.pgi");
        let mut scores = fs::read(&scores_path).expect("score bytes");
        scores[0] ^= 0xff;
        fs::set_permissions(&scores_path, fs::Permissions::from_mode(METADATA_MODE))
            .expect("scores writable");
        fs::write(&scores_path, scores).expect("corrupt scores magic");
        fs::set_permissions(&scores_path, fs::Permissions::from_mode(MEMBER_MODE))
            .expect("scores immutable");
        assert_eq!(
            install_transport(&transport, &magic_data)
                .expect_err("recovery structural conflict")
                .kind(),
            AssetErrorKind::InstallConflict
        );

        make_tree_writable(&root);
        fs::remove_dir_all(root).expect("recovery error cleanup");
    }

    fn write_test_stage(
        data: &Path,
        nonce: &str,
        bundle_id: &str,
        transport_id: &str,
        candidate: bool,
    ) {
        let stage = data.join(".staging").join(nonce);
        fs::create_dir(&stage).expect("test stage");
        fs::set_permissions(&stage, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .expect("test stage mode");
        fs::create_dir(stage.join("payload")).expect("test payload");
        fs::set_permissions(
            stage.join("payload"),
            fs::Permissions::from_mode(PRIVATE_DIR_MODE),
        )
        .expect("test payload mode");
        let marker = StageMarker {
            schema: STAGE_SCHEMA.to_owned(),
            nonce: nonce.to_owned(),
            euid: u64::from(effective_uid()),
            bundle_id: bundle_id.to_owned(),
            transport_id: transport_id.to_owned(),
        };
        fs::write(
            stage.join("marker.json"),
            canonical(&marker, AssetErrorKind::AssetIo, "test marker").expect("canonical marker"),
        )
        .expect("test marker");
        fs::set_permissions(
            stage.join("marker.json"),
            fs::Permissions::from_mode(STAGE_MARKER_MODE),
        )
        .expect("test marker mode");
        if candidate {
            let profile = ActiveProfile {
                schema: ACTIVE_SCHEMA.to_owned(),
                bundle_id: bundle_id.to_owned(),
            };
            fs::write(
                stage.join("active.candidate.json"),
                canonical(&profile, AssetErrorKind::AssetIo, "test candidate")
                    .expect("canonical candidate"),
            )
            .expect("test candidate");
            fs::set_permissions(
                stage.join("active.candidate.json"),
                fs::Permissions::from_mode(METADATA_MODE),
            )
            .expect("test candidate mode");
        }
    }

    fn make_tree_writable(path: &Path) {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return;
        };
        if metadata.is_dir() {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    make_tree_writable(&entry.path());
                }
            }
        } else if metadata.is_file() {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
    }
}
