//! Offline installation and bounded discovery of one coherent runtime profile.

use crate::{
    AssetError, AssetErrorKind, RuntimeProfile, SnvBundleInspection,
    canonical_runtime_profile_bytes, inspect_snv_bundle, parse_runtime_profile, runtime_profile_id,
};
use pangopup_index::{mask::MaskDomainsOpen, reference_admission::inspect_reference_admission};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
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
#[derive(Clone, Copy)]
enum SourceMutation {
    Replace,
    Truncate,
}

#[cfg(test)]
thread_local! {
    static SOURCE_MUTATION: std::cell::Cell<Option<SourceMutation>> =
        const { std::cell::Cell::new(None) };
    static REPLACE_DESTINATION: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn mutate_source_for_test(path: &Path) {
    let Some(mutation) = SOURCE_MUTATION.take() else {
        return;
    };
    match mutation {
        SourceMutation::Replace => {
            let old = path.with_extension("held-old");
            fs::rename(path, &old).expect("replace source path");
            fs::copy(&old, path).expect("replacement source");
        }
        SourceMutation::Truncate => {
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .expect("truncate source");
        }
    }
}

#[cfg(test)]
fn mutate_destination_for_test(components: &super::local::Dir) {
    if !REPLACE_DESTINATION.replace(false) {
        return;
    }
    let components = descriptor_path(components);
    fs::rename(components.join("model"), components.join("model-replaced"))
        .expect("move held destination");
    fs::create_dir(components.join("model")).expect("replacement destination");
    fs::set_permissions(
        components.join("model"),
        fs::Permissions::from_mode(DIR_PRIVATE),
    )
    .expect("replacement destination mode");
}

#[cfg(not(test))]
fn mutate_source_for_test(_path: &Path) {}

#[cfg(not(test))]
fn mutate_destination_for_test(_components: &super::local::Dir) {}

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
    install_with_profile(
        &bytes,
        &profile,
        InstallSources {
            model: model_bundle,
            reference: reference_bundle,
            mask,
        },
        data_root,
    )
}

