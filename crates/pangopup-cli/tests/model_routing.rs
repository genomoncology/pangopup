use serde_json::Value;
use std::{
    fs,
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
fn real_file_backed_model_route_json_table_and_filter_are_exact() {
    let output = run(&modeled_args("GRCh38:chr1:5051:A:AC"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let expected = "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5051,\"ref\":\"A\",\"alt\":\"AC\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001.1\",\"gain_score\":\"0.33\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50,\"warnings\":[\"no_annotated_sites\"]}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"reference_bundle_id\":\"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3\",\"mask_bytes\":260,\"mask_sha256\":\"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827\",\"masked\":true,\"window\":50}}\n";
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
