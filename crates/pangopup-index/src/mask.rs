//! Production runtime access to the selected GENCODE v38 mask member.
//!
//! This module is a self-contained, read-only decoder for the byte-identical
//! `domains` representation selected by Ticket 012. It deliberately contains
//! no candidate writer, codec selector, tracing, inspection, or qualification
//! operation.

use memmap2::Mmap;
use pangopup_core::{EnsemblGeneId, GencodeGeneId, GenomicPosition, Grch38Contig};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    ops::Range,
    os::unix::fs::MetadataExt,
    path::Path,
};

const MAGIC: &[u8; 8] = b"PGMBEN01";
const VERSION: u16 = 1;
const DOMAINS_CODEC: u8 = 2;
const HEADER_BYTES: usize = 160;
const DIRECTORY_BYTES: usize = 40;
const GENE_BYTES: usize = 40;
const DOMAIN_BYTES: usize = 16;
const SECTION_BYTES: usize = 24;
const SECTION_COUNT: usize = 5;
const MAX_CONTIGS: usize = 25;
const MAX_GENES: usize = 100_000;
const MAX_BOUNDARIES: usize = 10_000_000;
const MAX_BOUNDARIES_PER_GENE: usize = 100_000;
const MAX_DOMAINS: usize = 10_000_000;
const MAX_POSTINGS: usize = 10_000_000;
const MAX_MEMBER_BYTES: usize = 512 * 1024 * 1024;

/// Strand used to select Pangolin's plus or minus model output.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaskStrand {
    Plus,
    Minus,
}

impl MaskStrand {
    fn from_flag(value: u8) -> Result<Self, MaskError> {
        match value {
            0 => Ok(Self::Plus),
            1 => Ok(Self::Minus),
            _ => Err(MaskError::Invalid("strand")),
        }
    }
}

/// One containing GENCODE gene returned by a point query.
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

/// A production mask open or point-query failure.
#[derive(Debug)]
pub enum MaskError {
    Io(io::Error),
    Authentication(&'static str),
    UnsupportedCodec,
    Invalid(&'static str),
    Bounds(&'static str),
    Resource(&'static str),
    Arithmetic(&'static str),
}

impl fmt::Display for MaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("GENCODE mask I/O failed"),
            Self::Authentication(reason) => {
                write!(formatter, "GENCODE mask authentication failed: {reason}")
            }
            Self::UnsupportedCodec => {
                formatter.write_str("GENCODE mask is not the selected domains codec")
            }
            Self::Invalid(reason) => write!(formatter, "invalid GENCODE mask: {reason}"),
            Self::Bounds(reason) => write!(formatter, "GENCODE mask query out of bounds: {reason}"),
            Self::Resource(reason) => {
                write!(formatter, "GENCODE mask resource limit exceeded: {reason}")
            }
            Self::Arithmetic(reason) => {
                write!(formatter, "GENCODE mask arithmetic overflow: {reason}")
            }
        }
    }
}

/// Exact identity authenticated by a qualification open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaskMemberIdentity {
    bytes: u64,
    sha256: String,
}

impl MaskMemberIdentity {
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// One observed identity inseparably coupled to the exact retained mmap.
///
/// This is an identity receipt for caller-supplied bytes, not an installed
/// compatibility profile or an assertion that the mask is biologically
/// trusted.
pub struct IdentifiedMaskDomains {
    provider: MaskDomainsOpen,
    identity: MaskMemberIdentity,
}

impl IdentifiedMaskDomains {
    pub const fn identity(&self) -> &MaskMemberIdentity {
        &self.identity
    }

    pub fn file_len(&self) -> u64 {
        self.provider.file_len()
    }
}

impl std::error::Error for MaskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for MaskError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Reusable caller-owned storage for one mask point query.
///
/// Reserve enough gene and boundary capacity once to make warmed point queries
/// allocation-free.
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

    /// Matching plus-strand genes in authenticated upstream order.
    pub fn plus(&self) -> &[MaskQueryGene] {
        &self.genes[..self.plus_count]
    }

    /// Matching minus-strand genes in authenticated upstream order.
    pub fn minus(&self) -> &[MaskQueryGene] {
        &self.genes[self.plus_count..]
    }

