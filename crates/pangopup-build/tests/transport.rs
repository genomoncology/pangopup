use flate2::{Compression, write::GzEncoder};
use pangopup_assets::{
    ReleasePreparationContract, inspect_transport, pack_bundle, prepare_release,
    prepare_release_with_contract, test_reset_input_opens, test_take_input_opens, unpack_transport,
    verify_transport,
};
use pangopup_build::{build_bundle, verify_bundle};
use pangopup_index::{
    BundleManifest, IndexError, MAX_MANIFEST_BYTES, canonical_manifest_bytes,
    parse_bundle_manifest_bytes,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-transport-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary directory");
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn build_fixture(temp: &Temp) -> PathBuf {
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("source directory");
    for gene in ["ENSG00000000001", "ENSG00000000002"] {
        let bytes =
            fs::read(fixture(&format!("full-build-source/{gene}.tsv"))).expect("source fixture");
        let file = File::create(source.join(format!("{gene}.tsv.gz"))).expect("gzip file");
        let mut gzip = GzEncoder::new(file, Compression::default());
        gzip.write_all(&bytes).expect("gzip input");
        gzip.finish().expect("finish gzip");
    }
    let reference = temp.path().join("reference.fa");
    fs::copy(fixture("full-build-reference.fa"), &reference).expect("reference fixture");
    let bundle = temp.path().join("bundle");
    build_bundle(&source, &reference, &bundle).expect("build fixture bundle");
    bundle
}

fn exact_members(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(path)
        .expect("directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    names.sort();
    names
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir(destination).expect("copy destination");
    for member in exact_members(source) {
        fs::copy(source.join(&member), destination.join(member)).expect("copy member");
    }
}

fn invocation_stages(parent: &Path, output_name: &str) -> Vec<String> {
    let prefix = format!(".{output_name}.pangopup-stage-");
    exact_members(parent)
        .into_iter()
        .filter(|name| name.starts_with(&prefix))
        .collect()
}

fn encode_pinned(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 9).expect("encoder");
    encoder.include_checksum(true).expect("checksum");
    encoder.include_contentsize(true).expect("content size");
    encoder.include_dictid(false).expect("dictionary ID");
    encoder.long_distance_matching(false).expect("LDM");
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .expect("pledged size");
    encoder.write_all(bytes).expect("compress");
    encoder.finish().expect("finish")
}

fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn frame_builder_source(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn snv_builder_digest(mutation: Option<&str>) -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let evidence = workspace.join("crates/pangopup-build/src/source_fingerprint");
    let algorithm = fs::read(evidence.join("algorithm.v1")).expect("fingerprint algorithm");
    let inventory = fs::read(evidence.join("snv-inventory.v1")).expect("SNV fingerprint inventory");
    let declaration = std::str::from_utf8(&inventory).expect("UTF-8 SNV inventory");
    assert!(declaration.ends_with('\n'));

    let mut entries = Vec::new();
    let mut mutation_seen = mutation.is_none();
    for logical in declaration.lines() {
        let source = if let Some(relative) = logical
            .strip_prefix("dependencies/")
            .or_else(|| logical.strip_prefix("wiring/"))
        {
            evidence.join(relative)
        } else {
            workspace.join(logical)
        };
        let mut bytes = fs::read(source).expect("SNV builder-source input");
        if mutation == Some(logical) {
            bytes.push(0);
            mutation_seen = true;
        }
        entries.push((logical.as_bytes(), bytes));
    }
    entries.sort_unstable_by_key(|(logical, _)| *logical);

    let mut hash = Sha256::new();
    for bytes in [
        algorithm.as_slice(),
        b"pangopup.snv-builder-source.v1",
        inventory.as_slice(),
    ] {
        frame_builder_source(&mut hash, bytes);
    }
    for (logical, bytes) in entries {
        frame_builder_source(&mut hash, logical);
        frame_builder_source(&mut hash, &bytes);
    }
    assert!(mutation_seen, "requested causal input is inventoried");
    format!("sha256:{:x}", hash.finalize())
}

fn resign(transport: &Path, compressed: &[u8], scores: Option<&[u8]>) {
    let part = transport.join("payload.pgi.zst.part0000");
    fs::write(&part, compressed).expect("write re-signed payload");
    let manifest_path = transport.join("transport.json");
    let mut outer: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("transport manifest"))
            .expect("transport JSON");
    outer["payload"]["compressed_size"] = Value::from(compressed.len() as u64);
    outer["payload"]["compressed_sha256"] = Value::String(hash(compressed));
    outer["payload"]["parts"][0]["size"] = Value::from(compressed.len() as u64);
    outer["payload"]["parts"][0]["sha256"] = Value::String(hash(compressed));
    if let Some(scores) = scores {
        let inner_path = transport.join("bundle-manifest.json");
        let mut inner: BundleManifest =
            serde_json::from_slice(&fs::read(&inner_path).expect("inner manifest"))
                .expect("inner JSON");
        let member = inner
            .members
            .iter_mut()
            .find(|member| member.path == "scores.pgi")
            .expect("scores member");
        member.size = scores.len() as u64;
        member.sha256 = hash(scores);
        let bytes = canonical_manifest_bytes(&inner).expect("canonical inner manifest");
        fs::write(&inner_path, &bytes).expect("write inner manifest");
        let identity = hash(&bytes);
        outer["bundle"]["bundle_id"] = Value::String(identity.clone());
        outer["bundle"]["manifest"]["size"] = Value::from(bytes.len() as u64);
        outer["bundle"]["manifest"]["sha256"] = Value::String(identity);
        outer["bundle"]["scores"]["size"] = Value::from(scores.len() as u64);
        outer["bundle"]["scores"]["sha256"] = Value::String(hash(scores));
    }
    outer
        .as_object_mut()
        .expect("outer object")
        .remove("transport_id");
    let unsigned = serde_jcs::to_vec(&outer).expect("canonical unsigned transport");
    outer["transport_id"] = Value::String(hash(&unsigned));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&outer).expect("canonical transport"),
    )
    .expect("write transport manifest");
}

