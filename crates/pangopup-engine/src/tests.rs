use super::*;
use pangopup_core::{
    DnaBase, GeneScoreRecord, LookupProvenance, PrecomputedProvenance, ReferenceProvenance,
    SourceReferenceAmbiguity, ValueError,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Deserialize)]
struct CorpusCase {
    id: String,
    kind: String,
    #[serde(default)]
    input: Option<CaseInput>,
    #[serde(default)]
    context: Option<CaseContext>,
    #[serde(default)]
    strands: Vec<CaseStrand>,
}

#[derive(Clone, Debug, Deserialize)]
struct CaseInput {
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "alt")]
    alternate: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CaseContext {
    start_1based: u32,
    anchor_offset: usize,
    bases: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CaseStrand {
    strand: String,
    dtype: String,
    loss_bits: Vec<String>,
    gain_bits: Vec<String>,
    genes: Vec<CaseGene>,
    expected: CaseExpected,
}

#[derive(Clone, Debug, Deserialize)]
struct CaseGene {
    id: String,
    boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct CaseExpected {
    masked: Vec<ExpectedScore>,
    #[serde(default)]
    unmasked: Vec<ExpectedScore>,
}

#[derive(Clone, Debug, Deserialize)]
struct ExpectedScore {
    gene: String,
    gain_bits: String,
    gain_position: i16,
    loss_bits: String,
    loss_position: i16,
}

#[derive(Clone, Debug, Deserialize)]
struct KernelGolden {
    case_id: String,
    allele: String,
    strand: String,
    checkpoint_ordinal: usize,
    context_sha256: String,
    score_bits: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct PostprocessCase {
    id: String,
    kind: String,
    position: u32,
    distance: u16,
    gain_bits: Vec<String>,
    loss_bits: Vec<String>,
    genes: Vec<PostprocessGene>,
    expected: CaseExpected,
}

#[derive(Clone, Debug, Deserialize)]
struct PostprocessGene {
    id: String,
    boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct RoundingCase {
    id: String,
    kind: String,
    scalars: Vec<RoundingScalar>,
}

#[derive(Clone, Debug, Deserialize)]
struct RoundingScalar {
    dtype: String,
    bits: String,
    rendered: String,
}

type GoldenKey = (String, String, String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArrayReceipt {
    dtype: String,
    length: usize,
    loss_sha256: String,
    gain_sha256: String,
}

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn corpus_cases() -> Vec<CorpusCase> {
    fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility cases")
    .lines()
    .map(|line| serde_json::from_str(line).expect("parse compatibility case"))
    .collect()
}

fn kernel_goldens() -> BTreeMap<GoldenKey, RawScores> {
    let mut channels: BTreeMap<GoldenKey, Vec<Vec<f32>>> = BTreeMap::new();
    for line in fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-model-v1/kernel-golden.jsonl",
    ))
    .expect("read kernel goldens")
    .lines()
    {
        let record: KernelGolden = serde_json::from_str(line).expect("parse kernel golden");
        assert!(record.context_sha256.starts_with("sha256:"));
        let key = (record.case_id, record.allele, record.strand);
        let entry = channels
            .entry(key)
            .or_insert_with(|| vec![Vec::new(); CHANNELS]);
        entry[record.checkpoint_ordinal - 1] =
            record.score_bits.iter().map(|bits| le_f32(bits)).collect();
    }
    channels
        .into_iter()
        .map(|(key, channels)| {
            assert!(channels.iter().all(|channel| !channel.is_empty()));
            let score_length = channels[0].len();
            assert!(channels.iter().all(|channel| channel.len() == score_length));
            (
                key,
                RawScores {
                    score_length,
                    values: channels.into_iter().flatten().collect(),
                },
            )
        })
        .collect()
}

fn post_ensemble_receipts() -> BTreeMap<(String, String), ArrayReceipt> {
    let bytes = fs::read(repository_path(
        "tests/fixtures/pangolin-engine-v1/post-ensemble-sha256.tsv",
    ))
    .expect("read post-ensemble receipts");
    assert_eq!(bytes.len(), 3_026);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b"
    );
    let text = std::str::from_utf8(&bytes).expect("receipt UTF-8");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("case_id\tstrand\tdtype\tlength\tloss_sha256\tgain_sha256")
    );
    lines
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 6);
            (
                (fields[0].to_owned(), fields[1].to_owned()),
                ArrayReceipt {
                    dtype: fields[2].to_owned(),
                    length: fields[3].parse().expect("receipt length"),
                    loss_sha256: fields[4].to_owned(),
                    gain_sha256: fields[5].to_owned(),
                },
            )
        })
        .collect()
}

fn le_f32(bits: &str) -> f32 {
    let encoded = u32::from_str_radix(bits, 16).expect("little-endian f32 bytes");
    f32::from_le_bytes(encoded.to_be_bytes())
}

fn compat_f32(bits: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(bits, 16).expect("f32 bits"))
}

fn compat_f64(bits: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(bits, 16).expect("f64 bits"))
}

fn dummy_provenance() -> ReferenceProvenance {
    ReferenceProvenance::new(
        "test-reference".to_owned(),
        "test-profile".to_owned(),
        "test-format".to_owned(),
        "GRCh38".to_owned(),
        "test-accession".to_owned(),
        "test-sequences".to_owned(),
    )
}

