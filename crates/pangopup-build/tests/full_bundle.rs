use flate2::{Compression, write::GzEncoder};
use pangopup_build::{build_bundle, verify_bundle};
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, PangolinScore,
    RelativePosition, ScoreMagnitude, ScoreProvider,
};
use pangopup_index::{
    BundleManifest, BundleOpen, IndexReader, InputAlternative, InputLocus, OrdinaryInputLocus,
    StreamingIndexWriter, canonical_manifest_bytes,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
type ScoreEdit = fn(&mut Vec<u8>);
type ScoreMutation = (&'static str, ScoreEdit, &'static str);
type ManifestEdit = fn(&mut BundleManifest);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-full-bundle-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temp directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temp directory");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn prepare_inputs(temp: &Temp) -> (PathBuf, PathBuf) {
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("source directory");
    for gene in ["ENSG00000000001", "ENSG00000000002"] {
        let bytes = fs::read(fixture(&format!("full-build-source/{gene}.tsv")))
            .expect("read source template");
        let file = File::create(source.join(format!("{gene}.tsv.gz"))).expect("gzip file");
        let mut gzip = GzEncoder::new(file, Compression::default());
        gzip.write_all(&bytes).expect("gzip input");
        gzip.finish().expect("finish gzip");
    }
    let reference = temp.path().join("reference.fa");
    fs::copy(fixture("full-build-reference.fa"), &reference).expect("copy reference");
    (source, reference)
}

fn gzip_file(source: &Path, output: &Path) {
    let file = File::create(output).expect("gzip output");
    let mut gzip = GzEncoder::new(file, Compression::default());
    gzip.write_all(&fs::read(source).expect("read gzip source"))
        .expect("write gzip source");
    gzip.finish().expect("finish gzip");
}

fn copy_bundle(source: &Path, destination: &Path) {
    fs::create_dir(destination).expect("create bundle copy");
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        fs::copy(source.join(member), destination.join(member)).expect("copy bundle member");
    }
}