fn rewrite_outer(transport: &Path, mutate: impl FnOnce(&mut Value)) {
    let manifest_path = transport.join("transport.json");
    let mut outer: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("transport manifest"))
            .expect("transport JSON");
    mutate(&mut outer);
    outer
        .as_object_mut()
        .expect("outer object")
        .remove("transport_id");
    let unsigned = serde_jcs::to_vec(&outer).expect("canonical unsigned transport");
    outer["transport_id"] = Value::String(hash(&unsigned));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&outer).expect("canonical transport"),
    )
    .expect("write transport manifest");
}

fn miniature_release_contract(transport: &Path) -> (Vec<u8>, Vec<u8>) {
    let inspected = inspect_transport(transport).expect("inspect miniature transport");
    let inner: BundleManifest = serde_json::from_slice(&inspected.bundle_manifest_bytes)
        .expect("miniature bundle manifest");
    let notice = inner
        .members
        .iter()
        .find(|member| member.path == "NOTICE")
        .expect("notice member");
    let scores = inner
        .members
        .iter()
        .find(|member| member.path == "scores.pgi")
        .expect("scores member");
    let parts: Vec<_> = inspected
        .parts
        .iter()
        .map(|part| {
            serde_json::json!({
                "ordinal": part.ordinal,
                "path": part.path,
                "size": part.size,
                "sha256": part.sha256,
            })
        })
        .collect();
    let receipt_value = serde_json::json!({
        "schema": "pangopup.proof-receipt.v1",
        "source": {
            "archive_name": inner.source.archive_name,
            "archive_size": inner.source.published_archive_size,
            "archive_md5": inner.source.published_archive_md5,
            "observed_member_count": inner.source.observed_member_count,
            "observed_members_sha256": inner.source.observed_members_sha256,
        },
        "reference": {
            "assembly_accession": inner.reference.assembly_accession,
            "input_size": inner.reference.input_size,
            "input_sha256": inner.reference.input_sha256,
            "sequence_set_sha256": inner.reference.sequence_set_sha256,
        },
        "bundle": {
            "bundle_id": inspected.bundle_id,
            "builder_version": inner.builder.version,
            "builder_source_sha256": inner.builder.source_sha256,
            "manifest": {
                "size": inspected.bundle_manifest_size,
                "sha256": inspected.bundle_manifest_sha256,
            },
            "members": [
                {"path": "NOTICE", "size": notice.size, "sha256": notice.sha256},
                {"path": "scores.pgi", "size": scores.size, "sha256": scores.sha256},
            ],
        },
        "transport": {
            "transport_id": inspected.transport_id,
            "manifest": {
                "size": inspected.transport_bytes.len(),
                "sha256": inspected.transport_sha256,
            },
            "compressed": {
                "size": inspected.compressed_size,
                "sha256": inspected.compressed_sha256,
            },
            "parts": parts,
        },
        "tool": {
            "implementation_commit": "1111111111111111111111111111111111111111",
            "encoder_crate": inspected.compression.encoder_crate,
            "libzstd_version": inspected.compression.libzstd_version,
        },
        "verify": {
            "bundle": ["pangopup-build", "verify", "bundles/miniature"],
            "transport": ["pangopup-build", "transport", "verify", "--transport", "transports/miniature"],
        },
    });
    let mut receipt = serde_jcs::to_vec(&receipt_value).expect("canonical miniature receipt");
    receipt.push(b'\n');
    let receipt_sha256 = hash(&receipt);
    let tag = "miniature-snv-v1";
    let repository = "example/pangopup";
    let prefix = format!("https://github.com/{repository}/releases/download/{tag}/");
    let mut members = vec![
        (
            "transport.json".to_owned(),
            inspected.transport_bytes.len() as u64,
            inspected.transport_sha256,
        ),
        (
            "bundle-manifest.json".to_owned(),
            inspected.bundle_manifest_size,
            inspected.bundle_manifest_sha256,
        ),
        (
            "NOTICE".to_owned(),
            inspected.notice_size,
            inspected.notice_sha256,
        ),
    ];
    members.extend(
        inspected
            .parts
            .into_iter()
            .map(|part| (part.path, part.size, part.sha256)),
    );
    let profile_members: Vec<_> = members
        .into_iter()
        .map(|(name, size, sha256)| {
            serde_json::json!({
                "logical_path": name,
                "asset_name": name,
                "size": size,
                "sha256": sha256,
                "url": format!("{prefix}{name}"),
            })
        })
        .collect();
    let profile_value = serde_json::json!({
        "schema": "pangopup.release-profile.v1",
        "profile": tag,
        "repository": repository,
        "release": {
            "tag": tag,
            "title": "Miniature Pangopup SNV scores",
            "target_commit": "2222222222222222222222222222222222222222",
            "page_url": format!("https://github.com/{repository}/releases/tag/{tag}"),
        },
        "source": {
            "title": inner.source.title,
            "creators": inner.source.creators,
            "doi": inner.source.doi,
            "license": "CC-BY-4.0",
            "archive": {
                "name": inner.source.archive_name,
                "size": inner.source.published_archive_size,
                "md5": inner.source.published_archive_md5,
            },
            "assembly": "GRCh38",
            "masked": true,
            "window": 50,
        },
        "reference_compatibility": {
            "assembly": inner.reference.assembly,
            "assembly_accession": inner.reference.assembly_accession,
            "input_size": inner.reference.input_size,
            "input_sha256": inner.reference.input_sha256,
            "sequence_set_sha256": inner.reference.sequence_set_sha256,
            "ordinary_ref_mismatches": 0,
            "preserved_ref_n_loci": 0,
        },
        "bundle": {
            "schema": inner.schema,
            "index_format": inner.index_format,
            "bundle_id": inspected.bundle_id,
        },
        "transport": {
            "schema": "pangopup.snv-transport.v1",
            "transport_id": inspected.transport_id,
            "members": profile_members,
        },
        "proof": {
            "schema": "pangopup.proof-receipt.v1",
            "asset_name": "proof-receipt.json",
            "size": receipt.len(),
            "sha256": receipt_sha256,
        },
    });
    let profile = serde_jcs::to_vec(&profile_value).expect("canonical miniature profile");
    (receipt, profile)
}

