mod support;

use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, ScoreProvider};
use pangopup_index::BundleOpen;
use pangopup_index::{
    BundleManifest, InputLocus, SparseProvenanceManifest, bundle_id, canonical_manifest_bytes,
    sparse_writer::{SPARSE_INDEX_FORMAT, SparseIndexWriter},
};
use sha2::{Digest, Sha256};
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
    let stamp = support::software_version_field();
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
        // Ticket 0054 added the running version to every printed line for the
        // same reason the oracle has no naming leaf: the oracle carries
        // scoring bytes alone and would have to be regenerated on every
        // release if it carried a version. The strip is the whole field and
        // nothing wider, built from the version this build reports, so a line
        // that named some other version keeps its stamp and fails the byte
        // comparison below rather than being tidied away.
        let (scoring, stamped) = support::strip_exact(&scoring, stamp.as_bytes());
        assert_eq!(
            stamped,
            output.stdout.iter().filter(|byte| **byte == b'\n').count(),
            "{group} left a printed line without a software version"
        );
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

#[test]
fn runtime_v2_cross_format_gate_counts_real_provider_and_command_cases() {
    let fixture = fixture();
    let temp = tempfile::tempdir().expect("temp");
    let sparse_bundle = build_sparse_bundle(temp.path());
    let fixed = BundleOpen::open(&fixture.join("bundle")).expect("fixed BundleOpen");
    let sparse = BundleOpen::open(&sparse_bundle).expect("sparse BundleOpen");
    let all_requests = requests();
    assert_eq!(all_requests.len(), 1_000, "qualification request count");

    let mut fixed_direct = Vec::new();
    let mut sparse_direct = Vec::new();
    for request in &all_requests {
        let fixed_snv = snv(&fixed, &request.variant);
        let sparse_snv = snv(&sparse, &request.variant);
        let fixed_result = fixed.lookup(fixed_snv, request.gene).expect("fixed lookup");
        let sparse_result = sparse
            .lookup(sparse_snv, request.gene)
            .expect("sparse lookup");
        fixed_direct.extend_from_slice(
            &render_requests(
                OutputFormat::Jsonl,
                &[RenderRequest::new(fixed_snv, fixed_result)],
                None,
                None,
            )
            .expect("fixed render"),
        );
        sparse_direct.extend_from_slice(
            &render_requests(
                OutputFormat::Jsonl,
                &[RenderRequest::new(sparse_snv, sparse_result)],
                None,
                None,
            )
            .expect("sparse render"),
        );
    }
    assert_cross_format_bytes(
        &fixed_direct,
        &sparse_direct,
        &fixture.join("bundle"),
        &sparse_bundle,
        1_000,
    );

    let mut groups: BTreeMap<String, Vec<Request>> = BTreeMap::new();
    for request in all_requests {
        groups
            .entry(request.group.clone())
            .or_default()
            .push(request);
    }
    assert_eq!(groups.len(), 7, "qualification group count");
    for (group, group_requests) in groups {
        assert!(!group_requests.is_empty(), "zero-selected group: {group}");
        let run = |bundle: &Path| {
            let mut spawn = support::pangopup();
            spawn.command.arg("lookup").arg("--bundle").arg(bundle);
            for request in &group_requests {
                spawn.command.arg("--variant").arg(&request.variant);
            }
            if group != "unfiltered" {
                spawn.command.arg("--gene").arg(&group);
            }
            spawn.output().expect("command batch")
        };
        let fixed_output = run(&fixture.join("bundle"));
        let sparse_output = run(&sparse_bundle);
        assert!(fixed_output.status.success(), "fixed {group}");
        assert!(sparse_output.status.success(), "sparse {group}");
        assert_eq!(fixed_output.stderr, sparse_output.stderr);
        assert_cross_format_bytes(
            &fixed_output.stdout,
            &sparse_output.stdout,
            &fixture.join("bundle"),
            &sparse_bundle,
            group_requests.len(),
        );
    }

    let lines: Vec<serde_json::Value> = sparse_direct
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("result JSON"))
        .collect();
    assert_eq!(lines.len(), 1_000);
    assert_eq!(
        lines.iter().filter(|line| line["position"] == 1).count(),
        6,
        "six explicit position-1 misses"
    );
    assert!(
        lines
            .iter()
            .filter(|line| line["position"] == 1)
            .all(|line| {
                line["status"] == "not_found"
                    && line["records"].as_array().is_some_and(Vec::is_empty)
            })
    );
    let position_one: Vec<_> = requests()
        .into_iter()
        .filter(|request| request.variant.split(':').nth(2) == Some("1"))
        .collect();
    assert_eq!(position_one.len(), 6);
    let run_complete_fallback = |bundle: &Path, variants: &[&str]| {
        let mut fallback = support::pangopup();
        fallback.command.arg("lookup").arg("--bundle").arg(bundle);
        for variant in variants {
            fallback.command.arg("--variant").arg(variant);
        }
        fallback
            .command
            .arg("--reference-bundle")
            .arg(
                fixture
                    .parent()
                    .expect("fixture parent")
                    .join("reference-route-test/bundle"),
            )
            .arg("--mask")
            .arg(
                fixture
                    .parent()
                    .expect("fixture parent")
                    .join("route-mask/domains.pgm"),
            )
            .arg("--model-bundle")
            .arg(
                fixture
                    .parent()
                    .expect("fixture parent")
                    .join("pangolin-model-kernel-mini/bundle"),
            );
        fallback.output().expect("complete fallback")
    };
    let expected_rejection = b"{\"status\":\"error\",\"code\":\"MODEL_REJECTED\",\"message\":\"insufficient GRCh38 reference context\",\"details\":null}\n";
    let fixed_bundle = fixture.join("bundle");
    for request in &position_one {
        let fixed_rejected = run_complete_fallback(&fixed_bundle, &[&request.variant]);
        let sparse_rejected = run_complete_fallback(&sparse_bundle, &[&request.variant]);
        for (format, rejected) in [("fixed", &fixed_rejected), ("sparse", &sparse_rejected)] {
            assert_eq!(
                rejected.status.code(),
                Some(2),
                "{format}: {}",
                request.variant
            );
            assert!(
                rejected.stdout.is_empty(),
                "{format} emitted partial output for {}",
                request.variant
            );
            assert_eq!(
                rejected.stderr, expected_rejection,
                "{format}: {}",
                request.variant
            );
        }
        assert_eq!(fixed_rejected.stderr, sparse_rejected.stderr);
    }

    let mixed = ["GRCh38:chr12:6801301:G:A", "GRCh38:chr10:1:A:C"];
    let fixed_mixed = run_complete_fallback(&fixed_bundle, &mixed);
    let sparse_mixed = run_complete_fallback(&sparse_bundle, &mixed);
    for (format, rejected) in [("fixed", &fixed_mixed), ("sparse", &sparse_mixed)] {
        assert_eq!(rejected.status.code(), Some(2), "{format} mixed fallback");
        assert!(
            rejected.stdout.is_empty(),
            "{format} mixed fallback must be transactional"
        );
        assert_eq!(
            rejected.stderr, expected_rejection,
            "{format} mixed fallback"
        );
    }
    assert_eq!(fixed_mixed.stderr, sparse_mixed.stderr);
    for status in ["found", "not_found", "ambiguous_source_reference"] {
        assert!(
            lines.iter().any(|line| line["status"] == status),
            "named edge status {status}"
        );
    }
    assert!(
        lines.iter().any(|line| line["records"]
            .as_array()
            .is_some_and(|records| records.len() > 1)),
        "unfiltered overlap"
    );
}

