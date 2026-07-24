//! Production `PGRREF01` GRCh38 reference bundle.
//!
//! The mmap container is deliberately incompatible with Ticket 010's
//! benchmark-only `PGRBEN01` files. Integer fields are decoded explicitly;
//! mapped bytes are never cast to Rust structs.

use memmap2::Mmap;
use pangopup_core::{
    GenomicPosition, Grch38Contig, ReferenceError, ReferenceProvenance, ReferenceProvider,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const REFERENCE_SCHEMA: &str = "pangopup.reference.bundle.v1";
pub const REFERENCE_FORMAT: &str = "pangopup.reference.acgt2-rle.v1";
pub const PRODUCTION_PROFILE: &str = "refseq-grch38p14-primary-v1";
pub const MINI_PROFILE: &str = "pangopup-reference-mini-v1";
pub const MAX_MANIFEST_BYTES: u64 = 65_536;
pub const MAX_NOTICE_BYTES: u64 = 16_384;
pub const MAX_MEMBER_BYTES: u64 = 773_124_288;
pub const MAX_AMBIGUITY_RUNS: u64 = 65_536;
pub const MAX_WINDOW_BYTES: usize = 1_048_576;
pub const PRODUCTION_NOTICE: &str = "Pangopup RefSeq GRCh38.p14 reference bundle\n\nNCBI RefSeq GRCh38.p14 sequence\nAssembly: GRCh38.p14 (GCF_000001405.40)\nSource: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz\nAssembly report: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt\nPolicy and acknowledgment/disclaimer: https://www.ncbi.nlm.nih.gov/home/about/policies/\n\nPangopup selected the 25 required assembled-molecule sequences (chr1–chr22, chrX, chrY, and non-nuclear chrM), renamed them to canonical chr aliases, uppercased exact IUPAC bases, and encoded them as pangopup.reference.acgt2-rle.v1. Pangopup does not claim a Creative Commons license for NCBI data.\n";
pub const MINI_NOTICE: &str = "Pangopup synthetic reference fixture\n\nThis pangopup-reference-mini-v1 bundle contains synthetic GPL-3.0-only fixture data created by Pangopup. It is not a biological reference and must not be used for biological interpretation.\nLicense: https://www.gnu.org/licenses/gpl-3.0.html\n";

const MAGIC: &[u8; 8] = b"PGRREF01";
const VERSION: u16 = 1;
const ENCODING: u8 = 1;
const HEADER_BYTES: usize = 64;
const DIRECTORY_ENTRY_BYTES: usize = 48;
const CONTIG_COUNT: usize = 25;
const DIRECTORY_BYTES: usize = CONTIG_COUNT * DIRECTORY_ENTRY_BYTES;
const DENSE_OFFSET: u64 = 4096;
const RUN_BYTES: u64 = 16;
const ACGT_ASCII: [[u8; 4]; 256] = acgt_ascii();

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

#[derive(Clone, Copy, Debug)]
struct DirectoryEntry {
    code: u8,
    bases: u64,
    dense_offset: u64,
    dense_length: u64,
    run_offset: u64,
    run_count: u64,
}

#[derive(Clone, Copy, Debug)]
struct AmbiguityRun {
    start: u32,
    length: u32,
    code: u8,
}

/// Long-lived cheap-open reference bundle. The mapped member must remain
/// immutable and untruncated for this value's lifetime.
pub struct ReferenceBundleOpen {
    mmap: Mmap,
    entries: [DirectoryEntry; CONTIG_COUNT],
    runs: Vec<AmbiguityRun>,
    provenance: ReferenceProvenance,
    manifest: ReferenceManifest,
}

impl ReferenceBundleOpen {
    pub fn open(bundle: &Path) -> Result<Self, ReferenceIndexError> {
        let mut names = Vec::with_capacity(3);
        for (count, entry) in fs::read_dir(bundle)?.enumerate() {
            if count >= 3 {
                return Err(ReferenceIndexError::Corrupt("bundle member set"));
            }
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err(ReferenceIndexError::Corrupt("bundle member type"));
            }
            names.push(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| ReferenceIndexError::Corrupt("bundle member name"))?,
            );
        }
        names.sort();
        if names != ["NOTICE", "manifest.json", "reference.pgr"] {
            return Err(ReferenceIndexError::Corrupt("bundle member set"));
        }
        let manifest_bytes = read_bounded(&bundle.join("manifest.json"), MAX_MANIFEST_BYTES)?;
        let notice = read_bounded(&bundle.join("NOTICE"), MAX_NOTICE_BYTES)?;
        let reference = File::open(bundle.join("reference.pgr"))?;
        Self::open_authenticated(&manifest_bytes, &notice, &reference)
    }

    /// Open a qualification reader from already-authenticated bytes and a
    /// held descriptor. This deliberately exists only with the read-audit
    /// feature used by the maintenance qualification harness; runtime callers
    /// continue to open a complete bundle by path.
    #[cfg(feature = "test-read-audit")]
    #[doc(hidden)]
    pub fn open_qualification(
        manifest_bytes: &[u8],
        notice_bytes: &[u8],
        reference: &File,
    ) -> Result<Self, ReferenceIndexError> {
        Self::open_authenticated(manifest_bytes, notice_bytes, reference)
    }

    /// Open a qualification reader and return the number of dense payload
    /// bytes inspected by the constructor. Query reads are deliberately not
    /// instrumented, so qualification measures the production copy path.
    #[cfg(feature = "test-read-audit")]
    #[doc(hidden)]
    pub fn open_qualification_audited(
        manifest_bytes: &[u8],
        notice_bytes: &[u8],
        reference: &File,
    ) -> Result<(Self, u64), ReferenceIndexError> {
        Self::open_authenticated_audited(manifest_bytes, notice_bytes, reference)
    }

    fn open_authenticated(
        manifest_bytes: &[u8],
        notice: &[u8],
        reference: &File,
    ) -> Result<Self, ReferenceIndexError> {
        Self::open_authenticated_scoped(manifest_bytes, notice, reference, false)
            .map(|(reader, _)| reader)
    }

    #[cfg(feature = "test-read-audit")]
    fn open_authenticated_audited(
        manifest_bytes: &[u8],
        notice: &[u8],
        reference: &File,
    ) -> Result<(Self, u64), ReferenceIndexError> {
        Self::open_authenticated_scoped(manifest_bytes, notice, reference, true)
    }

    fn open_authenticated_scoped(
        manifest_bytes: &[u8],
        notice: &[u8],
        reference: &File,
        audit_dense_reads: bool,
    ) -> Result<(Self, u64), ReferenceIndexError> {
        let manifest = parse_manifest(manifest_bytes)?;
        let bundle_id = reference_bundle_id(manifest_bytes);
        let reference_size = reference.metadata()?.len();
        validate_members(&manifest, notice, reference_size)?;
        let reference = reference.try_clone()?;
        let (mmap, entries, runs, dense_open_bytes) = open_member(reference, audit_dense_reads)?;
        validate_manifest_against_member(&manifest, &entries, runs.len() as u64)?;
        let provenance = ReferenceProvenance::new(
            bundle_id,
            manifest.profile.clone(),
            manifest.reference_format.clone(),
            manifest.source.assembly.clone(),
            manifest.source.assembly_accession.clone(),
            manifest.sequences.sequence_set_sha256.clone(),
        );
        Ok((
            Self {
                mmap,
                entries,
                runs,
                provenance,
                manifest,
            },
            dense_open_bytes,
        ))
    }

    pub fn manifest(&self) -> &ReferenceManifest {
        &self.manifest
    }

    pub fn resolve_alias(&self, alias: &str) -> Result<Grch38Contig, ReferenceIndexError> {
        if let Ok(contig) = alias.parse::<Grch38Contig>() {
            return Ok(contig);
        }
        self.manifest
            .sequences
            .aliases
            .iter()
            .position(|entry| entry.accession == alias)
            .and_then(|index| Grch38Contig::from_code((index + 1) as u8).ok())
            .ok_or(ReferenceIndexError::Bounds)
    }

    pub fn contig_length(&self, contig: Grch38Contig) -> u64 {
        self.entries[(contig.code() - 1) as usize].bases
    }

    /// Exhaustive payload checks used only by private build certification.
    pub fn inspect_payload(&self) -> Result<(), ReferenceIndexError> {
        for entry in &self.entries {
            if entry.bases % 4 != 0 {
                let last = *self
                    .range(entry.dense_offset + entry.dense_length - 1, 1)?
                    .first()
                    .ok_or(ReferenceIndexError::Corrupt("dense payload"))?;
                if last >> ((entry.bases % 4) * 2) != 0 {
                    return Err(ReferenceIndexError::Corrupt("dense final padding"));
                }
            }
            let ambiguity_offset =
                align8(self.entries[24].dense_offset + self.entries[24].dense_length)?;
            let run_start = if entry.run_count == 0 {
                0
            } else {
                usize::try_from((entry.run_offset - ambiguity_offset) / RUN_BYTES)
                    .map_err(|_| ReferenceIndexError::Corrupt("run range"))?
            };
            for run in &self.runs[run_start..run_start + entry.run_count as usize] {
                let run_end = u64::from(run.start) + u64::from(run.length);
                for position in u64::from(run.start)..run_end {
                    let packed = self.range(entry.dense_offset + position / 4, 1)?[0];
                    if (packed >> ((position % 4) * 2)) & 3 != 0 {
                        return Err(ReferenceIndexError::Corrupt("ambiguity placeholder"));
                    }
                }
            }
        }
        let ambiguity_offset =
            align8(self.entries[24].dense_offset + self.entries[24].dense_length)?;
        let dense_end = self.entries[24].dense_offset + self.entries[24].dense_length;
        if self.mmap[dense_end as usize..ambiguity_offset as usize]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ReferenceIndexError::Corrupt("dense alignment padding"));
        }
        Ok(())
    }

    pub fn trace_window_pages(
        &self,
        contig: Grch38Contig,
        start: GenomicPosition,
        length: usize,
    ) -> Result<Vec<u64>, ReferenceIndexError> {
        let entry = self.entries[(contig.code() - 1) as usize];
        let zero = u64::from(start.get() - 1);
        let end = zero
            .checked_add(length as u64)
            .ok_or(ReferenceIndexError::Bounds)?;
        if length == 0 || end > entry.bases {
            return Err(ReferenceIndexError::Bounds);
        }
        let first = entry.dense_offset + zero / 4;
        let last = entry.dense_offset + end.div_ceil(4);
        Ok((first / 4096..=(last.saturating_sub(1)) / 4096).collect())
    }

    fn range(&self, offset: u64, length: u64) -> Result<&[u8], ReferenceIndexError> {
        let end = offset
            .checked_add(length)
            .ok_or(ReferenceIndexError::Corrupt("member arithmetic"))?;
        let start =
            usize::try_from(offset).map_err(|_| ReferenceIndexError::Corrupt("member range"))?;
        let end = usize::try_from(end).map_err(|_| ReferenceIndexError::Corrupt("member range"))?;
        self.mmap
            .get(start..end)
            .ok_or(ReferenceIndexError::Corrupt("member range"))
    }

    fn copy_exact(
        &self,
        contig: Grch38Contig,
        start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError> {
        if destination.is_empty() {
            return Err(ReferenceError::EmptyWindow);
        }
        let entry = self.entries[(contig.code() - 1) as usize];
        let begin = u64::from(start.get() - 1);
        let end = begin
            .checked_add(destination.len() as u64)
            .ok_or(ReferenceError::OutOfBounds)?;
        if end > entry.bases {
            return Err(ReferenceError::OutOfBounds);
        }
        let first_packed = begin / 4;
        let packed = self
            .range(
                entry.dense_offset + first_packed,
                end.div_ceil(4) - first_packed,
            )
            .map_err(|_| ReferenceError::CorruptProviderData)?;
        decode_packed(packed, first_packed, begin, destination)?;
        let ambiguity_offset =
            align8(self.entries[24].dense_offset + self.entries[24].dense_length)
                .map_err(|_| ReferenceError::CorruptProviderData)?;
        let run_start = if entry.run_count == 0 {
            0
        } else {
            usize::try_from((entry.run_offset - ambiguity_offset) / RUN_BYTES)
                .map_err(|_| ReferenceError::CorruptProviderData)?
        };
        let run_end = run_start
            .checked_add(entry.run_count as usize)
            .ok_or(ReferenceError::CorruptProviderData)?;
        let relevant = &self.runs[run_start..run_end];
        let first =
            relevant.partition_point(|run| u64::from(run.start) + u64::from(run.length) <= begin);
        for run in &relevant[first..] {
            let run_begin = u64::from(run.start);
            if run_begin >= end {
                break;
            }
            let run_end = run_begin + u64::from(run.length);
            let overlap_begin = begin.max(run_begin);
            let overlap_end = end.min(run_end);
            if overlap_begin < overlap_end {
                let symbol = iupac_ascii(run.code).ok_or(ReferenceError::CorruptProviderData)?;
                destination[(overlap_begin - begin) as usize..(overlap_end - begin) as usize]
                    .fill(symbol);
            }
        }
        Ok(())
    }
}

