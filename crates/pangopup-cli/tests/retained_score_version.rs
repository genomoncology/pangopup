//! A retained command-line score names the software that produced it.
//!
//! `pangopup lookup` output pins the assets that answered exactly. Ticket 0054
//! settled that it must also pin the software: a plain version string, not a
//! digest, on every result line of every invocation shape and both routes.
//!
//! The value belongs in `provenance`, beside the asset identifiers that already
//! say where the number came from, and it is the last key there. The HTTP score
//! item is out of scope: ticket 0052 settled what that item carries, and
//! `lib.rs` holds the boundary proof that the shared renderer leaves it alone.

mod support;

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Output,
};

use support::Spawn;

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn lookup_bundle() -> PathBuf {
    repository_path("tests/fixtures/snv-regression/bundle")
}

fn model_asset_args() -> Vec<String> {
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
/// `support` is the one place allowed to name the executable, and it redirects
/// every command at a cache home of its own. A run that inherited the operator's
/// `XDG_CACHE_HOME` would fill, evict or discard that person's model cache.
fn run(spawn: Spawn) -> Output {
    let output = spawn.output().expect("run pangopup");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_args(args: &[String]) -> Output {
    run(support::pangopup().args(args))
}

/// The version this build reports elsewhere. A consumer compares a retained
/// line against a running deployment, so the two must be the same string.
/// `support` owns the one reading of it, so every test in the suite that pins
/// printed bytes compares against the same string.
fn reported_version() -> String {
    support::software_version()
}

fn stamped_field(version: &str) -> String {
    format!(",\"software_version\":\"{version}\"")
}

fn lines(output: &Output) -> Vec<String> {
    String::from_utf8(output.stdout.clone())
        .expect("UTF-8 JSONL")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// An installed data directory, built from the miniature SNV bundle the suite
/// ships. This is the shape an operator reaches after `pangopup sync`.
fn installed_data_dir(scratch: &Path) -> PathBuf {
    let transport = scratch.join("transport");
    pangopup_assets::pack_bundle(&lookup_bundle(), &transport).expect("pack miniature SNV");
    let data = scratch.join("data");
    pangopup_assets::install_transport(&transport, &data).expect("install miniature SNV");
    data
}

fn private_scratch() -> tempfile::TempDir {
    let scratch = tempfile::tempdir().expect("temp");
    fs::set_permissions(scratch.path(), fs::Permissions::from_mode(0o700))
        .expect("private scratch");
    scratch
}

#[test]
fn every_invocation_shape_stamps_the_version_the_build_reports() {
    let version = reported_version();
    let scratch = private_scratch();
    let data = installed_data_dir(scratch.path());

    let installed = vec![
        "lookup".to_owned(),
        "--data-dir".to_owned(),
        data.display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
    ];
    let bundle = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
        "--variant".to_owned(),
        "GRCh38:chr1:1:A:C".to_owned(),
    ];
    let mut modeled = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    modeled.extend(model_asset_args());
    let mut model_only = vec![
        "lookup".to_owned(),
        "--model-only".to_owned(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    model_only.extend(model_asset_args());

    for (shape, args, kind) in [
        ("installed data directory", installed, "precomputed"),
        ("explicit bundle", bundle, "precomputed"),
        ("explicit model assets", modeled, "model"),
        ("--model-only", model_only, "model"),
    ] {
        let output = run_args(&args);
        let rendered = lines(&output);
        assert!(
            !rendered.is_empty(),
            "{shape} printed no result line to stamp"
        );
        for line in &rendered {
            let value: Value = serde_json::from_str(line).expect("a result line is JSON");
            let provenance = value
                .get("provenance")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{shape} printed a line without provenance"));
            assert_eq!(
                provenance.get("kind").and_then(Value::as_str),
                Some(kind),
                "{shape} took an unexpected route"
            );
            assert_eq!(
                provenance.get("software_version").and_then(Value::as_str),
                Some(version.as_str()),
                "{shape} must name the version this build reports: {line}"
            );
            // `serde_json` sorts an object's keys when it parses, so key
            // order is read off the printed bytes. Provenance is the last
            // object on the line, and the version is the last field in it.
            assert!(
                line.ends_with(&format!("{}}}}}", stamped_field(&version))),
                "{shape} must carry the version as the last provenance key: {line}"
            );
        }
    }
}

#[test]
fn stamping_the_modeled_line_moves_nothing_else_and_leaves_the_table_alone() {
    let version = reported_version();
    let mut args = vec![
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:AC".to_owned(),
    ];
    args.extend(model_asset_args());

    // The bytes this route printed before the version joined them. Removing the
    // one stamped field must give them back exactly, so no score, position,
    // status, gene name or existing provenance field can move behind the
    // addition.
    let unstamped = "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5051,\"ref\":\"A\",\"alt\":\"AC\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001.1\",\"stable_gene\":\"ENSG00000000001\",\"gain_score\":\"0.33\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50,\"warnings\":[\"no_annotated_sites\"]}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3\",\"mask_bytes\":260,\"mask_sha256\":\"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827\",\"masked\":true,\"window\":50}}\n";
    let stamped = unstamped.replace(
        "\"window\":50}}",
        &format!("\"window\":50{}}}}}", stamped_field(&version)),
    );

    let output = run_args(&args);
    let printed = String::from_utf8(output.stdout).expect("UTF-8 JSONL");
    assert_eq!(printed, stamped);
    assert_eq!(
        printed.matches("software_version").count(),
        1,
        "the version is stamped once per line"
    );
    assert_eq!(printed.replace(&stamped_field(&version), ""), unstamped);

    let mut table = args.clone();
    table.extend(["--format".to_owned(), "table".to_owned()]);
    let table_output = run_args(&table);
    let expected_table = concat!(
        "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
        "GRCh38\tchr1\t5051\tA\tAC\tfound\tENSG00000000001.1\t0.33\t-50\t0.00\t-50\t.\t.\t.\tsha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca\n",
    );
    assert_eq!(
        table_output.stdout,
        expected_table.as_bytes(),
        "the table form is unaffected"
    );
}

#[test]
fn stamping_the_precomputed_line_adds_one_field_and_keeps_the_rest() {
    let version = reported_version();
    let output = run_args(&[
        "lookup".to_owned(),
        "--bundle".to_owned(),
        lookup_bundle().display().to_string(),
        "--variant".to_owned(),
        "GRCh38:chr12:6801301:G:A".to_owned(),
        "--gene".to_owned(),
        "ENSG00000010610".to_owned(),
    ]);
    let printed = String::from_utf8(output.stdout).expect("UTF-8 JSONL");
    assert_eq!(printed.matches("software_version").count(), 1);

    // The line the tree printed before the version joined it. The naming leaf
    // rides on the committed gene-name index, so the comparison is on the
    // scored and provenance structure rather than on the whole byte string.
    let unstamped: Value =
        serde_json::from_str(&printed.replace(&stamped_field(&version), "")).expect("JSON");
    let object = unstamped.as_object().expect("a result line is an object");
    // Parsed keys come back sorted. Order is proved on the printed bytes by the
    // modeled test above and by the last-key assertion beside it; what is
    // proved here is the set, so a field added or dropped behind the version
    // fails.
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "alt",
            "assembly",
            "contig",
            "position",
            "provenance",
            "records",
            "ref",
            "source_reference_ambiguities",
            "status",
        ]
    );
    assert_eq!(object["status"], Value::from("found"));
    let record = object["records"][0].as_object().expect("one scored record");
    assert_eq!(record["gene"], Value::from("ENSG00000010610"));
    assert_eq!(record["gain_score"], Value::from("0.00"));
    assert_eq!(record["gain_position"], Value::from(-50));
    assert_eq!(record["loss_score"], Value::from("0.00"));
    assert_eq!(record["loss_position"], Value::from(-50));
    let provenance = object["provenance"]
        .as_object()
        .expect("precomputed provenance");
    assert_eq!(
        provenance.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "bundle_id",
            "kind",
            "masked",
            "source_archive_md5",
            "source_doi",
            "window",
        ],
        "the version is the only field this ticket adds"
    );
    assert_eq!(
        provenance["source_doi"],
        Value::from("10.5281/zenodo.15649338")
    );
    assert_eq!(provenance["masked"], Value::from(true));
    assert_eq!(provenance["window"], Value::from(50));
}
