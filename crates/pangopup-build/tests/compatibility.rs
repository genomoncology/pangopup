use pangopup_build::compatibility::inspect_corpus;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-compatibility-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create unique temporary directory");
        Self(path)
    }

    fn corpus(&self, name: &str) -> PathBuf {
        let destination = self.0.join(name);
        fs::create_dir(&destination).expect("create corpus copy");
        for member in ["NOTICE", "cases.jsonl", "manifest.json"] {
            fs::copy(fixture().join(member), destination.join(member)).expect("copy member");
        }
        destination
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pangolin-compat-v1")
}

fn assert_compatibility_invalid(corpus: &Path) {
    let error = inspect_corpus(corpus).expect_err("mutated corpus must fail closed");
    assert_eq!(error.code, "COMPATIBILITY_INVALID");
    assert!(
        !error
            .message
            .contains(&corpus.to_string_lossy().into_owned())
    );
}

fn assert_invalid_reason(corpus: &Path, reason: &str) {
    let error = inspect_corpus(corpus).expect_err("mutated corpus must fail closed");
    assert_eq!(error.code, "COMPATIBILITY_INVALID");
    assert!(
        error.message.contains(reason),
        "expected reason {reason:?}, got {:?}",
        error.message
    );
}

fn replace_once(text: &mut String, old: &str, new: &str) {
    assert_eq!(old.len(), new.len(), "fixed-width replacement");
    let offset = text.find(old).expect("unique mutation target");
    assert!(
        !text[offset + old.len()..].contains(old),
        "mutation target is unique"
    );
    text.replace_range(offset..offset + old.len(), new);
}

fn rewrite_cases(corpus: &Path, edit: impl FnOnce(&mut String)) {
    let cases_path = corpus.join("cases.jsonl");
    let mut cases = fs::read_to_string(&cases_path).expect("cases text");
    let old_bytes = cases.len();
    let old_sha = format!("{:x}", Sha256::digest(cases.as_bytes()));
    edit(&mut cases);
    fs::write(&cases_path, &cases).expect("write mutated cases");

    let mut manifest = fs::read_to_string(corpus.join("manifest.json")).expect("manifest text");
    let declaration =
        format!("\"filename\":\"cases.jsonl\",\"bytes\":{old_bytes},\"sha256\":\"{old_sha}\"");
    let new_declaration = format!(
        "\"filename\":\"cases.jsonl\",\"bytes\":{},\"sha256\":\"{:x}\"",
        cases.len(),
        Sha256::digest(cases.as_bytes())
    );
    let offset = manifest.find(&declaration).expect("cases declaration");
    manifest.replace_range(offset..offset + declaration.len(), &new_declaration);
    fs::write(corpus.join("manifest.json"), manifest).expect("write rebound manifest");
}

fn rewrite_manifest(corpus: &Path, edit: impl FnOnce(&mut String)) {
    let path = corpus.join("manifest.json");
    let mut manifest = fs::read_to_string(&path).expect("manifest text");
    edit(&mut manifest);
    fs::write(path, manifest).expect("write mutated manifest");
}

fn line_mutation(cases: &mut String, id: &str, edit: impl FnOnce(&mut String)) {
    let prefix = format!("{{\"id\":\"{id}\"");
    let start = cases.find(&prefix).expect("case line");
    let end = start + cases[start..].find('\n').expect("line ending");
    let mut line = cases[start..end].to_owned();
    edit(&mut line);
    cases.replace_range(start..end, &line);
}

fn replace_array_token(line: &mut String, field: &str, index: usize, replacement: &str) {
    let prefix = format!("\"{field}\":[");
    let mut cursor = line.find(&prefix).expect("array field") + prefix.len();
    for current in 0..=index {
        let quote = line[cursor..].find('"').expect("token start") + cursor;
        let end = line[quote + 1..].find('"').expect("token end") + quote + 1;
        if current == index {
            line.replace_range(quote + 1..end, replacement);
            return;
        }
        cursor = end + 1;
    }
}

