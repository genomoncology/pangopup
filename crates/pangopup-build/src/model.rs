//! Authenticated maintainer tooling for the raw Pangolin CPU model kernel.

use crate::CommandError;
use pangopup_model::{
    BundleKind, CheckpointIdentity, ConversionEnvironment, ConversionManifest, ExporterSettings,
    FileIdentity, GraphContract, MINI_PROFILE, ModelContext, ModelKernel, ModelManifest,
    ModelSource, PRODUCTION_PROFILE, PRODUCTION_UPSTREAM_COMMIT, Strand, TensorContract,
    canonical_manifest_bytes, inspect_bundle, production_checkpoints, sha256,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

const EVIDENCE_SCHEMA: &str = "pangopup-model-evidence-v1";
const EVIDENCE_PROFILE: &str = "pangolin-1.0.2-5cf94b8-kernel-evidence-v1";
const MAX_EVIDENCE_MANIFEST: u64 = 256 * 1024;
const MAX_INVENTORY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GOLDEN_BYTES: u64 = 8 * 1024 * 1024;
const MAX_HELPER_OUTPUT: usize = 1024 * 1024;
const MODEL_SOURCE_BYTES: u64 = 3_011;
const MODEL_SOURCE_SHA: &str =
    "sha256:4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8";
const CORPUS_MANIFEST_SHA: &str =
    "sha256:fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b";
const CORPUS_CASES_SHA: &str =
    "sha256:2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const CORPUS_NOTICE_SHA: &str =
    "sha256:edb9addea955d89820b82cc77c86b2e879f843081dcd57b0940dcefe1698d5da";
const UPSTREAM_URL: &str =
    "https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7";
const MODEL_NOTICE: &str = "Pangopup Pangolin model bundle\n\nPangolin source and trained model checkpoints\nProject author: Tony Zeng\nSource: https://github.com/tkzeng/Pangolin\nPinned commit: 5cf94b8db938c658391b4305cd7ce33297d44ff7\nLicense: GNU General Public License v3.0 only\nCitation: Zeng T, Li YI. Predicting RNA splicing from DNA sequence using Pangolin. Genome Biology 23, 103 (2022).\n\nThe twelve authenticated upstream checkpoint containers were converted without quantization to one ONNX graph by Pangopup. The graph retains one selected raw score channel from each checkpoint. Pangopup does not claim that the converted model is independently licensed from Pangolin.\n";
pub const SYNTHETIC_NOTICE: &str = "Pangopup synthetic model-kernel test fixture\n\nThis tiny ONNX graph contains no Pangolin checkpoint weights or genomic data. It exists only to exercise the authenticated model-bundle and ONNX Runtime path in offline tests.\n";

const EVIDENCE_HELPER: &str = include_str!("../../../tools/pangolin-model/evidence.py");
const CONVERTER_HELPER: &str = include_str!("../../../tools/pangolin-model/convert.py");
const COMPAT_CASES: &str = include_str!("../../../tests/fixtures/pangolin-compat-v1/cases.jsonl");
const CHECKED_EVIDENCE_MANIFEST: &[u8] =
    include_bytes!("../../../tests/fixtures/pangolin-model-v1/manifest.json");
const CHECKED_INVENTORY: &[u8] =
    include_bytes!("../../../tests/fixtures/pangolin-model-v1/checkpoint-tensors.jsonl");
const CHECKED_GOLDENS: &[u8] =
    include_bytes!("../../../tests/fixtures/pangolin-model-v1/kernel-golden.jsonl");

static STAGE_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct EvidenceArguments {
    pub upstream: PathBuf,
    pub python: PathBuf,
    pub corpus: PathBuf,
    pub output: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ConvertArguments {
    pub upstream: PathBuf,
    pub python: PathBuf,
    pub evidence: PathBuf,
    pub output: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusIdentity {
    manifest: FileIdentity,
    cases: FileIdentity,
    notice: FileIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceCounts {
    checkpoints: u64,
    tensors: u64,
    tensors_per_checkpoint: u64,
    elements_per_checkpoint: u64,
    int64_counters_per_checkpoint: u64,
    cases: u64,
    strands: u64,
    sequence_evaluations: u64,
    channel_arrays: u64,
    scalar_values: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceManifest {
    schema: String,
    profile: String,
    source: ModelSource,
    evidence_helper: FileIdentity,
    converter_helper: FileIdentity,
    corpus: CorpusIdentity,
    environment: ConversionEnvironment,
    counts: EvidenceCounts,
    members: Vec<FileIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InventoryRecord {
    checkpoint_ordinal: u8,
    name: String,
    shape: Vec<u64>,
    dtype: String,
    elements: u64,
    tensor_bytes: u64,
    value_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GoldenRecord {
    case_id: String,
    context_sha256: String,
    strand: String,
    allele: String,
    checkpoint_ordinal: u8,
    score_bits: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub profile: String,
    pub evidence_id: String,
    pub tensors: u64,
    pub channel_arrays: u64,
    pub scalar_values: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConvertOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub profile: String,
    pub bundle_id: String,
    pub model_bytes: u64,
    pub raw_checkpoint_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub schema: String,
    pub kind: BundleKind,
    pub profile: String,
    pub bundle_id: String,
    pub model_bytes: u64,
    pub notice_bytes: u64,
    pub checkpoints: usize,
    pub channels: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeIdentity {
    pub rustc: &'static str,
    pub ort_crate: &'static str,
    pub onnx_runtime: &'static str,
    pub execution_provider: &'static str,
    pub execution_mode: &'static str,
    pub graph_optimization: &'static str,
    pub intra_op_threads: u8,
    pub inter_op_threads: u8,
    pub architecture: &'static str,
    pub cpu: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct QualifyOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub profile: String,
    pub bundle_id: String,
    pub cases: u64,
    pub strands: u64,
    pub sequence_evaluations: u64,
    pub channel_arrays: u64,
    pub scalar_comparisons: u64,
    pub maximum_absolute_error: f64,
    pub runtime: RuntimeIdentity,
}

struct EvidenceOpen {
    manifest: EvidenceManifest,
    manifest_bytes: Vec<u8>,
    inventory_bytes: Vec<u8>,
    golden_bytes: Vec<u8>,
    goldens: Vec<GoldenRecord>,
}

pub fn create_model_evidence(
    arguments: &EvidenceArguments,
) -> Result<EvidenceOutcome, CommandError> {
    validate_python(&arguments.python)?;
    validate_upstream(&arguments.upstream)?;
    validate_corpus(&arguments.corpus)?;
    let (stage, parent) = create_stage(&arguments.output, "model-evidence")?;
    let mut guard = StageGuard::new(stage.clone());
    let inventory = stage.join("checkpoint-tensors.jsonl");
    let golden = stage.join("kernel-golden.jsonl");
    let result = (|| {
        run_python_helper(
            &arguments.python,
            EVIDENCE_HELPER,
            &[
                OsStr::new("--upstream"),
                arguments.upstream.as_os_str(),
                OsStr::new("--corpus"),
                arguments.corpus.as_os_str(),
                OsStr::new("--inventory"),
                inventory.as_os_str(),
                OsStr::new("--golden"),
                golden.as_os_str(),
            ],
            "model evidence helper",
            false,
        )?;
        let inventory_identity =
            identity_for(&inventory, "checkpoint-tensors.jsonl", MAX_INVENTORY_BYTES)?;
        let golden_identity = identity_for(&golden, "kernel-golden.jsonl", MAX_GOLDEN_BYTES)?;
        let manifest = production_evidence_manifest(inventory_identity, golden_identity);
        let manifest_bytes = canonical_evidence_manifest(&manifest)?;
        write_synced(&stage.join("manifest.json"), &manifest_bytes)?;
        sync_directory(&stage)?;
        let opened = open_evidence(&stage)?;
        publish_stage(&stage, &parent, &arguments.output, &mut guard)?;
        Ok(EvidenceOutcome {
            status: "ok",
            command: "model.evidence",
            profile: opened.manifest.profile,
            evidence_id: sha256(&opened.manifest_bytes),
            tensors: opened.manifest.counts.tensors,
            channel_arrays: opened.manifest.counts.channel_arrays,
            scalar_values: opened.manifest.counts.scalar_values,
        })
    })();
    if result.is_err() {
        guard.cleanup()?;
    }
    result
}

pub fn convert_model_bundle(arguments: &ConvertArguments) -> Result<ConvertOutcome, CommandError> {
    validate_python(&arguments.python)?;
    validate_upstream(&arguments.upstream)?;
    let evidence = open_evidence(&arguments.evidence)?;
    require_production_evidence(&evidence)?;
    let (stage, parent) = create_stage(&arguments.output, "model-bundle")?;
    let mut guard = StageGuard::new(stage.clone());
    let result = (|| {
        let model_path = stage.join("model.onnx");
        run_python_helper(
            &arguments.python,
            CONVERTER_HELPER,
            &[
                OsStr::new("--upstream"),
                arguments.upstream.as_os_str(),
                OsStr::new("--output"),
                model_path.as_os_str(),
            ],
            "model converter helper",
            true,
        )?;
        let model = identity_for(&model_path, "model.onnx", pangopup_model::MAX_MODEL_BYTES)?;
        write_synced(&stage.join("NOTICE"), MODEL_NOTICE.as_bytes())?;
        let notice = identity_for(
            &stage.join("NOTICE"),
            "NOTICE",
            pangopup_model::MAX_NOTICE_BYTES,
        )?;
        let manifest_identity = FileIdentity {
            filename: "manifest.json".to_owned(),
            bytes: evidence.manifest_bytes.len() as u64,
            sha256: sha256(&evidence.manifest_bytes),
        };
        let manifest = production_model_manifest(
            evidence.manifest.members[0].clone(),
            manifest_identity,
            notice,
            model,
        );
        let bytes = canonical_manifest_bytes(&manifest).map_err(model_error)?;
        write_synced(&stage.join("manifest.json"), &bytes)?;
        sync_directory(&stage)?;
        let inspection = inspect_bundle(&stage).map_err(model_error)?;
        let _kernel = ModelKernel::open(&stage).map_err(model_error)?;
        publish_stage(&stage, &parent, &arguments.output, &mut guard)?;
        Ok(ConvertOutcome {
            status: "ok",
            command: "model.convert",
            profile: inspection.profile,
            bundle_id: inspection.bundle_id.to_string(),
            model_bytes: inspection.model_bytes,
            raw_checkpoint_bytes: 34_527_852,
        })
    })();
    if result.is_err() {
        guard.cleanup()?;
    }
    result
}

pub fn inspect_model_bundle(bundle: &Path) -> Result<InspectOutcome, CommandError> {
    let inspection = inspect_bundle(bundle).map_err(model_error)?;
    Ok(InspectOutcome {
        status: "ok",
        command: "model.inspect",
        schema: inspection.schema,
        kind: inspection.kind,
        profile: inspection.profile,
        bundle_id: inspection.bundle_id.to_string(),
        model_bytes: inspection.model_bytes,
        notice_bytes: inspection.notice_bytes,
        checkpoints: inspection.checkpoints,
        channels: inspection.channels,
    })
}

/// Validate the checked production evidence trust root without invoking
/// Python, loading checkpoints, or opening a model bundle.
pub fn validate_checked_model_evidence(evidence: &Path) -> Result<(), CommandError> {
    let evidence = open_evidence(evidence)?;
    require_production_evidence(&evidence)
}

pub fn qualify_model_bundle(
    bundle: &Path,
    evidence: &Path,
) -> Result<QualifyOutcome, CommandError> {
    qualify_model_bundle_after_open(bundle, evidence, || Ok(()))
}

fn qualify_model_bundle_after_open(
    bundle: &Path,
    evidence: &Path,
    after_open: impl FnOnce() -> Result<(), CommandError>,
) -> Result<QualifyOutcome, CommandError> {
    let evidence = open_evidence(evidence)?;
    let mut kernel = ModelKernel::open(bundle).map_err(model_error)?;
    let kind = kernel.bundle_kind();
    match kind {
        BundleKind::Production => require_production_evidence(&evidence)?,
        BundleKind::SyntheticTest => require_mini_evidence(&evidence)?,
    }
    let sequences = match kind {
        BundleKind::Production => production_sequences()?,
        BundleKind::SyntheticTest => miniature_sequences(),
    };
    after_open()?;
    let mut expected = BTreeMap::new();
    for golden in &evidence.goldens {
        expected.insert(golden_key(golden), golden);
    }
    let mut maximum = 0.0_f64;
    let mut arrays = 0_u64;
    let mut scalars = 0_u64;
    for sequence in &sequences {
        let context = ModelContext::new(sequence.bases.as_bytes()).map_err(model_error)?;
        let scores = kernel
            .infer(&context, sequence.strand)
            .map_err(model_error)?;
        for ordinal in 1..=12 {
            let key = (
                sequence.case_id.as_str(),
                sequence.strand.symbol(),
                sequence.allele.as_str(),
                ordinal,
            );
            let golden = expected
                .remove(&key)
                .ok_or_else(|| invalid("missing golden channel"))?;
            let actual = scores
                .channel(ordinal as usize)
                .ok_or_else(|| invalid("missing runtime channel"))?;
            if actual.len() != golden.score_bits.len() {
                return Err(invalid("golden/runtime score length differs"));
            }
            for (value, bits) in actual.iter().copied().zip(&golden.score_bits) {
                let expected_value = decode_f32(bits)?;
                maximum = maximum.max(f64::from((value - expected_value).abs()));
                scalars += 1;
            }
            arrays += 1;
        }
    }
    if !expected.is_empty() {
        return Err(invalid("extra golden channels"));
    }
    if maximum > 1e-5 {
        return Err(CommandError::new(
            "MODEL_QUALIFICATION",
            format!("maximum absolute error {maximum} exceeds 0.00001"),
        ));
    }
    let counts = &evidence.manifest.counts;
    if arrays != counts.channel_arrays || scalars != counts.scalar_values {
        return Err(invalid("qualification count mismatch"));
    }
    Ok(QualifyOutcome {
        status: "ok",
        command: "model.qualify",
        profile: kernel.profile().to_owned(),
        bundle_id: kernel.bundle_identity().to_string(),
        cases: counts.cases,
        strands: counts.strands,
        sequence_evaluations: counts.sequence_evaluations,
        channel_arrays: arrays,
        scalar_comparisons: scalars,
        maximum_absolute_error: maximum,
        runtime: RuntimeIdentity {
            rustc: env!("PANGOPUP_RUSTC_VERSION"),
            ort_crate: "2.0.0-rc.12",
            onnx_runtime: "1.24.2",
            execution_provider: "CPUExecutionProvider",
            execution_mode: "sequential",
            graph_optimization: "all",
            intra_op_threads: 1,
            inter_op_threads: 1,
            architecture: std::env::consts::ARCH,
            cpu: cpu_identity(),
        },
    })
}

fn production_evidence_manifest(inventory: FileIdentity, golden: FileIdentity) -> EvidenceManifest {
    EvidenceManifest {
        schema: EVIDENCE_SCHEMA.to_owned(),
        profile: EVIDENCE_PROFILE.to_owned(),
        source: production_source(),
        evidence_helper: embedded_identity("tools/pangolin-model/evidence.py", EVIDENCE_HELPER),
        converter_helper: embedded_identity("tools/pangolin-model/convert.py", CONVERTER_HELPER),
        corpus: expected_corpus(),
        environment: conversion_environment(),
        counts: production_counts(),
        members: vec![inventory, golden],
    }
}

fn production_model_manifest(
    inventory: FileIdentity,
    evidence_manifest: FileIdentity,
    notice: FileIdentity,
    model: FileIdentity,
) -> ModelManifest {
    ModelManifest {
        schema: pangopup_model::MODEL_BUNDLE_SCHEMA.to_owned(),
        kind: BundleKind::Production,
        profile: PRODUCTION_PROFILE.to_owned(),
        source: production_source(),
        conversion: ConversionManifest {
            converter: embedded_identity("tools/pangolin-model/convert.py", CONVERTER_HELPER),
            checkpoint_inventory: inventory,
            qualification_evidence: evidence_manifest,
            environment: conversion_environment(),
            graph: graph_contract(),
        },
        members: vec![notice, model],
    }
}

fn production_source() -> ModelSource {
    ModelSource {
        identity: "pangolin-1.0.2-5cf94b8-checkpoints-v1".to_owned(),
        upstream_url: UPSTREAM_URL.to_owned(),
        upstream_commit: PRODUCTION_UPSTREAM_COMMIT.to_owned(),
        model_source: FileIdentity {
            filename: "pangolin/model.py".to_owned(),
            bytes: MODEL_SOURCE_BYTES,
            sha256: MODEL_SOURCE_SHA.to_owned(),
        },
        checkpoints: production_checkpoints(),
    }
}

fn graph_contract() -> GraphContract {
    GraphContract {
        opset: 17,
        input: TensorContract {
            name: "sequence".to_owned(),
            element_type: "f32".to_owned(),
            shape: vec!["1".to_owned(), "4".to_owned(), "N".to_owned()],
        },
        output: TensorContract {
            name: "replicate_scores".to_owned(),
            element_type: "f32".to_owned(),
            shape: vec!["1".to_owned(), "12".to_owned(), "N-10000".to_owned()],
        },
        channels: pangopup_model::EXPECTED_CHANNEL_MAPPING.to_vec(),
        exporter: ExporterSettings {
            dynamo: false,
            constant_folding: true,
            dynamic_axis: 2,
        },
    }
}

fn conversion_environment() -> ConversionEnvironment {
    ConversionEnvironment {
        python: "3.13.5".to_owned(),
        pytorch: "2.7.1+cpu".to_owned(),
        numpy: "2.5.1".to_owned(),
        onnx: "1.19.1".to_owned(),
    }
}

fn production_counts() -> EvidenceCounts {
    EvidenceCounts {
        checkpoints: 12,
        tensors: 3_024,
        tensors_per_checkpoint: 252,
        elements_per_checkpoint: 699_116,
        int64_counters_per_checkpoint: 32,
        cases: 14,
        strands: 18,
        sequence_evaluations: 36,
        channel_arrays: 432,
        scalar_values: 45_756,
    }
}

fn expected_corpus() -> CorpusIdentity {
    CorpusIdentity {
        manifest: FileIdentity {
            filename: "manifest.json".to_owned(),
            bytes: 5_337,
            sha256: CORPUS_MANIFEST_SHA.to_owned(),
        },
        cases: FileIdentity {
            filename: "cases.jsonl".to_owned(),
            bytes: 220_071,
            sha256: CORPUS_CASES_SHA.to_owned(),
        },
        notice: FileIdentity {
            filename: "NOTICE".to_owned(),
            bytes: 1_652,
            sha256: CORPUS_NOTICE_SHA.to_owned(),
        },
    }
}

fn embedded_identity(filename: &str, source: &str) -> FileIdentity {
    FileIdentity {
        filename: filename.to_owned(),
        bytes: source.len() as u64,
        sha256: sha256(source.as_bytes()),
    }
}

fn canonical_evidence_manifest(manifest: &EvidenceManifest) -> Result<Vec<u8>, CommandError> {
    serde_jcs::to_vec(manifest).map_err(|_| invalid("evidence manifest encoding"))
}

fn open_evidence(directory: &Path) -> Result<EvidenceOpen, CommandError> {
    require_exact_members(
        directory,
        &[
            "checkpoint-tensors.jsonl",
            "kernel-golden.jsonl",
            "manifest.json",
        ],
    )?;
    let manifest_bytes =
        read_regular_bounded(&directory.join("manifest.json"), MAX_EVIDENCE_MANIFEST)?;
    let manifest: EvidenceManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| invalid("evidence manifest JSON"))?;
    if canonical_evidence_manifest(&manifest)? != manifest_bytes {
        return Err(invalid("evidence manifest is not canonical"));
    }
    validate_evidence_manifest(&manifest)?;
    let inventory_bytes = read_regular_bounded(
        &directory.join("checkpoint-tensors.jsonl"),
        MAX_INVENTORY_BYTES,
    )?;
    let golden_bytes =
        read_regular_bounded(&directory.join("kernel-golden.jsonl"), MAX_GOLDEN_BYTES)?;
    validate_declared_member(&manifest.members[0], &inventory_bytes)?;
    validate_declared_member(&manifest.members[1], &golden_bytes)?;
    validate_inventory(&inventory_bytes, &manifest.counts)?;
    let goldens = validate_goldens(&golden_bytes, &manifest.counts, &manifest.profile)?;
    Ok(EvidenceOpen {
        manifest,
        manifest_bytes,
        inventory_bytes,
        golden_bytes,
        goldens,
    })
}

fn validate_evidence_manifest(manifest: &EvidenceManifest) -> Result<(), CommandError> {
    if manifest.schema != EVIDENCE_SCHEMA {
        return Err(incompatible("evidence schema"));
    }
    if manifest.members.len() != 2
        || manifest.members[0].filename != "checkpoint-tensors.jsonl"
        || manifest.members[1].filename != "kernel-golden.jsonl"
    {
        return Err(invalid("evidence member order"));
    }
    if manifest.profile == EVIDENCE_PROFILE {
        if manifest.source != production_source()
            || manifest.evidence_helper
                != embedded_identity("tools/pangolin-model/evidence.py", EVIDENCE_HELPER)
            || manifest.converter_helper
                != embedded_identity("tools/pangolin-model/convert.py", CONVERTER_HELPER)
            || manifest.corpus != expected_corpus()
            || manifest.environment != conversion_environment()
            || manifest.counts != production_counts()
        {
            return Err(invalid("production evidence identity"));
        }
    } else if manifest.profile != MINI_PROFILE {
        return Err(incompatible("evidence profile"));
    }
    Ok(())
}

fn validate_inventory(bytes: &[u8], counts: &EvidenceCounts) -> Result<(), CommandError> {
    let records: Vec<InventoryRecord> = parse_jsonl(bytes, "checkpoint inventory")?;
    if records.len() as u64 != counts.tensors {
        return Err(invalid("inventory record count"));
    }
    let mut by_checkpoint: BTreeMap<u8, (u64, u64, u64)> = BTreeMap::new();
    let mut previous = (0_u8, "");
    for record in &records {
        if record.checkpoint_ordinal == 0
            || record.checkpoint_ordinal > counts.checkpoints as u8
            || record.name.is_empty()
            || !valid_sha(&record.value_sha256)
            || !matches!(record.dtype.as_str(), "f32" | "i64")
        {
            return Err(invalid("inventory record identity"));
        }
        let element_size = if record.dtype == "f32" { 4 } else { 8 };
        let shape_elements = record
            .shape
            .iter()
            .try_fold(1_u64, |total, value| total.checked_mul(*value))
            .ok_or_else(|| invalid("inventory shape overflow"))?;
        if shape_elements != record.elements
            || record.tensor_bytes != record.elements * element_size
        {
            return Err(invalid("inventory tensor shape"));
        }
        let key = (record.checkpoint_ordinal, record.name.as_str());
        if record.checkpoint_ordinal < previous.0
            || (record.checkpoint_ordinal == previous.0 && record.name == previous.1)
        {
            return Err(invalid("inventory order"));
        }
        previous = key;
        let totals = by_checkpoint
            .entry(record.checkpoint_ordinal)
            .or_insert((0, 0, 0));
        totals.0 += 1;
        totals.1 += record.elements;
        totals.2 += u64::from(record.dtype == "i64");
    }
    if by_checkpoint.len() as u64 != counts.checkpoints
        || by_checkpoint.values().any(|totals| {
            *totals
                != (
                    counts.tensors_per_checkpoint,
                    counts.elements_per_checkpoint,
                    counts.int64_counters_per_checkpoint,
                )
        })
    {
        return Err(invalid("inventory checkpoint totals"));
    }
    Ok(())
}

fn validate_goldens(
    bytes: &[u8],
    counts: &EvidenceCounts,
    profile: &str,
) -> Result<Vec<GoldenRecord>, CommandError> {
    let records: Vec<GoldenRecord> = parse_jsonl(bytes, "kernel golden")?;
    if records.len() as u64 != counts.channel_arrays {
        return Err(invalid("golden array count"));
    }
    let sequences = if profile == EVIDENCE_PROFILE {
        production_sequences()?
    } else {
        miniature_sequences()
    };
    let mut expected_keys = Vec::new();
    for ordinal in 1..=counts.checkpoints as u8 {
        for sequence in &sequences {
            expected_keys.push((
                ordinal,
                sequence.case_id.as_str(),
                sequence.strand.symbol(),
                sequence.allele.as_str(),
                sequence.context_sha256.as_str(),
                sequence.bases.len() - 10_000,
            ));
        }
    }
    let mut scalar_count = 0_u64;
    for (record, expected) in records.iter().zip(expected_keys) {
        if (
            record.checkpoint_ordinal,
            record.case_id.as_str(),
            record.strand.as_str(),
            record.allele.as_str(),
            record.context_sha256.as_str(),
            record.score_bits.len(),
        ) != expected
        {
            return Err(invalid("golden order or identity"));
        }
        for bits in &record.score_bits {
            let value = decode_f32(bits)?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(invalid("golden value range"));
            }
        }
        scalar_count += record.score_bits.len() as u64;
    }
    if scalar_count != counts.scalar_values {
        return Err(invalid("golden scalar count"));
    }
    Ok(records)
}

fn parse_jsonl<T>(bytes: &[u8], member: &'static str) -> Result<Vec<T>, CommandError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(invalid("JSONL termination"));
    }
    let mut values = Vec::new();
    for line in bytes[..bytes.len() - 1].split(|byte| *byte == b'\n') {
        if line.is_empty() {
            return Err(invalid("empty JSONL record"));
        }
        let value: T = serde_json::from_slice(line).map_err(|_| invalid("JSONL record grammar"))?;
        if serde_jcs::to_vec(&value).map_err(|_| invalid("JSONL canonical encoding"))? != line {
            return Err(invalid("JSONL record is not canonical"));
        }
        values.push(value);
    }
    if values.is_empty() {
        return Err(CommandError::new(
            "MODEL_EVIDENCE",
            format!("{member} is empty"),
        ));
    }
    Ok(values)
}

fn require_production_evidence(evidence: &EvidenceOpen) -> Result<(), CommandError> {
    if evidence.manifest.profile != EVIDENCE_PROFILE {
        return Err(incompatible("production evidence required"));
    }
    if evidence.manifest_bytes != CHECKED_EVIDENCE_MANIFEST
        || evidence.inventory_bytes != CHECKED_INVENTORY
        || evidence.golden_bytes != CHECKED_GOLDENS
    {
        return Err(invalid(
            "production evidence is not byte-identical to checked trust root",
        ));
    }
    Ok(())
}

fn require_mini_evidence(evidence: &EvidenceOpen) -> Result<(), CommandError> {
    if evidence.manifest.profile != MINI_PROFILE {
        return Err(incompatible("synthetic evidence required"));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct RetainedSequence {
    case_id: String,
    context_sha256: String,
    strand: Strand,
    allele: String,
    bases: String,
}

fn production_sequences() -> Result<Vec<RetainedSequence>, CommandError> {
    let mut sequences = Vec::new();
    for line in COMPAT_CASES.lines() {
        let case: serde_json::Value =
            serde_json::from_str(line).map_err(|_| invalid("embedded compatibility JSON"))?;
        if case.get("kind").and_then(serde_json::Value::as_str) != Some("model") {
            continue;
        }
        let case_id = string_field(&case, &["id"])?;
        let bases = string_field(&case, &["context", "bases"])?;
        let context_sha = string_field(&case, &["context", "sha256"])?;
        let anchor = integer_field(&case, &["context", "anchor_offset"])? as usize;
        let reference = string_field(&case, &["input", "ref"])?;
        let alternate = string_field(&case, &["input", "alt"])?;
        if bases.get(anchor..anchor + reference.len()) != Some(reference.as_str()) {
            return Err(invalid("embedded compatibility REF mismatch"));
        }
        let alternate_bases = format!(
            "{}{}{}",
            &bases[..anchor],
            alternate,
            &bases[anchor + reference.len()..]
        );
        let strands = case
            .get("strands")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid("embedded compatibility strands"))?;
        for strand in strands {
            let symbol = strand
                .get("strand")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid("embedded compatibility strand"))?;
            let strand = parse_strand(symbol)?;
            for (allele, sequence) in [
                ("reference", bases.clone()),
                ("alternate", alternate_bases.clone()),
            ] {
                sequences.push(RetainedSequence {
                    case_id: case_id.clone(),
                    context_sha256: format!("sha256:{context_sha}"),
                    strand,
                    allele: allele.to_owned(),
                    bases: sequence,
                });
            }
        }
    }
    if sequences.len() != 36 {
        return Err(invalid("embedded compatibility sequence count"));
    }
    Ok(sequences)
}

fn miniature_sequences() -> Vec<RetainedSequence> {
    let mut reference = vec!['N'; 10_017];
    reference[0] = 'A';
    reference[8] = 'T';
    reference[10_000] = 'C';
    reference[10_008] = 'T';
    reference[10_016] = 'G';
    let mut alternate = reference.clone();
    alternate[0] = 'G';
    alternate[8] = 'A';
    alternate[10_008] = 'C';
    alternate[10_016] = 'T';
    let plus: String = reference.into_iter().collect();
    let minus: String = alternate.into_iter().collect();
    vec![
        mini_sequence("mini-plus", Strand::Plus, "reference", plus.clone()),
        mini_sequence("mini-plus", Strand::Plus, "alternate", minus.clone()),
        mini_sequence("mini-minus", Strand::Minus, "reference", plus),
        mini_sequence("mini-minus", Strand::Minus, "alternate", minus),
    ]
}

fn mini_sequence(case_id: &str, strand: Strand, allele: &str, bases: String) -> RetainedSequence {
    RetainedSequence {
        case_id: case_id.to_owned(),
        context_sha256: sha256(bases.as_bytes()),
        strand,
        allele: allele.to_owned(),
        bases,
    }
}

fn golden_key(record: &GoldenRecord) -> (&str, &str, &str, u8) {
    (
        record.case_id.as_str(),
        record.strand.as_str(),
        record.allele.as_str(),
        record.checkpoint_ordinal,
    )
}

fn decode_f32(bits: &str) -> Result<f32, CommandError> {
    if bits.len() != 8 || !bits.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(invalid("golden f32 bits"));
    }
    let raw = u32::from_str_radix(bits, 16).map_err(|_| invalid("golden f32 bits"))?;
    Ok(f32::from_le_bytes(raw.to_be_bytes()))
}

fn string_field(value: &serde_json::Value, path: &[&str]) -> Result<String, CommandError> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| invalid("embedded compatibility field"))?;
    }
    current
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("embedded compatibility string"))
}

