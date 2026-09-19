//! Trusted assembly of an already-built sparse SNV member.

use crate::{CommandError, source_fingerprint::sparse_assembler_source_sha256};
use pangopup_assets::{
    NOTICE, certify_bundle, certify_bundle_members, inspect_transport, unpack_transport,
    verify_transport,
};
use pangopup_index::{
    BundleManifest, MemberManifest, SparseProvenanceManifest, bundle_id, canonical_manifest_bytes,
    sparse_writer::SPARSE_INDEX_FORMAT,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const PRODUCTION_V1_AUTHORITY_BUNDLE_ID: &str =
    "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3";
const PRODUCTION_V1_AUTHORITY_MANIFEST: &[u8] =
    include_bytes!("../../../release-profiles/proofs/snv-grch38-v1-bundle-manifest.json");

#[derive(Clone, Copy)]
struct BuildProvenance<'a> {
    commit: &'a str,
    clean: &'a str,
}

#[derive(Clone, Debug)]
pub struct SparseBundleArguments {
    pub authority_bundle: PathBuf,
    pub sparse_member: PathBuf,
    pub candidate_commit: String,
    pub output: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SparseBundleOutcome {
    pub status: &'static str,
    pub bundle_id: String,
    pub authority_bundle_id: String,
    pub candidate_commit: String,
    pub assembler_source_sha256: String,
    pub score_bytes: u64,
    pub score_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SparseReleaseOutcome {
    pub status: &'static str,
    pub repository: &'static str,
    pub tag: &'static str,
    pub transport_id: String,
    pub bundle_id: String,
    pub asset_count: usize,
}

/// Assemble against the sole checked production corpus authority.
pub fn assemble_sparse_bundle(
    arguments: &SparseBundleArguments,
) -> Result<SparseBundleOutcome, CommandError> {
    assemble_sparse_bundle_with_authority(arguments, PRODUCTION_V1_AUTHORITY_BUNDLE_ID)
}

/// Derive the v2 proof and profile only after full transport reconstruction and
/// exhaustive sparse bundle certification. This function creates local upload
/// metadata. It does not publish or activate anything.
pub fn prepare_sparse_release(
    transport: &Path,
    tooling_commit: &str,
    release_target_commit: &str,
    output: &Path,
) -> Result<SparseReleaseOutcome, CommandError> {
    let authority = production_authority()?;
    prepare_sparse_release_against(
        transport,
        tooling_commit,
        release_target_commit,
        output,
        &authority,
        BuildProvenance {
            commit: env!("PANGOPUP_GIT_COMMIT"),
            clean: env!("PANGOPUP_GIT_CLEAN"),
        },
    )
}

fn prepare_sparse_release_against(
    transport: &Path,
    tooling_commit: &str,
    release_target_commit: &str,
    output: &Path,
    authority: &BundleManifest,
    build: BuildProvenance<'_>,
) -> Result<SparseReleaseOutcome, CommandError> {
    if !valid_commit(tooling_commit) {
        return Err(fail(
            "SPARSE_RELEASE_COMMIT",
            "tooling commit must be 40 lowercase hexadecimal characters",
        ));
    }
    if !valid_commit(release_target_commit) {
        return Err(fail(
            "SPARSE_RELEASE_COMMIT",
            "release target commit must be 40 lowercase hexadecimal characters",
        ));
    }
    let implementation_commit = validate_build_provenance(tooling_commit, build)?;
    let expected_authority = canonical_manifest_bytes(authority)
        .map(|bytes| bundle_id(&bytes))
        .map_err(|error| fail("SPARSE_AUTHORITY", error))?;
    ensure_absent(output)?;
    let (stage, mut guard) = create_stage(output)?;
    let prepared = (|| {
        let verified = verify_transport(transport).map_err(asset_error)?;
        let reconstructed = stage.join("verified-bundle");
        let unpacked = unpack_transport(transport, &reconstructed).map_err(asset_error)?;
        if verified.bundle_id != unpacked.bundle_id
            || verified.transport_id != unpacked.transport_id
        {
            return Err(fail(
                "SPARSE_RELEASE_VERIFY",
                "transport identity changed during complete verification",
            ));
        }
        let certified = certify_bundle(&reconstructed).map_err(asset_error)?;
        if certified.bundle_id != unpacked.bundle_id {
            return Err(fail(
                "SPARSE_RELEASE_VERIFY",
                "certified bundle identity differs from transport",
            ));
        }
        let manifest_bytes = fs::read(reconstructed.join("manifest.json"))
            .map_err(|error| io_error("read certified sparse manifest", error))?;
        let manifest = pangopup_index::parse_bundle_manifest_bytes(&manifest_bytes)
            .map_err(|error| fail("SPARSE_RELEASE_VERIFY", error))?;
        if manifest.index_format != SPARSE_INDEX_FORMAT {
            return Err(fail(
                "SPARSE_RELEASE_VERIFY",
                "release preparation requires sparse-direct-v1",
            ));
        }
        validate_sparse_authority(&manifest, authority, &expected_authority)?;
        let provenance = manifest
            .sparse_provenance
            .as_ref()
            .expect("authority validation requires sparse provenance");
        let inspection = inspect_transport(transport).map_err(asset_error)?;
        if inspection.transport_id != verified.transport_id
            || inspection.bundle_id != certified.bundle_id
        {
            return Err(fail(
                "SPARSE_RELEASE_VERIFY",
                "transport changed after complete verification",
            ));
        }

        let installed_members = vec![
            serde_json::json!({"path":"manifest.json","size":inspection.bundle_manifest_size}),
            serde_json::json!({"path":"NOTICE","size":inspection.notice_size}),
            serde_json::json!({"path":"scores.pgi","size":inspection.score_size}),
        ];
        let installed_bytes = inspection
            .bundle_manifest_size
            .checked_add(inspection.notice_size)
            .and_then(|value| value.checked_add(inspection.score_size))
            .ok_or_else(|| fail("SPARSE_RELEASE_SIZE", "installed byte sum overflow"))?;
        let mut download_members = vec![
            serde_json::json!({
                "path":"transport.json",
                "size":inspection.transport_bytes.len() as u64
            }),
            serde_json::json!({
                "path":"bundle-manifest.json",
                "size":inspection.bundle_manifest_size
            }),
            serde_json::json!({
                "path":"NOTICE",
                "size":inspection.notice_size
            }),
        ];
        download_members.extend(
            inspection
                .parts
                .iter()
                .map(|part| serde_json::json!({"path":part.path,"size":part.size})),
        );
        let download_bytes = (inspection.transport_bytes.len() as u64)
            .checked_add(inspection.bundle_manifest_size)
            .and_then(|value| value.checked_add(inspection.notice_size))
            .and_then(|value| value.checked_add(inspection.compressed_size))
            .ok_or_else(|| fail("SPARSE_RELEASE_SIZE", "download byte sum overflow"))?;
        let fresh_install_bytes = installed_bytes
            .checked_add(download_bytes)
            .ok_or_else(|| fail("SPARSE_RELEASE_SIZE", "fresh-install byte sum overflow"))?;
        let mut fresh_install_members = installed_members
            .iter()
            .map(|member| {
                serde_json::json!({
                    "path":format!("installed/{}", member["path"].as_str().expect("member path")),
                    "size":member["size"]
                })
            })
            .collect::<Vec<_>>();
        fresh_install_members.extend(download_members.iter().map(|member| {
            serde_json::json!({
                "path":format!("transport/{}", member["path"].as_str().expect("member path")),
                "size":member["size"]
            })
        }));
        let parts: Vec<_> = inspection
            .parts
            .iter()
            .map(|part| {
                serde_json::json!({
                    "ordinal":part.ordinal,
                    "path":part.path,
                    "size":part.size,
                    "sha256":part.sha256
                })
            })
            .collect();
        let members = manifest
            .members
            .iter()
            .map(|member| {
                serde_json::json!({
                    "path":member.path,
                    "size":member.size,
                    "sha256":member.sha256
                })
            })
            .collect::<Vec<_>>();
        let proof_value = serde_json::json!({
            "schema":"pangopup.sparse-proof-receipt.v1",
            "authority":{"bundle_id":expected_authority},
            "source":manifest.source,
            "reference":manifest.reference,
            "counts":manifest.counts,
            "logical_source":manifest.logical_source,
            "logical_decoded":manifest.logical_decoded,
            "bundle":{
                "schema":manifest.schema,
                "index_format":manifest.index_format,
                "bundle_id":certified.bundle_id,
                "builder_version":manifest.builder.version,
                "assembler_source_sha256":manifest.builder.source_sha256,
                "candidate_commit":provenance.candidate_commit,
                "manifest":{"size":inspection.bundle_manifest_size,"sha256":inspection.bundle_manifest_sha256},
                "members":members
            },
            "transport":{
                "schema":"pangopup.snv-transport.v1",
                "transport_id":inspection.transport_id,
                "manifest":{"size":inspection.transport_bytes.len() as u64,"sha256":inspection.transport_sha256},
                "compressed":{"size":inspection.compressed_size,"sha256":inspection.compressed_sha256},
                "compression":{
                    "format":inspection.compression.format.clone(),
                    "level":inspection.compression.level,
                    "checksum":inspection.compression.checksum,
                    "content_size":inspection.compression.content_size,
                    "dictionary":inspection.compression.dictionary,
                    "workers":inspection.compression.workers,
                    "encoder_crate":inspection.compression.encoder_crate.clone(),
                    "libzstd_version":inspection.compression.libzstd_version.clone()
                },
                "parts":parts
            },
            "tool":{"implementation_commit":implementation_commit},
            "sizing":{
                "snv_installed_members":installed_members,
                "snv_installed_bytes":installed_bytes,
                "fresh_install_members":fresh_install_members,
                "fresh_install_bytes":fresh_install_bytes
            }
        });
        let proof =
            serde_jcs::to_vec(&proof_value).map_err(|error| fail("SPARSE_RELEASE_JSON", error))?;
        let proof_sha256 = hash_bytes(&proof);
        let tag = "snv-grch38-v2";
        let repository = "genomoncology/pangopup";
        let prefix = format!("https://github.com/{repository}/releases/download/{tag}/");
        let mut profile_members = vec![
            profile_member(
                "transport.json",
                inspection.transport_bytes.len() as u64,
                &inspection.transport_sha256,
                &prefix,
            ),
            profile_member(
                "bundle-manifest.json",
                inspection.bundle_manifest_size,
                &inspection.bundle_manifest_sha256,
                &prefix,
            ),
            profile_member(
                "NOTICE",
                inspection.notice_size,
                &inspection.notice_sha256,
                &prefix,
            ),
        ];
        profile_members.extend(
            inspection
                .parts
                .iter()
                .map(|part| profile_member(&part.path, part.size, &part.sha256, &prefix)),
        );
        let profile_value = serde_json::json!({
            "schema":"pangopup.release-profile.v2",
            "profile":tag,
            "repository":repository,
            "release":{
                "tag":tag,
                "title":"Pangopup GRCh38 sparse SNV scores v2",
                "target_commit":release_target_commit,
                "page_url":format!("https://github.com/{repository}/releases/tag/{tag}")
            },
            "authority":{"bundle_id":expected_authority},
            "source":manifest.source,
            "reference":manifest.reference,
            "bundle":{
                "schema":manifest.schema,
                "index_format":manifest.index_format,
                "bundle_id":certified.bundle_id
            },
            "transport":{
                "schema":"pangopup.snv-transport.v1",
                "transport_id":inspection.transport_id,
                "members":profile_members
            },
            "proof":{
                "schema":"pangopup.sparse-proof-receipt.v1",
                "asset_name":"proof-receipt.json",
                "size":proof.len() as u64,
                "sha256":proof_sha256
            }
        });
        let profile = serde_jcs::to_vec(&profile_value)
            .map_err(|error| fail("SPARSE_RELEASE_JSON", error))?;
        let mut sum_members = vec![
            (
                inspection.transport_sha256.clone(),
                "transport.json".to_owned(),
            ),
            (
                inspection.bundle_manifest_sha256.clone(),
                "bundle-manifest.json".to_owned(),
            ),
            (inspection.notice_sha256.clone(), "NOTICE".to_owned()),
        ];
        sum_members.extend(
            inspection
                .parts
                .iter()
                .map(|part| (part.sha256.clone(), part.path.clone())),
        );
        sum_members.push((hash_bytes(&proof), "proof-receipt.json".to_owned()));
        sum_members.push((hash_bytes(&profile), "release-profile.json".to_owned()));
        let sums: String = sum_members
            .into_iter()
            .map(|(digest, name)| {
                format!(
                    "{}  {name}\n",
                    digest.strip_prefix("sha256:").expect("validated digest")
                )
            })
            .collect();
        let notes = format!(
            "# Pangopup GRCh38 sparse SNV scores v2\n\nCorpus authority: `{}`\n\nSparse bundle: `{}`\n\nTransport: `{}`\n\nCandidate-producing commit: `{}`\n\nRelease tooling commit: `{}`\n\nRelease target commit: `{}`\n\nSNV-only installed bytes: {} = manifest.json {} + NOTICE {} + scores.pgi {}.\n\nFresh-install bytes: {} = installed members {} + transport.json {} + bundle-manifest.json {} + transport NOTICE {} + compressed payload parts {}.\n",
            expected_authority,
            certified.bundle_id,
            inspection.transport_id,
            provenance.candidate_commit,
            implementation_commit,
            release_target_commit,
            installed_bytes,
            inspection.bundle_manifest_size,
            inspection.notice_size,
            inspection.score_size,
            fresh_install_bytes,
            installed_bytes,
            inspection.transport_bytes.len(),
            inspection.bundle_manifest_size,
            inspection.notice_size,
            inspection.compressed_size
        );
        guard.remove_child_directory("verified-bundle")?;
        write_synced(&stage.join("proof-receipt.json"), &proof)?;
        write_synced(&stage.join("release-profile.json"), &profile)?;
        write_synced(&stage.join("SHA256SUMS"), sums.as_bytes())?;
        write_synced(&stage.join("release-notes.md"), notes.as_bytes())?;
        sync_directory(&stage)?;
        publish_stage(output, &mut guard)?;
        Ok(SparseReleaseOutcome {
            status: "prepared",
            repository,
            tag,
            transport_id: inspection.transport_id,
            bundle_id: certified.bundle_id,
            asset_count: inspection.parts.len() + 6,
        })
    })();
    finish_staged(prepared, &mut guard)
}

fn profile_member(name: &str, size: u64, sha256: &str, prefix: &str) -> serde_json::Value {
    serde_json::json!({
        "logical_path":name,
        "asset_name":name,
        "size":size,
        "sha256":sha256,
        "url":format!("{prefix}{name}")
    })
}

fn production_authority() -> Result<BundleManifest, CommandError> {
    if bundle_id(PRODUCTION_V1_AUTHORITY_MANIFEST) != PRODUCTION_V1_AUTHORITY_BUNDLE_ID {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "checked v1 authority manifest identity is invalid",
        ));
    }
    let manifest = pangopup_index::parse_bundle_manifest_bytes(PRODUCTION_V1_AUTHORITY_MANIFEST)
        .map_err(|error| fail("SPARSE_AUTHORITY", error))?;
    if manifest.index_format != pangopup_index::INDEX_FORMAT {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "checked v1 authority manifest is not fixed-v1",
        ));
    }
    Ok(manifest)
}

