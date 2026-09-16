//! Candidate-only deterministic sparse SNV writer.
//!
//! This module produces bytes for qualification. It deliberately exposes no
//! reader, lookup, bundle, profile, or routing integration.

use crate::{AmbiguousInputLocus, IndexError, InputAlternative, InputLocus};
use pangopup_core::{DnaBase, EnsemblGeneId, Grch38Contig, PangolinScore};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Exact identity of the candidate bytes produced by this module.
pub const SPARSE_INDEX_FORMAT: &str = "pangopup.sparse-direct.v1";

const MAGIC: &[u8; 8] = b"PGSPRS01";
const VERSION: u32 = 1;
const HEADER_BYTES: u64 = 256;
const GENE_BYTES: u64 = 32;
const SEGMENT_BYTES: u64 = 48;
const BLOCK_BYTES: u64 = 40;
const EXCEPTION_BYTES: u64 = 40;
const BLOCK_LOCI: usize = 4096;
const RANK_STRIDE: usize = 64;
const BLOCK_HEADER_BYTES: usize = 16;
const RANK_BYTES: usize = 8;
const DEFAULT_POSITION: i16 = -50;
static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseWriteSummary {
    pub bytes: u64,
    pub genes: u64,
    pub loci: u64,
    pub ordinary_loci: u64,
    pub segments: u64,
    pub blocks: u64,
    pub exceptions: u64,
}

#[derive(Clone, Copy)]
struct GeneEntry {
    gene: EnsemblGeneId,
    segment_start: u64,
    segment_count: u32,
    ordinary_loci: u32,
}

#[derive(Clone, Copy)]
struct SegmentEntry {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
    end: u32,
    loci: u32,
    block_start: u32,
    block_count: u32,
}

#[derive(Clone, Copy)]
struct BlockEntry {
    segment: u64,
    first: u32,
    count: u32,
    payload_offset: u64,
    payload_len: u32,
    active_count: u32,
    pair_count: u32,
}

struct PendingSegment {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
    end: u32,
    loci: u32,
    block_start: u32,
}

/// A bounded-memory writer for one ordered stream of complete genes.
///
/// `scratch_path` and its derived exception scratch must not exist. The writer
/// owns both paths after creation and removes them on success, error, or drop.
pub struct SparseIndexWriter {
    payload_path: PathBuf,
    exception_path: PathBuf,
    payload_identity: FileIdentity,
    exception_identity: FileIdentity,
    payload_owned: bool,
    exception_owned: bool,
    payload: Option<File>,
    exceptions_file: Option<File>,
    payload_len: u64,
    genes: Vec<GeneEntry>,
    segments: Vec<SegmentEntry>,
    blocks: Vec<BlockEntry>,
    loci: u64,
    ordinary_loci: u64,
    exceptions: u64,
    previous_gene: Option<u64>,
    poisoned: bool,
    #[cfg(test)]
    fail_spool_after: Option<u64>,
    #[cfg(test)]
    fail_assembly_after: Option<u64>,
    #[cfg(test)]
    fail_cleanup_once: bool,
}