fn integer_field(value: &serde_json::Value, path: &[&str]) -> Result<u64, CommandError> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| invalid("embedded compatibility field"))?;
    }
    current
        .as_u64()
        .ok_or_else(|| invalid("embedded compatibility integer"))
}

fn parse_strand(value: &str) -> Result<Strand, CommandError> {
    match value {
        "+" => Ok(Strand::Plus),
        "-" => Ok(Strand::Minus),
        _ => Err(invalid("embedded compatibility strand")),
    }
}

fn validate_upstream(upstream: &Path) -> Result<(), CommandError> {
    validate_identity(
        &upstream.join("pangolin/model.py"),
        &FileIdentity {
            filename: "pangolin/model.py".to_owned(),
            bytes: MODEL_SOURCE_BYTES,
            sha256: MODEL_SOURCE_SHA.to_owned(),
        },
    )?;
    for checkpoint in production_checkpoints() {
        validate_identity(
            &upstream.join("pangolin/models").join(&checkpoint.filename),
            &checkpoint_file_identity(&checkpoint),
        )?;
    }
    Ok(())
}

fn checkpoint_file_identity(checkpoint: &CheckpointIdentity) -> FileIdentity {
    FileIdentity {
        filename: checkpoint.filename.clone(),
        bytes: checkpoint.bytes,
        sha256: checkpoint.sha256.clone(),
    }
}

