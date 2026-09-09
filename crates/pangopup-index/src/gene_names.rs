//! Private, explicitly decoded PangoPup gene-name index format.
//!
//! `PGNAME01` answers one Ensembl accession from a sorted key array, a
//! parallel fixed-stride record array, and one contiguous string run per gene.
//! One accession costs one binary search, one record read and one string-run
//! read, so the cost of a name does not grow with the number of genes the
//! index holds.
//!
//! The byte layout is not a public compatibility promise. Integer fields are
//! little-endian and mapped bytes are never cast to Rust structs. One string
//! carries a one-byte length prefix, so it holds at most 255 bytes. The
//! longest symbol, previous symbol or alias symbol either source publishes is
//! 32 bytes.
//!
//! The index ships inside the executable. A build carries one gene-name
//! vintage, so naming needs no installed asset, no path resolution and no
//! authentication step. Naming must never enter a published identity:
//! `scoring_identity`, `data_set_version` and `runtime_profile_id` all stay
//! where they are when the index changes.

use crate::IndexError;
use memmap2::Mmap;
use serde::Serialize;
use std::{cmp::Ordering, collections::BTreeSet, fs, path::Path};

const MAGIC: &[u8; 8] = b"PGNAME01";
const VERSION: u32 = 1;
const HEADER_SIZE: u64 = 128;
const KEY_STRIDE: u64 = 8;
const RECORD_STRIDE: u64 = 24;
const PAGE_SIZE: u64 = 4096;

/// The record field value that reports no identifier. HGNC's largest numeric
/// identifier and NCBI's largest Gene identifier both sit far below it.
const NO_IDENTIFIER: u32 = u32::MAX;

const SOURCE_HGNC: u8 = 1;
const SOURCE_NCBI: u8 = 2;
const WITHHELD_NONE: u8 = 0;
const WITHHELD_CONFLICTING_RECORDS: u8 = 1;
const WITHHELD_PLACEHOLDER_SYMBOL: u8 = 2;

/// An Ensembl gene accession is `ENSG` followed by exactly eleven digits. The
/// key is the eleven digits read as a u64. A 32-bit key truncates
/// `ENSG05220017861` into the key of `ENSG00925050565` and answers the wrong
/// gene, so the key section carries u64 values at stride eight.
const ACCESSION_PREFIX: &str = "ENSG";
const ACCESSION_DIGITS: usize = 11;

/// The gene-name index the build carries. `include_bytes!` puts it in
/// read-only data, so the loader demand-pages it and one lookup addresses the
/// pages it reads and no others.
const SHIPPED_INDEX: &[u8] = include_bytes!("../../../assets/gene-names/gene-names.pgn");

/// Which source named a gene. HGNC is the naming authority. NCBI names only
/// the accessions HGNC does not reach.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GeneNameSource {
    Hgnc,
    Ncbi,
}

impl GeneNameSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hgnc => "hgnc",
            Self::Ncbi => "ncbi",
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Hgnc => SOURCE_HGNC,
            Self::Ncbi => SOURCE_NCBI,
        }
    }

    const fn decode(code: u8) -> Option<Self> {
        match code {
            SOURCE_HGNC => Some(Self::Hgnc),
            SOURCE_NCBI => Some(Self::Ncbi),
            _ => None,
        }
    }
}

/// The labels one named gene carries. `symbol` and `source` are always
/// present. Every other field is absent where the source that named the gene
/// supplies nothing for it. A gene NCBI named carries no HGNC identifier,
/// because HGNC publishes no join from that accession to one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneNames {
    pub symbol: String,
    pub source: GeneNameSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hgnc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ncbi_gene_id: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prev_symbols: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias_symbols: Vec<String>,
}

/// Why an accession the index carries still reports no name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnnamedReason {
    /// The source that reaches the accession carries more than one record for
    /// it. Picking one would be invisible to a consumer.
    ConflictingRecords,
    /// The symbol is a clone-derived string rather than a symbol. A
    /// clone-derived name such as `AC092143.1` carries the period of its
    /// accession version.
    PlaceholderSymbol,
}

impl UnnamedReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConflictingRecords => "conflicting_records",
            Self::PlaceholderSymbol => "placeholder_symbol",
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::ConflictingRecords => WITHHELD_CONFLICTING_RECORDS,
            Self::PlaceholderSymbol => WITHHELD_PLACEHOLDER_SYMBOL,
        }
    }

    const fn decode(code: u8) -> Option<Self> {
        match code {
            WITHHELD_CONFLICTING_RECORDS => Some(Self::ConflictingRecords),
            WITHHELD_PLACEHOLDER_SYMBOL => Some(Self::PlaceholderSymbol),
            _ => None,
        }
    }
}