impl SparseIndexWriter {
    pub fn create(scratch_path: &Path) -> Result<Self, IndexError> {
        let payload = create_new(scratch_path)?;
        let payload_identity = file_identity(&payload)?;
        let exception_path = exception_scratch_path(scratch_path);
        let exceptions_file = match create_new(&exception_path) {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_file(scratch_path);
                return Err(error);
            }
        };
        let exception_identity = file_identity(&exceptions_file)?;
        Ok(Self {
            payload_path: scratch_path.to_owned(),
            exception_path,
            payload_identity,
            exception_identity,
            payload_owned: true,
            exception_owned: true,
            payload: Some(payload),
            exceptions_file: Some(exceptions_file),
            payload_len: 0,
            genes: Vec::new(),
            segments: Vec::new(),
            blocks: Vec::new(),
            loci: 0,
            ordinary_loci: 0,
            exceptions: 0,
            previous_gene: None,
            poisoned: false,
            #[cfg(test)]
            fail_spool_after: None,
            #[cfg(test)]
            fail_assembly_after: None,
            #[cfg(test)]
            fail_cleanup_once: false,
        })
    }

    pub fn scratch_bytes(&self) -> Result<u64, IndexError> {
        checked_add(
            self.payload_len,
            checked_mul(
                self.exceptions,
                EXCEPTION_BYTES,
                "sparse scratch exception length",
            )?,
            "sparse scratch length",
        )
    }

    pub fn push_gene(&mut self, input: &[InputLocus]) -> Result<(), IndexError> {
        if self.poisoned {
            return Err(IndexError::InvalidInput("poisoned sparse writer"));
        }
        match self.push_gene_inner(input) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn push_gene_inner(&mut self, input: &[InputLocus]) -> Result<(), IndexError> {
        let first = input
            .first()
            .ok_or(IndexError::InvalidInput("empty sparse gene"))?;
        let gene = locus_gene(first);
        if self
            .previous_gene
            .is_some_and(|previous| previous >= gene.numeric())
        {
            return Err(IndexError::InvalidInput("sparse gene order"));
        }
        if input.iter().any(|locus| locus_gene(locus) != gene) {
            return Err(IndexError::InvalidInput("mixed sparse gene"));
        }

        let mut previous_coordinate = None;
        for locus in input {
            let coordinate = locus_coordinate(locus);
            if previous_coordinate.is_some_and(|previous| previous >= coordinate) {
                return Err(IndexError::InvalidInput("sparse coordinate order"));
            }
            validate_locus(locus)?;
            previous_coordinate = Some(coordinate);
        }

        let segment_start = usize_u64(self.segments.len(), "sparse gene segment start")?;
        let mut ordinary_count = 0_u32;
        let mut block_loci = Vec::with_capacity(BLOCK_LOCI);
        let mut pending_segment: Option<PendingSegment> = None;

        for locus in input {
            match *locus {
                InputLocus::Ordinary(mut ordinary) => {
                    ordinary
                        .alternatives
                        .sort_by_key(|value| base_code(value.alternate));
                    let starts_new = pending_segment.as_ref().is_some_and(|segment| {
                        segment.contig != ordinary.contig
                            || segment.end.checked_add(1) != Some(ordinary.position.get())
                    });
                    if starts_new {
                        self.finish_pending_segment(
                            pending_segment
                                .take()
                                .ok_or(IndexError::Arithmetic("sparse pending segment"))?,
                            &mut block_loci,
                        )?;
                    }
                    if pending_segment.is_none() {
                        pending_segment = Some(PendingSegment {
                            gene,
                            contig: ordinary.contig,
                            start: ordinary.position.get(),
                            end: ordinary.position.get(),
                            loci: 0,
                            block_start: u32::try_from(self.blocks.len())
                                .map_err(|_| IndexError::Arithmetic("sparse block start"))?,
                        });
                    }
                    let segment = pending_segment
                        .as_mut()
                        .ok_or(IndexError::Arithmetic("sparse pending segment"))?;
                    segment.end = ordinary.position.get();
                    segment.loci = segment
                        .loci
                        .checked_add(1)
                        .ok_or(IndexError::Arithmetic("sparse segment loci"))?;
                    block_loci.push(ordinary);
                    if block_loci.len() == BLOCK_LOCI {
                        self.write_pending_block(segment, &block_loci)?;
                        block_loci.clear();
                    }
                    ordinary_count = ordinary_count
                        .checked_add(1)
                        .ok_or(IndexError::Arithmetic("sparse gene ordinary count"))?;
                }
                InputLocus::Ambiguous(mut ambiguous) => {
                    ambiguous
                        .alternatives
                        .sort_by_key(|value| base_code(value.alternate));
                    let bytes = encode_exception(&ambiguous)?;
                    self.write_exception(&bytes)?;
                    self.exceptions = checked_add(self.exceptions, 1, "sparse exception count")?;
                }
            }
        }
        if let Some(segment) = pending_segment.take() {
            self.finish_pending_segment(segment, &mut block_loci)?;
        }
        let segment_count_u64 = usize_u64(self.segments.len(), "sparse segment count")?
            .checked_sub(segment_start)
            .ok_or(IndexError::Arithmetic("sparse gene segment count"))?;
        let segment_count = u32::try_from(segment_count_u64)
            .map_err(|_| IndexError::Arithmetic("sparse gene segment count"))?;
        self.genes.push(GeneEntry {
            gene,
            segment_start,
            segment_count,
            ordinary_loci: ordinary_count,
        });
        self.loci = checked_add(
            self.loci,
            usize_u64(input.len(), "sparse gene locus count")?,
            "sparse locus count",
        )?;
        self.ordinary_loci = checked_add(
            self.ordinary_loci,
            u64::from(ordinary_count),
            "sparse ordinary locus count",
        )?;
        self.previous_gene = Some(gene.numeric());
        Ok(())
    }

    fn write_pending_block(
        &mut self,
        segment: &PendingSegment,
        loci: &[crate::OrdinaryInputLocus],
    ) -> Result<(), IndexError> {
        let segment_index = usize_u64(self.segments.len(), "sparse segment index")?;
        let encoded = encode_block(loci)?;
        let payload_offset = self.payload_len;
        self.write_payload(&encoded.bytes)?;
        let payload_len = u32::try_from(encoded.bytes.len())
            .map_err(|_| IndexError::Arithmetic("sparse block payload length"))?;
        let block_loci =
            u32::try_from(loci.len()).map_err(|_| IndexError::Arithmetic("sparse block count"))?;
        let prior_block_loci = segment
            .loci
            .checked_sub(block_loci)
            .ok_or(IndexError::Arithmetic("sparse block first"))?;
        self.blocks.push(BlockEntry {
            segment: segment_index,
            first: prior_block_loci,
            count: block_loci,
            payload_offset,
            payload_len,
            active_count: encoded.active_count,
            pair_count: encoded.pair_count,
        });
        self.payload_len = checked_add(
            self.payload_len,
            u64::from(payload_len),
            "sparse payload length",
        )?;
        Ok(())
    }

    fn finish_pending_segment(
        &mut self,
        segment: PendingSegment,
        block_loci: &mut Vec<crate::OrdinaryInputLocus>,
    ) -> Result<(), IndexError> {
        if !block_loci.is_empty() {
            self.write_pending_block(&segment, block_loci)?;
            block_loci.clear();
        }
        let block_count = u32::try_from(self.blocks.len())
            .map_err(|_| IndexError::Arithmetic("sparse block count"))?
            .checked_sub(segment.block_start)
            .ok_or(IndexError::Arithmetic("sparse block count"))?;
        self.segments.push(SegmentEntry {
            gene: segment.gene,
            contig: segment.contig,
            start: segment.start,
            end: segment.end,
            loci: segment.loci,
            block_start: segment.block_start,
            block_count,
        });
        Ok(())
    }

    fn write_payload(&mut self, bytes: &[u8]) -> Result<(), IndexError> {
        #[cfg(test)]
        if let Some(remaining) = self.fail_spool_after.as_mut() {
            return injected_write(
                self.payload
                    .as_mut()
                    .ok_or(IndexError::InvalidInput("closed sparse writer"))?,
                bytes,
                remaining,
            );
        }
        self.payload
            .as_mut()
            .ok_or(IndexError::InvalidInput("closed sparse writer"))?
            .write_all(bytes)?;
        Ok(())
    }

    fn write_exception(&mut self, bytes: &[u8]) -> Result<(), IndexError> {
        #[cfg(test)]
        if let Some(remaining) = self.fail_spool_after.as_mut() {
            return injected_write(
                self.exceptions_file
                    .as_mut()
                    .ok_or(IndexError::InvalidInput("closed sparse writer"))?,
                bytes,
                remaining,
            );
        }
        self.exceptions_file
            .as_mut()
            .ok_or(IndexError::InvalidInput("closed sparse writer"))?
            .write_all(bytes)?;
        Ok(())
    }

    pub fn finish(mut self, output: &Path) -> Result<SparseWriteSummary, IndexError> {
        if self.poisoned {
            return Err(IndexError::InvalidInput("poisoned sparse writer"));
        }
        if self.genes.is_empty() {
            self.poisoned = true;
            return Err(IndexError::InvalidInput("empty sparse stream"));
        }
        let result = self.finish_inner(output);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn finish_inner(&mut self, output: &Path) -> Result<SparseWriteSummary, IndexError> {
        let mut payload = self
            .payload
            .take()
            .ok_or(IndexError::InvalidInput("closed sparse writer"))?;
        let mut exceptions = self
            .exceptions_file
            .take()
            .ok_or(IndexError::InvalidInput("closed sparse writer"))?;
        payload.sync_all()?;
        exceptions.sync_all()?;
        payload.seek(SeekFrom::Start(0))?;
        exceptions.seek(SeekFrom::Start(0))?;

        let gene_count = usize_u64(self.genes.len(), "sparse gene count")?;
        let segment_count = usize_u64(self.segments.len(), "sparse segment count")?;
        let block_count = usize_u64(self.blocks.len(), "sparse block count")?;
        let gene_len = checked_mul(gene_count, GENE_BYTES, "sparse gene section length")?;
        let segment_len = checked_mul(
            segment_count,
            SEGMENT_BYTES,
            "sparse segment section length",
        )?;
        let block_len = checked_mul(block_count, BLOCK_BYTES, "sparse block section length")?;
        let exception_len = checked_mul(
            self.exceptions,
            EXCEPTION_BYTES,
            "sparse exception section length",
        )?;
        let gene_offset = HEADER_BYTES;
        let segment_offset = checked_add(gene_offset, gene_len, "sparse segment offset")?;
        let block_offset = checked_add(segment_offset, segment_len, "sparse block offset")?;
        let payload_offset = checked_add(block_offset, block_len, "sparse payload offset")?;
        let exception_offset =
            checked_add(payload_offset, self.payload_len, "sparse exception offset")?;
        let file_len = checked_add(exception_offset, exception_len, "sparse file length")?;

        let (stage_path, stage_identity, mut stage) = create_stage(output)?;
        let assembled = (|| -> Result<(), IndexError> {
            let header = encode_header(HeaderValues {
                file_len,
                gene_offset,
                gene_len,
                segment_offset,
                segment_len,
                block_offset,
                block_len,
                payload_offset,
                payload_len: self.payload_len,
                exception_offset,
                exception_len,
                gene_count,
                segment_count,
                block_count,
                ordinary_loci: self.ordinary_loci,
                exception_count: self.exceptions,
            })?;
            self.write_final(&mut stage, &header)?;
            for index in 0..self.genes.len() {
                let bytes = encode_gene(self.genes[index])?;
                self.write_final(&mut stage, &bytes)?;
            }
            for index in 0..self.segments.len() {
                let bytes = encode_segment(self.segments[index])?;
                self.write_final(&mut stage, &bytes)?;
            }
            for index in 0..self.blocks.len() {
                let bytes = encode_block_entry(self.blocks[index])?;
                self.write_final(&mut stage, &bytes)?;
            }
            self.copy_final(&mut stage, &mut payload)?;
            self.copy_final(&mut stage, &mut exceptions)?;
            stage.sync_all()?;
            if stage.metadata()?.len() != file_len {
                return Err(IndexError::Arithmetic("sparse assembled file length"));
            }
            drop(payload);
            drop(exceptions);
            self.cleanup_scratch()?;
            publish_noreplace(&stage_path, stage_identity, output)?;
            Ok(())
        })();
        drop(stage);
        if assembled.is_err() {
            remove_if_owned(&stage_path, stage_identity);
        }
        assembled?;
        Ok(SparseWriteSummary {
            bytes: file_len,
            genes: gene_count,
            loci: self.loci,
            ordinary_loci: self.ordinary_loci,
            segments: segment_count,
            blocks: block_count,
            exceptions: self.exceptions,
        })
    }

    fn write_final(&mut self, output: &mut File, bytes: &[u8]) -> Result<(), IndexError> {
        #[cfg(test)]
        if let Some(remaining) = self.fail_assembly_after.as_mut() {
            return injected_write(output, bytes, remaining);
        }
        output.write_all(bytes)?;
        Ok(())
    }

    fn copy_final(&mut self, output: &mut File, source: &mut File) -> Result<(), IndexError> {
        let mut input = BufReader::new(source);
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            self.write_final(output, &buffer[..count])?;
        }
    }

    fn cleanup_scratch(&mut self) -> Result<(), IndexError> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_cleanup_once) {
            return Err(IndexError::Io(io::Error::other(
                "injected sparse cleanup failure",
            )));
        }
        remove_owned(
            &self.payload_path,
            self.payload_identity,
            &mut self.payload_owned,
        )?;
        remove_owned(
            &self.exception_path,
            self.exception_identity,
            &mut self.exception_owned,
        )?;
        Ok(())
    }
}

