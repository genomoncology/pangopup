use pangopup_build::reference::{
    build_reference_bundle, certify_reference_bundle, inspect_reference_bundle,
    production_context_dense_pages, reference_window,
};
use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceError, ReferenceProvider};
use pangopup_index::reference::ReferenceBundleOpen;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
};

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("pangopup-reference-{label}-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale test directory");
        }
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/reference-production-mini")
        .join(name)
}

fn build(temp: &Temp, source: &str, leaf: &str) -> PathBuf {
    let output = temp.0.join(leaf);
    build_reference_bundle(
        "pangopup-reference-mini-v1",
        &fixture(source),
        &fixture("assembly_report.txt"),
        &output,
    )
    .expect("build miniature reference");
    output
}

fn copy_bundle(source: &Path, destination: &Path) {
    fs::create_dir(destination).expect("create copied bundle");
    for name in ["NOTICE", "manifest.json", "reference.pgr"] {
        fs::copy(source.join(name), destination.join(name)).expect("copy bundle member");
    }
}

fn mutate_byte(path: &Path, offset: u64, value: u8) {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .expect("open mutation target");
    file.seek(SeekFrom::Start(offset))
        .expect("seek mutation target");
    file.write_all(&[value]).expect("write mutation");
    file.sync_all().expect("sync mutation");
}

fn mutate_bytes(path: &Path, offset: u64, value: &[u8]) {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .expect("open mutation target");
    file.seek(SeekFrom::Start(offset))
        .expect("seek mutation target");
    file.write_all(value).expect("write mutation");
    file.sync_all().expect("sync mutation");
}

#[test]
fn miniature_build_reader_and_provenance_are_exact() {
    let temp = Temp::new("exact");
    let bundle = build(&temp, "source.fa", "bundle");
    let outcome = inspect_reference_bundle(&bundle).expect("inspect miniature");
    assert_eq!(outcome.profile, "pangopup-reference-mini-v1");
    assert_eq!(outcome.sequences, 25);
    assert_eq!(outcome.total_bases, 159);
    assert_eq!(outcome.integrity, "structural_only");
    assert!(!outcome.member_sha256_checked);

    let opened = ReferenceBundleOpen::open(&bundle).expect("open miniature");
    for alias in ["1", "chr1", "NC_000001.11"] {
        assert_eq!(
            opened.resolve_alias(alias).expect("accepted alias"),
            Grch38Contig::autosome(1).expect("chr1")
        );
    }
    for alias in ["01", "chr01", "Chr1", "MT", "chrMT", "NC_000001"] {
        assert!(
            opened.resolve_alias(alias).is_err(),
            "{alias} must be rejected"
        );
    }
    let mut bases = [0_u8; 30];
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut bases,
        )
        .expect("copy all IUPAC");
    assert_eq!(&bases, b"ACGTRYSWKMBDHVNACGTRYSWKMBDHVN");
    assert_eq!(opened.provenance().assembly(), "synthetic-mini");
    assert_eq!(
        opened.provenance().format(),
        "pangopup.reference.acgt2-rle.v1"
    );
}

#[test]
fn ambiguity_overlays_cover_before_inside_after_and_enclosing_windows() {
    let temp = Temp::new("ambiguity-overlaps");
    let bundle = build(&temp, "source.fa", "bundle");
    let opened = ReferenceBundleOpen::open(&bundle).expect("open miniature");
    let chr3 = Grch38Contig::autosome(3).expect("chr3");
    for (start, expected) in [
        (1, &b"NN"[..]),
        (2, &b"NNNAC"[..]),
        (3, &b"NN"[..]),
        (3, &b"NNACGTNN"[..]),
        (5, &b"ACGT"[..]),
        (7, &b"GTNNN"[..]),
        (9, &b"NNNN"[..]),
    ] {
        let mut actual = vec![0_u8; expected.len()];
        opened
            .copy_window(
                chr3,
                GenomicPosition::new(start).expect("valid start"),
                &mut actual,
            )
            .expect("ambiguity overlap window");
        assert_eq!(actual, expected, "start={start}");
    }
}