fn mutate(path: &Path, offset: usize, value: impl FnOnce(u8) -> u8) {
    let mut bytes = fs::read(path).expect("read mutation target");
    bytes[offset] = value(bytes[offset]);
    fs::write(path, bytes).expect("write mutation target");
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn mutate_scores(bundle: &Path, edit: impl FnOnce(&mut Vec<u8>)) {
    let path = bundle.join("scores.pgi");
    let mut bytes = fs::read(&path).expect("scores mutation bytes");
    edit(&mut bytes);
    fs::write(path, bytes).expect("write scores mutation");
    recanonicalize(bundle);
}

fn recanonicalize(bundle: &Path) {
    let manifest_path = bundle.join("manifest.json");
    let mut manifest: BundleManifest =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest JSON");
    let scores = fs::read(bundle.join("scores.pgi")).expect("scores");
    let member = manifest
        .members
        .iter_mut()
        .find(|member| member.path == "scores.pgi")
        .expect("scores member");
    member.size = scores.len() as u64;
    member.sha256 = format!("sha256:{:x}", Sha256::digest(&scores));
    fs::write(
        manifest_path,
        canonical_manifest_bytes(&manifest).expect("canonical manifest"),
    )
    .expect("write manifest");
}

fn rewrite_manifest(bundle: &Path, edit: impl FnOnce(&mut BundleManifest)) {
    let path = bundle.join("manifest.json");
    let mut manifest: BundleManifest =
        serde_json::from_slice(&fs::read(&path).expect("manifest bytes")).expect("manifest JSON");
    edit(&mut manifest);
    fs::write(
        path,
        canonical_manifest_bytes(&manifest).expect("canonical manifest"),
    )
    .expect("write manifest");
}

#[test]
fn plain_gzip_determinism_read_only_and_immutable_publication() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let outcome = build_bundle(&source, &reference, &first).expect("plain build");
    assert_eq!(outcome.status, "built");
    assert_eq!(outcome.counts.genes, 2);
    assert_eq!(outcome.counts.source_rows, 15);
    assert_eq!(outcome.counts.index_segments, 3);
    assert_eq!(verify_bundle(&first).expect("verify").members_verified, 2);
    build_bundle(&source, &reference, &second).expect("repeat build");
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        assert_eq!(
            fs::read(first.join(member)).expect("first member"),
            fs::read(second.join(member)).expect("second member")
        );
    }
    assert_eq!(
        build_bundle(&source, &reference, &first)
            .expect("already present")
            .status,
        "already_present"
    );

    let gzip_reference = temp.path().join("reference.fa.gz");
    gzip_file(&reference, &gzip_reference);
    let gzip_bundle = temp.path().join("gzip");
    build_bundle(&source, &gzip_reference, &gzip_bundle).expect("gzip reference build");
    let manifest: BundleManifest = serde_json::from_slice(
        &fs::read(gzip_bundle.join("manifest.json")).expect("gzip manifest"),
    )
    .expect("gzip manifest JSON");
    assert_eq!(manifest.reference.input_compression, "gzip");
    assert_eq!(manifest.reference.extra_record_count, 2);
    let plain_manifest: BundleManifest =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).expect("plain manifest"))
            .expect("plain manifest JSON");
    assert_eq!(
        manifest.reference.sequence_set_sha256,
        plain_manifest.reference.sequence_set_sha256
    );

    let incompatible = temp.path().join("incompatible");
    copy_bundle(&first, &incompatible);
    mutate(&incompatible.join("NOTICE"), 0, |byte| byte ^ 1);
    let before = fs::read(incompatible.join("NOTICE")).expect("before");
    assert_eq!(
        build_bundle(&source, &reference, &incompatible)
            .expect_err("invalid existing destination")
            .code,
        "PUBLICATION_DESTINATION"
    );
    assert_eq!(
        before,
        fs::read(incompatible.join("NOTICE")).expect("after")
    );
    let empty_destination = temp.path().join("empty-destination");
    fs::create_dir(&empty_destination).expect("empty destination");
    assert_eq!(
        build_bundle(&source, &reference, &empty_destination)
            .expect_err("empty existing destination")
            .code,
        "PUBLICATION_DESTINATION"
    );
    assert_eq!(
        fs::read_dir(&empty_destination)
            .expect("empty destination listing")
            .count(),
        0
    );
    assert!(
        fs::read_dir(temp.path())
            .expect("temp listing")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .contains("pangopup-stage"))
    );
}