fn validate_corpus(corpus: &Path) -> Result<(), CommandError> {
    require_exact_members(corpus, &["NOTICE", "cases.jsonl", "manifest.json"])?;
    let expected = expected_corpus();
    validate_identity(&corpus.join("manifest.json"), &expected.manifest)?;
    validate_identity(&corpus.join("cases.jsonl"), &expected.cases)?;
    validate_identity(&corpus.join("NOTICE"), &expected.notice)
}

fn validate_python(python: &Path) -> Result<(), CommandError> {
    if !python.is_file() {
        return Err(CommandError::new(
            "MODEL_ENVIRONMENT",
            "Python executable is not a regular file",
        ));
    }
    Ok(())
}

fn validate_identity(path: &Path, expected: &FileIdentity) -> Result<(), CommandError> {
    let bytes = read_regular_bounded(path, expected.bytes)?;
    if bytes.len() as u64 != expected.bytes || sha256(&bytes) != expected.sha256 {
        return Err(CommandError::new(
            "MODEL_INPUT",
            format!("authenticated input {} does not match", expected.filename),
        ));
    }
    Ok(())
}

fn validate_declared_member(expected: &FileIdentity, bytes: &[u8]) -> Result<(), CommandError> {
    if expected.bytes != bytes.len() as u64 || expected.sha256 != sha256(bytes) {
        return Err(invalid("evidence member identity"));
    }
    Ok(())
}

