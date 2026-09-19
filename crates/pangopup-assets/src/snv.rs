//! SNV notice identity and exhaustive bundle certification.

use crate::error::{AssetError, AssetErrorKind};
use pangopup_index::{
    BundleManifest, BundleOpen, DecodedSummary, INDEX_FORMAT, IndexError, InputLocus,
    LogicalManifest, VisitAllError, parse_bundle_manifest_bytes,
    sparse_writer::SPARSE_INDEX_FORMAT,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

pub const NOTICE: &[u8] = include_bytes!("../../../assets/notices/SNV-BUNDLE-NOTICE-v1");
pub const NOTICE_SHA256: &str =
    "sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7";
pub const MAX_FIXED11_BYTES: u64 = 17_179_869_184;
pub const MAX_SPARSE_DIRECT_BYTES: u64 = 3_221_225_472;
pub(super) const MAX_NOTICE_BYTES: u64 = 64 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleCertification {
    pub bundle_id: String,
    pub members_verified: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertifiedBundleSummary {
    pub genes: u64,
    pub loci: u64,
    pub ordinary_loci: u64,
    pub exceptions: u64,
    pub segments: u64,
    pub records: u64,
}

/// Exhaustively certified bundle opened from held member handles.
#[derive(Debug)]
pub struct CertifiedBundle {
    opened: BundleOpen,
    certification: BundleCertification,
    member_size: u64,
    member_sha256: String,
    summary: CertifiedBundleSummary,
}

impl CertifiedBundle {
    pub fn certification(&self) -> &BundleCertification {
        &self.certification
    }

    pub fn manifest(&self) -> &BundleManifest {
        self.opened.manifest()
    }

    pub fn index_format(&self) -> &str {
        &self.opened.manifest().index_format
    }

    pub fn member_size(&self) -> u64 {
        self.member_size
    }

    pub fn member_sha256(&self) -> &str {
        &self.member_sha256
    }

    pub fn summary(&self) -> CertifiedBundleSummary {
        self.summary
    }
}

/// Exhaustively certify an installed three-file bundle.
pub fn certify_bundle(path: &Path) -> Result<BundleCertification, AssetError> {
    preflight_bundle_files(path)?;
    let (manifest, manifest_metadata) = open_regular(
        &path.join("manifest.json"),
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    if manifest_metadata.len() > MAX_MANIFEST_BYTES {
        return Err(bundle_error_code("BUNDLE_INVALID", "manifest size"));
    }
    let (notice, _) = open_regular(
        &path.join("NOTICE"),
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    let (scores, _) = open_regular(
        &path.join("scores.pgi"),
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    Ok(certify_bundle_members(&manifest, &notice, &scores)?.certification)
}

/// Exhaustively certify the exact immutable member inodes supplied by a
/// maintainer caller. No member pathname is reopened by this operation.
pub fn certify_bundle_members(
    manifest: &File,
    notice: &File,
    scores: &File,
) -> Result<CertifiedBundle, AssetError> {
    certify_bundle_members_with_gene_limits(manifest, notice, scores, u64::MAX, u64::MAX)
}

/// Certify held members while bounding each fixed-v1 complete-gene allocation.
pub fn certify_bundle_members_with_gene_limits(
    manifest: &File,
    notice: &File,
    scores: &File,
    maximum_gene_loci: u64,
    maximum_gene_capacity_bytes: u64,
) -> Result<CertifiedBundle, AssetError> {
    let manifest_bytes = read_file_bounded(manifest, MAX_MANIFEST_BYTES)?;
    let notice_size = held_regular_size(notice, "NOTICE")?;
    if notice_size > MAX_NOTICE_BYTES {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE exceeds the fixed-v1 certification ceiling",
        ));
    }
    let scores_size = held_regular_size(scores, "scores.pgi")?;
    let parsed_manifest =
        parse_bundle_manifest_bytes(&manifest_bytes).map_err(map_manifest_error)?;
    let (member_limit, ceiling_message) = match parsed_manifest.index_format.as_str() {
        INDEX_FORMAT => (
            MAX_FIXED11_BYTES,
            "scores.pgi exceeds the fixed-v1 certification ceiling",
        ),
        SPARSE_INDEX_FORMAT => (
            MAX_SPARSE_DIRECT_BYTES,
            "scores.pgi exceeds the sparse-direct-v1 certification ceiling",
        ),
        _ => {
            return Err(AssetError {
                kind: AssetErrorKind::BundleIncompatible,
                legacy_code: Some("BUNDLE_INCOMPATIBLE"),
                message: "incompatible bundle: index format version".to_owned(),
            });
        }
    };
    if scores_size > member_limit {
        return Err(bundle_error_code("BUNDLE_INDEX", ceiling_message));
    }
    if parsed_manifest.index_format == SPARSE_INDEX_FORMAT {
        let declared_scores = inner_member(&parsed_manifest, "scores.pgi")?;
        if declared_scores.size > member_limit {
            return Err(bundle_error_code("BUNDLE_INDEX", ceiling_message));
        }
    }
    let opened =
        BundleOpen::open_members(&manifest_bytes, notice, scores).map_err(map_bundle_open_error)?;
    let notice_member = inner_member(opened.manifest(), "NOTICE")?;
    let scores_member = inner_member(opened.manifest(), "scores.pgi")?;
    if notice_member.size > MAX_NOTICE_BYTES || notice_member.size != NOTICE.len() as u64 {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE exceeds or differs from the exact fixed-v1 notice size",
        ));
    }
    if scores_member.size > member_limit {
        return Err(bundle_error_code("BUNDLE_INDEX", ceiling_message));
    }
    for member in &opened.manifest().members {
        let file = match member.path.as_str() {
            "NOTICE" => notice,
            "scores.pgi" => scores,
            _ => return Err(bundle_error("inner manifest member set mismatch")),
        };
        let actual = hash_file(file)?;
        if actual != member.sha256 {
            return Err(bundle_error_code(
                "BUNDLE_MEMBER_HASH",
                format!("bundle member {} has the wrong SHA-256", member.path),
            ));
        }
    }
    let notice_bytes = read_file_bounded(notice, MAX_NOTICE_BYTES).map_err(with_legacy_io)?;
    if notice_bytes != NOTICE {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE does not match Pangopup's byte-exact embedded notice",
        ));
    }
    let decoded = decode_reader(&opened, maximum_gene_loci, maximum_gene_capacity_bytes)?;
    if decoded.logical != opened.manifest().logical_decoded
        || opened.manifest().logical_source != opened.manifest().logical_decoded
    {
        return Err(bundle_error_code(
            "BUNDLE_LOGICAL_MISMATCH",
            "complete decoded logical stream does not match the manifest",
        ));
    }
    validate_decoded_counts(opened.manifest(), &decoded)?;
    let member_sha256 = scores_member.sha256.clone();
    let member_size = scores_member.size;
    let summary = CertifiedBundleSummary {
        genes: decoded.genes,
        loci: decoded.loci,
        ordinary_loci: decoded.ordinary_loci,
        exceptions: decoded.n_ref_loci,
        segments: decoded.index_segments,
        records: decoded.logical.records,
    };
    let certification = BundleCertification {
        bundle_id: opened.bundle_id().to_owned(),
        members_verified: 2,
    };
    Ok(CertifiedBundle {
        opened,
        certification,
        member_size,
        member_sha256,
        summary,
    })
}