struct FixtureReference {
    contig: Grch38Contig,
    start: GenomicPosition,
    bases: Vec<u8>,
    provenance: ReferenceProvenance,
}

impl ReferenceProvider for FixtureReference {
    fn copy_window(
        &self,
        contig: Grch38Contig,
        start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError> {
        assert_eq!(contig, self.contig, "reference contig");
        assert_eq!(start, self.start, "reference start");
        assert_eq!(destination.len(), self.bases.len(), "reference length");
        destination.copy_from_slice(&self.bases);
        Ok(())
    }

    fn provenance(&self) -> &ReferenceProvenance {
        &self.provenance
    }
}

struct FixtureMask {
    contig: Grch38Contig,
    position: GenomicPosition,
    genes: MaskGenes,
}

impl GeneMaskSource for FixtureMask {
    fn query(
        &mut self,
        contig: Grch38Contig,
        position: GenomicPosition,
    ) -> Result<MaskGenes, ModelScoringError> {
        assert_eq!(contig, self.contig, "mask contig");
        assert_eq!(position, self.position, "mask position");
        Ok(self.genes.clone())
    }
}

type ExpectedKernelCalls = Arc<Mutex<VecDeque<(Vec<u8>, Strand, RawScores)>>>;

struct FixtureKernel {
    expected: ExpectedKernelCalls,
}

type CandidateCalls = Arc<Mutex<Vec<(usize, usize, Vec<Strand>)>>>;

struct CandidateSpyKernel {
    representation: ModelRepresentation,
    calls: CandidateCalls,
    fail: bool,
}

impl RawKernel for CandidateSpyKernel {
    fn infer(&mut self, _: &[u8], _: Strand) -> Result<RawScores, ModelScoringError> {
        panic!("candidate path must not invoke singleton inference")
    }

    fn representation(&self) -> ModelRepresentation {
        self.representation
    }

