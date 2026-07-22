use super::{
    SourceLocus, TotalSummary, discover_members, index_locus, inspect_member_hashed,
    parse_member_gene,
};
use flate2::bufread::GzDecoder;
use pangopup_index::{
    AliasManifest, AttributionManifest, BuilderManifest, BundleCounts, BundleManifest, BundleOpen,
    IndexReader, InputLocus, LogicalManifest, MemberManifest, ReferenceManifest, SourceManifest,
    StreamingIndexWriter, VisitAllError, canonical_manifest_bytes,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fmt, fs,
    fs::File,
    io::{self, BufRead, BufReader, ErrorKind, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const NOTICE: &[u8] = include_bytes!("../../../NOTICE");
const REQUIRED: [(&str, &str); 25] = [
    ("chr1", "NC_000001.11"),
    ("chr2", "NC_000002.12"),
    ("chr3", "NC_000003.12"),
    ("chr4", "NC_000004.12"),
    ("chr5", "NC_000005.10"),
    ("chr6", "NC_000006.12"),
    ("chr7", "NC_000007.14"),
    ("chr8", "NC_000008.11"),
    ("chr9", "NC_000009.12"),
    ("chr10", "NC_000010.11"),
    ("chr11", "NC_000011.10"),
    ("chr12", "NC_000012.12"),
    ("chr13", "NC_000013.11"),
    ("chr14", "NC_000014.9"),
    ("chr15", "NC_000015.10"),
    ("chr16", "NC_000016.10"),
    ("chr17", "NC_000017.11"),
    ("chr18", "NC_000018.10"),
    ("chr19", "NC_000019.10"),
    ("chr20", "NC_000020.11"),
    ("chr21", "NC_000021.9"),
    ("chr22", "NC_000022.11"),
    ("chrX", "NC_000023.11"),
    ("chrY", "NC_000024.10"),
    ("chrM", "NC_012920.1"),
];
static STAGING_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
pub struct CommandError {
    pub status: &'static str,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: "error",
            code,
            message: message.into(),
            details: None,
        }
    }

    fn mismatch(mismatch_count: u64, examples: Vec<MismatchExample>) -> Self {
        Self {
            status: "error",
            code: "REFERENCE_MISMATCH",
            message: format!(
                "{mismatch_count} ordinary source references disagree with GRCh38.p14"
            ),
            details: serde_json::to_value(MismatchDetails {
                mismatch_count,
                examples,
            })
            .ok(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

#[derive(Clone, Debug, Serialize)]
struct MismatchDetails {
    mismatch_count: u64,
    examples: Vec<MismatchExample>,
}

#[derive(Clone, Debug, Serialize)]
struct MismatchExample {
    gene: String,
    contig: String,
    pos: u64,
    expected: String,
    observed: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildOutcome {
    pub status: &'static str,
    pub bundle_id: String,
    #[serde(flatten)]
    pub counts: BundleCounts,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifyOutcome {
    pub status: &'static str,
    pub bundle_id: String,
    pub members_verified: u64,
}

struct StageGuard {
    path: PathBuf,
    armed: bool,
}

impl StageGuard {
    fn mark_published(&mut self) {
        self.armed = false;
    }

    fn cleanup(&mut self) -> Result<(), CommandError> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        fs::remove_dir_all(&self.path).map_err(|error| io_error("remove staging directory", error))
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        // Panic/unwind fallback. Every ordinary return explicitly calls
        // `cleanup`, so a handled cleanup failure can be reported to the CLI.
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Copy)]
struct ReferenceEntry {
    offset: u64,
    length: u64,
}

struct ReferencePrepared {
    entries: BTreeMap<String, ReferenceEntry>,
    aliases: Vec<AliasManifest>,
    input_compression: String,
    input_size: u64,
    input_sha256: String,
    sequence_set_sha256: String,
    extra_record_count: u64,
    extra_accessions_sha256: String,
}

struct HashingReader<R> {
    inner: R,
    hash: Sha256,
    bytes: u64,
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hash: Sha256::new(),
            bytes: 0,
        }
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.hash.update(&buffer[..read]);
        self.bytes = self
            .bytes
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("reference input byte count overflow"))?;
        Ok(read)
    }
}

struct HashWriter(Sha256);

impl HashWriter {
    fn new() -> Self {
        Self(Sha256::new())
    }

    fn finish(self) -> String {
        format!("sha256:{:x}", self.0.finalize())
    }
}

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn build_bundle(
    source_dir: &Path,
    reference: &Path,
    output: &Path,
) -> Result<BuildOutcome, CommandError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error("create output parent", error))?;
    let output_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CommandError::new("UNSUPPORTED_INPUT", "output must have a UTF-8 file name")
        })?;
    let serial = STAGING_SERIAL.fetch_add(1, Ordering::Relaxed);
    let staging = parent.join(format!(
        ".{output_name}.pangopup-stage-{}-{serial}",
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(|error| io_error("create staging directory", error))?;
    let mut guard = StageGuard {
        path: staging.clone(),
        armed: true,
    };
    let result = build_staged(source_dir, reference, output, parent, &staging, &mut guard);
    let cleanup = guard.cleanup();
    match (result, cleanup) {
        (_, Err(cleanup_error)) => Err(cleanup_error),
        (result, Ok(())) => result,
    }
}

fn build_staged(
    source_dir: &Path,
    reference: &Path,
    output: &Path,
    parent: &Path,
    staging: &Path,
    guard: &mut StageGuard,
) -> Result<BuildOutcome, CommandError> {
    let reference_scratch = staging.join("reference.scratch");
    let prepared = prepare_reference(reference, &reference_scratch)?;
    let payload_scratch = staging.join("payload.scratch");
    let mut writer = StreamingIndexWriter::create(&payload_scratch)
        .map_err(|error| CommandError::new("IO", error.to_string()))?;
    let mut members = discover_members(source_dir)
        .map_err(|error| CommandError::new("SOURCE_INPUT", error.to_string()))?;
    members.sort_by(|left, right| left.0.cmp(&right.0));
    if members.is_empty() {
        return Err(CommandError::new(
            "SOURCE_EMPTY",
            "source directory contains no members",
        ));
    }
    let mut source_member_hash = Sha256::new();
    let mut total = TotalSummary::default();
    let mut logical_source = HashWriter::new();
    let mut logical_records = 0_u64;
    let mut mismatch_count = 0_u64;
    let mut mismatch_examples = Vec::new();
    let mut reference_file = File::open(&reference_scratch)
        .map_err(|error| io_error("open reference scratch", error))?;
    for (name, path) in members {
        let name = name
            .into_string()
            .map_err(|_| CommandError::new("SOURCE_MEMBER", "source member name is not UTF-8"))?;
        let gene = parse_member_gene(&name)
            .map_err(|error| CommandError::new("SOURCE_MEMBER", error.to_string()))?;
        let mut loci = Vec::new();
        let summary = inspect_member_hashed(
            &path,
            &name,
            gene,
            &mut |source: &SourceLocus| {
                loci.push(index_locus(*source));
                Ok::<_, Infallible>(())
            },
            &mut source_member_hash,
        )
        .map_err(|error| CommandError::new("SOURCE_INVALID", error.to_string()))?;
        total.add(summary);
        loci.sort_by_key(|locus| match locus {
            InputLocus::Ordinary(value) => (value.contig.code(), value.position.get(), 0_u8),
            InputLocus::Ambiguous(value) => (value.contig.code(), value.position.get(), 1_u8),
        });
        certify_gene(
            &loci,
            &prepared.entries,
            &mut reference_file,
            &mut mismatch_count,
            &mut mismatch_examples,
        )?;
        for locus in &loci {
            write_logical_text(&mut logical_source, *locus)
                .map_err(|error| io_error("hash logical source", error))?;
            logical_records = logical_records.checked_add(3).ok_or_else(|| {
                CommandError::new("SOURCE_COUNT", "logical record count overflow")
            })?;
        }
        writer
            .push_gene(&loci)
            .map_err(|error| CommandError::new("SOURCE_INDEX", error.to_string()))?;
    }
    if mismatch_count != 0 {
        mismatch_examples.sort_by(|left, right| {
            (
                &left.gene,
                &left.contig,
                left.pos,
                &left.expected,
                &left.observed,
            )
                .cmp(&(
                    &right.gene,
                    &right.contig,
                    right.pos,
                    &right.expected,
                    &right.observed,
                ))
        });
        mismatch_examples.truncate(20);
        return Err(CommandError::mismatch(mismatch_count, mismatch_examples));
    }
    let scores = staging.join("scores.pgi");
    let write_summary = writer
        .finish(&scores)
        .map_err(|error| CommandError::new("IO", error.to_string()))?;
    fs::remove_file(&payload_scratch).map_err(|error| io_error("remove payload scratch", error))?;
    fs::remove_file(&reference_scratch)
        .map_err(|error| io_error("remove reference scratch", error))?;
    let source_logical = LogicalManifest {
        records: logical_records,
        sha256: logical_source.finish(),
    };
    let decoded_logical = decode_logical(&scores)?;
    if decoded_logical != source_logical {
        return Err(CommandError::new(
            "BUNDLE_LOGICAL_MISMATCH",
            "decoded index does not match the canonical source stream",
        ));
    }
    let counts = BundleCounts {
        genes: total.genes,
        source_rows: total.rows,
        gene_loci: total.loci,
        ascending_members: total.ascending,
        descending_members: total.descending,
        source_segments: total.segments,
        index_segments: write_summary.segments,
        gap_transitions: total.gaps,
        omitted_bases: total.omitted_bases,
        n_ref_loci: total.ambiguous_ref_loci,
        n_omit_a: total.n_omit_a,
        n_omit_t: total.n_omit_t,
    };
    let notice_path = staging.join("NOTICE");
    write_synced(&notice_path, NOTICE)?;
    let member_manifests = vec![
        member_manifest(&notice_path, "NOTICE", "text/plain; charset=utf-8")?,
        member_manifest(&scores, "scores.pgi", "application/vnd.pangopup.fixed11")?,
    ];
    let manifest = BundleManifest {
        schema: "pangopup.bundle.v1".to_owned(),
        index_format: "pangopup.fixed11.v1".to_owned(),
        builder: BuilderManifest {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            source_sha256: format!("sha256:{}", env!("PANGOPUP_BUILDER_SOURCE_SHA256")),
        },
        source: SourceManifest {
            title: "Pangolin precomputed scores".to_owned(),
            creators: vec!["Nils Wagner".to_owned(), "Aleksandr Neverov".to_owned()],
            doi: "10.5281/zenodo.15649338".to_owned(),
            archive_name: "Pangolin_hg38_snvs_masked.zip".to_owned(),
            published_archive_size: 12_988_141_317,
            published_archive_md5: "md5:679ef0b50e511b6102b4b88fbf811108".to_owned(),
            observed_member_count: total.genes,
            observed_members_sha256: format!("sha256:{:x}", source_member_hash.finalize()),
            masked: true,
            window: 50,
        },
        reference: ReferenceManifest {
            assembly: "GRCh38.p14".to_owned(),
            assembly_accession: "GCF_000001405.40".to_owned(),
            input_compression: prepared.input_compression,
            input_size: prepared.input_size,
            input_sha256: prepared.input_sha256,
            sequence_set_sha256: prepared.sequence_set_sha256,
            aliases: prepared.aliases,
            extra_record_count: prepared.extra_record_count,
            extra_accessions_sha256: prepared.extra_accessions_sha256,
        },
        counts,
        logical_source: source_logical,
        logical_decoded: decoded_logical,
        members: member_manifests,
        attribution: AttributionManifest {
            notice_path: "NOTICE".to_owned(),
            license: "CC-BY-4.0".to_owned(),
            transformed: true,
        },
    };
    let manifest_bytes = canonical_manifest_bytes(&manifest)
        .map_err(|error| CommandError::new("BUNDLE_MANIFEST", error.to_string()))?;
    write_synced(&staging.join("manifest.json"), &manifest_bytes)?;
    sync_directory(staging)?;
    let staged = verify_bundle(staging)?;
    if fs::symlink_metadata(output).is_ok() {
        let existing = verify_bundle(output).map_err(|error| {
            CommandError::new(
                "PUBLICATION_DESTINATION",
                format!("existing destination is invalid and was left untouched: {error}"),
            )
        })?;
        if existing.bundle_id != staged.bundle_id {
            return Err(CommandError::new(
                "PUBLICATION_DESTINATION",
                "existing destination has a different bundle identity",
            ));
        }
        return Ok(BuildOutcome {
            status: "already_present",
            bundle_id: existing.bundle_id,
            counts,
        });
    }
    match rename_noreplace(staging, output) {
        Ok(()) => {
            guard.mark_published();
            sync_directory(parent)?;
            Ok(BuildOutcome {
                status: "built",
                bundle_id: staged.bundle_id,
                counts,
            })
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
            ) =>
        {
            let existing = verify_bundle(output)?;
            if existing.bundle_id != staged.bundle_id {
                return Err(CommandError::new(
                    "PUBLICATION_DESTINATION",
                    "concurrent publication produced a different bundle identity",
                ));
            }
            Ok(BuildOutcome {
                status: "already_present",
                bundle_id: existing.bundle_id,
                counts,
            })
        }
        Err(error) => Err(CommandError::new("PUBLICATION", error.to_string())),
    }
}

pub fn verify_bundle(path: &Path) -> Result<VerifyOutcome, CommandError> {
    let opened = BundleOpen::open(path)
        .map_err(|error| CommandError::new("BUNDLE_INVALID", error.to_string()))?;
    for member in &opened.manifest.members {
        let actual = hash_file(&path.join(&member.path))?;
        if actual != member.sha256 {
            return Err(CommandError::new(
                "BUNDLE_MEMBER_HASH",
                format!("bundle member {} has the wrong SHA-256", member.path),
            ));
        }
    }
    if fs::read(path.join("NOTICE")).map_err(|error| io_error("read NOTICE", error))? != NOTICE {
        return Err(CommandError::new(
            "BUNDLE_NOTICE",
            "NOTICE does not match the byte-exact notice embedded in the builder",
        ));
    }
    opened
        .index
        .verify_canonical_structure()
        .map_err(|error| CommandError::new("BUNDLE_INDEX", error.to_string()))?;
    let decoded = decode_reader(&opened.index)?;
    if decoded.logical != opened.manifest.logical_decoded
        || opened.manifest.logical_source != opened.manifest.logical_decoded
    {
        return Err(CommandError::new(
            "BUNDLE_LOGICAL_MISMATCH",
            "complete decoded logical stream does not match the manifest",
        ));
    }
    let counts = opened.manifest.counts;
    let direction_members = counts
        .ascending_members
        .checked_add(counts.descending_members)
        .ok_or_else(bundle_counts_overflow)?;
    let exception_shapes = counts
        .n_omit_a
        .checked_add(counts.n_omit_t)
        .ok_or_else(bundle_counts_overflow)?;
    let expected_rows = counts
        .gene_loci
        .checked_mul(3)
        .ok_or_else(bundle_counts_overflow)?;
    if counts.source_rows != decoded.logical.records
        || expected_rows != counts.source_rows
        || counts.gene_loci != decoded.loci
        || counts.genes != decoded.genes
        // Direction is source-only provenance. Offline verification can prove
        // its checked total, but not independently recover the split from the
        // canonical ascending fixed-v1 representation.
        || counts.genes != direction_members
        || opened.manifest.source.observed_member_count != counts.genes
        || counts.source_segments != decoded.source_segments
        || counts.gap_transitions != decoded.gaps
        || counts.omitted_bases != decoded.omitted_bases
        || counts.index_segments != decoded.index_segments
        || decoded.index_segments != opened.index.segment_count()
        || counts.n_ref_loci != opened.index.exception_count()
        || counts.n_ref_loci != decoded.n_ref_loci
        || counts.n_omit_a != decoded.n_omit_a
        || counts.n_omit_t != decoded.n_omit_t
        || exception_shapes != counts.n_ref_loci
    {
        return Err(CommandError::new(
            "BUNDLE_COUNTS",
            "manifest counts do not agree with the complete index decode",
        ));
    }
    Ok(VerifyOutcome {
        status: "verified",
        bundle_id: opened.bundle_id,
        members_verified: 2,
    })
}

fn bundle_counts_overflow() -> CommandError {
    CommandError::new(
        "BUNDLE_COUNTS",
        "manifest count arithmetic overflowed during complete verification",
    )
}

fn decode_logical(path: &Path) -> Result<LogicalManifest, CommandError> {
    let reader = IndexReader::open(path)
        .map_err(|error| CommandError::new("BUNDLE_INDEX", error.to_string()))?;
    decode_reader(&reader).map(|decoded| decoded.logical)
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

fn decode_reader(reader: &IndexReader) -> Result<DecodedFacts, CommandError> {
    let mut hash = HashWriter::new();
    let mut records = 0_u64;
    let mut genes = 0_u64;
    let mut loci_count = 0_u64;
    let mut source_segments = 0_u64;
    let mut index_segments = 0_u64;
    let mut gaps = 0_u64;
    let mut omitted_bases = 0_u64;
    let mut n_ref_loci = 0_u64;
    let mut n_omit_a = 0_u64;
    let mut n_omit_t = 0_u64;
    let mut previous: Option<(u64, u8, u32)> = None;
    let mut previous_ordinary: Option<(u64, u8, u32)> = None;
    reader
        .visit_all(|locus| {
            write_logical_text(&mut hash, locus)?;
            add_decoded(&mut records, 3)?;
            add_decoded(&mut loci_count, 1)?;
            let (gene, contig, position) = match locus {
                InputLocus::Ordinary(value) => {
                    let ordinary = (
                        value.gene.numeric(),
                        value.contig.code(),
                        value.position.get(),
                    );
                    if previous_ordinary.is_none_or(|previous| {
                        previous.0 != ordinary.0
                            || previous.1 != ordinary.1
                            || previous.2.checked_add(1) != Some(ordinary.2)
                    }) {
                        add_decoded(&mut index_segments, 1)?;
                    }
                    previous_ordinary = Some(ordinary);
                    ordinary
                }
                InputLocus::Ambiguous(value) => {
                    add_decoded(&mut n_ref_loci, 1)?;
                    match value.omitted {
                        pangopup_core::DnaBase::A => add_decoded(&mut n_omit_a, 1)?,
                        pangopup_core::DnaBase::T => add_decoded(&mut n_omit_t, 1)?,
                        _ => {
                            return Err(io::Error::new(
                                ErrorKind::InvalidData,
                                "invalid omitted exception base",
                            ));
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
                    add_decoded(&mut genes, 1)?;
                    add_decoded(&mut source_segments, 1)?;
                }
                Some((previous_gene, _, _)) if previous_gene != gene => {
                    add_decoded(&mut genes, 1)?;
                    add_decoded(&mut source_segments, 1)?;
                }
                Some((_, previous_contig, previous_position)) => {
                    if previous_contig != contig || position <= previous_position {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "decoded logical order",
                        ));
                    }
                    let distance = u64::from(position - previous_position);
                    if distance > 1 {
                        add_decoded(&mut gaps, 1)?;
                        add_decoded(&mut omitted_bases, distance - 1)?;
                        add_decoded(&mut source_segments, 1)?;
                    }
                }
            }
            previous = Some((gene, contig, position));
            Ok::<_, io::Error>(())
        })
        .map_err(|error| match error {
            VisitAllError::Index(error) => CommandError::new("BUNDLE_INDEX", error.to_string()),
            VisitAllError::Visitor(error) => io_error("decode logical stream", error),
        })?;
    Ok(DecodedFacts {
        logical: LogicalManifest {
            records,
            sha256: hash.finish(),
        },
        genes,
        loci: loci_count,
        source_segments,
        index_segments,
        gaps,
        omitted_bases,
        n_ref_loci,
        n_omit_a,
        n_omit_t,
    })
}

fn add_decoded(target: &mut u64, value: u64) -> io::Result<()> {
    *target = target
        .checked_add(value)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "decoded count overflow"))?;
    Ok(())
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

fn certify_gene(
    loci: &[InputLocus],
    references: &BTreeMap<String, ReferenceEntry>,
    reference: &mut File,
    mismatch_count: &mut u64,
    examples: &mut Vec<MismatchExample>,
) -> Result<(), CommandError> {
    let Some((contig, first, last)) = loci
        .iter()
        .filter_map(|locus| match locus {
            InputLocus::Ordinary(value) => {
                Some((value.contig, value.position.get(), value.position.get()))
            }
            InputLocus::Ambiguous(_) => None,
        })
        .fold(None, |state, current| match state {
            None => Some(current),
            Some((contig, first, last)) => {
                Some((contig, first.min(current.1), last.max(current.2)))
            }
        })
    else {
        return Ok(());
    };
    let accession = REQUIRED[usize::from(contig.code() - 1)].1;
    let entry = references
        .get(accession)
        .ok_or_else(|| CommandError::new("REFERENCE_MISSING_ACCESSION", accession))?;
    if u64::from(last) > entry.length {
        return Err(CommandError::new(
            "REFERENCE_RANGE",
            format!("{contig}:{last} is outside {accession}"),
        ));
    }
    let length = usize::try_from(u64::from(last - first) + 1)
        .map_err(|_| CommandError::new("REFERENCE_RANGE", "gene reference span is too large"))?;
    let mut sequence = vec![0_u8; length];
    reference
        .seek(SeekFrom::Start(entry.offset + u64::from(first - 1)))
        .and_then(|_| reference.read_exact(&mut sequence))
        .map_err(|error| io_error("read reference scratch", error))?;
    for locus in loci {
        let InputLocus::Ordinary(value) = locus else {
            continue;
        };
        let expected = value.reference.to_string();
        let observed = char::from(sequence[(value.position.get() - first) as usize]).to_string();
        if expected != observed {
            *mismatch_count += 1;
            if examples.len() < 20 {
                examples.push(MismatchExample {
                    gene: value.gene.to_string(),
                    contig: value.contig.to_string(),
                    pos: u64::from(value.position.get()),
                    expected,
                    observed,
                });
            }
        }
    }
    Ok(())
}

fn prepare_reference(input: &Path, scratch: &Path) -> Result<ReferencePrepared, CommandError> {
    let file = File::open(input).map_err(|error| io_error("open reference", error))?;
    let expected_size = file
        .metadata()
        .map_err(|error| io_error("stat opened reference", error))?
        .len();
    let hashing = HashingReader::new(file);
    let mut buffered = BufReader::new(hashing);
    let prefix = buffered
        .fill_buf()
        .map_err(|error| io_error("read reference prefix", error))?;
    if prefix.len() < 2 {
        return Err(CommandError::new(
            "REFERENCE_FASTA",
            "reference is too short to contain FASTA",
        ));
    }
    let gzip = prefix.starts_with(&[0x1f, 0x8b]);
    if gzip {
        let decoder = GzDecoder::new(buffered);
        let mut decoded = BufReader::new(decoder);
        let parsed = parse_fasta(&mut decoded, scratch)?;
        let decoder = decoded.into_inner();
        let mut buffered = decoder.into_inner();
        if !buffered
            .fill_buf()
            .map_err(|error| io_error("read gzip trailer", error))?
            .is_empty()
        {
            return Err(CommandError::new(
                "REFERENCE_GZIP",
                "reference must be one ordinary gzip member with no trailing bytes",
            ));
        }
        let hashing = buffered.into_inner();
        finish_reference(parsed, hashing, expected_size, "gzip")
    } else {
        let mut buffered = buffered;
        let parsed = parse_fasta(&mut buffered, scratch)?;
        let hashing = buffered.into_inner();
        finish_reference(parsed, hashing, expected_size, "none")
    }
}

struct ParsedReference {
    entries: BTreeMap<String, ReferenceEntry>,
    extras: Vec<String>,
    scratch_path: PathBuf,
}

fn parse_fasta(
    reader: &mut dyn BufRead,
    scratch_path: &Path,
) -> Result<ParsedReference, CommandError> {
    let required: BTreeSet<_> = REQUIRED.iter().map(|entry| entry.1).collect();
    let mut scratch =
        File::create(scratch_path).map_err(|error| io_error("create reference scratch", error))?;
    let mut entries = BTreeMap::new();
    let mut extras = Vec::new();
    let mut current: Option<(String, bool, u64, u64)> = None;
    let mut line = Vec::new();
    let mut scratch_offset = 0_u64;
    loop {
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|error| io_error("read reference FASTA", error))?;
        if read == 0 {
            break;
        }
        let has_newline = line.ends_with(b"\n");
        let has_crlf = line.ends_with(b"\r\n");
        let allowed_cr = has_crlf.then_some(line.len() - 2);
        if line
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\r' && Some(index) != allowed_cr)
        {
            return Err(CommandError::new(
                "REFERENCE_FASTA",
                "bare carriage return is not a permitted FASTA line ending",
            ));
        }
        if has_newline {
            line.pop();
        }
        if has_crlf {
            line.pop();
        }
        if line.first() == Some(&b'>') {
            finalize_reference_record(&mut entries, current.take())?;
            let header = std::str::from_utf8(&line[1..])
                .map_err(|_| CommandError::new("REFERENCE_FASTA", "FASTA header is not UTF-8"))?;
            let accession = header.split_ascii_whitespace().next().unwrap_or("");
            if accession.is_empty() {
                return Err(CommandError::new(
                    "REFERENCE_FASTA",
                    "FASTA header has no accession",
                ));
            }
            let wanted = required.contains(accession);
            if wanted && entries.contains_key(accession) {
                return Err(CommandError::new(
                    "REFERENCE_DUPLICATE_ACCESSION",
                    accession,
                ));
            }
            if !wanted {
                extras.push(accession.to_owned());
            }
            current = Some((accession.to_owned(), wanted, scratch_offset, 0));
            continue;
        }
        let Some((_, wanted, _, length)) = current.as_mut() else {
            return Err(CommandError::new(
                "REFERENCE_FASTA",
                "sequence precedes first FASTA header",
            ));
        };
        if line.is_empty() {
            return Err(CommandError::new(
                "REFERENCE_FASTA",
                "FASTA sequence lines must be nonempty",
            ));
        }
        for byte in &mut line {
            *byte = byte.to_ascii_uppercase();
            if !matches!(
                *byte,
                b'A' | b'C'
                    | b'G'
                    | b'T'
                    | b'R'
                    | b'Y'
                    | b'S'
                    | b'W'
                    | b'K'
                    | b'M'
                    | b'B'
                    | b'D'
                    | b'H'
                    | b'V'
                    | b'N'
            ) {
                return Err(CommandError::new(
                    "REFERENCE_INVALID_SEQUENCE",
                    format!("invalid FASTA sequence byte 0x{byte:02x}"),
                ));
            }
        }
        *length = length
            .checked_add(line.len() as u64)
            .ok_or_else(|| CommandError::new("REFERENCE_FASTA", "reference length overflow"))?;
        if *wanted {
            scratch
                .write_all(&line)
                .map_err(|error| io_error("write reference scratch", error))?;
            scratch_offset = scratch_offset
                .checked_add(line.len() as u64)
                .ok_or_else(|| CommandError::new("REFERENCE_FASTA", "scratch offset overflow"))?;
        }
    }
    finalize_reference_record(&mut entries, current.take())?;
    scratch
        .sync_all()
        .map_err(|error| io_error("sync reference scratch", error))?;
    for (_, accession) in REQUIRED {
        if !entries.contains_key(accession) {
            return Err(CommandError::new("REFERENCE_MISSING_ACCESSION", accession));
        }
    }
    Ok(ParsedReference {
        entries,
        extras,
        scratch_path: scratch_path.to_owned(),
    })
}

