//! Bounded reference-manifest admission for the disposable model cache.

use crate::reference::{
    ReferenceIndexError, ReferenceManifest, canonical_reference_manifest_bytes, reference_bundle_id,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
};

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_NOTICE_BYTES: u64 = 64 * 1024;
const EXACT_MEMBERS: [&str; 3] = ["NOTICE", "manifest.json", "reference.pgr"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceAdmission {
    bundle_id: String,
    profile: String,
    format: String,
    assembly: String,
    assembly_accession: String,
    sequence_set_sha256: String,
}

impl ReferenceAdmission {
    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn assembly(&self) -> &str {
        &self.assembly
    }

    pub fn assembly_accession(&self) -> &str {
        &self.assembly_accession
    }

    pub fn sequence_set_sha256(&self) -> &str {
        &self.sequence_set_sha256
    }
}

/// Read and validate only bounded canonical metadata.
///
/// This does not open, hash, or mmap `reference.pgr`; a cache miss still uses
/// the production identified open.
pub fn inspect_reference_admission(
    bundle: &Path,
) -> Result<ReferenceAdmission, ReferenceIndexError> {
    let metadata = fs::symlink_metadata(bundle)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(ReferenceIndexError::Corrupt("bundle directory type"));
    }
    let mut names = BTreeSet::new();
    for (index, entry) in fs::read_dir(bundle)?.enumerate() {
        if index >= EXACT_MEMBERS.len() {
            return Err(ReferenceIndexError::Corrupt("bundle member set"));
        }
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| std::io::Error::other("non-UTF-8 bundle member"))?;
        names.insert(name);
    }
    if names != EXACT_MEMBERS.into_iter().map(str::to_owned).collect() {
        return Err(ReferenceIndexError::Corrupt("bundle member set"));
    }
    let manifest_bytes = read_bounded(&bundle.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    let notice = read_bounded(&bundle.join("NOTICE"), MAX_NOTICE_BYTES)?;
    let manifest: ReferenceManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| ReferenceIndexError::Corrupt("manifest JSON"))?;
    if canonical_reference_manifest_bytes(&manifest)? != manifest_bytes
        || manifest.schema != "pangopup.reference.bundle.v1"
        || manifest.reference_format != "pangopup.reference.acgt2-rle.v1"
        || manifest.profile.is_empty()
        || manifest.source.assembly.is_empty()
        || manifest.source.assembly_accession.is_empty()
        || !valid_sha(&manifest.sequences.sequence_set_sha256)
        || manifest.members.len() != 2
        || manifest.members[0].path != "NOTICE"
        || manifest.members[0].size != notice.len() as u64
        || manifest.members[0].sha256 != format!("sha256:{:x}", Sha256::digest(&notice))
        || manifest.members[1].path != "reference.pgr"
        || manifest.members[1].size == 0
        || !valid_sha(&manifest.members[1].sha256)
    {
        return Err(ReferenceIndexError::Corrupt("manifest admission"));
    }
    let reference_metadata = fs::symlink_metadata(bundle.join("reference.pgr"))?;
    if !reference_metadata.file_type().is_file()
        || reference_metadata.file_type().is_symlink()
        || reference_metadata.nlink() != 1
        || reference_metadata.len() != manifest.members[1].size
    {
        return Err(ReferenceIndexError::Corrupt("reference admission member"));
    }
    Ok(ReferenceAdmission {
        bundle_id: reference_bundle_id(&manifest_bytes),
        profile: manifest.profile,
        format: manifest.reference_format,
        assembly: manifest.source.assembly,
        assembly_accession: manifest.source.assembly_accession,
        sequence_set_sha256: manifest.sequences.sequence_set_sha256,
    })
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ReferenceIndexError> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| {
        ReferenceIndexError::Io(std::io::Error::from_raw_os_error(error.raw_os_error()))
    })?;
    let file = File::from(descriptor);
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(ReferenceIndexError::Corrupt("bounded admission member"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(ReferenceIndexError::Corrupt("admission member length"));
    }
    Ok(bytes)
}

fn valid_sha(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}