#[test]
fn qualification_reader_uses_authenticated_bytes_and_held_member_after_substitution() {
    let temp = Temp::new("qualification-held");
    let bundle = build(&temp, "source.fa", "bundle");
    let manifest = fs::read(bundle.join("manifest.json")).expect("manifest bytes");
    let notice = fs::read(bundle.join("NOTICE")).expect("notice bytes");
    let reference = File::open(bundle.join("reference.pgr")).expect("held member");

    let retained = temp.0.join("retained");
    fs::rename(&bundle, &retained).expect("rename authenticated bundle");
    fs::create_dir(&bundle).expect("substitute bundle path");
    fs::write(bundle.join("manifest.json"), b"substitution").expect("substitute manifest");
    fs::write(bundle.join("NOTICE"), b"substitution").expect("substitute notice");
    fs::write(bundle.join("reference.pgr"), b"substitution").expect("substitute member");

    let (opened, dense_open_bytes) =
        ReferenceBundleOpen::open_qualification_audited(&manifest, &notice, &reference)
            .expect("open held qualification reader");
    assert_eq!(dense_open_bytes, 0);
    let mut bases = [0_u8; 15];
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut bases,
        )
        .expect("held window");
    assert_eq!(&bases, b"ACGTRYSWKMBDHVN");
    assert!(ReferenceBundleOpen::open(&bundle).is_err());
}

#[test]
fn plain_and_gzip_payloads_match_but_manifests_do_not() {
    let temp = Temp::new("compression");
    let plain = build(&temp, "source.fa", "plain");
    let gzip = build(&temp, "source.fa.gz", "gzip");
    assert_eq!(
        fs::read(plain.join("reference.pgr")).expect("plain payload"),
        fs::read(gzip.join("reference.pgr")).expect("gzip payload")
    );
    assert_ne!(
        fs::read(plain.join("manifest.json")).expect("plain manifest"),
        fs::read(gzip.join("manifest.json")).expect("gzip manifest")
    );
}

#[test]
fn window_bounds_preserve_destination_and_reads_are_bounded() {
    let temp = Temp::new("bounds");
    let bundle = build(&temp, "source.fa", "bundle");
    let opened = ReferenceBundleOpen::open(&bundle).expect("open miniature");
    let chr_m = Grch38Contig::M;
    let mut empty = [];
    assert_eq!(
        opened.copy_window(chr_m, GenomicPosition::new(1).expect("one"), &mut empty),
        Err(ReferenceError::EmptyWindow)
    );
    let mut unchanged = *b"sentinel";
    assert_eq!(
        opened.copy_window(
            chr_m,
            GenomicPosition::new(3).expect("three"),
            &mut unchanged
        ),
        Err(ReferenceError::OutOfBounds)
    );
    assert_eq!(&unchanged, b"sentinel");

    let manifest = fs::read(bundle.join("manifest.json")).expect("manifest bytes");
    let notice = fs::read(bundle.join("NOTICE")).expect("notice bytes");
    let reference = File::open(bundle.join("reference.pgr")).expect("reference member");
    let (reopened, dense_open_bytes) =
        ReferenceBundleOpen::open_qualification_audited(&manifest, &notice, &reference)
            .expect("cheap audited reopen");
    assert_eq!(dense_open_bytes, 0, "cheap open must not touch dense bytes");
    let mut actual = [0_u8; 9];
    reopened
        .copy_window(chr_m, GenomicPosition::new(1).expect("one"), &mut actual)
        .expect("terminal window");
    assert_eq!(&actual, b"ACGTNACGT");
}

#[test]
fn dense_substitution_passes_cheap_open_but_fails_certification() {
    let temp = Temp::new("corruption");
    let original = build(&temp, "source.fa", "original");
    let corrupt = temp.0.join("corrupt");
    copy_bundle(&original, &corrupt);
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(corrupt.join("reference.pgr"))
        .expect("open copied payload");
    file.seek(SeekFrom::Start(4096))
        .expect("seek first dense byte");
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte).expect("read dense byte");
    byte[0] ^= 0b0000_0001;
    file.seek(SeekFrom::Start(4096)).expect("reseak dense byte");
    file.write_all(&byte)
        .expect("write valid two-bit substitution");
    file.sync_all().expect("sync mutation");
    ReferenceBundleOpen::open(&corrupt).expect("cheap open deliberately does not hash dense bytes");
    assert!(certify_reference_bundle(&corrupt).is_err());
}