fn finalize_reference_record(
    entries: &mut BTreeMap<String, ReferenceEntry>,
    current: Option<(String, bool, u64, u64)>,
) -> Result<(), CommandError> {
    let Some((accession, wanted, offset, length)) = current else {
        return Ok(());
    };
    if length == 0 {
        return Err(CommandError::new(
            "REFERENCE_FASTA",
            format!("{accession} has an empty sequence"),
        ));
    }
    if wanted
        && entries
            .insert(accession.clone(), ReferenceEntry { offset, length })
            .is_some()
    {
        return Err(CommandError::new(
            "REFERENCE_DUPLICATE_ACCESSION",
            accession,
        ));
    }
    Ok(())
}

fn finish_reference(
    parsed: ParsedReference,
    hashing: HashingReader<File>,
    expected_size: u64,
    compression: &str,
) -> Result<ReferencePrepared, CommandError> {
    if hashing.bytes != expected_size {
        return Err(CommandError::new(
            "REFERENCE_INPUT",
            "reference changed length while it was being read",
        ));
    }
    let metadata_size = fs::metadata(&parsed.scratch_path)
        .map_err(|error| io_error("stat reference scratch", error))?
        .len();
    let final_end = parsed
        .entries
        .values()
        .map(|value| {
            value
                .offset
                .checked_add(value.length)
                .ok_or_else(|| CommandError::new("REFERENCE_FASTA", "scratch range overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    if final_end != metadata_size {
        return Err(CommandError::new(
            "REFERENCE_FASTA",
            "reference scratch accounting mismatch",
        ));
    }
    let mut scratch = File::open(&parsed.scratch_path)
        .map_err(|error| io_error("open reference scratch", error))?;
    let mut sequence_hash = Sha256::new();
    let mut aliases = Vec::with_capacity(25);
    let mut buffer = vec![0_u8; 64 * 1024];
    for (contig, accession) in REQUIRED {
        let entry = parsed.entries[accession];
        sequence_hash.update((accession.len() as u64).to_le_bytes());
        sequence_hash.update(accession.as_bytes());
        sequence_hash.update(entry.length.to_le_bytes());
        scratch
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|error| io_error("seek reference scratch", error))?;
        let mut remaining = entry.length;
        while remaining != 0 {
            let take = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| CommandError::new("REFERENCE_FASTA", "reference read size"))?;
            scratch
                .read_exact(&mut buffer[..take])
                .map_err(|error| io_error("hash reference sequence", error))?;
            sequence_hash.update(&buffer[..take]);
            remaining -= take as u64;
        }
        aliases.push(AliasManifest {
            contig: contig.to_owned(),
            accession: accession.to_owned(),
            length: entry.length,
        });
    }
    let mut extras = parsed.extras;
    extras.sort();
    let mut extras_hash = Sha256::new();
    for name in &extras {
        extras_hash.update((name.len() as u64).to_le_bytes());
        extras_hash.update(name.as_bytes());
    }
    Ok(ReferencePrepared {
        entries: parsed.entries,
        aliases,
        input_compression: compression.to_owned(),
        input_size: hashing.bytes,
        input_sha256: format!("sha256:{:x}", hashing.hash.finalize()),
        sequence_set_sha256: format!("sha256:{:x}", sequence_hash.finalize()),
        extra_record_count: extras.len() as u64,
        extra_accessions_sha256: format!("sha256:{:x}", extras_hash.finalize()),
    })
}

fn member_manifest(
    path: &Path,
    name: &str,
    media_type: &str,
) -> Result<MemberManifest, CommandError> {
    Ok(MemberManifest {
        path: name.to_owned(),
        size: fs::metadata(path)
            .map_err(|error| io_error("stat bundle member", error))?
            .len(),
        sha256: hash_file(path)?,
        media_type: media_type.to_owned(),
    })
}

fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file = File::open(path).map_err(|error| io_error("open bundle member", error))?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error("hash bundle member", error))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = File::create(path).map_err(|error| io_error("create bundle member", error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| io_error("write bundle member", error))
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            CommandError::new("PUBLICATION", format!("directory sync failed: {error}"))
        })
}

#[cfg(target_os = "linux")]
fn rename_noreplace(source: &Path, destination: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}

#[cfg(not(target_os = "linux"))]
fn rename_noreplace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-replace directory publication is unsupported on this target",
    ))
}

fn io_error(action: &str, error: io::Error) -> CommandError {
    CommandError::new("IO", format!("{action}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handled_staging_cleanup_failure_is_surfaced_and_not_hidden_by_drop() {
        let path = std::env::temp_dir().join(format!(
            "pangopup-cleanup-failure-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("temporary staging directory");
        let mut guard = StageGuard {
            path: path.clone(),
            armed: true,
        };
        fs::remove_dir(&path).expect("replace staging directory");
        fs::write(&path, b"not a directory").expect("replacement file");
        let error = guard.cleanup().expect_err("cleanup failure must surface");
        assert_eq!(error.code, "IO");
        assert!(error.message.contains("remove staging directory"));
        assert!(!guard.armed, "Drop is only an unwind fallback");
        drop(guard);
        assert!(
            path.is_file(),
            "Drop must not hide a handled cleanup failure"
        );
        fs::remove_file(path).expect("remove replacement file");
    }
}