fn install_with_profile(
    profile_bytes: &[u8],
    profile: &RuntimeProfile,
    sources: InstallSources<'_>,
    data_root: &Path,
) -> Result<RuntimeInstallOutcome, AssetError> {
    let canonical = canonical_runtime_profile_bytes(profile).map_err(profile_parse_error)?;
    if canonical != profile_bytes {
        return Err(profile_corrupt("runtime profile is not canonical"));
    }
    let profile_id = runtime_profile_id(profile_bytes)
        .map_err(profile_parse_error)?
        .to_string();

    let locked = super::local::acquire_shared_install_lock(data_root)?;
    let root = &locked.root;
    let root_path = descriptor_path(&root.dir);
    let snv = inspect_active_snv(&root_path)?;
    require_snv(profile, &snv)?;

    let runtime_dir = super::local::ensure_private_dir(&root.dir, "runtime", root)?;
    let components_dir = super::local::ensure_private_dir(&runtime_dir, "components", root)?;
    let model_dir = super::local::ensure_private_dir(&components_dir, "model", root)?;
    let reference_dir = super::local::ensure_private_dir(&components_dir, "reference", root)?;
    let mask_dir = super::local::ensure_private_dir(&components_dir, "mask", root)?;
    let profiles_dir = super::local::ensure_private_dir(&runtime_dir, "profiles", root)?;
    let staging_dir = super::local::ensure_private_dir(&runtime_dir, ".staging", root)?;
    let runtime = descriptor_path(&runtime_dir);
    let staging_root = descriptor_path(&staging_dir);
    reconcile_staging(&staging_root)?;
    reconcile_staged_active(&runtime)?;

    if let Some(status) = validate_ready(&root_path, profile)?
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
    let stage = staging_root.join(&nonce);
    create_private(&stage)?;
    let stage_dir = super::local::open_owned_dir(&staging_dir, &nonce, root, DIR_PRIVATE)?;
    let stage = descriptor_path(&stage_dir);
    let result = (|| {
        let staged_model = stage.join("model");
        let staged_reference = stage.join("reference");
        let staged_mask = stage.join("mask");
        copy_bundle(
            sources.model,
            &staged_model,
            "model.onnx",
            profile.model.member_bytes,
            &profile.model.member_sha256,
        )?;
        copy_bundle(
            sources.reference,
            &staged_reference,
            "reference.pgr",
            profile.reference.member_bytes,
            &profile.reference.member_sha256,
        )?;
        create_private(&staged_mask)?;
        copy_member(
            sources.mask,
            &staged_mask.join("domains.pgm"),
            profile.mask.member_bytes,
            &profile.mask.member_sha256,
        )?;
        transition!(StagedObjectsDurable);

        validate_staged(profile, &staged_model, &staged_reference, &staged_mask)?;

        let model_suffix = suffix(&profile.model.bundle_id)?;
        let reference_suffix = suffix(&profile.reference.bundle_id)?;
        let mask_suffix = suffix(&profile.mask.member_sha256)?;
        let model_published = publish_bundle(
            &staged_model,
            &model_dir,
            model_suffix,
            root,
            "model.onnx",
            profile.model.member_bytes,
        )?;
        let reference_published = publish_bundle(
            &staged_reference,
            &reference_dir,
            reference_suffix,
            root,
            "reference.pgr",
            profile.reference.member_bytes,
        )?;
        let mask_published = publish_mask(
            &staged_mask,
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
        validate_model_bundle(&descriptor_path(&model_published.bundle), profile)
            .map_err(|_| conflict("immutable model component conflicts"))?;
        let reference_member = authenticate_member(
            &reference_published.bundle,
            "reference.pgr",
            root,
            profile.reference.member_bytes,
            &profile.reference.member_sha256,
        )?;
        let admitted = inspect_reference_admission(&descriptor_path(&reference_published.bundle))
            .map_err(|_| conflict("immutable reference component conflicts"))?;
        if admitted.bundle_id() != profile.reference.bundle_id {
            return Err(conflict("immutable reference component conflicts"));
        }
        let mask_member = authenticate_member(
            &mask_published.dir,
            "domains.pgm",
            root,
            profile.mask.member_bytes,
            &profile.mask.member_sha256,
        )?;
        MaskDomainsOpen::open(&descriptor_path(&mask_published.dir).join("domains.pgm"))
            .map_err(|_| conflict("immutable mask component conflicts"))?;
        transition!(ComponentsPublished);

        let profile_suffix = suffix(&profile_id)?;
        let staged_profile = stage.join("profile");
        create_private(&staged_profile)?;
        let stored_profile = staged_profile.join("profile.json");
        write_new(&stored_profile, profile_bytes, FILE_IMMUTABLE)?;
        let receipt = receipt(profile, &profile_id);
        let receipt_bytes =
            serde_jcs::to_vec(&receipt).map_err(|_| output("serialize runtime receipt"))?;
        write_new(
            &staged_profile.join("receipt.json"),
            &receipt_bytes,
            FILE_IMMUTABLE,
        )?;
        sync_dir(&staged_profile)?;
        let published_profile = publish_directory(
            &staged_profile,
            &profiles_dir,
            profile_suffix,
            root,
            |path| validate_profile_directory(path, profile, &profile_id).map(|_| ()),
        )?;
        transition!(ProfilePublished);

        validate_profile_directory(
            &descriptor_path(&published_profile.dir),
            profile,
            &profile_id,
        )?;
        mutate_destination_for_test(&components_dir);
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
        let staged_active = runtime.join(".active.new");
        if staged_active.exists() {
            return Err(conflict("staged active pointer already exists"));
        }
        write_new(&staged_active, &active_bytes, FILE_PRIVATE)?;
        transition!(BeforeActiveRename);
        rename_replace_path(&staged_active, &runtime.join("active.json"))?;
        transition!(AfterActiveRename);
        sync_dir(&runtime)?;
        Ok(outcome("installed", profile, profile_id.clone()))
    })();
    let _ = remove_stage(&stage);
    result
}

pub fn runtime_local_status(data_root: &Path) -> Result<RuntimeLocalStatus, AssetError> {
    let Some(root) = super::local::open_root(data_root, false)? else {
        return Ok(RuntimeLocalStatus::Missing {
            data_dir: data_root.to_owned(),
        });
    };
    let installing = super::local::probe_install_lock(&root)?;
    let root_path = descriptor_path(&root.dir);
    runtime_local_status_with(
        &root_path,
        data_root,
        &crate::production_runtime_profile(),
        installing,
    )
}

fn runtime_local_status_with(
    root_path: &Path,
    display_root: &Path,
    trusted: &RuntimeProfile,
    installing: bool,
) -> Result<RuntimeLocalStatus, AssetError> {
    match validate_ready(root_path, trusted)? {
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
    validate_model_bundle(&descriptor_path(&model_bundle), profile)
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
    let admitted = inspect_reference_admission(&descriptor_path(&reference_bundle))
        .map_err(|_| conflict("immutable reference component conflicts"))?;
    if admitted.bundle_id() != profile.reference.bundle_id {
        return Err(conflict("immutable reference component conflicts"));
    }
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
    MaskDomainsOpen::open(&descriptor_path(&mask_identity).join("domains.pgm"))
        .map_err(|_| conflict("immutable mask component conflicts"))?;
    let profile_identity =
        super::local::open_owned_dir(profiles, suffix(&profile_id)?, root, DIR_IMMUTABLE)?;
    validate_profile_directory(&descriptor_path(&profile_identity), profile, &profile_id)?;
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

fn validate_ready(data_root: &Path, trusted: &RuntimeProfile) -> Result<Option<Ready>, AssetError> {
    let runtime = data_root.join("runtime");
    let active_path = runtime.join("active.json");
    if !nofollow_exists(&active_path)? {
        return Ok(None);
    }
    require_installed_file(&active_path, FILE_PRIVATE, MAX_JSON)?;
    let active: RuntimeActive = read_canonical(&active_path, MAX_JSON)?;
    if active.schema != ACTIVE_SCHEMA || !valid_identity(&active.profile_id) {
        return Err(profile_corrupt("runtime active state is invalid"));
    }
    let profile_dir = runtime.join("profiles").join(suffix(&active.profile_id)?);
    require_installed_dir(&profile_dir, DIR_IMMUTABLE)?;
    let profile_bytes = read_installed(&profile_dir.join("profile.json"), MAX_JSON)?;
    let profile = parse_runtime_profile(&profile_bytes)
        .map_err(|_| profile_corrupt("installed runtime profile is invalid"))?;
    if &profile != trusted {
        return Err(AssetError::new(
            AssetErrorKind::TransportIncompatible,
            "installed runtime profile is not trusted",
        ));
    }
    let observed_id = runtime_profile_id(&profile_bytes)
        .map_err(|_| profile_corrupt("installed runtime profile is invalid"))?
        .to_string();
    if observed_id != active.profile_id {
        return Err(profile_corrupt("runtime profile identity mismatch"));
    }
    let receipt = validate_profile_directory(&profile_dir, &profile, &observed_id)?;
    let model_path = data_root.join("runtime").join(&receipt.model.path);
    let reference_path = data_root.join("runtime").join(&receipt.reference.path);
    let mask_path = data_root.join("runtime").join(&receipt.mask.path);
    validate_component_shape(&model_path, "model.onnx", receipt.model.size, true)?;
    validate_component_shape(
        &reference_path,
        "reference.pgr",
        receipt.reference.size,
        true,
    )?;
    validate_component_shape(&mask_path, "domains.pgm", receipt.mask.size, false)?;
    Ok(Some(Ready {
        profile_id: observed_id,
        snv_bundle_id: receipt.snv_bundle_id,
        model_bundle_id: profile.model.bundle_id,
        reference_bundle_id: profile.reference.bundle_id,
        mask_sha256: profile.mask.member_sha256,
        model_path: relative_runtime_path(data_root, &model_path)?,
        reference_path: relative_runtime_path(data_root, &reference_path)?,
        mask_path: relative_runtime_path(data_root, &mask_path)?,
    }))
}

fn inspect_active_snv(data_root: &Path) -> Result<SnvBundleInspection, AssetError> {
    let active_path = data_root.join("active.json");
    if !nofollow_exists(&active_path)? {
        return Err(AssetError::new(
            AssetErrorKind::AssetsMissing,
            "an active SNV bundle is required",
        ));
    }
    require_installed_file(&active_path, FILE_PRIVATE, MAX_JSON)?;
    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Active {
        schema: String,
        bundle_id: String,
    }
    let active: Active = read_canonical(&active_path, MAX_JSON)?;
    if active.schema != "pangopup.active-profile.v1" || !valid_identity(&active.bundle_id) {
        return Err(AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV state is invalid",
        ));
    }
    let wrapper = data_root.join("bundles").join(suffix(&active.bundle_id)?);
    require_installed_dir(&wrapper, DIR_IMMUTABLE)?;
    exact_names(&wrapper, &["bundle", "receipt.json"])?;
    require_installed_file(&wrapper.join("receipt.json"), FILE_IMMUTABLE, MAX_JSON)?;
    let receipt: SnvInstallReceipt = read_canonical(&wrapper.join("receipt.json"), MAX_JSON)?;
    if receipt.schema != "pangopup.install-receipt.v1"
        || receipt.bundle_id != active.bundle_id
        || !valid_identity(&receipt.transport_id)
        || receipt.members.len() != 3
        || receipt.members[0].path != "bundle/NOTICE"
        || receipt.members[1].path != "bundle/manifest.json"
        || receipt.members[2].path != "bundle/scores.pgi"
        || receipt
            .members
            .iter()
            .any(|member| !valid_identity(&member.sha256))
    {
        return Err(AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV receipt is invalid",
        ));
    }
    let bundle = wrapper.join("bundle");
    require_installed_dir(&bundle, DIR_IMMUTABLE)?;
    exact_names(&bundle, &["NOTICE", "manifest.json", "scores.pgi"])?;
    require_installed_file(&bundle.join("NOTICE"), FILE_IMMUTABLE, MAX_NOTICE)?;
    require_installed_file(&bundle.join("manifest.json"), FILE_IMMUTABLE, MAX_JSON)?;
    require_installed_file(
        &bundle.join("scores.pgi"),
        FILE_IMMUTABLE,
        crate::MAX_FIXED11_BYTES,
    )?;
    let inspection = inspect_snv_bundle(&bundle).map_err(|_| {
        AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV bundle metadata is invalid",
        )
    })?;
    if inspection.bundle_id != active.bundle_id {
        return Err(AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV bundle identity mismatch",
        ));
    }
    if receipt.members[2].size != inspection.member_bytes
        || receipt.members[2].sha256 != inspection.member_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV receipt does not match its manifest",
        ));
    }
    let notice = read_installed(&bundle.join("NOTICE"), MAX_NOTICE)?;
    let manifest = read_installed(&bundle.join("manifest.json"), MAX_JSON)?;
    if notice != crate::NOTICE
        || receipt.members[0].size != notice.len() as u64
        || receipt.members[0].sha256 != format!("sha256:{:x}", Sha256::digest(&notice))
        || receipt.members[1].size != manifest.len() as u64
        || receipt.members[1].sha256 != format!("sha256:{:x}", Sha256::digest(&manifest))
    {
        return Err(AssetError::new(
            AssetErrorKind::AssetStateInvalid,
            "active SNV receipt does not match installed metadata",
        ));
    }
    Ok(inspection)
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

