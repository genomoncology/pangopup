use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, PangolinScore, RelativePosition,
    ScoreMagnitude,
};
use pangopup_index::{
    AmbiguousInputLocus, InputAlternative, InputLocus, OrdinaryInputLocus,
    sparse_writer::{SPARSE_INDEX_FORMAT, SparseIndexWriter},
};
use std::{fs, path::PathBuf};

const HEADER: usize = 256;
const GENE: usize = 32;
const SEGMENT: usize = 48;
const BLOCK: usize = 40;
const EXCEPTION: usize = 40;
const BLOCK_LOCI: usize = 4096;
const RANK_STRIDE: usize = 64;

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-sparse-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create sparse test directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove sparse test directory");
    }
}

#[test]
fn deterministic_candidate_round_trips_edges_with_independent_decoder() {
    let temp = Temp::new("roundtrip");
    let genes = miniature();
    let mut outputs = Vec::new();
    for run in 0..2 {
        let scratch = temp.0.join(format!("scratch-{run}"));
        let output = temp.0.join(format!("candidate-{run}.pgi"));
        let mut writer = SparseIndexWriter::create(&scratch).expect("create sparse writer");
        for gene in &genes {
            writer.push_gene(gene).expect("submit complete gene");
        }
        let summary = writer.finish(&output).expect("finish sparse writer");
        assert_eq!(summary.genes, 3);
        assert_eq!(
            summary.loci,
            genes.iter().map(|gene| gene.len() as u64).sum::<u64>()
        );
        assert!(summary.blocks >= 3, "fixture crosses a block boundary");
        assert_eq!(summary.exceptions, 2);
        assert!(!scratch.exists());
        assert!(!PathBuf::from(format!("{}.exceptions", scratch.display())).exists());
        outputs.push(fs::read(output).expect("read candidate"));
    }
    assert_eq!(
        outputs[0], outputs[1],
        "same input must produce the same bytes"
    );
    let decoded = decode_candidate(&outputs[0]);
    let mut expected: Vec<_> = genes.into_iter().flatten().map(canonical).collect();
    expected.sort_by_key(key);
    assert_eq!(decoded, expected);
    assert_eq!(SPARSE_INDEX_FORMAT, "pangopup.sparse-direct.v1");
}