impl Drop for SparseIndexWriter {
    fn drop(&mut self) {
        self.payload.take();
        self.exceptions_file.take();
        let _ = remove_owned(
            &self.payload_path,
            self.payload_identity,
            &mut self.payload_owned,
        );
        let _ = remove_owned(
            &self.exception_path,
            self.exception_identity,
            &mut self.exception_owned,
        );
    }
}

struct EncodedBlock {
    bytes: Vec<u8>,
    active_count: u32,
    pair_count: u32,
}

fn encode_block(loci: &[crate::OrdinaryInputLocus]) -> Result<EncodedBlock, IndexError> {
    if loci.is_empty() || loci.len() > BLOCK_LOCI {
        return Err(IndexError::InvalidInput("sparse block locus count"));
    }
    let refs_len = checked_div_ceil_usize(
        loci.len()
            .checked_mul(2)
            .ok_or(IndexError::Arithmetic("sparse reference bits"))?,
        8,
        "sparse reference bytes",
    )?;
    let active_len = checked_div_ceil_usize(loci.len(), 8, "sparse active bytes")?;
    let rank_count = checked_div_ceil_usize(loci.len(), RANK_STRIDE, "sparse rank count")?;
    let mut references = vec![0_u8; refs_len];
    let mut active = vec![0_u8; active_len];
    let mut ranks = Vec::with_capacity(
        rank_count
            .checked_mul(RANK_BYTES)
            .ok_or(IndexError::Arithmetic("sparse rank bytes"))?,
    );
    let mut masks = Vec::new();
    let mut values = Vec::new();
    let mut pair_count = 0_u32;
    for (ordinal, locus) in loci.iter().enumerate() {
        if ordinal % RANK_STRIDE == 0 {
            ranks.extend_from_slice(
                &u32::try_from(masks.len())
                    .map_err(|_| IndexError::Arithmetic("sparse active rank"))?
                    .to_le_bytes(),
            );
            ranks.extend_from_slice(&pair_count.to_le_bytes());
        }
        references[ordinal * 2 / 8] |= base_code(locus.reference) << (ordinal * 2 % 8);
        let mut mask = 0_u8;
        for (alternative_index, alternative) in locus.alternatives.iter().enumerate() {
            for (kind, magnitude, position) in [
                (
                    0,
                    alternative.score.gain(),
                    alternative.score.gain_position(),
                ),
                (
                    1,
                    alternative.score.loss(),
                    alternative.score.loss_position(),
                ),
            ] {
                if magnitude.hundredths() != 0 || position.get() != DEFAULT_POSITION {
                    let bit = alternative_index
                        .checked_mul(2)
                        .and_then(|value| value.checked_add(kind))
                        .ok_or(IndexError::Arithmetic("sparse score mask bit"))?;
                    mask |= 1_u8
                        .checked_shl(
                            u32::try_from(bit)
                                .map_err(|_| IndexError::Arithmetic("sparse score mask bit"))?,
                        )
                        .ok_or(IndexError::Arithmetic("sparse score mask bit"))?;
                    values.extend_from_slice(&encode_pair(alternative.score, kind)?.to_le_bytes());
                    pair_count = pair_count
                        .checked_add(1)
                        .ok_or(IndexError::Arithmetic("sparse score pair count"))?;
                }
            }
        }
        if mask != 0 {
            active[ordinal / 8] |= 1 << (ordinal % 8);
            masks.push(mask);
        }
    }
    let mask_bits = masks
        .len()
        .checked_mul(6)
        .ok_or(IndexError::Arithmetic("sparse mask bits"))?;
    let masks_len = checked_div_ceil_usize(mask_bits, 8, "sparse mask bytes")?;
    let mut packed_masks = vec![0_u8; masks_len];
    for (index, mask) in masks.iter().copied().enumerate() {
        let bit = index
            .checked_mul(6)
            .ok_or(IndexError::Arithmetic("sparse mask offset"))?;
        let shifted = u16::from(mask) << (bit % 8);
        packed_masks[bit / 8] |= shifted.to_le_bytes()[0];
        if bit % 8 > 2 {
            packed_masks[bit / 8 + 1] |= shifted.to_le_bytes()[1];
        }
    }
    let capacity = [
        BLOCK_HEADER_BYTES,
        references.len(),
        active.len(),
        ranks.len(),
        packed_masks.len(),
        values.len(),
    ]
    .into_iter()
    .try_fold(0_usize, |total, length| total.checked_add(length))
    .ok_or(IndexError::Arithmetic("sparse block length"))?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(
        &u32::try_from(masks.len())
            .map_err(|_| IndexError::Arithmetic("sparse active count"))?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&pair_count.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(rank_count)
            .map_err(|_| IndexError::Arithmetic("sparse rank count"))?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&references);
    bytes.extend_from_slice(&active);
    bytes.extend_from_slice(&ranks);
    bytes.extend_from_slice(&packed_masks);
    bytes.extend_from_slice(&values);
    if bytes.len() != capacity {
        return Err(IndexError::Arithmetic("sparse block encoding"));
    }
    Ok(EncodedBlock {
        bytes,
        active_count: u32::try_from(masks.len())
            .map_err(|_| IndexError::Arithmetic("sparse active count"))?,
        pair_count,
    })
}