fn identity_for(path: &Path, filename: &str, maximum: u64) -> Result<FileIdentity, CommandError> {
    let bytes = read_regular_bounded(path, maximum)?;
    Ok(FileIdentity {
        filename: filename.to_owned(),
        bytes: bytes.len() as u64,
        sha256: sha256(&bytes),
    })
}

fn read_regular_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, CommandError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io_command("inspect bounded input", error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("input is not a regular file"));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(invalid("input link count"));
    }
    if metadata.len() > maximum {
        return Err(invalid("input size bound"));
    }
    let mut file = File::open(path).map_err(|error| io_command("open bounded input", error))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_command("read bounded input", error))?;
    if bytes.len() as u64 != metadata.len() {
        return Err(invalid("input changed while reading"));
    }
    Ok(bytes)
}

fn require_exact_members(directory: &Path, expected: &[&str]) -> Result<(), CommandError> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|error| io_command("inspect directory", error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("directory type"));
    }
    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(directory).map_err(|error| io_command("read directory", error))? {
        let entry = entry.map_err(|error| io_command("read directory entry", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 member"))?;
        observed.insert(name);
    }
    let expected: BTreeSet<_> = expected.iter().map(|name| (*name).to_owned()).collect();
    if observed != expected {
        return Err(invalid("directory member set"));
    }
    Ok(())
}

fn run_python_helper(
    python: &Path,
    source: &str,
    arguments: &[&OsStr],
    label: &'static str,
    allow_bounded_stderr: bool,
) -> Result<(), CommandError> {
    let mut child = Command::new(python)
        .arg("-I")
        .arg("-c")
        .arg(source)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| io_command("start Python helper", error))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CommandError::new("MODEL_HELPER", "helper stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CommandError::new("MODEL_HELPER", "helper stderr unavailable"))?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, MAX_HELPER_OUTPUT));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_HELPER_OUTPUT));
    let status = child
        .wait()
        .map_err(|error| io_command("wait for Python helper", error))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| CommandError::new("MODEL_HELPER", "helper stdout reader failed"))?
        .map_err(|error| io_command("read helper stdout", error))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| CommandError::new("MODEL_HELPER", "helper stderr reader failed"))?
        .map_err(|error| io_command("read helper stderr", error))?;
    helper_status(status, &stdout, &stderr, label, allow_bounded_stderr)
}