#[test]
fn independent_decoder_checks_every_directory_rank_value_and_reserved_byte() {
    let temp = Temp::new("decoder-mutations");
    let scratch = temp.0.join("payload.scratch");
    let output = temp.0.join("candidate.pgi");
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    for gene in miniature() {
        writer.push_gene(&gene).expect("gene");
    }
    writer.finish(&output).expect("candidate");
    let bytes = fs::read(output).expect("candidate bytes");
    let expected = decode_candidate(&bytes);
    let sections = [24, 40, 56, 72, 88]
        .map(|at| (u64_at(&bytes, at) as usize, u64_at(&bytes, at + 8) as usize));
    let gene_count = u64_at(&bytes, 104) as usize;
    let segment_count = u64_at(&bytes, 112) as usize;
    let block_count = u64_at(&bytes, 120) as usize;
    let exception_count = u64_at(&bytes, 136) as usize;

    for offset in [
        16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136,
    ] {
        assert_mutation_detected(&bytes, &expected, offset, "header field");
    }
    for offset in 152..HEADER {
        assert_mutation_detected(&bytes, &expected, offset, "header reserved byte");
    }

    for index in 0..gene_count {
        let base = sections[0].0 + index * GENE;
        for field in [0, 8, 16, 20] {
            assert_mutation_detected(&bytes, &expected, base + field, "gene directory field");
        }
        for offset in base + 24..base + GENE {
            assert_mutation_detected(&bytes, &expected, offset, "gene reserved byte");
        }
    }

    for index in 0..segment_count {
        let base = sections[1].0 + index * SEGMENT;
        for field in [0, 8, 16, 20, 24, 28, 32] {
            assert_mutation_detected(&bytes, &expected, base + field, "segment directory field");
        }
        for offset in base + 9..base + 16 {
            assert_mutation_detected(&bytes, &expected, offset, "segment reserved byte");
        }
        for offset in base + 36..base + SEGMENT {
            assert_mutation_detected(&bytes, &expected, offset, "segment reserved byte");
        }
    }

    for index in 0..block_count {
        let base = sections[2].0 + index * BLOCK;
        for field in [0, 8, 12, 16, 24, 28, 32] {
            assert_mutation_detected(&bytes, &expected, base + field, "block directory field");
        }
        for offset in base + 36..base + BLOCK {
            assert_mutation_detected(&bytes, &expected, offset, "block reserved byte");
        }

        let count = u32_at(&bytes, base + 12) as usize;
        let raw = sections[3].0 + u64_at(&bytes, base + 16) as usize;
        let active_total = u32_at(&bytes, raw) as usize;
        let pair_total = u32_at(&bytes, raw + 4) as usize;
        let rank_total = u32_at(&bytes, raw + 8) as usize;
        let refs_len = (count * 2).div_ceil(8);
        let active_len = count.div_ceil(8);
        let ranks_start = raw + 16 + refs_len + active_len;
        let masks_len = (active_total * 6).div_ceil(8);
        let values_start = ranks_start + rank_total * 8 + masks_len;
        for offset in raw + 12..raw + 16 {
            assert_mutation_detected(&bytes, &expected, offset, "block header reserved byte");
        }
        for rank in 0..rank_total {
            assert_mutation_detected(
                &bytes,
                &expected,
                ranks_start + rank * 8,
                "rank active value",
            );
            assert_mutation_detected(
                &bytes,
                &expected,
                ranks_start + rank * 8 + 4,
                "rank pair value",
            );
        }
        for pair in 0..pair_total {
            assert_mutation_detected(
                &bytes,
                &expected,
                values_start + pair * 2,
                "score pair value",
            );
        }
    }

    for index in 0..exception_count {
        let base = sections[4].0 + index * EXCEPTION;
        for field in [0, 1, 2, 3, 4, 8, 16, 24, 26, 28, 30, 32, 34] {
            assert_mutation_detected(&bytes, &expected, base + field, "exception field");
        }
        for offset in base + 5..base + 8 {
            assert_mutation_detected(&bytes, &expected, offset, "exception reserved byte");
        }
        for offset in base + 20..base + 24 {
            assert_mutation_detected(&bytes, &expected, offset, "exception reserved byte");
        }
        for offset in base + 36..base + EXCEPTION {
            assert_mutation_detected(&bytes, &expected, offset, "exception reserved byte");
        }
    }
}

fn assert_mutation_detected(
    bytes: &[u8],
    expected: &[InputLocus],
    offset: usize,
    description: &str,
) {
    let mut changed = bytes.to_vec();
    changed[offset] ^= 1;
    let decoded = std::panic::catch_unwind(|| decode_candidate(&changed));
    assert!(
        decoded.is_err() || decoded.expect("checked successful decode") != expected,
        "decoder ignored {description} at byte {offset}"
    );
}

