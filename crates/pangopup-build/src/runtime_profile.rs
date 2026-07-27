//! Production-only composition of the four-asset runtime profile.

use crate::CommandError;
use pangopup_assets::{
    MaskProfile, ModelProfile, ReferenceProfile, RuntimeProfile, ScoringProfile, SnvProfile,
    canonical_runtime_profile_bytes, inspect_snv_bundle, production_runtime_profile,
    runtime_profile_id,
};
use pangopup_core::ReferenceProvider;
use pangopup_index::{
    mask::{MaskDomainsOpen, MaskError},
    reference::{ReferenceBundleOpen, ReferenceIndexError},
};
use pangopup_model::{ModelError, ModelRepresentation, inspect_runtime_profile_bundle};
use serde::Serialize;
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrepareRuntimeProfileOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub profile_id: String,
    pub bytes: u64,
}

pub fn prepare_runtime_profile(
    snv_bundle: &Path,
    model_bundle: &Path,
    reference_bundle: &Path,
    mask: &Path,
    output: &Path,
) -> Result<PrepareRuntimeProfileOutcome, CommandError> {
    let trusted = production_runtime_profile();
    let snv = inspect_snv_bundle(snv_bundle).map_err(map_snv_error)?;
    if snv.bundle_id != trusted.snv.bundle_id
        || snv.format != trusted.snv.format
        || snv.member_bytes != trusted.snv.member_bytes
        || snv.member_sha256 != trusted.snv.member_sha256
    {
        return Err(incompatible(
            "SNV bundle is not the accepted production member",
        ));
    }
    let model = inspect_runtime_profile_bundle(model_bundle).map_err(map_model_error)?;
    if model.bundle_id.as_str() != trusted.model.bundle_id
        || model.profile != trusted.model.profile
        || representation(model.representation) != trusted.model.representation
        || model.member_bytes != trusted.model.member_bytes
        || model.member_sha256 != trusted.model.member_sha256
    {
        return Err(incompatible(
            "model bundle is not the accepted production member",
        ));
    }
    let reference =
        ReferenceBundleOpen::open_identified(reference_bundle).map_err(map_reference_error)?;

    let reference_provenance = reference.provenance();
    if reference_provenance.bundle_id() != trusted.reference.bundle_id
        || reference_provenance.profile() != trusted.reference.profile
        || reference_provenance.format() != trusted.reference.format
        || reference_provenance.assembly() != trusted.reference.assembly
        || reference_provenance.assembly_accession() != trusted.reference.assembly_accession
        || reference_provenance.sequence_set_sha256() != trusted.reference.sequence_set_sha256
        || reference.identity().bytes() != trusted.reference.member_bytes
        || reference.identity().sha256() != trusted.reference.member_sha256
    {
        return Err(incompatible(
            "reference bundle is not the accepted production member",
        ));
    }
    let mask = MaskDomainsOpen::open_identified(mask).map_err(map_mask_error)?;
    let mask_sha256 = format!("sha256:{}", mask.identity().sha256());
    if mask.identity().bytes() != trusted.mask.member_bytes
        || mask_sha256 != trusted.mask.member_sha256
    {
        return Err(incompatible("mask is not the accepted production member"));
    }
    let profile = RuntimeProfile {
        schema: "pangopup.runtime-profile.v1".to_owned(),
        snv: SnvProfile {
            bundle_id: snv.bundle_id,
            format: snv.format,
            member_bytes: snv.member_bytes,
            member_sha256: snv.member_sha256,
        },
        model: ModelProfile {
            bundle_id: model.bundle_id.to_string(),
            profile: model.profile,
            representation: representation(model.representation).to_owned(),
            member_bytes: model.member_bytes,
            member_sha256: model.member_sha256,
        },
        reference: ReferenceProfile {
            bundle_id: reference_provenance.bundle_id().to_owned(),
            profile: reference_provenance.profile().to_owned(),
            format: reference_provenance.format().to_owned(),
            assembly: reference_provenance.assembly().to_owned(),
            assembly_accession: reference_provenance.assembly_accession().to_owned(),
            sequence_set_sha256: reference_provenance.sequence_set_sha256().to_owned(),
            member_bytes: reference.identity().bytes(),
            member_sha256: reference.identity().sha256().to_owned(),
        },
        mask: MaskProfile {
            format: "pangopup.gencode-v38-domains.v1".to_owned(),
            member_bytes: mask.identity().bytes(),
            member_sha256: mask_sha256,
        },
        scoring: ScoringProfile {
            assembly: "GRCh38".to_owned(),
            semantics: "pangopup-variant-score-v1".to_owned(),
            distance: 50,
            masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
            cpu_policy: "sequential:1/1".to_owned(),
        },
    };
    profile
        .require_trusted_production()
        .map_err(|_| incompatible("runtime assets do not match the accepted production tuple"))?;
    let bytes = canonical_runtime_profile_bytes(&profile)
        .map_err(|_| corrupt("runtime profile serialization failed"))?;
    let identity = runtime_profile_id(&bytes)
        .map_err(|_| corrupt("runtime profile identity failed"))?
        .to_string();
    publish_new(output, &bytes)?;
    Ok(PrepareRuntimeProfileOutcome {
        status: "ok",
        command: "runtime-profile.prepare",
        profile_id: identity,
        bytes: bytes.len() as u64,
    })
}