    fn infer_variant(
        &mut self,
        reference: &[u8],
        alternate: &[u8],
        strands: &[Strand],
    ) -> Result<(Vec<(RawScores, RawScores)>, InferenceAccounting), ModelScoringError> {
        self.calls.lock().expect("candidate calls").push((
            reference.len(),
            alternate.len(),
            strands.to_vec(),
        ));
        if self.fail {
            return Err(ModelScoringError::ModelProvider);
        }
        let scores = strands
            .iter()
            .map(|_| {
                (
                    RawScores {
                        score_length: reference.len() - CONTEXT_FLANKS,
                        values: vec![0.0; CHANNELS * (reference.len() - CONTEXT_FLANKS)],
                    },
                    RawScores {
                        score_length: alternate.len() - CONTEXT_FLANKS,
                        values: vec![0.0; CHANNELS * (alternate.len() - CONTEXT_FLANKS)],
                    },
                )
            })
            .collect();
        Ok((
            scores,
            InferenceAccounting {
                session_invocations: 1,
                logical_context_evaluations: strands.len() * 2,
                batch_size: match self.representation {
                    ModelRepresentation::ZeroPaddedBatch => strands.len() * 2,
                    ModelRepresentation::PairedStrandBatch => strands.len(),
                    ModelRepresentation::Singleton => unreachable!(),
                },
                padded_input_elements: 0,
            },
        ))
    }
}

impl RawKernel for FixtureKernel {
    fn infer(&mut self, context: &[u8], strand: Strand) -> Result<RawScores, ModelScoringError> {
        let (expected_context, expected_strand, scores) = self
            .expected
            .lock()
            .expect("kernel queue")
            .pop_front()
            .expect("unexpected kernel call");
        assert_eq!(context, expected_context, "kernel context bytes");
        assert_eq!(strand, expected_strand, "kernel strand order");
        Ok(scores)
    }
}

fn mask_gene(case: &CaseGene) -> MaskGene {
    MaskGene {
        identity: GencodeGeneId::from_str(&case.id).expect("GENCODE identity"),
        boundaries: case
            .boundaries
            .iter()
            .map(|position| GenomicPosition::new(*position).expect("boundary"))
            .collect(),
    }
}

fn raw_key(case: &str, allele: &str, strand: &str) -> GoldenKey {
    (case.to_owned(), allele.to_owned(), strand.to_owned())
}

fn assert_post_ensemble(
    case_id: &str,
    arrays: &ScoreArrays,
    expected: &CaseStrand,
    receipts: &BTreeMap<(String, String), ArrayReceipt>,
) {
    let receipt = receipts
        .get(&(case_id.to_owned(), expected.strand.clone()))
        .expect("post-ensemble receipt");
    match arrays {
        ScoreArrays::F32 { loss, gain } => {
            assert_eq!(expected.dtype, "f32");
            assert_eq!(loss.len(), expected.loss_bits.len());
            assert_eq!(gain.len(), expected.gain_bits.len());
            assert_eq!(receipt.dtype, "f32");
            assert_eq!(receipt.length, loss.len());
            assert_eq!(receipt.loss_sha256, f32_array_sha256(loss));
            assert_eq!(receipt.gain_sha256, f32_array_sha256(gain));
        }
        ScoreArrays::F64 { loss, gain } => {
            assert_eq!(expected.dtype, "f64");
            assert_eq!(loss.len(), expected.loss_bits.len());
            assert_eq!(gain.len(), expected.gain_bits.len());
            assert_eq!(receipt.dtype, "f64");
            assert_eq!(receipt.length, loss.len());
            assert_eq!(receipt.loss_sha256, f64_array_sha256(loss));
            assert_eq!(receipt.gain_sha256, f64_array_sha256(gain));
        }
    }
}

fn f32_array_sha256(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn f64_array_sha256(values: &[f64]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn expected_hundredths(dtype: &str, bits: &str, loss: bool) -> u8 {
    let rounded = match dtype {
        "f32" => f64::from((compat_f32(bits) * 100.0_f32).round_ties_even()),
        "f64" => (compat_f64(bits) * 100.0_f64).round_ties_even(),
        other => panic!("unsupported fixture dtype {other}"),
    };
    let magnitude = if loss { -rounded } else { rounded };
    magnitude as u8
}

#[test]
fn frozen_model_cases_replay_all_raw_calls_arrays_order_and_public_scores() {
    let goldens = kernel_goldens();
    let receipts = post_ensemble_receipts();
    let cases: Vec<_> = corpus_cases()
        .into_iter()
        .filter(|case| case.kind == "model")
        .collect();
    assert_eq!(cases.len(), 14);
    assert_eq!(goldens.len(), 36);
    assert_eq!(receipts.len(), 18);
    let mut evaluations = 0_usize;

    for case in cases {
        let input = case.input.as_ref().expect("model input");
        let context = case.context.as_ref().expect("model context");
        let contig = Grch38Contig::from_str(&input.contig).expect("contig");
        let position = GenomicPosition::new(input.position).expect("position");
        let variant = Grch38Variant::new(
            contig,
            position,
            input.reference.clone(),
            input.alternate.clone(),
        )
        .expect("fixture variant");
        assert_eq!(context.anchor_offset, FLANK as usize);
        assert_eq!(context.start_1based, input.position - FLANK);
        assert_eq!(
            format!("{:x}", Sha256::digest(context.bases.as_bytes())),
            context.sha256
        );
        let shape = classify(&variant).expect("supported fixture shape");
        let mut alternate = Vec::new();
        alternate.extend_from_slice(&context.bases.as_bytes()[..context.anchor_offset]);
        alternate.extend_from_slice(input.alternate.as_bytes());
        alternate.extend_from_slice(
            &context.bases.as_bytes()[context.anchor_offset + input.reference.len()..],
        );

        let mut plus = Vec::new();
        let mut minus = Vec::new();
        let mut calls = VecDeque::new();
        let mut expected_records = Vec::new();
        for strand in &case.strands {
            let strand_value = match strand.strand.as_str() {
                "+" => Strand::Plus,
                "-" => Strand::Minus,
                other => panic!("fixture strand {other}"),
            };
            let reference_scores = goldens
                .get(&raw_key(&case.id, "reference", &strand.strand))
                .expect("reference golden")
                .clone();
            let alternate_scores = goldens
                .get(&raw_key(&case.id, "alternate", &strand.strand))
                .expect("alternate golden")
                .clone();
            assert_post_ensemble(
                &case.id,
                &post_ensemble(shape, &reference_scores, &alternate_scores).expect("post ensemble"),
                strand,
                &receipts,
            );
            calls.push_back((
                context.bases.as_bytes().to_vec(),
                strand_value,
                reference_scores,
            ));
            calls.push_back((alternate.clone(), strand_value, alternate_scores));
            evaluations += 2;

            let genes: Vec<_> = strand.genes.iter().map(mask_gene).collect();
            match strand_value {
                Strand::Plus => plus = genes,
                Strand::Minus => minus = genes,
            }
            for expected in &strand.expected.masked {
                let source_gene = strand
                    .genes
                    .iter()
                    .find(|gene| gene.id == expected.gene)
                    .expect("expected gene source");
                expected_records.push((
                    expected.gene.clone(),
                    expected_hundredths(&strand.dtype, &expected.gain_bits, false),
                    expected.gain_position,
                    expected_hundredths(&strand.dtype, &expected.loss_bits, true),
                    expected.loss_position,
                    source_gene.boundaries.is_empty(),
                ));
            }
        }

        let shared_calls = Arc::new(Mutex::new(calls));
        let mut engine = ScoringEngine {
            reference: Box::new(FixtureReference {
                contig,
                start: GenomicPosition::new(context.start_1based).expect("context start"),
                bases: context.bases.as_bytes().to_vec(),
                provenance: dummy_provenance(),
            }),
            mask: Box::new(FixtureMask {
                contig,
                position,
                genes: MaskGenes { plus, minus },
            }),
            kernel: Box::new(FixtureKernel {
                expected: Arc::clone(&shared_calls),
            }),
            last_accounting: InferenceAccounting::default(),
        };
        let result = engine.score(&variant).expect("score fixture");
        assert!(
            shared_calls.lock().expect("kernel queue").is_empty(),
            "{} left kernel calls",
            case.id
        );
        let records = result.records().expect("scored model result");
        assert_eq!(records.len(), expected_records.len(), "{}", case.id);
        for (record, expected) in records.iter().zip(&expected_records) {
            assert_eq!(
                record.gene().to_string(),
                expected.0,
                "{} gene order",
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
            assert_eq!(
                record.warnings(),
                if expected.5 {
                    &[ModelWarning::NoAnnotatedSites][..]
                } else {
                    &[][..]
                },
                "{} warnings",
                case.id
            );
        }
    }
    assert_eq!(evaluations, 36);
}

fn controlled_cases() -> Vec<PostprocessCase> {
    fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility cases")
    .lines()
    .filter_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).expect("case JSON");
        (value["kind"] == "postprocess" && value["id"] != "P04-rounding-signed-zero")
            .then(|| serde_json::from_value(value).expect("postprocess case"))
    })
    .collect()
}

fn rounding_case() -> RoundingCase {
    fs::read_to_string(repository_path(
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
    ))
    .expect("read compatibility cases")
    .lines()
    .find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).expect("case JSON");
        (value["id"] == "P04-rounding-signed-zero")
            .then(|| serde_json::from_value(value).expect("rounding case"))
    })
    .expect("P04 rounding case")
}

