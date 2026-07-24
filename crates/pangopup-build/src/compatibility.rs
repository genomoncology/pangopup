//! Pinned Pangolin 1.0.2 compatibility-corpus capture and inspection.

use crate::CommandError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

const SCHEMA: &str = "pangopup-compat-v1";
const PROFILE: &str = "pangolin-1.0.2-5cf94b8-grch38-v1";
const MANIFEST_MAX: u64 = 128 * 1024;
const CASES_MAX: u64 = 3_800_000;
const NOTICE_MAX: u64 = 64 * 1024;
const AGGREGATE_MAX: u64 = 4 * 1024 * 1024;
const LINE_MAX: usize = 256 * 1024;
const STRING_MAX: usize = 8 * 1024;
const TOKEN_MAX: usize = 128;
const CONTEXT_MAX: usize = 10_200;
const ARRAY_MAX: usize = 200;
const GENE_MAX: usize = 4;
const BOUNDARY_MAX: usize = 512;

const HELPER_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/pangolin_compat_capture.py"
));

const UPSTREAM_SOURCES: [(&str, u64, &str); 4] = [
    (
        "pangolin/__init__.py",
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "pangolin/model.py",
        3_011,
        "4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8",
    ),
    (
        "pangolin/pangolin.py",
        12_164,
        "f3f25c4febf64d01ef42f967dc5cf10f6856bf3aa92d26f09803ad408169da9b",
    ),
    (
        "setup.py",
        681,
        "2dd5c6abefd582be2249180412a46a77c7573cc1c1a54301232a4f1e43c0aa76",
    ),
];

const CONTEXT_SHAS: [&str; 14] = [
    "092d912061e0ed808a0e85be2b7d31e7e453909b9bb2b26483eb258441b92f5d",
    "e06bc587c8bc14b2cb15650c694e8c7144c17ea66dc19e070bc5a80785089a78",
    "d5d4f5638376c25acf165b49965402b60dda40f72cbc473fc9a165f65e6503f2",
    "7aa9b416eda9f4c78682db8426cf187ad199344eef5a70d441faefdcba142034",
    "42616586627a49d1f570f4555c0af3e05a15210d1930687e668860c9cd7075f2",
    "91479315c99339e16d0afba8d59b86d73fae272eb028b97aeb88030b45444966",
    "2c8e55f10f1b3cf94258d1f71be0fa677c6e159168c0968d5475453fc1562b71",
    "f2c86eb8c818fbf682a9d9f67c602ee7f4ec7c7f6de846bb2c091dafe30a803c",
    "4220078c1a6eafbcb5e2555d4cb2e379feb5b88c18f4d46049d125f38bcfe843",
    "a83cf1026ba2c584e8501a23cee9411f0c5a08b52f413e60cb059ebf909ad68c",
    "42616586627a49d1f570f4555c0af3e05a15210d1930687e668860c9cd7075f2",
    "2c8e55f10f1b3cf94258d1f71be0fa677c6e159168c0968d5475453fc1562b71",
    "f2c86eb8c818fbf682a9d9f67c602ee7f4ec7c7f6de846bb2c091dafe30a803c",
    "22ccdbb5a9cc6c68c03cca27a9a5e3d142cddc783219d72bc8132c66048d5876",
];

const CASE_IDS: [&str; 24] = [
    "M01-snv-cd4-precomputed",
    "M02-snv-wrap53-tp53-precomputed",
    "M03-snv-afap1l2-precomputed",
    "M04-snv-grk1-precomputed",
    "M05-snv-same-strand-overlap",
    "M06-snv-gene-start-plus-one",
    "M07-mnv-plus",
    "M08-mnv-both-strands",
    "M09-insertion-short-plus",
    "M10-insertion-short-both",
    "M11-insertion-long-overlap",
    "M12-deletion-short-plus",
    "M13-deletion-short-both",
    "M14-deletion-ref100-overlap",
    "R01-complex-replacement",
    "R02-deletion-ref101",
    "R03-reference-mismatch",
    "R04-no-containing-gene",
    "R05-left-context",
    "R06-right-context",
    "P01-same-strand-order",
    "P02-empty-boundaries",
    "P03-first-extremum",
    "P04-rounding-signed-zero",
];

const COVERAGE: [&str; 28] = [
    "shape.snv",
    "shape.mnv_equal",
    "shape.insertion_anchored",
    "shape.deletion_anchored",
    "strand.plus",
    "strand.minus",
    "overlap.same_strand",
    "overlap.opposite_strand",
    "mask.masked",
    "mask.unmasked",
    "boundary.gene_start_plus_one",
    "indel.insertion_short",
    "indel.insertion_long",
    "indel.deletion_short",
    "indel.deletion_ref_100",
    "lookup.precomputed_observation",
    "effect.zero_or_low",
    "effect.nonzero",
    "reject.complex_unequal",
    "reject.deletion_ref_101",
    "reject.ref_mismatch",
    "reject.no_gene",
    "reject.left_context",
    "reject.right_context",
    "postprocess.same_strand_order",
    "postprocess.empty_boundaries",
    "postprocess.first_extremum",
    "postprocess.rounding_signed_zero",
];

const CHECKPOINTS: [(&str, &str); 12] = [
    (
        "final.1.0.3.v2",
        "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501",
    ),
    (
        "final.2.0.3.v2",
        "c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26",
    ),
    (
        "final.3.0.3.v2",
        "ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676",
    ),
    (
        "final.1.2.3.v2",
        "559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a",
    ),
    (
        "final.2.2.3.v2",
        "48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5",
    ),
    (
        "final.3.2.3.v2",
        "7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129",
    ),
    (
        "final.1.4.3.v2",
        "c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6",
    ),
    (
        "final.2.4.3.v2",
        "e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e",
    ),
    (
        "final.3.4.3.v2",
        "9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6",
    ),
    (
        "final.1.6.3.v2",
        "2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e",
    ),
    (
        "final.2.6.3.v2",
        "7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f",
    ),
    (
        "final.3.6.3.v2",
        "756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd",
    ),
];

#[derive(Debug, Serialize)]
pub struct InspectOutcome {
    pub status: &'static str,
    pub schema: &'static str,
    pub profile: &'static str,
    pub cases: u8,
    pub scored_cases: u8,
    pub rejection_cases: u8,
    pub postprocess_cases: u8,
    pub coverage_cells: u8,
}

