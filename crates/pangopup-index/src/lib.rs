//! Private, explicitly decoded Pangopup index format.
//!
//! The byte layout is not a public compatibility promise. Integer fields are
//! little-endian and mapped bytes are never cast to Rust structs.

use memmap2::Mmap;
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, PangolinScore,
    RelativePosition, ScoreMagnitude,
};
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File},
    io::{self, Write},
    path::Path,
};

const MAGIC: &[u8; 8] = b"PNGPIDX1";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 320;
const SEGMENT_SIZE: usize = 96;
const TREE_NODE_SIZE: usize = 32;
const EXCEPTION_SIZE: usize = 40;
const PAGE_SIZE: u64 = 4096;
const NONE: u64 = u64::MAX;

/// One alternate record in canonical logical input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputAlternative {
    pub alternate: DnaBase,
    pub score: PangolinScore,
}

/// One ordinary, concrete-reference locus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryInputLocus {
    pub gene: EnsemblGeneId,
    pub contig: Grch38Contig,
    pub position: GenomicPosition,
    pub reference: DnaBase,
    pub alternatives: [InputAlternative; 3],
}

/// One source `REF=N` exception. It is not an SNV.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmbiguousInputLocus {
    pub gene: EnsemblGeneId,
    pub contig: Grch38Contig,
    pub position: GenomicPosition,
    pub alternatives: [InputAlternative; 3],
    pub omitted: DnaBase,
}

/// Canonical logical input shared by the measured codecs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputLocus {
    Ordinary(OrdinaryInputLocus),
    Ambiguous(AmbiguousInputLocus),
}

/// One exact gene-specific lookup record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneScore {
    pub gene: EnsemblGeneId,
    pub score: PangolinScore,
}

/// An ambiguous source-reference record at a queried coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAmbiguity {
    pub gene: EnsemblGeneId,
    pub omitted: DnaBase,
    pub published_alternates: [DnaBase; 3],
}

/// Complete private-kernel lookup result.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LookupResult {
    pub records: Vec<GeneScore>,
    pub ambiguities: Vec<SourceAmbiguity>,
}

/// Instrumentation describes encoded work, not physical storage reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LookupMetrics {
    pub logical_bytes_decoded: u64,
    pub unique_mapped_pages_addressed: u64,
    pub interval_nodes_visited: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteSummary {
    pub bytes: u64,
    pub loci: u64,
    pub segments: u64,
    pub exceptions: u64,
}

