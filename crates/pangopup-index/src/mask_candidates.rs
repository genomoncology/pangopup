//! Closed, benchmark-only GENCODE mask encodings for Ticket 012.
//!
//! These layouts deliberately use a non-production magic and are public only
//! so the private maintenance builder and benchmark can exercise identical
//! bytes. Runtime consumers must wait for the selected layout to be hardened
//! under a separately versioned production format.

use memmap2::Mmap;
use pangopup_core::{EnsemblGeneId, GencodeGeneId, GenomicPosition, Grch38Contig, ValueError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{File, OpenOptions},
    io::{self, Write},
    ops::Range,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

const MAGIC: &[u8; 8] = b"PGMBEN01";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 160;
const DIRECTORY_BYTES: usize = 40;
const GENE_BYTES: usize = 40;
const INDEX_BYTES: usize = 16;
const SECTION_BYTES: usize = 24;
const SECTION_COUNT: usize = 5;
const NONE: u32 = u32::MAX;
const BIN_SHIFT: u32 = 16;
const MAX_CONTIGS: usize = 25;
const MAX_GENES: usize = 100_000;
const MAX_BOUNDARIES: usize = 10_000_000;
const MAX_BOUNDARIES_PER_GENE: usize = 100_000;
const MAX_POSTINGS: usize = 10_000_000;
const MAX_MEMBER_BYTES: usize = 512 * 1024 * 1024;
const MAX_EXACT_ID_BYTES: usize = 64;
const LOGICAL_PAGE_BYTES: usize = 4_096;
const CANCELLATION_WORK_INTERVAL: usize = 1_024;
const WRITE_CHUNK_BYTES: usize = 1024 * 1024;

/// Strand used to choose Pangolin's plus or minus model output.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaskStrand {
    Plus,
    Minus,
}

impl MaskStrand {
    const fn flag(self) -> u8 {
        match self {
            Self::Plus => 0,
            Self::Minus => 1,
        }
    }

    fn from_flag(value: u8) -> Result<Self, MaskCandidateError> {
        match value {
            0 => Ok(Self::Plus),
            1 => Ok(Self::Minus),
            _ => Err(MaskCandidateError::Corrupt("strand")),
        }
    }
}

/// One record in the candidate-independent canonical mask stream.
///
/// `start` and `end` retain the inclusive GTF span. Membership deliberately
/// follows Pangolin's effective `(start, end]` rule. Exon boundaries are a
/// sorted set because masking uses only boundary membership: assignment and
/// `isin` are invariant to source order and duplicates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMaskGene {
    identity: GencodeGeneId,
    contig: Grch38Contig,
    strand: MaskStrand,
    start: GenomicPosition,
    end: GenomicPosition,
    query_rank: u32,
    boundaries: Vec<GenomicPosition>,
}

impl CanonicalMaskGene {
    pub fn new(
        identity: GencodeGeneId,
        contig: Grch38Contig,
        strand: MaskStrand,
        start: GenomicPosition,
        end: GenomicPosition,
        query_rank: u32,
        mut boundaries: Vec<GenomicPosition>,
    ) -> Result<Self, MaskCandidateError> {
        if start > end {
            return Err(MaskCandidateError::Input("gene span"));
        }
        if identity.to_string().len() > MAX_EXACT_ID_BYTES {
            return Err(MaskCandidateError::Resource("gene identity"));
        }
        if boundaries.len() > MAX_BOUNDARIES_PER_GENE {
            return Err(MaskCandidateError::Resource("gene boundaries"));
        }
        if boundaries
            .iter()
            .any(|boundary| *boundary < start || *boundary > end)
        {
            return Err(MaskCandidateError::Input("exon boundary span"));
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Ok(Self {
            identity,
            contig,
            strand,
            start,
            end,
            query_rank,
            boundaries,
        })
    }

    pub const fn identity(&self) -> GencodeGeneId {
        self.identity
    }

    pub const fn stable_identity(&self) -> EnsemblGeneId {
        self.identity.stable()
    }

    pub const fn contig(&self) -> Grch38Contig {
        self.contig
    }

    pub const fn strand(&self) -> MaskStrand {
        self.strand
    }

    pub const fn start(&self) -> GenomicPosition {
        self.start
    }

    pub const fn end(&self) -> GenomicPosition {
        self.end
    }

    pub const fn query_rank(&self) -> u32 {
        self.query_rank
    }

    pub fn boundaries(&self) -> &[GenomicPosition] {
        &self.boundaries
    }

    pub fn contains(&self, position: GenomicPosition) -> bool {
        position > self.start && position <= self.end
    }
}

/// The three exact representations compared by the Ticket 012 harness.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaskCandidateCodec {
    IntervalTree,
    Domains,
    BinnedPostings,
}

impl MaskCandidateCodec {
    pub const ALL: [Self; 3] = [Self::IntervalTree, Self::Domains, Self::BinnedPostings];

    pub const fn code(self) -> u8 {
        match self {
            Self::IntervalTree => 1,
            Self::Domains => 2,
            Self::BinnedPostings => 3,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::IntervalTree => "interval-tree",
            Self::Domains => "domains",
            Self::BinnedPostings => "binned-postings",
        }
    }

    pub const fn filename(self) -> &'static str {
        match self {
            Self::IntervalTree => "interval-tree.pgm",
            Self::Domains => "domains.pgm",
            Self::BinnedPostings => "binned-postings.pgm",
        }
    }

    fn from_code(value: u8) -> Result<Self, MaskCandidateError> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.code() == value)
            .ok_or(MaskCandidateError::Corrupt("candidate codec"))
    }
}

impl fmt::Display for MaskCandidateCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Debug)]
pub enum MaskCandidateError {
    Io(io::Error),
    Cancelled,
    Input(&'static str),
    Corrupt(&'static str),
    Bounds(&'static str),
    Resource(&'static str),
    Arithmetic(&'static str),
}

impl fmt::Display for MaskCandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("mask candidate I/O failed"),
            Self::Cancelled => formatter.write_str("mask candidate operation cancelled"),
            Self::Input(reason) => write!(formatter, "invalid logical mask input: {reason}"),
            Self::Corrupt(reason) => write!(formatter, "invalid mask candidate: {reason}"),
            Self::Bounds(reason) => write!(formatter, "mask query out of bounds: {reason}"),
            Self::Resource(reason) => write!(formatter, "mask candidate resource limit: {reason}"),
            Self::Arithmetic(reason) => {
                write!(formatter, "mask candidate arithmetic overflow: {reason}")
            }
        }
    }
}

impl std::error::Error for MaskCandidateError {}

impl From<io::Error> for MaskCandidateError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

struct CancellationCheck<'a> {
    requested: &'a dyn Fn() -> bool,
    work_since_check: usize,
}

impl<'a> CancellationCheck<'a> {
    fn new(requested: &'a dyn Fn() -> bool) -> Result<Self, MaskCandidateError> {
        let result = Self {
            requested,
            work_since_check: 0,
        };
        result.check_now()?;
        Ok(result)
    }