fn decode_packed(
    packed: &[u8],
    first_packed: u64,
    begin: u64,
    destination: &mut [u8],
) -> Result<(), ReferenceError> {
    let end = begin
        .checked_add(destination.len() as u64)
        .ok_or(ReferenceError::CorruptProviderData)?;
    if first_packed != begin / 4 {
        return Err(ReferenceError::CorruptProviderData);
    }
    let required = usize::try_from(end.div_ceil(4) - first_packed)
        .map_err(|_| ReferenceError::CorruptProviderData)?;
    if packed.len() < required {
        return Err(ReferenceError::CorruptProviderData);
    }

    let mut genomic = begin;
    let mut output = 0_usize;
    while genomic < end && !genomic.is_multiple_of(4) {
        let byte = packed[((genomic / 4) - first_packed) as usize];
        destination[output] = ACGT_ASCII[byte as usize][(genomic % 4) as usize];
        genomic += 1;
        output += 1;
    }
    while genomic.checked_add(4).is_some_and(|next| next <= end) {
        let byte = packed[((genomic / 4) - first_packed) as usize];
        destination[output..output + 4].copy_from_slice(&ACGT_ASCII[byte as usize]);
        genomic += 4;
        output += 4;
    }
    while genomic < end {
        let byte = packed[((genomic / 4) - first_packed) as usize];
        destination[output] = ACGT_ASCII[byte as usize][(genomic % 4) as usize];
        genomic += 1;
        output += 1;
    }
    Ok(())
}

