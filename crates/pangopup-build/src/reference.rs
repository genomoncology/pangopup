//! Authenticated production reference build and private certification.

use crate::CommandError;
use flate2::bufread::GzDecoder;
use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceProvider};
use pangopup_index::reference::{
    AttributionManifest, BuilderManifest, InputManifest, MINI_NOTICE, MINI_PROFILE, MemberManifest,
    PRODUCTION_NOTICE, PRODUCTION_PROFILE, REFERENCE_FORMAT, REFERENCE_SCHEMA,
    ReferenceAliasManifest, ReferenceBundleOpen, ReferenceContigPlan, ReferenceIndexError,
    ReferenceManifest, ReferenceMemberWriter, SequenceManifest, SourceManifest,
    canonical_reference_manifest_bytes, production_contig_lengths, production_fasta_url,
    production_report_url, reference_bundle_id, required_accession,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, ErrorKind, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MINI_FASTA_BYTES: u64 = 951;
const MINI_FASTA_SHA: &str = "79d7b83500bf5afb6d6d152222012dc062e32c8f4adcd8074dc894d1b2d083c7";
const MINI_GZIP_BYTES: u64 = 320;
const MINI_GZIP_SHA: &str = "595ef8065180c0fc8f690f755bec4a8584635f52e4b5105fff1e158241af9e26";
const MINI_REPORT_BYTES: u64 = 2_082;
const MINI_REPORT_SHA: &str = "024b92cbc00745ce7cbb8facd0f708bfee60281cb157ba70e5f93963b7c9a51a";
const MINI_SEQUENCE_SHA: &str = "84f8c3a1ba478eaefdb0c7f7ece1bc2b52d5b1c44f283669adc44a740f56337c";
const MINI_EXTRA_SHA: &str = "94503d1eade142e780f5db5c529275cffda81ace979371f0d54e1d2e80cf912f";
const COMPATIBILITY_CORPUS_BYTES: usize = 220_071;
const COMPATIBILITY_CORPUS_SHA: &str =
    "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const COMPATIBILITY_CORPUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/pangolin-compat-v1/cases.jsonl"
));

static STAGING_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceBuildOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub profile: String,
    pub bundle_id: String,
    pub members: Vec<ReferenceBuildMember>,
    pub certification: ReferenceCertification,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceBuildMember {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceCertification {
    pub total_bases: u64,
    pub sequence_set_sha256: String,
    pub contexts_verified: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceInspectOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub profile: String,
    pub format: String,
    pub bundle_id: String,
    pub sequences: u64,
    pub total_bases: u64,
    pub integrity: &'static str,
    pub member_sha256_checked: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceWindowOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contig: String,
    pub start: u32,
    pub length: usize,
    pub bases: String,
    pub provenance: ReferenceWindowProvenance,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceWindowProvenance {
    pub bundle_id: String,
    pub profile: String,
    pub format: String,
    pub assembly: String,
    pub assembly_accession: String,
    pub sequence_set_sha256: String,
}

pub const QUALIFICATION_MAX_P50_NS: u64 = 5_586;
pub const QUALIFICATION_MAX_P95_NS: u64 = 6_100;
pub const QUALIFICATION_MAX_OPEN_HEAP_BYTES: u64 = 2_097_152;
pub const QUALIFICATION_MAX_BUILDER_HEAP_BYTES: u64 = 16_777_216;
pub const QUALIFICATION_MAX_MEMBER_BYTES: u64 = 773_124_288;
const QUALIFICATION_TOTAL_BASES: u64 = 3_088_286_401;
const QUALIFICATION_SEQUENCE_SET_SHA256: &str =
    "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4";
const QUALIFICATION_EXTRA_RECORD_COUNT: u64 = 680;
const QUALIFICATION_EXTRA_ACCESSIONS_SHA256: &str =
    "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb";
const QUALIFICATION_CONTEXTS_VERIFIED: u64 = 14;
const QUALIFICATION_UNIQUE_DENSE_PAGES: [u64; 8] = [
    31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
];
const QUALIFICATION_PER_CASE_PAGE_COUNT_SUM: u64 = 20;

#[derive(Clone, Debug)]
pub struct ReferenceQualificationMeasurements<'a> {
    pub total_bases: u64,
    pub sequence_set_sha256: &'a str,
    pub extra_record_count: u64,
    pub extra_accessions_sha256: &'a str,
    pub contexts_verified: u64,
    pub headline_p50_ns: u64,
    pub headline_p95_ns: u64,
    pub allocation_calls: u64,
    pub allocation_bytes: u64,
    pub open_peak_heap_bytes: u64,
    pub builder_peak_heap_bytes: u64,
    pub dense_bytes_read_during_open: u64,
    pub member_bytes: u64,
    pub unique_dense_pages: &'a [u64],
    pub per_case_page_count_sum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceQualificationFailureClass {
    LogicalSequence,
    LogicalExtras,
    Latency,
    Allocations,
    Heap,
    Storage,
    Pages,
}

impl ReferenceQualificationFailureClass {
    fn wire_name(self) -> &'static str {
        match self {
            Self::LogicalSequence => "logical-sequence",
            Self::LogicalExtras => "logical-extras",
            Self::Latency => "latency",
            Self::Allocations => "allocations",
            Self::Heap => "heap",
            Self::Storage => "storage",
            Self::Pages => "pages",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceQualificationRejection {
    pub class: ReferenceQualificationFailureClass,
    pub message: String,
}

fn qualification_rejection(
    class: ReferenceQualificationFailureClass,
    message: String,
) -> ReferenceQualificationRejection {
    let message = if message.is_ascii() && message.len() <= 256 {
        message
    } else {
        let bytes = message.as_bytes();
        format!(
            "{} diagnostic_len={} diagnostic_sha256=sha256:{:x}",
            class.wire_name(),
            bytes.len(),
            Sha256::digest(bytes)
        )
    };
    ReferenceQualificationRejection { class, message }
}

fn observable_identity(value: &str) -> String {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        value.to_owned()
    } else {
        format!(
            "invalid(len={},sha256:{:x})",
            value.len(),
            Sha256::digest(value.as_bytes())
        )
    }
}

fn pages_message(observed: &ReferenceQualificationMeasurements<'_>) -> String {
    let literal = format!(
        "pages unique={:?}/{:?} per_case_sum={}/{}",
        observed.unique_dense_pages,
        QUALIFICATION_UNIQUE_DENSE_PAGES,
        observed.per_case_page_count_sum,
        QUALIFICATION_PER_CASE_PAGE_COUNT_SUM
    );
    if literal.len() <= 256 {
        return literal;
    }
    let mut hash = Sha256::new();
    for page in observed.unique_dense_pages {
        hash.update(page.to_be_bytes());
    }
    format!(
        "pages unique_len={} unique_u64be_sha256=sha256:{:x} limit={:?} per_case_sum={}/{}",
        observed.unique_dense_pages.len(),
        hash.finalize(),
        QUALIFICATION_UNIQUE_DENSE_PAGES,
        observed.per_case_page_count_sum,
        QUALIFICATION_PER_CASE_PAGE_COUNT_SUM
    )
}

pub fn evaluate_reference_qualification(
    observed: &ReferenceQualificationMeasurements<'_>,
) -> Result<(), ReferenceQualificationRejection> {
    if observed.total_bases != QUALIFICATION_TOTAL_BASES
        || observed.sequence_set_sha256 != QUALIFICATION_SEQUENCE_SET_SHA256
        || observed.contexts_verified != QUALIFICATION_CONTEXTS_VERIFIED
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::LogicalSequence,
            format!(
                "logical-sequence b={}/{} s={}/{} c={}/{}",
                observed.total_bases,
                QUALIFICATION_TOTAL_BASES,
                observable_identity(observed.sequence_set_sha256),
                QUALIFICATION_SEQUENCE_SET_SHA256,
                observed.contexts_verified,
                QUALIFICATION_CONTEXTS_VERIFIED
            ),
        ));
    }
    if observed.extra_record_count != QUALIFICATION_EXTRA_RECORD_COUNT
        || observed.extra_accessions_sha256 != QUALIFICATION_EXTRA_ACCESSIONS_SHA256
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::LogicalExtras,
            format!(
                "logical-extras n={}/{} s={}/{}",
                observed.extra_record_count,
                QUALIFICATION_EXTRA_RECORD_COUNT,
                observable_identity(observed.extra_accessions_sha256),
                QUALIFICATION_EXTRA_ACCESSIONS_SHA256
            ),
        ));
    }
    if observed.headline_p50_ns > QUALIFICATION_MAX_P50_NS
        || observed.headline_p95_ns > QUALIFICATION_MAX_P95_NS
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::Latency,
            format!(
                "latency p50_ns={}/{} p95_ns={}/{}",
                observed.headline_p50_ns,
                QUALIFICATION_MAX_P50_NS,
                observed.headline_p95_ns,
                QUALIFICATION_MAX_P95_NS
            ),
        ));
    }
    if observed.allocation_calls != 0 || observed.allocation_bytes != 0 {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::Allocations,
            format!(
                "allocations calls={}/0 bytes={}/0",
                observed.allocation_calls, observed.allocation_bytes
            ),
        ));
    }
    if observed.open_peak_heap_bytes > QUALIFICATION_MAX_OPEN_HEAP_BYTES
        || observed.builder_peak_heap_bytes > QUALIFICATION_MAX_BUILDER_HEAP_BYTES
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::Heap,
            format!(
                "heap open_bytes={}/{} builder_bytes={}/{}",
                observed.open_peak_heap_bytes,
                QUALIFICATION_MAX_OPEN_HEAP_BYTES,
                observed.builder_peak_heap_bytes,
                QUALIFICATION_MAX_BUILDER_HEAP_BYTES
            ),
        ));
    }
    if observed.dense_bytes_read_during_open != 0
        || observed.member_bytes > QUALIFICATION_MAX_MEMBER_BYTES
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::Storage,
            format!(
                "storage dense_open_bytes={}/0 member_bytes={}/{}",
                observed.dense_bytes_read_during_open,
                observed.member_bytes,
                QUALIFICATION_MAX_MEMBER_BYTES
            ),
        ));
    }
    if observed.unique_dense_pages != QUALIFICATION_UNIQUE_DENSE_PAGES
        || observed.per_case_page_count_sum != QUALIFICATION_PER_CASE_PAGE_COUNT_SUM
    {
        return Err(qualification_rejection(
            ReferenceQualificationFailureClass::Pages,
            pages_message(observed),
        ));
    }
    Ok(())
}

