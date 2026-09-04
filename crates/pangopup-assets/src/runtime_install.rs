//! Offline installation and bounded discovery of one coherent runtime profile.

use crate::{
    AssetError, AssetErrorKind, RuntimeProfile, SnvBundleInspection,
    canonical_runtime_profile_bytes, parse_runtime_profile, runtime_profile_id,
};
use pangopup_index::{
    mask::{AdmittedMaskDomains, MaskDomainsOpen},
    reference_admission::{InstalledReference, admit_installed_reference},
};
use pangopup_model::{ModelAdmission, ModelKernel, inspect_held_model_admission};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const ACTIVE_SCHEMA: &str = "pangopup.runtime-active.v1";
const RECEIPT_SCHEMA: &str = "pangopup.runtime-install-receipt.v1";
const MAX_JSON: u64 = 1024 * 1024;
const MAX_NOTICE: u64 = 64 * 1024;
const DIR_PRIVATE: u32 = 0o700;
const DIR_IMMUTABLE: u32 = 0o555;
const FILE_PRIVATE: u32 = 0o600;
const FILE_IMMUTABLE: u32 = 0o444;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitionFault {
    StagedObjectsDurable,
    ComponentsPublished,
    ProfilePublished,
    BeforeActiveRename,
    AfterActiveRename,
}