fn assert_cross_format_bytes(
    fixed: &[u8],
    sparse: &[u8],
    fixed_bundle: &Path,
    sparse_bundle: &Path,
    expected_lines: usize,
) {
    let fixed_id = independently_admitted_bundle_id(fixed_bundle, "pangopup.fixed11.v1");
    let sparse_id = independently_admitted_bundle_id(sparse_bundle, SPARSE_INDEX_FORMAT);
    assert_ne!(fixed_id, sparse_id);
    assert_eq!(
        normalize_response_identities(fixed, &fixed_id, expected_lines)
            .expect("fixed response identities"),
        normalize_response_identities(sparse, &sparse_id, expected_lines)
            .expect("sparse response identities"),
        "only the five field-scoped, independently validated identities may change"
    );
}

fn independently_admitted_bundle_id(bundle: &Path, expected_format: &str) -> String {
    let manifest_bytes = fs::read(bundle.join("manifest.json")).expect("manifest bytes");
    let manifest: BundleManifest = serde_json::from_slice(&manifest_bytes).expect("manifest JSON");
    assert_eq!(manifest.index_format, expected_format);
    assert_eq!(
        canonical_manifest_bytes(&manifest).expect("canonical manifest"),
        manifest_bytes,
        "manifest admission is canonical"
    );
    let score = manifest
        .members
        .iter()
        .find(|member| member.path == "scores.pgi")
        .expect("score member");
    let score_bytes = fs::read(bundle.join("scores.pgi")).expect("score bytes");
    assert_eq!(score.size, score_bytes.len() as u64);
    assert_eq!(
        score.sha256,
        format!("sha256:{:x}", Sha256::digest(&score_bytes))
    );
    let admitted = bundle_id(&manifest_bytes);
    assert_eq!(
        BundleOpen::open(bundle)
            .expect("admitted provider")
            .bundle_id(),
        admitted
    );
    admitted
}

