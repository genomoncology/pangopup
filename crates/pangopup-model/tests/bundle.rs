use pangopup_model::{
    BundleKind, ConversionManifestV2, CpuExecutionMode, CpuPolicy, CpuPolicyError,
    ExporterSettingsV2, GraphContractV2, IntraOpThreads, ModelContext, ModelKernel,
    ModelManifestV2, ModelRepresentation, Strand, TensorContract, bundle_identity,
    canonical_manifest_bytes, canonical_manifest_v2_bytes, inspect_bundle, parse_manifest_bytes,
    parse_manifest_v2_bytes, sha256,
};
use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn fixture_bundle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pangolin-model-kernel-mini/bundle")
}

fn candidate_bundle(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
        .join("bundle")
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
fn cpu_policy_candidates_are_closed_and_round_trip() {
    let candidates = [
        CpuPolicy::SEQUENTIAL_AUTO_1,
        CpuPolicy::SEQUENTIAL_1_1,
        CpuPolicy::SEQUENTIAL_2_1,
        CpuPolicy::SEQUENTIAL_4_1,
        CpuPolicy::SEQUENTIAL_8_1,
        CpuPolicy::PARALLEL_1_2,
        CpuPolicy::PARALLEL_1_4,
        CpuPolicy::PARALLEL_1_8,
    ];
    for candidate in candidates {
        assert_eq!(
            candidate.to_string().parse::<CpuPolicy>(),
            Ok(candidate),
            "{candidate}"
        );
    }
    for invalid in [
        "",
        "sequential",
        "sequential:auto/0",
        "sequential:3/1",
        "sequential:1/2",
        "parallel:auto/2",
        "parallel:1/3",
        "SEQUENTIAL:1/1",
    ] {
        assert_eq!(
            invalid.parse::<CpuPolicy>(),
            Err(CpuPolicyError::UnknownCandidate),
            "{invalid}"
        );
    }
}

#[test]
fn cpu_policy_validates_sequential_inter_op_and_pins_default() {
    assert_eq!(
        CpuPolicy::new(
            CpuExecutionMode::Sequential,
            IntraOpThreads::Auto,
            NonZeroUsize::new(2).expect("nonzero")
        ),
        Err(CpuPolicyError::SequentialInterOp)
    );
    assert_eq!(
        CpuPolicy::new(
            CpuExecutionMode::Parallel,
            IntraOpThreads::Fixed(NonZeroUsize::MIN),
            NonZeroUsize::new(2).expect("nonzero")
        )
        .expect("valid parallel policy"),
        CpuPolicy::PARALLEL_1_2
    );
    assert_eq!(CpuPolicy::production_default(), CpuPolicy::SEQUENTIAL_1_1);
}

#[test]
fn cpu_policy_rejects_thread_counts_outside_the_ort_domain() {
    let maximum = NonZeroUsize::new(i32::MAX as usize).expect("positive i32 maximum");
    let too_large =
        NonZeroUsize::new(i32::MAX as usize + 1).expect("usize represents i32 maximum plus one");

    assert!(
        CpuPolicy::new(
            CpuExecutionMode::Parallel,
            IntraOpThreads::Fixed(maximum),
            maximum
        )
        .is_ok()
    );
    assert_eq!(
        CpuPolicy::new(
            CpuExecutionMode::Parallel,
            IntraOpThreads::Fixed(too_large),
            NonZeroUsize::MIN
        ),
        Err(CpuPolicyError::ThreadCountOutOfRange("intra-op"))
    );
    assert_eq!(
        CpuPolicy::new(CpuExecutionMode::Parallel, IntraOpThreads::Auto, too_large),
        Err(CpuPolicyError::ThreadCountOutOfRange("inter-op"))
    );

    for candidate in [
        "sequential:auto/1",
        "sequential:1/1",
        "sequential:2/1",
        "sequential:4/1",
        "sequential:8/1",
        "parallel:1/2",
        "parallel:1/4",
        "parallel:1/8",
    ] {
        assert!(candidate.parse::<CpuPolicy>().is_ok(), "{candidate}");
    }
}

#[test]
fn miniature_runs_under_both_cpu_execution_families() {
    let context = ModelContext::new(vec![b'A'; 10_001]).expect("valid context");
    let mut sequential =
        ModelKernel::open_with_cpu_policy(&fixture_bundle(), CpuPolicy::SEQUENTIAL_AUTO_1)
            .expect("open sequential automatic policy");
    let sequential_scores = sequential
        .infer(&context, Strand::Plus)
        .expect("sequential inference");

    let mut parallel =
        ModelKernel::open_with_cpu_policy(&fixture_bundle(), CpuPolicy::PARALLEL_1_2)
            .expect("open parallel policy");
    let parallel_scores = parallel
        .infer(&context, Strand::Plus)
        .expect("parallel inference");

    assert_eq!(sequential_scores.shape(), [1, 12, 1]);
    assert_eq!(parallel_scores.shape(), sequential_scores.shape());
    assert_eq!(parallel_scores.values(), sequential_scores.values());
}

#[test]
fn zero_padded_candidate_runs_real_ort_at_b1_b2_and_b4_with_bounds_and_strands() {
    let mut kernel = ModelKernel::open_experimental_with_cpu_policy(
        &candidate_bundle("pangolin-model-kernel-mini-zero-padded"),
        CpuPolicy::SEQUENTIAL_1_1,
    )
    .expect("open zero-padded candidate");
    let mut minimum_bases = vec![b'N'; 10_001];
    minimum_bases[0] = b'A';
    let minimum = ModelContext::new(minimum_bases).expect("minimum context");
    let mut maximum_bases = vec![b'N'; 10_200];
    maximum_bases[0] = b'C';
    let maximum = ModelContext::new(maximum_bases).expect("maximum context");

    let b1 = kernel
        .infer_batch(&[pangopup_model::BatchItem {
            context: &minimum,
            strand: Strand::Plus,
        }])
        .expect("B1");
    assert_eq!(b1.items().len(), 1);
    assert_eq!(b1.items()[0].shape(), [1, 12, 1]);
    assert_eq!(b1.items()[0].channel(1).expect("A channel"), &[1.0]);
    assert_eq!(b1.accounting().batch_size, 1);

    let b2 = kernel
        .infer_batch(&[
            pangopup_model::BatchItem {
                context: &minimum,
                strand: Strand::Plus,
            },
            pangopup_model::BatchItem {
                context: &minimum,
                strand: Strand::Minus,
            },
        ])
        .expect("B2");
    assert_eq!(b2.items().len(), 2);
    assert_eq!(b2.items()[0].channel(1).expect("plus A"), &[1.0]);
    assert_eq!(b2.items()[1].channel(4).expect("minus T"), &[1.0]);
    assert_eq!(b2.accounting().batch_size, 2);

    let b4 = kernel
        .infer_batch(&[
            pangopup_model::BatchItem {
                context: &minimum,
                strand: Strand::Plus,
            },
            pangopup_model::BatchItem {
                context: &maximum,
                strand: Strand::Plus,
            },
            pangopup_model::BatchItem {
                context: &minimum,
                strand: Strand::Minus,
            },
            pangopup_model::BatchItem {
                context: &maximum,
                strand: Strand::Minus,
            },
        ])
        .expect("B4");
    assert_eq!(
        b4.items()
            .iter()
            .map(pangopup_model::ReplicateScores::score_length)
            .collect::<Vec<_>>(),
        [1, 200, 1, 200]
    );
    assert_eq!(b4.accounting().session_invocations, 1);
    assert_eq!(b4.accounting().logical_context_evaluations, 4);
    assert_eq!(b4.accounting().batch_size, 4);
    assert_eq!(b4.accounting().padded_input_elements, 1_592);

    let paired = kernel
        .infer_variant(&[
            pangopup_model::StrandPair {
                reference: &minimum,
                alternate: &maximum,
                strand: Strand::Plus,
            },
            pangopup_model::StrandPair {
                reference: &minimum,
                alternate: &maximum,
                strand: Strand::Minus,
            },
        ])
        .expect("two-strand B4 pair grouping");
    assert_eq!(paired.pairs().len(), 2);
    assert_eq!(paired.pairs()[0].reference().score_length(), 1);
    assert_eq!(paired.pairs()[0].alternate().score_length(), 200);
    assert_eq!(
        paired.pairs()[1].reference().channel(4).expect("minus T"),
        &[1.0]
    );
    assert_eq!(paired.accounting().batch_size, 4);
    assert_eq!(paired.accounting().padded_input_elements, 1_592);

    assert!(matches!(
        kernel.infer_batch(&[]),
        Err(pangopup_model::ModelError::BatchCount {
            observed: 0,
            maximum: 4
        })
    ));
    let too_many = [pangopup_model::BatchItem {
        context: &minimum,
        strand: Strand::Plus,
    }; 5];
    assert!(matches!(
        kernel.infer_batch(&too_many),
        Err(pangopup_model::ModelError::BatchCount {
            observed: 5,
            maximum: 4
        })
    ));
}

#[test]
fn paired_candidate_runs_real_ort_at_b1_b2_unequal_extremes_and_rejects_shape_order() {
    let mut kernel = ModelKernel::open_experimental_with_cpu_policy(
        &candidate_bundle("pangolin-model-kernel-mini-paired-strand"),
        CpuPolicy::SEQUENTIAL_1_1,
    )
    .expect("open paired candidate");
    let mut reference_bases = vec![b'N'; 10_001];
    reference_bases[0] = b'A';
    let reference = ModelContext::new(reference_bases).expect("minimum reference");
    let mut alternate_bases = vec![b'N'; 10_200];
    alternate_bases[0] = b'C';
    let alternate = ModelContext::new(alternate_bases).expect("maximum alternate");

    let b1 = kernel
        .infer_variant(&[pangopup_model::StrandPair {
            reference: &reference,
            alternate: &alternate,
            strand: Strand::Plus,
        }])
        .expect("paired B1");
    assert_eq!(b1.pairs().len(), 1);
    assert_eq!(b1.pairs()[0].reference().shape(), [1, 12, 1]);
    assert_eq!(b1.pairs()[0].alternate().shape(), [1, 12, 200]);
    assert_eq!(b1.accounting().batch_size, 1);

    let b2 = kernel
        .infer_variant(&[
            pangopup_model::StrandPair {
                reference: &reference,
                alternate: &alternate,
                strand: Strand::Plus,
            },
            pangopup_model::StrandPair {
                reference: &reference,
                alternate: &alternate,
                strand: Strand::Minus,
            },
        ])
        .expect("paired B2");
    assert_eq!(b2.pairs().len(), 2);
    assert_eq!(
        b2.pairs()[0].reference().channel(1).expect("plus A"),
        &[1.0]
    );
    assert_eq!(
        b2.pairs()[1].reference().channel(4).expect("minus T"),
        &[1.0]
    );
    assert_eq!(b2.accounting().session_invocations, 1);
    assert_eq!(b2.accounting().logical_context_evaluations, 4);
    assert_eq!(b2.accounting().batch_size, 2);
    assert_eq!(b2.accounting().padded_input_elements, 0);

    let other_reference = ModelContext::new(vec![b'N'; 10_002]).expect("other reference");
    assert!(matches!(
        kernel.infer_variant(&[
            pangopup_model::StrandPair {
                reference: &reference,
                alternate: &alternate,
                strand: Strand::Plus,
            },
            pangopup_model::StrandPair {
                reference: &other_reference,
                alternate: &alternate,
                strand: Strand::Minus,
            },
        ]),
        Err(pangopup_model::ModelError::InvalidBundle(
            "paired strand contexts have inconsistent allele lengths"
        ))
    ));
    let too_many = [pangopup_model::StrandPair {
        reference: &reference,
        alternate: &alternate,
        strand: Strand::Plus,
    }; 3];
    assert!(matches!(
        kernel.infer_variant(&too_many),
        Err(pangopup_model::ModelError::BatchCount {
            observed: 3,
            maximum: 2
        })
    ));
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

#[test]
fn v2_contract_rejects_rebound_representation_shapes_profile_and_unknown_fields() {
    let legacy_bytes = fs::read(fixture_bundle().join("manifest.json")).expect("v1 manifest");
    let legacy = parse_manifest_bytes(&legacy_bytes).expect("v1 manifest contract");
    let manifest = ModelManifestV2 {
        schema: "pangopup-model-bundle-v2".to_owned(),
        kind: BundleKind::SyntheticTest,
        profile: "pangopup-model-kernel-mini-zero-padded-v2".to_owned(),
        source: legacy.source,
        conversion: ConversionManifestV2 {
            converter: legacy.conversion.converter,
            checkpoint_inventory: legacy.conversion.checkpoint_inventory,
            qualification_evidence: legacy.conversion.qualification_evidence,
            environment: legacy.conversion.environment,
            graph: GraphContractV2 {
                representation: ModelRepresentation::ZeroPaddedBatch,
                opset: 17,
                inputs: vec![TensorContract {
                    name: "sequence".to_owned(),
                    element_type: "f32".to_owned(),
                    shape: vec!["B".to_owned(), "4".to_owned(), "N".to_owned()],
                }],
                outputs: vec![TensorContract {
                    name: "replicate_scores".to_owned(),
                    element_type: "f32".to_owned(),
                    shape: vec!["B".to_owned(), "12".to_owned(), "N-10000".to_owned()],
                }],
                channels: legacy.conversion.graph.channels,
                exporter: ExporterSettingsV2 {
                    dynamo: false,
                    constant_folding: true,
                    dynamic_axes: vec![0, 2],
                },
            },
        },
        members: legacy.members,
    };
    let bytes = canonical_manifest_v2_bytes(&manifest).expect("canonical v2 manifest");
    let manifest = parse_manifest_v2_bytes(&bytes).expect("exact v2 manifest");
    assert_eq!(
        canonical_manifest_v2_bytes(&manifest).expect("canonical"),
        bytes
    );

    let mut wrong_shape = manifest.clone();
    wrong_shape.conversion.graph.inputs[0].shape[0] = "1".to_owned();
    let rebound = canonical_manifest_v2_bytes(&wrong_shape).expect("canonical rebound");
    assert!(parse_manifest_v2_bytes(&rebound).is_err());

    let mut wrong_profile = manifest;
    wrong_profile.profile = "pangopup-model-kernel-mini-v1".to_owned();
    let rebound = canonical_manifest_v2_bytes(&wrong_profile).expect("canonical rebound");
    assert!(parse_manifest_v2_bytes(&rebound).is_err());

    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("manifest JSON");
    value["unexpected"] = serde_json::json!(true);
    let rebound = serde_jcs::to_vec(&value).expect("canonical JSON");
    assert!(parse_manifest_v2_bytes(&rebound).is_err());
}