fn validate_inherited_authority(
    sparse: &BundleManifest,
    authority: &BundleManifest,
) -> Result<(), CommandError> {
    let provenance = sparse
        .sparse_provenance
        .as_ref()
        .ok_or_else(|| fail("SPARSE_RELEASE_VERIFY", "sparse provenance is missing"))?;
    if sparse.attribution != authority.attribution
        || sparse.source != authority.source
        || sparse.reference != authority.reference
        || sparse.counts != authority.counts
        || sparse.logical_source != authority.logical_source
        || sparse.logical_decoded != authority.logical_decoded
        || provenance.corpus_authority_builder != authority.builder
    {
        return Err(fail(
            "SPARSE_RELEASE_AUTHORITY",
            "sparse inherited facts differ from the authenticated v1 authority",
        ));
    }
    Ok(())
}

fn validate_sparse_authority(
    sparse: &BundleManifest,
    authority: &BundleManifest,
    expected_authority: &str,
) -> Result<(), CommandError> {
    let provenance = sparse
        .sparse_provenance
        .as_ref()
        .ok_or_else(|| fail("SPARSE_RELEASE_VERIFY", "sparse provenance is missing"))?;
    if provenance.corpus_authority_bundle_id != expected_authority {
        return Err(fail(
            "SPARSE_RELEASE_AUTHORITY",
            "sparse bundle does not name the authenticated v1 authority",
        ));
    }
    validate_inherited_authority(sparse, authority)
}