#[derive(Debug, Serialize)]
pub struct CaptureOutcome {
    pub status: &'static str,
    pub schema: &'static str,
    pub profile: &'static str,
    pub corpus_sha256: String,
    pub cases: u8,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    profile: String,
    upstream: Upstream,
    checkpoints: Vec<Checkpoint>,
    reference: Reference,
    annotation: Annotation,
    environment: Environment,
    coverage: Vec<String>,
    case_ids: Vec<String>,
    members: Vec<Member>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Upstream {
    url: String,
    commit: String,
    declared_version: String,
    license: String,
    helper_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    ordinal: u8,
    filename: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    source_url: String,
    source_bytes: u64,
    source_sha256: String,
    assembly_report_url: String,
    assembly_report_bytes: u64,
    assembly_report_sha256: String,
    transform: String,
    derived_bytes: u64,
    derived_sha256: String,
    contigs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Annotation {
    database_url: String,
    database_bytes: u64,
    database_sha256: String,
    gtf_url: String,
    gtf_bytes: u64,
    gtf_md5: String,
    gtf_sha256: String,
    filter: String,
    logical_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    python: String,
    pytorch: String,
    numpy: String,
    pandas: String,
    pyfastx: String,
    gffutils: String,
    pyvcf3: String,
    platform: String,
    cuda: bool,
    helper_torch_intraop_threads: u8,
    helper_torch_interop_threads: u8,
    cli_omp_threads: u8,
    cli_torch_interop_threads_observed: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Member {
    filename: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantInput {
    assembly: String,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    alt: String,
    distance: u16,
    allele_shape: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Context {
    start_1based: u32,
    anchor_offset: u16,
    bases: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Gene {
    id: String,
    boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GeneExpected {
    gene: String,
    gain_bits: String,
    gain_position: i32,
    loss_bits: String,
    loss_position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StrandExpected {
    unmasked: Vec<GeneExpected>,
    masked: Vec<GeneExpected>,
    cli_unmasked: String,
    cli_masked: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StrandCase {
    strand: String,
    dtype: String,
    loss_bits: Vec<String>,
    gain_bits: Vec<String>,
    genes: Vec<Gene>,
    expected: StrandExpected,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Precomputed {
    source_member: String,
    gene: String,
    gain_bits: String,
    gain_position: i32,
    loss_bits: String,
    loss_position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelCase {
    id: String,
    kind: String,
    coverage: Vec<String>,
    input: VariantInput,
    context: Context,
    strands: Vec<StrandCase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    precomputed: Vec<Precomputed>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RejectionCase {
    id: String,
    kind: String,
    coverage: Vec<String>,
    input: VariantInput,
    first_operation: String,
    normalized_category: String,
    witness: RejectionWitness,
    upstream_evidence: UpstreamEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum UpstreamEvidence {
    #[serde(rename = "cli")]
    Cli { warning: String },
    #[serde(rename = "rule_replay")]
    RuleReplay { reason: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum RejectionWitness {
    Shape {
        ref_len: u16,
        alt_len: u16,
    },
    Deletion {
        ref_len: u16,
        alt_len: u16,
        twice_distance: u16,
    },
    Mismatch {
        true_anchor: String,
    },
    NoGene {
        query_empty: bool,
        previous: AnnotationRow,
        following: AnnotationRow,
    },
    Context {
        side: String,
        required: i32,
        available: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AnnotationRow {
    rowid: u32,
    id: String,
    start: u32,
    end: u32,
    strand: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum PostprocessCase {
    Vector(VectorPostprocessCase),
    Round(RoundPostprocessCase),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorPostprocessCase {
    id: String,
    kind: String,
    coverage: Vec<String>,
    position: u32,
    distance: u16,
    scenario: String,
    gain_bits: Vec<String>,
    loss_bits: Vec<String>,
    genes: Vec<Gene>,
    expected: VectorExpected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    unmasked: Vec<GeneExpected>,
    masked: Vec<GeneExpected>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundPostprocessCase {
    id: String,
    kind: String,
    coverage: Vec<String>,
    position: u32,
    distance: u16,
    scenario: String,
    scalars: Vec<TypedScalar>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TypedScalar {
    dtype: String,
    bits: String,
    rendered: String,
}

impl PostprocessCase {
    fn id(&self) -> &str {
        match self {
            Self::Vector(value) => &value.id,
            Self::Round(value) => &value.id,
        }
    }

    fn coverage(&self) -> &[String] {
        match self {
            Self::Vector(value) => &value.coverage,
            Self::Round(value) => &value.coverage,
        }
    }
}

#[derive(Clone, Debug)]
enum Case {
    Model(ModelCase),
    Rejection(RejectionCase),
    Postprocess(PostprocessCase),
}

impl Case {
    fn id(&self) -> &str {
        match self {
            Self::Model(v) => &v.id,
            Self::Rejection(v) => &v.id,
            Self::Postprocess(v) => v.id(),
        }
    }
    fn coverage(&self) -> &[String] {
        match self {
            Self::Model(v) => &v.coverage,
            Self::Rejection(v) => &v.coverage,
            Self::Postprocess(v) => v.coverage(),
        }
    }
}

pub fn inspect_corpus(path: &Path) -> Result<InspectOutcome, CommandError> {
    let members = open_members(path)?;
    let manifest: Manifest = decode_json(&members["manifest.json"], "manifest.json", MANIFEST_MAX)?;
    validate_manifest(&manifest, &members)?;
    validate_notice(&members["NOTICE"])?;
    let cases = decode_cases(&members["cases.jsonl"])?;
    validate_cases(&manifest, &cases)?;
    Ok(InspectOutcome {
        status: "valid",
        schema: SCHEMA,
        profile: PROFILE,
        cases: 24,
        scored_cases: 14,
        rejection_cases: 6,
        postprocess_cases: 4,
        coverage_cells: 28,
    })
}

fn invalid(member: &str, case: Option<&str>, reason: impl AsRef<str>) -> CommandError {
    let prefix = case.map_or_else(|| member.to_owned(), |id| format!("{member}:{id}"));
    CommandError::new(
        "COMPATIBILITY_INVALID",
        format!("{prefix}: {}", reason.as_ref()),
    )
}

fn io_failure(member: &str, error: &io::Error) -> CommandError {
    CommandError::new("IO", format!("{member}: {error}"))
}

fn open_members(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, CommandError> {
    let root = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| CommandError::new("IO", format!("corpus: {error}")))?;
    let root_stat = rustix::fs::fstat(&root)
        .map_err(|error| CommandError::new("IO", format!("corpus: {error}")))?;
    if !rustix::fs::FileType::from_raw_mode(root_stat.st_mode).is_dir() {
        return Err(invalid("corpus", None, "must be a real directory"));
    }
    let mut names = Vec::new();
    let anchored_root = PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
    let entries = fs::read_dir(&anchored_root).map_err(|e| io_failure("corpus", &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_failure("corpus", &e))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("corpus", None, "member name is not UTF-8"))?;
        names.push(name);
        if names.len() > 3 {
            return Err(invalid("corpus", None, "contains extra members"));
        }
    }
    names.sort();
    if names != ["NOTICE", "cases.jsonl", "manifest.json"] {
        return Err(invalid(
            "corpus",
            None,
            "must contain exactly NOTICE, cases.jsonl, and manifest.json",
        ));
    }
    let mut result = BTreeMap::new();
    let mut total = 0_u64;
    for name in names {
        let max = match name.as_str() {
            "manifest.json" => MANIFEST_MAX,
            "cases.jsonl" => CASES_MAX,
            _ => NOTICE_MAX,
        };
        let member = rustix::fs::openat(
            &root,
            name.as_str(),
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| CommandError::new("IO", format!("{name}: {error}")))?;
        let stat = rustix::fs::fstat(&member)
            .map_err(|error| CommandError::new("IO", format!("{name}: {error}")))?;
        if !rustix::fs::FileType::from_raw_mode(stat.st_mode).is_file() || stat.st_nlink != 1 {
            return Err(invalid(&name, None, "must be a regular single-link file"));
        }
        let length = u64::try_from(stat.st_size)
            .map_err(|_| invalid(&name, None, "member has negative size"))?;
        if length > max {
            return Err(invalid(&name, None, "member exceeds its byte bound"));
        }
        total = total
            .checked_add(length)
            .ok_or_else(|| invalid("corpus", None, "aggregate byte overflow"))?;
        if total > AGGREGATE_MAX {
            return Err(invalid("corpus", None, "aggregate bytes exceed bound"));
        }
        let mut file = File::from(member);
        let capacity = usize::try_from(length)
            .map_err(|_| invalid(&name, None, "member size is not addressable"))?;
        let mut bytes = Vec::with_capacity(capacity);
        file.read_to_end(&mut bytes)
            .map_err(|e| io_failure(&name, &e))?;
        if bytes.len() != capacity {
            return Err(invalid(&name, None, "member changed while reading"));
        }
        let after = file.metadata().map_err(|e| io_failure(&name, &e))?;
        if after.len() != length || after.nlink() != 1 {
            return Err(invalid(&name, None, "member changed while reading"));
        }
        result.insert(name, bytes);
    }
    Ok(result)
}

fn decode_json<T: for<'de> Deserialize<'de> + Serialize>(
    bytes: &[u8],
    member: &str,
    max: u64,
) -> Result<T, CommandError> {
    if bytes.len() as u64 > max
        || !bytes.ends_with(b"\n")
        || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n')
    {
        return Err(invalid(
            member,
            None,
            "must be one compact JSON line with terminal LF",
        ));
    }
    let value: T = serde_json::from_slice(bytes)
        .map_err(|_| invalid(member, None, "invalid or non-closed JSON schema"))?;
    let mut canonical = serde_json::to_vec(&value)
        .map_err(|_| invalid(member, None, "cannot serialize closed JSON schema"))?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(invalid(member, None, "JSON is not canonical"));
    }
    Ok(value)
}

fn validate_manifest(
    manifest: &Manifest,
    members: &BTreeMap<String, Vec<u8>>,
) -> Result<(), CommandError> {
    let helper = embedded_helper_sha256();
    if manifest.schema != SCHEMA
        || manifest.profile != PROFILE
        || manifest.upstream.url
            != "https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7"
        || manifest.upstream.commit != "5cf94b8db938c658391b4305cd7ce33297d44ff7"
        || manifest.upstream.declared_version != "1.0.2"
        || manifest.upstream.license != "GPL-3.0-only"
        || manifest.upstream.helper_sha256 != helper
    {
        return Err(invalid(
            "manifest.json",
            None,
            "upstream profile identity mismatch",
        ));
    }
    if manifest.checkpoints.len() != 12 {
        return Err(invalid("manifest.json", None, "checkpoint count mismatch"));
    }
    for (index, (actual, expected)) in manifest.checkpoints.iter().zip(CHECKPOINTS).enumerate() {
        if actual.ordinal != (index + 1) as u8
            || actual.filename != expected.0
            || actual.bytes != 2_877_321
            || actual.sha256 != expected.1
        {
            return Err(invalid(
                "manifest.json",
                None,
                "checkpoint identity mismatch",
            ));
        }
    }
    let reference = &manifest.reference;
    if reference.source_url
        != "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz"
        || reference.source_bytes != 972_898_531
        || reference.source_sha256
            != "11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"
        || reference.assembly_report_url
            != "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt"
        || reference.assembly_report_bytes != 80_454
        || reference.assembly_report_sha256
            != "64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38"
        || reference.derived_bytes != 671_294_255
        || reference.derived_sha256
            != "81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb"
        || reference.transform
            != "select NC_000003.12, NC_000010.11, NC_000012.12, NC_000013.11, NC_000017.11, NC_012920.1; rename chr3/chr10/chr12/chr13/chr17/chrM; uppercase; preserve 80-base wrapping"
        || reference.contigs != ["chr3", "chr10", "chr12", "chr13", "chr17", "chrM"]
    {
        return Err(invalid(
            "manifest.json",
            None,
            "reference identity mismatch",
        ));
    }
    let annotation = &manifest.annotation;
    if annotation.database_url
        != "https://www.dropbox.com/sh/6zo0aegoalvgd9f/AADOhGYJo8tbUhpscp3wSFj6a/gencode.v38.annotation.db?dl=1"
        || annotation.database_bytes != 380_366_848
        || annotation.database_sha256
            != "221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6"
        || annotation.gtf_url
            != "https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/gencode.v38.annotation.gtf.gz"
        || annotation.gtf_bytes != 46_556_621
        || annotation.gtf_md5 != "16fcae8ca8e488cd8056cf317d963407"
        || annotation.gtf_sha256
            != "22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050"
        || annotation.filter != "Ensembl_canonical"
        || annotation.logical_sha256
            != "e2ea5989b7e9d9886a753534bb5d424549e0428b65d978cdc7ae71dddf945771"
    {
        return Err(invalid(
            "manifest.json",
            None,
            "annotation identity mismatch",
        ));
    }
    let env = &manifest.environment;
    if env.python != "3.13.5"
        || env.pytorch != "2.7.1+cpu"
        || env.numpy != "2.5.1"
        || env.pandas != "3.0.3"
        || env.pyfastx != "2.3.1"
        || env.gffutils != "0.14"
        || env.pyvcf3 != "1.0.4"
        || env.platform != "linux-x86_64"
        || env.cuda
        || env.helper_torch_intraop_threads != 1
        || env.helper_torch_interop_threads != 1
        || env.cli_omp_threads != 1
        || env.cli_torch_interop_threads_observed != 16
    {
        return Err(invalid(
            "manifest.json",
            None,
            "capture environment mismatch",
        ));
    }
    if manifest.coverage.iter().map(String::as_str).ne(COVERAGE)
        || manifest.case_ids.iter().map(String::as_str).ne(CASE_IDS)
    {
        return Err(invalid(
            "manifest.json",
            None,
            "ordered coverage or case IDs mismatch",
        ));
    }
    if manifest.members.len() != 2
        || manifest.members[0].filename != "cases.jsonl"
        || manifest.members[1].filename != "NOTICE"
    {
        return Err(invalid(
            "manifest.json",
            None,
            "member declarations mismatch",
        ));
    }
    for declaration in &manifest.members {
        let bytes = &members[&declaration.filename];
        if declaration.bytes != bytes.len() as u64
            || declaration.sha256 != format!("{:x}", Sha256::digest(bytes))
        {
            return Err(invalid("manifest.json", None, "member digest mismatch"));
        }
    }
    for value in [
        &reference.source_url,
        &reference.assembly_report_url,
        &annotation.database_url,
        &annotation.gtf_url,
        &reference.transform,
    ] {
        bounded_string(value, "manifest.json", None)?;
    }
    Ok(())
}

fn validate_notice(bytes: &[u8]) -> Result<(), CommandError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("NOTICE", None, "must be UTF-8"))?;
    if !text.ends_with('\n') || text.contains("\r") {
        return Err(invalid("NOTICE", None, "must use LF and end with LF"));
    }
    for required in [
        "https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7",
        "GPL-3.0",
        "Zeng",
        "Genome Biology",
        "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz",
        "https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt",
        "https://www.ncbi.nlm.nih.gov/home/about/policies/",
        "https://www.dropbox.com/sh/6zo0aegoalvgd9f/AADOhGYJo8tbUhpscp3wSFj6a/gencode.v38.annotation.db?dl=1",
        "https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/gencode.v38.annotation.gtf.gz",
        "https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/MD5SUMS",
        "https://www.gencodegenes.org/human/release_38.html",
        "https://www.gencodegenes.org/pages/data_access.html",
        "https://www.gencodegenes.org/pages/citing_gencode.html",
        "transformed",
    ] {
        if !text.contains(required) {
            return Err(invalid("NOTICE", None, "required attribution is missing"));
        }
    }
    Ok(())
}

fn decode_cases(bytes: &[u8]) -> Result<Vec<Case>, CommandError> {
    if !bytes.ends_with(b"\n") {
        return Err(invalid("cases.jsonl", None, "must end with LF"));
    }
    let mut result = Vec::new();
    for raw in bytes[..bytes.len() - 1].split(|b| *b == b'\n') {
        if raw.is_empty() {
            return Err(invalid("cases.jsonl", None, "contains an empty line"));
        }
        if raw.len() > LINE_MAX {
            return Err(invalid("cases.jsonl", None, "line exceeds byte bound"));
        }
        let head: CaseHead = serde_json::from_slice(raw)
            .map_err(|_| invalid("cases.jsonl", None, "case is not valid JSON"))?;
        bounded_token(&head.id, "cases.jsonl", Some(&head.id))?;
        let case = match head.kind.as_str() {
            "model" => Case::Model(
                serde_json::from_slice(raw)
                    .map_err(|_| invalid("cases.jsonl", Some(&head.id), "invalid model schema"))?,
            ),
            "rejection" => {
                Case::Rejection(serde_json::from_slice(raw).map_err(|_| {
                    invalid("cases.jsonl", Some(&head.id), "invalid rejection schema")
                })?)
            }
            "postprocess" => Case::Postprocess(serde_json::from_slice(raw).map_err(|_| {
                invalid("cases.jsonl", Some(&head.id), "invalid postprocess schema")
            })?),
            _ => return Err(invalid("cases.jsonl", Some(&head.id), "unknown case kind")),
        };
        let canonical = match &case {
            Case::Model(value) => serde_json::to_vec(value),
            Case::Rejection(value) => serde_json::to_vec(value),
            Case::Postprocess(value) => serde_json::to_vec(value),
        }
        .map_err(|_| invalid("cases.jsonl", Some(&head.id), "cannot serialize case"))?;
        if canonical != raw {
            return Err(invalid(
                "cases.jsonl",
                Some(&head.id),
                "case JSON is not canonical",
            ));
        }
        result.push(case);
        if result.len() > 24 {
            return Err(invalid("cases.jsonl", None, "too many cases"));
        }
    }
    if result.len() != 24 {
        return Err(invalid("cases.jsonl", None, "case count mismatch"));
    }
    Ok(result)
}

#[derive(Deserialize)]
struct CaseHead {
    id: String,
    kind: String,
}

fn validate_cases(manifest: &Manifest, cases: &[Case]) -> Result<(), CommandError> {
    let ids: Vec<_> = cases.iter().map(Case::id).collect();
    if ids != CASE_IDS {
        return Err(invalid(
            "cases.jsonl",
            None,
            "case order or identity mismatch",
        ));
    }
    let allowed: BTreeSet<_> = COVERAGE.into_iter().collect();
    let mut observed = BTreeSet::new();
    for case in cases {
        for cell in case.coverage() {
            if !allowed.contains(cell.as_str()) {
                return Err(invalid(
                    "cases.jsonl",
                    Some(case.id()),
                    "unsupported coverage cell",
                ));
            }
            observed.insert(cell.as_str());
        }
        match case {
            Case::Model(value) => validate_model(value)?,
            Case::Rejection(value) => validate_rejection(value)?,
            Case::Postprocess(value) => validate_postprocess(value)?,
        }
    }
    if observed != allowed {
        return Err(invalid("cases.jsonl", None, "coverage is incomplete"));
    }
    let logical = logical_annotation_sha(cases);
    if manifest.annotation.logical_sha256 != logical {
        return Err(invalid(
            "manifest.json",
            None,
            "logical annotation digest mismatch",
        ));
    }
    Ok(())
}

fn fixed_strands(id: &str) -> &'static [(&'static str, &'static [&'static str])] {
    match id {
        "M01-snv-cd4-precomputed"
        | "M06-snv-gene-start-plus-one"
        | "M07-mnv-plus"
        | "M09-insertion-short-plus"
        | "M12-deletion-short-plus" => &[("+", &["ENSG00000010610.10"])],
        "M02-snv-wrap53-tp53-precomputed"
        | "M08-mnv-both-strands"
        | "M10-insertion-short-both"
        | "M13-deletion-short-both" => &[
            ("+", &["ENSG00000141499.17"]),
            ("-", &["ENSG00000141510.18"]),
        ],
        "M03-snv-afap1l2-precomputed" => &[("-", &["ENSG00000169129.15"])],
        "M04-snv-grk1-precomputed" => &[("+", &["ENSG00000185974.7"])],
        "M05-snv-same-strand-overlap"
        | "M11-insertion-long-overlap"
        | "M14-deletion-ref100-overlap" => &[("+", &["ENSG00000283563.1", "ENSG00000144642.22"])],
        _ => &[],
    }
}

fn validate_model(case: &ModelCase) -> Result<(), CommandError> {
    let index = CASE_IDS[..14]
        .iter()
        .position(|id| *id == case.id)
        .ok_or_else(|| invalid("cases.jsonl", Some(&case.id), "unknown model case"))?;
    let plan = model_plans()
        .into_iter()
        .nth(index)
        .ok_or_else(|| invalid("cases.jsonl", Some(&case.id), "missing fixed model plan"))?;
    if case.kind != "model" || case.input.assembly != "GRCh38" || case.input.distance != 50 {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "model profile mismatch",
        ));
    }
    validate_input(&case.input, &case.id)?;
    if case.input.contig != plan.contig
        || case.input.position != plan.position
        || case.input.reference != plan.reference
        || case.input.alt != plan.alt
        || case.input.distance != plan.distance
        || case.input.allele_shape != plan.allele_shape
        || case.coverage != plan.coverage
        || case.precomputed != plan.precomputed
    {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "fixed model case identity mismatch",
        ));
    }
    let expected_len = 100 + case.input.reference.len();
    if case.context.anchor_offset != 5050
        || case.context.start_1based != case.input.position.saturating_sub(5050)
        || case.context.bases.len() != 10_100 + case.input.reference.len()
        || case.context.bases.len() > CONTEXT_MAX
        || case.context.sha256 != format!("{:x}", Sha256::digest(case.context.bases.as_bytes()))
    {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "context contract mismatch",
        ));
    }
    if !dna(&case.context.bases)
        || case
            .context
            .bases
            .get(5050..5050 + case.input.reference.len())
            != Some(case.input.reference.as_str())
    {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "context REF anchor mismatch",
        ));
    }
    if case.context.sha256 != CONTEXT_SHAS[index] {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "fixed context identity mismatch",
        ));
    }
    let expected_strands = fixed_strands(&case.id);
    if case.strands.len() != expected_strands.len() {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "invalid strand count",
        ));
    }
    let mut order = String::new();
    for (strand, (expected_strand, expected_genes)) in
        case.strands.iter().zip(expected_strands.iter())
    {
        if !matches!(strand.strand.as_str(), "+" | "-") || order.as_str() >= strand.strand.as_str()
        {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "invalid strand order",
            ));
        }
        if strand.genes.is_empty() || strand.genes.len() > GENE_MAX {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "gene count out of bounds",
            ));
        }
        validate_genes(&strand.genes, &case.id)?;
        if strand.strand != *expected_strand
            || strand
                .genes
                .iter()
                .map(|gene| gene.id.as_str())
                .ne(expected_genes.iter().copied())
        {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "fixed strand or gene order mismatch",
            ));
        }
        order = strand.strand.clone();
        if strand.loss_bits.len() != expected_len
            || strand.gain_bits.len() != expected_len
            || expected_len > ARRAY_MAX
        {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "score array length mismatch",
            ));
        }
        let expected_dtype = if case.input.allele_shape == "deletion_anchored" {
            "f64"
        } else {
            "f32"
        };
        if strand.dtype != expected_dtype {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "shape and score dtype disagree",
            ));
        }
        let loss = parse_typed_bits(&strand.dtype, &strand.loss_bits, &case.id)?;
        let gain = parse_typed_bits(&strand.dtype, &strand.gain_bits, &case.id)?;
        let unmasked = replay_typed_scores(
            loss.clone(),
            gain.clone(),
            &strand.genes,
            case.input.position,
            case.input.distance,
            false,
            &case.id,
        )?;
        let masked = replay_typed_scores(
            loss,
            gain,
            &strand.genes,
            case.input.position,
            case.input.distance,
            true,
            &case.id,
        )?;
        if unmasked != strand.expected.unmasked || masked != strand.expected.masked {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "semantic score replay mismatch",
            ));
        }
        let cli_unmasked = format_cli(&unmasked, &strand.dtype, &case.id)?;
        let cli_masked = format_cli(&masked, &strand.dtype, &case.id)?;
        if cli_unmasked != strand.expected.cli_unmasked || cli_masked != strand.expected.cli_masked
        {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "public output replay mismatch",
            ));
        }
        let max_pos = case.input.reference.len() as i32 + 49;
        for expected in unmasked.iter().chain(masked.iter()) {
            if !(-50..=max_pos).contains(&expected.gain_position)
                || !(-50..=max_pos).contains(&expected.loss_position)
            {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "relative position out of range",
                ));
            }
        }
    }
    let required_precomputed = case.id.starts_with("M01")
        || case.id.starts_with("M02")
        || case.id.starts_with("M03")
        || case.id.starts_with("M04");
    if required_precomputed == case.precomputed.is_empty() {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "precomputed observation presence mismatch",
        ));
    }
    for value in &case.precomputed {
        parse_bit(&value.gain_bits, &case.id)?;
        parse_bit(&value.loss_bits, &case.id)?;
        bounded_string(&value.source_member, "cases.jsonl", Some(&case.id))?;
    }
    Ok(())
}