/// A deterministic writer or validated-reader failure.
#[derive(Debug)]
pub enum IndexError {
    Io(io::Error),
    InvalidInput(&'static str),
    Corrupt(&'static str),
    Arithmetic(&'static str),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "index I/O failed: {error}"),
            Self::InvalidInput(reason) => write!(f, "invalid logical index input: {reason}"),
            Self::Corrupt(reason) => write!(f, "invalid index: {reason}"),
            Self::Arithmetic(reason) => write!(f, "index arithmetic overflow: {reason}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<io::Error> for IndexError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone)]
struct SegmentBuild {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
    end: u32,
    loci: Vec<OrdinaryInputLocus>,
    payload: Vec<u8>,
    active_count: u32,
    pair_count: u32,
    refs_len: u64,
    active_len: u64,
    masks_len: u64,
    values_len: u64,
    ranks_len: u64,
    payload_rel: u64,
}

#[derive(Clone, Copy)]
struct TreeBuild {
    segment: usize,
    left: Option<usize>,
    right: Option<usize>,
    max_end: u32,
    contig: Grch38Contig,
}

/// Write a deterministic fixed 11-byte prototype artifact.
pub fn write_index(path: &Path, input: &[InputLocus]) -> Result<WriteSummary, IndexError> {
    let (mut segments, mut exceptions) = canonicalize(input)?;
    let mut payload = Vec::new();
    for segment in &mut segments {
        segment.payload_rel = usize_u64(payload.len(), "payload relative offset")?;
        payload.extend_from_slice(&segment.payload);
    }

    let mut roots = [NONE; 25];
    let mut tree = Vec::with_capacity(segments.len());
    for code in 1_u8..=25 {
        let mut indices: Vec<_> = segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| (segment.contig.code() == code).then_some(index))
            .collect();
        indices.sort_by_key(|index| {
            let segment = &segments[*index];
            (segment.start, segment.end, segment.gene.numeric())
        });
        if !indices.is_empty() {
            roots[usize::from(code - 1)] = usize_u64(
                build_tree(&indices, &segments, &mut tree),
                "tree root index",
            )?;
        }
    }

    exceptions.sort_by_key(|locus| {
        (
            locus.contig.code(),
            locus.position.get(),
            locus.gene.numeric(),
        )
    });

    let segment_len = checked_mul_u64(
        usize_u64(segments.len(), "segment count")?,
        SEGMENT_SIZE as u64,
        "segment section length",
    )?;
    let tree_len = checked_mul_u64(
        usize_u64(tree.len(), "tree node count")?,
        TREE_NODE_SIZE as u64,
        "tree section length",
    )?;
    let payload_len = usize_u64(payload.len(), "payload length")?;
    let exception_len = checked_mul_u64(
        usize_u64(exceptions.len(), "exception count")?,
        EXCEPTION_SIZE as u64,
        "exception section length",
    )?;
    let segment_offset = HEADER_SIZE as u64;
    let tree_offset = checked_add_u64(segment_offset, segment_len, "tree offset")?;
    let payload_offset = checked_add_u64(tree_offset, tree_len, "payload offset")?;
    let exception_offset = checked_add_u64(payload_offset, payload_len, "exception offset")?;
    let file_len = checked_add_u64(exception_offset, exception_len, "file length")?;

    let mut bytes = vec![0_u8; HEADER_SIZE];
    bytes[0..8].copy_from_slice(MAGIC);
    put_u32(&mut bytes, 8, VERSION)?;
    put_u32(&mut bytes, 12, HEADER_SIZE as u32)?;
    put_u64(&mut bytes, 16, file_len)?;
    put_section(&mut bytes, 24, segment_offset, segment_len)?;
    put_section(&mut bytes, 40, tree_offset, tree_len)?;
    put_section(&mut bytes, 56, payload_offset, payload_len)?;
    put_section(&mut bytes, 72, exception_offset, exception_len)?;
    put_u64(&mut bytes, 88, usize_u64(segments.len(), "segment count")?)?;
    put_u64(&mut bytes, 96, usize_u64(tree.len(), "tree count")?)?;
    put_u64(
        &mut bytes,
        104,
        usize_u64(exceptions.len(), "exception count")?,
    )?;
    for (index, root) in roots.into_iter().enumerate() {
        put_u64(&mut bytes, 112 + index * 8, root)?;
    }

    for segment in &segments {
        encode_segment(&mut bytes, segment)?;
    }
    for node in &tree {
        encode_tree_node(&mut bytes, node)?;
    }
    bytes.extend_from_slice(&payload);
    for exception in &exceptions {
        encode_exception(&mut bytes, exception)?;
    }
    if usize_u64(bytes.len(), "written file length")? != file_len {
        return Err(IndexError::Arithmetic("written file length mismatch"));
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(WriteSummary {
        bytes: file_len,
        loci: usize_u64(input.len(), "locus count")?,
        segments: usize_u64(segments.len(), "segment count")?,
        exceptions: usize_u64(exceptions.len(), "exception count")?,
    })
}

fn canonicalize(
    input: &[InputLocus],
) -> Result<(Vec<SegmentBuild>, Vec<AmbiguousInputLocus>), IndexError> {
    let mut ordinary = Vec::new();
    let mut exceptions = Vec::new();
    for locus in input {
        match *locus {
            InputLocus::Ordinary(mut locus) => {
                locus
                    .alternatives
                    .sort_by_key(|value| base_code(value.alternate));
                let expected: Vec<_> = DnaBase::ALL
                    .into_iter()
                    .filter(|base| *base != locus.reference)
                    .collect();
                if locus
                    .alternatives
                    .iter()
                    .map(|value| value.alternate)
                    .ne(expected)
                {
                    return Err(IndexError::InvalidInput("ordinary alternate set"));
                }
                ordinary.push(locus);
            }
            InputLocus::Ambiguous(mut locus) => {
                locus
                    .alternatives
                    .sort_by_key(|value| base_code(value.alternate));
                let expected: Vec<_> = DnaBase::ALL
                    .into_iter()
                    .filter(|base| *base != locus.omitted)
                    .collect();
                if !matches!(locus.omitted, DnaBase::A | DnaBase::T)
                    || locus
                        .alternatives
                        .iter()
                        .map(|value| value.alternate)
                        .ne(expected)
                {
                    return Err(IndexError::InvalidInput("ambiguous alternate set"));
                }
                exceptions.push(locus);
            }
        }
    }
    ordinary.sort_by_key(|locus| {
        (
            locus.gene.numeric(),
            locus.contig.code(),
            locus.position.get(),
        )
    });
    for pair in ordinary.windows(2) {
        if pair[0].gene == pair[1].gene
            && pair[0].contig == pair[1].contig
            && pair[0].position == pair[1].position
        {
            return Err(IndexError::InvalidInput("duplicate ordinary locus"));
        }
    }
    for pair in exceptions.windows(2) {
        if pair[0].gene == pair[1].gene
            && pair[0].contig == pair[1].contig
            && pair[0].position == pair[1].position
        {
            return Err(IndexError::InvalidInput("duplicate exception locus"));
        }
    }

    let mut groups: Vec<Vec<OrdinaryInputLocus>> = Vec::new();
    for locus in ordinary {
        let continues = groups
            .last()
            .and_then(|group| group.last())
            .is_some_and(|last| {
                last.gene == locus.gene
                    && last.contig == locus.contig
                    && last.position.get().checked_add(1) == Some(locus.position.get())
            });
        if !continues {
            groups.push(Vec::new());
        }
        groups
            .last_mut()
            .ok_or(IndexError::Arithmetic("segment grouping"))?
            .push(locus);
    }
    let mut segments = Vec::with_capacity(groups.len());
    for loci in groups {
        segments.push(encode_segment_payload(loci)?);
    }
    Ok((segments, exceptions))
}

fn encode_segment_payload(loci: Vec<OrdinaryInputLocus>) -> Result<SegmentBuild, IndexError> {
    let first = loci
        .first()
        .ok_or(IndexError::InvalidInput("empty segment"))?;
    let last = loci
        .last()
        .ok_or(IndexError::InvalidInput("empty segment"))?;
    let payload_len = loci
        .len()
        .checked_mul(11)
        .ok_or(IndexError::Arithmetic("fixed payload length"))?;
    let mut payload = Vec::with_capacity(payload_len);
    for locus in &loci {
        payload.extend_from_slice(&encode_fixed_locus(locus));
    }
    Ok(SegmentBuild {
        gene: first.gene,
        contig: first.contig,
        start: first.position.get(),
        end: last.position.get(),
        loci,
        payload,
        active_count: 0,
        pair_count: 0,
        refs_len: usize_u64(payload_len, "fixed payload length")?,
        active_len: 0,
        masks_len: 0,
        values_len: 0,
        ranks_len: 0,
        payload_rel: 0,
    })
}

fn build_tree(indices: &[usize], segments: &[SegmentBuild], nodes: &mut Vec<TreeBuild>) -> usize {
    let middle = indices.len() / 2;
    let segment_index = indices[middle];
    let node_index = nodes.len();
    nodes.push(TreeBuild {
        segment: segment_index,
        left: None,
        right: None,
        max_end: segments[segment_index].end,
        contig: segments[segment_index].contig,
    });
    let left =
        (!indices[..middle].is_empty()).then(|| build_tree(&indices[..middle], segments, nodes));
    let right = (!indices[middle + 1..].is_empty())
        .then(|| build_tree(&indices[middle + 1..], segments, nodes));
    let mut max_end = segments[segment_index].end;
    if let Some(left) = left {
        max_end = max_end.max(nodes[left].max_end);
    }
    if let Some(right) = right {
        max_end = max_end.max(nodes[right].max_end);
    }
    nodes[node_index].left = left;
    nodes[node_index].right = right;
    nodes[node_index].max_end = max_end;
    node_index
}

fn encode_segment(bytes: &mut Vec<u8>, segment: &SegmentBuild) -> Result<(), IndexError> {
    let start = bytes.len();
    bytes.resize(start + SEGMENT_SIZE, 0);
    bytes[start] = segment.contig.code();
    put_u64(bytes, start + 8, segment.gene.numeric())?;
    put_u32(bytes, start + 16, segment.start)?;
    put_u32(bytes, start + 20, segment.end)?;
    put_u32(
        bytes,
        start + 24,
        u32::try_from(segment.loci.len())
            .map_err(|_| IndexError::Arithmetic("segment locus count"))?,
    )?;
    put_u32(bytes, start + 28, segment.active_count)?;
    put_u32(bytes, start + 32, segment.pair_count)?;
    put_u32(
        bytes,
        start + 36,
        u32::try_from(segment.ranks_len / 16).map_err(|_| IndexError::Arithmetic("rank count"))?,
    )?;
    for (offset, value) in [
        (40, segment.payload_rel),
        (
            48,
            usize_u64(segment.payload.len(), "segment payload length")?,
        ),
        (56, segment.refs_len),
        (64, segment.active_len),
        (72, segment.masks_len),
        (80, segment.values_len),
        (88, segment.ranks_len),
    ] {
        put_u64(bytes, start + offset, value)?;
    }
    Ok(())
}

fn encode_tree_node(bytes: &mut Vec<u8>, node: &TreeBuild) -> Result<(), IndexError> {
    let start = bytes.len();
    bytes.resize(start + TREE_NODE_SIZE, 0);
    put_u64(bytes, start, usize_u64(node.segment, "tree segment index")?)?;
    put_u64(
        bytes,
        start + 8,
        node.left
            .map_or(Ok(NONE), |value| usize_u64(value, "left node"))?,
    )?;
    put_u64(
        bytes,
        start + 16,
        node.right
            .map_or(Ok(NONE), |value| usize_u64(value, "right node"))?,
    )?;
    put_u32(bytes, start + 24, node.max_end)?;
    bytes[start + 28] = node.contig.code();
    Ok(())
}

fn encode_exception(bytes: &mut Vec<u8>, locus: &AmbiguousInputLocus) -> Result<(), IndexError> {
    let start = bytes.len();
    bytes.resize(start + EXCEPTION_SIZE, 0);
    bytes[start] = locus.contig.code();
    bytes[start + 1] = base_code(locus.omitted);
    for (index, alternate) in locus.alternatives.iter().enumerate() {
        bytes[start + 2 + index] = base_code(alternate.alternate);
    }
    put_u64(bytes, start + 8, locus.gene.numeric())?;
    put_u32(bytes, start + 16, locus.position.get())?;
    for (index, alternate) in locus.alternatives.iter().enumerate() {
        let score_offset = start + 24 + index * 4;
        bytes[score_offset] = alternate.score.gain().hundredths();
        bytes[score_offset + 1] = position_code(alternate.score.gain_position());
        bytes[score_offset + 2] = alternate.score.loss().hundredths();
        bytes[score_offset + 3] = position_code(alternate.score.loss_position());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Header {
    file_len: u64,
    segment_offset: u64,
    segment_len: u64,
    tree_offset: u64,
    tree_len: u64,
    payload_offset: u64,
    payload_len: u64,
    exception_offset: u64,
    exception_len: u64,
    segment_count: u64,
    tree_count: u64,
    exception_count: u64,
    roots: [u64; 25],
}

#[derive(Clone, Copy)]
struct SegmentView {
    contig: Grch38Contig,
    gene: EnsemblGeneId,
    start: u32,
    end: u32,
    loci: u32,
    active_count: u32,
    pair_count: u32,
    rank_count: u32,
    payload_rel: u64,
    payload_len: u64,
    refs_len: u64,
    active_len: u64,
    masks_len: u64,
    values_len: u64,
    ranks_len: u64,
}

#[derive(Clone, Copy)]
struct TreeView {
    segment: u64,
    left: u64,
    right: u64,
    max_end: u32,
    contig: Grch38Contig,
}

/// Validated memory-mapped reader for the private fixed 11-byte format.
#[derive(Debug)]
pub struct IndexReader {
    map: Mmap,
    header: Header,
}

impl IndexReader {
    /// Map a file and validate its cheap structural metadata.
    ///
    /// # Safety contract
    ///
    /// The sole unsafe operation is `Mmap::map`. The file must be published as
    /// an immutable inode and never truncated or modified while mapped. Every
    /// subsequent byte access is bounds-checked and explicitly decoded.
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        let file = File::open(path)?;
        // SAFETY: The deployment contract requires an immutable, unmodified
        // inode for the lifetime of this mapping. No mapped byte is interpreted
        // until the structural checks below pass, and no native struct casts
        // are performed.
        let map = unsafe { Mmap::map(&file) }.map_err(IndexError::Io)?;
        let header = decode_header(&map)?;
        validate_structure(&map, &header)?;
        Ok(Self { map, header })
    }

    pub fn lookup(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
    ) -> Result<LookupResult, IndexError> {
        self.lookup_inner(snv, gene, None)
    }

    pub fn lookup_measured(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
    ) -> Result<(LookupResult, LookupMetrics), IndexError> {
        let mut work = Work::default();
        let result = self.lookup_inner(snv, gene, Some(&mut work))?;
        Ok((
            result,
            LookupMetrics {
                logical_bytes_decoded: work.logical_bytes,
                unique_mapped_pages_addressed: work.pages.len() as u64,
                interval_nodes_visited: work.nodes,
            },
        ))
    }

    /// Measure a complete gene-filtered workload with one shared page set.
    pub fn lookup_gene_batch_measured(
        &self,
        queries: &[(Grch38Snv, EnsemblGeneId)],
    ) -> Result<LookupMetrics, IndexError> {
        let mut work = Work::default();
        for (snv, gene) in queries {
            self.lookup_inner(*snv, Some(*gene), Some(&mut work))?;
        }
        Ok(LookupMetrics {
            logical_bytes_decoded: work.logical_bytes,
            unique_mapped_pages_addressed: work.pages.len() as u64,
            interval_nodes_visited: work.nodes,
        })
    }

    /// Measure a mixed filtered/unfiltered workload with one shared page set.
    pub fn lookup_batch_measured(
        &self,
        queries: &[(Grch38Snv, Option<EnsemblGeneId>)],
    ) -> Result<LookupMetrics, IndexError> {
        let mut work = Work::default();
        for (snv, gene) in queries {
            self.lookup_inner(*snv, *gene, Some(&mut work))?;
        }
        Ok(LookupMetrics {
            logical_bytes_decoded: work.logical_bytes,
            unique_mapped_pages_addressed: work.pages.len() as u64,
            interval_nodes_visited: work.nodes,
        })
    }

    pub fn file_len(&self) -> u64 {
        self.header.file_len
    }

    /// Encoded metadata work performed by structural validation during open.
    pub fn open_metrics(&self) -> LookupMetrics {
        let mut work = Work::default();
        let mut optional = Some(&mut work);
        touch(&mut optional, 0, HEADER_SIZE as u64);
        touch(
            &mut optional,
            self.header.segment_offset,
            self.header.segment_len,
        );
        touch(&mut optional, self.header.tree_offset, self.header.tree_len);
        touch(
            &mut optional,
            self.header.exception_offset,
            self.header.exception_len,
        );
        let roots = self
            .header
            .roots
            .iter()
            .filter(|root| **root != NONE)
            .count() as u64;
        let repeated_segments = self.header.segment_len.saturating_mul(2);
        let repeated_tree = self
            .header
            .tree_len
            .saturating_add(roots.saturating_mul(TREE_NODE_SIZE as u64));
        LookupMetrics {
            logical_bytes_decoded: work
                .logical_bytes
                .saturating_add(repeated_segments)
                .saturating_add(repeated_tree),
            unique_mapped_pages_addressed: work.pages.len() as u64,
            interval_nodes_visited: 0,
        }
    }

    /// Exhaustive prototype certification against the already validated input.
    /// This is an offline path and deliberately scans every supplied locus.
    pub fn verify_exact(&self, input: &[InputLocus]) -> Result<(), IndexError> {
        for locus in input {
            match *locus {
                InputLocus::Ordinary(locus) => {
                    for alternate in locus.alternatives {
                        let snv = Grch38Snv::new(
                            locus.contig,
                            locus.position,
                            locus.reference,
                            alternate.alternate,
                        )
                        .map_err(|_| IndexError::InvalidInput("ordinary SNV"))?;
                        let result = self.lookup(snv, Some(locus.gene))?;
                        if result.records.as_slice()
                            != [GeneScore {
                                gene: locus.gene,
                                score: alternate.score,
                            }]
                            || !result.ambiguities.is_empty()
                        {
                            return Err(IndexError::Corrupt("ordinary round-trip mismatch"));
                        }
                    }
                }
                InputLocus::Ambiguous(expected) => {
                    let mut found = false;
                    for index in 0..self.header.exception_count {
                        let actual = self.exception(index, &mut None)?;
                        if actual.gene == expected.gene
                            && actual.contig == expected.contig
                            && actual.position == expected.position
                        {
                            if actual != expected {
                                return Err(IndexError::Corrupt("exception round-trip mismatch"));
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(IndexError::Corrupt("missing exception round-trip record"));
                    }
                }
            }
        }
        Ok(())
    }

    fn lookup_inner(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
        mut work: Option<&mut Work>,
    ) -> Result<LookupResult, IndexError> {
        let mut result = LookupResult::default();
        match gene {
            Some(gene) => self.lookup_gene_segments(snv, gene, &mut result, &mut work)?,
            None => {
                let root = self.header.roots[usize::from(snv.contig().code() - 1)];
                if root != NONE {
                    self.query_tree(root, snv, &mut result, &mut work)?;
                }
            }
        }
        self.lookup_exceptions(snv, gene, &mut result, &mut work)?;
        result.records.sort_by_key(|record| record.gene.numeric());
        result
            .ambiguities
            .sort_by_key(|record| record.gene.numeric());
        Ok(result)
    }

    fn lookup_gene_segments(
        &self,
        snv: Grch38Snv,
        gene: EnsemblGeneId,
        result: &mut LookupResult,
        work: &mut Option<&mut Work>,
    ) -> Result<(), IndexError> {
        let target = (gene.numeric(), snv.contig().code(), snv.position().get());
        let mut low = 0_u64;
        let mut high = self.header.segment_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let segment = self.segment(middle, work)?;
            let key = (segment.gene.numeric(), segment.contig.code(), segment.start);
            if key <= target {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == 0 {
            return Ok(());
        }
        let segment = self.segment(low - 1, work)?;
        if segment.gene == gene
            && segment.contig == snv.contig()
            && segment.start <= snv.position().get()
            && snv.position().get() <= segment.end
        {
            self.decode_segment_score(&segment, snv, result, work)?;
        }
        Ok(())
    }

    fn query_tree(
        &self,
        node_index: u64,
        snv: Grch38Snv,
        result: &mut LookupResult,
        work: &mut Option<&mut Work>,
    ) -> Result<(), IndexError> {
        if let Some(current) = work.as_deref_mut() {
            current.nodes += 1;
        }
        let node = self.tree_node(node_index, work)?;
        if node.contig != snv.contig() {
            return Err(IndexError::Corrupt("tree contig mismatch"));
        }
        let position = snv.position().get();
        if node.left != NONE {
            let left = self.tree_node(node.left, work)?;
            if left.max_end >= position {
                self.query_tree(node.left, snv, result, work)?;
            }
        }
        let segment = self.segment(node.segment, work)?;
        if segment.start <= position && position <= segment.end {
            self.decode_segment_score(&segment, snv, result, work)?;
        }
        if node.right != NONE && segment.start <= position {
            self.query_tree(node.right, snv, result, work)?;
        }
        Ok(())
    }

    fn decode_segment_score(
        &self,
        segment: &SegmentView,
        snv: Grch38Snv,
        result: &mut LookupResult,
        work: &mut Option<&mut Work>,
    ) -> Result<(), IndexError> {
        let ordinal = u64::from(snv.position().get() - segment.start);
        if ordinal >= u64::from(segment.loci) {
            return Err(IndexError::Corrupt("segment locus ordinal"));
        }
        let base = checked_add_u64(
            self.header.payload_offset,
            segment.payload_rel,
            "payload base",
        )?;
        let record_offset = checked_add_u64(
            base,
            checked_mul_u64(ordinal, 11, "fixed record offset")?,
            "fixed record address",
        )?;
        touch(work, record_offset, 11);
        let start = usize::try_from(record_offset)
            .map_err(|_| IndexError::Corrupt("fixed record offset"))?;
        let raw = self
            .map
            .get(start..start + 11)
            .ok_or(IndexError::Corrupt("truncated fixed record"))?;
        if let Some(score) = decode_fixed_locus(raw, snv)? {
            result.records.push(GeneScore {
                gene: segment.gene,
                score,
            });
        }
        Ok(())
    }

    fn lookup_exceptions(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
        result: &mut LookupResult,
        work: &mut Option<&mut Work>,
    ) -> Result<(), IndexError> {
        if let Some(gene) = gene {
            let target = (snv.contig().code(), snv.position().get(), gene.numeric());
            let mut low = 0_u64;
            let mut high = self.header.exception_count;
            while low < high {
                let middle = low + (high - low) / 2;
                let exception = self.exception(middle, work)?;
                let key = (
                    exception.contig.code(),
                    exception.position.get(),
                    exception.gene.numeric(),
                );
                if key < target {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            if low < self.header.exception_count {
                let exception = self.exception(low, work)?;
                if (
                    exception.contig.code(),
                    exception.position.get(),
                    exception.gene.numeric(),
                ) == target
                {
                    result.ambiguities.push(SourceAmbiguity {
                        gene: exception.gene,
                        omitted: exception.omitted,
                        published_alternates: exception.alternatives.map(|value| value.alternate),
                    });
                }
            }
            return Ok(());
        }
        let target = (snv.contig().code(), snv.position().get());
        let mut low = 0_u64;
        let mut high = self.header.exception_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let exception = self.exception(middle, work)?;
            let key = (exception.contig.code(), exception.position.get());
            if key < target {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let mut index = low;
        while index < self.header.exception_count {
            let exception = self.exception(index, work)?;
            if (exception.contig.code(), exception.position.get()) != target {
                break;
            }
            result.ambiguities.push(SourceAmbiguity {
                gene: exception.gene,
                omitted: exception.omitted,
                published_alternates: exception.alternatives.map(|value| value.alternate),
            });
            index += 1;
        }
        Ok(())
    }

    fn segment(&self, index: u64, work: &mut Option<&mut Work>) -> Result<SegmentView, IndexError> {
        if index >= self.header.segment_count {
            return Err(IndexError::Corrupt("segment index out of range"));
        }
        let offset = checked_add_u64(
            self.header.segment_offset,
            checked_mul_u64(index, SEGMENT_SIZE as u64, "segment offset")?,
            "segment address",
        )?;
        touch(work, offset, SEGMENT_SIZE as u64);
        decode_segment_view(&self.map, offset)
    }

    fn tree_node(&self, index: u64, work: &mut Option<&mut Work>) -> Result<TreeView, IndexError> {
        if index >= self.header.tree_count {
            return Err(IndexError::Corrupt("tree node index out of range"));
        }
        let offset = checked_add_u64(
            self.header.tree_offset,
            checked_mul_u64(index, TREE_NODE_SIZE as u64, "tree node offset")?,
            "tree node address",
        )?;
        touch(work, offset, TREE_NODE_SIZE as u64);
        decode_tree_view(&self.map, offset)
    }

    fn exception(
        &self,
        index: u64,
        work: &mut Option<&mut Work>,
    ) -> Result<AmbiguousInputLocus, IndexError> {
        if index >= self.header.exception_count {
            return Err(IndexError::Corrupt("exception index out of range"));
        }
        let offset = checked_add_u64(
            self.header.exception_offset,
            checked_mul_u64(index, EXCEPTION_SIZE as u64, "exception offset")?,
            "exception address",
        )?;
        touch(work, offset, EXCEPTION_SIZE as u64);
        decode_exception(&self.map, offset)
    }
}

#[derive(Default)]
struct Work {
    logical_bytes: u64,
    pages: BTreeSet<u64>,
    nodes: u64,
}

fn touch(work: &mut Option<&mut Work>, offset: u64, length: u64) {
    let Some(work) = work.as_deref_mut() else {
        return;
    };
    work.logical_bytes = work.logical_bytes.saturating_add(length);
    if length == 0 {
        return;
    }
    let first = offset / PAGE_SIZE;
    let last = offset.saturating_add(length - 1) / PAGE_SIZE;
    for page in first..=last {
        work.pages.insert(page);
    }
}

fn decode_header(bytes: &[u8]) -> Result<Header, IndexError> {
    if bytes.len() < HEADER_SIZE {
        return Err(IndexError::Corrupt("truncated header"));
    }
    if bytes.get(0..8) != Some(MAGIC.as_slice()) {
        return Err(IndexError::Corrupt("wrong magic"));
    }
    if read_u32(bytes, 8)? != VERSION {
        return Err(IndexError::Corrupt("unsupported version"));
    }
    if read_u32(bytes, 12)? != HEADER_SIZE as u32 {
        return Err(IndexError::Corrupt("wrong header size"));
    }
    if bytes[312..320].iter().any(|byte| *byte != 0) {
        return Err(IndexError::Corrupt("header reserved bytes"));
    }
    let mut roots = [NONE; 25];
    for (index, root) in roots.iter_mut().enumerate() {
        *root = read_u64(bytes, (112 + index * 8) as u64)?;
    }
    Ok(Header {
        file_len: read_u64(bytes, 16)?,
        segment_offset: read_u64(bytes, 24)?,
        segment_len: read_u64(bytes, 32)?,
        tree_offset: read_u64(bytes, 40)?,
        tree_len: read_u64(bytes, 48)?,
        payload_offset: read_u64(bytes, 56)?,
        payload_len: read_u64(bytes, 64)?,
        exception_offset: read_u64(bytes, 72)?,
        exception_len: read_u64(bytes, 80)?,
        segment_count: read_u64(bytes, 88)?,
        tree_count: read_u64(bytes, 96)?,
        exception_count: read_u64(bytes, 104)?,
        roots,
    })
}

fn validate_structure(bytes: &[u8], header: &Header) -> Result<(), IndexError> {
    if usize_u64(bytes.len(), "mapped file length")? != header.file_len {
        return Err(IndexError::Corrupt("declared file length"));
    }
    let sections = [
        (header.segment_offset, header.segment_len),
        (header.tree_offset, header.tree_len),
        (header.payload_offset, header.payload_len),
        (header.exception_offset, header.exception_len),
    ];
    let mut previous_end = HEADER_SIZE as u64;
    for (offset, length) in sections {
        if offset < previous_end {
            return Err(IndexError::Corrupt("overlapping or unordered sections"));
        }
        let end = checked_add_u64(offset, length, "section end")?;
        if end > header.file_len {
            return Err(IndexError::Corrupt("section outside file"));
        }
        previous_end = end;
    }
    if previous_end != header.file_len {
        return Err(IndexError::Corrupt("trailing unsectioned bytes"));
    }
    for (length, count, width, reason) in [
        (
            header.segment_len,
            header.segment_count,
            SEGMENT_SIZE as u64,
            "segment section length",
        ),
        (
            header.tree_len,
            header.tree_count,
            TREE_NODE_SIZE as u64,
            "tree section length",
        ),
        (
            header.exception_len,
            header.exception_count,
            EXCEPTION_SIZE as u64,
            "exception section length",
        ),
    ] {
        if checked_mul_u64(count, width, reason)? != length {
            return Err(IndexError::Corrupt(reason));
        }
    }
    if header.segment_count != header.tree_count {
        return Err(IndexError::Corrupt("tree and segment counts differ"));
    }

    let mut previous_gene_key = None;
    let mut previous_payload_end = 0_u64;
    for index in 0..header.segment_count {
        let offset = header.segment_offset + index * SEGMENT_SIZE as u64;
        let segment = decode_segment_view(bytes, offset)?;
        let key = (segment.gene.numeric(), segment.contig.code(), segment.start);
        if previous_gene_key.is_some_and(|previous| previous >= key) {
            return Err(IndexError::Corrupt("segment directory order"));
        }
        previous_gene_key = Some(key);
        if segment.start == 0
            || segment.end < segment.start
            || u64::from(segment.end - segment.start) + 1 != u64::from(segment.loci)
        {
            return Err(IndexError::Corrupt("segment coordinate span"));
        }
        let expected_payload =
            checked_mul_u64(u64::from(segment.loci), 11, "fixed segment payload length")?;
        if segment.refs_len != expected_payload
            || segment.active_count != 0
            || segment.pair_count != 0
            || segment.rank_count != 0
            || segment.active_len != 0
            || segment.masks_len != 0
            || segment.values_len != 0
            || segment.ranks_len != 0
        {
            return Err(IndexError::Corrupt("segment payload lengths"));
        }
        let sum = [
            segment.refs_len,
            segment.active_len,
            segment.masks_len,
            segment.values_len,
            segment.ranks_len,
        ]
        .into_iter()
        .try_fold(0_u64, |total, value| {
            checked_add_u64(total, value, "segment payload sum")
        })?;
        if sum != segment.payload_len
            || segment.payload_rel < previous_payload_end
            || checked_add_u64(
                segment.payload_rel,
                segment.payload_len,
                "segment payload end",
            )? > header.payload_len
        {
            return Err(IndexError::Corrupt("segment payload range"));
        }
        previous_payload_end = segment.payload_rel + segment.payload_len;
    }
    if previous_payload_end != header.payload_len && header.segment_count != 0 {
        return Err(IndexError::Corrupt("unclaimed payload bytes"));
    }

    for index in 0..header.tree_count {
        let node = decode_tree_view(bytes, header.tree_offset + index * TREE_NODE_SIZE as u64)?;
        if node.segment >= header.segment_count
            || (node.left != NONE && node.left >= header.tree_count)
            || (node.right != NONE && node.right >= header.tree_count)
        {
            return Err(IndexError::Corrupt("tree link out of range"));
        }
        let segment = decode_segment_view(
            bytes,
            header.segment_offset + node.segment * SEGMENT_SIZE as u64,
        )?;
        if node.contig != segment.contig || node.max_end < segment.end {
            return Err(IndexError::Corrupt("tree node metadata"));
        }
    }
    let mut expected_node = 0_u64;
    for (index, root) in header.roots.iter().copied().enumerate() {
        if root != NONE {
            if root >= header.tree_count {
                return Err(IndexError::Corrupt("tree root out of range"));
            }
            let node = decode_tree_view(bytes, header.tree_offset + root * TREE_NODE_SIZE as u64)?;
            if usize::from(node.contig.code() - 1) != index {
                return Err(IndexError::Corrupt("tree root contig"));
            }
            validate_tree_subtree(
                bytes,
                header,
                root,
                node.contig,
                &mut expected_node,
                None,
                None,
                0,
            )?;
        }
    }
    if expected_node != header.tree_count {
        return Err(IndexError::Corrupt("tree connectivity or coverage"));
    }
    let mut previous_exception = None;
    for index in 0..header.exception_count {
        let exception = decode_exception(
            bytes,
            header.exception_offset + index * EXCEPTION_SIZE as u64,
        )?;
        let key = (
            exception.contig.code(),
            exception.position.get(),
            exception.gene.numeric(),
        );
        if previous_exception.is_some_and(|previous| previous >= key) {
            return Err(IndexError::Corrupt("exception directory order"));
        }
        previous_exception = Some(key);
    }
    Ok(())
}

type TreeKey = (u32, u32, u64);

#[derive(Clone, Copy)]
struct ValidatedSubtree {
    height: u32,
    max_end: u32,
}

#[allow(clippy::too_many_arguments)]
fn validate_tree_subtree(
    bytes: &[u8],
    header: &Header,
    node_index: u64,
    contig: Grch38Contig,
    expected_node: &mut u64,
    lower: Option<TreeKey>,
    upper: Option<TreeKey>,
    depth: u32,
) -> Result<ValidatedSubtree, IndexError> {
    if depth > 64 {
        return Err(IndexError::Corrupt("tree depth or balance"));
    }
    if node_index != *expected_node {
        return Err(IndexError::Corrupt("tree connectivity or preorder"));
    }
    *expected_node = expected_node
        .checked_add(1)
        .ok_or(IndexError::Arithmetic("tree traversal count"))?;
    let node = decode_tree_view(
        bytes,
        header.tree_offset + node_index * TREE_NODE_SIZE as u64,
    )?;
    let segment = decode_segment_view(
        bytes,
        header.segment_offset + node.segment * SEGMENT_SIZE as u64,
    )?;
    if node.contig != contig || segment.contig != contig {
        return Err(IndexError::Corrupt("tree subtree contig"));
    }
    let key = (segment.start, segment.end, segment.gene.numeric());
    if lower.is_some_and(|bound| key <= bound) || upper.is_some_and(|bound| key >= bound) {
        return Err(IndexError::Corrupt("tree BST ordering"));
    }

    let left = if node.left == NONE {
        None
    } else {
        Some(validate_tree_subtree(
            bytes,
            header,
            node.left,
            contig,
            expected_node,
            lower,
            Some(key),
            depth + 1,
        )?)
    };
    let right = if node.right == NONE {
        None
    } else {
        Some(validate_tree_subtree(
            bytes,
            header,
            node.right,
            contig,
            expected_node,
            Some(key),
            upper,
            depth + 1,
        )?)
    };
    let left_height = left.map_or(0, |subtree| subtree.height);
    let right_height = right.map_or(0, |subtree| subtree.height);
    if left_height.abs_diff(right_height) > 1 {
        return Err(IndexError::Corrupt("tree balance"));
    }
    let exact_max = segment
        .end
        .max(left.map_or(0, |subtree| subtree.max_end))
        .max(right.map_or(0, |subtree| subtree.max_end));
    if node.max_end != exact_max {
        return Err(IndexError::Corrupt("tree subtree maximum"));
    }
    Ok(ValidatedSubtree {
        height: 1 + left_height.max(right_height),
        max_end: exact_max,
    })
}

fn decode_segment_view(bytes: &[u8], offset: u64) -> Result<SegmentView, IndexError> {
    let start = usize::try_from(offset).map_err(|_| IndexError::Corrupt("segment offset"))?;
    let slice = bytes
        .get(start..start + SEGMENT_SIZE)
        .ok_or(IndexError::Corrupt("truncated segment"))?;
    if slice[1..8].iter().any(|byte| *byte != 0) {
        return Err(IndexError::Corrupt("segment reserved bytes"));
    }
    Ok(SegmentView {
        contig: Grch38Contig::from_code(slice[0])
            .map_err(|_| IndexError::Corrupt("contig code"))?,
        gene: EnsemblGeneId::from_numeric(read_u64(slice, 8)?)
            .map_err(|_| IndexError::Corrupt("gene code"))?,
        start: read_u32(slice, 16)?,
        end: read_u32(slice, 20)?,
        loci: read_u32(slice, 24)?,
        active_count: read_u32(slice, 28)?,
        pair_count: read_u32(slice, 32)?,
        rank_count: read_u32(slice, 36)?,
        payload_rel: read_u64(slice, 40)?,
        payload_len: read_u64(slice, 48)?,
        refs_len: read_u64(slice, 56)?,
        active_len: read_u64(slice, 64)?,
        masks_len: read_u64(slice, 72)?,
        values_len: read_u64(slice, 80)?,
        ranks_len: read_u64(slice, 88)?,
    })
}

fn decode_tree_view(bytes: &[u8], offset: u64) -> Result<TreeView, IndexError> {
    let start = usize::try_from(offset).map_err(|_| IndexError::Corrupt("tree offset"))?;
    let slice = bytes
        .get(start..start + TREE_NODE_SIZE)
        .ok_or(IndexError::Corrupt("truncated tree node"))?;
    if slice[29..32].iter().any(|byte| *byte != 0) {
        return Err(IndexError::Corrupt("tree reserved bytes"));
    }
    Ok(TreeView {
        segment: read_u64(slice, 0)?,
        left: read_u64(slice, 8)?,
        right: read_u64(slice, 16)?,
        max_end: read_u32(slice, 24)?,
        contig: Grch38Contig::from_code(slice[28])
            .map_err(|_| IndexError::Corrupt("tree contig code"))?,
    })
}

fn decode_exception(bytes: &[u8], offset: u64) -> Result<AmbiguousInputLocus, IndexError> {
    let start = usize::try_from(offset).map_err(|_| IndexError::Corrupt("exception offset"))?;
    let slice = bytes
        .get(start..start + EXCEPTION_SIZE)
        .ok_or(IndexError::Corrupt("truncated exception"))?;
    if slice[5..8]
        .iter()
        .chain(slice[20..24].iter())
        .chain(slice[36..40].iter())
        .any(|byte| *byte != 0)
    {
        return Err(IndexError::Corrupt("exception reserved bytes"));
    }
    let omitted = decode_base(slice[1])?;
    if !matches!(omitted, DnaBase::A | DnaBase::T) {
        return Err(IndexError::Corrupt("exception omitted allele"));
    }
    let mut alternatives = [InputAlternative {
        alternate: DnaBase::A,
        score: default_score()?,
    }; 3];
    for (index, alternative) in alternatives.iter_mut().enumerate() {
        let base = decode_base(slice[2 + index])?;
        let score_offset = 24 + index * 4;
        let gain = decode_score_byte(slice[score_offset])?;
        let gain_position = decode_position_byte(slice[score_offset + 1])?;
        let loss = decode_score_byte(slice[score_offset + 2])?;
        let loss_position = decode_position_byte(slice[score_offset + 3])?;
        *alternative = InputAlternative {
            alternate: base,
            score: PangolinScore::new(gain, gain_position, loss, loss_position),
        };
    }
    let expected: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != omitted)
        .collect();
    if alternatives
        .iter()
        .map(|value| value.alternate)
        .ne(expected)
    {
        return Err(IndexError::Corrupt("exception alternate codes"));
    }
    Ok(AmbiguousInputLocus {
        contig: Grch38Contig::from_code(slice[0])
            .map_err(|_| IndexError::Corrupt("exception contig code"))?,
        omitted,
        gene: EnsemblGeneId::from_numeric(read_u64(slice, 8)?)
            .map_err(|_| IndexError::Corrupt("exception gene code"))?,
        position: GenomicPosition::new(read_u32(slice, 16)?)
            .map_err(|_| IndexError::Corrupt("exception position"))?,
        alternatives,
    })
}

fn encode_pair(magnitude: ScoreMagnitude, position: RelativePosition) -> u16 {
    u16::from(magnitude.hundredths()) | ((position.get() as i16 + 50) as u16) << 7
}

fn encode_fixed_locus(locus: &OrdinaryInputLocus) -> [u8; 11] {
    let mut bits = u128::from(base_code(locus.reference));
    for (index, alternative) in locus.alternatives.iter().enumerate() {
        let gain = encode_pair(alternative.score.gain(), alternative.score.gain_position());
        let loss = encode_pair(alternative.score.loss(), alternative.score.loss_position());
        let score = u32::from(gain) | (u32::from(loss) << 14);
        bits |= u128::from(score) << (3 + index * 28);
    }
    let expanded = bits.to_le_bytes();
    let mut record = [0_u8; 11];
    record.copy_from_slice(&expanded[..11]);
    record
}

fn decode_fixed_locus(raw: &[u8], snv: Grch38Snv) -> Result<Option<PangolinScore>, IndexError> {
    if raw.len() != 11 {
        return Err(IndexError::Corrupt("fixed record length"));
    }
    if raw[10] & 0x80 != 0 {
        return Err(IndexError::Corrupt("fixed record reserved bit"));
    }
    let mut expanded = [0_u8; 16];
    expanded[..11].copy_from_slice(raw);
    let bits = u128::from_le_bytes(expanded);
    let reference = decode_base((bits & 0b111) as u8)?;
    if reference != snv.reference() {
        return Ok(None);
    }
    let alternate_index = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != reference)
        .position(|base| base == snv.alternate());
    let Some(alternate_index) = alternate_index else {
        return Ok(None);
    };
    let encoded = ((bits >> (3 + alternate_index * 28)) & ((1_u128 << 28) - 1)) as u32;
    let gain = decode_pair_code((encoded & 0x3fff) as u16)?;
    let loss = decode_pair_code((encoded >> 14) as u16)?;
    Ok(Some(PangolinScore::new(gain.0, gain.1, loss.0, loss.1)))
}

fn decode_pair_code(value: u16) -> Result<(ScoreMagnitude, RelativePosition), IndexError> {
    let magnitude = value & 0x7f;
    let position = (value >> 7) & 0x7f;
    if magnitude > 100 || position > 100 {
        return Err(IndexError::Corrupt("fixed score value code"));
    }
    Ok((
        ScoreMagnitude::new(magnitude).map_err(|_| IndexError::Corrupt("fixed score magnitude"))?,
        RelativePosition::new(position as i16 - 50)
            .map_err(|_| IndexError::Corrupt("fixed relative position"))?,
    ))
}

fn position_code(position: RelativePosition) -> u8 {
    (position.get() as i16 + 50) as u8
}
fn decode_score_byte(value: u8) -> Result<ScoreMagnitude, IndexError> {
    ScoreMagnitude::new(u16::from(value)).map_err(|_| IndexError::Corrupt("exception score code"))
}
fn decode_position_byte(value: u8) -> Result<RelativePosition, IndexError> {
    RelativePosition::new(i16::from(value) - 50)
        .map_err(|_| IndexError::Corrupt("exception position code"))
}
fn default_pair() -> Result<(ScoreMagnitude, RelativePosition), IndexError> {
    Ok((
        ScoreMagnitude::new(0).map_err(|_| IndexError::Corrupt("default score"))?,
        RelativePosition::new(-50).map_err(|_| IndexError::Corrupt("default position"))?,
    ))
}
fn default_score() -> Result<PangolinScore, IndexError> {
    let pair = default_pair()?;
    Ok(PangolinScore::new(pair.0, pair.1, pair.0, pair.1))
}

fn base_code(base: DnaBase) -> u8 {
    match base {
        DnaBase::A => 0,
        DnaBase::C => 1,
        DnaBase::G => 2,
        DnaBase::T => 3,
    }
}
fn decode_base(code: u8) -> Result<DnaBase, IndexError> {
    match code {
        0 => Ok(DnaBase::A),
        1 => Ok(DnaBase::C),
        2 => Ok(DnaBase::G),
        3 => Ok(DnaBase::T),
        _ => Err(IndexError::Corrupt("allele code")),
    }
}

fn put_section(bytes: &mut [u8], offset: usize, start: u64, length: u64) -> Result<(), IndexError> {
    put_u64(bytes, offset, start)?;
    put_u64(bytes, offset + 8, length)
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), IndexError> {
    let target = bytes
        .get_mut(offset..offset + 4)
        .ok_or(IndexError::Arithmetic("u32 write"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), IndexError> {
    let target = bytes
        .get_mut(offset..offset + 8)
        .ok_or(IndexError::Arithmetic("u64 write"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn read_u32(bytes: &[u8], offset: u64) -> Result<u32, IndexError> {
    let start = usize::try_from(offset).map_err(|_| IndexError::Corrupt("u32 offset"))?;
    let raw: [u8; 4] = bytes
        .get(start..start + 4)
        .ok_or(IndexError::Corrupt("truncated u32"))?
        .try_into()
        .map_err(|_| IndexError::Corrupt("u32 decode"))?;
    Ok(u32::from_le_bytes(raw))
}
fn read_u64(bytes: &[u8], offset: u64) -> Result<u64, IndexError> {
    let start = usize::try_from(offset).map_err(|_| IndexError::Corrupt("u64 offset"))?;
    let raw: [u8; 8] = bytes
        .get(start..start + 8)
        .ok_or(IndexError::Corrupt("truncated u64"))?
        .try_into()
        .map_err(|_| IndexError::Corrupt("u64 decode"))?;
    Ok(u64::from_le_bytes(raw))
}
fn checked_add_u64(left: u64, right: u64, reason: &'static str) -> Result<u64, IndexError> {
    left.checked_add(right)
        .ok_or(IndexError::Arithmetic(reason))
}
fn checked_mul_u64(left: u64, right: u64, reason: &'static str) -> Result<u64, IndexError> {
    left.checked_mul(right)
        .ok_or(IndexError::Arithmetic(reason))
}
fn usize_u64(value: usize, reason: &'static str) -> Result<u64, IndexError> {
    u64::try_from(value).map_err(|_| IndexError::Arithmetic(reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "pangopup-index-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn score(gain: u16, gain_pos: i16, loss: u16, loss_pos: i16) -> PangolinScore {
        PangolinScore::new(
            ScoreMagnitude::new(gain).expect("score"),
            RelativePosition::new(gain_pos).expect("position"),
            ScoreMagnitude::new(loss).expect("score"),
            RelativePosition::new(loss_pos).expect("position"),
        )
    }

    fn ordinary(gene: &str, start: u32, count: u32) -> Vec<InputLocus> {
        let gene = gene.parse().expect("gene");
        (start..start + count)
            .map(|position| {
                InputLocus::Ordinary(OrdinaryInputLocus {
                    gene,
                    contig: "chr1".parse().expect("contig"),
                    position: GenomicPosition::new(position).expect("position"),
                    reference: DnaBase::A,
                    alternatives: [
                        InputAlternative {
                            alternate: DnaBase::C,
                            score: default_score().expect("default"),
                        },
                        InputAlternative {
                            alternate: DnaBase::G,
                            score: score(35, 25, 0, -50),
                        },
                        InputAlternative {
                            alternate: DnaBase::T,
                            score: score(0, -50, 10, 2),
                        },
                    ],
                })
            })
            .collect()
    }

    #[test]
    fn exact_round_trip_and_ambiguity() {
        let mut input = ordinary("ENSG00000000001", 100, 300);
        input.extend(ordinary("ENSG00000000002", 200, 100));
        input.push(InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: "ENSG00000000003".parse().expect("gene"),
            contig: "chr1".parse().expect("contig"),
            position: GenomicPosition::new(250).expect("position"),
            omitted: DnaBase::T,
            alternatives: [
                InputAlternative {
                    alternate: DnaBase::A,
                    score: score(1, -50, 0, -50),
                },
                InputAlternative {
                    alternate: DnaBase::C,
                    score: score(0, -50, 0, -50),
                },
                InputAlternative {
                    alternate: DnaBase::G,
                    score: score(0, 50, 100, 50),
                },
            ],
        }));
        let path = path("roundtrip");
        write_index(&path, &input).expect("write");
        let reader = IndexReader::open(&path).expect("open");
        let snv = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(250).expect("position"),
            DnaBase::A,
            DnaBase::G,
        )
        .expect("snv");
        let result = reader.lookup(snv, None).expect("lookup");
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.records[0].score.gain().hundredths(), 35);
        assert_eq!(result.ambiguities.len(), 1);
        let measured = reader.lookup_measured(snv, None).expect("measured").1;
        assert!(measured.logical_bytes_decoded > 0 && measured.unique_mapped_pages_addressed > 0);
        fs::remove_file(path).expect("remove");
    }

    #[test]
    fn open_rejects_structural_corruption_and_lookup_rejects_touched_values() {
        let mut input = ordinary("ENSG00000000001", 100, 300);
        input.extend(ordinary("ENSG00000000003", 200, 1));
        input.push(InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: "ENSG00000000002".parse().expect("gene"),
            contig: "chr1".parse().expect("contig"),
            position: GenomicPosition::new(200).expect("position"),
            omitted: DnaBase::T,
            alternatives: [
                InputAlternative {
                    alternate: DnaBase::A,
                    score: default_score().expect("score"),
                },
                InputAlternative {
                    alternate: DnaBase::C,
                    score: default_score().expect("score"),
                },
                InputAlternative {
                    alternate: DnaBase::G,
                    score: default_score().expect("score"),
                },
            ],
        }));
        let original = path("mutations");
        write_index(&original, &input).expect("write");
        let bytes = fs::read(&original).expect("read");
        for (label, offset, value, expected) in [
            ("magic", 0, b'X', "wrong magic"),
            ("version", 8, 2, "unsupported version"),
            ("section", 24, 0, "overlapping or unordered sections"),
            ("count", 88, 3, "segment section length"),
        ] {
            let path = path(label);
            let mut changed = bytes.clone();
            changed[offset] = value;
            fs::write(&path, changed).expect("write mutation");
            assert!(
                IndexReader::open(&path)
                    .expect_err("must reject")
                    .to_string()
                    .contains(expected)
            );
            fs::remove_file(path).expect("remove");
        }
        let truncated = path("truncated");
        fs::write(&truncated, &bytes[..bytes.len() - 1]).expect("write truncated");
        assert!(IndexReader::open(&truncated).is_err());
        fs::remove_file(truncated).expect("remove");

        let outside = path("outside");
        let mut changed = bytes.clone();
        changed[80..88].copy_from_slice(&(bytes.len() as u64 + 1).to_le_bytes());
        fs::write(&outside, changed).expect("write outside-file mutation");
        assert!(
            IndexReader::open(&outside)
                .expect_err("must reject")
                .to_string()
                .contains("section outside file")
        );
        fs::remove_file(outside).expect("remove");

        let invalid_contig = path("contig");
        let header = decode_header(&bytes).expect("header");
        let mut changed = bytes.clone();
        changed[header.segment_offset as usize] = 0;
        fs::write(&invalid_contig, changed).expect("write contig mutation");
        assert!(IndexReader::open(&invalid_contig).is_err());
        fs::remove_file(invalid_contig).expect("remove");

        let invalid_exception = path("exception");
        let mut changed = bytes.clone();
        changed[header.exception_offset as usize + 1] = 1;
        fs::write(&invalid_exception, changed).expect("write exception mutation");
        assert!(IndexReader::open(&invalid_exception).is_err());
        fs::remove_file(invalid_exception).expect("remove");

        let root = header.roots[0];
        let root_offset = header.tree_offset as usize + root as usize * TREE_NODE_SIZE;
        let root_node = decode_tree_view(&bytes, root_offset as u64).expect("root node");
        let root_segment = decode_segment_view(
            &bytes,
            header.segment_offset + root_node.segment * SEGMENT_SIZE as u64,
        )
        .expect("root segment");
        assert!(root_node.max_end > root_segment.end);

        let low_max = path("low-max");
        let mut changed = bytes.clone();
        changed[root_offset + 24..root_offset + 28]
            .copy_from_slice(&root_segment.end.to_le_bytes());
        fs::write(&low_max, changed).expect("write low-max mutation");
        assert!(
            IndexReader::open(&low_max)
                .expect_err("must reject low subtree maximum")
                .to_string()
                .contains("tree subtree maximum")
        );
        fs::remove_file(low_max).expect("remove");

        let cycle = path("cycle");
        let mut changed = bytes.clone();
        changed[root_offset + 8..root_offset + 16].copy_from_slice(&root.to_le_bytes());
        fs::write(&cycle, changed).expect("write cycle mutation");
        assert!(
            IndexReader::open(&cycle)
                .expect_err("must reject cycle")
                .to_string()
                .contains("tree connectivity or preorder")
        );
        fs::remove_file(cycle).expect("remove");

        let segment = decode_segment_view(&bytes, header.segment_offset).expect("segment");
        let first_record = header.payload_offset + segment.payload_rel;

        let invalid_allele = path("allele");
        let mut changed = bytes.clone();
        changed[first_record as usize] = (changed[first_record as usize] & !0b111) | 0b111;
        fs::write(&invalid_allele, changed).expect("write allele mutation");
        let reader = IndexReader::open(&invalid_allele).expect("payload is lazy");
        let first = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(100).expect("position"),
            DnaBase::A,
            DnaBase::C,
        )
        .expect("snv");
        assert!(
            reader
                .lookup(first, None)
                .expect_err("must reject touched allele")
                .to_string()
                .contains("allele code")
        );
        fs::remove_file(invalid_allele).expect("remove");

        let invalid_score = path("score");
        let mut changed = bytes.clone();
        let record_start = first_record as usize;
        let mut expanded = [0_u8; 16];
        expanded[..11].copy_from_slice(&changed[record_start..record_start + 11]);
        let mut bits = u128::from_le_bytes(expanded);
        bits = (bits & !(0x7f_u128 << 3)) | (0x7f_u128 << 3);
        changed[record_start..record_start + 11].copy_from_slice(&bits.to_le_bytes()[..11]);
        fs::write(&invalid_score, changed).expect("write score mutation");
        let reader = IndexReader::open(&invalid_score).expect("payload is lazy");
        assert!(
            reader
                .lookup(first, None)
                .expect_err("must reject touched score")
                .to_string()
                .contains("fixed score value code")
        );
        fs::remove_file(invalid_score).expect("remove");

        let record = header.payload_offset + segment.payload_rel + 11;
        let corrupt = path("value");
        let mut changed = bytes.clone();
        changed[record as usize + 10] |= 0x80;
        fs::write(&corrupt, changed).expect("write value mutation");
        let reader = IndexReader::open(&corrupt).expect("structural open does not scan payload");
        let first = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(100).expect("position"),
            DnaBase::A,
            DnaBase::G,
        )
        .expect("snv");
        assert!(reader.lookup(first, None).is_ok());
        let touched = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(101).expect("position"),
            DnaBase::A,
            DnaBase::G,
        )
        .expect("snv");
        assert!(reader.lookup(touched, None).is_err());
        fs::remove_file(corrupt).expect("remove");
        fs::remove_file(original).expect("remove");
    }

    #[test]
    fn augmented_tree_is_output_sensitive_on_nested_and_disjoint_intervals() {
        let mut input = Vec::new();
        for gene in 1..=19_916_u64 {
            let text = format!("ENSG{gene:011}");
            let start = if gene <= 64 {
                gene as u32
            } else {
                10_000 + gene as u32 * 3
            };
            let count = if gene <= 64 { 200 - gene as u32 } else { 1 };
            input.extend(ordinary(&text, start, count));
        }
        let path = path("tree");
        write_index(&path, &input).expect("write");
        let reader = IndexReader::open(&path).expect("open");
        let snv = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(100).expect("position"),
            DnaBase::A,
            DnaBase::C,
        )
        .expect("snv");
        let (result, metrics) = reader.lookup_measured(snv, None).expect("lookup");
        assert_eq!(result.records.len(), 64);
        assert!(
            metrics.interval_nodes_visited < 160,
            "visited {}",
            metrics.interval_nodes_visited
        );
        let selected_gene: EnsemblGeneId = "ENSG00000019916".parse().expect("gene");
        let selected_position = 10_000 + 19_916 * 3;
        let selected = Grch38Snv::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(selected_position).expect("position"),
            DnaBase::A,
            DnaBase::C,
        )
        .expect("snv");
        let (result, metrics) = reader
            .lookup_measured(selected, Some(selected_gene))
            .expect("gene-filtered lookup");
        assert_eq!(result.records.len(), 1);
        assert_eq!(metrics.interval_nodes_visited, 0);
        assert!(
            metrics.logical_bytes_decoded < 2_000,
            "gene-filtered lookup decoded {} bytes",
            metrics.logical_bytes_decoded
        );
        fs::remove_file(path).expect("remove");
    }
}
