//! Closed, benchmark-only reference encodings for format selection.
//!
//! These types are public solely so the maintenance builder and custom
//! benchmark can exercise the same bytes. They are not a production runtime
//! format or compatibility promise.

use memmap2::Mmap;
use pangopup_core::Grch38Contig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::cell::RefCell;
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[cfg(test)]
thread_local! {
    static CANDIDATE_READS: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
}

fn audit_read(offset: u64, length: u64) {
    #[cfg(test)]
    CANDIDATE_READS.with(|reads| reads.borrow_mut().push((offset, length)));
    #[cfg(not(test))]
    let _ = (offset, length);
}

#[cfg(test)]
fn reset_candidate_reads() {
    CANDIDATE_READS.with(|reads| reads.borrow_mut().clear());
}

#[cfg(test)]
fn candidate_read_pages() -> Vec<u64> {
    CANDIDATE_READS.with(|reads| {
        pages_for_ranges(
            &reads
                .borrow()
                .iter()
                .map(|(start, length)| (*start, start + length))
                .collect::<Vec<_>>(),
        )
    })
}

#[cfg(test)]
fn candidate_read_ranges() -> Vec<(u64, u64)> {
    CANDIDATE_READS.with(|reads| reads.borrow().clone())
}

pub const PAGE_BYTES: u64 = 4096;
const HEADER_BYTES: usize = 64;
const DIRECTORY_ENTRY_BYTES: usize = 48;
const MAGIC: &[u8; 8] = b"PGRBEN01";
const VERSION: u16 = 1;

const fn acgt_ascii_table() -> [[u8; 4]; 256] {
    let mut table = [[0_u8; 4]; 256];
    let mut byte = 0;
    while byte < 256 {
        table[byte][0] = b"ACGT"[byte & 3];
        table[byte][1] = b"ACGT"[(byte >> 2) & 3];
        table[byte][2] = b"ACGT"[(byte >> 4) & 3];
        table[byte][3] = b"ACGT"[(byte >> 6) & 3];
        byte += 1;
    }
    table
}

const ACGT_ASCII: [[u8; 4]; 256] = acgt_ascii_table();

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateCodec {
    Ascii8,
    Iupac4,
    Acgt2RleV1,
}

impl CandidateCodec {
    pub const ALL: [Self; 3] = [Self::Ascii8, Self::Iupac4, Self::Acgt2RleV1];

    pub const fn code(self) -> u8 {
        match self {
            Self::Ascii8 => 1,
            Self::Iupac4 => 2,
            Self::Acgt2RleV1 => 3,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Ascii8 => "ascii8",
            Self::Iupac4 => "iupac4",
            Self::Acgt2RleV1 => "acgt2-rle-v1",
        }
    }

    pub const fn filename(self) -> &'static str {
        match self {
            Self::Ascii8 => "ascii8.pgr",
            Self::Iupac4 => "iupac4.pgr",
            Self::Acgt2RleV1 => "acgt2-rle-v1.pgr",
        }
    }

    fn from_code(value: u8) -> Result<Self, CandidateError> {
        Self::ALL
            .into_iter()
            .find(|codec| codec.code() == value)
            .ok_or(CandidateError::Container("codec"))
    }
}

impl fmt::Display for CandidateCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Debug)]
pub enum CandidateError {
    Io(io::Error),
    Input(&'static str),
    Container(&'static str),
    Bounds(&'static str),
    Resource(&'static str),
    Arithmetic(&'static str),
}

impl CandidateError {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Input(_) => "oracle",
            Self::Container(_) => "container",
            Self::Bounds(_) => "bounds",
            Self::Resource(_) | Self::Arithmetic(_) => "resource",
        }
    }
}

impl fmt::Display for CandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("candidate I/O failed"),
            Self::Input(reason) => write!(formatter, "invalid reference input: {reason}"),
            Self::Container(reason) => write!(formatter, "invalid candidate container: {reason}"),
            Self::Bounds(reason) => write!(formatter, "reference window out of bounds: {reason}"),
            Self::Resource(reason) => write!(formatter, "candidate resource limit: {reason}"),
            Self::Arithmetic(reason) => {
                write!(formatter, "candidate arithmetic overflow: {reason}")
            }
        }
    }
}

impl std::error::Error for CandidateError {}

impl From<io::Error> for CandidateError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContigPlan {
    pub contig: Grch38Contig,
    pub bases: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct DirectoryEntry {
    code: u8,
    bases: u64,
    data_offset: u64,
    data_length: u64,
    auxiliary_offset: u64,
    auxiliary_count: u64,
}

#[derive(Clone, Copy, Debug)]
struct AmbiguityRun {
    start: u32,
    length: u32,
    code: u8,
}

/// Incrementally writes all three candidates while the builder streams FASTA.
pub struct CandidateSetWriter {
    plans: Vec<ContigPlan>,
    writers: [CandidateWriter; 3],
    next_contig: usize,
}

impl CandidateSetWriter {
    pub fn create(root: &Path, plans: &[ContigPlan]) -> Result<Self, CandidateError> {
        validate_plans(plans)?;
        let writers = [
            CandidateWriter::create(&root.join("ascii8.pgr"), CandidateCodec::Ascii8, plans)?,
            CandidateWriter::create(&root.join("iupac4.pgr"), CandidateCodec::Iupac4, plans)?,
            CandidateWriter::create(
                &root.join("acgt2-rle-v1.pgr"),
                CandidateCodec::Acgt2RleV1,
                plans,
            )?,
        ];
        Ok(Self {
            plans: plans.to_vec(),
            writers,
            next_contig: 0,
        })
    }

    pub fn begin_contig(&mut self, contig: Grch38Contig) -> Result<(), CandidateError> {
        let expected = self
            .plans
            .get(self.next_contig)
            .ok_or(CandidateError::Input("extra contig"))?;
        if expected.contig != contig {
            return Err(CandidateError::Input("contig order"));
        }
        for writer in &mut self.writers {
            writer.begin_contig(*expected)?;
        }
        Ok(())
    }

    pub fn write_bases(&mut self, bases: &[u8]) -> Result<(), CandidateError> {
        for writer in &mut self.writers {
            writer.write_bases(bases)?;
        }
        Ok(())
    }