#[test]
fn release_preparation_is_deterministic_atomic_and_never_opens_a_part() {
    use std::os::unix::fs::PermissionsExt;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("release.transport");
    pack_bundle(&bundle, &transport).expect("pack miniature release transport");
    let (receipt, profile) = miniature_release_contract(&transport);
    let receipt_path = temp.path().join("proof-receipt.json");
    fs::write(&receipt_path, &receipt).expect("miniature receipt");
    let receipt_sha256 = hash(&receipt);
    let contract = ReleasePreparationContract {
        receipt_bytes: &receipt,
        receipt_sha256: &receipt_sha256,
        profile_bytes: &profile,
    };
    test_reset_input_opens();
    let first = temp.path().join("prepared-a");
    let outcome = prepare_release_with_contract(&transport, &receipt_path, &first, contract)
        .expect("prepare miniature release");
    assert_eq!(outcome.status, "prepared");
    assert_eq!(outcome.asset_count, 7);
    assert_eq!(
        fs::metadata(&first)
            .expect("prepared output metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let opened = test_take_input_opens();
    assert_eq!(opened.len(), 4);
    assert!(opened.iter().all(|path| {
        [
            "proof-receipt.json",
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
        ]
        .iter()
        .any(|name| path.ends_with(name))
    }));
    assert!(
        opened
            .iter()
            .all(|path| !path.contains("payload.pgi.zst.part"))
    );
    assert_eq!(
        exact_members(&first),
        [
            "SHA256SUMS",
            "proof-receipt.json",
            "release-notes.md",
            "release-profile.json"
        ]
    );
    assert_eq!(
        fs::read(first.join("proof-receipt.json")).expect("proof copy"),
        receipt
    );
    assert_eq!(
        fs::read(first.join("release-profile.json")).expect("profile"),
        profile
    );
    let sums = fs::read_to_string(first.join("SHA256SUMS")).expect("SHA list");
    assert!(sums.ends_with('\n'));
    assert_eq!(sums.lines().count(), 6);
    assert_eq!(
        sums.lines()
            .map(|line| line.split_once("  ").expect("digest separator").1)
            .collect::<Vec<_>>(),
        [
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
            "payload.pgi.zst.part0000",
            "proof-receipt.json",
            "release-profile.json",
        ]
    );
    let notes = fs::read_to_string(first.join("release-notes.md")).expect("release notes");
    for required in [
        "Nils Wagner",
        "Aleksandr Neverov",
        "10.5281/zenodo.15649338",
        "CC BY 4.0",
        "does not name an exact FASTA/patch release or GENCODE release",
        "RefSeq GRCh38.p14",
        "per-gene TSV rows",
        "model weights",
        "remote sync",
        "pangopup assets install --transport \"$transport_dir\"",
        "downloads exactly the 4 transport members",
    ] {
        assert!(notes.contains(required), "release notes omit {required}");
    }
    assert_eq!(notes.matches("curl --fail --location --output").count(), 4);
    for member in [
        "transport.json",
        "bundle-manifest.json",
        "NOTICE",
        "payload.pgi.zst.part0000",
    ] {
        assert!(notes.contains(&format!("$transport_dir/{member}")));
        assert!(notes.contains(&format!(
            "https://github.com/example/pangopup/releases/download/miniature-snv-v1/{member}"
        )));
    }

    let second = temp.path().join("prepared-b");
    prepare_release_with_contract(&transport, &receipt_path, &second, contract)
        .expect("repeat miniature release preparation");
    for member in exact_members(&first) {
        assert_eq!(
            fs::read(first.join(&member)).expect("first output"),
            fs::read(second.join(&member)).expect("second output")
        );
    }
    assert_eq!(
        prepare_release_with_contract(&transport, &receipt_path, &first, contract)
            .expect_err("output conflict")
            .kind(),
        pangopup_assets::AssetErrorKind::OutputConflict
    );
    assert!(invocation_stages(temp.path(), "prepared-a").is_empty());
}

#[test]
fn public_release_contract_rejects_miniature_and_metadata_mismatch() {
    use std::os::unix::fs::symlink;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("release.transport");
    pack_bundle(&bundle, &transport).expect("pack miniature release transport");
    let (mut receipt, profile) = miniature_release_contract(&transport);
    let receipt_path = temp.path().join("proof-receipt.json");
    fs::write(&receipt_path, &receipt).expect("miniature receipt");
    assert_eq!(
        prepare_release(
            &transport,
            &receipt_path,
            &temp.path().join("public-rejected")
        )
        .expect_err("public contract must reject miniature")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );

    receipt[100] ^= 1;
    fs::write(&receipt_path, &receipt).expect("mutated receipt");
    let receipt_sha256 = hash(&receipt);
    let contract = ReleasePreparationContract {
        receipt_bytes: &receipt,
        receipt_sha256: &receipt_sha256,
        profile_bytes: &profile,
    };
    assert_eq!(
        prepare_release_with_contract(
            &transport,
            &receipt_path,
            &temp.path().join("malformed-rejected"),
            contract,
        )
        .expect_err("malformed receipt")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
    assert!(invocation_stages(temp.path(), "malformed-rejected").is_empty());

    let (valid_receipt, valid_profile) = miniature_release_contract(&transport);
    let mut value: Value = serde_json::from_slice(&valid_receipt).expect("valid receipt value");
    value["transport"]["compressed"]["sha256"] =
        Value::String(format!("sha256:{}", "0".repeat(64)));
    let mut mismatched = serde_jcs::to_vec(&value).expect("canonical mismatched receipt");
    mismatched.push(b'\n');
    fs::write(&receipt_path, &mismatched).expect("mismatched receipt");
    let mismatched_hash = hash(&mismatched);
    let mismatched_contract = ReleasePreparationContract {
        receipt_bytes: &mismatched,
        receipt_sha256: &mismatched_hash,
        profile_bytes: &valid_profile,
    };
    assert_eq!(
        prepare_release_with_contract(
            &transport,
            &receipt_path,
            &temp.path().join("metadata-rejected"),
            mismatched_contract,
        )
        .expect_err("receipt metadata mismatch")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
    assert!(invocation_stages(temp.path(), "metadata-rejected").is_empty());

    let symlinked_receipt = temp.path().join("symlinked-receipt.json");
    symlink(&receipt_path, &symlinked_receipt).expect("symlink receipt");
    assert_eq!(
        prepare_release(
            &transport,
            &symlinked_receipt,
            &temp.path().join("symlink-rejected"),
        )
        .expect_err("symlinked receipt")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
}

#[test]
fn bounded_transport_inspection_rejects_part_shape_and_size_without_opening_it() {
    use std::os::unix::fs::symlink;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let baseline = temp.path().join("inspection.transport");
    pack_bundle(&bundle, &baseline).expect("pack inspection fixture");
    let part_name = exact_members(&baseline)
        .into_iter()
        .find(|name| name.starts_with("payload.pgi.zst.part"))
        .expect("payload part");
    let expected_size = fs::metadata(baseline.join(&part_name))
        .expect("part metadata")
        .len();

    let wrong_size = temp.path().join("wrong-size.transport");
    copy_directory(&baseline, &wrong_size);
    File::options()
        .write(true)
        .open(wrong_size.join(&part_name))
        .expect("open part for fixture mutation")
        .set_len(expected_size - 1)
        .expect("truncate fixture part");
    test_reset_input_opens();
    assert_eq!(
        inspect_transport(&wrong_size)
            .expect_err("wrong part size")
            .kind(),
        pangopup_assets::AssetErrorKind::PartSetInvalid
    );
    assert!(
        test_take_input_opens()
            .iter()
            .all(|path| !path.contains("payload.pgi.zst.part"))
    );

    let symlinked = temp.path().join("symlinked-part.transport");
    copy_directory(&baseline, &symlinked);
    fs::remove_file(symlinked.join(&part_name)).expect("remove copied part");
    symlink(baseline.join(&part_name), symlinked.join(&part_name)).expect("symlink part");
    assert_eq!(
        inspect_transport(&symlinked)
            .expect_err("symlinked part")
            .kind(),
        pangopup_assets::AssetErrorKind::PartSetInvalid
    );
}

#[test]
fn deterministic_pack_verify_unpack_and_conflict() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let first = temp.path().join("first.transport");
    let second = temp.path().join("second.transport");
    let packed = pack_bundle(&bundle, &first).expect("pack");
    let repeated = pack_bundle(&bundle, &second).expect("repeat pack");
    assert_eq!(packed.transport_id, repeated.transport_id);
    assert_eq!(exact_members(&first), exact_members(&second));
    for member in exact_members(&first) {
        assert_eq!(
            fs::read(first.join(&member)).expect("first member"),
            fs::read(second.join(&member)).expect("second member")
        );
    }
    let verified = verify_transport(&first).expect("verify transport");
    assert_eq!(verified.transport_id, packed.transport_id);
    let unpacked = temp.path().join("unpacked");
    let outcome = unpack_transport(&first, &unpacked).expect("unpack");
    assert_eq!(outcome.bundle_id, packed.bundle_id);
    verify_bundle(&unpacked).expect("shared bundle verification");
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        assert_eq!(
            fs::read(bundle.join(member)).expect("bundle member"),
            fs::read(unpacked.join(member)).expect("unpacked member")
        );
    }
    assert_eq!(
        unpack_transport(&first, &unpacked)
            .expect_err("destination conflict")
            .kind()
            .code(),
        "OUTPUT_CONFLICT"
    );
}

#[test]
fn builder_identity_covers_assets_manifest_notice_and_certification_source() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let manifest: BundleManifest =
        serde_json::from_slice(&fs::read(bundle.join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    let actual = snv_builder_digest(None);
    assert_eq!(
        actual,
        "sha256:b3bdc4d9d8e710fb554fd47f0cfc6f6a7bb764451069e6ae4a98534d8c5dc6a2"
    );
    assert_eq!(manifest.builder.source_sha256, actual);
    for causal in [
        "wiring/snv-root-wiring.v1",
        "NOTICE",
        "crates/pangopup-assets/src/snv.rs",
    ] {
        assert_ne!(actual, snv_builder_digest(Some(causal)), "{causal}");
    }
}

#[test]
fn shared_inner_manifest_parser_is_bounded_duplicate_aware_and_canonical() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let bytes = fs::read(bundle.join("manifest.json")).expect("manifest");
    parse_bundle_manifest_bytes(&bytes).expect("canonical manifest");

    let text = String::from_utf8(bytes.clone()).expect("UTF-8 manifest");
    let duplicate = text.replacen(
        "\"path\":\"NOTICE\"",
        "\"path\":\"NOTICE\",\"path\":\"NOTICE\"",
        1,
    );
    assert!(matches!(
        parse_bundle_manifest_bytes(duplicate.as_bytes()),
        Err(IndexError::Corrupt("manifest JSON"))
    ));

    let oversized = vec![b' '; MAX_MANIFEST_BYTES as usize + 1];
    assert!(matches!(
        parse_bundle_manifest_bytes(&oversized),
        Err(IndexError::Corrupt("manifest size"))
    ));

    let mut noncanonical = bytes;
    noncanonical.push(b'\n');
    assert!(matches!(
        parse_bundle_manifest_bytes(&noncanonical),
        Err(IndexError::Corrupt("manifest is not canonical"))
    ));
}

#[test]
fn corrupt_part_and_member_set_fail_closed_without_publication() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = exact_members(&transport)
        .into_iter()
        .find(|name| name.starts_with("payload."))
        .expect("part");
    let path = transport.join(part);
    let mut bytes = fs::read(&path).expect("part bytes");
    bytes[0] ^= 1;
    fs::write(&path, bytes).expect("mutate part");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("part hash")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );
    let output = temp.path().join("must-not-exist");
    assert!(unpack_transport(&transport, &output).is_err());
    assert!(!output.exists());
    assert!(invocation_stages(temp.path(), "must-not-exist").is_empty());

    fs::write(transport.join("extra"), b"x").expect("extra member");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("exact set")
            .kind()
            .code(),
        "PART_SET_INVALID"
    );
}

