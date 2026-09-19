//! Maintainer-only certified fixed-v1 to sparse-candidate conversion.

use crate::CommandError;
use pangopup_assets::certify_bundle_members_with_gene_limits;
use pangopup_index::{
    DecodedSummary, InputLocus, VisitAllError,
    sparse_reader::SparseIndexReader,
    sparse_writer::{SPARSE_INDEX_FORMAT, SparseIndexWriter, SparseWriteSummary},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_GENE_LOCI: u64 = 3_000_000;
pub const MAX_GENE_CAPACITY_BYTES: u64 = 512 * 1024 * 1024;
pub const ADR_0027_SIZE_GATE_BYTES: u64 = 3_221_225_472;

static STAGE_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct SparseCandidateArguments {
    pub fixed_bundle: PathBuf,
    pub scratch: PathBuf,
    pub candidate: PathBuf,
    pub report: PathBuf,
    pub expected_bundle_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateCounts {
    pub genes: u64,
    pub loci: u64,
    pub ordinary_loci: u64,
    pub segments: u64,
    pub blocks: u64,
    pub exceptions: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedCounts {
    pub genes: u64,
    pub loci: u64,
    pub ordinary_loci: u64,
    pub segments: u64,
    pub exceptions: u64,
    pub records: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneBufferMeasurements {
    pub maximum_length: u64,
    pub maximum_capacity: u64,
    pub maximum_capacity_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseCandidateReport {
    pub schema: String,
    pub input_bundle_id: String,
    pub fixed_format: String,
    pub fixed_member_bytes: u64,
    pub fixed_member_sha256: String,
    pub candidate_format: String,
    pub candidate_bytes: u64,
    pub candidate_sha256: String,
    pub writer_counts: CandidateCounts,
    pub decoded_counts: DecodedCounts,
    pub logical_source_records: u64,
    pub logical_source_sha256: String,
    pub logical_decoded_records: u64,
    pub logical_decoded_sha256: String,
    pub gene_buffer: GeneBufferMeasurements,
    pub adr_0027_size_gate_bytes: u64,
    pub adr_0027_size_gate_passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseCandidateOutcome {
    pub status: &'static str,
    pub candidate_sha256: String,
    pub candidate_bytes: u64,
    pub report_schema: &'static str,
    pub size_gate_passed: bool,
}

/// Convert one exhaustively certified fixed-v1 bundle into a candidate file.
///
/// Every input member must remain immutable for this complete call. The
/// candidate is copied only from its held verified descriptor into a
/// create-new destination. The create-new report is the completion marker.
pub fn build_sparse_candidate(
    arguments: &SparseCandidateArguments,
) -> Result<SparseCandidateOutcome, CommandError> {
    build_sparse_candidate_inner(arguments, Limits::production(), &mut Hooks::default())
}

#[derive(Clone, Copy)]
struct Limits {
    gene_loci: u64,
    gene_capacity_bytes: u64,
}

impl Limits {
    const fn production() -> Self {
        Self {
            gene_loci: MAX_GENE_LOCI,
            gene_capacity_bytes: MAX_GENE_CAPACITY_BYTES,
        }
    }
}

fn build_sparse_candidate_inner(
    arguments: &SparseCandidateArguments,
    limits: Limits,
    hooks: &mut Hooks,
) -> Result<SparseCandidateOutcome, CommandError> {
    for (label, path) in [
        ("scratch", &arguments.scratch),
        ("candidate", &arguments.candidate),
        ("report", &arguments.report),
    ] {
        require_absent(label, path)?;
    }
    validate_bundle_member_set(&arguments.fixed_bundle)?;
    let manifest = open_regular(&arguments.fixed_bundle.join("manifest.json"), "manifest")?;
    let notice = open_regular(&arguments.fixed_bundle.join("NOTICE"), "NOTICE")?;
    let scores = open_regular(&arguments.fixed_bundle.join("scores.pgi"), "scores.pgi")?;
    let input_identities = [identity(&manifest)?, identity(&notice)?, identity(&scores)?];
    hooks.after_input_open(arguments)?;

    let certified = certify_bundle_members_with_gene_limits(
        &manifest,
        &notice,
        &scores,
        limits.gene_loci,
        limits.gene_capacity_bytes,
    )
    .map_err(|error| {
        CommandError::new(
            error.legacy_build_code().unwrap_or(error.kind().code()),
            error.to_string(),
        )
    })?;
    if let Some(expected) = &arguments.expected_bundle_id
        && expected != &certified.certification().bundle_id
    {
        return Err(CommandError::new(
            "SPARSE_INPUT_IDENTITY",
            "fixed bundle identity does not match --expected-bundle-id",
        ));
    }

    let candidate_stage = unique_stage_path(&arguments.scratch, "candidate");

    let mut writer = SparseIndexWriter::create(&arguments.scratch)
        .map_err(|error| command_error("SPARSE_WRITE", error))?;
    let fixed_index = certified
        .index()
        .map_err(|error| command_error("SPARSE_FIXED_TRAVERSAL", error))?;
    let traversal = fixed_index
        .visit_genes_bounded(limits.gene_loci, limits.gene_capacity_bytes, |gene| {
            writer.push_gene(gene)
        })
        .map_err(|error| visit_error("SPARSE_FIXED_TRAVERSAL", error))?;
    let mut held_stage = writer
        .finish_held(&candidate_stage)
        .map_err(|error| command_error("SPARSE_WRITE", error))?;
    let writer_summary = held_stage.summary();
    let stage_identity = match hooks.capture_stage_identity(held_stage.file()) {
        Ok(identity) => identity,
        Err(original) => {
            hooks.before_stage_capture_cleanup(&candidate_stage);
            return Err(match held_stage.cleanup() {
                Ok(()) => original,
                Err(cleanup) => combine_cleanup_error(
                    original,
                    command_error("SPARSE_PUBLICATION_CLEANUP", cleanup),
                ),
            });
        }
    };
    let (_, stage_file) = held_stage.into_parts();
    let mut stage = OwnedFile::from_parts(candidate_stage.clone(), stage_identity, stage_file);
    let prepared = (|| -> Result<_, CommandError> {
        hooks.after_candidate_open(&candidate_stage)?;
        let candidate_sha256 = hash_file(&stage.file)?;
        let candidate_reader = SparseIndexReader::open_file(&stage.file)
            .map_err(|error| command_error("SPARSE_CANDIDATE", error))?;
        let (decoded_summary, decoded_records, decoded_sha256) =
            decode_candidate(&candidate_reader)?;

        validate_parity(
            certified.manifest(),
            writer_summary,
            decoded_summary,
            decoded_records,
            &decoded_sha256,
        )?;
        validate_held_input(&manifest, input_identities[0], "manifest")?;
        validate_held_input(&notice, input_identities[1], "NOTICE")?;
        validate_held_input(&scores, input_identities[2], "scores.pgi")?;

        let writer_counts = CandidateCounts::from(writer_summary);
        let decoded_counts = DecodedCounts::from_summary(decoded_summary, decoded_records);
        let report = SparseCandidateReport {
            schema: "pangopup.sparse-candidate-report.v1".to_owned(),
            input_bundle_id: certified.certification().bundle_id.clone(),
            fixed_format: certified.manifest().index_format.clone(),
            fixed_member_bytes: certified.fixed_member_size(),
            fixed_member_sha256: certified.fixed_member_sha256().to_owned(),
            candidate_format: SPARSE_INDEX_FORMAT.to_owned(),
            candidate_bytes: writer_summary.bytes,
            candidate_sha256: candidate_sha256.clone(),
            writer_counts,
            decoded_counts,
            logical_source_records: certified.manifest().logical_source.records,
            logical_source_sha256: certified.manifest().logical_source.sha256.clone(),
            logical_decoded_records: decoded_records,
            logical_decoded_sha256: decoded_sha256,
            gene_buffer: GeneBufferMeasurements {
                maximum_length: traversal.maximum_buffered_gene_loci,
                maximum_capacity: traversal.maximum_buffered_gene_capacity,
                maximum_capacity_bytes: traversal.maximum_buffered_gene_capacity_bytes,
            },
            adr_0027_size_gate_bytes: ADR_0027_SIZE_GATE_BYTES,
            adr_0027_size_gate_passed: writer_summary.bytes <= ADR_0027_SIZE_GATE_BYTES,
        };
        let report_bytes = serde_jcs::to_vec(&report)
            .map_err(|error| CommandError::new("SPARSE_REPORT", error.to_string()))?;
        Ok((candidate_sha256, report, report_bytes))
    })();
    let (candidate_sha256, report, report_bytes) = match prepared {
        Ok(prepared) => prepared,
        Err(original) => {
            hooks.before_cleanup(arguments);
            return Err(cleanup_after_error(original, None, None, &mut stage, hooks));
        }
    };

    let mut candidate_output = None;
    let mut report_output = None;
    let published = (|| -> Result<(), CommandError> {
        require_stage_identity(&stage)?;
        hooks.between_stage_check_and_publish(&stage.path)?;
        candidate_output = Some(OwnedFile::create(&arguments.candidate, "candidate", hooks)?);
        let output = candidate_output
            .as_mut()
            .expect("candidate ownership registered after creation");
        hooks.copy_candidate(
            &stage.file,
            &mut output.file,
            &candidate_sha256,
            writer_summary.bytes,
        )?;
        hooks.sync_file(&output.file)?;
        hooks.sync_publication_parent(&arguments.candidate)?;
        hooks.before_report_publish(arguments)?;
        if !path_has_identity(&arguments.candidate, output.identity)? {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "candidate path no longer names the created destination inode",
            ));
        }
        report_output = Some(OwnedFile::create(&arguments.report, "report", hooks)?);
        let output = report_output
            .as_mut()
            .expect("report ownership registered after creation");
        hooks.write_report(&mut output.file, &report_bytes)?;
        hooks.sync_file(&output.file)?;
        hooks.sync_publication_parent(&arguments.report)?;
        hooks.before_report_ownership_check(arguments)?;
        if !path_has_identity(&arguments.report, output.identity)? {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "report path no longer names the created destination inode",
            ));
        }
        let candidate = candidate_output
            .as_ref()
            .expect("candidate exists through report publication");
        hooks.before_final_candidate_ownership_check(arguments)?;
        if !path_has_identity(&arguments.candidate, candidate.identity)? {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "candidate path changed during report publication",
            ));
        }
        Ok(())
    })();
    if let Err(original) = published {
        hooks.before_cleanup(arguments);
        return Err(cleanup_after_error(
            original,
            report_output.as_mut(),
            candidate_output.as_mut(),
            &mut stage,
            hooks,
        ));
    }
    if let Err(original) = stage.cleanup(hooks) {
        return Err(cleanup_after_error(
            original,
            report_output.as_mut(),
            candidate_output.as_mut(),
            &mut stage,
            hooks,
        ));
    }
    candidate_output
        .as_mut()
        .expect("candidate exists after successful publication")
        .disarm();
    report_output
        .as_mut()
        .expect("report exists after successful publication")
        .disarm();

    Ok(SparseCandidateOutcome {
        status: "built",
        candidate_sha256,
        candidate_bytes: writer_summary.bytes,
        report_schema: "pangopup.sparse-candidate-report.v1",
        size_gate_passed: report.adr_0027_size_gate_passed,
    })
}