struct Profile {
    name: &'static str,
    assembly: &'static str,
    assembly_accession: &'static str,
    fasta_url: &'static str,
    report_url: &'static str,
    policy_url: &'static str,
    notice: &'static str,
    lengths: [u64; 25],
    total_bases: u64,
    sequence_sha: &'static str,
    extra_count: u64,
    extra_sha: &'static str,
    contexts: u64,
    max_decoded_bytes: u64,
    max_records: usize,
    max_accession_bytes: usize,
}

pub fn build_reference_bundle(
    profile_name: &str,
    source: &Path,
    assembly_report: &Path,
    output: &Path,
) -> Result<ReferenceBuildOutcome, CommandError> {
    let profile = profile(profile_name)?;
    ensure_absent(output)?;
    let (report_identity, report_bytes) =
        authenticate_regular(assembly_report, 128 * 1024, "REFERENCE_INPUT")?;
    validate_report_identity(&profile, &report_identity)?;
    parse_assembly_report(&profile, &report_bytes)?;

    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|_| CommandError::new("IO", "output parent creation failed"))?;
    let leaf = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CommandError::new("IO", "output path is invalid"))?;
    let stage = parent.join(format!(
        ".{leaf}.reference-stage-{}-{}",
        std::process::id(),
        STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage)
        .map_err(|_| CommandError::new("IO", "private staging creation failed"))?;
    let mut guard = StageGuard {
        path: stage.clone(),
        armed: true,
    };
    let result = build_staged(&profile, source, &report_identity, &stage)
        .and_then(|outcome| publish_stage(&stage, parent, output, &mut guard).map(|()| outcome));
    if result.is_err() {
        guard.cleanup()?;
    }
    result
}

pub fn inspect_reference_bundle(bundle: &Path) -> Result<ReferenceInspectOutcome, CommandError> {
    let opened = ReferenceBundleOpen::open(bundle).map_err(bundle_error)?;
    let provenance = opened.provenance();
    Ok(ReferenceInspectOutcome {
        ok: true,
        command: "reference.inspect",
        profile: provenance.profile().to_owned(),
        format: provenance.format().to_owned(),
        bundle_id: provenance.bundle_id().to_owned(),
        sequences: opened.manifest().sequences.aliases.len() as u64,
        total_bases: opened.manifest().sequences.total_bases,
        integrity: "structural_only",
        member_sha256_checked: false,
    })
}

pub fn reference_window(
    bundle: &Path,
    alias: &str,
    start: u32,
    length: usize,
) -> Result<ReferenceWindowOutcome, CommandError> {
    let opened = ReferenceBundleOpen::open(bundle).map_err(bundle_error)?;
    let contig = opened
        .resolve_alias(alias)
        .map_err(|_| CommandError::new("REFERENCE_WINDOW", "reference alias is unsupported"))?;
    let start = GenomicPosition::new(start)
        .map_err(|_| CommandError::new("REFERENCE_WINDOW", "reference window is invalid"))?;
    let mut bases = vec![0_u8; length];
    opened
        .copy_window(contig, start, &mut bases)
        .map_err(|_| CommandError::new("REFERENCE_WINDOW", "reference window is invalid"))?;
    let provenance = opened.provenance();
    Ok(ReferenceWindowOutcome {
        ok: true,
        command: "reference.window",
        contig: contig.to_string(),
        start: start.get(),
        length,
        bases: String::from_utf8(bases)
            .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "reference bundle is invalid"))?,
        provenance: ReferenceWindowProvenance {
            bundle_id: provenance.bundle_id().to_owned(),
            profile: provenance.profile().to_owned(),
            format: provenance.format().to_owned(),
            assembly: provenance.assembly().to_owned(),
            assembly_accession: provenance.assembly_accession().to_owned(),
            sequence_set_sha256: provenance.sequence_set_sha256().to_owned(),
        },
    })
}