#[test]
fn controlled_masking_and_first_extrema_match_the_frozen_vectors() {
    let cases = controlled_cases();
    assert_eq!(cases.len(), 3);
    for case in cases {
        assert_eq!(case.kind, "postprocess");
        let mut gain: Vec<_> = case.gain_bits.iter().map(|bits| compat_f32(bits)).collect();
        let mut loss: Vec<_> = case.loss_bits.iter().map(|bits| compat_f32(bits)).collect();
        let window_start = i64::from(case.position) - i64::from(case.distance);

        if case.genes.is_empty() {
            let expected = case.expected.unmasked.first().expect("unmasked extremum");
            assert_eq!(expected.gene, "UNMASKED");
            assert_eq!(
                format!("{:08x}", gain[first_maximum(&gain)].to_bits()),
                expected.gain_bits
            );
            assert_eq!(
                first_maximum(&gain) as i16 - case.distance as i16,
                expected.gain_position
            );
            assert_eq!(
                format!("{:08x}", loss[first_minimum(&loss)].to_bits()),
                expected.loss_bits
            );
            assert_eq!(
                first_minimum(&loss) as i16 - case.distance as i16,
                expected.loss_position
            );
            continue;
        }

        for (gene, expected) in case.genes.iter().zip(&case.expected.masked) {
            assert_eq!(gene.id, expected.gene);
            let boundaries: Vec<_> = gene
                .boundaries
                .iter()
                .map(|position| GenomicPosition::new(*position).expect("boundary"))
                .collect();
            let warning = apply_mask(&mut loss, &mut gain, &boundaries, window_start);
            assert_eq!(
                warning,
                boundaries
                    .is_empty()
                    .then_some(ModelWarning::NoAnnotatedSites)
            );
            let gain_index = first_maximum(&gain);
            let loss_index = first_minimum(&loss);
            assert_eq!(
                format!("{:08x}", gain[gain_index].to_bits()),
                expected.gain_bits,
                "{} gain",
                case.id
            );
            assert_eq!(
                gain_index as i16 - case.distance as i16,
                expected.gain_position
            );
            assert_eq!(
                format!("{:08x}", loss[loss_index].to_bits()),
                expected.loss_bits,
                "{} loss",
                case.id
            );
            assert_eq!(
                loss_index as i16 - case.distance as i16,
                expected.loss_position
            );
        }
    }
}

fn test_gene(boundaries: &[u32]) -> MaskGene {
    MaskGene {
        identity: GencodeGeneId::from_str("ENSG00000000001.1").expect("test gene"),
        boundaries: boundaries
            .iter()
            .map(|position| GenomicPosition::new(*position).expect("boundary"))
            .collect(),
    }
}

#[test]
fn public_rounding_zero_sign_and_position_boundaries_are_exact() {
    let rounding = rounding_case();
    assert_eq!(rounding.id, "P04-rounding-signed-zero");
    assert_eq!(rounding.kind, "postprocess");
    assert_eq!(rounding.scalars.len(), 12);
    for scalar in &rounding.scalars {
        let observed = match scalar.dtype.as_str() {
            "f32" => {
                let rounded = (compat_f32(&scalar.bits) * 100.0_f32).round_ties_even() / 100.0_f32;
                f64::from(rounded)
            }
            "f64" => (compat_f64(&scalar.bits) * 100.0_f64).round_ties_even() / 100.0_f64,
            other => panic!("rounding dtype {other}"),
        };
        let expected: f64 = scalar.rendered.parse().expect("rendered scalar");
        assert_eq!(observed.to_bits(), expected.to_bits(), "{}", scalar.bits);
    }

    assert_eq!(
        PublicScore::gain_hundredths(0.005_f32).expect("f32 +half"),
        0
    );
    assert_eq!(
        PublicScore::loss_hundredths(-0.005_f32).expect("f32 -half"),
        0
    );
    assert_eq!(
        PublicScore::gain_hundredths(0.005_f64).expect("f64 +half"),
        0
    );
    assert_eq!(
        PublicScore::loss_hundredths(-0.005_f64).expect("f64 -half"),
        0
    );
    assert_eq!(
        PublicScore::gain_hundredths(-0.0_f32).expect("negative gain zero"),
        0
    );
    assert_eq!(
        PublicScore::loss_hundredths(0.0_f64).expect("positive loss zero"),
        0
    );
    assert!(PublicScore::gain_hundredths(-0.01_f32).is_err());
    assert!(PublicScore::loss_hundredths(0.01_f64).is_err());
    assert!(PublicScore::gain_hundredths(1.01_f32).is_err());
    assert!(PublicScore::loss_hundredths(-1.01_f64).is_err());

    let mut gain = vec![0.0_f64; 200];
    gain[199] = 1.0;
    let records = score_typed(
        vec![0.0_f64; 200],
        gain,
        &[test_gene(&[1])],
        GenomicPosition::new(100).expect("position"),
    )
    .expect("score +149");
    assert_eq!(records[0].score().gain_position().get(), 149);
    assert_eq!(records[0].score().gain().hundredths(), 100);
    assert_eq!(records[0].score().loss_position().get(), -50);

    let warned = score_typed(
        vec![-0.1_f32; 101],
        vec![0.1_f32; 101],
        &[test_gene(&[])],
        GenomicPosition::new(100).expect("position"),
    )
    .expect("empty boundaries");
    assert_eq!(warned[0].warnings(), &[ModelWarning::NoAnnotatedSites]);
}