#[test]
fn reference_failures_have_stable_codes_details_and_clean_scratch() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let original = fs::read_to_string(&reference).expect("reference text");
    let cases = [
        (
            "missing.fa",
            original
                .split(">NC_012920.1")
                .next()
                .expect("prefix")
                .to_owned(),
            "REFERENCE_MISSING_ACCESSION",
        ),
        (
            "duplicate.fa",
            format!("{original}>NC_000001.11 duplicate\nACGT\n"),
            "REFERENCE_DUPLICATE_ACCESSION",
        ),
        (
            "invalid.fa",
            original.replacen("ACGT", "ACG!", 1),
            "REFERENCE_INVALID_SEQUENCE",
        ),
    ];
    for (name, text, code) in cases {
        let path = temp.path().join(name);
        fs::write(&path, text).expect("failure FASTA");
        let error = build_bundle(&source, &path, &temp.path().join(format!("out-{name}")))
            .expect_err("reference must fail");
        assert_eq!(error.code, code);
    }
    let mismatch = temp.path().join("mismatch.fa");
    fs::write(&mismatch, original.replacen("ACGT", "TCGT", 1)).expect("mismatch FASTA");
    let error = build_bundle(&source, &mismatch, &temp.path().join("mismatch-out"))
        .expect_err("reference mismatch");
    assert_eq!(error.code, "REFERENCE_MISMATCH");
    let details = error.details.expect("mismatch details");
    assert_eq!(details["mismatch_count"], 1);
    assert_eq!(details["examples"][0]["gene"], "ENSG00000000001");
    assert_eq!(details["examples"][0]["pos"], 1);
    let gzip = temp.path().join("trailing.fa.gz");
    gzip_file(&reference, &gzip);
    File::options()
        .append(true)
        .open(&gzip)
        .and_then(|mut file| file.write_all(b"trailing"))
        .expect("append gzip trailing bytes");
    assert_eq!(
        build_bundle(&source, &gzip, &temp.path().join("trailing-out"))
            .expect_err("trailing gzip")
            .code,
        "REFERENCE_GZIP"
    );

    let crlf = temp.path().join("crlf.fa");
    fs::write(&crlf, original.replace('\n', "\r\n")).expect("CRLF FASTA");
    build_bundle(&source, &crlf, &temp.path().join("crlf-out")).expect("CRLF plain FASTA");
    let crlf_gzip = temp.path().join("crlf.fa.gz");
    gzip_file(&crlf, &crlf_gzip);
    build_bundle(&source, &crlf_gzip, &temp.path().join("crlf-gzip-out")).expect("CRLF gzip FASTA");

    let bare_cr = temp.path().join("bare-cr.fa");
    fs::write(&bare_cr, original.replacen('\n', "\r", 1)).expect("bare CR FASTA");
    assert_eq!(
        build_bundle(&source, &bare_cr, &temp.path().join("bare-cr-out"))
            .expect_err("plain bare CR")
            .code,
        "REFERENCE_FASTA"
    );
    let bare_cr_gzip = temp.path().join("bare-cr.fa.gz");
    gzip_file(&bare_cr, &bare_cr_gzip);
    assert_eq!(
        build_bundle(
            &source,
            &bare_cr_gzip,
            &temp.path().join("bare-cr-gzip-out"),
        )
        .expect_err("gzip bare CR")
        .code,
        "REFERENCE_FASTA"
    );

    let source_bare_cr = temp.path().join("source-bare-cr");
    fs::create_dir(&source_bare_cr).expect("bare CR source");
    for gene in ["ENSG00000000001", "ENSG00000000002"] {
        let mut bytes =
            fs::read(fixture(&format!("full-build-source/{gene}.tsv"))).expect("source fixture");
        if gene.ends_with('1') {
            let newline = bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .expect("newline");
            bytes[newline] = b'\r';
        }
        let file = File::create(source_bare_cr.join(format!("{gene}.tsv.gz")))
            .expect("bare CR gzip member");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(&bytes).expect("bare CR member bytes");
        encoder.finish().expect("finish bare CR member");
    }
    assert_eq!(
        build_bundle(
            &source_bare_cr,
            &reference,
            &temp.path().join("source-bare-cr-out"),
        )
        .expect_err("source bare CR")
        .code,
        "SOURCE_INVALID"
    );
    assert!(
        fs::read_dir(temp.path())
            .expect("temp listing")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .contains("pangopup-stage"))
    );
}

#[test]
fn verifier_rejects_outer_member_and_set_corruption() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let good = temp.path().join("good");
    build_bundle(&source, &reference, &good).expect("build good bundle");
    for (name, action, expected) in [
        ("missing", "missing", "BUNDLE_INVALID"),
        ("extra", "extra", "BUNDLE_INVALID"),
        ("notice", "notice", "BUNDLE_MEMBER_HASH"),
        ("scores", "scores", "BUNDLE_MEMBER_HASH"),
    ] {
        let copy = temp.path().join(name);
        copy_bundle(&good, &copy);
        match action {
            "missing" => fs::remove_file(copy.join("NOTICE")).expect("remove member"),
            "extra" => fs::write(copy.join("extra"), b"x").expect("extra member"),
            "notice" => mutate(&copy.join("NOTICE"), 0, |byte| byte ^ 1),
            "scores" => mutate(&copy.join("scores.pgi"), 704, |byte| byte ^ 1),
            _ => unreachable!(),
        }
        assert_eq!(
            verify_bundle(&copy).expect_err("corrupt bundle").code,
            expected
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let copy = temp.path().join("symlink");
        copy_bundle(&good, &copy);
        fs::remove_file(copy.join("NOTICE")).expect("remove notice");
        symlink(good.join("NOTICE"), copy.join("NOTICE")).expect("symlink notice");
        assert_eq!(
            verify_bundle(&copy).expect_err("symlink bundle").code,
            "BUNDLE_INVALID"
        );
    }

    let nonregular = temp.path().join("nonregular");
    copy_bundle(&good, &nonregular);
    fs::remove_file(nonregular.join("NOTICE")).expect("remove regular notice");
    fs::create_dir(nonregular.join("NOTICE")).expect("substitute directory for member");
    assert_eq!(
        verify_bundle(&nonregular)
            .expect_err("non-regular member")
            .code,
        "BUNDLE_INVALID"
    );

    let substituted = temp.path().join("substituted");
    copy_bundle(&good, &substituted);
    let mut replacement = vec![
        b'x';
        fs::metadata(good.join("NOTICE"))
            .expect("notice size")
            .len() as usize
    ];
    replacement[0] = b'y';
    fs::write(substituted.join("NOTICE"), replacement).expect("substitute notice member");
    assert_eq!(
        verify_bundle(&substituted)
            .expect_err("substituted regular member")
            .code,
        "BUNDLE_MEMBER_HASH"
    );
}