struct BoundedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn drain_bounded(mut reader: impl Read, maximum: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        truncated |= count > remaining;
    }
    Ok(BoundedOutput { bytes, truncated })
}

fn helper_status(
    status: ExitStatus,
    stdout: &BoundedOutput,
    stderr: &BoundedOutput,
    label: &'static str,
    allow_bounded_stderr: bool,
) -> Result<(), CommandError> {
    if stdout.truncated || stderr.truncated {
        return Err(CommandError::new(
            "MODEL_HELPER",
            format!("{label} exceeded its output bound"),
        ));
    }
    if !status.success() {
        let digest = Sha256::digest(&stderr.bytes);
        return Err(CommandError::new(
            "MODEL_HELPER",
            format!(
                "{label} failed status={} stderr_bytes={} stderr_sha256={digest:x}",
                status.code().unwrap_or(-1),
                stderr.bytes.len()
            ),
        ));
    }
    if !stdout.bytes.is_empty() || (!allow_bounded_stderr && !stderr.bytes.is_empty()) {
        return Err(CommandError::new(
            "MODEL_HELPER",
            format!("{label} emitted unexpected output"),
        ));
    }
    Ok(())
}

fn create_stage(output: &Path, label: &str) -> Result<(PathBuf, PathBuf), CommandError> {
    if fs::symlink_metadata(output).is_ok() {
        return Err(CommandError::new(
            "ALREADY_EXISTS",
            "model output already exists",
        ));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_owned();
    fs::create_dir_all(&parent).map_err(|error| io_command("create output parent", error))?;
    let name = output
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| CommandError::new("MODEL_INPUT", "output name is invalid"))?;
    let stage = parent.join(format!(
        ".{name}.{label}.{}.{}",
        std::process::id(),
        STAGE_SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage).map_err(|error| io_command("create model stage", error))?;
    Ok((stage, parent))
}

struct StageGuard {
    path: PathBuf,
    armed: bool,
}

impl StageGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn cleanup(&mut self) -> Result<(), CommandError> {
        if self.armed {
            fs::remove_dir_all(&self.path)
                .map_err(|error| io_command("remove model stage", error))?;
            self.armed = false;
        }
        Ok(())
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn publish_stage(
    stage: &Path,
    parent: &Path,
    output: &Path,
    guard: &mut StageGuard,
) -> Result<(), CommandError> {
    #[cfg(target_os = "linux")]
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|error| {
        if matches!(
            error.kind(),
            ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
        ) {
            CommandError::new("ALREADY_EXISTS", "model output already exists")
        } else {
            io_command("publish model output", error)
        }
    })?;
    #[cfg(not(target_os = "linux"))]
    return Err(CommandError::new(
        "MODEL_PUBLICATION",
        "atomic no-replace model publication is unsupported",
    ));
    guard.armed = false;
    sync_directory(parent)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_command("create model member", error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| io_command("write model member", error))
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_command("sync model directory", error))
}