#[derive(Default)]
struct Hooks {
    #[cfg(test)]
    after_input_open_hook: Option<fn(&SparseCandidateArguments)>,
    #[cfg(test)]
    after_candidate_open_hook: Option<fn(&Path)>,
    #[cfg(test)]
    before_stage_capture_cleanup_hook: Option<fn(&Path)>,
    #[cfg(test)]
    fail_stage_capture: bool,
    #[cfg(test)]
    fail_stage_identity: bool,
    #[cfg(test)]
    between_stage_check_and_publish_hook: Option<fn(&Path)>,
    #[cfg(test)]
    before_report_publish_hook: Option<fn(&SparseCandidateArguments)>,
    #[cfg(test)]
    before_report_ownership_check_hook: Option<fn(&SparseCandidateArguments)>,
    #[cfg(test)]
    before_final_candidate_ownership_check_hook: Option<fn(&SparseCandidateArguments)>,
    #[cfg(test)]
    before_cleanup_hook: Option<fn(&SparseCandidateArguments)>,
    #[cfg(test)]
    fail_write_call: Option<u8>,
    #[cfg(test)]
    write_calls: u8,
    #[cfg(test)]
    fail_file_sync_call: Option<u8>,
    #[cfg(test)]
    file_sync_calls: u8,
    #[cfg(test)]
    fail_publication_sync_call: Option<u8>,
    #[cfg(test)]
    publication_sync_calls: u8,
    #[cfg(test)]
    fail_unlink_call: Option<u8>,
    #[cfg(test)]
    unlink_calls: u8,
    #[cfg(test)]
    fail_cleanup_sync_call: Option<u8>,
    #[cfg(test)]
    cleanup_sync_calls: u8,
    #[cfg(test)]
    fail_created_identity_call: Option<u8>,
    #[cfg(test)]
    created_identity_calls: u8,
}