fn validate_build_provenance(
    supplied: &str,
    compiled: BuildProvenance<'_>,
) -> Result<String, CommandError> {
    if !valid_commit(compiled.commit) {
        return Err(fail(
            "SPARSE_RELEASE_BUILD",
            "compiled Git commit is unavailable or invalid",
        ));
    }
    match compiled.clean {
        "true" => {}
        "false" => {
            return Err(fail(
                "SPARSE_RELEASE_BUILD",
                "release tooling was compiled from a dirty checkout",
            ));
        }
        _ => {
            return Err(fail(
                "SPARSE_RELEASE_BUILD",
                "compiled Git cleanliness is unavailable",
            ));
        }
    }
    if supplied != compiled.commit {
        return Err(fail(
            "SPARSE_RELEASE_BUILD",
            "tooling commit differs from the compiled Git commit",
        ));
    }
    Ok(compiled.commit.to_owned())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn assemble_sparse_bundle_with_authority(
    arguments: &SparseBundleArguments,
    expected_authority: &str,
) -> Result<SparseBundleOutcome, CommandError> {
    if !valid_sha256(expected_authority) {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "invalid checked authority identity",
        ));
    }
    if !valid_commit(&arguments.candidate_commit) {
        return Err(fail(
            "SPARSE_CANDIDATE_COMMIT",
            "candidate commit must be 40 lowercase hexadecimal characters",
        ));
    }
    ensure_absent(&arguments.output)?;
    validate_bundle_members(&arguments.authority_bundle)?;
    let authority_manifest = open_regular(
        &arguments.authority_bundle.join("manifest.json"),
        "authority manifest",
    )?;
    let authority_notice = open_regular(
        &arguments.authority_bundle.join("NOTICE"),
        "authority NOTICE",
    )?;
    let authority_scores = open_regular(
        &arguments.authority_bundle.join("scores.pgi"),
        "authority scores",
    )?;
    let authority_identities = [
        identity(&authority_manifest)?,
        identity(&authority_notice)?,
        identity(&authority_scores)?,
    ];
    let authority =
        certify_bundle_members(&authority_manifest, &authority_notice, &authority_scores)
            .map_err(asset_error)?;
    if authority.certification().bundle_id != expected_authority {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "fixed-v1 bundle does not match the checked corpus authority",
        ));
    }
    if authority.index_format() != pangopup_index::INDEX_FORMAT {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "corpus authority must use fixed-v1",
        ));
    }

    let sparse = open_regular(&arguments.sparse_member, "sparse member")?;
    let sparse_identity = identity(&sparse)?;
    let score_bytes = sparse_identity.len;
    if score_bytes > pangopup_assets::MAX_SPARSE_DIRECT_BYTES {
        return Err(fail(
            "SPARSE_MEMBER",
            "sparse member exceeds the sparse-direct-v1 ceiling",
        ));
    }
    let score_sha256 = hash_file(&sparse)?;

    let (stage, mut guard) = create_stage(&arguments.output)?;
    let assembled = (|| {
        write_synced(&stage.join("NOTICE"), NOTICE)?;
        copy_held(
            &sparse,
            &stage.join("scores.pgi"),
            score_bytes,
            &score_sha256,
        )?;
        let mut manifest = authority.manifest().clone();
        let authority_builder = manifest.builder.clone();
        manifest.index_format = SPARSE_INDEX_FORMAT.to_owned();
        manifest.builder.version = env!("CARGO_PKG_VERSION").to_owned();
        manifest.builder.source_sha256 = format!("sha256:{}", sparse_assembler_source_sha256());
        manifest.sparse_provenance = Some(SparseProvenanceManifest {
            corpus_authority_bundle_id: expected_authority.to_owned(),
            corpus_authority_builder: authority_builder,
            candidate_commit: arguments.candidate_commit.clone(),
        });
        manifest.members = vec![
            MemberManifest {
                path: "NOTICE".to_owned(),
                size: NOTICE.len() as u64,
                sha256: pangopup_assets::NOTICE_SHA256.to_owned(),
                media_type: "text/plain; charset=utf-8".to_owned(),
            },
            MemberManifest {
                path: "scores.pgi".to_owned(),
                size: score_bytes,
                sha256: score_sha256.clone(),
                media_type: "application/vnd.pangopup.sparse-direct".to_owned(),
            },
        ];
        let manifest_bytes = canonical_manifest_bytes(&manifest)
            .map_err(|error| fail("SPARSE_MANIFEST", error.to_string()))?;
        write_synced(&stage.join("manifest.json"), &manifest_bytes)?;
        sync_directory(&stage)?;
        let certified = certify_bundle(&stage).map_err(asset_error)?;
        for ((name, path), expected) in [
            (
                "authority manifest",
                arguments.authority_bundle.join("manifest.json"),
            ),
            (
                "authority NOTICE",
                arguments.authority_bundle.join("NOTICE"),
            ),
            (
                "authority scores",
                arguments.authority_bundle.join("scores.pgi"),
            ),
        ]
        .into_iter()
        .zip(authority_identities)
        {
            if !same_identity(&path, expected, name)? {
                return Err(fail(
                    "SPARSE_AUTHORITY",
                    format!("{name} path changed during assembly"),
                ));
            }
        }
        if !same_identity(&arguments.sparse_member, sparse_identity, "sparse member")? {
            return Err(fail(
                "SPARSE_MEMBER",
                "sparse member path changed during assembly",
            ));
        }
        publish_stage(&arguments.output, &mut guard)?;
        Ok(SparseBundleOutcome {
            status: "assembled",
            bundle_id: certified.bundle_id,
            authority_bundle_id: expected_authority.to_owned(),
            candidate_commit: arguments.candidate_commit.clone(),
            assembler_source_sha256: manifest.builder.source_sha256,
            score_bytes,
            score_sha256,
        })
    })();
    finish_staged(assembled, &mut guard)
}

