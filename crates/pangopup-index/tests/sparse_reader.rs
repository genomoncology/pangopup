use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, PangolinScore,
    RelativePosition, ScoreMagnitude,
};
use pangopup_index::{
    AmbiguousInputLocus, InputAlternative, InputLocus, OrdinaryInputLocus,
    sparse_reader::SparseIndexReader, sparse_writer::SparseIndexWriter,
};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-sparse-reader-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create temp directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temp directory");
    }
}

#[test]
fn lookup_and_stream_match_an_independent_oracle() {
    let temp = Temp::new("oracle");
    let genes = fixture();
    let output = write(&temp, &genes);
    let reader = SparseIndexReader::open(&output).expect("open reader");

    let mut actual = Vec::new();
    let summary = reader
        .visit_all(|locus| {
            actual.push(locus);
            Ok::<_, ()>(())
        })
        .expect("visit all");
    let expected: Vec<_> = genes.iter().flatten().copied().map(canonical).collect();
    assert_eq!(actual, expected);
    assert_eq!(summary.genes, 3);
    assert_eq!(summary.ordinary_loci, 6);
    assert_eq!(summary.exceptions, 2);
    assert_eq!(summary.loci, 8);

    let query = snv(1, 64, DnaBase::T, DnaBase::A);
    let (records, ambiguity) = reader.lookup_parts(query, None).expect("overlap lookup");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].gene(), gene(1));
    assert_eq!(records[0].score(), nondefault_score(10));
    assert_eq!(records[1].gene(), gene(2));
    assert_eq!(records[1].score(), nondefault_score(30));
    assert!(ambiguity.is_empty());

    let (records, _) = reader
        .lookup_parts(query, Some(gene(2)))
        .expect("filtered overlap lookup");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].gene(), gene(2));
    assert_eq!(records[0].score(), nondefault_score(30));

    let default = reader
        .lookup_parts(snv(1, 63, DnaBase::G, DnaBase::A), Some(gene(1)))
        .expect("default lookup")
        .0;
    assert_eq!(default.len(), 1);
    assert_eq!(default[0].score(), default_score());

    let mismatch = snv(1, 64, DnaBase::A, DnaBase::C);
    assert!(
        reader
            .lookup_parts(mismatch, None)
            .expect("reference mismatch")
            .0
            .is_empty()
    );
    let miss = snv(1, 99, DnaBase::A, DnaBase::C);
    assert_eq!(
        reader.lookup_parts(miss, None).expect("pure miss"),
        (Vec::new(), Vec::new())
    );

    let exception = snv(2, 77, DnaBase::C, DnaBase::G);
    let (_, ambiguities) = reader
        .lookup_parts(exception, None)
        .expect("exception lookup");
    assert_eq!(ambiguities.len(), 1);
    assert_eq!(ambiguities[0].gene(), gene(3));
    assert_eq!(ambiguities[0].omitted_alternate(), DnaBase::T);
    assert_eq!(
        ambiguities[0].published_alternates(),
        &[DnaBase::A, DnaBase::C, DnaBase::G]
    );
    let (_, ambiguities) = reader
        .lookup_parts(snv(1, 3, DnaBase::G, DnaBase::T), None)
        .expect("second exception shape");
    assert_eq!(ambiguities.len(), 1);
    assert_eq!(ambiguities[0].omitted_alternate(), DnaBase::A);
    assert_eq!(
        ambiguities[0].published_alternates(),
        &[DnaBase::C, DnaBase::G, DnaBase::T]
    );
    reader.verify_all().expect("full verification");
}

#[test]
fn exception_only_file_and_gene_stream_without_ordinary_blocks() {
    let temp = Temp::new("exception-only");
    let genes = vec![
        vec![exception(1, 1, 1, DnaBase::A)],
        vec![exception(2, 25, u32::MAX, DnaBase::T)],
    ];
    let output = write(&temp, &genes);
    let reader = SparseIndexReader::open(&output).expect("open exception-only file");
    assert_eq!(reader.segment_count(), 0);
    assert_eq!(reader.block_count(), 0);
    let mut actual = Vec::new();
    let summary = reader
        .visit_all(|locus| {
            actual.push(locus);
            Ok::<_, ()>(())
        })
        .expect("visit exception-only file");
    assert_eq!(
        actual,
        genes
            .into_iter()
            .flatten()
            .map(canonical)
            .collect::<Vec<_>>()
    );
    assert_eq!(summary.genes, 2);
    assert_eq!(summary.ordinary_loci, 0);
    assert_eq!(summary.exceptions, 2);
}