/// What one Ensembl accession yields from the index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneNaming {
    Named(GeneNames),
    Unnamed(UnnamedReason),
}

/// What the index reaches. Every admitted accession is named by one source or
/// carried without a name.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GeneNameCounts {
    pub accessions: u64,
    pub named_by_hgnc: u64,
    pub named_by_ncbi: u64,
    pub unnamed: u64,
}

/// What one measured lookup addressed. The page count is the always-on
/// regression guard for the cost of a name. It counts from opening the index
/// through returning the answer, so a reader that decoded the whole index at
/// open and then answered from a map fails the ceiling, and a reader that
/// answered from a structure built beside the index fails the floor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LookupMetrics {
    pub logical_bytes_decoded: u64,
    pub unique_pages_addressed: u64,
}

/// One accession the builder admitted, in key order. `source` is the source
/// that reached the accession. A withheld record still records which source
/// held it back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneNameEntry {
    pub accession: String,
    pub source: GeneNameSource,
    pub naming: GeneNaming,
}

/// Open the gene-name index the build carries.
pub fn shipped() -> GeneNameIndex {
    GeneNameIndex::embedded(&mut None).expect("the embedded gene-name index is valid")
}

/// Resolve one accession against the shipped index and report every page the
/// whole path addressed, counted from open.
pub fn shipped_naming_measured(accession: &str) -> (Option<GeneNaming>, LookupMetrics) {
    let mut work = Work::default();
    let index = GeneNameIndex::embedded(&mut Some(&mut work))
        .expect("the embedded gene-name index is valid");
    let naming = index.naming_inner(accession, &mut Some(&mut work));
    (
        naming,
        LookupMetrics {
            logical_bytes_decoded: work.logical_bytes,
            unique_pages_addressed: work.pages.len() as u64,
        },
    )
}

/// Write one `PGNAME01` index. `entries` must be ordered by accession and
/// carry each accession once. Identical entries produce identical bytes: the
/// writer sorts nothing, pads nothing and records no timestamp.
pub fn write(path: &Path, entries: &[GeneNameEntry]) -> Result<GeneNameCounts, IndexError> {
    let mut counts = GeneNameCounts::default();
    let mut keys = Vec::with_capacity(entries.len() * KEY_STRIDE as usize);
    let mut records = Vec::with_capacity(entries.len() * RECORD_STRIDE as usize);
    let mut strings = Vec::new();
    let mut previous: Option<u64> = None;

    for entry in entries {
        let key = accession_key(&entry.accession).ok_or(IndexError::InvalidInput(
            "gene accession is not ENSG + 11 digits",
        ))?;
        if previous.is_some_and(|last| last >= key) {
            return Err(IndexError::InvalidInput(
                "gene accessions are not ascending",
            ));
        }
        previous = Some(key);
        keys.extend_from_slice(&key.to_le_bytes());

        let run_offset = u32::try_from(strings.len())
            .map_err(|_| IndexError::Arithmetic("gene-name string section offset"))?;
        let (source, withheld, hgnc, ncbi, prev_count, alias_count) = match &entry.naming {
            GeneNaming::Named(names) => {
                push_string(&mut strings, &names.symbol)?;
                for symbol in &names.prev_symbols {
                    push_string(&mut strings, symbol)?;
                }
                for symbol in &names.alias_symbols {
                    push_string(&mut strings, symbol)?;
                }
                match names.source {
                    GeneNameSource::Hgnc => counts.named_by_hgnc += 1,
                    GeneNameSource::Ncbi => counts.named_by_ncbi += 1,
                }
                (
                    names.source,
                    WITHHELD_NONE,
                    hgnc_numeric(names.hgnc_id.as_deref())?,
                    identifier(names.ncbi_gene_id)?,
                    count(names.prev_symbols.len())?,
                    count(names.alias_symbols.len())?,
                )
            }
            GeneNaming::Unnamed(reason) => {
                counts.unnamed += 1;
                (
                    entry.source,
                    reason.code(),
                    NO_IDENTIFIER,
                    NO_IDENTIFIER,
                    0,
                    0,
                )
            }
        };
        let run_len = u32::try_from(strings.len())
            .map_err(|_| IndexError::Arithmetic("gene-name string section offset"))?
            - run_offset;

        records.extend_from_slice(&run_offset.to_le_bytes());
        records.extend_from_slice(&run_len.to_le_bytes());
        records.extend_from_slice(&hgnc.to_le_bytes());
        records.extend_from_slice(&ncbi.to_le_bytes());
        records.extend_from_slice(&prev_count.to_le_bytes());
        records.extend_from_slice(&alias_count.to_le_bytes());
        records.push(source.code());
        records.push(withheld);
        records.extend_from_slice(&0_u16.to_le_bytes());
        counts.accessions += 1;
    }

    let key_offset = HEADER_SIZE;
    let record_offset = key_offset + keys.len() as u64;
    let string_offset = record_offset + records.len() as u64;
    let file_len = string_offset + strings.len() as u64;

    let mut header = Vec::with_capacity(HEADER_SIZE as usize);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    header.extend_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    header.extend_from_slice(&file_len.to_le_bytes());
    header.extend_from_slice(&key_offset.to_le_bytes());
    header.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    header.extend_from_slice(&record_offset.to_le_bytes());
    header.extend_from_slice(&(records.len() as u64).to_le_bytes());
    header.extend_from_slice(&string_offset.to_le_bytes());
    header.extend_from_slice(&(strings.len() as u64).to_le_bytes());
    header.extend_from_slice(&counts.accessions.to_le_bytes());
    header.extend_from_slice(&counts.named_by_hgnc.to_le_bytes());
    header.extend_from_slice(&counts.named_by_ncbi.to_le_bytes());
    header.extend_from_slice(&counts.unnamed.to_le_bytes());
    header.resize(HEADER_SIZE as usize, 0);

    let mut file = header;
    file.extend_from_slice(&keys);
    file.extend_from_slice(&records);
    file.extend_from_slice(&strings);
    fs::write(path, &file)?;
    Ok(counts)
}