fn validate_locus(locus: &InputLocus) -> Result<(), IndexError> {
    match locus {
        InputLocus::Ordinary(value) => validate_alternatives(
            &value.alternatives,
            value.reference,
            "ordinary alternate set",
        )?,
        InputLocus::Ambiguous(value) => {
            if !matches!(value.omitted, DnaBase::A | DnaBase::T) {
                return Err(IndexError::InvalidInput("ambiguous omitted base"));
            }
            validate_alternatives(
                &value.alternatives,
                value.omitted,
                "ambiguous alternate set",
            )?;
        }
    }
    Ok(())
}

fn validate_alternatives(
    alternatives: &[InputAlternative; 3],
    excluded: DnaBase,
    reason: &'static str,
) -> Result<(), IndexError> {
    let mut observed = alternatives.map(|value| base_code(value.alternate));
    observed.sort_unstable();
    let expected: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != excluded)
        .map(base_code)
        .collect();
    if observed.as_slice() != expected {
        return Err(IndexError::InvalidInput(reason));
    }
    for alternative in alternatives {
        validate_score(alternative.score)?;
    }
    Ok(())
}

fn validate_score(score: PangolinScore) -> Result<(), IndexError> {
    for position in [score.gain_position(), score.loss_position()] {
        if !(-50..=50).contains(&position.get()) {
            return Err(IndexError::InvalidInput("sparse relative position"));
        }
    }
    Ok(())
}