fn validate_staged(
    profile: &RuntimeProfile,
    model: &Path,
    reference: &Path,
    mask: &Path,
) -> Result<(), AssetError> {
    validate_model_bundle(model, profile)?;
    let reference_admission = inspect_reference_admission(reference)
        .map_err(|_| profile_corrupt("staged reference bundle is invalid"))?;
    if reference_admission.bundle_id() != profile.reference.bundle_id
        || reference_admission.profile() != profile.reference.profile
        || reference_admission.format() != profile.reference.format
        || reference_admission.assembly() != profile.reference.assembly
        || reference_admission.assembly_accession() != profile.reference.assembly_accession
        || reference_admission.sequence_set_sha256() != profile.reference.sequence_set_sha256
    {
        return Err(profile_corrupt(
            "staged reference facts do not match profile",
        ));
    }
    MaskDomainsOpen::open(&mask.join("domains.pgm"))
        .map_err(|_| profile_corrupt("staged mask is structurally invalid"))?;
    Ok(())
}

fn validate_model_bundle(bundle: &Path, profile: &RuntimeProfile) -> Result<(), AssetError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Member {
        bytes: u64,
        filename: String,
        sha256: String,
    }
    let manifest_bytes = read_installed(&bundle.join("manifest.json"), MAX_JSON)?;
    let value: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| profile_corrupt("staged model manifest is invalid"))?;
    if serde_jcs::to_vec(&value).map_err(|_| profile_corrupt("staged model manifest is invalid"))?
        != manifest_bytes
    {
        return Err(profile_corrupt("staged model manifest is not canonical"));
    }
    let object = value
        .as_object()
        .ok_or_else(|| profile_corrupt("staged model manifest is invalid"))?;
    let schema = object.get("schema").and_then(serde_json::Value::as_str);
    let declared_profile = object.get("profile").and_then(serde_json::Value::as_str);
    let members: Vec<Member> = serde_json::from_value(
        object
            .get("members")
            .cloned()
            .ok_or_else(|| profile_corrupt("staged model members are invalid"))?,
    )
    .map_err(|_| profile_corrupt("staged model members are invalid"))?;
    if schema != Some("pangopup-model-bundle-v1")
        || declared_profile != Some(profile.model.profile.as_str())
        || profile.model.representation != "singleton"
        || format!("sha256:{:x}", Sha256::digest(&manifest_bytes)) != profile.model.bundle_id
        || members.len() != 2
        || members[0].filename != "NOTICE"
        || members[1].filename != "model.onnx"
        || members.iter().any(|member| !valid_identity(&member.sha256))
    {
        return Err(profile_corrupt("staged model facts do not match profile"));
    }
    for member in members {
        let path = bundle.join(&member.filename);
        let metadata =
            fs::metadata(&path).map_err(|_| profile_corrupt("staged model member is missing"))?;
        if metadata.len() != member.bytes {
            return Err(profile_corrupt("staged model member size mismatch"));
        }
        if member.filename == "NOTICE" && digest_small(&path, MAX_NOTICE)? != member.sha256 {
            return Err(profile_corrupt("staged model notice identity mismatch"));
        }
        if member.filename == "model.onnx"
            && (member.bytes != profile.model.member_bytes
                || member.sha256 != profile.model.member_sha256)
        {
            return Err(profile_corrupt("staged model member identity mismatch"));
        }
    }
    Ok(())
}