fn remove_array_token(line: &mut String, field: &str, index: usize) {
    let prefix = format!("\"{field}\":[");
    let mut cursor = line.find(&prefix).expect("array field") + prefix.len();
    for current in 0..=index {
        let quote = line[cursor..].find('"').expect("token start") + cursor;
        let end = line[quote + 1..].find('"').expect("token end") + quote + 1;
        if current == index {
            let comma_after = line.as_bytes().get(end + 1) == Some(&b',');
            let range = if comma_after {
                quote..end + 2
            } else {
                quote.saturating_sub(1)..end + 1
            };
            line.replace_range(range, "");
            return;
        }
        cursor = end + 1;
    }
}

#[test]
fn checked_corpus_is_valid_and_retains_shape_derived_dtypes() {
    let outcome = inspect_corpus(&fixture()).expect("checked compatibility corpus");
    assert_eq!(outcome.cases, 24);
    assert_eq!(outcome.scored_cases, 14);
    assert_eq!(outcome.rejection_cases, 6);
    assert_eq!(outcome.postprocess_cases, 4);
    assert_eq!(outcome.coverage_cells, 28);

    let cases = fs::read_to_string(fixture().join("cases.jsonl")).expect("cases");
    let values: Vec<Value> = cases
        .lines()
        .map(|line| serde_json::from_str(line).expect("closed fixture JSON"))
        .collect();
    for value in &values[..11] {
        assert!(
            value["strands"]
                .as_array()
                .expect("strands")
                .iter()
                .all(|s| s["dtype"] == "f32")
        );
    }
    for value in &values[11..14] {
        assert!(
            value["strands"]
                .as_array()
                .expect("strands")
                .iter()
                .all(|s| s["dtype"] == "f64")
        );
        assert!(
            value["strands"][0]["loss_bits"][0]
                .as_str()
                .is_some_and(|bits| bits.len() == 16)
        );
    }
    let scalars = values[23]["scalars"].as_array().expect("typed controls");
    assert_eq!(scalars.len(), 12);
    assert_eq!(scalars[6]["rendered"], "0.20999999344348907");
    assert_eq!(scalars[11]["rendered"], "-0.05");
}

#[test]
fn semantic_mutations_survive_hash_rebinding_and_fail_replay() {
    let temp = Temp::new();

    let raw = temp.corpus("raw-score");
    rewrite_cases(&raw, |cases| {
        line_mutation(cases, "M02-snv-wrap53-tp53-precomputed", |line| {
            replace_array_token(line, "gain_bits", 68, "3f800000");
        });
    });
    assert_compatibility_invalid(&raw);

    let expected = temp.corpus("expected-position");
    rewrite_cases(&expected, |cases| {
        line_mutation(cases, "M02-snv-wrap53-tp53-precomputed", |line| {
            let start = line.find("\"expected\":").expect("expected object");
            let relative = line[start..]
                .find("\"gain_position\":18")
                .expect("expected gain position");
            let offset = start + relative;
            line.replace_range(offset..offset + 18, "\"gain_position\":19");
        });
    });
    assert_compatibility_invalid(&expected);

    let order = temp.corpus("gene-order");
    rewrite_cases(&order, |cases| {
        line_mutation(cases, "M05-snv-same-strand-overlap", |line| {
            let genes = line.find("\"genes\":[").expect("genes array");
            let relative = line[genes..].find("ENSG00000283563.1").expect("first gene");
            let offset = genes + relative;
            line.replace_range(offset..offset + 19, "ENSG00000144642.2");
        });
    });
    assert_compatibility_invalid(&order);

    let anchor = temp.corpus("reference-anchor");
    rewrite_cases(&anchor, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            let bases_start = line.find("\"bases\":\"").expect("context bases") + 9;
            let anchor = bases_start + 5050;
            let replacement = if line.as_bytes()[anchor] == b'A' {
                "C"
            } else {
                "A"
            };
            line.replace_range(anchor..anchor + 1, replacement);
            let bases_end = line[bases_start..].find('"').expect("context end") + bases_start;
            let digest = format!(
                "{:x}",
                Sha256::digest(&line.as_bytes()[bases_start..bases_end])
            );
            let sha_start = line[bases_end..]
                .find("\"sha256\":\"")
                .expect("context digest")
                + bases_end
                + 10;
            line.replace_range(sha_start..sha_start + 64, &digest);
        });
    });
    assert_compatibility_invalid(&anchor);

    let rejection = temp.corpus("rejection-category");
    rewrite_cases(&rejection, |cases| {
        replace_once(
            cases,
            "\"normalized_category\":\"not_in_gene\"",
            "\"normalized_category\":\"bad_in_gene\"",
        );
    });
    assert_compatibility_invalid(&rejection);
}