impl Hooks {
    fn capture_stage_identity(&mut self, file: &File) -> Result<FileIdentity, CommandError> {
        #[cfg(test)]
        if self.fail_stage_capture {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "injected stage capture failure",
            ));
        }
        #[cfg(test)]
        if self.fail_stage_identity {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "injected stage identity failure",
            ));
        }
        identity(file)
    }

    fn before_stage_capture_cleanup(&mut self, _stage: &Path) {
        #[cfg(test)]
        if let Some(hook) = self.before_stage_capture_cleanup_hook {
            hook(_stage);
        }
    }

    fn after_input_open(
        &mut self,
        _arguments: &SparseCandidateArguments,
    ) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.after_input_open_hook {
            hook(_arguments);
        }
        Ok(())
    }

    fn after_candidate_open(&mut self, _stage: &Path) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.after_candidate_open_hook {
            hook(_stage);
        }
        Ok(())
    }

    fn between_stage_check_and_publish(&mut self, _stage: &Path) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.between_stage_check_and_publish_hook {
            hook(_stage);
        }
        Ok(())
    }

    fn before_report_publish(
        &mut self,
        _arguments: &SparseCandidateArguments,
    ) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.before_report_publish_hook {
            hook(_arguments);
        }
        Ok(())
    }

    fn before_report_ownership_check(
        &mut self,
        _arguments: &SparseCandidateArguments,
    ) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.before_report_ownership_check_hook {
            hook(_arguments);
        }
        Ok(())
    }

    fn before_final_candidate_ownership_check(
        &mut self,
        _arguments: &SparseCandidateArguments,
    ) -> Result<(), CommandError> {
        #[cfg(test)]
        if let Some(hook) = self.before_final_candidate_ownership_check_hook {
            hook(_arguments);
        }
        Ok(())
    }

    fn created_identity(&mut self, file: &File) -> Result<FileIdentity, CommandError> {
        #[cfg(test)]
        {
            self.created_identity_calls = self.created_identity_calls.saturating_add(1);
            if self.fail_created_identity_call == Some(self.created_identity_calls) {
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION",
                    "injected created-file identity failure",
                ));
            }
        }
        identity(file)
    }

    fn copy_candidate(
        &mut self,
        source: &File,
        destination: &mut File,
        expected_sha256: &str,
        expected_bytes: u64,
    ) -> Result<(), CommandError> {
        let fail = self.next_write_fails();
        let mut source = source
            .try_clone()
            .map_err(|error| io_error("clone held candidate", error))?;
        source
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error("rewind held candidate", error))?;
        let mut digest = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| io_error("read held candidate", error))?;
            if count == 0 {
                break;
            }
            if fail {
                destination
                    .write_all(&buffer[..count.min(7)])
                    .map_err(|error| io_error("write candidate", error))?;
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION",
                    "injected candidate write failure",
                ));
            }
            destination
                .write_all(&buffer[..count])
                .map_err(|error| io_error("write candidate", error))?;
            digest.update(&buffer[..count]);
            total = total.checked_add(count as u64).ok_or_else(|| {
                CommandError::new("SPARSE_PUBLICATION", "candidate size overflow")
            })?;
        }
        let actual_sha256 = format!("sha256:{:x}", digest.finalize());
        if total != expected_bytes || actual_sha256 != expected_sha256 {
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "copied candidate does not match the held verified descriptor",
            ));
        }
        Ok(())
    }

    fn write_report(&mut self, destination: &mut File, bytes: &[u8]) -> Result<(), CommandError> {
        let fail = self.next_write_fails();
        if fail {
            destination
                .write_all(&bytes[..bytes.len().min(7)])
                .map_err(|error| io_error("write report", error))?;
            return Err(CommandError::new(
                "SPARSE_PUBLICATION",
                "injected report write failure",
            ));
        }
        destination
            .write_all(bytes)
            .map_err(|error| io_error("write report", error))
    }

    fn next_write_fails(&mut self) -> bool {
        #[cfg(test)]
        {
            self.write_calls = self.write_calls.saturating_add(1);
            self.fail_write_call == Some(self.write_calls)
        }
        #[cfg(not(test))]
        false
    }

    fn sync_file(&mut self, file: &File) -> Result<(), CommandError> {
        #[cfg(test)]
        {
            self.file_sync_calls = self.file_sync_calls.saturating_add(1);
            if self.fail_file_sync_call == Some(self.file_sync_calls) {
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION",
                    "injected file sync failure",
                ));
            }
        }
        file.sync_all()
            .map_err(|error| io_error("sync published file", error))
    }

    fn sync_publication_parent(&mut self, path: &Path) -> Result<(), CommandError> {
        #[cfg(test)]
        {
            self.publication_sync_calls = self.publication_sync_calls.saturating_add(1);
            if self.fail_publication_sync_call == Some(self.publication_sync_calls) {
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION",
                    "injected directory sync failure",
                ));
            }
        }
        sync_parent(path)
    }

    fn unlink_owned(&mut self, path: &Path) -> Result<(), CommandError> {
        #[cfg(test)]
        {
            self.unlink_calls = self.unlink_calls.saturating_add(1);
            if self.fail_unlink_call == Some(self.unlink_calls) {
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION_CLEANUP",
                    "injected unlink failure",
                ));
            }
        }
        fs::remove_file(path).map_err(|error| io_error("remove owned file", error))
    }

    fn sync_cleanup_parent(&mut self, path: &Path) -> Result<(), CommandError> {
        #[cfg(test)]
        {
            self.cleanup_sync_calls = self.cleanup_sync_calls.saturating_add(1);
            if self.fail_cleanup_sync_call == Some(self.cleanup_sync_calls) {
                return Err(CommandError::new(
                    "SPARSE_PUBLICATION_CLEANUP",
                    "injected cleanup directory sync failure",
                ));
            }
        }
        sync_parent(path)
    }

    fn before_cleanup(&mut self, _arguments: &SparseCandidateArguments) {
        #[cfg(test)]
        if let Some(hook) = self.before_cleanup_hook {
            hook(_arguments);
        }
    }
}

