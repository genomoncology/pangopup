//! Byte-producing reference build adapter.

use crate::command_error::CommandError;
use crate::reference_certification::{ReferenceCertification, certify_reference_bundle};
use crate::source_fingerprint::reference_source_sha256;
use flate2::bufread::GzDecoder;
use pangopup_core::Grch38Contig;
use pangopup_index::reference::{
    AttributionManifest, BuilderManifest, InputManifest, MINI_NOTICE, MINI_PROFILE, MemberManifest,
    PRODUCTION_NOTICE, PRODUCTION_PROFILE, REFERENCE_FORMAT, REFERENCE_SCHEMA, ROUTE_TEST_NOTICE,
    ROUTE_TEST_PROFILE, ReferenceAliasManifest, ReferenceContigPlan, ReferenceIndexError,
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
const ROUTE_FASTA_BYTES: u64 = 11_136;
const ROUTE_FASTA_SHA: &str = "81a5af971ad9c72b3a679a678ca05f2b050a865a622e272abc77eb1be43c3eb8";
const ROUTE_REPORT_BYTES: u64 = 2_112;
const ROUTE_REPORT_SHA: &str = "ca184c4c4448bdb9af66899d1d84f7925ab993528f774fe843a45825015a9c5f";
const ROUTE_SEQUENCE_SHA: &str = "afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3";
const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
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
            source_sha256: format!("sha256:{}", reference_source_sha256()),
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
    let maximum = match profile.name {
        PRODUCTION_PROFILE => 972_898_531,
        MINI_PROFILE => MINI_FASTA_BYTES,
        ROUTE_TEST_PROFILE => ROUTE_FASTA_BYTES,
        _ => unreachable!("closed profile"),
    };
    let (mut file, identity) = open_held_regular(source, maximum, "REFERENCE_INPUT")?;
    let allowed_size = match profile.name {
        PRODUCTION_PROFILE => identity.size == 972_898_531,
        MINI_PROFILE => matches!(identity.size, MINI_FASTA_BYTES | MINI_GZIP_BYTES),
        ROUTE_TEST_PROFILE => identity.size == ROUTE_FASTA_BYTES,
        _ => false,
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
        ROUTE_TEST_PROFILE => Ok(Profile {
            name: ROUTE_TEST_PROFILE,
            assembly: "synthetic-route-test",
            assembly_accession: ROUTE_TEST_PROFILE,
            fasta_url: "urn:pangopup:fixture:reference-route-test-v1:source",
            report_url: "urn:pangopup:fixture:reference-route-test-v1:assembly-report",
            policy_url: "https://www.gnu.org/licenses/gpl-3.0.html",
            notice: ROUTE_TEST_NOTICE,
            lengths: [
                10_101, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            ],
            total_bases: 10_125,
            sequence_sha: ROUTE_SEQUENCE_SHA,
            extra_count: 0,
            extra_sha: EMPTY_SHA,
            contexts: 1,
            max_decoded_bytes: ROUTE_FASTA_BYTES,
            max_records: 25,
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
    let valid = match profile.name {
        PRODUCTION_PROFILE => {
            observed.compression == "gzip"
                && observed.bytes == 972_898_531
                && sha == "11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"
        }
        MINI_PROFILE => {
            (observed.compression == "none"
                && observed.bytes == MINI_FASTA_BYTES
                && sha == MINI_FASTA_SHA)
                || (observed.compression == "gzip"
                    && observed.bytes == MINI_GZIP_BYTES
                    && sha == MINI_GZIP_SHA)
        }
        ROUTE_TEST_PROFILE => {
            observed.compression == "none"
                && observed.bytes == ROUTE_FASTA_BYTES
                && sha == ROUTE_FASTA_SHA
        }
        _ => false,
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
    let expected = match profile.name {
        PRODUCTION_PROFILE => (
            80_454,
            "64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38",
        ),
        MINI_PROFILE => (MINI_REPORT_BYTES, MINI_REPORT_SHA),
        ROUTE_TEST_PROFILE => (ROUTE_REPORT_BYTES, ROUTE_REPORT_SHA),
        _ => {
            return Err(CommandError::new(
                "REFERENCE_INPUT",
                "reference profile is invalid",
            ));
        }
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
    let mut file = File::open(path).map_err(|_| CommandError::new("IO", "artifact read failed"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| CommandError::new("IO", "artifact read failed"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
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

#[cfg(test)]
#[path = "reference_builder_tests.rs"]
mod tests;
