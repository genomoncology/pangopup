//! Candidate-only bounded reader for `pangopup.sparse-direct.v1`.
//!
//! This reader exists for qualification. It has no bundle, asset, profile, or
//! runtime routing integration.

use crate::{
    AmbiguousInputLocus, DecodedSummary, IndexError, InputAlternative, InputLocus,
    OrdinaryInputLocus, VisitAllError,
};
use memmap2::Mmap;
use pangopup_core::{
    DnaBase, EnsemblGeneId, GeneScoreRecord, GenomicPosition, Grch38Contig, Grch38Snv,
    PangolinScore, RelativePosition, ScoreMagnitude, SourceReferenceAmbiguity,
};
#[cfg(feature = "test-read-audit")]
use std::cell::{Cell, RefCell};
use std::{convert::Infallible, fs::File, path::Path};

const MAGIC: &[u8; 8] = b"PGSPRS01";
const VERSION: u32 = 1;
const HEADER_BYTES: u64 = 256;
const GENE_BYTES: u64 = 32;
const SEGMENT_BYTES: u64 = 48;
const BLOCK_BYTES: u64 = 40;
const EXCEPTION_BYTES: u64 = 40;
const BLOCK_LOCI: u32 = 4096;
const RANK_STRIDE: u32 = 64;
const BLOCK_HEADER_BYTES: u64 = 16;
const RANK_BYTES: u64 = 8;

#[cfg(feature = "test-read-audit")]
thread_local! {
    static TEST_PAYLOAD_READ_AUDIT_ENABLED: Cell<bool> = const { Cell::new(false) };
    static TEST_PAYLOAD_READ_OFFSETS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Reset the test-only count of ordinary payload bytes accessed by this reader.
#[cfg(feature = "test-read-audit")]
pub fn test_reset_sparse_payload_read_bytes() {
    TEST_PAYLOAD_READ_OFFSETS.with(|offsets| offsets.borrow_mut().clear());
    TEST_PAYLOAD_READ_AUDIT_ENABLED.set(true);
}

/// Return the test-only count of ordinary payload bytes accessed by this reader.
#[cfg(feature = "test-read-audit")]
pub fn test_sparse_payload_read_bytes() -> usize {
    TEST_PAYLOAD_READ_OFFSETS.with(|offsets| offsets.borrow().len())
}

/// Report whether the reader accessed one absolute file byte in ordinary payload.
#[cfg(feature = "test-read-audit")]
pub fn test_sparse_payload_byte_was_read(absolute_offset: usize) -> bool {
    TEST_PAYLOAD_READ_OFFSETS.with(|offsets| offsets.borrow().contains(&absolute_offset))
}

#[cfg(feature = "test-read-audit")]
fn record_payload_read(absolute_offset: usize, bytes: usize) {
    if !TEST_PAYLOAD_READ_AUDIT_ENABLED.get() {
        return;
    }
    TEST_PAYLOAD_READ_OFFSETS.with(|offsets| {
        let mut offsets = offsets.borrow_mut();
        offsets.extend(absolute_offset..absolute_offset.saturating_add(bytes));
    });
}

#[cfg(not(feature = "test-read-audit"))]
fn record_payload_read(_absolute_offset: usize, _bytes: usize) {}

#[derive(Clone, Copy, Debug)]
struct Section {
    offset: u64,
    len: u64,
}

#[derive(Clone, Copy, Debug)]
struct Header {
    file_len: u64,
    genes: Section,
    segments: Section,
    blocks: Section,
    payload: Section,
    exceptions: Section,
    gene_count: u64,
    segment_count: u64,
    block_count: u64,
    ordinary_loci: u64,
    exception_count: u64,
}

#[derive(Clone, Copy, Debug)]
struct GeneView {
    gene: EnsemblGeneId,
    segment_start: u64,
    segment_count: u32,
    ordinary_loci: u32,
}

#[derive(Clone, Copy, Debug)]
struct SegmentView {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
    end: u32,
    loci: u32,
    block_start: u32,
    block_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct BlockView {
    segment: u64,
    first: u32,
    count: u32,
    payload_offset: u64,
    payload_len: u32,
    active_count: u32,
    pair_count: u32,
}

#[derive(Clone, Copy)]
struct Payload<'a> {
    raw: &'a [u8],
    absolute_start: usize,
}

impl Payload<'_> {
    fn len(self) -> usize {
        self.raw.len()
    }

    fn byte(self, offset: usize) -> Result<u8, IndexError> {
        let value = *self
            .raw
            .get(offset)
            .ok_or(IndexError::Corrupt("truncated sparse payload byte"))?;
        let absolute = self
            .absolute_start
            .checked_add(offset)
            .ok_or(IndexError::Arithmetic("sparse payload audit offset"))?;
        record_payload_read(absolute, 1);
        Ok(value)
    }

    fn u16(self, offset: usize) -> Result<u16, IndexError> {
        let first = self.byte(offset)?;
        let second = self.byte(
            offset
                .checked_add(1)
                .ok_or(IndexError::Arithmetic("sparse payload u16 offset"))?,
        )?;
        Ok(u16::from_le_bytes([first, second]))
    }

    fn u32(self, offset: usize) -> Result<u32, IndexError> {
        let mut value = [0_u8; 4];
        for (index, byte) in value.iter_mut().enumerate() {
            *byte = self.byte(
                offset
                    .checked_add(index)
                    .ok_or(IndexError::Arithmetic("sparse payload u32 offset"))?,
            )?;
        }
        Ok(u32::from_le_bytes(value))
    }
}

#[derive(Clone, Copy)]
struct BlockLayout<'a> {
    payload: Payload<'a>,
    references_start: usize,
    active_start: usize,
    ranks_start: usize,
    masks_start: usize,
    values_start: usize,
}

/// Validated memory-mapped reader for the sparse-direct v1 candidate.
#[derive(Debug)]
pub struct SparseIndexReader {
    map: Mmap,
    header: Header,
}