#[test]
fn untouched_pair_corruption_survives_open_and_unrelated_lookup_but_not_verify() {
    let temp = Temp::new("corruption-boundary");
    let score = nondefault_score(10);
    let genes = vec![vec![
        ordinary(1, 1, 1, DnaBase::A, score),
        ordinary(1, 1, 10, DnaBase::C, score),
    ]];
    let output = write(&temp, &genes);
    let mut bytes = fs::read(&output).expect("candidate bytes");
    let block_section = u64_at(&bytes, 56) as usize;
    let payload_section = u64_at(&bytes, 72) as usize;
    let second_block = block_section + 40;
    let raw = payload_section + u64_at(&bytes, second_block + 16) as usize;
    let count = u32_at(&bytes, second_block + 12) as usize;
    let active = u32_at(&bytes, second_block + 28) as usize;
    let values = raw
        + 16
        + (count * 2).div_ceil(8)
        + count.div_ceil(8)
        + count.div_ceil(64) * 8
        + (active * 6).div_ceil(8);
    bytes[values + 1] |= 0xc0;
    fs::write(&output, bytes).expect("write mutation");

    let reader = SparseIndexReader::open(&output).expect("metadata open survives payload mutation");
    reader
        .lookup_parts(snv(1, 1, DnaBase::A, DnaBase::C), None)
        .expect("unrelated lookup survives");
    assert!(reader.verify_all().is_err());
}

#[test]
fn lookup_validates_unrequested_pairs_at_the_addressed_locus() {
    let temp = Temp::new("addressed-corruption");
    let genes = vec![vec![ordinary(1, 1, 1, DnaBase::A, nondefault_score(10))]];
    let output = write(&temp, &genes);
    let mut bytes = fs::read(&output).expect("candidate bytes");
    let block = u64_at(&bytes, 56) as usize;
    let raw = u64_at(&bytes, 72) as usize + u64_at(&bytes, block + 16) as usize;
    let count = u32_at(&bytes, block + 12) as usize;
    let active = u32_at(&bytes, block + 28) as usize;
    let pairs = u32_at(&bytes, block + 32) as usize;
    let values = raw
        + 16
        + (count * 2).div_ceil(8)
        + count.div_ceil(8)
        + count.div_ceil(64) * 8
        + (active * 6).div_ceil(8);
    bytes[values + (pairs - 1) * 2 + 1] |= 0xc0;
    fs::write(&output, bytes).expect("write mutation");
    let reader = SparseIndexReader::open(&output).expect("metadata open");
    assert!(
        reader
            .lookup_parts(snv(1, 1, DnaBase::A, DnaBase::C), None)
            .is_err()
    );
}

#[cfg(feature = "test-read-audit")]
#[test]
fn selected_block_lookup_leaves_unrelated_pairs_unread_for_verify_all() {
    use pangopup_index::sparse_reader::{
        test_reset_sparse_payload_read_bytes, test_sparse_payload_byte_was_read,
    };

    let temp = Temp::new("selected-block-unread-pair");
    let genes = vec![vec![
        ordinary(1, 1, 1, DnaBase::A, nondefault_score(10)),
        ordinary(1, 1, 2, DnaBase::C, nondefault_score(20)),
    ]];
    let output = write(&temp, &genes);
    let mut bytes = fs::read(&output).expect("candidate bytes");
    let block = u64_at(&bytes, 56) as usize;
    let raw = u64_at(&bytes, 72) as usize + u64_at(&bytes, block + 16) as usize;
    let count = u32_at(&bytes, block + 12) as usize;
    let active = u32_at(&bytes, block + 28) as usize;
    let values = raw
        + 16
        + (count * 2).div_ceil(8)
        + count.div_ceil(8)
        + count.div_ceil(64) * 8
        + (active * 6).div_ceil(8);
    let second_locus_pair = values + 6 * 2;
    bytes[second_locus_pair + 1] |= 0xc0;
    fs::write(&output, bytes).expect("write unrelated pair corruption");

    test_reset_sparse_payload_read_bytes();
    let reader = SparseIndexReader::open(&output).expect("metadata open");
    reader
        .lookup_parts(snv(1, 1, DnaBase::A, DnaBase::C), None)
        .expect("selected block lookup");
    assert!(!test_sparse_payload_byte_was_read(second_locus_pair));
    assert!(!test_sparse_payload_byte_was_read(second_locus_pair + 1));
    assert!(reader.verify_all().is_err());
}

