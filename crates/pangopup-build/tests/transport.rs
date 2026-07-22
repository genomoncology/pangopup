use flate2::{Compression, write::GzEncoder};
use pangopup_assets::{pack_bundle, unpack_transport, verify_transport};
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

fn collect_rs(directory: &Path, workspace: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .expect("source directory")
        .map(|entry| entry.expect("source entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs(&path, workspace, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(
                path.strip_prefix(workspace)
                    .expect("relative source")
                    .to_owned(),
            );
        }
    }
}

fn builder_digest(mutation: Option<&Path>) -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("NOTICE"),
    ];
    for name in [
        "pangopup-core",
        "pangopup-index",
        "pangopup-assets",
        "pangopup-build",
    ] {
        let root = PathBuf::from(format!("crates/{name}"));
        paths.push(root.join("Cargo.toml"));
        collect_rs(&workspace.join(&root), &workspace, &mut paths);
    }
    paths.sort();
    let mut hash = Sha256::new();
    for relative in paths {
        let name = relative.to_str().expect("UTF-8 path").as_bytes();
        let mut bytes = fs::read(workspace.join(&relative)).expect("builder source");
        if mutation.is_some_and(|target| relative == target) {
            bytes.push(0);
        }
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
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
    let actual = builder_digest(None);
    assert_eq!(manifest.builder.source_sha256, actual);
    assert_ne!(
        actual,
        builder_digest(Some(Path::new("crates/pangopup-assets/src/lib.rs")))
    );
    assert_ne!(actual, builder_digest(Some(Path::new("NOTICE"))));
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