/// Private exhaustive certification. It is intentionally not exposed by the
/// maintenance CLI.
pub fn certify_reference_bundle(bundle: &Path) -> Result<ReferenceCertification, CommandError> {
    let opened = ReferenceBundleOpen::open(bundle).map_err(bundle_error)?;
    opened.inspect_payload().map_err(bundle_error)?;
    let manifest = opened.manifest();
    for member in &manifest.members {
        let actual = hash_file(&bundle.join(&member.path))?;
        if actual != member.sha256 {
            return Err(CommandError::new(
                "REFERENCE_BUNDLE",
                "reference member integrity failed",
            ));
        }
    }
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut total = 0_u64;
    for code in 1..=25 {
        let contig = Grch38Contig::from_code(code)
            .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "reference contig is invalid"))?;
        let alias = &manifest.sequences.aliases[(code - 1) as usize];
        digest.update((alias.accession.len() as u64).to_le_bytes());
        digest.update(alias.accession.as_bytes());
        digest.update(alias.length.to_le_bytes());
        let mut position = 1_u64;
        while position <= alias.length {
            let take = usize::try_from((alias.length - position + 1).min(buffer.len() as u64))
                .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "reference range is invalid"))?;
            opened
                .copy_window(
                    contig,
                    GenomicPosition::new(position as u32).map_err(|_| {
                        CommandError::new("REFERENCE_BUNDLE", "reference range is invalid")
                    })?,
                    &mut buffer[..take],
                )
                .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "reference decode failed"))?;
            digest.update(&buffer[..take]);
            position += take as u64;
            total += take as u64;
        }
    }
    let observed = format!("sha256:{:x}", digest.finalize());
    if total != manifest.sequences.total_bases || observed != manifest.sequences.sequence_set_sha256
    {
        return Err(CommandError::new(
            "REFERENCE_BUNDLE",
            "reference logical identity failed",
        ));
    }
    let contexts = if manifest.profile == MINI_PROFILE {
        verify_mini_contexts(&opened)?
    } else {
        verify_production_contexts(&opened)?
    };
    Ok(ReferenceCertification {
        total_bases: total,
        sequence_set_sha256: observed,
        contexts_verified: contexts,
    })
}

fn build_staged(
    profile: &Profile,
    source: &Path,
    report: &ObservedInput,
    stage: &Path,
) -> Result<ReferenceBuildOutcome, CommandError> {
    let plans = std::array::from_fn(|index| ReferenceContigPlan {
        contig: Grch38Contig::from_code((index + 1) as u8).expect("fixed contig"),
        bases: profile.lengths[index],
    });
    let reference_path = stage.join("reference.pgr");
    let mut writer =
        ReferenceMemberWriter::create(&reference_path, &plans).map_err(index_build_error)?;
    let source_identity = stream_source(profile, source, &mut writer)?;
    let run_counts = writer.finish().map_err(index_build_error)?;
    let total_runs = run_counts.iter().sum::<u64>();
    let notice_path = stage.join("NOTICE");
    write_synced(&notice_path, profile.notice.as_bytes())?;
    let members = vec![
        member(&notice_path, "NOTICE", "text/plain; charset=utf-8")?,
        member(
            &reference_path,
            "reference.pgr",
            "application/vnd.pangopup.reference-acgt2-rle",
        )?,
    ];
    let aliases = (1_u8..=25)
        .map(|code| {
            let contig = Grch38Contig::from_code(code).expect("fixed contig");
            ReferenceAliasManifest {
                contig: contig.to_string(),
                accession: required_accession(contig).to_owned(),
                length: profile.lengths[(code - 1) as usize],
                ambiguity_runs: run_counts[(code - 1) as usize],
            }
        })
        .collect();
    let manifest = ReferenceManifest {
        schema: REFERENCE_SCHEMA.to_owned(),
        reference_format: REFERENCE_FORMAT.to_owned(),
        profile: profile.name.to_owned(),
        builder: BuilderManifest {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            source_sha256: format!("sha256:{}", env!("PANGOPUP_BUILDER_SOURCE_SHA256")),
        },
        source: SourceManifest {
            assembly: profile.assembly.to_owned(),
            assembly_accession: profile.assembly_accession.to_owned(),
            fasta: InputManifest {
                url: profile.fasta_url.to_owned(),
                compression: Some(source_identity.compression.to_owned()),
                bytes: source_identity.bytes,
                sha256: source_identity.sha256,
            },
            assembly_report: InputManifest {
                url: profile.report_url.to_owned(),
                compression: None,
                bytes: report.bytes,
                sha256: report.sha256.clone(),
            },
        },
        sequences: SequenceManifest {
            total_bases: profile.total_bases,
            sequence_set_sha256: format!("sha256:{}", profile.sequence_sha),
            extra_record_count: profile.extra_count,
            extra_accessions_sha256: format!("sha256:{}", profile.extra_sha),
            ambiguity_runs: total_runs,
            aliases,
        },
        members: members.clone(),
        attribution: AttributionManifest {
            notice_path: "NOTICE".to_owned(),
            policy_url: profile.policy_url.to_owned(),
            transformed: true,
        },
    };
    let manifest_bytes =
        canonical_reference_manifest_bytes(&manifest).map_err(index_build_error)?;
    write_synced(&stage.join("manifest.json"), &manifest_bytes)?;
    sync_directory(stage)?;
    let certification = certify_reference_bundle(stage)?;
    if certification.sequence_set_sha256 != format!("sha256:{}", profile.sequence_sha)
        || certification.total_bases != profile.total_bases
        || certification.contexts_verified != profile.contexts
    {
        return Err(CommandError::new(
            "REFERENCE_BUNDLE",
            "private certification did not match the profile",
        ));
    }
    Ok(ReferenceBuildOutcome {
        ok: true,
        command: "reference.build",
        profile: profile.name.to_owned(),
        bundle_id: reference_bundle_id(&manifest_bytes),
        members: members
            .into_iter()
            .map(|member| ReferenceBuildMember {
                path: member.path,
                size: member.size,
                sha256: member.sha256,
            })
            .collect(),
        certification,
    })
}

struct ObservedInput {
    bytes: u64,
    sha256: String,
    compression: &'static str,
}