fn validate_bundle_members(bundle: &Path) -> Result<(), CommandError> {
    let expected = BTreeSet::from([
        "NOTICE".to_owned(),
        "manifest.json".to_owned(),
        "scores.pgi".to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(bundle).map_err(|error| io_error("read authority bundle", error))? {
        let entry = entry.map_err(|error| io_error("read authority member", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| fail("SPARSE_AUTHORITY", "authority member name is not UTF-8"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| io_error("inspect authority member", error))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(fail(
                "SPARSE_AUTHORITY",
                "authority members must be regular files",
            ));
        }
        actual.insert(name);
    }
    if actual != expected {
        return Err(fail(
            "SPARSE_AUTHORITY",
            "authority bundle member set mismatch",
        ));
    }
    Ok(())
}

fn open_regular(path: &Path, label: &str) -> Result<File, CommandError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io_error(&format!("inspect {label}"), error))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(fail(
            "SPARSE_INPUT",
            format!("{label} must be a regular file"),
        ));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = File::open(path);
    file.map_err(|error| io_error(&format!("open {label}"), error))
}

#[derive(Clone, Copy)]
struct FileIdentity {
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

fn identity(file: &File) -> Result<FileIdentity, CommandError> {
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect held input", error))?;
    if !metadata.file_type().is_file() {
        return Err(fail("SPARSE_INPUT", "input is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            len: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    Ok(FileIdentity {
        len: metadata.len(),
    })
}

fn same_identity(path: &Path, expected: FileIdentity, label: &str) -> Result<bool, CommandError> {
    let file = match open_regular(path, label) {
        Ok(file) => file,
        Err(_) => return Ok(false),
    };
    let actual = identity(&file)?;
    #[cfg(unix)]
    return Ok(actual.len == expected.len
        && actual.device == expected.device
        && actual.inode == expected.inode);
    #[cfg(not(unix))]
    Ok(actual.len == expected.len)
}

fn hash_file(file: &File) -> Result<String, CommandError> {
    let mut input = file
        .try_clone()
        .map_err(|error| io_error("clone held sparse member", error))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error("rewind held sparse member", error))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| io_error("hash held sparse member", error))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn copy_held(
    input: &File,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), CommandError> {
    let mut input = input
        .try_clone()
        .map_err(|error| io_error("clone held sparse member", error))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error("rewind held sparse member", error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error("create staged sparse member", error))?;
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| io_error("read held sparse member", error))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| io_error("write staged sparse member", error))?;
        hash.update(&buffer[..read]);
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| fail("SPARSE_MEMBER", "sparse member size overflow"))?;
    }
    output
        .sync_all()
        .map_err(|error| io_error("sync staged sparse member", error))?;
    if total != expected_size || format!("sha256:{:x}", hash.finalize()) != expected_sha256 {
        return Err(fail(
            "SPARSE_MEMBER",
            "sparse member changed while it was copied",
        ));
    }
    Ok(())
}

struct StageGuard {
    path: PathBuf,
    directory: File,
    identity: DirectoryIdentity,
    armed: bool,
}