#[test]
fn invalid_submission_poisoning_and_paths_are_preserved() {
    let temp = Temp::new("failures");
    let score = default_score();
    let gene1 = gene(1);
    let gene2 = gene(2);
    let valid1 = ordinary(gene1, 1, 1, DnaBase::A, score);
    let valid2 = ordinary(gene2, 1, 1, DnaBase::A, score);

    let cases: Vec<Vec<InputLocus>> = vec![
        Vec::new(),
        vec![valid1, valid2],
        vec![
            ordinary(gene1, 1, 2, DnaBase::A, score),
            ordinary(gene1, 1, 1, DnaBase::C, score),
        ],
        vec![valid1, valid1],
        vec![
            valid1,
            InputLocus::Ambiguous(AmbiguousInputLocus {
                gene: gene1,
                contig: contig(1),
                position: position(1),
                alternatives: alternatives(DnaBase::A, score),
                omitted: DnaBase::A,
            }),
        ],
        vec![InputLocus::Ordinary(OrdinaryInputLocus {
            gene: gene1,
            contig: contig(1),
            position: position(1),
            reference: DnaBase::A,
            alternatives: [
                InputAlternative {
                    alternate: DnaBase::C,
                    score,
                },
                InputAlternative {
                    alternate: DnaBase::C,
                    score,
                },
                InputAlternative {
                    alternate: DnaBase::T,
                    score,
                },
            ],
        })],
        vec![InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: gene1,
            contig: contig(1),
            position: position(1),
            alternatives: alternatives(DnaBase::C, score),
            omitted: DnaBase::C,
        })],
        vec![InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: gene1,
            contig: contig(1),
            position: position(1),
            alternatives: [
                InputAlternative {
                    alternate: DnaBase::C,
                    score,
                },
                InputAlternative {
                    alternate: DnaBase::C,
                    score,
                },
                InputAlternative {
                    alternate: DnaBase::T,
                    score,
                },
            ],
            omitted: DnaBase::A,
        })],
        vec![ordinary(
            gene1,
            1,
            1,
            DnaBase::A,
            PangolinScore::new(magnitude(1), relative(51), magnitude(0), relative(-50)),
        )],
    ];
    for (index, invalid) in cases.iter().enumerate() {
        let scratch = temp.0.join(format!("invalid-{index}.scratch"));
        let output = temp.0.join(format!("invalid-{index}.pgi"));
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        assert!(writer.push_gene(invalid).is_err(), "case {index} must fail");
        assert!(
            writer.push_gene(&[valid1]).is_err(),
            "case {index} must poison the writer"
        );
        assert!(writer.finish(&output).is_err());
        assert!(!output.exists());
        assert!(!scratch.exists());
    }

    let scratch = temp.0.join("order.scratch");
    let output = temp.0.join("order.pgi");
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    writer.push_gene(&[valid2]).expect("first gene");
    assert!(writer.push_gene(&[valid1]).is_err());
    assert!(writer.finish(&output).is_err());
    assert!(!output.exists());

    let scratch = temp.0.join("repeated.scratch");
    let output = temp.0.join("repeated.pgi");
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    writer.push_gene(&[valid1]).expect("first gene");
    assert!(writer.push_gene(&[valid1]).is_err());
    assert!(writer.finish(&output).is_err());
    assert!(!output.exists());

    let scratch = temp.0.join("empty-stream.scratch");
    let output = temp.0.join("empty-stream.pgi");
    let writer = SparseIndexWriter::create(&scratch).expect("writer");
    assert!(writer.finish(&output).is_err());
    assert!(!output.exists());

    let existing_scratch = temp.0.join("existing.scratch");
    fs::write(&existing_scratch, b"keep scratch").expect("seed scratch");
    assert!(SparseIndexWriter::create(&existing_scratch).is_err());
    assert_eq!(
        fs::read(&existing_scratch).expect("scratch"),
        b"keep scratch"
    );

    let blocked_sidecar = temp.0.join("blocked.scratch");
    let blocked_exception = PathBuf::from(format!("{}.exceptions", blocked_sidecar.display()));
    fs::write(&blocked_exception, b"keep sidecar").expect("seed sidecar");
    assert!(SparseIndexWriter::create(&blocked_sidecar).is_err());
    assert!(!blocked_sidecar.exists());
    assert_eq!(
        fs::read(blocked_exception).expect("sidecar"),
        b"keep sidecar"
    );

    let scratch = temp.0.join("output.scratch");
    let output = temp.0.join("existing.pgi");
    fs::write(&output, b"keep output").expect("seed output");
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    writer.push_gene(&[valid1]).expect("valid gene");
    assert!(writer.finish(&output).is_err());
    assert_eq!(fs::read(&output).expect("output"), b"keep output");
    assert!(!scratch.exists());
    assert_eq!(
        fs::read_dir(&temp.0)
            .expect("directory")
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains("pangopup-sparse-stage"))
            .count(),
        0
    );
}

#[test]
fn bare_relative_output_is_published_and_cleaned() {
    let temp = Temp::new("bare-output");
    let scratch = temp.0.join("payload.scratch");
    let output = PathBuf::from(format!(
        "pangopup-sparse-bare-output-{}.pgi",
        std::process::id()
    ));
    let _ = fs::remove_file(&output);
    assert!(
        output
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
    );
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    writer
        .push_gene(&[ordinary(gene(1), 1, 1, DnaBase::A, default_score())])
        .expect("gene");
    writer.finish(&output).expect("bare output");
    assert!(output.exists());
    assert!(!scratch.exists());
    fs::remove_file(output).expect("remove bare output");
}