fn stream_source(
    profile: &Profile,
    source: &Path,
    writer: &mut ReferenceMemberWriter,
) -> Result<ObservedInput, CommandError> {
    let maximum = if profile.name == PRODUCTION_PROFILE {
        972_898_531
    } else {
        MINI_FASTA_BYTES
    };
    let (mut file, identity) = open_held_regular(source, maximum, "REFERENCE_INPUT")?;
    let allowed_size = if profile.name == PRODUCTION_PROFILE {
        identity.size == 972_898_531
    } else {
        matches!(identity.size, MINI_FASTA_BYTES | MINI_GZIP_BYTES)
    };
    if !allowed_size {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference source identity mismatch",
        ));
    }
    let mut prefix = [0_u8; 2];
    file.read_exact(&mut prefix)
        .map_err(|_| CommandError::new("REFERENCE_INPUT", "reference source read failed"))?;
    file.rewind()
        .map_err(|_| CommandError::new("REFERENCE_INPUT", "reference source seek failed"))?;
    let compression = if prefix == [0x1f, 0x8b] {
        "gzip"
    } else {
        "none"
    };
    let observed = authenticate_held(&mut file, &identity, compression, "REFERENCE_INPUT")?;
    validate_source_identity(profile, &observed)?;
    file.rewind()
        .map_err(|_| CommandError::new("REFERENCE_INPUT", "reference source seek failed"))?;

    let mut buffered = BufReader::new(file);
    let mut extras = if compression == "gzip" {
        let decoder = GzDecoder::new(buffered);
        let bounded = DecodedLimit::new(decoder, profile.max_decoded_bytes);
        let mut decoded = BufReader::new(bounded);
        let extras = parse_fasta(&mut decoded, profile, writer)?;
        let decoder = decoded.into_inner();
        let decoder = decoder.into_inner();
        let mut buffered = decoder.into_inner();
        if !buffered
            .fill_buf()
            .map_err(|_| CommandError::new("REFERENCE_INPUT", "reference gzip trailer failed"))?
            .is_empty()
        {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "reference gzip has trailing data",
            ));
        }
        verify_held_identity(&buffered.into_inner(), &identity, "REFERENCE_INPUT")?;
        extras
    } else {
        let bounded = DecodedLimit::new(buffered, profile.max_decoded_bytes);
        let mut decoded = BufReader::new(bounded);
        let extras = parse_fasta(&mut decoded, profile, writer)?;
        let bounded = decoded.into_inner();
        buffered = bounded.into_inner();
        verify_held_identity(&buffered.into_inner(), &identity, "REFERENCE_INPUT")?;
        extras
    };
    extras.sort();
    let mut digest = Sha256::new();
    for accession in &extras {
        digest.update((accession.len() as u64).to_le_bytes());
        digest.update(accession.as_bytes());
    }
    if extras.len() as u64 != profile.extra_count
        || format!("{:x}", digest.finalize()) != profile.extra_sha
    {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference extra-record identity mismatch",
        ));
    }
    Ok(observed)
}

fn parse_fasta(
    reader: &mut dyn BufRead,
    profile: &Profile,
    writer: &mut ReferenceMemberWriter,
) -> Result<Vec<String>, CommandError> {
    let required: BTreeMap<_, _> = (1_u8..=25)
        .map(|code| {
            let contig = Grch38Contig::from_code(code).expect("fixed contig");
            (required_accession(contig), contig)
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut extras = Vec::new();
    let mut active: Option<(Option<Grch38Contig>, u64)> = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = read_bounded_line(reader, &mut line, 1_048_576)?;
        if read == 0 {
            break;
        }
        trim_line(&mut line)?;
        if line.first() == Some(&b'>') {
            finish_fasta_record(&mut active, &profile.lengths, writer)?;
            let header = std::str::from_utf8(&line[1..]).map_err(|_| {
                CommandError::new("REFERENCE_INPUT", "reference FASTA header is invalid")
            })?;
            let accession = header.split_ascii_whitespace().next().unwrap_or("");
            if accession.is_empty() {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA header is invalid",
                ));
            }
            if accession.len() > profile.max_accession_bytes {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA accession exceeds limit",
                ));
            }
            if seen.len() >= profile.max_records {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA record count exceeds limit",
                ));
            }
            let contig = required.get(accession).copied();
            if !seen.insert(accession.to_owned()) {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA accession is duplicated",
                ));
            }
            if let Some(contig) = contig {
                writer.begin_contig(contig).map_err(index_build_error)?;
            } else {
                extras.push(accession.to_owned());
            }
            active = Some((contig, 0));
        } else {
            let Some((contig, count)) = active.as_mut() else {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference sequence precedes its header",
                ));
            };
            if line.is_empty() {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA line is invalid",
                ));
            }
            *count = count
                .checked_add(line.len() as u64)
                .ok_or_else(|| CommandError::new("REFERENCE_INPUT", "reference length overflow"))?;
            if contig.is_some() {
                writer.write_bases(&line).map_err(index_build_error)?;
            } else if line
                .iter()
                .any(|byte| pangopup_index::reference::iupac_code(*byte).is_none())
            {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "reference FASTA contains an invalid IUPAC symbol",
                ));
            }
        }
    }
    finish_fasta_record(&mut active, &profile.lengths, writer)?;
    for accession in required.keys() {
        if !seen.contains(*accession) {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "reference FASTA is missing a required accession",
            ));
        }
    }
    Ok(extras)
}

fn finish_fasta_record(
    active: &mut Option<(Option<Grch38Contig>, u64)>,
    lengths: &[u64; 25],
    writer: &mut ReferenceMemberWriter,
) -> Result<(), CommandError> {
    let Some((contig, count)) = active.take() else {
        return Ok(());
    };
    if count == 0 {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference FASTA has an empty record",
        ));
    }
    if let Some(contig) = contig {
        if count != lengths[(contig.code() - 1) as usize] {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "reference sequence length disagrees with assembly report",
            ));
        }
        writer.finish_contig().map_err(index_build_error)?;
    }
    Ok(())
}

fn trim_line(line: &mut Vec<u8>) -> Result<(), CommandError> {
    if line.ends_with(b"\n") {
        line.pop();
    }
    if line.ends_with(b"\r") {
        line.pop();
    }
    if line.contains(&b'\r') {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference FASTA line ending is invalid",
        ));
    }
    Ok(())
}

fn parse_assembly_report(profile: &Profile, bytes: &[u8]) -> Result<(), CommandError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CommandError::new("REFERENCE_INPUT", "assembly report is not UTF-8"))?;
    let mut observed = BTreeMap::new();
    for line in text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 10 {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "assembly report row is malformed",
            ));
        }
        if fields[1] != "assembled-molecule" {
            continue;
        }
        let accession = fields[6];
        if let Some(code) = (1_u8..=25).find(|code| {
            required_accession(Grch38Contig::from_code(*code).expect("fixed")) == accession
        }) {
            let contig = Grch38Contig::from_code(code).expect("fixed");
            let expected_unit = if contig == Grch38Contig::M {
                "non-nuclear"
            } else {
                "Primary Assembly"
            };
            let length = fields[8].parse::<u64>().ok();
            if fields[7] != expected_unit
                || fields[9] != contig.to_string()
                || length != Some(profile.lengths[(code - 1) as usize])
                || observed.insert(accession, ()).is_some()
            {
                return Err(CommandError::new(
                    "REFERENCE_INPUT",
                    "assembly report required row is invalid",
                ));
            }
        }
    }
    if observed.len() != 25 {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "assembly report is missing required rows",
        ));
    }
    Ok(())
}