impl SparseIndexReader {
    /// Map a candidate file and validate all metadata without reading its
    /// ordinary score payload.
    ///
    /// The mapped inode must remain immutable and must not be truncated or
    /// modified for the life of this reader. In-place changes can cause an
    /// operating-system fault before Rust can return a typed error.
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        let file = File::open(path)?;
        Self::open_file(&file)
    }

    /// Map and validate an already-open immutable candidate file.
    ///
    /// The caller must keep the mapped inode immutable for the life of this
    /// reader. In-place truncation can cause an operating-system fault.
    pub fn open_file(file: &File) -> Result<Self, IndexError> {
        // SAFETY: The deployment contract requires an immutable inode. All
        // accesses are bounds checked and bytes are decoded explicitly.
        let map = unsafe { Mmap::map(file) }.map_err(IndexError::Io)?;
        let header = decode_header(&map)?;
        validate_metadata(&map, &header)?;
        Ok(Self { map, header })
    }

    pub fn file_len(&self) -> u64 {
        self.header.file_len
    }

    pub fn gene_count(&self) -> u64 {
        self.header.gene_count
    }

    pub fn segment_count(&self) -> u64 {
        self.header.segment_count
    }

    pub fn block_count(&self) -> u64 {
        self.header.block_count
    }

    pub fn exception_count(&self) -> u64 {
        self.header.exception_count
    }

    /// Return every matching gene in canonical numeric order.
    pub fn lookup_parts(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
    ) -> Result<(Vec<GeneScoreRecord>, Vec<SourceReferenceAmbiguity>), IndexError> {
        let mut records = Vec::new();
        let mut ambiguities = Vec::new();
        match gene {
            Some(gene) => {
                if let Some(index) = self.find_gene(gene)? {
                    let row = self.gene(index)?;
                    let end = checked_add(
                        row.segment_start,
                        u64::from(row.segment_count),
                        "sparse filtered segment end",
                    )?;
                    for segment_index in row.segment_start..end {
                        self.lookup_segment(segment_index, snv, &mut records)?;
                    }
                    let (start, end) = self.exception_gene_range(gene)?;
                    for index in start..end {
                        let exception = self.exception(index)?;
                        if exception.contig == snv.contig() && exception.position == snv.position()
                        {
                            ambiguities.push(SourceReferenceAmbiguity::new(
                                exception.gene,
                                exception.omitted,
                            ));
                        }
                    }
                }
            }
            None => {
                for segment_index in 0..self.header.segment_count {
                    self.lookup_segment(segment_index, snv, &mut records)?;
                }
                for index in 0..self.header.exception_count {
                    let exception = self.exception(index)?;
                    if exception.contig == snv.contig() && exception.position == snv.position() {
                        ambiguities.push(SourceReferenceAmbiguity::new(
                            exception.gene,
                            exception.omitted,
                        ));
                    }
                }
            }
        }
        Ok((records, ambiguities))
    }

    /// Stream every logical locus in canonical gene, contig, coordinate order.
    /// Each ordinary block is validated once before any locus from it is sent
    /// to the visitor.
    pub fn visit_all<E>(
        &self,
        mut visitor: impl FnMut(InputLocus) -> Result<(), E>,
    ) -> Result<DecodedSummary, VisitAllError<E>> {
        let mut summary = DecodedSummary {
            genes: self.header.gene_count,
            loci: checked_add(
                self.header.ordinary_loci,
                self.header.exception_count,
                "sparse logical locus count",
            )
            .map_err(VisitAllError::Index)?,
            ordinary_loci: self.header.ordinary_loci,
            exceptions: self.header.exception_count,
            segments: self.header.segment_count,
        };
        // Keep construction explicit if DecodedSummary gains a field.
        summary.genes = self.header.gene_count;

        for gene_index in 0..self.header.gene_count {
            let gene = self.gene(gene_index).map_err(VisitAllError::Index)?;
            let (mut exception_index, exception_end) = self
                .exception_gene_range(gene.gene)
                .map_err(VisitAllError::Index)?;
            let segment_end = checked_add(
                gene.segment_start,
                u64::from(gene.segment_count),
                "sparse traversal segment end",
            )
            .map_err(VisitAllError::Index)?;

            for segment_index in gene.segment_start..segment_end {
                let segment = self.segment(segment_index).map_err(VisitAllError::Index)?;
                while exception_index < exception_end {
                    let exception = self
                        .exception(exception_index)
                        .map_err(VisitAllError::Index)?;
                    if (exception.contig.code(), exception.position.get())
                        >= (segment.contig.code(), segment.start)
                    {
                        break;
                    }
                    visitor(InputLocus::Ambiguous(exception)).map_err(VisitAllError::Visitor)?;
                    exception_index += 1;
                }
                let block_end = checked_add(
                    u64::from(segment.block_start),
                    u64::from(segment.block_count),
                    "sparse traversal block end",
                )
                .map_err(VisitAllError::Index)?;
                for block_index in u64::from(segment.block_start)..block_end {
                    let block = self.block(block_index).map_err(VisitAllError::Index)?;
                    let layout = self
                        .validate_block(block, true)
                        .map_err(VisitAllError::Index)?;
                    for ordinal in 0..block.count {
                        let position = segment
                            .start
                            .checked_add(block.first)
                            .and_then(|value| value.checked_add(ordinal))
                            .ok_or_else(|| {
                                VisitAllError::Index(IndexError::Arithmetic(
                                    "sparse traversal coordinate",
                                ))
                            })?;
                        while exception_index < exception_end {
                            let exception = self
                                .exception(exception_index)
                                .map_err(VisitAllError::Index)?;
                            if (exception.contig.code(), exception.position.get())
                                >= (segment.contig.code(), position)
                            {
                                break;
                            }
                            visitor(InputLocus::Ambiguous(exception))
                                .map_err(VisitAllError::Visitor)?;
                            exception_index += 1;
                        }
                        let ordinary = decode_ordinary(
                            &layout,
                            block,
                            ordinal,
                            segment.gene,
                            segment.contig,
                            position,
                        )
                        .map_err(VisitAllError::Index)?;
                        visitor(InputLocus::Ordinary(ordinary)).map_err(VisitAllError::Visitor)?;
                    }
                }
            }
            while exception_index < exception_end {
                let exception = self
                    .exception(exception_index)
                    .map_err(VisitAllError::Index)?;
                visitor(InputLocus::Ambiguous(exception)).map_err(VisitAllError::Visitor)?;
                exception_index += 1;
            }
        }
        Ok(summary)
    }

    /// Exhaustively validate every ordinary score pair and exception.
    pub fn verify_all(&self) -> Result<DecodedSummary, IndexError> {
        self.visit_all(|_| Ok::<(), Infallible>(()))
            .map_err(|error| match error {
                VisitAllError::Index(error) => error,
                VisitAllError::Visitor(never) => match never {},
            })
    }

    fn lookup_segment(
        &self,
        segment_index: u64,
        snv: Grch38Snv,
        records: &mut Vec<GeneScoreRecord>,
    ) -> Result<(), IndexError> {
        let segment = self.segment(segment_index)?;
        let position = snv.position().get();
        if segment.contig != snv.contig() || position < segment.start || position > segment.end {
            return Ok(());
        }
        let ordinal = position - segment.start;
        let mut low = u64::from(segment.block_start);
        let mut high = checked_add(
            low,
            u64::from(segment.block_count),
            "sparse lookup block end",
        )?;
        while low < high {
            let middle = low + (high - low) / 2;
            let block = self.block(middle)?;
            if block.first <= ordinal {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == u64::from(segment.block_start) {
            return Err(IndexError::Corrupt("sparse missing lookup block"));
        }
        let block = self.block(low - 1)?;
        let block_end = block
            .first
            .checked_add(block.count)
            .ok_or(IndexError::Arithmetic("sparse lookup block range"))?;
        if ordinal >= block_end {
            return Err(IndexError::Corrupt("sparse lookup block gap"));
        }
        let block_ordinal = ordinal - block.first;
        let layout = self.validate_block(block, false)?;
        let reference = reference_at(&layout, block_ordinal)?;
        // Decode every pair for the addressed locus before applying REF or ALT.
        let ordinary = decode_ordinary(
            &layout,
            block,
            block_ordinal,
            segment.gene,
            segment.contig,
            position,
        )?;
        if reference != snv.reference() {
            return Ok(());
        }
        if let Some(alternative) = ordinary
            .alternatives
            .iter()
            .find(|value| value.alternate == snv.alternate())
        {
            records.push(GeneScoreRecord::new(segment.gene, alternative.score));
        }
        Ok(())
    }

    fn validate_block(
        &self,
        block: BlockView,
        validate_all_pairs: bool,
    ) -> Result<BlockLayout<'_>, IndexError> {
        let absolute = checked_add(
            self.header.payload.offset,
            block.payload_offset,
            "sparse block payload address",
        )?;
        let start = usize_from_u64(absolute, "sparse block payload offset")?;
        let len = usize::try_from(block.payload_len)
            .map_err(|_| IndexError::Corrupt("sparse block payload length"))?;
        let end = start
            .checked_add(len)
            .ok_or(IndexError::Arithmetic("sparse block payload end"))?;
        let raw = self
            .map
            .get(start..end)
            .ok_or(IndexError::Corrupt("truncated sparse block"))?;
        let payload = Payload {
            raw,
            absolute_start: start,
        };
        if payload.u32(0)? != block.active_count
            || payload.u32(4)? != block.pair_count
            || payload.u32(8)? != div_ceil_u32(block.count, RANK_STRIDE)?
            || payload.u32(12)? != 0
        {
            return Err(IndexError::Corrupt("sparse block header"));
        }
        let references_len = div_ceil_u64(
            checked_mul(u64::from(block.count), 2, "sparse reference bits")?,
            8,
            "sparse reference bytes",
        )?;
        let active_len = div_ceil_u64(u64::from(block.count), 8, "sparse active bytes")?;
        let rank_count = u64::from(div_ceil_u32(block.count, RANK_STRIDE)?);
        let ranks_len = checked_mul(rank_count, RANK_BYTES, "sparse rank bytes")?;
        let masks_len = div_ceil_u64(
            checked_mul(u64::from(block.active_count), 6, "sparse mask bits")?,
            8,
            "sparse mask bytes",
        )?;
        let values_len = checked_mul(u64::from(block.pair_count), 2, "sparse score value bytes")?;
        let references_start = usize::try_from(BLOCK_HEADER_BYTES)
            .map_err(|_| IndexError::Arithmetic("sparse block header length"))?;
        let active_start = add_usize_u64(references_start, references_len)?;
        let ranks_start = add_usize_u64(active_start, active_len)?;
        let masks_start = add_usize_u64(ranks_start, ranks_len)?;
        let values_start = add_usize_u64(masks_start, masks_len)?;
        let expected_end = add_usize_u64(values_start, values_len)?;
        if expected_end != payload.len() {
            return Err(IndexError::Corrupt("sparse block encoded length"));
        }
        let layout = BlockLayout {
            payload,
            references_start,
            active_start,
            ranks_start,
            masks_start,
            values_start,
        };
        validate_payload_tail_bits(
            payload,
            references_start,
            active_start,
            u64::from(block.count) * 2,
            "sparse reference tail bits",
        )?;
        validate_payload_tail_bits(
            payload,
            active_start,
            ranks_start,
            u64::from(block.count),
            "sparse active tail bits",
        )?;
        validate_payload_tail_bits(
            payload,
            masks_start,
            values_start,
            u64::from(block.active_count) * 6,
            "sparse mask tail bits",
        )?;

        let mut active_seen = 0_u32;
        let mut pairs_seen = 0_u32;
        for ordinal in 0..block.count {
            if ordinal % RANK_STRIDE == 0 {
                let rank = u64::from(ordinal / RANK_STRIDE);
                let rank_offset = add_usize_u64(
                    ranks_start,
                    checked_mul(rank, RANK_BYTES, "sparse rank address")?,
                )?;
                if payload.u32(rank_offset)? != active_seen
                    || payload.u32(rank_offset + 4)? != pairs_seen
                {
                    return Err(IndexError::Corrupt("sparse rank checkpoint"));
                }
            }
            if is_active(&layout, ordinal)? {
                let mask = mask_at(&layout, active_seen)?;
                if mask == 0 {
                    return Err(IndexError::Corrupt("sparse active mask mismatch"));
                }
                active_seen = active_seen
                    .checked_add(1)
                    .ok_or(IndexError::Arithmetic("sparse active count"))?;
                pairs_seen = pairs_seen
                    .checked_add(mask.count_ones())
                    .ok_or(IndexError::Arithmetic("sparse pair count"))?;
            }
        }
        if active_seen != block.active_count || pairs_seen != block.pair_count {
            return Err(IndexError::Corrupt("sparse block totals"));
        }
        if validate_all_pairs {
            for pair in 0..block.pair_count {
                decode_pair_at(&layout, pair)?;
            }
        }
        Ok(layout)
    }

    fn find_gene(&self, target: EnsemblGeneId) -> Result<Option<u64>, IndexError> {
        let mut low = 0_u64;
        let mut high = self.header.gene_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let gene = self.gene(middle)?.gene;
            if gene.numeric() < target.numeric() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low < self.header.gene_count && self.gene(low)?.gene == target {
            Ok(Some(low))
        } else {
            Ok(None)
        }
    }

    fn exception_gene_range(&self, gene: EnsemblGeneId) -> Result<(u64, u64), IndexError> {
        let first = self.lower_exception_gene(gene.numeric())?;
        let end_numeric = gene.numeric().checked_add(1);
        let end = match end_numeric {
            Some(value) => self.lower_exception_gene(value)?,
            None => self.header.exception_count,
        };
        Ok((first, end))
    }

    fn lower_exception_gene(&self, gene: u64) -> Result<u64, IndexError> {
        let mut low = 0_u64;
        let mut high = self.header.exception_count;
        while low < high {
            let middle = low + (high - low) / 2;
            if self.exception(middle)?.gene.numeric() < gene {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }

    fn gene(&self, index: u64) -> Result<GeneView, IndexError> {
        if index >= self.header.gene_count {
            return Err(IndexError::Corrupt("sparse gene index"));
        }
        let offset = entry_offset(self.header.genes.offset, index, GENE_BYTES, "sparse gene")?;
        decode_gene(&self.map, offset)
    }

    fn segment(&self, index: u64) -> Result<SegmentView, IndexError> {
        if index >= self.header.segment_count {
            return Err(IndexError::Corrupt("sparse segment index"));
        }
        let offset = entry_offset(
            self.header.segments.offset,
            index,
            SEGMENT_BYTES,
            "sparse segment",
        )?;
        decode_segment(&self.map, offset)
    }

    fn block(&self, index: u64) -> Result<BlockView, IndexError> {
        if index >= self.header.block_count {
            return Err(IndexError::Corrupt("sparse block index"));
        }
        let offset = entry_offset(
            self.header.blocks.offset,
            index,
            BLOCK_BYTES,
            "sparse block",
        )?;
        decode_block(&self.map, offset)
    }

    fn exception(&self, index: u64) -> Result<AmbiguousInputLocus, IndexError> {
        if index >= self.header.exception_count {
            return Err(IndexError::Corrupt("sparse exception index"));
        }
        let offset = entry_offset(
            self.header.exceptions.offset,
            index,
            EXCEPTION_BYTES,
            "sparse exception",
        )?;
        decode_exception(&self.map, offset)
    }
}

fn decode_header(bytes: &[u8]) -> Result<Header, IndexError> {
    if bytes.get(..8) != Some(MAGIC.as_slice()) {
        return Err(IndexError::Incompatible("sparse magic"));
    }
    if read_u32(bytes, 8)? != VERSION {
        return Err(IndexError::Incompatible("sparse version"));
    }
    if read_u32(bytes, 12)? != HEADER_BYTES as u32 {
        return Err(IndexError::Corrupt("sparse header length"));
    }
    let header = Header {
        file_len: read_u64(bytes, 16)?,
        genes: section(bytes, 24)?,
        segments: section(bytes, 40)?,
        blocks: section(bytes, 56)?,
        payload: section(bytes, 72)?,
        exceptions: section(bytes, 88)?,
        gene_count: read_u64(bytes, 104)?,
        segment_count: read_u64(bytes, 112)?,
        block_count: read_u64(bytes, 120)?,
        ordinary_loci: read_u64(bytes, 128)?,
        exception_count: read_u64(bytes, 136)?,
    };
    if read_u32(bytes, 144)? != BLOCK_LOCI || read_u32(bytes, 148)? != RANK_STRIDE {
        return Err(IndexError::Incompatible("sparse layout constants"));
    }
    require_zero(bytes, 152, HEADER_BYTES as usize, "sparse header reserved")?;
    if header.file_len != bytes.len() as u64 {
        return Err(IndexError::Corrupt("sparse file length"));
    }
    Ok(header)
}

fn validate_metadata(bytes: &[u8], header: &Header) -> Result<(), IndexError> {
    checked_add(
        header.ordinary_loci,
        header.exception_count,
        "sparse logical locus count",
    )?;
    let sections = [
        header.genes,
        header.segments,
        header.blocks,
        header.payload,
        header.exceptions,
    ];
    let mut next = HEADER_BYTES;
    for section in sections {
        if section.offset != next {
            return Err(IndexError::Corrupt("sparse section adjacency"));
        }
        next = checked_add(section.offset, section.len, "sparse section end")?;
        if next > header.file_len {
            return Err(IndexError::Corrupt("sparse section bounds"));
        }
    }
    if next != header.file_len {
        return Err(IndexError::Corrupt("sparse unclaimed file bytes"));
    }
    for (actual, count, width, reason) in [
        (
            header.genes.len,
            header.gene_count,
            GENE_BYTES,
            "sparse gene section length",
        ),
        (
            header.segments.len,
            header.segment_count,
            SEGMENT_BYTES,
            "sparse segment section length",
        ),
        (
            header.blocks.len,
            header.block_count,
            BLOCK_BYTES,
            "sparse block section length",
        ),
        (
            header.exceptions.len,
            header.exception_count,
            EXCEPTION_BYTES,
            "sparse exception section length",
        ),
    ] {
        if checked_mul(count, width, reason)? != actual {
            return Err(IndexError::Corrupt(reason));
        }
    }
    if header.gene_count == 0 {
        return Err(IndexError::Corrupt("empty sparse gene directory"));
    }

    let mut segment_cursor = 0_u64;
    let mut ordinary_total = 0_u64;
    let mut previous_gene = None;
    for index in 0..header.gene_count {
        let gene = decode_gene(
            bytes,
            entry_offset(header.genes.offset, index, GENE_BYTES, "sparse gene")?,
        )?;
        if previous_gene.is_some_and(|value| value >= gene.gene.numeric()) {
            return Err(IndexError::Corrupt("sparse gene ordering"));
        }
        if gene.segment_start != segment_cursor {
            return Err(IndexError::Corrupt("sparse gene segment ownership"));
        }
        let segment_end = checked_add(
            segment_cursor,
            u64::from(gene.segment_count),
            "sparse gene segment range",
        )?;
        if segment_end > header.segment_count {
            return Err(IndexError::Corrupt("sparse gene segment range"));
        }
        let mut gene_loci = 0_u64;
        for segment_index in segment_cursor..segment_end {
            let segment = decode_segment(
                bytes,
                entry_offset(
                    header.segments.offset,
                    segment_index,
                    SEGMENT_BYTES,
                    "sparse segment",
                )?,
            )?;
            if segment.gene != gene.gene {
                return Err(IndexError::Corrupt("sparse segment gene ownership"));
            }
            gene_loci = checked_add(gene_loci, u64::from(segment.loci), "sparse gene loci")?;
        }
        if gene_loci != u64::from(gene.ordinary_loci) {
            return Err(IndexError::Corrupt("sparse gene ordinary count"));
        }
        ordinary_total = checked_add(ordinary_total, gene_loci, "sparse ordinary total")?;
        segment_cursor = segment_end;
        previous_gene = Some(gene.gene.numeric());
    }
    if segment_cursor != header.segment_count || ordinary_total != header.ordinary_loci {
        return Err(IndexError::Corrupt("sparse gene directory coverage"));
    }

    let mut block_cursor = 0_u64;
    let mut payload_cursor = 0_u64;
    let mut previous_segment: Option<SegmentView> = None;
    for segment_index in 0..header.segment_count {
        let segment = decode_segment(
            bytes,
            entry_offset(
                header.segments.offset,
                segment_index,
                SEGMENT_BYTES,
                "sparse segment",
            )?,
        )?;
        if let Some(previous) = previous_segment {
            let previous_key = (
                previous.gene.numeric(),
                previous.contig.code(),
                previous.start,
            );
            let key = (segment.gene.numeric(), segment.contig.code(), segment.start);
            if previous_key >= key {
                return Err(IndexError::Corrupt("sparse segment ordering"));
            }
            if previous.gene == segment.gene
                && previous.contig == segment.contig
                && previous.end >= segment.start
            {
                return Err(IndexError::Corrupt("overlapping sparse segments"));
            }
        }
        if u64::from(segment.block_start) != block_cursor {
            return Err(IndexError::Corrupt("sparse segment block ownership"));
        }
        let block_end = checked_add(
            block_cursor,
            u64::from(segment.block_count),
            "sparse segment block range",
        )?;
        if block_end > header.block_count {
            return Err(IndexError::Corrupt("sparse segment block range"));
        }
        let mut first = 0_u32;
        for block_index in block_cursor..block_end {
            let block = decode_block(
                bytes,
                entry_offset(
                    header.blocks.offset,
                    block_index,
                    BLOCK_BYTES,
                    "sparse block",
                )?,
            )?;
            if block.segment != segment_index || block.first != first {
                return Err(IndexError::Corrupt("sparse block ownership"));
            }
            if !(1..=BLOCK_LOCI).contains(&block.count) {
                return Err(IndexError::Corrupt("sparse block locus count"));
            }
            first = first
                .checked_add(block.count)
                .ok_or(IndexError::Arithmetic("sparse block locus coverage"))?;
            if block.payload_offset != payload_cursor {
                return Err(IndexError::Corrupt("sparse block payload coverage"));
            }
            let maximum_pairs = checked_mul(
                u64::from(block.active_count),
                6,
                "sparse maximum pair count",
            )?;
            if block.active_count > block.count || u64::from(block.pair_count) > maximum_pairs {
                return Err(IndexError::Corrupt("sparse block summary counts"));
            }
            let expected_len = expected_block_len(block)?;
            if expected_len != u64::from(block.payload_len) {
                return Err(IndexError::Corrupt("sparse block payload length"));
            }
            payload_cursor = checked_add(
                payload_cursor,
                u64::from(block.payload_len),
                "sparse payload coverage",
            )?;
            if payload_cursor > header.payload.len {
                return Err(IndexError::Corrupt("sparse block payload range"));
            }
        }
        if first != segment.loci {
            return Err(IndexError::Corrupt("sparse segment locus coverage"));
        }
        block_cursor = block_end;
        previous_segment = Some(segment);
    }
    if block_cursor != header.block_count || payload_cursor != header.payload.len {
        return Err(IndexError::Corrupt("sparse block directory coverage"));
    }

    let mut previous_exception = None;
    for index in 0..header.exception_count {
        let exception = decode_exception(
            bytes,
            entry_offset(
                header.exceptions.offset,
                index,
                EXCEPTION_BYTES,
                "sparse exception",
            )?,
        )?;
        let key = (
            exception.gene.numeric(),
            exception.contig.code(),
            exception.position.get(),
        );
        if previous_exception.is_some_and(|previous| previous >= key) {
            return Err(IndexError::Corrupt("sparse exception ordering"));
        }
        let gene_index = find_gene_in(bytes, header, exception.gene)?
            .ok_or(IndexError::Corrupt("sparse exception gene"))?;
        let gene = decode_gene(
            bytes,
            entry_offset(header.genes.offset, gene_index, GENE_BYTES, "sparse gene")?,
        )?;
        let segment_end = checked_add(
            gene.segment_start,
            u64::from(gene.segment_count),
            "sparse exception segment range",
        )?;
        for segment_index in gene.segment_start..segment_end {
            let segment = decode_segment(
                bytes,
                entry_offset(
                    header.segments.offset,
                    segment_index,
                    SEGMENT_BYTES,
                    "sparse segment",
                )?,
            )?;
            if segment.contig == exception.contig
                && segment.start <= exception.position.get()
                && exception.position.get() <= segment.end
            {
                return Err(IndexError::Corrupt(
                    "sparse exception overlaps ordinary locus",
                ));
            }
        }
        previous_exception = Some(key);
    }
    Ok(())
}

fn find_gene_in(
    bytes: &[u8],
    header: &Header,
    target: EnsemblGeneId,
) -> Result<Option<u64>, IndexError> {
    let mut low = 0_u64;
    let mut high = header.gene_count;
    while low < high {
        let middle = low + (high - low) / 2;
        let gene = decode_gene(
            bytes,
            entry_offset(header.genes.offset, middle, GENE_BYTES, "sparse gene")?,
        )?
        .gene;
        if gene.numeric() < target.numeric() {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if low < header.gene_count {
        let gene = decode_gene(
            bytes,
            entry_offset(header.genes.offset, low, GENE_BYTES, "sparse gene")?,
        )?
        .gene;
        if gene == target {
            return Ok(Some(low));
        }
    }
    Ok(None)
}

fn expected_block_len(block: BlockView) -> Result<u64, IndexError> {
    let references = div_ceil_u64(
        checked_mul(u64::from(block.count), 2, "sparse reference bits")?,
        8,
        "sparse reference bytes",
    )?;
    let active = div_ceil_u64(u64::from(block.count), 8, "sparse active bytes")?;
    let ranks = checked_mul(
        u64::from(div_ceil_u32(block.count, RANK_STRIDE)?),
        RANK_BYTES,
        "sparse rank bytes",
    )?;
    let masks = div_ceil_u64(
        checked_mul(u64::from(block.active_count), 6, "sparse mask bits")?,
        8,
        "sparse mask bytes",
    )?;
    let values = checked_mul(u64::from(block.pair_count), 2, "sparse score bytes")?;
    [BLOCK_HEADER_BYTES, references, active, ranks, masks, values]
        .into_iter()
        .try_fold(0_u64, |total, value| {
            checked_add(total, value, "sparse block length")
        })
}

fn decode_gene(bytes: &[u8], offset: u64) -> Result<GeneView, IndexError> {
    let row = fixed_row(bytes, offset, GENE_BYTES, "sparse gene row")?;
    require_zero(row, 24, 32, "sparse gene reserved")?;
    Ok(GeneView {
        gene: EnsemblGeneId::from_numeric(read_u64(row, 0)?)
            .map_err(|_| IndexError::Corrupt("sparse gene"))?,
        segment_start: read_u64(row, 8)?,
        segment_count: read_u32(row, 16)?,
        ordinary_loci: read_u32(row, 20)?,
    })
}

fn decode_segment(bytes: &[u8], offset: u64) -> Result<SegmentView, IndexError> {
    let row = fixed_row(bytes, offset, SEGMENT_BYTES, "sparse segment row")?;
    require_zero(row, 9, 16, "sparse segment reserved")?;
    require_zero(row, 36, 48, "sparse segment reserved")?;
    let start = read_u32(row, 16)?;
    let end = read_u32(row, 20)?;
    let loci = read_u32(row, 24)?;
    if loci == 0
        || start == 0
        || end
            != start
                .checked_add(loci - 1)
                .ok_or(IndexError::Arithmetic("sparse segment end"))?
    {
        return Err(IndexError::Corrupt("sparse segment coordinates"));
    }
    if read_u32(row, 32)? == 0 {
        return Err(IndexError::Corrupt("sparse segment block count"));
    }
    Ok(SegmentView {
        gene: EnsemblGeneId::from_numeric(read_u64(row, 0)?)
            .map_err(|_| IndexError::Corrupt("sparse segment gene"))?,
        contig: Grch38Contig::from_code(row[8])
            .map_err(|_| IndexError::Corrupt("sparse segment contig"))?,
        start,
        end,
        loci,
        block_start: read_u32(row, 28)?,
        block_count: read_u32(row, 32)?,
    })
}

fn decode_block(bytes: &[u8], offset: u64) -> Result<BlockView, IndexError> {
    let row = fixed_row(bytes, offset, BLOCK_BYTES, "sparse block row")?;
    require_zero(row, 36, 40, "sparse block reserved")?;
    Ok(BlockView {
        segment: read_u64(row, 0)?,
        first: read_u32(row, 8)?,
        count: read_u32(row, 12)?,
        payload_offset: read_u64(row, 16)?,
        payload_len: read_u32(row, 24)?,
        active_count: read_u32(row, 28)?,
        pair_count: read_u32(row, 32)?,
    })
}

fn decode_exception(bytes: &[u8], offset: u64) -> Result<AmbiguousInputLocus, IndexError> {
    let row = fixed_row(bytes, offset, EXCEPTION_BYTES, "sparse exception row")?;
    require_zero(row, 5, 8, "sparse exception reserved")?;
    require_zero(row, 20, 24, "sparse exception reserved")?;
    require_zero(row, 36, 40, "sparse exception reserved")?;
    let contig = Grch38Contig::from_code(row[0])
        .map_err(|_| IndexError::Corrupt("sparse exception contig"))?;
    let omitted = decode_base(row[1])?;
    if !matches!(omitted, DnaBase::A | DnaBase::T) {
        return Err(IndexError::Corrupt("sparse exception omitted base"));
    }
    let expected = canonical_alternates(omitted);
    let decode_alternative = |index: usize| -> Result<InputAlternative, IndexError> {
        let alternate = decode_base(row[2 + index])?;
        let gain = decode_pair(read_u16(row, 24 + index * 4)?)?;
        let loss = decode_pair(read_u16(row, 26 + index * 4)?)?;
        Ok(InputAlternative {
            alternate,
            score: PangolinScore::new(gain.0, gain.1, loss.0, loss.1),
        })
    };
    let alternatives = [
        decode_alternative(0)?,
        decode_alternative(1)?,
        decode_alternative(2)?,
    ];
    if alternatives.map(|value| value.alternate) != expected {
        return Err(IndexError::Corrupt("sparse exception alternatives"));
    }
    Ok(AmbiguousInputLocus {
        gene: EnsemblGeneId::from_numeric(read_u64(row, 8)?)
            .map_err(|_| IndexError::Corrupt("sparse exception gene"))?,
        contig,
        position: GenomicPosition::new(read_u32(row, 16)?)
            .map_err(|_| IndexError::Corrupt("sparse exception coordinate"))?,
        alternatives,
        omitted,
    })
}

fn decode_ordinary(
    layout: &BlockLayout<'_>,
    block: BlockView,
    ordinal: u32,
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    position: u32,
) -> Result<OrdinaryInputLocus, IndexError> {
    let reference = reference_at(layout, ordinal)?;
    let (mask, pair_start) = mask_and_pair_start(layout, block, ordinal)?;
    let mut pair = pair_start;
    let default_component = (
        ScoreMagnitude::new(0).map_err(|_| IndexError::Corrupt("sparse default score"))?,
        RelativePosition::new(-50).map_err(|_| IndexError::Corrupt("sparse default position"))?,
    );
    let default_score = PangolinScore::new(
        default_component.0,
        default_component.1,
        default_component.0,
        default_component.1,
    );
    let mut alternatives = canonical_alternates(reference).map(|alternate| InputAlternative {
        alternate,
        score: default_score,
    });
    for (alternative_index, alternative) in alternatives.iter_mut().enumerate() {
        let mut components = [default_component; 2];
        for kind in 0..2_u32 {
            let mask_bit = u32::try_from(alternative_index)
                .map_err(|_| IndexError::Arithmetic("sparse alternative index"))?
                .checked_mul(2)
                .and_then(|value| value.checked_add(kind))
                .ok_or(IndexError::Arithmetic("sparse alternative mask bit"))?;
            if mask & (1 << mask_bit) != 0 {
                components[kind as usize] = decode_pair_at(layout, pair)?;
                pair = pair
                    .checked_add(1)
                    .ok_or(IndexError::Arithmetic("sparse locus pair index"))?;
            }
        }
        alternative.score = PangolinScore::new(
            components[0].0,
            components[0].1,
            components[1].0,
            components[1].1,
        );
    }
    Ok(OrdinaryInputLocus {
        gene,
        contig,
        position: GenomicPosition::new(position)
            .map_err(|_| IndexError::Corrupt("sparse ordinary coordinate"))?,
        reference,
        alternatives,
    })
}

fn canonical_alternates(excluded: DnaBase) -> [DnaBase; 3] {
    match excluded {
        DnaBase::A => [DnaBase::C, DnaBase::G, DnaBase::T],
        DnaBase::C => [DnaBase::A, DnaBase::G, DnaBase::T],
        DnaBase::G => [DnaBase::A, DnaBase::C, DnaBase::T],
        DnaBase::T => [DnaBase::A, DnaBase::C, DnaBase::G],
    }
}

fn mask_and_pair_start(
    layout: &BlockLayout<'_>,
    block: BlockView,
    ordinal: u32,
) -> Result<(u8, u32), IndexError> {
    if ordinal >= block.count {
        return Err(IndexError::Corrupt("sparse block ordinal"));
    }
    let rank_index = ordinal / RANK_STRIDE;
    let rank_offset = add_usize_u64(
        layout.ranks_start,
        checked_mul(u64::from(rank_index), RANK_BYTES, "sparse rank offset")?,
    )?;
    let mut active = layout.payload.u32(rank_offset)?;
    let mut pairs = layout.payload.u32(rank_offset + 4)?;
    let first = rank_index * RANK_STRIDE;
    for current in first..ordinal {
        if is_active(layout, current)? {
            let mask = mask_at(layout, active)?;
            active = active
                .checked_add(1)
                .ok_or(IndexError::Arithmetic("sparse active index"))?;
            pairs = pairs
                .checked_add(mask.count_ones())
                .ok_or(IndexError::Arithmetic("sparse pair index"))?;
        }
    }
    if !is_active(layout, ordinal)? {
        return Ok((0, pairs));
    }
    Ok((mask_at(layout, active)?, pairs))
}

fn reference_at(layout: &BlockLayout<'_>, ordinal: u32) -> Result<DnaBase, IndexError> {
    let bit = u64::from(ordinal) * 2;
    let byte = add_usize_u64(layout.references_start, bit / 8)?;
    let value = (layout.payload.byte(byte)? >> (bit % 8)) & 3;
    decode_base(value)
}

fn is_active(layout: &BlockLayout<'_>, ordinal: u32) -> Result<bool, IndexError> {
    let byte = add_usize_u64(layout.active_start, u64::from(ordinal / 8))?;
    Ok(layout.payload.byte(byte)? & (1 << (ordinal % 8)) != 0)
}

fn mask_at(layout: &BlockLayout<'_>, active: u32) -> Result<u8, IndexError> {
    let bit = u64::from(active) * 6;
    let byte = add_usize_u64(layout.masks_start, bit / 8)?;
    let low = u16::from(layout.payload.byte(byte)?);
    let high = if byte + 1 < layout.values_start {
        u16::from(layout.payload.byte(byte + 1)?) << 8
    } else {
        0
    };
    Ok(((low | high) >> (bit % 8)) as u8 & 0x3f)
}

fn decode_pair_at(
    layout: &BlockLayout<'_>,
    pair: u32,
) -> Result<(ScoreMagnitude, RelativePosition), IndexError> {
    let offset = add_usize_u64(layout.values_start, u64::from(pair) * 2)?;
    decode_pair(layout.payload.u16(offset)?)
}

fn decode_pair(value: u16) -> Result<(ScoreMagnitude, RelativePosition), IndexError> {
    if value >> 14 != 0 {
        return Err(IndexError::Corrupt("sparse score reserved bits"));
    }
    let magnitude = ScoreMagnitude::new(value & 0x7f)
        .map_err(|_| IndexError::Corrupt("sparse score magnitude"))?;
    let position_code = (value >> 7) & 0x7f;
    if position_code > 100 {
        return Err(IndexError::Corrupt("sparse score position"));
    }
    let position = RelativePosition::new(position_code as i16 - 50)
        .map_err(|_| IndexError::Corrupt("sparse score position"))?;
    Ok((magnitude, position))
}

fn decode_base(value: u8) -> Result<DnaBase, IndexError> {
    match value {
        0 => Ok(DnaBase::A),
        1 => Ok(DnaBase::C),
        2 => Ok(DnaBase::G),
        3 => Ok(DnaBase::T),
        _ => Err(IndexError::Corrupt("sparse base code")),
    }
}

fn section(bytes: &[u8], offset: usize) -> Result<Section, IndexError> {
    Ok(Section {
        offset: read_u64(bytes, offset)?,
        len: read_u64(bytes, offset + 8)?,
    })
}

fn fixed_row<'a>(
    bytes: &'a [u8],
    offset: u64,
    len: u64,
    reason: &'static str,
) -> Result<&'a [u8], IndexError> {
    let start = usize_from_u64(offset, reason)?;
    let row_len = usize_from_u64(len, reason)?;
    let end = start
        .checked_add(row_len)
        .ok_or(IndexError::Arithmetic(reason))?;
    bytes.get(start..end).ok_or(IndexError::Corrupt(reason))
}

fn entry_offset(
    section: u64,
    index: u64,
    width: u64,
    reason: &'static str,
) -> Result<u64, IndexError> {
    checked_add(section, checked_mul(index, width, reason)?, reason)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, IndexError> {
    let end = offset
        .checked_add(2)
        .ok_or(IndexError::Arithmetic("sparse u16 offset"))?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(IndexError::Corrupt("truncated sparse u16"))?
            .try_into()
            .map_err(|_| IndexError::Corrupt("truncated sparse u16"))?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, IndexError> {
    let end = offset
        .checked_add(4)
        .ok_or(IndexError::Arithmetic("sparse u32 offset"))?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(IndexError::Corrupt("truncated sparse u32"))?
            .try_into()
            .map_err(|_| IndexError::Corrupt("truncated sparse u32"))?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, IndexError> {
    let end = offset
        .checked_add(8)
        .ok_or(IndexError::Arithmetic("sparse u64 offset"))?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(IndexError::Corrupt("truncated sparse u64"))?
            .try_into()
            .map_err(|_| IndexError::Corrupt("truncated sparse u64"))?,
    ))
}

fn require_zero(
    bytes: &[u8],
    start: usize,
    end: usize,
    reason: &'static str,
) -> Result<(), IndexError> {
    if !bytes
        .get(start..end)
        .ok_or(IndexError::Corrupt(reason))?
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(IndexError::Corrupt(reason));
    }
    Ok(())
}

fn validate_payload_tail_bits(
    payload: Payload<'_>,
    start: usize,
    end: usize,
    used_bits: u64,
    reason: &'static str,
) -> Result<(), IndexError> {
    if start > end || end > payload.len() {
        return Err(IndexError::Corrupt(reason));
    }
    let remainder = used_bits % 8;
    if remainder != 0 {
        let last_offset = end.checked_sub(1).ok_or(IndexError::Corrupt(reason))?;
        if last_offset < start {
            return Err(IndexError::Corrupt(reason));
        }
        let last = payload.byte(last_offset)?;
        let used_mask = (1_u16 << remainder) - 1;
        if last & !(used_mask as u8) != 0 {
            return Err(IndexError::Corrupt(reason));
        }
    }
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

fn div_ceil_u64(value: u64, divisor: u64, reason: &'static str) -> Result<u64, IndexError> {
    value
        .checked_add(divisor - 1)
        .ok_or(IndexError::Arithmetic(reason))
        .map(|sum| sum / divisor)
}

fn div_ceil_u32(value: u32, divisor: u32) -> Result<u32, IndexError> {
    value
        .checked_add(divisor - 1)
        .ok_or(IndexError::Arithmetic("sparse division ceiling"))
        .map(|sum| sum / divisor)
}

fn usize_from_u64(value: u64, reason: &'static str) -> Result<usize, IndexError> {
    usize::try_from(value).map_err(|_| IndexError::Arithmetic(reason))
}

fn add_usize_u64(left: usize, right: u64) -> Result<usize, IndexError> {
    left.checked_add(usize_from_u64(right, "sparse byte offset")?)
        .ok_or(IndexError::Arithmetic("sparse byte offset"))
}