fn digest_small(path: &Path, maximum: u64) -> Result<String, AssetError> {
    let bytes = read_installed(path, maximum)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn copy_bundle(
    source: &Path,
    destination: &Path,
    payload: &str,
    payload_size: u64,
    payload_sha: &str,
) -> Result<(), AssetError> {
    require_source_dir(source)?;
    exact_source_names(source, &["NOTICE", "manifest.json", payload])?;
    create_private(destination)?;
    copy_member(
        &source.join("manifest.json"),
        &destination.join("manifest.json"),
        MAX_JSON,
        "",
    )?;
    copy_member(
        &source.join("NOTICE"),
        &destination.join("NOTICE"),
        MAX_NOTICE,
        "",
    )?;
    copy_member(
        &source.join(payload),
        &destination.join(payload),
        payload_size,
        payload_sha,
    )?;
    sync_dir(destination)?;
    Ok(())
}

fn exact_source_names(path: &Path, expected: &[&str]) -> Result<(), AssetError> {
    let names = fs::read_dir(path)
        .map_err(|_| input("inspect source directory"))?
        .enumerate()
        .map(|(index, entry)| {
            if index >= expected.len() {
                return Err(AssetError::new(
                    AssetErrorKind::StagingInvalid,
                    "source member set is unsafe",
                ));
            }
            entry
                .map_err(|_| input("inspect source directory entry"))?
                .file_name()
                .into_string()
                .map_err(|_| {
                    AssetError::new(
                        AssetErrorKind::StagingInvalid,
                        "source member name is unsafe",
                    )
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
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
    source: &Path,
    destination: &Path,
    maximum_or_exact: u64,
    expected_sha: &str,
) -> Result<(), AssetError> {
    let before_path = fs::symlink_metadata(source).map_err(|_| input("open source member"))?;
    if !before_path.file_type().is_file()
        || before_path.file_type().is_symlink()
        || before_path.nlink() != 1
        || (!expected_sha.is_empty() && before_path.len() != maximum_or_exact)
        || (expected_sha.is_empty() && before_path.len() > maximum_or_exact)
    {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member is unsafe",
        ));
    }
    let mut input_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(source)
        .map_err(|_| input("open source member"))?;
    let held = input_file
        .metadata()
        .map_err(|_| input("inspect source member"))?;
    if held.dev() != before_path.dev() || held.ino() != before_path.ino() {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member changed before copy",
        ));
    }
    mutate_source_for_test(source);
    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_PRIVATE)
        .open(destination)
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
    let after_path = fs::symlink_metadata(source).map_err(|_| input("reinspect source member"))?;
    if after_held.dev() != held.dev()
        || after_held.ino() != held.ino()
        || after_held.len() != held.len()
        || after_path.dev() != held.dev()
        || after_path.ino() != held.ino()
        || after_path.len() != held.len()
        || total != held.len()
    {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source member changed during copy",
        ));
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