#[test]
fn independent_transport_layers_fail_at_their_declared_boundaries() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let baseline = temp.path().join("baseline");
    pack_bundle(&bundle, &baseline).expect("pack baseline");

    let copied_manifest = temp.path().join("copied-manifest");
    copy_directory(&baseline, &copied_manifest);
    let path = copied_manifest.join("bundle-manifest.json");
    let mut bytes = fs::read(&path).expect("bundle manifest");
    bytes[10] ^= 1;
    fs::write(path, bytes).expect("corrupt copied manifest");
    assert_eq!(
        verify_transport(&copied_manifest)
            .expect_err("copied manifest identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    let notice = temp.path().join("notice");
    copy_directory(&baseline, &notice);
    let path = notice.join("NOTICE");
    let mut bytes = fs::read(&path).expect("notice");
    bytes[0] ^= 1;
    fs::write(path, bytes).expect("corrupt notice");
    assert_eq!(
        verify_transport(&notice)
            .expect_err("notice identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    for (label, mutation) in [
        ("missing", "missing"),
        ("renamed", "renamed"),
        ("sized", "sized"),
    ] {
        let case = temp.path().join(label);
        copy_directory(&baseline, &case);
        let part = case.join("payload.pgi.zst.part0000");
        match mutation {
            "missing" => fs::remove_file(part).expect("remove part"),
            "renamed" => {
                fs::rename(part, case.join("payload.pgi.zst.part0001")).expect("rename part")
            }
            "sized" => {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(part)
                    .expect("open part");
                file.set_len(1).expect("truncate part");
            }
            _ => unreachable!(),
        }
        assert_eq!(
            verify_transport(&case)
                .expect_err("invalid part set")
                .kind()
                .code(),
            "PART_SET_INVALID"
        );
    }

    let whole_hash = temp.path().join("whole-hash");
    copy_directory(&baseline, &whole_hash);
    rewrite_outer(&whole_hash, |outer| {
        outer["payload"]["compressed_sha256"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    });
    assert_eq!(
        verify_transport(&whole_hash)
            .expect_err("whole stream identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    for (label, truncate) in [("checksum", false), ("truncated", true)] {
        let case = temp.path().join(label);
        copy_directory(&baseline, &case);
        let part = case.join("payload.pgi.zst.part0000");
        let mut compressed = fs::read(part).expect("compressed payload");
        if truncate {
            compressed.pop();
        } else {
            let last = compressed.last_mut().expect("checksum byte");
            *last ^= 1;
        }
        resign(&case, &compressed, None);
        assert_eq!(
            verify_transport(&case)
                .expect_err("invalid compressed stream")
                .kind()
                .code(),
            "COMPRESSION_INVALID"
        );
    }

    let decoded_hash = temp.path().join("decoded-hash");
    copy_directory(&baseline, &decoded_hash);
    let part = decoded_hash.join("payload.pgi.zst.part0000");
    let mut scores =
        zstd::stream::decode_all(fs::read(part).expect("compressed payload").as_slice())
            .expect("decode payload");
    let last = scores.last_mut().expect("score byte");
    *last ^= 1;
    let compressed = encode_pinned(&scores);
    resign(&decoded_hash, &compressed, None);
    assert_eq!(
        verify_transport(&decoded_hash)
            .expect_err("decoded score hash")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );
}

#[test]
fn self_consistent_semantic_corruption_passes_integrity_but_not_unpack() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = transport.join("payload.pgi.zst.part0000");
    let mut scores =
        zstd::stream::decode_all(fs::read(&part).expect("compressed payload").as_slice())
            .expect("decode");
    scores[312] |= 0x80;
    let compressed = encode_pinned(&scores);
    resign(&transport, &compressed, Some(&scores));
    verify_transport(&transport).expect("integrity-only verification");
    let output = temp.path().join("invalid-bundle");
    assert_eq!(
        unpack_transport(&transport, &output)
            .expect_err("semantic certification")
            .kind()
            .code(),
        "BUNDLE_INVALID"
    );
    assert!(!output.exists());
    assert!(invocation_stages(temp.path(), "invalid-bundle").is_empty());
}

#[test]
fn hash_consistent_trailing_and_second_frames_are_compression_errors() {
    for second_frame in [false, true] {
        let temp = Temp::new();
        let bundle = build_fixture(&temp);
        let transport = temp.path().join("transport");
        pack_bundle(&bundle, &transport).expect("pack");
        let part = transport.join("payload.pgi.zst.part0000");
        let mut compressed = fs::read(&part).expect("payload");
        if second_frame {
            compressed.extend_from_within(..);
        } else {
            compressed.push(0);
        }
        resign(&transport, &compressed, None);
        assert_eq!(
            verify_transport(&transport)
                .expect_err("compression structure")
                .kind()
                .code(),
            "COMPRESSION_INVALID"
        );
    }

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("invalid-magic");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = transport.join("payload.pgi.zst.part0000");
    let mut compressed = fs::read(part).expect("payload");
    compressed[0] ^= 1;
    resign(&transport, &compressed, None);
    assert_eq!(
        verify_transport(&transport)
            .expect_err("hash-consistent invalid magic")
            .kind()
            .code(),
        "COMPRESSION_INVALID"
    );
}

#[test]
fn concurrent_unpack_has_one_atomic_winner() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let output = temp.path().join("race-output");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let transport = transport.clone();
        let output = output.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            unpack_transport(&transport, &output)
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().expect("thread"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .next()
            .expect("loser")
            .kind()
            .code(),
        "OUTPUT_CONFLICT"
    );
    verify_bundle(&output).expect("winner is complete");
}

#[test]
fn oversized_manifest_is_rejected_before_allocation() {
    let temp = Temp::new();
    let transport = temp.path().join("oversized");
    fs::create_dir(&transport).expect("transport directory");
    let file = File::create(transport.join("transport.json")).expect("manifest");
    file.set_len(1024 * 1024 + 1).expect("oversized manifest");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("manifest cap")
            .kind()
            .code(),
        "MANIFEST_INVALID"
    );
}

#[test]
fn bundle_certification_caps_members_before_hashing_or_mapping() {
    let temp = Temp::new();
    let original = build_fixture(&temp);

    let oversized_notice = temp.path().join("oversized-notice");
    copy_directory(&original, &oversized_notice);
    File::create(oversized_notice.join("NOTICE"))
        .expect("notice")
        .set_len(64 * 1024 + 1)
        .expect("sparse oversized notice");
    assert_eq!(
        verify_bundle(&oversized_notice)
            .expect_err("notice cap")
            .code,
        "BUNDLE_NOTICE"
    );

    let oversized_scores = temp.path().join("oversized-scores");
    copy_directory(&original, &oversized_scores);
    File::create(oversized_scores.join("scores.pgi"))
        .expect("scores")
        .set_len(17_179_869_184 + 1)
        .expect("sparse oversized scores");
    assert_eq!(
        verify_bundle(&oversized_scores)
            .expect_err("score cap")
            .code,
        "BUNDLE_INDEX"
    );

    let extra = temp.path().join("extra-member");
    copy_directory(&original, &extra);
    fs::write(extra.join("fourth"), b"").expect("fourth member");
    assert_eq!(
        verify_bundle(&extra).expect_err("bounded exact set").code,
        "BUNDLE_INVALID"
    );
}

#[test]
fn typed_asset_error_mapping_covers_io_and_future_versions() {
    let temp = Temp::new();
    let binary = env!("CARGO_BIN_EXE_pangopup-build");
    let missing_path = temp.path().join("missing");
    assert_eq!(
        verify_bundle(&missing_path)
            .expect_err("legacy missing bundle")
            .code,
        "BUNDLE_INVALID"
    );
    assert_eq!(
        verify_transport(&missing_path)
            .expect_err("missing transport")
            .kind()
            .code(),
        "INPUT_IO"
    );
    let missing_cli = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&missing_path)
        .output()
        .expect("missing CLI");
    assert_eq!(missing_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(missing_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"INPUT_IO\"")
    );

    let bundle = build_fixture(&temp);
    let blocked_parent = temp.path().join("blocked-parent");
    fs::write(&blocked_parent, b"not a directory").expect("blocking file");
    assert_eq!(
        pack_bundle(&bundle, &blocked_parent.join("transport"))
            .expect_err("output parent")
            .kind()
            .code(),
        "OUTPUT_IO"
    );
    let output_cli = Command::new(binary)
        .args(["transport", "pack", "--bundle"])
        .arg(&bundle)
        .arg("--output")
        .arg(blocked_parent.join("transport-cli"))
        .output()
        .expect("output CLI");
    assert_eq!(output_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(output_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"OUTPUT_IO\"")
    );

    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    rewrite_outer(&transport, |outer| {
        outer["schema"] = Value::String("pangopup.snv-transport.v2".to_owned());
        outer["future"] = Value::Bool(true);
    });
    assert_eq!(
        verify_transport(&transport)
            .expect_err("future transport")
            .kind()
            .code(),
        "TRANSPORT_INCOMPATIBLE"
    );
    let future_cli = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&transport)
        .output()
        .expect("future CLI");
    assert_eq!(future_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(future_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"TRANSPORT_INCOMPATIBLE\"")
    );
}