fn encode_pair(score: PangolinScore, kind: usize) -> Result<u16, IndexError> {
    let (magnitude, position) = if kind == 0 {
        (score.gain(), score.gain_position())
    } else {
        (score.loss(), score.loss_position())
    };
    let shifted = i32::from(position.get())
        .checked_add(50)
        .ok_or(IndexError::Arithmetic("sparse relative position"))?;
    let position =
        u16::try_from(shifted).map_err(|_| IndexError::InvalidInput("sparse relative position"))?;
    Ok(u16::from(magnitude.hundredths()) | (position << 7))
}

fn encode_exception(locus: &AmbiguousInputLocus) -> Result<[u8; 40], IndexError> {
    let mut bytes = [0_u8; 40];
    bytes[0] = locus.contig.code();
    bytes[1] = base_code(locus.omitted);
    for (index, alternative) in locus.alternatives.iter().enumerate() {
        bytes[2 + index] = base_code(alternative.alternate);
    }
    bytes[8..16].copy_from_slice(&locus.gene.numeric().to_le_bytes());
    bytes[16..20].copy_from_slice(&locus.position.get().to_le_bytes());
    for (index, alternative) in locus.alternatives.iter().enumerate() {
        let start = 24 + index * 4;
        bytes[start..start + 2].copy_from_slice(&encode_pair(alternative.score, 0)?.to_le_bytes());
        bytes[start + 2..start + 4]
            .copy_from_slice(&encode_pair(alternative.score, 1)?.to_le_bytes());
    }
    Ok(bytes)
}

