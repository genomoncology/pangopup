use pangopup_core::{Grch38Variant, ReferenceProvider};
use pangopup_engine::VariantScorer;
use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
use pangopup_model::{CpuPolicy, ModelKernel, ONNX_RUNTIME_VERSION, ORT_CRATE_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MODEL_BUNDLE: &str = "/home/ian/workspace/data/pangopup-model-018/bundle";
const MODEL_ID: &str = "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43";
const REFERENCE_BUNDLE: &str = "/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle";
const REFERENCE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const MASK_MEMBER: &str = "/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm";
const MASK_BYTES: u64 = 6_703_320;
const MASK_SHA256: &str = "714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";
const EXPECTED_AFFINITY: &str = "0,2,4,6,8,10,12,14";
const WARMUPS: usize = 2;
const SAMPLES: usize = 7;
const CASE_IDS: [&str; 2] = ["M09-insertion-short-plus", "M10-insertion-short-both"];

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
struct OpenMeasurements {
    model_ns: u128,
    reference_ns: u128,
    mask_ns: u128,
    total_ns: u128,
}

#[derive(Serialize)]
struct CaseMeasurement<'a> {
    id: &'a str,
    qualified: bool,
    records: usize,
    warmups: usize,
    samples: usize,
    p50_ns: u128,
    p95_ns: u128,
}

#[derive(Serialize)]
struct RuntimeVersions<'a> {
    pangopup_model: &'a str,
    ort_crate: &'a str,
    onnx_runtime: &'a str,
}

#[derive(Serialize)]
struct AssetIdentities<'a> {
    model_bundle: &'a str,
    reference_bundle: &'a str,
    reference_member: &'a str,
    mask_bytes: u64,
    mask_sha256: &'a str,
}

#[derive(Serialize)]
struct Measurement<'a> {
    schema: &'a str,
    policy: String,
    affinity: &'a str,
    target_os: &'a str,
    target_arch: &'a str,
    runtime: RuntimeVersions<'a>,
    assets: AssetIdentities<'a>,
    component_open: OpenMeasurements,
    maximum_rss_kib: u64,
    cases: Vec<CaseMeasurement<'a>>,
}

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn cpu_allow_list() -> String {
    fs::read_to_string("/proc/self/status")
        .expect("read Linux process status")
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .map(str::trim)
        .map(str::to_owned)
        .expect("Cpus_allowed_list in Linux process status")
}

fn vm_hwm_kib() -> u64 {
    fs::read_to_string("/proc/self/status")
        .expect("read Linux process status")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")
                .and_then(|value| value.split_ascii_whitespace().next())
                .and_then(|value| value.parse().ok())
        })
        .expect("VmHWM in Linux process status")
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

fn assert_exact(case: &CorpusCase, result: &pangopup_core::ModelScoreResult) -> usize {
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
    assert_eq!(records.len(), expected.len(), "{} record count", case.id);
    for (record, expected) in records.iter().zip(expected) {
        assert_eq!(record.gene().to_string(), expected.0, "{} gene", case.id);
        assert!(
            record.warnings().is_empty(),
            "{} unexpected warnings",
            case.id
        );
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

fn percentile(samples: &mut [Duration], numerator: usize, denominator: usize) -> Duration {
    samples.sort_unstable();
    let rank = samples
        .len()
        .saturating_mul(numerator)
        .div_ceil(denominator)
        .saturating_sub(1);
    samples[rank.min(samples.len() - 1)]
}

fn measure_case<'a>(scorer: &mut VariantScorer, case: &'a CorpusCase) -> CaseMeasurement<'a> {
    let variant = Grch38Variant::new(
        case.input.contig.parse().expect("contig"),
        pangopup_core::GenomicPosition::new(case.input.position).expect("position"),
        case.input.reference.clone(),
        case.input.alternate.clone(),
    )
    .expect("variant");
    let mut records = 0;
    for _ in 0..WARMUPS {
        let result = scorer.score(&variant).expect("warmup score");
        records = assert_exact(case, black_box(&result));
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let result = scorer.score(&variant).expect("measured score");
        let elapsed = started.elapsed();
        records = assert_exact(case, black_box(&result));
        samples.push(elapsed);
    }
    let p50_ns = percentile(&mut samples, 50, 100).as_nanos();
    let p95_ns = percentile(&mut samples, 95, 100).as_nanos();
    CaseMeasurement {
        id: &case.id,
        qualified: true,
        records,
        warmups: WARMUPS,
        samples: SAMPLES,
        p50_ns,
        p95_ns,
    }
}