#[test]
fn structural_header_padding_and_run_corruption_fail_cheap_open() {
    let temp = Temp::new("structure");
    let original = build(&temp, "source.fa", "original");

    let header = temp.0.join("header");
    copy_bundle(&original, &header);
    mutate_byte(&header.join("reference.pgr"), 12, 1);
    assert!(ReferenceBundleOpen::open(&header).is_err());

    let padding = temp.0.join("padding");
    copy_bundle(&original, &padding);
    mutate_byte(&padding.join("reference.pgr"), 1264, 1);
    assert!(ReferenceBundleOpen::open(&padding).is_err());

    let run = temp.0.join("run");
    copy_bundle(&original, &run);
    let mut file = File::open(run.join("reference.pgr")).expect("open run payload");
    file.seek(SeekFrom::Start(48))
        .expect("seek ambiguity offset");
    let mut bytes = [0_u8; 8];
    file.read_exact(&mut bytes).expect("read ambiguity offset");
    drop(file);
    mutate_byte(
        &run.join("reference.pgr"),
        u64::from_le_bytes(bytes) + 8,
        15,
    );
    assert!(ReferenceBundleOpen::open(&run).is_err());

    for (label, offset, bytes) in [
        ("magic", 0, vec![b'X']),
        ("version", 8, 2_u16.to_le_bytes().to_vec()),
        ("encoding", 10, vec![2]),
        ("count", 11, vec![24]),
        ("file-length", 16, 1_u64.to_le_bytes().to_vec()),
        ("directory-code", 64, vec![2]),
        ("directory-gap", 128, 5000_u64.to_le_bytes().to_vec()),
    ] {
        let corrupt = temp.0.join(label);
        copy_bundle(&original, &corrupt);
        mutate_bytes(&corrupt.join("reference.pgr"), offset, &bytes);
        assert!(
            ReferenceBundleOpen::open(&corrupt).is_err(),
            "{label} corruption must fail"
        );
    }

    let mut header = [0_u8; 64];
    File::open(original.join("reference.pgr"))
        .expect("open original member")
        .read_exact(&mut header)
        .expect("read header");
    let ambiguity = u64::from_le_bytes(header[48..56].try_into().expect("ambiguity"));
    for (label, relative, bytes) in [
        ("run-zero-length", 4, 0_u32.to_le_bytes().to_vec()),
        ("run-bad-code", 8, vec![15]),
        ("run-reserved", 9, vec![1]),
    ] {
        let corrupt = temp.0.join(label);
        copy_bundle(&original, &corrupt);
        mutate_bytes(&corrupt.join("reference.pgr"), ambiguity + relative, &bytes);
        assert!(
            ReferenceBundleOpen::open(&corrupt).is_err(),
            "{label} corruption must fail"
        );
    }

    let mut first_run = [0_u8; 16];
    let mut second_run = [0_u8; 16];
    let mut member = File::open(original.join("reference.pgr")).expect("open runs");
    member.seek(SeekFrom::Start(ambiguity)).expect("seek runs");
    member.read_exact(&mut first_run).expect("first run");
    member.read_exact(&mut second_run).expect("second run");

    let overlap = temp.0.join("run-overlap");
    copy_bundle(&original, &overlap);
    mutate_bytes(
        &overlap.join("reference.pgr"),
        ambiguity + 16,
        &first_run[0..4],
    );
    assert!(ReferenceBundleOpen::open(&overlap).is_err());

    let coalescing = temp.0.join("run-coalescing");
    copy_bundle(&original, &coalescing);
    mutate_byte(
        &coalescing.join("reference.pgr"),
        ambiguity + 16 + 8,
        first_run[8],
    );
    assert!(ReferenceBundleOpen::open(&coalescing).is_err());

    let truncated = temp.0.join("truncated");
    copy_bundle(&original, &truncated);
    let truncated_path = truncated.join("reference.pgr");
    let length = fs::metadata(&truncated_path)
        .expect("member metadata")
        .len();
    File::options()
        .write(true)
        .open(&truncated_path)
        .expect("truncate member")
        .set_len(length - 1)
        .expect("set truncated length");
    assert!(ReferenceBundleOpen::open(&truncated).is_err());

    let trailing = temp.0.join("member-trailing");
    copy_bundle(&original, &trailing);
    File::options()
        .append(true)
        .open(trailing.join("reference.pgr"))
        .expect("open trailing member")
        .write_all(&[0])
        .expect("append trailing member");
    assert!(ReferenceBundleOpen::open(&trailing).is_err());
}

