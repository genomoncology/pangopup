use pangopup_core::{
    GeneScoreRecord, Grch38Snv, Grch38Variant, LookupResult, ModelGeneScoreRecord, ModelWarning,
    SourceReferenceAmbiguity,
};
use pangopup_engine::{ModelProvenance, RoutedResult};
use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Jsonl,
    Table,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderRequest {
    result: RoutedResult,
}

impl RenderRequest {
    pub fn new(snv: Grch38Snv, result: LookupResult) -> Self {
        let variant = Grch38Variant::new(
            snv.contig(),
            snv.position(),
            snv.reference().to_string(),
            snv.alternate().to_string(),
        )
        .expect("a typed SNV is a valid literal variant");
        Self {
            result: RoutedResult::Precomputed { variant, result },
        }
    }

    pub const fn from_routed(result: RoutedResult) -> Self {
        Self { result }
    }

    pub const fn result(&self) -> &RoutedResult {
        &self.result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderError(&'static str);

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for RenderError {}

/// Render already-materialized lookup results through the shipped CLI wire
/// boundary. The binary and performance harness both call this function.
pub fn render_requests(
    format: OutputFormat,
    requests: &[RenderRequest],
) -> Result<Vec<u8>, RenderError> {
    match format {
        OutputFormat::Jsonl => render_jsonl(requests),
        OutputFormat::Table => render_table(requests),
    }
}

/// Render one result as an already-validated raw JSON object. Envelope
/// adapters use this form when byte-stable object key order matters.
pub fn render_result_raw(
    result: RoutedResult,
) -> Result<Box<serde_json::value::RawValue>, RenderError> {
    let mut bytes = render_jsonl(&[RenderRequest::from_routed(result)])?;
    if bytes.pop() != Some(b'\n') {
        return Err(RenderError("lookup result serialization failed"));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| RenderError("lookup result serialization failed"))?;
    serde_json::value::RawValue::from_string(text)
        .map_err(|_| RenderError("lookup result serialization failed"))
}

fn precomputed_status(result: &LookupResult) -> &'static str {
    match (
        result.records().is_empty(),
        result.source_reference_ambiguities().is_empty(),
    ) {
        (false, true) => "found",
        (true, false) => "ambiguous_source_reference",
        (false, false) => "mixed",
        (true, true) => "not_found",
    }
}

fn render_jsonl(requests: &[RenderRequest]) -> Result<Vec<u8>, RenderError> {
    let mut output = Vec::new();
    for request in requests {
        match &request.result {
            RoutedResult::Precomputed { variant, result } => {
                let provenance = result
                    .provenance()
                    .precomputed()
                    .ok_or(RenderError("unsupported provider provenance"))?;
                let line = JsonPrecomputedResult {
                    assembly: "GRCh38",
                    contig: variant.contig().to_string(),
                    position: variant.position().get(),
                    reference: variant.reference(),
                    alternate: variant.alternate(),
                    status: precomputed_status(result),
                    records: result.records().iter().map(JsonRecord::from).collect(),
                    source_reference_ambiguities: result
                        .source_reference_ambiguities()
                        .iter()
                        .map(JsonAmbiguity::from)
                        .collect(),
                    provenance: JsonPrecomputedProvenance {
                        kind: "precomputed",
                        bundle_id: provenance.bundle_id(),
                        source_doi: provenance.source_doi(),
                        source_archive_md5: provenance.source_archive_md5(),
                        masked: provenance.masked(),
                        window: provenance.window(),
                    },
                };
                serde_json::to_writer(&mut output, &line)
                    .map_err(|_| RenderError("lookup result serialization failed"))?;
            }
            RoutedResult::Modeled {
                variant,
                records,
                provenance,
            } => {
                let line = JsonModeledResult {
                    assembly: "GRCh38",
                    contig: variant.contig().to_string(),
                    position: variant.position().get(),
                    reference: variant.reference(),
                    alternate: variant.alternate(),
                    status: if records.is_empty() {
                        "not_found"
                    } else {
                        "found"
                    },
                    records: records.iter().map(JsonModelRecord::from).collect(),
                    source_reference_ambiguities: [],
                    provenance: JsonModelProvenance::from(provenance),
                };
                serde_json::to_writer(&mut output, &line)
                    .map_err(|_| RenderError("lookup result serialization failed"))?;
            }
        }
        output.push(b'\n');
    }
    Ok(output)
}

#[derive(Serialize)]
struct JsonPrecomputedResult<'a> {
    assembly: &'static str,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: &'a str,
    #[serde(rename = "alt")]
    alternate: &'a str,
    status: &'static str,
    records: Vec<JsonRecord>,
    source_reference_ambiguities: Vec<JsonAmbiguity>,
    provenance: JsonPrecomputedProvenance<'a>,
}

#[derive(Serialize)]
struct JsonModeledResult<'a> {
    assembly: &'static str,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: &'a str,
    #[serde(rename = "alt")]
    alternate: &'a str,
    status: &'static str,
    records: Vec<JsonModelRecord>,
    source_reference_ambiguities: [(); 0],
    provenance: JsonModelProvenance<'a>,
}