#[cfg(test)]
thread_local! {
    static TRANSITION_FAULT: std::cell::Cell<Option<TransitionFault>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
type StagingReplacement = Box<dyn FnOnce() -> Result<(), AssetError>>;

#[cfg(test)]
type SourceReplacement = Box<dyn FnOnce()>;

#[cfg(test)]
thread_local! {
    static SOURCE_MUTATION: std::cell::RefCell<Option<SourceReplacement>> =
        const { std::cell::RefCell::new(None) };
    static REPLACE_DESTINATION: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static REPLACE_RUNTIME_BEFORE_RETURN: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static REPLACE_STAGING_DIRECTORY: std::cell::RefCell<Option<StagingReplacement>> =
        const { std::cell::RefCell::new(None) };
    static REPLACE_SOURCE_DIRECTORY: std::cell::RefCell<Option<SourceReplacement>> =
        const { std::cell::RefCell::new(None) };
    static REPLACE_PROFILE_PATH: std::cell::RefCell<Option<SourceReplacement>> =
        const { std::cell::RefCell::new(None) };
    static STATUS_READ_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn mutate_source_for_test() {
    SOURCE_MUTATION.with(|mutation| {
        if let Some(mutation) = mutation.borrow_mut().take() {
            mutation();
        }
    });
}

#[cfg(test)]
fn replace_staging_directory_for_test() -> Result<(), AssetError> {
    REPLACE_STAGING_DIRECTORY.with(|replacement| {
        let Some(replacement) = replacement.borrow_mut().take() else {
            return Ok(());
        };
        replacement()
    })
}

#[cfg(test)]
fn replace_source_directory_for_test() {
    REPLACE_SOURCE_DIRECTORY.with(|replacement| {
        if let Some(replacement) = replacement.borrow_mut().take() {
            replacement();
        }
    });
}

#[cfg(test)]
fn replace_profile_path_for_test() {
    REPLACE_PROFILE_PATH.with(|replacement| {
        if let Some(replacement) = replacement.borrow_mut().take() {
            replacement();
        }
    });
}

#[cfg(test)]
fn mutate_destination_for_test(components: &super::local::Dir, root: &super::local::Root) {
    if !REPLACE_DESTINATION.replace(false) {
        return;
    }
    super::local::rename_owned_replace(components, "model", components, "model-replaced")
        .expect("move held destination");
    super::local::create_owned_dir(components, "model", DIR_PRIVATE, root)
        .expect("replacement destination");
}

#[cfg(test)]
fn mutate_runtime_before_return_for_test(bundle: &super::local::Dir, root: &super::local::Root) {
    if !REPLACE_RUNTIME_BEFORE_RETURN.replace(false) {
        return;
    }
    super::local::set_mode(&bundle.file, DIR_PRIVATE).expect("make test bundle writable");
    super::local::rename_owned_replace(bundle, "model.onnx", bundle, "model.held")
        .expect("move admitted model");
    let mut held = super::local::open_owned_file(
        bundle,
        "model.held",
        FILE_IMMUTABLE,
        root,
        AssetErrorKind::StagingInvalid,
    )
    .expect("open admitted model");
    let mut replacement = super::local::create_owned_file(bundle, "model.onnx", FILE_PRIVATE, root)
        .expect("create replacement model");
    std::io::copy(&mut held, &mut replacement).expect("replace admitted model");
    super::local::set_mode(&replacement, FILE_IMMUTABLE).expect("replacement mode");
    super::local::set_mode(&bundle.file, DIR_IMMUTABLE).expect("restore bundle mode");
}

#[cfg(not(test))]
fn mutate_source_for_test() {}

#[cfg(not(test))]
fn replace_staging_directory_for_test() -> Result<(), AssetError> {
    Ok(())
}

#[cfg(not(test))]
fn replace_source_directory_for_test() {}

#[cfg(not(test))]
fn replace_profile_path_for_test() {}

#[cfg(not(test))]
fn mutate_destination_for_test(_components: &super::local::Dir, _root: &super::local::Root) {}

#[cfg(not(test))]
fn mutate_runtime_before_return_for_test(_bundle: &super::local::Dir, _root: &super::local::Root) {}

#[cfg(test)]
fn transition(point: TransitionFault) -> Result<(), AssetError> {
    if TRANSITION_FAULT.get() == Some(point) {
        TRANSITION_FAULT.set(None);
        Err(output("injected runtime installation transition failure"))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
macro_rules! transition {
    ($point:ident) => {};
}

#[cfg(test)]
macro_rules! transition {
    ($point:ident) => {
        transition(TransitionFault::$point)?
    };
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeInstallOutcome {
    pub status: &'static str,
    pub profile_id: String,
    pub snv_bundle_id: String,
    pub model_bundle_id: String,
    pub reference_bundle_id: String,
    pub mask_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeLocalStatus {
    Missing {
        data_dir: PathBuf,
    },
    Installing {
        data_dir: PathBuf,
    },
    Ready {
        profile_id: String,
        snv_bundle_id: String,
        model_bundle_id: String,
        reference_bundle_id: String,
        mask_sha256: String,
        model_path: PathBuf,
        reference_path: PathBuf,
        mask_path: PathBuf,
        installing: bool,
    },
}

/// One immutable installed model bundle represented only by authenticated,
/// held inputs.
pub struct InstalledModelInput {
    manifest_bytes: Vec<u8>,
    notice_bytes: Vec<u8>,
    member: File,
    admission: ModelAdmission,
}

impl InstalledModelInput {
    pub const fn admission(&self) -> &ModelAdmission {
        &self.admission
    }

    /// Initialize the production kernel through the admitted descriptor.
    pub fn open(self) -> Result<ModelKernel, AssetError> {
        ModelKernel::open_held_authenticated(&self.manifest_bytes, &self.notice_bytes, self.member)
            .map_err(|_| profile_corrupt_runtime())
    }

    /// Initialize this authenticated held model using service-owned CPU
    /// scheduling rather than the portable default.
    pub fn open_with_cpu_policy(
        self,
        policy: pangopup_model::CpuPolicy,
    ) -> Result<ModelKernel, AssetError> {
        ModelKernel::open_held_authenticated_with_cpu_policy(
            &self.manifest_bytes,
            &self.notice_bytes,
            self.member,
            policy,
        )
        .map_err(|_| profile_corrupt_runtime())
    }
}

/// The exact installed model-side tuple bound to an already-open SNV identity.
pub struct InstalledRuntimeProfile {
    profile: RuntimeProfile,
    model: InstalledModelInput,
    reference: InstalledReference,
    mask: AdmittedMaskDomains,
}

impl std::fmt::Debug for InstalledRuntimeProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledRuntimeProfile")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl InstalledRuntimeProfile {
    pub const fn profile(&self) -> &RuntimeProfile {
        &self.profile
    }

    pub fn into_parts(
        self,
    ) -> (
        RuntimeProfile,
        InstalledModelInput,
        InstalledReference,
        AdmittedMaskDomains,
    ) {
        (self.profile, self.model, self.reference, self.mask)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeActive {
    schema: String,
    profile_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeReceipt {
    schema: String,
    profile_id: String,
    snv_bundle_id: String,
    model: InstalledComponent,
    reference: InstalledComponent,
    mask: InstalledComponent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledComponent {
    path: String,
    size: u64,
    sha256: String,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnvInstallReceipt {
    schema: String,
    bundle_id: String,
    transport_id: String,
    members: Vec<InstalledComponent>,
}

#[derive(Clone, Copy)]
struct InstallSources<'a> {
    model: &'a Path,
    reference: &'a Path,
    mask: &'a Path,
}

pub fn install_runtime_profile(
    profile_path: &Path,
    model_bundle: &Path,
    reference_bundle: &Path,
    mask: &Path,
    data_root: &Path,
) -> Result<RuntimeInstallOutcome, AssetError> {
    let bytes = read_small_source(profile_path, MAX_JSON)?;
    let profile = parse_runtime_profile(&bytes).map_err(profile_parse_error)?;
    profile
        .require_trusted_production()
        .map_err(profile_parse_error)?;
    install_with_stager(&bytes, &profile, data_root, |stage, root| {
        stage_local_sources(
            InstallSources {
                model: model_bundle,
                reference: reference_bundle,
                mask,
            },
            stage,
            root,
            &profile,
        )
    })
}

#[cfg(feature = "test-fixtures")]
pub fn install_test_runtime_profile(
    profile: &RuntimeProfile,
    model_bundle: &Path,
    reference_bundle: &Path,
    mask: &Path,
    data_root: &Path,
) -> Result<RuntimeInstallOutcome, AssetError> {
    let bytes = canonical_runtime_profile_bytes(profile).map_err(profile_parse_error)?;
    install_with_stager(&bytes, profile, data_root, |stage, root| {
        stage_local_sources(
            InstallSources {
                model: model_bundle,
                reference: reference_bundle,
                mask,
            },
            stage,
            root,
            profile,
        )
    })
}

fn stage_local_sources(
    sources: InstallSources<'_>,
    stage: &super::local::Dir,
    root: &super::local::Root,
    profile: &RuntimeProfile,
) -> Result<(), AssetError> {
    let model = super::local::create_owned_dir(stage, "model", DIR_PRIVATE, root)?;
    copy_bundle(
        sources.model,
        &model,
        root,
        "model.onnx",
        profile.model.member_bytes,
        &profile.model.member_sha256,
    )?;
    let reference = super::local::create_owned_dir(stage, "reference", DIR_PRIVATE, root)?;
    copy_bundle(
        sources.reference,
        &reference,
        root,
        "reference.pgr",
        profile.reference.member_bytes,
        &profile.reference.member_sha256,
    )?;
    let mask = super::local::create_owned_dir(stage, "mask", DIR_PRIVATE, root)?;
    copy_path_member(
        sources.mask,
        &mask,
        "domains.pgm",
        root,
        profile.mask.member_bytes,
        &profile.mask.member_sha256,
    )?;
    mask.file.sync_all().map_err(|_| output("sync staged mask"))
}

pub(crate) fn install_with_stager<F>(
    profile_bytes: &[u8],
    profile: &RuntimeProfile,
    data_root: &Path,
    stage_sources: F,
) -> Result<RuntimeInstallOutcome, AssetError>
where
    F: FnOnce(&super::local::Dir, &super::local::Root) -> Result<(), AssetError>,
{
    let canonical = canonical_runtime_profile_bytes(profile).map_err(profile_parse_error)?;
    if canonical != profile_bytes {
        return Err(profile_corrupt("runtime profile is not canonical"));
    }
    let profile_id = runtime_profile_id(profile_bytes)
        .map_err(profile_parse_error)?
        .to_string();

    let locked = super::local::acquire_shared_install_lock(data_root)?;
    let root = &locked.root;
    let snv = super::local::inspect_active_snv_locked(root)?;
    require_snv(profile, &snv)?;

    let runtime_dir = super::local::ensure_private_dir(&root.dir, "runtime", root)?;
    let components_dir = super::local::ensure_private_dir(&runtime_dir, "components", root)?;
    let model_dir = super::local::ensure_private_dir(&components_dir, "model", root)?;
    let reference_dir = super::local::ensure_private_dir(&components_dir, "reference", root)?;
    let mask_dir = super::local::ensure_private_dir(&components_dir, "mask", root)?;
    let profiles_dir = super::local::ensure_private_dir(&runtime_dir, "profiles", root)?;
    let staging_dir = super::local::ensure_private_dir(&runtime_dir, ".staging", root)?;
    reconcile_staging(&staging_dir, root)?;
    reconcile_staged_active(&runtime_dir, root)?;

    if let Some(status) = validate_ready_held(root, profile)?
        && status.profile_id == profile_id
    {
        authenticate_ready_capabilities(
            root,
            profile,
            &runtime_dir,
            &components_dir,
            &model_dir,
            &reference_dir,
            &mask_dir,
            &profiles_dir,
            &staging_dir,
        )?;
        return Ok(outcome("reused", profile, profile_id));
    }

    let nonce = format!(
        "{:016x}-{:08x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| output("create staging nonce"))?
            .as_nanos(),
        std::process::id()
    );
    let stage_dir = super::local::create_owned_dir(&staging_dir, &nonce, DIR_PRIVATE, root)?;
    let mut cleanup_stage_name = true;
    let result = (|| {
        stage_sources(&stage_dir, root)?;
        transition!(StagedObjectsDurable);
        replace_staging_directory_for_test()?;
        if super::local::named_identity_matches(&staging_dir, &nonce, &stage_dir.file).is_err() {
            cleanup_stage_name = false;
            return Err(profile_corrupt("runtime staging directory was replaced"));
        }
        let staged_model = super::local::open_owned_dir(&stage_dir, "model", root, DIR_PRIVATE)?;
        let staged_reference =
            super::local::open_owned_dir(&stage_dir, "reference", root, DIR_PRIVATE)?;
        let staged_mask = super::local::open_owned_dir(&stage_dir, "mask", root, DIR_PRIVATE)?;

        validate_staged_held(
            profile,
            &staged_model,
            &staged_reference,
            &staged_mask,
            root,
        )?;

        let model_suffix = suffix(&profile.model.bundle_id)?;
        let reference_suffix = suffix(&profile.reference.bundle_id)?;
        let mask_suffix = suffix(&profile.mask.member_sha256)?;
        let model_published = publish_bundle(
            &stage_dir,
            "model",
            &model_dir,
            model_suffix,
            root,
            "model.onnx",
            profile.model.member_bytes,
        )?;
        let reference_published = publish_bundle(
            &stage_dir,
            "reference",
            &reference_dir,
            reference_suffix,
            root,
            "reference.pgr",
            profile.reference.member_bytes,
        )?;
        let mask_published = publish_mask(
            &stage_dir,
            "mask",
            &mask_dir,
            mask_suffix,
            root,
            profile.mask.member_bytes,
        )?;
        let model_member = authenticate_member(
            &model_published.bundle,
            "model.onnx",
            root,
            profile.model.member_bytes,
            &profile.model.member_sha256,
        )?;
        validate_model_bundle_held(&model_published.bundle, profile, root)
            .map_err(|_| conflict("immutable model component conflicts"))?;
        let reference_member = authenticate_member(
            &reference_published.bundle,
            "reference.pgr",
            root,
            profile.reference.member_bytes,
            &profile.reference.member_sha256,
        )?;
        validate_reference_bundle_held(&reference_published.bundle, profile, root)
            .map_err(|_| conflict("immutable reference component conflicts"))?;
        let mask_member = authenticate_member(
            &mask_published.dir,
            "domains.pgm",
            root,
            profile.mask.member_bytes,
            &profile.mask.member_sha256,
        )?;
        validate_mask_held(&mask_published.dir, profile, root)
            .map_err(|_| conflict("immutable mask component conflicts"))?;
        transition!(ComponentsPublished);

        let profile_suffix = suffix(&profile_id)?;
        let staged_profile =
            super::local::create_owned_dir(&stage_dir, "profile", DIR_PRIVATE, root)?;
        write_new_held(
            &staged_profile,
            "profile.json",
            profile_bytes,
            FILE_IMMUTABLE,
            root,
        )?;
        let receipt = receipt(profile, &profile_id);
        let receipt_bytes =
            serde_jcs::to_vec(&receipt).map_err(|_| output("serialize runtime receipt"))?;
        write_new_held(
            &staged_profile,
            "receipt.json",
            &receipt_bytes,
            FILE_IMMUTABLE,
            root,
        )?;
        staged_profile
            .file
            .sync_all()
            .map_err(|_| output("sync staged profile"))?;
        let published_profile = publish_directory(
            &stage_dir,
            "profile",
            &profiles_dir,
            profile_suffix,
            root,
            |dir| validate_profile_directory_held(dir, profile, &profile_id, root).map(|_| ()),
        )?;
        transition!(ProfilePublished);

        validate_profile_directory_held(&published_profile.dir, profile, &profile_id, root)?;
        mutate_destination_for_test(&components_dir, root);
        verify_runtime_topology(
            root,
            &runtime_dir,
            &components_dir,
            &model_dir,
            &reference_dir,
            &mask_dir,
            &profiles_dir,
            &staging_dir,
            &model_published,
            &reference_published,
            &mask_published,
            &published_profile,
            &model_member,
            &reference_member,
            &mask_member,
            model_suffix,
            reference_suffix,
            mask_suffix,
            profile_suffix,
        )?;
        let active = RuntimeActive {
            schema: ACTIVE_SCHEMA.to_owned(),
            profile_id: profile_id.clone(),
        };
        let active_bytes =
            serde_jcs::to_vec(&active).map_err(|_| output("serialize runtime active state"))?;
        if super::local::open_owned_file_optional(
            &runtime_dir,
            ".active.new",
            FILE_PRIVATE,
            root,
            AssetErrorKind::InstallConflict,
        )?
        .is_some()
        {
            return Err(conflict("staged active pointer already exists"));
        }
        write_new_held(
            &runtime_dir,
            ".active.new",
            &active_bytes,
            FILE_PRIVATE,
            root,
        )?;
        transition!(BeforeActiveRename);
        super::local::rename_owned_replace(
            &runtime_dir,
            ".active.new",
            &runtime_dir,
            "active.json",
        )?;
        transition!(AfterActiveRename);
        runtime_dir
            .file
            .sync_all()
            .map_err(|_| output("sync runtime directory"))?;
        Ok(outcome("installed", profile, profile_id.clone()))
    })();
    // A failed identity check proves that this name no longer denotes our
    // held stage. Do not authorize name-based cleanup of its replacement.
    if cleanup_stage_name
        && super::local::named_identity_matches(&staging_dir, &nonce, &stage_dir.file).is_ok()
    {
        let _ = remove_stage(&staging_dir, &nonce);
    }
    result
}

#[cfg(test)]
fn install_with_profile(
    profile_bytes: &[u8],
    profile: &RuntimeProfile,
    sources: InstallSources<'_>,
    data_root: &Path,
) -> Result<RuntimeInstallOutcome, AssetError> {
    install_with_stager(profile_bytes, profile, data_root, |stage, root| {
        stage_local_sources(sources, stage, root, profile)
    })
}

pub fn runtime_local_status(data_root: &Path) -> Result<RuntimeLocalStatus, AssetError> {
    let Some(root) = super::local::open_root(data_root, false)? else {
        return Ok(RuntimeLocalStatus::Missing {
            data_dir: data_root.to_owned(),
        });
    };
    let installing = super::local::probe_install_lock(&root)?;
    runtime_local_status_with(
        &root,
        data_root,
        &crate::production_runtime_profile(),
        installing,
    )
}

pub(crate) fn runtime_local_status_locked(
    locked: &super::local::LockedRoot,
) -> Result<RuntimeLocalStatus, AssetError> {
    runtime_local_status_with(
        &locked.root,
        &locked.root.path,
        &crate::production_runtime_profile(),
        false,
    )
}

#[cfg(test)]
pub(crate) fn runtime_local_status_locked_with_profile(
    locked: &super::local::LockedRoot,
    profile: &RuntimeProfile,
) -> Result<RuntimeLocalStatus, AssetError> {
    runtime_local_status_with(&locked.root, &locked.root.path, profile, false)
}

/// Admit the active installed runtime tuple for the exact already-open SNV
/// identity. Neither the SNV payload nor the dense reference payload is
/// scanned.
pub fn open_installed_runtime_profile(
    data_root: &Path,
    expected_snv_bundle_id: &str,
) -> Result<InstalledRuntimeProfile, AssetError> {
    open_installed_runtime_profile_with(
        data_root,
        Some(expected_snv_bundle_id),
        &crate::production_runtime_profile(),
    )
}

#[cfg(feature = "test-fixtures")]
pub fn open_test_runtime_profile(
    data_root: &Path,
    expected_snv_bundle_id: &str,
    trusted: &RuntimeProfile,
) -> Result<InstalledRuntimeProfile, AssetError> {
    open_installed_runtime_profile_with(data_root, Some(expected_snv_bundle_id), trusted)
}

/// Admit the model-side members of the active canonical runtime profile.
///
/// This path validates the complete trusted profile identity but neither
/// requires nor opens the separately installed SNV lookup bundle.
pub fn open_installed_runtime_profile_for_model(
    data_root: &Path,
) -> Result<InstalledRuntimeProfile, AssetError> {
    open_installed_runtime_profile_with(data_root, None, &crate::production_runtime_profile())
}

fn open_installed_runtime_profile_with(
    data_root: &Path,
    expected_snv_bundle_id: Option<&str>,
    trusted: &RuntimeProfile,
) -> Result<InstalledRuntimeProfile, AssetError> {
    let root = super::local::open_root(data_root, false)
        .map_err(|_| profile_unsafe_runtime())?
        .ok_or_else(runtime_missing)?;
    open_installed_runtime_profile_from_root(&root, expected_snv_bundle_id, trusted)
}

fn open_installed_runtime_profile_from_root(
    root: &super::local::Root,
    expected_snv_bundle_id: Option<&str>,
    trusted: &RuntimeProfile,
) -> Result<InstalledRuntimeProfile, AssetError> {
    let runtime = open_runtime_dir_optional(&root.dir, "runtime", root, DIR_PRIVATE)?
        .ok_or_else(runtime_missing)?;
    let active_file = super::local::open_owned_file_optional(
        &runtime,
        "active.json",
        FILE_PRIVATE,
        root,
        AssetErrorKind::StagingInvalid,
    )
    .map_err(|_| profile_unsafe_runtime())?
    .ok_or_else(runtime_missing)?;
    require_held_file(&active_file, FILE_PRIVATE, MAX_JSON, None)?;
    let active_bytes = super::local::read_bounded_handle_ref(
        &active_file,
        MAX_JSON,
        AssetErrorKind::BundleInvalid,
    )
    .map_err(|_| profile_corrupt_runtime())?;
    let active: RuntimeActive = parse_canonical_runtime_bytes(&active_bytes)?;
    if active.schema != ACTIVE_SCHEMA || !valid_identity(&active.profile_id) {
        return Err(profile_corrupt_runtime());
    }

    let components = open_runtime_dir(&runtime, "components", root, DIR_PRIVATE)?;
    let model_parent = open_runtime_dir(&components, "model", root, DIR_PRIVATE)?;
    let reference_parent = open_runtime_dir(&components, "reference", root, DIR_PRIVATE)?;
    let mask_parent = open_runtime_dir(&components, "mask", root, DIR_PRIVATE)?;
    let profiles = open_runtime_dir(&runtime, "profiles", root, DIR_PRIVATE)?;
    let profile_dir =
        open_runtime_dir(&profiles, suffix(&active.profile_id)?, root, DIR_IMMUTABLE)?;
    require_runtime_names(&profile_dir, &["profile.json", "receipt.json"])?;
    let profile_file = open_runtime_file(&profile_dir, "profile.json", root, FILE_IMMUTABLE)?;
    let receipt_file = open_runtime_file(&profile_dir, "receipt.json", root, FILE_IMMUTABLE)?;
    require_held_file(&profile_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    require_held_file(&receipt_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    let profile_bytes = super::local::read_bounded_handle_ref(
        &profile_file,
        MAX_JSON,
        AssetErrorKind::BundleInvalid,
    )
    .map_err(|_| profile_corrupt_runtime())?;
    let profile = parse_runtime_profile(&profile_bytes).map_err(|_| profile_corrupt_runtime())?;
    let observed_id = runtime_profile_id(&profile_bytes)
        .map_err(|_| profile_corrupt_runtime())?
        .to_string();
    if observed_id != active.profile_id {
        return Err(profile_corrupt_runtime());
    }
    if &profile != trusted
        || expected_snv_bundle_id.is_some_and(|expected| {
            profile.snv.bundle_id != expected || trusted.snv.bundle_id != expected
        })
    {
        return Err(profile_incompatible_runtime());
    }
    let receipt_bytes = super::local::read_bounded_handle_ref(
        &receipt_file,
        MAX_JSON,
        AssetErrorKind::BundleInvalid,
    )
    .map_err(|_| profile_corrupt_runtime())?;
    let installed_receipt: RuntimeReceipt = parse_canonical_runtime_bytes(&receipt_bytes)?;
    if installed_receipt != receipt(&profile, &observed_id) {
        return Err(profile_corrupt_runtime());
    }

    let model_identity = open_runtime_dir(
        &model_parent,
        suffix(&profile.model.bundle_id)?,
        root,
        DIR_IMMUTABLE,
    )?;
    require_runtime_names(&model_identity, &["bundle"])?;
    let model_bundle = open_runtime_dir(&model_identity, "bundle", root, DIR_IMMUTABLE)?;
    let model_files = open_installed_bundle(
        &model_bundle,
        "model.onnx",
        profile.model.member_bytes,
        root,
    )?;
    let model_admission = inspect_held_model_admission(
        &model_files.manifest_bytes,
        &model_files.notice_bytes,
        &model_files.member,
    )
    .map_err(|_| profile_corrupt_runtime())?;
    if model_admission.bundle_id().as_str() != profile.model.bundle_id
        || model_admission.profile() != profile.model.profile
        || model_admission.representation().to_string() != profile.model.representation
    {
        return Err(profile_incompatible_runtime());
    }

    let reference_identity = open_runtime_dir(
        &reference_parent,
        suffix(&profile.reference.bundle_id)?,
        root,
        DIR_IMMUTABLE,
    )?;
    require_runtime_names(&reference_identity, &["bundle"])?;
    let reference_bundle = open_runtime_dir(&reference_identity, "bundle", root, DIR_IMMUTABLE)?;
    let reference_files = open_installed_bundle(
        &reference_bundle,
        "reference.pgr",
        profile.reference.member_bytes,
        root,
    )?;
    if format!(
        "sha256:{:x}",
        Sha256::digest(&reference_files.manifest_bytes)
    ) != profile.reference.bundle_id
    {
        return Err(profile_incompatible_runtime());
    }
    // SAFETY: the installer authenticated this descriptor before immutable
    // publication. This admission rechecks the held topology, metadata, exact
    // size, canonical metadata and trusted profile before construction.
    let reference = unsafe {
        admit_installed_reference(
            &reference_files.manifest_bytes,
            &reference_files.notice_bytes,
            reference_files
                .member
                .try_clone()
                .map_err(|_| profile_corrupt_runtime())?,
        )
    }
    .map_err(|_| profile_corrupt_runtime())?;
    if reference.manifest().profile != profile.reference.profile
        || reference.manifest().reference_format != profile.reference.format
        || reference.manifest().source.assembly != profile.reference.assembly
        || reference.manifest().source.assembly_accession != profile.reference.assembly_accession
        || reference.manifest().sequences.sequence_set_sha256
            != profile.reference.sequence_set_sha256
    {
        return Err(profile_incompatible_runtime());
    }

    let mask_identity = open_runtime_dir(
        &mask_parent,
        suffix(&profile.mask.member_sha256)?,
        root,
        DIR_IMMUTABLE,
    )?;
    require_runtime_names(&mask_identity, &["domains.pgm"])?;
    let mask_file = open_runtime_file(&mask_identity, "domains.pgm", root, FILE_IMMUTABLE)?;
    require_held_file(
        &mask_file,
        FILE_IMMUTABLE,
        profile.mask.member_bytes,
        Some(profile.mask.member_bytes),
    )?;
    let mask = MaskDomainsOpen::admit_held(
        mask_file
            .try_clone()
            .map_err(|_| profile_corrupt_runtime())?,
    )
    .map_err(|_| profile_corrupt_runtime())?;
    if mask.identity().bytes() != profile.mask.member_bytes
        || format!("sha256:{}", mask.identity().sha256()) != profile.mask.member_sha256
    {
        return Err(profile_incompatible_runtime());
    }

    mutate_runtime_before_return_for_test(&model_bundle, root);
    for (parent, name, held) in [
        (&root.dir, "runtime", &runtime.file),
        (&runtime, "active.json", &active_file),
        (&runtime, "components", &components.file),
        (&components, "model", &model_parent.file),
        (&components, "reference", &reference_parent.file),
        (&components, "mask", &mask_parent.file),
        (&runtime, "profiles", &profiles.file),
        (&profiles, suffix(&active.profile_id)?, &profile_dir.file),
        (&profile_dir, "profile.json", &profile_file),
        (&profile_dir, "receipt.json", &receipt_file),
        (
            &model_parent,
            suffix(&profile.model.bundle_id)?,
            &model_identity.file,
        ),
        (&model_identity, "bundle", &model_bundle.file),
        (&model_bundle, "manifest.json", &model_files.manifest_file),
        (&model_bundle, "NOTICE", &model_files.notice_file),
        (&model_bundle, "model.onnx", &model_files.member),
        (
            &reference_parent,
            suffix(&profile.reference.bundle_id)?,
            &reference_identity.file,
        ),
        (&reference_identity, "bundle", &reference_bundle.file),
        (
            &reference_bundle,
            "manifest.json",
            &reference_files.manifest_file,
        ),
        (&reference_bundle, "NOTICE", &reference_files.notice_file),
        (&reference_bundle, "reference.pgr", &reference_files.member),
        (
            &mask_parent,
            suffix(&profile.mask.member_sha256)?,
            &mask_identity.file,
        ),
        (&mask_identity, "domains.pgm", &mask_file),
    ] {
        super::local::named_identity_matches(parent, name, held)
            .map_err(|_| profile_corrupt_runtime())?;
    }

    Ok(InstalledRuntimeProfile {
        profile,
        model: InstalledModelInput {
            manifest_bytes: model_files.manifest_bytes,
            notice_bytes: model_files.notice_bytes,
            member: model_files.member,
            admission: model_admission,
        },
        reference,
        mask,
    })
}

struct InstalledBundleFiles {
    manifest_bytes: Vec<u8>,
    notice_bytes: Vec<u8>,
    manifest_file: File,
    notice_file: File,
    member: File,
}

fn open_runtime_dir_optional(
    parent: &super::local::Dir,
    name: &str,
    root: &super::local::Root,
    mode: u32,
) -> Result<Option<super::local::Dir>, AssetError> {
    let Some(dir) = super::local::open_owned_dir_optional(parent, name, root)
        .map_err(|_| profile_unsafe_runtime())?
    else {
        return Ok(None);
    };
    let metadata = dir.file.metadata().map_err(|_| profile_corrupt_runtime())?;
    if metadata.mode() & 0o777 != mode {
        return Err(profile_unsafe_runtime());
    }
    Ok(Some(dir))
}

fn open_runtime_dir(
    parent: &super::local::Dir,
    name: &str,
    root: &super::local::Root,
    mode: u32,
) -> Result<super::local::Dir, AssetError> {
    open_runtime_dir_optional(parent, name, root, mode)?.ok_or_else(profile_corrupt_runtime)
}

fn open_runtime_file(
    parent: &super::local::Dir,
    name: &str,
    root: &super::local::Root,
    mode: u32,
) -> Result<File, AssetError> {
    super::local::open_owned_file_optional(parent, name, mode, root, AssetErrorKind::StagingInvalid)
        .map_err(|_| profile_unsafe_runtime())?
        .ok_or_else(profile_corrupt_runtime)
}

fn require_held_file(
    file: &File,
    mode: u32,
    maximum: u64,
    exact: Option<u64>,
) -> Result<(), AssetError> {
    let metadata = file.metadata().map_err(|_| profile_corrupt_runtime())?;
    if !metadata.file_type().is_file()
        || metadata.uid() != effective_uid()
        || metadata.nlink() != 1
        || metadata.mode() & 0o777 != mode
    {
        return Err(profile_unsafe_runtime());
    }
    if metadata.len() > maximum || exact.is_some_and(|value| metadata.len() != value) {
        return Err(profile_corrupt_runtime());
    }
    Ok(())
}

fn require_runtime_names(dir: &super::local::Dir, expected: &[&str]) -> Result<(), AssetError> {
    let names = super::local::read_names_bounded(dir, expected.len().saturating_add(1))
        .map_err(|_| profile_unsafe_runtime())?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if names.iter().any(|name| !expected.contains(name)) {
        return Err(profile_unsafe_runtime());
    }
    if names != expected {
        return Err(profile_corrupt_runtime());
    }
    Ok(())
}

fn open_installed_bundle(
    bundle: &super::local::Dir,
    payload_name: &str,
    payload_bytes: u64,
    root: &super::local::Root,
) -> Result<InstalledBundleFiles, AssetError> {
    require_runtime_names(bundle, &["NOTICE", "manifest.json", payload_name])?;
    let manifest = open_runtime_file(bundle, "manifest.json", root, FILE_IMMUTABLE)?;
    let notice = open_runtime_file(bundle, "NOTICE", root, FILE_IMMUTABLE)?;
    let member = open_runtime_file(bundle, payload_name, root, FILE_IMMUTABLE)?;
    require_held_file(&manifest, FILE_IMMUTABLE, MAX_JSON, None)?;
    require_held_file(&notice, FILE_IMMUTABLE, MAX_NOTICE, None)?;
    require_held_file(&member, FILE_IMMUTABLE, payload_bytes, Some(payload_bytes))?;
    Ok(InstalledBundleFiles {
        manifest_bytes: super::local::read_bounded_handle_ref(
            &manifest,
            MAX_JSON,
            AssetErrorKind::BundleInvalid,
        )
        .map_err(|_| profile_corrupt_runtime())?,
        notice_bytes: super::local::read_bounded_handle_ref(
            &notice,
            MAX_NOTICE,
            AssetErrorKind::BundleInvalid,
        )
        .map_err(|_| profile_corrupt_runtime())?,
        manifest_file: manifest,
        notice_file: notice,
        member,
    })
}

fn parse_canonical_runtime_bytes<T>(bytes: &[u8]) -> Result<T, AssetError>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|_| profile_corrupt_runtime())?;
    if serde_jcs::to_vec(&value).map_err(|_| profile_corrupt_runtime())? != bytes {
        return Err(profile_corrupt_runtime());
    }
    Ok(value)
}

fn runtime_local_status_with(
    root: &super::local::Root,
    display_root: &Path,
    trusted: &RuntimeProfile,
    installing: bool,
) -> Result<RuntimeLocalStatus, AssetError> {
    match validate_ready_held(root, trusted)? {
        Some(ready) => Ok(RuntimeLocalStatus::Ready {
            profile_id: ready.profile_id,
            snv_bundle_id: ready.snv_bundle_id,
            model_bundle_id: ready.model_bundle_id,
            reference_bundle_id: ready.reference_bundle_id,
            mask_sha256: ready.mask_sha256,
            model_path: ready.model_path,
            reference_path: ready.reference_path,
            mask_path: ready.mask_path,
            installing,
        }),
        None if installing => Ok(RuntimeLocalStatus::Installing {
            data_dir: display_root.to_owned(),
        }),
        None => Ok(RuntimeLocalStatus::Missing {
            data_dir: display_root.to_owned(),
        }),
    }
}

fn validate_ready_held(
    root: &super::local::Root,
    trusted: &RuntimeProfile,
) -> Result<Option<Ready>, AssetError> {
    match validate_ready_held_inner(root, trusted) {
        Err(error) if error.kind() == AssetErrorKind::StagingInvalid => {
            Err(profile_corrupt_runtime())
        }
        result => result,
    }
}

fn validate_ready_held_inner(
    root: &super::local::Root,
    trusted: &RuntimeProfile,
) -> Result<Option<Ready>, AssetError> {
    let Some(runtime) = open_runtime_dir_optional(&root.dir, "runtime", root, DIR_PRIVATE)? else {
        return Ok(None);
    };
    let Some(active_file) = super::local::open_owned_file_optional(
        &runtime,
        "active.json",
        FILE_PRIVATE,
        root,
        AssetErrorKind::StagingInvalid,
    )
    .map_err(|_| profile_unsafe_runtime())?
    else {
        return Ok(None);
    };
    require_held_file(&active_file, FILE_PRIVATE, MAX_JSON, None)?;
    let active_bytes = read_status_metadata(&active_file, MAX_JSON)?;
    let active: RuntimeActive = parse_canonical_runtime_bytes(&active_bytes)?;
    if active.schema != ACTIVE_SCHEMA || !valid_identity(&active.profile_id) {
        return Err(profile_corrupt_runtime());
    }

    let components = open_runtime_dir(&runtime, "components", root, DIR_PRIVATE)?;
    let model_parent = open_runtime_dir(&components, "model", root, DIR_PRIVATE)?;
    let reference_parent = open_runtime_dir(&components, "reference", root, DIR_PRIVATE)?;
    let mask_parent = open_runtime_dir(&components, "mask", root, DIR_PRIVATE)?;
    let profiles = open_runtime_dir(&runtime, "profiles", root, DIR_PRIVATE)?;
    let profile_dir =
        open_runtime_dir(&profiles, suffix(&active.profile_id)?, root, DIR_IMMUTABLE)?;
    require_runtime_names(&profile_dir, &["profile.json", "receipt.json"])?;
    let profile_file = open_runtime_file(&profile_dir, "profile.json", root, FILE_IMMUTABLE)?;
    let receipt_file = open_runtime_file(&profile_dir, "receipt.json", root, FILE_IMMUTABLE)?;
    require_held_file(&profile_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    require_held_file(&receipt_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    let profile_bytes = read_status_metadata(&profile_file, MAX_JSON)?;
    let profile = parse_runtime_profile(&profile_bytes).map_err(|_| profile_corrupt_runtime())?;
    let profile_id = runtime_profile_id(&profile_bytes)
        .map_err(|_| profile_corrupt_runtime())?
        .to_string();
    if profile_id != active.profile_id {
        return Err(profile_corrupt_runtime());
    }
    if &profile != trusted {
        return Err(profile_incompatible_runtime());
    }
    let receipt_bytes = read_status_metadata(&receipt_file, MAX_JSON)?;
    let installed_receipt: RuntimeReceipt = parse_canonical_runtime_bytes(&receipt_bytes)?;
    if installed_receipt != receipt(&profile, &profile_id) {
        return Err(profile_corrupt_runtime());
    }

    let model = open_status_bundle(
        &model_parent,
        suffix(&profile.model.bundle_id)?,
        "model.onnx",
        profile.model.member_bytes,
        root,
    )?;
    let reference = open_status_bundle(
        &reference_parent,
        suffix(&profile.reference.bundle_id)?,
        "reference.pgr",
        profile.reference.member_bytes,
        root,
    )?;
    let mask_identity = open_runtime_dir(
        &mask_parent,
        suffix(&profile.mask.member_sha256)?,
        root,
        DIR_IMMUTABLE,
    )?;
    require_runtime_names(&mask_identity, &["domains.pgm"])?;
    let mask_file = open_runtime_file(&mask_identity, "domains.pgm", root, FILE_IMMUTABLE)?;
    require_held_file(
        &mask_file,
        FILE_IMMUTABLE,
        profile.mask.member_bytes,
        Some(profile.mask.member_bytes),
    )?;

    for (parent, name, held) in [
        (&root.dir, "runtime", &runtime.file),
        (&runtime, "active.json", &active_file),
        (&runtime, "components", &components.file),
        (&components, "model", &model_parent.file),
        (&components, "reference", &reference_parent.file),
        (&components, "mask", &mask_parent.file),
        (&runtime, "profiles", &profiles.file),
        (&profiles, suffix(&profile_id)?, &profile_dir.file),
        (&profile_dir, "profile.json", &profile_file),
        (&profile_dir, "receipt.json", &receipt_file),
        (
            &model_parent,
            suffix(&profile.model.bundle_id)?,
            &model.identity.file,
        ),
        (&model.identity, "bundle", &model.bundle.file),
        (&model.bundle, "manifest.json", &model.manifest),
        (&model.bundle, "NOTICE", &model.notice),
        (&model.bundle, "model.onnx", &model.member),
        (
            &reference_parent,
            suffix(&profile.reference.bundle_id)?,
            &reference.identity.file,
        ),
        (&reference.identity, "bundle", &reference.bundle.file),
        (&reference.bundle, "manifest.json", &reference.manifest),
        (&reference.bundle, "NOTICE", &reference.notice),
        (&reference.bundle, "reference.pgr", &reference.member),
        (
            &mask_parent,
            suffix(&profile.mask.member_sha256)?,
            &mask_identity.file,
        ),
        (&mask_identity, "domains.pgm", &mask_file),
    ] {
        super::local::named_identity_matches(parent, name, held)
            .map_err(|_| profile_corrupt_runtime())?;
    }

    Ok(Some(Ready {
        profile_id,
        snv_bundle_id: profile.snv.bundle_id.clone(),
        model_bundle_id: profile.model.bundle_id.clone(),
        reference_bundle_id: profile.reference.bundle_id.clone(),
        mask_sha256: profile.mask.member_sha256.clone(),
        model_path: PathBuf::from("runtime")
            .join("components/model")
            .join(suffix(&profile.model.bundle_id)?)
            .join("bundle"),
        reference_path: PathBuf::from("runtime")
            .join("components/reference")
            .join(suffix(&profile.reference.bundle_id)?)
            .join("bundle"),
        mask_path: PathBuf::from("runtime")
            .join("components/mask")
            .join(suffix(&profile.mask.member_sha256)?)
            .join("domains.pgm"),
    }))
}

fn read_status_metadata(file: &File, maximum: u64) -> Result<Vec<u8>, AssetError> {
    let bytes = super::local::read_bounded_handle_ref(file, maximum, AssetErrorKind::BundleInvalid)
        .map_err(|_| profile_corrupt_runtime())?;
    #[cfg(test)]
    STATUS_READ_BYTES.set(STATUS_READ_BYTES.get().saturating_add(bytes.len() as u64));
    Ok(bytes)
}

struct StatusBundle {
    identity: super::local::Dir,
    bundle: super::local::Dir,
    manifest: File,
    notice: File,
    member: File,
}

fn open_status_bundle(
    parent: &super::local::Dir,
    identity: &str,
    payload: &str,
    payload_bytes: u64,
    root: &super::local::Root,
) -> Result<StatusBundle, AssetError> {
    let identity = open_runtime_dir(parent, identity, root, DIR_IMMUTABLE)?;
    require_runtime_names(&identity, &["bundle"])?;
    let bundle = open_runtime_dir(&identity, "bundle", root, DIR_IMMUTABLE)?;
    require_runtime_names(&bundle, &["NOTICE", "manifest.json", payload])?;
    let manifest = open_runtime_file(&bundle, "manifest.json", root, FILE_IMMUTABLE)?;
    let notice = open_runtime_file(&bundle, "NOTICE", root, FILE_IMMUTABLE)?;
    let member = open_runtime_file(&bundle, payload, root, FILE_IMMUTABLE)?;
    require_held_file(&manifest, FILE_IMMUTABLE, MAX_JSON, None)?;
    require_held_file(&notice, FILE_IMMUTABLE, MAX_NOTICE, None)?;
    require_held_file(&member, FILE_IMMUTABLE, payload_bytes, Some(payload_bytes))?;
    Ok(StatusBundle {
        identity,
        bundle,
        manifest,
        notice,
        member,
    })
}

struct Ready {
    profile_id: String,
    snv_bundle_id: String,
    model_bundle_id: String,
    reference_bundle_id: String,
    mask_sha256: String,
    model_path: PathBuf,
    reference_path: PathBuf,
    mask_path: PathBuf,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_ready_capabilities(
    root: &super::local::Root,
    profile: &RuntimeProfile,
    runtime: &super::local::Dir,
    components: &super::local::Dir,
    model_parent: &super::local::Dir,
    reference_parent: &super::local::Dir,
    mask_parent: &super::local::Dir,
    profiles: &super::local::Dir,
    staging: &super::local::Dir,
) -> Result<(), AssetError> {
    let profile_bytes = canonical_runtime_profile_bytes(profile).map_err(profile_parse_error)?;
    let profile_id = runtime_profile_id(&profile_bytes)
        .map_err(profile_parse_error)?
        .to_string();
    let model_identity = super::local::open_owned_dir(
        model_parent,
        suffix(&profile.model.bundle_id)?,
        root,
        DIR_IMMUTABLE,
    )?;
    let model_bundle =
        super::local::open_owned_dir(&model_identity, "bundle", root, DIR_IMMUTABLE)?;
    let model_member = authenticate_member(
        &model_bundle,
        "model.onnx",
        root,
        profile.model.member_bytes,
        &profile.model.member_sha256,
    )?;
    validate_model_bundle_held(&model_bundle, profile, root)
        .map_err(|_| conflict("immutable model component conflicts"))?;
    let reference_identity = super::local::open_owned_dir(
        reference_parent,
        suffix(&profile.reference.bundle_id)?,
        root,
        DIR_IMMUTABLE,
    )?;
    let reference_bundle =
        super::local::open_owned_dir(&reference_identity, "bundle", root, DIR_IMMUTABLE)?;
    let reference_member = authenticate_member(
        &reference_bundle,
        "reference.pgr",
        root,
        profile.reference.member_bytes,
        &profile.reference.member_sha256,
    )?;
    validate_reference_bundle_held(&reference_bundle, profile, root)
        .map_err(|_| conflict("immutable reference component conflicts"))?;
    let mask_identity = super::local::open_owned_dir(
        mask_parent,
        suffix(&profile.mask.member_sha256)?,
        root,
        DIR_IMMUTABLE,
    )?;
    let mask_member = authenticate_member(
        &mask_identity,
        "domains.pgm",
        root,
        profile.mask.member_bytes,
        &profile.mask.member_sha256,
    )?;
    validate_mask_held(&mask_identity, profile, root)
        .map_err(|_| conflict("immutable mask component conflicts"))?;
    let profile_identity =
        super::local::open_owned_dir(profiles, suffix(&profile_id)?, root, DIR_IMMUTABLE)?;
    validate_profile_directory_held(&profile_identity, profile, &profile_id, root)?;
    verify_runtime_topology(
        root,
        runtime,
        components,
        model_parent,
        reference_parent,
        mask_parent,
        profiles,
        staging,
        &PublishedBundle {
            identity: model_identity,
            bundle: model_bundle,
        },
        &PublishedBundle {
            identity: reference_identity,
            bundle: reference_bundle,
        },
        &PublishedDirectory { dir: mask_identity },
        &PublishedDirectory {
            dir: profile_identity,
        },
        &model_member,
        &reference_member,
        &mask_member,
        suffix(&profile.model.bundle_id)?,
        suffix(&profile.reference.bundle_id)?,
        suffix(&profile.mask.member_sha256)?,
        suffix(&profile_id)?,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_runtime_topology(
    root: &super::local::Root,
    runtime: &super::local::Dir,
    components: &super::local::Dir,
    model_parent: &super::local::Dir,
    reference_parent: &super::local::Dir,
    mask_parent: &super::local::Dir,
    profiles: &super::local::Dir,
    staging: &super::local::Dir,
    model: &PublishedBundle,
    reference: &PublishedBundle,
    mask: &PublishedDirectory,
    profile: &PublishedDirectory,
    model_member: &File,
    reference_member: &File,
    mask_member: &File,
    model_name: &str,
    reference_name: &str,
    mask_name: &str,
    profile_name: &str,
) -> Result<(), AssetError> {
    super::local::named_identity_matches(&root.dir, "runtime", &runtime.file)?;
    super::local::named_identity_matches(runtime, "components", &components.file)?;
    super::local::named_identity_matches(runtime, "profiles", &profiles.file)?;
    super::local::named_identity_matches(runtime, ".staging", &staging.file)?;
    super::local::named_identity_matches(components, "model", &model_parent.file)?;
    super::local::named_identity_matches(components, "reference", &reference_parent.file)?;
    super::local::named_identity_matches(components, "mask", &mask_parent.file)?;
    super::local::named_identity_matches(model_parent, model_name, &model.identity.file)?;
    super::local::named_identity_matches(&model.identity, "bundle", &model.bundle.file)?;
    super::local::named_identity_matches(&model.bundle, "model.onnx", model_member)?;
    super::local::named_identity_matches(
        reference_parent,
        reference_name,
        &reference.identity.file,
    )?;
    super::local::named_identity_matches(&reference.identity, "bundle", &reference.bundle.file)?;
    super::local::named_identity_matches(&reference.bundle, "reference.pgr", reference_member)?;
    super::local::named_identity_matches(mask_parent, mask_name, &mask.dir.file)?;
    super::local::named_identity_matches(&mask.dir, "domains.pgm", mask_member)?;
    super::local::named_identity_matches(profiles, profile_name, &profile.dir.file)
}

fn require_snv(profile: &RuntimeProfile, actual: &SnvBundleInspection) -> Result<(), AssetError> {
    if actual.bundle_id != profile.snv.bundle_id
        || actual.format != profile.snv.format
        || actual.member_bytes != profile.snv.member_bytes
        || actual.member_sha256 != profile.snv.member_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportIncompatible,
            "active SNV bundle does not match the runtime profile",
        ));
    }
    Ok(())
}

fn validate_staged_held(
    profile: &RuntimeProfile,
    model: &super::local::Dir,
    reference: &super::local::Dir,
    mask: &super::local::Dir,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    validate_model_bundle_held(model, profile, root)?;
    validate_reference_bundle_held(reference, profile, root)?;
    validate_mask_held(mask, profile, root)
}

fn validate_model_bundle_held(
    bundle: &super::local::Dir,
    profile: &RuntimeProfile,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    let files = open_installed_bundle(bundle, "model.onnx", profile.model.member_bytes, root)?;
    let admission =
        inspect_held_model_admission(&files.manifest_bytes, &files.notice_bytes, &files.member)
            .map_err(|_| profile_corrupt("staged model bundle is invalid"))?;
    if admission.bundle_id().as_str() != profile.model.bundle_id
        || admission.profile() != profile.model.profile
        || admission.representation().to_string() != profile.model.representation
    {
        return Err(profile_corrupt("staged model facts do not match profile"));
    }
    Ok(())
}

fn validate_reference_bundle_held(
    bundle: &super::local::Dir,
    profile: &RuntimeProfile,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    let files = open_installed_bundle(
        bundle,
        "reference.pgr",
        profile.reference.member_bytes,
        root,
    )?;
    if format!("sha256:{:x}", Sha256::digest(&files.manifest_bytes)) != profile.reference.bundle_id
    {
        return Err(profile_corrupt(
            "staged reference bundle identity does not match profile",
        ));
    }
    // SAFETY: staging authenticated this exact descriptor against the trusted
    // profile before this bounded admission.
    let admitted = unsafe {
        admit_installed_reference(
            &files.manifest_bytes,
            &files.notice_bytes,
            files
                .member
                .try_clone()
                .map_err(|_| profile_corrupt("staged reference bundle is invalid"))?,
        )
    }
    .map_err(|_| profile_corrupt("staged reference bundle is invalid"))?;
    let manifest = admitted.manifest();
    if manifest.profile != profile.reference.profile
        || manifest.reference_format != profile.reference.format
        || manifest.source.assembly != profile.reference.assembly
        || manifest.source.assembly_accession != profile.reference.assembly_accession
        || manifest.sequences.sequence_set_sha256 != profile.reference.sequence_set_sha256
    {
        return Err(profile_corrupt(
            "staged reference facts do not match profile",
        ));
    }
    Ok(())
}

fn validate_mask_held(
    mask: &super::local::Dir,
    profile: &RuntimeProfile,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    require_runtime_names(mask, &["domains.pgm"])?;
    let file = open_runtime_file(mask, "domains.pgm", root, FILE_IMMUTABLE)?;
    require_held_file(
        &file,
        FILE_IMMUTABLE,
        profile.mask.member_bytes,
        Some(profile.mask.member_bytes),
    )?;
    let admitted = MaskDomainsOpen::admit_held(file)
        .map_err(|_| profile_corrupt("staged mask is structurally invalid"))?;
    if admitted.identity().bytes() != profile.mask.member_bytes
        || format!("sha256:{}", admitted.identity().sha256()) != profile.mask.member_sha256
    {
        return Err(profile_corrupt("staged mask facts do not match profile"));
    }
    Ok(())
}

fn copy_bundle(
    source: &Path,
    destination: &super::local::Dir,
    root: &super::local::Root,
    payload: &str,
    payload_size: u64,
    payload_sha: &str,
) -> Result<(), AssetError> {
    let source = super::local::open_held_directory(
        source,
        AssetErrorKind::InputIo,
        AssetErrorKind::StagingInvalid,
    )
    .map_err(|error| {
        if error.kind() == AssetErrorKind::StagingInvalid {
            AssetError::new(AssetErrorKind::StagingInvalid, "source directory is unsafe")
        } else {
            input("inspect source directory")
        }
    })?;
    exact_source_names(&source, &["NOTICE", "manifest.json", payload])?;
    copy_member(
        &source,
        "manifest.json",
        destination,
        "manifest.json",
        root,
        MAX_JSON,
        "",
    )?;
    replace_source_directory_for_test();
    copy_member(
        &source,
        "NOTICE",
        destination,
        "NOTICE",
        root,
        MAX_NOTICE,
        "",
    )?;
    copy_member(
        &source,
        payload,
        destination,
        payload,
        root,
        payload_size,
        payload_sha,
    )?;
    destination
        .file
        .sync_all()
        .map_err(|_| output("sync staged bundle"))
}

fn exact_source_names(source: &super::local::Dir, expected: &[&str]) -> Result<(), AssetError> {
    let names = super::local::read_names_bounded(source, expected.len())
        .map_err(|_| {
            AssetError::new(
                AssetErrorKind::StagingInvalid,
                "source member set is unsafe",
            )
        })?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().map(|name| (*name).to_owned()).collect();
    if names != expected {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member set is unsafe",
        ));
    }
    Ok(())
}

fn copy_member(
    source: &super::local::Dir,
    source_name: &str,
    destination: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    maximum_or_exact: u64,
    expected_sha: &str,
) -> Result<(), AssetError> {
    let (input_file, held) = super::local::open_held_regular(
        source,
        source_name,
        AssetErrorKind::InputIo,
        AssetErrorKind::StagingInvalid,
    )
    .map_err(|error| {
        if error.kind() == AssetErrorKind::StagingInvalid {
            AssetError::new(AssetErrorKind::StagingInvalid, "source member is unsafe")
        } else {
            input("open source member")
        }
    })?;
    mutate_source_for_test();
    copy_open_member(
        input_file,
        held,
        Some((source, source_name)),
        destination,
        destination_name,
        root,
        maximum_or_exact,
        expected_sha,
    )
}

fn copy_path_member(
    source: &Path,
    destination: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    maximum_or_exact: u64,
    expected_sha: &str,
) -> Result<(), AssetError> {
    let input_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(source)
        .map_err(|_| input("open source member"))?;
    let held = input_file
        .metadata()
        .map_err(|_| input("inspect source member"))?;
    copy_open_member(
        input_file,
        held,
        None,
        destination,
        destination_name,
        root,
        maximum_or_exact,
        expected_sha,
    )
}

#[allow(clippy::too_many_arguments)]
fn copy_open_member(
    mut input_file: File,
    held: fs::Metadata,
    source_name: Option<(&super::local::Dir, &str)>,
    destination: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    maximum_or_exact: u64,
    expected_sha: &str,
) -> Result<(), AssetError> {
    if !held.file_type().is_file()
        || held.nlink() != 1
        || (!expected_sha.is_empty() && held.len() != maximum_or_exact)
        || (expected_sha.is_empty() && held.len() > maximum_or_exact)
    {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member is unsafe",
        ));
    }
    let mut output_file =
        super::local::create_owned_file(destination, destination_name, FILE_PRIVATE, root)
            .map_err(|_| output("create staged member"))?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let cap = maximum_or_exact;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = input_file
            .read(&mut buffer)
            .map_err(|_| input("read source member"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| profile_corrupt("source member is too large"))?;
        if total > cap {
            return Err(profile_corrupt("source member exceeds its size bound"));
        }
        output_file
            .write_all(&buffer[..count])
            .map_err(|_| output("write staged member"))?;
        hasher.update(&buffer[..count]);
    }
    let after_held = input_file
        .metadata()
        .map_err(|_| input("inspect copied source member"))?;
    if !same_source_state(&after_held, &held) || total != held.len() {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member changed during copy",
        ));
    }
    if let Some((source, name)) = source_name {
        let (_, named) = super::local::open_held_regular(
            source,
            name,
            AssetErrorKind::InputIo,
            AssetErrorKind::StagingInvalid,
        )
        .map_err(|_| {
            AssetError::new(
                AssetErrorKind::StagingInvalid,
                "source member changed during copy",
            )
        })?;
        if !same_source_state(&named, &held) {
            return Err(AssetError::new(
                AssetErrorKind::StagingInvalid,
                "source member changed during copy",
            ));
        }
    }
    if !expected_sha.is_empty() && format!("sha256:{:x}", hasher.finalize()) != expected_sha {
        return Err(profile_corrupt(
            "source member identity does not match profile",
        ));
    }
    output_file
        .flush()
        .map_err(|_| output("flush staged member"))?;
    super::local::set_mode(&output_file, FILE_IMMUTABLE)?;
    output_file
        .sync_all()
        .map_err(|_| output("sync staged member"))?;
    Ok(())
}

fn same_source_state(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

struct PublishedBundle {
    identity: super::local::Dir,
    bundle: super::local::Dir,
}

struct PublishedDirectory {
    dir: super::local::Dir,
}

fn publish_bundle(
    source_parent: &super::local::Dir,
    source_name: &str,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    payload: &str,
    expected_payload: u64,
) -> Result<PublishedBundle, AssetError> {
    let source = super::local::open_owned_dir(source_parent, source_name, root, DIR_PRIVATE)?;
    let wrapper_name = format!("{source_name}.wrapper");
    let wrapper = super::local::create_owned_dir(source_parent, &wrapper_name, DIR_PRIVATE, root)?;
    super::local::rename_owned_replace(source_parent, source_name, &wrapper, "bundle")?;
    super::local::set_mode(&source.file, DIR_IMMUTABLE)?;
    source
        .file
        .sync_all()
        .map_err(|_| output("sync runtime bundle mode"))?;
    wrapper
        .file
        .sync_all()
        .map_err(|_| output("sync runtime wrapper"))?;
    let published = publish_directory(
        source_parent,
        &wrapper_name,
        destination_parent,
        destination_name,
        root,
        |dir| validate_component_shape_held(dir, payload, expected_payload, true, root),
    )?;
    let bundle = super::local::open_owned_dir(&published.dir, "bundle", root, DIR_IMMUTABLE)?;
    Ok(PublishedBundle {
        identity: published.dir,
        bundle,
    })
}

fn publish_mask(
    source_parent: &super::local::Dir,
    source_name: &str,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    expected: u64,
) -> Result<PublishedDirectory, AssetError> {
    publish_directory(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        root,
        |dir| validate_component_shape_held(dir, "domains.pgm", expected, false, root),
    )
}

fn publish_directory(
    source_parent: &super::local::Dir,
    source_name: &str,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    validate_existing: impl FnOnce(&super::local::Dir) -> Result<(), AssetError>,
) -> Result<PublishedDirectory, AssetError> {
    if let Some(existing) =
        super::local::open_owned_dir_optional(destination_parent, destination_name, root)?
    {
        validate_existing(&existing)
            .map_err(|_| conflict("immutable runtime component conflicts"))?;
        remove_stage(source_parent, source_name)?;
        return Ok(PublishedDirectory { dir: existing });
    }
    super::local::rename_owned_noreplace(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
    )?;
    let published =
        super::local::open_owned_dir(destination_parent, destination_name, root, DIR_PRIVATE)?;
    super::local::set_mode(&published.file, DIR_IMMUTABLE)?;
    published
        .file
        .sync_all()
        .map_err(|_| output("sync runtime object mode"))?;
    destination_parent
        .file
        .sync_all()
        .map_err(|_| output("sync runtime directory"))?;
    Ok(PublishedDirectory { dir: published })
}

fn validate_component_shape_held(
    identity: &super::local::Dir,
    payload: &str,
    expected_size: u64,
    bundle: bool,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    if bundle {
        require_runtime_names(identity, &["bundle"])?;
        let bundle = open_runtime_dir(identity, "bundle", root, DIR_IMMUTABLE)?;
        open_installed_bundle(&bundle, payload, expected_size, root).map(|_| ())
    } else {
        require_runtime_names(identity, &[payload])?;
        let member = open_runtime_file(identity, payload, root, FILE_IMMUTABLE)?;
        require_held_file(&member, FILE_IMMUTABLE, expected_size, Some(expected_size))
    }
}

fn authenticate_member(
    parent: &super::local::Dir,
    name: &str,
    root: &super::local::Root,
    expected_size: u64,
    expected_sha: &str,
) -> Result<File, AssetError> {
    let mut file = super::local::open_owned_file(
        parent,
        name,
        FILE_IMMUTABLE,
        root,
        AssetErrorKind::InstallConflict,
    )
    .map_err(|_| conflict("immutable runtime component conflicts"))?;
    let metadata = file
        .metadata()
        .map_err(|_| conflict("immutable runtime component conflicts"))?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 || metadata.len() != expected_size {
        return Err(conflict("immutable runtime component conflicts"));
    }
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| conflict("immutable runtime component conflicts"))?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or_else(|| conflict("immutable runtime component conflicts"))?;
        if observed > expected_size {
            return Err(conflict("immutable runtime component conflicts"));
        }
        hasher.update(&buffer[..count]);
    }
    if observed != expected_size || format!("sha256:{:x}", hasher.finalize()) != expected_sha {
        return Err(conflict("immutable runtime component conflicts"));
    }
    Ok(file)
}

fn validate_profile_directory_held(
    directory: &super::local::Dir,
    profile: &RuntimeProfile,
    profile_id: &str,
    root: &super::local::Root,
) -> Result<RuntimeReceipt, AssetError> {
    require_runtime_names(directory, &["profile.json", "receipt.json"])?;
    let profile_file = open_runtime_file(directory, "profile.json", root, FILE_IMMUTABLE)?;
    let receipt_file = open_runtime_file(directory, "receipt.json", root, FILE_IMMUTABLE)?;
    require_held_file(&profile_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    require_held_file(&receipt_file, FILE_IMMUTABLE, MAX_JSON, None)?;
    let profile_bytes = super::local::read_bounded_handle_ref(
        &profile_file,
        MAX_JSON,
        AssetErrorKind::BundleInvalid,
    )?;
    if runtime_profile_id(&profile_bytes)
        .map_err(|_| profile_corrupt("installed profile is invalid"))?
        .as_str()
        != profile_id
        || parse_runtime_profile(&profile_bytes)
            .map_err(|_| profile_corrupt("installed profile is invalid"))?
            != *profile
    {
        return Err(profile_corrupt("installed profile identity conflicts"));
    }
    let receipt_bytes = super::local::read_bounded_handle_ref(
        &receipt_file,
        MAX_JSON,
        AssetErrorKind::BundleInvalid,
    )?;
    let installed_receipt: RuntimeReceipt = parse_canonical_runtime_bytes(&receipt_bytes)?;
    if installed_receipt != receipt(profile, profile_id) {
        return Err(profile_corrupt("installed runtime receipt is invalid"));
    }
    Ok(installed_receipt)
}

fn receipt(profile: &RuntimeProfile, profile_id: &str) -> RuntimeReceipt {
    RuntimeReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        profile_id: profile_id.to_owned(),
        snv_bundle_id: profile.snv.bundle_id.clone(),
        model: InstalledComponent {
            path: format!(
                "components/model/{}/bundle",
                suffix_unchecked(&profile.model.bundle_id)
            ),
            size: profile.model.member_bytes,
            sha256: profile.model.member_sha256.clone(),
        },
        reference: InstalledComponent {
            path: format!(
                "components/reference/{}/bundle",
                suffix_unchecked(&profile.reference.bundle_id)
            ),
            size: profile.reference.member_bytes,
            sha256: profile.reference.member_sha256.clone(),
        },
        mask: InstalledComponent {
            path: format!(
                "components/mask/{}/domains.pgm",
                suffix_unchecked(&profile.mask.member_sha256)
            ),
            size: profile.mask.member_bytes,
            sha256: profile.mask.member_sha256.clone(),
        },
    }
}

fn outcome(
    status: &'static str,
    profile: &RuntimeProfile,
    profile_id: String,
) -> RuntimeInstallOutcome {
    RuntimeInstallOutcome {
        status,
        profile_id,
        snv_bundle_id: profile.snv.bundle_id.clone(),
        model_bundle_id: profile.model.bundle_id.clone(),
        reference_bundle_id: profile.reference.bundle_id.clone(),
        mask_sha256: profile.mask.member_sha256.clone(),
    }
}

fn reconcile_staging(
    staging: &super::local::Dir,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    let stages = super::local::read_names_bounded(staging, 128)?;
    for name in &stages {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        {
            return Err(profile_corrupt("runtime staging entry is unsafe"));
        }
        super::local::open_owned_dir(staging, name, root, DIR_PRIVATE)
            .map_err(|_| profile_corrupt("runtime staging entry is unsafe"))?;
    }
    for name in stages {
        remove_stage(staging, &name)?;
    }
    Ok(())
}

fn reconcile_staged_active(
    runtime: &super::local::Dir,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    let Some(file) = super::local::open_owned_file_optional(
        runtime,
        ".active.new",
        FILE_PRIVATE,
        root,
        AssetErrorKind::BundleInvalid,
    )?
    else {
        return Ok(());
    };
    require_held_file(&file, FILE_PRIVATE, MAX_JSON, None)?;
    super::local::remove_owned_file(runtime, ".active.new")?;
    runtime
        .file
        .sync_all()
        .map_err(|_| output("sync runtime directory"))
}

fn read_small_source(path: &Path, maximum: u64) -> Result<Vec<u8>, AssetError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| input("open runtime profile"))?;
    let before = file
        .metadata()
        .map_err(|_| input("inspect runtime profile"))?;
    if !before.file_type().is_file() || before.nlink() != 1 || before.len() > maximum {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "runtime profile input is unsafe",
        ));
    }
    replace_profile_path_for_test();
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| input("read runtime profile"))?;
    let after = file
        .metadata()
        .map_err(|_| input("reinspect runtime profile"))?;
    if bytes.len() as u64 != before.len() || !same_source_state(&before, &after) {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "runtime profile input changed while reading",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), AssetError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_PRIVATE)
        .open(path)
        .map_err(|_| output("create runtime metadata"))?;
    file.write_all(bytes)
        .map_err(|_| output("write runtime metadata"))?;
    super::local::set_mode(&file, mode)?;
    file.sync_all().map_err(|_| output("sync runtime metadata"))
}