fn miniature() -> Vec<Vec<InputLocus>> {
    let mut first = Vec::new();
    for coordinate in 1..=BLOCK_LOCI as u32 + 2 {
        let reference = DnaBase::ALL[(coordinate as usize - 1) % 4];
        if matches!(coordinate, 63..=65 | 4095..=4097) {
            first.push(ordinary_with_alternatives(
                gene(1),
                1,
                coordinate,
                reference,
                varied_alternatives(reference, coordinate),
            ));
        } else {
            let score = match coordinate {
                2 => PangolinScore::new(magnitude(100), relative(50), magnitude(99), relative(-49)),
                3 => PangolinScore::new(magnitude(0), relative(0), magnitude(0), relative(50)),
                _ => default_score(),
            };
            first.push(ordinary(gene(1), 1, coordinate, reference, score));
        }
    }
    first.push(ordinary(gene(1), 1, 5_000, DnaBase::T, default_score()));
    first.push(InputLocus::Ambiguous(AmbiguousInputLocus {
        gene: gene(1),
        contig: contig(1),
        position: position(5_001),
        alternatives: alternatives(DnaBase::A, edge_score()),
        omitted: DnaBase::A,
    }));

    let second = vec![
        ordinary(gene(2), 1, 1, DnaBase::G, edge_score()),
        InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: gene(2),
            contig: contig(2),
            position: position(77),
            alternatives: alternatives(DnaBase::T, default_score()),
            omitted: DnaBase::T,
        }),
    ];
    let third = vec![ordinary(gene(3), 25, u32::MAX, DnaBase::C, edge_score())];
    vec![first, second, third]
}

#[derive(Clone, Copy)]
struct GeneDirectory {
    gene: u64,
    segment_start: usize,
    segment_count: usize,
    ordinary_loci: u32,
}

#[derive(Clone, Copy)]
struct SegmentDirectory {
    gene: u64,
    contig: u8,
    start: u32,
    end: u32,
    loci: u32,
    block_start: usize,
    block_count: usize,
}