    fn check_now(&self) -> Result<(), MaskCandidateError> {
        if (self.requested)() {
            Err(MaskCandidateError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn account(&mut self, work: usize) -> Result<(), MaskCandidateError> {
        self.work_since_check = self.work_since_check.saturating_add(work);
        if self.work_since_check >= CANCELLATION_WORK_INTERVAL {
            self.work_since_check = 0;
            self.check_now()?;
        }
        Ok(())
    }
}

fn never_cancelled() -> bool {
    false
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DirectoryEntry {
    contig: u8,
    gene_start: u64,
    gene_count: u64,
    index_start: u64,
    index_count: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Section {
    offset: u64,
    count: u64,
    stride: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TreeNode {
    gene: u32,
    max_end: u32,
    left: u32,
    right: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DomainEntry {
    begin: u32,
    end: u32,
    posting_start: u32,
    posting_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BinEntry {
    bin: u32,
    posting_start: u32,
    posting_count: u32,
}

#[derive(Default)]
struct Layout {
    directories: Vec<DirectoryEntry>,
    trees: Vec<TreeNode>,
    domains: Vec<DomainEntry>,
    bins: Vec<BinEntry>,
    postings: Vec<u32>,
}

/// Write one closed benchmark member. The path must not already exist.
pub fn write_mask_candidate(
    path: &Path,
    codec: MaskCandidateCodec,
    genes: &[CanonicalMaskGene],
) -> Result<u64, MaskCandidateError> {
    write_mask_candidate_with_cancellation(path, codec, genes, &never_cancelled)
}

/// Write one candidate while polling a caller-owned cancellation predicate.
///
/// The predicate is checked before work begins and after bounded units of
/// validation, layout construction, encoding, and file output. Cancellation
/// is reported distinctly as [`MaskCandidateError::Cancelled`]. Point-query
/// decoding never consults this hook.
pub fn write_mask_candidate_with_cancellation(
    path: &Path,
    codec: MaskCandidateCodec,
    genes: &[CanonicalMaskGene],
    cancellation_requested: &dyn Fn() -> bool,
) -> Result<u64, MaskCandidateError> {
    let mut cancellation = CancellationCheck::new(cancellation_requested)?;
    let bytes = encode_candidate(codec, genes, &mut cancellation)?;
    cancellation.check_now()?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    for chunk in bytes.chunks(WRITE_CHUNK_BYTES) {
        cancellation.check_now()?;
        file.write_all(chunk)?;
        cancellation.account(chunk.len())?;
    }
    file.sync_all()?;
    cancellation.check_now()?;
    u64::try_from(bytes.len()).map_err(|_| MaskCandidateError::Arithmetic("member size"))
}

fn encode_candidate(
    codec: MaskCandidateCodec,
    genes: &[CanonicalMaskGene],
    cancellation: &mut CancellationCheck<'_>,
) -> Result<Vec<u8>, MaskCandidateError> {
    validate_stream(genes, cancellation)?;
    let layout = build_layout(codec, genes, cancellation)?;
    let mut boundary_count = 0_usize;
    for gene in genes {
        cancellation.account(1)?;
        boundary_count = boundary_count
            .checked_add(gene.boundaries.len())
            .ok_or(MaskCandidateError::Arithmetic("boundary count"))?;
    }
    if boundary_count > MAX_BOUNDARIES {
        return Err(MaskCandidateError::Resource("boundary count"));
    }
    let index_count = match codec {
        MaskCandidateCodec::IntervalTree => layout.trees.len(),
        MaskCandidateCodec::Domains => layout.domains.len(),
        MaskCandidateCodec::BinnedPostings => layout.bins.len(),
    };
    let descriptors = [
        (layout.directories.len(), DIRECTORY_BYTES),
        (genes.len(), GENE_BYTES),
        (boundary_count, 4),
        (index_count, INDEX_BYTES),
        (layout.postings.len(), 4),
    ];
    let mut sections = [Section::default(); SECTION_COUNT];
    let mut cursor = HEADER_BYTES;
    for (slot, (count, stride)) in sections.iter_mut().zip(descriptors) {
        cancellation.account(1)?;
        let bytes = count
            .checked_mul(stride)
            .ok_or(MaskCandidateError::Arithmetic("section size"))?;
        *slot = Section {
            offset: u64::try_from(cursor)
                .map_err(|_| MaskCandidateError::Arithmetic("section offset"))?,
            count: u64::try_from(count)
                .map_err(|_| MaskCandidateError::Arithmetic("section count"))?,
            stride: u32::try_from(stride)
                .map_err(|_| MaskCandidateError::Arithmetic("section stride"))?,
        };
        cursor = cursor
            .checked_add(bytes)
            .ok_or(MaskCandidateError::Arithmetic("member size"))?;
    }
    if cursor > MAX_MEMBER_BYTES {
        return Err(MaskCandidateError::Resource("member bytes"));
    }

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(cursor)
        .map_err(|_| MaskCandidateError::Resource("member bytes"))?;
    while bytes.len() < cursor {
        cancellation.check_now()?;
        let next = bytes.len().saturating_add(WRITE_CHUNK_BYTES).min(cursor);
        bytes.resize(next, 0);
    }
    bytes[0..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    bytes[10] = codec.code();
    bytes[11] = u8::try_from(layout.directories.len())
        .map_err(|_| MaskCandidateError::Resource("contig count"))?;
    put_u64(&mut bytes, 16, cursor as u64)?;
    put_u64(&mut bytes, 24, HEADER_BYTES as u64)?;
    for (index, section) in sections.iter().enumerate() {
        let offset = 32 + index * SECTION_BYTES;
        put_u64(&mut bytes, offset, section.offset)?;
        put_u64(&mut bytes, offset + 8, section.count)?;
        put_u32(&mut bytes, offset + 16, section.stride)?;
    }

    for (index, entry) in layout.directories.iter().enumerate() {
        cancellation.account(1)?;
        let offset = section_offset(&sections[0], index, DIRECTORY_BYTES)?;
        bytes[offset] = entry.contig;
        put_u64(&mut bytes, offset + 8, entry.gene_start)?;
        put_u64(&mut bytes, offset + 16, entry.gene_count)?;
        put_u64(&mut bytes, offset + 24, entry.index_start)?;
        put_u64(&mut bytes, offset + 32, entry.index_count)?;
    }

    let mut boundary_cursor = 0_u32;
    for (index, gene) in genes.iter().enumerate() {
        cancellation.account(1)?;
        let offset = section_offset(&sections[1], index, GENE_BYTES)?;
        bytes[offset] = gene.contig.code();
        bytes[offset + 1] = gene.strand.flag() | u8::from(gene.identity.is_par_y()) << 1;
        put_u32(&mut bytes, offset + 4, gene.start.get())?;
        put_u32(&mut bytes, offset + 8, gene.end.get())?;
        put_u32(&mut bytes, offset + 12, gene.query_rank)?;
        put_u64(&mut bytes, offset + 16, gene.identity.stable().numeric())?;
        put_u32(&mut bytes, offset + 24, gene.identity.version().get())?;
        put_u32(&mut bytes, offset + 28, boundary_cursor)?;
        let count = u32::try_from(gene.boundaries.len())
            .map_err(|_| MaskCandidateError::Resource("gene boundaries"))?;
        put_u32(&mut bytes, offset + 32, count)?;
        boundary_cursor = boundary_cursor
            .checked_add(count)
            .ok_or(MaskCandidateError::Arithmetic("boundary cursor"))?;
    }
    let mut index = 0_usize;
    for gene in genes {
        for boundary in &gene.boundaries {
            cancellation.account(1)?;
            let offset = section_offset(&sections[2], index, 4)?;
            put_u32(&mut bytes, offset, boundary.get())?;
            index += 1;
        }
    }

    match codec {
        MaskCandidateCodec::IntervalTree => {
            for (index, node) in layout.trees.iter().enumerate() {
                cancellation.account(1)?;
                let offset = section_offset(&sections[3], index, INDEX_BYTES)?;
                put_u32(&mut bytes, offset, node.gene)?;
                put_u32(&mut bytes, offset + 4, node.max_end)?;
                put_u32(&mut bytes, offset + 8, node.left)?;
                put_u32(&mut bytes, offset + 12, node.right)?;
            }
        }
        MaskCandidateCodec::Domains => {
            for (index, domain) in layout.domains.iter().enumerate() {
                cancellation.account(1)?;
                let offset = section_offset(&sections[3], index, INDEX_BYTES)?;
                put_u32(&mut bytes, offset, domain.begin)?;
                put_u32(&mut bytes, offset + 4, domain.end)?;
                put_u32(&mut bytes, offset + 8, domain.posting_start)?;
                put_u32(&mut bytes, offset + 12, domain.posting_count)?;
            }
        }
        MaskCandidateCodec::BinnedPostings => {
            for (index, bin) in layout.bins.iter().enumerate() {
                cancellation.account(1)?;
                let offset = section_offset(&sections[3], index, INDEX_BYTES)?;
                put_u32(&mut bytes, offset, bin.bin)?;
                put_u32(&mut bytes, offset + 8, bin.posting_start)?;
                put_u32(&mut bytes, offset + 12, bin.posting_count)?;
            }
        }
    }
    for (index, posting) in layout.postings.iter().enumerate() {
        cancellation.account(1)?;
        let offset = section_offset(&sections[4], index, 4)?;
        put_u32(&mut bytes, offset, *posting)?;
    }
    cancellation.check_now()?;
    Ok(bytes)
}

fn validate_stream(
    genes: &[CanonicalMaskGene],
    cancellation: &mut CancellationCheck<'_>,
) -> Result<(), MaskCandidateError> {
    if genes.is_empty() || genes.len() > MAX_GENES {
        return Err(MaskCandidateError::Resource("gene count"));
    }
    let mut contigs = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut prior: Option<(u8, u32)> = None;
    let mut boundaries = 0_usize;
    for gene in genes {
        cancellation.account(1)?;
        let key = (gene.contig.code(), gene.query_rank);
        if prior.is_some_and(|value| key <= value) {
            return Err(MaskCandidateError::Input("canonical stream order"));
        }
        if !identities.insert(gene.identity) {
            return Err(MaskCandidateError::Input("duplicate exact gene identity"));
        }
        prior = Some(key);
        contigs.insert(gene.contig.code());
        boundaries = boundaries
            .checked_add(gene.boundaries.len())
            .ok_or(MaskCandidateError::Arithmetic("boundary count"))?;
        if boundaries > MAX_BOUNDARIES {
            return Err(MaskCandidateError::Resource("boundary count"));
        }
    }
    if contigs.len() > MAX_CONTIGS {
        return Err(MaskCandidateError::Resource("contig count"));
    }
    Ok(())
}

fn build_layout(
    codec: MaskCandidateCodec,
    genes: &[CanonicalMaskGene],
    cancellation: &mut CancellationCheck<'_>,
) -> Result<Layout, MaskCandidateError> {
    let mut layout = Layout::default();
    let mut start = 0_usize;
    while start < genes.len() {
        cancellation.account(1)?;
        let code = genes[start].contig.code();
        let mut end = start + 1;
        while end < genes.len() && genes[end].contig.code() == code {
            cancellation.account(1)?;
            end += 1;
        }
        let index_start = match codec {
            MaskCandidateCodec::IntervalTree => layout.trees.len(),
            MaskCandidateCodec::Domains => layout.domains.len(),
            MaskCandidateCodec::BinnedPostings => layout.bins.len(),
        };
        match codec {
            MaskCandidateCodec::IntervalTree => {
                let mut indices = Vec::with_capacity(end - start);
                for index in start..end {
                    cancellation.account(1)?;
                    indices.push(
                        u32::try_from(index)
                            .map_err(|_| MaskCandidateError::Resource("gene index"))?,
                    );
                }
                cancellation.check_now()?;
                indices.sort_unstable_by_key(|index| {
                    let gene = &genes[*index as usize];
                    (gene.start.get(), gene.end.get(), gene.query_rank)
                });
                cancellation.check_now()?;
                append_tree(&indices, genes, &mut layout.trees, cancellation)?;
            }
            MaskCandidateCodec::Domains => {
                append_domains(start, end, genes, &mut layout, cancellation)?;
            }
            MaskCandidateCodec::BinnedPostings => {
                append_bins(start, end, genes, &mut layout, cancellation)?;
            }
        }
        let index_end = match codec {
            MaskCandidateCodec::IntervalTree => layout.trees.len(),
            MaskCandidateCodec::Domains => layout.domains.len(),
            MaskCandidateCodec::BinnedPostings => layout.bins.len(),
        };
        layout.directories.push(DirectoryEntry {
            contig: code,
            gene_start: start as u64,
            gene_count: (end - start) as u64,
            index_start: index_start as u64,
            index_count: (index_end - index_start) as u64,
        });
        start = end;
    }
    if layout.postings.len() > MAX_POSTINGS {
        return Err(MaskCandidateError::Resource("posting count"));
    }
    Ok(layout)
}

fn append_tree(
    indices: &[u32],
    genes: &[CanonicalMaskGene],
    nodes: &mut Vec<TreeNode>,
    cancellation: &mut CancellationCheck<'_>,
) -> Result<(u32, u32), MaskCandidateError> {
    cancellation.account(1)?;
    if indices.is_empty() {
        return Ok((NONE, 0));
    }
    let middle = indices.len() / 2;
    let gene_index = indices[middle];
    let node_index = u32::try_from(nodes.len())
        .map_err(|_| MaskCandidateError::Resource("interval node count"))?;
    nodes.push(TreeNode::default());
    let (left, left_max) = append_tree(&indices[..middle], genes, nodes, cancellation)?;
    let (right, right_max) = append_tree(&indices[middle + 1..], genes, nodes, cancellation)?;
    let gene = genes
        .get(gene_index as usize)
        .ok_or(MaskCandidateError::Input("interval gene"))?;
    let max_end = gene.end.get().max(left_max).max(right_max);
    nodes[node_index as usize] = TreeNode {
        gene: gene_index,
        max_end,
        left,
        right,
    };
    Ok((node_index, max_end))
}

#[derive(Default)]
struct Events {
    add: Vec<u32>,
    remove: Vec<u32>,
}

fn append_domains(
    start: usize,
    end: usize,
    genes: &[CanonicalMaskGene],
    layout: &mut Layout,
    cancellation: &mut CancellationCheck<'_>,
) -> Result<(), MaskCandidateError> {
    let mut events: BTreeMap<u32, Events> = BTreeMap::new();
    for (index, gene) in genes.iter().enumerate().take(end).skip(start) {
        cancellation.account(1)?;
        if gene.start == gene.end {
            continue;
        }
        let index = u32::try_from(index).map_err(|_| MaskCandidateError::Resource("gene index"))?;
        let begin = gene
            .start
            .get()
            .checked_add(1)
            .ok_or(MaskCandidateError::Arithmetic("effective start"))?;
        events.entry(begin).or_default().add.push(index);
        if let Some(after) = gene.end.get().checked_add(1) {
            events.entry(after).or_default().remove.push(index);
        }
    }
    let mut positions = Vec::with_capacity(events.len());
    for position in events.keys().copied() {
        cancellation.account(1)?;
        positions.push(position);
    }
    let mut active = BTreeSet::new();
    for (event_index, position) in positions.iter().copied().enumerate() {
        cancellation.account(1)?;
        let event = events
            .get(&position)
            .ok_or(MaskCandidateError::Input("domain event"))?;
        for index in &event.remove {
            cancellation.account(1)?;
            let gene = genes
                .get(*index as usize)
                .ok_or(MaskCandidateError::Input("domain removal"))?;
            active.remove(&(gene.query_rank, *index));
        }
        for index in &event.add {
            cancellation.account(1)?;
            let gene = genes
                .get(*index as usize)
                .ok_or(MaskCandidateError::Input("domain addition"))?;
            active.insert((gene.query_rank, *index));
        }
        if active.is_empty() {
            continue;
        }
        let domain_end = positions
            .get(event_index + 1)
            .map_or(u32::MAX, |next| next - 1);
        let posting_start = u32::try_from(layout.postings.len())
            .map_err(|_| MaskCandidateError::Resource("posting count"))?;
        for (_, index) in &active {
            cancellation.account(1)?;
            layout.postings.push(*index);
        }
        let posting_count = u32::try_from(active.len())
            .map_err(|_| MaskCandidateError::Resource("domain cardinality"))?;
        layout.domains.push(DomainEntry {
            begin: position,
            end: domain_end,
            posting_start,
            posting_count,
        });
    }
    Ok(())
}

fn append_bins(
    start: usize,
    end: usize,
    genes: &[CanonicalMaskGene],
    layout: &mut Layout,
    cancellation: &mut CancellationCheck<'_>,
) -> Result<(), MaskCandidateError> {
    let mut bins: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut posting_count = 0_usize;
    for (index, gene) in genes.iter().enumerate().take(end).skip(start) {
        cancellation.account(1)?;
        if gene.start == gene.end {
            continue;
        }
        let index = u32::try_from(index).map_err(|_| MaskCandidateError::Resource("gene index"))?;
        let first = gene.start.get() >> BIN_SHIFT;
        let last = gene
            .end
            .get()
            .checked_sub(1)
            .ok_or(MaskCandidateError::Arithmetic("bin end"))?
            >> BIN_SHIFT;
        for bin in first..=last {
            cancellation.account(1)?;
            bins.entry(bin).or_default().push(index);
            posting_count = posting_count
                .checked_add(1)
                .ok_or(MaskCandidateError::Arithmetic("posting count"))?;
            if posting_count > MAX_POSTINGS {
                return Err(MaskCandidateError::Resource("posting count"));
            }
        }
    }
    for (bin, mut indices) in bins {
        cancellation.account(1)?;
        cancellation.check_now()?;
        indices.sort_unstable_by_key(|index| genes[*index as usize].query_rank);
        cancellation.check_now()?;
        let posting_start = u32::try_from(layout.postings.len())
            .map_err(|_| MaskCandidateError::Resource("posting count"))?;
        let posting_count = u32::try_from(indices.len())
            .map_err(|_| MaskCandidateError::Resource("bin cardinality"))?;
        for index in indices {
            cancellation.account(1)?;
            layout.postings.push(index);
        }
        layout.bins.push(BinEntry {
            bin,
            posting_start,
            posting_count,
        });
    }
    Ok(())
}

fn section_offset(
    section: &Section,
    index: usize,
    stride: usize,
) -> Result<usize, MaskCandidateError> {
    let relative = index
        .checked_mul(stride)
        .ok_or(MaskCandidateError::Arithmetic("section index"))?;
    usize::try_from(section.offset)
        .ok()
        .and_then(|offset| offset.checked_add(relative))
        .ok_or(MaskCandidateError::Arithmetic("section offset"))
}

fn record_pages(
    pages: &mut BTreeSet<u64>,
    offset: usize,
    length: usize,
) -> Result<(), MaskCandidateError> {
    if length == 0 {
        return Ok(());
    }
    let last = offset
        .checked_add(length - 1)
        .ok_or(MaskCandidateError::Arithmetic("page trace"))?;
    let first_page = offset / LOGICAL_PAGE_BYTES;
    let last_page = last / LOGICAL_PAGE_BYTES;
    for page in first_page..=last_page {
        pages.insert(
            u64::try_from(page).map_err(|_| MaskCandidateError::Arithmetic("page number"))?,
        );
    }
    Ok(())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), MaskCandidateError> {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or(MaskCandidateError::Arithmetic("write u32"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), MaskCandidateError> {
    bytes
        .get_mut(offset..offset + 8)
        .ok_or(MaskCandidateError::Arithmetic("write u64"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// One decoded query hit. Its boundary range addresses the caller-owned
/// storage in [`MaskQueryBuffer`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaskQueryGene {
    identity: GencodeGeneId,
    contig: Grch38Contig,
    strand: MaskStrand,
    start: GenomicPosition,
    end: GenomicPosition,
    query_rank: u32,
    boundaries: Range<usize>,
}

impl MaskQueryGene {
    pub const fn identity(&self) -> GencodeGeneId {
        self.identity
    }

    pub const fn stable_identity(&self) -> EnsemblGeneId {
        self.identity.stable()
    }

    pub const fn contig(&self) -> Grch38Contig {
        self.contig
    }

    pub const fn strand(&self) -> MaskStrand {
        self.strand
    }

    pub const fn start(&self) -> GenomicPosition {
        self.start
    }

    pub const fn end(&self) -> GenomicPosition {
        self.end
    }

    pub const fn query_rank(&self) -> u32 {
        self.query_rank
    }
}

/// Reusable caller-owned query storage shared by every candidate.
#[derive(Debug, Default)]
pub struct MaskQueryBuffer {
    genes: Vec<MaskQueryGene>,
    boundaries: Vec<GenomicPosition>,
    matches: Vec<u32>,
    plus_count: usize,
}

impl MaskQueryBuffer {
    pub fn with_capacity(genes: usize, boundaries: usize) -> Self {
        Self {
            genes: Vec::with_capacity(genes),
            boundaries: Vec::with_capacity(boundaries),
            matches: Vec::with_capacity(genes),
            plus_count: 0,
        }
    }

    pub fn plus(&self) -> &[MaskQueryGene] {
        &self.genes[..self.plus_count]
    }

    pub fn minus(&self) -> &[MaskQueryGene] {
        &self.genes[self.plus_count..]
    }

    pub fn boundaries(&self, gene: &MaskQueryGene) -> &[GenomicPosition] {
        &self.boundaries[gene.boundaries.clone()]
    }

    /// Candidate-harness scratch cardinality. Runtime callers do not need this;
    /// the benchmark uses it to preallocate identical allocation-free buffers.
    #[doc(hidden)]
    pub fn scratch_match_count(&self) -> usize {
        self.matches.len()
    }

    fn clear(&mut self) {
        self.genes.clear();
        self.boundaries.clear();
        self.matches.clear();
        self.plus_count = 0;
    }
}

#[derive(Clone, Copy, Debug)]
struct GeneWire {
    contig: u8,
    strand: MaskStrand,
    par_y: bool,
    start: u32,
    end: u32,
    rank: u32,
    stable: u64,
    version: u32,
    boundary_start: u32,
    boundary_count: u32,
}

/// Compile-time-selected access policy shared by the ordinary decoder and
/// the qualification-only page auditor. `DirectAccess` contains no tracing
/// state or trace branch, while `TracingAccess` records each decoded range.
/// Both are monomorphized through the same decoder methods below.
trait MappedAccess {
    fn mapped_range(
        &mut self,
        offset: usize,
        length: usize,
        reason: &'static str,
    ) -> Result<&[u8], MaskCandidateError>;
}

struct DirectAccess<'a> {
    mmap: &'a [u8],
}

impl MappedAccess for DirectAccess<'_> {
    #[inline]
    fn mapped_range(
        &mut self,
        offset: usize,
        length: usize,
        reason: &'static str,
    ) -> Result<&[u8], MaskCandidateError> {
        mapped_range(self.mmap, offset, length, reason)
    }
}

struct TracingAccess<'a> {
    mmap: &'a [u8],
    pages: BTreeSet<u64>,
}

impl MappedAccess for TracingAccess<'_> {
    fn mapped_range(
        &mut self,
        offset: usize,
        length: usize,
        reason: &'static str,
    ) -> Result<&[u8], MaskCandidateError> {
        record_pages(&mut self.pages, offset, length)?;
        mapped_range(self.mmap, offset, length, reason)
    }
}

fn mapped_range<'a>(
    mmap: &'a [u8],
    offset: usize,
    length: usize,
    reason: &'static str,
) -> Result<&'a [u8], MaskCandidateError> {
    mmap.get(
        offset
            ..offset
                .checked_add(length)
                .ok_or(MaskCandidateError::Arithmetic("mapped range"))?,
    )
    .ok_or(MaskCandidateError::Corrupt(reason))
}

/// Cheap-open mmap reader for one benchmark-only mask member.
pub struct MaskCandidateReader {
    mmap: Mmap,
    codec: MaskCandidateCodec,
    sections: [Section; SECTION_COUNT],
    directories: Vec<DirectoryEntry>,
    open_pages: Vec<u64>,
}

/// Deterministic logical 4,096-byte pages actually decoded by one open and
/// point query. This is a format-comparison metric, not an operating-system
/// page-fault counter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaskPageTrace {
    pub metadata_pages: Vec<u64>,
    pub payload_pages: Vec<u64>,
}

impl MaskCandidateReader {
    pub fn open(path: &Path) -> Result<Self, MaskCandidateError> {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| {
            MaskCandidateError::Io(io::Error::from_raw_os_error(error.raw_os_error()))
        })?;
        Self::open_held(File::from(descriptor))
    }

    /// Open a caller-authenticated descriptor without reopening its pathname.
    pub fn open_held(file: File) -> Result<Self, MaskCandidateError> {
        let metadata = file.metadata()?;
        let length = usize::try_from(metadata.len())
            .map_err(|_| MaskCandidateError::Resource("member bytes"))?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || !(HEADER_BYTES..=MAX_MEMBER_BYTES).contains(&length)
        {
            return Err(MaskCandidateError::Corrupt("member length or type"));
        }
        // SAFETY: the read-only map is owned by this reader and every byte
        // access is bounds checked before decoding.
        let mmap = unsafe { Mmap::map(&file)? };
        let mut open_pages = BTreeSet::new();
        record_pages(&mut open_pages, 0, HEADER_BYTES)?;
        let header = mmap
            .get(..HEADER_BYTES)
            .ok_or(MaskCandidateError::Corrupt("header"))?;
        if &header[0..8] != MAGIC
            || le_u16(header, 8)? != VERSION
            || le_u64(header, 16)? != length as u64
            || le_u64(header, 24)? != HEADER_BYTES as u64
            || header[12..16].iter().any(|byte| *byte != 0)
            || header[152..160].iter().any(|byte| *byte != 0)
        {
            return Err(MaskCandidateError::Corrupt("header fields"));
        }
        let codec = MaskCandidateCodec::from_code(header[10])?;
        let directory_count = header[11] as usize;
        if directory_count == 0 || directory_count > MAX_CONTIGS {
            return Err(MaskCandidateError::Corrupt("contig count"));
        }
        let expected_strides = [DIRECTORY_BYTES, GENE_BYTES, 4, INDEX_BYTES, 4];
        let mut sections = [Section::default(); SECTION_COUNT];
        let mut cursor = HEADER_BYTES as u64;
        for index in 0..SECTION_COUNT {
            let offset = 32 + index * SECTION_BYTES;
            if header[offset + 20..offset + 24]
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(MaskCandidateError::Corrupt("section reserved"));
            }
            let section = Section {
                offset: le_u64(header, offset)?,
                count: le_u64(header, offset + 8)?,
                stride: le_u32(header, offset + 16)?,
            };
            if section.offset != cursor || section.stride as usize != expected_strides[index] {
                return Err(MaskCandidateError::Corrupt("section layout"));
            }
            let bytes = section
                .count
                .checked_mul(section.stride as u64)
                .ok_or(MaskCandidateError::Arithmetic("section size"))?;
            cursor = cursor
                .checked_add(bytes)
                .ok_or(MaskCandidateError::Arithmetic("section end"))?;
            if cursor > length as u64 {
                return Err(MaskCandidateError::Corrupt("section bounds"));
            }
            sections[index] = section;
        }
        if cursor != length as u64
            || sections[0].count != directory_count as u64
            || sections[1].count == 0
            || sections[1].count > MAX_GENES as u64
            || sections[2].count > MAX_BOUNDARIES as u64
            || sections[4].count > MAX_POSTINGS as u64
            || (codec == MaskCandidateCodec::IntervalTree && sections[4].count != 0)
        {
            return Err(MaskCandidateError::Corrupt("section counts"));
        }

        let mut directories = Vec::with_capacity(directory_count);
        let mut prior_contig = 0_u8;
        let mut expected_gene = 0_u64;
        let mut expected_index = 0_u64;
        for index in 0..directory_count {
            let offset = section_offset(&sections[0], index, DIRECTORY_BYTES)?;
            record_pages(&mut open_pages, offset, DIRECTORY_BYTES)?;
            let bytes = mmap
                .get(offset..offset + DIRECTORY_BYTES)
                .ok_or(MaskCandidateError::Corrupt("directory"))?;
            if bytes[1..8].iter().any(|byte| *byte != 0) {
                return Err(MaskCandidateError::Corrupt("directory reserved"));
            }
            let entry = DirectoryEntry {
                contig: bytes[0],
                gene_start: le_u64(bytes, 8)?,
                gene_count: le_u64(bytes, 16)?,
                index_start: le_u64(bytes, 24)?,
                index_count: le_u64(bytes, 32)?,
            };
            Grch38Contig::from_code(entry.contig)
                .map_err(|_| MaskCandidateError::Corrupt("contig code"))?;
            if entry.contig <= prior_contig
                || entry.gene_count == 0
                || entry.gene_start != expected_gene
                || entry.index_start != expected_index
                || (codec == MaskCandidateCodec::IntervalTree
                    && entry.index_count != entry.gene_count)
            {
                return Err(MaskCandidateError::Corrupt("directory order or counts"));
            }
            expected_gene = expected_gene
                .checked_add(entry.gene_count)
                .ok_or(MaskCandidateError::Arithmetic("gene directory"))?;
            expected_index = expected_index
                .checked_add(entry.index_count)
                .ok_or(MaskCandidateError::Arithmetic("index directory"))?;
            if expected_gene > sections[1].count || expected_index > sections[3].count {
                return Err(MaskCandidateError::Corrupt("directory range"));
            }
            prior_contig = entry.contig;
            directories.push(entry);
        }
        if expected_gene != sections[1].count || expected_index != sections[3].count {
            return Err(MaskCandidateError::Corrupt("directory coverage"));
        }
        Ok(Self {
            mmap,
            codec,
            sections,
            directories,
            open_pages: open_pages.into_iter().collect(),
        })
    }

    pub const fn codec(&self) -> MaskCandidateCodec {
        self.codec
    }

    pub fn file_len(&self) -> u64 {
        self.mmap.len() as u64
    }

    /// Run the real decoder through its explicit qualification-only access
    /// policy while recording every mapped payload page it touches. Ordinary
    /// and timed queries use a separately monomorphized direct-access policy
    /// with no trace state and no per-read trace branch.
    pub fn query_with_page_trace(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        output: &mut MaskQueryBuffer,
    ) -> Result<MaskPageTrace, MaskCandidateError> {
        output.clear();
        let mut access = TracingAccess {
            mmap: self.mmap.as_ref(),
            pages: BTreeSet::new(),
        };
        let result = self.query_inner(&mut access, contig, position, None, output);
        if result.is_err() {
            output.clear();
        }
        result?;
        Ok(MaskPageTrace {
            metadata_pages: self.open_pages.clone(),
            payload_pages: access.pages.into_iter().collect(),
        })
    }

    /// Query all containing records. Plus-strand records precede minus-strand
    /// records, and each strand retains the authenticated upstream rank.
    pub fn query(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskCandidateError> {
        self.query_stable(contig, position, None, output)
    }

    /// Apply the existing unversioned gene filter only after the request
    /// contig has selected its records. Exact version and `_PAR_Y` identity are
    /// retained in every result.
    pub fn query_stable(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskCandidateError> {
        output.clear();
        let mut access = DirectAccess {
            mmap: self.mmap.as_ref(),
        };
        let result = self.query_inner(&mut access, contig, position, stable, output);
        if result.is_err() {
            output.clear();
        }
        result
    }

    fn query_inner<A: MappedAccess>(
        &self,
        access: &mut A,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskCandidateError> {
        let Some(directory) = self
            .directories
            .binary_search_by_key(&contig.code(), |entry| entry.contig)
            .ok()
            .and_then(|index| self.directories.get(index))
        else {
            return Ok(());
        };
        match self.codec {
            MaskCandidateCodec::IntervalTree => {
                self.interval_matches(access, directory, position, &mut output.matches)?
            }
            MaskCandidateCodec::Domains => {
                self.domain_matches(access, directory, position, &mut output.matches)?
            }
            MaskCandidateCodec::BinnedPostings => {
                self.bin_matches(access, directory, position, &mut output.matches)?
            }
        }
        output.matches.sort_unstable_by_key(|index| {
            self.read_gene_wire(access, *index as u64)
                .map_or(u32::MAX, |gene| gene.rank)
        });
        for strand in [MaskStrand::Plus, MaskStrand::Minus] {
            for match_index in 0..output.matches.len() {
                let index = output.matches[match_index];
                let wire = self.read_gene_wire(access, index as u64)?;
                if wire.contig != contig.code()
                    || position.get() <= wire.start
                    || position.get() > wire.end
                    || wire.strand != strand
                {
                    continue;
                }
                let identity = self.wire_identity(wire)?;
                if stable.is_none_or(|filter| filter == identity.stable()) {
                    self.push_gene(access, wire, identity, output)?;
                }
            }
            if strand == MaskStrand::Plus {
                output.plus_count = output.genes.len();
            }
        }
        Ok(())
    }

    fn interval_matches<A: MappedAccess>(
        &self,
        access: &mut A,
        directory: &DirectoryEntry,
        position: GenomicPosition,
        matches: &mut Vec<u32>,
    ) -> Result<(), MaskCandidateError> {
        if directory.index_count == 0 {
            return Ok(());
        }
        let root = u32::try_from(directory.index_start)
            .map_err(|_| MaskCandidateError::Corrupt("interval root"))?;
        let mut stack = [NONE; 64];
        stack[0] = root;
        let mut depth = 1_usize;
        let mut visits = 0_u64;
        while depth > 0 {
            depth -= 1;
            let index = stack[depth];
            if !contains_index(directory, index) {
                return Err(MaskCandidateError::Corrupt("interval child"));
            }
            visits += 1;
            if visits > directory.index_count {
                return Err(MaskCandidateError::Corrupt("interval cycle"));
            }
            let node = self.read_tree_node(access, index as u64)?;
            if node.max_end < position.get() {
                continue;
            }
            if !contains_gene(directory, node.gene) {
                return Err(MaskCandidateError::Corrupt("interval gene"));
            }
            let gene = self.read_gene_wire(access, node.gene as u64)?;
            if position.get() > gene.start && position.get() <= gene.end {
                matches.push(node.gene);
            }
            if node.left != NONE {
                push_stack(&mut stack, &mut depth, node.left)?;
            }
            if node.right != NONE && gene.start < position.get() {
                push_stack(&mut stack, &mut depth, node.right)?;
            }
        }
        Ok(())
    }

    fn domain_matches<A: MappedAccess>(
        &self,
        access: &mut A,
        directory: &DirectoryEntry,
        position: GenomicPosition,
        matches: &mut Vec<u32>,
    ) -> Result<(), MaskCandidateError> {
        let mut low = 0_u64;
        let mut high = directory.index_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let domain = self.read_domain(access, directory.index_start + middle)?;
            if domain.begin <= position.get() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == 0 {
            return Ok(());
        }
        let domain = self.read_domain(access, directory.index_start + low - 1)?;
        if position.get() <= domain.end {
            self.append_postings(access, domain.posting_start, domain.posting_count, matches)?;
        }
        Ok(())
    }

    fn bin_matches<A: MappedAccess>(
        &self,
        access: &mut A,
        directory: &DirectoryEntry,
        position: GenomicPosition,
        matches: &mut Vec<u32>,
    ) -> Result<(), MaskCandidateError> {
        let wanted = position
            .get()
            .checked_sub(1)
            .ok_or(MaskCandidateError::Bounds("position"))?
            >> BIN_SHIFT;
        let mut low = 0_u64;
        let mut high = directory.index_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let bin = self.read_bin(access, directory.index_start + middle)?;
            if bin.bin < wanted {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low < directory.index_count {
            let bin = self.read_bin(access, directory.index_start + low)?;
            if bin.bin == wanted {
                self.append_postings(access, bin.posting_start, bin.posting_count, matches)?;
            }
        }
        Ok(())
    }

    fn append_postings<A: MappedAccess>(
        &self,
        access: &mut A,
        start: u32,
        count: u32,
        matches: &mut Vec<u32>,
    ) -> Result<(), MaskCandidateError> {
        let end = (start as u64)
            .checked_add(count as u64)
            .ok_or(MaskCandidateError::Arithmetic("posting range"))?;
        if end > self.sections[4].count {
            return Err(MaskCandidateError::Corrupt("posting range"));
        }
        for index in start as u64..end {
            let offset = section_offset(&self.sections[4], index as usize, 4)?;
            matches.push(le_u32(
                access.mapped_range(offset, 4, "posting record")?,
                0,
            )?);
        }
        Ok(())
    }

    fn push_gene<A: MappedAccess>(
        &self,
        access: &mut A,
        wire: GeneWire,
        identity: GencodeGeneId,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskCandidateError> {
        let boundary_end = (wire.boundary_start as u64)
            .checked_add(wire.boundary_count as u64)
            .ok_or(MaskCandidateError::Arithmetic("boundary range"))?;
        if boundary_end > self.sections[2].count
            || wire.boundary_count as usize > MAX_BOUNDARIES_PER_GENE
        {
            return Err(MaskCandidateError::Corrupt("boundary range"));
        }
        let boundary_begin = output.boundaries.len();
        let mut prior = 0_u32;
        for boundary_index in wire.boundary_start as u64..boundary_end {
            let offset = section_offset(&self.sections[2], boundary_index as usize, 4)?;
            let boundary = le_u32(access.mapped_range(offset, 4, "exon boundary")?, 0)?;
            if boundary < wire.start || boundary > wire.end || (prior != 0 && boundary <= prior) {
                return Err(MaskCandidateError::Corrupt("exon boundaries"));
            }
            let boundary = GenomicPosition::new(boundary)
                .map_err(|_| MaskCandidateError::Corrupt("exon boundary"))?;
            output.boundaries.push(boundary);
            prior = boundary.get();
        }
        let boundary_end = output.boundaries.len();
        output.genes.push(MaskQueryGene {
            identity,
            contig: Grch38Contig::from_code(wire.contig)
                .map_err(|_| MaskCandidateError::Corrupt("gene contig"))?,
            strand: wire.strand,
            start: GenomicPosition::new(wire.start)
                .map_err(|_| MaskCandidateError::Corrupt("gene start"))?,
            end: GenomicPosition::new(wire.end)
                .map_err(|_| MaskCandidateError::Corrupt("gene end"))?,
            query_rank: wire.rank,
            boundaries: boundary_begin..boundary_end,
        });
        Ok(())
    }

    fn wire_identity(&self, wire: GeneWire) -> Result<GencodeGeneId, MaskCandidateError> {
        let stable = EnsemblGeneId::from_numeric(wire.stable)
            .map_err(|_| MaskCandidateError::Corrupt("stable gene identity"))?;
        GencodeGeneId::new(stable, wire.version, wire.par_y)
            .map_err(|_| MaskCandidateError::Corrupt("exact gene identity"))
    }

    fn read_gene_wire<A: MappedAccess>(
        &self,
        access: &mut A,
        index: u64,
    ) -> Result<GeneWire, MaskCandidateError> {
        if index >= self.sections[1].count {
            return Err(MaskCandidateError::Corrupt("gene index"));
        }
        let offset = section_offset(&self.sections[1], index as usize, GENE_BYTES)?;
        let bytes = access.mapped_range(offset, GENE_BYTES, "gene record")?;
        if bytes[2..4].iter().any(|byte| *byte != 0)
            || bytes[36..40].iter().any(|byte| *byte != 0)
            || bytes[1] & !3 != 0
        {
            return Err(MaskCandidateError::Corrupt("gene reserved"));
        }
        let start = le_u32(bytes, 4)?;
        let end = le_u32(bytes, 8)?;
        if start == 0 || start > end {
            return Err(MaskCandidateError::Corrupt("gene span"));
        }
        Ok(GeneWire {
            contig: bytes[0],
            strand: MaskStrand::from_flag(bytes[1] & 1)?,
            par_y: bytes[1] & 2 != 0,
            start,
            end,
            rank: le_u32(bytes, 12)?,
            stable: le_u64(bytes, 16)?,
            version: le_u32(bytes, 24)?,
            boundary_start: le_u32(bytes, 28)?,
            boundary_count: le_u32(bytes, 32)?,
        })
    }

    fn read_tree_node<A: MappedAccess>(
        &self,
        access: &mut A,
        index: u64,
    ) -> Result<TreeNode, MaskCandidateError> {
        let bytes = self.index_record(access, index)?;
        Ok(TreeNode {
            gene: le_u32(bytes, 0)?,
            max_end: le_u32(bytes, 4)?,
            left: le_u32(bytes, 8)?,
            right: le_u32(bytes, 12)?,
        })
    }

    fn read_domain<A: MappedAccess>(
        &self,
        access: &mut A,
        index: u64,
    ) -> Result<DomainEntry, MaskCandidateError> {
        let bytes = self.index_record(access, index)?;
        let result = DomainEntry {
            begin: le_u32(bytes, 0)?,
            end: le_u32(bytes, 4)?,
            posting_start: le_u32(bytes, 8)?,
            posting_count: le_u32(bytes, 12)?,
        };
        if result.begin == 0 || result.begin > result.end || result.posting_count == 0 {
            return Err(MaskCandidateError::Corrupt("domain record"));
        }
        Ok(result)
    }

    fn read_bin<A: MappedAccess>(
        &self,
        access: &mut A,
        index: u64,
    ) -> Result<BinEntry, MaskCandidateError> {
        let bytes = self.index_record(access, index)?;
        if bytes[4..8].iter().any(|byte| *byte != 0) {
            return Err(MaskCandidateError::Corrupt("bin reserved"));
        }
        let result = BinEntry {
            bin: le_u32(bytes, 0)?,
            posting_start: le_u32(bytes, 8)?,
            posting_count: le_u32(bytes, 12)?,
        };
        if result.posting_count == 0 {
            return Err(MaskCandidateError::Corrupt("bin record"));
        }
        Ok(result)
    }

    fn index_record<'a, A: MappedAccess>(
        &self,
        access: &'a mut A,
        index: u64,
    ) -> Result<&'a [u8], MaskCandidateError> {
        if index >= self.sections[3].count {
            return Err(MaskCandidateError::Corrupt("index range"));
        }
        let offset = section_offset(&self.sections[3], index as usize, INDEX_BYTES)?;
        access.mapped_range(offset, INDEX_BYTES, "index record")
    }

    /// Exhaustive offline structural certification. Runtime open deliberately
    /// does not call this payload-wide pass.
    pub fn inspect_payload(&self) -> Result<(), MaskCandidateError> {
        self.inspect_payload_with_cancellation(&never_cancelled)
    }

    /// Exhaustively certify a payload while polling a caller-owned
    /// cancellation predicate at bounded intervals.
    pub fn inspect_payload_with_cancellation(
        &self,
        cancellation_requested: &dyn Fn() -> bool,
    ) -> Result<(), MaskCandidateError> {
        self.inspect_genes_with_cancellation(cancellation_requested)
            .map(|_| ())
    }

    /// Decode and certify the complete candidate-independent logical stream.
    ///
    /// This is qualification-only evidence. Runtime open and point lookup do
    /// not call it because doing so would page through the complete payload.
    #[doc(hidden)]
    pub fn inspect_genes(&self) -> Result<Vec<CanonicalMaskGene>, MaskCandidateError> {
        self.inspect_genes_with_cancellation(&never_cancelled)
    }

    /// Decode and certify the complete logical stream with bounded
    /// cancellation checks. This remains qualification-only work.
    #[doc(hidden)]
    pub fn inspect_genes_with_cancellation(
        &self,
        cancellation_requested: &dyn Fn() -> bool,
    ) -> Result<Vec<CanonicalMaskGene>, MaskCandidateError> {
        let mut cancellation = CancellationCheck::new(cancellation_requested)?;
        let mut access = DirectAccess {
            mmap: self.mmap.as_ref(),
        };
        let mut genes = Vec::with_capacity(self.sections[1].count as usize);
        for directory in &self.directories {
            cancellation.account(1)?;
            let end = directory
                .gene_start
                .checked_add(directory.gene_count)
                .ok_or(MaskCandidateError::Arithmetic("gene directory"))?;
            for index in directory.gene_start..end {
                cancellation.account(1)?;
                let wire = self.read_gene_wire(&mut access, index)?;
                if wire.contig != directory.contig {
                    return Err(MaskCandidateError::Corrupt("gene contig"));
                }
                let identity = self.wire_identity(wire)?;
                let mut boundaries = Vec::with_capacity(wire.boundary_count as usize);
                let boundary_end = (wire.boundary_start as u64)
                    .checked_add(wire.boundary_count as u64)
                    .ok_or(MaskCandidateError::Arithmetic("boundary range"))?;
                if boundary_end > self.sections[2].count {
                    return Err(MaskCandidateError::Corrupt("boundary range"));
                }
                let mut prior = 0_u32;
                for boundary_index in wire.boundary_start as u64..boundary_end {
                    cancellation.account(1)?;
                    let offset = section_offset(&self.sections[2], boundary_index as usize, 4)?;
                    let value = le_u32(access.mapped_range(offset, 4, "exon boundary")?, 0)?;
                    if value < wire.start || value > wire.end || (prior != 0 && value <= prior) {
                        return Err(MaskCandidateError::Corrupt("exon boundaries"));
                    }
                    boundaries.push(
                        GenomicPosition::new(value)
                            .map_err(|_| MaskCandidateError::Corrupt("exon boundary"))?,
                    );
                    prior = value;
                }
                genes.push(CanonicalMaskGene::new(
                    identity,
                    Grch38Contig::from_code(wire.contig)
                        .map_err(|_| MaskCandidateError::Corrupt("gene contig"))?,
                    wire.strand,
                    GenomicPosition::new(wire.start)
                        .map_err(|_| MaskCandidateError::Corrupt("gene start"))?,
                    GenomicPosition::new(wire.end)
                        .map_err(|_| MaskCandidateError::Corrupt("gene end"))?,
                    wire.rank,
                    boundaries,
                )?);
            }
        }
        cancellation.check_now()?;
        let expected = encode_candidate(self.codec, &genes, &mut cancellation)?;
        if expected.len() != self.mmap.len() {
            return Err(MaskCandidateError::Corrupt("noncanonical payload"));
        }
        for (expected_chunk, actual_chunk) in expected
            .chunks(WRITE_CHUNK_BYTES)
            .zip(self.mmap.chunks(WRITE_CHUNK_BYTES))
        {
            cancellation.check_now()?;
            if expected_chunk != actual_chunk {
                return Err(MaskCandidateError::Corrupt("noncanonical payload"));
            }
            cancellation.account(expected_chunk.len())?;
        }
        cancellation.check_now()?;
        Ok(genes)
    }
}

fn contains_index(directory: &DirectoryEntry, index: u32) -> bool {
    let index = index as u64;
    index >= directory.index_start
        && index < directory.index_start.saturating_add(directory.index_count)
}

fn contains_gene(directory: &DirectoryEntry, index: u32) -> bool {
    let index = index as u64;
    index >= directory.gene_start
        && index < directory.gene_start.saturating_add(directory.gene_count)
}

fn push_stack(
    stack: &mut [u32; 64],
    depth: &mut usize,
    value: u32,
) -> Result<(), MaskCandidateError> {
    let slot = stack
        .get_mut(*depth)
        .ok_or(MaskCandidateError::Corrupt("interval depth"))?;
    *slot = value;
    *depth += 1;
    Ok(())
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, MaskCandidateError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(MaskCandidateError::Corrupt("u16"))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, MaskCandidateError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(MaskCandidateError::Corrupt("u32"))
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, MaskCandidateError> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(MaskCandidateError::Corrupt("u64"))
}

impl From<ValueError> for MaskCandidateError {
    fn from(_: ValueError) -> Self {
        Self::Input("typed value")
    }
}
