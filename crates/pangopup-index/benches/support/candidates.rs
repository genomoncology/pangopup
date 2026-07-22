#![allow(dead_code)]

use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use memmap2::Mmap;
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, PangolinScore,
    RelativePosition, ScoreMagnitude,
};
use pangopup_index::{
    AmbiguousInputLocus, IndexReader, InputAlternative, InputLocus, OrdinaryInputLocus, write_index,
};
use std::{
    env,
    error::Error,
    fs::{self, File},
    hint::black_box,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const MAGIC: &[u8; 8] = b"PGCAND01";
const HEADER: usize = 80;
const SEGMENT: usize = 40;
const NODE: usize = 32;
const BLOCK: usize = 32;
const EXCEPTION: usize = 40;
const NONE: u64 = u64::MAX;
const DIRECT_BLOCK_LOCI: usize = 4096;
const DIRECT_RANK_STRIDE: usize = 64;
const DIRECT_HEADER: usize = 8;
const DIRECT_RANK: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Codec {
    Direct,
    Fixed,
    Zstd(u32),
    Lz4(u32),
}

impl Codec {
    fn name(self) -> String {
        match self {
            Self::Direct => "direct-sparse".into(),
            Self::Fixed => "fixed-11".into(),
            Self::Zstd(size) => format!("zstd-{size}"),
            Self::Lz4(size) => format!("lz4-{size}"),
        }
    }

    fn block_size(self) -> usize {
        match self {
            Self::Direct => DIRECT_BLOCK_LOCI,
            Self::Fixed => usize::MAX,
            Self::Zstd(size) | Self::Lz4(size) => size as usize,
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Direct => 3,
            Self::Fixed => 0,
            Self::Zstd(_) => 1,
            Self::Lz4(_) => 2,
        }
    }
}

#[derive(Clone)]
struct Locus {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    position: GenomicPosition,
    reference: DnaBase,
    alternatives: [InputAlternative; 3],
}

#[derive(Clone, Copy)]
struct SegmentMeta {
    gene: EnsemblGeneId,
    contig: Grch38Contig,
    start: u32,
    end: u32,
    locus_start: u64,
    loci: u32,
    block_start: u32,
    blocks: u32,
}

#[derive(Clone, Copy)]
struct NodeMeta {
    segment: u64,
    left: u64,
    right: u64,
    max_end: u32,
    contig: Grch38Contig,
}

#[derive(Clone, Copy)]
struct BlockMeta {
    segment: u32,
    first: u32,
    count: u32,
    encoding: u8,
    payload_offset: u64,
    payload_len: u32,
    raw_len: u32,
}

struct Canonical {
    loci: Vec<Locus>,
    segments: Vec<SegmentMeta>,
    nodes: Vec<NodeMeta>,
    exceptions: Vec<AmbiguousInputLocus>,
}

pub struct CandidateReader {
    codec: Codec,
    map: Mmap,
    segments: Vec<SegmentMeta>,
    nodes: Vec<NodeMeta>,
    blocks: Vec<BlockMeta>,
    roots: [u64; 25],
    payload_offset: u64,
    exception_offset: u64,
    exception_count: u64,
}

type TabixInner =
    noodles_csi::io::IndexedReader<noodles_bgzf::io::Reader<File>, noodles_tabix::Index>;

struct TabixReader {
    inner: TabixInner,
}

impl TabixReader {
    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            inner: noodles_tabix::io::indexed_reader::Builder::default().build_from_path(path)?,
        })
    }

    fn lookup(
        &mut self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
    ) -> Result<usize, Box<dyn Error>> {
        let region = format!("{}:{}-{}", snv.contig(), snv.position(), snv.position()).parse()?;
        let mut records = 0;
        for result in self.inner.query(&region)? {
            let record = result?;
            let fields: Vec<_> = record.as_ref().split('\t').collect();
            if fields.len() != 9 {
                return Err("Tabix row field count".into());
            }
            if fields[2] == snv.reference().to_string()
                && fields[3] == snv.alternate().to_string()
                && gene.is_none_or(|expected| fields[8] == expected.to_string())
            {
                fields[4].parse::<u8>()?;
                fields[5].parse::<i8>()?;
                fields[6].parse::<u8>()?;
                fields[7].parse::<i8>()?;
                records += 1;
            }
        }
        Ok(records)
    }

    fn lookup_ambiguous(
        &mut self,
        contig: Grch38Contig,
        position: GenomicPosition,
        gene: EnsemblGeneId,
    ) -> Result<usize, Box<dyn Error>> {
        let region = format!("{contig}:{position}-{position}").parse()?;
        let mut found = false;
        for result in self.inner.query(&region)? {
            let record = result?;
            let fields: Vec<_> = record.as_ref().split('\t').collect();
            if fields.len() == 9 && fields[2] == "N" && fields[8] == gene.to_string() {
                found = true;
            }
        }
        Ok(usize::from(found))
    }
}

struct SpecialQueries {
    cross_block: Vec<(Grch38Snv, EnsemblGeneId)>,
    overlap: Grch38Snv,
    absent: (Grch38Snv, EnsemblGeneId),
    ambiguous: (Grch38Contig, GenomicPosition, EnsemblGeneId),
}

#[derive(Default)]
pub struct CandidateWork {
    pub logical_bytes: u64,
    pub pages: Vec<u64>,
    pub directory_entries: u64,
}

impl CandidateWork {
    pub fn unique_pages(&mut self) -> usize {
        self.pages.sort_unstable();
        self.pages.dedup();
        self.pages.len()
    }

    fn touch(&mut self, offset: u64, length: u64) {
        self.logical_bytes += length;
        if length == 0 {
            return;
        }
        for page in offset / 4096..=(offset + length - 1) / 4096 {
            self.pages.push(page);
        }
    }
}