fn decode_candidate(bytes: &[u8]) -> Vec<InputLocus> {
    assert_eq!(&bytes[..8], b"PGSPRS01");
    assert_eq!(u32_at(bytes, 8), 1);
    assert_eq!(u32_at(bytes, 12), HEADER as u32);
    assert_eq!(u64_at(bytes, 16), bytes.len() as u64);
    let sections = [24, 40, 56, 72, 88].map(|at| (u64_at(bytes, at), u64_at(bytes, at + 8)));
    assert_eq!(sections[0].0, HEADER as u64);
    for pair in sections.windows(2) {
        assert_eq!(pair[0].0 + pair[0].1, pair[1].0);
    }
    assert_eq!(sections[4].0 + sections[4].1, bytes.len() as u64);
    let gene_count = u64_at(bytes, 104) as usize;
    let segment_count = u64_at(bytes, 112) as usize;
    let block_count = u64_at(bytes, 120) as usize;
    let ordinary_count = u64_at(bytes, 128) as usize;
    let exception_count = u64_at(bytes, 136) as usize;
    assert_eq!(u32_at(bytes, 144), BLOCK_LOCI as u32);
    assert_eq!(u32_at(bytes, 148), RANK_STRIDE as u32);
    assert!(bytes[152..HEADER].iter().all(|byte| *byte == 0));
    assert_eq!(sections[0].1 as usize, gene_count * GENE);
    assert_eq!(sections[1].1 as usize, segment_count * SEGMENT);
    assert_eq!(sections[2].1 as usize, block_count * BLOCK);
    assert_eq!(sections[4].1 as usize, exception_count * EXCEPTION);

    let gene_base = sections[0].0 as usize;
    let mut previous_gene = 0;
    let mut genes = Vec::with_capacity(gene_count);
    for index in 0..gene_count {
        let row = &bytes[gene_base + index * GENE..gene_base + (index + 1) * GENE];
        let gene = u64_at(row, 0);
        assert!(gene > previous_gene);
        previous_gene = gene;
        assert!(row[24..].iter().all(|byte| *byte == 0));
        genes.push(GeneDirectory {
            gene,
            segment_start: u64_at(row, 8) as usize,
            segment_count: u32_at(row, 16) as usize,
            ordinary_loci: u32_at(row, 20),
        });
    }

    let segment_base = sections[1].0 as usize;
    let mut segments = Vec::with_capacity(segment_count);
    let mut previous_segment = None;
    for segment_index in 0..segment_count {
        let row = &bytes
            [segment_base + segment_index * SEGMENT..segment_base + (segment_index + 1) * SEGMENT];
        assert!(row[9..16].iter().all(|byte| *byte == 0));
        assert!(row[36..].iter().all(|byte| *byte == 0));
        let segment = SegmentDirectory {
            gene: u64_at(row, 0),
            contig: row[8],
            start: u32_at(row, 16),
            end: u32_at(row, 20),
            loci: u32_at(row, 24),
            block_start: u32_at(row, 28) as usize,
            block_count: u32_at(row, 32) as usize,
        };
        assert!(segment.loci > 0);
        assert!(segment.block_count > 0);
        assert_eq!(
            segment.end,
            segment
                .start
                .checked_add(segment.loci - 1)
                .expect("segment coordinate end")
        );
        let order = (segment.gene, segment.contig, segment.start);
        assert!(previous_segment.is_none_or(|previous| previous < order));
        previous_segment = Some(order);
        segments.push(segment);
    }

    let mut segment_cursor = 0_usize;
    let mut gene_ordinary_total = 0_u64;
    for gene_row in &genes {
        assert_eq!(gene_row.segment_start, segment_cursor);
        let segment_end = segment_cursor + gene_row.segment_count;
        let gene_segments = segments
            .get(segment_cursor..segment_end)
            .expect("gene segment bounds");
        assert!(
            gene_segments
                .iter()
                .all(|segment| segment.gene == gene_row.gene)
        );
        let ordinary_loci: u32 = gene_segments.iter().map(|segment| segment.loci).sum();
        assert_eq!(gene_row.ordinary_loci, ordinary_loci);
        gene_ordinary_total += u64::from(ordinary_loci);
        segment_cursor = segment_end;
    }
    assert_eq!(segment_cursor, segment_count);
    assert_eq!(gene_ordinary_total, ordinary_count as u64);

    let block_base = sections[2].0 as usize;
    let payload_base = sections[3].0 as usize;
    let mut decoded = Vec::with_capacity(ordinary_count + exception_count);
    let mut block_cursor = 0_usize;
    let mut payload_cursor = 0_usize;
    for (segment_index, segment) in segments.iter().copied().enumerate() {
        assert_eq!(segment.block_start, block_cursor);
        let mut seen = 0_u32;
        for block_index in segment.block_start..segment.block_start + segment.block_count {
            let block =
                &bytes[block_base + block_index * BLOCK..block_base + (block_index + 1) * BLOCK];
            assert_eq!(u64_at(block, 0), segment_index as u64);
            assert_eq!(u32_at(block, 8), seen);
            assert!(block[36..].iter().all(|byte| *byte == 0));
            let count = u32_at(block, 12) as usize;
            let offset = u64_at(block, 16) as usize;
            let length = u32_at(block, 24) as usize;
            assert!((1..=BLOCK_LOCI).contains(&count));
            assert_eq!(offset, payload_cursor);
            let raw = &bytes[payload_base + offset..payload_base + offset + length];
            let loci_in_block = decode_block(
                raw,
                count,
                gene(segment.gene),
                contig(segment.contig),
                segment.start + seen,
            );
            assert_eq!(u32_at(block, 28), active_count(raw));
            assert_eq!(u32_at(block, 32), pair_count(raw));
            decoded.extend(loci_in_block);
            seen += count as u32;
            payload_cursor += length;
            block_cursor += 1;
        }
        assert_eq!(seen, segment.loci);
    }
    assert_eq!(block_cursor, block_count);
    assert_eq!(payload_cursor, sections[3].1 as usize);
    assert_eq!(decoded.len(), ordinary_count);

    let exception_base = sections[4].0 as usize;
    let mut previous_exception = None;
    for index in 0..exception_count {
        let row =
            &bytes[exception_base + index * EXCEPTION..exception_base + (index + 1) * EXCEPTION];
        assert!(row[5..8].iter().all(|byte| *byte == 0));
        assert!(row[20..24].iter().all(|byte| *byte == 0));
        assert!(row[36..40].iter().all(|byte| *byte == 0));
        let omitted = decode_base(row[1]);
        let alternatives = [0, 1, 2].map(|alternative| InputAlternative {
            alternate: decode_base(row[2 + alternative]),
            score: decode_score_pair(row, 24 + alternative * 4),
        });
        assert!(
            alternatives
                .windows(2)
                .all(|pair| pair[0].alternate < pair[1].alternate)
        );
        let expected: Vec<_> = DnaBase::ALL
            .into_iter()
            .filter(|base| *base != omitted)
            .collect();
        assert_eq!(
            alternatives.map(|alternative| alternative.alternate),
            expected.as_slice()
        );
        let exception_order = (u64_at(row, 8), row[0], u32_at(row, 16));
        assert!(previous_exception.is_none_or(|previous| previous < exception_order));
        previous_exception = Some(exception_order);
        decoded.push(InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: gene(exception_order.0),
            contig: contig(exception_order.1),
            position: position(exception_order.2),
            alternatives,
            omitted,
        }));
    }
    decoded.sort_by_key(key);
    decoded
}