#[cfg(unix)]
#[test]
fn read_only_inputs_work_and_symlinked_parts_are_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        fs::set_permissions(bundle.join(member), fs::Permissions::from_mode(0o444))
            .expect("read-only bundle member");
    }
    fs::set_permissions(&bundle, fs::Permissions::from_mode(0o555))
        .expect("read-only bundle directory");
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack read-only bundle");
    for member in exact_members(&transport) {
        fs::set_permissions(transport.join(member), fs::Permissions::from_mode(0o444))
            .expect("read-only transport member");
    }
    fs::set_permissions(&transport, fs::Permissions::from_mode(0o555))
        .expect("read-only transport directory");
    verify_transport(&transport).expect("verify read-only transport");
    unpack_transport(&transport, &temp.path().join("unpacked")).expect("unpack read-only");

    fs::set_permissions(&transport, fs::Permissions::from_mode(0o755))
        .expect("restore transport directory");
    let part_name = exact_members(&transport)
        .into_iter()
        .find(|name| name.starts_with("payload."))
        .expect("part name");
    let part = transport.join(&part_name);
    fs::set_permissions(&part, fs::Permissions::from_mode(0o644)).expect("restore part");
    let replacement = temp.path().join("replacement-part");
    fs::copy(&part, &replacement).expect("replacement bytes");
    fs::remove_file(&part).expect("remove part");
    symlink(&replacement, &part).expect("symlink part");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("symlink rejection")
            .kind()
            .code(),
        "PART_SET_INVALID"
    );
    fs::set_permissions(&bundle, fs::Permissions::from_mode(0o755))
        .expect("restore bundle directory");
}