/// Validated reader for the private gene-name index format.
///
/// The reader answers from the index bytes on every lookup. It builds no map
/// beside them: a map would move the cost of a name from the lookup to the
/// open, where the same bytes are still read and the page budget still counts
/// them.
pub struct GeneNameIndex {
    bytes: Bytes,
    header: Header,
}

impl GeneNameIndex {
    /// Map a gene-name index file and validate its cheap structural metadata.
    ///
    /// # Safety contract
    ///
    /// The sole unsafe operation is `Mmap::map`. The file must be an immutable
    /// inode, never truncated or modified while mapped. Every subsequent byte
    /// access is bounds-checked and explicitly decoded.
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        let file = fs::File::open(path)?;
        // SAFETY: the caller supplies an immutable, unmodified index file. No
        // mapped byte is interpreted until the structural checks below pass,
        // and no native struct cast is performed.
        let map = unsafe { Mmap::map(&file) }.map_err(IndexError::Io)?;
        Self::admit(Bytes::Mapped(map), &mut None)
    }

    fn embedded(work: &mut Option<&mut Work>) -> Result<Self, IndexError> {
        Self::admit(Bytes::Embedded(SHIPPED_INDEX), work)
    }

    fn admit(bytes: Bytes, work: &mut Option<&mut Work>) -> Result<Self, IndexError> {
        let header = decode_header(bytes.as_slice(), work)?;
        validate_structure(bytes.as_slice(), &header)?;
        Ok(Self { bytes, header })
    }

    /// What the index reaches.
    pub const fn counts(&self) -> GeneNameCounts {
        self.header.counts
    }

    /// What one stable Ensembl accession yields. An accession neither source
    /// reaches is not in the index and reports nothing.
    pub fn naming(&self, accession: &str) -> Option<GeneNaming> {
        self.naming_inner(accession, &mut None)
    }

    /// The labels for one stable Ensembl accession. An accession the index
    /// carries without a name reports nothing, so the whole object is absent
    /// rather than empty.
    pub fn names(&self, accession: &str) -> Option<GeneNames> {
        match self.naming_inner(accession, &mut None) {
            Some(GeneNaming::Named(names)) => Some(names),
            Some(GeneNaming::Unnamed(_)) | None => None,
        }
    }

    fn naming_inner(&self, accession: &str, work: &mut Option<&mut Work>) -> Option<GeneNaming> {
        let key = accession_key(accession)?;
        let ordinal = self.locate(key, work)?;
        let record = self.record(ordinal, work)?;
        if let Some(reason) = UnnamedReason::decode(record.withheld) {
            return Some(GeneNaming::Unnamed(reason));
        }
        if record.withheld != WITHHELD_NONE {
            return None;
        }
        let mut run = self.string_run(&record, work)?;
        let symbol = take_string(&mut run)?;
        let prev_symbols = take_strings(&mut run, record.prev_count)?;
        let alias_symbols = take_strings(&mut run, record.alias_count)?;
        if !run.is_empty() {
            return None;
        }
        Some(GeneNaming::Named(GeneNames {
            symbol,
            source: record.source,
            hgnc_id: record.hgnc.map(|value| format!("HGNC:{value}")),
            ncbi_gene_id: record.ncbi,
            prev_symbols,
            alias_symbols,
        }))
    }

    /// The ordinal of one key, found by binary search over the fixed-stride
    /// key array. Nothing scans and nothing is decoded into a map.
    fn locate(&self, key: u64, work: &mut Option<&mut Work>) -> Option<u64> {
        let bytes = self.bytes.as_slice();
        let mut low = 0_u64;
        let mut high = self.header.counts.accessions;
        while low < high {
            let middle = low + (high - low) / 2;
            let candidate = read_u64(bytes, self.header.key_offset + middle * KEY_STRIDE, work)?;
            match candidate.cmp(&key) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Some(middle),
            }
        }
        None
    }

    fn record(&self, ordinal: u64, work: &mut Option<&mut Work>) -> Option<RecordView> {
        let bytes = self.bytes.as_slice();
        let base = self.header.record_offset + ordinal * RECORD_STRIDE;
        touch(work, base, RECORD_STRIDE);
        let field = |offset: u64| read_field(bytes, base + offset);
        Some(RecordView {
            run_offset: u64::from(read_u32_at(bytes, base)?),
            run_len: u64::from(read_u32_at(bytes, base + 4)?),
            hgnc: identifier_value(read_u32_at(bytes, base + 8)?),
            ncbi: identifier_value(read_u32_at(bytes, base + 12)?),
            prev_count: read_u16_at(bytes, base + 16)?,
            alias_count: read_u16_at(bytes, base + 18)?,
            source: GeneNameSource::decode(field(20)?)?,
            withheld: field(21)?,
        })
    }

    fn string_run<'a>(
        &'a self,
        record: &RecordView,
        work: &mut Option<&mut Work>,
    ) -> Option<&'a [u8]> {
        if record.run_offset.checked_add(record.run_len)? > self.header.string_len {
            return None;
        }
        let start = self.header.string_offset.checked_add(record.run_offset)?;
        touch(work, start, record.run_len);
        let start = usize::try_from(start).ok()?;
        let len = usize::try_from(record.run_len).ok()?;
        self.bytes.as_slice().get(start..start.checked_add(len)?)
    }

    /// Prove the writer's canonical section, record and string-run layout.
    ///
    /// Runtime open validates only what one lookup needs, so it stays cheap
    /// and page-bounded. This stricter method reads every record and is the
    /// offline determinism proof.
    pub fn verify_canonical_structure(&self) -> Result<(), IndexError> {
        let bytes = self.bytes.as_slice();
        let count = self.header.counts.accessions;
        let sections = [
            (self.header.key_offset, self.header.key_len),
            (self.header.record_offset, self.header.record_len),
            (self.header.string_offset, self.header.string_len),
        ];
        let mut previous_end = HEADER_SIZE;
        for (offset, length) in sections {
            if offset != previous_end {
                return Err(IndexError::Corrupt("noncanonical section padding"));
            }
            previous_end = offset
                .checked_add(length)
                .ok_or(IndexError::Arithmetic("canonical section end"))?;
        }
        if previous_end != self.header.file_len {
            return Err(IndexError::Corrupt("noncanonical trailing bytes"));
        }

        let mut previous_key: Option<u64> = None;
        let mut previous_run_end = 0_u64;
        let mut observed = GeneNameCounts::default();
        for ordinal in 0..count {
            let key = read_u64(
                bytes,
                self.header.key_offset + ordinal * KEY_STRIDE,
                &mut None,
            )
            .ok_or(IndexError::Corrupt("truncated key section"))?;
            if previous_key.is_some_and(|last| last >= key) {
                return Err(IndexError::Corrupt("noncanonical key order"));
            }
            previous_key = Some(key);

            let base = self.header.record_offset + ordinal * RECORD_STRIDE;
            if read_u16_at(bytes, base + 22) != Some(0) {
                return Err(IndexError::Corrupt("record reserved bytes"));
            }
            let record = self
                .record(ordinal, &mut None)
                .ok_or(IndexError::Corrupt("undecodable record"))?;
            if record.run_offset != previous_run_end {
                return Err(IndexError::Corrupt("noncanonical string-run padding"));
            }
            previous_run_end = record
                .run_offset
                .checked_add(record.run_len)
                .ok_or(IndexError::Arithmetic("canonical string-run end"))?;

            let mut run = self
                .string_run(&record, &mut None)
                .ok_or(IndexError::Corrupt("string run outside the string section"))?;
            match UnnamedReason::decode(record.withheld) {
                Some(_) => {
                    observed.unnamed += 1;
                    if !run.is_empty()
                        || record.prev_count != 0
                        || record.alias_count != 0
                        || record.hgnc.is_some()
                        || record.ncbi.is_some()
                    {
                        return Err(IndexError::Corrupt("withheld record carries a name"));
                    }
                }
                None if record.withheld == WITHHELD_NONE => {
                    match record.source {
                        GeneNameSource::Hgnc => observed.named_by_hgnc += 1,
                        GeneNameSource::Ncbi => observed.named_by_ncbi += 1,
                    }
                    let expected = 1 + u64::from(record.prev_count) + u64::from(record.alias_count);
                    for _ in 0..expected {
                        take_string(&mut run).ok_or(IndexError::Corrupt("truncated string run"))?;
                    }
                    if !run.is_empty() {
                        return Err(IndexError::Corrupt("unclaimed string-run bytes"));
                    }
                }
                None => return Err(IndexError::Corrupt("unknown withheld reason")),
            }
            observed.accessions += 1;
        }
        if previous_run_end != self.header.string_len {
            return Err(IndexError::Corrupt("noncanonical unclaimed string bytes"));
        }
        if observed != self.header.counts {
            return Err(IndexError::Corrupt(
                "header counts disagree with the records",
            ));
        }
        Ok(())
    }
}