fn decode_block(
    raw: &[u8],
    count: usize,
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
) -> Vec<InputLocus> {
    assert_eq!(u32_at(raw, 12), 0);
    let active_total = active_count(raw) as usize;
    let pairs_total = pair_count(raw) as usize;
    let ranks = u32_at(raw, 8) as usize;
    assert_eq!(ranks, count.div_ceil(RANK_STRIDE));
    let refs_start = 16;
    let refs_len = (count * 2).div_ceil(8);
    let active_start = refs_start + refs_len;
    let active_len = count.div_ceil(8);
    let ranks_start = active_start + active_len;
    let masks_start = ranks_start + ranks * 8;
    let masks_len = (active_total * 6).div_ceil(8);
    let values_start = masks_start + masks_len;
    assert_eq!(values_start + pairs_total * 2, raw.len());
    let mut active_index = 0_usize;
    let mut pair_index = 0_usize;
    let mut output = Vec::with_capacity(count);
    for ordinal in 0..count {
        if ordinal % RANK_STRIDE == 0 {
            let rank = ranks_start + (ordinal / RANK_STRIDE) * 8;
            assert_eq!(u32_at(raw, rank), active_index as u32);
            assert_eq!(u32_at(raw, rank + 4), pair_index as u32);
        }
        let reference = decode_base((raw[refs_start + ordinal * 2 / 8] >> (ordinal * 2 % 8)) & 3);
        let mask = if raw[active_start + ordinal / 8] & (1 << (ordinal % 8)) == 0 {
            0
        } else {
            let bit = active_index * 6;
            let packed = u16::from(raw[masks_start + bit / 8])
                | (u16::from(*raw.get(masks_start + bit / 8 + 1).unwrap_or(&0)) << 8);
            active_index += 1;
            ((packed >> (bit % 8)) & 0x3f) as u8
        };
        let mut alternatives = alternatives(reference, default_score());
        alternatives.sort_by_key(|value| value.alternate);
        for (alternative_index, alternative) in alternatives.iter_mut().enumerate() {
            let mut components = [(magnitude(0), relative(-50)); 2];
            for (kind, component) in components.iter_mut().enumerate() {
                if mask & (1 << (alternative_index * 2 + kind)) != 0 {
                    *component = decode_pair(u16_at(raw, values_start + pair_index * 2));
                    pair_index += 1;
                }
            }
            alternative.score = PangolinScore::new(
                components[0].0,
                components[0].1,
                components[1].0,
                components[1].1,
            );
        }
        output.push(InputLocus::Ordinary(OrdinaryInputLocus {
            gene,
            contig,
            position: position(start + ordinal as u32),
            reference,
            alternatives,
        }));
    }
    assert_eq!(active_index, active_total);
    assert_eq!(pair_index, pairs_total);
    assert_zero_tail_bits(&raw[refs_start..active_start], count * 2);
    assert_zero_tail_bits(&raw[active_start..ranks_start], count);
    assert_zero_tail_bits(&raw[masks_start..values_start], active_total * 6);
    output
}

fn assert_zero_tail_bits(bytes: &[u8], used_bits: usize) {
    let remainder = used_bits % 8;
    if remainder != 0 {
        let mask = !((1_u16 << remainder) - 1) as u8;
        assert_eq!(bytes.last().copied().expect("tail byte") & mask, 0);
    }
}

fn active_count(raw: &[u8]) -> u32 {
    u32_at(raw, 0)
}
fn pair_count(raw: &[u8]) -> u32 {
    u32_at(raw, 4)
}

fn decode_score_pair(bytes: &[u8], at: usize) -> PangolinScore {
    let gain = decode_pair(u16_at(bytes, at));
    let loss = decode_pair(u16_at(bytes, at + 2));
    PangolinScore::new(gain.0, gain.1, loss.0, loss.1)
}