    /// Normalized, ascending exon boundaries for one current result.
    ///
    /// `gene` must come from this buffer's current [`Self::plus`] or
    /// [`Self::minus`] slice. A later query replaces the buffer contents, so a
    /// cloned or retained result from an earlier query must not be used here.
    pub fn boundaries(&self, gene: &MaskQueryGene) -> &[GenomicPosition] {
        &self.boundaries[gene.boundaries.clone()]
    }

    fn clear(&mut self) {
        self.genes.clear();
        self.boundaries.clear();
        self.matches.clear();
        self.plus_count = 0;
    }
}

/// Thread-safe capability needed by Pangolin-compatible masking.
pub trait MaskProvider: Send + Sync {
    /// Query all containing genes, optionally filtering by stable Ensembl ID.
    ///
    /// A miss succeeds with empty output. Every error also clears `output`.
    fn query(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable_gene: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct Section {
    offset: u64,
    count: u64,
    stride: u32,
}

#[derive(Clone, Copy, Debug)]
struct DirectoryEntry {
    contig: u8,
    gene_start: u64,
    gene_count: u64,
    domain_start: u64,
    domain_count: u64,
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

#[derive(Clone, Copy, Debug)]
struct DomainWire {
    begin: u32,
    end: u32,
    posting_start: u32,
    posting_count: u32,
}

/// One read-only mmap of the selected `PGMBEN01` v1 domains member.
pub struct MaskDomainsOpen {
    // The qualification path hashes and maps through this exact descriptor.
    // Retain it so the authenticated inode outlives the mapped provider.
    _file: File,
    mmap: Mmap,
    sections: [Section; SECTION_COUNT],
    directories: Vec<DirectoryEntry>,
}

impl MaskDomainsOpen {
    /// Open one regular, no-follow mask member with bounded structural checks.
    ///
    /// Exact whole-file identity belongs to asset installation and is not
    /// recomputed during ordinary runtime open.
    pub fn open(path: &Path) -> Result<Self, MaskError> {
        Self::open_file(open_descriptor(path)?)
    }

    /// Identify and structurally open one explicit caller-supplied member.
    ///
    /// Hashing, pathname checks, mmap construction, and all later queries use
    /// the same held regular, single-link descriptor.
    pub fn open_identified(path: &Path) -> Result<IdentifiedMaskDomains, MaskError> {
        Self::open_identified_with(path, || {}, |_| {})
    }

    fn open_identified_with(
        path: &Path,
        after_open: impl FnOnce(),
        mut after_chunk: impl FnMut(u64),
    ) -> Result<IdentifiedMaskDomains, MaskError> {
        let mut file = open_descriptor(path)?;
        let before = checked_metadata(&file)?;
        after_open();

        file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut observed = 0_u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            observed = observed
                .checked_add(count as u64)
                .ok_or(MaskError::Authentication("member length"))?;
            if observed > before.len() {
                return Err(MaskError::Authentication("member length"));
            }
            hasher.update(&buffer[..count]);
            after_chunk(observed);
        }
        if observed != before.len() {
            return Err(MaskError::Authentication("member length"));
        }
        let after = checked_metadata(&file)?;
        validate_retained_path(path, &before, &after)?;
        let identity = MaskMemberIdentity {
            bytes: observed,
            sha256: format!("{:x}", hasher.finalize()),
        };
        let provider = Self::open_file(file)?;
        Ok(IdentifiedMaskDomains { provider, identity })
    }

    /// Authenticate and open one retained mask member for qualification.
    ///
    /// Hashing, structural validation, and mmap construction all use the same
    /// held descriptor. The pathname is checked after hashing so replacement
    /// during the operation fails instead of producing a misleading receipt.
    /// As required by ADR 0013, the verified inode must remain immutable;
    /// concurrent in-place mutation or truncation is outside the threat model.
    pub fn open_qualification(
        path: &Path,
        expected_bytes: u64,
        expected_sha256: &str,
    ) -> Result<(Self, MaskMemberIdentity), MaskError> {
        Self::open_qualification_with(path, expected_bytes, expected_sha256, || {})
    }

    fn open_qualification_with(
        path: &Path,
        expected_bytes: u64,
        expected_sha256: &str,
        after_open: impl FnOnce(),
    ) -> Result<(Self, MaskMemberIdentity), MaskError> {
        if expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(MaskError::Authentication("expected SHA-256"));
        }
        let mut file = open_descriptor(path)?;
        let before = checked_metadata(&file)?;
        if before.len() != expected_bytes {
            return Err(MaskError::Authentication("member length"));
        }

        after_open();

        file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut observed = 0_u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            observed = observed
                .checked_add(count as u64)
                .ok_or(MaskError::Authentication("member length"))?;
            if observed > expected_bytes {
                return Err(MaskError::Authentication("member length"));
            }
            hasher.update(&buffer[..count]);
        }
        if observed != expected_bytes {
            return Err(MaskError::Authentication("member length"));
        }
        let sha256 = format!("{:x}", hasher.finalize());
        if sha256 != expected_sha256 {
            return Err(MaskError::Authentication("member SHA-256"));
        }

        let after = checked_metadata(&file)?;
        validate_retained_path(path, &before, &after)?;

        let identity = MaskMemberIdentity {
            bytes: observed,
            sha256,
        };
        let provider = Self::open_file(file)?;
        Ok((provider, identity))
    }

