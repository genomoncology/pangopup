use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Case {
    id: String,
    input: Input,
    strands: Vec<Strand>,
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
struct Strand {
    dtype: String,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    masked: Vec<Score>,
}

#[derive(Deserialize)]
struct Score {
    gene: String,
    gain_bits: String,
    gain_position: i16,
    loss_bits: String,
    loss_position: i16,
}

fn fixture(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
        .join(relative)
}

fn hundredths(dtype: &str, bits: &str) -> String {
    let rounded = match dtype {
        "f32" => f64::from(
            (f32::from_bits(u32::from_str_radix(bits, 16).expect("f32 bits")) * 100.0)
                .round_ties_even(),
        ),
        "f64" => (f64::from_bits(u64::from_str_radix(bits, 16).expect("f64 bits")) * 100.0)
            .round_ties_even(),
        other => panic!("unexpected dtype {other}"),
    };
    let normalized = if rounded == 0.0 { 0.0 } else { rounded };
    format!("{:.2}", normalized / 100.0)
}

#[test]
fn production_container_oracle_is_derived_from_the_frozen_compatibility_corpus() {
    let corpus = fs::read_to_string(fixture("pangolin-compat-v1/cases.jsonl"))
        .expect("compatibility corpus");
    let cases = corpus
        .lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).expect("case JSON");
            (value["kind"] == "model")
                .then(|| serde_json::from_value::<Case>(value).expect("model case"))
        })
        .collect::<Vec<_>>();
    let oracle: Value = serde_json::from_slice(
        &fs::read(fixture(
            "container-qualification/production-model-oracle.json",
        ))
        .expect("container oracle"),
    )
    .expect("oracle JSON");
    let results = oracle["results"].as_array().expect("oracle results");
    assert_eq!(cases.len(), 14);
    assert_eq!(results.len(), cases.len());

    for (case, result) in cases.iter().zip(results) {
        assert_eq!(result["assembly"], "GRCh38");
        assert_eq!(result["contig"], case.input.contig);
        assert_eq!(result["position"], case.input.position);
        assert_eq!(result["ref"], case.input.reference);
        assert_eq!(result["alt"], case.input.alternate);
        assert_eq!(result["status"], "found");
        assert_eq!(result["source_reference_ambiguities"], Value::Array(vec![]));
        let records = result["records"].as_array().expect("records");
        let expected = case
            .strands
            .iter()
            .flat_map(|strand| {
                strand.expected.masked.iter().map(|score| {
                    (
                        strand.dtype.as_str(),
                        score.gene.as_str(),
                        score.gain_bits.as_str(),
                        score.gain_position,
                        score.loss_bits.as_str(),
                        score.loss_position,
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(records.len(), expected.len());
        for (record, expected) in records.iter().zip(expected) {
            assert_eq!(record["gene"], expected.1);
            assert_eq!(record["gain_score"], hundredths(expected.0, expected.2));
            assert_eq!(record["gain_position"], expected.3);
            assert_eq!(record["loss_score"], hundredths(expected.0, expected.4));
            assert_eq!(record["loss_position"], expected.5);
            assert_eq!(record["warnings"], Value::Array(vec![]));
        }
    }

    let negative_loss = cases
        .iter()
        .zip(results)
        .find(|(case, _)| case.id == "M08-mnv-both-strands")
        .expect("negative-loss sentinel");
    assert_eq!(negative_loss.1["records"][1]["loss_score"], "-0.08");

    let provenance = &oracle["provenance"];
    assert_eq!(provenance["kind"], "model");
    assert_eq!(provenance["scoring_semantics"], "pangopup-variant-score-v1");
    assert_eq!(provenance["effective_cpu_policy"], "sequential:1/1");
    assert_eq!(
        provenance["model_bundle_id"],
        "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43"
    );
    assert_eq!(
        provenance["reference_bundle_id"],
        "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f"
    );
    assert_eq!(
        provenance["mask_sha256"],
        "sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"
    );
}