enum Bytes {
    Embedded(&'static [u8]),
    Mapped(Mmap),
}

impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Embedded(bytes) => bytes,
            Self::Mapped(map) => map,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Header {
    file_len: u64,
    key_offset: u64,
    key_len: u64,
    record_offset: u64,
    record_len: u64,
    string_offset: u64,
    string_len: u64,
    counts: GeneNameCounts,
}

#[derive(Clone, Copy, Debug)]
struct RecordView {
    run_offset: u64,
    run_len: u64,
    hgnc: Option<u32>,
    ncbi: Option<u32>,
    prev_count: u16,
    alias_count: u16,
    source: GeneNameSource,
    withheld: u8,
}

#[derive(Default)]
struct Work {
    logical_bytes: u64,
    pages: BTreeSet<u64>,
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

fn decode_header(bytes: &[u8], work: &mut Option<&mut Work>) -> Result<Header, IndexError> {
    touch(work, 0, HEADER_SIZE);
    if bytes.len() < HEADER_SIZE as usize {
        return Err(IndexError::Corrupt("truncated gene-name header"));
    }
    if bytes.get(0..8) != Some(MAGIC.as_slice()) {
        return Err(IndexError::Corrupt("wrong gene-name magic"));
    }
    if read_u32_at(bytes, 8) != Some(VERSION) {
        return Err(IndexError::Incompatible("gene-name index header version"));
    }
    if read_u32_at(bytes, 12) != Some(HEADER_SIZE as u32) {
        return Err(IndexError::Corrupt("wrong gene-name header size"));
    }
    if bytes[104..HEADER_SIZE as usize]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(IndexError::Corrupt("gene-name header reserved bytes"));
    }
    let field = |offset: u64| {
        read_u64(bytes, offset, &mut None).ok_or(IndexError::Corrupt("truncated gene-name header"))
    };
    Ok(Header {
        file_len: field(16)?,
        key_offset: field(24)?,
        key_len: field(32)?,
        record_offset: field(40)?,
        record_len: field(48)?,
        string_offset: field(56)?,
        string_len: field(64)?,
        counts: GeneNameCounts {
            accessions: field(72)?,
            named_by_hgnc: field(80)?,
            named_by_ncbi: field(88)?,
            unnamed: field(96)?,
        },
    })
}

/// Validate what one lookup depends on and nothing more. Every key and record
/// read is in bounds once the strides and the section extents agree, so the
/// lookup path never rereads this. A string run is checked where it is read.
fn validate_structure(bytes: &[u8], header: &Header) -> Result<(), IndexError> {
    if bytes.len() as u64 != header.file_len {
        return Err(IndexError::Corrupt("declared gene-name file length"));
    }
    let count = header.counts.accessions;
    if header.key_len != count.saturating_mul(KEY_STRIDE)
        || header.record_len != count.saturating_mul(RECORD_STRIDE)
    {
        return Err(IndexError::Corrupt("gene-name section stride"));
    }
    if header.counts.accessions
        != header
            .counts
            .named_by_hgnc
            .saturating_add(header.counts.named_by_ncbi)
            .saturating_add(header.counts.unnamed)
    {
        return Err(IndexError::Corrupt("gene-name header counts"));
    }
    let sections = [
        (header.key_offset, header.key_len),
        (header.record_offset, header.record_len),
        (header.string_offset, header.string_len),
    ];
    let mut previous_end = HEADER_SIZE;
    for (offset, length) in sections {
        if offset < previous_end {
            return Err(IndexError::Corrupt("overlapping gene-name sections"));
        }
        let end = offset
            .checked_add(length)
            .ok_or(IndexError::Arithmetic("gene-name section end"))?;
        if end > header.file_len {
            return Err(IndexError::Corrupt("gene-name section outside file"));
        }
        previous_end = end;
    }
    if previous_end != header.file_len {
        return Err(IndexError::Corrupt("trailing unsectioned gene-name bytes"));
    }
    Ok(())
}

/// The numeric part of an Ensembl gene accession. The index key is that value
/// and nothing else, so an accession of any other shape reaches no record.
fn accession_key(accession: &str) -> Option<u64> {
    let digits = accession.strip_prefix(ACCESSION_PREFIX)?;
    if digits.len() != ACCESSION_DIGITS || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn push_string(strings: &mut Vec<u8>, value: &str) -> Result<(), IndexError> {
    if value.is_empty() {
        return Err(IndexError::InvalidInput("gene-name string is empty"));
    }
    let length = u8::try_from(value.len())
        .map_err(|_| IndexError::InvalidInput("gene-name string exceeds its length prefix"))?;
    strings.push(length);
    strings.extend_from_slice(value.as_bytes());
    Ok(())
}

fn take_string(run: &mut &[u8]) -> Option<String> {
    let (length, rest) = run.split_first()?;
    let length = usize::from(*length);
    let (value, rest) = rest.split_at_checked(length)?;
    *run = rest;
    std::str::from_utf8(value).ok().map(ToOwned::to_owned)
}

fn take_strings(run: &mut &[u8], count: u16) -> Option<Vec<String>> {
    (0..count).map(|_| take_string(run)).collect()
}

fn count(value: usize) -> Result<u16, IndexError> {
    u16::try_from(value).map_err(|_| IndexError::InvalidInput("gene-name symbol count"))
}

fn identifier(value: Option<u32>) -> Result<u32, IndexError> {
    match value {
        None => Ok(NO_IDENTIFIER),
        Some(NO_IDENTIFIER) => Err(IndexError::InvalidInput("gene identifier is reserved")),
        Some(value) => Ok(value),
    }
}

const fn identifier_value(raw: u32) -> Option<u32> {
    if raw == NO_IDENTIFIER {
        None
    } else {
        Some(raw)
    }
}

fn hgnc_numeric(hgnc_id: Option<&str>) -> Result<u32, IndexError> {
    let Some(hgnc_id) = hgnc_id else {
        return Ok(NO_IDENTIFIER);
    };
    let numeric = hgnc_id
        .strip_prefix("HGNC:")
        .and_then(|digits| digits.parse().ok())
        .ok_or(IndexError::InvalidInput("HGNC identifier is not HGNC:<n>"))?;
    identifier(Some(numeric))
}

fn read_field(bytes: &[u8], offset: u64) -> Option<u8> {
    bytes.get(usize::try_from(offset).ok()?).copied()
}

fn read_u16_at(bytes: &[u8], offset: u64) -> Option<u16> {
    let start = usize::try_from(offset).ok()?;
    let raw: [u8; 2] = bytes.get(start..start.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(raw))
}

fn read_u32_at(bytes: &[u8], offset: u64) -> Option<u32> {
    let start = usize::try_from(offset).ok()?;
    let raw: [u8; 4] = bytes.get(start..start.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn read_u64(bytes: &[u8], offset: u64, work: &mut Option<&mut Work>) -> Option<u64> {
    touch(work, offset, 8);
    let start = usize::try_from(offset).ok()?;
    let raw: [u8; 8] = bytes.get(start..start.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw))
}