#[derive(Clone)]
enum SpyReferenceOutcome {
    Bases(Vec<u8>),
    Error(ReferenceError),
}

struct SpyReference {
    events: Arc<Mutex<Vec<String>>>,
    outcome: SpyReferenceOutcome,
    provenance: ReferenceProvenance,
}

impl ReferenceProvider for SpyReference {
    fn copy_window(
        &self,
        _contig: Grch38Contig,
        _start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError> {
        self.events
            .lock()
            .expect("events")
            .push("reference".to_owned());
        match &self.outcome {
            SpyReferenceOutcome::Bases(bases) => {
                assert_eq!(destination.len(), bases.len());
                destination.copy_from_slice(bases);
                Ok(())
            }
            SpyReferenceOutcome::Error(error) => Err(error.clone()),
        }
    }

    fn provenance(&self) -> &ReferenceProvenance {
        &self.provenance
    }
}

struct SpyMask {
    events: Arc<Mutex<Vec<String>>>,
    result: Result<MaskGenes, ModelScoringError>,
}

impl GeneMaskSource for SpyMask {
    fn query(
        &mut self,
        _contig: Grch38Contig,
        _position: GenomicPosition,
    ) -> Result<MaskGenes, ModelScoringError> {
        self.events.lock().expect("events").push("mask".to_owned());
        self.result.clone()
    }
}

struct SpyKernel {
    events: Arc<Mutex<Vec<String>>>,
    calls: usize,
    fail_on: Option<usize>,
}

impl RawKernel for SpyKernel {
    fn infer(&mut self, context: &[u8], strand: Strand) -> Result<RawScores, ModelScoringError> {
        self.calls += 1;
        self.events
            .lock()
            .expect("events")
            .push(format!("model:{}", strand.symbol()));
        if self.fail_on == Some(self.calls) {
            return Err(ModelScoringError::ModelProvider);
        }
        let score_length = context.len() - CONTEXT_FLANKS;
        Ok(RawScores {
            score_length,
            values: vec![0.5; CHANNELS * score_length],
        })
    }
}

fn valid_variant(position: u32) -> Grch38Variant {
    Grch38Variant::new(
        Grch38Contig::autosome(1).expect("chr1"),
        GenomicPosition::new(position).expect("position"),
        "A",
        "C",
    )
    .expect("variant")
}

fn reference_context(reference: &[u8]) -> Vec<u8> {
    let mut bases = vec![b'N'; CONTEXT_BASES + reference.len()];
    bases[FLANK as usize..FLANK as usize + reference.len()].copy_from_slice(reference);
    bases
}

fn spy_engine(
    events: Arc<Mutex<Vec<String>>>,
    reference: SpyReferenceOutcome,
    mask: Result<MaskGenes, ModelScoringError>,
    fail_on: Option<usize>,
) -> ScoringEngine {
    ScoringEngine {
        reference: Box::new(SpyReference {
            events: Arc::clone(&events),
            outcome: reference,
            provenance: dummy_provenance(),
        }),
        mask: Box::new(SpyMask {
            events: Arc::clone(&events),
            result: mask,
        }),
        kernel: Box::new(SpyKernel {
            events,
            calls: 0,
            fail_on,
        }),
        last_accounting: InferenceAccounting::default(),
    }
}

#[test]
fn rejection_and_provider_failures_short_circuit_in_frozen_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(reference_context(b"A")),
        Ok(MaskGenes::default()),
        None,
    );
    let unanchored = Grch38Variant::new(
        Grch38Contig::autosome(1).expect("chr1"),
        GenomicPosition::new(6_000).expect("position"),
        "A",
        "TC",
    )
    .expect("literal variant");
    assert_eq!(
        engine
            .score(&unanchored)
            .expect("shape rejection")
            .rejection(),
        Some(&ModelRejection::UnsupportedVariantShape)
    );
    assert!(events.lock().expect("events").is_empty());

    let overlength = Grch38Variant::new(
        Grch38Contig::autosome(1).expect("chr1"),
        GenomicPosition::new(6_000).expect("position"),
        "A".repeat(101),
        format!("{}C", "A".repeat(100)),
    )
    .expect("overlength MNV");
    assert!(matches!(
        engine
            .score(&overlength)
            .expect("length rejection")
            .rejection(),
        Some(ModelRejection::AlleleTooLong { .. })
    ));
    assert!(events.lock().expect("events").is_empty());

    let overlength_insertion = Grch38Variant::new(
        Grch38Contig::autosome(1).expect("chr1"),
        GenomicPosition::new(6_000).expect("position"),
        "A",
        format!("A{}", "C".repeat(100)),
    )
    .expect("overlength insertion");
    assert!(matches!(
        engine
            .score(&overlength_insertion)
            .expect("insertion length rejection")
            .rejection(),
        Some(ModelRejection::AlleleTooLong { .. })
    ));
    assert!(events.lock().expect("events").is_empty());

    assert_eq!(
        engine
            .score(&valid_variant(5_050))
            .expect("left context")
            .rejection(),
        Some(&ModelRejection::InsufficientReferenceContext)
    );
    assert!(events.lock().expect("events").is_empty());

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Error(ReferenceError::CorruptProviderData),
        Ok(MaskGenes::default()),
        None,
    );
    assert_eq!(
        engine.score(&valid_variant(6_000)),
        Err(ModelScoringError::ReferenceProvider(
            ReferenceError::CorruptProviderData
        ))
    );
    assert_eq!(*events.lock().expect("events"), ["reference"]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut mismatch = reference_context(b"G");
    mismatch[FLANK as usize] = b'G';
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(mismatch),
        Ok(MaskGenes::default()),
        None,
    );
    assert_eq!(
        engine
            .score(&valid_variant(6_000))
            .expect("mismatch rejection")
            .rejection(),
        Some(&ModelRejection::ReferenceMismatch)
    );
    assert_eq!(*events.lock().expect("events"), ["reference"]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut unsupported = reference_context(b"A");
    unsupported[0] = b'R';
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(unsupported),
        Ok(MaskGenes::default()),
        None,
    );
    assert!(matches!(
        engine
            .score(&valid_variant(6_000))
            .expect("symbol rejection")
            .rejection(),
        Some(ModelRejection::UnsupportedReferenceSymbol {
            offset: 0,
            symbol: b'R'
        })
    ));
    assert_eq!(*events.lock().expect("events"), ["reference"]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(reference_context(b"A")),
        Err(ModelScoringError::MaskProvider),
        None,
    );
    assert_eq!(
        engine.score(&valid_variant(6_000)),
        Err(ModelScoringError::MaskProvider)
    );
    assert_eq!(*events.lock().expect("events"), ["reference", "mask"]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(reference_context(b"A")),
        Ok(MaskGenes::default()),
        None,
    );
    assert_eq!(
        engine
            .score(&valid_variant(6_000))
            .expect("not in gene")
            .rejection(),
        Some(&ModelRejection::NotInGene)
    );
    assert_eq!(*events.lock().expect("events"), ["reference", "mask"]);

    let both = MaskGenes {
        plus: vec![test_gene(&[6_000])],
        minus: vec![test_gene(&[6_000])],
    };
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = spy_engine(
        Arc::clone(&events),
        SpyReferenceOutcome::Bases(reference_context(b"A")),
        Ok(both),
        Some(2),
    );
    assert_eq!(
        engine.score(&valid_variant(6_000)),
        Err(ModelScoringError::ModelProvider)
    );
    assert_eq!(
        *events.lock().expect("events"),
        ["reference", "mask", "model:+", "model:+"]
    );
}