#[test]
fn lookup_and_traversal_cross_rank_and_block_boundaries() {
    let temp = Temp::new("boundaries");
    let loci: Vec<_> = (1..=4097)
        .map(|coordinate| {
            let reference = DnaBase::ALL[(coordinate as usize - 1) % 4];
            if matches!(coordinate, 63..=65 | 4095..=4097) {
                ordinary_with_alternatives(
                    1,
                    1,
                    coordinate,
                    reference,
                    boundary_alternatives(reference, coordinate),
                )
            } else {
                ordinary(1, 1, coordinate, reference, default_score())
            }
        })
        .collect();
    let genes = vec![loci];
    let expected: Vec<_> = genes.iter().flatten().copied().map(canonical).collect();
    let output = write(&temp, &genes);
    let reader = SparseIndexReader::open(&output).expect("open boundary reader");
    assert_eq!(reader.block_count(), 2);
    for coordinate in [63, 64, 65, 4095, 4096, 4097] {
        let reference = DnaBase::ALL[(coordinate as usize - 1) % 4];
        for expected_alternative in boundary_alternatives(reference, coordinate) {
            let (records, _) = reader
                .lookup_parts(
                    snv(1, coordinate, reference, expected_alternative.alternate),
                    Some(gene(1)),
                )
                .expect("boundary lookup");
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].gene(), gene(1));
            assert_eq!(records[0].score(), expected_alternative.score);
        }
    }
    let mut actual = Vec::new();
    let summary = reader
        .visit_all(|locus| {
            actual.push(locus);
            Ok::<_, ()>(())
        })
        .expect("boundary traversal");
    assert_eq!(actual, expected);
    assert_eq!(summary.ordinary_loci, 4097);
}