impl ReferenceProvider for ReferenceBundleOpen {
    fn copy_window(
        &self,
        contig: Grch38Contig,
        start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError> {
        self.copy_exact(contig, start, destination)
    }

    fn provenance(&self) -> &ReferenceProvenance {
        &self.provenance
    }
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

fn parse_manifest(bytes: &[u8]) -> Result<ReferenceManifest, ReferenceIndexError> {
    let manifest: ReferenceManifest =
        serde_json::from_slice(bytes).map_err(|_| ReferenceIndexError::Corrupt("manifest JSON"))?;
    if canonical_reference_manifest_bytes(&manifest)? != bytes {
        return Err(ReferenceIndexError::Corrupt("manifest canonical bytes"));
    }
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &ReferenceManifest) -> Result<(), ReferenceIndexError> {
    if manifest.schema != REFERENCE_SCHEMA {
        return Err(ReferenceIndexError::Incompatible("bundle schema"));
    }
    if manifest.reference_format != REFERENCE_FORMAT {
        return Err(ReferenceIndexError::Incompatible("reference format"));
    }
    if !matches!(manifest.profile.as_str(), PRODUCTION_PROFILE | MINI_PROFILE) {
        return Err(ReferenceIndexError::Incompatible("reference profile"));
    }
    if manifest.builder.version.is_empty()
        || !valid_sha(&manifest.builder.source_sha256)
        || !valid_sha(&manifest.source.fasta.sha256)
        || !valid_sha(&manifest.source.assembly_report.sha256)
        || !valid_sha(&manifest.sequences.sequence_set_sha256)
        || !valid_sha(&manifest.sequences.extra_accessions_sha256)
        || manifest.source.assembly_report.compression.is_some()
        || !matches!(
            manifest.source.fasta.compression.as_deref(),
            Some("none" | "gzip")
        )
        || manifest.sequences.aliases.len() != CONTIG_COUNT
        || manifest.sequences.total_bases == 0
        || manifest.sequences.ambiguity_runs > MAX_AMBIGUITY_RUNS
        || manifest.attribution.notice_path != "NOTICE"
        || !manifest.attribution.transformed
    {
        return Err(ReferenceIndexError::Corrupt("manifest fixed values"));
    }
    if manifest.members.len() != 2
        || manifest.members[0].path != "NOTICE"
        || manifest.members[0].media_type != "text/plain; charset=utf-8"
        || manifest.members[1].path != "reference.pgr"
        || manifest.members[1].media_type != "application/vnd.pangopup.reference-acgt2-rle"
        || manifest
            .members
            .iter()
            .any(|member| !valid_sha(&member.sha256))
    {
        return Err(ReferenceIndexError::Corrupt("manifest members"));
    }
    for (index, alias) in manifest.sequences.aliases.iter().enumerate() {
        let contig = Grch38Contig::from_code((index + 1) as u8)
            .map_err(|_| ReferenceIndexError::Corrupt("manifest alias"))?;
        if alias.contig != contig.to_string()
            || alias.accession != required_accession(contig)
            || alias.length == 0
        {
            return Err(ReferenceIndexError::Corrupt("manifest aliases"));
        }
    }
    let expected_lengths = if manifest.profile == PRODUCTION_PROFILE {
        production_contig_lengths()
    } else {
        [
            30, 16, 12, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 9,
        ]
    };
    if manifest
        .sequences
        .aliases
        .iter()
        .zip(expected_lengths)
        .any(|(alias, expected)| alias.length != expected)
    {
        return Err(ReferenceIndexError::Corrupt("profile sequence lengths"));
    }
    if manifest.profile == PRODUCTION_PROFILE
        && (manifest.source.assembly != "GRCh38.p14"
            || manifest.source.assembly_accession != "GCF_000001405.40"
            || manifest.source.fasta.url != production_fasta_url()
            || manifest.source.fasta.compression.as_deref() != Some("gzip")
            || manifest.source.fasta.bytes != 972_898_531
            || manifest.source.fasta.sha256
                != "sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"
            || manifest.source.assembly_report.url != production_report_url()
            || manifest.source.assembly_report.bytes != 80_454
            || manifest.source.assembly_report.sha256
                != "sha256:64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38"
            || manifest.sequences.total_bases != 3_088_286_401
            || manifest.sequences.sequence_set_sha256
                != "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4"
            || manifest.sequences.extra_record_count != 680
            || manifest.sequences.extra_accessions_sha256
                != "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb"
            || manifest.attribution.policy_url
                != "https://www.ncbi.nlm.nih.gov/home/about/policies/")
    {
        return Err(ReferenceIndexError::Corrupt("production profile"));
    }
    if manifest.profile == MINI_PROFILE
        && (manifest.source.assembly != "synthetic-mini"
            || manifest.source.assembly_accession != MINI_PROFILE
            || manifest.source.fasta.url != "urn:pangopup:fixture:reference-mini-v1:source"
            || manifest.source.assembly_report.url
                != "urn:pangopup:fixture:reference-mini-v1:assembly-report"
            || manifest.source.assembly_report.bytes != 2_082
            || manifest.source.assembly_report.sha256
                != "sha256:024b92cbc00745ce7cbb8facd0f708bfee60281cb157ba70e5f93963b7c9a51a"
            || !matches!(
                (
                    manifest.source.fasta.compression.as_deref(),
                    manifest.source.fasta.bytes,
                    manifest.source.fasta.sha256.as_str()
                ),
                (
                    Some("none"),
                    951,
                    "sha256:79d7b83500bf5afb6d6d152222012dc062e32c8f4adcd8074dc894d1b2d083c7"
                ) | (
                    Some("gzip"),
                    320,
                    "sha256:595ef8065180c0fc8f690f755bec4a8584635f52e4b5105fff1e158241af9e26"
                )
            )
            || manifest.sequences.total_bases != 159
            || manifest.sequences.sequence_set_sha256
                != "sha256:84f8c3a1ba478eaefdb0c7f7ece1bc2b52d5b1c44f283669adc44a740f56337c"
            || manifest.sequences.extra_record_count != 1
            || manifest.sequences.extra_accessions_sha256
                != "sha256:94503d1eade142e780f5db5c529275cffda81ace979371f0d54e1d2e80cf912f"
            || manifest.attribution.policy_url != "https://www.gnu.org/licenses/gpl-3.0.html")
    {
        return Err(ReferenceIndexError::Corrupt("mini profile"));
    }
    Ok(())
}

fn validate_members(
    manifest: &ReferenceManifest,
    notice: &[u8],
    reference_size: u64,
) -> Result<(), ReferenceIndexError> {
    let expected_notice = if manifest.profile == PRODUCTION_PROFILE {
        PRODUCTION_NOTICE.as_bytes()
    } else {
        MINI_NOTICE.as_bytes()
    };
    if notice != expected_notice
        || manifest.members[0].size != notice.len() as u64
        || manifest.members[0].sha256 != format!("sha256:{:x}", Sha256::digest(notice))
        || manifest.members[1].size != reference_size
        || reference_size > MAX_MEMBER_BYTES
    {
        return Err(ReferenceIndexError::Corrupt("member identity"));
    }
    Ok(())
}

struct OpenReadAudit {
    enabled: bool,
    dense_end: u64,
    dense_bytes: u64,
}

impl OpenReadAudit {
    fn range<'a>(
        &mut self,
        mmap: &'a [u8],
        offset: u64,
        length: u64,
        reason: &'static str,
    ) -> Result<&'a [u8], ReferenceIndexError> {
        let end = offset
            .checked_add(length)
            .ok_or(ReferenceIndexError::Corrupt(reason))?;
        if self.enabled {
            let overlap_start = offset.max(DENSE_OFFSET);
            let overlap_end = end.min(self.dense_end);
            if overlap_start < overlap_end {
                self.dense_bytes = self.dense_bytes.saturating_add(overlap_end - overlap_start);
            }
        }
        let start = usize::try_from(offset).map_err(|_| ReferenceIndexError::Corrupt(reason))?;
        let end = usize::try_from(end).map_err(|_| ReferenceIndexError::Corrupt(reason))?;
        mmap.get(start..end)
            .ok_or(ReferenceIndexError::Corrupt(reason))
    }
}

