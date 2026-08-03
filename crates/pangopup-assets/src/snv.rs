//! SNV notice identity and exhaustive fixed-v1 bundle certification.

use crate::error::{AssetError, AssetErrorKind};
use pangopup_index::{
    BundleManifest, BundleOpen, IndexError, IndexReader, InputLocus, LogicalManifest, VisitAllError,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
};

pub const NOTICE: &[u8] = include_bytes!("../../../assets/notices/SNV-BUNDLE-NOTICE-v1");
pub const NOTICE_SHA256: &str =
    "sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7";
pub const MAX_FIXED11_BYTES: u64 = 17_179_869_184;
pub(super) const MAX_NOTICE_BYTES: u64 = 64 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleCertification {
    pub bundle_id: String,
    pub members_verified: u64,
}

/// Exhaustively certify an installed three-file bundle.
pub fn certify_bundle(path: &Path) -> Result<BundleCertification, AssetError> {
    preflight_bundle_files(path)?;
    let opened = BundleOpen::open(path).map_err(|error| match error {
        IndexError::Io(_) => AssetError {
            kind: AssetErrorKind::InputIo,
            legacy_code: Some("BUNDLE_INVALID"),
            message: error.to_string(),
        },
        _ => bundle_error(error.to_string()),
    })?;
    let notice_member = inner_member(opened.manifest(), "NOTICE")?;
    let scores_member = inner_member(opened.manifest(), "scores.pgi")?;
    if notice_member.size > MAX_NOTICE_BYTES || notice_member.size != NOTICE.len() as u64 {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE exceeds or differs from the exact fixed-v1 notice size",
        ));
    }
    if scores_member.size > MAX_FIXED11_BYTES {
        return Err(bundle_error_code(
            "BUNDLE_INDEX",
            "scores.pgi exceeds the fixed-v1 certification ceiling",
        ));
    }
    for member in &opened.manifest().members {
        let actual = hash_bundle_member(&path.join(&member.path))?;
        if actual != member.sha256 {
            return Err(bundle_error_code(
                "BUNDLE_MEMBER_HASH",
                format!("bundle member {} has the wrong SHA-256", member.path),
            ));
        }
    }
    let notice = read_bounded(
        &path.join("NOTICE"),
        MAX_NOTICE_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )
    .map_err(with_legacy_io)?;
    if notice != NOTICE {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE does not match Pangopup's byte-exact embedded notice",
        ));
    }
    opened
        .index()
        .verify_canonical_structure()
        .map_err(|error| bundle_error_code("BUNDLE_INDEX", error.to_string()))?;
    let decoded = decode_reader(opened.index())?;
    if decoded.logical != opened.manifest().logical_decoded
        || opened.manifest().logical_source != opened.manifest().logical_decoded
    {
        return Err(bundle_error_code(
            "BUNDLE_LOGICAL_MISMATCH",
            "complete decoded logical stream does not match the manifest",
        ));
    }
    validate_decoded_counts(opened.manifest(), opened.index(), &decoded)?;
    Ok(BundleCertification {
        bundle_id: opened.bundle_id().to_owned(),
        members_verified: 2,
    })
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
    source_segments: u64,
    index_segments: u64,
    gaps: u64,
    omitted_bases: u64,
    n_ref_loci: u64,
    n_omit_a: u64,
    n_omit_t: u64,
}

fn decode_reader(reader: &IndexReader) -> Result<DecodedFacts, AssetError> {
    let mut hash = HashSink::new();
    let mut facts = DecodedFacts {
        logical: LogicalManifest {
            records: 0,
            sha256: String::new(),
        },
        genes: 0,
        loci: 0,
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
    reader
        .visit_all(|locus| {
            write_logical_text(&mut hash, locus)?;
            add(&mut facts.logical.records, 3)?;
            add(&mut facts.loci, 1)?;
            let (gene, contig, position) = match locus {
                InputLocus::Ordinary(value) => {
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
                        _ => return Err(io::Error::other("invalid omitted exception base")),
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
    facts.logical.sha256 = hash.finish();
    Ok(facts)
}

fn validate_decoded_counts(
    manifest: &BundleManifest,
    reader: &IndexReader,
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
        || decoded.index_segments != reader.segment_count()
        || counts.n_ref_loci != reader.exception_count()
        || counts.n_ref_loci != decoded.n_ref_loci
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

fn hash_bundle_member(path: &Path) -> Result<String, AssetError> {
    let (mut file, _) = open_regular(path, AssetErrorKind::InputIo, AssetErrorKind::BundleInvalid)
        .map_err(with_legacy_io)?;
    let mut hash = Sha256::new();
    copy_hash(&mut file, &mut hash, None).map_err(|error| AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("IO"),
        message: error.to_string(),
    })?;
    Ok(format!("sha256:{:x}", hash.finalize()))
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

fn read_bounded(
    path: &Path,
    cap: u64,
    io_kind: AssetErrorKind,
    invalid_kind: AssetErrorKind,
) -> Result<Vec<u8>, AssetError> {
    let (file, metadata) = open_regular(path, io_kind, invalid_kind)?;
    if metadata.len() > cap {
        return Err(AssetError::new(
            invalid_kind,
            "bounded input exceeds size limit",
        ));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| AssetError::new(invalid_kind, "bounded input size conversion"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(AssetError::new(
            invalid_kind,
            "bounded input grew beyond size limit",
        ));
    }
    Ok(bytes)
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