    fn open_file(file: File) -> Result<Self, MaskError> {
        let metadata = checked_metadata(&file)?;
        let length =
            usize::try_from(metadata.len()).map_err(|_| MaskError::Resource("member bytes"))?;

        // SAFETY: the read-only map is owned by this reader and every byte
        // access is bounds checked before decoding.
        let mmap = unsafe { Mmap::map(&file)? };
        let header = mmap
            .get(..HEADER_BYTES)
            .ok_or(MaskError::Invalid("header"))?;
        if &header[0..8] != MAGIC
            || le_u16(header, 8)? != VERSION
            || le_u64(header, 16)? != length as u64
            || le_u64(header, 24)? != HEADER_BYTES as u64
            || header[12..16].iter().any(|byte| *byte != 0)
            || header[152..160].iter().any(|byte| *byte != 0)
        {
            return Err(MaskError::Invalid("header fields"));
        }
        match header[10] {
            DOMAINS_CODEC => {}
            1 | 3 => return Err(MaskError::UnsupportedCodec),
            _ => return Err(MaskError::Invalid("codec")),
        }

        let directory_count = header[11] as usize;
        if directory_count == 0 || directory_count > MAX_CONTIGS {
            return Err(MaskError::Invalid("contig count"));
        }
        let expected_strides = [DIRECTORY_BYTES, GENE_BYTES, 4, DOMAIN_BYTES, 4];
        let mut sections = [Section::default(); SECTION_COUNT];
        let mut cursor = HEADER_BYTES as u64;
        for index in 0..SECTION_COUNT {
            let offset = 32 + index * SECTION_BYTES;
            if header[offset + 20..offset + 24]
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(MaskError::Invalid("section reserved"));
            }
            let section = Section {
                offset: le_u64(header, offset)?,
                count: le_u64(header, offset + 8)?,
                stride: le_u32(header, offset + 16)?,
            };
            if section.offset != cursor || section.stride as usize != expected_strides[index] {
                return Err(MaskError::Invalid("section layout"));
            }
            let bytes = section
                .count
                .checked_mul(section.stride as u64)
                .ok_or(MaskError::Arithmetic("section size"))?;
            cursor = cursor
                .checked_add(bytes)
                .ok_or(MaskError::Arithmetic("section end"))?;
            if cursor > length as u64 {
                return Err(MaskError::Invalid("section bounds"));
            }
            sections[index] = section;
        }
        if cursor != length as u64
            || sections[0].count != directory_count as u64
            || sections[1].count == 0
            || sections[1].count > MAX_GENES as u64
            || sections[2].count > MAX_BOUNDARIES as u64
            || sections[3].count == 0
            || sections[3].count > MAX_DOMAINS as u64
            || sections[4].count > MAX_POSTINGS as u64
        {
            return Err(MaskError::Invalid("section counts"));
        }