#[test]
fn semantic_corruption_reaches_inner_verifier_after_rehash() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let good = temp.path().join("good");
    build_bundle(&source, &reference, &good).expect("build good bundle");
    for (name, offset) in [
        ("header-reserved", 312_usize),
        ("section-gap", 40),
        ("segment-reserved", 321),
        ("segment-payload-gap", 360),
        ("tree-reserved", 608 + 29),
        ("payload-reserved", 704 + 10),
        ("exception-reserved", 748 + 5),
    ] {
        let copy = temp.path().join(name);
        copy_bundle(&good, &copy);
        mutate(&copy.join("scores.pgi"), offset, |byte| byte | 0x80);
        recanonicalize(&copy);
        assert_eq!(
            verify_bundle(&copy).expect_err("inner corruption").code,
            if name == "payload-reserved" {
                "BUNDLE_INDEX"
            } else {
                "BUNDLE_INVALID"
            }
        );
    }
    let notice = temp.path().join("notice-rehashed");
    copy_bundle(&good, &notice);
    mutate(&notice.join("NOTICE"), 0, |byte| byte ^ 1);
    let bytes = fs::read(notice.join("NOTICE")).expect("mutated notice");
    rewrite_manifest(&notice, |manifest| {
        manifest.members[0].sha256 = format!("sha256:{:x}", Sha256::digest(&bytes));
    });
    assert_eq!(
        verify_bundle(&notice)
            .expect_err("rehashed notice substitution")
            .code,
        "BUNDLE_NOTICE"
    );

    let counts = temp.path().join("counts-recanonicalized");
    copy_bundle(&good, &counts);
    rewrite_manifest(&counts, |manifest| manifest.counts.source_segments += 1);
    assert_eq!(
        verify_bundle(&counts)
            .expect_err("false source counts")
            .code,
        "BUNDLE_COUNTS"
    );

    let aliases = temp.path().join("aliases-recanonicalized");
    copy_bundle(&good, &aliases);
    rewrite_manifest(&aliases, |manifest| manifest.reference.aliases.swap(0, 1));
    assert_eq!(
        verify_bundle(&aliases)
            .expect_err("noncanonical aliases")
            .code,
        "BUNDLE_INVALID"
    );
}