impl From<SparseWriteSummary> for CandidateCounts {
    fn from(value: SparseWriteSummary) -> Self {
        Self {
            genes: value.genes,
            loci: value.loci,
            ordinary_loci: value.ordinary_loci,
            segments: value.segments,
            blocks: value.blocks,
            exceptions: value.exceptions,
        }
    }
}

impl DecodedCounts {
    fn from_summary(value: DecodedSummary, records: u64) -> Self {
        Self {
            genes: value.genes,
            loci: value.loci,
            ordinary_loci: value.ordinary_loci,
            segments: value.segments,
            exceptions: value.exceptions,
            records,
        }
    }
}

fn decode_candidate(
    reader: &SparseIndexReader,
) -> Result<(DecodedSummary, u64, String), CommandError> {
    let mut hash = Sha256::new();
    let mut records = 0_u64;
    let summary = reader
        .visit_all(|locus| {
            write_logical_text(&mut hash, locus)?;
            records = records
                .checked_add(3)
                .ok_or_else(|| io::Error::other("decoded record count overflow"))?;
            Ok::<_, io::Error>(())
        })
        .map_err(|error| visit_error("SPARSE_CANDIDATE", error))?;
    Ok((summary, records, format!("sha256:{:x}", hash.finalize())))
}

fn validate_parity(
    manifest: &pangopup_index::BundleManifest,
    writer: SparseWriteSummary,
    decoded: DecodedSummary,
    records: u64,
    digest: &str,
) -> Result<(), CommandError> {
    let counts = manifest.counts;
    let ordinary = counts
        .gene_loci
        .checked_sub(counts.n_ref_loci)
        .ok_or_else(|| {
            CommandError::new("SPARSE_PARITY", "manifest exception count exceeds loci")
        })?;
    if writer.genes != counts.genes
        || writer.loci != counts.gene_loci
        || writer.ordinary_loci != ordinary
        || writer.segments != counts.index_segments
        || writer.exceptions != counts.n_ref_loci
        || decoded.genes != writer.genes
        || decoded.loci != writer.loci
        || decoded.ordinary_loci != writer.ordinary_loci
        || decoded.segments != writer.segments
        || decoded.exceptions != writer.exceptions
        || records != manifest.logical_decoded.records
        || digest != manifest.logical_decoded.sha256
        || manifest.logical_source != manifest.logical_decoded
    {
        return Err(CommandError::new(
            "SPARSE_PARITY",
            "fixed manifest, sparse writer, and sparse decode do not match",
        ));
    }
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

fn validate_bundle_member_set(bundle: &Path) -> Result<(), CommandError> {
    let expected = BTreeSet::from([
        "NOTICE".to_owned(),
        "manifest.json".to_owned(),
        "scores.pgi".to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(bundle).map_err(|error| io_error("read fixed bundle", error))? {
        let entry = entry.map_err(|error| io_error("read fixed bundle member", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| CommandError::new("SPARSE_INPUT", "bundle member name is not UTF-8"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| io_error("inspect fixed bundle member", error))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(CommandError::new(
                "SPARSE_INPUT",
                "fixed bundle members must be regular files",
            ));
        }
        actual.insert(name);
    }
    if actual != expected {
        return Err(CommandError::new(
            "SPARSE_INPUT",
            "fixed bundle member set mismatch",
        ));
    }
    Ok(())
}

fn open_regular(path: &Path, label: &str) -> Result<File, CommandError> {
    let before =
        fs::symlink_metadata(path).map_err(|error| io_error(&format!("inspect {label}"), error))?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(CommandError::new(
            "SPARSE_INPUT",
            format!("{label} is not a regular file"),
        ));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = File::open(path);
    let file = file.map_err(|error| io_error(&format!("open {label}"), error))?;
    if !file
        .metadata()
        .map_err(|error| io_error(&format!("inspect opened {label}"), error))?
        .file_type()
        .is_file()
    {
        return Err(CommandError::new(
            "SPARSE_INPUT",
            format!("{label} is not a regular file"),
        ));
    }
    Ok(file)
}

fn hash_file(file: &File) -> Result<String, CommandError> {
    let mut held = file
        .try_clone()
        .map_err(|error| io_error("clone candidate", error))?;
    held.seek(SeekFrom::Start(0))
        .map_err(|error| io_error("rewind candidate", error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = held
            .read(&mut buffer)
            .map_err(|error| io_error("hash candidate", error))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

#[derive(Clone, Copy)]
struct FileIdentity {
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

fn identity(file: &File) -> Result<FileIdentity, CommandError> {
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect held file", error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            len: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileIdentity {
            len: metadata.len(),
        })
    }
}

fn path_has_identity(path: &Path, expected: FileIdentity) -> Result<bool, CommandError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error("inspect publication path", error)),
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(metadata.dev() == expected.device && metadata.ino() == expected.inode)
    }
    #[cfg(not(unix))]
    {
        Ok(true)
    }
}

fn validate_held_input(
    file: &File,
    expected: FileIdentity,
    label: &str,
) -> Result<(), CommandError> {
    let actual = identity(file)?;
    if actual.len != expected.len {
        return Err(CommandError::new(
            "SPARSE_INPUT_CHANGED",
            format!("held {label} changed length"),
        ));
    }
    #[cfg(unix)]
    if actual.device != expected.device || actual.inode != expected.inode {
        return Err(CommandError::new(
            "SPARSE_INPUT_CHANGED",
            format!("held {label} changed identity"),
        ));
    }
    Ok(())
}

fn require_stage_identity(stage: &OwnedFile) -> Result<(), CommandError> {
    let actual = identity(&stage.file)?;
    if actual.len != stage.identity.len
        || !same_identity(actual, stage.identity)
        || !path_has_identity(&stage.path, stage.identity)?
    {
        return Err(CommandError::new(
            "SPARSE_PUBLICATION",
            "staged path no longer names the held verified inode",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn same_identity(left: FileIdentity, right: FileIdentity) -> bool {
    left.device == right.device && left.inode == right.inode
}

#[cfg(not(unix))]
fn same_identity(_left: FileIdentity, _right: FileIdentity) -> bool {
    true
}

struct OwnedFile {
    path: PathBuf,
    identity: FileIdentity,
    file: File,
    active: bool,
}

impl OwnedFile {
    fn from_parts(path: PathBuf, identity: FileIdentity, file: File) -> Self {
        Self {
            path,
            identity,
            file,
            active: true,
        }
    }

    fn create(path: &Path, label: &str, hooks: &mut Hooks) -> Result<Self, CommandError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| io_error(&format!("create {label}"), error))?;
        match hooks.created_identity(&file) {
            Ok(identity) => Ok(Self::from_parts(path.to_owned(), identity, file)),
            Err(original) => {
                let cleanup = identity(&file).and_then(|identity| {
                    let mut owned = Self::from_parts(path.to_owned(), identity, file);
                    owned.cleanup(hooks)
                });
                Err(match cleanup {
                    Ok(()) => original,
                    Err(cleanup) => combine_cleanup_error(original, cleanup),
                })
            }
        }
    }

    fn cleanup(&mut self, hooks: &mut Hooks) -> Result<(), CommandError> {
        if !self.active {
            return Ok(());
        }
        if path_has_identity(&self.path, self.identity)? {
            hooks.unlink_owned(&self.path)?;
            self.active = false;
            hooks.sync_cleanup_parent(&self.path)?;
            return Ok(());
        }
        if held_link_count(&self.file)? == 0 {
            self.active = false;
            hooks.sync_cleanup_parent(&self.path)?;
            return Ok(());
        }
        Err(CommandError::new(
            "SPARSE_PUBLICATION_CLEANUP",
            "builder-owned inode moved to an unknown path",
        ))
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

#[cfg(unix)]
fn held_link_count(file: &File) -> Result<u64, CommandError> {
    use std::os::unix::fs::MetadataExt;
    file.metadata()
        .map(|metadata| metadata.nlink())
        .map_err(|error| io_error("inspect held link count", error))
}

#[cfg(not(unix))]
fn held_link_count(_file: &File) -> Result<u64, CommandError> {
    Ok(1)
}

fn cleanup_after_error(
    original: CommandError,
    report: Option<&mut OwnedFile>,
    candidate: Option<&mut OwnedFile>,
    stage: &mut OwnedFile,
    hooks: &mut Hooks,
) -> CommandError {
    let mut failures = Vec::new();
    for owned in [report, candidate, Some(stage)].into_iter().flatten() {
        if let Err(error) = owned.cleanup(hooks) {
            failures.push(error.to_string());
        }
    }
    if failures.is_empty() {
        original
    } else {
        CommandError::new(
            "SPARSE_PUBLICATION_CLEANUP",
            format!("{original}; cleanup also failed: {}", failures.join("; ")),
        )
    }
}

fn combine_cleanup_error(original: CommandError, cleanup: CommandError) -> CommandError {
    CommandError::new(
        "SPARSE_PUBLICATION_CLEANUP",
        format!("{original}; cleanup also failed: {cleanup}"),
    )
}

fn sync_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync publication directory", error))
}

fn unique_stage_path(base: &Path, label: &str) -> PathBuf {
    let serial = STAGE_SERIAL.fetch_add(1, Ordering::Relaxed);
    let name = base
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("pangopup");
    base.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".{name}.{label}.stage.{}.{serial}",
            std::process::id()
        ))
}

fn require_absent(label: &str, path: &Path) -> Result<(), CommandError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&format!("inspect {label}"), error)),
        Ok(_) => Err(CommandError::new(
            "SPARSE_DESTINATION_EXISTS",
            format!("{label} path already exists"),
        )),
    }
}