struct HeaderValues {
    file_len: u64,
    gene_offset: u64,
    gene_len: u64,
    segment_offset: u64,
    segment_len: u64,
    block_offset: u64,
    block_len: u64,
    payload_offset: u64,
    payload_len: u64,
    exception_offset: u64,
    exception_len: u64,
    gene_count: u64,
    segment_count: u64,
    block_count: u64,
    ordinary_loci: u64,
    exception_count: u64,
}

fn encode_header(values: HeaderValues) -> Result<[u8; 256], IndexError> {
    let mut bytes = [0_u8; 256];
    bytes[..8].copy_from_slice(MAGIC);
    put_u32(&mut bytes, 8, VERSION)?;
    put_u32(
        &mut bytes,
        12,
        u32::try_from(HEADER_BYTES).map_err(|_| IndexError::Arithmetic("sparse header size"))?,
    )?;
    put_u64(&mut bytes, 16, values.file_len)?;
    for (offset, section_offset, section_len) in [
        (24, values.gene_offset, values.gene_len),
        (40, values.segment_offset, values.segment_len),
        (56, values.block_offset, values.block_len),
        (72, values.payload_offset, values.payload_len),
        (88, values.exception_offset, values.exception_len),
    ] {
        put_u64(&mut bytes, offset, section_offset)?;
        put_u64(&mut bytes, offset + 8, section_len)?;
    }
    for (offset, value) in [
        (104, values.gene_count),
        (112, values.segment_count),
        (120, values.block_count),
        (128, values.ordinary_loci),
        (136, values.exception_count),
    ] {
        put_u64(&mut bytes, offset, value)?;
    }
    put_u32(
        &mut bytes,
        144,
        u32::try_from(BLOCK_LOCI).map_err(|_| IndexError::Arithmetic("sparse block loci"))?,
    )?;
    put_u32(
        &mut bytes,
        148,
        u32::try_from(RANK_STRIDE).map_err(|_| IndexError::Arithmetic("sparse rank stride"))?,
    )?;
    Ok(bytes)
}

fn encode_gene(value: GeneEntry) -> Result<[u8; 32], IndexError> {
    let mut bytes = [0_u8; 32];
    put_u64(&mut bytes, 0, value.gene.numeric())?;
    put_u64(&mut bytes, 8, value.segment_start)?;
    put_u32(&mut bytes, 16, value.segment_count)?;
    put_u32(&mut bytes, 20, value.ordinary_loci)?;
    Ok(bytes)
}

fn encode_segment(value: SegmentEntry) -> Result<[u8; 48], IndexError> {
    let mut bytes = [0_u8; 48];
    put_u64(&mut bytes, 0, value.gene.numeric())?;
    bytes[8] = value.contig.code();
    put_u32(&mut bytes, 16, value.start)?;
    put_u32(&mut bytes, 20, value.end)?;
    put_u32(&mut bytes, 24, value.loci)?;
    put_u32(&mut bytes, 28, value.block_start)?;
    put_u32(&mut bytes, 32, value.block_count)?;
    Ok(bytes)
}