#[test]
fn resigned_mutations_reach_tree_payload_exception_and_canonical_checks() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let good = temp.path().join("good");
    build_bundle(&source, &reference, &good).expect("build good bundle");
    let original_id = verify_bundle(&good).expect("good verify").bundle_id;

    let tree_cases: [(&str, ScoreEdit); 3] = [
        ("tree-link", |bytes: &mut Vec<u8>| {
            let tree = get_u64(bytes, 40) as usize;
            put_u64(bytes, tree + 8, u64::MAX);
        }),
        ("tree-max", |bytes: &mut Vec<u8>| {
            let tree = get_u64(bytes, 40) as usize;
            // Root segment ends at 2, while its right subtree ends at 4.
            put_u32(bytes, tree + 24, 2);
        }),
        ("tree-balance", |bytes: &mut Vec<u8>| {
            let tree = get_u64(bytes, 40) as usize;
            // Three valid ordered segments are relinked into a left-only
            // chain. Connectivity, preorder, BST ordering, and subtree maxima
            // remain valid, so the independent AVL balance check is reached.
            put_u64(bytes, tree, 2);
            put_u64(bytes, tree + 8, 1);
            put_u64(bytes, tree + 16, u64::MAX);
            put_u32(bytes, tree + 24, 4);
            put_u64(bytes, tree + 32, 1);
            put_u64(bytes, tree + 40, 2);
            put_u64(bytes, tree + 48, u64::MAX);
            put_u32(bytes, tree + 32 + 24, 2);
            put_u64(bytes, tree + 64, 0);
            put_u64(bytes, tree + 72, u64::MAX);
            put_u64(bytes, tree + 80, u64::MAX);
            put_u32(bytes, tree + 64 + 24, 2);
        }),
    ];
    for (name, edit) in tree_cases {
        let copy = temp.path().join(name);
        copy_bundle(&good, &copy);
        mutate_scores(&copy, edit);
        let error = verify_bundle(&copy).expect_err("tree mutation must fail");
        assert_eq!(error.code, "BUNDLE_INVALID", "{name}: {error}");
        if name == "tree-balance" {
            assert!(error.message.contains("tree balance"), "{error}");
        }
    }

    let semantic_cases: [ScoreMutation; 4] = [
        (
            "payload-reference",
            |bytes: &mut Vec<u8>| {
                let payload = get_u64(bytes, 56) as usize;
                bytes[payload] = (bytes[payload] & !0b111) | 1;
            },
            "BUNDLE_LOGICAL_MISMATCH",
        ),
        (
            "payload-score",
            |bytes: &mut Vec<u8>| {
                let payload = get_u64(bytes, 56) as usize;
                bytes[payload] ^= 1 << 3;
            },
            "BUNDLE_LOGICAL_MISMATCH",
        ),
        (
            "exception-allele",
            |bytes: &mut Vec<u8>| {
                let exception = get_u64(bytes, 72) as usize;
                bytes[exception + 2] = 0;
            },
            "BUNDLE_INVALID",
        ),
        (
            "exception-score",
            |bytes: &mut Vec<u8>| {
                let exception = get_u64(bytes, 72) as usize;
                bytes[exception + 24] = 1;
            },
            "BUNDLE_LOGICAL_MISMATCH",
        ),
    ];
    for (name, edit, code) in semantic_cases {
        let copy = temp.path().join(name);
        copy_bundle(&good, &copy);
        mutate_scores(&copy, edit);
        assert_eq!(
            verify_bundle(&copy).expect_err("semantic mutation").code,
            code,
            "{name}"
        );
    }

    let padding = temp.path().join("section-padding");
    copy_bundle(&good, &padding);
    mutate_scores(&padding, |bytes| {
        let tree = get_u64(bytes, 40) as usize;
        bytes.insert(tree, 0);
        for field in [16_usize, 40, 56, 72] {
            let shifted = get_u64(bytes, field) + 1;
            put_u64(bytes, field, shifted);
        }
    });
    IndexReader::open(&padding.join("scores.pgi")).expect("cheap fixed-v1 open permits padding");
    assert_eq!(
        verify_bundle(&padding)
            .expect_err("canonical verifier rejects padding")
            .code,
        "BUNDLE_INDEX"
    );

    let boundary = temp.path().join("segment-boundary");
    copy_bundle(&good, &boundary);
    mutate_scores(&boundary, |bytes| {
        let segments = get_u64(bytes, 24) as usize;
        let tree = get_u64(bytes, 40) as usize;
        let exception = get_u64(bytes, 72) as usize;
        // Gene 2's ordinary positions 2 and 4 become adjacent 2 and 3;
        // move its exception to 4 and keep the interval tree exact. This is a
        // structurally valid but non-maximal fixed-v1 segmentation.
        put_u32(bytes, segments + 2 * 96 + 16, 3);
        put_u32(bytes, segments + 2 * 96 + 20, 3);
        put_u32(bytes, tree + 24, 3);
        put_u32(bytes, tree + 2 * 32 + 24, 3);
        put_u32(bytes, exception + 16, 4);
    });
    IndexReader::open(&boundary.join("scores.pgi"))
        .expect("cheap fixed-v1 open permits noncanonical segment boundary");
    let error = verify_bundle(&boundary).expect_err("canonical segment boundary");
    assert_eq!(error.code, "BUNDLE_INDEX");
    assert!(error.message.contains("noncanonical adjacent segments"));

    let direction = temp.path().join("source-direction-provenance");
    copy_bundle(&good, &direction);
    rewrite_manifest(&direction, |manifest| {
        manifest.counts.ascending_members = manifest.counts.genes;
        manifest.counts.descending_members = 0;
    });
    let changed = verify_bundle(&direction).expect("direction split is source-only provenance");
    assert_ne!(changed.bundle_id, original_id);
}