        let mut directories = Vec::with_capacity(directory_count);
        let mut prior_contig = 0_u8;
        let mut expected_gene = 0_u64;
        let mut expected_domain = 0_u64;
        for index in 0..directory_count {
            let offset = section_offset(&sections[0], index, DIRECTORY_BYTES)?;
            let bytes = mapped_range(&mmap, offset, DIRECTORY_BYTES, "directory")?;
            if bytes[1..8].iter().any(|byte| *byte != 0) {
                return Err(MaskError::Invalid("directory reserved"));
            }
            let entry = DirectoryEntry {
                contig: bytes[0],
                gene_start: le_u64(bytes, 8)?,
                gene_count: le_u64(bytes, 16)?,
                domain_start: le_u64(bytes, 24)?,
                domain_count: le_u64(bytes, 32)?,
            };
            Grch38Contig::from_code(entry.contig).map_err(|_| MaskError::Invalid("contig code"))?;
            if entry.contig <= prior_contig
                || entry.gene_count == 0
                || entry.domain_count == 0
                || entry.gene_start != expected_gene
                || entry.domain_start != expected_domain
            {
                return Err(MaskError::Invalid("directory order or counts"));
            }
            expected_gene = expected_gene
                .checked_add(entry.gene_count)
                .ok_or(MaskError::Arithmetic("gene directory"))?;
            expected_domain = expected_domain
                .checked_add(entry.domain_count)
                .ok_or(MaskError::Arithmetic("domain directory"))?;
            if expected_gene > sections[1].count || expected_domain > sections[3].count {
                return Err(MaskError::Invalid("directory range"));
            }
            prior_contig = entry.contig;
            directories.push(entry);
        }
        if expected_gene != sections[1].count || expected_domain != sections[3].count {
            return Err(MaskError::Invalid("directory coverage"));
        }

        Ok(Self {
            _file: file,
            mmap,
            sections,
            directories,
        })
    }

    pub fn file_len(&self) -> u64 {
        self.mmap.len() as u64
    }