fn write_new_held(
    parent: &super::local::Dir,
    name: &str,
    bytes: &[u8],
    mode: u32,
    root: &super::local::Root,
) -> Result<(), AssetError> {
    let mut file = super::local::create_owned_file(parent, name, FILE_PRIVATE, root)?;
    file.write_all(bytes)
        .map_err(|_| output("write runtime metadata"))?;
    super::local::set_mode(&file, mode)?;
    file.sync_all().map_err(|_| output("sync runtime metadata"))
}

fn remove_stage(parent: &super::local::Dir, name: &str) -> Result<(), AssetError> {
    super::local::remove_owned_tree_bounded(
        parent
            .file
            .try_clone()
            .map_err(|_| output("clone runtime staging parent"))?,
        name,
        8,
        64,
    )
}

fn suffix(identity: &str) -> Result<&str, AssetError> {
    if !valid_identity(identity) {
        return Err(profile_corrupt("runtime identity is invalid"));
    }
    Ok(&identity[7..])
}

fn suffix_unchecked(identity: &str) -> &str {
    &identity[7..]
}

fn valid_identity(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and no failure mode.
    unsafe { libc::geteuid() }
}

fn profile_parse_error(error: impl std::fmt::Display) -> AssetError {
    AssetError::new(AssetErrorKind::TransportIncompatible, error.to_string())
}

