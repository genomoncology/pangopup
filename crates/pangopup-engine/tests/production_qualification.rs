use pangopup_core::{Grch38Variant, ReferenceProvider};
use pangopup_engine::VariantScorer;
use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
use pangopup_model::{CpuPolicy, ModelKernel, ModelRepresentation, inspect_bundle};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const REFERENCE_BUNDLE: &str = "/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle";
const REFERENCE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const MASK_MEMBER: &str = "/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm";
const MASK_BYTES: u64 = 6_703_320;
const MASK_SHA256: &str = "714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";
const ARRAY_RECEIPT_BYTES: u64 = 3_026;
const ARRAY_RECEIPT_SHA256: &str =
    "3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b";

#[derive(Deserialize)]
struct CorpusCase {
    id: String,
    input: Input,
    strands: Vec<StrandCase>,
}

#[derive(Deserialize)]
struct Input {
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "alt")]
    alternate: String,
}

#[derive(Deserialize)]
struct StrandCase {
    dtype: String,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    masked: Vec<ExpectedScore>,
}

#[derive(Deserialize)]
struct ExpectedScore {
    gene: String,
    gain_bits: String,
    gain_position: i16,
    loss_bits: String,
    loss_position: i16,
}

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn checked_array_receipt_sha256() -> String {
    let mut bytes = Vec::with_capacity(ARRAY_RECEIPT_BYTES as usize);
    File::open(repository_path(
        "tests/fixtures/pangolin-engine-v1/post-ensemble-sha256.tsv",
    ))
    .expect("open post-ensemble receipt")
    .take(ARRAY_RECEIPT_BYTES + 1)
    .read_to_end(&mut bytes)
    .expect("read bounded post-ensemble receipt");
    assert_eq!(bytes.len() as u64, ARRAY_RECEIPT_BYTES);
    let observed = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(observed, ARRAY_RECEIPT_SHA256);
    observed
}

fn compat_f32(bits: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(bits, 16).expect("f32 bits"))
}

fn compat_f64(bits: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(bits, 16).expect("f64 bits"))
}

fn hundredths(dtype: &str, bits: &str, loss: bool) -> u8 {
    let rounded = match dtype {
        "f32" => f64::from((compat_f32(bits) * 100.0_f32).round_ties_even()),
        "f64" => (compat_f64(bits) * 100.0_f64).round_ties_even(),
        other => panic!("unexpected dtype {other}"),
    };
    (if loss { -rounded } else { rounded }) as u8
}

#[test]
#[ignore = "opens the retained production model, reference, and mask exactly once"]
fn retained_production_assets_match_all_masked_model_cases() {
    let array_receipt_sha256 = checked_array_receipt_sha256();
    let model_bundle = env::var_os("PANGOPUP_MODEL_BUNDLE")
        .expect("set PANGOPUP_MODEL_BUNDLE to one authenticated candidate");
    let policy_text =
        env::var("PANGOPUP_CPU_POLICY").unwrap_or_else(|_| CpuPolicy::SEQUENTIAL_1_1.to_string());
    let policy: CpuPolicy = policy_text.parse().expect("closed CPU policy");
    assert!(
        policy == CpuPolicy::SEQUENTIAL_1_1 || policy == CpuPolicy::SEQUENTIAL_8_1,
        "qualification permits only Ticket 022 selected policies"
    );
    let inspection = inspect_bundle(Path::new(&model_bundle)).expect("inspect model bundle");
    let model = match inspection.representation {
        ModelRepresentation::Singleton => {
            ModelKernel::open_with_cpu_policy(Path::new(&model_bundle), policy)
        }
        ModelRepresentation::ZeroPaddedBatch | ModelRepresentation::PairedStrandBatch => {
            ModelKernel::open_experimental_with_cpu_policy(Path::new(&model_bundle), policy)
        }
    }
    .expect("open production model");
    let model_id = model.bundle_identity().to_string();
    let model_profile = model.profile().to_owned();
    let representation = model.representation().to_string();
    let reference =
        ReferenceBundleOpen::open(Path::new(REFERENCE_BUNDLE)).expect("open production reference");
    assert_eq!(reference.provenance().bundle_id(), REFERENCE_ID);
    let (mask, mask_identity) =
        MaskDomainsOpen::open_qualification(Path::new(MASK_MEMBER), MASK_BYTES, MASK_SHA256)
            .expect("authenticate production mask");
    assert_eq!(mask_identity.bytes(), MASK_BYTES);
    assert_eq!(mask_identity.sha256(), MASK_SHA256);

    let corpus = fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility corpus");
    let cases: Vec<CorpusCase> = corpus
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("parse corpus JSON");
            (value["kind"] == "model")
                .then(|| serde_json::from_value(value).expect("parse model case"))
        })
        .collect();
    assert_eq!(cases.len(), 14);

    let mut scorer = if inspection.representation == ModelRepresentation::Singleton {
        VariantScorer::new(reference, mask, model)
    } else {
        VariantScorer::new_experimental(reference, mask, model)
    };
    let mut record_count = 0_usize;
    for case in &cases {
        let variant = Grch38Variant::new(
            case.input.contig.parse().expect("contig"),
            pangopup_core::GenomicPosition::new(case.input.position).expect("position"),
            case.input.reference.clone(),
            case.input.alternate.clone(),
        )
        .expect("variant");
        let result = scorer.score(&variant).expect("production model score");
        let records = result.records().expect("scored result");
        let expected: Vec<_> = case
            .strands
            .iter()
            .flat_map(|strand| {
                strand.expected.masked.iter().map(|score| {
                    (
                        score.gene.as_str(),
                        hundredths(&strand.dtype, &score.gain_bits, false),
                        score.gain_position,
                        hundredths(&strand.dtype, &score.loss_bits, true),
                        score.loss_position,
                    )
                })
            })
            .collect();
        assert_eq!(records.len(), expected.len(), "{}", case.id);
        for (record, expected) in records.iter().zip(expected) {
            assert_eq!(record.gene().to_string(), expected.0, "{} gene", case.id);
            assert_eq!(
                record.score().gain().hundredths(),
                expected.1,
                "{} gain",
                case.id
            );
            assert_eq!(
                record.score().gain_position().get(),
                expected.2,
                "{} gain position",
                case.id
            );
            assert_eq!(
                record.score().loss().hundredths(),
                expected.3,
                "{} loss",
                case.id
            );
            assert_eq!(
                record.score().loss_position().get(),
                expected.4,
                "{} loss position",
                case.id
            );
        }
        record_count += records.len();
    }

    println!(
        "{{\"status\":\"qualified\",\"cases\":{},\"records\":{},\"model\":\"{}\",\"model_profile\":\"{}\",\"representation\":\"{}\",\"policy\":\"{}\",\"reference\":\"{}\",\"mask_bytes\":{},\"mask_sha256\":\"{}\",\"post_ensemble_receipt_sha256\":\"{}\"}}",
        cases.len(),
        record_count,
        model_id,
        model_profile,
        representation,
        policy,
        REFERENCE_ID,
        mask_identity.bytes(),
        mask_identity.sha256(),
        array_receipt_sha256
    );
}