fn visit_error<E: std::fmt::Display>(code: &'static str, error: VisitAllError<E>) -> CommandError {
    match error {
        VisitAllError::Index(error) => command_error(code, error),
        VisitAllError::Visitor(error) => CommandError::new(code, error.to_string()),
    }
}

fn command_error(code: &'static str, error: impl std::fmt::Display) -> CommandError {
    CommandError::new(code, error.to_string())
}

fn io_error(action: &str, error: io::Error) -> CommandError {
    CommandError::new("IO", format!("{action}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture_bundle() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/snv-regression/bundle")
    }

    fn copy_bundle(source: &Path, destination: &Path) {
        fs::create_dir(destination).expect("bundle directory");
        for member in ["NOTICE", "manifest.json", "scores.pgi"] {
            fs::copy(source.join(member), destination.join(member)).expect("copy bundle member");
        }
    }

    fn test_arguments(temp: &TempDir) -> SparseCandidateArguments {
        let bundle = temp.path().join("bundle");
        copy_bundle(&fixture_bundle(), &bundle);
        SparseCandidateArguments {
            fixed_bundle: bundle,
            scratch: temp.path().join("scratch"),
            candidate: temp.path().join("candidate.pgi"),
            report: temp.path().join("report.json"),
            expected_bundle_id: None,
        }
    }

    fn replace_input_path(arguments: &SparseCandidateArguments) {
        let scores = arguments.fixed_bundle.join("scores.pgi");
        fs::rename(&scores, arguments.fixed_bundle.join("scores.held")).expect("rename scores");
        fs::write(scores, b"replacement").expect("replace scores path");
    }

    fn replace_candidate_stage(stage: &Path) {
        fs::rename(stage, stage.with_extension("held")).expect("rename candidate stage");
        fs::write(stage, b"replacement stage").expect("replace candidate stage");
    }

    fn replace_candidate_stage_after_check(stage: &Path) {
        fs::remove_file(stage).expect("unlink candidate stage");
        fs::write(stage, b"replacement after identity check").expect("replace candidate stage");
    }

    fn mutate_candidate_logical_stream(stage: &Path) {
        let mut bytes = fs::read(stage).expect("read candidate stage");
        let payload = usize::try_from(u64::from_le_bytes(
            bytes[72..80].try_into().expect("payload offset bytes"),
        ))
        .expect("payload offset");
        let blocks = usize::try_from(u64::from_le_bytes(
            bytes[56..64].try_into().expect("block offset bytes"),
        ))
        .expect("block offset");
        let first_payload = usize::try_from(u64::from_le_bytes(
            bytes[blocks + 16..blocks + 24]
                .try_into()
                .expect("block payload offset bytes"),
        ))
        .expect("block payload offset");
        bytes[payload + first_payload + 16] ^= 1;
        fs::write(stage, bytes).expect("write valid logical mutation");
    }

    fn replace_published_candidate(arguments: &SparseCandidateArguments) {
        fs::rename(
            &arguments.candidate,
            arguments.candidate.with_extension("owned"),
        )
        .expect("rename published candidate");
        fs::write(&arguments.candidate, b"replacement final").expect("replace final candidate");
    }

    fn replace_report(arguments: &SparseCandidateArguments) {
        fs::rename(&arguments.report, arguments.report.with_extension("owned"))
            .expect("rename published report");
        fs::write(&arguments.report, b"replacement report").expect("replace final report");
    }

    fn assert_no_candidate_stage(temp: &TempDir) {
        let stages: Vec<_> = fs::read_dir(temp.path())
            .expect("read temp directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("candidate.stage")
            })
            .map(|entry| entry.path())
            .collect();
        assert!(stages.is_empty(), "unexpected candidate stages: {stages:?}");
    }

    #[test]
    fn production_limits_match_the_ticket() {
        let limits = Limits::production();
        assert_eq!(limits.gene_loci, 3_000_000);
        assert_eq!(limits.gene_capacity_bytes, 512 * 1024 * 1024);
        assert_eq!(ADR_0027_SIZE_GATE_BYTES, 3_221_225_472);
    }

    #[test]
    fn held_input_descriptor_survives_path_replacement() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            after_input_open_hook: Some(replace_input_path),
            ..Hooks::default()
        };
        build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect("held input conversion");
        assert!(arguments.candidate.exists());
        assert!(arguments.report.exists());
        assert_eq!(
            fs::read(arguments.fixed_bundle.join("scores.pgi")).expect("read replacement input"),
            b"replacement"
        );
    }

    #[test]
    fn certification_applies_gene_limit_before_writer_creation() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let error = build_sparse_candidate_inner(
            &arguments,
            Limits {
                gene_loci: 1,
                gene_capacity_bytes: MAX_GENE_CAPACITY_BYTES,
            },
            &mut Hooks::default(),
        )
        .expect_err("certification gene limit");
        assert!(error.message.contains("allocation limit"));
        assert!(!arguments.scratch.exists());
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn finished_stage_capture_and_identity_failures_clean_the_guarded_path() {
        for (label, mut hooks) in [
            (
                "capture",
                Hooks {
                    fail_stage_capture: true,
                    ..Hooks::default()
                },
            ),
            (
                "identity",
                Hooks {
                    fail_stage_identity: true,
                    ..Hooks::default()
                },
            ),
        ] {
            let temp = TempDir::new().expect("temp");
            let arguments = test_arguments(&temp);
            let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
                .expect_err(label);
            assert!(
                error
                    .message
                    .contains(&format!("injected stage {label} failure"))
            );
            assert!(!arguments.scratch.exists(), "{label}");
            assert!(!arguments.candidate.exists(), "{label}");
            assert!(!arguments.report.exists(), "{label}");
            assert_no_candidate_stage(&temp);
        }
    }

    #[test]
    fn stage_identity_failure_reports_replaced_path_cleanup_failure() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_stage_identity: true,
            before_stage_capture_cleanup_hook: Some(replace_candidate_stage),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("identity and cleanup failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(error.message.contains("injected stage identity failure"));
        assert!(error.message.contains("cleanup also failed"));
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
    }

    #[test]
    fn replaced_candidate_stage_never_publishes_the_replacement() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            after_candidate_open_hook: Some(replace_candidate_stage),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("replaced stage");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(
            error
                .message
                .contains("staged path no longer names the held verified inode")
        );
        assert!(
            error
                .message
                .contains("builder-owned inode moved to an unknown path")
        );
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
        let replacements = fs::read_dir(temp.path())
            .expect("read temp directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("candidate.stage")
            })
            .count();
        assert_eq!(replacements, 2);
    }

    #[test]
    fn held_descriptor_copy_survives_replacement_after_identity_check() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            between_stage_check_and_publish_hook: Some(replace_candidate_stage_after_check),
            ..Hooks::default()
        };
        let outcome = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect("held descriptor publication");
        assert_eq!(hash_path(&arguments.candidate), outcome.candidate_sha256);
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(&arguments.report).expect("report bytes"))
                .expect("report JSON");
        assert_eq!(report["candidate_sha256"], outcome.candidate_sha256);
        assert_eq!(report["candidate_bytes"], outcome.candidate_bytes);
        let replacement = fs::read_dir(temp.path())
            .expect("temp directory")
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("candidate.stage")
            })
            .expect("replacement stage");
        assert_eq!(
            fs::read(replacement.path()).expect("replacement stage bytes"),
            b"replacement after identity check"
        );
    }

    #[test]
    fn valid_candidate_logical_change_fails_complete_parity() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            after_candidate_open_hook: Some(mutate_candidate_logical_stream),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("candidate logical mismatch");
        assert_eq!(error.code, "SPARSE_PARITY");
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn created_candidate_and_report_identity_failures_do_not_leak_outputs() {
        for (label, call) in [("candidate", 1), ("report", 2)] {
            let temp = TempDir::new().expect("temp");
            let arguments = test_arguments(&temp);
            let mut hooks = Hooks {
                fail_created_identity_call: Some(call),
                ..Hooks::default()
            };
            let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
                .expect_err(label);
            assert!(
                error
                    .message
                    .contains("injected created-file identity failure"),
                "{label}: {error}"
            );
            assert!(!arguments.candidate.exists(), "{label}");
            assert!(!arguments.report.exists(), "{label}");
            assert_no_candidate_stage(&temp);
        }
    }

    #[test]
    fn created_file_identity_cleanup_failure_is_aggregated() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_created_identity_call: Some(1),
            fail_unlink_call: Some(1),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("created identity cleanup failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(
            error
                .message
                .contains("injected created-file identity failure")
        );
        assert!(error.message.contains("injected unlink failure"));
        assert!(arguments.candidate.exists());
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn candidate_replacement_before_report_is_detected_and_preserved() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            before_report_publish_hook: Some(replace_published_candidate),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("candidate replacement before report");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(
            error
                .message
                .contains("candidate path no longer names the created destination inode")
        );
        assert_eq!(
            fs::read(&arguments.candidate).expect("candidate replacement"),
            b"replacement final"
        );
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn candidate_replacement_after_report_is_detected_and_preserved() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            before_final_candidate_ownership_check_hook: Some(replace_published_candidate),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("candidate replacement after report");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(
            error
                .message
                .contains("candidate path changed during report publication")
        );
        assert_eq!(
            fs::read(&arguments.candidate).expect("candidate replacement"),
            b"replacement final"
        );
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn report_replacement_at_ownership_check_is_detected_and_preserved() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            before_report_ownership_check_hook: Some(replace_report),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("report replacement");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(
            error
                .message
                .contains("report path no longer names the created destination inode")
        );
        assert!(!arguments.candidate.exists());
        assert_eq!(
            fs::read(&arguments.report).expect("report replacement"),
            b"replacement report"
        );
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn every_publication_boundary_cleans_owned_outputs() {
        for (label, mut hooks) in [
            (
                "candidate-write",
                Hooks {
                    fail_write_call: Some(1),
                    ..Hooks::default()
                },
            ),
            (
                "candidate-file-sync",
                Hooks {
                    fail_file_sync_call: Some(1),
                    ..Hooks::default()
                },
            ),
            (
                "candidate-directory-sync",
                Hooks {
                    fail_publication_sync_call: Some(1),
                    ..Hooks::default()
                },
            ),
            (
                "report-write",
                Hooks {
                    fail_write_call: Some(2),
                    ..Hooks::default()
                },
            ),
            (
                "report-file-sync",
                Hooks {
                    fail_file_sync_call: Some(2),
                    ..Hooks::default()
                },
            ),
            (
                "report-directory-sync",
                Hooks {
                    fail_publication_sync_call: Some(2),
                    ..Hooks::default()
                },
            ),
        ] {
            let temp = TempDir::new().expect("temp");
            let arguments = test_arguments(&temp);
            let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
                .expect_err(label);
            assert_eq!(error.code, "SPARSE_PUBLICATION", "{label}");
            assert!(!arguments.candidate.exists(), "{label}");
            assert!(!arguments.report.exists(), "{label}");
            assert_no_candidate_stage(&temp);
        }
    }

    #[test]
    fn cleanup_error_keeps_original_failure_and_replaced_path() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_write_call: Some(2),
            before_cleanup_hook: Some(replace_published_candidate),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("cleanup failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(error.message.contains("injected report write failure"));
        assert!(error.message.contains("cleanup also failed"));
        assert_eq!(
            fs::read(&arguments.candidate).expect("read replacement candidate"),
            b"replacement final"
        );
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn unlink_failure_preserves_original_and_cleanup_errors() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_write_call: Some(2),
            fail_unlink_call: Some(1),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("unlink failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(error.message.contains("injected report write failure"));
        assert!(error.message.contains("injected unlink failure"));
        assert!(arguments.report.exists());
        assert!(!arguments.candidate.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn cleanup_directory_sync_failure_preserves_both_errors() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_write_call: Some(2),
            fail_cleanup_sync_call: Some(1),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("cleanup directory sync failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(error.message.contains("injected report write failure"));
        assert!(
            error
                .message
                .contains("injected cleanup directory sync failure")
        );
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    #[test]
    fn final_stage_cleanup_failure_rolls_back_completed_outputs() {
        let temp = TempDir::new().expect("temp");
        let arguments = test_arguments(&temp);
        let mut hooks = Hooks {
            fail_unlink_call: Some(1),
            ..Hooks::default()
        };
        let error = build_sparse_candidate_inner(&arguments, Limits::production(), &mut hooks)
            .expect_err("stage cleanup failure");
        assert_eq!(error.code, "SPARSE_PUBLICATION_CLEANUP");
        assert!(error.message.contains("injected unlink failure"));
        assert!(!arguments.candidate.exists());
        assert!(!arguments.report.exists());
        assert_no_candidate_stage(&temp);
    }

    fn hash_path(path: &Path) -> String {
        format!(
            "sha256:{:x}",
            Sha256::digest(fs::read(path).expect("hash path bytes"))
        )
    }
}