fn open_member(
    file: File,
    audit_dense_reads: bool,
) -> Result<(Mmap, [DirectoryEntry; CONTIG_COUNT], Vec<AmbiguityRun>, u64), ReferenceIndexError> {
    let length = file.metadata()?.len();
    if !(DENSE_OFFSET..=MAX_MEMBER_BYTES).contains(&length) {
        return Err(ReferenceIndexError::Corrupt("member length"));
    }
    // SAFETY: the read-only mapping and bounds-checked access remain owned by
    // ReferenceBundleOpen. The documented contract requires immutable,
    // untruncated member bytes for the reader lifetime.
    let mmap = unsafe { Mmap::map(&file)? };
    let header = mmap
        .get(..HEADER_BYTES)
        .ok_or(ReferenceIndexError::Corrupt("header"))?;
    if &header[0..8] != MAGIC
        || le_u16(header, 8)? != VERSION
        || header[10] != ENCODING
        || header[11] as usize != CONTIG_COUNT
        || header[12..16].iter().any(|byte| *byte != 0)
        || le_u64(header, 16)? != length
        || le_u64(header, 24)? != HEADER_BYTES as u64
        || le_u64(header, 32)? != DIRECTORY_BYTES as u64
        || le_u64(header, 40)? != DENSE_OFFSET
    {
        return Err(ReferenceIndexError::Corrupt("header fields"));
    }
    let ambiguity_offset = le_u64(header, 48)?;
    let total_runs = le_u64(header, 56)?;
    if ambiguity_offset % 8 != 0 || total_runs > MAX_AMBIGUITY_RUNS {
        return Err(ReferenceIndexError::Corrupt("ambiguity header"));
    }
    let mut audit = OpenReadAudit {
        enabled: audit_dense_reads,
        dense_end: ambiguity_offset,
        dense_bytes: 0,
    };
    let exact_length = ambiguity_offset
        .checked_add(
            total_runs
                .checked_mul(RUN_BYTES)
                .ok_or(ReferenceIndexError::Corrupt("ambiguity arithmetic"))?,
        )
        .ok_or(ReferenceIndexError::Corrupt("ambiguity arithmetic"))?;
    if exact_length != length {
        return Err(ReferenceIndexError::Corrupt("member closure"));
    }
    let mut entries = [DirectoryEntry {
        code: 0,
        bases: 0,
        dense_offset: 0,
        dense_length: 0,
        run_offset: 0,
        run_count: 0,
    }; CONTIG_COUNT];
    let mut expected_dense = DENSE_OFFSET;
    let mut expected_run_index = 0_u64;
    for (index, entry) in entries.iter_mut().enumerate() {
        let start = HEADER_BYTES + index * DIRECTORY_ENTRY_BYTES;
        let bytes = audit.range(
            &mmap,
            start as u64,
            DIRECTORY_ENTRY_BYTES as u64,
            "directory",
        )?;
        if bytes[0] != (index + 1) as u8 || bytes[1..8].iter().any(|byte| *byte != 0) {
            return Err(ReferenceIndexError::Corrupt("directory order"));
        }
        *entry = DirectoryEntry {
            code: bytes[0],
            bases: le_u64(bytes, 8)?,
            dense_offset: le_u64(bytes, 16)?,
            dense_length: le_u64(bytes, 24)?,
            run_offset: le_u64(bytes, 32)?,
            run_count: le_u64(bytes, 40)?,
        };
        if entry.bases == 0
            || entry.bases > u32::MAX as u64
            || entry.dense_offset != expected_dense
            || entry.dense_length != entry.bases.div_ceil(4)
        {
            return Err(ReferenceIndexError::Corrupt("directory dense closure"));
        }
        expected_dense = entry
            .dense_offset
            .checked_add(entry.dense_length)
            .ok_or(ReferenceIndexError::Corrupt("directory arithmetic"))?;
        if entry.run_count == 0 {
            if entry.run_offset != 0 {
                return Err(ReferenceIndexError::Corrupt("empty run offset"));
            }
        } else if entry.run_offset != ambiguity_offset + expected_run_index * RUN_BYTES {
            return Err(ReferenceIndexError::Corrupt("run directory closure"));
        }
        expected_run_index = expected_run_index
            .checked_add(entry.run_count)
            .ok_or(ReferenceIndexError::Corrupt("run count"))?;
    }
    if align8(expected_dense)? != ambiguity_offset || expected_run_index != total_runs {
        return Err(ReferenceIndexError::Corrupt("section closure"));
    }
    if audit
        .range(
            &mmap,
            (HEADER_BYTES + DIRECTORY_BYTES) as u64,
            DENSE_OFFSET - (HEADER_BYTES + DIRECTORY_BYTES) as u64,
            "header padding",
        )?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ReferenceIndexError::Corrupt("header padding"));
    }
    let mut runs = Vec::with_capacity(total_runs as usize);
    for entry in &entries {
        let mut prior: Option<AmbiguityRun> = None;
        for local in 0..entry.run_count {
            let offset = ambiguity_offset + (runs.len() as u64) * RUN_BYTES;
            let bytes = audit.range(&mmap, offset, RUN_BYTES, "ambiguity table")?;
            if bytes[9..16].iter().any(|byte| *byte != 0) {
                return Err(ReferenceIndexError::Corrupt("ambiguity reserved bytes"));
            }
            let run = AmbiguityRun {
                start: le_u32(bytes, 0)?,
                length: le_u32(bytes, 4)?,
                code: bytes[8],
            };
            let end = u64::from(run.start) + u64::from(run.length);
            if run.length == 0 || iupac_ascii(run.code).is_none() || end > entry.bases {
                return Err(ReferenceIndexError::Corrupt("ambiguity run"));
            }
            if let Some(previous) = prior {
                let previous_end = u64::from(previous.start) + u64::from(previous.length);
                if u64::from(run.start) < previous_end
                    || (u64::from(run.start) == previous_end && run.code == previous.code)
                {
                    return Err(ReferenceIndexError::Corrupt("ambiguity run order"));
                }
            }
            prior = Some(run);
            runs.push(run);
            let _ = local;
        }
    }
    Ok((mmap, entries, runs, audit.dense_bytes))
}