struct PublishedBundle {
    identity: super::local::Dir,
    bundle: super::local::Dir,
}

struct PublishedDirectory {
    dir: super::local::Dir,
}

fn publish_bundle(
    source: &Path,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    payload: &str,
    expected_payload: u64,
) -> Result<PublishedBundle, AssetError> {
    let wrapper = source.with_extension("wrapper");
    create_private(&wrapper)?;
    rename_replace_path(source, &wrapper.join("bundle"))?;
    set_mode(&wrapper.join("bundle"), DIR_IMMUTABLE)?;
    sync_dir(&wrapper)?;
    let published = publish_directory(
        &wrapper,
        destination_parent,
        destination_name,
        root,
        |path| validate_component_shape(&path.join("bundle"), payload, expected_payload, true),
    )?;
    let bundle = super::local::open_owned_dir(&published.dir, "bundle", root, DIR_IMMUTABLE)?;
    Ok(PublishedBundle {
        identity: published.dir,
        bundle,
    })
}

fn publish_mask(
    source: &Path,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    expected: u64,
) -> Result<PublishedDirectory, AssetError> {
    publish_directory(source, destination_parent, destination_name, root, |path| {
        validate_component_shape(&path.join("domains.pgm"), "domains.pgm", expected, false)
    })
}

fn publish_directory(
    source: &Path,
    destination_parent: &super::local::Dir,
    destination_name: &str,
    root: &super::local::Root,
    validate_existing: impl FnOnce(&Path) -> Result<(), AssetError>,
) -> Result<PublishedDirectory, AssetError> {
    if let Some(existing) =
        super::local::open_owned_dir_optional(destination_parent, destination_name, root)?
    {
        validate_existing(&descriptor_path(&existing))
            .map_err(|_| conflict("immutable runtime component conflicts"))?;
        remove_stage(source)?;
        return Ok(PublishedDirectory { dir: existing });
    }
    let (source_parent, source_name) = open_parent(source)?;
    rustix::fs::renameat_with(
        &source_parent,
        source_name,
        &destination_parent.file,
        destination_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        let error = std::io::Error::from(error);
        if matches!(
            error.kind(),
            std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::DirectoryNotEmpty
        ) {
            conflict("immutable runtime identity already exists")
        } else {
            output("publish runtime object")
        }
    })?;
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