fn validate_input(input: &VariantInput, id: &str) -> Result<(), CommandError> {
    for value in [
        &input.assembly,
        &input.contig,
        &input.reference,
        &input.alt,
        &input.allele_shape,
    ] {
        bounded_token(value, "cases.jsonl", Some(id))?;
    }
    if input.position == 0 || !dna(&input.reference) || !dna(&input.alt) {
        return Err(invalid("cases.jsonl", Some(id), "invalid variant input"));
    }
    let shape = if input.reference.len() == 1 && input.alt.len() == 1 {
        "snv"
    } else if input.reference.len() == input.alt.len() {
        "mnv_equal"
    } else if input.reference.len() == 1 {
        "insertion_anchored"
    } else if input.alt.len() == 1 {
        "deletion_anchored"
    } else {
        "complex_unequal"
    };
    if input.allele_shape != shape {
        return Err(invalid("cases.jsonl", Some(id), "allele shape mismatch"));
    }
    Ok(())
}

fn validate_genes(genes: &[Gene], id: &str) -> Result<(), CommandError> {
    validate_gene_bounds(genes, id, true)
}

fn validate_gene_bounds(
    genes: &[Gene],
    id: &str,
    require_boundary_pairs: bool,
) -> Result<(), CommandError> {
    let mut ids = BTreeSet::new();
    for gene in genes {
        bounded_token(&gene.id, "cases.jsonl", Some(id))?;
        if !ids.insert(&gene.id)
            || gene.boundaries.len() > BOUNDARY_MAX
            || (require_boundary_pairs && gene.boundaries.len() % 2 != 0)
            || gene.boundaries.contains(&0)
        {
            return Err(invalid(
                "cases.jsonl",
                Some(id),
                "invalid gene or boundaries",
            ));
        }
    }
    Ok(())
}

fn validate_rejection(case: &RejectionCase) -> Result<(), CommandError> {
    let expected_inputs = rejection_inputs();
    let index = expected_inputs
        .iter()
        .position(|input| input.id == case.id)
        .ok_or_else(|| invalid("cases.jsonl", Some(&case.id), "unknown rejection case"))?;
    let expected = &expected_inputs[index];
    if case.kind != "rejection" {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "wrong rejection kind",
        ));
    }
    if case.input.assembly != "GRCh38"
        || case.input.contig != expected.contig
        || case.input.position != expected.position
        || case.input.reference != expected.reference
        || case.input.alt != expected.alt
        || case.input.distance != 50
        || case.coverage != [COVERAGE[18 + index]]
    {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "fixed rejection case identity mismatch",
        ));
    }
    validate_input(&case.input, &case.id)?;
    let valid = match (
        &case.witness,
        case.normalized_category.as_str(),
        case.first_operation.as_str(),
    ) {
        (
            RejectionWitness::Shape { ref_len, alt_len },
            "unsupported_variant_shape",
            "variant_shape_guard",
        ) => {
            *ref_len == case.input.reference.len() as u16
                && *alt_len == case.input.alt.len() as u16
                && *ref_len > 1
                && *alt_len > 1
                && ref_len != alt_len
        }
        (
            RejectionWitness::Deletion {
                ref_len,
                alt_len,
                twice_distance,
            },
            "deletion_too_large",
            "deletion_length_guard",
        ) => {
            *ref_len == 101 && *alt_len == 1 && *twice_distance == 100 && *ref_len > *twice_distance
        }
        (
            RejectionWitness::Mismatch { true_anchor },
            "reference_mismatch",
            "reference_anchor_compare",
        ) => dna(true_anchor) && true_anchor != &case.input.reference,
        (
            RejectionWitness::NoGene {
                query_empty,
                previous,
                following,
            },
            "not_in_gene",
            "get_genes_empty",
        ) => {
            *query_empty
                && previous.rowid == 244405
                && previous.id == "ENSG00000126746.18"
                && previous.start == 6666477
                && previous.end == 6689572
                && previous.strand == "-"
                && following.rowid == 244419
                && following.id == "ENSG00000139200.14"
                && following.start == 6693791
                && following.end == 6700815
                && following.strand == "-"
                && previous.end < case.input.position
                && following.start > case.input.position
        }
        (
            RejectionWitness::Context {
                side,
                required,
                available,
            },
            "insufficient_reference_context",
            "reference_slice",
        ) => {
            let computed = if side == "left" {
                i64::from(case.input.position) - 5_050
            } else {
                i64::from(case.input.position) + case.input.reference.len() as i64 + 5_049
            };
            i64::from(*required) == computed
                && ((side == "left" && *available == 1 && computed < i64::from(*available))
                    || (side == "right"
                        && *available == 16_569
                        && computed > i64::from(*available)))
        }
        _ => false,
    };
    if !valid {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "rejection replay mismatch",
        ));
    }
    match (&case.upstream_evidence, case.id.as_str()) {
        (UpstreamEvidence::Cli { warning }, id)
            if !matches!(id, "R05-left-context" | "R06-right-context") =>
        {
            bounded_string(warning, "cases.jsonl", Some(&case.id))
        }
        (UpstreamEvidence::RuleReplay { reason }, id)
            if matches!(id, "R05-left-context" | "R06-right-context")
                && reason == "excluded_from_cli_native_reference_slice_crash" =>
        {
            Ok(())
        }
        _ => Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "upstream evidence boundary mismatch",
        )),
    }
}

fn gene_expected(
    gene: &str,
    gain_bits: &str,
    gain_position: i32,
    loss_bits: &str,
    loss_position: i32,
) -> GeneExpected {
    GeneExpected {
        gene: gene.into(),
        gain_bits: gain_bits.into(),
        gain_position,
        loss_bits: loss_bits.into(),
        loss_position,
    }
}

fn fixed_vector_case(scenario: &str) -> Option<VectorPostprocessCase> {
    let (id, coverage, gain, loss, genes, unmasked, masked) = match scenario {
        "order-v1" => (
            "P01-same-strand-order",
            "postprocess.same_strand_order",
            ["3dcccccd", "3f4ccccd", "3e4ccccd", "3f333333", "3e99999a"],
            ["bdcccccd", "bf19999a", "be4ccccd", "bf000000", "becccccd"],
            vec![
                Gene {
                    id: "GENE_A".into(),
                    boundaries: vec![99],
                },
                Gene {
                    id: "GENE_B".into(),
                    boundaries: vec![101],
                },
            ],
            vec![
                gene_expected("GENE_A", "3f4ccccd", -1, "bf19999a", -1),
                gene_expected("GENE_B", "3f4ccccd", -1, "bf19999a", -1),
            ],
            vec![
                gene_expected("GENE_A", "3f333333", 1, "bf19999a", -1),
                gene_expected("GENE_B", "3e99999a", 2, "00000000", -2),
            ],
        ),
        "empty-v1" => (
            "P02-empty-boundaries",
            "postprocess.empty_boundaries",
            ["3dcccccd", "3e4ccccd", "3e99999a", "3e4ccccd", "3dcccccd"],
            ["becccccd", "be99999a", "be4ccccd", "bdcccccd", "bf000000"],
            vec![Gene {
                id: "GENE_EMPTY".into(),
                boundaries: vec![],
            }],
            vec![gene_expected("GENE_EMPTY", "3e99999a", 0, "bf000000", 2)],
            vec![gene_expected("GENE_EMPTY", "3e99999a", 0, "00000000", -2)],
        ),
        "tie-v1" => (
            "P03-first-extremum",
            "postprocess.first_extremum",
            ["3dcccccd", "3f4ccccd", "3f4ccccd", "3e4ccccd", "00000000"],
            ["bf000000", "bdcccccd", "bf000000", "00000000", "00000000"],
            vec![],
            vec![gene_expected("UNMASKED", "3f4ccccd", -1, "bf000000", -2)],
            vec![],
        ),
        _ => return None,
    };
    Some(VectorPostprocessCase {
        id: id.into(),
        kind: "postprocess".into(),
        coverage: vec![coverage.into()],
        position: 100,
        distance: 2,
        scenario: scenario.into(),
        gain_bits: gain.into_iter().map(Into::into).collect(),
        loss_bits: loss.into_iter().map(Into::into).collect(),
        genes,
        expected: VectorExpected { unmasked, masked },
    })
}