    fn query_inner(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable_gene: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskError> {
        let Some(directory) = self
            .directories
            .binary_search_by_key(&contig.code(), |entry| entry.contig)
            .ok()
            .and_then(|index| self.directories.get(index))
        else {
            return Ok(());
        };

        let mut low = 0_u64;
        let mut high = directory.domain_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let domain = self.read_domain(directory.domain_start + middle)?;
            if domain.begin <= position.get() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == 0 {
            return Ok(());
        }
        let domain = self.read_domain(directory.domain_start + low - 1)?;
        if position.get() > domain.end {
            return Ok(());
        }

        let posting_end = (domain.posting_start as u64)
            .checked_add(domain.posting_count as u64)
            .ok_or(MaskError::Arithmetic("posting range"))?;
        if posting_end > self.sections[4].count {
            return Err(MaskError::Invalid("posting range"));
        }
        for posting_index in domain.posting_start as u64..posting_end {
            let offset = section_offset(&self.sections[4], posting_index as usize, 4)?;
            let gene_index = le_u32(mapped_range(&self.mmap, offset, 4, "posting record")?, 0)?;
            if !contains_gene(directory, gene_index) {
                return Err(MaskError::Invalid("posting gene range"));
            }
            let gene = self.read_gene(gene_index as u64)?;
            if gene.contig != contig.code()
                || position.get() <= gene.start
                || position.get() > gene.end
            {
                return Err(MaskError::Invalid("posting gene contig or span"));
            }
            output.matches.push(gene_index);
        }

        output.matches.sort_unstable_by_key(|index| {
            self.read_gene(*index as u64)
                .map_or(u32::MAX, |gene| gene.rank)
        });
        for strand in [MaskStrand::Plus, MaskStrand::Minus] {
            let mut prior_rank = None;
            for match_index in 0..output.matches.len() {
                let index = output.matches[match_index];
                let gene = self.read_gene(index as u64)?;
                if gene.contig != contig.code()
                    || position.get() <= gene.start
                    || position.get() > gene.end
                {
                    return Err(MaskError::Invalid("posting gene contig or span"));
                }
                if gene.strand != strand {
                    continue;
                }
                if prior_rank.is_some_and(|prior| gene.rank <= prior) {
                    return Err(MaskError::Invalid("gene rank order"));
                }
                prior_rank = Some(gene.rank);
                let identity = self.gene_identity(gene)?;
                if stable_gene.is_none_or(|filter| filter == identity.stable()) {
                    self.push_gene(gene, identity, output)?;
                }
            }
            if strand == MaskStrand::Plus {
                output.plus_count = output.genes.len();
            }
        }
        Ok(())
    }

    fn read_domain(&self, index: u64) -> Result<DomainWire, MaskError> {
        if index >= self.sections[3].count {
            return Err(MaskError::Invalid("domain range"));
        }
        let offset = section_offset(&self.sections[3], index as usize, DOMAIN_BYTES)?;
        let bytes = mapped_range(&self.mmap, offset, DOMAIN_BYTES, "domain record")?;
        let domain = DomainWire {
            begin: le_u32(bytes, 0)?,
            end: le_u32(bytes, 4)?,
            posting_start: le_u32(bytes, 8)?,
            posting_count: le_u32(bytes, 12)?,
        };
        if domain.begin == 0 || domain.begin > domain.end || domain.posting_count == 0 {
            return Err(MaskError::Invalid("domain record"));
        }
        Ok(domain)
    }

    fn read_gene(&self, index: u64) -> Result<GeneWire, MaskError> {
        if index >= self.sections[1].count {
            return Err(MaskError::Invalid("gene index"));
        }
        let offset = section_offset(&self.sections[1], index as usize, GENE_BYTES)?;
        let bytes = mapped_range(&self.mmap, offset, GENE_BYTES, "gene record")?;
        if bytes[2..4].iter().any(|byte| *byte != 0)
            || bytes[36..40].iter().any(|byte| *byte != 0)
            || bytes[1] & !3 != 0
        {
            return Err(MaskError::Invalid("gene reserved"));
        }
        let start = le_u32(bytes, 4)?;
        let end = le_u32(bytes, 8)?;
        if start == 0 || start > end {
            return Err(MaskError::Invalid("gene span"));
        }
        Grch38Contig::from_code(bytes[0]).map_err(|_| MaskError::Invalid("gene contig"))?;
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

    fn gene_identity(&self, gene: GeneWire) -> Result<GencodeGeneId, MaskError> {
        let stable = EnsemblGeneId::from_numeric(gene.stable)
            .map_err(|_| MaskError::Invalid("stable gene identity"))?;
        GencodeGeneId::new(stable, gene.version, gene.par_y)
            .map_err(|_| MaskError::Invalid("exact gene identity"))
    }

    fn push_gene(
        &self,
        gene: GeneWire,
        identity: GencodeGeneId,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskError> {
        let boundary_end = (gene.boundary_start as u64)
            .checked_add(gene.boundary_count as u64)
            .ok_or(MaskError::Arithmetic("boundary range"))?;
        if boundary_end > self.sections[2].count
            || gene.boundary_count as usize > MAX_BOUNDARIES_PER_GENE
        {
            return Err(MaskError::Invalid("boundary range"));
        }

        let boundary_begin = output.boundaries.len();
        let mut prior = 0_u32;
        for boundary_index in gene.boundary_start as u64..boundary_end {
            let offset = section_offset(&self.sections[2], boundary_index as usize, 4)?;
            let boundary = le_u32(mapped_range(&self.mmap, offset, 4, "exon boundary")?, 0)?;
            if boundary < gene.start || boundary > gene.end || (prior != 0 && boundary <= prior) {
                return Err(MaskError::Invalid("exon boundaries"));
            }
            output.boundaries.push(
                GenomicPosition::new(boundary).map_err(|_| MaskError::Invalid("exon boundary"))?,
            );
            prior = boundary;
        }
        let boundary_end = output.boundaries.len();
        output.genes.push(MaskQueryGene {
            identity,
            contig: Grch38Contig::from_code(gene.contig)
                .map_err(|_| MaskError::Invalid("gene contig"))?,
            strand: gene.strand,
            start: GenomicPosition::new(gene.start)
                .map_err(|_| MaskError::Invalid("gene start"))?,
            end: GenomicPosition::new(gene.end).map_err(|_| MaskError::Invalid("gene end"))?,
            query_rank: gene.rank,
            boundaries: boundary_begin..boundary_end,
        });
        Ok(())
    }
}

fn open_descriptor(path: &Path) -> Result<File, MaskError> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| MaskError::Io(io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(descriptor))
}

fn checked_metadata(file: &File) -> Result<fs::Metadata, MaskError> {
    let metadata = file.metadata()?;
    let length =
        usize::try_from(metadata.len()).map_err(|_| MaskError::Resource("member bytes"))?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || !(HEADER_BYTES..=MAX_MEMBER_BYTES).contains(&length)
    {
        return Err(MaskError::Invalid("member length or type"));
    }
    Ok(metadata)
}

fn same_file_state(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn validate_retained_path(
    path: &Path,
    before: &fs::Metadata,
    after: &fs::Metadata,
) -> Result<(), MaskError> {
    if !same_file_state(before, after) {
        return Err(MaskError::Authentication("member changed during hashing"));
    }
    let path_metadata = fs::symlink_metadata(path)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || path_metadata.nlink() != 1
        || path_metadata.dev() != after.dev()
        || path_metadata.ino() != after.ino()
    {
        return Err(MaskError::Authentication("path replaced during hashing"));
    }
    Ok(())
}

impl MaskProvider for MaskDomainsOpen {
    #[inline]
    fn query(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable_gene: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskError> {
        output.clear();
        let result = self.query_inner(contig, position, stable_gene, output);
        if result.is_err() {
            output.clear();
        }
        result
    }
}

impl MaskProvider for IdentifiedMaskDomains {
    #[inline]
    fn query(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        stable_gene: Option<EnsemblGeneId>,
        output: &mut MaskQueryBuffer,
    ) -> Result<(), MaskError> {
        self.provider.query(contig, position, stable_gene, output)
    }
}

fn contains_gene(directory: &DirectoryEntry, index: u32) -> bool {
    let index = index as u64;
    index >= directory.gene_start
        && index < directory.gene_start.saturating_add(directory.gene_count)
}

fn section_offset(section: &Section, index: usize, stride: usize) -> Result<usize, MaskError> {
    if index as u64 >= section.count {
        return Err(MaskError::Invalid("section index"));
    }
    let relative = index
        .checked_mul(stride)
        .ok_or(MaskError::Arithmetic("section index"))?;
    usize::try_from(section.offset)
        .ok()
        .and_then(|offset| offset.checked_add(relative))
        .ok_or(MaskError::Arithmetic("section offset"))
}

fn mapped_range<'a>(
    mmap: &'a [u8],
    offset: usize,
    length: usize,
    reason: &'static str,
) -> Result<&'a [u8], MaskError> {
    mmap.get(
        offset
            ..offset
                .checked_add(length)
                .ok_or(MaskError::Arithmetic("mapped range"))?,
    )
    .ok_or(MaskError::Invalid(reason))
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, MaskError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(MaskError::Invalid("u16"))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, MaskError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(MaskError::Invalid("u32"))
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, MaskError> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(MaskError::Invalid("u64"))
}

#[cfg(test)]
mod qualification_tests {
    use super::*;
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::symlink,
        path::PathBuf,
    };

    const FIXTURE_BYTES: u64 = 880;
    const FIXTURE_SHA256: &str = "76d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43";

    #[test]
    fn replacement_after_descriptor_open_fails_qualification() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/gencode-mask-mini/domains.pgm");
        let scratch = std::env::temp_dir().join(format!(
            "pangopup-mask-qualification-replace-{}",
            std::process::id()
        ));
        if scratch.exists() {
            fs::remove_dir_all(&scratch).expect("remove stale scratch");
        }
        fs::create_dir(&scratch).expect("create scratch");
        let path = scratch.join("domains.pgm");
        let held = scratch.join("opened.pgm");
        fs::copy(&fixture, &path).expect("copy fixture");

        let replacement_path = path.clone();
        let replacement_fixture = fixture.clone();
        let result =
            MaskDomainsOpen::open_qualification_with(&path, FIXTURE_BYTES, FIXTURE_SHA256, || {
                fs::rename(&replacement_path, &held).expect("rename opened member");
                fs::copy(&replacement_fixture, &replacement_path).expect("replace path");
            });
        assert!(matches!(
            result,
            Err(MaskError::Authentication(
                "member changed during hashing" | "path replaced during hashing"
            ))
        ));
        fs::remove_dir_all(&scratch).expect("remove scratch");
    }

