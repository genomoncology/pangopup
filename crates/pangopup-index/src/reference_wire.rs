//! Production `PGRREF01` GRCh38 reference bundle.
//!
//! The mmap container is deliberately incompatible with Ticket 010's
//! benchmark-only `PGRBEN01` files. Integer fields are decoded explicitly;
//! mapped bytes are never cast to Rust structs.

use pangopup_core::Grch38Contig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, io};
pub const REFERENCE_SCHEMA: &str = "pangopup.reference.bundle.v1";
pub const REFERENCE_FORMAT: &str = "pangopup.reference.acgt2-rle.v1";
pub const PRODUCTION_PROFILE: &str = "refseq-grch38p14-primary-v1";
pub const MINI_PROFILE: &str = "pangopup-reference-mini-v1";
pub const ROUTE_TEST_PROFILE: &str = "pangopup-reference-route-test-v1";
pub const MAX_MANIFEST_BYTES: u64 = 65_536;
pub const MAX_NOTICE_BYTES: u64 = 16_384;
pub const MAX_MEMBER_BYTES: u64 = 773_124_288;
pub const MAX_AMBIGUITY_RUNS: u64 = 65_536;
pub const MAX_WINDOW_BYTES: usize = 1_048_576;
pub const PRODUCTION_NOTICE: &str = "Pangopup RefSeq GRCh38.p14 reference bundle\n\nNCBI RefSeq GRCh38.p14 sequence\nAssembly: GRCh38.p14 (GCF_000001405.40)\nSource: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz\nAssembly report: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt\nPolicy and acknowledgment/disclaimer: https://www.ncbi.nlm.nih.gov/home/about/policies/\n\nPangopup selected the 25 required assembled-molecule sequences (chr1–chr22, chrX, chrY, and non-nuclear chrM), renamed them to canonical chr aliases, uppercased exact IUPAC bases, and encoded them as pangopup.reference.acgt2-rle.v1. Pangopup does not claim a Creative Commons license for NCBI data.\n";
pub const MINI_NOTICE: &str = "Pangopup synthetic reference fixture\n\nThis pangopup-reference-mini-v1 bundle contains synthetic GPL-3.0-only fixture data created by Pangopup. It is not a biological reference and must not be used for biological interpretation.\nLicense: https://www.gnu.org/licenses/gpl-3.0.html\n";
pub const ROUTE_TEST_NOTICE: &str = "Pangopup synthetic route reference fixture\n\nThis pangopup-reference-route-test-v1 bundle contains synthetic GPL-3.0-only fixture data created by Pangopup. It is not a biological reference and must not be used for biological interpretation.\nLicense: https://www.gnu.org/licenses/gpl-3.0.html\n";

