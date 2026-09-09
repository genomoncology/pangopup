use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static SCRATCH_SERIAL: AtomicU64 = AtomicU64::new(0);

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn lookup_bundle() -> PathBuf {
    repository_path("tests/fixtures/snv-regression/bundle")
}

fn fallback_args() -> Vec<String> {
    vec![
        "--reference-bundle".to_owned(),
        repository_path("tests/fixtures/reference-route-test/bundle")
            .display()
            .to_string(),
        "--mask".to_owned(),
        repository_path("tests/fixtures/route-mask/domains.pgm")
            .display()
            .to_string(),
        "--model-bundle".to_owned(),
        repository_path("tests/fixtures/pangolin-model-kernel-mini/bundle")
            .display()
            .to_string(),
    ]
}

/// Run the shipped executable against a private cache home of its own.
///
/// The model cache is on by default and its path comes from `XDG_CACHE_HOME`,
/// or from `HOME` when that is unset. A run that inherits either from the
/// person running the suite reaches that person's own cache file: it fills it
/// with rows scored from miniature fixtures, and a cache that is discarded when
/// the setup that filled it changes would be discarded outright. A private home
/// per run also keeps one test in this file from filling, evicting or
/// discarding the cache another test is reading.
fn isolated(command: &mut Command) -> Output {
    let home = tempfile::tempdir().expect("private cache home");
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).expect("private home");
    command
        .env("XDG_CACHE_HOME", home.path())
        .env("HOME", home.path())
        .output()
        .expect("run pangopup")
}

fn run(args: &[String]) -> Output {
    isolated(Command::new(env!("CARGO_BIN_EXE_pangopup")).args(args))
}

fn modeled_args(variant: &str) -> Vec<String> {
    let mut args = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        variant.to_owned(),
    ];
    args.extend(fallback_args());
    args
}

fn model_only_args(variant: &str) -> Vec<String> {
    let mut args = vec![
        "lookup".to_owned(),
        "--model-only".to_owned(),
        "--variant".to_owned(),
        variant.to_owned(),
    ];
    args.extend(fallback_args());
    args
}

fn error(output: &Output) -> Value {
    assert!(output.stdout.is_empty());
    let line: Value = serde_json::from_slice(&output.stderr).expect("compact JSON error");
    assert_eq!(
        output.stderr.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    line
}

#[test]
fn authoritative_installed_hit_ignores_malformed_cache_environment_and_missing_runtime() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let transport = temp.path().join("transport");
    pangopup_assets::pack_bundle(&lookup_bundle(), &transport).expect("pack miniature SNV");
    let data = temp.path().join("data");
    pangopup_assets::install_transport(&transport, &data).expect("install miniature SNV");

    let output = isolated(
        Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "lookup",
                "--data-dir",
                data.to_str().expect("UTF-8 data path"),
                "--variant",
                "GRCh38:chr12:6801301:G:A",
            ])
            .env("PANGOPUP_MODEL_CACHE", "relative/is/invalid")
            .env("PANGOPUP_MODEL_CACHE_MAX_ENTRIES", "not-a-limit"),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"kind\":\"precomputed\""));
}