#[derive(Serialize)]
struct JsonRecord {
    gene: String,
    stable_gene: String,
    gain_score: String,
    gain_position: i16,
    loss_score: String,
    loss_position: i16,
}

impl From<&GeneScoreRecord> for JsonRecord {
    fn from(value: &GeneScoreRecord) -> Self {
        let score = value.score();
        Self {
            gene: value.gene().to_string(),
            stable_gene: value.gene().to_string(),
            gain_score: score.gain().to_string(),
            gain_position: score.gain_position().get(),
            loss_score: score.loss_text().to_string(),
            loss_position: score.loss_position().get(),
        }
    }
}

#[derive(Serialize)]
struct JsonModelRecord {
    gene: String,
    stable_gene: String,
    gain_score: String,
    gain_position: i16,
    loss_score: String,
    loss_position: i16,
    warnings: Vec<&'static str>,
}

impl From<&ModelGeneScoreRecord> for JsonModelRecord {
    fn from(value: &ModelGeneScoreRecord) -> Self {
        let score = value.score();
        Self {
            gene: value.gene().to_string(),
            stable_gene: value.gene().stable().to_string(),
            gain_score: score.gain().to_string(),
            gain_position: score.gain_position().get(),
            loss_score: score.loss_text().to_string(),
            loss_position: score.loss_position().get(),
            warnings: value
                .warnings()
                .iter()
                .map(|warning| match warning {
                    ModelWarning::NoAnnotatedSites => "no_annotated_sites",
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct JsonAmbiguity {
    gene: String,
    source_ref: &'static str,
    published_alts: [String; 3],
    omitted_alt: String,
}

impl From<&SourceReferenceAmbiguity> for JsonAmbiguity {
    fn from(value: &SourceReferenceAmbiguity) -> Self {
        Self {
            gene: value.gene().to_string(),
            source_ref: value.source_reference(),
            published_alts: value.published_alternates().map(|base| base.to_string()),
            omitted_alt: value.omitted_alternate().to_string(),
        }
    }
}

#[derive(Serialize)]
struct JsonPrecomputedProvenance<'a> {
    kind: &'static str,
    bundle_id: &'a str,
    source_doi: &'a str,
    source_archive_md5: &'a str,
    masked: bool,
    window: u32,
}

#[derive(Serialize)]
struct JsonModelProvenance<'a> {
    kind: &'static str,
    scoring_semantics: &'static str,
    model_bundle_id: &'a str,
    model_profile: &'a str,
    effective_cpu_policy: &'a str,
    reference_bundle_id: &'a str,
    reference_profile: &'a str,
    reference_sequence_set_sha256: &'a str,
    mask_bytes: u64,
    mask_sha256: &'a str,
    masked: bool,
    window: u32,
}

impl<'a> From<&'a ModelProvenance> for JsonModelProvenance<'a> {
    fn from(value: &'a ModelProvenance) -> Self {
        Self {
            kind: "model",
            scoring_semantics: value.scoring_semantics(),
            model_bundle_id: value.model_bundle_id(),
            model_profile: value.model_profile(),
            effective_cpu_policy: value.effective_cpu_policy(),
            reference_bundle_id: value.reference().bundle_id(),
            reference_profile: value.reference().profile(),
            reference_sequence_set_sha256: value.reference().sequence_set_sha256(),
            mask_bytes: value.mask_bytes(),
            mask_sha256: value.mask_sha256(),
            masked: value.masked(),
            window: value.window(),
        }
    }
}

fn render_table(requests: &[RenderRequest]) -> Result<Vec<u8>, RenderError> {
    let mut output = String::from(
        "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
    );
    for request in requests {
        match &request.result {
            RoutedResult::Precomputed { variant, result } => {
                let bundle_id = result
                    .provenance()
                    .precomputed()
                    .ok_or(RenderError("unsupported provider provenance"))?
                    .bundle_id();
                let prefix = table_prefix(variant, precomputed_status(result));
                for record in result.records() {
                    let score = record.score();
                    output.push_str(&format!(
                        "{prefix}\t{}\t{}\t{}\t{}\t{}\t.\t.\t.\t{bundle_id}\n",
                        record.gene(),
                        score.gain(),
                        score.gain_position(),
                        score.loss_text(),
                        score.loss_position()
                    ));
                }
                for ambiguity in result.source_reference_ambiguities() {
                    let alts = ambiguity
                        .published_alternates()
                        .map(|base| base.to_string())
                        .join(",");
                    output.push_str(&format!(
                        "{prefix}\t{}\t.\t.\t.\t.\t{}\t{}\t{}\t{bundle_id}\n",
                        ambiguity.gene(),
                        ambiguity.source_reference(),
                        alts,
                        ambiguity.omitted_alternate()
                    ));
                }
                if result.records().is_empty() && result.source_reference_ambiguities().is_empty() {
                    output.push_str(&format!("{prefix}\t.\t.\t.\t.\t.\t.\t.\t.\t{bundle_id}\n"));
                }
            }
            RoutedResult::Modeled {
                variant,
                records,
                provenance,
            } => push_modeled_table(&mut output, variant, records, provenance.model_bundle_id()),
        }
    }
    Ok(output.into_bytes())
}

fn push_modeled_table(
    output: &mut String,
    variant: &Grch38Variant,
    records: &[ModelGeneScoreRecord],
    bundle_id: &str,
) {
    let status = if records.is_empty() {
        "not_found"
    } else {
        "found"
    };
    let prefix = table_prefix(variant, status);
    for record in records {
        let score = record.score();
        output.push_str(&format!(
            "{prefix}\t{}\t{}\t{}\t{}\t{}\t.\t.\t.\t{bundle_id}\n",
            record.gene(),
            score.gain(),
            score.gain_position(),
            score.loss_text(),
            score.loss_position()
        ));
    }
    if records.is_empty() {
        output.push_str(&format!("{prefix}\t.\t.\t.\t.\t.\t.\t.\t.\t{bundle_id}\n"));
    }
}

fn table_prefix(variant: &Grch38Variant, status: &str) -> String {
    format!(
        "GRCh38\t{}\t{}\t{}\t{}\t{status}",
        variant.contig(),
        variant.position(),
        variant.reference(),
        variant.alternate(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pangopup_core::{
        DnaBase, EnsemblGeneId, GencodeGeneId, GeneScoreRecord, GenomicPosition, Grch38Contig,
        Grch38Snv, Grch38Variant, LookupProvenance, ModelGeneScoreRecord, ModelWarning,
        PangolinScore, PrecomputedProvenance, RelativePosition, ScoreMagnitude,
        SourceReferenceAmbiguity,
    };
    use std::str::FromStr;

    const BUNDLE_ID: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn gene(value: &str) -> EnsemblGeneId {
        EnsemblGeneId::from_str(value).expect("gene")
    }

    fn score(gain: u16, gain_position: i16, loss: u16, loss_position: i16) -> PangolinScore {
        PangolinScore::new(
            ScoreMagnitude::new(gain).expect("gain"),
            RelativePosition::new(gain_position).expect("gain position"),
            ScoreMagnitude::new(loss).expect("loss"),
            RelativePosition::new(loss_position).expect("loss position"),
        )
    }

    fn provenance() -> LookupProvenance {
        LookupProvenance::Precomputed(PrecomputedProvenance::new(
            BUNDLE_ID.to_owned(),
            "10.5281/zenodo.15649338".to_owned(),
            "679ef0b50e511b6102b4b88fbf811108".to_owned(),
            true,
            50,
        ))
    }

    fn request(
        position: u32,
        records: Vec<GeneScoreRecord>,
        ambiguities: Vec<SourceReferenceAmbiguity>,
    ) -> RenderRequest {
        RenderRequest::new(
            Grch38Snv::new(
                "chr1".parse().expect("contig"),
                GenomicPosition::new(position).expect("position"),
                DnaBase::A,
                DnaBase::C,
            )
            .expect("SNV"),
            LookupResult::new(records, ambiguities, provenance()),
        )
    }

    fn status_matrix() -> Vec<RenderRequest> {
        vec![
            request(
                1,
                vec![
                    GeneScoreRecord::new(gene("ENSG00000000002"), score(0, -50, 0, -50)),
                    GeneScoreRecord::new(gene("ENSG00000000001"), score(35, 25, 0, -50)),
                ],
                vec![],
            ),
            request(
                2,
                vec![],
                vec![SourceReferenceAmbiguity::new(
                    gene("ENSG00000000003"),
                    DnaBase::A,
                )],
            ),
            request(
                3,
                vec![GeneScoreRecord::new(
                    gene("ENSG00000000004"),
                    score(0, -50, 10, 2),
                )],
                vec![SourceReferenceAmbiguity::new(
                    gene("ENSG00000000005"),
                    DnaBase::T,
                )],
            ),
            request(4, vec![], vec![]),
        ]
    }

    #[test]
    fn jsonl_is_byte_exact_for_every_status_and_multiplicity() {
        let expected = concat!(
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":1,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001\",\"stable_gene\":\"ENSG00000000001\",\"gain_score\":\"0.35\",\"gain_position\":25,\"loss_score\":\"0.00\",\"loss_position\":-50},{\"gene\":\"ENSG00000000002\",\"stable_gene\":\"ENSG00000000002\",\"gain_score\":\"0.00\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":2,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"ambiguous_source_reference\",\"records\":[],\"source_reference_ambiguities\":[{\"gene\":\"ENSG00000000003\",\"source_ref\":\"N\",\"published_alts\":[\"C\",\"G\",\"T\"],\"omitted_alt\":\"A\"}],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":3,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"mixed\",\"records\":[{\"gene\":\"ENSG00000000004\",\"stable_gene\":\"ENSG00000000004\",\"gain_score\":\"0.00\",\"gain_position\":-50,\"loss_score\":\"-0.10\",\"loss_position\":2}],\"source_reference_ambiguities\":[{\"gene\":\"ENSG00000000005\",\"source_ref\":\"N\",\"published_alts\":[\"A\",\"C\",\"G\"],\"omitted_alt\":\"T\"}],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":4,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"not_found\",\"records\":[],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
        );
        assert_eq!(
            render_requests(OutputFormat::Jsonl, &status_matrix()).expect("render"),
            expected.as_bytes()
        );
    }

    #[test]
    fn table_is_byte_exact_for_header_statuses_rows_and_final_lf() {
        let expected = concat!(
            "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
            "GRCh38\tchr1\t1\tA\tC\tfound\tENSG00000000001\t0.35\t25\t0.00\t-50\t.\t.\t.\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "GRCh38\tchr1\t1\tA\tC\tfound\tENSG00000000002\t0.00\t-50\t0.00\t-50\t.\t.\t.\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "GRCh38\tchr1\t2\tA\tC\tambiguous_source_reference\tENSG00000000003\t.\t.\t.\t.\tN\tC,G,T\tA\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "GRCh38\tchr1\t3\tA\tC\tmixed\tENSG00000000004\t0.00\t-50\t-0.10\t2\t.\t.\t.\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "GRCh38\tchr1\t3\tA\tC\tmixed\tENSG00000000005\t.\t.\t.\t.\tN\tA,C,G\tT\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
            "GRCh38\tchr1\t4\tA\tC\tnot_found\t.\t.\t.\t.\t.\t.\t.\t.\tsha256:0000000000000000000000000000000000000000000000000000000000000000\n",
        );
        let actual = render_requests(OutputFormat::Table, &status_matrix()).expect("render");
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(actual.last(), Some(&b'\n'));
        assert_eq!(actual.iter().filter(|byte| **byte == b'\n').count(), 7);
    }

    fn model_variant(position: u32, alternate: &str) -> Grch38Variant {
        Grch38Variant::new(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(position).expect("position"),
            "A",
            alternate,
        )
        .expect("model variant")
    }

    fn model_record() -> ModelGeneScoreRecord {
        ModelGeneScoreRecord::new(
            GencodeGeneId::from_str("ENSG00000000001.1").expect("GENCODE gene"),
            score(35, 25, 10, 2),
            vec![ModelWarning::NoAnnotatedSites],
        )
    }

    #[test]
    fn structured_records_report_source_and_stable_gene_identity() {
        let precomputed = GeneScoreRecord::new(gene("ENSG00000157764"), score(35, 25, 0, -50));
        let precomputed =
            serde_json::to_value(JsonRecord::from(&precomputed)).expect("precomputed JSON");
        assert_eq!(precomputed["gene"], "ENSG00000157764");
        assert_eq!(precomputed["stable_gene"], "ENSG00000157764");

        let modeled = ModelGeneScoreRecord::new(
            GencodeGeneId::from_str("ENSG00000157764.14_PAR_Y").expect("GENCODE gene"),
            score(35, 25, 10, 2),
            Vec::new(),
        );
        let modeled = serde_json::to_value(JsonModelRecord::from(&modeled)).expect("modeled JSON");
        assert_eq!(modeled["gene"], "ENSG00000157764.14_PAR_Y");
        assert_eq!(modeled["stable_gene"], "ENSG00000157764");
    }

    fn synthetic_json_provenance() -> JsonModelProvenance<'static> {
        JsonModelProvenance {
            kind: "model",
            scoring_semantics: "pangopup-variant-score-v1",
            model_bundle_id: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            model_profile: "pangopup-model-kernel-mini-v1",
            effective_cpu_policy: "sequential:1/1",
            reference_bundle_id: "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            reference_profile: "pangopup-reference-route-test-v1",
            reference_sequence_set_sha256: "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            mask_bytes: 512,
            mask_sha256: "sha256:4444444444444444444444444444444444444444444444444444444444444444",
            masked: true,
            window: 50,
        }
    }

    #[test]
    fn modeled_jsonl_key_order_warnings_and_filtered_miss_are_byte_exact() {
        let found = model_variant(5_051, "AC");
        let miss = model_variant(5_052, "AG");
        let records = [model_record()];
        let found_line = JsonModeledResult {
            assembly: "GRCh38",
            contig: "chr1".to_owned(),
            position: 5_051,
            reference: found.reference(),
            alternate: found.alternate(),
            status: "found",
            records: records.iter().map(JsonModelRecord::from).collect(),
            source_reference_ambiguities: [],
            provenance: synthetic_json_provenance(),
        };
        let miss_line = JsonModeledResult {
            assembly: "GRCh38",
            contig: "chr1".to_owned(),
            position: 5_052,
            reference: miss.reference(),
            alternate: miss.alternate(),
            status: "not_found",
            records: Vec::new(),
            source_reference_ambiguities: [],
            provenance: synthetic_json_provenance(),
        };
        let mut actual = serde_json::to_vec(&found_line).expect("found JSON");
        actual.push(b'\n');
        serde_json::to_writer(&mut actual, &miss_line).expect("miss JSON");
        actual.push(b'\n');
        let expected = concat!(
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5051,\"ref\":\"A\",\"alt\":\"AC\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001.1\",\"stable_gene\":\"ENSG00000000001\",\"gain_score\":\"0.35\",\"gain_position\":25,\"loss_score\":\"-0.10\",\"loss_position\":2,\"warnings\":[\"no_annotated_sites\"]}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"mask_bytes\":512,\"mask_sha256\":\"sha256:4444444444444444444444444444444444444444444444444444444444444444\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":5052,\"ref\":\"A\",\"alt\":\"AG\",\"status\":\"not_found\",\"records\":[],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"model\",\"scoring_semantics\":\"pangopup-variant-score-v1\",\"model_bundle_id\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",\"model_profile\":\"pangopup-model-kernel-mini-v1\",\"effective_cpu_policy\":\"sequential:1/1\",\"reference_bundle_id\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\"reference_profile\":\"pangopup-reference-route-test-v1\",\"reference_sequence_set_sha256\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"mask_bytes\":512,\"mask_sha256\":\"sha256:4444444444444444444444444444444444444444444444444444444444444444\",\"masked\":true,\"window\":50}}\n",
        );
        assert_eq!(actual, expected.as_bytes());
    }

    #[test]
    fn modeled_table_found_and_filtered_miss_are_byte_exact() {
        let mut actual = String::from(
            "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
        );
        let bundle_id = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        push_modeled_table(
            &mut actual,
            &model_variant(5_051, "AC"),
            &[model_record()],
            bundle_id,
        );
        push_modeled_table(&mut actual, &model_variant(5_052, "AG"), &[], bundle_id);
        let expected = concat!(
            "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
            "GRCh38\tchr1\t5051\tA\tAC\tfound\tENSG00000000001.1\t0.35\t25\t-0.10\t2\t.\t.\t.\tsha256:1111111111111111111111111111111111111111111111111111111111111111\n",
            "GRCh38\tchr1\t5052\tA\tAG\tnot_found\t.\t.\t.\t.\t.\t.\t.\t.\tsha256:1111111111111111111111111111111111111111111111111111111111111111\n",
        );
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }
}
