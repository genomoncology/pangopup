use flate2::read::GzDecoder;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::cmp::Reverse;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const SCORE_VALUES: usize = 101;
const POS_VALUES: usize = 101;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct Pair {
    score: u8,
    pos: i8,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct Record {
    gain: Pair,
    loss: Pair,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct Row {
    chrom: u8,
    pos: u32,
    reference: u8,
    alternate: u8,
    record: Record,
}

#[derive(Clone, Copy)]
struct EncodedLocus {
    reference: u8,
    alt_mask: u8,
    records: [Record; 3],
}

const BLOCK_LOCI: [usize; 3] = [256, 4096, 65_536];

struct BlockCompressor {
    limit: usize,
    loci: Vec<EncodedLocus>,
    blocks: u64,
    sparse_raw: u64,
    fixed_zstd: u64,
    sparse_zstd: u64,
    fixed_lz4: u64,
    sparse_lz4: u64,
}

impl BlockCompressor {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            loci: Vec::with_capacity(limit),
            blocks: 0,
            sparse_raw: 0,
            fixed_zstd: 0,
            sparse_zstd: 0,
            fixed_lz4: 0,
            sparse_lz4: 0,
        }
    }

    fn push(&mut self, locus: EncodedLocus) -> Result<(), String> {
        self.loci.push(locus);
        if self.loci.len() == self.limit {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        if self.loci.is_empty() {
            return Ok(());
        }
        let mut fixed = Vec::with_capacity(self.loci.len() * 11);
        for locus in &self.loci {
            fixed.extend_from_slice(&pack_locus(locus.reference, locus.records));
        }
        let sparse = encode_sparse(&self.loci);
        self.blocks += 1;
        self.sparse_raw += sparse.len() as u64;
        let fixed_zstd = zstd::bulk::compress(&fixed, 1)
            .map_err(|error| format!("zstd fixed compression: {error}"))?;
        let sparse_zstd = zstd::bulk::compress(&sparse, 1)
            .map_err(|error| format!("zstd sparse compression: {error}"))?;
        self.fixed_zstd += fixed_zstd.len().min(fixed.len()) as u64;
        self.sparse_zstd += sparse_zstd.len().min(sparse.len()) as u64;
        if self.limit == 4096 {
            let fixed_lz4 = lz4_flex::block::compress(&fixed);
            let sparse_lz4 = lz4_flex::block::compress(&sparse);
            self.fixed_lz4 += fixed_lz4.len().min(fixed.len()) as u64;
            self.sparse_lz4 += sparse_lz4.len().min(sparse.len()) as u64;
        }
        self.loci.clear();
        Ok(())
    }
}

struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl BitWriter {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            current: 0,
            used: 0,
        }
    }

    fn push(&mut self, value: u32, bits: u8) {
        for bit in 0..bits {
            self.current |= (((value >> bit) & 1) as u8) << self.used;
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

fn encode_sparse(loci: &[EncodedLocus]) -> Vec<u8> {
    let mut writer = BitWriter::new(loci.len() * 2);
    for locus in loci {
        writer.push(u32::from(locus.reference), 3);
    }
    for pair_index in 0..6 {
        for locus in loci {
            let pair = locus_pair(*locus, pair_index);
            writer.push(u32::from(pair != (Pair { score: 0, pos: -50 })), 1);
        }
        for locus in loci {
            let pair = locus_pair(*locus, pair_index);
            if pair != (Pair { score: 0, pos: -50 }) {
                writer.push(u32::from(pair.score), 7);
                writer.push((i16::from(pair.pos) + 50) as u32, 7);
            }
        }
    }
    writer.finish()
}

fn locus_pair(locus: EncodedLocus, pair_index: usize) -> Pair {
    let record = locus.records[pair_index / 2];
    if pair_index % 2 == 0 {
        record.gain
    } else {
        record.loss
    }
}

struct Stats {
    files: u64,
    rows: u64,
    loci: u64,
    raw_bytes: u64,
    compressed_bytes: u64,
    ascending_files: u64,
    descending_files: u64,
    one_locus_files: u64,
    malformed: u64,
    gap_transitions: u64,
    gap_bases: u64,
    max_gap_bases: u64,
    files_with_gaps: u64,
    gap_sizes: FxHashMap<u32, u64>,
    gap_files: Vec<String>,
    n_ref_files: Vec<String>,
    ref_counts: [u64; 5],
    alt_counts: [u64; 4],
    n_ref_missing_alt: [u64; 4],
    chrom_counts: [u64; 25],
    gain_scores: [u64; SCORE_VALUES],
    loss_scores: [u64; SCORE_VALUES],
    gain_positions: [u64; POS_VALUES],
    loss_positions: [u64; POS_VALUES],
    gain_pair: Vec<u64>,
    loss_pair: Vec<u64>,
    score_pair: Vec<u64>,
    record_counts: FxHashMap<u32, u64>,
    locus_counts: FxHashMap<u128, u64>,
    packed_byte_counts: [[u64; 256]; 11],
    default_pairs: u64,
    zero_score_nondefault_pos: u64,
    both_scores_zero: u64,
    nondefault_pairs_per_locus: [u64; 7],
    all_scores_zero_loci: u64,
    identical_alt_records: u64,
    same_previous_locus: u64,
    comparable_previous_loci: u64,
    same_previous_alt_records: u64,
    comparable_previous_alt_records: u64,
    lengths: Vec<u32>,
    spans: Vec<u32>,
    blocks: [u64; 3],
    sparse_block_raw: [u64; 3],
    fixed_zstd: [u64; 3],
    sparse_zstd: [u64; 3],
    fixed_lz4_4096: u64,
    sparse_lz4_4096: u64,
}

impl Stats {
    fn new() -> Self {
        Self {
            files: 0,
            rows: 0,
            loci: 0,
            raw_bytes: 0,
            compressed_bytes: 0,
            ascending_files: 0,
            descending_files: 0,
            one_locus_files: 0,
            malformed: 0,
            gap_transitions: 0,
            gap_bases: 0,
            max_gap_bases: 0,
            files_with_gaps: 0,
            gap_sizes: FxHashMap::default(),
            gap_files: Vec::new(),
            n_ref_files: Vec::new(),
            ref_counts: [0; 5],
            alt_counts: [0; 4],
            n_ref_missing_alt: [0; 4],
            chrom_counts: [0; 25],
            gain_scores: [0; SCORE_VALUES],
            loss_scores: [0; SCORE_VALUES],
            gain_positions: [0; POS_VALUES],
            loss_positions: [0; POS_VALUES],
            gain_pair: vec![0; SCORE_VALUES * POS_VALUES],
            loss_pair: vec![0; SCORE_VALUES * POS_VALUES],
            score_pair: vec![0; SCORE_VALUES * SCORE_VALUES],
            record_counts: FxHashMap::default(),
            locus_counts: FxHashMap::default(),
            packed_byte_counts: [[0; 256]; 11],
            default_pairs: 0,
            zero_score_nondefault_pos: 0,
            both_scores_zero: 0,
            nondefault_pairs_per_locus: [0; 7],
            all_scores_zero_loci: 0,
            identical_alt_records: 0,
            same_previous_locus: 0,
            comparable_previous_loci: 0,
            same_previous_alt_records: 0,
            comparable_previous_alt_records: 0,
            lengths: Vec::new(),
            spans: Vec::new(),
            blocks: [0; 3],
            sparse_block_raw: [0; 3],
            fixed_zstd: [0; 3],
            sparse_zstd: [0; 3],
            fixed_lz4_4096: 0,
            sparse_lz4_4096: 0,
        }
    }

    fn merge(mut self, other: Self) -> Self {
        self.files += other.files;
        self.rows += other.rows;
        self.loci += other.loci;
        self.raw_bytes += other.raw_bytes;
        self.compressed_bytes += other.compressed_bytes;
        self.ascending_files += other.ascending_files;
        self.descending_files += other.descending_files;
        self.one_locus_files += other.one_locus_files;
        self.malformed += other.malformed;
        self.gap_transitions += other.gap_transitions;
        self.gap_bases += other.gap_bases;
        self.max_gap_bases = self.max_gap_bases.max(other.max_gap_bases);
        self.files_with_gaps += other.files_with_gaps;
        for (key, count) in other.gap_sizes {
            *self.gap_sizes.entry(key).or_insert(0) += count;
        }
        self.gap_files.extend(other.gap_files);
        self.n_ref_files.extend(other.n_ref_files);
        merge_arrays(&mut self.ref_counts, &other.ref_counts);
        merge_arrays(&mut self.alt_counts, &other.alt_counts);
        merge_arrays(&mut self.n_ref_missing_alt, &other.n_ref_missing_alt);
        merge_arrays(&mut self.chrom_counts, &other.chrom_counts);
        merge_arrays(&mut self.gain_scores, &other.gain_scores);
        merge_arrays(&mut self.loss_scores, &other.loss_scores);
        merge_arrays(&mut self.gain_positions, &other.gain_positions);
        merge_arrays(&mut self.loss_positions, &other.loss_positions);
        merge_slices(&mut self.gain_pair, &other.gain_pair);
        merge_slices(&mut self.loss_pair, &other.loss_pair);
        merge_slices(&mut self.score_pair, &other.score_pair);
        for (key, count) in other.record_counts {
            *self.record_counts.entry(key).or_insert(0) += count;
        }
        for (key, count) in other.locus_counts {
            *self.locus_counts.entry(key).or_insert(0) += count;
        }
        for i in 0..11 {
            merge_arrays(
                &mut self.packed_byte_counts[i],
                &other.packed_byte_counts[i],
            );
        }
        self.default_pairs += other.default_pairs;
        self.zero_score_nondefault_pos += other.zero_score_nondefault_pos;
        self.both_scores_zero += other.both_scores_zero;
        merge_arrays(
            &mut self.nondefault_pairs_per_locus,
            &other.nondefault_pairs_per_locus,
        );
        self.all_scores_zero_loci += other.all_scores_zero_loci;
        self.identical_alt_records += other.identical_alt_records;
        self.same_previous_locus += other.same_previous_locus;
        self.comparable_previous_loci += other.comparable_previous_loci;
        self.same_previous_alt_records += other.same_previous_alt_records;
        self.comparable_previous_alt_records += other.comparable_previous_alt_records;
        self.lengths.extend(other.lengths);
        self.spans.extend(other.spans);
        merge_arrays(&mut self.blocks, &other.blocks);
        merge_arrays(&mut self.sparse_block_raw, &other.sparse_block_raw);
        merge_arrays(&mut self.fixed_zstd, &other.fixed_zstd);
        merge_arrays(&mut self.sparse_zstd, &other.sparse_zstd);
        self.fixed_lz4_4096 += other.fixed_lz4_4096;
        self.sparse_lz4_4096 += other.sparse_lz4_4096;
        self
    }
}

fn merge_arrays<const N: usize>(left: &mut [u64; N], right: &[u64; N]) {
    for (a, b) in left.iter_mut().zip(right) {
        *a += *b;
    }
}

fn merge_slices(left: &mut [u64], right: &[u64]) {
    for (a, b) in left.iter_mut().zip(right) {
        *a += *b;
    }
}

fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .ok_or_else(|| "usage: pangopup-entropy SOURCE_DIR".to_string())?;
    let mut paths: Vec<PathBuf> = fs::read_dir(&root)
        .map_err(|error| format!("read {root}: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "gz"))
        .collect();
    paths.sort();
    eprintln!(
        "scanning {} files with {} threads",
        paths.len(),
        rayon::current_num_threads()
    );

    let stats = paths
        .par_iter()
        .try_fold(Stats::new, |mut stats, path| {
            scan_file(&mut stats, path)?;
            Ok::<_, String>(stats)
        })
        .try_reduce(Stats::new, |left, right| Ok(left.merge(right)))?;
    report(stats);
    Ok(())
}

fn scan_file(stats: &mut Stats, path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    stats.compressed_bytes += file
        .metadata()
        .map_err(|error| format!("metadata {}: {error}", path.display()))?
        .len();
    let decoder = GzDecoder::new(file);
    let mut reader = BufReader::with_capacity(1024 * 1024, decoder);
    let mut line = Vec::with_capacity(96);
    let header_len = reader
        .read_until(b'\n', &mut line)
        .map_err(|error| format!("header {}: {error}", path.display()))?;
    stats.raw_bytes += header_len as u64;
    if trim_newline(&line) != b"chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos" {
        return Err(format!("bad header: {}", path.display()));
    }
    line.clear();
    let mut chunk = [Row::default(); 3];
    let mut chunk_len = 0usize;
    let mut first_pos = None;
    let mut previous_pos = None;
    let mut direction = 0i64;
    let mut previous_records: Option<[Record; 3]> = None;
    let mut previous_ref = 0u8;
    let mut file_loci = 0u32;
    let gaps_before = stats.gap_transitions;
    let n_before = stats.ref_counts[4];
    let do_compress = std::env::var_os("PANGOPUP_COMPRESS").is_some();
    let mut compressors = do_compress.then(|| BLOCK_LOCI.map(BlockCompressor::new));
    loop {
        let bytes = reader
            .read_until(b'\n', &mut line)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if bytes == 0 {
            break;
        }
        stats.raw_bytes += bytes as u64;
        let row = parse_row(trim_newline(&line))
            .map_err(|error| format!("{} row {}: {error}", path.display(), stats.rows + 1))?;
        chunk[chunk_len] = row;
        chunk_len += 1;
        stats.rows += 1;
        if chunk_len == 3 {
            let locus = process_locus(
                stats,
                &mut chunk,
                &mut first_pos,
                &mut previous_pos,
                &mut direction,
                &mut previous_records,
                &mut previous_ref,
            )
            .map_err(|error| format!("{} locus {}: {error}", path.display(), file_loci + 1))?;
            if let Some(compressors) = &mut compressors {
                for compressor in compressors {
                    compressor.push(locus)?;
                }
            }
            file_loci += 1;
            chunk_len = 0;
        }
        line.clear();
    }
    if chunk_len != 0 {
        return Err(format!("partial locus at end: {}", path.display()));
    }
    stats.files += 1;
    stats.lengths.push(file_loci);
    let span = match (first_pos, previous_pos) {
        (Some(first), Some(last)) => first.abs_diff(last) + 1,
        _ => 0,
    };
    stats.spans.push(span);
    stats.files_with_gaps += u64::from(stats.gap_transitions > gaps_before);
    if stats.gap_transitions > gaps_before {
        stats.gap_files.push(path.display().to_string());
    }
    if stats.ref_counts[4] > n_before {
        stats.n_ref_files.push(path.display().to_string());
    }
    if let Some(compressors) = &mut compressors {
        for (index, compressor) in compressors.iter_mut().enumerate() {
            compressor.flush()?;
            stats.blocks[index] += compressor.blocks;
            stats.sparse_block_raw[index] += compressor.sparse_raw;
            stats.fixed_zstd[index] += compressor.fixed_zstd;
            stats.sparse_zstd[index] += compressor.sparse_zstd;
            stats.fixed_lz4_4096 += compressor.fixed_lz4;
            stats.sparse_lz4_4096 += compressor.sparse_lz4;
        }
    }
    match direction {
        -1 => stats.descending_files += 1,
        0 => stats.one_locus_files += 1,
        1 => stats.ascending_files += 1,
        _ => return Err(format!("invalid direction: {}", path.display())),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_locus(
    stats: &mut Stats,
    chunk: &mut [Row; 3],
    first_pos: &mut Option<u32>,
    previous_pos: &mut Option<u32>,
    direction: &mut i64,
    previous_records: &mut Option<[Record; 3]>,
    previous_ref: &mut u8,
) -> Result<EncodedLocus, String> {
    let key = (chunk[0].chrom, chunk[0].pos, chunk[0].reference);
    if chunk
        .iter()
        .any(|row| (row.chrom, row.pos, row.reference) != key)
    {
        return Err("rows are not grouped in three-SNV loci".to_string());
    }
    let mut seen = [false; 4];
    for row in chunk.iter() {
        if row.alternate == row.reference || seen[row.alternate as usize] {
            return Err("invalid alternate set".to_string());
        }
        seen[row.alternate as usize] = true;
    }
    if chunk[0].reference < 4 {
        if seen
            .iter()
            .enumerate()
            .any(|(base, present)| *present != (base != chunk[0].reference as usize))
        {
            return Err("alternate set is not all non-reference bases".to_string());
        }
    } else {
        let missing = seen
            .iter()
            .position(|present| !present)
            .ok_or_else(|| "N-reference locus has no missing alternate".to_string())?;
        stats.n_ref_missing_alt[missing] += 1;
    }
    chunk.sort_unstable_by_key(|row| row.alternate);
    if let Some(previous) = *previous_pos {
        let delta = i64::from(chunk[0].pos) - i64::from(previous);
        let sign = delta.signum();
        if *direction == 0 {
            if sign == 0 {
                return Err("duplicate genomic position".to_string());
            }
            *direction = sign;
        } else if sign != *direction {
            return Err(format!("position direction changed with delta {delta}"));
        }
        let gap = delta.unsigned_abs().saturating_sub(1);
        if gap != 0 {
            stats.gap_transitions += 1;
            stats.gap_bases += gap;
            stats.max_gap_bases = stats.max_gap_bases.max(gap);
            *stats.gap_sizes.entry(gap as u32).or_insert(0) += 1;
        }
    } else {
        *first_pos = Some(chunk[0].pos);
    }
    *previous_pos = Some(chunk[0].pos);

    stats.loci += 1;
    stats.ref_counts[chunk[0].reference as usize] += 1;
    stats.chrom_counts[chunk[0].chrom as usize] += 1;
    let mut records = [Record::default(); 3];
    let mut record_index = 0usize;
    let mut nondefault = 0usize;
    let mut all_zero = true;
    for row in chunk.iter() {
        stats.alt_counts[row.alternate as usize] += 1;
        observe_record(stats, row.record);
        records[record_index] = row.record;
        record_index += 1;
        for pair in [row.record.gain, row.record.loss] {
            if pair.score == 0 && pair.pos == -50 {
                stats.default_pairs += 1;
            } else {
                nondefault += 1;
                if pair.score == 0 {
                    stats.zero_score_nondefault_pos += 1;
                }
            }
        }
        all_zero &= row.record.gain.score == 0 && row.record.loss.score == 0;
    }
    stats.nondefault_pairs_per_locus[nondefault] += 1;
    stats.all_scores_zero_loci += u64::from(all_zero);
    stats.identical_alt_records += u64::from(records[0] == records[1] && records[1] == records[2]);
    if let Some(previous) = previous_records {
        stats.comparable_previous_loci += 1;
        stats.same_previous_locus +=
            u64::from(*previous == records && *previous_ref == chunk[0].reference);
        stats.comparable_previous_alt_records += 3;
        stats.same_previous_alt_records += previous
            .iter()
            .zip(records)
            .filter(|(left, right)| **left == *right)
            .count() as u64;
    }
    *previous_records = Some(records);
    *previous_ref = chunk[0].reference;

    let alt_mask = seen.iter().enumerate().fold(0u8, |mask, (base, present)| {
        mask | (u8::from(*present) << base)
    });
    let locus = EncodedLocus {
        reference: chunk[0].reference,
        alt_mask,
        records,
    };
    *stats.locus_counts.entry(pack_locus_key(locus)).or_insert(0) += 1;

    let packed = pack_locus(chunk[0].reference, records);
    for (index, byte) in packed.iter().enumerate() {
        stats.packed_byte_counts[index][*byte as usize] += 1;
    }
    Ok(locus)
}

fn observe_record(stats: &mut Stats, record: Record) {
    let gain_score = record.gain.score as usize;
    let loss_score = record.loss.score as usize;
    let gain_pos = (i16::from(record.gain.pos) + 50) as usize;
    let loss_pos = (i16::from(record.loss.pos) + 50) as usize;
    stats.gain_scores[gain_score] += 1;
    stats.loss_scores[loss_score] += 1;
    stats.gain_positions[gain_pos] += 1;
    stats.loss_positions[loss_pos] += 1;
    stats.gain_pair[gain_score * POS_VALUES + gain_pos] += 1;
    stats.loss_pair[loss_score * POS_VALUES + loss_pos] += 1;
    stats.score_pair[gain_score * SCORE_VALUES + loss_score] += 1;
    stats.both_scores_zero += u64::from(gain_score == 0 && loss_score == 0);
    *stats.record_counts.entry(pack_record(record)).or_insert(0) += 1;
}

fn pack_record(record: Record) -> u32 {
    u32::from(record.gain.score)
        | (((i16::from(record.gain.pos) + 50) as u32) << 7)
        | (u32::from(record.loss.score) << 14)
        | (((i16::from(record.loss.pos) + 50) as u32) << 21)
}

fn pack_locus(reference: u8, records: [Record; 3]) -> [u8; 11] {
    let mut bits = u128::from(reference);
    for (index, record) in records.into_iter().enumerate() {
        bits |= u128::from(pack_record(record)) << (3 + 28 * index);
    }
    let raw = bits.to_le_bytes();
    let mut packed = [0u8; 11];
    packed.copy_from_slice(&raw[..11]);
    packed
}

fn pack_locus_key(locus: EncodedLocus) -> u128 {
    let mut bits = u128::from(locus.reference);
    for (index, record) in locus.records.into_iter().enumerate() {
        bits |= u128::from(pack_record(record)) << (3 + 28 * index);
    }
    bits | (u128::from(locus.alt_mask) << 96)
}

fn parse_row(line: &[u8]) -> Result<Row, String> {
    let mut fields = line.split(|byte| *byte == b'\t');
    let chrom = parse_chrom(next(&mut fields)?)?;
    let pos = parse_u32(next(&mut fields)?)?;
    let reference = parse_base(next(&mut fields)?)?;
    let alternate = parse_base(next(&mut fields)?)?;
    let (gain_negative, gain_score) = parse_centi(next(&mut fields)?)?;
    let gain_pos = parse_i8(next(&mut fields)?)?;
    let (loss_negative, loss_score) = parse_centi(next(&mut fields)?)?;
    let loss_pos = parse_i8(next(&mut fields)?)?;
    if fields.next().is_some() {
        return Err("extra field".to_string());
    }
    if gain_negative && gain_score != 0 {
        return Err("negative gain score".to_string());
    }
    if !loss_negative && loss_score != 0 {
        return Err("positive loss score".to_string());
    }
    if !(-50..=50).contains(&gain_pos) || !(-50..=50).contains(&loss_pos) {
        return Err("score position outside -50..50".to_string());
    }
    Ok(Row {
        chrom,
        pos,
        reference,
        alternate,
        record: Record {
            gain: Pair {
                score: gain_score,
                pos: gain_pos,
            },
            loss: Pair {
                score: loss_score,
                pos: loss_pos,
            },
        },
    })
}

fn next<'a>(fields: &mut impl Iterator<Item = &'a [u8]>) -> Result<&'a [u8], String> {
    fields.next().ok_or_else(|| "missing field".to_string())
}

fn parse_base(value: &[u8]) -> Result<u8, String> {
    match value {
        b"A" => Ok(0),
        b"C" => Ok(1),
        b"G" => Ok(2),
        b"T" => Ok(3),
        b"N" => Ok(4),
        _ => Err("invalid base".to_string()),
    }
}

fn parse_chrom(value: &[u8]) -> Result<u8, String> {
    let suffix = value
        .strip_prefix(b"chr")
        .ok_or_else(|| "chromosome lacks chr prefix".to_string())?;
    match suffix {
        b"X" => Ok(22),
        b"Y" => Ok(23),
        b"M" | b"MT" => Ok(24),
        _ => {
            let number = parse_u32(suffix)?;
            if (1..=22).contains(&number) {
                Ok((number - 1) as u8)
            } else {
                Err("unsupported chromosome".to_string())
            }
        }
    }
}

fn parse_u32(value: &[u8]) -> Result<u32, String> {
    if value.is_empty() {
        return Err("empty unsigned integer".to_string());
    }
    value.iter().try_fold(0u32, |acc, byte| {
        let digit = byte
            .checked_sub(b'0')
            .filter(|digit| *digit <= 9)
            .ok_or_else(|| "invalid unsigned integer".to_string())?;
        acc.checked_mul(10)
            .and_then(|number| number.checked_add(u32::from(digit)))
            .ok_or_else(|| "unsigned integer overflow".to_string())
    })
}

fn parse_i8(value: &[u8]) -> Result<i8, String> {
    let (negative, digits) = if let Some(rest) = value.strip_prefix(b"-") {
        (true, rest)
    } else {
        (false, value)
    };
    let number = parse_u32(digits)?;
    let signed = if negative {
        -(number as i32)
    } else {
        number as i32
    };
    i8::try_from(signed).map_err(|_| "signed integer overflow".to_string())
}

fn parse_centi(value: &[u8]) -> Result<(bool, u8), String> {
    let (negative, unsigned) = if let Some(rest) = value.strip_prefix(b"-") {
        (true, rest)
    } else {
        (false, value)
    };
    let mut parts = unsigned.split(|byte| *byte == b'.');
    let whole = parse_u32(parts.next().unwrap_or_default())?;
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some() || fraction.len() > 2 || !fraction.iter().all(u8::is_ascii_digit) {
        return Err("score is not an exact hundredth".to_string());
    }
    let fraction_value = match fraction {
        [] => 0,
        [one] => u32::from(one - b'0') * 10,
        [one, two] => u32::from(one - b'0') * 10 + u32::from(two - b'0'),
        _ => unreachable!(),
    };
    let centi = whole
        .checked_mul(100)
        .and_then(|number| number.checked_add(fraction_value))
        .ok_or_else(|| "score overflow".to_string())?;
    if centi > 100 {
        return Err("score outside magnitude 0..1".to_string());
    }
    Ok((negative, centi as u8))
}

fn trim_newline(mut value: &[u8]) -> &[u8] {
    if value.ends_with(b"\n") {
        value = &value[..value.len() - 1];
    }
    if value.ends_with(b"\r") {
        value = &value[..value.len() - 1];
    }
    value
}

fn entropy(counts: impl IntoIterator<Item = u64>) -> f64 {
    let counts: Vec<u64> = counts.into_iter().filter(|count| *count != 0).collect();
    let total = counts.iter().sum::<u64>() as f64;
    counts
        .into_iter()
        .map(|count| {
            let probability = count as f64 / total;
            -probability * probability.log2()
        })
        .sum()
}

fn top<const N: usize>(counts: &[u64; N], offset: i32, limit: usize) -> String {
    let mut values: Vec<(usize, u64)> = counts.iter().copied().enumerate().collect();
    values.sort_unstable_by_key(|(_, count)| Reverse(*count));
    values
        .into_iter()
        .take(limit)
        .map(|(value, count)| format!("{}:{count}", value as i32 + offset))
        .collect::<Vec<_>>()
        .join(",")
}

fn percentile(sorted: &[u32], numerator: usize, denominator: usize) -> u32 {
    let index = (sorted.len() - 1) * numerator / denominator;
    sorted[index]
}

fn report(mut stats: Stats) {
    stats.lengths.sort_unstable();
    stats.spans.sort_unstable();
    let record_entropy = entropy(stats.record_counts.values().copied());
    let locus_entropy = entropy(stats.locus_counts.values().copied());
    let ref_entropy = entropy(stats.ref_counts);
    let gain_pair_entropy = entropy(stats.gain_pair.iter().copied());
    let loss_pair_entropy = entropy(stats.loss_pair.iter().copied());
    let score_pair_entropy = entropy(stats.score_pair.iter().copied());
    let packed_byte_entropy: f64 = stats
        .packed_byte_counts
        .iter()
        .map(|counts| entropy(*counts))
        .sum();
    let zero_order_bytes =
        (record_entropy * stats.rows as f64 + ref_entropy * stats.loci as f64) / 8.0;
    let fixed_11 = stats.loci * 11;
    let sparse_byte = stats.loci + 2 * (6 * stats.loci - stats.default_pairs);
    let sparse_bit = (8 * stats.loci + 14 * (6 * stats.loci - stats.default_pairs) + 7) / 8;
    println!("files={}", stats.files);
    println!("rows={}", stats.rows);
    println!("loci={}", stats.loci);
    println!("raw_tsv_bytes={}", stats.raw_bytes);
    println!("source_gzip_bytes={}", stats.compressed_bytes);
    println!(
        "direction=ascending:{} descending:{} one_locus:{}",
        stats.ascending_files, stats.descending_files, stats.one_locus_files
    );
    println!("malformed={}", stats.malformed);
    println!(
        "gaps=files:{} transitions:{} omitted_bases:{} max_omitted_run:{} unique_gap_sizes:{}",
        stats.files_with_gaps,
        stats.gap_transitions,
        stats.gap_bases,
        stats.max_gap_bases,
        stats.gap_sizes.len()
    );
    println!("gap_files={:?}", stats.gap_files);
    println!("n_ref_files={:?}", stats.n_ref_files);
    println!(
        "gene_loci=min:{} p25:{} p50:{} p75:{} p90:{} p95:{} p99:{} max:{}",
        stats.lengths[0],
        percentile(&stats.lengths, 1, 4),
        percentile(&stats.lengths, 1, 2),
        percentile(&stats.lengths, 3, 4),
        percentile(&stats.lengths, 9, 10),
        percentile(&stats.lengths, 19, 20),
        percentile(&stats.lengths, 99, 100),
        stats.lengths[stats.lengths.len() - 1]
    );
    println!(
        "gene_span=min:{} p25:{} p50:{} p75:{} p90:{} p95:{} p99:{} max:{}",
        stats.spans[0],
        percentile(&stats.spans, 1, 4),
        percentile(&stats.spans, 1, 2),
        percentile(&stats.spans, 3, 4),
        percentile(&stats.spans, 9, 10),
        percentile(&stats.spans, 19, 20),
        percentile(&stats.spans, 99, 100),
        stats.spans[stats.spans.len() - 1]
    );
    println!(
        "ref_counts=A:{} C:{} G:{} T:{} N:{}",
        stats.ref_counts[0],
        stats.ref_counts[1],
        stats.ref_counts[2],
        stats.ref_counts[3],
        stats.ref_counts[4]
    );
    println!(
        "alt_counts=A:{} C:{} G:{} T:{}",
        stats.alt_counts[0], stats.alt_counts[1], stats.alt_counts[2], stats.alt_counts[3]
    );
    println!(
        "n_ref_missing_alt=A:{} C:{} G:{} T:{}",
        stats.n_ref_missing_alt[0],
        stats.n_ref_missing_alt[1],
        stats.n_ref_missing_alt[2],
        stats.n_ref_missing_alt[3]
    );
    println!(
        "gain_zero={} loss_zero={} both_zero_records={}",
        stats.gain_scores[0], stats.loss_scores[0], stats.both_scores_zero
    );
    println!(
        "default_pairs={} zero_score_nondefault_pos={}",
        stats.default_pairs, stats.zero_score_nondefault_pos
    );
    println!(
        "all_scores_zero_loci={} identical_alt_records={}",
        stats.all_scores_zero_loci, stats.identical_alt_records
    );
    println!(
        "same_previous_locus={}/{} same_previous_alt_record={}/{}",
        stats.same_previous_locus,
        stats.comparable_previous_loci,
        stats.same_previous_alt_records,
        stats.comparable_previous_alt_records
    );
    println!(
        "nondefault_pairs_per_locus={:?}",
        stats.nondefault_pairs_per_locus
    );
    println!("gain_score_top={}", top(&stats.gain_scores, 0, 12));
    println!("loss_magnitude_top={}", top(&stats.loss_scores, 0, 12));
    println!("gain_position_top={}", top(&stats.gain_positions, -50, 12));
    println!("loss_position_top={}", top(&stats.loss_positions, -50, 12));
    println!(
        "entropy_bits=ref:{ref_entropy:.6} gain_score:{:.6} loss_mag:{:.6} gain_pos:{:.6} loss_pos:{:.6} gain_pair:{gain_pair_entropy:.6} loss_pair:{loss_pair_entropy:.6} score_pair:{score_pair_entropy:.6} full_record:{record_entropy:.6} packed_byte_positions_sum:{packed_byte_entropy:.6}",
        entropy(stats.gain_scores),
        entropy(stats.loss_scores),
        entropy(stats.gain_positions),
        entropy(stats.loss_positions)
    );
    println!("unique_full_records={}", stats.record_counts.len());
    println!(
        "locus_entropy_bits={locus_entropy:.6} unique_locus_patterns={} locus_zero_order_bytes={:.0}",
        stats.locus_counts.len(),
        locus_entropy * stats.loci as f64 / 8.0
    );
    let mut record_frequencies: Vec<(u32, u64)> = stats.record_counts.into_iter().collect();
    record_frequencies.sort_unstable_by_key(|(_, count)| Reverse(*count));
    for limit in [1usize, 2, 4, 16, 256, 4096, 65_536] {
        let covered: u64 = record_frequencies
            .iter()
            .take(limit)
            .map(|(_, count)| *count)
            .sum();
        println!("top_records_{limit}={covered}");
    }
    println!(
        "size_bytes=fixed_11:{fixed_11} sparse_byte:{sparse_byte} sparse_bit:{sparse_bit} zero_order_record_plus_ref:{zero_order_bytes:.0}"
    );
    println!(
        "ratios_to_source_gzip=fixed_11:{:.6} sparse_byte:{:.6} sparse_bit:{:.6} zero_order:{:.6}",
        fixed_11 as f64 / stats.compressed_bytes as f64,
        sparse_byte as f64 / stats.compressed_bytes as f64,
        sparse_bit as f64 / stats.compressed_bytes as f64,
        zero_order_bytes / stats.compressed_bytes as f64
    );
    for index in 0..3 {
        let directory = stats.blocks[index] * 8;
        println!(
            "block_{}_bytes=blocks:{} sparse_raw:{} fixed_zstd1:{} sparse_zstd1:{} with_u64_dir_fixed:{} with_u64_dir_sparse:{}",
            BLOCK_LOCI[index],
            stats.blocks[index],
            stats.sparse_block_raw[index],
            stats.fixed_zstd[index],
            stats.sparse_zstd[index],
            stats.fixed_zstd[index] + directory,
            stats.sparse_zstd[index] + directory
        );
    }
    let directory_4096 = stats.blocks[1] * 8;
    println!(
        "block_4096_lz4_bytes=fixed:{} sparse:{} with_u64_dir_fixed:{} with_u64_dir_sparse:{}",
        stats.fixed_lz4_4096,
        stats.sparse_lz4_4096,
        stats.fixed_lz4_4096 + directory_4096,
        stats.sparse_lz4_4096 + directory_4096
    );
}