fn selected_cases() -> Vec<CorpusCase> {
    let corpus = fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility corpus");
    let cases: Vec<CorpusCase> = corpus
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("parse corpus JSON");
            let id = value["id"].as_str()?;
            CASE_IDS
                .contains(&id)
                .then(|| serde_json::from_value(value).expect("parse selected model case"))
        })
        .collect();
    assert_eq!(
        cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>(),
        CASE_IDS
    );
    cases
}

#[test]
#[ignore = "coordinator-only complete-request production measurement"]
fn complete_variant_cpu_policy_release_measurement() {
    let policy_text = env::var("PANGOPUP_CPU_POLICY")
        .expect("set PANGOPUP_CPU_POLICY to exactly one reviewed candidate");
    let policy: CpuPolicy = policy_text
        .parse()
        .expect("PANGOPUP_CPU_POLICY must name a reviewed candidate");
    assert_eq!(policy.to_string(), policy_text, "canonical policy spelling");

    let affinity = cpu_allow_list();
    assert_eq!(
        affinity, EXPECTED_AFFINITY,
        "run under taskset -c {EXPECTED_AFFINITY}"
    );

    let all_open_started = Instant::now();
    let model_started = Instant::now();
    let model = ModelKernel::open_with_cpu_policy(Path::new(MODEL_BUNDLE), policy)
        .expect("open authenticated production model");
    let model_ns = model_started.elapsed().as_nanos();
    assert_eq!(model.bundle_identity().as_str(), MODEL_ID);

    let reference_started = Instant::now();
    let reference = ReferenceBundleOpen::open_identified(Path::new(REFERENCE_BUNDLE))
        .expect("open authenticated production reference");
    let reference_ns = reference_started.elapsed().as_nanos();
    assert_eq!(reference.provenance().bundle_id(), REFERENCE_ID);
    let reference_member = reference.identity().sha256().to_owned();

    let mask_started = Instant::now();
    let mask =
        MaskDomainsOpen::open_identified(Path::new(MASK_MEMBER)).expect("open authenticated mask");
    let mask_ns = mask_started.elapsed().as_nanos();
    assert_eq!(mask.identity().bytes(), MASK_BYTES);
    assert_eq!(mask.identity().sha256(), MASK_SHA256);
    let total_ns = all_open_started.elapsed().as_nanos();

    let mut scorer = VariantScorer::new(reference, mask, model);
    let cases = selected_cases();
    let measurements = cases
        .iter()
        .map(|case| measure_case(&mut scorer, case))
        .collect();

    let output = Measurement {
        schema: "pangopup-cpu-policy-measurement-v1",
        policy: policy.to_string(),
        affinity: &affinity,
        target_os: env::consts::OS,
        target_arch: env::consts::ARCH,
        runtime: RuntimeVersions {
            pangopup_model: env!("CARGO_PKG_VERSION"),
            ort_crate: ORT_CRATE_VERSION,
            onnx_runtime: ONNX_RUNTIME_VERSION,
        },
        assets: AssetIdentities {
            model_bundle: MODEL_ID,
            reference_bundle: REFERENCE_ID,
            reference_member: &reference_member,
            mask_bytes: MASK_BYTES,
            mask_sha256: MASK_SHA256,
        },
        component_open: OpenMeasurements {
            model_ns,
            reference_ns,
            mask_ns,
            total_ns,
        },
        maximum_rss_kib: vm_hwm_kib(),
        cases: measurements,
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("measurement JSON")
    );
}