    #[test]
    fn identified_open_rejects_symlink_mutation_and_replacement() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/gencode-mask-mini/domains.pgm");
        let scratch = std::env::temp_dir().join(format!(
            "pangopup-mask-identified-controls-{}",
            std::process::id()
        ));
        if scratch.exists() {
            fs::remove_dir_all(&scratch).expect("remove stale scratch");
        }
        fs::create_dir(&scratch).expect("create scratch");

        let path = scratch.join("domains.pgm");
        fs::copy(&fixture, &path).expect("copy fixture");
        let link = scratch.join("linked.pgm");
        symlink(&path, &link).expect("create symlink");
        assert!(MaskDomainsOpen::open_identified(&link).is_err());

        let mutation_path = path.clone();
        let mut mutated = false;
        let result = MaskDomainsOpen::open_identified_with(
            &path,
            || {},
            |_| {
                if !mutated {
                    let mut file = File::options()
                        .write(true)
                        .open(&mutation_path)
                        .expect("open mutation target");
                    file.seek(SeekFrom::Start(200)).expect("seek mutation");
                    file.write_all(&[1]).expect("mutate during hash");
                    file.sync_all().expect("sync mutation");
                    mutated = true;
                }
            },
        );
        assert!(matches!(
            result,
            Err(MaskError::Authentication("member changed during hashing"))
        ));