#[test]
fn all_six_frozen_rejection_cases_replay_at_the_expected_first_operation() {
    let cases: Vec<_> = corpus_cases()
        .into_iter()
        .filter(|case| case.kind == "rejection")
        .collect();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let input = case.input.as_ref().expect("rejection input");
        let contig = Grch38Contig::from_str(&input.contig).expect("contig");
        let position = GenomicPosition::new(input.position).expect("position");
        let variant = Grch38Variant::new(
            contig,
            position,
            input.reference.clone(),
            input.alternate.clone(),
        )
        .expect("literal rejection variant");
        let events = Arc::new(Mutex::new(Vec::new()));
        let (reference, mask, expected, expected_events) = match case.id.as_str() {
            "R01-complex-replacement" => (
                SpyReferenceOutcome::Bases(Vec::new()),
                Ok(MaskGenes::default()),
                ModelRejection::UnsupportedVariantShape,
                Vec::<&str>::new(),
            ),
            "R02-deletion-ref101" => (
                SpyReferenceOutcome::Bases(Vec::new()),
                Ok(MaskGenes::default()),
                ModelRejection::AlleleTooLong {
                    reference_length: 101,
                    alternate_length: 1,
                },
                Vec::new(),
            ),
            "R03-reference-mismatch" => (
                SpyReferenceOutcome::Bases(reference_context(b"C")),
                Ok(MaskGenes::default()),
                ModelRejection::ReferenceMismatch,
                vec!["reference"],
            ),
            "R04-no-containing-gene" => (
                SpyReferenceOutcome::Bases(reference_context(b"T")),
                Ok(MaskGenes::default()),
                ModelRejection::NotInGene,
                vec!["reference", "mask"],
            ),
            "R05-left-context" => (
                SpyReferenceOutcome::Bases(Vec::new()),
                Ok(MaskGenes::default()),
                ModelRejection::InsufficientReferenceContext,
                Vec::new(),
            ),
            "R06-right-context" => (
                SpyReferenceOutcome::Error(ReferenceError::OutOfBounds),
                Ok(MaskGenes::default()),
                ModelRejection::InsufficientReferenceContext,
                vec!["reference"],
            ),
            other => panic!("unexpected rejection {other}"),
        };
        let mut engine = spy_engine(Arc::clone(&events), reference, mask, None);
        assert_eq!(
            engine
                .score(&variant)
                .expect("expected rejection")
                .rejection(),
            Some(&expected),
            "{}",
            case.id
        );
        assert_eq!(
            *events.lock().expect("events"),
            expected_events,
            "{} first operation",
            case.id
        );
    }
}