fn validate_postprocess(case: &PostprocessCase) -> Result<(), CommandError> {
    let PostprocessCase::Vector(case) = case else {
        let PostprocessCase::Round(case) = case else {
            unreachable!()
        };
        if case.kind != "postprocess"
            || case.id != "P04-rounding-signed-zero"
            || case.position != 100
            || case.distance != 2
            || case.scenario != "round-v1"
            || case.coverage != ["postprocess.rounding_signed_zero"]
            || case.scalars.len() != 12
        {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "rounding profile mismatch",
            ));
        }
        let expected = rounding_scalars();
        if case.scalars != expected {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "rounding replay mismatch",
            ));
        }
        for scalar in &case.scalars {
            if render_bits(&scalar.dtype, &scalar.bits, &case.id)? != scalar.rendered {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "rounding semantic mismatch",
                ));
            }
        }
        return Ok(());
    };
    if case.gain_bits.len() > ARRAY_MAX
        || case.loss_bits.len() > ARRAY_MAX
        || case.genes.len() > GENE_MAX
    {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "postprocess profile mismatch",
        ));
    }
    let fixed = fixed_vector_case(&case.scenario).ok_or_else(|| {
        invalid(
            "cases.jsonl",
            Some(&case.id),
            "unknown postprocess scenario",
        )
    })?;
    if case != &fixed {
        return Err(invalid(
            "cases.jsonl",
            Some(&case.id),
            "fixed postprocess vector mismatch",
        ));
    }
    let gain = parse_bits(&case.gain_bits, &case.id)?;
    let loss = parse_bits(&case.loss_bits, &case.id)?;
    validate_gene_bounds(&case.genes, &case.id, false)?;
    match case.scenario.as_str() {
        "order-v1" => {
            if gain.len() != 5 || loss.len() != 5 || case.genes.len() != 2 {
                return Err(invalid("cases.jsonl", Some(&case.id), "order vector shape"));
            }
            let unmasked = score_genes(
                &loss,
                &gain,
                &case.genes,
                case.position,
                case.distance,
                false,
                &case.id,
            )?;
            let masked = score_genes(
                &loss,
                &gain,
                &case.genes,
                case.position,
                case.distance,
                true,
                &case.id,
            )?;
            if unmasked != case.expected.unmasked || masked != case.expected.masked {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "order replay mismatch",
                ));
            }
        }
        "empty-v1" => {
            if case.genes.len() != 1 || !case.genes[0].boundaries.is_empty() {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "empty-boundary shape",
                ));
            }
            let unmasked = score_genes(
                &loss,
                &gain,
                &case.genes,
                case.position,
                case.distance,
                false,
                &case.id,
            )?;
            let masked = score_genes(
                &loss,
                &gain,
                &case.genes,
                case.position,
                case.distance,
                true,
                &case.id,
            )?;
            if unmasked != case.expected.unmasked || masked != case.expected.masked {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "empty replay mismatch",
                ));
            }
        }
        "tie-v1" => {
            let expected = score_genes(
                &loss,
                &gain,
                &[Gene {
                    id: "UNMASKED".into(),
                    boundaries: vec![],
                }],
                case.position,
                case.distance,
                false,
                &case.id,
            )?;
            if expected != case.expected.unmasked {
                return Err(invalid(
                    "cases.jsonl",
                    Some(&case.id),
                    "tie replay mismatch",
                ));
            }
        }
        _ => {
            return Err(invalid(
                "cases.jsonl",
                Some(&case.id),
                "unknown postprocess scenario",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum TypedScores {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

fn parse_typed_bits(dtype: &str, values: &[String], id: &str) -> Result<TypedScores, CommandError> {
    match dtype {
        "f32" => values
            .iter()
            .map(|value| parse_bit(value, id))
            .collect::<Result<Vec<_>, _>>()
            .map(TypedScores::F32),
        "f64" => values
            .iter()
            .map(|value| parse_bit64(value, id))
            .collect::<Result<Vec<_>, _>>()
            .map(TypedScores::F64),
        _ => Err(invalid("cases.jsonl", Some(id), "unsupported score dtype")),
    }
}

fn parse_bits(values: &[String], id: &str) -> Result<Vec<f32>, CommandError> {
    values.iter().map(|v| parse_bit(v, id)).collect()
}
fn parse_bit(value: &str, id: &str) -> Result<f32, CommandError> {
    if value.len() != 8
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("cases.jsonl", Some(id), "malformed f32 bits"));
    }
    let bits = u32::from_str_radix(value, 16)
        .map_err(|_| invalid("cases.jsonl", Some(id), "malformed f32 bits"))?;
    let value = f32::from_bits(bits);
    if !value.is_finite() {
        return Err(invalid("cases.jsonl", Some(id), "nonfinite f32 bits"));
    }
    Ok(value)
}

fn parse_bit64(value: &str, id: &str) -> Result<f64, CommandError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("cases.jsonl", Some(id), "malformed f64 bits"));
    }
    let bits = u64::from_str_radix(value, 16)
        .map_err(|_| invalid("cases.jsonl", Some(id), "malformed f64 bits"))?;
    let value = f64::from_bits(bits);
    if !value.is_finite() {
        return Err(invalid("cases.jsonl", Some(id), "nonfinite f64 bits"));
    }
    Ok(value)
}

trait ReplayNumber: Copy + PartialOrd {
    fn zero() -> Self;
    fn bits_hex(self) -> String;
    fn public_render(self) -> String;
}

impl ReplayNumber for f32 {
    fn zero() -> Self {
        0.0
    }
    fn bits_hex(self) -> String {
        format!("{:08x}", self.to_bits())
    }
    fn public_render(self) -> String {
        render_f32(self)
    }
}

impl ReplayNumber for f64 {
    fn zero() -> Self {
        0.0
    }
    fn bits_hex(self) -> String {
        format!("{:016x}", self.to_bits())
    }
    fn public_render(self) -> String {
        render_f64(self)
    }
}

fn replay_typed_scores(
    loss: TypedScores,
    gain: TypedScores,
    genes: &[Gene],
    position: u32,
    distance: u16,
    masked: bool,
    id: &str,
) -> Result<Vec<GeneExpected>, CommandError> {
    match (loss, gain) {
        (TypedScores::F32(loss), TypedScores::F32(gain)) => {
            score_genes(&loss, &gain, genes, position, distance, masked, id)
        }
        (TypedScores::F64(loss), TypedScores::F64(gain)) => {
            score_genes(&loss, &gain, genes, position, distance, masked, id)
        }
        _ => Err(invalid("cases.jsonl", Some(id), "score dtype disagreement")),
    }
}

fn score_genes<T: ReplayNumber>(
    loss: &[T],
    gain: &[T],
    genes: &[Gene],
    position: u32,
    distance: u16,
    masked: bool,
    id: &str,
) -> Result<Vec<GeneExpected>, CommandError> {
    if loss.len() != gain.len() || loss.is_empty() {
        return Err(invalid("cases.jsonl", Some(id), "score vector shape"));
    }
    let mut loss = loss.to_vec();
    let mut gain = gain.to_vec();
    let mut result = Vec::new();
    for gene in genes {
        if masked {
            let indices: Vec<usize> = gene
                .boundaries
                .iter()
                .filter_map(|absolute| {
                    let relative = i64::from(*absolute) - i64::from(position - distance as u32);
                    usize::try_from(relative)
                        .ok()
                        .filter(|index| *index < loss.len())
                })
                .collect();
            if indices.is_empty() && gene.boundaries.is_empty() {
                for value in &mut loss {
                    *value = numpy_maximum(*value, T::zero());
                }
            } else {
                for index in &indices {
                    gain[*index] = numpy_minimum(gain[*index], T::zero());
                }
                let set: BTreeSet<_> = indices.into_iter().collect();
                for (index, value) in loss.iter_mut().enumerate() {
                    if !set.contains(&index) {
                        *value = numpy_maximum(*value, T::zero());
                    }
                }
            }
        }
        let gi = first_max(&gain);
        let li = first_min(&loss);
        result.push(GeneExpected {
            gene: gene.id.clone(),
            gain_bits: gain[gi].bits_hex(),
            gain_position: gi as i32 - distance as i32,
            loss_bits: loss[li].bits_hex(),
            loss_position: li as i32 - distance as i32,
        });
    }
    Ok(result)
}
fn numpy_minimum<T: Copy + PartialOrd>(left: T, right: T) -> T {
    if left < right { left } else { right }
}
fn numpy_maximum<T: Copy + PartialOrd>(left: T, right: T) -> T {
    if left > right { left } else { right }
}
fn first_max<T: PartialOrd>(values: &[T]) -> usize {
    let mut best = 0;
    for i in 1..values.len() {
        if values[i] > values[best] {
            best = i
        }
    }
    best
}
fn first_min<T: PartialOrd>(values: &[T]) -> usize {
    let mut best = 0;
    for i in 1..values.len() {
        if values[i] < values[best] {
            best = i
        }
    }
    best
}
fn format_cli(values: &[GeneExpected], dtype: &str, id: &str) -> Result<String, CommandError> {
    values
        .iter()
        .map(|v| {
            let gain = render_bits(dtype, &v.gain_bits, id)?;
            let loss = render_bits(dtype, &v.loss_bits, id)?;
            Ok(format!(
                "{}|{}:{}|{}:{}|Warnings:",
                v.gene, v.gain_position, gain, v.loss_position, loss
            ))
        })
        .collect::<Result<Vec<_>, CommandError>>()
        .map(|parts| parts.join(","))
}
fn render_bits(dtype: &str, bits: &str, id: &str) -> Result<String, CommandError> {
    match dtype {
        "f32" => parse_bit(bits, id).map(ReplayNumber::public_render),
        "f64" => parse_bit64(bits, id).map(ReplayNumber::public_render),
        _ => Err(invalid("cases.jsonl", Some(id), "unsupported score dtype")),
    }
}
fn render_f32(value: f32) -> String {
    // NumPy 2.5.1 keeps every operation in binary32 for an `np.float32`
    // scalar. Its empty f-string formatter then exposes the exact widened
    // binary32 value rather than a cosmetic two-decimal spelling.
    let rounded = (value * 100.0_f32).round_ties_even() / 100.0_f32;
    if rounded == 0.0 {
        if rounded.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        }
    } else {
        let mut rendered = format!("{}", f64::from(rounded));
        if !rendered.contains(['.', 'e', 'E']) {
            rendered.push_str(".0");
        }
        rendered
    }
}
fn render_f64(value: f64) -> String {
    let rounded = (value * 100.0_f64).round_ties_even() / 100.0_f64;
    render_f64_scalar(rounded)
}
fn render_f64_scalar(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let mut rendered = format!("{value}");
    if !rendered.contains(['.', 'e', 'E']) {
        rendered.push_str(".0");
    }
    rendered
}

fn rounding_scalars() -> Vec<TypedScalar> {
    [
        ("f32", "00000000", "0.0"),
        ("f32", "80000000", "-0.0"),
        ("f32", "3ba3d70a", "0.0"),
        ("f32", "bba3d70a", "-0.0"),
        ("f32", "3f80a3d7", "1.0"),
        ("f32", "bf80a3d7", "-1.0"),
        ("f32", "3e570a3d", "0.20999999344348907"),
        ("f32", "bc23d70a", "-0.009999999776482582"),
        ("f64", "0000000000000000", "0.0"),
        ("f64", "8000000000000000", "-0.0"),
        ("f64", "3fcae147ae147ae1", "0.21"),
        ("f64", "bfa999999999999a", "-0.05"),
    ]
    .into_iter()
    .map(|(dtype, bits, rendered)| TypedScalar {
        dtype: dtype.to_owned(),
        bits: bits.to_owned(),
        rendered: rendered.to_owned(),
    })
    .collect()
}
fn dna(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| matches!(b, b'A' | b'C' | b'G' | b'T' | b'N'))
}
fn bounded_string(value: &str, member: &str, case: Option<&str>) -> Result<(), CommandError> {
    if value.len() > STRING_MAX {
        Err(invalid(member, case, "string exceeds bound"))
    } else {
        Ok(())
    }
}
fn bounded_token(value: &str, member: &str, case: Option<&str>) -> Result<(), CommandError> {
    if value.is_empty() || value.len() > TOKEN_MAX {
        Err(invalid(member, case, "token exceeds bound"))
    } else {
        Ok(())
    }
}
fn logical_annotation_sha(cases: &[Case]) -> String {
    let mut hash = Sha256::new();
    for case in cases {
        match case {
            Case::Model(v) => {
                for strand in &v.strands {
                    hash.update(v.id.as_bytes());
                    hash.update(strand.strand.as_bytes());
                    for gene in &strand.genes {
                        hash.update(gene.id.as_bytes());
                        for boundary in &gene.boundaries {
                            hash.update(boundary.to_be_bytes());
                        }
                    }
                }
            }
            Case::Rejection(v) => {
                if let RejectionWitness::NoGene {
                    previous,
                    following,
                    ..
                } = &v.witness
                {
                    for row in [previous, following] {
                        hash.update(row.rowid.to_be_bytes());
                        hash.update(row.id.as_bytes());
                        hash.update(row.start.to_be_bytes());
                        hash.update(row.end.to_be_bytes());
                        hash.update(row.strand.as_bytes());
                    }
                }
            }
            Case::Postprocess(_) => {}
        }
    }
    format!("{:x}", hash.finalize())
}