#[cfg(unix)]
#[test]
fn sigkill_never_leaves_a_partial_final_directory() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let output = temp.path().join("killed-output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .arg("transport")
        .arg("unpack")
        .arg("--transport")
        .arg(&transport)
        .arg("--output")
        .arg(&output)
        .spawn()
        .expect("spawn unpack");
    let mut observed_stage = false;
    while child.try_wait().expect("poll unpack").is_none() {
        if !invocation_stages(temp.path(), "killed-output").is_empty() {
            observed_stage = true;
            child.kill().expect("kill staged unpack");
            break;
        }
        std::thread::yield_now();
    }
    child.wait().expect("wait for killed unpack");
    assert!(observed_stage, "the test must kill after staging exists");
    if output.exists() {
        assert_eq!(
            exact_members(&output),
            ["NOTICE", "manifest.json", "scores.pgi"]
        );
        verify_bundle(&output).expect("post-rename output must already be complete");
    } else {
        assert!(!invocation_stages(temp.path(), "killed-output").is_empty());
    }
}

#[test]
fn maintenance_cli_pins_grammar_json_and_streams() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    let binary = env!("CARGO_BIN_EXE_pangopup-build");
    let pack = Command::new(binary)
        .args(["transport", "pack", "--output"])
        .arg(&transport)
        .arg("--bundle")
        .arg(&bundle)
        .output()
        .expect("pack CLI");
    assert!(pack.status.success());
    assert!(pack.stderr.is_empty());
    let stdout = String::from_utf8(pack.stdout).expect("UTF-8 JSON");
    let packed: Value = serde_json::from_str(stdout.trim_end()).expect("pack JSON");
    let transport_id = packed["transport_id"].as_str().expect("transport ID");
    let bundle_id = packed["bundle_id"].as_str().expect("bundle ID");
    let part_count = packed["part_count"].as_u64().expect("part count");
    let compressed = packed["compressed_bytes"]
        .as_u64()
        .expect("compressed bytes");
    assert_eq!(
        stdout,
        format!(
            "{{\"status\":\"packed\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\",\"part_count\":{part_count},\"compressed_bytes\":{compressed}}}\n"
        )
    );

    let verify = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&transport)
        .output()
        .expect("verify CLI");
    assert!(verify.status.success());
    assert!(verify.stderr.is_empty());
    assert_eq!(
        String::from_utf8(verify.stdout).expect("UTF-8 JSON"),
        format!(
            "{{\"status\":\"verified\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\",\"part_count\":{part_count},\"compressed_bytes\":{compressed}}}\n"
        )
    );

    let unpacked = temp.path().join("cli-unpacked");
    let unpack = Command::new(binary)
        .args(["transport", "unpack", "--output"])
        .arg(&unpacked)
        .arg("--transport")
        .arg(&transport)
        .output()
        .expect("unpack CLI");
    assert!(unpack.status.success());
    assert!(unpack.stderr.is_empty());
    assert_eq!(
        String::from_utf8(unpack.stdout).expect("UTF-8 JSON"),
        format!(
            "{{\"status\":\"unpacked\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\"}}\n"
        )
    );

    let missing = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(temp.path().join("missing-transport"))
        .output()
        .expect("missing input CLI");
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .expect("UTF-8 error")
            .starts_with("{\"status\":\"error\",\"code\":\"INPUT_IO\"")
    );

    for arguments in [
        vec!["transport", "pack"],
        vec!["transport", "verify", "--transport", "--unknown"],
        vec![
            "transport",
            "verify",
            "--transport",
            "a",
            "--transport",
            "b",
        ],
        vec!["transport", "unpack", "x", "--output", "y"],
    ] {
        let output = Command::new(binary)
            .args(arguments)
            .output()
            .expect("usage CLI");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .expect("UTF-8 error")
                .starts_with("{\"status\":\"error\",\"code\":\"CLI_USAGE\"")
        );
    }
}