fn map_manifest_error(error: IndexError) -> AssetError {
    match error {
        IndexError::Io(_) => AssetError {
            kind: AssetErrorKind::InputIo,
            legacy_code: Some("BUNDLE_INVALID"),
            message: error.to_string(),
        },
        IndexError::Incompatible(_) => AssetError {
            kind: AssetErrorKind::BundleIncompatible,
            legacy_code: Some("BUNDLE_INCOMPATIBLE"),
            message: error.to_string(),
        },
        _ => bundle_error(error.to_string()),
    }
}

fn map_bundle_open_error(error: IndexError) -> AssetError {
    match error {
        IndexError::Io(_) => AssetError {
            kind: AssetErrorKind::InputIo,
            legacy_code: Some("BUNDLE_INVALID"),
            message: error.to_string(),
        },
        _ => bundle_error(error.to_string()),
    }
}

fn held_regular_size(file: &File, label: &str) -> Result<u64, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(bundle_error(format!("held {label} is not a regular file")));
    }
    Ok(metadata.len())
}

fn preflight_bundle_files(path: &Path) -> Result<(), AssetError> {
    let expected = BTreeSet::from([
        "NOTICE".to_owned(),
        "manifest.json".to_owned(),
        "scores.pgi".to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for (count, entry) in fs::read_dir(path)
        .map_err(|error| bundle_input_io("read bundle directory", error))?
        .enumerate()
    {
        if count >= 3 {
            return Err(bundle_error_code(
                "BUNDLE_INVALID",
                "bundle contains more than three entries",
            ));
        }
        let entry = entry.map_err(|error| bundle_input_io("read bundle entry", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| bundle_error("bundle member name is not UTF-8"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| bundle_input_io("inspect bundle member", error))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(bundle_error("bundle members must be regular files"));
        }
        let limit = match name.as_str() {
            "manifest.json" => MAX_MANIFEST_BYTES,
            "NOTICE" => MAX_NOTICE_BYTES,
            "scores.pgi" => MAX_FIXED11_BYTES,
            _ => {
                return Err(bundle_error_code(
                    "BUNDLE_INVALID",
                    "bundle member set mismatch",
                ));
            }
        };
        if metadata.len() > limit {
            let code = if name == "NOTICE" {
                "BUNDLE_NOTICE"
            } else if name == "scores.pgi" {
                "BUNDLE_INDEX"
            } else {
                "BUNDLE_INVALID"
            };
            return Err(bundle_error_code(code, "bundle member exceeds size limit"));
        }
        actual.insert(name);
    }
    if actual != expected {
        return Err(bundle_error_code(
            "BUNDLE_INVALID",
            "bundle member set mismatch",
        ));
    }
    Ok(())
}

struct DecodedFacts {
    logical: LogicalManifest,
    genes: u64,
    loci: u64,
    ordinary_loci: u64,
    source_segments: u64,
    index_segments: u64,
    gaps: u64,
    omitted_bases: u64,
    n_ref_loci: u64,
    n_omit_a: u64,
    n_omit_t: u64,
}

fn decode_reader(
    reader: &BundleOpen,
    maximum_gene_loci: u64,
    maximum_gene_capacity_bytes: u64,
) -> Result<DecodedFacts, AssetError> {
    let mut hash = HashSink::new();
    let mut facts = DecodedFacts {
        logical: LogicalManifest {
            records: 0,
            sha256: String::new(),
        },
        genes: 0,
        loci: 0,
        ordinary_loci: 0,
        source_segments: 0,
        index_segments: 0,
        gaps: 0,
        omitted_bases: 0,
        n_ref_loci: 0,
        n_omit_a: 0,
        n_omit_t: 0,
    };
    let mut previous: Option<(u64, u8, u32)> = None;
    let mut previous_ordinary: Option<(u64, u8, u32)> = None;
    let decoded_summary = reader
        .visit_all_bounded(maximum_gene_loci, maximum_gene_capacity_bytes, |locus| {
            write_logical_text(&mut hash, locus)?;
            add(&mut facts.logical.records, 3)?;
            add(&mut facts.loci, 1)?;
            let (gene, contig, position) = match locus {
                InputLocus::Ordinary(value) => {
                    add(&mut facts.ordinary_loci, 1)?;
                    let current = (
                        value.gene.numeric(),
                        value.contig.code(),
                        value.position.get(),
                    );
                    if previous_ordinary.is_none_or(|prior| {
                        prior.0 != current.0
                            || prior.1 != current.1
                            || prior.2.checked_add(1) != Some(current.2)
                    }) {
                        add(&mut facts.index_segments, 1)?;
                    }
                    previous_ordinary = Some(current);
                    current
                }
                InputLocus::Ambiguous(value) => {
                    add(&mut facts.n_ref_loci, 1)?;
                    match value.omitted.to_string().as_str() {
                        "A" => add(&mut facts.n_omit_a, 1)?,
                        "T" => add(&mut facts.n_omit_t, 1)?,
                        _ => {
                            return Err(io::Error::other("invalid omitted exception base"));
                        }
                    }
                    (
                        value.gene.numeric(),
                        value.contig.code(),
                        value.position.get(),
                    )
                }
            };
            match previous {
                None => {
                    add(&mut facts.genes, 1)?;
                    add(&mut facts.source_segments, 1)?;
                }
                Some((prior_gene, _, _)) if prior_gene != gene => {
                    add(&mut facts.genes, 1)?;
                    add(&mut facts.source_segments, 1)?;
                }
                Some((_, prior_contig, prior_position)) => {
                    if prior_contig != contig || position <= prior_position {
                        return Err(io::Error::other("decoded logical order"));
                    }
                    let distance = u64::from(position - prior_position);
                    if distance > 1 {
                        add(&mut facts.gaps, 1)?;
                        add(&mut facts.omitted_bases, distance - 1)?;
                        add(&mut facts.source_segments, 1)?;
                    }
                }
            }
            previous = Some((gene, contig, position));
            Ok::<_, io::Error>(())
        })
        .map_err(|error| match error {
            VisitAllError::Index(error) => bundle_error_code("BUNDLE_INDEX", error.to_string()),
            VisitAllError::Visitor(error) => bundle_error(error.to_string()),
        })?;
    require_traversal_summary(decoded_summary, &facts)?;
    facts.logical.sha256 = hash.finish();
    Ok(facts)
}

fn validate_decoded_counts(
    manifest: &BundleManifest,
    decoded: &DecodedFacts,
) -> Result<(), AssetError> {
    let counts = manifest.counts;
    let directions = counts
        .ascending_members
        .checked_add(counts.descending_members)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    let shapes = counts
        .n_omit_a
        .checked_add(counts.n_omit_t)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    let rows = counts
        .gene_loci
        .checked_mul(3)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    if counts.source_rows != decoded.logical.records
        || rows != counts.source_rows
        || counts.gene_loci != decoded.loci
        || counts.genes != decoded.genes
        || counts.genes != directions
        || manifest.source.observed_member_count != counts.genes
        || counts.source_segments != decoded.source_segments
        || counts.gap_transitions != decoded.gaps
        || counts.omitted_bases != decoded.omitted_bases
        || counts.index_segments != decoded.index_segments
        || counts.n_ref_loci != decoded.n_ref_loci
        || counts.gene_loci.checked_sub(counts.n_ref_loci) != Some(decoded.ordinary_loci)
        || counts.n_omit_a != decoded.n_omit_a
        || counts.n_omit_t != decoded.n_omit_t
        || shapes != counts.n_ref_loci
    {
        return Err(bundle_error_code(
            "BUNDLE_COUNTS",
            "manifest counts do not agree with complete index decode",
        ));
    }
    Ok(())
}

fn require_traversal_summary(
    summary: DecodedSummary,
    decoded: &DecodedFacts,
) -> Result<(), AssetError> {
    if summary.genes != decoded.genes
        || summary.loci != decoded.loci
        || summary.ordinary_loci != decoded.ordinary_loci
        || summary.exceptions != decoded.n_ref_loci
        || summary.segments != decoded.index_segments
    {
        return Err(bundle_error_code(
            "BUNDLE_INDEX",
            "index traversal summary does not agree with complete decode",
        ));
    }
    Ok(())
}

struct HashSink(Sha256);

impl HashSink {
    fn new() -> Self {
        Self(Sha256::new())
    }

    fn finish(self) -> String {
        format!("sha256:{:x}", self.0.finalize())
    }
}

impl Write for HashSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_logical_text(output: &mut impl Write, locus: InputLocus) -> io::Result<()> {
    let (kind, gene, contig, position, reference, mut alternatives, omitted) = match locus {
        InputLocus::Ordinary(value) => (
            "O",
            value.gene,
            value.contig,
            value.position,
            value.reference.to_string(),
            value.alternatives,
            None,
        ),
        InputLocus::Ambiguous(value) => (
            "N",
            value.gene,
            value.contig,
            value.position,
            "N".to_owned(),
            value.alternatives,
            Some(value.omitted),
        ),
    };
    alternatives.sort_by_key(|value| value.alternate);
    for alternative in alternatives {
        write!(
            output,
            "{kind}\t{gene}\t{contig}\t{position}\t{reference}\t{}\t{}\t{}\t{}\t{}",
            alternative.alternate,
            alternative.score.gain().hundredths(),
            alternative.score.gain_position().get(),
            alternative.score.loss().hundredths(),
            alternative.score.loss_position().get()
        )?;
        if let Some(omitted) = omitted {
            write!(output, "\t{omitted}")?;
        }
        writeln!(output)?;
    }
    Ok(())
}

fn add(target: &mut u64, amount: u64) -> io::Result<()> {
    *target = target
        .checked_add(amount)
        .ok_or_else(|| io::Error::other("decoded count overflow"))?;
    Ok(())
}

fn hash_file(file: &File) -> Result<String, AssetError> {
    let mut file = file.try_clone().map_err(|error| AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("IO"),
        message: error.to_string(),
    })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("IO"),
        message: error.to_string(),
    })?;
    let mut hash = Sha256::new();
    copy_hash(&mut file, &mut hash, None).map_err(|error| AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("IO"),
        message: error.to_string(),
    })?;
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn read_file_bounded(file: &File, cap: u64) -> Result<Vec<u8>, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if !metadata.file_type().is_file() || metadata.len() > cap {
        return Err(bundle_error("bounded input exceeds size limit"));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| bundle_error("bounded input size conversion"))?;
    let mut held = file
        .try_clone()
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    held.seek(SeekFrom::Start(0))
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    let mut bytes = Vec::with_capacity(capacity);
    held.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(bundle_error("bounded input grew beyond size limit"));
    }
    Ok(bytes)
}

