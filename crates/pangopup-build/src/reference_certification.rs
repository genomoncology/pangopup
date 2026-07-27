//! Post-build reference inspection, certification, and qualification.

use crate::command_error::CommandError;
use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceProvider};
use pangopup_index::reference::{
    MINI_PROFILE, ROUTE_TEST_PROFILE, ReferenceBundleOpen, ReferenceIndexError,
    production_contig_lengths,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

const COMPATIBILITY_CORPUS_BYTES: usize = 220_071;
const COMPATIBILITY_CORPUS_SHA: &str =
    "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const COMPATIBILITY_CORPUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/pangolin-compat-v1/cases.jsonl"
));

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
    let contexts = match manifest.profile.as_str() {
        MINI_PROFILE => verify_mini_contexts(&opened)?,
        ROUTE_TEST_PROFILE => verify_route_context(&opened)?,
        _ => verify_production_contexts(&opened)?,
    };
    Ok(ReferenceCertification {
        total_bases: total,
        sequence_set_sha256: observed,
        contexts_verified: contexts,
    })
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

fn verify_route_context(opened: &ReferenceBundleOpen) -> Result<u64, CommandError> {
    let mut actual = vec![0_u8; 10_101];
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut actual,
        )
        .map_err(|_| CommandError::new("REFERENCE_BUNDLE", "route context decode failed"))?;
    if actual.iter().any(|base| *base != b'A') {
        return Err(CommandError::new(
            "REFERENCE_BUNDLE",
            "route context mismatch",
        ));
    }
    Ok(1)
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

fn bundle_error(_error: ReferenceIndexError) -> CommandError {
    CommandError::new("REFERENCE_BUNDLE", "reference bundle is invalid")
}

#[cfg(test)]
#[path = "reference_certification_tests.rs"]
mod tests;