impl StageGuard {
    fn owns_named_path(&self) -> Result<bool, CommandError> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(io_error("inspect sparse stage path", error)),
        };
        Ok(metadata.file_type().is_dir() && self.identity.matches(&metadata))
    }

    fn require_owned(&self) -> Result<(), CommandError> {
        if !self.owns_named_path()? {
            return Err(fail(
                "SPARSE_STAGE_IDENTITY",
                "sparse stage path was replaced",
            ));
        }
        Ok(())
    }

    fn remove_child_directory(&self, name: &str) -> Result<(), CommandError> {
        remove_child_directory(&self.directory, name)
    }

    fn cleanup(&mut self) -> Result<(), CommandError> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        remove_directory_contents(&self.directory)?;
        if !self.owns_named_path()? {
            return Err(fail(
                "SPARSE_STAGE_IDENTITY",
                "sparse stage path was replaced before cleanup",
            ));
        }
        fs::remove_dir(&self.path).map_err(|error| io_error("remove sparse bundle stage", error))
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.cleanup();
        }
    }
}

#[derive(Clone, Copy)]
struct DirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl DirectoryIdentity {
    fn from_file(file: &File) -> Result<Self, CommandError> {
        let metadata = file
            .metadata()
            .map_err(|error| io_error("inspect held sparse stage", error))?;
        if !metadata.file_type().is_dir() {
            return Err(fail(
                "SPARSE_STAGE_IDENTITY",
                "sparse stage is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(not(unix))]
        Ok(Self {})
    }

    fn matches(self, metadata: &fs::Metadata) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            metadata.dev() == self.device && metadata.ino() == self.inode
        }
        #[cfg(not(unix))]
        {
            metadata.file_type().is_dir()
        }
    }
}

fn create_stage(output: &Path) -> Result<(PathBuf, StageGuard), CommandError> {
    let parent = usable_parent(output)?;
    fs::create_dir_all(parent).map_err(|error| io_error("create output parent", error))?;
    let name = output
        .file_name()
        .ok_or_else(|| fail("SPARSE_OUTPUT", "output has no file name"))?
        .to_string_lossy();
    for serial in 0..128_u32 {
        let path = parent.join(format!(
            ".{name}.pangopup-stage-{}-{serial}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                let directory = {
                    use std::os::unix::fs::OpenOptionsExt;
                    OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                        .open(&path)
                };
                #[cfg(not(unix))]
                let directory = File::open(&path);
                let directory = match directory {
                    Ok(directory) => directory,
                    Err(error) => {
                        let _ = fs::remove_dir(&path);
                        return Err(io_error("open sparse bundle stage", error));
                    }
                };
                let identity = DirectoryIdentity::from_file(&directory)?;
                let mut guard = StageGuard {
                    path: path.clone(),
                    directory,
                    identity,
                    armed: true,
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(error) = guard
                        .directory
                        .set_permissions(fs::Permissions::from_mode(0o700))
                    {
                        let failure = io_error("make sparse bundle stage private", error);
                        return match guard.cleanup() {
                            Ok(()) => Err(failure),
                            Err(cleanup) => Err(cleanup),
                        };
                    }
                }
                return Ok((path, guard));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error("create sparse bundle stage", error)),
        }
    }
    Err(fail("SPARSE_OUTPUT", "could not create a unique stage"))
}

fn publish_stage(output: &Path, guard: &mut StageGuard) -> Result<(), CommandError> {
    guard.require_owned()?;
    rename_checked_stage(output, guard)
}

#[cfg(test)]
fn publish_stage_with_hook(
    output: &Path,
    guard: &mut StageGuard,
    after_ownership_check: impl FnOnce(),
) -> Result<(), CommandError> {
    guard.require_owned()?;
    after_ownership_check();
    rename_checked_stage(output, guard)
}

fn rename_checked_stage(output: &Path, guard: &mut StageGuard) -> Result<(), CommandError> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        &guard.path,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|error| {
        if matches!(
            error.kind(),
            ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
        ) {
            fail("SPARSE_OUTPUT_CONFLICT", "output already exists")
        } else {
            io_error("publish sparse bundle", error)
        }
    })?;
    let published = fs::symlink_metadata(output)
        .map_err(|error| io_error("inspect published sparse bundle", error))?;
    if !published.file_type().is_dir() || !guard.identity.matches(&published) {
        return Err(fail(
            "SPARSE_STAGE_IDENTITY",
            "published sparse bundle differs from the held stage",
        ));
    }
    guard.armed = false;
    sync_directory(usable_parent(output)?)
}

fn usable_parent(path: &Path) -> Result<&Path, CommandError> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(fail("SPARSE_OUTPUT", "output has no parent")),
    }
}

#[cfg(unix)]
fn remove_directory_contents(directory: &File) -> Result<(), CommandError> {
    remove_directory_contents_fd(directory)
}

#[cfg(unix)]
fn remove_directory_contents_fd<F: std::os::fd::AsFd>(directory: &F) -> Result<(), CommandError> {
    use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};
    use std::ffi::CString;

    let entries = Dir::read_from(directory)
        .map_err(|error| io_error("read held sparse stage child", error.into()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| io_error("read sparse stage child entry", error.into()))?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            names.push(CString::new(name.to_bytes()).expect("directory entry excludes NUL"));
        }
    }
    for name in names {
        let stat = rustix::fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| io_error("inspect sparse stage child entry", error.into()))?;
        if FileType::from_raw_mode(stat.st_mode).is_dir() {
            let child = rustix::fs::openat(
                directory,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| io_error("open nested sparse stage child", error.into()))?;
            let opened = rustix::fs::fstat(&child)
                .map_err(|error| io_error("inspect nested sparse stage child", error.into()))?;
            if stat.st_dev != opened.st_dev || stat.st_ino != opened.st_ino {
                return Err(fail(
                    "SPARSE_STAGE_IDENTITY",
                    "nested sparse stage child changed while it was opened",
                ));
            }
            remove_directory_contents_fd(&child)?;
            let final_stat = rustix::fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| io_error("reinspect nested sparse stage child", error.into()))?;
            if opened.st_dev != final_stat.st_dev || opened.st_ino != final_stat.st_ino {
                return Err(fail(
                    "SPARSE_STAGE_IDENTITY",
                    "nested sparse stage child was replaced during cleanup",
                ));
            }
            rustix::fs::unlinkat(directory, &name, AtFlags::REMOVEDIR)
                .map_err(|error| io_error("remove nested sparse stage child", error.into()))?;
        } else {
            rustix::fs::unlinkat(directory, &name, AtFlags::empty())
                .map_err(|error| io_error("remove nested sparse stage member", error.into()))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn remove_child_directory(directory: &File, name: &str) -> Result<(), CommandError> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};

    let stat = rustix::fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| io_error("inspect verified sparse bundle", error.into()))?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir() {
        return Err(fail(
            "SPARSE_STAGE_IDENTITY",
            "verified sparse bundle path is not a directory",
        ));
    }
    let child = rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| io_error("open verified sparse bundle", error.into()))?;
    let opened = rustix::fs::fstat(&child)
        .map_err(|error| io_error("inspect held verified sparse bundle", error.into()))?;
    if stat.st_dev != opened.st_dev || stat.st_ino != opened.st_ino {
        return Err(fail(
            "SPARSE_STAGE_IDENTITY",
            "verified sparse bundle changed while it was opened",
        ));
    }
    remove_directory_contents_fd(&child)?;
    let final_stat = rustix::fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| io_error("reinspect verified sparse bundle", error.into()))?;
    if opened.st_dev != final_stat.st_dev || opened.st_ino != final_stat.st_ino {
        return Err(fail(
            "SPARSE_STAGE_IDENTITY",
            "verified sparse bundle was replaced during cleanup",
        ));
    }
    rustix::fs::unlinkat(directory, name, AtFlags::REMOVEDIR)
        .map_err(|error| io_error("remove verified sparse bundle", error.into()))
}