#[test]
fn real_file_backed_model_route_json_table_and_filter_are_exact() {
    let output = run(&modeled_args("GRCh38:chr1:5051:A:AC"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let expected = "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5051,\"ref\":\"A\",\"alt\":\"AC\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001.1\",\"stable_gene\":\"ENSG00000000001\",\"gain_score\":\"0.33\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50,\"warnings\":[\"no_annotated_sites\"]}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3\",\"mask_bytes\":260,\"mask_sha256\":\"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827\",\"masked\":true,\"window\":50}}\n";
    assert_eq!(output.stdout, expected.as_bytes());

    let mut round_trip = modeled_args("GRCh38:chr1:5051:A:AC");
    round_trip.extend(["--gene".to_owned(), "ENSG00000000001.1".to_owned()]);
    let output = run(&round_trip);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());

    // The same file-backed path must also be reached by a one-base lookup
    // miss, rather than only by the non-SNV shortcut.
    let output = run(&modeled_args("GRCh38:chr1:5051:A:C"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected_snv = expected.replace("\"alt\":\"AC\"", "\"alt\":\"C\"");
    assert_eq!(output.stdout, expected_snv.as_bytes());

    let mut filtered = modeled_args("GRCh38:NC_000001.11:5051:A:AC");
    filtered.extend(["--gene".to_owned(), "ENSG00000000002".to_owned()]);
    let output = run(&filtered);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("filtered JSON");
    assert!(text.contains("\"contig\":\"chr1\""));
    assert!(text.contains("\"status\":\"not_found\",\"records\":[]"));
    assert!(text.contains("\"kind\":\"model\""));

    let mut table = modeled_args("GRCh38:chr1:5051:A:AC");
    table.extend(["--format".to_owned(), "table".to_owned()]);
    let output = run(&table);
    assert!(output.status.success());
    let expected = concat!(
        "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
        "GRCh38\tchr1\t5051\tA\tAC\tfound\tENSG00000000001.1\t0.33\t-50\t0.00\t-50\t.\t.\t.\tsha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\n",
    );
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn exact_insertion_matches_anchored_form_and_reuses_cache_in_both_directions() {
    let exact = "GRCh38:chr1:INS:5051:5052:C";
    let literal = "GRCh38:chr1:5051:A:AC";
    let exact_output = run(&model_only_args(exact));
    let literal_output = run(&model_only_args(literal));
    assert!(
        exact_output.status.success(),
        "{}",
        String::from_utf8_lossy(&exact_output.stderr)
    );
    assert_eq!(exact_output.stdout, literal_output.stdout);
    let ordinary_exact = run(&modeled_args(exact));
    let ordinary_literal = run(&modeled_args(literal));
    assert!(
        ordinary_exact.status.success(),
        "{}",
        String::from_utf8_lossy(&ordinary_exact.stderr)
    );
    assert_eq!(ordinary_exact.stdout, ordinary_literal.stdout);

    for (first, second, name) in [
        (exact, literal, "exact-first.sqlite3"),
        (literal, exact, "literal-first.sqlite3"),
    ] {
        let temp = tempfile::tempdir().expect("temp");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
        let cache = temp.path().join(name);
        let mut first_args = model_only_args(first);
        first_args.extend(["--model-cache".to_owned(), cache.display().to_string()]);
        let first_output = run(&first_args);
        assert!(
            first_output.status.success(),
            "{}",
            String::from_utf8_lossy(&first_output.stderr)
        );
        let mut second_args = model_only_args(second);
        second_args.extend(["--model-cache".to_owned(), cache.display().to_string()]);
        let second_output = run(&second_args);
        assert!(
            second_output.status.success(),
            "{}",
            String::from_utf8_lossy(&second_output.stderr)
        );
        assert_eq!(first_output.stdout, second_output.stdout);
        let connection = rusqlite::Connection::open(&cache).expect("open cache");
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM entries", [], |row| row.get(0))
            .expect("cache rows");
        assert_eq!(rows, 1, "equivalent forms must share one cache identity");
    }
}

#[test]
fn exact_deletion_matches_the_equivalent_anchored_cli_outcome() {
    let exact = "GRCh38:chr1:DEL:5052:5052:A";
    let literal = "GRCh38:chr1:5051:AA:A";
    for arguments in [
        (model_only_args(exact), model_only_args(literal)),
        (modeled_args(exact), modeled_args(literal)),
    ] {
        let exact_output = run(&arguments.0);
        let literal_output = run(&arguments.1);
        assert_eq!(exact_output.status.code(), Some(2));
        assert_eq!(exact_output.status.code(), literal_output.status.code());
        assert_eq!(exact_output.stdout, literal_output.stdout);
        assert_eq!(exact_output.stderr, literal_output.stderr);
    }
}

#[test]
fn explicit_model_only_bypasses_snv_assets_and_reuses_the_exact_cache() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let cache = temp.path().join("model-only.sqlite3");
    let ordinary = run(&[
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
    ]);
    assert!(ordinary.status.success());
    assert!(String::from_utf8_lossy(&ordinary.stdout).contains("\"kind\":\"precomputed\""));

    let mut args = model_only_args("GRCh38:chr1:5051:A:C");
    args.extend(["--model-cache".to_owned(), cache.display().to_string()]);

    let first = isolated(
        Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args(&args)
            .env("PANGOPUP_DATA_DIR", "relative/invalid-snv-installation"),
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains("\"kind\":\"model\""));

    let second = isolated(
        Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args(&args)
            .env("PANGOPUP_DATA_DIR", "relative/invalid-snv-installation"),
    );
    assert!(second.status.success());
    assert_eq!(second.stdout, first.stdout);
    assert!(second.stderr.is_empty());
    assert!(cache.is_file());

    let mut table_args = args;
    table_args.extend(["--format".to_owned(), "table".to_owned()]);
    let table = run(&table_args);
    assert!(table.status.success());
    let table = String::from_utf8(table.stdout).expect("UTF-8 table");
    assert!(
        table.contains("GRCh38\tchr1\t5051\tA\tC\tfound\tENSG00000000001.1\t0.33\t-50\t0.00\t-50")
    );
    assert!(
        table.contains("sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca")
    );
}

#[test]
fn model_only_batch_is_ordered_and_transactional() {
    let mut ordered = model_only_args("GRCh38:chr1:5051:A:C");
    ordered.splice(
        4..4,
        ["--variant".to_owned(), "GRCh38:chr1:5051:A:AC".to_owned()],
    );
    let output = run(&ordered);
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).expect("UTF-8 output");
    let lines = lines.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"alt\":\"C\""));
    assert!(lines[1].contains("\"alt\":\"AC\""));
    assert!(lines.iter().all(|line| line.contains("\"kind\":\"model\"")));

    let mut rejected = model_only_args("GRCh38:chr1:5051:A:C");
    rejected.splice(
        4..4,
        ["--variant".to_owned(), "GRCh38:chr1:5051:A:TC".to_owned()],
    );
    let output = run(&rejected);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "MODEL_REJECTED");
}