fn profile(name: &str) -> Result<Profile, CommandError> {
    match name {
        PRODUCTION_PROFILE => Ok(Profile {
            name: PRODUCTION_PROFILE,
            assembly: "GRCh38.p14",
            assembly_accession: "GCF_000001405.40",
            fasta_url: production_fasta_url(),
            report_url: production_report_url(),
            policy_url: "https://www.ncbi.nlm.nih.gov/home/about/policies/",
            notice: PRODUCTION_NOTICE,
            lengths: production_contig_lengths(),
            total_bases: 3_088_286_401,
            sequence_sha: "2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4",
            extra_count: 680,
            extra_sha: "0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
            contexts: 14,
            max_decoded_bytes: 4_632_429_602,
            max_records: 705,
            max_accession_bytes: 64,
        }),
        MINI_PROFILE => Ok(Profile {
            name: MINI_PROFILE,
            assembly: "synthetic-mini",
            assembly_accession: MINI_PROFILE,
            fasta_url: "urn:pangopup:fixture:reference-mini-v1:source",
            report_url: "urn:pangopup:fixture:reference-mini-v1:assembly-report",
            policy_url: "https://www.gnu.org/licenses/gpl-3.0.html",
            notice: MINI_NOTICE,
            lengths: [
                30, 16, 12, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 9,
            ],
            total_bases: 159,
            sequence_sha: MINI_SEQUENCE_SHA,
            extra_count: 1,
            extra_sha: MINI_EXTRA_SHA,
            contexts: 4,
            max_decoded_bytes: MINI_FASTA_BYTES,
            max_records: 26,
            max_accession_bytes: 64,
        }),
        _ => Err(CommandError::new(
            "CLI_USAGE",
            "reference profile is unsupported",
        )),
    }
}

fn validate_source_identity(
    profile: &Profile,
    observed: &ObservedInput,
) -> Result<(), CommandError> {
    let sha = observed.sha256.strip_prefix("sha256:").unwrap_or("");
    let valid = if profile.name == PRODUCTION_PROFILE {
        observed.compression == "gzip"
            && observed.bytes == 972_898_531
            && sha == "11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"
    } else {
        (observed.compression == "none"
            && observed.bytes == MINI_FASTA_BYTES
            && sha == MINI_FASTA_SHA)
            || (observed.compression == "gzip"
                && observed.bytes == MINI_GZIP_BYTES
                && sha == MINI_GZIP_SHA)
    };
    if valid {
        Ok(())
    } else {
        Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference source identity mismatch",
        ))
    }
}

fn validate_report_identity(
    profile: &Profile,
    observed: &ObservedInput,
) -> Result<(), CommandError> {
    let expected = if profile.name == PRODUCTION_PROFILE {
        (
            80_454,
            "64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38",
        )
    } else {
        (MINI_REPORT_BYTES, MINI_REPORT_SHA)
    };
    if observed.bytes == expected.0 && observed.sha256 == format!("sha256:{}", expected.1) {
        Ok(())
    } else {
        Err(CommandError::new(
            "REFERENCE_INPUT",
            "assembly report identity mismatch",
        ))
    }
}

fn authenticate_regular(
    path: &Path,
    maximum: u64,
    code: &'static str,
) -> Result<(ObservedInput, Vec<u8>), CommandError> {
    let (mut file, identity) = open_held_regular(path, maximum, code)?;
    let observed = authenticate_held(&mut file, &identity, "none", code)?;
    file.rewind()
        .map_err(|_| CommandError::new(code, "input seek failed"))?;
    let mut retained = Vec::with_capacity(identity.size as usize);
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| CommandError::new(code, "input read failed"))?;
        if read == 0 {
            break;
        }
        retained.extend_from_slice(&buffer[..read]);
    }
    verify_held_identity(&file, &identity, code)?;
    Ok((observed, retained))
}

#[derive(Clone, Copy)]
struct HeldIdentity {
    size: u64,
    device: u64,
    inode: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn open_held_regular(
    path: &Path,
    maximum: u64,
    code: &'static str,
) -> Result<(File, HeldIdentity), CommandError> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| CommandError::new(code, "input is unavailable"))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|_| CommandError::new(code, "input metadata failed"))?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(CommandError::new(
            code,
            "input must be a bounded regular file",
        ));
    }
    let identity = HeldIdentity {
        size: metadata.len(),
        device: metadata.dev(),
        inode: metadata.ino(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    };
    Ok((file, identity))
}

fn verify_held_identity(
    file: &File,
    expected: &HeldIdentity,
    code: &'static str,
) -> Result<(), CommandError> {
    let metadata = file
        .metadata()
        .map_err(|_| CommandError::new(code, "input metadata failed"))?;
    let unchanged = metadata.file_type().is_file()
        && metadata.len() == expected.size
        && metadata.dev() == expected.device
        && metadata.ino() == expected.inode
        && metadata.mtime() == expected.modified_seconds
        && metadata.mtime_nsec() == expected.modified_nanoseconds
        && metadata.ctime() == expected.changed_seconds
        && metadata.ctime_nsec() == expected.changed_nanoseconds;
    if unchanged {
        Ok(())
    } else {
        Err(CommandError::new(code, "input changed while reading"))
    }
}

fn authenticate_held(
    file: &mut File,
    identity: &HeldIdentity,
    compression: &'static str,
    code: &'static str,
) -> Result<ObservedInput, CommandError> {
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| CommandError::new(code, "input read failed"))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| CommandError::new(code, "input length overflow"))?;
        if bytes > identity.size {
            return Err(CommandError::new(code, "input changed while reading"));
        }
        hash.update(&buffer[..read]);
    }
    verify_held_identity(file, identity, code)?;
    if bytes != identity.size {
        return Err(CommandError::new(code, "input changed while reading"));
    }
    Ok(ObservedInput {
        bytes,
        sha256: format!("sha256:{:x}", hash.finalize()),
        compression,
    })
}

struct DecodedLimit<R> {
    inner: R,
    remaining: u64,
}

impl<R> DecodedLimit<R> {
    fn new(inner: R, maximum: u64) -> Self {
        Self {
            inner,
            remaining: maximum,
        }
    }

    fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for DecodedLimit<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut extra = [0_u8; 1];
            return match self.inner.read(&mut extra)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "decoded reference exceeds profile limit",
                )),
            };
        }
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        let take = output.len().min(remaining);
        let read = self.inner.read(&mut output[..take])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

fn read_bounded_line(
    reader: &mut dyn BufRead,
    line: &mut Vec<u8>,
    maximum: usize,
) -> Result<usize, CommandError> {
    let mut total = 0_usize;
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| CommandError::new("REFERENCE_INPUT", "reference FASTA read failed"))?;
        if available.is_empty() {
            return Ok(total);
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        if total
            .checked_add(take)
            .is_none_or(|length| length > maximum + 2)
        {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "reference FASTA line exceeds limit",
            ));
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        total += take;
        if line.last() == Some(&b'\n') {
            return Ok(total);
        }
    }
}