fn profile_corrupt(message: &'static str) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, message)
}

fn runtime_missing() -> AssetError {
    AssetError::new(
        AssetErrorKind::AssetsMissing,
        "installed runtime profile is missing",
    )
}

fn profile_unsafe_runtime() -> AssetError {
    AssetError::new(
        AssetErrorKind::StagingInvalid,
        "installed runtime state is unsafe",
    )
}

fn profile_corrupt_runtime() -> AssetError {
    AssetError::new(
        AssetErrorKind::BundleInvalid,
        "installed runtime profile is invalid",
    )
}

fn profile_incompatible_runtime() -> AssetError {
    AssetError::new(
        AssetErrorKind::TransportIncompatible,
        "installed runtime profile is incompatible",
    )
}

fn conflict(message: &'static str) -> AssetError {
    AssetError::new(AssetErrorKind::InstallConflict, message)
}

fn input(message: &'static str) -> AssetError {
    AssetError::new(AssetErrorKind::InputIo, message)
}

fn output(message: &'static str) -> AssetError {
    AssetError::new(AssetErrorKind::OutputIo, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspect_snv_bundle;
    use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceProvider};
    use pangopup_index::mask::{MaskProvider, MaskQueryBuffer};
    use pangopup_index::reference_admission::inspect_reference_admission;
    use pangopup_model::{MIN_CONTEXT_LENGTH, ModelContext, Strand, StrandPair};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tempfile::TempDir;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures")
            .join(name)
    }

    fn digest(path: &Path) -> String {
        let bytes = fs::read(path).expect("read fixture");
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    fn copy_fixture_bundle(source: &Path, destination: &Path) {
        fs::create_dir(destination).expect("bundle directory");
        for name in ["NOTICE", "manifest.json", "scores.pgi"] {
            fs::copy(source.join(name), destination.join(name)).expect("copy fixture");
            fs::set_permissions(
                destination.join(name),
                fs::Permissions::from_mode(FILE_IMMUTABLE),
            )
            .expect("member mode");
        }
        fs::set_permissions(destination, fs::Permissions::from_mode(DIR_IMMUTABLE))
            .expect("bundle mode");
    }

    fn install_mini_snv(root: &Path) -> SnvBundleInspection {
        fs::create_dir(root).expect("root");
        fs::set_permissions(root, fs::Permissions::from_mode(DIR_PRIVATE)).expect("root mode");
        let source = fixture("snv-regression/bundle");
        let inspection = inspect_snv_bundle(&source).expect("SNV fixture");
        let bundles = root.join("bundles");
        fs::create_dir(&bundles).expect("bundles");
        fs::set_permissions(&bundles, fs::Permissions::from_mode(DIR_PRIVATE))
            .expect("bundles mode");
        let wrapper = bundles.join(suffix(&inspection.bundle_id).expect("identity"));
        fs::create_dir(&wrapper).expect("wrapper");
        let bundle = wrapper.join("bundle");
        copy_fixture_bundle(&source, &bundle);
        let receipt = SnvInstallReceipt {
            schema: "pangopup.install-receipt.v1".to_owned(),
            bundle_id: inspection.bundle_id.clone(),
            transport_id: format!("sha256:{}", "1".repeat(64)),
            members: ["NOTICE", "manifest.json", "scores.pgi"]
                .into_iter()
                .map(|name| InstalledComponent {
                    path: format!("bundle/{name}"),
                    size: fs::metadata(source.join(name)).expect("metadata").len(),
                    sha256: digest(&source.join(name)),
                })
                .collect(),
        };
        write_new(
            &wrapper.join("receipt.json"),
            &serde_jcs::to_vec(&receipt).expect("receipt"),
            FILE_IMMUTABLE,
        )
        .expect("receipt");
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(DIR_IMMUTABLE))
            .expect("wrapper mode");
        let active = serde_json::json!({
            "bundle_id": inspection.bundle_id,
            "schema": "pangopup.active-profile.v1"
        });
        write_new(
            &root.join("active.json"),
            &serde_jcs::to_vec(&active).expect("active"),
            FILE_PRIVATE,
        )
        .expect("active state");
        inspection
    }

    fn miniature_profile(snv: &SnvBundleInspection) -> RuntimeProfile {
        let model_path = fixture("pangolin-model-kernel-mini/bundle");
        let model_manifest = fs::read(model_path.join("manifest.json")).expect("model manifest");
        let model_json: serde_json::Value =
            serde_json::from_slice(&model_manifest).expect("model JSON");
        let model_member = model_json["members"]
            .as_array()
            .expect("members")
            .iter()
            .find(|member| member["filename"] == "model.onnx")
            .expect("model member");
        let reference_path = fixture("reference-route-test/bundle");
        let reference = inspect_reference_admission(&reference_path).expect("reference fixture");
        let reference_member = reference_path.join("reference.pgr");
        let mask = fixture("route-mask/domains.pgm");
        RuntimeProfile {
            schema: crate::RUNTIME_PROFILE_SCHEMA.to_owned(),
            snv: crate::SnvProfile {
                bundle_id: snv.bundle_id.clone(),
                format: snv.format.clone(),
                member_bytes: snv.member_bytes,
                member_sha256: snv.member_sha256.clone(),
            },
            model: crate::ModelProfile {
                bundle_id: format!("sha256:{:x}", Sha256::digest(&model_manifest)),
                profile: model_json["profile"].as_str().expect("profile").to_owned(),
                representation: "singleton".to_owned(),
                member_bytes: model_member["bytes"].as_u64().expect("model bytes"),
                member_sha256: model_member["sha256"]
                    .as_str()
                    .expect("model sha")
                    .to_owned(),
            },
            reference: crate::ReferenceProfile {
                bundle_id: reference.bundle_id().to_owned(),
                profile: reference.profile().to_owned(),
                format: reference.format().to_owned(),
                assembly: reference.assembly().to_owned(),
                assembly_accession: reference.assembly_accession().to_owned(),
                sequence_set_sha256: reference.sequence_set_sha256().to_owned(),
                member_bytes: fs::metadata(&reference_member)
                    .expect("reference member")
                    .len(),
                member_sha256: digest(&reference_member),
            },
            mask: crate::MaskProfile {
                format: "pangopup.gencode-v38-domains.v1".to_owned(),
                member_bytes: fs::metadata(&mask).expect("mask").len(),
                member_sha256: digest(&mask),
            },
            scoring: crate::ScoringProfile {
                assembly: "GRCh38".to_owned(),
                semantics: "pangopup-variant-score-v1".to_owned(),
                distance: 50,
                masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
                cpu_policy: "sequential:1/1".to_owned(),
            },
        }
    }

    fn miniature_status(
        root: &Path,
        profile: &RuntimeProfile,
    ) -> Result<RuntimeLocalStatus, AssetError> {
        let opened = crate::local::open_root(root, false)?
            .ok_or_else(|| AssetError::new(AssetErrorKind::AssetsMissing, "missing test root"))?;
        runtime_local_status_with(&opened, root, profile, false)
    }

    fn install_mini_runtime(root: &Path) -> (SnvBundleInspection, RuntimeProfile) {
        let snv = install_mini_snv(root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            root,
        )
        .expect("install miniature runtime");
        (snv, profile)
    }

    #[test]
    fn fixture_combined_status_is_ready_under_the_shared_observation_guard() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (_, profile) = install_mini_runtime(&root);

        let crate::CombinedStatusResult::Valid(status) =
            crate::provisioning::combined_local_status_with_runtime_profile(&root, &profile)
                .expect("combined fixture status")
        else {
            panic!("fixture pair was not a valid combined status")
        };
        assert_eq!(status.status, "ready");
        assert!(!status.installing);
        assert!(matches!(status.snv, crate::SnvStatusObservation::Ready(_)));
        assert!(matches!(
            status.runtime,
            crate::RuntimeStatusObservation::Ready(_)
        ));
    }

    #[test]
    fn runtime_status_reads_only_bounded_metadata_and_payload_sizes() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (snv, profile) = install_mini_runtime(&root);
        let profile_id = runtime_profile_id(
            &canonical_runtime_profile_bytes(&profile).expect("canonical profile"),
        )
        .expect("profile id")
        .to_string();
        let profile_dir = root
            .join("runtime/profiles")
            .join(suffix(&profile_id).expect("profile suffix"));
        let expected_read_bytes = [
            root.join("runtime/active.json"),
            profile_dir.join("profile.json"),
            profile_dir.join("receipt.json"),
        ]
        .into_iter()
        .map(|path| fs::metadata(path).expect("bounded metadata").len())
        .sum::<u64>();

        let mask = root
            .join("runtime/components/mask")
            .join(suffix(&profile.mask.member_sha256).expect("mask suffix"))
            .join("domains.pgm");
        fs::set_permissions(&mask, fs::Permissions::from_mode(0o600)).expect("make mask writable");
        let mut bytes = fs::read(&mask).expect("read mask fixture");
        let last = bytes.last_mut().expect("nonempty mask fixture");
        *last ^= 0xff;
        fs::write(&mask, bytes).expect("change an unbounded payload byte");
        fs::set_permissions(&mask, fs::Permissions::from_mode(FILE_IMMUTABLE))
            .expect("restore mask mode");

        STATUS_READ_BYTES.set(0);
        assert!(matches!(
            miniature_status(&root, &profile).expect("metadata-only status"),
            RuntimeLocalStatus::Ready { .. }
        ));
        assert_eq!(STATUS_READ_BYTES.get(), expected_read_bytes);
        assert_eq!(
            open_installed_runtime_profile_with(&root, Some(&snv.bundle_id), &profile)
                .expect_err("full admission detects payload corruption")
                .kind(),
            AssetErrorKind::TransportIncompatible
        );
    }

    #[test]
    fn receipt_is_path_free_and_deterministic() {
        let profile = crate::production_runtime_profile();
        let bytes = crate::canonical_runtime_profile_bytes(&profile).expect("profile");
        let id = crate::runtime_profile_id(&bytes).expect("id").to_string();
        let receipt = receipt(&profile, &id);
        let encoded = serde_jcs::to_vec(&receipt).expect("receipt");
        let text = String::from_utf8(encoded).expect("UTF-8");
        assert!(text.contains("\"schema\":\"pangopup.runtime-install-receipt.v1\""));
        assert!(!text.contains("/home/"));
        assert!(!text.contains("http"));
        assert!(!text.contains("timestamp"));
    }

    #[test]
    fn identities_require_prefixed_lowercase_sha256() {
        assert!(valid_identity(&format!("sha256:{}", "a".repeat(64))));
        assert!(!valid_identity(&format!("sha256:{}", "A".repeat(64))));
        assert!(!valid_identity(&"a".repeat(64)));
    }

    #[test]
    fn miniature_profile_installs_atomically_and_reuses_without_copy() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let profile_bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let sources = InstallSources {
            model: &model,
            reference: &reference,
            mask: &mask,
        };
        let installed =
            install_with_profile(&profile_bytes, &profile, sources, &root).expect("install");
        assert_eq!(installed.status, "installed");
        let ready = miniature_status(&root, &profile).expect("status");
        let RuntimeLocalStatus::Ready {
            profile_id,
            model_path,
            reference_path,
            mask_path,
            installing,
            ..
        } = ready
        else {
            panic!("not ready");
        };
        assert_eq!(profile_id, installed.profile_id);
        assert!(!installing);
        assert!(model_path.starts_with("runtime/components/model"));
        assert!(reference_path.starts_with("runtime/components/reference"));
        assert!(mask_path.starts_with("runtime/components/mask"));
        assert_eq!(
            fs::metadata(root.join(&model_path))
                .expect("model wrapper")
                .mode()
                & 0o777,
            DIR_IMMUTABLE
        );
        assert_eq!(
            fs::metadata(root.join(&model_path).join("model.onnx"))
                .expect("model member")
                .mode()
                & 0o777,
            FILE_IMMUTABLE
        );
        assert_eq!(
            fs::metadata(root.join("runtime/active.json"))
                .expect("active")
                .mode()
                & 0o777,
            FILE_PRIVATE
        );

        let installed_model = root.join(model_path).join("model.onnx");
        let before = fs::metadata(&installed_model).expect("installed model");
        let reused =
            install_with_profile(&profile_bytes, &profile, sources, &root).expect("reinstall");
        let after = fs::metadata(installed_model).expect("reused model");
        assert_eq!(reused.status, "reused");
        assert_eq!(before.ino(), after.ino());
        assert_eq!(before.mtime(), after.mtime());
    }

    #[test]
    fn installed_runtime_admits_real_held_miniature_capabilities() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (snv, profile) = install_mini_runtime(&root);
        let installed = open_installed_runtime_profile_with(&root, Some(&snv.bundle_id), &profile)
            .expect("installed admission");
        let (_, model, reference, mask) = installed.into_parts();
        assert_eq!(
            model.admission().bundle_id().as_str(),
            profile.model.bundle_id
        );
        assert_eq!(reference.manifest().profile, profile.reference.profile);
        assert_eq!(mask.identity().bytes(), profile.mask.member_bytes);
        mask.open().expect("open held mask");
        model.open().expect("open held model");
    }

    #[test]
    fn model_only_runtime_admission_does_not_require_the_snv_installation() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (_snv, profile) = install_mini_runtime(&root);
        fs::remove_file(root.join("active.json")).expect("remove active SNV profile");
        fs::rename(
            root.join("bundles"),
            temp.path().join("removed-snv-bundles"),
        )
        .expect("make SNV bundles unavailable");

        let installed = open_installed_runtime_profile_with(&root, None, &profile)
            .expect("model-side runtime admission");
        let (_, model, reference, mask) = installed.into_parts();
        model.open().expect("model remains available");
        assert_eq!(reference.manifest().profile, profile.reference.profile);
        mask.open().expect("mask remains available");
    }

    #[test]
    fn installed_runtime_is_bound_to_snv_identity_and_detects_pre_return_replacement() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (snv, profile) = install_mini_runtime(&root);
        let incompatible = open_installed_runtime_profile_with(
            &root,
            Some(&format!("sha256:{}", "f".repeat(64))),
            &profile,
        )
        .expect_err("SNV identity mismatch");
        assert_eq!(incompatible.kind(), AssetErrorKind::TransportIncompatible);
        assert_eq!(
            incompatible.to_string(),
            "installed runtime profile is incompatible"
        );

        REPLACE_RUNTIME_BEFORE_RETURN.set(true);
        let replaced = open_installed_runtime_profile_with(&root, Some(&snv.bundle_id), &profile)
            .expect_err("pre-return pathname replacement");
        assert_eq!(replaced.kind(), AssetErrorKind::BundleInvalid);
        assert_eq!(replaced.to_string(), "installed runtime profile is invalid");
    }

    #[test]
    fn installed_runtime_missing_error_is_compact_and_exact() {
        let temp = TempDir::new().expect("temp");
        let error = open_installed_runtime_profile(
            &temp.path().join("missing"),
            &crate::production_runtime_profile().snv.bundle_id,
        )
        .expect_err("missing profile");
        assert_eq!(error.kind(), AssetErrorKind::AssetsMissing);
        assert_eq!(error.to_string(), "installed runtime profile is missing");
    }

    #[test]
    fn installed_runtime_malformed_and_unsafe_states_have_exact_classes() {
        let malformed = TempDir::new().expect("temp");
        let malformed_root = malformed.path().join("data");
        let (snv, profile) = install_mini_runtime(&malformed_root);
        fs::write(
            malformed_root.join("runtime/active.json"),
            b"{\"schema\":\"wrong\"}",
        )
        .expect("malformed active");
        let error =
            open_installed_runtime_profile_with(&malformed_root, Some(&snv.bundle_id), &profile)
                .expect_err("malformed profile");
        assert_eq!(error.kind(), AssetErrorKind::BundleInvalid);
        assert_eq!(error.to_string(), "installed runtime profile is invalid");

        let bad_mode = TempDir::new().expect("temp");
        let bad_mode_root = bad_mode.path().join("data");
        let (snv, profile) = install_mini_runtime(&bad_mode_root);
        fs::set_permissions(
            bad_mode_root.join("runtime/active.json"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("unsafe mode");
        let error =
            open_installed_runtime_profile_with(&bad_mode_root, Some(&snv.bundle_id), &profile)
                .expect_err("unsafe mode");
        assert_eq!(error.kind(), AssetErrorKind::StagingInvalid);
        assert_eq!(error.to_string(), "installed runtime state is unsafe");

        let bad_link = TempDir::new().expect("temp");
        let bad_link_root = bad_link.path().join("data");
        let (snv, profile) = install_mini_runtime(&bad_link_root);
        fs::hard_link(
            bad_link_root.join("runtime/active.json"),
            bad_link.path().join("active-link.json"),
        )
        .expect("unsafe link");
        let error =
            open_installed_runtime_profile_with(&bad_link_root, Some(&snv.bundle_id), &profile)
                .expect_err("unsafe link");
        assert_eq!(error.kind(), AssetErrorKind::StagingInvalid);
        assert_eq!(error.to_string(), "installed runtime state is unsafe");

        let bad_entry = TempDir::new().expect("temp");
        let bad_entry_root = bad_entry.path().join("data");
        let (snv, profile) = install_mini_runtime(&bad_entry_root);
        let profile_id =
            runtime_profile_id(&canonical_runtime_profile_bytes(&profile).expect("profile bytes"))
                .expect("profile id")
                .to_string();
        let profile_dir = bad_entry_root
            .join("runtime/profiles")
            .join(suffix(&profile_id).expect("suffix"));
        fs::set_permissions(&profile_dir, fs::Permissions::from_mode(DIR_PRIVATE))
            .expect("writable profile dir");
        fs::write(profile_dir.join("unexpected"), b"x").expect("unsafe entry");
        fs::set_permissions(&profile_dir, fs::Permissions::from_mode(DIR_IMMUTABLE))
            .expect("restore profile dir");
        let error =
            open_installed_runtime_profile_with(&bad_entry_root, Some(&snv.bundle_id), &profile)
                .expect_err("unsafe entry");
        assert_eq!(error.kind(), AssetErrorKind::StagingInvalid);
        assert_eq!(error.to_string(), "installed runtime state is unsafe");
    }

    #[test]
    fn admitted_runtime_capabilities_survive_all_pathname_replacements() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let (snv, profile) = install_mini_runtime(&root);
        let installed = open_installed_runtime_profile_with(&root, Some(&snv.bundle_id), &profile)
            .expect("installed admission");
        let (_, model, reference, mask) = installed.into_parts();

        let model_path = root
            .join("runtime/components/model")
            .join(suffix(&profile.model.bundle_id).expect("model suffix"))
            .join("bundle/model.onnx");
        let reference_path = root
            .join("runtime/components/reference")
            .join(suffix(&profile.reference.bundle_id).expect("reference suffix"))
            .join("bundle/reference.pgr");
        let mask_path = root
            .join("runtime/components/mask")
            .join(suffix(&profile.mask.member_sha256).expect("mask suffix"))
            .join("domains.pgm");
        for (ordinal, path) in [model_path, reference_path, mask_path]
            .into_iter()
            .enumerate()
        {
            let metadata = fs::metadata(&path).expect("member metadata");
            let parent = path.parent().expect("member parent");
            fs::set_permissions(parent, fs::Permissions::from_mode(DIR_PRIVATE))
                .expect("writable member parent");
            let held = temp.path().join(format!("held-{ordinal}"));
            fs::rename(&path, &held).expect("retain original inode");
            fs::write(&path, vec![0_u8; metadata.len() as usize]).expect("replace pathname");
            fs::set_permissions(&path, fs::Permissions::from_mode(FILE_IMMUTABLE))
                .expect("replacement mode");
            fs::set_permissions(parent, fs::Permissions::from_mode(DIR_IMMUTABLE))
                .expect("restore member parent");
        }

        let mut base = [0_u8; 1];
        reference
            .copy_window(
                Grch38Contig::autosome(1).expect("chr1"),
                GenomicPosition::new(5_051).expect("position"),
                &mut base,
            )
            .expect("query held reference");
        assert_eq!(base, [b'A']);

        let mask = mask.open().expect("open held mask");
        let mut genes = MaskQueryBuffer::default();
        mask.query(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(5_051).expect("position"),
            None,
            &mut genes,
        )
        .expect("query held mask");
        assert!(!genes.plus().is_empty());

        let mut model = model.open().expect("open held model");
        let context = ModelContext::new(vec![b'N'; MIN_CONTEXT_LENGTH]).expect("context");
        model
            .infer_variant(&[StrandPair {
                reference: &context,
                alternate: &context,
                strand: Strand::Plus,
            }])
            .expect("score held model");
    }

    #[test]
    fn runtime_status_distinguishes_missing_and_malformed() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        fs::create_dir(&root).expect("root");
        fs::set_permissions(&root, fs::Permissions::from_mode(DIR_PRIVATE)).expect("root mode");
        assert!(matches!(
            runtime_local_status(&root).expect("missing"),
            RuntimeLocalStatus::Missing { .. }
        ));
        let runtime = root.join("runtime");
        fs::create_dir(&runtime).expect("runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(DIR_PRIVATE))
            .expect("runtime mode");
        write_new(&runtime.join("active.json"), b"{}", FILE_PRIVATE).expect("bad active");
        assert_eq!(
            runtime_local_status(&root).expect_err("malformed").kind(),
            AssetErrorKind::BundleInvalid
        );
        fs::remove_file(runtime.join("active.json")).expect("remove malformed active");
        std::os::unix::fs::symlink("missing-target", runtime.join("active.json"))
            .expect("dangling active");
        assert_eq!(
            runtime_local_status(&root)
                .expect_err("dangling active")
                .kind(),
            AssetErrorKind::BundleInvalid
        );
    }

    #[test]
    fn transition_failures_never_expose_a_partial_profile_and_retry_cleanly() {
        for point in [
            TransitionFault::StagedObjectsDurable,
            TransitionFault::ComponentsPublished,
            TransitionFault::ProfilePublished,
            TransitionFault::BeforeActiveRename,
            TransitionFault::AfterActiveRename,
        ] {
            let temp = TempDir::new().expect("temp");
            let root = temp.path().join("data");
            let snv = install_mini_snv(&root);
            let profile = miniature_profile(&snv);
            let profile_bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
            let model = fixture("pangolin-model-kernel-mini/bundle");
            let reference = fixture("reference-route-test/bundle");
            let mask = fixture("route-mask/domains.pgm");
            let sources = InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            };
            TRANSITION_FAULT.set(Some(point));
            let error = install_with_profile(&profile_bytes, &profile, sources, &root)
                .expect_err("injected failure");
            assert_eq!(error.kind(), AssetErrorKind::OutputIo);
            let status = miniature_status(&root, &profile).expect("bounded status");
            if point == TransitionFault::AfterActiveRename {
                assert!(matches!(status, RuntimeLocalStatus::Ready { .. }));
            } else {
                assert!(matches!(status, RuntimeLocalStatus::Missing { .. }));
            }
            let retried =
                install_with_profile(&profile_bytes, &profile, sources, &root).expect("retry");
            assert!(matches!(retried.status, "installed" | "reused"));
            assert!(matches!(
                miniature_status(&root, &profile).expect("ready"),
                RuntimeLocalStatus::Ready { .. }
            ));
        }
    }

    #[test]
    fn failed_replacement_preserves_the_prior_active_profile() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let original = miniature_profile(&snv);
        let original_bytes = canonical_runtime_profile_bytes(&original).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let sources = InstallSources {
            model: &model,
            reference: &reference,
            mask: &mask,
        };
        let installed = install_with_profile(&original_bytes, &original, sources, &root)
            .expect("original install");
        let mut replacement = original.clone();
        replacement.scoring.cpu_policy = "sequential:2/1".to_owned();
        let replacement_bytes =
            canonical_runtime_profile_bytes(&replacement).expect("replacement profile");
        TRANSITION_FAULT.set(Some(TransitionFault::BeforeActiveRename));
        install_with_profile(&replacement_bytes, &replacement, sources, &root)
            .expect_err("injected replacement failure");
        let RuntimeLocalStatus::Ready { profile_id, .. } =
            miniature_status(&root, &original).expect("original remains ready")
        else {
            panic!("prior profile was not preserved");
        };
        assert_eq!(profile_id, installed.profile_id);
    }

    #[test]
    fn shared_lock_is_nonblocking_and_status_reports_installing() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let _lock = crate::local::acquire_shared_install_lock(&root).expect("hold lock");
        assert!(matches!(
            runtime_local_status(&root).expect("status"),
            RuntimeLocalStatus::Installing { .. }
        ));
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let error = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect_err("locked install");
        assert_eq!(error.kind(), AssetErrorKind::AssetLocked);
    }

    #[test]
    fn multiply_linked_source_fails_without_active_runtime_state() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let model = temp.path().join("model");
        fs::create_dir(&model).expect("model");
        for name in ["NOTICE", "manifest.json", "model.onnx"] {
            fs::copy(
                fixture("pangolin-model-kernel-mini/bundle").join(name),
                model.join(name),
            )
            .expect("copy model");
        }
        fs::hard_link(model.join("model.onnx"), temp.path().join("second-link"))
            .expect("hard link");
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let error = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect_err("hardlinked source");
        assert_eq!(error.kind(), AssetErrorKind::StagingInvalid);
        assert!(!root.join("runtime/active.json").exists());
    }

    #[test]
    fn same_identity_same_size_component_corruption_is_a_conflict() {
        for component in ["model", "reference", "mask"] {
            let temp = TempDir::new().expect("temp");
            let root = temp.path().join("data");
            let snv = install_mini_snv(&root);
            let profile = miniature_profile(&snv);
            let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
            let model = fixture("pangolin-model-kernel-mini/bundle");
            let reference = fixture("reference-route-test/bundle");
            let mask = fixture("route-mask/domains.pgm");
            let sources = InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            };
            TRANSITION_FAULT.set(Some(TransitionFault::ComponentsPublished));
            install_with_profile(&bytes, &profile, sources, &root)
                .expect_err("stop after components");
            let member = match component {
                "model" => root
                    .join("runtime/components/model")
                    .join(suffix(&profile.model.bundle_id).expect("model id"))
                    .join("bundle/model.onnx"),
                "reference" => root
                    .join("runtime/components/reference")
                    .join(suffix(&profile.reference.bundle_id).expect("reference id"))
                    .join("bundle/reference.pgr"),
                _ => root
                    .join("runtime/components/mask")
                    .join(suffix(&profile.mask.member_sha256).expect("mask id"))
                    .join("domains.pgm"),
            };
            fs::set_permissions(&member, fs::Permissions::from_mode(0o644))
                .expect("make test member writable");
            let file = OpenOptions::new()
                .write(true)
                .open(&member)
                .expect("open test member");
            rustix::io::pwrite(&file, b"X", 0).expect("same-size corruption");
            fs::set_permissions(&member, fs::Permissions::from_mode(FILE_IMMUTABLE))
                .expect("restore member mode");
            let error = install_with_profile(&bytes, &profile, sources, &root)
                .expect_err("collision must fail");
            assert_eq!(error.kind(), AssetErrorKind::InstallConflict, "{component}");
            assert!(!root.join("runtime/active.json").exists());
        }
    }

    #[test]
    fn replaced_intermediate_destination_fails_before_activation() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        REPLACE_DESTINATION.set(true);
        let error = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect_err("replaced intermediate destination");
        assert_eq!(error.kind(), AssetErrorKind::BundleInvalid);
        assert!(!root.join("runtime/active.json").exists());
        assert_eq!(
            fs::read_dir(root.join("runtime/components/model"))
                .expect("replacement model directory")
                .count(),
            0
        );
    }

    #[test]
    fn replaced_staging_directory_is_refused_at_use() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let sources = InstallSources {
            model: &fixture("pangolin-model-kernel-mini/bundle"),
            reference: &fixture("reference-route-test/bundle"),
            mask: &fixture("route-mask/domains.pgm"),
        };

        let replacement_root = root.clone();
        let replacement_sentinel = std::rc::Rc::new(std::cell::RefCell::new(None));
        let observed_sentinel = std::rc::Rc::clone(&replacement_sentinel);
        REPLACE_STAGING_DIRECTORY.with(|replacement| {
            *replacement.borrow_mut() = Some(Box::new(move || {
                let staging = replacement_root.join("runtime/.staging");
                let admitted = fs::read_dir(&staging)
                    .map_err(|_| output("inspect test staging directory"))?
                    .next()
                    .ok_or_else(|| output("find test staging directory"))?
                    .map_err(|_| output("inspect test staging entry"))?
                    .path();
                let name = admitted
                    .file_name()
                    .ok_or_else(|| output("name test staging directory"))?;
                let held = staging.join(format!("{}.held", name.to_string_lossy()));
                fs::rename(&admitted, &held)
                    .map_err(|_| output("move admitted test staging directory"))?;
                fs::create_dir(&admitted)
                    .map_err(|_| output("replace admitted test staging directory"))?;
                fs::set_permissions(&admitted, fs::Permissions::from_mode(DIR_PRIVATE))
                    .map_err(|_| output("set replacement staging mode"))?;
                let sentinel = admitted.join("foreign-sentinel");
                fs::write(&sentinel, b"foreign replacement")
                    .map_err(|_| output("write replacement staging sentinel"))?;
                *observed_sentinel.borrow_mut() = Some(sentinel);
                Ok(())
            }));
        });

        let result = install_with_profile(&bytes, &profile, sources, &root);

        result.expect_err("a replaced admitted staging directory must be refused");
        assert!(!root.join("runtime/active.json").exists());
        let sentinel = replacement_sentinel
            .borrow()
            .clone()
            .expect("replacement sentinel path");
        assert_eq!(
            fs::read(sentinel).expect("foreign replacement must survive"),
            b"foreign replacement"
        );
    }

    #[test]
    fn profile_collision_and_receipt_shape_fail_closed() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let sources = InstallSources {
            model: &model,
            reference: &reference,
            mask: &mask,
        };
        TRANSITION_FAULT.set(Some(TransitionFault::ProfilePublished));
        install_with_profile(&bytes, &profile, sources, &root).expect_err("stop after profile");
        let id = runtime_profile_id(&bytes).expect("id").to_string();
        let receipt = root
            .join("runtime/profiles")
            .join(suffix(&id).expect("profile id"))
            .join("receipt.json");
        fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644))
            .expect("make receipt writable");
        let file = OpenOptions::new()
            .write(true)
            .open(&receipt)
            .expect("open receipt");
        rustix::io::pwrite(&file, b"X", 0).expect("corrupt receipt");
        fs::set_permissions(&receipt, fs::Permissions::from_mode(FILE_IMMUTABLE))
            .expect("restore receipt mode");
        assert_eq!(
            install_with_profile(&bytes, &profile, sources, &root)
                .expect_err("profile collision")
                .kind(),
            AssetErrorKind::InstallConflict
        );
        assert!(!root.join("runtime/active.json").exists());
    }

    #[test]
    fn installed_receipt_mode_and_link_count_are_enforced() {
        for hardlink in [false, true] {
            let temp = TempDir::new().expect("temp");
            let root = temp.path().join("data");
            let snv = install_mini_snv(&root);
            let profile = miniature_profile(&snv);
            let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
            let model = fixture("pangolin-model-kernel-mini/bundle");
            let reference = fixture("reference-route-test/bundle");
            let mask = fixture("route-mask/domains.pgm");
            install_with_profile(
                &bytes,
                &profile,
                InstallSources {
                    model: &model,
                    reference: &reference,
                    mask: &mask,
                },
                &root,
            )
            .expect("install");
            let id = runtime_profile_id(&bytes).expect("id").to_string();
            let receipt = root
                .join("runtime/profiles")
                .join(suffix(&id).expect("id"))
                .join("receipt.json");
            if hardlink {
                fs::hard_link(&receipt, temp.path().join("receipt-link")).expect("hard link");
            } else {
                fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644))
                    .expect("wrong mode");
            }
            assert!(miniature_status(&root, &profile).is_err());
        }
    }

    #[test]
    fn source_replacement_truncation_symlink_and_extra_entry_fail_before_activation() {
        for case in ["replace", "truncate", "symlink", "extra"] {
            let temp = TempDir::new().expect("temp");
            let root = temp.path().join("data");
            let snv = install_mini_snv(&root);
            let profile = miniature_profile(&snv);
            let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
            let model = temp.path().join("model");
            fs::create_dir(&model).expect("model");
            for name in ["NOTICE", "manifest.json", "model.onnx"] {
                fs::copy(
                    fixture("pangolin-model-kernel-mini/bundle").join(name),
                    model.join(name),
                )
                .expect("copy model");
            }
            match case {
                "replace" => {
                    let source = model.join("manifest.json");
                    SOURCE_MUTATION.with(|mutation| {
                        *mutation.borrow_mut() = Some(Box::new(move || {
                            let held = source.with_extension("held-old");
                            fs::rename(&source, &held).expect("replace source path");
                            fs::copy(&held, &source).expect("replacement source");
                        }));
                    });
                }
                "truncate" => {
                    let source = model.join("manifest.json");
                    SOURCE_MUTATION.with(|mutation| {
                        *mutation.borrow_mut() = Some(Box::new(move || {
                            OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(source)
                                .expect("truncate source");
                        }));
                    });
                }
                "symlink" => {
                    fs::remove_file(model.join("model.onnx")).expect("remove model");
                    std::os::unix::fs::symlink(
                        fixture("pangolin-model-kernel-mini/bundle/model.onnx"),
                        model.join("model.onnx"),
                    )
                    .expect("symlink model");
                }
                _ => {
                    fs::write(model.join("extra"), b"x").expect("extra member");
                }
            }
            let reference = fixture("reference-route-test/bundle");
            let mask = fixture("route-mask/domains.pgm");
            install_with_profile(
                &bytes,
                &profile,
                InstallSources {
                    model: &model,
                    reference: &reference,
                    mask: &mask,
                },
                &root,
            )
            .expect_err("unsafe source");
            assert!(!root.join("runtime/active.json").exists(), "{case}");
        }
    }

    #[test]
    fn source_bundle_copy_keeps_one_admitted_parent_after_path_replacement() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = temp.path().join("model");
        fs::create_dir(&model).expect("model source");
        for name in ["NOTICE", "manifest.json", "model.onnx"] {
            fs::copy(
                fixture("pangolin-model-kernel-mini/bundle").join(name),
                model.join(name),
            )
            .expect("copy model member");
        }
        let held_model = temp.path().join("model-held");
        let replacement_model = model.clone();
        REPLACE_SOURCE_DIRECTORY.with(|replacement| {
            *replacement.borrow_mut() = Some(Box::new(move || {
                fs::rename(&replacement_model, &held_model).expect("move admitted source");
                fs::create_dir(&replacement_model).expect("replace source directory");
                fs::write(replacement_model.join("NOTICE"), b"decoy").expect("write decoy member");
            }));
        });
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");

        let installed = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect("copy from admitted source directory");

        assert_eq!(installed.status, "installed");
        assert_eq!(
            fs::read(model.join("NOTICE")).expect("read decoy source"),
            b"decoy"
        );
    }

    #[test]
    fn runtime_profile_read_keeps_the_admitted_file_after_path_replacement() {
        let temp = TempDir::new().expect("temp");
        let profile = temp.path().join("runtime-profile.json");
        let held_profile = temp.path().join("runtime-profile-held.json");
        let original = b"original profile";
        fs::write(&profile, original).expect("write profile");
        let replacement_profile = profile.clone();
        REPLACE_PROFILE_PATH.with(|replacement| {
            *replacement.borrow_mut() = Some(Box::new(move || {
                fs::rename(&replacement_profile, &held_profile).expect("move admitted profile");
                fs::write(&replacement_profile, b"decoy profile").expect("replace profile path");
            }));
        });

        assert_eq!(
            read_small_source(&profile, MAX_JSON).expect("read admitted profile"),
            original
        );
        assert_eq!(
            fs::read(&profile).expect("read decoy profile"),
            b"decoy profile"
        );
    }

    #[test]
    fn read_only_source_files_and_non_private_source_directories_install() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let model = temp.path().join("model");
        let reference = temp.path().join("reference");
        for (source, destination, members) in [
            (
                fixture("pangolin-model-kernel-mini/bundle"),
                model.as_path(),
                ["NOTICE", "manifest.json", "model.onnx"],
            ),
            (
                fixture("reference-route-test/bundle"),
                reference.as_path(),
                ["NOTICE", "manifest.json", "reference.pgr"],
            ),
        ] {
            fs::create_dir(destination).expect("source directory");
            for name in members {
                fs::copy(source.join(name), destination.join(name)).expect("copy source");
                fs::set_permissions(destination.join(name), fs::Permissions::from_mode(0o400))
                    .expect("read-only source");
            }
            fs::set_permissions(destination, fs::Permissions::from_mode(0o775))
                .expect("source directory mode");
        }
        let mask = temp.path().join("domains.pgm");
        fs::copy(fixture("route-mask/domains.pgm"), &mask).expect("copy mask");
        fs::set_permissions(&mask, fs::Permissions::from_mode(0o400)).expect("mask mode");
        let installed = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect("read-only install");
        assert_eq!(installed.status, "installed");
    }

    #[test]
    fn excessive_orphan_staging_entries_fail_without_partial_cleanup_or_activation() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("data");
        let snv = install_mini_snv(&root);
        let profile = miniature_profile(&snv);
        let bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
        let runtime = root.join("runtime");
        let staging = runtime.join(".staging");
        fs::create_dir(&runtime).expect("runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(DIR_PRIVATE))
            .expect("runtime mode");
        fs::create_dir(&staging).expect("staging");
        fs::set_permissions(&staging, fs::Permissions::from_mode(DIR_PRIVATE))
            .expect("staging mode");
        for index in 0..129 {
            let orphan = staging.join(format!("{index:032x}"));
            fs::create_dir(&orphan).expect("orphan");
            fs::set_permissions(&orphan, fs::Permissions::from_mode(DIR_PRIVATE))
                .expect("orphan mode");
        }
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let reference = fixture("reference-route-test/bundle");
        let mask = fixture("route-mask/domains.pgm");
        let error = install_with_profile(
            &bytes,
            &profile,
            InstallSources {
                model: &model,
                reference: &reference,
                mask: &mask,
            },
            &root,
        )
        .expect_err("bounded staging");
        assert_eq!(error.kind(), AssetErrorKind::BundleInvalid);
        assert_eq!(
            fs::read_dir(&staging).expect("staging remains").count(),
            129
        );
        assert!(!runtime.join("active.json").exists());
    }
}