static CAPTURE_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelPlan {
    id: String,
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    alt: String,
    distance: u16,
    allele_shape: String,
    coverage: Vec<String>,
    precomputed: Vec<Precomputed>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperObservation {
    id: String,
    imported_module: String,
    strands: Vec<HelperStrand>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperStrand {
    strand: String,
    dtype: String,
    loss_bits: Vec<String>,
    gain_bits: Vec<String>,
    genes: Vec<Gene>,
}

pub fn capture_corpus(arguments: &CaptureArguments) -> Result<CaptureOutcome, CommandError> {
    if fs::symlink_metadata(&arguments.output).is_ok() {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "output already exists",
        ));
    }
    preflight(arguments)?;
    let parent = arguments
        .output
        .parent()
        .ok_or_else(|| CommandError::new("IO", "output has no parent"))?;
    let name = arguments
        .output
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| CommandError::new("IO", "output name is not UTF-8"))?;
    let serial = CAPTURE_SERIAL.fetch_add(1, Ordering::Relaxed);
    let staging = parent.join(format!(".{name}.capture-{}-{serial}", std::process::id()));
    fs::create_dir(&staging)
        .map_err(|e| CommandError::new("IO", format!("capture staging: {e}")))?;
    let outcome = match capture_into(arguments, &staging) {
        Ok(outcome) => outcome,
        Err(error) => return Err(cleanup_unpublished_staging(&staging, error)),
    };
    publish_staging(&staging, parent, &arguments.output)?;
    Ok(outcome)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyncPoint {
    Staging,
    ParentAfterPublish,
}

fn cleanup_unpublished_staging(staging: &Path, original: CommandError) -> CommandError {
    match fs::remove_dir_all(staging) {
        Ok(()) => original,
        Err(error) if error.kind() == io::ErrorKind::NotFound => original,
        Err(error) => CommandError::new(
            "IO",
            format!(
                "{}; capture staging cleanup failed: {error}",
                original.message
            ),
        ),
    }
}

fn publish_staging(staging: &Path, parent: &Path, output: &Path) -> Result<(), CommandError> {
    publish_staging_with(
        staging,
        parent,
        output,
        |path, _| sync_directory(path),
        |source, destination| {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                source,
                rustix::fs::CWD,
                destination,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| CommandError::new("IO", format!("publish corpus: {error}")))
        },
    )
}

fn publish_staging_with(
    staging: &Path,
    parent: &Path,
    output: &Path,
    mut sync: impl FnMut(&Path, SyncPoint) -> Result<(), CommandError>,
    mut rename: impl FnMut(&Path, &Path) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    if let Err(error) = sync(staging, SyncPoint::Staging) {
        return Err(cleanup_unpublished_staging(staging, error));
    }
    if let Err(error) = rename(staging, output) {
        return Err(cleanup_unpublished_staging(staging, error));
    }
    // Once rename succeeds the corpus is published and must never be removed
    // as "cleanup." A parent-directory sync failure is reported honestly while
    // preserving the complete published output for inspection/recovery.
    sync(parent, SyncPoint::ParentAfterPublish)
}

fn preflight(arguments: &CaptureArguments) -> Result<(), CommandError> {
    let python = arguments
        .python
        .canonicalize()
        .map_err(|e| CommandError::new("IO", format!("python: {e}")))?;
    let python_meta =
        fs::metadata(&python).map_err(|e| CommandError::new("IO", format!("python: {e}")))?;
    if !python_meta.is_file() {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "python must resolve to a regular file",
        ));
    }
    require_file(
        &arguments.reference_source,
        Some(972_898_531),
        Some("11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"),
        "reference source",
    )?;
    require_file(
        &arguments.assembly_report,
        Some(80_454),
        Some("64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38"),
        "assembly report",
    )?;
    require_file(
        &arguments.reference,
        Some(671_294_255),
        Some("81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb"),
        "derived reference",
    )?;
    require_file(
        &arguments.annotation_db,
        Some(380_366_848),
        Some("221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6"),
        "annotation database",
    )?;
    require_file(
        &arguments.annotation_gtf,
        Some(46_556_621),
        Some("22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050"),
        "annotation GTF",
    )?;
    let head = command_text(
        Command::new("git")
            .args(["-C"])
            .arg(&arguments.upstream)
            .args(["rev-parse", "HEAD"]),
        "upstream revision",
    )?;
    if head.trim() != "5cf94b8db938c658391b4305cd7ce33297d44ff7" {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "upstream revision mismatch",
        ));
    }
    let diff = Command::new("git")
        .args(["-C"])
        .arg(&arguments.upstream)
        .args([
            "diff",
            "--quiet",
            "--",
            "setup.py",
            "pangolin/__init__.py",
            "pangolin/pangolin.py",
            "pangolin/model.py",
            "pangolin/models",
        ])
        .status()
        .map_err(|e| CommandError::new("IO", format!("upstream diff: {e}")))?;
    if !diff.success() {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "tracked upstream source is modified",
        ));
    }
    authenticate_upstream_sources(&arguments.upstream)?;
    for (filename, digest) in CHECKPOINTS {
        require_file(
            &arguments.upstream.join("pangolin/models").join(filename),
            Some(2_877_321),
            Some(digest),
            filename,
        )?;
    }
    let probe = r#"import importlib.metadata,inspect,json,platform,sys; import pangolin.pangolin as p; import torch,numpy,pandas,pyfastx,gffutils; print(json.dumps({'python':sys.version.split()[0],'pytorch':torch.__version__,'numpy':numpy.__version__,'pandas':pandas.__version__,'pyfastx':pyfastx.__version__,'gffutils':gffutils.__version__,'pyvcf3':importlib.metadata.version('PyVCF3'),'platform':platform.system().lower()+'-'+platform.machine(),'pangolin':importlib.metadata.version('pangolin'),'module':inspect.getfile(p),'cuda':torch.cuda.is_available()},sort_keys=True))"#;
    let mut command = Command::new(&arguments.python);
    command.arg("-c").arg(probe);
    capture_env(&mut command, &arguments.upstream);
    let output = command_text(&mut command, "capture environment")?;
    let value: serde_json::Value = serde_json::from_str(output.trim()).map_err(|_| {
        CommandError::new(
            "COMPATIBILITY_INVALID",
            "capture environment probe is not JSON",
        )
    })?;
    let expected_module = arguments
        .upstream
        .join("pangolin/pangolin.py")
        .canonicalize()
        .map_err(|e| CommandError::new("IO", format!("upstream module: {e}")))?;
    let valid = value["python"] == "3.13.5"
        && value["pytorch"] == "2.7.1+cpu"
        && value["numpy"] == "2.5.1"
        && value["pandas"] == "3.0.3"
        && value["pyfastx"] == "2.3.1"
        && value["gffutils"] == "0.14"
        && value["pyvcf3"] == "1.0.4"
        && value["platform"] == "linux-x86_64"
        && value["pangolin"] == "1.0.2"
        && value["cuda"] == false
        && value["module"].as_str().is_some_and(|path| {
            Path::new(path).canonicalize().ok().as_ref() == Some(&expected_module)
        });
    if !valid {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "capture environment or imported module mismatch",
        ));
    }
    probe_execution_profiles(arguments)?;
    Ok(())
}

fn probe_execution_profiles(arguments: &CaptureArguments) -> Result<(), CommandError> {
    let helper_probe = r#"import json,torch; torch.set_num_threads(1); torch.set_num_interop_threads(1); print(json.dumps({'cuda':torch.cuda.is_available(),'intraop':torch.get_num_threads(),'interop':torch.get_num_interop_threads()},sort_keys=True))"#;
    let mut helper = Command::new(&arguments.python);
    helper.arg("-c").arg(helper_probe);
    capture_env(&mut helper, &arguments.upstream);
    let helper: serde_json::Value =
        serde_json::from_str(command_text(&mut helper, "helper execution-profile probe")?.trim())
            .map_err(|_| {
            CommandError::new(
                "COMPATIBILITY_INVALID",
                "helper execution-profile probe is not JSON",
            )
        })?;
    if helper["cuda"] != false || helper["intraop"] != 1 || helper["interop"] != 1 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "helper execution profile mismatch",
        ));
    }

    let cli_probe = r#"import json,os,torch; print(json.dumps({'cuda':torch.cuda.is_available(),'omp':os.environ.get('OMP_NUM_THREADS'),'intraop':torch.get_num_threads(),'interop':torch.get_num_interop_threads()},sort_keys=True))"#;
    let mut cli = Command::new(&arguments.python);
    cli.arg("-c").arg(cli_probe);
    capture_env(&mut cli, &arguments.upstream);
    let cli: serde_json::Value =
        serde_json::from_str(command_text(&mut cli, "CLI execution-profile probe")?.trim())
            .map_err(|_| {
                CommandError::new(
                    "COMPATIBILITY_INVALID",
                    "CLI execution-profile probe is not JSON",
                )
            })?;
    if cli["cuda"] != false || cli["omp"] != "1" || cli["intraop"] != 1 || cli["interop"] != 16 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "CLI execution profile mismatch",
        ));
    }
    Ok(())
}

fn authenticate_upstream_sources(upstream: &Path) -> Result<(), CommandError> {
    for source in UPSTREAM_SOURCES {
        authenticate_upstream_source(upstream, source)?;
    }
    Ok(())
}

fn authenticate_upstream_source(
    upstream: &Path,
    (relative, bytes, sha256): (&str, u64, &str),
) -> Result<(), CommandError> {
    require_file(
        &upstream.join(relative),
        Some(bytes),
        Some(sha256),
        relative,
    )
}

fn embedded_helper_sha256() -> String {
    format!("{:x}", Sha256::digest(HELPER_BYTES))
}

fn authenticate_live_helper(path: &Path) -> Result<(), CommandError> {
    require_file(
        path,
        Some(HELPER_BYTES.len() as u64),
        Some(&embedded_helper_sha256()),
        "capture helper",
    )
}