fn validate_manifest_against_member(
    manifest: &ReferenceManifest,
    entries: &[DirectoryEntry; CONTIG_COUNT],
    total_runs: u64,
) -> Result<(), ReferenceIndexError> {
    let mut total_bases = 0_u64;
    let mut manifest_runs = 0_u64;
    for ((alias, entry), expected_code) in manifest
        .sequences
        .aliases
        .iter()
        .zip(entries)
        .zip(1_u8..=25)
    {
        if entry.code != expected_code
            || alias.length != entry.bases
            || alias.ambiguity_runs != entry.run_count
        {
            return Err(ReferenceIndexError::Corrupt("manifest/member aliases"));
        }
        total_bases = total_bases
            .checked_add(entry.bases)
            .ok_or(ReferenceIndexError::Corrupt("base count"))?;
        manifest_runs = manifest_runs
            .checked_add(alias.ambiguity_runs)
            .ok_or(ReferenceIndexError::Corrupt("run count"))?;
    }
    if total_bases != manifest.sequences.total_bases
        || manifest_runs != manifest.sequences.ambiguity_runs
        || manifest_runs != total_runs
    {
        return Err(ReferenceIndexError::Corrupt("manifest/member totals"));
    }
    Ok(())
}

/// Streaming writer used by the authenticated builder. Call each contig once;
/// input order may differ from contig-code order.
pub struct ReferenceMemberWriter {
    file: File,
    entries: [DirectoryEntry; CONTIG_COUNT],
    seen: [bool; CONTIG_COUNT],
    run_scratch: File,
    run_scratch_path: PathBuf,
    run_counts: [u64; CONTIG_COUNT],
    total_runs: u64,
    active: Option<ActiveWrite>,
}

