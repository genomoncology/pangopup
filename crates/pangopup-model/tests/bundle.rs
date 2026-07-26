use pangopup_model::{
    ModelContext, ModelKernel, Strand, bundle_identity, canonical_manifest_bytes, inspect_bundle,
    parse_manifest_bytes, sha256,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn fixture_bundle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pangolin-model-kernel-mini/bundle")
}

fn copied_bundle() -> (TempDir, PathBuf) {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let destination = temporary.path().join("bundle");
    fs::create_dir(&destination).expect("bundle directory");
    for name in ["manifest.json", "NOTICE", "model.onnx"] {
        fs::copy(fixture_bundle().join(name), destination.join(name)).expect("copy fixture member");
    }
    (temporary, destination)
}

fn rebind_model(bundle: &Path, bytes: &[u8]) {
    fs::write(bundle.join("model.onnx"), bytes).expect("write rebound graph");
    let manifest_bytes = fs::read(bundle.join("manifest.json")).expect("manifest bytes");
    let mut manifest = parse_manifest_bytes(&manifest_bytes).expect("fixture manifest");
    let model = manifest
        .members
        .iter_mut()
        .find(|member| member.filename == "model.onnx")
        .expect("model member");
    model.bytes = bytes.len() as u64;
    model.sha256 = sha256(bytes);
    fs::write(
        bundle.join("manifest.json"),
        canonical_manifest_bytes(&manifest).expect("canonical manifest"),
    )
    .expect("write rebound manifest");
}

#[test]
fn checked_miniature_runs_real_ort_at_both_length_bounds() {
    let mut kernel = ModelKernel::open(&fixture_bundle()).expect("open real ORT fixture");
    assert_eq!(
        kernel.bundle_identity().as_str(),
        "sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca"
    );

    let minimum = ModelContext::new(vec![b'N'; 10_001]).expect("minimum context");
    let minimum_scores = kernel
        .infer(&minimum, Strand::Plus)
        .expect("minimum inference");
    assert_eq!(minimum_scores.shape(), [1, 12, 1]);
    assert!(minimum_scores.values().iter().all(|value| *value == 0.0));

    let maximum = ModelContext::new(vec![b'A'; 10_200]).expect("maximum context");
    let maximum_scores = kernel
        .infer(&maximum, Strand::Plus)
        .expect("maximum inference");
    assert_eq!(maximum_scores.shape(), [1, 12, 200]);
    for ordinal in 1..=12 {
        let expected = if [1, 5, 9].contains(&ordinal) {
            1.0
        } else {
            0.0
        };
        assert!(
            maximum_scores
                .channel(ordinal)
                .expect("channel")
                .iter()
                .all(|value| *value == expected)
        );
    }
}

#[test]
fn minus_strand_is_reverse_complemented_and_returned_in_genomic_order() {
    let mut bases = vec![b'N'; 10_017];
    bases[0] = b'A';
    let context = ModelContext::new(bases).expect("sentinel context");
    let mut kernel = ModelKernel::open(&fixture_bundle()).expect("open real ORT fixture");

    let plus = kernel
        .infer(&context, Strand::Plus)
        .expect("plus inference");
    assert_eq!(
        plus.channel(1).expect("A channel"),
        &[
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        ]
    );

    let minus = kernel
        .infer(&context, Strand::Minus)
        .expect("minus inference");
    assert_eq!(
        minus.channel(4).expect("T channel"),
        &[
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        ]
    );
}

#[test]
fn inspection_binds_exact_canonical_manifest_bytes() {
    let bytes = fs::read(fixture_bundle().join("manifest.json")).expect("manifest");
    let parsed = parse_manifest_bytes(&bytes).expect("canonical manifest");
    let inspected = inspect_bundle(&fixture_bundle()).expect("inspect fixture");
    assert_eq!(inspected.bundle_id, bundle_identity(&bytes));
    assert_eq!(canonical_manifest_bytes(&parsed).expect("canonical"), bytes);
}

#[test]
fn rebound_checkpoint_channel_order_is_rejected_by_manifest_contract() {
    let bytes = fs::read(fixture_bundle().join("manifest.json")).expect("manifest");
    let mut manifest = parse_manifest_bytes(&bytes).expect("canonical manifest");
    manifest.conversion.graph.channels.swap(2, 3);
    let rebound = canonical_manifest_bytes(&manifest).expect("rebound canonical manifest");
    assert!(parse_manifest_bytes(&rebound).is_err());
}

#[test]
fn missing_extra_and_corrupt_members_are_rejected() {
    let (_temporary, bundle) = copied_bundle();
    fs::remove_file(bundle.join("NOTICE")).expect("remove notice");
    assert!(inspect_bundle(&bundle).is_err());

    let (_temporary, bundle) = copied_bundle();
    fs::write(bundle.join("extra"), b"unexpected").expect("write extra");
    assert!(inspect_bundle(&bundle).is_err());

    let (_temporary, bundle) = copied_bundle();
    let mut model = fs::read(bundle.join("model.onnx")).expect("model");
    model[0] ^= 1;
    fs::write(bundle.join("model.onnx"), model).expect("corrupt model");
    assert!(inspect_bundle(&bundle).is_err());
}

#[cfg(unix)]
#[test]
fn symlinked_and_multiply_linked_members_are_rejected() {
    use std::os::unix::fs::symlink;

    let (_temporary, bundle) = copied_bundle();
    fs::remove_file(bundle.join("model.onnx")).expect("remove model");
    symlink(
        fixture_bundle().join("model.onnx"),
        bundle.join("model.onnx"),
    )
    .expect("model symlink");
    assert!(inspect_bundle(&bundle).is_err());

    let (_temporary, bundle) = copied_bundle();
    fs::hard_link(bundle.join("model.onnx"), bundle.join("model-link")).expect("hard link");
    fs::remove_file(bundle.join("NOTICE")).expect("remove notice");
    fs::rename(bundle.join("model-link"), bundle.join("NOTICE")).expect("replace notice");
    assert!(inspect_bundle(&bundle).is_err());
}

#[test]
fn semantically_rebound_wrong_graph_name_is_rejected_by_kernel() {
    let (_temporary, bundle) = copied_bundle();
    let mut model = fs::read(bundle.join("model.onnx")).expect("model");
    let mut replacements = 0;
    for offset in 0..=model.len() - b"sequence".len() {
        if &model[offset..offset + b"sequence".len()] == b"sequence" {
            model[offset..offset + b"sequence".len()].copy_from_slice(b"sequencx");
            replacements += 1;
        }
    }
    assert_eq!(replacements, 2);
    rebind_model(&bundle, &model);
    assert!(inspect_bundle(&bundle).is_ok());
    assert!(ModelKernel::open(&bundle).is_err());
}

#[test]
fn initialized_session_never_reopens_replaced_model_path() {
    let (_temporary, bundle) = copied_bundle();
    let mut kernel = ModelKernel::open(&bundle).expect("open kernel");
    fs::rename(
        bundle.join("model.onnx"),
        bundle.join("authenticated-model"),
    )
    .expect("move authenticated member");
    fs::write(bundle.join("model.onnx"), b"replaced").expect("replace path");

    let context = ModelContext::new(vec![b'N'; 10_001]).expect("context");
    let scores = kernel
        .infer(&context, Strand::Plus)
        .expect("session retains authenticated graph");
    assert_eq!(scores.shape(), [1, 12, 1]);
}
