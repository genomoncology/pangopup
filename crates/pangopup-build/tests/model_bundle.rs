use pangopup_build::model::{qualify_model_bundle, validate_checked_model_evidence};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

type JsonMutation = Box<dyn FnOnce(&mut serde_json::Value)>;

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(relative)
}

fn copy_directory(source: &Path, name: &str) -> (TempDir, PathBuf) {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let destination = temporary.path().join(name);
    fs::create_dir(&destination).expect("fixture copy directory");
    for entry in fs::read_dir(source).expect("fixture directory") {
        let entry = entry.expect("fixture entry");
        fs::copy(entry.path(), destination.join(entry.file_name())).expect("copy fixture member");
    }
    (temporary, destination)
}

fn canonical_line(value: &serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_jcs::to_vec(value).expect("canonical JSON");
    bytes.push(b'\n');
    bytes
}

fn rebind_evidence_member(directory: &Path, filename: &str, bytes: &[u8]) {
    fs::write(directory.join(filename), bytes).expect("write evidence member");
    let manifest_path = directory.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest JSON");
    let member = manifest["members"]
        .as_array_mut()
        .expect("members")
        .iter_mut()
        .find(|member| member["filename"] == filename)
        .expect("declared member");
    member["bytes"] = serde_json::json!(bytes.len());
    member["sha256"] = serde_json::json!(format!("sha256:{:x}", Sha256::digest(bytes)));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&manifest).expect("canonical manifest"),
    )
    .expect("write rebound manifest");
}

fn mutate_first_jsonl(
    directory: &Path,
    filename: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let path = directory.join(filename);
    let bytes = fs::read(&path).expect("JSONL bytes");
    let split = bytes.iter().position(|byte| *byte == b'\n').expect("line");
    let mut first: serde_json::Value =
        serde_json::from_slice(&bytes[..split]).expect("first record");
    mutate(&mut first);
    let mut rebound = canonical_line(&first);
    rebound.extend_from_slice(&bytes[split + 1..]);
    rebind_evidence_member(directory, filename, &rebound);
}

#[test]
fn checked_production_evidence_is_exact_and_offline() {
    validate_checked_model_evidence(&fixture("pangolin-model-v1")).expect("checked evidence");
}

#[test]
fn rebound_inventory_order_name_shape_dtype_and_digest_are_rejected() {
    let mutations: Vec<JsonMutation> = vec![
        Box::new(|record| record["checkpoint_ordinal"] = serde_json::json!(2)),
        Box::new(|record| record["name"] = serde_json::json!("rebound.tensor")),
        Box::new(|record| {
            record["shape"] = serde_json::json!([2]);
            record["elements"] = serde_json::json!(2);
            record["tensor_bytes"] = serde_json::json!(8);
        }),
        Box::new(|record| {
            record["dtype"] = serde_json::json!("i64");
            record["tensor_bytes"] = serde_json::json!(8);
        }),
        Box::new(|record| {
            record["value_sha256"] = serde_json::json!(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            );
        }),
    ];
    for (index, mutation) in mutations.into_iter().enumerate() {
        let (_temporary, evidence) =
            copy_directory(&fixture("pangolin-model-v1"), &format!("evidence-{index}"));
        mutate_first_jsonl(&evidence, "checkpoint-tensors.jsonl", mutation);
        assert!(validate_checked_model_evidence(&evidence).is_err());
    }
}

#[test]
fn rebound_golden_bit_truncation_and_extra_record_are_rejected() {
    let (_temporary, evidence) = copy_directory(&fixture("pangolin-model-v1"), "golden-bit");
    mutate_first_jsonl(&evidence, "kernel-golden.jsonl", |record| {
        record["score_bits"][0] = serde_json::json!("00000000");
    });
    assert!(validate_checked_model_evidence(&evidence).is_err());

    let (_temporary, evidence) = copy_directory(&fixture("pangolin-model-v1"), "truncated");
    let path = evidence.join("kernel-golden.jsonl");
    let mut bytes = fs::read(&path).expect("goldens");
    bytes.pop();
    rebind_evidence_member(&evidence, "kernel-golden.jsonl", &bytes);
    assert!(validate_checked_model_evidence(&evidence).is_err());

    let (_temporary, evidence) = copy_directory(&fixture("pangolin-model-v1"), "extra");
    let path = evidence.join("kernel-golden.jsonl");
    let mut bytes = fs::read(&path).expect("goldens");
    let first_end = bytes.iter().position(|byte| *byte == b'\n').expect("line");
    let first = bytes[..=first_end].to_vec();
    bytes.extend_from_slice(&first);
    rebind_evidence_member(&evidence, "kernel-golden.jsonl", &bytes);
    assert!(validate_checked_model_evidence(&evidence).is_err());
}

#[test]
fn semantically_rebound_mini_golden_is_rejected_by_real_inference() {
    let (_temporary, evidence) = copy_directory(
        &fixture("pangolin-model-kernel-mini/evidence"),
        "mini-evidence",
    );
    mutate_first_jsonl(&evidence, "kernel-golden.jsonl", |record| {
        let current = record["score_bits"][0].as_str().expect("score bit");
        record["score_bits"][0] = serde_json::json!(if current == "00000000" {
            "0000803f"
        } else {
            "00000000"
        });
    });
    assert!(
        qualify_model_bundle(&fixture("pangolin-model-kernel-mini/bundle"), &evidence).is_err()
    );
}