fn capture_into(
    arguments: &CaptureArguments,
    staging: &Path,
) -> Result<CaptureOutcome, CommandError> {
    let plans = model_plans();
    let plan_path = staging.join("capture-plan.json");
    write_synced(
        &plan_path,
        &serde_json::to_vec(&plans)
            .map_err(|e| CommandError::new("COMPATIBILITY_INVALID", e.to_string()))?,
    )?;
    let helper_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/pangolin_compat_capture.py")
        .canonicalize()
        .map_err(|e| CommandError::new("IO", format!("helper: {e}")))?;
    // The helper is a live source-tree path rather than an embedded temporary.
    // Re-authenticate its exact bytes immediately before constructing the
    // Python child so a post-build worktree edit cannot change capture code.
    authenticate_live_helper(&helper_path)?;
    let mut helper = Command::new(&arguments.python);
    helper
        .arg(&helper_path)
        .arg("--plan")
        .arg(&plan_path)
        .arg("--reference")
        .arg(&arguments.reference)
        .arg("--annotation-db")
        .arg(&arguments.annotation_db);
    capture_env(&mut helper, &arguments.upstream);
    let helper_output = bounded_output(&mut helper, CASES_MAX as usize, "upstream helper")?;
    let observations: Vec<HelperObservation> =
        serde_json::from_slice(&helper_output).map_err(|_| {
            CommandError::new("COMPATIBILITY_INVALID", "upstream helper output is invalid")
        })?;
    if observations.len() != 14 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "helper case count mismatch",
        ));
    }
    let expected_module = arguments
        .upstream
        .join("pangolin/pangolin.py")
        .canonicalize()
        .map_err(|e| CommandError::new("IO", e.to_string()))?;
    for observation in &observations {
        if Path::new(&observation.imported_module)
            .canonicalize()
            .ok()
            .as_ref()
            != Some(&expected_module)
        {
            return Err(CommandError::new(
                "COMPATIBILITY_INVALID",
                "helper imported outside pinned checkout",
            ));
        }
    }
    let contexts = extract_contexts(&arguments.reference, &plans)?;
    let (cli_unmasked, warnings_unmasked) = run_cli(arguments, staging, &plans, false)?;
    let (cli_masked, warnings_masked) = run_cli(arguments, staging, &plans, true)?;
    let mut cases: Vec<Case> = Vec::new();
    for (plan, observation) in plans.iter().zip(&observations) {
        if observation.id != plan.id {
            return Err(CommandError::new(
                "COMPATIBILITY_INVALID",
                "helper case order mismatch",
            ));
        }
        let mut strands = Vec::new();
        for raw in &observation.strands {
            let expected_dtype = if plan.allele_shape == "deletion_anchored" {
                "f64"
            } else {
                "f32"
            };
            if raw.dtype != expected_dtype {
                return Err(CommandError::new(
                    "COMPATIBILITY_INVALID",
                    format!("{}: helper score dtype disagrees with shape", plan.id),
                ));
            }
            let loss = parse_typed_bits(&raw.dtype, &raw.loss_bits, &plan.id)?;
            let gain = parse_typed_bits(&raw.dtype, &raw.gain_bits, &plan.id)?;
            let unmasked = replay_typed_scores(
                loss.clone(),
                gain.clone(),
                &raw.genes,
                plan.position,
                plan.distance,
                false,
                &plan.id,
            )?;
            let masked = replay_typed_scores(
                loss,
                gain,
                &raw.genes,
                plan.position,
                plan.distance,
                true,
                &plan.id,
            )?;
            let expected = StrandExpected {
                cli_unmasked: format_cli(&unmasked, &raw.dtype, &plan.id)?,
                cli_masked: format_cli(&masked, &raw.dtype, &plan.id)?,
                unmasked,
                masked,
            };
            strands.push(StrandCase {
                strand: raw.strand.clone(),
                dtype: raw.dtype.clone(),
                loss_bits: raw.loss_bits.clone(),
                gain_bits: raw.gain_bits.clone(),
                genes: raw.genes.clone(),
                expected,
            });
        }
        strands.sort_by_key(|s| if s.strand == "+" { 0 } else { 1 });
        let combined_unmasked = strands
            .iter()
            .map(|s| s.expected.cli_unmasked.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let combined_masked = strands
            .iter()
            .map(|s| s.expected.cli_masked.as_str())
            .collect::<Vec<_>>()
            .join(",");
        compare_cli_field(
            &plan.id,
            "unmasked",
            &combined_unmasked,
            cli_unmasked.get(&plan.id),
        )?;
        compare_cli_field(
            &plan.id,
            "masked",
            &combined_masked,
            cli_masked.get(&plan.id),
        )?;
        let bases = contexts
            .get(&plan.id)
            .ok_or_else(|| CommandError::new("COMPATIBILITY_INVALID", "missing reference context"))?
            .clone();
        cases.push(Case::Model(ModelCase {
            id: plan.id.clone(),
            kind: "model".into(),
            coverage: plan.coverage.clone(),
            input: VariantInput {
                assembly: "GRCh38".into(),
                contig: plan.contig.clone(),
                position: plan.position,
                reference: plan.reference.clone(),
                alt: plan.alt.clone(),
                distance: plan.distance,
                allele_shape: plan.allele_shape.clone(),
            },
            context: Context {
                start_1based: plan.position - 5050,
                anchor_offset: 5050,
                bases: bases.clone(),
                sha256: format!("{:x}", Sha256::digest(bases.as_bytes())),
            },
            strands,
            precomputed: plan.precomputed.clone(),
        }));
    }
    cases.extend(rejection_cases(&warnings_unmasked, &warnings_masked)?);
    cases.extend(postprocess_cases()?);
    if cases.len() != 24 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "captured case count mismatch",
        ));
    }
    let notice = notice_bytes();
    let cases_bytes = serialize_cases(&cases)?;
    write_synced(&staging.join("cases.jsonl"), &cases_bytes)?;
    write_synced(&staging.join("NOTICE"), notice.as_bytes())?;
    let manifest = build_manifest(&cases, &cases_bytes, notice.as_bytes());
    let mut manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| CommandError::new("COMPATIBILITY_INVALID", e.to_string()))?;
    manifest_bytes.push(b'\n');
    write_synced(&staging.join("manifest.json"), &manifest_bytes)?;
    fs::remove_file(&plan_path)
        .map_err(|e| CommandError::new("IO", format!("remove plan: {e}")))?;
    for name in ["variants.csv", "unmasked.csv", "masked.csv"] {
        let path = staging.join(name);
        if path.exists() {
            fs::remove_file(path)
                .map_err(|e| CommandError::new("IO", format!("remove capture temporary: {e}")))?;
        }
    }
    let inspected = inspect_corpus(staging)?;
    if inspected.cases != 24 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "self-inspection failed",
        ));
    }
    Ok(CaptureOutcome {
        status: "captured",
        schema: SCHEMA,
        profile: PROFILE,
        corpus_sha256: format!("{:x}", Sha256::digest(&manifest_bytes)),
        cases: 24,
        bytes: (manifest_bytes.len() + cases_bytes.len() + notice.len()) as u64,
    })
}

fn compare_cli_field(
    case: &str,
    mode: &str,
    helper: &str,
    upstream: Option<&String>,
) -> Result<(), CommandError> {
    let upstream = upstream.map(String::as_str).unwrap_or("<missing>");
    if helper == upstream {
        return Ok(());
    }
    let difference = helper
        .bytes()
        .zip(upstream.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| helper.len().min(upstream.len()));
    Err(CommandError::new(
        "COMPATIBILITY_INVALID",
        format!(
            "{case}: {mode} CLI field mismatch at byte {difference}; helper_len={}; upstream_len={}; helper_prefix={:?}; upstream_prefix={:?}",
            helper.len(),
            upstream.len(),
            helper.chars().take(512).collect::<String>(),
            upstream.chars().take(512).collect::<String>()
        ),
    ))
}

fn model_plans() -> Vec<ModelPlan> {
    let ref100 = "TTTTTTGCACCTAAATTTAGGATTATATTCAAATAGCAAATGCCTTGAAGTGCTCTGATACTGAGCTTCCCAGTTTTTGTTGAGCTAGTGACATATTTGT";
    let rows = [
        (
            "M01-snv-cd4-precomputed",
            "chr12",
            6801301,
            "G",
            "A",
            "snv",
            vec![
                "shape.snv",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
                "lookup.precomputed_observation",
                "effect.zero_or_low",
            ],
        ),
        (
            "M02-snv-wrap53-tp53-precomputed",
            "chr17",
            7686079,
            "A",
            "T",
            "snv",
            vec![
                "shape.snv",
                "strand.plus",
                "strand.minus",
                "overlap.opposite_strand",
                "mask.masked",
                "mask.unmasked",
                "lookup.precomputed_observation",
                "effect.nonzero",
            ],
        ),
        (
            "M03-snv-afap1l2-precomputed",
            "chr10",
            114306065,
            "A",
            "T",
            "snv",
            vec![
                "shape.snv",
                "strand.minus",
                "mask.masked",
                "mask.unmasked",
                "lookup.precomputed_observation",
            ],
        ),
        (
            "M04-snv-grk1-precomputed",
            "chr13",
            113723021,
            "C",
            "G",
            "snv",
            vec![
                "shape.snv",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
                "lookup.precomputed_observation",
            ],
        ),
        (
            "M05-snv-same-strand-overlap",
            "chr3",
            29000000,
            "T",
            "C",
            "snv",
            vec![
                "shape.snv",
                "strand.plus",
                "overlap.same_strand",
                "mask.masked",
                "mask.unmasked",
            ],
        ),
        (
            "M06-snv-gene-start-plus-one",
            "chr12",
            6786859,
            "A",
            "G",
            "snv",
            vec![
                "shape.snv",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
                "boundary.gene_start_plus_one",
            ],
        ),
        (
            "M07-mnv-plus",
            "chr12",
            6801303,
            "GG",
            "AC",
            "mnv_equal",
            vec![
                "shape.mnv_equal",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
            ],
        ),
        (
            "M08-mnv-both-strands",
            "chr17",
            7687421,
            "GCCC",
            "ATTA",
            "mnv_equal",
            vec![
                "shape.mnv_equal",
                "strand.plus",
                "strand.minus",
                "overlap.opposite_strand",
                "mask.masked",
                "mask.unmasked",
            ],
        ),
        (
            "M09-insertion-short-plus",
            "chr12",
            6801303,
            "G",
            "GA",
            "insertion_anchored",
            vec![
                "shape.insertion_anchored",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
                "indel.insertion_short",
            ],
        ),
        (
            "M10-insertion-short-both",
            "chr17",
            7687421,
            "G",
            "GACG",
            "insertion_anchored",
            vec![
                "shape.insertion_anchored",
                "strand.plus",
                "strand.minus",
                "overlap.opposite_strand",
                "mask.masked",
                "mask.unmasked",
                "indel.insertion_short",
            ],
        ),
        (
            "M11-insertion-long-overlap",
            "chr3",
            29000000,
            "T",
            "TACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTAC",
            "insertion_anchored",
            vec![
                "shape.insertion_anchored",
                "strand.plus",
                "overlap.same_strand",
                "mask.masked",
                "mask.unmasked",
                "indel.insertion_long",
            ],
        ),
        (
            "M12-deletion-short-plus",
            "chr12",
            6801303,
            "GG",
            "G",
            "deletion_anchored",
            vec![
                "shape.deletion_anchored",
                "strand.plus",
                "mask.masked",
                "mask.unmasked",
                "indel.deletion_short",
            ],
        ),
        (
            "M13-deletion-short-both",
            "chr17",
            7687421,
            "GCCC",
            "G",
            "deletion_anchored",
            vec![
                "shape.deletion_anchored",
                "strand.plus",
                "strand.minus",
                "overlap.opposite_strand",
                "mask.masked",
                "mask.unmasked",
                "indel.deletion_short",
            ],
        ),
        (
            "M14-deletion-ref100-overlap",
            "chr3",
            29000000,
            ref100,
            "T",
            "deletion_anchored",
            vec![
                "shape.deletion_anchored",
                "strand.plus",
                "overlap.same_strand",
                "mask.masked",
                "mask.unmasked",
                "indel.deletion_ref_100",
            ],
        ),
    ];
    rows.into_iter()
        .map(
            |(id, contig, position, reference, alt, shape, coverage)| ModelPlan {
                id: id.into(),
                contig: contig.into(),
                position,
                reference: reference.into(),
                alt: alt.into(),
                distance: 50,
                allele_shape: shape.into(),
                coverage: coverage.into_iter().map(Into::into).collect(),
                precomputed: precomputed_for(id),
            },
        )
        .collect()
}

fn precomputed_for(id: &str) -> Vec<Precomputed> {
    match id {
        "M01-snv-cd4-precomputed" => vec![pre(
            "ENSG00000010610.tsv.gz",
            "ENSG00000010610.10",
            "00000000",
            -50,
            "80000000",
            -50,
        )],
        "M02-snv-wrap53-tp53-precomputed" => vec![
            pre(
                "ENSG00000141499.tsv.gz",
                "ENSG00000141499.17",
                "3e570a3d",
                18,
                "80000000",
                -50,
            ),
            pre(
                "ENSG00000141510.tsv.gz",
                "ENSG00000141510.18",
                "00000000",
                -50,
                "80000000",
                -50,
            ),
        ],
        "M03-snv-afap1l2-precomputed" => vec![pre(
            "ENSG00000169129.tsv.gz",
            "ENSG00000169129.15",
            "3d75c28f",
            12,
            "00000000",
            -50,
        )],
        "M04-snv-grk1-precomputed" => vec![pre(
            "ENSG00000185974.tsv.gz",
            "ENSG00000185974.7",
            "3cf5c28f",
            0,
            "80000000",
            -50,
        )],
        _ => vec![],
    }
}
fn pre(source: &str, gene: &str, gain: &str, gp: i32, loss: &str, lp: i32) -> Precomputed {
    Precomputed {
        source_member: source.into(),
        gene: gene.into(),
        gain_bits: gain.into(),
        gain_position: gp,
        loss_bits: loss.into(),
        loss_position: lp,
    }
}