fn verify_mini_contexts(opened: &ReferenceBundleOpen) -> Result<u64, CommandError> {
    let contexts = [
        ("chr1", 1_u32, "ACGTRYSWKMBDHVN"),
        ("chr1", 30, "N"),
        ("chr2", 13, "TTTT"),
        ("chr3", 3, "NNACGTNN"),
    ];
    for (alias, start, expected) in contexts {
        let contig = opened.resolve_alias(alias).map_err(bundle_error)?;
        let mut actual = vec![0_u8; expected.len()];
        opened
            .copy_window(
                contig,
                GenomicPosition::new(start).expect("nonzero"),
                &mut actual,
            )
            .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "mini context decode failed"))?;
        if actual != expected.as_bytes() {
            return Err(CommandError::new(
                "REFERENCE_BUNDLE",
                "mini context mismatch",
            ));
        }
    }
    Ok(4)
}

#[derive(serde::Deserialize)]
struct CompatibilityCase {
    id: String,
    input: CompatibilityInput,
    context: CompatibilityContext,
}
#[derive(serde::Deserialize)]
struct CompatibilityInput {
    contig: String,
}
#[derive(serde::Deserialize)]
struct CompatibilityContext {
    start_1based: u32,
    bases: String,
}

/// Pure production-layout prediction used by tests and the retained
/// qualification harness. It reads only the frozen literal corpus, never a
/// reference member.
pub fn production_context_dense_pages() -> Result<Vec<Vec<u64>>, CommandError> {
    let corpus = compatibility_corpus()?;
    let lengths = production_contig_lengths();
    let mut offsets = [0_u64; 25];
    let mut next = 4096_u64;
    for (index, length) in lengths.into_iter().enumerate() {
        offsets[index] = next;
        next += length.div_ceil(4);
    }
    let mut traces = Vec::new();
    for line in corpus.lines() {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| {
            CommandError::new("REFERENCE_BUNDLE", "compatibility corpus is invalid")
        })?;
        if !value
            .get("id")
            .and_then(|value| value.as_str())
            .is_some_and(|id| id.starts_with('M'))
        {
            continue;
        }
        let case: CompatibilityCase = serde_json::from_value(value).map_err(|_| {
            CommandError::new("REFERENCE_BUNDLE", "compatibility context is invalid")
        })?;
        let contig = case.input.contig.parse::<Grch38Contig>().map_err(|_| {
            CommandError::new("REFERENCE_BUNDLE", "compatibility contig is invalid")
        })?;
        let begin = u64::from(case.context.start_1based - 1);
        let end = begin + case.context.bases.len() as u64;
        let first = offsets[(contig.code() - 1) as usize] + begin / 4;
        let last = offsets[(contig.code() - 1) as usize] + end.div_ceil(4);
        traces.push((first / 4096..=(last - 1) / 4096).collect());
    }
    Ok(traces)
}

fn verify_production_contexts(opened: &ReferenceBundleOpen) -> Result<u64, CommandError> {
    let corpus = compatibility_corpus()?;
    let mut verified = 0_u64;
    for line in corpus.lines() {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| {
            CommandError::new("REFERENCE_BUNDLE", "compatibility corpus is invalid")
        })?;
        if !value
            .get("id")
            .and_then(|value| value.as_str())
            .is_some_and(|id| id.starts_with('M'))
        {
            continue;
        }
        let case: CompatibilityCase = serde_json::from_value(value).map_err(|_| {
            CommandError::new("REFERENCE_BUNDLE", "compatibility context is invalid")
        })?;
        let contig = opened
            .resolve_alias(&case.input.contig)
            .map_err(bundle_error)?;
        let mut actual = vec![0_u8; case.context.bases.len()];
        opened
            .copy_window(
                contig,
                GenomicPosition::new(case.context.start_1based).map_err(|_| {
                    CommandError::new("REFERENCE_BUNDLE", "compatibility context is invalid")
                })?,
                &mut actual,
            )
            .map_err(|_| {
                CommandError::new("REFERENCE_BUNDLE", "compatibility context is out of range")
            })?;
        if actual != case.context.bases.as_bytes() {
            return Err(CommandError::new(
                "REFERENCE_BUNDLE",
                format!("compatibility context {} mismatched", case.id),
            ));
        }
        verified += 1;
    }
    if verified != 14 {
        return Err(CommandError::new(
            "REFERENCE_BUNDLE",
            "compatibility context count mismatch",
        ));
    }
    Ok(verified)
}

fn compatibility_corpus() -> Result<&'static str, CommandError> {
    let mut hash = Sha256::new();
    hash.update(COMPATIBILITY_CORPUS.as_bytes());
    if COMPATIBILITY_CORPUS.len() != COMPATIBILITY_CORPUS_BYTES
        || format!("{:x}", hash.finalize()) != COMPATIBILITY_CORPUS_SHA
    {
        return Err(CommandError::new(
            "REFERENCE_BUNDLE",
            "compiled compatibility corpus identity mismatch",
        ));
    }
    Ok(COMPATIBILITY_CORPUS)
}

fn member(path: &Path, name: &str, media_type: &str) -> Result<MemberManifest, CommandError> {
    Ok(MemberManifest {
        path: name.to_owned(),
        size: fs::metadata(path)
            .map_err(|_| CommandError::new("IO", "bundle member metadata failed"))?
            .len(),
        sha256: hash_file(path)?,
        media_type: media_type.to_owned(),
    })
}

fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file =
        File::open(path).map_err(|_| CommandError::new("IO", "bundle member open failed"))?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| CommandError::new("IO", "bundle member read failed"))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file =
        File::create(path).map_err(|_| CommandError::new("IO", "bundle member creation failed"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| CommandError::new("IO", "bundle member write failed"))
}

