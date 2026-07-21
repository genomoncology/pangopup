use flate2::{Compression, write::GzEncoder};
use pangopup_build::{
    InspectMemberError, MAX_SOURCE_HEADER_BYTES, MAX_SOURCE_ROW_BYTES, SourceLocus, SourceVisitor,
    inspect_directory, inspect_member,
};
use pangopup_core::{DnaBase, EnsemblGeneId, Grch38Snv};
use std::{
    convert::Infallible,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

const HEADER: &str = "chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos";
const DEFAULT_GROUP: &str = "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50\n\
chr1\t100\tA\tG\t0.01\t50\t-0.01\t-50\n\
chr1\t100\tA\tT\t1.0\t0\t-1.0\t50";

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempSource(PathBuf);

impl TempSource {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-ingestion-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary source directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn gzip(&self, filename: &str, body: &str) {
        fs::write(self.0.join(filename), gzip_bytes(body)).expect("write member");
    }

    fn bytes(&self, filename: &str, bytes: &[u8]) {
        fs::write(self.0.join(filename), bytes).expect("write raw member");
    }

    fn inspect_error(&self) -> String {
        inspect_directory(self.path(), &mut Vec::new())
            .expect_err("source should fail")
            .to_string()
    }
}

fn gzip_bytes(body: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body.as_bytes()).expect("write member");
    encoder.finish().expect("finish member")
}

impl Drop for TempSource {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary source directory");
    }
}

fn source(body: &str) -> String {
    format!("{HEADER}\n{body}\n")
}

#[test]
fn fixture_has_exact_totals_and_source_order() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pangolin-precompute");
    let mut output = Vec::new();
    let total = inspect_directory(&fixture, &mut output).expect("fixture must validate");
    assert_eq!(total.genes, 6);
    assert_eq!(total.rows, 6_342);
    assert_eq!(total.loci, 2_114);
    assert_eq!(total.ascending, 4);
    assert_eq!(total.descending, 2);
    assert_eq!(total.segments, 9);
    assert_eq!(total.gaps, 3);
    assert_eq!(total.omitted_bases, 50_002);
    assert_eq!(total.ambiguous_ref_loci, 2);
    assert_eq!(total.n_omit_a, 1);
    assert_eq!(total.n_omit_t, 1);

    let rendered = String::from_utf8(output).expect("UTF-8 report");
    let lines: Vec<_> = rendered.lines().collect();
    assert_eq!(lines.len(), 7);
    assert!(lines[0].contains("gene=ENSG00000010610"));
    assert!(lines[5].contains("gene=ENSG00000185974"));
    assert_eq!(
        lines[6],
        "total genes=6 rows=6342 loci=2114 ascending=4 descending=2 segments=9 gaps=3 omitted_bases=50002 ambiguous_ref_loci=2 n_omit_a=1 n_omit_t=1"
    );
}

#[derive(Default)]
struct FixtureFacts {
    ordinary_loci: u64,
    ambiguous_loci: u64,
    reference_mask: u8,
    saw_zero_gain: bool,
    saw_zero_loss: bool,
    saw_nonzero_gain: bool,
    saw_nonzero_loss: bool,
    saw_nonzero_at_minus_50: bool,
    saw_nonzero_at_plus_50: bool,
    last_position: Option<u32>,
    positions_in_order: bool,
    wrap53_gt: Option<(u8, i8, u8, i8)>,
    tp53_gt: Option<(u8, i8, u8, i8)>,
}

impl SourceVisitor for FixtureFacts {
    type Error = Infallible;

    fn visit(&mut self, locus: &SourceLocus) -> Result<(), Self::Error> {
        let position = match locus {
            SourceLocus::Ordinary(locus) => {
                self.ordinary_loci += 1;
                self.reference_mask |= base_bit(locus.reference);
                let mut alternate_mask = 0_u8;
                for value in &locus.alternatives {
                    let snv = Grch38Snv::new(
                        locus.contig,
                        locus.position,
                        locus.reference,
                        value.alternate,
                    )
                    .expect("validated source locus");
                    assert_eq!(snv.alternate(), value.alternate);
                    alternate_mask |= base_bit(value.alternate);
                    let gain = value.score.gain().hundredths();
                    let loss = value.score.loss().hundredths();
                    self.saw_zero_gain |= gain == 0;
                    self.saw_zero_loss |= loss == 0;
                    self.saw_nonzero_gain |= gain != 0;
                    self.saw_nonzero_loss |= loss != 0;
                    self.saw_nonzero_at_minus_50 |= (gain != 0
                        && value.score.gain_position().get() == -50)
                        || (loss != 0 && value.score.loss_position().get() == -50);
                    self.saw_nonzero_at_plus_50 |= (gain != 0
                        && value.score.gain_position().get() == 50)
                        || (loss != 0 && value.score.loss_position().get() == 50);
                    if locus.position.get() == 7_686_072
                        && locus.reference == DnaBase::G
                        && value.alternate == DnaBase::T
                    {
                        let score = (
                            gain,
                            value.score.gain_position().get(),
                            loss,
                            value.score.loss_position().get(),
                        );
                        match locus.gene.to_string().as_str() {
                            "ENSG00000141499" => self.wrap53_gt = Some(score),
                            "ENSG00000141510" => self.tp53_gt = Some(score),
                            _ => {}
                        }
                    }
                }
                assert_eq!(alternate_mask | base_bit(locus.reference), 0b1111);
                locus.position.get()
            }
            SourceLocus::AmbiguousReference(locus) => {
                self.ambiguous_loci += 1;
                let alternate_mask = locus
                    .alternatives
                    .iter()
                    .fold(0_u8, |mask, value| mask | base_bit(value.alternate));
                assert_eq!(alternate_mask | base_bit(locus.omitted), 0b1111);
                locus.position.get()
            }
        };
        if self
            .last_position
            .is_some_and(|previous| previous == position)
        {
            self.positions_in_order = false;
        }
        self.last_position = Some(position);
        Ok(())
    }
}

