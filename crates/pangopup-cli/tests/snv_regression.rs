mod support;

use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, ScoreProvider};
use pangopup_index::BundleOpen;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};
#[cfg(unix)]
use std::{ffi::OsString, os::unix::ffi::OsStringExt};

#[derive(Clone)]
struct Request {
    group: String,
    variant: String,
    gene: Option<EnsemblGeneId>,
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/snv-regression")
}

fn requests() -> Vec<Request> {
    fs::read_to_string(fixture().join("requests.tsv"))
        .expect("request fixture")
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 5, "request width");
            Request {
                group: fields[1].to_owned(),
                variant: fields[3].to_owned(),
                gene: (fields[4] != ".")
                    .then(|| EnsemblGeneId::from_str(fields[4]).expect("fixture gene")),
            }
        })
        .collect()
}

fn snv(provider: &BundleOpen, variant: &str) -> Grch38Snv {
    let fields: Vec<_> = variant.split(':').collect();
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[0], "GRCh38");
    let (contig, length) = provider.resolve_contig(fields[1]).expect("fixture contig");
    let position = fields[2].parse::<u32>().expect("fixture position");
    assert!(position <= length);
    Grch38Snv::new(
        contig,
        GenomicPosition::new(position).expect("nonzero fixture position"),
        DnaBase::parse(fields[3]).expect("fixture REF"),
        DnaBase::parse(fields[4]).expect("fixture ALT"),
    )
    .expect("fixture SNV")
}

#[test]
fn all_one_thousand_direct_tsv_expectations_pass_one_real_provider() {
    let fixture = fixture();
    let provider = BundleOpen::open(&fixture.join("bundle")).expect("open fixture once");
    let expected = fs::read(fixture.join("expected.jsonl")).expect("direct TSV oracle");
    let requests = requests();
    assert_eq!(requests.len(), 1_000);

    let mut actual = Vec::new();
    let mut record_count = 0_usize;
    for request in requests {
        let snv = snv(&provider, &request.variant);
        let result = provider.lookup(snv, request.gene).expect("real lookup");
        record_count += result.records().len() + result.source_reference_ambiguities().len();
        actual.extend_from_slice(
            &render_requests(
                OutputFormat::Jsonl,
                &[RenderRequest::new(snv, result)],
                None,
            )
            .expect("production renderer"),
        );
    }
    assert!(
        record_count >= 994,
        "fixture unexpectedly lost expected results"
    );
    assert_eq!(actual, expected);
}

#[test]
fn seven_cli_batches_match_the_direct_oracle_subsets() {
    let fixture = fixture();
    let mut groups: BTreeMap<String, Vec<Request>> = BTreeMap::new();
    for request in requests() {
        groups
            .entry(request.group.clone())
            .or_default()
            .push(request);
    }
    assert_eq!(groups.len(), 7);
    let mut total_named = 0_usize;
    for (group, requests) in groups {
        let mut spawn = support::pangopup();
        spawn
            .command
            .arg("lookup")
            .arg("--bundle")
            .arg(fixture.join("bundle"));
        for request in &requests {
            spawn.command.arg("--variant").arg(&request.variant);
        }
        if group != "unfiltered" {
            spawn.command.arg("--gene").arg(&group);
        }
        let output = spawn.output().expect("run CLI batch");
        assert!(
            output.status.success(),
            "{group} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        // The oracle comes from `pangopup-regression-fixture`, which joins the
        // source TSV by hand and never calls the CLI renderer or the gene-name
        // index. That independence is what the comparison is worth, so the
        // oracle carries scoring bytes alone. Take the naming objects out of
        // the CLI's output and compare the rest exactly. A naming object that
        // did not close, or any other new field, still fails here.
        let (scoring, named) = strip_gene_names(&output.stdout);
        assert_eq!(
            scoring,
            fs::read(fixture.join("expected").join(format!("{group}.jsonl")))
                .expect("group oracle"),
            "{group} scoring bytes"
        );
        assert!(
            named > 0,
            "{group} rendered no gene name, so the shipped index reached nothing"
        );
        total_named += named;
    }
    assert!(
        total_named >= 900,
        "the seven batches named {total_named} records, so naming stopped reaching the batch route"
    );
}

/// Remove every `gene_names` object and report how many were removed. The
/// object is a leaf on a record and its own braces are the only ones it
/// carries, so the scan closes on the first `}` and never crosses a record.
fn strip_gene_names(rendered: &[u8]) -> (Vec<u8>, usize) {
    const OPEN: &[u8] = b",\"gene_names\":{";
    let mut scoring = Vec::with_capacity(rendered.len());
    let mut removed = 0;
    let mut rest = rendered;
    while let Some(at) = rest.windows(OPEN.len()).position(|window| window == OPEN) {
        scoring.extend_from_slice(&rest[..at]);
        let body = &rest[at + OPEN.len()..];
        let close = body
            .iter()
            .position(|byte| *byte == b'}')
            .expect("a gene_names object closes");
        assert!(
            !body[..close].contains(&b'{'),
            "a gene_names object carries no nested object"
        );
        rest = &body[close + 1..];
        removed += 1;
    }
    scoring.extend_from_slice(rest);
    (scoring, removed)
}

#[cfg(unix)]
#[test]
fn non_utf8_lookup_data_dir_is_a_path_error_not_cli_usage() {
    let output = support::pangopup()
        .arg("lookup")
        .arg("--data-dir")
        .arg(OsString::from_vec(b"/tmp/pangopup-\xff".to_vec()))
        .arg("--variant")
        .arg("GRCh38:1:1:A:C")
        .output()
        .expect("run non-UTF-8 path case");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).expect("JSON error");
    assert_eq!(error["code"], "PATH_INVALID");
}