    pub fn end_contig(&mut self) -> Result<(), CandidateError> {
        for writer in &mut self.writers {
            writer.end_contig()?;
        }
        self.next_contig += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), CandidateError> {
        if self.next_contig != self.plans.len() {
            return Err(CandidateError::Input("missing contig"));
        }
        for writer in &mut self.writers {
            writer.finish()?;
        }
        Ok(())
    }
}

fn validate_plans(plans: &[ContigPlan]) -> Result<(), CandidateError> {
    if plans.is_empty() || plans.len() > u8::MAX as usize {
        return Err(CandidateError::Input("contig count"));
    }
    let mut prior = 0;
    for plan in plans {
        if plan.contig.code() <= prior || plan.bases == 0 || plan.bases > u32::MAX as u64 {
            return Err(CandidateError::Input("contig plan"));
        }
        prior = plan.contig.code();
    }
    let directory_end = HEADER_BYTES
        .checked_add(
            plans
                .len()
                .checked_mul(DIRECTORY_ENTRY_BYTES)
                .ok_or(CandidateError::Arithmetic("directory"))?,
        )
        .ok_or(CandidateError::Arithmetic("directory"))?;
    if directory_end > PAGE_BYTES as usize {
        return Err(CandidateError::Resource("directory"));
    }
    Ok(())
}

struct CandidateWriter {
    codec: CandidateCodec,
    writer: BufWriter<File>,
    plans: Vec<ContigPlan>,
    entries: Vec<DirectoryEntry>,
    current: Option<ContigPlan>,
    current_bases: u64,
    packed: u8,
    packed_count: u8,
    ambiguity_scratch: Option<(PathBuf, File)>,
    ambiguity_count: u64,
    current_run: Option<AmbiguityRun>,
    position: u64,
}

impl CandidateWriter {
    fn create(
        path: &Path,
        codec: CandidateCodec,
        plans: &[ContigPlan],
    ) -> Result<Self, CandidateError> {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&[0; PAGE_BYTES as usize])?;
        let ambiguity_scratch = if codec == CandidateCodec::Acgt2RleV1 {
            let filename = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or(CandidateError::Input("candidate filename"))?;
            let scratch_path = path.with_file_name(format!(".{filename}.runs.scratch"));
            let scratch = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&scratch_path)?;
            Some((scratch_path, scratch))
        } else {
            None
        };
        Ok(Self {
            codec,
            writer,
            plans: plans.to_vec(),
            entries: Vec::with_capacity(plans.len()),
            current: None,
            current_bases: 0,
            packed: 0,
            packed_count: 0,
            ambiguity_scratch,
            ambiguity_count: 0,
            current_run: None,
            position: PAGE_BYTES,
        })
    }

    fn begin_contig(&mut self, plan: ContigPlan) -> Result<(), CandidateError> {
        if self.current.is_some() {
            return Err(CandidateError::Input("nested contig"));
        }
        self.current = Some(plan);
        self.current_bases = 0;
        self.packed = 0;
        self.packed_count = 0;
        self.ambiguity_count = 0;
        if let Some((_, scratch)) = self.ambiguity_scratch.as_mut() {
            scratch.set_len(0)?;
            scratch.seek(SeekFrom::Start(0))?;
        }
        self.current_run = None;
        Ok(())
    }

    fn write_bases(&mut self, bases: &[u8]) -> Result<(), CandidateError> {
        let plan = self
            .current
            .ok_or(CandidateError::Input("bases outside contig"))?;
        let additional =
            u64::try_from(bases.len()).map_err(|_| CandidateError::Arithmetic("bases"))?;
        if self
            .current_bases
            .checked_add(additional)
            .ok_or(CandidateError::Arithmetic("bases"))?
            > plan.bases
        {
            return Err(CandidateError::Input("contig longer than plan"));
        }
        for raw in bases {
            let base = raw.to_ascii_uppercase();
            let code = iupac_code(base).ok_or(CandidateError::Input("unsupported IUPAC symbol"))?;
            match self.codec {
                CandidateCodec::Ascii8 => self.writer.write_all(&[base])?,
                CandidateCodec::Iupac4 => {
                    if self.packed_count == 0 {
                        self.packed = code;
                        self.packed_count = 1;
                    } else {
                        self.packed |= code << 4;
                        self.writer.write_all(&[self.packed])?;
                        self.packed = 0;
                        self.packed_count = 0;
                    }
                }
                CandidateCodec::Acgt2RleV1 => {
                    let dense = if code <= 3 { code } else { 0 };
                    self.packed |= dense << (self.packed_count * 2);
                    self.packed_count += 1;
                    if self.packed_count == 4 {
                        self.writer.write_all(&[self.packed])?;
                        self.packed = 0;
                        self.packed_count = 0;
                    }
                    self.extend_run(code)?;
                }
            }
            self.current_bases += 1;
        }
        Ok(())
    }

    fn extend_run(&mut self, code: u8) -> Result<(), CandidateError> {
        if self.codec != CandidateCodec::Acgt2RleV1 {
            return Ok(());
        }
        let position = u32::try_from(self.current_bases)
            .map_err(|_| CandidateError::Resource("ambiguity position"))?;
        match (code > 3, self.current_run.as_mut()) {
            (true, Some(run)) if run.code == code => {
                run.length = run
                    .length
                    .checked_add(1)
                    .ok_or(CandidateError::Arithmetic("ambiguity run"))?;
            }
            (true, Some(_)) => {
                self.finish_current_run()?;
                self.current_run = Some(AmbiguityRun {
                    start: position,
                    length: 1,
                    code,
                });
            }
            (true, None) => {
                self.current_run = Some(AmbiguityRun {
                    start: position,
                    length: 1,
                    code,
                })
            }
            (false, Some(_)) => {
                self.finish_current_run()?;
            }
            (false, None) => {}
        }
        Ok(())
    }

    fn finish_current_run(&mut self) -> Result<(), CandidateError> {
        let Some(run) = self.current_run.take() else {
            return Ok(());
        };
        let (_, scratch) = self
            .ambiguity_scratch
            .as_mut()
            .ok_or(CandidateError::Input("ambiguity scratch"))?;
        write_run(scratch, run)?;
        self.ambiguity_count = self
            .ambiguity_count
            .checked_add(1)
            .ok_or(CandidateError::Arithmetic("ambiguity count"))?;
        Ok(())
    }

    fn end_contig(&mut self) -> Result<(), CandidateError> {
        let plan = self
            .current
            .take()
            .ok_or(CandidateError::Input("missing contig"))?;
        if self.current_bases != plan.bases {
            return Err(CandidateError::Input("contig length differs from plan"));
        }
        let data_offset = self.position;
        if self.packed_count != 0 {
            if self.codec == CandidateCodec::Iupac4 {
                self.packed |= 15 << 4;
            }
            self.writer.write_all(&[self.packed])?;
            self.packed = 0;
            self.packed_count = 0;
        }
        let data_length = match self.codec {
            CandidateCodec::Ascii8 => plan.bases,
            CandidateCodec::Iupac4 => plan.bases.div_ceil(2),
            CandidateCodec::Acgt2RleV1 => plan.bases.div_ceil(4),
        };
        self.position = data_offset
            .checked_add(data_length)
            .ok_or(CandidateError::Arithmetic("payload"))?;
        let (auxiliary_offset, auxiliary_count) = if self.codec == CandidateCodec::Acgt2RleV1 {
            self.finish_current_run()?;
            if self.ambiguity_count == 0 {
                (0, 0)
            } else {
                let aligned = self
                    .position
                    .checked_add(7)
                    .ok_or(CandidateError::Arithmetic("alignment"))?
                    & !7;
                let padding = usize::try_from(aligned - self.position)
                    .map_err(|_| CandidateError::Arithmetic("alignment"))?;
                self.writer.write_all(&[0; 7][..padding])?;
                let (_, scratch) = self
                    .ambiguity_scratch
                    .as_mut()
                    .ok_or(CandidateError::Input("ambiguity scratch"))?;
                scratch.seek(SeekFrom::Start(0))?;
                let count = self.ambiguity_count;
                let expected = count
                    .checked_mul(16)
                    .ok_or(CandidateError::Arithmetic("runs"))?;
                let copied = io::copy(&mut scratch.take(expected), &mut self.writer)?;
                if copied != expected {
                    return Err(CandidateError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "ambiguity scratch truncated",
                    )));
                }
                self.position = aligned
                    .checked_add(
                        count
                            .checked_mul(16)
                            .ok_or(CandidateError::Arithmetic("runs"))?,
                    )
                    .ok_or(CandidateError::Arithmetic("runs"))?;
                (aligned, count)
            }
        } else {
            (0, 0)
        };
        self.entries.push(DirectoryEntry {
            code: plan.contig.code(),
            bases: plan.bases,
            data_offset,
            data_length,
            auxiliary_offset,
            auxiliary_count,
        });
        Ok(())
    }

    fn finish(&mut self) -> Result<(), CandidateError> {
        if self.current.is_some() || self.entries.len() != self.plans.len() {
            return Err(CandidateError::Input("incomplete candidate"));
        }
        self.writer.flush()?;
        let file_length = self.position;
        self.writer.seek(SeekFrom::Start(0))?;
        let mut header = [0_u8; HEADER_BYTES];
        header[0..8].copy_from_slice(MAGIC);
        header[8..10].copy_from_slice(&VERSION.to_le_bytes());
        header[10] = self.codec.code();
        header[11] = self.entries.len() as u8;
        header[16..24].copy_from_slice(&file_length.to_le_bytes());
        header[24..32].copy_from_slice(&(HEADER_BYTES as u64).to_le_bytes());
        header[32..40]
            .copy_from_slice(&((self.entries.len() * DIRECTORY_ENTRY_BYTES) as u64).to_le_bytes());
        header[40..48].copy_from_slice(&PAGE_BYTES.to_le_bytes());
        header[48..56].copy_from_slice(&(file_length - PAGE_BYTES).to_le_bytes());
        self.writer.write_all(&header)?;
        for entry in &self.entries {
            let mut bytes = [0_u8; DIRECTORY_ENTRY_BYTES];
            bytes[0] = entry.code;
            bytes[8..16].copy_from_slice(&entry.bases.to_le_bytes());
            bytes[16..24].copy_from_slice(&entry.data_offset.to_le_bytes());
            bytes[24..32].copy_from_slice(&entry.data_length.to_le_bytes());
            bytes[32..40].copy_from_slice(&entry.auxiliary_offset.to_le_bytes());
            bytes[40..48].copy_from_slice(&entry.auxiliary_count.to_le_bytes());
            self.writer.write_all(&bytes)?;
        }
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        if let Some((path, _)) = self.ambiguity_scratch.take() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

impl Drop for CandidateWriter {
    fn drop(&mut self) {
        if let Some((path, _)) = self.ambiguity_scratch.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn write_run(writer: &mut File, run: AmbiguityRun) -> Result<(), CandidateError> {
    writer.write_all(&run.start.to_le_bytes())?;
    writer.write_all(&run.length.to_le_bytes())?;
    writer.write_all(&[run.code])?;
    writer.write_all(&[0; 7])?;
    Ok(())
}

/// A cheap-open mmap reader for one benchmark candidate.
pub struct CandidateReader {
    mmap: Mmap,
    codec: CandidateCodec,
    entries: Vec<DirectoryEntry>,
}

impl CandidateReader {
    pub fn open(path: &Path) -> Result<Self, CandidateError> {
        let file = File::open(path)?;
        let length = file.metadata()?.len();
        if length < PAGE_BYTES {
            return Err(CandidateError::Container("file length"));
        }
        // SAFETY: the read-only map is retained by this reader and every byte
        // access is bounds checked before decoding.
        let mmap = unsafe { Mmap::map(&file)? };
        let header = mmap
            .get(..HEADER_BYTES)
            .ok_or(CandidateError::Container("header"))?;
        if &header[0..8] != MAGIC || le_u16(header, 8)? != VERSION {
            return Err(CandidateError::Container("magic or version"));
        }
        if header[12..16].iter().any(|byte| *byte != 0)
            || header[56..64].iter().any(|byte| *byte != 0)
        {
            return Err(CandidateError::Container("reserved header"));
        }
        let codec = CandidateCodec::from_code(header[10])?;
        let count = header[11] as usize;
        if count == 0
            || le_u64(header, 16)? != length
            || le_u64(header, 24)? != 64
            || le_u64(header, 32)? != (count * 48) as u64
            || le_u64(header, 40)? != PAGE_BYTES
            || le_u64(header, 48)? != length - PAGE_BYTES
        {
            return Err(CandidateError::Container("header fields"));
        }
        audit_read(0, HEADER_BYTES as u64);
        audit_read(HEADER_BYTES as u64, (count * DIRECTORY_ENTRY_BYTES) as u64);
        let directory_end = HEADER_BYTES
            .checked_add(
                count
                    .checked_mul(DIRECTORY_ENTRY_BYTES)
                    .ok_or(CandidateError::Arithmetic("directory"))?,
            )
            .ok_or(CandidateError::Arithmetic("directory"))?;
        if directory_end > PAGE_BYTES as usize {
            return Err(CandidateError::Container("directory size"));
        }
        let mut entries = Vec::with_capacity(count);
        let mut expected_offset = PAGE_BYTES;
        let mut prior_code = 0;
        for index in 0..count {
            let start = HEADER_BYTES + index * DIRECTORY_ENTRY_BYTES;
            let bytes = &mmap[start..start + DIRECTORY_ENTRY_BYTES];
            if bytes[1..8].iter().any(|byte| *byte != 0) {
                return Err(CandidateError::Container("directory reserved"));
            }
            let entry = DirectoryEntry {
                code: bytes[0],
                bases: le_u64(bytes, 8)?,
                data_offset: le_u64(bytes, 16)?,
                data_length: le_u64(bytes, 24)?,
                auxiliary_offset: le_u64(bytes, 32)?,
                auxiliary_count: le_u64(bytes, 40)?,
            };
            Grch38Contig::from_code(entry.code)
                .map_err(|_| CandidateError::Container("contig code"))?;
            if entry.code <= prior_code
                || entry.bases == 0
                || entry.bases > u32::MAX as u64
                || entry.data_offset != expected_offset
            {
                return Err(CandidateError::Container("directory order or range"));
            }
            let expected_data = match codec {
                CandidateCodec::Ascii8 => entry.bases,
                CandidateCodec::Iupac4 => entry.bases.div_ceil(2),
                CandidateCodec::Acgt2RleV1 => entry.bases.div_ceil(4),
            };
            if entry.data_length != expected_data {
                return Err(CandidateError::Container("encoded length"));
            }
            let dense_end = entry
                .data_offset
                .checked_add(entry.data_length)
                .ok_or(CandidateError::Arithmetic("data end"))?;
            expected_offset = match codec {
                CandidateCodec::Ascii8 | CandidateCodec::Iupac4 => {
                    if entry.auxiliary_offset != 0 || entry.auxiliary_count != 0 {
                        return Err(CandidateError::Container("unexpected auxiliary table"));
                    }
                    dense_end
                }
                CandidateCodec::Acgt2RleV1 if entry.auxiliary_count == 0 => {
                    if entry.auxiliary_offset != 0 {
                        return Err(CandidateError::Container("empty auxiliary table"));
                    }
                    dense_end
                }
                CandidateCodec::Acgt2RleV1 => {
                    let aligned = dense_end
                        .checked_add(7)
                        .ok_or(CandidateError::Arithmetic("alignment"))?
                        & !7;
                    if entry.auxiliary_offset != aligned {
                        return Err(CandidateError::Container("auxiliary alignment"));
                    }
                    aligned
                        .checked_add(
                            entry
                                .auxiliary_count
                                .checked_mul(16)
                                .ok_or(CandidateError::Arithmetic("runs"))?,
                        )
                        .ok_or(CandidateError::Arithmetic("runs"))?
                }
            };
            if expected_offset > length {
                return Err(CandidateError::Container("section bounds"));
            }
            prior_code = entry.code;
            entries.push(entry);
        }
        if expected_offset != length {
            return Err(CandidateError::Container("trailing bytes"));
        }
        Ok(Self {
            mmap,
            codec,
            entries,
        })
    }

    pub const fn codec(&self) -> CandidateCodec {
        self.codec
    }

    pub fn copy_window(
        &self,
        contig: Grch38Contig,
        start_1based: u64,
        destination: &mut [u8],
    ) -> Result<(), CandidateError> {
        let entry = self.entry(contig)?;
        let start = start_1based
            .checked_sub(1)
            .ok_or(CandidateError::Bounds("start must be one"))?;
        let end = start
            .checked_add(destination.len() as u64)
            .ok_or(CandidateError::Bounds("window end"))?;
        if destination.is_empty() || end > entry.bases {
            return Err(CandidateError::Bounds("window range"));
        }
        self.validate_window(entry, start, end)?;
        match self.codec {
            CandidateCodec::Ascii8 => {
                let begin = (entry.data_offset + start) as usize;
                destination.copy_from_slice(&self.mmap[begin..begin + destination.len()]);
            }
            CandidateCodec::Iupac4 => {
                let first = start / 2;
                let packed = self.range(entry.data_offset + first, end.div_ceil(2) - first)?;
                for (offset, output) in destination.iter_mut().enumerate() {
                    let position = start + offset as u64;
                    let byte = packed[(position / 2 - first) as usize];
                    let code = if position.is_multiple_of(2) {
                        byte & 15
                    } else {
                        byte >> 4
                    };
                    *output = iupac_ascii(code).ok_or(CandidateError::Container("IUPAC code"))?;
                }
            }
            CandidateCodec::Acgt2RleV1 => {
                self.copy_acgt_dense(entry, start, end, destination)?;
                self.overlay_runs(entry, start, end, destination)?;
            }
        }
        Ok(())
    }

    fn validate_window(
        &self,
        entry: &DirectoryEntry,
        start: u64,
        end: u64,
    ) -> Result<(), CandidateError> {
        match self.codec {
            CandidateCodec::Ascii8 => {
                let bytes = self.range(entry.data_offset + start, end - start)?;
                if bytes.iter().any(|byte| iupac_code(*byte).is_none()) {
                    return Err(CandidateError::Container("ASCII symbol"));
                }
            }
            CandidateCodec::Iupac4 => {
                let first = start / 2;
                let packed = self.range(entry.data_offset + first, end.div_ceil(2) - first)?;
                for position in start..end {
                    let byte = packed[(position / 2 - first) as usize];
                    let code = if position.is_multiple_of(2) {
                        byte & 15
                    } else {
                        byte >> 4
                    };
                    if iupac_ascii(code).is_none() {
                        return Err(CandidateError::Container("IUPAC code"));
                    }
                }
            }
            CandidateCodec::Acgt2RleV1 => {
                self.validate_runs(entry, start, end)?;
            }
        }
        Ok(())
    }

    pub fn inspect_payload(&self) -> Result<(), CandidateError> {
        let directory_end = HEADER_BYTES
            .checked_add(
                self.entries
                    .len()
                    .checked_mul(DIRECTORY_ENTRY_BYTES)
                    .ok_or(CandidateError::Arithmetic("directory"))?,
            )
            .ok_or(CandidateError::Arithmetic("directory"))?;
        if self.mmap[directory_end..PAGE_BYTES as usize]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(CandidateError::Container("header padding"));
        }
        for entry in &self.entries {
            match self.codec {
                CandidateCodec::Ascii8 => {
                    let bytes = self.range(entry.data_offset, entry.data_length)?;
                    if bytes
                        .iter()
                        .any(|byte| iupac_code(*byte).is_none() || !byte.is_ascii_uppercase())
                    {
                        return Err(CandidateError::Container("ASCII symbol"));
                    }
                }
                CandidateCodec::Iupac4 => {
                    let bytes = self.range(entry.data_offset, entry.data_length)?;
                    for (index, packed) in bytes.iter().enumerate() {
                        let final_odd = index + 1 == bytes.len() && entry.bases % 2 == 1;
                        if iupac_ascii(packed & 15).is_none()
                            || (if final_odd {
                                packed >> 4 != 15
                            } else {
                                iupac_ascii(packed >> 4).is_none()
                            })
                        {
                            return Err(CandidateError::Container("IUPAC payload"));
                        }
                    }
                }
                CandidateCodec::Acgt2RleV1 => {
                    if entry.auxiliary_count > 0 {
                        let dense_end = entry
                            .data_offset
                            .checked_add(entry.data_length)
                            .ok_or(CandidateError::Arithmetic("dense end"))?;
                        let padding = self.range(
                            dense_end,
                            entry
                                .auxiliary_offset
                                .checked_sub(dense_end)
                                .ok_or(CandidateError::Container("auxiliary alignment"))?,
                        )?;
                        if padding.iter().any(|byte| *byte != 0) {
                            return Err(CandidateError::Container("auxiliary alignment"));
                        }
                    }
                    if entry.bases % 4 != 0 {
                        let last = *self
                            .range(entry.data_offset + entry.data_length - 1, 1)?
                            .first()
                            .ok_or(CandidateError::Container("dense payload"))?;
                        let used = (entry.bases % 4) * 2;
                        if last >> used != 0 {
                            return Err(CandidateError::Container("two-bit padding"));
                        }
                    }
                    self.validate_all_runs(entry)?;
                }
            }
        }
        Ok(())
    }

    fn copy_acgt_dense(
        &self,
        entry: &DirectoryEntry,
        start: u64,
        end: u64,
        destination: &mut [u8],
    ) -> Result<(), CandidateError> {
        let first_packed = start / 4;
        let bytes = self.range(
            entry.data_offset + first_packed,
            end.div_ceil(4) - first_packed,
        )?;
        let mut genomic = start;
        let mut output = 0_usize;
        while genomic < end && !genomic.is_multiple_of(4) {
            let packed = bytes[(genomic / 4 - first_packed) as usize];
            destination[output] = ACGT_ASCII[packed as usize][(genomic % 4) as usize];
            genomic += 1;
            output += 1;
        }
        while genomic.checked_add(4).is_some_and(|next| next <= end) {
            let packed = bytes[(genomic / 4 - first_packed) as usize];
            destination[output..output + 4].copy_from_slice(&ACGT_ASCII[packed as usize]);
            genomic += 4;
            output += 4;
        }
        while genomic < end {
            let packed = bytes[(genomic / 4 - first_packed) as usize];
            destination[output] = ACGT_ASCII[packed as usize][(genomic % 4) as usize];
            genomic += 1;
            output += 1;
        }
        Ok(())
    }

    pub fn trace_window(
        &self,
        contig: Grch38Contig,
        start_1based: u64,
        length: usize,
    ) -> Result<Vec<u64>, CandidateError> {
        let entry = self.entry(contig)?;
        let start = start_1based
            .checked_sub(1)
            .ok_or(CandidateError::Bounds("start must be one"))?;
        let end = start
            .checked_add(length as u64)
            .ok_or(CandidateError::Bounds("window end"))?;
        if length == 0 || end > entry.bases {
            return Err(CandidateError::Bounds("window range"));
        }
        let mut ranges = Vec::new();
        match self.codec {
            CandidateCodec::Ascii8 => {
                ranges.push((entry.data_offset + start, entry.data_offset + end))
            }
            CandidateCodec::Iupac4 => ranges.push((
                entry.data_offset + start / 2,
                entry.data_offset + end.div_ceil(2),
            )),
            CandidateCodec::Acgt2RleV1 => {
                ranges.push((
                    entry.data_offset + start / 4,
                    entry.data_offset + end.div_ceil(4),
                ));
                let mut examined = Vec::new();
                self.walk_examined_runs(entry, start, end, |index, _| {
                    examined.push(index);
                    Ok(())
                })?;
                for index in examined {
                    let offset = entry.auxiliary_offset + index * 16;
                    ranges.push((offset, offset + 16));
                }
            }
        }
        Ok(pages_for_ranges(&ranges))
    }

    pub fn open_trace_pages(&self) -> Vec<u64> {
        pages_for_ranges(&[(0, 64), (64, 64 + self.entries.len() as u64 * 48)])
    }

    pub fn file_len(&self) -> u64 {
        self.mmap.len() as u64
    }

    fn entry(&self, contig: Grch38Contig) -> Result<&DirectoryEntry, CandidateError> {
        self.entries
            .binary_search_by_key(&contig.code(), |entry| entry.code)
            .ok()
            .and_then(|index| self.entries.get(index))
            .ok_or(CandidateError::Bounds("unknown contig"))
    }

    fn range(&self, offset: u64, length: u64) -> Result<&[u8], CandidateError> {
        let end = offset
            .checked_add(length)
            .ok_or(CandidateError::Arithmetic("range"))?;
        let start = usize::try_from(offset).map_err(|_| CandidateError::Arithmetic("range"))?;
        let end = usize::try_from(end).map_err(|_| CandidateError::Arithmetic("range"))?;
        let result = self
            .mmap
            .get(start..end)
            .ok_or(CandidateError::Container("section bounds"))?;
        audit_read(offset, length);
        Ok(result)
    }

    fn read_run(&self, entry: &DirectoryEntry, index: u64) -> Result<AmbiguityRun, CandidateError> {
        if index >= entry.auxiliary_count {
            return Err(CandidateError::Container("run index"));
        }
        let bytes = self.range(entry.auxiliary_offset + index * 16, 16)?;
        if bytes[9..16].iter().any(|byte| *byte != 0) {
            return Err(CandidateError::Container("run reserved"));
        }
        let run = AmbiguityRun {
            start: le_u32(bytes, 0)?,
            length: le_u32(bytes, 4)?,
            code: bytes[8],
        };
        if run.length == 0
            || !(4..=14).contains(&run.code)
            || (run.start as u64)
                .checked_add(run.length as u64)
                .is_none_or(|end| end > entry.bases)
        {
            return Err(CandidateError::Container("ambiguity run"));
        }
        Ok(run)
    }

    fn validate_all_runs(&self, entry: &DirectoryEntry) -> Result<(), CandidateError> {
        let mut prior: Option<AmbiguityRun> = None;
        for index in 0..entry.auxiliary_count {
            let run = self.read_run(entry, index)?;
            if let Some(previous) = prior {
                let previous_end = previous
                    .start
                    .checked_add(previous.length)
                    .ok_or(CandidateError::Arithmetic("run end"))?;
                if run.start < previous_end
                    || (run.start == previous_end && run.code == previous.code)
                {
                    return Err(CandidateError::Container("run order or coalescing"));
                }
            }
            prior = Some(run);
        }
        Ok(())
    }

    fn walk_examined_runs(
        &self,
        entry: &DirectoryEntry,
        start: u64,
        end: u64,
        mut visitor: impl FnMut(u64, AmbiguityRun) -> Result<(), CandidateError>,
    ) -> Result<(), CandidateError> {
        let mut low = 0;
        let mut high = entry.auxiliary_count;
        while low < high {
            let mid = low + (high - low) / 2;
            let run = self.read_run(entry, mid)?;
            visitor(mid, run)?;
            let run_end = (run.start as u64)
                .checked_add(run.length as u64)
                .ok_or(CandidateError::Arithmetic("run end"))?;
            if run_end > start {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        let mut index = low;
        while index < entry.auxiliary_count {
            let run = self.read_run(entry, index)?;
            visitor(index, run)?;
            if run.start as u64 >= end {
                break;
            }
            index += 1;
        }
        Ok(())
    }

    fn validate_runs(
        &self,
        entry: &DirectoryEntry,
        start: u64,
        end: u64,
    ) -> Result<(), CandidateError> {
        self.walk_examined_runs(entry, start, end, |_, _| Ok(()))
    }

    fn overlay_runs(
        &self,
        entry: &DirectoryEntry,
        start: u64,
        end: u64,
        destination: &mut [u8],
    ) -> Result<(), CandidateError> {
        self.walk_examined_runs(entry, start, end, |_, run| {
            let run_start = run.start as u64;
            let run_end = run_start + run.length as u64;
            if run_start < end {
                let overlap_start = run_start.max(start);
                let overlap_end = run_end.min(end);
                if overlap_start < overlap_end {
                    let base =
                        iupac_ascii(run.code).ok_or(CandidateError::Container("run code"))?;
                    destination[(overlap_start - start) as usize..(overlap_end - start) as usize]
                        .fill(base);
                }
            }
            Ok(())
        })
    }
}

fn pages_for_ranges(ranges: &[(u64, u64)]) -> Vec<u64> {
    let mut pages = BTreeSet::new();
    for &(start, end) in ranges {
        if start < end {
            for page in start / PAGE_BYTES..=(end - 1) / PAGE_BYTES {
                pages.insert(page);
            }
        }
    }
    pages.into_iter().collect()
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, CandidateError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|slice| slice.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(CandidateError::Container("u16"))
}
fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, CandidateError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(CandidateError::Container("u32"))
}
fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, CandidateError> {
    bytes
        .get(offset..offset + 8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(CandidateError::Container("u64"))
}

pub fn iupac_code(base: u8) -> Option<u8> {
    b"ACGTRYSWKMBDHVN"
        .iter()
        .position(|candidate| *candidate == base.to_ascii_uppercase())
        .map(|index| index as u8)
}

pub fn iupac_ascii(code: u8) -> Option<u8> {
    b"ACGTRYSWKMBDHVN".get(code as usize).copied()
}

pub fn sha256_file(path: &Path) -> Result<(u64, String), CandidateError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let bytes = io::copy(&mut file, &mut hasher)?;
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

/// Privately stage and atomically publish one benchmark report without
/// replacing an existing path. A post-rename parent-sync failure rolls the
/// report back so an error never leaves a report behind.
pub fn publish_benchmark_report(output: &Path, bytes: &[u8]) -> Result<(), CandidateError> {
    let parent = output
        .parent()
        .ok_or(CandidateError::Input("report parent"))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(CandidateError::Input("report name"))?;
    let staging = parent.join(format!(".{name}.staging-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&staging)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&staging);
        return Err(CandidateError::Io(error));
    }
    let result = publish_report_with(
        || {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &staging,
                rustix::fs::CWD,
                output,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| CandidateError::Io(io::Error::from_raw_os_error(error.raw_os_error())))
        },
        || {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(CandidateError::Io)
        },
        || {
            fs::remove_file(output)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        },
    );
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

fn publish_report_with(
    rename: impl FnOnce() -> Result<(), CandidateError>,
    sync_parent: impl FnOnce() -> Result<(), CandidateError>,
    rollback: impl FnOnce() -> Result<(), CandidateError>,
) -> Result<(), CandidateError> {
    rename()?;
    if sync_parent().is_err() {
        rollback()?;
        return Err(CandidateError::Io(io::Error::other(
            "report parent sync failed",
        )));
    }
    Ok(())
}

/// Inputs to the pure deterministic selection evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionCandidate {
    pub codec: CandidateCodec,
    pub headline_p50_ns: u64,
    pub headline_p95_ns: u64,
    pub unique_page_count: u64,
    pub member_bytes: u64,
    pub zstd_bytes: u64,
    pub evidence_valid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStatus {
    Selected,
    Proposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    Speed,
    Pages,
    FileBytes,
    ZstdBytes,
    InvalidEvidence,
    ExactTie,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SelectionResult {
    pub status: SelectionStatus,
    pub codec: Option<CandidateCodec>,
    pub reason: SelectionReason,
}

pub fn select_candidate(candidates: &[SelectionCandidate]) -> SelectionResult {
    if candidates.len() != 3
        || CandidateCodec::ALL.iter().any(|codec| {
            candidates
                .iter()
                .filter(|candidate| candidate.codec == *codec)
                .count()
                != 1
        })
        || candidates.iter().any(|candidate| !candidate.evidence_valid)
    {
        return SelectionResult {
            status: SelectionStatus::Proposed,
            codec: None,
            reason: SelectionReason::InvalidEvidence,
        };
    }
    let speed: Vec<_> = candidates
        .iter()
        .filter(|winner| {
            candidates
                .iter()
                .filter(|other| other.codec != winner.codec)
                .all(|other| speed_dominates(winner, other))
        })
        .collect();
    if let [winner] = speed.as_slice() {
        return selected(winner.codec, SelectionReason::Speed);
    }
    let mut survivors: Vec<_> = candidates.iter().collect();
    retain_minimum(&mut survivors, |value| value.unique_page_count);
    if let [winner] = survivors.as_slice() {
        return selected(winner.codec, SelectionReason::Pages);
    }
    retain_minimum(&mut survivors, |value| value.member_bytes);
    if let [winner] = survivors.as_slice() {
        return selected(winner.codec, SelectionReason::FileBytes);
    }
    retain_minimum(&mut survivors, |value| value.zstd_bytes);
    if let [winner] = survivors.as_slice() {
        return selected(winner.codec, SelectionReason::ZstdBytes);
    }
    SelectionResult {
        status: SelectionStatus::Proposed,
        codec: None,
        reason: SelectionReason::ExactTie,
    }
}

fn speed_dominates(winner: &SelectionCandidate, other: &SelectionCandidate) -> bool {
    (winner.headline_p50_ns as u128) * 100 <= (other.headline_p50_ns as u128) * 95
        && (winner.headline_p95_ns as u128) * 100 <= (other.headline_p95_ns as u128) * 95
}

fn retain_minimum(
    candidates: &mut Vec<&SelectionCandidate>,
    metric: impl Fn(&SelectionCandidate) -> u64,
) {
    if let Some(minimum) = candidates.iter().map(|candidate| metric(candidate)).min() {
        candidates.retain(|candidate| metric(candidate) == minimum);
    }
}

fn selected(codec: CandidateCodec, reason: SelectionReason) -> SelectionResult {
    SelectionResult {
        status: SelectionStatus::Selected,
        codec: Some(codec),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn fixture_member(codec: CandidateCodec) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/reference-candidates-mini/candidates")
            .join(codec.filename())
    }

    fn scratch_member(codec: CandidateCodec) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pangopup-reference-reader-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::copy(fixture_member(codec), &path).expect("copy candidate");
        path
    }

    fn candidate(
        codec: CandidateCodec,
        p50: u64,
        p95: u64,
        pages: u64,
        bytes: u64,
        zstd: u64,
    ) -> SelectionCandidate {
        SelectionCandidate {
            codec,
            headline_p50_ns: p50,
            headline_p95_ns: p95,
            unique_page_count: pages,
            member_bytes: bytes,
            zstd_bytes: zstd,
            evidence_valid: true,
        }
    }

    #[test]
    fn reference_candidate_speed_requires_both_quantiles() {
        let values = [
            candidate(CandidateCodec::Ascii8, 94, 95, 9, 9, 9),
            candidate(CandidateCodec::Iupac4, 100, 100, 8, 8, 8),
            candidate(CandidateCodec::Acgt2RleV1, 101, 101, 7, 7, 7),
        ];
        assert_eq!(
            select_candidate(&values).codec,
            Some(CandidateCodec::Ascii8)
        );
        let values = [
            candidate(CandidateCodec::Ascii8, 94, 96, 9, 9, 9),
            candidate(CandidateCodec::Iupac4, 100, 100, 8, 8, 8),
            candidate(CandidateCodec::Acgt2RleV1, 101, 101, 7, 7, 7),
        ];
        assert_eq!(select_candidate(&values).reason, SelectionReason::Pages);
    }

    #[test]
    fn reference_candidate_ties_follow_closed_order() {
        let values = [
            candidate(CandidateCodec::Ascii8, 100, 100, 3, 8, 7),
            candidate(CandidateCodec::Iupac4, 100, 100, 3, 7, 9),
            candidate(CandidateCodec::Acgt2RleV1, 100, 100, 4, 6, 5),
        ];
        assert_eq!(
            select_candidate(&values),
            selected(CandidateCodec::Iupac4, SelectionReason::FileBytes)
        );
        let values = CandidateCodec::ALL.map(|codec| candidate(codec, 100, 100, 3, 7, 5));
        assert_eq!(select_candidate(&values).status, SelectionStatus::Proposed);
    }

    #[test]
    fn reference_candidate_invalid_evidence_never_selects() {
        let mut values = CandidateCodec::ALL.map(|codec| candidate(codec, 100, 100, 3, 7, 5));
        values[0].evidence_valid = false;
        assert_eq!(
            select_candidate(&values).reason,
            SelectionReason::InvalidEvidence
        );
    }

    #[test]
    fn reference_candidate_manual_contexts_and_pages_are_exact() {
        let chr3 = Grch38Contig::autosome(3).expect("chr3");
        let chrm = Grch38Contig::M;
        let windows = [
            (chr3, 1, b"ACGTRYSWKMBDHVN".as_slice()),
            (chr3, 4093, b"AAAAAAAA".as_slice()),
            (chr3, 8185, b"AAAAAAAAAAAAAAAA".as_slice()),
            (chr3, 16369, b"AAAAAAAAAAAAAAAA".as_slice()),
            (chrm, 1, b"NNRRACTGACGT".as_slice()),
        ];
        let expected_pages = [
            [vec![1], vec![1, 2], vec![2, 3], vec![4], vec![5]],
            [vec![1], vec![1], vec![1, 2], vec![2], vec![3]],
            [vec![1, 2], vec![1, 2], vec![1, 2], vec![1, 2], vec![2]],
        ];
        let expected_union = [6, 4, 3];
        for (codec_index, codec) in CandidateCodec::ALL.into_iter().enumerate() {
            let reader = CandidateReader::open(&fixture_member(codec)).expect("open fixture");
            let mut union: BTreeSet<u64> = reader.open_trace_pages().into_iter().collect();
            for (window_index, (contig, start, expected)) in windows.iter().enumerate() {
                let mut actual = vec![0_u8; expected.len()];
                reader
                    .copy_window(*contig, *start, &mut actual)
                    .expect("copy window");
                assert_eq!(&actual, expected);
                let pages = reader
                    .trace_window(*contig, *start, expected.len())
                    .expect("trace window");
                assert_eq!(pages, expected_pages[codec_index][window_index]);
                union.extend(pages);
            }
            assert_eq!(union.len(), expected_union[codec_index]);
        }
    }

    #[test]
    fn reference_candidate_bounds_do_not_modify_destination() {
        let reader =
            CandidateReader::open(&fixture_member(CandidateCodec::Iupac4)).expect("open fixture");
        let mut destination = [0x5a; 8];
        let before = destination;
        assert!(
            reader
                .copy_window(
                    Grch38Contig::autosome(3).expect("chr3"),
                    0,
                    &mut destination
                )
                .is_err()
        );
        assert_eq!(destination, before);
        assert!(
            reader
                .copy_window(
                    Grch38Contig::autosome(4).expect("chr4"),
                    1,
                    &mut destination
                )
                .is_err()
        );
        assert_eq!(destination, before);
        assert!(
            reader
                .copy_window(
                    Grch38Contig::autosome(3).expect("chr3"),
                    u64::MAX,
                    &mut destination
                )
                .is_err()
        );
        assert_eq!(destination, before);
    }

    #[test]
    fn reference_candidate_corrupt_header_padding_and_trailing_bytes_fail() {
        let path = scratch_member(CandidateCodec::Ascii8);
        let mut bytes = fs::read(&path).expect("read scratch");
        bytes[12] = 1;
        fs::write(&path, &bytes).expect("write corrupt header");
        assert!(CandidateReader::open(&path).is_err());
        fs::remove_file(&path).expect("remove scratch");

        let path = scratch_member(CandidateCodec::Iupac4);
        let mut bytes = fs::read(&path).expect("read scratch");
        bytes.push(0);
        fs::write(&path, &bytes).expect("write trailing byte");
        assert!(CandidateReader::open(&path).is_err());
        fs::remove_file(&path).expect("remove scratch");
    }

    fn mutate(codec: CandidateCodec, change: impl FnOnce(&mut Vec<u8>)) -> PathBuf {
        let path = scratch_member(codec);
        let mut bytes = fs::read(&path).expect("read candidate");
        change(&mut bytes);
        fs::write(&path, bytes).expect("write mutation");
        path
    }

    #[test]
    fn reference_candidate_structural_corruptions_fail_closed() {
        for path in [
            mutate(CandidateCodec::Ascii8, |bytes| bytes[10] = 9),
            mutate(CandidateCodec::Ascii8, |bytes| bytes[64 + 48] = 2),
            mutate(CandidateCodec::Ascii8, |bytes| {
                bytes[80..88].copy_from_slice(&4097_u64.to_le_bytes())
            }),
            mutate(CandidateCodec::Ascii8, |bytes| {
                bytes.truncate(bytes.len() - 1)
            }),
        ] {
            assert!(CandidateReader::open(&path).is_err());
            fs::remove_file(path).expect("remove mutation");
        }

        let path = mutate(CandidateCodec::Ascii8, |bytes| bytes[200] = 1);
        let reader = CandidateReader::open(&path).expect("padding is exhaustive validation");
        assert!(reader.inspect_payload().is_err());
        fs::remove_file(path).expect("remove mutation");

        let path = mutate(CandidateCodec::Iupac4, |bytes| {
            bytes[4096] = (bytes[4096] & 0xf0) | 15
        });
        let reader = CandidateReader::open(&path).expect("open invalid payload");
        let mut destination = [0x5a; 1];
        assert!(
            reader
                .copy_window(
                    Grch38Contig::autosome(3).expect("chr3"),
                    1,
                    &mut destination
                )
                .is_err()
        );
        assert_eq!(destination, [0x5a; 1]);
        assert!(reader.inspect_payload().is_err());
        fs::remove_file(path).expect("remove mutation");

        let path = mutate(CandidateCodec::Iupac4, |bytes| bytes[4096 + 8195] &= 0x0f);
        let reader = CandidateReader::open(&path).expect("open invalid odd padding");
        assert!(reader.inspect_payload().is_err());
        fs::remove_file(path).expect("remove mutation");

        let path = mutate(CandidateCodec::Acgt2RleV1, |bytes| {
            bytes[4096 + 4097] |= 0xc0
        });
        let reader = CandidateReader::open(&path).expect("open invalid two-bit padding");
        assert!(reader.inspect_payload().is_err());
        fs::remove_file(path).expect("remove mutation");
    }

    #[test]
    fn reference_candidate_run_corruptions_fail_closed() {
        let mutations: [fn(&mut Vec<u8>); 3] = [
            |bytes| bytes[8204..8208].copy_from_slice(&0_u32.to_le_bytes()),
            |bytes| bytes[8216..8220].copy_from_slice(&4_u32.to_le_bytes()),
            |bytes| bytes[8224] = bytes[8208],
        ];
        for (index, change) in mutations.into_iter().enumerate() {
            let path = mutate(CandidateCodec::Acgt2RleV1, change);
            let reader = CandidateReader::open(&path).expect("open invalid runs");
            if index == 0 {
                let mut destination = [0x5a; 15];
                assert!(
                    reader
                        .copy_window(
                            Grch38Contig::autosome(3).expect("chr3"),
                            1,
                            &mut destination
                        )
                        .is_err()
                );
                assert_eq!(destination, [0x5a; 15]);
            }
            assert!(reader.inspect_payload().is_err());
            fs::remove_file(path).expect("remove mutation");
        }
    }

    #[test]
    fn reference_candidate_all_bounds_preserve_destination() {
        let reader =
            CandidateReader::open(&fixture_member(CandidateCodec::Ascii8)).expect("open fixture");
        let chr3 = Grch38Contig::autosome(3).expect("chr3");
        let mut empty = [];
        assert!(reader.copy_window(chr3, 1, &mut empty).is_err());
        let mut destination = [0x5a; 16];
        let expected = destination;
        assert!(reader.copy_window(chr3, 16_390, &mut destination).is_err());
        assert_eq!(destination, expected);
        assert!(
            reader
                .copy_window(chr3, u64::MAX - 2, &mut destination)
                .is_err()
        );
        assert_eq!(destination, expected);
    }

    #[test]
    fn reference_candidate_actual_reads_match_declared_trace() {
        reset_candidate_reads();
        let reader = CandidateReader::open(&fixture_member(CandidateCodec::Acgt2RleV1))
            .expect("open fixture");
        assert_eq!(candidate_read_ranges(), vec![(0, 64), (64, 96)]);
        reset_candidate_reads();
        let mut destination = [0_u8; 16];
        reader
            .copy_window(
                Grch38Contig::autosome(3).expect("chr3"),
                16_369,
                &mut destination,
            )
            .expect("copy audited window");
        let actual = candidate_read_pages();
        let declared = reader
            .trace_window(Grch38Contig::autosome(3).expect("chr3"), 16_369, 16)
            .expect("declared trace");
        assert_eq!(actual, declared);
    }

    #[test]
    fn reference_candidate_evaluator_covers_zstd_and_bad_candidate_sets() {
        let values = [
            candidate(CandidateCodec::Ascii8, 100, 100, 3, 7, 6),
            candidate(CandidateCodec::Iupac4, 100, 100, 3, 7, 5),
            candidate(CandidateCodec::Acgt2RleV1, 100, 100, 4, 6, 4),
        ];
        assert_eq!(
            select_candidate(&values),
            selected(CandidateCodec::Iupac4, SelectionReason::ZstdBytes)
        );
        assert_eq!(
            select_candidate(&values[..2]).reason,
            SelectionReason::InvalidEvidence
        );
        let duplicate = [values[0].clone(), values[0].clone(), values[2].clone()];
        assert_eq!(
            select_candidate(&duplicate).reason,
            SelectionReason::InvalidEvidence
        );
    }

    #[test]
    fn reference_candidate_zstd_settings_are_deterministic() {
        fn encode(bytes: &[u8]) -> Vec<u8> {
            let mut encoder = zstd::stream::Encoder::new(Vec::new(), 9).expect("encoder");
            encoder.include_checksum(true).expect("checksum");
            encoder.include_contentsize(true).expect("content size");
            encoder.include_dictid(false).expect("dictionary ID");
            encoder
                .long_distance_matching(false)
                .expect("long-distance matching");
            encoder.multithread(0).expect("workers");
            encoder
                .set_pledged_src_size(Some(bytes.len() as u64))
                .expect("pledged size");
            encoder.write_all(bytes).expect("encode");
            encoder.finish().expect("finish")
        }
        let input = b"pangopup deterministic reference candidate zstd control";
        assert_eq!(encode(input), encode(input));
    }

    #[test]
    fn reference_candidate_report_publication_rolls_back_sync_failure() {
        let rolled_back = std::cell::Cell::new(false);
        let result = publish_report_with(
            || Ok(()),
            || Err(CandidateError::Io(io::Error::other("injected sync"))),
            || {
                rolled_back.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(rolled_back.get());
    }
}