const ALLOWED_RESPONSE_IDENTITIES: [&str; 5] = [
    "snv_bundle_id",
    "bundle_id",
    "runtime_profile_id",
    "data_set_version",
    "scoring_identity",
];

fn normalize_response_identities(
    bytes: &[u8],
    bundle_id: &str,
    expected_lines: usize,
) -> Result<Vec<u8>, String> {
    let expected = ALLOWED_RESPONSE_IDENTITIES.map(|field| {
        if field == "bundle_id" {
            (field, Some(bundle_id), expected_lines)
        } else {
            (field, None, 0)
        }
    });
    let mut counts = [0_usize; 5];
    let mut bad = None;
    fn visit(
        value: &serde_json::Value,
        expected: &[(&str, Option<&str>, usize); 5],
        counts: &mut [usize; 5],
        bad: &mut Option<String>,
    ) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    if let Some(index) = ALLOWED_RESPONSE_IDENTITIES
                        .iter()
                        .position(|field| key == field)
                    {
                        counts[index] += 1;
                        if value.as_str() != expected[index].1 {
                            *bad = Some(format!("incorrect {} identity", expected[index].0));
                        }
                    } else {
                        visit(value, expected, counts, bad);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, expected, counts, bad);
                }
            }
            _ => {}
        }
    }
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value = serde_json::from_slice(line).map_err(|error| error.to_string())?;
        visit(&value, &expected, &mut counts, &mut bad);
    }
    if let Some(bad) = bad {
        return Err(bad);
    }
    for (index, (field, _, count)) in expected.iter().enumerate() {
        if counts[index] != *count {
            return Err(format!(
                "{field} occurred {} times, expected {count}",
                counts[index]
            ));
        }
    }
    let bare_count = bytes
        .windows(bundle_id.len())
        .filter(|window| *window == bundle_id.as_bytes())
        .count();
    if bare_count != expected_lines {
        return Err("bundle identity appeared outside its allowed field".to_owned());
    }
    let value = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let needle = format!("\"bundle_id\":\"{bundle_id}\"");
    if value.matches(&needle).count() != expected_lines {
        return Err("bundle identity wire count was incorrect".to_owned());
    }
    Ok(value
        .replace(&needle, "\"bundle_id\":\"<validated-bundle_id>\"")
        .into_bytes())
}