#[test]
fn model_only_grammar_rejects_contradictory_and_duplicate_inputs() {
    for extra in [
        vec!["--model-only", "--bundle", "/missing/snv"],
        vec!["--model-only", "--model-only"],
        vec!["--model-only", "--data-dir", "/tmp/data"],
    ] {
        let mut args = vec![
            "lookup".to_owned(),
            "--variant".to_owned(),
            "GRCh38:chr1:5051:A:C".to_owned(),
        ];
        args.extend(extra.into_iter().map(str::to_owned));
        if args.contains(&"--data-dir".to_owned()) {
            args.extend(fallback_args());
        }
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(error(&output)["code"], "CLI_USAGE");
    }
}

#[test]
fn hit_only_missing_assets_model_required_grammar_and_rejection_are_stable() {
    let hit = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
        "--reference-bundle".to_owned(),
        "/missing/reference".to_owned(),
        "--mask".to_owned(),
        "/missing/mask".to_owned(),
        "--model-bundle".to_owned(),
        "/missing/model".to_owned(),
    ];
    let output = run(&hit);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("hit output")
            .contains("\"kind\":\"precomputed\"")
    );

    let legacy_miss = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr10:1:A:C".to_owned(),
    ];
    let output = run(&legacy_miss);
    assert!(output.status.success());
    let oracle = fs::read_to_string(repository_path(
        "tests/fixtures/snv-regression/expected.jsonl",
    ))
    .expect("read frozen oracle");
    let expected = oracle
        .lines()
        .find(|line| {
            line.contains("\"contig\":\"chr10\",\"position\":1,\"ref\":\"A\",\"alt\":\"C\"")
        })
        .expect("frozen miss");
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes());

    let missing = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    let output = run(&missing);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "MODEL_ASSETS_REQUIRED");

    let missing_bundle = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        "/secret/nonexistent-lookup-bundle".to_owned(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    let output = run(&missing_bundle);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"status\":\"error\",\"code\":\"MODEL_ASSETS_REQUIRED\",\"message\":\"model scoring requires --model-bundle, --reference-bundle, and --mask\",\"details\":null}\n"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("nonexistent-lookup-bundle"));

    let mixed_without_fallback = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    let output = run(&mixed_without_fallback);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "MODEL_ASSETS_REQUIRED");

    let mut partial = missing.clone();
    partial.extend(["--mask".to_owned(), "/mask".to_owned()]);
    let output = run(&partial);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "CLI_USAGE");

    let output = run(&modeled_args("GRCh38:chr1:5051:A:TC"));
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "MODEL_REJECTED");
}