pub fn write_candidate(
    path: &Path,
    codec: Codec,
    input: &[InputLocus],
) -> Result<u64, Box<dyn Error>> {
    let mut corpus = canonical(input)?;
    let mut payload = Vec::new();
    let mut blocks = Vec::new();
    match codec {
        Codec::Fixed => {
            for locus in &corpus.loci {
                payload.extend_from_slice(&fixed_record(locus));
            }
            for (segment_index, segment) in corpus.segments.iter_mut().enumerate() {
                segment.block_start = u32::try_from(blocks.len())?;
                segment.blocks = 1;
                blocks.push(BlockMeta {
                    segment: u32::try_from(segment_index)?,
                    first: 0,
                    count: segment.loci,
                    encoding: 0,
                    payload_offset: segment.locus_start * 11,
                    payload_len: segment.loci.checked_mul(11).ok_or("fixed length")?,
                    raw_len: segment.loci.checked_mul(11).ok_or("fixed length")?,
                });
            }
        }
        Codec::Direct => {
            for (segment_index, segment) in corpus.segments.iter_mut().enumerate() {
                segment.block_start = u32::try_from(blocks.len())?;
                let start = usize::try_from(segment.locus_start)?;
                let end = start + segment.loci as usize;
                for (block_number, chunk) in corpus.loci[start..end]
                    .chunks(DIRECT_BLOCK_LOCI)
                    .enumerate()
                {
                    let raw = hierarchical_block(chunk);
                    let payload_offset = u64::try_from(payload.len())?;
                    payload.extend_from_slice(&raw);
                    blocks.push(BlockMeta {
                        segment: u32::try_from(segment_index)?,
                        first: u32::try_from(block_number * DIRECT_BLOCK_LOCI)?,
                        count: u32::try_from(chunk.len())?,
                        encoding: 3,
                        payload_offset,
                        payload_len: u32::try_from(raw.len())?,
                        raw_len: u32::try_from(raw.len())?,
                    });
                }
                segment.blocks = u32::try_from(blocks.len())? - segment.block_start;
            }
        }
        Codec::Zstd(_) | Codec::Lz4(_) => {
            let block_size = codec.block_size();
            for (segment_index, segment) in corpus.segments.iter_mut().enumerate() {
                segment.block_start = u32::try_from(blocks.len())?;
                let start = usize::try_from(segment.locus_start)?;
                let end = start + segment.loci as usize;
                for (block_number, chunk) in corpus.loci[start..end].chunks(block_size).enumerate()
                {
                    let raw = sparse_block(chunk);
                    let compressed = match codec {
                        Codec::Zstd(_) => zstd::bulk::compress(&raw, 1)?,
                        Codec::Lz4(_) => compress_prepend_size(&raw),
                        Codec::Direct | Codec::Fixed => unreachable!(),
                    };
                    let (encoding, stored) = if compressed.len() < raw.len() {
                        (codec.code(), compressed)
                    } else {
                        (0, raw.clone())
                    };
                    let payload_offset = u64::try_from(payload.len())?;
                    payload.extend_from_slice(&stored);
                    blocks.push(BlockMeta {
                        segment: u32::try_from(segment_index)?,
                        first: u32::try_from(block_number * block_size)?,
                        count: u32::try_from(chunk.len())?,
                        encoding,
                        payload_offset,
                        payload_len: u32::try_from(stored.len())?,
                        raw_len: u32::try_from(raw.len())?,
                    });
                }
                segment.blocks = u32::try_from(blocks.len())? - segment.block_start;
            }
        }
    }

    let segment_offset = HEADER as u64;
    let node_offset = segment_offset + corpus.segments.len() as u64 * SEGMENT as u64;
    let block_offset = node_offset + corpus.nodes.len() as u64 * NODE as u64;
    let payload_offset = block_offset + blocks.len() as u64 * BLOCK as u64;
    let exception_offset = payload_offset + payload.len() as u64;
    let file_len = exception_offset + corpus.exceptions.len() as u64 * EXCEPTION as u64;
    let mut bytes = vec![0_u8; HEADER];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8] = codec.code();
    put_u32(
        &mut bytes,
        12,
        u32::try_from(codec.block_size()).unwrap_or(u32::MAX),
    );
    for (offset, value) in [
        (16, file_len),
        (24, corpus.segments.len() as u64),
        (32, corpus.nodes.len() as u64),
        (40, blocks.len() as u64),
        (48, payload_offset),
        (56, exception_offset),
        (64, corpus.exceptions.len() as u64),
    ] {
        put_u64(&mut bytes, offset, value);
    }
    for segment in &corpus.segments {
        let start = bytes.len();
        bytes.resize(start + SEGMENT, 0);
        put_u64(&mut bytes, start, segment.gene.numeric());
        bytes[start + 8] = segment.contig.code();
        put_u32(&mut bytes, start + 12, segment.start);
        put_u32(&mut bytes, start + 16, segment.end);
        put_u64(&mut bytes, start + 20, segment.locus_start);
        put_u32(&mut bytes, start + 28, segment.loci);
        put_u32(&mut bytes, start + 32, segment.block_start);
        put_u32(&mut bytes, start + 36, segment.blocks);
    }
    for node in &corpus.nodes {
        let start = bytes.len();
        bytes.resize(start + NODE, 0);
        put_u64(&mut bytes, start, node.segment);
        put_u64(&mut bytes, start + 8, node.left);
        put_u64(&mut bytes, start + 16, node.right);
        put_u32(&mut bytes, start + 24, node.max_end);
        bytes[start + 28] = node.contig.code();
    }
    for block in &blocks {
        let start = bytes.len();
        bytes.resize(start + BLOCK, 0);
        put_u32(&mut bytes, start, block.segment);
        put_u32(&mut bytes, start + 4, block.first);
        put_u32(&mut bytes, start + 8, block.count);
        bytes[start + 12] = block.encoding;
        put_u64(&mut bytes, start + 16, block.payload_offset);
        put_u32(&mut bytes, start + 24, block.payload_len);
        put_u32(&mut bytes, start + 28, block.raw_len);
    }
    bytes.extend_from_slice(&payload);
    for exception in &corpus.exceptions {
        encode_exception(&mut bytes, exception);
    }
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(file_len)
}