fn representation(value: ModelRepresentation) -> &'static str {
    match value {
        ModelRepresentation::Singleton => "singleton",
        ModelRepresentation::ZeroPaddedBatch => "zero-padded-batch",
        ModelRepresentation::PairedStrandBatch => "paired-strand-batch",
    }
}

fn map_snv_error(error: pangopup_assets::RuntimeProfileError) -> CommandError {
    use pangopup_assets::RuntimeProfileError;
    match error {
        RuntimeProfileError::InputIo => input_io("SNV bundle input failed"),
        RuntimeProfileError::UnsafeInput => unsafe_input("SNV bundle is unsafe or changed"),
        RuntimeProfileError::Incompatible => incompatible("SNV bundle is incompatible"),
        _ => corrupt("SNV bundle metadata is corrupt"),
    }
}

fn map_model_error(error: ModelError) -> CommandError {
    match error {
        ModelError::Io { source, .. } if is_unsafe_io(&source) => {
            unsafe_input("model bundle path or member is unsafe")
        }
        ModelError::Io { .. } => input_io("model bundle input failed"),
        ModelError::IncompatibleBundle(_) => incompatible("model bundle is incompatible"),
        ModelError::InvalidBundle(reason) if unsafe_model_reason(reason) => {
            unsafe_input("model bundle path or member is unsafe")
        }
        ModelError::InvalidBundle(_) => corrupt("model bundle is corrupt"),
        _ => corrupt("model bundle inspection failed"),
    }
}

fn map_reference_error(error: ReferenceIndexError) -> CommandError {
    match error {
        ReferenceIndexError::Io(error) if is_unsafe_io(&error) => {
            unsafe_input("reference bundle path or member is unsafe")
        }
        ReferenceIndexError::Io(_) => input_io("reference bundle input failed"),
        ReferenceIndexError::Incompatible(_) => incompatible("reference bundle is incompatible"),
        ReferenceIndexError::Corrupt(reason) if unsafe_reference_reason(reason) => {
            unsafe_input("reference bundle path or member is unsafe")
        }
        ReferenceIndexError::Corrupt(_) | ReferenceIndexError::Bounds => {
            corrupt("reference bundle is corrupt")
        }
    }
}

fn map_mask_error(error: MaskError) -> CommandError {
    match error {
        MaskError::Io(error) if is_unsafe_io(&error) => {
            unsafe_input("mask path or member is unsafe")
        }
        MaskError::Io(_) => input_io("mask input failed"),
        MaskError::UnsupportedCodec => incompatible("mask format is incompatible"),
        MaskError::Authentication("path replaced during hashing")
        | MaskError::Authentication("member changed during hashing")
        | MaskError::Invalid("member length or type") => {
            unsafe_input("mask path or member is unsafe")
        }
        MaskError::Authentication(_)
        | MaskError::Invalid(_)
        | MaskError::Bounds(_)
        | MaskError::Resource(_)
        | MaskError::Arithmetic(_) => corrupt("mask is corrupt"),
    }
}