#[test]
fn manifest_count_overflow_is_typed_in_library_and_cli() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let good = temp.path().join("good");
    build_bundle(&source, &reference, &good).expect("build good bundle");

    let overflow_cases: [(&str, ManifestEdit); 3] = [
        ("directions", |manifest: &mut BundleManifest| {
            manifest.counts.ascending_members = u64::MAX;
            manifest.counts.descending_members = 1;
        }),
        ("rows", |manifest: &mut BundleManifest| {
            manifest.counts.gene_loci = u64::MAX;
        }),
        ("exceptions", |manifest: &mut BundleManifest| {
            manifest.counts.n_omit_a = u64::MAX;
            manifest.counts.n_omit_t = 1;
        }),
    ];
    for (name, edit) in overflow_cases {
        let copy = temp.path().join(name);
        copy_bundle(&good, &copy);
        rewrite_manifest(&copy, edit);
        assert_eq!(
            verify_bundle(&copy).expect_err("overflow must fail").code,
            "BUNDLE_COUNTS"
        );
    }

    let cli = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .arg("verify")
        .arg(temp.path().join("directions"))
        .output()
        .expect("run verify CLI");
    assert_eq!(cli.status.code(), Some(1));
    assert!(cli.stdout.is_empty());
    let stderr = String::from_utf8(cli.stderr).expect("JSON stderr");
    assert_eq!(stderr.lines().count(), 1);
    let error: serde_json::Value = serde_json::from_str(stderr.trim()).expect("error JSON");
    assert_eq!(error["code"], "BUNDLE_COUNTS");
    assert_eq!(error["details"], serde_json::Value::Null);
}

#[test]
fn concurrent_publication_converges_on_one_immutable_bundle() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let output = temp.path().join("bundle");
    let first = {
        let source = source.clone();
        let reference = reference.clone();
        let output = output.clone();
        thread::spawn(move || build_bundle(&source, &reference, &output))
    };
    let second = {
        let source = source.clone();
        let reference = reference.clone();
        let output = output.clone();
        thread::spawn(move || build_bundle(&source, &reference, &output))
    };
    let first = first.join().expect("first thread").expect("first build");
    let second = second.join().expect("second thread").expect("second build");
    assert_eq!(first.bundle_id, second.bundle_id);
    assert!(matches!(
        (first.status, second.status),
        ("built", "already_present") | ("already_present", "built")
    ));
    verify_bundle(&output).expect("published bundle");
}