        fs::copy(&fixture, &path).expect("restore fixture");
        let replacement_path = path.clone();
        let held = scratch.join("opened.pgm");
        let replacement_fixture = fixture.clone();
        let mut replaced = false;
        let result = MaskDomainsOpen::open_identified_with(
            &path,
            || {},
            |_| {
                if !replaced {
                    fs::rename(&replacement_path, &held).expect("rename opened member");
                    fs::copy(&replacement_fixture, &replacement_path).expect("replace path");
                    replaced = true;
                }
            },
        );
        assert!(matches!(
            result,
            Err(MaskError::Authentication(
                "member changed during hashing" | "path replaced during hashing"
            ))
        ));
        fs::remove_dir_all(&scratch).expect("remove scratch");
    }

    #[test]
    fn identified_open_reports_and_queries_the_same_retained_descriptor() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/gencode-mask-mini/domains.pgm");
        let scratch = std::env::temp_dir().join(format!(
            "pangopup-mask-identified-held-{}",
            std::process::id()
        ));
        if scratch.exists() {
            fs::remove_dir_all(&scratch).expect("remove stale scratch");
        }
        fs::create_dir(&scratch).expect("create scratch");
        let path = scratch.join("domains.pgm");
        fs::copy(&fixture, &path).expect("copy fixture");

        let identified = MaskDomainsOpen::open_identified(&path).expect("identified open");
        assert_eq!(identified.identity().bytes(), FIXTURE_BYTES);
        assert_eq!(identified.identity().sha256(), FIXTURE_SHA256);
        let retained = scratch.join("retained.pgm");
        fs::rename(&path, &retained).expect("rename identified path");
        fs::write(&path, b"replacement").expect("replace pathname");

        let mut output = MaskQueryBuffer::default();
        identified
            .query(
                Grch38Contig::autosome(1).expect("chr1"),
                GenomicPosition::new(2).expect("position"),
                None,
                &mut output,
            )
            .expect("query retained mmap");
        assert_eq!(output.plus()[0].identity().to_string(), "ENSG00000000001.1");
        fs::remove_dir_all(&scratch).expect("remove scratch");
    }
}