fn inner_member<'a>(
    manifest: &'a BundleManifest,
    path: &str,
) -> Result<&'a pangopup_index::MemberManifest, AssetError> {
    manifest
        .members
        .iter()
        .find(|member| member.path == path)
        .ok_or_else(|| bundle_error(format!("inner manifest lacks {path}")))
}

fn copy_hash(
    reader: &mut impl Read,
    hash: &mut Sha256,
    mut second: Option<&mut Sha256>,
) -> io::Result<u64> {
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        if let Some(other) = second.as_deref_mut() {
            other.update(&buffer[..read]);
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("hash size overflow"))?;
    }
    Ok(total)
}

fn open_regular(
    path: &Path,
    io_kind: AssetErrorKind,
    invalid_kind: AssetErrorKind,
) -> Result<(File, fs::Metadata), AssetError> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        AssetError::new(io_kind, format!("inspect {}: {error}", path.display()))
    })?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(AssetError::new(
            invalid_kind,
            "required input is not a regular file",
        ));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = File::open(path);
    let file = file.map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    #[cfg(any(test, feature = "test-read-audit"))]
    crate::input_audit::record_test_input_open(path);
    let metadata = file
        .metadata()
        .map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(AssetError::new(
            invalid_kind,
            "opened input is not a regular file",
        ));
    }
    Ok((file, metadata))
}