#[test]
fn concurrent_provider_reads_are_exact() {
    let temp = Temp::new("threads");
    let bundle = build(&temp, "source.fa", "bundle");
    let opened = Arc::new(ReferenceBundleOpen::open(&bundle).expect("open miniature"));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let opened = Arc::clone(&opened);
            std::thread::spawn(move || {
                let mut bases = [0_u8; 15];
                opened
                    .copy_window(
                        Grch38Contig::autosome(1).expect("chr1"),
                        GenomicPosition::new(1).expect("one"),
                        &mut bases,
                    )
                    .expect("threaded copy");
                bases
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(&thread.join().expect("join reader"), b"ACGTRYSWKMBDHVN");
    }
}

#[test]
fn maintenance_window_uses_typed_provider() {
    let temp = Temp::new("maintenance");
    let bundle = build(&temp, "source.fa", "bundle");
    let result = reference_window(&bundle, "NC_000003.12", 3, 8).expect("maintenance window");
    assert_eq!(result.contig, "chr3");
    assert_eq!(result.bases, "NNACGTNN");
}

#[test]
fn production_context_pages_are_predicted_without_payload_bytes() {
    let traces = production_context_dense_pages().expect("predict production pages");
    assert_eq!(traces.len(), 14);
    assert_eq!(traces.iter().map(Vec::len).sum::<usize>(), 20);
    let unique: std::collections::BTreeSet<_> = traces.into_iter().flatten().collect();
    assert_eq!(
        unique.into_iter().collect::<Vec<_>>(),
        vec![
            31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495
        ]
    );
}

#[test]
fn invalid_source_never_publishes_or_leaves_staging() {
    let temp = Temp::new("invalid-source");
    let changed = temp.0.join("changed.fa");
    fs::copy(fixture("source.fa"), &changed).expect("copy source");
    File::options()
        .append(true)
        .open(&changed)
        .expect("open changed source")
        .write_all(b"X")
        .expect("change source identity");
    let output = temp.0.join("bundle");
    let error = build_reference_bundle(
        "pangopup-reference-mini-v1",
        &changed,
        &fixture("assembly_report.txt"),
        &output,
    )
    .expect_err("changed source must fail");
    assert_eq!(error.code, "REFERENCE_INPUT");
    assert!(!output.exists());
    assert_eq!(fs::read_dir(&temp.0).expect("list temp").count(), 1);
}

#[test]
fn malformed_gzip_symlinked_input_and_read_only_inputs_are_handled_safely() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = Temp::new("input-hardening");
    let mut malformed = fs::read(fixture("source.fa.gz")).expect("gzip fixture");
    let middle = malformed.len() / 2;
    malformed[middle] ^= 1;
    let malformed_path = temp.0.join("malformed.fa.gz");
    fs::write(&malformed_path, &malformed).expect("malformed gzip");
    assert!(
        build_reference_bundle(
            "pangopup-reference-mini-v1",
            &malformed_path,
            &fixture("assembly_report.txt"),
            &temp.0.join("malformed-output")
        )
        .is_err()
    );

    let mut trailing = fs::read(fixture("source.fa.gz")).expect("gzip fixture");
    trailing.pop();
    trailing.push(b'X');
    let trailing_path = temp.0.join("trailing.fa.gz");
    fs::write(&trailing_path, trailing).expect("trailing gzip");
    assert!(
        build_reference_bundle(
            "pangopup-reference-mini-v1",
            &trailing_path,
            &fixture("assembly_report.txt"),
            &temp.0.join("trailing-output")
        )
        .is_err()
    );

    let linked = temp.0.join("linked.fa");
    symlink(fixture("source.fa"), &linked).expect("source symlink");
    assert!(
        build_reference_bundle(
            "pangopup-reference-mini-v1",
            &linked,
            &fixture("assembly_report.txt"),
            &temp.0.join("linked-output")
        )
        .is_err()
    );

    let readonly_source = temp.0.join("readonly.fa.gz");
    let readonly_report = temp.0.join("readonly-report.txt");
    fs::copy(fixture("source.fa.gz"), &readonly_source).expect("copy readonly source");
    fs::copy(fixture("assembly_report.txt"), &readonly_report).expect("copy readonly report");
    fs::set_permissions(&readonly_source, fs::Permissions::from_mode(0o444))
        .expect("source permissions");
    fs::set_permissions(&readonly_report, fs::Permissions::from_mode(0o444))
        .expect("report permissions");
    build_reference_bundle(
        "pangopup-reference-mini-v1",
        &readonly_source,
        &readonly_report,
        &temp.0.join("readonly-output"),
    )
    .expect("read-only inputs build");
}