fn cpu_identity() -> String {
    let Ok(contents) = fs::read_to_string("/proc/cpuinfo") else {
        return "unavailable".to_owned();
    };
    contents
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .unwrap_or("unavailable")
        .to_owned()
}

fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    })
}

fn model_error(error: impl std::fmt::Display) -> CommandError {
    CommandError::new("MODEL_BUNDLE", error.to_string())
}

fn invalid(message: &'static str) -> CommandError {
    CommandError::new("MODEL_EVIDENCE", message)
}

fn incompatible(message: &'static str) -> CommandError {
    CommandError::new("MODEL_INCOMPATIBLE", message)
}

fn io_command(action: &'static str, error: io::Error) -> CommandError {
    CommandError::new("IO", format!("{action}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_fixture_directory(source: &Path, destination: &Path) {
        fs::create_dir(destination).expect("create fixture copy");
        for entry in fs::read_dir(source).expect("read fixture") {
            let entry = entry.expect("fixture entry");
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .expect("copy fixture member");
        }
    }

    #[test]
    fn bounded_pipe_reader_drains_but_retains_only_limit() {
        let data = vec![b'x'; 100];
        let result = drain_bounded(data.as_slice(), 10).expect("drain");
        assert_eq!(result.bytes, vec![b'x'; 10]);
        assert!(result.truncated);
    }

    #[test]
    fn little_endian_f32_bits_are_decoded_exactly() {
        assert_eq!(decode_f32("0000803f").expect("one"), 1.0);
        assert_eq!(decode_f32("00000000").expect("zero"), 0.0);
        assert!(decode_f32("not-bits").is_err());
    }

    #[test]
    fn production_counts_bind_ticket_oracle() {
        let counts = production_counts();
        assert_eq!(counts.tensors, 3_024);
        assert_eq!(counts.sequence_evaluations, 36);
        assert_eq!(counts.channel_arrays, 432);
        assert_eq!(counts.scalar_values, 45_756);
    }

    #[test]
    fn production_sequence_construction_matches_count_and_bounds() {
        let sequences = production_sequences().expect("production sequences");
        assert_eq!(sequences.len(), 36);
        assert!(
            sequences
                .iter()
                .all(|item| (10_001..=10_200).contains(&item.bases.len()))
        );
        assert_eq!(
            sequences
                .iter()
                .map(|item| (item.bases.len() - 10_000) * 12)
                .sum::<usize>(),
            45_756
        );
    }

    #[test]
    fn qualification_receipt_uses_held_kernel_identity_after_path_replacement() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let source = fixtures.join("pangolin-model-kernel-mini/bundle");
        let evidence = fixtures.join("pangolin-model-kernel-mini/evidence");
        let temporary = tempfile::tempdir().expect("temporary directory");
        let live = temporary.path().join("live");
        let replacement = temporary.path().join("replacement");
        let retained = temporary.path().join("retained");
        copy_fixture_directory(&source, &live);
        copy_fixture_directory(&source, &replacement);

        let notice = b"semantically different synthetic test notice\n";
        fs::write(replacement.join("NOTICE"), notice).expect("replace notice");
        let manifest_path = replacement.join("manifest.json");
        let manifest_bytes = fs::read(&manifest_path).expect("manifest");
        let mut manifest =
            pangopup_model::parse_manifest_bytes(&manifest_bytes).expect("manifest grammar");
        let member = manifest
            .members
            .iter_mut()
            .find(|member| member.filename == "NOTICE")
            .expect("notice member");
        member.bytes = notice.len() as u64;
        member.sha256 = sha256(notice);
        fs::write(
            &manifest_path,
            canonical_manifest_bytes(&manifest).expect("canonical manifest"),
        )
        .expect("write rebound manifest");

        let original = inspect_bundle(&live).expect("original inspection");
        let replacement_identity = inspect_bundle(&replacement)
            .expect("replacement inspection")
            .bundle_id;
        assert_ne!(original.bundle_id, replacement_identity);

        let outcome = qualify_model_bundle_after_open(&live, &evidence, || {
            fs::rename(&live, &retained).expect("retain original path");
            fs::rename(&replacement, &live).expect("replace bundle path");
            Ok(())
        })
        .expect("held kernel qualifies");

        assert_eq!(outcome.bundle_id, original.bundle_id.to_string());
        assert_eq!(outcome.profile, original.profile);
        assert_eq!(
            inspect_bundle(&live)
                .expect("replacement now at live path")
                .bundle_id,
            replacement_identity
        );
        assert_ne!(outcome.bundle_id, replacement_identity.to_string());
    }
}