fn is_unsafe_io(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}

fn unsafe_model_reason(reason: &str) -> bool {
    matches!(
        reason,
        "bundle directory type"
            | "member set"
            | "non-UTF-8 member name"
            | "member is not a regular file"
            | "member link count"
            | "member set changed during open"
            | "bundle member changed during open"
            | "bundle directory changed during open"
            | "model admission member"
    )
}

fn unsafe_reference_reason(reason: &str) -> bool {
    matches!(
        reason,
        "bundle member set"
            | "bundle member type"
            | "bundle member name"
            | "reference member length or type"
            | "reference member changed during hashing"
            | "reference member path replaced during hashing"
    )
}

fn publish_new(output: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if bytes.len() > 64 * 1024 {
        return Err(corrupt("runtime profile output exceeds its bound"));
    }
    let parent = output
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut directory_options = OpenOptions::new();
    directory_options
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY);
    let directory = directory_options
        .open(parent)
        .map_err(|_| unsafe_input("runtime profile output parent is unsafe"))?;
    let parent_metadata = directory
        .metadata()
        .map_err(|_| output_io("runtime profile output parent is unavailable"))?;
    if !parent_metadata.file_type().is_dir() {
        return Err(unsafe_input("runtime profile output parent is unsafe"));
    }
    let name = output
        .file_name()
        .filter(|value| !value.as_bytes().contains(&b'/'))
        .ok_or_else(|| unsafe_input("runtime profile output name is unsafe"))?;
    let sequence = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stage_name = format!(
        ".{}.pangopup-stage-{}-{sequence}",
        String::from_utf8_lossy(name.as_bytes()),
        std::process::id()
    );
    let stage = CString::new(stage_name.as_bytes())
        .map_err(|_| unsafe_input("runtime profile staging name is unsafe"))?;
    let output_name = CString::new(name.as_bytes())
        .map_err(|_| unsafe_input("runtime profile output name is unsafe"))?;
    // SAFETY: the held directory descriptor and C string remain valid for the
    // syscall; a successful result transfers one newly owned descriptor.
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            stage.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if descriptor < 0 {
        return Err(output_io("runtime profile staging creation failed"));
    }
    // SAFETY: `openat` returned one newly owned descriptor above.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let result = (|| {
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| output_io("runtime profile staging write failed"))?;
        rename_noreplace(&directory, &stage, &output_name)?;
        directory
            .sync_all()
            .map_err(|_| output_io("runtime profile parent sync failed"))
    })();
    if result.is_err() {
        // SAFETY: the held directory and stage name remain valid. Failure is
        // best-effort cleanup of an unpublished private file.
        unsafe {
            libc::unlinkat(directory.as_raw_fd(), stage.as_ptr(), 0);
        }
    }
    result
}

