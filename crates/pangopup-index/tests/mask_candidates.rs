use pangopup_core::{EnsemblGeneId, GencodeGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask_candidates::{
    CanonicalMaskGene, MaskCandidateCodec, MaskCandidateError, MaskCandidateReader,
    MaskQueryBuffer, MaskQueryGene, MaskStrand, write_mask_candidate,
    write_mask_candidate_with_cancellation,
};
use serde::Deserialize;
use std::{
    cell::Cell,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

static SCRATCH_SERIAL: AtomicU64 = AtomicU64::new(0);

fn assert_sync<T: Sync>() {}

#[test]
fn ordinary_reader_is_sync_without_runtime_trace_state() {
    assert_sync::<MaskCandidateReader>();
}

#[test]
fn bounded_cancellation_interrupts_long_build_and_full_inspection() {
    let genes = long_gene_stream(4_096);
    let scratch = Scratch::new();
    let cancelled_path = scratch.path().join("cancelled-build.pgm");
    let build_checks = Cell::new(0_u32);
    let cancel_during_build = || {
        build_checks.set(build_checks.get() + 1);
        build_checks.get() >= 3
    };
    assert!(matches!(
        write_mask_candidate_with_cancellation(
            &cancelled_path,
            MaskCandidateCodec::Domains,
            &genes,
            &cancel_during_build,
        ),
        Err(MaskCandidateError::Cancelled)
    ));
    assert_eq!(build_checks.get(), 3);
    assert!(
        !cancelled_path.exists(),
        "encoding cancellation must occur before publication"
    );

    let complete_path = scratch.path().join("complete.pgm");
    write_mask_candidate(&complete_path, MaskCandidateCodec::Domains, &genes)
        .expect("write complete candidate");
    let reader = MaskCandidateReader::open(&complete_path).expect("open complete candidate");
    let inspection_checks = Cell::new(0_u32);
    let cancel_during_inspection = || {
        inspection_checks.set(inspection_checks.get() + 1);
        inspection_checks.get() >= 3
    };
    assert!(matches!(
        reader.inspect_payload_with_cancellation(&cancel_during_inspection),
        Err(MaskCandidateError::Cancelled)
    ));
    assert_eq!(inspection_checks.get(), 3);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema: String,
    aliases: Vec<[String; 2]>,
    genes: Vec<GeneFixture>,
    queries: Vec<QueryFixture>,
    order_sensitive: OrderFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneFixture {
    id: String,
    contig: String,
    strand: String,
    start: u32,
    end: u32,
    rank: u32,
    source_boundaries: Vec<u32>,
    canonical_boundaries: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryFixture {
    contig: String,
    position: u32,
    plus: Vec<ExpectedGene>,
    minus: Vec<ExpectedGene>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExpectedGene {
    id: String,
    boundaries: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrderFixture {
    contig: String,
    position: u32,
    distance: u32,
    gain: Vec<i16>,
    loss: Vec<i16>,
    genes: Vec<String>,
    expected: Vec<ExpectedMask>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExpectedMask {
    id: String,
    gain: i16,
    gain_index: usize,
    loss: i16,
    loss_index: usize,
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let serial = SCRATCH_SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-mask-candidates-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create scratch");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gencode-mask-mini/fixture.json")
}

fn fixture() -> Fixture {
    serde_json::from_slice(&fs::read(fixture_path()).expect("read fixture")).expect("parse fixture")
}

fn typed_genes(fixture: &Fixture) -> Vec<CanonicalMaskGene> {
    fixture
        .genes
        .iter()
        .map(|gene| {
            CanonicalMaskGene::new(
                GencodeGeneId::from_str(&gene.id).expect("exact gene"),
                Grch38Contig::from_str(&gene.contig).expect("contig"),
                match gene.strand.as_str() {
                    "+" => MaskStrand::Plus,
                    "-" => MaskStrand::Minus,
                    _ => panic!("fixture strand"),
                },
                GenomicPosition::new(gene.start).expect("start"),
                GenomicPosition::new(gene.end).expect("end"),
                gene.rank,
                gene.source_boundaries
                    .iter()
                    .copied()
                    .map(|value| GenomicPosition::new(value).expect("boundary"))
                    .collect(),
            )
            .expect("canonical gene")
        })
        .collect()
}

fn build_candidates(
    root: &Path,
    genes: &[CanonicalMaskGene],
) -> Vec<(MaskCandidateCodec, PathBuf)> {
    MaskCandidateCodec::ALL
        .into_iter()
        .map(|codec| {
            let path = root.join(codec.filename());
            write_mask_candidate(&path, codec, genes).expect("write candidate");
            (codec, path)
        })
        .collect()
}

fn render(buffer: &MaskQueryBuffer, genes: &[MaskQueryGene]) -> Vec<ExpectedGene> {
    genes
        .iter()
        .map(|gene| ExpectedGene {
            id: gene.identity().to_string(),
            boundaries: buffer
                .boundaries(gene)
                .iter()
                .map(|boundary| boundary.get())
                .collect(),
        })
        .collect()
}

fn independent_expected(
    genes: &[CanonicalMaskGene],
    contig: Grch38Contig,
    position: GenomicPosition,
    strand: MaskStrand,
) -> Vec<ExpectedGene> {
    genes
        .iter()
        .filter(|gene| {
            gene.contig() == contig && gene.strand() == strand && gene.contains(position)
        })
        .map(|gene| ExpectedGene {
            id: gene.identity().to_string(),
            boundaries: gene.boundaries().iter().map(|value| value.get()).collect(),
        })
        .collect()
}

fn long_gene_stream(count: u32) -> Vec<CanonicalMaskGene> {
    (0..count)
        .map(|rank| {
            CanonicalMaskGene::new(
                GencodeGeneId::new(
                    EnsemblGeneId::from_numeric(1_000_000 + u64::from(rank)).expect("stable ID"),
                    1,
                    false,
                )
                .expect("exact ID"),
                Grch38Contig::autosome(1).expect("chr1"),
                MaskStrand::Plus,
                GenomicPosition::new(1).expect("start"),
                GenomicPosition::new(100).expect("end"),
                rank,
                Vec::new(),
            )
            .expect("long-stream gene")
        })
        .collect()
}

#[test]
fn miniature_oracle_is_independent_complete_and_exact_for_every_candidate() {
    let fixture = fixture();
    assert_eq!(fixture.schema, "pangopup-gencode-mask-mini-v1");
    assert_eq!(fixture.aliases.len(), 50);
    for [alias, canonical] in &fixture.aliases {
        assert_eq!(
            Grch38Contig::from_str(alias)
                .expect("supported alias")
                .to_string(),
            *canonical
        );
    }

    let genes = typed_genes(&fixture);
    for (source, canonical) in fixture.genes.iter().zip(&genes) {
        assert_eq!(
            canonical
                .boundaries()
                .iter()
                .map(|value| value.get())
                .collect::<Vec<_>>(),
            source.canonical_boundaries
        );
    }
    let par = &genes[6];
    let x = &genes[5];
    assert_eq!(x.stable_identity(), par.stable_identity());
    assert_ne!(x.identity(), par.identity());
    assert!(!x.identity().is_par_y());
    assert!(par.identity().is_par_y());

    let scratch = Scratch::new();
    let candidates = build_candidates(scratch.path(), &genes);
    let mut sizes = BTreeSet::new();
    for (codec, path) in candidates {
        let reader = MaskCandidateReader::open(&path).expect("open candidate");
        assert_eq!(reader.codec(), codec);
        reader.inspect_payload().expect("inspect candidate");
        sizes.insert(reader.file_len());
        let mut output = MaskQueryBuffer::with_capacity(8, 32);
        for query in &fixture.queries {
            let contig = Grch38Contig::from_str(&query.contig).expect("query contig");
            reader
                .query(
                    contig,
                    GenomicPosition::new(query.position).expect("query position"),
                    &mut output,
                )
                .expect("query candidate");
            assert_eq!(render(&output, output.plus()), query.plus, "{codec} plus");
            assert_eq!(
                render(&output, output.minus()),
                query.minus,
                "{codec} minus"
            );
        }

        // The small explicit oracle pins every named edge. This second pass
        // exhausts every coordinate in the miniature range against a simple
        // source-record implementation independent of all three codecs.
        for code in 1..=25 {
            let contig = Grch38Contig::from_code(code).expect("primary contig");
            for value in 1..=220 {
                let position = GenomicPosition::new(value).expect("position");
                reader
                    .query(contig, position, &mut output)
                    .expect("exhaustive miniature query");
                assert_eq!(
                    render(&output, output.plus()),
                    independent_expected(&genes, contig, position, MaskStrand::Plus),
                    "{codec} {contig}:{value} plus"
                );
                assert_eq!(
                    render(&output, output.minus()),
                    independent_expected(&genes, contig, position, MaskStrand::Minus),
                    "{codec} {contig}:{value} minus"
                );
            }
        }
    }
    assert!(
        sizes.len() >= 2,
        "candidate layouts must be materially distinct"
    );
}

#[test]
fn stable_filter_is_contig_scoped_and_retains_exact_par_identity() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    let stable = EnsemblGeneId::from_str("ENSG00000228572").expect("stable ID");
    for (_, path) in build_candidates(scratch.path(), &genes) {
        let reader = MaskCandidateReader::open(&path).expect("open candidate");
        let mut output = MaskQueryBuffer::default();
        reader
            .query_stable(
                Grch38Contig::X,
                GenomicPosition::new(101).expect("position"),
                Some(stable),
                &mut output,
            )
            .expect("X filter");
        assert_eq!(output.plus()[0].identity().to_string(), "ENSG00000228572.7");
        reader
            .query_stable(
                Grch38Contig::Y,
                GenomicPosition::new(201).expect("position"),
                Some(stable),
                &mut output,
            )
            .expect("Y filter");
        assert_eq!(output.plus().len(), 1);
        assert_eq!(
            output.plus()[0].identity().to_string(),
            "ENSG00000228572.7_PAR_Y"
        );
    }
}

#[test]
fn held_open_and_actual_page_trace_are_deterministic_and_nofollow() {
    use std::os::unix::fs::symlink;

    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    for (codec, path) in build_candidates(scratch.path(), &genes) {
        let alias = scratch.path().join(format!("{}.alias", codec.name()));
        symlink(&path, &alias).expect("create candidate symlink");
        assert!(
            MaskCandidateReader::open(&alias).is_err(),
            "{codec} must reject a followed candidate name"
        );

        let file = fs::File::open(&path).expect("open held candidate");
        let moved = scratch.path().join(format!("{}.moved", codec.name()));
        fs::rename(&path, &moved).expect("rename held candidate");
        let reader = MaskCandidateReader::open_held(file).expect("open held descriptor");
        let mut output = MaskQueryBuffer::with_capacity(8, 32);
        let first = reader
            .query_with_page_trace(
                Grch38Contig::from_str("chr2").expect("contig"),
                GenomicPosition::new(15).expect("position"),
                &mut output,
            )
            .expect("first trace");
        let second = reader
            .query_with_page_trace(
                Grch38Contig::from_str("chr2").expect("contig"),
                GenomicPosition::new(15).expect("position"),
                &mut output,
            )
            .expect("second trace");
        assert_eq!(first, second, "{codec} trace must be deterministic");
        assert!(!first.metadata_pages.is_empty());
        assert!(!first.payload_pages.is_empty());
        assert!(
            first
                .metadata_pages
                .windows(2)
                .all(|pages| pages[0] < pages[1])
        );
        assert!(
            first
                .payload_pages
                .windows(2)
                .all(|pages| pages[0] < pages[1])
        );
    }
}

#[test]
fn recorded_same_strand_order_changes_masking_result() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    let path = scratch.path().join("order.pgm");
    write_mask_candidate(&path, MaskCandidateCodec::IntervalTree, &genes).expect("write candidate");
    let reader = MaskCandidateReader::open(&path).expect("open candidate");
    let order = &fixture.order_sensitive;
    let contig = Grch38Contig::from_str(&order.contig).expect("contig");
    let mut output = MaskQueryBuffer::default();
    reader
        .query(
            contig,
            GenomicPosition::new(order.position).expect("position"),
            &mut output,
        )
        .expect("query");
    let selected = output
        .plus()
        .iter()
        .filter(|gene| order.genes.contains(&gene.identity().to_string()))
        .map(|gene| {
            (
                gene.identity().to_string(),
                output
                    .boundaries(gene)
                    .iter()
                    .map(|value| value.get())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let actual = replay_mask(order, &selected);
    assert_eq!(actual, order.expected);

    let mut reversed = selected;
    reversed.reverse();
    assert_ne!(replay_mask(order, &reversed), order.expected);
}

fn replay_mask(order: &OrderFixture, genes: &[(String, Vec<u32>)]) -> Vec<ExpectedMask> {
    let mut gain = order.gain.clone();
    let mut loss = order.loss.clone();
    let window_start = order.position - order.distance;
    genes
        .iter()
        .map(|(id, boundaries)| {
            let positions = boundaries
                .iter()
                .filter_map(|boundary| boundary.checked_sub(window_start))
                .filter_map(|position| {
                    usize::try_from(position)
                        .ok()
                        .filter(|position| *position < gain.len())
                })
                .collect::<BTreeSet<_>>();
            for position in &positions {
                gain[*position] = gain[*position].min(0);
            }
            for (index, value) in loss.iter_mut().enumerate() {
                if !positions.contains(&index) {
                    *value = (*value).max(0);
                }
            }
            let (gain_index, gain_value) = gain
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|(index, value)| (*value, std::cmp::Reverse(*index)))
                .expect("gain value");
            let (loss_index, loss_value) = loss
                .iter()
                .copied()
                .enumerate()
                .min_by_key(|(index, value)| (*value, *index))
                .expect("loss value");
            ExpectedMask {
                id: id.clone(),
                gain: gain_value,
                gain_index,
                loss: loss_value,
                loss_index,
            }
        })
        .collect()
}

fn mutate(path: &Path, change: impl FnOnce(&mut Vec<u8>)) -> PathBuf {
    let target = path.with_extension(format!(
        "mutation-{}",
        SCRATCH_SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut bytes = fs::read(path).expect("read candidate");
    change(&mut bytes);
    fs::write(&target, bytes).expect("write mutation");
    target
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

#[test]
fn candidate_open_rejects_header_directory_truncation_and_trailing_corruption() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    for (_, path) in build_candidates(scratch.path(), &genes) {
        let changes: [fn(&mut Vec<u8>); 6] = [
            |bytes| bytes[0] ^= 1,
            |bytes| bytes[12] = 1,
            |bytes| bytes[10] = 99,
            |bytes| bytes[160] = 0,
            |bytes| bytes.truncate(bytes.len() - 1),
            |bytes| bytes.push(0),
        ];
        for change in changes {
            let mutation = mutate(&path, change);
            assert!(MaskCandidateReader::open(&mutation).is_err());
        }
    }
}

#[test]
fn payload_corruption_is_lazy_but_touched_queries_and_inspection_fail_closed() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    for (codec, path) in build_candidates(scratch.path(), &genes) {
        let index_mutation = mutate(&path, |bytes| {
            let index_offset = u64_at(bytes, 32 + 3 * 24) as usize;
            match codec {
                MaskCandidateCodec::IntervalTree => {
                    bytes[index_offset + 8..index_offset + 12]
                        .copy_from_slice(&999_999_u32.to_le_bytes());
                }
                MaskCandidateCodec::Domains => {
                    bytes[index_offset + 8..index_offset + 12]
                        .copy_from_slice(&u32::MAX.to_le_bytes());
                }
                MaskCandidateCodec::BinnedPostings => bytes[index_offset + 4] = 1,
            }
        });
        let reader = MaskCandidateReader::open(&index_mutation).expect("cheap open");
        let mut output = MaskQueryBuffer::default();
        assert!(
            reader
                .query(
                    Grch38Contig::autosome(1).expect("chr1"),
                    GenomicPosition::new(2).expect("position"),
                    &mut output,
                )
                .is_err()
        );
        assert!(output.plus().is_empty() && output.minus().is_empty());
        assert!(reader.inspect_payload().is_err());

        let boundary_mutation = mutate(&path, |bytes| {
            let boundary_offset = u64_at(bytes, 32 + 2 * 24) as usize;
            let boundary_count = u64_at(bytes, 32 + 2 * 24 + 8) as usize;
            let last = boundary_offset + (boundary_count - 1) * 4;
            bytes[last..last + 4].copy_from_slice(&0_u32.to_le_bytes());
        });
        let reader = MaskCandidateReader::open(&boundary_mutation).expect("cheap open");
        reader
            .query(
                Grch38Contig::autosome(1).expect("chr1"),
                GenomicPosition::new(2).expect("position"),
                &mut output,
            )
            .expect("unrelated query avoids corrupt payload");
        assert!(
            reader
                .query(
                    Grch38Contig::Y,
                    GenomicPosition::new(201).expect("position"),
                    &mut output,
                )
                .is_err()
        );
        assert!(output.plus().is_empty() && output.minus().is_empty());
        assert!(reader.inspect_payload().is_err());
    }
}

#[test]
fn payload_inspection_rejects_rank_identity_and_boundary_range_corruption() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    for (_, path) in build_candidates(scratch.path(), &genes) {
        let mutations: [fn(&mut Vec<u8>); 4] = [
            |bytes| {
                let genes = u64_at(bytes, 32 + 24) as usize;
                let second_rank = genes + 40 + 12;
                let third_rank = genes + 80 + 12;
                let rank = bytes[second_rank..second_rank + 4].to_vec();
                bytes[third_rank..third_rank + 4].copy_from_slice(&rank);
            },
            |bytes| {
                let genes = u64_at(bytes, 32 + 24) as usize;
                bytes[genes + 16..genes + 24].copy_from_slice(&u64::MAX.to_le_bytes());
            },
            |bytes| {
                let genes = u64_at(bytes, 32 + 24) as usize;
                bytes[genes + 24..genes + 28].copy_from_slice(&0_u32.to_le_bytes());
            },
            |bytes| {
                let genes = u64_at(bytes, 32 + 24) as usize;
                bytes[genes + 28..genes + 32].copy_from_slice(&u32::MAX.to_le_bytes());
            },
        ];
        for (index, change) in mutations.into_iter().enumerate() {
            let mutation = mutate(&path, change);
            let reader = MaskCandidateReader::open(&mutation).expect("cheap open");
            assert!(
                reader.inspect_payload().is_err(),
                "mutation {index} must fail for {}",
                path.display()
            );
        }
    }
}

#[test]
fn writer_rejects_noncanonical_streams_duplicates_and_bad_boundaries() {
    let fixture = fixture();
    let genes = typed_genes(&fixture);
    let scratch = Scratch::new();
    let mut reordered = genes.clone();
    reordered.swap(0, 1);
    assert!(
        write_mask_candidate(
            &scratch.path().join("reordered.pgm"),
            MaskCandidateCodec::Domains,
            &reordered,
        )
        .is_err()
    );

    let first = &genes[0];
    let duplicate = CanonicalMaskGene::new(
        first.identity(),
        first.contig(),
        first.strand(),
        first.start(),
        first.end(),
        1,
        first.boundaries().to_vec(),
    )
    .expect("duplicate fixture");
    assert!(
        write_mask_candidate(
            &scratch.path().join("duplicate.pgm"),
            MaskCandidateCodec::Domains,
            &[first.clone(), duplicate],
        )
        .is_err()
    );

    // This stream is otherwise canonically ordered and each contig has only
    // one record. It therefore distinguishes a global exact-ID check from the
    // former `(contig, exact ID)` check.
    let cross_contig_duplicate = CanonicalMaskGene::new(
        first.identity(),
        Grch38Contig::autosome(2).expect("chr2"),
        first.strand(),
        GenomicPosition::new(10).expect("start"),
        GenomicPosition::new(20).expect("end"),
        0,
        Vec::new(),
    )
    .expect("cross-contig duplicate fixture");
    assert!(matches!(
        write_mask_candidate(
            &scratch.path().join("cross-contig-duplicate.pgm"),
            MaskCandidateCodec::Domains,
            &[first.clone(), cross_contig_duplicate],
        ),
        Err(MaskCandidateError::Input("duplicate exact gene identity"))
    ));
    assert!(
        CanonicalMaskGene::new(
            first.identity(),
            first.contig(),
            first.strand(),
            first.start(),
            first.end(),
            first.query_rank(),
            vec![GenomicPosition::new(5).expect("outside boundary")],
        )
        .is_err()
    );
}