#[test]
fn open_rejects_metadata_mutations_truncation_and_checked_arithmetic_values() {
    let temp = Temp::new("metadata-mutations");
    let output = write(&temp, &fixture());
    let original = fs::read(&output).expect("candidate bytes");
    let sections = [24, 40, 56, 72, 88].map(|offset| u64_at(&original, offset) as usize);

    for offset in [
        0, 8, 12, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 148,
    ] {
        assert_open_rejects(&output, &original, |bytes| bytes[offset] ^= 1);
    }
    for offset in 152..256 {
        assert_open_rejects(&output, &original, |bytes| bytes[offset] = 1);
    }

    let gene = sections[0];
    for offset in [0, 8, 16, 20] {
        assert_open_rejects(&output, &original, |bytes| bytes[gene + offset] = 0xff);
    }
    for offset in 24..32 {
        assert_open_rejects(&output, &original, |bytes| bytes[gene + offset] = 1);
    }

    let segment = sections[1];
    for offset in [0, 8, 16, 20, 24, 28, 32] {
        assert_open_rejects(&output, &original, |bytes| bytes[segment + offset] = 0xff);
    }
    for offset in (9..16).chain(36..48) {
        assert_open_rejects(&output, &original, |bytes| bytes[segment + offset] = 1);
    }

    let block = sections[2];
    for offset in [0, 8, 12, 16, 24, 28, 32] {
        assert_open_rejects(&output, &original, |bytes| bytes[block + offset] = 0xff);
    }
    for offset in 36..40 {
        assert_open_rejects(&output, &original, |bytes| bytes[block + offset] = 1);
    }

    let exception = sections[4];
    for offset in [0, 1, 2, 3, 4] {
        assert_open_rejects(&output, &original, |bytes| bytes[exception + offset] = 0xff);
    }
    assert_open_rejects(&output, &original, |bytes| {
        bytes[exception + 8..exception + 16].fill(0);
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[exception + 16..exception + 20].fill(0);
    });
    for offset in [25, 27, 29, 31, 33, 35] {
        assert_open_rejects(&output, &original, |bytes| {
            bytes[exception + offset] |= 0xc0;
        });
    }
    for offset in (5..8).chain(20..24).chain(36..40) {
        assert_open_rejects(&output, &original, |bytes| bytes[exception + offset] = 1);
    }

    for length in [
        0,
        7,
        255,
        sections[0] + 31,
        sections[1] + 47,
        sections[2] + 39,
        sections[3],
        sections[4] + 39,
        original.len() - 1,
    ] {
        fs::write(&output, &original[..length]).expect("write truncation");
        assert!(SparseIndexReader::open(&output).is_err(), "length {length}");
    }
    fs::write(&output, &original).expect("restore candidate");
    assert_open_rejects(&output, &original, |bytes| {
        bytes[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[104..112].copy_from_slice(&u64::MAX.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[120..128].copy_from_slice(&u64::MAX.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[128..136].copy_from_slice(&u64::MAX.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[sections[1] + 16..sections[1] + 20].copy_from_slice(&u32::MAX.to_le_bytes());
    });
    fs::write(&output, original).expect("restore candidate");
}

#[test]
fn lookup_rejects_rank_array_and_tail_bit_mutations() {
    let temp = Temp::new("payload-arrays");
    let loci: Vec<_> = (1..=65)
        .map(|coordinate| ordinary(1, 1, coordinate, DnaBase::A, nondefault_score(10)))
        .collect();
    let output = write(&temp, &[loci]);
    let original = fs::read(&output).expect("candidate bytes");
    let block = u64_at(&original, 56) as usize;
    let raw = u64_at(&original, 72) as usize + u64_at(&original, block + 16) as usize;
    let count = u32_at(&original, block + 12) as usize;
    let active = u32_at(&original, block + 28) as usize;
    let references = raw + 16;
    let active_flags = references + (count * 2).div_ceil(8);
    let ranks = active_flags + count.div_ceil(8);
    let masks = ranks + count.div_ceil(64) * 8;
    let values = masks + (active * 6).div_ceil(8);
    let query = snv(1, 1, DnaBase::A, DnaBase::C);

    for mutate in [
        (raw, 1_u8),
        (raw + 4, 1),
        (raw + 8, 1),
        (raw + 12, 1_u8),
        (ranks, 1),
        (ranks + 4, 1),
        (ranks + 8, 1),
        (ranks + 12, 1),
        (active_flags, 0),
        (references + (count * 2).div_ceil(8) - 1, 0x80),
        (active_flags + count.div_ceil(8) - 1, 0x80),
        (masks + (active * 6).div_ceil(8) - 1, 0x80),
        (values + 1, 0xc0),
    ] {
        let mut bytes = original.clone();
        if mutate.1 == 0 {
            bytes[mutate.0] = 0;
        } else {
            bytes[mutate.0] ^= mutate.1;
        }
        fs::write(&output, bytes).expect("write payload mutation");
        let reader = SparseIndexReader::open(&output).expect("metadata open");
        assert!(
            reader.lookup_parts(query, None).is_err(),
            "mutation {mutate:?}"
        );
    }

    assert_lookup_rejects(&output, &original, query, |bytes| {
        bytes[masks] &= 0xc0;
    });
    assert_lookup_rejects(&output, &original, query, |bytes| {
        bytes[masks] &= !1;
    });
    assert_lookup_rejects(&output, &original, query, |bytes| {
        let value = u16_at(bytes, values);
        bytes[values..values + 2].copy_from_slice(&((value & !0x7f) | 101).to_le_bytes());
    });
    assert_lookup_rejects(&output, &original, query, |bytes| {
        let value = u16_at(bytes, values);
        bytes[values..values + 2].copy_from_slice(&((value & 0x7f) | (101 << 7)).to_le_bytes());
    });
    let reference_mismatch = snv(1, 1, DnaBase::C, DnaBase::G);
    assert_lookup_rejects(&output, &original, reference_mismatch, |bytes| {
        bytes[ranks] ^= 1;
    });
}

#[test]
fn open_rejects_overlap_and_exception_ownership_violations() {
    let temp = Temp::new("ownership-corruption");
    let output = write(
        &temp,
        &[vec![
            ordinary(1, 1, 1, DnaBase::A, default_score()),
            ordinary(1, 1, 2, DnaBase::C, default_score()),
            exception(1, 1, 3, DnaBase::A),
            ordinary(1, 1, 4, DnaBase::G, default_score()),
        ]],
    );
    let original = fs::read(&output).expect("candidate bytes");
    let segments = u64_at(&original, 40) as usize;
    let second_segment = segments + 48;
    let exceptions = u64_at(&original, 88) as usize;

    assert_open_rejects(&output, &original, |bytes| {
        bytes[second_segment + 16..second_segment + 20].copy_from_slice(&2_u32.to_le_bytes());
        bytes[second_segment + 20..second_segment + 24].copy_from_slice(&2_u32.to_le_bytes());
    });
    assert_open_rejects(&output, &original, |bytes| {
        bytes[exceptions + 16..exceptions + 20].copy_from_slice(&1_u32.to_le_bytes());
    });

    let exception_only = Temp::new("undeclared-exception-gene");
    let exception_output = write(&exception_only, &[vec![exception(1, 1, 3, DnaBase::A)]]);
    let exception_bytes = fs::read(&exception_output).expect("exception candidate");
    let exception_section = u64_at(&exception_bytes, 88) as usize;
    assert_open_rejects(&exception_output, &exception_bytes, |bytes| {
        bytes[exception_section + 8..exception_section + 16].copy_from_slice(&2_u64.to_le_bytes());
    });
}

#[test]
fn valid_score_mutation_changes_the_oracle_without_claiming_corruption() {
    let temp = Temp::new("valid-score-change");
    let genes = vec![vec![ordinary(1, 1, 1, DnaBase::A, nondefault_score(10))]];
    let output = write(&temp, &genes);
    let mut bytes = fs::read(&output).expect("candidate bytes");
    let block = u64_at(&bytes, 56) as usize;
    let raw = u64_at(&bytes, 72) as usize + u64_at(&bytes, block + 16) as usize;
    let count = u32_at(&bytes, block + 12) as usize;
    let active = u32_at(&bytes, block + 28) as usize;
    let values = raw
        + 16
        + (count * 2).div_ceil(8)
        + count.div_ceil(8)
        + count.div_ceil(64) * 8
        + (active * 6).div_ceil(8);
    bytes[values] += 1;
    fs::write(&output, bytes).expect("write valid score mutation");
    let reader = SparseIndexReader::open(&output).expect("valid metadata");
    reader.verify_all().expect("valid encoding");
    let mut actual = Vec::new();
    reader
        .visit_all(|locus| {
            actual.push(locus);
            Ok::<_, ()>(())
        })
        .expect("changed traversal");
    assert_ne!(actual, genes.into_iter().flatten().collect::<Vec<_>>());
}

#[cfg(feature = "test-read-audit")]
#[test]
fn open_reads_no_ordinary_payload_and_lookup_is_the_positive_control() {
    use pangopup_index::sparse_reader::{
        test_reset_sparse_payload_read_bytes, test_sparse_payload_read_bytes,
    };

    let temp = Temp::new("read-audit");
    let output = write(
        &temp,
        &[vec![ordinary(1, 1, 1, DnaBase::A, nondefault_score(10))]],
    );
    test_reset_sparse_payload_read_bytes();
    let reader = SparseIndexReader::open(&output).expect("open reader");
    assert_eq!(test_sparse_payload_read_bytes(), 0);
    reader
        .lookup_parts(snv(1, 1, DnaBase::A, DnaBase::C), None)
        .expect("lookup");
    assert!(test_sparse_payload_read_bytes() > 0);
}

fn fixture() -> Vec<Vec<InputLocus>> {
    vec![
        vec![
            ordinary(1, 1, 63, DnaBase::G, default_score()),
            ordinary(1, 1, 64, DnaBase::T, nondefault_score(10)),
            ordinary_with_alternatives(1, 1, 65, DnaBase::A, partial_alternatives()),
            ordinary(1, 2, 5, DnaBase::C, nondefault_score(20)),
        ],
        vec![
            ordinary(2, 1, 64, DnaBase::T, nondefault_score(30)),
            ordinary(2, 1, u32::MAX, DnaBase::A, default_score()),
        ],
        vec![
            exception(3, 1, 3, DnaBase::A),
            exception(3, 2, 77, DnaBase::T),
        ],
    ]
}

fn write(temp: &Temp, genes: &[Vec<InputLocus>]) -> PathBuf {
    let scratch = temp.0.join("payload.scratch");
    let output = temp.0.join("candidate.pgi");
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    for gene in genes {
        writer.push_gene(gene).expect("push gene");
    }
    writer.finish(&output).expect("finish writer");
    output
}

fn ordinary(
    gene_number: u64,
    contig_code: u8,
    coordinate: u32,
    reference: DnaBase,
    score: PangolinScore,
) -> InputLocus {
    ordinary_with_alternatives(
        gene_number,
        contig_code,
        coordinate,
        reference,
        alternatives(reference, score),
    )
}

fn ordinary_with_alternatives(
    gene_number: u64,
    contig_code: u8,
    coordinate: u32,
    reference: DnaBase,
    alternatives: [InputAlternative; 3],
) -> InputLocus {
    InputLocus::Ordinary(OrdinaryInputLocus {
        gene: gene(gene_number),
        contig: contig(contig_code),
        position: position(coordinate),
        reference,
        alternatives,
    })
}

fn exception(gene_number: u64, contig_code: u8, coordinate: u32, omitted: DnaBase) -> InputLocus {
    InputLocus::Ambiguous(AmbiguousInputLocus {
        gene: gene(gene_number),
        contig: contig(contig_code),
        position: position(coordinate),
        alternatives: alternatives(omitted, nondefault_score(40)),
        omitted,
    })
}

fn alternatives(excluded: DnaBase, score: PangolinScore) -> [InputAlternative; 3] {
    alternate_bases(excluded).map(|alternate| InputAlternative { alternate, score })
}

fn alternate_bases(excluded: DnaBase) -> [DnaBase; 3] {
    match excluded {
        DnaBase::A => [DnaBase::C, DnaBase::G, DnaBase::T],
        DnaBase::C => [DnaBase::A, DnaBase::G, DnaBase::T],
        DnaBase::G => [DnaBase::A, DnaBase::C, DnaBase::T],
        DnaBase::T => [DnaBase::A, DnaBase::C, DnaBase::G],
    }
}

fn boundary_alternatives(reference: DnaBase, coordinate: u32) -> [InputAlternative; 3] {
    let seed = (coordinate % 20) as u16;
    let [first, second, third] = alternate_bases(reference);
    [
        InputAlternative {
            alternate: first,
            score: score(10 + seed, -49, 0, -50),
        },
        InputAlternative {
            alternate: second,
            score: score(0, -50, 20 + seed, 49),
        },
        InputAlternative {
            alternate: third,
            score: score(0, 0, 30 + seed, 1),
        },
    ]
}

fn canonical(mut locus: InputLocus) -> InputLocus {
    match &mut locus {
        InputLocus::Ordinary(value) => value
            .alternatives
            .sort_by_key(|alternative| alternative.alternate),
        InputLocus::Ambiguous(value) => value
            .alternatives
            .sort_by_key(|alternative| alternative.alternate),
    }
    locus
}

fn default_score() -> PangolinScore {
    score(0, -50, 0, -50)
}

fn zero_with_positions() -> PangolinScore {
    score(0, 0, 0, 50)
}

fn partial_alternatives() -> [InputAlternative; 3] {
    [
        InputAlternative {
            alternate: DnaBase::C,
            score: zero_with_positions(),
        },
        InputAlternative {
            alternate: DnaBase::G,
            score: default_score(),
        },
        InputAlternative {
            alternate: DnaBase::T,
            score: score(11, -1, 0, -50),
        },
    ]
}

fn nondefault_score(seed: u16) -> PangolinScore {
    score(seed, -49, seed + 1, 49)
}

fn score(gain: u16, gain_position: i16, loss: u16, loss_position: i16) -> PangolinScore {
    PangolinScore::new(
        ScoreMagnitude::new(gain).expect("gain"),
        RelativePosition::new(gain_position).expect("gain position"),
        ScoreMagnitude::new(loss).expect("loss"),
        RelativePosition::new(loss_position).expect("loss position"),
    )
}

fn snv(contig_code: u8, coordinate: u32, reference: DnaBase, alternate: DnaBase) -> Grch38Snv {
    Grch38Snv::new(
        contig(contig_code),
        position(coordinate),
        reference,
        alternate,
    )
    .expect("SNV")
}

fn gene(value: u64) -> EnsemblGeneId {
    EnsemblGeneId::from_numeric(value).expect("gene")
}

fn contig(value: u8) -> Grch38Contig {
    Grch38Contig::from_code(value).expect("contig")
}

fn position(value: u32) -> GenomicPosition {
    GenomicPosition::new(value).expect("position")
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

fn assert_open_rejects(output: &PathBuf, original: &[u8], mutate: impl FnOnce(&mut [u8])) {
    let mut bytes = original.to_vec();
    mutate(&mut bytes);
    let changed: Vec<_> = bytes
        .iter()
        .zip(original)
        .enumerate()
        .filter_map(|(index, (actual, expected))| (actual != expected).then_some(index))
        .collect();
    fs::write(output, bytes).expect("write metadata mutation");
    assert!(
        SparseIndexReader::open(output).is_err(),
        "open accepted changed bytes {changed:?}"
    );
}

fn assert_lookup_rejects(
    output: &PathBuf,
    original: &[u8],
    query: Grch38Snv,
    mutate: impl FnOnce(&mut [u8]),
) {
    let mut bytes = original.to_vec();
    mutate(&mut bytes);
    fs::write(output, bytes).expect("write lookup mutation");
    let reader = SparseIndexReader::open(output).expect("metadata open");
    assert!(reader.lookup_parts(query, None).is_err());
}