fn rename_noreplace(
    directory: &File,
    stage: &CString,
    output: &CString,
) -> Result<(), CommandError> {
    // SAFETY: both C strings and the held directory descriptor remain valid
    // for the duration of the syscall.
    let status = unsafe {
        libc::renameat2(
            directory.as_raw_fd(),
            stage.as_ptr(),
            directory.as_raw_fd(),
            output.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::AlreadyExists {
        Err(output_conflict())
    } else {
        Err(output_io("runtime profile atomic publication failed"))
    }
}

fn incompatible(message: &'static str) -> CommandError {
    CommandError::new("PROFILE_INCOMPATIBLE", message)
}

fn unsafe_input(message: &'static str) -> CommandError {
    CommandError::new("PROFILE_UNSAFE", message)
}

fn corrupt(message: &'static str) -> CommandError {
    CommandError::new("PROFILE_CORRUPT", message)
}

fn input_io(message: &'static str) -> CommandError {
    CommandError::new("INPUT_IO", message)
}

fn output_conflict() -> CommandError {
    CommandError::new("OUTPUT_CONFLICT", "runtime profile output already exists")
}

fn output_io(message: &'static str) -> CommandError {
    CommandError::new("OUTPUT_IO", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    use tempfile::tempdir;

    #[test]
    fn output_publication_is_no_replace_private_and_atomic() {
        let temp = tempdir().expect("temp");
        let output = temp.path().join("profile.json");
        publish_new(&output, b"first").expect("publish");
        assert_eq!(fs::read(&output).expect("read"), b"first");
        assert_eq!(
            fs::metadata(&output)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            publish_new(&output, b"second").expect_err("conflict").code,
            "OUTPUT_CONFLICT"
        );
        assert_eq!(fs::read(&output).expect("unchanged"), b"first");
        assert_eq!(
            fs::read_dir(temp.path()).expect("list").count(),
            1,
            "no staging file remains"
        );
    }

    #[test]
    fn output_rejects_symlink_and_oversized_bytes_without_partial_final() {
        let temp = tempdir().expect("temp");
        let target = temp.path().join("target");
        fs::write(&target, b"target").expect("target");
        let output = temp.path().join("profile.json");
        std::os::unix::fs::symlink(&target, &output).expect("symlink");
        assert_eq!(
            publish_new(&output, b"value").expect_err("symlink").code,
            "OUTPUT_CONFLICT"
        );
        assert_eq!(fs::read(&target).expect("target unchanged"), b"target");
        fs::remove_file(&output).expect("remove link");
        assert_eq!(
            publish_new(&output, &vec![0; 64 * 1024 + 1])
                .expect_err("oversized")
                .code,
            "PROFILE_CORRUPT"
        );
        assert!(!output.exists());
    }

    #[test]
    fn component_errors_map_to_stable_redacted_categories() {
        let missing_model = map_model_error(ModelError::Io {
            operation: "open",
            source: io::Error::from(io::ErrorKind::NotFound),
        });
        let incompatible_model =
            map_model_error(ModelError::IncompatibleBundle("untrusted detail"));
        let unsafe_model = map_model_error(ModelError::InvalidBundle("member set"));
        let corrupt_model = map_model_error(ModelError::InvalidBundle("member digest"));
        let missing_reference =
            map_reference_error(ReferenceIndexError::Io(io::ErrorKind::NotFound.into()));
        let incompatible_reference =
            map_reference_error(ReferenceIndexError::Incompatible("untrusted detail"));
        let unsafe_reference =
            map_reference_error(ReferenceIndexError::Corrupt("bundle member type"));
        let corrupt_reference =
            map_reference_error(ReferenceIndexError::Corrupt("member identity"));
        let missing_mask = map_mask_error(MaskError::Io(io::ErrorKind::NotFound.into()));
        let incompatible_mask = map_mask_error(MaskError::UnsupportedCodec);
        let unsafe_mask = map_mask_error(MaskError::Authentication("path replaced during hashing"));
        let corrupt_mask = map_mask_error(MaskError::Authentication("member SHA-256"));

        for error in [missing_model, missing_reference, missing_mask] {
            assert_eq!(error.code, "INPUT_IO");
        }
        for error in [
            incompatible_model,
            incompatible_reference,
            incompatible_mask,
        ] {
            assert_eq!(error.code, "PROFILE_INCOMPATIBLE");
        }
        for error in [unsafe_model, unsafe_reference, unsafe_mask] {
            assert_eq!(error.code, "PROFILE_UNSAFE");
        }
        for error in [corrupt_model, corrupt_reference, corrupt_mask] {
            assert_eq!(error.code, "PROFILE_CORRUPT");
        }
        for error in [
            map_model_error(ModelError::IncompatibleBundle("/secret/untrusted")),
            map_reference_error(ReferenceIndexError::Corrupt("/secret/untrusted")),
            map_mask_error(MaskError::Invalid("/secret/untrusted")),
        ] {
            assert!(!error.message.contains("secret"));
            assert!(error.message.len() < 128);
        }
    }
}