#[test]
fn component_and_scoring_failures_are_redacted_and_transactional() {
    let missing = repository_path("target/secret-model-routing-component");
    let good_reference = repository_path("tests/fixtures/reference-route-test/bundle");
    let good_mask = repository_path("tests/fixtures/route-mask/domains.pgm");
    for (reference, mask, model, code) in [
        (
            missing.clone(),
            missing.clone(),
            missing.clone(),
            "REFERENCE_BUNDLE_INVALID",
        ),
        (
            good_reference.clone(),
            missing.clone(),
            missing.clone(),
            "MASK_INVALID",
        ),
        (
            good_reference.clone(),
            good_mask.clone(),
            missing.clone(),
            "MODEL_BUNDLE_INVALID",
        ),
    ] {
        let args = vec![
            "lookup".to_owned(),
            "--bundle".to_owned(),
            lookup_bundle().display().to_string(),
            "--variant".to_owned(),
            "GRCh38:chr1:5051:A:AC".to_owned(),
            "--reference-bundle".to_owned(),
            reference.display().to_string(),
            "--mask".to_owned(),
            mask.display().to_string(),
            "--model-bundle".to_owned(),
            model.display().to_string(),
        ];
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1));
        let failure = error(&output);
        assert_eq!(failure["code"], code);
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("secret-model-routing-component")
        );
    }

    let serial = SCRATCH_SERIAL.fetch_add(1, Ordering::Relaxed);
    let scratch = repository_path(&format!("target/model-routing-mask-{serial}"));
    fs::create_dir_all(&scratch).expect("create scratch");
    let corrupt_mask = scratch.join("domains.pgm");
    let mut bytes = fs::read(&good_mask).expect("read mask");
    bytes[256..260].copy_from_slice(&u32::MAX.to_le_bytes());
    fs::write(&corrupt_mask, bytes).expect("write touched-payload corruption");
    let mut args = modeled_args("GRCh38:chr1:5051:A:AC");
    let mask_index = args
        .iter()
        .position(|value| value == "--mask")
        .expect("mask flag")
        + 1;
    args[mask_index] = corrupt_mask.display().to_string();
    let output = run(&args);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(error(&output)["code"], "MODEL_SCORING");

    let mut batch = modeled_args("GRCh38:chr12:6801301:G:A");
    batch.splice(
        5..5,
        ["--variant".to_owned(), "GRCh38:chr1:5051:A:TC".to_owned()],
    );
    let output = run(&batch);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error(&output)["code"], "MODEL_REJECTED");
    fs::remove_dir_all(&scratch).expect("remove scratch");
}

#[test]
fn busy_cache_open_falls_back_to_model_and_skips_fill() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private tempdir");
    let cache = temp.path().join("cache.sqlite3");

    let mut seed = modeled_args("GRCh38:chr1:5051:A:AC");
    seed.extend([
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:ACC".to_owned(),
        "--model-cache".to_owned(),
        cache.display().to_string(),
        "--model-cache-max-entries".to_owned(),
        "unlimited".to_owned(),
    ]);
    let seeded = run(&seed);
    assert!(
        seeded.status.success(),
        "{}",
        String::from_utf8_lossy(&seeded.stderr)
    );

    let baseline_cache = temp.path().join("baseline.sqlite3");
    let mut baseline_args = modeled_args("GRCh38:chr1:5051:A:AG");
    baseline_args.extend([
        "--model-cache".to_owned(),
        baseline_cache.display().to_string(),
    ]);
    let baseline = run(&baseline_args);
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );

    let blocker = rusqlite::Connection::open(&cache).expect("open cache blocker");
    let rows: i64 = blocker
        .query_row("SELECT count(*) FROM entries", [], |row| row.get(0))
        .expect("seeded rows");
    assert_eq!(rows, 2);
    blocker
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold cache write lock");

    let mut busy_args = modeled_args("GRCh38:chr1:5051:A:AG");
    busy_args.extend([
        "--model-cache".to_owned(),
        cache.display().to_string(),
        "--model-cache-max-entries".to_owned(),
        "1".to_owned(),
    ]);
    let busy = run(&busy_args);
    blocker.execute_batch("ROLLBACK").expect("release lock");

    assert!(
        busy.status.success(),
        "{}",
        String::from_utf8_lossy(&busy.stderr)
    );
    assert_eq!(busy.stdout, baseline.stdout);
    let rows_after: i64 = blocker
        .query_row("SELECT count(*) FROM entries", [], |row| row.get(0))
        .expect("rows after fallback");
    assert_eq!(rows_after, 2, "busy-open fallback must skip cache fill");
}