struct ActiveWrite {
    code: u8,
    expected: u64,
    position: u64,
    packed: u8,
    in_byte: u8,
    current_run: Option<AmbiguityRun>,
}

impl ReferenceMemberWriter {
    pub fn create(
        path: &Path,
        plans: &[ReferenceContigPlan; CONTIG_COUNT],
    ) -> Result<Self, ReferenceIndexError> {
        let mut expected_dense = DENSE_OFFSET;
        let mut entries = [DirectoryEntry {
            code: 0,
            bases: 0,
            dense_offset: 0,
            dense_length: 0,
            run_offset: 0,
            run_count: 0,
        }; CONTIG_COUNT];
        for (index, (entry, plan)) in entries.iter_mut().zip(plans).enumerate() {
            if plan.contig.code() != (index + 1) as u8
                || plan.bases == 0
                || plan.bases > u32::MAX as u64
            {
                return Err(ReferenceIndexError::Corrupt("writer plan"));
            }
            let dense_length = plan.bases.div_ceil(4);
            *entry = DirectoryEntry {
                code: plan.contig.code(),
                bases: plan.bases,
                dense_offset: expected_dense,
                dense_length,
                run_offset: 0,
                run_count: 0,
            };
            expected_dense = expected_dense
                .checked_add(dense_length)
                .ok_or(ReferenceIndexError::Corrupt("writer arithmetic"))?;
        }
        let ambiguity_offset = align8(expected_dense)?;
        if ambiguity_offset > MAX_MEMBER_BYTES {
            return Err(ReferenceIndexError::Corrupt("writer member size"));
        }
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.set_len(ambiguity_offset)?;
        let run_scratch_path = path.with_extension("ambiguity.scratch");
        let run_scratch = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&run_scratch_path)?;
        Ok(Self {
            file,
            entries,
            seen: [false; CONTIG_COUNT],
            run_scratch,
            run_scratch_path,
            run_counts: [0; CONTIG_COUNT],
            total_runs: 0,
            active: None,
        })
    }

    pub fn write_contig(
        &mut self,
        contig: Grch38Contig,
        sequence: &[u8],
    ) -> Result<(), ReferenceIndexError> {
        self.begin_contig(contig)?;
        self.write_bases(sequence)?;
        self.finish_contig()
    }

    pub fn begin_contig(&mut self, contig: Grch38Contig) -> Result<(), ReferenceIndexError> {
        let index = (contig.code() - 1) as usize;
        let entry = self.entries[index];
        if self.active.is_some() || self.seen[index] {
            return Err(ReferenceIndexError::Corrupt("writer contig state"));
        }
        self.file.seek(SeekFrom::Start(entry.dense_offset))?;
        self.active = Some(ActiveWrite {
            code: contig.code(),
            expected: entry.bases,
            position: 0,
            packed: 0,
            in_byte: 0,
            current_run: None,
        });
        Ok(())
    }

    pub fn write_bases(&mut self, sequence: &[u8]) -> Result<(), ReferenceIndexError> {
        let mut active = self
            .active
            .take()
            .ok_or(ReferenceIndexError::Corrupt("writer contig state"))?;
        for &original in sequence {
            if active.position >= active.expected {
                return Err(ReferenceIndexError::Corrupt("writer sequence length"));
            }
            let byte = original.to_ascii_uppercase();
            let code = iupac_code(byte).ok_or(ReferenceIndexError::Corrupt("writer IUPAC"))?;
            let dense = if code < 4 { code } else { 0 };
            active.packed |= dense << (active.in_byte * 2);
            active.in_byte += 1;
            if active.in_byte == 4 {
                self.file.write_all(&[active.packed])?;
                active.packed = 0;
                active.in_byte = 0;
            }
            if code >= 4 {
                let position = u32::try_from(active.position)
                    .map_err(|_| ReferenceIndexError::Corrupt("writer position"))?;
                match active.current_run.as_mut() {
                    Some(run) if run.code == code && run.start + run.length == position => {
                        run.length = run
                            .length
                            .checked_add(1)
                            .ok_or(ReferenceIndexError::Corrupt("writer run"))?;
                    }
                    Some(run) => {
                        self.append_run(active.code, *run)?;
                        *run = AmbiguityRun {
                            start: position,
                            length: 1,
                            code,
                        };
                    }
                    None => {
                        active.current_run = Some(AmbiguityRun {
                            start: position,
                            length: 1,
                            code,
                        })
                    }
                }
            } else if let Some(run) = active.current_run.take() {
                self.append_run(active.code, run)?;
            }
            active.position += 1;
        }
        self.active = Some(active);
        Ok(())
    }

    pub fn finish_contig(&mut self) -> Result<(), ReferenceIndexError> {
        let mut active = self
            .active
            .take()
            .ok_or(ReferenceIndexError::Corrupt("writer contig state"))?;
        if active.position != active.expected {
            return Err(ReferenceIndexError::Corrupt("writer sequence length"));
        }
        if active.in_byte != 0 {
            self.file.write_all(&[active.packed])?;
        }
        if let Some(run) = active.current_run.take() {
            self.append_run(active.code, run)?;
        }
        self.seen[(active.code - 1) as usize] = true;
        Ok(())
    }

    fn append_run(
        &mut self,
        contig_code: u8,
        run: AmbiguityRun,
    ) -> Result<(), ReferenceIndexError> {
        if self.total_runs >= MAX_AMBIGUITY_RUNS {
            return Err(ReferenceIndexError::Corrupt("writer run limit"));
        }
        let mut bytes = [0_u8; RUN_BYTES as usize];
        bytes[0] = contig_code;
        bytes[1..5].copy_from_slice(&run.start.to_le_bytes());
        bytes[5..9].copy_from_slice(&run.length.to_le_bytes());
        bytes[9] = run.code;
        self.run_scratch.write_all(&bytes)?;
        self.run_counts[(contig_code - 1) as usize] += 1;
        self.total_runs += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<[u64; CONTIG_COUNT], ReferenceIndexError> {
        if self.active.is_some() || self.seen.iter().any(|seen| !seen) {
            return Err(ReferenceIndexError::Corrupt("writer missing contig"));
        }
        let dense_end = self.entries[24].dense_offset + self.entries[24].dense_length;
        let ambiguity_offset = align8(dense_end)?;
        self.file.seek(SeekFrom::Start(ambiguity_offset))?;
        self.run_scratch.flush()?;
        let mut scratch_record = [0_u8; RUN_BYTES as usize];
        for wanted_code in 1_u8..=25 {
            self.run_scratch.seek(SeekFrom::Start(0))?;
            for _ in 0..self.total_runs {
                self.run_scratch.read_exact(&mut scratch_record)?;
                if scratch_record[0] == wanted_code {
                    self.file.write_all(&scratch_record[1..5])?;
                    self.file.write_all(&scratch_record[5..9])?;
                    self.file.write_all(&[scratch_record[9]])?;
                    self.file.write_all(&[0; 7])?;
                }
            }
        }
        let file_length = ambiguity_offset + self.total_runs * RUN_BYTES;
        if file_length > MAX_MEMBER_BYTES {
            return Err(ReferenceIndexError::Corrupt("writer member size"));
        }
        self.file.set_len(file_length)?;
        let mut prior = 0_u64;
        for (entry, count) in self.entries.iter_mut().zip(self.run_counts) {
            entry.run_count = count;
            entry.run_offset = if count == 0 {
                0
            } else {
                ambiguity_offset + prior * RUN_BYTES
            };
            prior += count;
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut header = [0_u8; HEADER_BYTES];
        header[0..8].copy_from_slice(MAGIC);
        header[8..10].copy_from_slice(&VERSION.to_le_bytes());
        header[10] = ENCODING;
        header[11] = CONTIG_COUNT as u8;
        header[16..24].copy_from_slice(&file_length.to_le_bytes());
        header[24..32].copy_from_slice(&(HEADER_BYTES as u64).to_le_bytes());
        header[32..40].copy_from_slice(&(DIRECTORY_BYTES as u64).to_le_bytes());
        header[40..48].copy_from_slice(&DENSE_OFFSET.to_le_bytes());
        header[48..56].copy_from_slice(&ambiguity_offset.to_le_bytes());
        header[56..64].copy_from_slice(&self.total_runs.to_le_bytes());
        self.file.write_all(&header)?;
        for entry in self.entries {
            let mut bytes = [0_u8; DIRECTORY_ENTRY_BYTES];
            bytes[0] = entry.code;
            bytes[8..16].copy_from_slice(&entry.bases.to_le_bytes());
            bytes[16..24].copy_from_slice(&entry.dense_offset.to_le_bytes());
            bytes[24..32].copy_from_slice(&entry.dense_length.to_le_bytes());
            bytes[32..40].copy_from_slice(&entry.run_offset.to_le_bytes());
            bytes[40..48].copy_from_slice(&entry.run_count.to_le_bytes());
            self.file.write_all(&bytes)?;
        }
        self.file.sync_all()?;
        fs::remove_file(&self.run_scratch_path)?;
        Ok(self.run_counts)
    }
}

impl Drop for ReferenceMemberWriter {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.run_scratch_path);
    }
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