#[test]
fn checked_real_onnx_kernel_scores_in_memory_reference_and_mask() {
    let bundle = repository_path("tests/fixtures/pangolin-model-kernel-mini/bundle");
    let kernel = ModelKernel::open(&bundle).expect("open checked synthetic ONNX bundle");
    let variant = valid_variant(6_000);
    let mut engine = ScoringEngine {
        reference: Box::new(FixtureReference {
            contig: variant.contig(),
            start: GenomicPosition::new(950).expect("start"),
            bases: reference_context(b"A"),
            provenance: dummy_provenance(),
        }),
        mask: Box::new(FixtureMask {
            contig: variant.contig(),
            position: variant.position(),
            genes: MaskGenes {
                plus: vec![test_gene(&[])],
                minus: Vec::new(),
            },
        }),
        kernel: Box::new(ProductionKernel(kernel)),
        last_accounting: InferenceAccounting::default(),
    };
    let result = engine.score(&variant).expect("real ONNX scoring");
    let records = result.records().expect("scored");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].gene().to_string(), "ENSG00000000001.1");
    assert_eq!(records[0].score().gain_position().get(), -50);
    assert_eq!(records[0].score().loss_position().get(), -50);
    assert_eq!(records[0].warnings(), &[ModelWarning::NoAnnotatedSites]);
}

#[test]
fn candidate_spy_groups_all_six_shapes_once_and_returns_no_partial_failure() {
    let shapes = [
        ("equal-one", "A", "C", false),
        ("equal-two", "A", "C", true),
        ("insertion-one", "A", "AC", false),
        ("insertion-two", "A", "AC", true),
        ("deletion-one", "AC", "A", false),
        ("deletion-two", "AC", "A", true),
    ];
    for representation in [
        ModelRepresentation::ZeroPaddedBatch,
        ModelRepresentation::PairedStrandBatch,
    ] {
        for (id, reference, alternate, both) in shapes {
            let contig = Grch38Contig::from_str("1").expect("contig");
            let position = GenomicPosition::new(6_000).expect("position");
            let variant =
                Grch38Variant::new(contig, position, reference.to_owned(), alternate.to_owned())
                    .expect("variant");
            let calls = Arc::new(Mutex::new(Vec::new()));
            let plus = vec![test_gene(&[])];
            let minus = if both {
                vec![test_gene(&[])]
            } else {
                Vec::new()
            };
            let mut engine = ScoringEngine {
                reference: Box::new(FixtureReference {
                    contig,
                    start: GenomicPosition::new(950).expect("start"),
                    bases: reference_context(reference.as_bytes()),
                    provenance: dummy_provenance(),
                }),
                mask: Box::new(FixtureMask {
                    contig,
                    position,
                    genes: MaskGenes { plus, minus },
                }),
                kernel: Box::new(CandidateSpyKernel {
                    representation,
                    calls: Arc::clone(&calls),
                    fail: false,
                }),
                last_accounting: InferenceAccounting::default(),
            };
            let result = engine.score(&variant).expect("candidate score");
            assert_eq!(
                result.records().expect("records").len(),
                if both { 2 } else { 1 },
                "{representation} {id}"
            );
            let observed = calls.lock().expect("calls");
            assert_eq!(observed.len(), 1, "{representation} {id}");
            assert_eq!(
                observed[0],
                (
                    CONTEXT_BASES + reference.len(),
                    CONTEXT_BASES + alternate.len(),
                    if both {
                        vec![Strand::Plus, Strand::Minus]
                    } else {
                        vec![Strand::Plus]
                    }
                ),
                "{representation} {id}"
            );
            assert_eq!(engine.last_accounting.session_invocations, 1);
            assert_eq!(
                engine.last_accounting.batch_size,
                if representation == ModelRepresentation::ZeroPaddedBatch {
                    if both { 4 } else { 2 }
                } else if both {
                    2
                } else {
                    1
                }
            );
        }
    }

    let contig = Grch38Contig::from_str("1").expect("contig");
    let position = GenomicPosition::new(6_000).expect("position");
    let variant =
        Grch38Variant::new(contig, position, "A".to_owned(), "AC".to_owned()).expect("variant");
    let mut engine = ScoringEngine {
        reference: Box::new(FixtureReference {
            contig,
            start: GenomicPosition::new(950).expect("start"),
            bases: reference_context(b"A"),
            provenance: dummy_provenance(),
        }),
        mask: Box::new(FixtureMask {
            contig,
            position,
            genes: MaskGenes {
                plus: vec![test_gene(&[])],
                minus: vec![test_gene(&[])],
            },
        }),
        kernel: Box::new(CandidateSpyKernel {
            representation: ModelRepresentation::ZeroPaddedBatch,
            calls: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        }),
        last_accounting: InferenceAccounting::default(),
    };
    assert!(matches!(
        engine.score(&variant),
        Err(ModelScoringError::ModelProvider)
    ));
    assert_eq!(engine.last_accounting, InferenceAccounting::default());
}