#[test]
fn fixed_controls_reject_rebound_non_extremal_masked_and_boundary_mutations() {
    let temp = Temp::new();

    let non_extremal = temp.corpus("non-extremal-control");
    rewrite_cases(&non_extremal, |cases| {
        line_mutation(cases, "P01-same-strand-order", |line| {
            replace_array_token(line, "gain_bits", 0, "3e000000");
        });
    });
    assert_invalid_reason(&non_extremal, "fixed postprocess vector mismatch");

    let masked_score = temp.corpus("masked-score");
    rewrite_cases(&masked_score, |cases| {
        line_mutation(cases, "P01-same-strand-order", |line| {
            let masked = line.find("\"masked\":[").expect("masked expectations");
            let relative = line[masked..]
                .find("\"gain_bits\":\"3f333333\"")
                .expect("masked score");
            let offset = masked + relative + 13;
            line.replace_range(offset..offset + 8, "3f19999a");
        });
    });
    assert_invalid_reason(&masked_score, "fixed postprocess vector mismatch");

    let boundary = temp.corpus("exon-boundary");
    rewrite_cases(&boundary, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            replace_once(line, "6789528", "6789529");
        });
    });
    assert_compatibility_invalid(&boundary);
}

#[test]
fn line_context_gene_and_formula_bounds_fail_closed() {
    let temp = Temp::new();

    let line_bound = temp.corpus("line-bound");
    rewrite_cases(&line_bound, |cases| {
        line_mutation(cases, "R01-complex-replacement", |line| {
            let start = line.find("\"warning\":\"").expect("warning") + 11;
            let end = line[start..].find('"').expect("warning end") + start;
            line.replace_range(start..end, &"x".repeat(256 * 1024 + 1));
        });
    });
    assert_invalid_reason(&line_bound, "line exceeds byte bound");

    let context_bound = temp.corpus("context-bound");
    rewrite_cases(&context_bound, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            let start = line.find("\"bases\":\"").expect("context") + 9;
            let end = line[start..].find('"').expect("context end") + start;
            let replacement = "A".repeat(10_201);
            line.replace_range(start..end, &replacement);
            let new_end = start + replacement.len();
            let sha = format!("{:x}", Sha256::digest(replacement.as_bytes()));
            let sha_start =
                line[new_end..].find("\"sha256\":\"").expect("context sha") + new_end + 10;
            line.replace_range(sha_start..sha_start + 64, &sha);
        });
    });
    assert_invalid_reason(&context_bound, "context contract mismatch");

    let gene_bound = temp.corpus("gene-bound");
    rewrite_cases(&gene_bound, |cases| {
        line_mutation(cases, "P01-same-strand-order", |line| {
            let start = line.find("\"genes\":[").expect("genes") + 8;
            let end = line[start..].find("],\"expected\"").expect("genes end") + start + 1;
            let genes = (0..5)
                .map(|index| format!("{{\"id\":\"GENE_{index}\",\"boundaries\":[]}}"))
                .collect::<Vec<_>>()
                .join(",");
            line.replace_range(start..end, &format!("[{genes}]"));
        });
    });
    assert_invalid_reason(&gene_bound, "postprocess profile mismatch");

    let formula = temp.corpus("formula-length");
    rewrite_cases(&formula, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            remove_array_token(line, "gain_bits", 100);
        });
    });
    assert_invalid_reason(&formula, "score array length mismatch");
}