fn base_bit(base: DnaBase) -> u8 {
    match base {
        DnaBase::A => 1,
        DnaBase::C => 2,
        DnaBase::G => 4,
        DnaBase::T => 8,
    }
}

#[test]
fn fixture_preserves_exact_edge_cases_and_overlap_scores() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pangolin-precompute");
    let mut facts = FixtureFacts {
        positions_in_order: true,
        ..FixtureFacts::default()
    };
    for entry in fs::read_dir(&fixture).expect("fixture directory") {
        let entry = entry.expect("fixture member");
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        let Some(gene_text) = filename.strip_suffix(".tsv.gz") else {
            continue;
        };
        let gene = EnsemblGeneId::from_str(gene_text).expect("fixture gene ID");
        facts.last_position = None;
        inspect_member(&entry.path(), &filename, gene, &mut facts).expect("fixture member");
    }
    assert_eq!(facts.ordinary_loci, 2_112);
    assert_eq!(facts.ambiguous_loci, 2);
    assert_eq!(facts.reference_mask, 0b1111);
    assert!(facts.saw_zero_gain && facts.saw_zero_loss);
    assert!(facts.saw_nonzero_gain && facts.saw_nonzero_loss);
    assert!(facts.saw_nonzero_at_minus_50 && facts.saw_nonzero_at_plus_50);
    assert!(facts.positions_in_order);
    assert_eq!(facts.wrap53_gt, Some((35, 25, 0, -50)));
    assert_eq!(facts.tp53_gt, Some((0, -50, 0, -50)));
}

#[test]
fn accepts_ascending_descending_gaps_and_both_n_shapes() {
    let temp = TempSource::new();
    temp.gzip(
        "ENSG00000000001.tsv.gz",
        &source(&format!(
            "{DEFAULT_GROUP}\n{}\n{}\n{}",
            DEFAULT_GROUP.replace("\t100\t", "\t102\t"),
            "chr1\t103\tN\tC\t0.0\t-50\t-0.0\t-50",
            "chr1\t103\tN\tG\t0.0\t-50\t-0.0\t-50\nchr1\t103\tN\tT\t0.0\t-50\t-0.0\t-50"
        )),
    );
    temp.gzip(
        "ENSG00000000002.tsv.gz",
        &source(&format!(
            "{}\n{DEFAULT_GROUP}",
            DEFAULT_GROUP.replace("\t100\t", "\t101\t")
        )),
    );
    temp.gzip(
        "ENSG00000000003.tsv.gz",
        &source("chr1\t100\tN\tA\t0.0\t-50\t-0.0\t-50\nchr1\t100\tN\tC\t0.0\t-50\t-0.0\t-50\nchr1\t100\tN\tG\t0.0\t-50\t-0.0\t-50"),
    );
    fs::write(temp.path().join("README.txt"), "ignored").expect("unrelated file");
    fs::create_dir(temp.path().join("nested")).expect("nested directory");
    let mut output = Vec::new();
    let total = inspect_directory(temp.path(), &mut output).expect("valid shapes");
    assert_eq!(total.genes, 3);
    assert_eq!(total.ascending, 2);
    assert_eq!(total.descending, 1);
    assert_eq!(total.gaps, 1);
    assert_eq!(total.ambiguous_ref_loci, 2);
    assert_eq!(total.n_omit_a, 1);
    assert_eq!(total.n_omit_t, 1);
}