#[test]
fn opened_provider_is_send_sync_and_concurrent_results_match_serial_oracle() {
    fn assert_provider<T: ScoreProvider + Send + Sync>() {}
    assert_provider::<BundleOpen>();

    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let output = temp.path().join("lookup-bundle");
    build_bundle(&source, &reference, &output).expect("build bundle");
    let provider = Arc::new(BundleOpen::open(&output).expect("open bundle"));
    let frozen_provenance = provider.provenance().clone();
    assert_eq!(provider.bundle_id(), frozen_provenance.bundle_id());
    assert!(std::ptr::eq(provider.provenance(), provider.provenance()));
    let snv = Grch38Snv::new(
        "chr1".parse().expect("contig"),
        GenomicPosition::new(2).expect("position"),
        DnaBase::C,
        DnaBase::T,
    )
    .expect("SNV");
    let expected = provider.lookup(snv, None).expect("serial lookup");
    assert_eq!(
        expected.provenance().precomputed(),
        Some(&frozen_provenance)
    );
    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let provider = Arc::clone(&provider);
        let barrier = Arc::clone(&barrier);
        let expected = expected.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..100 {
                assert_eq!(
                    provider.lookup(snv, None).expect("concurrent lookup"),
                    expected
                );
            }
        }));
    }
    barrier.wait();
    for handle in handles {
        handle.join().expect("lookup worker");
    }
}

#[test]
fn oversized_manifest_is_rejected_before_json_decode() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let output = temp.path().join("oversized-manifest-bundle");
    build_bundle(&source, &reference, &output).expect("build bundle");
    File::options()
        .write(true)
        .open(output.join("manifest.json"))
        .expect("open manifest")
        .set_len(1024 * 1024 + 1)
        .expect("extend manifest");
    assert!(
        BundleOpen::open(&output)
            .expect_err("oversized manifest must fail")
            .to_string()
            .contains("manifest size")
    );
}

#[test]
fn production_writer_spools_scaled_payload_instead_of_accumulating_it() {
    let temp = Temp::new();
    let payload = temp.path().join("payload");
    let output = temp.path().join("scores.pgi");
    let mut writer = StreamingIndexWriter::create(&payload).expect("writer");
    let magnitude = ScoreMagnitude::new(0).expect("score");
    let relative = RelativePosition::new(-50).expect("position");
    let score = PangolinScore::new(magnitude, relative, magnitude, relative);
    for numeric in 1..=10_000_u64 {
        let gene = EnsemblGeneId::from_numeric(numeric).expect("gene");
        let alternatives = [DnaBase::C, DnaBase::G, DnaBase::T]
            .map(|alternate| InputAlternative { alternate, score });
        writer
            .push_gene(&[InputLocus::Ordinary(OrdinaryInputLocus {
                gene,
                contig: Grch38Contig::from_code(1).expect("contig"),
                position: GenomicPosition::new(1).expect("position"),
                reference: DnaBase::A,
                alternatives,
            })])
            .expect("push gene");
    }
    assert_eq!(writer.scratch_bytes(), 110_000);
    let summary = writer.finish(&output).expect("finish scaled writer");
    assert_eq!(summary.loci, 10_000);
    assert_eq!(summary.segments, 10_000);
    assert!(fs::metadata(output).expect("index metadata").len() > 1_000_000);
}

#[test]
fn source_and_reference_inputs_can_be_read_only() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let temp = Temp::new();
        let (source, reference) = prepare_inputs(&temp);
        for entry in fs::read_dir(&source).expect("source members") {
            fs::set_permissions(
                entry.expect("member").path(),
                fs::Permissions::from_mode(0o444),
            )
            .expect("readonly member");
        }
        fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).expect("readonly source");
        fs::set_permissions(&reference, fs::Permissions::from_mode(0o444))
            .expect("readonly reference");
        build_bundle(&source, &reference, &temp.path().join("bundle")).expect("read-only build");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).expect("restore source");
    }
}

#[test]
fn manifest_is_closed_and_canonical() {
    let temp = Temp::new();
    let (source, reference) = prepare_inputs(&temp);
    let bundle = temp.path().join("bundle");
    build_bundle(&source, &reference, &bundle).expect("bundle");
    let path = bundle.join("manifest.json");
    let mut bytes = fs::read(&path).expect("manifest");
    bytes.pop();
    bytes.extend_from_slice(b",\"unknown\":true}");
    fs::write(&path, bytes).expect("unknown manifest key");
    assert_eq!(
        verify_bundle(&bundle).expect_err("closed manifest").code,
        "BUNDLE_INVALID"
    );
}