#[test]
fn duplicate_missing_identity_license_coverage_and_typed_inputs_fail_closed() {
    let temp = Temp::new();

    let duplicate_case = temp.corpus("duplicate-case");
    rewrite_cases(&duplicate_case, |cases| {
        let mut lines = cases.lines().map(str::to_owned).collect::<Vec<_>>();
        lines[23] = lines[22].clone();
        *cases = format!("{}\n", lines.join("\n"));
    });
    assert_invalid_reason(&duplicate_case, "case order or identity mismatch");

    let missing_case = temp.corpus("missing-case");
    rewrite_cases(&missing_case, |cases| {
        let mut lines = cases.lines().collect::<Vec<_>>();
        lines.pop();
        *cases = format!("{}\n", lines.join("\n"));
    });
    assert_invalid_reason(&missing_case, "case count mismatch");

    let duplicate_checkpoint = temp.corpus("duplicate-checkpoint");
    rewrite_manifest(&duplicate_checkpoint, |manifest| {
        replace_once(manifest, "final.2.0.3.v2", "final.1.0.3.v2");
    });
    assert_invalid_reason(&duplicate_checkpoint, "checkpoint identity mismatch");

    let missing_checkpoint = temp.corpus("missing-checkpoint");
    rewrite_manifest(&missing_checkpoint, |manifest| {
        let start = manifest
            .find(",{\"ordinal\":12")
            .expect("last checkpoint start");
        let end = manifest[start..]
            .find("],\"reference\":")
            .expect("checkpoint array end")
            + start;
        manifest.replace_range(start..end, "");
    });
    assert_invalid_reason(&missing_checkpoint, "checkpoint count mismatch");

    let invalid_dna = temp.corpus("invalid-dna");
    rewrite_cases(&invalid_dna, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            replace_once(line, "\"alt\":\"A\"", "\"alt\":\"X\"");
        });
    });
    assert_invalid_reason(&invalid_dna, "invalid variant input");

    let invalid_strand = temp.corpus("invalid-strand");
    rewrite_cases(&invalid_strand, |cases| {
        line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
            replace_once(line, "\"strand\":\"+\"", "\"strand\":\"?\"");
        });
    });
    assert_invalid_reason(&invalid_strand, "invalid strand order");

    for (name, replacement) in [
        ("malformed-bits", "zzzzzzzz"),
        ("wrong-width-bits", "0000000"),
    ] {
        let corpus = temp.corpus(name);
        rewrite_cases(&corpus, |cases| {
            line_mutation(cases, "M01-snv-cd4-precomputed", |line| {
                replace_array_token(line, "gain_bits", 0, replacement);
            });
        });
        assert_invalid_reason(&corpus, "malformed f32 bits");
    }

    let missing_license = temp.corpus("missing-license");
    rewrite_manifest(&missing_license, |manifest| {
        let field = ",\"license\":\"GPL-3.0-only\"";
        let offset = manifest.find(field).expect("license field");
        manifest.replace_range(offset..offset + field.len(), "");
    });
    assert_invalid_reason(&missing_license, "invalid or non-closed JSON schema");

    let missing_coverage = temp.corpus("missing-coverage");
    rewrite_manifest(&missing_coverage, |manifest| {
        let field = "\"coverage\":[\"shape.snv\",";
        let offset = manifest.find(field).expect("first coverage cell");
        manifest.replace_range(offset..offset + field.len(), "\"coverage\":[");
    });
    assert_invalid_reason(&missing_coverage, "ordered coverage or case IDs mismatch");
}

