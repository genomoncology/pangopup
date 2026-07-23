use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, ScoreProvider};
use pangopup_index::BundleOpen;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
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
            &render_requests(OutputFormat::Jsonl, &[RenderRequest::new(snv, result)])
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
    let executable = env!("CARGO_BIN_EXE_pangopup");
    for (group, requests) in groups {
        let mut command = Command::new(executable);
        command
            .arg("lookup")
            .arg("--bundle")
            .arg(fixture.join("bundle"));
        for request in &requests {
            command.arg("--variant").arg(&request.variant);
        }
        if group != "unfiltered" {
            command.arg("--gene").arg(&group);
        }
        let output = command.output().expect("run CLI batch");
        assert!(
            output.status.success(),
            "{group} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            fs::read(fixture.join("expected").join(format!("{group}.jsonl")))
                .expect("group oracle"),
            "{group} output"
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_lookup_data_dir_is_a_path_error_not_cli_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
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