#[test]
fn core_rejects_identical_alleles_before_a_scorer_can_be_built() {
    assert_eq!(
        Grch38Variant::new(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(6_000).expect("position"),
            "A",
            "A"
        ),
        Err(ValueError::SameAlleles)
    );
}

type LookupCalls = Arc<Mutex<Vec<(Grch38Snv, Option<EnsemblGeneId>)>>>;

struct SpyLookup {
    calls: LookupCalls,
    response: LookupResult,
}

impl ScoreProvider for SpyLookup {
    fn lookup(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
    ) -> Result<LookupResult, LookupError> {
        self.calls.lock().expect("lookup calls").push((snv, gene));
        Ok(self.response.clone())
    }
}

fn lookup_provenance() -> LookupProvenance {
    LookupProvenance::Precomputed(PrecomputedProvenance::new(
        "sha256:lookup".to_owned(),
        "10.5281/zenodo.15649338".to_owned(),
        "source-md5".to_owned(),
        true,
        50,
    ))
}

fn router_with(
    records: Vec<GeneScoreRecord>,
    ambiguities: Vec<SourceReferenceAmbiguity>,
) -> (LookupFirstRouter<SpyLookup>, LookupCalls) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (
        LookupFirstRouter::new(SpyLookup {
            calls: Arc::clone(&calls),
            response: LookupResult::new(records, ambiguities, lookup_provenance()),
        }),
        calls,
    )
}

fn route_request(reference: &str, alternate: &str, gene: Option<EnsemblGeneId>) -> RouteRequest {
    RouteRequest::new(
        Grch38Variant::new(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(6_000).expect("position"),
            reference,
            alternate,
        )
        .expect("route variant"),
        gene,
    )
}

#[test]
fn router_preserves_authoritative_records_and_ambiguities_without_model() {
    let stable = EnsemblGeneId::from_str("ENSG00000000001").expect("stable");
    let score = PangolinScore::new(
        ScoreMagnitude::new(10).expect("gain"),
        RelativePosition::new(1).expect("gain position"),
        ScoreMagnitude::new(20).expect("loss"),
        RelativePosition::new(-1).expect("loss position"),
    );
    for (records, ambiguities) in [
        (vec![GeneScoreRecord::new(stable, score)], Vec::new()),
        (
            Vec::new(),
            vec![SourceReferenceAmbiguity::new(stable, DnaBase::A)],
        ),
    ] {
        let (router, calls) = router_with(records, ambiguities);
        let decision = router
            .inspect(route_request("A", "C", Some(stable)))
            .expect("route");
        assert!(matches!(
            decision,
            RouteDecision::Authoritative(RoutedResult::Precomputed { .. })
        ));
        assert_eq!(calls.lock().expect("calls").len(), 1);
        assert_eq!(calls.lock().expect("calls")[0].1, Some(stable));
    }
}

#[test]
fn pure_snv_misses_require_model_and_non_snvs_skip_lookup() {
    let stable = EnsemblGeneId::from_str("ENSG00000000001").expect("stable");
    let (router, calls) = router_with(Vec::new(), Vec::new());
    for gene in [None, Some(stable)] {
        let decision = router
            .inspect(route_request("A", "C", gene))
            .expect("SNV route");
        let RouteDecision::ModelRequired(required) = decision else {
            panic!("pure miss must require model")
        };
        assert_eq!(required.gene(), gene);
    }
    assert_eq!(calls.lock().expect("calls").len(), 2);

    let decision = router
        .inspect(route_request("A", "AC", Some(stable)))
        .expect("non-SNV route");
    assert!(matches!(decision, RouteDecision::ModelRequired(_)));
    assert_eq!(
        calls.lock().expect("calls").len(),
        2,
        "non-SNV must not call lookup provider"
    );
}

#[test]
fn model_completion_masks_all_genes_before_filtering_and_preserves_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let first = test_gene(&[]);
    let second = MaskGene {
        identity: GencodeGeneId::from_str("ENSG00000000002.3").expect("second gene"),
        boundaries: Vec::new(),
    };
    let scorer = VariantScorer {
        engine: spy_engine(
            Arc::clone(&events),
            SpyReferenceOutcome::Bases(reference_context(b"A")),
            Ok(MaskGenes {
                plus: vec![first, second],
                minus: Vec::new(),
            }),
            None,
        ),
    };
    let mut fallback = ModelFallback {
        scorer,
        provenance: ModelProvenance {
            model_bundle_id: "sha256:model".to_owned(),
            model_profile: "test-model".to_owned(),
            reference: dummy_provenance(),
            mask_bytes: 123,
            mask_sha256: "sha256:mask".to_owned(),
        },
    };
    let filter = EnsemblGeneId::from_str("ENSG00000000002").expect("filter");
    let required = ModelRequired {
        request: route_request("A", "C", Some(filter)),
    };
    let result = fallback.complete(required).expect("model completion");
    let records = result.modeled_records().expect("modeled records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].gene().to_string(), "ENSG00000000002.3");
    assert_eq!(
        *events.lock().expect("events"),
        vec!["reference", "mask", "model:+", "model:+"],
        "mask query happens once before model evaluation and filtering"
    );
}