fn nofollow_exists(path: &Path) -> Result<bool, AssetError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(profile_corrupt("runtime object cannot be inspected")),
    }
}

fn validate_profile_directory(
    path: &Path,
    profile: &RuntimeProfile,
    profile_id: &str,
) -> Result<RuntimeReceipt, AssetError> {
    require_installed_dir(path, DIR_IMMUTABLE)?;
    exact_names(path, &["profile.json", "receipt.json"])?;
    let profile_bytes = read_installed(&path.join("profile.json"), MAX_JSON)?;
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
    require_installed_file(&path.join("receipt.json"), FILE_IMMUTABLE, MAX_JSON)?;
    let installed_receipt: RuntimeReceipt = read_canonical(&path.join("receipt.json"), MAX_JSON)?;
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

fn validate_component_shape(
    path: &Path,
    payload: &str,
    expected_size: u64,
    bundle: bool,
) -> Result<(), AssetError> {
    if bundle {
        require_installed_dir(path, DIR_IMMUTABLE)?;
        let payload = if payload.is_empty() {
            if path.to_string_lossy().contains("/model/") {
                "model.onnx"
            } else {
                "reference.pgr"
            }
        } else {
            payload
        };
        exact_names(path, &["NOTICE", "manifest.json", payload])?;
        require_installed_file(&path.join("NOTICE"), FILE_IMMUTABLE, MAX_NOTICE)?;
        require_installed_file(&path.join("manifest.json"), FILE_IMMUTABLE, MAX_JSON)?;
        require_installed_file(&path.join(payload), FILE_IMMUTABLE, expected_size)?;
        if fs::metadata(path.join(payload))
            .map_err(|_| profile_corrupt("installed component is missing"))?
            .len()
            != expected_size
        {
            return Err(profile_corrupt("installed component size mismatch"));
        }
    } else {
        require_installed_file(path, FILE_IMMUTABLE, expected_size)?;
        if fs::metadata(path)
            .map_err(|_| profile_corrupt("installed mask is missing"))?
            .len()
            != expected_size
        {
            return Err(profile_corrupt("installed mask size mismatch"));
        }
    }
    Ok(())
}

fn reconcile_staging(staging: &Path) -> Result<(), AssetError> {
    let mut stages = Vec::new();
    for (index, entry) in fs::read_dir(staging)
        .map_err(|_| output("inspect runtime staging"))?
        .enumerate()
    {
        if index >= 128 {
            return Err(profile_corrupt("runtime staging entry limit exceeded"));
        }
        let entry = entry.map_err(|_| output("inspect runtime staging entry"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| profile_corrupt("runtime staging name is invalid"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        {
            return Err(profile_corrupt("runtime staging entry is unsafe"));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| output("inspect runtime staging entry"))?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != effective_uid()
        {
            return Err(profile_corrupt("runtime staging entry is unsafe"));
        }
        stages.push(entry.path());
    }
    for stage in stages {
        remove_stage(&stage)?;
    }
    Ok(())
}

fn reconcile_staged_active(runtime: &Path) -> Result<(), AssetError> {
    let path = runtime.join(".active.new");
    if !nofollow_exists(&path)? {
        return Ok(());
    }
    require_installed_file(&path, FILE_PRIVATE, MAX_JSON)?;
    fs::remove_file(path).map_err(|_| output("remove staged active pointer"))?;
    sync_dir(runtime)
}

fn create_private(path: &Path) -> Result<(), AssetError> {
    fs::create_dir(path).map_err(|_| output("create staged directory"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(DIR_PRIVATE))
        .map_err(|_| output("set staged directory mode"))
}

fn require_source_dir(path: &Path) -> Result<(), AssetError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| input("inspect source directory"))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "source directory is unsafe",
        ));
    }
    Ok(())
}

