use pangopup_core::{GeneScoreRecord, Grch38Snv, LookupResult, SourceReferenceAmbiguity};
use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Jsonl,
    Table,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderRequest {
    snv: Grch38Snv,
    result: LookupResult,
}

impl RenderRequest {
    pub const fn new(snv: Grch38Snv, result: LookupResult) -> Self {
        Self { snv, result }
    }

    pub const fn snv(&self) -> Grch38Snv {
        self.snv
    }

    pub const fn result(&self) -> &LookupResult {
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

fn status(result: &LookupResult) -> &'static str {
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
        let provenance = request
            .result
            .provenance()
            .precomputed()
            .ok_or(RenderError("unsupported provider provenance"))?;
        let records: Vec<_> = request
            .result
            .records()
            .iter()
            .map(JsonRecord::from)
            .collect();
        let ambiguities: Vec<_> = request
            .result
            .source_reference_ambiguities()
            .iter()
            .map(JsonAmbiguity::from)
            .collect();
        let line = JsonResult {
            assembly: "GRCh38",
            contig: request.snv.contig().to_string(),
            position: request.snv.position().get(),
            reference: request.snv.reference().to_string(),
            alternate: request.snv.alternate().to_string(),
            status: status(&request.result),
            records,
            source_reference_ambiguities: ambiguities,
            provenance: JsonProvenance {
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
        output.push(b'\n');
    }
    Ok(output)
}

#[derive(Serialize)]
struct JsonResult<'a> {
    assembly: &'static str,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "alt")]
    alternate: String,
    status: &'static str,
    records: Vec<JsonRecord>,
    source_reference_ambiguities: Vec<JsonAmbiguity>,
    provenance: JsonProvenance<'a>,
}

#[derive(Serialize)]
struct JsonRecord {
    gene: String,
    gain_score: String,
    gain_position: i8,
    loss_score: String,
    loss_position: i8,
}

impl From<&GeneScoreRecord> for JsonRecord {
    fn from(value: &GeneScoreRecord) -> Self {
        let score = value.score();
        Self {
            gene: value.gene().to_string(),
            gain_score: score.gain().to_string(),
            gain_position: score.gain_position().get(),
            loss_score: score.loss_text().to_string(),
            loss_position: score.loss_position().get(),
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
struct JsonProvenance<'a> {
    kind: &'static str,
    bundle_id: &'a str,
    source_doi: &'a str,
    source_archive_md5: &'a str,
    masked: bool,
    window: u32,
}

fn render_table(requests: &[RenderRequest]) -> Result<Vec<u8>, RenderError> {
    let mut output = String::from(
        "ASSEMBLY\tCONTIG\tPOS\tREF\tALT\tSTATUS\tGENE\tGAIN_SCORE\tGAIN_POS\tLOSS_SCORE\tLOSS_POS\tSOURCE_REF\tPUBLISHED_ALTS\tOMITTED_ALT\tBUNDLE_ID\n",
    );
    for request in requests {
        let bundle_id = request
            .result
            .provenance()
            .precomputed()
            .ok_or(RenderError("unsupported provider provenance"))?
            .bundle_id();
        let prefix = format!(
            "GRCh38\t{}\t{}\t{}\t{}\t{}",
            request.snv.contig(),
            request.snv.position(),
            request.snv.reference(),
            request.snv.alternate(),
            status(&request.result)
        );
        for record in request.result.records() {
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
        for ambiguity in request.result.source_reference_ambiguities() {
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
        if request.result.records().is_empty()
            && request.result.source_reference_ambiguities().is_empty()
        {
            output.push_str(&format!("{prefix}\t.\t.\t.\t.\t.\t.\t.\t.\t{bundle_id}\n"));
        }
    }
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pangopup_core::{
        DnaBase, EnsemblGeneId, GeneScoreRecord, GenomicPosition, Grch38Snv, LookupProvenance,
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
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":1,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"found\",\"records\":[{\"gene\":\"ENSG00000000001\",\"gain_score\":\"0.35\",\"gain_position\":25,\"loss_score\":\"0.00\",\"loss_position\":-50},{\"gene\":\"ENSG00000000002\",\"gain_score\":\"0.00\",\"gain_position\":-50,\"loss_score\":\"0.00\",\"loss_position\":-50}],\"source_reference_ambiguities\":[],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":2,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"ambiguous_source_reference\",\"records\":[],\"source_reference_ambiguities\":[{\"gene\":\"ENSG00000000003\",\"source_ref\":\"N\",\"published_alts\":[\"C\",\"G\",\"T\"],\"omitted_alt\":\"A\"}],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
            "{\"assembly\":\"GRCh38\",\"contig\":\"chr1\",\"position\":3,\"ref\":\"A\",\"alt\":\"C\",\"status\":\"mixed\",\"records\":[{\"gene\":\"ENSG00000000004\",\"gain_score\":\"0.00\",\"gain_position\":-50,\"loss_score\":\"-0.10\",\"loss_position\":2}],\"source_reference_ambiguities\":[{\"gene\":\"ENSG00000000005\",\"source_ref\":\"N\",\"published_alts\":[\"A\",\"C\",\"G\"],\"omitted_alt\":\"T\"}],\"provenance\":{\"kind\":\"precomputed\",\"bundle_id\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"source_doi\":\"10.5281/zenodo.15649338\",\"source_archive_md5\":\"679ef0b50e511b6102b4b88fbf811108\",\"masked\":true,\"window\":50}}\n",
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
}