impl CandidateReader {
    pub fn open(path: &Path, codec: Codec) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        // SAFETY: benchmark artifacts are immutable for the lifetime of the reader.
        let map = unsafe { Mmap::map(&file)? };
        if map.get(..8) != Some(MAGIC) || map.get(8).copied() != Some(codec.code()) {
            return Err("candidate header".into());
        }
        let file_len = get_u64(&map, 16)?;
        if file_len != map.len() as u64 {
            return Err("candidate length".into());
        }
        let segment_count = get_u64(&map, 24)?;
        let node_count = get_u64(&map, 32)?;
        let block_count = get_u64(&map, 40)?;
        let payload_offset = get_u64(&map, 48)?;
        let exception_offset = get_u64(&map, 56)?;
        let exception_count = get_u64(&map, 64)?;
        let mut segments = Vec::with_capacity(segment_count as usize);
        for index in 0..segment_count {
            let start = HEADER + index as usize * SEGMENT;
            segments.push(SegmentMeta {
                gene: EnsemblGeneId::from_numeric(get_u64(&map, start)?).map_err(|_| "gene")?,
                contig: Grch38Contig::from_code(map[start + 8]).map_err(|_| "contig")?,
                start: get_u32(&map, start + 12)?,
                end: get_u32(&map, start + 16)?,
                locus_start: get_u64(&map, start + 20)?,
                loci: get_u32(&map, start + 28)?,
                block_start: get_u32(&map, start + 32)?,
                blocks: get_u32(&map, start + 36)?,
            });
        }
        let node_base = HEADER + segment_count as usize * SEGMENT;
        let mut nodes = Vec::with_capacity(node_count as usize);
        for index in 0..node_count {
            let start = node_base + index as usize * NODE;
            nodes.push(NodeMeta {
                segment: get_u64(&map, start)?,
                left: get_u64(&map, start + 8)?,
                right: get_u64(&map, start + 16)?,
                max_end: get_u32(&map, start + 24)?,
                contig: Grch38Contig::from_code(map[start + 28]).map_err(|_| "node contig")?,
            });
        }
        let block_base = node_base + node_count as usize * NODE;
        let mut blocks = Vec::with_capacity(block_count as usize);
        for index in 0..block_count {
            let start = block_base + index as usize * BLOCK;
            blocks.push(BlockMeta {
                segment: get_u32(&map, start)?,
                first: get_u32(&map, start + 4)?,
                count: get_u32(&map, start + 8)?,
                encoding: map[start + 12],
                payload_offset: get_u64(&map, start + 16)?,
                payload_len: get_u32(&map, start + 24)?,
                raw_len: get_u32(&map, start + 28)?,
            });
        }
        let mut roots = [NONE; 25];
        let mut has_parent = vec![false; nodes.len()];
        for node in &nodes {
            for child in [node.left, node.right] {
                if child != NONE {
                    has_parent[child as usize] = true;
                }
            }
        }
        for (index, node) in nodes.iter().enumerate() {
            if !has_parent[index] {
                roots[usize::from(node.contig.code() - 1)] = index as u64;
            }
        }
        Ok(Self {
            codec,
            map,
            segments,
            nodes,
            blocks,
            roots,
            payload_offset,
            exception_offset,
            exception_count,
        })
    }

    fn open_metrics(&self) -> (u64, u64) {
        let logical = HEADER as u64
            + self.segments.len() as u64 * SEGMENT as u64
            + self.nodes.len() as u64 * NODE as u64
            + self.blocks.len() as u64 * BLOCK as u64;
        let pages = self.payload_offset.div_ceil(4096);
        (logical, pages)
    }

    pub fn lookup(
        &self,
        snv: Grch38Snv,
        gene: Option<EnsemblGeneId>,
        work: &mut CandidateWork,
    ) -> Result<Vec<(EnsemblGeneId, PangolinScore)>, Box<dyn Error>> {
        let mut output = Vec::new();
        if let Some(gene) = gene {
            let target = (gene.numeric(), snv.contig().code(), snv.position().get());
            let mut low = 0_usize;
            let mut high = self.segments.len();
            while low < high {
                let middle = low + (high - low) / 2;
                work.directory_entries += 1;
                let segment = self.segments[middle];
                let key = (segment.gene.numeric(), segment.contig.code(), segment.start);
                if key <= target {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            if low != 0 {
                work.directory_entries += 1;
                let segment = self.segments[low - 1];
                if segment.gene == gene
                    && segment.contig == snv.contig()
                    && segment.start <= snv.position().get()
                    && snv.position().get() <= segment.end
                {
                    self.decode(low - 1, snv, work, &mut output)?;
                }
            }
        } else {
            let root = self.roots[usize::from(snv.contig().code() - 1)];
            if root != NONE {
                self.query(root, snv, work, &mut output)?;
            }
        }
        Ok(output)
    }

    pub fn exception_records(&self) -> Result<Vec<AmbiguousInputLocus>, Box<dyn Error>> {
        let mut records = Vec::with_capacity(self.exception_count as usize);
        let mut work = None;
        for index in 0..self.exception_count {
            records.push(self.exception_at(index, &mut work)?);
        }
        Ok(records)
    }

    fn ambiguity_count(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        gene: EnsemblGeneId,
        mut work: Option<&mut CandidateWork>,
    ) -> Result<usize, Box<dyn Error>> {
        let target = (contig.code(), position.get(), gene.numeric());
        let mut low = 0_u64;
        let mut high = self.exception_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let record = self.exception_at(middle, &mut work)?;
            let key = (
                record.contig.code(),
                record.position.get(),
                record.gene.numeric(),
            );
            if key < target {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == self.exception_count {
            return Ok(0);
        }
        let record = self.exception_at(low, &mut work)?;
        Ok(usize::from(
            (
                record.contig.code(),
                record.position.get(),
                record.gene.numeric(),
            ) == target,
        ))
    }

    fn exception_at(
        &self,
        index: u64,
        work: &mut Option<&mut CandidateWork>,
    ) -> Result<AmbiguousInputLocus, Box<dyn Error>> {
        if index >= self.exception_count {
            return Err("exception index".into());
        }
        let offset = self.exception_offset + index * EXCEPTION as u64;
        if let Some(work) = work.as_deref_mut() {
            work.touch(offset, EXCEPTION as u64);
        }
        let start = usize::try_from(offset)?;
        let slice = self
            .map
            .get(start..start + EXCEPTION)
            .ok_or("exception bounds")?;
        let mut alternatives = [InputAlternative {
            alternate: DnaBase::A,
            score: decode_score(0)?,
        }; 3];
        for (alternate_index, alternative) in alternatives.iter_mut().enumerate() {
            *alternative = InputAlternative {
                alternate: decode_base(slice[2 + alternate_index])?,
                score: decode_score(get_u32(slice, 24 + alternate_index * 4)?)?,
            };
        }
        Ok(AmbiguousInputLocus {
            gene: EnsemblGeneId::from_numeric(get_u64(slice, 8)?)?,
            contig: Grch38Contig::from_code(slice[0])?,
            position: GenomicPosition::new(get_u32(slice, 16)?)?,
            alternatives,
            omitted: decode_base(slice[1])?,
        })
    }

    fn query(
        &self,
        node_index: u64,
        snv: Grch38Snv,
        work: &mut CandidateWork,
        output: &mut Vec<(EnsemblGeneId, PangolinScore)>,
    ) -> Result<(), Box<dyn Error>> {
        let node = self.nodes[node_index as usize];
        let segment = self.segments[node.segment as usize];
        let position = snv.position().get();
        if node.left != NONE && self.nodes[node.left as usize].max_end >= position {
            self.query(node.left, snv, work, output)?;
        }
        if segment.start <= position && position <= segment.end {
            self.decode(node.segment as usize, snv, work, output)?;
        }
        if node.right != NONE && segment.start <= position {
            self.query(node.right, snv, work, output)?;
        }
        Ok(())
    }

    fn decode(
        &self,
        segment_index: usize,
        snv: Grch38Snv,
        work: &mut CandidateWork,
        output: &mut Vec<(EnsemblGeneId, PangolinScore)>,
    ) -> Result<(), Box<dyn Error>> {
        let segment = self.segments[segment_index];
        let ordinal = snv.position().get() - segment.start;
        let score = match self.codec {
            Codec::Fixed => {
                let offset = self.payload_offset + (segment.locus_start + u64::from(ordinal)) * 11;
                work.touch(offset, 11);
                decode_fixed(
                    self.map
                        .get(offset as usize..offset as usize + 11)
                        .ok_or("fixed bounds")?,
                    snv,
                )?
            }
            Codec::Direct | Codec::Zstd(_) | Codec::Lz4(_) => {
                let block_index =
                    segment.block_start as usize + ordinal as usize / self.codec.block_size();
                let block = self.blocks[block_index];
                let offset = self.payload_offset + block.payload_offset;
                let stored = self
                    .map
                    .get(offset as usize..offset as usize + block.payload_len as usize)
                    .ok_or("block bounds")?;
                if matches!(self.codec, Codec::Direct) {
                    if block.encoding != 3 {
                        return Err("direct block encoding".into());
                    }
                    decode_hierarchical(
                        stored,
                        (ordinal - block.first) as usize,
                        block.count as usize,
                        snv,
                        offset,
                        work,
                    )?
                } else {
                    work.touch(offset, u64::from(block.payload_len));
                    let raw = match block.encoding {
                        0 => stored.to_vec(),
                        1 => zstd::bulk::decompress(stored, block.raw_len as usize)?,
                        2 => decompress_size_prepended(stored)?,
                        _ => return Err("block encoding".into()),
                    };
                    decode_sparse(
                        &raw,
                        (ordinal - block.first) as usize,
                        block.count as usize,
                        snv,
                    )?
                }
            }
        };
        if let Some(score) = score {
            output.push((segment.gene, score));
        }
        Ok(())
    }
}

fn canonical(input: &[InputLocus]) -> Result<Canonical, Box<dyn Error>> {
    let mut loci = Vec::new();
    let mut exceptions = Vec::new();
    for item in input {
        match *item {
            InputLocus::Ordinary(locus) => loci.push(Locus {
                gene: locus.gene,
                contig: locus.contig,
                position: locus.position,
                reference: locus.reference,
                alternatives: locus.alternatives,
            }),
            InputLocus::Ambiguous(locus) => exceptions.push(locus),
        }
    }
    loci.sort_by_key(|locus| {
        (
            locus.gene.numeric(),
            locus.contig.code(),
            locus.position.get(),
        )
    });
    exceptions.sort_by_key(|locus| {
        (
            locus.contig.code(),
            locus.position.get(),
            locus.gene.numeric(),
        )
    });
    let mut segments = Vec::new();
    let mut start = 0;
    while start < loci.len() {
        let mut end = start + 1;
        while end < loci.len()
            && loci[end].gene == loci[start].gene
            && loci[end].contig == loci[start].contig
            && loci[end - 1].position.get() + 1 == loci[end].position.get()
        {
            end += 1;
        }
        segments.push(SegmentMeta {
            gene: loci[start].gene,
            contig: loci[start].contig,
            start: loci[start].position.get(),
            end: loci[end - 1].position.get(),
            locus_start: start as u64,
            loci: (end - start) as u32,
            block_start: 0,
            blocks: 0,
        });
        start = end;
    }
    let mut nodes = Vec::new();
    for code in 1..=25_u8 {
        let mut indices: Vec<_> = segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| (segment.contig.code() == code).then_some(index))
            .collect();
        indices.sort_by_key(|index| {
            (
                segments[*index].start,
                segments[*index].end,
                segments[*index].gene.numeric(),
            )
        });
        if !indices.is_empty() {
            build_tree(&indices, &segments, &mut nodes);
        }
    }
    Ok(Canonical {
        loci,
        segments,
        nodes,
        exceptions,
    })
}

fn build_tree(indices: &[usize], segments: &[SegmentMeta], nodes: &mut Vec<NodeMeta>) -> usize {
    let middle = indices.len() / 2;
    let segment = indices[middle];
    let index = nodes.len();
    nodes.push(NodeMeta {
        segment: segment as u64,
        left: NONE,
        right: NONE,
        max_end: segments[segment].end,
        contig: segments[segment].contig,
    });
    let left =
        (!indices[..middle].is_empty()).then(|| build_tree(&indices[..middle], segments, nodes));
    let right = (!indices[middle + 1..].is_empty())
        .then(|| build_tree(&indices[middle + 1..], segments, nodes));
    nodes[index].left = left.map_or(NONE, |value| value as u64);
    nodes[index].right = right.map_or(NONE, |value| value as u64);
    nodes[index].max_end = [
        segments[segment].end,
        left.map(|value| nodes[value].max_end).unwrap_or(0),
        right.map(|value| nodes[value].max_end).unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    index
}

fn fixed_record(locus: &Locus) -> [u8; 11] {
    let mut bits = u128::from(base_code(locus.reference));
    let mut shift = 3;
    for alternate in locus.alternatives {
        let score = score_code(alternate.score);
        bits |= u128::from(score) << shift;
        shift += 28;
    }
    let raw = bits.to_le_bytes();
    let mut output = [0_u8; 11];
    output.copy_from_slice(&raw[..11]);
    output
}

fn decode_fixed(raw: &[u8], snv: Grch38Snv) -> Result<Option<PangolinScore>, Box<dyn Error>> {
    let mut expanded = [0_u8; 16];
    expanded[..11].copy_from_slice(raw);
    let bits = u128::from_le_bytes(expanded);
    let reference = decode_base((bits & 0b111) as u8)?;
    if reference != snv.reference() {
        return Ok(None);
    }
    let alternatives: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != reference)
        .collect();
    let Some(index) = alternatives
        .iter()
        .position(|base| *base == snv.alternate())
    else {
        return Ok(None);
    };
    Ok(Some(decode_score(
        ((bits >> (3 + index * 28)) & ((1 << 28) - 1)) as u32,
    )?))
}

fn hierarchical_block(loci: &[Locus]) -> Vec<u8> {
    let refs_len = (loci.len() * 2).div_ceil(8);
    let active_len = loci.len().div_ceil(8);
    let mut refs = vec![0_u8; refs_len];
    let mut active = vec![0_u8; active_len];
    let mut masks = Vec::new();
    let mut values = Vec::new();
    let mut ranks = Vec::with_capacity(loci.len().div_ceil(DIRECT_RANK_STRIDE) * DIRECT_RANK);
    let mut pair_count = 0_u16;
    for (ordinal, locus) in loci.iter().enumerate() {
        if ordinal % DIRECT_RANK_STRIDE == 0 {
            ranks.extend_from_slice(&(masks.len() as u16).to_le_bytes());
            ranks.extend_from_slice(&pair_count.to_le_bytes());
        }
        refs[ordinal * 2 / 8] |= base_code(locus.reference) << (ordinal * 2 % 8);
        let mut mask = 0_u8;
        for (alternate_index, alternate) in locus.alternatives.iter().enumerate() {
            for (kind, magnitude, position) in [
                (0, alternate.score.gain(), alternate.score.gain_position()),
                (1, alternate.score.loss(), alternate.score.loss_position()),
            ] {
                if magnitude.hundredths() != 0 || position.get() != -50 {
                    let bit = alternate_index * 2 + kind;
                    mask |= 1 << bit;
                    values.extend_from_slice(&pair_code(magnitude, position).to_le_bytes());
                }
            }
        }
        if mask != 0 {
            active[ordinal / 8] |= 1 << (ordinal % 8);
            masks.push(mask);
            pair_count += mask.count_ones() as u16;
        }
    }
    let masks_len = (masks.len() * 6).div_ceil(8);
    let mut packed_masks = vec![0_u8; masks_len];
    for (index, mask) in masks.iter().copied().enumerate() {
        let bit = index * 6;
        let shifted = u16::from(mask) << (bit % 8);
        packed_masks[bit / 8] |= shifted as u8;
        if bit % 8 > 2 {
            packed_masks[bit / 8 + 1] |= (shifted >> 8) as u8;
        }
    }
    let mut bytes = Vec::with_capacity(
        DIRECT_HEADER + refs.len() + active.len() + ranks.len() + packed_masks.len() + values.len(),
    );
    bytes.extend_from_slice(&(masks.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&u32::from(pair_count).to_le_bytes());
    bytes.extend(refs);
    bytes.extend(active);
    bytes.extend(ranks);
    bytes.extend(packed_masks);
    bytes.extend(values);
    bytes
}

fn decode_hierarchical(
    raw: &[u8],
    ordinal: usize,
    count: usize,
    snv: Grch38Snv,
    file_offset: u64,
    work: &mut CandidateWork,
) -> Result<Option<PangolinScore>, Box<dyn Error>> {
    if ordinal >= count {
        return Err("direct ordinal".into());
    }
    work.touch(file_offset, DIRECT_HEADER as u64);
    let active_total = get_u32(raw, 0)? as usize;
    let pair_total = get_u32(raw, 4)? as usize;
    let refs_len = (count * 2).div_ceil(8);
    let active_len = count.div_ceil(8);
    let rank_count = count.div_ceil(DIRECT_RANK_STRIDE);
    let ranks_len = rank_count * DIRECT_RANK;
    let masks_len = (active_total * 6).div_ceil(8);
    let refs_start = DIRECT_HEADER;
    let active_start = refs_start + refs_len;
    let ranks_start = active_start + active_len;
    let masks_start = ranks_start + ranks_len;
    let values_start = masks_start + masks_len;
    let expected_len = values_start
        .checked_add(pair_total.checked_mul(2).ok_or("direct pair length")?)
        .ok_or("direct payload length")?;
    if expected_len != raw.len() {
        return Err("direct payload length".into());
    }
    let reference_byte = refs_start + ordinal * 2 / 8;
    work.touch(file_offset + reference_byte as u64, 1);
    let reference = decode_base((raw[reference_byte] >> (ordinal * 2 % 8)) & 0b11)?;
    if reference != snv.reference() {
        return Ok(None);
    }
    let Some(alternate) = alternate_index(reference, snv.alternate()) else {
        return Ok(None);
    };
    let active = raw
        .get(active_start..active_start + active_len)
        .ok_or("direct active bounds")?;
    work.touch(file_offset + active_start as u64 + (ordinal / 8) as u64, 1);
    if active[ordinal / 8] & (1 << (ordinal % 8)) == 0 {
        return Ok(Some(decode_score(0)?));
    }
    let checkpoint = ordinal / DIRECT_RANK_STRIDE;
    let checkpoint_offset = ranks_start + checkpoint * DIRECT_RANK;
    work.touch(file_offset + checkpoint_offset as u64, DIRECT_RANK as u64);
    let active_checkpoint = get_u16(raw, checkpoint_offset)? as usize;
    let pair_checkpoint = get_u16(raw, checkpoint_offset + 2)? as usize;
    let checkpoint_locus = checkpoint * DIRECT_RANK_STRIDE;
    let active_before = active_checkpoint + popcount_range(active, checkpoint_locus, ordinal)?;
    if active_before >= active_total {
        return Err("direct active rank".into());
    }
    let masks = raw
        .get(masks_start..masks_start + masks_len)
        .ok_or("direct mask bounds")?;
    let prior_mask_start = active_checkpoint * 6;
    let prior_mask_end = active_before * 6;
    if prior_mask_end > prior_mask_start {
        let first_byte = prior_mask_start / 8;
        let last_byte = (prior_mask_end - 1) / 8;
        work.touch(
            file_offset + masks_start as u64 + first_byte as u64,
            (last_byte - first_byte + 1) as u64,
        );
    }
    let pairs_before = pair_checkpoint + popcount_range(masks, prior_mask_start, prior_mask_end)?;
    let mask = read_six_bits(masks, active_before)?;
    let mask_byte = active_before * 6 / 8;
    work.touch(
        file_offset + masks_start as u64 + mask_byte as u64,
        if active_before * 6 % 8 > 2 { 2 } else { 1 },
    );
    let mut selected = [None, None];
    for (kind, selected_pair) in selected.iter_mut().enumerate() {
        let bit = alternate * 2 + kind;
        if mask & (1 << bit) != 0 {
            let local = (mask & ((1 << bit) - 1)).count_ones() as usize;
            *selected_pair = Some(pairs_before + local);
        }
    }
    let mut pair = |index: Option<usize>| -> Result<_, Box<dyn Error>> {
        index.map_or_else(
            || decode_pair(0),
            |index| {
                let offset = values_start + index * 2;
                work.touch(file_offset + offset as u64, 2);
                decode_pair(get_u16(raw, offset)?)
            },
        )
    };
    let gain = pair(selected[0])?;
    let loss = pair(selected[1])?;
    Ok(Some(PangolinScore::new(gain.0, gain.1, loss.0, loss.1)))
}

fn popcount_range(bytes: &[u8], start: usize, end: usize) -> Result<usize, Box<dyn Error>> {
    if start > end || end > bytes.len() * 8 {
        return Err("rank bit range".into());
    }
    if start == end {
        return Ok(0);
    }
    let first = start / 8;
    let last = (end - 1) / 8;
    if first == last {
        let width = end - start;
        let mask = if width == 8 {
            u8::MAX
        } else {
            ((1_u16 << width) - 1) as u8
        };
        return Ok(((bytes[first] >> (start % 8)) & mask).count_ones() as usize);
    }
    let mut count = (bytes[first] & (u8::MAX << (start % 8))).count_ones() as usize;
    count += bytes[first + 1..last]
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum::<usize>();
    let tail_bits = end % 8;
    let tail_mask = if tail_bits == 0 {
        u8::MAX
    } else {
        ((1_u16 << tail_bits) - 1) as u8
    };
    count += (bytes[last] & tail_mask).count_ones() as usize;
    Ok(count)
}

fn read_six_bits(bytes: &[u8], index: usize) -> Result<u8, Box<dyn Error>> {
    let bit = index.checked_mul(6).ok_or("mask bit offset")?;
    let byte = bit / 8;
    let shift = bit % 8;
    let low = u16::from(*bytes.get(byte).ok_or("mask bounds")?);
    let high = bytes.get(byte + 1).copied().map_or(0, u16::from);
    Ok((((low | (high << 8)) >> shift) & 0x3f) as u8)
}

fn alternate_index(reference: DnaBase, alternate: DnaBase) -> Option<usize> {
    DnaBase::ALL
        .into_iter()
        .filter(|base| *base != reference)
        .position(|base| base == alternate)
}

fn sparse_block(loci: &[Locus]) -> Vec<u8> {
    let refs_len = (loci.len() * 2).div_ceil(8);
    let bitmap_len = loci.len().div_ceil(8);
    let mut bytes = vec![0_u8; refs_len + bitmap_len * 6];
    for (index, locus) in loci.iter().enumerate() {
        bytes[index * 2 / 8] |= base_code(locus.reference) << (index * 2 % 8);
        for (alternate_index, alternate) in locus.alternatives.iter().enumerate() {
            for (kind, magnitude, position) in [
                (0, alternate.score.gain(), alternate.score.gain_position()),
                (1, alternate.score.loss(), alternate.score.loss_position()),
            ] {
                let pair = alternate_index * 2 + kind;
                if magnitude.hundredths() != 0 || position.get() != -50 {
                    bytes[refs_len + pair * bitmap_len + index / 8] |= 1 << (index % 8);
                }
            }
        }
    }
    let mut values = Vec::new();
    for pair in 0..6 {
        for locus in loci {
            let alternate = &locus.alternatives[pair / 2];
            let (magnitude, position) = if pair % 2 == 0 {
                (alternate.score.gain(), alternate.score.gain_position())
            } else {
                (alternate.score.loss(), alternate.score.loss_position())
            };
            if magnitude.hundredths() != 0 || position.get() != -50 {
                values.extend_from_slice(&pair_code(magnitude, position).to_le_bytes());
            }
        }
    }
    bytes.extend(values);
    bytes
}

fn decode_sparse(
    raw: &[u8],
    ordinal: usize,
    count: usize,
    snv: Grch38Snv,
) -> Result<Option<PangolinScore>, Box<dyn Error>> {
    let refs_len = (count * 2).div_ceil(8);
    let bitmap_len = count.div_ceil(8);
    let reference = decode_base((raw[ordinal * 2 / 8] >> (ordinal * 2 % 8)) & 0b11)?;
    if reference != snv.reference() {
        return Ok(None);
    }
    let alternatives: Vec<_> = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != reference)
        .collect();
    let Some(alternate) = alternatives
        .iter()
        .position(|base| *base == snv.alternate())
    else {
        return Ok(None);
    };
    let mut value_index = 0;
    let mut selected = [None, None];
    for pair in 0..6 {
        let bitmap = &raw[refs_len + pair * bitmap_len..refs_len + (pair + 1) * bitmap_len];
        for locus in 0..count {
            if bitmap[locus / 8] & (1 << (locus % 8)) != 0 {
                if locus == ordinal && pair / 2 == alternate {
                    selected[pair % 2] = Some(value_index);
                }
                value_index += 1;
            }
        }
    }
    let values = refs_len + bitmap_len * 6;
    let default = (ScoreMagnitude::new(0)?, RelativePosition::new(-50)?);
    let pair = |selected: Option<usize>| -> Result<_, Box<dyn Error>> {
        if let Some(index) = selected {
            decode_pair(get_u16(raw, values + index * 2)?)
        } else {
            Ok(default)
        }
    };
    let gain = pair(selected[0])?;
    let loss = pair(selected[1])?;
    Ok(Some(PangolinScore::new(gain.0, gain.1, loss.0, loss.1)))
}

fn score_code(score: PangolinScore) -> u32 {
    u32::from(pair_code(score.gain(), score.gain_position()))
        | (u32::from(pair_code(score.loss(), score.loss_position())) << 14)
}
fn decode_score(value: u32) -> Result<PangolinScore, Box<dyn Error>> {
    let gain = decode_pair((value & 0x3fff) as u16)?;
    let loss = decode_pair((value >> 14) as u16)?;
    Ok(PangolinScore::new(gain.0, gain.1, loss.0, loss.1))
}
fn pair_code(score: ScoreMagnitude, position: RelativePosition) -> u16 {
    u16::from(score.hundredths()) | ((position.get() as i16 + 50) as u16) << 7
}
fn decode_pair(value: u16) -> Result<(ScoreMagnitude, RelativePosition), Box<dyn Error>> {
    Ok((
        ScoreMagnitude::new(value & 0x7f)?,
        RelativePosition::new(((value >> 7) & 0x7f) as i16 - 50)?,
    ))
}
fn base_code(base: DnaBase) -> u8 {
    match base {
        DnaBase::A => 0,
        DnaBase::C => 1,
        DnaBase::G => 2,
        DnaBase::T => 3,
    }
}
fn decode_base(value: u8) -> Result<DnaBase, Box<dyn Error>> {
    match value {
        0 => Ok(DnaBase::A),
        1 => Ok(DnaBase::C),
        2 => Ok(DnaBase::G),
        3 => Ok(DnaBase::T),
        _ => Err("base code".into()),
    }
}

fn encode_exception(bytes: &mut Vec<u8>, locus: &AmbiguousInputLocus) {
    let start = bytes.len();
    bytes.resize(start + EXCEPTION, 0);
    bytes[start] = locus.contig.code();
    bytes[start + 1] = base_code(locus.omitted);
    put_u64(bytes, start + 8, locus.gene.numeric());
    put_u32(bytes, start + 16, locus.position.get());
    for (index, alternate) in locus.alternatives.iter().enumerate() {
        bytes[start + 2 + index] = base_code(alternate.alternate);
        put_u32(bytes, start + 24 + index * 4, score_code(alternate.score));
    }
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn Error>> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("u16 bounds")?
            .try_into()?,
    ))
}
fn get_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn Error>> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("u32 bounds")?
            .try_into()?,
    ))
}
fn get_u64(bytes: &[u8], offset: usize) -> Result<u64, Box<dyn Error>> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or("u64 bounds")?
            .try_into()?,
    ))
}