#[test]
fn response_identity_normalization_rejects_missing_wrong_shared_and_unrelated_changes() {
    let fixed_id = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let sparse_id = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let fixed = format!(
        "{{\"bundle_id\":\"{fixed_id}\",\"gene_names\":{{\"symbol\":\"CD4\"}},\"software_version\":\"1.2.3\"}}\n"
    );
    let sparse = format!(
        "{{\"bundle_id\":\"{sparse_id}\",\"gene_names\":{{\"symbol\":\"CD4\"}},\"software_version\":\"1.2.3\"}}\n"
    );
    assert_eq!(
        normalize_response_identities(fixed.as_bytes(), fixed_id, 1).expect("fixed"),
        normalize_response_identities(sparse.as_bytes(), sparse_id, 1).expect("sparse")
    );
    assert!(normalize_response_identities(b"{}\n", fixed_id, 1).is_err());
    assert!(normalize_response_identities(sparse.as_bytes(), fixed_id, 1).is_err());
    let shared_wrong = sparse.replace(sparse_id, fixed_id);
    assert!(normalize_response_identities(shared_wrong.as_bytes(), sparse_id, 1).is_err());
    let wrong_allowed = fixed.replace(
        "\"bundle_id\":",
        &format!("\"snv_bundle_id\":\"{fixed_id}\",\"bundle_id\":"),
    );
    assert!(normalize_response_identities(wrong_allowed.as_bytes(), fixed_id, 1).is_err());
    let unrelated_id = fixed.replace(
        "\"gene_names\":{",
        &format!("\"unrelated\":\"{fixed_id}\",\"gene_names\":{{"),
    );
    assert!(normalize_response_identities(unrelated_id.as_bytes(), fixed_id, 1).is_err());
    let gene_changed = sparse.replace("CD4", "CD8A");
    assert_ne!(
        normalize_response_identities(fixed.as_bytes(), fixed_id, 1).expect("fixed"),
        normalize_response_identities(gene_changed.as_bytes(), sparse_id, 1).expect("sparse")
    );
    let version_changed = sparse.replace("1.2.3", "9.9.9");
    assert_ne!(
        normalize_response_identities(fixed.as_bytes(), fixed_id, 1).expect("fixed"),
        normalize_response_identities(version_changed.as_bytes(), sparse_id, 1).expect("sparse")
    );
}

fn build_sparse_bundle(root: &Path) -> PathBuf {
    let fixed_path = fixture().join("bundle");
    let fixed = BundleOpen::open(&fixed_path).expect("fixed traversal source");
    let mut genes: Vec<Vec<InputLocus>> = Vec::new();
    fixed
        .visit_all_bounded(10_000, 1024 * 1024, |locus| {
            let gene = match locus {
                InputLocus::Ordinary(value) => value.gene,
                InputLocus::Ambiguous(value) => value.gene,
            };
            if genes
                .last()
                .and_then(|items| items.first())
                .is_none_or(|first| {
                    let first_gene = match first {
                        InputLocus::Ordinary(value) => value.gene,
                        InputLocus::Ambiguous(value) => value.gene,
                    };
                    first_gene != gene
                })
            {
                genes.push(Vec::new());
            }
            genes.last_mut().expect("gene group").push(locus);
            Ok::<_, std::convert::Infallible>(())
        })
        .expect("fixed traversal");
    let scratch = root.join("sparse.scratch");
    let score_path = root.join("scores.pgi");
    let mut writer = SparseIndexWriter::create(&scratch).expect("sparse writer");
    for gene in &genes {
        writer.push_gene(gene).expect("sparse gene");
    }
    writer.finish(&score_path).expect("sparse finish");
    let bundle = root.join("bundle");
    fs::create_dir(&bundle).expect("bundle dir");
    fs::copy(fixed_path.join("NOTICE"), bundle.join("NOTICE")).expect("NOTICE");
    fs::rename(score_path, bundle.join("scores.pgi")).expect("scores");
    let mut manifest: BundleManifest = serde_json::from_slice(
        &fs::read(fixed_path.join("manifest.json")).expect("fixed manifest"),
    )
    .expect("manifest JSON");
    manifest.index_format = SPARSE_INDEX_FORMAT.to_owned();
    manifest.sparse_provenance = Some(SparseProvenanceManifest {
        corpus_authority_bundle_id: fixed.bundle_id().to_owned(),
        corpus_authority_builder: manifest.builder.clone(),
        candidate_commit: "2222222222222222222222222222222222222222".to_owned(),
    });
    let member = manifest
        .members
        .iter_mut()
        .find(|member| member.path == "scores.pgi")
        .expect("score member");
    let scores = fs::read(bundle.join("scores.pgi")).expect("scores bytes");
    member.size = scores.len() as u64;
    member.sha256 = format!("sha256:{:x}", Sha256::digest(&scores));
    member.media_type = "application/vnd.pangopup.sparse-direct".to_owned();
    let bytes = canonical_manifest_bytes(&manifest).expect("sparse manifest");
    fs::write(bundle.join("manifest.json"), &bytes).expect("write manifest");
    assert_eq!(
        bundle_id(&bytes),
        BundleOpen::open(&bundle)
            .expect("open generated sparse bundle")
            .bundle_id()
    );
    bundle
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