fn require_installed_dir(path: &Path, mode: u32) -> Result<(), AssetError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| profile_corrupt("installed directory is missing"))?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != mode
    {
        return Err(profile_corrupt("installed directory is unsafe"));
    }
    Ok(())
}

fn require_installed_file(path: &Path, mode: u32, maximum: u64) -> Result<(), AssetError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| profile_corrupt("installed file is missing"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != effective_uid()
        || metadata.nlink() != 1
        || metadata.mode() & 0o777 != mode
        || metadata.len() > maximum
    {
        return Err(profile_corrupt("installed file is unsafe"));
    }
    Ok(())
}

fn exact_names(path: &Path, expected: &[&str]) -> Result<(), AssetError> {
    let names = fs::read_dir(path)
        .map_err(|_| profile_corrupt("directory cannot be inspected"))?
        .enumerate()
        .map(|(index, entry)| {
            if index >= expected.len() {
                return Err(profile_corrupt("directory member set is invalid"));
            }
            entry
                .map_err(|_| profile_corrupt("directory entry cannot be inspected"))?
                .file_name()
                .into_string()
                .map_err(|_| profile_corrupt("directory entry name is invalid"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = expected.iter().map(|name| (*name).to_owned()).collect();
    if names != expected {
        return Err(profile_corrupt("directory member set is invalid"));
    }
    Ok(())
}

fn read_small_source(path: &Path, maximum: u64) -> Result<Vec<u8>, AssetError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| input("open runtime profile"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1
    {
        return Err(AssetError::new(
            AssetErrorKind::StagingInvalid,
            "runtime profile input is unsafe",
        ));
    }
    read_bounded(path, maximum, AssetErrorKind::InputIo)
}

fn read_installed(path: &Path, maximum: u64) -> Result<Vec<u8>, AssetError> {
    require_installed_file(path, FILE_IMMUTABLE, maximum)?;
    read_bounded(path, maximum, AssetErrorKind::BundleInvalid)
}

fn read_canonical<T: DeserializeOwned + Serialize>(
    path: &Path,
    maximum: u64,
) -> Result<T, AssetError> {
    let bytes = read_bounded(path, maximum, AssetErrorKind::BundleInvalid)?;
    let value: T =
        serde_json::from_slice(&bytes).map_err(|_| profile_corrupt("installed JSON is invalid"))?;
    let canonical =
        serde_jcs::to_vec(&value).map_err(|_| profile_corrupt("installed JSON is invalid"))?;
    if canonical != bytes {
        return Err(profile_corrupt("installed JSON is not canonical"));
    }
    Ok(value)
}

fn read_bounded(path: &Path, maximum: u64, kind: AssetErrorKind) -> Result<Vec<u8>, AssetError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| AssetError::new(kind, "file cannot be opened"))?;
    let metadata = file
        .metadata()
        .map_err(|_| AssetError::new(kind, "file cannot be inspected"))?;
    if metadata.len() > maximum {
        return Err(AssetError::new(kind, "file exceeds its size bound"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AssetError::new(kind, "file cannot be read"))?;
    if bytes.len() as u64 != metadata.len() {
        return Err(AssetError::new(kind, "file changed while reading"));
    }
    Ok(bytes)
}

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

fn rename_replace_path(source: &Path, destination: &Path) -> Result<(), AssetError> {
    let (source_parent, source_name) = open_parent(source)?;
    let (destination_parent, destination_name) = open_parent(destination)?;
    rustix::fs::renameat(
        &source_parent,
        source_name,
        &destination_parent,
        destination_name,
    )
    .map_err(|_| output("activate runtime profile"))
}

fn open_parent(path: &Path) -> Result<(File, String), AssetError> {
    let parent = path
        .parent()
        .ok_or_else(|| output("resolve runtime object parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| output("resolve runtime object name"))?
        .to_owned();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent.join("."))
        .map_err(|_| output("open runtime object parent"))?;
    Ok((file, name))
}

fn descriptor_path(dir: &super::local::Dir) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/.", dir.file.as_raw_fd()))
}

fn sync_dir(path: &Path) -> Result<(), AssetError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| output("sync runtime directory"))
}

fn set_mode(path: &Path, mode: u32) -> Result<(), AssetError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| output("inspect runtime object mode"))?;
    let flags = libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | if metadata.file_type().is_dir() {
            libc::O_DIRECTORY
        } else {
            0
        };
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
        .map_err(|_| output("open runtime object for mode"))?;
    super::local::set_mode(&file, mode)?;
    file.sync_all()
        .map_err(|_| output("sync runtime object mode"))
}

fn remove_stage(path: &Path) -> Result<(), AssetError> {
    if !nofollow_exists(path)? {
        return Ok(());
    }
    let (parent, name) = open_parent(path)?;
    super::local::remove_owned_tree_bounded(parent, &name, 8, 64)
}

fn relative_runtime_path(data_root: &Path, path: &Path) -> Result<PathBuf, AssetError> {
    let relative = path
        .strip_prefix(data_root)
        .map_err(|_| profile_corrupt("installed runtime path escaped data root"))?;
    Ok(relative.to_owned())
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
    use std::os::unix::fs::MetadataExt;
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
        runtime_local_status_with(&descriptor_path(&opened.dir), root, profile, false)
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
                "replace" => SOURCE_MUTATION.set(Some(SourceMutation::Replace)),
                "truncate" => SOURCE_MUTATION.set(Some(SourceMutation::Truncate)),
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