#[test]
fn visitor_receives_each_locus_immediately_in_source_order() {
    let temp = TempSource::new();
    let rows = [100_u32, 102, 103]
        .into_iter()
        .map(|position| DEFAULT_GROUP.replace("\t100\t", &format!("\t{position}\t")))
        .collect::<Vec<_>>()
        .join("\n");
    temp.gzip("ENSG00000000001.tsv.gz", &source(&rows));
    let member = temp.path().join("ENSG00000000001.tsv.gz");
    let gene = EnsemblGeneId::from_str("ENSG00000000001").expect("gene");
    let mut visited = Vec::new();
    inspect_member(
        &member,
        "ENSG00000000001.tsv.gz",
        gene,
        &mut |locus: &SourceLocus| {
            let position = match locus {
                SourceLocus::Ordinary(locus) => locus.position.get(),
                SourceLocus::AmbiguousReference(locus) => locus.position.get(),
            };
            visited.push(position);
            Ok::<(), Infallible>(())
        },
    )
    .expect("valid member");
    assert_eq!(visited, [100, 102, 103]);
}

#[test]
fn accepts_mitochondrial_source_rows() {
    let temp = TempSource::new();
    temp.gzip(
        "ENSG00000000001.tsv.gz",
        &source(&DEFAULT_GROUP.replace("chr1", "chrM")),
    );
    let mut output = Vec::new();
    let total = inspect_directory(temp.path(), &mut output).expect("mitochondrial member");
    assert_eq!(total.genes, 1);
    assert!(
        String::from_utf8(output)
            .expect("UTF-8 report")
            .contains("contig=chrM")
    );
}

#[test]
fn source_adapter_requires_canonical_contig_spelling() {
    for noncanonical in ["1", "chr01"] {
        let temp = TempSource::new();
        temp.gzip(
            "ENSG00000000001.tsv.gz",
            &source(&DEFAULT_GROUP.replace("chr1", noncanonical)),
        );
        let error = temp.inspect_error();
        assert!(error.contains(":2: source contig must use canonical"));
        assert!(error.contains(noncanonical));
    }
}

#[test]
fn rejects_truncated_trailing_and_concatenated_gzip_data() {
    let filename = "ENSG00000000001.tsv.gz";
    let complete = gzip_bytes(&source(DEFAULT_GROUP));

    let truncated = TempSource::new();
    truncated.bytes(filename, &complete[..complete.len() - 4]);
    let error = truncated.inspect_error();
    assert!(error.starts_with("ENSG00000000001.tsv.gz:5:"));
    assert!(error.contains("unexpected end of file"));

    let trailing = TempSource::new();
    let mut with_trailing = complete.clone();
    with_trailing.extend_from_slice(b"literal trailing bytes");
    trailing.bytes(filename, &with_trailing);
    assert_eq!(
        trailing.inspect_error(),
        "ENSG00000000001.tsv.gz: trailing bytes after the gzip member are not permitted"
    );

    let concatenated = TempSource::new();
    let mut two_members = complete;
    two_members.extend_from_slice(&gzip_bytes(&source(DEFAULT_GROUP)));
    concatenated.bytes(filename, &two_members);
    assert_eq!(
        concatenated.inspect_error(),
        "ENSG00000000001.tsv.gz: concatenated gzip members are not permitted"
    );
}

#[test]
fn rejects_oversized_header_and_row_without_unbounded_reads() {
    let header = TempSource::new();
    header.gzip(
        "ENSG00000000001.tsv.gz",
        &format!("{}\n", "x".repeat(MAX_SOURCE_HEADER_BYTES + 1)),
    );
    assert_eq!(
        header.inspect_error(),
        format!("ENSG00000000001.tsv.gz:1: source header exceeds {MAX_SOURCE_HEADER_BYTES} bytes")
    );

    let row = TempSource::new();
    row.gzip(
        "ENSG00000000001.tsv.gz",
        &format!("{HEADER}\n{}\n", "x".repeat(MAX_SOURCE_ROW_BYTES + 1)),
    );
    assert_eq!(
        row.inspect_error(),
        format!("ENSG00000000001.tsv.gz:2: source row exceeds {MAX_SOURCE_ROW_BYTES} bytes")
    );
}

#[derive(Debug, Eq, PartialEq)]
struct StopVisitor;

#[test]
fn fallible_visitor_can_stop_streaming_without_losing_its_error() {
    let temp = TempSource::new();
    temp.gzip("ENSG00000000001.tsv.gz", &source(DEFAULT_GROUP));
    let member = temp.path().join("ENSG00000000001.tsv.gz");
    let gene = EnsemblGeneId::from_str("ENSG00000000001").expect("gene");
    let result = inspect_member(
        &member,
        "ENSG00000000001.tsv.gz",
        gene,
        &mut |_locus: &SourceLocus| Err(StopVisitor),
    );
    assert!(matches!(
        result,
        Err(InspectMemberError::Visitor(StopVisitor))
    ));
}