fn decode_pair(value: u16) -> (ScoreMagnitude, RelativePosition) {
    assert_eq!(value >> 14, 0);
    (
        magnitude(value & 0x7f),
        relative(((value >> 7) as i16) - 50),
    )
}

fn canonical(locus: InputLocus) -> InputLocus {
    match locus {
        InputLocus::Ordinary(mut value) => {
            value
                .alternatives
                .sort_by_key(|alternative| alternative.alternate);
            InputLocus::Ordinary(value)
        }
        InputLocus::Ambiguous(mut value) => {
            value
                .alternatives
                .sort_by_key(|alternative| alternative.alternate);
            InputLocus::Ambiguous(value)
        }
    }
}

fn key(locus: &InputLocus) -> (u64, u8, u32) {
    match locus {
        InputLocus::Ordinary(value) => (
            value.gene.numeric(),
            value.contig.code(),
            value.position.get(),
        ),
        InputLocus::Ambiguous(value) => (
            value.gene.numeric(),
            value.contig.code(),
            value.position.get(),
        ),
    }
}

fn ordinary(
    gene: EnsemblGeneId,
    contig_code: u8,
    coordinate: u32,
    reference: DnaBase,
    score: PangolinScore,
) -> InputLocus {
    ordinary_with_alternatives(
        gene,
        contig_code,
        coordinate,
        reference,
        alternatives(reference, score),
    )
}

fn ordinary_with_alternatives(
    gene: EnsemblGeneId,
    contig_code: u8,
    coordinate: u32,
    reference: DnaBase,
    alternatives: [InputAlternative; 3],
) -> InputLocus {
    InputLocus::Ordinary(OrdinaryInputLocus {
        gene,
        contig: contig(contig_code),
        position: position(coordinate),
        reference,
        alternatives,
    })
}

fn varied_alternatives(excluded: DnaBase, seed: u32) -> [InputAlternative; 3] {
    let mut values: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != excluded)
        .map(|alternate| {
            let score = match alternate {
                DnaBase::A => PangolinScore::new(
                    magnitude(10 + (seed % 7) as u16),
                    relative(-49),
                    magnitude(0),
                    relative(-50),
                ),
                DnaBase::C => PangolinScore::new(
                    magnitude(0),
                    relative(-50),
                    magnitude(20 + (seed % 7) as u16),
                    relative(49),
                ),
                DnaBase::G => {
                    PangolinScore::new(magnitude(0), relative(0), magnitude(0), relative(-50))
                }
                DnaBase::T => PangolinScore::new(
                    magnitude(30 + (seed % 7) as u16),
                    relative(-1),
                    magnitude(40 + (seed % 7) as u16),
                    relative(1),
                ),
            };
            InputAlternative { alternate, score }
        })
        .collect();
    values.rotate_left((seed as usize) % 3);
    values.try_into().expect("three varied alternatives")
}

fn alternatives(excluded: DnaBase, score: PangolinScore) -> [InputAlternative; 3] {
    let mut values: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != excluded)
        .map(|alternate| InputAlternative { alternate, score })
        .collect();
    values.reverse();
    values.try_into().expect("three alternatives")
}

fn default_score() -> PangolinScore {
    PangolinScore::new(magnitude(0), relative(-50), magnitude(0), relative(-50))
}

fn edge_score() -> PangolinScore {
    PangolinScore::new(magnitude(1), relative(-50), magnitude(100), relative(50))
}

fn magnitude(value: u16) -> ScoreMagnitude {
    ScoreMagnitude::new(value).expect("magnitude")
}
fn relative(value: i16) -> RelativePosition {
    RelativePosition::new(value).expect("relative")
}
fn position(value: u32) -> GenomicPosition {
    GenomicPosition::new(value).expect("position")
}
fn gene(value: u64) -> EnsemblGeneId {
    EnsemblGeneId::from_numeric(value).expect("gene")
}
fn contig(value: u8) -> Grch38Contig {
    Grch38Contig::from_code(value).expect("contig")
}

fn decode_base(value: u8) -> DnaBase {
    match value {
        0 => DnaBase::A,
        1 => DnaBase::C,
        2 => DnaBase::G,
        3 => DnaBase::T,
        _ => panic!("invalid base code {value}"),
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().expect("u16"))
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32"))
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("u64"))
}