fn extract_contexts(
    reference: &Path,
    plans: &[ModelPlan],
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut needed: BTreeMap<String, Vec<&ModelPlan>> = BTreeMap::new();
    for plan in plans {
        needed.entry(plan.contig.clone()).or_default().push(plan);
    }
    let file =
        File::open(reference).map_err(|e| CommandError::new("IO", format!("reference: {e}")))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut line = String::new();
    let mut contig = String::new();
    let mut offset = 0_u32;
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| CommandError::new("IO", format!("reference: {e}")))?;
        if read == 0 {
            break;
        }
        let text = line.trim_end_matches(['\n', '\r']);
        if let Some(name) = text.strip_prefix('>') {
            contig = name
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            offset = 0;
            continue;
        }
        if let Some(cases) = needed.get(&contig) {
            let start_line = offset + 1;
            let end_line = offset + text.len() as u32;
            for plan in cases {
                let start = plan.position - 5050;
                let end = start + 10_100 + plan.reference.len() as u32 - 1;
                let from = start.max(start_line);
                let to = end.min(end_line);
                if from <= to {
                    let local = (from - start_line) as usize..(to - start_line + 1) as usize;
                    out.entry(plan.id.clone())
                        .or_default()
                        .push_str(&text[local]);
                }
            }
        }
        offset = offset.checked_add(text.len() as u32).ok_or_else(|| {
            CommandError::new("COMPATIBILITY_INVALID", "reference coordinate overflow")
        })?;
    }
    for plan in plans {
        if out
            .get(&plan.id)
            .is_none_or(|v| v.len() != 10_100 + plan.reference.len())
        {
            return Err(CommandError::new(
                "COMPATIBILITY_INVALID",
                format!("{}: reference context incomplete", plan.id),
            ));
        }
    }
    Ok(out)
}

type CliObservations = (BTreeMap<String, String>, BTreeMap<String, String>);

fn run_cli(
    arguments: &CaptureArguments,
    staging: &Path,
    plans: &[ModelPlan],
    masked: bool,
) -> Result<CliObservations, CommandError> {
    let input = staging.join("variants.csv");
    if !input.exists() {
        let mut bytes = b"ID,CHROM,POS,REF,ALT\n".to_vec();
        for plan in plans {
            writeln!(
                &mut bytes,
                "{},{},{},{},{}",
                plan.id, plan.contig, plan.position, plan.reference, plan.alt
            )
            .map_err(|e| CommandError::new("IO", e.to_string()))?;
        }
        for rejection in rejection_inputs().into_iter().take(4) {
            writeln!(
                &mut bytes,
                "{},{},{},{},{}",
                rejection.id,
                rejection.contig,
                rejection.position,
                rejection.reference,
                rejection.alt
            )
            .map_err(|e| CommandError::new("IO", e.to_string()))?;
        }
        write_synced(&input, &bytes)?;
    }
    let prefix = staging.join(if masked { "masked" } else { "unmasked" });
    let mut command = Command::new(&arguments.python);
    command
        .arg("-m")
        .arg("pangolin.pangolin")
        .arg("-m")
        .arg(if masked { "True" } else { "False" })
        .arg("-d")
        .arg("50")
        .arg(&input)
        .arg(&arguments.reference)
        .arg(&arguments.annotation_db)
        .arg(&prefix);
    capture_env(&mut command, &arguments.upstream);
    let stdout = bounded_output(&mut command, 1024 * 1024, "unmodified upstream CLI")?;
    let text = String::from_utf8(stdout)
        .map_err(|_| CommandError::new("COMPATIBILITY_INVALID", "CLI stdout is not UTF-8"))?;
    let output = prefix.with_extension("csv");
    let mut csv_file =
        File::open(&output).map_err(|e| CommandError::new("IO", format!("CLI CSV: {e}")))?;
    let csv_length = csv_file
        .metadata()
        .map_err(|e| CommandError::new("IO", format!("CLI CSV: {e}")))?
        .len();
    if csv_length > 1024 * 1024 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "CLI CSV exceeds bound",
        ));
    }
    let mut bytes = Vec::with_capacity(csv_length as usize);
    csv_file
        .read_to_end(&mut bytes)
        .map_err(|e| CommandError::new("IO", format!("CLI CSV: {e}")))?;
    if bytes.len() as u64 != csv_length {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            "CLI CSV changed while reading",
        ));
    }
    let csv = String::from_utf8(bytes)
        .map_err(|_| CommandError::new("COMPATIBILITY_INVALID", "CLI CSV is not UTF-8"))?;
    let mut scores = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let mut parts = line.splitn(6, ',');
        let id = parts.next().unwrap_or_default();
        for _ in 0..4 {
            let _ = parts.next();
        }
        let score = parts.next().unwrap_or_default();
        scores.insert(id.to_owned(), score.to_owned());
    }
    let mut warnings = BTreeMap::new();
    for (index, rejection) in rejection_inputs().iter().take(4).enumerate() {
        let line_number = plans.len() + index + 1;
        let marker = format!("[Line {line_number}]");
        if let Some(line) = text
            .lines()
            .find(|line| line.contains(&marker) && line.contains("WARNING"))
        {
            warnings.insert(rejection.id.to_owned(), line.to_owned());
        } else {
            return Err(CommandError::new(
                "COMPATIBILITY_INVALID",
                format!("{}: CLI rejection warning missing", rejection.id),
            ));
        }
    }
    Ok((scores, warnings))
}

struct RejectionInput {
    id: &'static str,
    contig: &'static str,
    position: u32,
    reference: String,
    alt: &'static str,
}
fn rejection_inputs() -> Vec<RejectionInput> {
    let r100 = "TTTTTTGCACCTAAATTTAGGATTATATTCAAATAGCAAATGCCTTGAAGTGCTCTGATACTGAGCTTCCCAGTTTTTGTTGAGCTAGTGACATATTTGT";
    vec![
        RejectionInput {
            id: "R01-complex-replacement",
            contig: "chr12",
            position: 6801303,
            reference: "GG".into(),
            alt: "AAA",
        },
        RejectionInput {
            id: "R02-deletion-ref101",
            contig: "chr3",
            position: 29000000,
            reference: format!("{r100}T"),
            alt: "T",
        },
        RejectionInput {
            id: "R03-reference-mismatch",
            contig: "chr13",
            position: 113723021,
            reference: "A".into(),
            alt: "G",
        },
        RejectionInput {
            id: "R04-no-containing-gene",
            contig: "chr12",
            position: 6691000,
            reference: "T".into(),
            alt: "C",
        },
        RejectionInput {
            id: "R05-left-context",
            contig: "chrM",
            position: 600,
            reference: "A".into(),
            alt: "G",
        },
        RejectionInput {
            id: "R06-right-context",
            contig: "chrM",
            position: 16000,
            reference: "G".into(),
            alt: "A",
        },
    ]
}

fn rejection_cases(
    unmasked: &BTreeMap<String, String>,
    masked: &BTreeMap<String, String>,
) -> Result<Vec<Case>, CommandError> {
    rejection_inputs()
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let (first, category, witness, coverage) = match index {
                0 => (
                    "variant_shape_guard",
                    "unsupported_variant_shape",
                    RejectionWitness::Shape {
                        ref_len: 2,
                        alt_len: 3,
                    },
                    "reject.complex_unequal",
                ),
                1 => (
                    "deletion_length_guard",
                    "deletion_too_large",
                    RejectionWitness::Deletion {
                        ref_len: 101,
                        alt_len: 1,
                        twice_distance: 100,
                    },
                    "reject.deletion_ref_101",
                ),
                2 => (
                    "reference_anchor_compare",
                    "reference_mismatch",
                    RejectionWitness::Mismatch {
                        true_anchor: "C".into(),
                    },
                    "reject.ref_mismatch",
                ),
                3 => (
                    "get_genes_empty",
                    "not_in_gene",
                    RejectionWitness::NoGene {
                        query_empty: true,
                        previous: AnnotationRow {
                            rowid: 244405,
                            id: "ENSG00000126746.18".into(),
                            start: 6666477,
                            end: 6689572,
                            strand: "-".into(),
                        },
                        following: AnnotationRow {
                            rowid: 244419,
                            id: "ENSG00000139200.14".into(),
                            start: 6693791,
                            end: 6700815,
                            strand: "-".into(),
                        },
                    },
                    "reject.no_gene",
                ),
                4 => (
                    "reference_slice",
                    "insufficient_reference_context",
                    RejectionWitness::Context {
                        side: "left".into(),
                        required: -4450,
                        available: 1,
                    },
                    "reject.left_context",
                ),
                _ => (
                    "reference_slice",
                    "insufficient_reference_context",
                    RejectionWitness::Context {
                        side: "right".into(),
                        required: 21050,
                        available: 16569,
                    },
                    "reject.right_context",
                ),
            };
            let upstream_evidence = if index < 4 {
                let warning = unmasked.get(row.id).ok_or_else(|| {
                    CommandError::new(
                        "COMPATIBILITY_INVALID",
                        format!("{}: unmasked CLI warning is missing", row.id),
                    )
                })?;
                if masked.get(row.id) != Some(warning) {
                    return Err(CommandError::new(
                        "COMPATIBILITY_INVALID",
                        format!("{}: masked and unmasked CLI warnings differ", row.id),
                    ));
                }
                UpstreamEvidence::Cli {
                    warning: warning.clone(),
                }
            } else {
                UpstreamEvidence::RuleReplay {
                    reason: "excluded_from_cli_native_reference_slice_crash".into(),
                }
            };
            Ok(Case::Rejection(RejectionCase {
                id: row.id.into(),
                kind: "rejection".into(),
                coverage: vec![coverage.into()],
                input: VariantInput {
                    assembly: "GRCh38".into(),
                    contig: row.contig.into(),
                    position: row.position,
                    reference: row.reference,
                    alt: row.alt.into(),
                    distance: 50,
                    allele_shape: if index == 0 {
                        "complex_unequal"
                    } else if index == 1 {
                        "deletion_anchored"
                    } else {
                        "snv"
                    }
                    .into(),
                },
                first_operation: first.into(),
                normalized_category: category.into(),
                witness,
                upstream_evidence,
            }))
        })
        .collect()
}