pub(crate) const MAGIC: &[u8; 8] = b"PGRREF01";
pub(crate) const VERSION: u16 = 1;
pub(crate) const ENCODING: u8 = 1;
pub(crate) const HEADER_BYTES: usize = 64;
pub(crate) const DIRECTORY_ENTRY_BYTES: usize = 48;
pub(crate) const CONTIG_COUNT: usize = 25;
pub(crate) const DIRECTORY_BYTES: usize = CONTIG_COUNT * DIRECTORY_ENTRY_BYTES;
pub(crate) const DENSE_OFFSET: u64 = 4096;
pub(crate) const RUN_BYTES: u64 = 16;
pub(crate) const ACGT_ASCII: [[u8; 4]; 256] = acgt_ascii();

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceManifest {
    pub schema: String,
    pub reference_format: String,
    pub profile: String,
    pub builder: BuilderManifest,
    pub source: SourceManifest,
    pub sequences: SequenceManifest,
    pub members: Vec<MemberManifest>,
    pub attribution: AttributionManifest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderManifest {
    pub version: String,
    pub source_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifest {
    pub assembly: String,
    pub assembly_accession: String,
    pub fasta: InputManifest,
    pub assembly_report: InputManifest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputManifest {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceManifest {
    pub total_bases: u64,
    pub sequence_set_sha256: String,
    pub extra_record_count: u64,
    pub extra_accessions_sha256: String,
    pub ambiguity_runs: u64,
    pub aliases: Vec<ReferenceAliasManifest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceAliasManifest {
    pub contig: String,
    pub accession: String,
    pub length: u64,
    pub ambiguity_runs: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemberManifest {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionManifest {
    pub notice_path: String,
    pub policy_url: String,
    pub transformed: bool,
}

#[derive(Debug)]
pub enum ReferenceIndexError {
    Io(io::Error),
    Incompatible(&'static str),
    Corrupt(&'static str),
    Bounds,
}

impl fmt::Display for ReferenceIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "reference I/O failed: {error}"),
            Self::Incompatible(reason) => write!(formatter, "incompatible reference: {reason}"),
            Self::Corrupt(reason) => write!(formatter, "corrupt reference: {reason}"),
            Self::Bounds => formatter.write_str("reference window is out of bounds"),
        }
    }
}

impl std::error::Error for ReferenceIndexError {}

impl From<io::Error> for ReferenceIndexError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceContigPlan {
    pub contig: Grch38Contig,
    pub bases: u64,
}

pub fn canonical_reference_manifest_bytes(
    manifest: &ReferenceManifest,
) -> Result<Vec<u8>, ReferenceIndexError> {
    let bytes = serde_jcs::to_vec(manifest)
        .map_err(|_| ReferenceIndexError::Corrupt("manifest encoding"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ReferenceIndexError::Corrupt("manifest size"));
    }
    Ok(bytes)
}

pub fn reference_bundle_id(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectoryEntry {
    pub(crate) code: u8,
    pub(crate) bases: u64,
    pub(crate) dense_offset: u64,
    pub(crate) dense_length: u64,
    pub(crate) run_offset: u64,
    pub(crate) run_count: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AmbiguityRun {
    pub(crate) start: u32,
    pub(crate) length: u32,
    pub(crate) code: u8,
}

pub fn required_accession(contig: Grch38Contig) -> &'static str {
    const VALUES: [&str; 25] = [
        "NC_000001.11",
        "NC_000002.12",
        "NC_000003.12",
        "NC_000004.12",
        "NC_000005.10",
        "NC_000006.12",
        "NC_000007.14",
        "NC_000008.11",
        "NC_000009.12",
        "NC_000010.11",
        "NC_000011.10",
        "NC_000012.12",
        "NC_000013.11",
        "NC_000014.9",
        "NC_000015.10",
        "NC_000016.10",
        "NC_000017.11",
        "NC_000018.10",
        "NC_000019.10",
        "NC_000020.11",
        "NC_000021.9",
        "NC_000022.11",
        "NC_000023.11",
        "NC_000024.10",
        "NC_012920.1",
    ];
    VALUES[(contig.code() - 1) as usize]
}

pub fn production_contig_lengths() -> [u64; 25] {
    [
        248_956_422,
        242_193_529,
        198_295_559,
        190_214_555,
        181_538_259,
        170_805_979,
        159_345_973,
        145_138_636,
        138_394_717,
        133_797_422,
        135_086_622,
        133_275_309,
        114_364_328,
        107_043_718,
        101_991_189,
        90_338_345,
        83_257_441,
        80_373_285,
        58_617_616,
        64_444_167,
        46_709_983,
        50_818_468,
        156_040_895,
        57_227_415,
        16_569,
    ]
}

pub fn production_fasta_url() -> &'static str {
    "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz"
}

pub fn production_report_url() -> &'static str {
    "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt"
}

pub fn iupac_code(byte: u8) -> Option<u8> {
    match byte.to_ascii_uppercase() {
        b'A' => Some(0),
        b'C' => Some(1),
        b'G' => Some(2),
        b'T' => Some(3),
        b'R' => Some(4),
        b'Y' => Some(5),
        b'S' => Some(6),
        b'W' => Some(7),
        b'K' => Some(8),
        b'M' => Some(9),
        b'B' => Some(10),
        b'D' => Some(11),
        b'H' => Some(12),
        b'V' => Some(13),
        b'N' => Some(14),
        _ => None,
    }
}

pub fn iupac_ascii(code: u8) -> Option<u8> {
    b"ACGTRYSWKMBDHVN".get(code as usize).copied()
}

const fn acgt_ascii() -> [[u8; 4]; 256] {
    let mut table = [[0_u8; 4]; 256];
    let symbols = *b"ACGT";
    let mut byte = 0;
    while byte < 256 {
        table[byte][0] = symbols[byte & 3];
        table[byte][1] = symbols[(byte >> 2) & 3];
        table[byte][2] = symbols[(byte >> 4) & 3];
        table[byte][3] = symbols[(byte >> 6) & 3];
        byte += 1;
    }
    table
}

pub(crate) fn align8(value: u64) -> Result<u64, ReferenceIndexError> {
    value
        .checked_add(7)
        .map(|value| value & !7)
        .ok_or(ReferenceIndexError::Corrupt("alignment"))
}

pub(crate) fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(crate) fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, ReferenceIndexError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(ReferenceIndexError::Corrupt("u16"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u16"))?,
    ))
}

pub(crate) fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, ReferenceIndexError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(ReferenceIndexError::Corrupt("u32"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u32"))?,
    ))
}

pub(crate) fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, ReferenceIndexError> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(ReferenceIndexError::Corrupt("u64"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u64"))?,
    ))
}