fn encode_block_entry(value: BlockEntry) -> Result<[u8; 40], IndexError> {
    let mut bytes = [0_u8; 40];
    put_u64(&mut bytes, 0, value.segment)?;
    put_u32(&mut bytes, 8, value.first)?;
    put_u32(&mut bytes, 12, value.count)?;
    put_u64(&mut bytes, 16, value.payload_offset)?;
    put_u32(&mut bytes, 24, value.payload_len)?;
    put_u32(&mut bytes, 28, value.active_count)?;
    put_u32(&mut bytes, 32, value.pair_count)?;
    Ok(bytes)
}

fn locus_gene(locus: &InputLocus) -> EnsemblGeneId {
    match locus {
        InputLocus::Ordinary(value) => value.gene,
        InputLocus::Ambiguous(value) => value.gene,
    }
}

fn locus_coordinate(locus: &InputLocus) -> (u8, u32) {
    match locus {
        InputLocus::Ordinary(value) => (value.contig.code(), value.position.get()),
        InputLocus::Ambiguous(value) => (value.contig.code(), value.position.get()),
    }
}

fn base_code(base: DnaBase) -> u8 {
    match base {
        DnaBase::A => 0,
        DnaBase::C => 1,
        DnaBase::G => 2,
        DnaBase::T => 3,
    }
}

fn create_new(path: &Path) -> Result<File, IndexError> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)?)
}

fn exception_scratch_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".exceptions");
    PathBuf::from(name)
}

fn create_stage(output: &Path) -> Result<(PathBuf, FileIdentity, File), IndexError> {
    if output.file_name().is_none() {
        return Err(IndexError::InvalidInput("sparse output path"));
    }
    for _ in 0..1024 {
        let sequence = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = output
            .file_name()
            .ok_or(IndexError::InvalidInput("sparse output path"))?
            .to_owned();
        name.push(format!(
            ".pangopup-sparse-stage-{}-{sequence}",
            std::process::id()
        ));
        let stage = output.with_file_name(name);
        match create_new(&stage) {
            Ok(file) => {
                let identity = file_identity(&file)?;
                return Ok((stage, identity, file));
            }
            Err(IndexError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(IndexError::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no unused sparse stage path",
    )))
}

fn publish_noreplace(
    stage: &Path,
    stage_identity: FileIdentity,
    output: &Path,
) -> Result<(), IndexError> {
    require_identity(stage, stage_identity)?;
    let parent = usable_parent(output)?;
    let parent = File::open(parent)?;
    fs::hard_link(stage, output)?;
    if let Err(error) = fs::remove_file(stage) {
        let _ = fs::remove_file(output);
        return Err(IndexError::Io(error));
    }
    if let Err(error) = parent.sync_all() {
        let _ = fs::remove_file(output);
        return Err(IndexError::Io(error));
    }
    Ok(())
}

fn usable_parent(path: &Path) -> Result<&Path, IndexError> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(IndexError::InvalidInput("sparse output parent")),
    }
}

fn file_identity(file: &File) -> Result<FileIdentity, IndexError> {
    let metadata = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(FileIdentity {})
    }
}

