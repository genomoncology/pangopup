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

fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(args)
        .output()
        .expect("run pangopup")
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

    let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args([
            "lookup",
            "--data-dir",
            data.to_str().expect("UTF-8 data path"),
            "--variant",
            "GRCh38:chr12:6801301:G:A",
        ])
        .env("PANGOPUP_MODEL_CACHE", "relative/is/invalid")
        .env("PANGOPUP_MODEL_CACHE_MAX_ENTRIES", "not-a-limit")
        .output()
        .expect("run isolated Pangopup");
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
    let expected = "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5051,\"ref\":\"A\",\"alt\":\"AC\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001.1\",\"gain_score\":\"0.33\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50,\"warnings\":[\"no_annotated_sites\"]}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3\",\"mask_bytes\":260,\"mask_sha256\":\"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827\",\"masked\":true,\"window\":50}}\n";
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

    let first = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(&args)
        .env("PANGOPUP_DATA_DIR", "relative/invalid-snv-installation")
        .output()
        .expect("first model-only process");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains("\"kind\":\"model\""));

    let second = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(&args)
        .env("PANGOPUP_DATA_DIR", "relative/invalid-snv-installation")
        .output()
        .expect("second model-only process");
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