#[cfg(not(unix))]
fn remove_directory_contents(_directory: &File) -> Result<(), CommandError> {
    Err(fail(
        "SPARSE_STAGE_IDENTITY",
        "safe sparse stage cleanup is unsupported on this platform",
    ))
}

#[cfg(not(unix))]
fn remove_child_directory(_directory: &File, _name: &str) -> Result<(), CommandError> {
    Err(fail(
        "SPARSE_STAGE_IDENTITY",
        "safe sparse child cleanup is unsupported on this platform",
    ))
}

fn finish_staged<T>(
    result: Result<T, CommandError>,
    guard: &mut StageGuard,
) -> Result<T, CommandError> {
    let cleanup = guard.cleanup();
    match (result, cleanup) {
        (_, Err(error)) => Err(error),
        (result, Ok(())) => result,
    }
}

fn ensure_absent(path: &Path) -> Result<(), CommandError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(fail("SPARSE_OUTPUT_CONFLICT", "output already exists")),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect sparse bundle output", error)),
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create sparse bundle member", error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| io_error("write sparse bundle member", error))
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync sparse bundle directory", error))
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn asset_error(error: pangopup_assets::AssetError) -> CommandError {
    fail(
        error.legacy_build_code().unwrap_or(error.kind().code()),
        error,
    )
}

fn io_error(action: &str, error: io::Error) -> CommandError {
    fail("SPARSE_IO", format!("{action}: {error}"))
}