fn postprocess_cases() -> Result<Vec<Case>, CommandError> {
    let mut result = ["order-v1", "empty-v1", "tie-v1"]
        .into_iter()
        .map(|scenario| {
            fixed_vector_case(scenario)
                .map(PostprocessCase::Vector)
                .map(Case::Postprocess)
                .ok_or_else(|| {
                    CommandError::new("COMPATIBILITY_INVALID", "missing fixed vector case")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    result.push(Case::Postprocess(PostprocessCase::Round(
        RoundPostprocessCase {
            id: "P04-rounding-signed-zero".into(),
            kind: "postprocess".into(),
            coverage: vec!["postprocess.rounding_signed_zero".into()],
            position: 100,
            distance: 2,
            scenario: "round-v1".into(),
            scalars: rounding_scalars(),
        },
    )));
    Ok(result)
}

fn serialize_cases(cases: &[Case]) -> Result<Vec<u8>, CommandError> {
    let mut out = Vec::new();
    for case in cases {
        match case {
            Case::Model(v) => serde_json::to_writer(&mut out, v),
            Case::Rejection(v) => serde_json::to_writer(&mut out, v),
            Case::Postprocess(v) => serde_json::to_writer(&mut out, v),
        }
        .map_err(|e| CommandError::new("COMPATIBILITY_INVALID", e.to_string()))?;
        out.push(b'\n');
    }
    Ok(out)
}

fn build_manifest(cases: &[Case], case_bytes: &[u8], notice: &[u8]) -> Manifest {
    Manifest{schema:SCHEMA.into(),profile:PROFILE.into(),upstream:Upstream{url:"https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7".into(),commit:"5cf94b8db938c658391b4305cd7ce33297d44ff7".into(),declared_version:"1.0.2".into(),license:"GPL-3.0-only".into(),helper_sha256:embedded_helper_sha256()},checkpoints:CHECKPOINTS.into_iter().enumerate().map(|(i,(name,sha))|Checkpoint{ordinal:(i+1) as u8,filename:name.into(),bytes:2_877_321,sha256:sha.into()}).collect(),reference:Reference{source_url:"https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz".into(),source_bytes:972_898_531,source_sha256:"11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3".into(),assembly_report_url:"https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt".into(),assembly_report_bytes:80_454,assembly_report_sha256:"64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38".into(),transform:"select NC_000003.12, NC_000010.11, NC_000012.12, NC_000013.11, NC_000017.11, NC_012920.1; rename chr3/chr10/chr12/chr13/chr17/chrM; uppercase; preserve 80-base wrapping".into(),derived_bytes:671_294_255,derived_sha256:"81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb".into(),contigs:["chr3","chr10","chr12","chr13","chr17","chrM"].into_iter().map(Into::into).collect()},annotation:Annotation{database_url:"https://www.dropbox.com/sh/6zo0aegoalvgd9f/AADOhGYJo8tbUhpscp3wSFj6a/gencode.v38.annotation.db?dl=1".into(),database_bytes:380_366_848,database_sha256:"221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6".into(),gtf_url:"https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/gencode.v38.annotation.gtf.gz".into(),gtf_bytes:46_556_621,gtf_md5:"16fcae8ca8e488cd8056cf317d963407".into(),gtf_sha256:"22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050".into(),filter:"Ensembl_canonical".into(),logical_sha256:logical_annotation_sha(cases)},environment:Environment{python:"3.13.5".into(),pytorch:"2.7.1+cpu".into(),numpy:"2.5.1".into(),pandas:"3.0.3".into(),pyfastx:"2.3.1".into(),gffutils:"0.14".into(),pyvcf3:"1.0.4".into(),platform:"linux-x86_64".into(),cuda:false,helper_torch_intraop_threads:1,helper_torch_interop_threads:1,cli_omp_threads:1,cli_torch_interop_threads_observed:16},coverage:COVERAGE.into_iter().map(Into::into).collect(),case_ids:CASE_IDS.into_iter().map(Into::into).collect(),members:vec![Member{filename:"cases.jsonl".into(),bytes:case_bytes.len() as u64,sha256:format!("{:x}",Sha256::digest(case_bytes))},Member{filename:"NOTICE".into(),bytes:notice.len() as u64,sha256:format!("{:x}",Sha256::digest(notice))}]}
}

fn notice_bytes() -> String {
    r#"Pangopup compatibility corpus notice

Pangolin 1.0.2 source and model checkpoints
Copyright (C) Tony Zeng and Pangolin contributors
Source: https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7
License: GNU General Public License v3.0 only (GPL-3.0-only)
Citation: Zeng T, Li YI. Predicting RNA splicing from DNA sequence using Pangolin. Genome Biology (2022).

NCBI RefSeq GRCh38.p14 sequence
Source: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz
Assembly report: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt
Policy and acknowledgment/disclaimer: https://www.ncbi.nlm.nih.gov/home/about/policies/

GENCODE release 38 annotation
Upstream database: https://www.dropbox.com/sh/6zo0aegoalvgd9f/AADOhGYJo8tbUhpscp3wSFj6a/gencode.v38.annotation.db?dl=1
Source: https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/gencode.v38.annotation.gtf.gz
Checksums: https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/MD5SUMS
Release: https://www.gencodegenes.org/human/release_38.html
Data access: https://www.gencodegenes.org/pages/data_access.html
Citation guidance: https://www.gencodegenes.org/pages/citing_gencode.html

Pangopup transformed reference sequence names, uppercased and retained bounded
sequence contexts, and transformed selected GENCODE gene/exon facts into this
small compatibility corpus. The corpus retains observed model arrays and does
not contain the model checkpoints, whole reference, GTF, or SQLite database.
"#.into()
}

fn capture_env(command: &mut Command, upstream: &Path) {
    command
        .env("PYTHONPATH", upstream)
        .env("CUDA_VISIBLE_DEVICES", "")
        .env("OMP_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("NUMEXPR_NUM_THREADS", "1")
        .stdin(Stdio::null());
}
fn bounded_output(command: &mut Command, max: usize, label: &str) -> Result<Vec<u8>, CommandError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| CommandError::new("IO", format!("{label}: {e}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CommandError::new("IO", format!("{label}: stdout unavailable")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CommandError::new("IO", format!("{label}: stderr unavailable")))?;
    let stdout_reader = std::thread::spawn(move || drain_bounded(stdout, max));
    let stderr_reader = std::thread::spawn(move || drain_bounded(stderr, max));
    let status = child
        .wait()
        .map_err(|e| CommandError::new("IO", format!("{label}: {e}")))?;
    let (stdout, stdout_exceeded) = stdout_reader
        .join()
        .map_err(|_| CommandError::new("IO", format!("{label}: stdout reader failed")))?
        .map_err(|e| CommandError::new("IO", format!("{label}: {e}")))?;
    let (_, stderr_exceeded) = stderr_reader
        .join()
        .map_err(|_| CommandError::new("IO", format!("{label}: stderr reader failed")))?
        .map_err(|e| CommandError::new("IO", format!("{label}: {e}")))?;
    if stdout_exceeded || stderr_exceeded {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            format!("{label} output exceeds bound"),
        ));
    }
    if !status.success() {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            format!("{label} failed"),
        ));
    }
    Ok(stdout)
}

fn drain_bounded(mut reader: impl Read, max: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::with_capacity(max.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    let mut exceeded = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let available = max.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(available)]);
        exceeded |= read > available;
    }
    Ok((retained, exceeded))
}
fn command_text(command: &mut Command, label: &str) -> Result<String, CommandError> {
    String::from_utf8(bounded_output(command, 64 * 1024, label)?)
        .map_err(|_| CommandError::new("COMPATIBILITY_INVALID", format!("{label} is not UTF-8")))
}
fn require_file(
    path: &Path,
    size: Option<u64>,
    sha: Option<&str>,
    label: &str,
) -> Result<(), CommandError> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| CommandError::new("IO", format!("{label}: {e}")))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() || meta.nlink() != 1 {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            format!("{label} must be a regular single-link file"),
        ));
    }
    if size.is_some_and(|expected| meta.len() != expected) {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            format!("{label} size mismatch"),
        ));
    }
    if let Some(expected) = sha
        && hash_file(path)? != expected
    {
        return Err(CommandError::new(
            "COMPATIBILITY_INVALID",
            format!("{label} digest mismatch"),
        ));
    }
    Ok(())
}
fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file = File::open(path).map_err(|e| CommandError::new("IO", e.to_string()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| CommandError::new("IO", e.to_string()))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path).map_err(|e| {
        CommandError::new(
            "IO",
            format!(
                "write {}: {e}",
                path.file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("member")
            ),
        )
    })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| CommandError::new("IO", e.to_string()))
}
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| CommandError::new("IO", format!("sync directory: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let serial = CAPTURE_SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-compatibility-unit-{}-{serial}-{name}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create unit scratch");
        path
    }

    #[test]
    fn live_helper_and_imported_package_initializer_fail_closed_on_mutation() {
        let root = scratch("source-auth");
        let helper = root.join("helper.py");
        fs::write(&helper, HELPER_BYTES).expect("write exact helper");
        authenticate_live_helper(&helper).expect("exact live helper");
        fs::write(&helper, [HELPER_BYTES, b"# mutation\n"].concat()).expect("mutate helper");
        assert_eq!(
            authenticate_live_helper(&helper)
                .expect_err("mutated helper")
                .code,
            "COMPATIBILITY_INVALID"
        );

        let initializer = UPSTREAM_SOURCES
            .iter()
            .copied()
            .find(|(path, _, _)| *path == "pangolin/__init__.py")
            .expect("initializer is an authenticated imported module");
        let initializer_path = root.join(initializer.0);
        fs::create_dir_all(initializer_path.parent().expect("initializer parent"))
            .expect("create package directory");
        fs::write(&initializer_path, b"").expect("write exact initializer");
        authenticate_upstream_source(&root, initializer).expect("exact initializer");
        fs::write(&initializer_path, b"# mutation\n").expect("mutate initializer");
        assert_eq!(
            authenticate_upstream_source(&root, initializer)
                .expect_err("mutated initializer")
                .code,
            "COMPATIBILITY_INVALID"
        );
        fs::remove_dir_all(root).expect("remove source-auth scratch");
    }

    #[test]
    fn publication_cleans_only_unpublished_staging_and_preserves_published_output() {
        let root = scratch("publication");

        let staging_sync = root.join("staging-sync");
        let output_sync = root.join("output-sync");
        fs::create_dir(&staging_sync).expect("create sync staging");
        let error = publish_staging_with(
            &staging_sync,
            &root,
            &output_sync,
            |_, point| {
                if point == SyncPoint::Staging {
                    Err(CommandError::new("IO", "injected staging sync failure"))
                } else {
                    Ok(())
                }
            },
            |_, _| panic!("rename must not run after staging sync failure"),
        )
        .expect_err("staging sync failure");
        assert_eq!(error.code, "IO");
        assert!(!staging_sync.exists());
        assert!(!output_sync.exists());

        let staging_rename = root.join("staging-rename");
        let output_rename = root.join("output-rename");
        fs::create_dir(&staging_rename).expect("create rename staging");
        publish_staging_with(
            &staging_rename,
            &root,
            &output_rename,
            |_, _| Ok(()),
            |_, _| Err(CommandError::new("IO", "injected rename failure")),
        )
        .expect_err("rename failure");
        assert!(!staging_rename.exists());
        assert!(!output_rename.exists());

        let staging_parent = root.join("staging-parent");
        let output_parent = root.join("output-parent");
        fs::create_dir(&staging_parent).expect("create parent staging");
        fs::write(staging_parent.join("complete"), b"complete").expect("write staged member");
        let error = publish_staging_with(
            &staging_parent,
            &root,
            &output_parent,
            |_, point| {
                if point == SyncPoint::ParentAfterPublish {
                    Err(CommandError::new("IO", "injected parent sync failure"))
                } else {
                    Ok(())
                }
            },
            |source, destination| {
                fs::rename(source, destination).map_err(|e| {
                    CommandError::new("IO", format!("test publish rename failed: {e}"))
                })
            },
        )
        .expect_err("post-publication parent sync failure");
        assert_eq!(error.code, "IO");
        assert!(!staging_parent.exists());
        assert_eq!(
            fs::read(output_parent.join("complete")).expect("published output retained"),
            b"complete"
        );

        fs::remove_dir_all(root).expect("remove publication scratch");
    }

    #[test]
    fn controlled_postprocessing_replays_exactly() {
        let cases = postprocess_cases().expect("controlled cases");
        assert_eq!(cases.len(), 4);
        for case in cases {
            let Case::Postprocess(case) = case else {
                panic!("postprocess case");
            };
            validate_postprocess(&case).expect("independent replay");
        }
    }

    #[test]
    fn boundary_rejections_are_rule_only_and_never_cli_inputs() {
        let inputs = rejection_inputs();
        let cli_ids: Vec<_> = inputs.iter().take(4).map(|value| value.id).collect();
        assert_eq!(
            cli_ids,
            [
                "R01-complex-replacement",
                "R02-deletion-ref101",
                "R03-reference-mismatch",
                "R04-no-containing-gene"
            ]
        );
        assert!(!cli_ids.contains(&"R05-left-context"));
        assert!(!cli_ids.contains(&"R06-right-context"));

        let warnings = BTreeMap::from([
            ("R01-complex-replacement".to_owned(), "warning 1".to_owned()),
            ("R02-deletion-ref101".to_owned(), "warning 2".to_owned()),
            ("R03-reference-mismatch".to_owned(), "warning 3".to_owned()),
            ("R04-no-containing-gene".to_owned(), "warning 4".to_owned()),
        ]);
        let cases = rejection_cases(&warnings, &warnings).expect("rejection cases");
        for case in cases {
            let Case::Rejection(case) = case else {
                panic!("rejection case");
            };
            validate_rejection(&case).expect("rejection replay");
        }
    }

    #[test]
    fn pinned_rounding_keeps_signed_zero() {
        let values = [
            "00000000", "80000000", "3ba3d70a", "bba3d70a", "3f80a3d7", "bf80a3d7", "3e570a3d",
            "bc23d70a",
        ];
        let formatted: Vec<_> = values
            .iter()
            .map(|bits| parse_bit(bits, "round").expect("bits"))
            .map(render_f32)
            .collect();
        assert_eq!(
            formatted,
            [
                "0.0",
                "-0.0",
                "0.0",
                "-0.0",
                "1.0",
                "-1.0",
                "0.20999999344348907",
                "-0.009999999776482582"
            ]
        );
    }

    #[test]
    fn deletion_scores_replay_as_f64_without_narrowing() {
        let scores = parse_typed_bits("f64", &["bfa999999c000000".to_owned()], "deletion-f64")
            .expect("f64 bits");
        let TypedScores::F64(values) = scores else {
            panic!("f64 score vector");
        };
        assert_eq!(values[0].to_bits(), 0xbfa9_9999_9c00_0000);
        assert_eq!(render_f64(values[0]), "-0.05");
        assert_ne!(
            format!("{:08x}", (values[0] as f32).to_bits()),
            "bfa999999c000000"
        );
    }

    #[test]
    fn typed_rounding_control_is_closed_and_dtype_aware() {
        let controls = rounding_scalars();
        assert_eq!(controls.len(), 12);
        for control in &controls {
            assert_eq!(
                render_bits(&control.dtype, &control.bits, "round").expect("render"),
                control.rendered
            );
        }
        assert_eq!(controls[6].rendered, "0.20999999344348907");
        assert_eq!(controls[10].rendered, "0.21");
        assert_eq!(controls[11].rendered, "-0.05");
    }

    #[test]
    fn masking_uses_numpy_second_operand_for_signed_zero_ties() {
        let loss = [0.1, f32::from_bits(0x8000_0000)];
        let gain = [f32::from_bits(0x8000_0000), -1.0];
        let genes = [Gene {
            id: "GENE".to_owned(),
            boundaries: vec![100],
        }];
        let replay =
            score_genes(&loss, &gain, &genes, 100, 0, true, "signed-zero").expect("masked replay");
        assert_eq!(replay[0].gain_bits, "00000000");
        assert_eq!(replay[0].loss_bits, "00000000");
    }
}

#[derive(Clone, Debug)]
pub struct CaptureArguments {
    pub upstream: PathBuf,
    pub python: PathBuf,
    pub reference_source: PathBuf,
    pub assembly_report: PathBuf,
    pub reference: PathBuf,
    pub annotation_db: PathBuf,
    pub annotation_gtf: PathBuf,
    pub output: PathBuf,
}