fn require_identity(path: &Path, expected: FileIdentity) -> Result<(), IndexError> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != expected.device || metadata.ino() != expected.inode {
            return Err(IndexError::Io(io::Error::other(
                "writer-owned path was replaced",
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = (metadata, expected);
    Ok(())
}

fn remove_owned(path: &Path, identity: FileIdentity, owned: &mut bool) -> Result<(), IndexError> {
    if !*owned {
        return Ok(());
    }
    require_identity(path, identity)?;
    fs::remove_file(path)?;
    *owned = false;
    Ok(())
}

fn remove_if_owned(path: &Path, identity: FileIdentity) {
    if require_identity(path, identity).is_ok() {
        let _ = fs::remove_file(path);
    }
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), IndexError> {
    let end = offset
        .checked_add(4)
        .ok_or(IndexError::Arithmetic("sparse u32 offset"))?;
    bytes
        .get_mut(offset..end)
        .ok_or(IndexError::Arithmetic("sparse u32 bounds"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), IndexError> {
    let end = offset
        .checked_add(8)
        .ok_or(IndexError::Arithmetic("sparse u64 offset"))?;
    bytes
        .get_mut(offset..end)
        .ok_or(IndexError::Arithmetic("sparse u64 bounds"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn checked_add(left: u64, right: u64, reason: &'static str) -> Result<u64, IndexError> {
    left.checked_add(right)
        .ok_or(IndexError::Arithmetic(reason))
}

fn checked_mul(left: u64, right: u64, reason: &'static str) -> Result<u64, IndexError> {
    left.checked_mul(right)
        .ok_or(IndexError::Arithmetic(reason))
}

fn usize_u64(value: usize, reason: &'static str) -> Result<u64, IndexError> {
    u64::try_from(value).map_err(|_| IndexError::Arithmetic(reason))
}

fn checked_div_ceil_usize(
    value: usize,
    divisor: usize,
    reason: &'static str,
) -> Result<usize, IndexError> {
    value
        .checked_add(divisor - 1)
        .ok_or(IndexError::Arithmetic(reason))
        .map(|sum| sum / divisor)
}

#[cfg(test)]
fn injected_write(file: &mut File, bytes: &[u8], remaining: &mut u64) -> Result<(), IndexError> {
    let allowed = usize::try_from((*remaining).min(bytes.len() as u64))
        .map_err(|_| IndexError::Arithmetic("injected write limit"))?;
    if allowed != 0 {
        file.write_all(&bytes[..allowed])?;
        *remaining -= allowed as u64;
    }
    if allowed < bytes.len() {
        return Err(IndexError::Io(io::Error::other(
            "injected sparse write failure",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputAlternative, OrdinaryInputLocus};
    use pangopup_core::{GenomicPosition, RelativePosition, ScoreMagnitude};

    struct Temp(PathBuf);

    impl Temp {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pangopup-sparse-unit-{label}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    #[test]
    fn injected_spool_failure_poisons_and_cleans_owned_paths() {
        let temp = Temp::new("spool-failure");
        let scratch = temp.0.join("payload.scratch");
        let output = temp.0.join("candidate.pgi");
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        writer.fail_spool_after = Some(3);
        assert!(writer.push_gene(&[ordinary(1)]).is_err());
        assert!(writer.finish(&output).is_err());
        assert!(!scratch.exists());
        assert!(!exception_scratch_path(&scratch).exists());
        assert!(!output.exists());
    }

    #[test]
    fn injected_assembly_failure_does_not_publish_or_leave_owned_paths() {
        let temp = Temp::new("assembly-failure");
        let scratch = temp.0.join("payload.scratch");
        let output = temp.0.join("candidate.pgi");
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        writer.push_gene(&[ordinary(1)]).expect("gene");
        writer.fail_assembly_after = Some(381);
        assert!(writer.finish(&output).is_err());
        assert!(!scratch.exists());
        assert!(!exception_scratch_path(&scratch).exists());
        assert!(!output.exists());
        assert_eq!(
            fs::read_dir(&temp.0)
                .expect("directory")
                .filter_map(Result::ok)
                .count(),
            0
        );
    }

    #[test]
    fn cleanup_failure_prevents_publication_and_preserves_existing_output() {
        let temp = Temp::new("cleanup-failure");
        let scratch = temp.0.join("payload.scratch");
        let output = temp.0.join("candidate.pgi");
        fs::write(&output, b"existing output").expect("seed output");
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        writer.push_gene(&[ordinary(1)]).expect("gene");
        writer.fail_cleanup_once = true;
        assert!(writer.finish(&output).is_err());
        assert_eq!(fs::read(&output).expect("output"), b"existing output");
        assert!(!scratch.exists());
        assert!(!exception_scratch_path(&scratch).exists());
    }

    #[test]
    fn cleanup_failure_prevents_publication_when_output_is_absent() {
        let temp = Temp::new("cleanup-failure-absent-output");
        let scratch = temp.0.join("payload.scratch");
        let output = temp.0.join("candidate.pgi");
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        writer.push_gene(&[ordinary(1)]).expect("gene");
        writer.fail_cleanup_once = true;
        assert!(writer.finish(&output).is_err());
        assert!(!output.exists());
        assert!(!scratch.exists());
        assert!(!exception_scratch_path(&scratch).exists());
    }

    #[test]
    fn replaced_scratch_is_never_removed_or_published() {
        let temp = Temp::new("scratch-replacement");
        let scratch = temp.0.join("payload.scratch");
        let output = temp.0.join("candidate.pgi");
        let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
        writer.push_gene(&[ordinary(1)]).expect("gene");
        fs::remove_file(&scratch).expect("unlink owned scratch name");
        fs::write(&scratch, b"replacement scratch").expect("replace scratch name");
        assert!(writer.finish(&output).is_err());
        assert!(!output.exists());
        assert_eq!(
            fs::read(&scratch).expect("replacement scratch"),
            b"replacement scratch"
        );
    }

    fn ordinary(numeric: u64) -> InputLocus {
        let zero = ScoreMagnitude::new(0).expect("zero");
        let position = RelativePosition::new(-50).expect("position");
        let score = PangolinScore::new(zero, position, zero, position);
        InputLocus::Ordinary(OrdinaryInputLocus {
            gene: EnsemblGeneId::from_numeric(numeric).expect("gene"),
            contig: Grch38Contig::from_code(1).expect("contig"),
            position: GenomicPosition::new(1).expect("coordinate"),
            reference: DnaBase::A,
            alternatives: [DnaBase::C, DnaBase::G, DnaBase::T]
                .map(|alternate| InputAlternative { alternate, score }),
        })
    }
}