fn align8(value: u64) -> Result<u64, ReferenceIndexError> {
    value
        .checked_add(7)
        .map(|value| value & !7)
        .ok_or(ReferenceIndexError::Corrupt("alignment"))
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ReferenceIndexError> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > maximum {
        return Err(ReferenceIndexError::Corrupt("bounded member size"));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != size {
        return Err(ReferenceIndexError::Corrupt("bounded member changed"));
    }
    Ok(bytes)
}

fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, ReferenceIndexError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(ReferenceIndexError::Corrupt("u16"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u16"))?,
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, ReferenceIndexError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(ReferenceIndexError::Corrupt("u32"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u32"))?,
    ))
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, ReferenceIndexError> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(ReferenceIndexError::Corrupt("u64"))?
            .try_into()
            .map_err(|_| ReferenceIndexError::Corrupt("u64"))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use memmap2::MmapOptions;

    fn decode_packed_scalar(packed: &[u8], first_packed: u64, begin: u64, destination: &mut [u8]) {
        for (offset, output) in destination.iter_mut().enumerate() {
            let position = begin + offset as u64;
            let byte = packed[(position / 4 - first_packed) as usize];
            *output = ACGT_ASCII[byte as usize][(position % 4) as usize];
        }
    }

    #[test]
    fn aligned_decoder_matches_scalar_for_every_bounded_start_and_length() {
        let packed: Vec<u8> = (0_u8..=u8::MAX).collect();
        let bases = packed.len() as u64 * 4;
        for begin in 0..bases {
            for length in 1..=33_u64.min(bases - begin) {
                let first = begin / 4;
                let end = begin + length;
                let bytes = &packed[first as usize..end.div_ceil(4) as usize];
                let mut expected = vec![0x55; length as usize];
                let mut actual = vec![0xaa; length as usize];
                decode_packed_scalar(bytes, first, begin, &mut expected);
                decode_packed(bytes, first, begin, &mut actual).expect("optimized decode");
                assert_eq!(actual, expected, "begin={begin} length={length}");
            }
        }
    }

    #[test]
    fn aligned_decoder_covers_each_start_and_short_group_tail_shape() {
        let packed = [0b11_10_01_00, 0b00_01_10_11, 0b10_00_11_01];
        for start_mod in 0..4_u64 {
            for length in [1_usize, 2, 3, 4, 5, 7, 8, 9] {
                let begin = start_mod;
                let end = begin + length as u64;
                let first = begin / 4;
                let bytes = &packed[first as usize..end.div_ceil(4) as usize];
                let mut expected = vec![0; length];
                let mut actual = vec![0; length];
                decode_packed_scalar(bytes, first, begin, &mut expected);
                decode_packed(bytes, first, begin, &mut actual).expect("shape decode");
                assert_eq!(actual, expected, "start_mod={start_mod} length={length}");
            }
        }
    }

    #[test]
    fn corrupt_decoder_input_preserves_destination() {
        for (packed, first, begin) in [(&[][..], 0, 0), (&[0][..], 1, 0)] {
            let mut destination = *b"sentinel";
            assert_eq!(
                decode_packed(packed, first, begin, &mut destination),
                Err(ReferenceError::CorruptProviderData)
            );
            assert_eq!(&destination, b"sentinel");
        }
        let mut destination = *b"sentinel";
        assert_eq!(
            decode_packed(&[0; 3], u64::MAX / 4, u64::MAX - 3, &mut destination),
            Err(ReferenceError::CorruptProviderData)
        );
        assert_eq!(&destination, b"sentinel");
    }

    #[test]
    fn constructor_read_audit_counts_only_dense_intersections() {
        let bytes = vec![0_u8; 8192];
        let mut audit = OpenReadAudit {
            enabled: true,
            dense_end: 6144,
            dense_bytes: 0,
        };
        audit
            .range(&bytes, 0, 4096, "fixture")
            .expect("structural range");
        audit
            .range(&bytes, 4094, 4, "fixture")
            .expect("left overlap");
        audit
            .range(&bytes, 6142, 4, "fixture")
            .expect("right overlap");
        audit
            .range(&bytes, 6144, 32, "fixture")
            .expect("ambiguity range");
        assert_eq!(audit.dense_bytes, 4);

        let mut disabled = OpenReadAudit {
            enabled: false,
            dense_end: 6144,
            dense_bytes: 0,
        };
        disabled
            .range(&bytes, 4096, 128, "fixture")
            .expect("ordinary open range");
        assert_eq!(disabled.dense_bytes, 0);
    }

    #[test]
    fn packed_decode_crosses_an_actual_4096_byte_mmap_page() {
        let path = std::env::temp_dir().join(format!(
            "pangopup-packed-page-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let mut file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create packed page fixture");
        file.set_len(8192).expect("size packed page fixture");
        file.seek(SeekFrom::Start(4095)).expect("seek page edge");
        file.write_all(&[0b11_10_01_00, 0b11_10_01_00])
            .expect("write page edge");
        file.sync_all().expect("sync page edge");
        // SAFETY: the file is held open and unchanged for the map lifetime.
        let map = unsafe { MmapOptions::new().map(&file) }.expect("map page fixture");
        let mut actual = [0_u8; 4];
        decode_packed(&map[4095..4097], 4095, 4095 * 4 + 3, &mut actual)
            .expect("decode across page");
        assert_eq!(&actual, b"TACG");
        drop(map);
        drop(file);
        fs::remove_file(path).expect("remove page fixture");
    }
}