#[test]
fn concurrent_builds_publish_exactly_one_bundle_without_replace() {
    let temp = Temp::new("concurrent-publish");
    let output = temp.0.join("bundle");
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let output = output.clone();
            std::thread::spawn(move || {
                barrier.wait();
                build_reference_bundle(
                    "pangopup-reference-mini-v1",
                    &fixture("source.fa.gz"),
                    &fixture("assembly_report.txt"),
                    &output,
                )
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().expect("join build"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .filter(|error| error.code == "ALREADY_EXISTS")
            .count(),
        1
    );
    inspect_reference_bundle(&output).expect("published winner");
    assert!(fs::read_dir(&temp.0).expect("temp contents").all(|entry| {
        !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .contains("reference-stage")
    }));
}

#[test]
fn reference_cli_grammar_errors_are_canonical_empty_stderr_and_redacted() {
    let cases: Vec<(Vec<&str>, i32, &str, &str)> = vec![
        (
            vec!["reference", "inspect"],
            2,
            "CLI_USAGE",
            "reference.inspect",
        ),
        (
            vec![
                "reference",
                "inspect",
                "--bundle",
                "/secret/one",
                "--bundle",
                "/secret/two",
            ],
            2,
            "CLI_USAGE",
            "reference.inspect",
        ),
        (
            vec![
                "reference",
                "window",
                "--bundle",
                "/secret/bundle",
                "--contig",
                "chr1",
                "--start",
                "nope",
                "--length",
                "1",
            ],
            2,
            "CLI_USAGE",
            "reference.window",
        ),
        (
            vec![
                "reference",
                "window",
                "--bundle",
                "/secret/bundle",
                "--contig",
                "chr1",
                "--start",
                "1",
                "--length",
                "1048577",
            ],
            2,
            "CLI_USAGE",
            "reference.window",
        ),
        (
            vec![
                "reference",
                "inspect",
                "--bundle",
                "/secret/pangopup-do-not-disclose",
            ],
            1,
            "REFERENCE_BUNDLE",
            "reference.inspect",
        ),
        (
            vec![
                "reference",
                "build",
                "--profile",
                "pangopup-reference-mini-v1",
                "--source",
                "/secret/source-do-not-disclose",
                "--assembly-report",
                "/secret/report-do-not-disclose",
                "--output",
                "/secret/output-do-not-disclose",
            ],
            1,
            "REFERENCE_INPUT",
            "reference.build",
        ),
    ];
    for (arguments, exit, code, command) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
            .args(arguments)
            .output()
            .expect("run reference CLI error");
        assert_eq!(output.status.code(), Some(exit));
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        let stdout = std::str::from_utf8(&output.stdout).expect("UTF-8 stdout");
        assert!(!stdout.contains("/secret/"));
        let value: serde_json::Value = serde_json::from_str(stdout).expect("error JSON");
        assert_eq!(value["ok"], false);
        assert_eq!(value["command"], command);
        assert_eq!(value["error"]["code"], code);
        assert_eq!(
            serde_jcs::to_vec(&value).expect("canonical error JSON"),
            stdout.trim_end().as_bytes()
        );
    }
}