#[allow(dead_code)]
pub fn assert_roundtrip(input: &[InputLocus], directory: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    for codec in codecs() {
        let path = directory.join(format!("{}.candidate", codec.name()));
        write_candidate(&path, codec, input)?;
        let reader = CandidateReader::open(&path, codec)?;
        for item in input {
            if let InputLocus::Ordinary(locus) = item {
                for alternate in locus.alternatives {
                    let snv = Grch38Snv::new(
                        locus.contig,
                        locus.position,
                        locus.reference,
                        alternate.alternate,
                    )?;
                    let values =
                        reader.lookup(snv, Some(locus.gene), &mut CandidateWork::default())?;
                    if values.as_slice() != [(locus.gene, alternate.score)] {
                        return Err(format!("{} roundtrip mismatch", codec.name()).into());
                    }
                }
            }
        }
        let mut expected: Vec<_> = input
            .iter()
            .filter_map(|item| match item {
                InputLocus::Ambiguous(locus) => Some(*locus),
                InputLocus::Ordinary(_) => None,
            })
            .collect();
        expected.sort_by_key(|locus| {
            (
                locus.contig.code(),
                locus.position.get(),
                locus.gene.numeric(),
            )
        });
        if reader.exception_records()? != expected {
            return Err(format!("{} exception roundtrip mismatch", codec.name()).into());
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn assert_bounded_gene_filter(directory: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let zero = PangolinScore::new(
        ScoreMagnitude::new(0)?,
        RelativePosition::new(-50)?,
        ScoreMagnitude::new(0)?,
        RelativePosition::new(-50)?,
    );
    let alternatives = [
        InputAlternative {
            alternate: DnaBase::C,
            score: zero,
        },
        InputAlternative {
            alternate: DnaBase::G,
            score: zero,
        },
        InputAlternative {
            alternate: DnaBase::T,
            score: zero,
        },
    ];
    let contig = Grch38Contig::from_code(1)?;
    let mut input = Vec::with_capacity(19_916);
    for numeric in 1..=19_916_u64 {
        input.push(InputLocus::Ordinary(OrdinaryInputLocus {
            gene: EnsemblGeneId::from_numeric(numeric)?,
            contig,
            position: GenomicPosition::new(10_000 + numeric as u32 * 2)?,
            reference: DnaBase::A,
            alternatives,
        }));
    }
    let path = directory.join("bounded-gene-filter.candidate");
    write_candidate(&path, Codec::Direct, &input)?;
    let reader = CandidateReader::open(&path, Codec::Direct)?;
    let gene = EnsemblGeneId::from_numeric(19_916)?;
    let snv = Grch38Snv::new(
        contig,
        GenomicPosition::new(10_000 + 19_916 * 2)?,
        DnaBase::A,
        DnaBase::C,
    )?;
    let mut work = CandidateWork::default();
    let result = reader.lookup(snv, Some(gene), &mut work)?;
    if result.len() != 1 || work.directory_entries > 20 {
        return Err(format!(
            "candidate gene filter was not logarithmic: records={} directory_entries={}",
            result.len(),
            work.directory_entries
        )
        .into());
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn codecs() -> [Codec; 8] {
    [
        Codec::Direct,
        Codec::Fixed,
        Codec::Zstd(1024),
        Codec::Zstd(2048),
        Codec::Zstd(4096),
        Codec::Lz4(1024),
        Codec::Lz4(2048),
        Codec::Lz4(4096),
    ]
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let corpus_path = env::var_os("PANGOPUP_BENCH_CORPUS")
        .map(PathBuf::from)
        .ok_or("set PANGOPUP_BENCH_CORPUS to a logical corpus")?;
    let output = env::var_os("PANGOPUP_BENCH_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/index-benchmark"));
    fs::create_dir_all(&output)?;
    let input = read_logical_corpus(&corpus_path)?;
    let expected_exceptions = input
        .iter()
        .filter(|item| matches!(item, InputLocus::Ambiguous(_)))
        .count();
    let queries = select_queries(&input, 100)?;
    let special = select_special_queries(&input)?;
    write_query_manifest(&output.join("query-manifest.tsv"), &queries)?;
    let iterations = env::var("PANGOPUP_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200);
    if iterations == 0 {
        return Err("PANGOPUP_BENCH_ITERATIONS must be positive".into());
    }
    println!(
        "benchmark corpus_loci={} iterations={} cache=warm-os-page-cache",
        input.len(),
        iterations
    );

    let direct_path = output.join("selected-fixed-11.pgi");
    let started = Instant::now();
    let direct_summary = write_index(&direct_path, &input)?;
    let serialization = started.elapsed();
    let direct = IndexReader::open(&direct_path)?;
    benchmark_direct(
        "selected-fixed-11",
        &direct_path,
        &direct,
        &queries,
        iterations,
        serialization,
        direct_summary.bytes,
    )?;
    benchmark_special_direct(
        &direct,
        &queries[..10],
        &special,
        iterations,
        direct_summary.bytes,
    )?;
    for codec in codecs() {
        let path = output.join(format!("{}.candidate", codec.name()));
        let started = Instant::now();
        let bytes = write_candidate(&path, codec, &input)?;
        let serialization = started.elapsed();
        let reader = CandidateReader::open(&path, codec)?;
        if reader.exception_records()?.len() != expected_exceptions {
            return Err(format!("{} exception count mismatch", codec.name()).into());
        }
        benchmark_candidate(
            &path,
            &reader,
            &queries,
            iterations,
            serialization,
            bytes,
            codec,
        )?;
        benchmark_special_candidate(
            &codec.name(),
            &reader,
            &queries[..10],
            &special,
            iterations,
            bytes,
        )?;
    }
    let tabix_path = output.join("tabix.tsv.gz");
    let started = Instant::now();
    write_tabix_baseline(&tabix_path, &input)?;
    let serialization = started.elapsed();
    benchmark_tabix(&tabix_path, &queries, &special, iterations, serialization)?;
    Ok(())
}

fn write_tabix_baseline(path: &Path, input: &[InputLocus]) -> Result<(), Box<dyn Error>> {
    let plain = path.with_extension("");
    let mut indices: Vec<_> = (0..input.len()).collect();
    indices.sort_by_key(|index| match input[*index] {
        InputLocus::Ordinary(locus) => (
            locus.contig.code(),
            locus.position.get(),
            locus.gene.numeric(),
            0_u8,
        ),
        InputLocus::Ambiguous(locus) => (
            locus.contig.code(),
            locus.position.get(),
            locus.gene.numeric(),
            1_u8,
        ),
    });
    let mut writer = BufWriter::new(File::create(&plain)?);
    for index in indices {
        let (gene, contig, position, reference, alternatives) = match input[index] {
            InputLocus::Ordinary(locus) => (
                locus.gene,
                locus.contig,
                locus.position,
                locus.reference.to_string(),
                locus.alternatives,
            ),
            InputLocus::Ambiguous(locus) => (
                locus.gene,
                locus.contig,
                locus.position,
                "N".to_owned(),
                locus.alternatives,
            ),
        };
        for alternative in alternatives {
            writeln!(
                writer,
                "{contig}\t{position}\t{reference}\t{}\t{}\t{}\t{}\t{}\t{gene}",
                alternative.alternate,
                alternative.score.gain().hundredths(),
                alternative.score.gain_position().get(),
                alternative.score.loss().hundredths(),
                alternative.score.loss_position().get(),
            )?;
        }
    }
    writer.into_inner()?.sync_all()?;
    let status = Command::new("/usr/bin/bgzip")
        .args(["-f"])
        .arg(&plain)
        .status()?;
    if !status.success() {
        return Err("bgzip failed".into());
    }
    let status = Command::new("/usr/bin/tabix")
        .args(["-f", "-s", "1", "-b", "2", "-e", "2"])
        .arg(path)
        .status()?;
    if !status.success() {
        return Err("tabix indexing failed".into());
    }
    Ok(())
}

fn benchmark_tabix(
    path: &Path,
    queries: &[(Grch38Snv, EnsemblGeneId)],
    special: &SpecialQueries,
    iterations: usize,
    serialization: Duration,
) -> Result<(), Box<dyn Error>> {
    let index_path = PathBuf::from(format!("{}.tbi", path.display()));
    let bytes = fs::metadata(path)?.len() + fs::metadata(index_path)?.len();
    let open = measure(iterations, || {
        black_box(TabixReader::open(path).expect("Tabix open"));
    });
    report("tabix", "open-only", bytes, serialization, &open, 0, 0);
    for count in [1, 10, 100] {
        let reopen = measure(iterations, || {
            let mut reader = TabixReader::open(path).expect("Tabix open");
            for (query, gene) in &queries[..count] {
                let records = reader.lookup(*query, Some(*gene)).expect("Tabix query");
                assert_eq!(records, 1);
                black_box(records);
            }
        });
        report(
            "tabix",
            &format!("reopen-plus-query-{count}"),
            bytes,
            serialization,
            &reopen,
            0,
            0,
        );
        let mut reader = TabixReader::open(path)?;
        let one_open = measure(iterations, || {
            for (query, gene) in &queries[..count] {
                let records = reader.lookup(*query, Some(*gene)).expect("Tabix query");
                assert_eq!(records, 1);
                black_box(records);
            }
        });
        report(
            "tabix",
            &format!("one-open-{count}"),
            bytes,
            serialization,
            &one_open,
            0,
            0,
        );
    }
    let mut reader = TabixReader::open(path)?;
    for (mode, operation) in [
        ("same-block-hits", 0_u8),
        ("cross-block-hits", 1),
        ("gene-filtered-hit", 2),
        ("all-overlap-hits", 3),
        ("absent-allele", 4),
        ("ref-n-outcome", 5),
    ] {
        let measured = measure(iterations, || match operation {
            0 => {
                for (query, gene) in &queries[..10] {
                    black_box(reader.lookup(*query, Some(*gene)).expect("same block"));
                }
            }
            1 => {
                for (query, gene) in &special.cross_block {
                    black_box(reader.lookup(*query, Some(*gene)).expect("cross block"));
                }
            }
            2 => {
                black_box(
                    reader
                        .lookup(queries[0].0, Some(queries[0].1))
                        .expect("gene filter"),
                );
            }
            3 => {
                assert_eq!(reader.lookup(special.overlap, None).expect("overlap"), 2);
            }
            4 => {
                assert_eq!(
                    reader
                        .lookup(special.absent.0, Some(special.absent.1))
                        .expect("absent"),
                    0
                );
            }
            5 => {
                assert_eq!(
                    reader
                        .lookup_ambiguous(
                            special.ambiguous.0,
                            special.ambiguous.1,
                            special.ambiguous.2
                        )
                        .expect("ambiguous"),
                    1
                );
            }
            _ => unreachable!(),
        });
        report("tabix", mode, bytes, serialization, &measured, 0, 0);
    }
    Ok(())
}

fn benchmark_direct(
    name: &str,
    path: &Path,
    reader: &IndexReader,
    queries: &[(Grch38Snv, EnsemblGeneId)],
    iterations: usize,
    serialization: Duration,
    bytes: u64,
) -> Result<(), Box<dyn Error>> {
    let open = measure(iterations, || {
        black_box(IndexReader::open(path).expect("open"));
    });
    let open_metrics = reader.open_metrics();
    report(
        name,
        "open-only",
        bytes,
        serialization,
        &open,
        open_metrics.logical_bytes_decoded,
        open_metrics.unique_mapped_pages_addressed,
    );
    for count in [1, 10, 100] {
        let metrics = reader.lookup_gene_batch_measured(&queries[..count])?;
        let timings = measure(iterations, || {
            let opened = IndexReader::open(path).expect("open");
            for (query, gene) in &queries[..count] {
                black_box(opened.lookup_parts(*query, Some(*gene)).expect("lookup"));
            }
        });
        report(
            name,
            &format!("reopen-plus-query-{count}"),
            bytes,
            serialization,
            &timings,
            metrics.logical_bytes_decoded,
            metrics.unique_mapped_pages_addressed,
        );
        let timings = measure(iterations, || {
            for (query, gene) in &queries[..count] {
                black_box(reader.lookup_parts(*query, Some(*gene)).expect("lookup"));
            }
        });
        report(
            name,
            &format!("one-open-{count}"),
            bytes,
            serialization,
            &timings,
            metrics.logical_bytes_decoded,
            metrics.unique_mapped_pages_addressed,
        );
    }
    Ok(())
}

fn benchmark_candidate(
    path: &Path,
    reader: &CandidateReader,
    queries: &[(Grch38Snv, EnsemblGeneId)],
    iterations: usize,
    serialization: Duration,
    bytes: u64,
    codec: Codec,
) -> Result<(), Box<dyn Error>> {
    let name = codec.name();
    let open = measure(iterations, || {
        black_box(CandidateReader::open(path, codec).expect("open"));
    });
    let (open_logical, open_pages) = reader.open_metrics();
    report(
        &name,
        "open-only",
        bytes,
        serialization,
        &open,
        open_logical,
        open_pages,
    );
    for count in [1, 10, 100] {
        let mut representative = CandidateWork::default();
        for (query, gene) in &queries[..count] {
            black_box(reader.lookup(*query, Some(*gene), &mut representative)?);
        }
        let logical = representative.logical_bytes;
        let pages = representative.unique_pages() as u64;
        let timings = measure(iterations, || {
            let opened = CandidateReader::open(path, codec).expect("open");
            for (query, gene) in &queries[..count] {
                black_box(
                    opened
                        .lookup(*query, Some(*gene), &mut CandidateWork::default())
                        .expect("lookup"),
                );
            }
        });
        report(
            &name,
            &format!("reopen-plus-query-{count}"),
            bytes,
            serialization,
            &timings,
            logical,
            pages,
        );
        let timings = measure(iterations, || {
            for (query, gene) in &queries[..count] {
                black_box(
                    reader
                        .lookup(*query, Some(*gene), &mut CandidateWork::default())
                        .expect("lookup"),
                );
            }
        });
        report(
            &name,
            &format!("one-open-{count}"),
            bytes,
            serialization,
            &timings,
            logical,
            pages,
        );
    }
    Ok(())
}

fn benchmark_special_direct(
    reader: &IndexReader,
    same_block: &[(Grch38Snv, EnsemblGeneId)],
    special: &SpecialQueries,
    iterations: usize,
    bytes: u64,
) -> Result<(), Box<dyn Error>> {
    for operation in 0..6_u8 {
        let metrics = match operation {
            0 => reader.lookup_gene_batch_measured(same_block)?,
            1 => reader.lookup_gene_batch_measured(&special.cross_block)?,
            2 => reader.lookup_gene_batch_measured(&same_block[..1])?,
            3 => reader.lookup_batch_measured(&[(special.overlap, None)])?,
            4 => reader.lookup_gene_batch_measured(&[special.absent])?,
            5 => reader
                .lookup_batch_measured(&[(ambiguous_snv(special)?, Some(special.ambiguous.2))])?,
            _ => unreachable!(),
        };
        let measured = measure(iterations, || match operation {
            0 => {
                for (query, gene) in same_block {
                    black_box(
                        reader
                            .lookup_parts(*query, Some(*gene))
                            .expect("same block"),
                    );
                }
            }
            1 => {
                for (query, gene) in &special.cross_block {
                    black_box(
                        reader
                            .lookup_parts(*query, Some(*gene))
                            .expect("cross block"),
                    );
                }
            }
            2 => {
                black_box(
                    reader
                        .lookup_parts(same_block[0].0, Some(same_block[0].1))
                        .expect("gene filtered"),
                );
            }
            3 => {
                assert_eq!(
                    reader
                        .lookup_parts(special.overlap, None)
                        .expect("overlap")
                        .0
                        .len(),
                    2
                );
            }
            4 => {
                assert!(
                    reader
                        .lookup_parts(special.absent.0, Some(special.absent.1))
                        .expect("absent")
                        .0
                        .is_empty()
                );
            }
            5 => {
                let snv = ambiguous_snv(special).expect("ambiguous SNV");
                assert_eq!(
                    reader
                        .lookup_parts(snv, Some(special.ambiguous.2))
                        .expect("ambiguous")
                        .1
                        .len(),
                    1
                );
            }
            _ => unreachable!(),
        });
        report(
            "selected-fixed-11",
            special_mode(operation),
            bytes,
            Duration::ZERO,
            &measured,
            metrics.logical_bytes_decoded,
            metrics.unique_mapped_pages_addressed,
        );
    }
    Ok(())
}

fn benchmark_special_candidate(
    name: &str,
    reader: &CandidateReader,
    same_block: &[(Grch38Snv, EnsemblGeneId)],
    special: &SpecialQueries,
    iterations: usize,
    bytes: u64,
) -> Result<(), Box<dyn Error>> {
    for operation in 0..6_u8 {
        let mut work = CandidateWork::default();
        match operation {
            0 => {
                for (query, gene) in same_block {
                    black_box(reader.lookup(*query, Some(*gene), &mut work)?);
                }
            }
            1 => {
                for (query, gene) in &special.cross_block {
                    black_box(reader.lookup(*query, Some(*gene), &mut work)?);
                }
            }
            2 => {
                black_box(reader.lookup(same_block[0].0, Some(same_block[0].1), &mut work)?);
            }
            3 => {
                black_box(reader.lookup(special.overlap, None, &mut work)?);
            }
            4 => {
                black_box(reader.lookup(special.absent.0, Some(special.absent.1), &mut work)?);
            }
            5 => {
                black_box(reader.ambiguity_count(
                    special.ambiguous.0,
                    special.ambiguous.1,
                    special.ambiguous.2,
                    Some(&mut work),
                )?);
            }
            _ => unreachable!(),
        }
        let logical = work.logical_bytes;
        let pages = work.unique_pages() as u64;
        let measured = measure(iterations, || match operation {
            0 => {
                for (query, gene) in same_block {
                    black_box(
                        reader
                            .lookup(*query, Some(*gene), &mut CandidateWork::default())
                            .expect("same block"),
                    );
                }
            }
            1 => {
                for (query, gene) in &special.cross_block {
                    black_box(
                        reader
                            .lookup(*query, Some(*gene), &mut CandidateWork::default())
                            .expect("cross block"),
                    );
                }
            }
            2 => {
                black_box(
                    reader
                        .lookup(
                            same_block[0].0,
                            Some(same_block[0].1),
                            &mut CandidateWork::default(),
                        )
                        .expect("gene filtered"),
                );
            }
            3 => {
                assert_eq!(
                    reader
                        .lookup(special.overlap, None, &mut CandidateWork::default())
                        .expect("overlap")
                        .len(),
                    2
                );
            }
            4 => {
                assert!(
                    reader
                        .lookup(
                            special.absent.0,
                            Some(special.absent.1),
                            &mut CandidateWork::default()
                        )
                        .expect("absent")
                        .is_empty()
                );
            }
            5 => {
                assert_eq!(
                    reader
                        .ambiguity_count(
                            special.ambiguous.0,
                            special.ambiguous.1,
                            special.ambiguous.2,
                            None,
                        )
                        .expect("ambiguous"),
                    1
                );
            }
            _ => unreachable!(),
        });
        report(
            name,
            special_mode(operation),
            bytes,
            Duration::ZERO,
            &measured,
            logical,
            pages,
        );
    }
    Ok(())
}

fn special_mode(operation: u8) -> &'static str {
    match operation {
        0 => "same-block-hits",
        1 => "cross-block-hits",
        2 => "gene-filtered-hit",
        3 => "all-overlap-hits",
        4 => "absent-allele",
        5 => "ref-n-outcome",
        _ => unreachable!(),
    }
}

fn ambiguous_snv(special: &SpecialQueries) -> Result<Grch38Snv, Box<dyn Error>> {
    Ok(Grch38Snv::new(
        special.ambiguous.0,
        special.ambiguous.1,
        DnaBase::A,
        DnaBase::C,
    )?)
}

struct Measurements {
    timings: Vec<Duration>,
    allocations_per_sample: f64,
    minor_faults_per_sample: f64,
    major_faults_per_sample: f64,
}

fn measure(iterations: usize, mut operation: impl FnMut()) -> Measurements {
    for _ in 0..20 {
        operation();
    }
    let mut values = Vec::with_capacity(iterations);
    let before_allocations = crate::allocation_count();
    let before_faults = resource_usage();
    for _ in 0..iterations {
        let start = Instant::now();
        operation();
        values.push(start.elapsed());
    }
    values.sort();
    let after_faults = resource_usage();
    Measurements {
        timings: values,
        allocations_per_sample: (crate::allocation_count() - before_allocations) as f64
            / iterations as f64,
        minor_faults_per_sample: (after_faults.0 - before_faults.0) as f64 / iterations as f64,
        major_faults_per_sample: (after_faults.1 - before_faults.1) as f64 / iterations as f64,
    }
}

fn resource_usage() -> (i64, i64) {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` points to writable storage for one `rusage`, and
    // `RUSAGE_SELF` requests statistics for this process.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status == 0 {
        // SAFETY: A successful `getrusage` initialized the structure.
        let usage = unsafe { usage.assume_init() };
        (usage.ru_minflt, usage.ru_majflt)
    } else {
        (0, 0)
    }
}
fn report(
    codec: &str,
    mode: &str,
    bytes: u64,
    serialization: Duration,
    measurements: &Measurements,
    logical: u64,
    pages: u64,
) {
    let timings = &measurements.timings;
    let percentile = |numerator: usize, denominator: usize| {
        let rank = (timings.len() * numerator).div_ceil(denominator);
        timings[rank.saturating_sub(1).min(timings.len() - 1)]
    };
    let total: Duration = timings.iter().copied().sum();
    let throughput = timings.len() as f64 / total.as_secs_f64();
    println!(
        "codec={codec} mode={mode} p50_ns={} p95_ns={} p99_ns={} samples={} throughput_per_s={throughput:.1} allocations_per_sample={:.2} minor_faults_per_sample={:.3} major_faults_per_sample={:.3} artifact_bytes={bytes} serialization_ms={:.3} logical_bytes={logical} unique_pages={pages}",
        percentile(50, 100).as_nanos(),
        percentile(95, 100).as_nanos(),
        percentile(99, 100).as_nanos(),
        timings.len(),
        measurements.allocations_per_sample,
        measurements.minor_faults_per_sample,
        measurements.major_faults_per_sample,
        serialization.as_secs_f64() * 1000.0
    );
}

fn select_queries(
    input: &[InputLocus],
    count: usize,
) -> Result<Vec<(Grch38Snv, EnsemblGeneId)>, Box<dyn Error>> {
    let mut queries = Vec::new();
    for item in input {
        if let InputLocus::Ordinary(locus) = item {
            let alternate = locus.alternatives[0].alternate;
            queries.push((
                Grch38Snv::new(locus.contig, locus.position, locus.reference, alternate)?,
                locus.gene,
            ));
            if queries.len() == count {
                break;
            }
        }
    }
    if queries.len() != count {
        return Err("corpus has fewer than 100 ordinary loci".into());
    }
    Ok(queries)
}

fn select_special_queries(input: &[InputLocus]) -> Result<SpecialQueries, Box<dyn Error>> {
    let ordinary: Vec<_> = input
        .iter()
        .filter_map(|item| match item {
            InputLocus::Ordinary(locus) => Some(locus),
            InputLocus::Ambiguous(_) => None,
        })
        .collect();
    let mut cross_block = Vec::with_capacity(10);
    for index in 0..10 {
        let locus = ordinary[index * (ordinary.len() - 1) / 9];
        cross_block.push((
            Grch38Snv::new(
                locus.contig,
                locus.position,
                locus.reference,
                locus.alternatives[0].alternate,
            )?,
            locus.gene,
        ));
    }
    let first = ordinary.first().ok_or("no ordinary loci")?;
    let fake_reference = DnaBase::ALL
        .into_iter()
        .find(|base| *base != first.reference)
        .ok_or("no fake reference")?;
    let absent = (
        Grch38Snv::new(
            first.contig,
            first.position,
            fake_reference,
            first.reference,
        )?,
        first.gene,
    );
    let ambiguous = input
        .iter()
        .find_map(|item| match item {
            InputLocus::Ambiguous(locus) => Some((locus.contig, locus.position, locus.gene)),
            InputLocus::Ordinary(_) => None,
        })
        .ok_or("no REF=N locus")?;
    Ok(SpecialQueries {
        cross_block,
        overlap: Grch38Snv::new(
            "chr17".parse()?,
            GenomicPosition::new(7_686_072)?,
            DnaBase::G,
            DnaBase::T,
        )?,
        absent,
        ambiguous,
    })
}

fn write_query_manifest(
    path: &Path,
    queries: &[(Grch38Snv, EnsemblGeneId)],
) -> Result<(), Box<dyn Error>> {
    let mut writer = BufWriter::new(File::create(path)?);
    writeln!(
        writer,
        "ordinal\tcontig\tposition\tref\talt\tgene\tworkload"
    )?;
    for (index, (query, gene)) in queries.iter().enumerate() {
        writeln!(
            writer,
            "{}\t{}\t{}\t{}\t{}\t{}\tprimary-distinct-gene-filtered",
            index + 1,
            query.contig(),
            query.position(),
            query.reference(),
            query.alternate(),
            gene
        )?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_logical_corpus(path: &Path) -> Result<Vec<InputLocus>, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    if bytes.get(..8) != Some(b"PGLOG001") {
        return Err("logical corpus magic".into());
    }
    let count = get_u64(&bytes, 8)? as usize;
    let mut offset = 16;
    let mut output = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = bytes[offset];
        let contig = Grch38Contig::from_code(bytes[offset + 1])?;
        let allele = decode_base(bytes[offset + 2])?;
        let gene = EnsemblGeneId::from_numeric(get_u64(&bytes, offset + 4)?)?;
        let position = GenomicPosition::new(get_u32(&bytes, offset + 12)?)?;
        let mut alternatives = [InputAlternative {
            alternate: DnaBase::A,
            score: PangolinScore::new(
                ScoreMagnitude::new(0)?,
                RelativePosition::new(-50)?,
                ScoreMagnitude::new(0)?,
                RelativePosition::new(-50)?,
            ),
        }; 3];
        for (index, alternative) in alternatives.iter_mut().enumerate() {
            let base = offset + 16 + index * 5;
            *alternative = InputAlternative {
                alternate: decode_base(bytes[base])?,
                score: PangolinScore::new(
                    ScoreMagnitude::new(u16::from(bytes[base + 1]))?,
                    RelativePosition::new(i16::from(bytes[base + 2]) - 50)?,
                    ScoreMagnitude::new(u16::from(bytes[base + 3]))?,
                    RelativePosition::new(i16::from(bytes[base + 4]) - 50)?,
                ),
            };
        }
        output.push(if kind == 0 {
            InputLocus::Ordinary(OrdinaryInputLocus {
                gene,
                contig,
                position,
                reference: allele,
                alternatives,
            })
        } else {
            InputLocus::Ambiguous(AmbiguousInputLocus {
                gene,
                contig,
                position,
                omitted: allele,
                alternatives,
            })
        });
        offset += 32;
    }
    Ok(output)
}