#[test]
fn provenance_schema_and_member_layout_fail_closed() {
    let temp = Temp::new();

    let checkpoint = temp.corpus("checkpoint");
    let path = checkpoint.join("manifest.json");
    let mut manifest = fs::read_to_string(&path).expect("manifest");
    replace_once(
        &mut manifest,
        "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501",
        "00478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501",
    );
    fs::write(path, manifest).expect("checkpoint mutation");
    assert_compatibility_invalid(&checkpoint);

    let execution_profile = temp.corpus("execution-profile");
    let path = execution_profile.join("manifest.json");
    let mut manifest = fs::read_to_string(&path).expect("manifest");
    replace_once(
        &mut manifest,
        "\"cli_torch_interop_threads_observed\":16",
        "\"cli_torch_interop_threads_observed\":15",
    );
    fs::write(path, manifest).expect("execution-profile mutation");
    assert_compatibility_invalid(&execution_profile);

    let unknown = temp.corpus("unknown-field");
    rewrite_cases(&unknown, |cases| {
        line_mutation(cases, "P04-rounding-signed-zero", |line| {
            line.insert_str(line.len() - 1, ",\"future\":true");
        });
    });
    assert_compatibility_invalid(&unknown);

    let extra = temp.corpus("extra");
    fs::write(extra.join("extra"), b"x").expect("extra member");
    assert_compatibility_invalid(&extra);

    let missing = temp.corpus("missing");
    fs::remove_file(missing.join("NOTICE")).expect("remove member");
    assert_compatibility_invalid(&missing);

    let linked = temp.corpus("hardlink");
    let hardlink_source = temp.0.join("hardlink-notice-source");
    fs::copy(fixture().join("NOTICE"), &hardlink_source).expect("hardlink source");
    fs::remove_file(linked.join("NOTICE")).expect("remove copied notice");
    fs::hard_link(&hardlink_source, linked.join("NOTICE")).expect("hard link");
    assert_compatibility_invalid(&linked);

    let symlinked = temp.corpus("symlink-member");
    fs::remove_file(symlinked.join("NOTICE")).expect("remove copied notice");
    symlink(fixture().join("NOTICE"), symlinked.join("NOTICE")).expect("symlink member");
    let error = inspect_corpus(&symlinked).expect_err("symlink must fail");
    assert!(matches!(error.code, "IO" | "COMPATIBILITY_INVALID"));

    let nested = temp.corpus("nested");
    fs::create_dir(nested.join("nested")).expect("nested member");
    assert_compatibility_invalid(&nested);

    let oversized = temp.corpus("oversized");
    fs::OpenOptions::new()
        .write(true)
        .open(oversized.join("cases.jsonl"))
        .expect("open cases")
        .set_len(3_800_001)
        .expect("extend cases");
    assert_compatibility_invalid(&oversized);
}

#[test]
fn malformed_typed_values_and_declared_bounds_fail_closed() {
    let temp = Temp::new();

    let dtype = temp.corpus("dtype");
    rewrite_cases(&dtype, |cases| {
        line_mutation(cases, "M12-deletion-short-plus", |line| {
            replace_once(line, "\"dtype\":\"f64\"", "\"dtype\":\"f32\"");
        });
    });
    assert_compatibility_invalid(&dtype);

    let nonfinite = temp.corpus("nonfinite");
    rewrite_cases(&nonfinite, |cases| {
        line_mutation(cases, "P04-rounding-signed-zero", |line| {
            replace_once(line, "\"bits\":\"3f80a3d7\"", "\"bits\":\"7f800000\"");
        });
    });
    assert_compatibility_invalid(&nonfinite);

    let oversized_array = temp.corpus("oversized-array");
    rewrite_cases(&oversized_array, |cases| {
        line_mutation(cases, "P01-same-strand-order", |line| {
            let start = line.find("\"gain_bits\":[").expect("gain vector") + 13;
            line.insert_str(start, &"\"00000000\",".repeat(201));
        });
    });
    assert_compatibility_invalid(&oversized_array);

    let oversized_boundaries = temp.corpus("oversized-boundaries");
    rewrite_cases(&oversized_boundaries, |cases| {
        line_mutation(cases, "P01-same-strand-order", |line| {
            let values = (1..=514)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let old = "\"boundaries\":[99]";
            let offset = line.find(old).expect("boundary vector");
            line.replace_range(
                offset..offset + old.len(),
                &format!("\"boundaries\":[{values}]"),
            );
        });
    });
    assert_compatibility_invalid(&oversized_boundaries);

    let oversized_string = temp.corpus("oversized-string");
    rewrite_cases(&oversized_string, |cases| {
        line_mutation(cases, "R01-complex-replacement", |line| {
            let start = line.find("\"warning\":\"").expect("warning") + 11;
            let end = line[start..].find('"').expect("warning end") + start;
            line.replace_range(start..end, &"x".repeat(8 * 1024 + 1));
        });
    });
    assert_compatibility_invalid(&oversized_string);
}
