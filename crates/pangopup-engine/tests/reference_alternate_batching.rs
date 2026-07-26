use pangopup_core::{Grch38Variant, ModelScoreResult, ReferenceProvider};
use pangopup_engine::VariantScorer;
use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
use pangopup_model::{
    CpuPolicy, ModelKernel, ModelRepresentation, ONNX_RUNTIME_VERSION, ORT_CRATE_VERSION,
    inspect_bundle,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const REFERENCE_BUNDLE: &str = "/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle";
const REFERENCE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const MASK_MEMBER: &str = "/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm";
const MASK_BYTES: u64 = 6_703_320;
const MASK_SHA256: &str = "714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";
const EXPECTED_AFFINITY: &str = "0,2,4,6,8,10,12,14";
const WARMUPS: usize = 1;
const SAMPLES: usize = 5;
const CASE_IDS: [&str; 6] = [
    "M07-mnv-plus",
    "M08-mnv-both-strands",
    "M09-insertion-short-plus",
    "M10-insertion-short-both",
    "M12-deletion-short-plus",
    "M13-deletion-short-both",
];

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

#[derive(Serialize)]
struct CaseMeasurement {
    id: String,
    qualified: bool,
    records: usize,
    warmups: usize,
    samples: usize,
    p50_ns: u128,
    p95_ns: u128,
    session_invocations: usize,
    logical_context_evaluations: usize,
    batch_size: usize,
    padded_input_elements: usize,
}

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn status_field(prefix: &str) -> String {
    fs::read_to_string("/proc/self/status")
        .expect("read Linux process status")
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .map(str::to_owned)
        .expect("required Linux process status field")
}

fn vm_hwm_kib() -> u64 {
    status_field("VmHWM:")
        .split_ascii_whitespace()
        .next()
        .expect("VmHWM value")
        .parse()
        .expect("VmHWM integer")
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

fn assert_exact(case: &CorpusCase, result: &ModelScoreResult) -> usize {
    let records = result.records().expect("scored result");
    let expected = case
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
        .collect::<Vec<_>>();
    assert_eq!(records.len(), expected.len(), "{} record count", case.id);
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
    records.len()
}

fn percentile(samples: &mut [Duration], numerator: usize) -> Duration {
    samples.sort_unstable();
    let rank = samples
        .len()
        .saturating_mul(numerator)
        .div_ceil(100)
        .saturating_sub(1);
    samples[rank.min(samples.len() - 1)]
}

fn measure_case(scorer: &mut VariantScorer, case: &CorpusCase) -> CaseMeasurement {
    let variant = Grch38Variant::new(
        case.input.contig.parse().expect("contig"),
        pangopup_core::GenomicPosition::new(case.input.position).expect("position"),
        case.input.reference.clone(),
        case.input.alternate.clone(),
    )
    .expect("variant");
    let warmup = scorer.score(&variant).expect("warmup score");
    let mut records = assert_exact(case, black_box(&warmup));
    let expected_accounting = scorer.last_model_accounting();
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let result = scorer.score(&variant).expect("measured score");
        let elapsed = started.elapsed();
        records = assert_exact(case, black_box(&result));
        assert_eq!(scorer.last_model_accounting(), expected_accounting);
        samples.push(elapsed);
    }
    CaseMeasurement {
        id: case.id.clone(),
        qualified: true,
        records,
        warmups: WARMUPS,
        samples: SAMPLES,
        p50_ns: percentile(&mut samples.clone(), 50).as_nanos(),
        p95_ns: percentile(&mut samples, 95).as_nanos(),
        session_invocations: expected_accounting.session_invocations,
        logical_context_evaluations: expected_accounting.logical_context_evaluations,
        batch_size: expected_accounting.batch_size,
        padded_input_elements: expected_accounting.padded_input_elements,
    }
}

fn selected_cases() -> Vec<CorpusCase> {
    let corpus = fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility corpus");
    let cases = corpus
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("corpus JSON");
            CASE_IDS
                .contains(&value["id"].as_str()?)
                .then(|| serde_json::from_value::<CorpusCase>(value).expect("selected model case"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>(),
        CASE_IDS
    );
    cases
}

fn directory_bytes(path: &Path) -> u64 {
    ["manifest.json", "model.onnx", "NOTICE"]
        .iter()
        .map(|name| fs::metadata(path.join(name)).expect("bundle member").len())
        .sum()
}

#[test]
#[ignore = "coordinator-only rotated production candidate measurement"]
fn reference_alternate_batching_release_measurement() {
    let bundle = env::var_os("PANGOPUP_MODEL_BUNDLE")
        .expect("set PANGOPUP_MODEL_BUNDLE to one Ticket 022 candidate");
    let bundle = Path::new(&bundle);
    let policy_text = env::var("PANGOPUP_CPU_POLICY")
        .expect("set PANGOPUP_CPU_POLICY to sequential:1/1, sequential:8/1, or parallel:1/8");
    let policy: CpuPolicy = policy_text.parse().expect("closed CPU policy");
    assert!(
        policy == CpuPolicy::SEQUENTIAL_1_1
            || policy == CpuPolicy::SEQUENTIAL_8_1
            || policy == CpuPolicy::PARALLEL_1_8,
        "policy is outside Ticket 022 matrix"
    );
    let affinity = status_field("Cpus_allowed_list:");
    assert_eq!(affinity, EXPECTED_AFFINITY);
    let round = env::var("PANGOPUP_MEASUREMENT_ROUND").expect("set round 1, 2, or 3");
    assert!(matches!(round.as_str(), "1" | "2" | "3"));

    let inspection = inspect_bundle(bundle).expect("inspect candidate");
    if policy == CpuPolicy::PARALLEL_1_8 {
        assert_eq!(
            inspection.representation,
            ModelRepresentation::PairedStrandBatch,
            "parallel diagnostic is paired-only"
        );
    }
    let all_open = Instant::now();
    let model_open = Instant::now();
    let model = if inspection.representation == ModelRepresentation::Singleton {
        ModelKernel::open_with_cpu_policy(bundle, policy)
    } else {
        ModelKernel::open_experimental_with_cpu_policy(bundle, policy)
    }
    .expect("open measured model");
    let model_ns = model_open.elapsed().as_nanos();
    let model_id = model.bundle_identity().to_string();
    let representation = model.representation().to_string();

    let reference_open = Instant::now();
    let reference =
        ReferenceBundleOpen::open_identified(Path::new(REFERENCE_BUNDLE)).expect("reference");
    let reference_ns = reference_open.elapsed().as_nanos();
    assert_eq!(reference.provenance().bundle_id(), REFERENCE_ID);
    let reference_member = reference.identity().sha256().to_owned();

    let mask_open = Instant::now();
    let mask = MaskDomainsOpen::open_identified(Path::new(MASK_MEMBER)).expect("mask");
    let mask_ns = mask_open.elapsed().as_nanos();
    assert_eq!(mask.identity().bytes(), MASK_BYTES);
    assert_eq!(mask.identity().sha256(), MASK_SHA256);
    let total_ns = all_open.elapsed().as_nanos();

    let mut scorer = if inspection.representation == ModelRepresentation::Singleton {
        VariantScorer::new(reference, mask, model)
    } else {
        VariantScorer::new_experimental(reference, mask, model)
    };
    let cases = selected_cases()
        .iter()
        .map(|case| measure_case(&mut scorer, case))
        .collect::<Vec<_>>();
    let output = serde_json::json!({
        "schema": "pangopup-reference-alternate-batching-measurement-v1",
        "policy": policy.to_string(),
        "representation": representation,
        "affinity": affinity,
        "round": round,
        "runtime": {
            "pangopup_model": env!("CARGO_PKG_VERSION"),
            "ort_crate": ORT_CRATE_VERSION,
            "onnx_runtime": ONNX_RUNTIME_VERSION,
        },
        "assets": {
            "model_bundle": model_id,
            "model_profile": inspection.profile,
            "graph_bytes": inspection.model_bytes,
            "model_bytes": inspection.model_bytes,
            "bundle_bytes": directory_bytes(bundle),
            "reference_bundle": REFERENCE_ID,
            "reference_member": reference_member,
            "mask_bytes": MASK_BYTES,
            "mask_sha256": MASK_SHA256,
        },
        "component_open": {
            "model_ns": model_ns,
            "reference_ns": reference_ns,
            "mask_ns": mask_ns,
            "total_ns": total_ns,
        },
        "maximum_rss_kib": vm_hwm_kib(),
        "cases": cases,
    });
    println!(
        "{}",
        serde_json::to_string(&output).expect("measurement JSON")
    );
}