fn fail(code: &'static str, message: impl ToString) -> CommandError {
    CommandError::new(code, message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SparseCandidateArguments, build_sparse_candidate, verify_bundle};
    use std::sync::Mutex;
    use tempfile::TempDir;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn copy_bundle(source: &Path, destination: &Path) {
        fs::create_dir(destination).expect("bundle destination");
        for member in ["NOTICE", "manifest.json", "scores.pgi"] {
            fs::copy(source.join(member), destination.join(member)).expect("copy bundle member");
        }
    }

    #[test]
    fn miniature_assembly_is_deterministic_certified_and_authority_bound() {
        let temp = TempDir::new().expect("temporary directory");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle");
        let authority = temp.path().join("authority");
        copy_bundle(&fixture, &authority);
        let authority_id = verify_bundle(&authority).expect("authority").bundle_id;
        let sparse = temp.path().join("candidate.pgi");
        build_sparse_candidate(&SparseCandidateArguments {
            fixed_bundle: authority.clone(),
            scratch: temp.path().join("scratch"),
            candidate: sparse.clone(),
            report: temp.path().join("candidate.json"),
            expected_bundle_id: Some(authority_id.clone()),
        })
        .expect("candidate");

        let arguments = |name: &str| SparseBundleArguments {
            authority_bundle: authority.clone(),
            sparse_member: sparse.clone(),
            candidate_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            output: temp.path().join(name),
        };
        let first = arguments("first");
        let second = arguments("second");
        let first_outcome =
            assemble_sparse_bundle_with_authority(&first, &authority_id).expect("first assembly");
        let second_outcome =
            assemble_sparse_bundle_with_authority(&second, &authority_id).expect("second assembly");
        assert_eq!(first_outcome, second_outcome);
        for member in ["NOTICE", "manifest.json", "scores.pgi"] {
            assert_eq!(
                fs::read(first.output.join(member)).expect("first member"),
                fs::read(second.output.join(member)).expect("second member")
            );
        }
        let certified = certify_bundle(&first.output).expect("certified sparse bundle");
        assert_eq!(certified.bundle_id, first_outcome.bundle_id);
        assert_ne!(certified.bundle_id, authority_id);
        let manifest = pangopup_index::parse_bundle_manifest_bytes(
            &fs::read(first.output.join("manifest.json")).expect("manifest"),
        )
        .expect("sparse manifest");
        assert_eq!(manifest.source, verify_bundle_manifest(&authority).source);
        assert_eq!(
            manifest.reference,
            verify_bundle_manifest(&authority).reference
        );
        assert_eq!(manifest.counts, verify_bundle_manifest(&authority).counts);
        assert_eq!(manifest.builder.version, env!("CARGO_PKG_VERSION"));
        let authority_manifest = verify_bundle_manifest(&authority);
        let provenance = manifest.sparse_provenance.expect("sparse provenance");
        assert_eq!(provenance.corpus_authority_bundle_id, authority_id);
        assert_eq!(
            provenance.corpus_authority_builder,
            authority_manifest.builder
        );

        let transport = temp.path().join("transport");
        pangopup_assets::pack_bundle(&first.output, &transport).expect("pack sparse transport");
        let verified = pangopup_assets::verify_transport(&transport).expect("verify transport");
        assert_eq!(verified.bundle_id, first_outcome.bundle_id);
        assert!(verified.part_count >= 1);
        let unpacked = temp.path().join("unpacked");
        pangopup_assets::unpack_transport(&transport, &unpacked).expect("unpack transport");
        assert_eq!(
            fs::read(unpacked.join("manifest.json")).expect("unpacked manifest"),
            fs::read(first.output.join("manifest.json")).expect("assembled manifest")
        );
        let data_root = temp.path().join("isolated-data");
        let installed =
            pangopup_assets::install_transport(&transport, &data_root).expect("isolated install");
        assert_eq!(installed.bundle_id, first_outcome.bundle_id);

        let release_a = temp.path().join("release-a");
        let release_b = temp.path().join("release-b");
        let tooling_commit = "abcdef1234567890abcdef1234567890abcdef12";
        let release_target_commit = "1111111111111111111111111111111111111111";
        let build = BuildProvenance {
            commit: tooling_commit,
            clean: "true",
        };
        let release_outcome = prepare_sparse_release_against(
            &transport,
            tooling_commit,
            release_target_commit,
            &release_a,
            &authority_manifest,
            build,
        )
        .expect("prepare sparse release");
        prepare_sparse_release_against(
            &transport,
            tooling_commit,
            release_target_commit,
            &release_b,
            &authority_manifest,
            build,
        )
        .expect("repeat sparse release");
        assert_eq!(release_outcome.bundle_id, first_outcome.bundle_id);
        for member in [
            "proof-receipt.json",
            "release-profile.json",
            "SHA256SUMS",
            "release-notes.md",
        ] {
            assert_eq!(
                fs::read(release_a.join(member)).expect("first release member"),
                fs::read(release_b.join(member)).expect("second release member")
            );
        }
        let proof = fs::read(release_a.join("proof-receipt.json")).expect("proof");
        let proof_value: serde_json::Value = serde_json::from_slice(&proof).expect("proof JSON");
        assert_eq!(
            proof,
            serde_jcs::to_vec(&proof_value).expect("canonical proof")
        );
        assert_eq!(proof_value["tool"]["implementation_commit"], tooling_commit);
        let profile: serde_json::Value = serde_json::from_slice(
            &fs::read(release_a.join("release-profile.json")).expect("profile"),
        )
        .expect("profile JSON");
        assert_eq!(profile["release"]["target_commit"], release_target_commit);
        assert_eq!(
            proof_value["sizing"]["snv_installed_bytes"],
            proof_value["sizing"]["snv_installed_members"]
                .as_array()
                .expect("installed members")
                .iter()
                .map(|member| member["size"].as_u64().expect("member size"))
                .sum::<u64>()
        );
        assert_eq!(
            proof_value["sizing"]["fresh_install_bytes"],
            proof_value["sizing"]["fresh_install_members"]
                .as_array()
                .expect("download members")
                .iter()
                .map(|member| member["size"].as_u64().expect("member size"))
                .sum::<u64>()
        );
    }

    fn verify_bundle_manifest(path: &Path) -> pangopup_index::BundleManifest {
        pangopup_index::parse_bundle_manifest_bytes(
            &fs::read(path.join("manifest.json")).expect("manifest"),
        )
        .expect("parsed manifest")
    }

    #[test]
    fn candidate_commit_is_syntax_only_but_strict() {
        assert!(valid_commit("1234567890abcdef1234567890abcdef12345678"));
        for invalid in [
            "1234",
            "1234567890ABCDEF1234567890abcdef12345678",
            "g234567890abcdef1234567890abcdef12345678",
        ] {
            assert!(!valid_commit(invalid));
        }
    }

    #[test]
    fn production_authority_is_exact_and_every_inherited_fact_is_checked() {
        let authority = production_authority().expect("checked production authority");
        assert_eq!(
            bundle_id(PRODUCTION_V1_AUTHORITY_MANIFEST),
            PRODUCTION_V1_AUTHORITY_BUNDLE_ID
        );
        let mut sparse = authority.clone();
        let authority_builder = authority.builder.clone();
        sparse.index_format = SPARSE_INDEX_FORMAT.to_owned();
        sparse.builder.version = env!("CARGO_PKG_VERSION").to_owned();
        sparse.sparse_provenance = Some(SparseProvenanceManifest {
            corpus_authority_bundle_id: PRODUCTION_V1_AUTHORITY_BUNDLE_ID.to_owned(),
            corpus_authority_builder: authority_builder,
            candidate_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
        });
        validate_sparse_authority(&sparse, &authority, PRODUCTION_V1_AUTHORITY_BUNDLE_ID)
            .expect("exact inherited authority");

        let mut forged_id = sparse.clone();
        forged_id
            .sparse_provenance
            .as_mut()
            .expect("provenance")
            .corpus_authority_bundle_id =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        assert_eq!(
            validate_sparse_authority(&forged_id, &authority, PRODUCTION_V1_AUTHORITY_BUNDLE_ID,)
                .expect_err("forged authority identity")
                .code,
            "SPARSE_RELEASE_AUTHORITY"
        );

        let mutations: [fn(&mut BundleManifest); 6] = [
            |manifest| manifest.attribution.transformed = false,
            |manifest| manifest.source.window += 1,
            |manifest| manifest.reference.input_size += 1,
            |manifest| manifest.counts.genes += 1,
            |manifest| manifest.logical_source.records += 1,
            |manifest| manifest.logical_decoded.records += 1,
        ];
        for mutate in mutations {
            let mut forged = sparse.clone();
            mutate(&mut forged);
            let error =
                validate_sparse_authority(&forged, &authority, PRODUCTION_V1_AUTHORITY_BUNDLE_ID)
                    .expect_err("forged inherited authority fact");
            assert_eq!(error.code, "SPARSE_RELEASE_AUTHORITY");
        }
        let mut forged_builder = sparse;
        forged_builder
            .sparse_provenance
            .as_mut()
            .expect("provenance")
            .corpus_authority_builder
            .version = "forged".to_owned();
        assert_eq!(
            validate_sparse_authority(
                &forged_builder,
                &authority,
                PRODUCTION_V1_AUTHORITY_BUNDLE_ID,
            )
            .expect_err("forged authority builder")
            .code,
            "SPARSE_RELEASE_AUTHORITY"
        );
    }

    #[test]
    fn tooling_commit_requires_matching_available_clean_compiled_provenance() {
        let supplied = "1234567890abcdef1234567890abcdef12345678";
        assert_eq!(
            validate_build_provenance(
                supplied,
                BuildProvenance {
                    commit: supplied,
                    clean: "true"
                }
            )
            .expect("matching clean provenance"),
            supplied
        );
        for (commit, clean, message) in [
            (
                "abcdef1234567890abcdef1234567890abcdef12",
                "true",
                "tooling commit differs",
            ),
            ("unavailable", "true", "compiled Git commit is unavailable"),
            (supplied, "false", "dirty checkout"),
            (supplied, "unavailable", "cleanliness is unavailable"),
        ] {
            let error = validate_build_provenance(supplied, BuildProvenance { commit, clean })
                .expect_err("invalid compiled provenance");
            assert_eq!(error.code, "SPARSE_RELEASE_BUILD");
            assert!(error.message.contains(message), "{}", error.message);
        }
    }

    #[test]
    fn bare_relative_assembly_and_preparation_outputs_succeed() {
        let _cwd = CWD_LOCK.lock().expect("working-directory lock");
        let original = std::env::current_dir().expect("current directory");
        let temp = TempDir::new().expect("temporary directory");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle");
        let authority = temp.path().join("authority");
        copy_bundle(&fixture, &authority);
        let authority_manifest = verify_bundle_manifest(&authority);
        let authority_id = verify_bundle(&authority).expect("authority").bundle_id;
        let sparse = temp.path().join("candidate.pgi");
        build_sparse_candidate(&SparseCandidateArguments {
            fixed_bundle: authority.clone(),
            scratch: temp.path().join("scratch"),
            candidate: sparse.clone(),
            report: temp.path().join("candidate.json"),
            expected_bundle_id: Some(authority_id.clone()),
        })
        .expect("candidate");
        std::env::set_current_dir(temp.path()).expect("enter temporary directory");
        let result = (|| {
            let bundle = PathBuf::from("relative-bundle");
            assemble_sparse_bundle_with_authority(
                &SparseBundleArguments {
                    authority_bundle: authority,
                    sparse_member: sparse,
                    candidate_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
                    output: bundle.clone(),
                },
                &authority_id,
            )?;
            let transport = temp.path().join("transport");
            pangopup_assets::pack_bundle(&bundle, &transport).map_err(asset_error)?;
            let tooling = "abcdef1234567890abcdef1234567890abcdef12";
            prepare_sparse_release_against(
                &transport,
                tooling,
                "1111111111111111111111111111111111111111",
                Path::new("relative-release"),
                &authority_manifest,
                BuildProvenance {
                    commit: tooling,
                    clean: "true",
                },
            )?;
            assert!(bundle.is_dir());
            assert!(Path::new("relative-release").is_dir());
            Ok::<_, CommandError>(())
        })();
        std::env::set_current_dir(original).expect("restore working directory");
        result.expect("bare relative release flow");
    }

    #[test]
    fn replaced_stage_is_never_published_or_recursively_cleaned() {
        for operation in ["publish", "cleanup"] {
            let temp = TempDir::new().expect("temporary directory");
            let output = temp.path().join("output");
            let (stage, mut guard) = create_stage(&output).expect("stage");
            write_synced(&stage.join("owned"), b"owned").expect("owned member");
            let displaced = temp.path().join(format!("displaced-{operation}"));
            fs::rename(&stage, &displaced).expect("displace owned stage");
            fs::create_dir(&stage).expect("replacement stage");
            fs::write(stage.join("replacement"), b"must survive").expect("replacement member");

            let error = if operation == "publish" {
                let error = publish_stage(&output, &mut guard).expect_err("replaced publication");
                assert_eq!(
                    guard.cleanup().expect_err("replaced cleanup").code,
                    "SPARSE_STAGE_IDENTITY"
                );
                error
            } else {
                guard.cleanup().expect_err("replaced cleanup")
            };
            assert_eq!(error.code, "SPARSE_STAGE_IDENTITY");
            assert!(!output.exists());
            assert_eq!(
                fs::read(stage.join("replacement")).expect("replacement survives"),
                b"must survive"
            );
            assert!(!displaced.join("owned").exists());
        }
    }

    #[test]
    fn replacement_between_ownership_check_and_rename_is_rejected_after_rename() {
        let temp = TempDir::new().expect("temporary directory");
        let output = temp.path().join("output");
        let (stage, mut guard) = create_stage(&output).expect("stage");
        write_synced(&stage.join("owned"), b"owned").expect("owned member");
        let displaced = temp.path().join("displaced-owned-stage");

        let error = publish_stage_with_hook(&output, &mut guard, || {
            fs::rename(&stage, &displaced).expect("displace checked stage");
            fs::create_dir(&stage).expect("replacement stage");
            fs::write(stage.join("replacement"), b"must survive").expect("replacement member");
        })
        .expect_err("replacement publication");

        assert_eq!(error.code, "SPARSE_STAGE_IDENTITY");
        assert_eq!(
            fs::read(output.join("replacement")).expect("published replacement survives"),
            b"must survive"
        );
        assert!(displaced.join("owned").is_file());
        assert_eq!(
            guard.cleanup().expect_err("detached owned cleanup").code,
            "SPARSE_STAGE_IDENTITY"
        );
        assert!(!displaced.join("owned").exists());
        assert_eq!(
            fs::read(output.join("replacement")).expect("replacement remains after cleanup"),
            b"must survive"
        );
    }

    #[test]
    fn fixed_manifest_remains_closed_and_sparse_requires_provenance() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle/manifest.json");
        let fixed = fs::read(&fixture).expect("fixed manifest");
        let mut extended: serde_json::Value =
            serde_json::from_slice(&fixed).expect("fixed manifest JSON");
        extended["sparse_provenance"] = serde_json::json!({
            "corpus_authority_bundle_id":PRODUCTION_V1_AUTHORITY_BUNDLE_ID,
            "corpus_authority_builder":extended["builder"].clone(),
            "candidate_commit":"1234567890abcdef1234567890abcdef12345678"
        });
        let extended = serde_jcs::to_vec(&extended).expect("canonical extension");
        assert!(matches!(
            pangopup_index::parse_bundle_manifest_bytes(&extended),
            Err(pangopup_index::IndexError::Corrupt("manifest JSON"))
        ));

        let mut sparse = pangopup_index::parse_bundle_manifest_bytes(&fixed).expect("fixed");
        sparse.index_format = SPARSE_INDEX_FORMAT.to_owned();
        assert!(matches!(
            canonical_manifest_bytes(&sparse),
            Err(pangopup_index::IndexError::Corrupt(
                "manifest format provenance"
            ))
        ));
    }

    #[test]
    fn production_assembly_rejects_a_valid_nonproduction_authority_atomically() {
        let temp = TempDir::new().expect("temporary directory");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle");
        let authority = temp.path().join("authority");
        copy_bundle(&fixture, &authority);
        let sparse = temp.path().join("candidate.pgi");
        fs::write(&sparse, b"syntactically irrelevant").expect("candidate placeholder");
        let output = temp.path().join("must-not-exist");
        let error = assemble_sparse_bundle(&SparseBundleArguments {
            authority_bundle: authority,
            sparse_member: sparse,
            candidate_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            output: output.clone(),
        })
        .expect_err("nonproduction authority");
        assert_eq!(error.code, "SPARSE_AUTHORITY");
        assert!(!output.exists());
        assert!(
            fs::read_dir(temp.path())
                .expect("temporary root")
                .all(|entry| !entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".must-not-exist.pangopup-stage-"))
        );
    }
}