fn bundle_input_io(action: &str, error: io::Error) -> AssetError {
    AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("BUNDLE_INVALID"),
        message: format!("{action}: {error}"),
    }
}

fn with_legacy_io(mut error: AssetError) -> AssetError {
    if error.kind == AssetErrorKind::InputIo {
        error.legacy_code = Some("IO");
    }
    error
}

fn bundle_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, message)
}

fn bundle_error_code(code: &'static str, message: impl Into<String>) -> AssetError {
    AssetError {
        kind: AssetErrorKind::BundleInvalid,
        legacy_code: Some(code),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pangopup_core::{
        DnaBase, EnsemblGeneId, GenomicPosition, PangolinScore, RelativePosition, ScoreMagnitude,
    };
    use pangopup_index::{
        AmbiguousInputLocus, BundleCounts, BundleManifest, INDEX_FORMAT, InputAlternative,
        InputLocus, OrdinaryInputLocus, bundle_id, canonical_manifest_bytes,
        sparse_writer::{SPARSE_INDEX_FORMAT, SparseIndexWriter},
        write_index,
    };
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pangopup-sparse-certification-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create temporary directory");
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove temporary directory");
        }
    }

    const SPARSE_MEDIA_TYPE: &str = "application/vnd.pangopup.sparse-direct";
    const FIXED_MEDIA_TYPE: &str = "application/vnd.pangopup.fixed11";

    #[test]
    fn fixed_and_sparse_certification_report_the_same_exhaustive_facts() {
        let temp = Temp::new();
        let genes = miniature();
        let fixed = write_bundle(&temp, "fixed", INDEX_FORMAT, FIXED_MEDIA_TYPE, &genes);
        let sparse = write_bundle(
            &temp,
            "sparse",
            SPARSE_INDEX_FORMAT,
            SPARSE_MEDIA_TYPE,
            &genes,
        );

        let fixed_certified = certify_path(&fixed, 10, 1024 * 1024).expect("fixed certification");
        let sparse_certified = certify_path(&sparse, 0, 0).expect("streaming sparse certification");
        assert_eq!(fixed_certified.index_format(), INDEX_FORMAT);
        assert_eq!(sparse_certified.index_format(), SPARSE_INDEX_FORMAT);
        assert_eq!(fixed_certified.summary(), sparse_certified.summary());
        assert_eq!(fixed_certified.summary().genes, 3);
        assert_eq!(fixed_certified.summary().loci, 4);
        assert_eq!(fixed_certified.summary().ordinary_loci, 1);
        assert_eq!(fixed_certified.summary().exceptions, 3);
        assert_eq!(fixed_certified.summary().segments, 1);
        assert_eq!(fixed_certified.summary().records, 12);
        for (bundle, certified) in [(&fixed, &fixed_certified), (&sparse, &sparse_certified)] {
            let scores = fs::read(bundle.join("scores.pgi")).expect("scores bytes");
            assert_eq!(certified.member_size(), scores.len() as u64);
            assert_eq!(
                certified.member_sha256(),
                format!("sha256:{:x}", Sha256::digest(scores))
            );
            let manifest = fs::read(bundle.join("manifest.json")).expect("manifest bytes");
            assert_eq!(certified.certification().bundle_id, bundle_id(&manifest));
        }

        let fixed_limit = certify_path(&fixed, 0, 0).expect_err("fixed gene limit remains active");
        assert_eq!(fixed_limit.legacy_build_code(), Some("BUNDLE_INDEX"));
        assert!(fixed_limit.to_string().contains("allocation limit"));
    }

    #[test]
    fn sparse_certification_rejects_payload_digest_count_and_size_failures() {
        let temp = Temp::new();
        let genes = miniature();
        let original = write_bundle(
            &temp,
            "original",
            SPARSE_INDEX_FORMAT,
            SPARSE_MEDIA_TYPE,
            &genes,
        );

        let corrupt = copy_bundle(&temp, &original, "corrupt");
        corrupt_sparse_pair(&corrupt.join("scores.pgi"));
        refresh_score_member(&corrupt);
        assert_code(&corrupt, "BUNDLE_INDEX");

        let logical = copy_bundle(&temp, &original, "logical");
        edit_manifest(&logical, |manifest| {
            let digest = format!("sha256:{}", "0".repeat(64));
            manifest.logical_source.sha256.clone_from(&digest);
            manifest.logical_decoded.sha256 = digest;
        });
        assert_code(&logical, "BUNDLE_LOGICAL_MISMATCH");

        let counts = copy_bundle(&temp, &original, "counts");
        edit_manifest(&counts, |manifest| manifest.counts.gap_transitions += 1);
        assert_code(&counts, "BUNDLE_COUNTS");

        let declared = copy_bundle(&temp, &original, "declared-oversize");
        edit_manifest(&declared, |manifest| {
            manifest.members[1].size = MAX_SPARSE_DIRECT_BYTES + 1;
        });
        assert_code(&declared, "BUNDLE_INDEX");

        let held = copy_bundle(&temp, &original, "held-oversize");
        File::options()
            .write(true)
            .open(held.join("scores.pgi"))
            .expect("held scores")
            .set_len(MAX_SPARSE_DIRECT_BYTES + 1)
            .expect("sparse oversized file");
        assert_code(&held, "BUNDLE_INDEX");
    }

    #[test]
    fn fixed_declared_oversize_preserves_member_size_mismatch_error() {
        let temp = Temp::new();
        let fixed = write_bundle(
            &temp,
            "fixed-declared-oversize",
            INDEX_FORMAT,
            FIXED_MEDIA_TYPE,
            &miniature(),
        );
        edit_manifest(&fixed, |manifest| {
            manifest.members[1].size = MAX_FIXED11_BYTES + 1;
        });

        let error = certify_path(&fixed, u64::MAX, u64::MAX)
            .expect_err("held member must disagree with fixed declaration");
        assert_eq!(error.kind(), AssetErrorKind::BundleInvalid);
        assert_eq!(error.kind().code(), "BUNDLE_INVALID");
        assert_eq!(error.legacy_build_code(), None);
        assert!(error.to_string().contains("bundle member size"));
    }

    #[test]
    fn sparse_certification_rejects_format_media_and_payload_mismatch() {
        let temp = Temp::new();
        let genes = miniature();
        let sparse = write_bundle(
            &temp,
            "sparse-mismatch-source",
            SPARSE_INDEX_FORMAT,
            SPARSE_MEDIA_TYPE,
            &genes,
        );
        let fixed = write_bundle(
            &temp,
            "fixed-mismatch-source",
            INDEX_FORMAT,
            FIXED_MEDIA_TYPE,
            &genes,
        );

        let format = copy_bundle(&temp, &sparse, "format-mismatch");
        edit_manifest(&format, |manifest| {
            manifest.index_format = "pangopup.unknown.v1".to_owned();
        });
        let format_error = certify_path(&format, u64::MAX, u64::MAX)
            .expect_err("unsupported format must remain typed");
        assert_eq!(format_error.kind(), AssetErrorKind::BundleIncompatible);
        assert_eq!(format_error.kind().code(), "BUNDLE_INCOMPATIBLE");
        assert_eq!(
            format_error.legacy_build_code(),
            Some("BUNDLE_INCOMPATIBLE")
        );

        let media = copy_bundle(&temp, &sparse, "media-mismatch");
        edit_manifest(&media, |manifest| {
            manifest.members[1].media_type = FIXED_MEDIA_TYPE.to_owned();
        });
        assert_eq!(
            certify_path(&media, u64::MAX, u64::MAX)
                .expect_err("media mismatch")
                .kind(),
            AssetErrorKind::BundleInvalid
        );

        let payload = copy_bundle(&temp, &sparse, "payload-mismatch");
        fs::copy(fixed.join("scores.pgi"), payload.join("scores.pgi"))
            .expect("replace sparse payload");
        refresh_score_member(&payload);
        assert_eq!(
            certify_path(&payload, u64::MAX, u64::MAX)
                .expect_err("payload mismatch")
                .kind(),
            AssetErrorKind::BundleInvalid
        );
    }

    #[test]
    fn direction_counts_are_certified_only_as_their_declared_sum() {
        let temp = Temp::new();
        let genes = miniature();
        let sparse = write_bundle(
            &temp,
            "direction-split",
            SPARSE_INDEX_FORMAT,
            SPARSE_MEDIA_TYPE,
            &genes,
        );
        edit_manifest(&sparse, |manifest| {
            manifest.counts.ascending_members += 1;
            manifest.counts.descending_members -= 1;
        });
        certify_path(&sparse, 0, 0).expect("direction sum remains reconstructable");
    }

    fn miniature() -> Vec<Vec<InputLocus>> {
        vec![
            vec![ambiguous(1, 10, DnaBase::A)],
            vec![ordinary(2, 20, DnaBase::A), ambiguous(2, 22, DnaBase::T)],
            vec![ambiguous(3, 30, DnaBase::T)],
        ]
    }

    fn ordinary(gene: u64, position: u32, reference: DnaBase) -> InputLocus {
        let alternatives = DnaBase::ALL
            .into_iter()
            .filter(|base| *base != reference)
            .map(|alternate| InputAlternative {
                alternate,
                score: score(alternate as u16 + 1),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("three alternatives");
        InputLocus::Ordinary(OrdinaryInputLocus {
            gene: EnsemblGeneId::from_numeric(gene).expect("gene"),
            contig: "chr1".parse().expect("contig"),
            position: GenomicPosition::new(position).expect("position"),
            reference,
            alternatives,
        })
    }

    fn ambiguous(gene: u64, position: u32, omitted: DnaBase) -> InputLocus {
        let alternatives = DnaBase::ALL
            .into_iter()
            .filter(|base| *base != omitted)
            .map(|alternate| InputAlternative {
                alternate,
                score: score(alternate as u16 + 10),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("three alternatives");
        InputLocus::Ambiguous(AmbiguousInputLocus {
            gene: EnsemblGeneId::from_numeric(gene).expect("gene"),
            contig: "chr1".parse().expect("contig"),
            position: GenomicPosition::new(position).expect("position"),
            alternatives,
            omitted,
        })
    }

    fn score(seed: u16) -> PangolinScore {
        PangolinScore::new(
            ScoreMagnitude::new(seed).expect("gain"),
            RelativePosition::new(-10).expect("gain position"),
            ScoreMagnitude::new(seed + 1).expect("loss"),
            RelativePosition::new(10).expect("loss position"),
        )
    }

    fn write_bundle(
        temp: &Temp,
        label: &str,
        format: &str,
        media_type: &str,
        genes: &[Vec<InputLocus>],
    ) -> PathBuf {
        let bundle = temp.0.join(label);
        fs::create_dir(&bundle).expect("bundle directory");
        let fixture = fixture_bundle();
        fs::copy(fixture.join("NOTICE"), bundle.join("NOTICE")).expect("notice");
        let scores = bundle.join("scores.pgi");
        match format {
            INDEX_FORMAT => {
                write_index(
                    &scores,
                    &genes.iter().flatten().copied().collect::<Vec<_>>(),
                )
                .expect("fixed index");
            }
            SPARSE_INDEX_FORMAT => {
                let mut writer =
                    SparseIndexWriter::create(&bundle.join("scores.scratch")).expect("writer");
                for gene in genes {
                    writer.push_gene(gene).expect("sparse gene");
                }
                writer.finish(&scores).expect("sparse index");
            }
            _ => panic!("unsupported test format"),
        }
        let mut manifest: BundleManifest = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("fixture manifest"),
        )
        .expect("manifest");
        manifest.index_format = format.to_owned();
        manifest.counts = BundleCounts {
            genes: 3,
            source_rows: 12,
            gene_loci: 4,
            ascending_members: 2,
            descending_members: 1,
            source_segments: 4,
            index_segments: 1,
            gap_transitions: 1,
            omitted_bases: 1,
            n_ref_loci: 3,
            n_omit_a: 1,
            n_omit_t: 2,
        };
        manifest.source.observed_member_count = 3;
        let mut logical = HashSink::new();
        for locus in genes.iter().flatten().copied() {
            write_logical_text(&mut logical, locus).expect("logical text");
        }
        let logical = LogicalManifest {
            records: 12,
            sha256: logical.finish(),
        };
        manifest.logical_source = logical.clone();
        manifest.logical_decoded = logical;
        manifest.members[1].media_type = media_type.to_owned();
        manifest.members[1].size = fs::metadata(&scores).expect("scores metadata").len();
        manifest.members[1].sha256 =
            hash_file(&File::open(&scores).expect("scores file")).expect("scores digest");
        fs::write(
            bundle.join("manifest.json"),
            canonical_manifest_bytes(&manifest).expect("manifest bytes"),
        )
        .expect("write manifest");
        bundle
    }

    fn fixture_bundle() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/snv-regression/bundle")
    }

    fn certify_path(
        bundle: &Path,
        maximum_gene_loci: u64,
        maximum_gene_capacity_bytes: u64,
    ) -> Result<CertifiedBundle, AssetError> {
        let manifest = File::open(bundle.join("manifest.json")).expect("manifest file");
        let notice = File::open(bundle.join("NOTICE")).expect("notice file");
        let scores = File::open(bundle.join("scores.pgi")).expect("scores file");
        certify_bundle_members_with_gene_limits(
            &manifest,
            &notice,
            &scores,
            maximum_gene_loci,
            maximum_gene_capacity_bytes,
        )
    }

    fn copy_bundle(temp: &Temp, source: &Path, label: &str) -> PathBuf {
        let destination = temp.0.join(label);
        fs::create_dir(&destination).expect("copy directory");
        for name in ["NOTICE", "manifest.json", "scores.pgi"] {
            fs::copy(source.join(name), destination.join(name)).expect("copy member");
        }
        destination
    }

    fn edit_manifest(bundle: &Path, edit: impl FnOnce(&mut BundleManifest)) {
        let path = bundle.join("manifest.json");
        let mut manifest: BundleManifest =
            serde_json::from_slice(&fs::read(&path).expect("manifest bytes")).expect("manifest");
        edit(&mut manifest);
        fs::write(
            path,
            canonical_manifest_bytes(&manifest).expect("canonical manifest"),
        )
        .expect("write manifest");
    }

    fn refresh_score_member(bundle: &Path) {
        edit_manifest(bundle, |manifest| {
            let bytes = fs::read(bundle.join("scores.pgi")).expect("scores bytes");
            manifest.members[1].size = bytes.len() as u64;
            manifest.members[1].sha256 = format!("sha256:{:x}", Sha256::digest(bytes));
        });
    }

    fn corrupt_sparse_pair(path: &Path) {
        let mut bytes = fs::read(path).expect("sparse bytes");
        let block = u64_at(&bytes, 56) as usize;
        let raw = u64_at(&bytes, 72) as usize + u64_at(&bytes, block + 16) as usize;
        let count = u32_at(&bytes, block + 12) as usize;
        let active = u32_at(&bytes, block + 28) as usize;
        let pairs = u32_at(&bytes, block + 32) as usize;
        let values = raw
            + 16
            + (count * 2).div_ceil(8)
            + count.div_ceil(8)
            + count.div_ceil(64) * 8
            + (active * 6).div_ceil(8);
        bytes[values + (pairs - 1) * 2 + 1] |= 0xc0;
        fs::write(path, bytes).expect("corrupt sparse pair");
    }

    fn assert_code(bundle: &Path, expected: &'static str) {
        let error = certify_path(bundle, u64::MAX, u64::MAX).expect_err(expected);
        assert_eq!(error.legacy_build_code(), Some(expected));
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
    }
}