/// The path the shipped executable resolves for its default model cache from
/// one environment, spelled the same way `resolve_model_cache_options` spells
/// it: `XDG_CACHE_HOME` when it is set, otherwise `HOME/.cache`.
fn default_model_cache(cache_home: Option<&Path>, home: Option<&Path>) -> PathBuf {
    let root = match (cache_home, home) {
        (Some(root), _) => root.to_owned(),
        (None, Some(home)) => home.join(".cache"),
        (None, None) => {
            panic!("neither XDG_CACHE_HOME nor HOME is set, so no default cache exists")
        }
    };
    root.join("pangopup/model-results.sqlite3")
}

/// What the cache file and its two SQLite sidecars look like right now. `None`
/// for a file that is not there. A byte length and a modification time together
/// move on any write, so comparing this across a run says whether the run wrote.
fn cache_fingerprint(cache: &Path) -> Vec<Option<(u64, std::time::SystemTime)>> {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            fs::metadata(PathBuf::from(format!("{}{suffix}", cache.display())))
                .ok()
                .map(|metadata| {
                    (
                        metadata.len(),
                        metadata.modified().expect("modification time"),
                    )
                })
        })
        .collect()
}

/// No run in this file may reach the model cache of whoever runs the suite.
///
/// The cache is on by default and its path comes from the environment, so a
/// spawn that does not redirect `XDG_CACHE_HOME` and `HOME` fills the operator's
/// own file with rows scored from miniature fixtures. A cache that is discarded
/// when the setup that filled it changes makes that worse: the suite would
/// discard the operator's cache outright rather than only adding to it.
#[test]
fn no_run_here_reaches_the_ambient_model_cache() {
    // The positive control comes first. These arguments have to be the kind of
    // run that fills a default cache, and the file has to take the name the
    // second half watches for -- otherwise the second half proves nothing.
    let probe = tempfile::tempdir().expect("probe cache home");
    fs::set_permissions(probe.path(), fs::Permissions::from_mode(0o700)).expect("private probe");
    let filled = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(model_only_args("GRCh38:chr1:5051:A:AC"))
        .env("XDG_CACHE_HOME", probe.path())
        .env("HOME", probe.path())
        .output()
        .expect("run pangopup");
    assert!(
        filled.status.success(),
        "{}",
        String::from_utf8_lossy(&filled.stderr)
    );
    let probed = default_model_cache(Some(probe.path()), Some(probe.path()));
    assert!(
        probed.is_file(),
        "a modelled run with no --model-cache must fill the default cache under its own cache \
         home, and nothing appeared at {}",
        probed.display()
    );

    // The same run through this file's helper must leave the cache the suite
    // inherited exactly as it found it, whether or not that file exists yet.
    let ambient = default_model_cache(
        std::env::var_os("XDG_CACHE_HOME").as_deref().map(Path::new),
        std::env::var_os("HOME").as_deref().map(Path::new),
    );
    assert_ne!(
        ambient, probed,
        "the inherited cache home must not be the probe, or this proves nothing"
    );
    let before = cache_fingerprint(&ambient);
    let output = run(&model_only_args("GRCh38:chr1:5051:A:C"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        cache_fingerprint(&ambient),
        before,
        "a run reached {}, the model cache of whoever is running the suite",
        ambient.display()
    );
}