fn ensure_absent(output: &Path) -> Result<(), CommandError> {
    match fs::symlink_metadata(output) {
        Ok(_) => Err(CommandError::new(
            "ALREADY_EXISTS",
            "reference output already exists",
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(CommandError::new("IO", "reference output metadata failed")),
    }
}

fn publish_stage(
    stage: &Path,
    parent: &Path,
    output: &Path,
    guard: &mut StageGuard,
) -> Result<(), CommandError> {
    publish_stage_with(stage, parent, output, guard, sync_directory)
}

fn publish_stage_with<F>(
    stage: &Path,
    parent: &Path,
    output: &Path,
    guard: &mut StageGuard,
    mut sync_parent: F,
) -> Result<(), CommandError>
where
    F: FnMut(&Path) -> Result<(), CommandError>,
{
    #[cfg(target_os = "linux")]
    let renamed = rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from);
    #[cfg(not(target_os = "linux"))]
    let renamed: io::Result<()> = Err(io::Error::new(
        ErrorKind::Unsupported,
        "no-replace rename unsupported",
    ));
    renamed.map_err(|error| {
        if matches!(
            error.kind(),
            ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
        ) {
            CommandError::new("ALREADY_EXISTS", "reference output already exists")
        } else {
            CommandError::new("IO", "reference publication failed")
        }
    })?;
    guard.armed = false;
    if sync_parent(parent).is_err() {
        fs::remove_dir_all(output)
            .map_err(|_| CommandError::new("IO", "reference publication rollback failed"))?;
        sync_parent(parent)
            .map_err(|_| CommandError::new("IO", "reference publication rollback sync failed"))?;
        return Err(CommandError::new("IO", "reference parent sync failed"));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| CommandError::new("IO", "directory sync failed"))
}

struct StageGuard {
    path: PathBuf,
    armed: bool,
}
impl StageGuard {
    fn cleanup(&mut self) -> Result<(), CommandError> {
        self.armed = false;
        match fs::remove_dir_all(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CommandError::new("IO", "private staging cleanup failed")),
        }
    }
}
impl Drop for StageGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn index_build_error(_error: ReferenceIndexError) -> CommandError {
    CommandError::new("REFERENCE_INPUT", "reference encoding failed")
}
fn bundle_error(_error: ReferenceIndexError) -> CommandError {
    CommandError::new("REFERENCE_BUNDLE", "reference bundle is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, sync::atomic::AtomicUsize};

    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    const MINI_FASTA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/reference-production-mini/source.fa"
    ));
    const MINI_REPORT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/reference-production-mini/assembly_report.txt"
    ));

    struct Temp(PathBuf);
    impl Temp {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pangopup-reference-unit-{label}-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create unit temp");
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn parse_mini_fasta(bytes: &[u8]) -> Result<Vec<String>, CommandError> {
        let profile = profile(MINI_PROFILE).expect("mini profile");
        let temp = Temp::new("fasta");
        let plans = std::array::from_fn(|index| ReferenceContigPlan {
            contig: Grch38Contig::from_code((index + 1) as u8).expect("contig"),
            bases: profile.lengths[index],
        });
        let mut writer =
            ReferenceMemberWriter::create(&temp.0.join("reference.pgr"), &plans).expect("writer");
        let mut reader = BufReader::new(Cursor::new(bytes));
        let extras = parse_fasta(&mut reader, &profile, &mut writer)?;
        writer.finish().map_err(index_build_error)?;
        Ok(extras)
    }

    #[test]
    fn report_parser_rejects_malformed_duplicate_and_missing_required_rows() {
        let profile = profile(MINI_PROFILE).expect("mini profile");
        parse_assembly_report(&profile, MINI_REPORT.as_bytes()).expect("valid report");

        let mut duplicate = MINI_REPORT.to_owned();
        duplicate.push_str(MINI_REPORT.lines().nth(1).expect("required row"));
        duplicate.push('\n');
        assert!(parse_assembly_report(&profile, duplicate.as_bytes()).is_err());

        assert!(parse_assembly_report(&profile, b"malformed\trow\n").is_err());
        let missing = MINI_REPORT
            .lines()
            .filter(|line| !line.contains("NC_000001.11"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse_assembly_report(&profile, missing.as_bytes()).is_err());
    }

    #[test]
    fn fasta_parser_accepts_reordering_and_rejects_missing_duplicate_and_resource_excess() {
        assert_eq!(
            parse_mini_fasta(MINI_FASTA.as_bytes()).expect("reordered valid FASTA"),
            vec!["NC_000025.0"]
        );

        let duplicate = format!("{MINI_FASTA}>NC_000001.11 duplicate\n{}\n", "A".repeat(30));
        assert!(parse_mini_fasta(duplicate.as_bytes()).is_err());

        let missing = MINI_FASTA
            .split_inclusive('\n')
            .skip_while(|line| !line.starts_with(">NC_000001.11"))
            .skip(2)
            .collect::<String>();
        assert!(parse_mini_fasta(missing.as_bytes()).is_err());

        let mut excess = MINI_FASTA.to_owned();
        excess.push_str(">E\nA\n");
        assert_eq!(
            parse_mini_fasta(excess.as_bytes())
                .expect_err("record ceiling")
                .message,
            "reference FASTA record count exceeds limit"
        );

        let long_accession = format!(">{}\nA\n", "E".repeat(65));
        assert_eq!(
            parse_mini_fasta(long_accession.as_bytes())
                .expect_err("accession ceiling")
                .message,
            "reference FASTA accession exceeds limit"
        );

        let mut decoded = DecodedLimit::new(Cursor::new(vec![b'A'; 33]), 32);
        let mut output = Vec::new();
        assert!(decoded.read_to_end(&mut output).is_err());
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn publication_parent_sync_failure_is_removed_and_durably_resynced() {
        let temp = Temp::new("rollback");
        let stage = temp.0.join("stage");
        let output = temp.0.join("output");
        fs::create_dir(&stage).expect("stage");
        fs::write(stage.join("member"), b"member").expect("member");
        let mut guard = StageGuard {
            path: stage.clone(),
            armed: true,
        };
        let calls = std::cell::Cell::new(0_u8);
        let result = publish_stage_with(&stage, &temp.0, &output, &mut guard, |_| {
            let call = calls.get();
            calls.set(call + 1);
            if call == 0 {
                Err(CommandError::new("IO", "injected first parent sync"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.expect_err("injected failure").code, "IO");
        assert_eq!(calls.get(), 2);
        assert!(!stage.exists());
        assert!(!output.exists());
    }

    fn passing_qualification<'a>(pages: &'a [u64]) -> ReferenceQualificationMeasurements<'a> {
        ReferenceQualificationMeasurements {
            total_bases: 3_088_286_401,
            sequence_set_sha256: "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4",
            extra_record_count: 680,
            extra_accessions_sha256: "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
            contexts_verified: 14,
            headline_p50_ns: QUALIFICATION_MAX_P50_NS,
            headline_p95_ns: QUALIFICATION_MAX_P95_NS,
            allocation_calls: 0,
            allocation_bytes: 0,
            open_peak_heap_bytes: QUALIFICATION_MAX_OPEN_HEAP_BYTES,
            builder_peak_heap_bytes: QUALIFICATION_MAX_BUILDER_HEAP_BYTES,
            dense_bytes_read_during_open: 0,
            member_bytes: QUALIFICATION_MAX_MEMBER_BYTES,
            unique_dense_pages: pages,
            per_case_page_count_sum: 20,
        }
    }

    #[test]
    fn qualification_thresholds_keep_boundaries_and_report_every_observed_limit() {
        let pages = [
            31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
        ];
        evaluate_reference_qualification(&passing_qualification(&pages)).expect("boundaries pass");

        let assert_rejection = |observed: &ReferenceQualificationMeasurements<'_>,
                                class,
                                message: &str| {
            let rejection = evaluate_reference_qualification(observed).expect_err("must reject");
            assert_eq!(rejection.class, class);
            assert_eq!(rejection.message, message);
            assert!(rejection.message.is_ascii());
            assert!(rejection.message.len() <= 256);
        };

        let mut observed = passing_qualification(&pages);
        observed.total_bases -= 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::LogicalSequence,
            "logical-sequence b=3088286400/3088286401 s=sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
        );
        let mut observed = passing_qualification(&pages);
        observed.sequence_set_sha256 =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::LogicalSequence,
            "logical-sequence b=3088286401/3088286401 s=sha256:0000000000000000000000000000000000000000000000000000000000000000/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
        );
        let mut observed = passing_qualification(&pages);
        observed.contexts_verified += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::LogicalSequence,
            "logical-sequence b=3088286401/3088286401 s=sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=15/14",
        );
        let mut observed = passing_qualification(&pages);
        observed.extra_record_count += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::LogicalExtras,
            "logical-extras n=681/680 s=sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
        );
        let mut observed = passing_qualification(&pages);
        observed.extra_accessions_sha256 =
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::LogicalExtras,
            "logical-extras n=680/680 s=sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
        );
        let mut observed = passing_qualification(&pages);
        observed.headline_p50_ns += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Latency,
            "latency p50_ns=5587/5586 p95_ns=6100/6100",
        );
        let mut observed = passing_qualification(&pages);
        observed.headline_p95_ns += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Latency,
            "latency p50_ns=5586/5586 p95_ns=6101/6100",
        );
        let mut observed = passing_qualification(&pages);
        observed.allocation_calls = 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Allocations,
            "allocations calls=1/0 bytes=0/0",
        );
        let mut observed = passing_qualification(&pages);
        observed.allocation_bytes = 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Allocations,
            "allocations calls=0/0 bytes=1/0",
        );
        let mut observed = passing_qualification(&pages);
        observed.open_peak_heap_bytes += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Heap,
            "heap open_bytes=2097153/2097152 builder_bytes=16777216/16777216",
        );
        let mut observed = passing_qualification(&pages);
        observed.builder_peak_heap_bytes += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Heap,
            "heap open_bytes=2097152/2097152 builder_bytes=16777217/16777216",
        );
        let mut observed = passing_qualification(&pages);
        observed.dense_bytes_read_during_open = 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Storage,
            "storage dense_open_bytes=1/0 member_bytes=773124288/773124288",
        );
        let mut observed = passing_qualification(&pages);
        observed.member_bytes += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Storage,
            "storage dense_open_bytes=0/0 member_bytes=773124289/773124288",
        );
        let wrong_pages = [31_748];
        let observed = passing_qualification(&wrong_pages);
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Pages,
            "pages unique=[31748]/[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495] per_case_sum=20/20",
        );
        let mut observed = passing_qualification(&pages);
        observed.per_case_page_count_sum += 1;
        assert_rejection(
            &observed,
            ReferenceQualificationFailureClass::Pages,
            "pages unique=[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495]/[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495] per_case_sum=21/20",
        );
    }

    #[test]
    fn qualification_diagnostics_are_bounded_ascii_and_redact_invalid_identity_text() {
        let pages = [
            31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
        ];
        let secret = "not-a-sha\nsecret-token-\u{2603}";
        let mut observed = passing_qualification(&pages);
        observed.sequence_set_sha256 = secret;
        let rejection = evaluate_reference_qualification(&observed).expect_err("invalid identity");
        assert_eq!(
            rejection.class,
            ReferenceQualificationFailureClass::LogicalSequence
        );
        assert_eq!(
            rejection.message,
            format!(
                "logical-sequence b=3088286401/3088286401 s=invalid(len=26,sha256:{:x})/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
                Sha256::digest(secret.as_bytes())
            )
        );
        assert!(rejection.message.is_ascii());
        assert!(rejection.message.len() <= 256);
        assert!(!rejection.message.contains("secret-token"));

        let huge_pages = [u64::MAX; 64];
        let observed = passing_qualification(&huge_pages);
        let rejection = evaluate_reference_qualification(&observed).expect_err("huge page vector");
        assert_eq!(rejection.class, ReferenceQualificationFailureClass::Pages);
        assert!(
            rejection
                .message
                .starts_with("pages unique_len=64 unique_u64be_sha256=sha256:")
        );
        assert!(rejection.message.is_ascii());
        assert!(rejection.message.len() <= 256);
    }

    #[test]
    fn qualification_diagnostics_are_constructively_bounded_over_maximal_public_values() {
        let pages = QUALIFICATION_UNIQUE_DENSE_PAGES;
        let assert_exact = |observed: &ReferenceQualificationMeasurements<'_>,
                            class,
                            expected: String| {
            let rejection = evaluate_reference_qualification(observed).expect_err("must reject");
            assert_eq!(rejection.class, class);
            assert_eq!(rejection.message, expected);
            assert!(rejection.message.is_ascii());
            assert!(rejection.message.len() <= 256);
        };
        let invalid = "x\nsecret-\u{2603}".repeat(65_536);

        let mut observed = passing_qualification(&pages);
        observed.total_bases = u64::MAX;
        observed.sequence_set_sha256 = &invalid;
        observed.contexts_verified = u64::MAX;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::LogicalSequence,
            format!(
                "logical-sequence b=18446744073709551615/3088286401 s=invalid(len={},sha256:{:x})/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=18446744073709551615/14",
                invalid.len(),
                Sha256::digest(invalid.as_bytes())
            ),
        );

        let mut observed = passing_qualification(&pages);
        observed.extra_record_count = u64::MAX;
        observed.extra_accessions_sha256 = &invalid;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::LogicalExtras,
            format!(
                "logical-extras n=18446744073709551615/680 s=invalid(len={},sha256:{:x})/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
                invalid.len(),
                Sha256::digest(invalid.as_bytes())
            ),
        );

        let mut observed = passing_qualification(&pages);
        observed.headline_p50_ns = u64::MAX;
        observed.headline_p95_ns = u64::MAX;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Latency,
            "latency p50_ns=18446744073709551615/5586 p95_ns=18446744073709551615/6100".to_owned(),
        );

        let mut observed = passing_qualification(&pages);
        observed.allocation_calls = u64::MAX;
        observed.allocation_bytes = u64::MAX;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Allocations,
            "allocations calls=18446744073709551615/0 bytes=18446744073709551615/0".to_owned(),
        );

        let mut observed = passing_qualification(&pages);
        observed.open_peak_heap_bytes = u64::MAX;
        observed.builder_peak_heap_bytes = u64::MAX;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Heap,
            "heap open_bytes=18446744073709551615/2097152 builder_bytes=18446744073709551615/16777216".to_owned(),
        );

        let mut observed = passing_qualification(&pages);
        observed.dense_bytes_read_during_open = u64::MAX;
        observed.member_bytes = u64::MAX;
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Storage,
            "storage dense_open_bytes=18446744073709551615/0 member_bytes=18446744073709551615/773124288".to_owned(),
        );

        let huge_pages = [u64::MAX; 4_096];
        let observed = passing_qualification(&huge_pages);
        let mut hash = Sha256::new();
        for page in &huge_pages {
            hash.update(page.to_be_bytes());
        }
        assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Pages,
            format!(
                "pages unique_len=4096 unique_u64be_sha256=sha256:{:x} limit={:?} per_case_sum=20/20",
                hash.finalize(),
                QUALIFICATION_UNIQUE_DENSE_PAGES
            ),
        );

        let unbounded = "\u{2603}".repeat(300);
        let rejection = qualification_rejection(
            ReferenceQualificationFailureClass::Latency,
            unbounded.clone(),
        );
        assert_eq!(
            rejection.message,
            format!(
                "latency diagnostic_len={} diagnostic_sha256=sha256:{:x}",
                unbounded.len(),
                Sha256::digest(unbounded.as_bytes())
            )
        );
        assert!(rejection.message.is_ascii());
        assert!(rejection.message.len() <= 256);
    }
}