#[test]
fn validation_error_families_are_typed_and_precise() {
    let cases = [
        ("wrong\theader\n", "header does not match"),
        (&source("chr1\t100\tA\tC\t0.0\t-50\t-0.0"), "exactly eight"),
        (
            &source("chr23\t100\tA\tC\t0.0\t-50\t-0.0\t-50"),
            "source contig must use canonical",
        ),
        (&source("chr1\t100\tB\tC\t0.0\t-50\t-0.0\t-50"), "DNA base"),
        (
            &source("chr1\tzero\tA\tC\t0.0\t-50\t-0.0\t-50"),
            "invalid one-based",
        ),
        (&source("chr1\t0\tA\tC\t0.0\t-50\t-0.0\t-50"), "one-based"),
        (
            &source("chr1\t100\tA\tA\t0.0\t-50\t-0.0\t-50"),
            "must differ",
        ),
        (
            &source("chr1\t100\tA\tC\t0.001\t-50\t-0.0\t-50"),
            "exact hundredth",
        ),
        (&source("chr1\t100\tA\tC\t1.01\t-50\t-0.0\t-50"), "outside"),
        (
            &source("chr1\t100\tA\tC\t0.0\t51\t-0.0\t-50"),
            "relative position",
        ),
        (
            &source("chr1\t100\tA\tC\t0.0\t-50\t0.01\t-50"),
            "loss score",
        ),
        (
            &source("chr1\t100\tA\tC\t-0.01\t-50\t-0.0\t-50"),
            "gain score",
        ),
    ];
    for (body, reason) in cases {
        let temp = TempSource::new();
        temp.gzip("ENSG00000000001.tsv.gz", body);
        assert!(temp.inspect_error().contains(reason), "expected {reason}");
    }
}

#[test]
fn grouping_order_and_chromosome_failures_are_rejected() {
    let cases = [
        (
            "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50",
            "expected exactly three",
        ),
        (
            "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tG\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tG\t0.0\t-50\t-0.0\t-50",
            "duplicate alternate G",
        ),
        (
            "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tG\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tT\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tC\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tG\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tT\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50",
            "direction reverses",
        ),
        (
            "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tG\t0.0\t-50\t-0.0\t-50\nchr1\t100\tA\tT\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tC\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tG\t0.0\t-50\t-0.0\t-50\nchr1\t101\tA\tT\t0.0\t-50\t-0.0\t-50\nchr1\t99\tA\tC\t0.0\t-50\t-0.0\t-50",
            "direction reverses",
        ),
        (
            "chr1\t100\tA\tC\t0.0\t-50\t-0.0\t-50\nchr2\t100\tA\tG\t0.0\t-50\t-0.0\t-50",
            "mixes chromosomes",
        ),
        (
            "chr1\t100\tN\tA\t0.0\t-50\t-0.0\t-50\nchr1\t100\tN\tG\t0.0\t-50\t-0.0\t-50\nchr1\t100\tN\tT\t0.0\t-50\t-0.0\t-50",
            "must omit A or T",
        ),
    ];
    for (rows, reason) in cases {
        let temp = TempSource::new();
        temp.gzip("ENSG00000000001.tsv.gz", &source(rows));
        assert!(temp.inspect_error().contains(reason), "expected {reason}");
    }
}

#[test]
fn discovery_rejects_empty_invalid_names_and_symlinks() {
    let empty = TempSource::new();
    fs::write(empty.path().join("notes.txt"), "ignored").expect("unrelated file");
    assert!(empty.inspect_error().contains("contains no .tsv.gz"));

    let invalid = TempSource::new();
    invalid.gzip("not-a-gene.tsv.gz", &source(DEFAULT_GROUP));
    assert!(
        invalid
            .inspect_error()
            .contains("filename must be exactly ENSG")
    );

    let empty_member = TempSource::new();
    empty_member.gzip("ENSG00000000001.tsv.gz", "");
    assert!(
        empty_member
            .inspect_error()
            .contains("missing source header")
    );

    let header_only = TempSource::new();
    header_only.gzip("ENSG00000000001.tsv.gz", &format!("{HEADER}\n"));
    assert!(header_only.inspect_error().contains("has no data rows"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let linked = TempSource::new();
        let target = linked.path().join("target.gz");
        fs::write(&target, "not followed").expect("target");
        symlink(&target, linked.path().join("ENSG00000000001.tsv.gz")).expect("symlink");
        assert!(linked.inspect_error().contains("not a symlink"));
    }
}

#[test]
fn checked_malformed_fixture_reports_the_exact_line() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pangolin-precompute-malformed");
    let mut output = Vec::new();
    assert_eq!(
        inspect_directory(&fixture, &mut output)
            .expect_err("malformed fixture")
            .to_string(),
        "ENSG00000000003.tsv.gz:4: duplicate alternate G at chr1:100 A"
    );
    assert!(output.is_empty());
}
