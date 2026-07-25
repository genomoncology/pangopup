//! Private Ticket 012 GENCODE observation, candidate, and benchmark support.
//!
//! Normal gates exercise only synthetic inputs. The exact production database
//! and GTF are explicit maintainer inputs to the feature-gated binary.

use flate2::bufread::GzDecoder;
use pangopup_core::{GencodeGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask_candidates::{
    CanonicalMaskGene, MaskCandidateCodec, MaskCandidateReader, MaskQueryBuffer, MaskStrand,
    write_mask_candidate_with_cancellation,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    ops::Range,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[cfg(test)]
use std::cell::Cell;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub const MASK_PROFILE: &str = "pangolin-1.0.2-5cf94b8-grch38-v1";
pub const OBSERVATION_SCHEMA: &str = "pangopup-mask-observation-v3";
pub const CANONICAL_SCHEMA: &str = "pangopup-mask-canonical-v1";
pub const INVENTORY_SCHEMA: &str = "pangopup-mask-inventory-v1";
pub const PERFORMANCE_SCHEMA: &str = "pangopup-mask-performance-manifest-v1";
pub const PHASE_RECEIPT_SCHEMA: &str = "pangopup-mask-phase-receipt-v1";
pub const FAILURE_SCHEMA: &str = "pangopup-mask-failure-v1";
pub const PREFLIGHT_FAILURE_SCHEMA: &str = "pangopup-mask-preflight-failure-v1";
pub const REPORT_SCHEMA: &str = "pangopup-mask-benchmark-report-v1";
pub const BUILDER_SOURCE_SHA256: &str = env!("PANGOPUP_MASK_BUILDER_SOURCE_SHA256");
pub const PINNED_MASK_ZSTANDARD: &str =
    "zstd-0.13.3/libzstd-1.5.7;level=9;checksum;content-size;no-dict-id;no-long-distance;workers=0";

pub const DATABASE_BYTES: u64 = 380_366_848;
pub const DATABASE_SHA256: &str =
    "221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6";
pub const GTF_BYTES: u64 = 46_556_621;
pub const GTF_SHA256: &str = "22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050";
pub const SCHEMA_SHA256: &str = "99a2bb9a60b4f425dcbf0a497355ea9a204a6d38b9abf69e714db3ef252f7a49";
pub const SQL_ROW_CONTROL_SHA256: &str =
    "2b51bd95640bf4a70aa7a8d44110b390b458a76aefb83458747b051cfd3eba3c";

const MAX_DATABASE_BYTES: u64 = DATABASE_BYTES;
const MAX_GTF_BYTES: u64 = GTF_BYTES;
const MAX_PYTHON_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PYTHON_LAUNCHER_BYTES: u64 = 4 * 1024;
const MAX_PYVENV_CONFIG_BYTES: u64 = 16 * 1024;
const MAX_OBSERVATION_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_ENVIRONMENT_MODULES: usize = 512;
const MAX_MODULE_IDENTITY_BYTES: usize = 512;
const MAX_ENVIRONMENT_BYTES: usize =
    MAX_ENVIRONMENT_MODULES * MAX_MODULE_IDENTITY_BYTES + MAX_METADATA_BYTES;
const MAX_CAPTURE_CONTRACT_BYTES: usize = MAX_ENVIRONMENT_BYTES + MAX_METADATA_BYTES;
const MAX_PERFORMANCE_BYTES: usize = 1024 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;
const MAX_GENES: usize = 100_000;
const MAX_BOUNDARIES: usize = 10_000_000;
const MAX_BOUNDARIES_PER_GENE: usize = 100_000;
const MAX_GTF_ATTRIBUTES: usize = 256;
const MAX_GTF_ATTRIBUTE_KEY_BYTES: usize = 64;
const MAX_GTF_ATTRIBUTE_VALUE_BYTES: usize = 4 * 1024;
const MAX_CONTIGS: usize = 25;
const CANDIDATE_MEMBER_MAX: u64 = 512 * 1024 * 1024;
const STAGE_PREFIX: &str = ".pangopup-mask-stage-";
const PREFLIGHT_FAILURE_PREFIX: &str = ".pangopup-mask-preflight-failure-";
const CAPTURE_RECEIPT: &str = "capture-receipt.json";
const PREPARE_RECEIPT: &str = "prepare-receipt.json";
const BENCHMARK_RECEIPT: &str = "benchmark-receipt.json";
const FAILURE_RECEIPT: &str = "failure.json";
const REUSE_AUTHORIZATION_MEMBER: &str = "reuse-authorization.json";
const SNAPSHOT_DATABASE: &str = "source/gencode.v38.annotation.db";
const SNAPSHOT_GTF: &str = "source/gencode.v38.annotation.gtf.gz";
const SNAPSHOT_PYVENV_CONFIG: &str = "source/pyvenv.cfg";
const OBSERVATION_MEMBER: &str = "capture/observation.jsonl";
const ENVIRONMENT_MEMBER: &str = "capture/environment.json";
const CANONICAL_MEMBER: &str = "prepare/canonical.jsonl";
const INVENTORY_MEMBER: &str = "prepare/inventory.json";
const PERFORMANCE_MEMBER: &str = "prepare/performance.json";
const CANDIDATE_DIRECTORY: &str = "prepare/candidates";
const CAPTURE_CONTRACT_SCHEMA: &str = "pangopup-mask-capture-contract-v3";
const CAPTURE_PROMOTION_AUTHORIZATION_SCHEMA: &str =
    "pangopup-mask-capture-promotion-authorization-v1";
const HELPER_EXCEPTION_PREFIX: &[u8] = b"PANGOPUP_HELPER_EXCEPTION:";
static CANCELLATION_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static TEST_CANCEL_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Signal-safe cancellation hook used only by the private qualification CLI.
pub fn request_cancellation() {
    CANCELLATION_REQUESTED.store(true, Ordering::SeqCst);
}

fn check_cancellation() -> Result<(), MaskBuildError> {
    #[cfg(test)]
    let test_cancelled = TEST_CANCEL_AFTER.with(|remaining| match remaining.get() {
        Some(0) => true,
        Some(value) => {
            remaining.set(Some(value - 1));
            false
        }
        None => false,
    });
    #[cfg(not(test))]
    let test_cancelled = false;
    if test_cancelled || CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
        Err(MaskBuildError::new(
            "CANCELLED",
            "qualification was cancelled",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn test_cancel_after(checks: usize) {
    TEST_CANCEL_AFTER.with(|remaining| remaining.set(Some(checks)));
}

#[cfg(test)]
fn clear_test_cancellation() {
    TEST_CANCEL_AFTER.with(|remaining| remaining.set(None));
}

/// Exact Python helper whose output freezes gffutils point-query behavior.
pub const OBSERVATION_HELPER: &str = r#"
import hashlib, json, os, sqlite3, stat, sys, warnings

PRIMARY = [f"chr{i}" for i in range(1, 23)] + ["chrX", "chrY", "chrM"]
MAX_GENES = 100000
MAX_BOUNDARIES = 10000000
MAX_BOUNDARIES_PER_GENE = 100000
MAX_PYTHON_BYTES = 128 * 1024 * 1024
MAX_ENVIRONMENT_MODULES = 512
MAX_MODULE_IDENTITY_BYTES = 512
MAX_ENVIRONMENT_NON_MODULE_BYTES = 64 * 1024
MAX_ENVIRONMENT_BYTES = (MAX_ENVIRONMENT_MODULES * MAX_MODULE_IDENTITY_BYTES +
                         MAX_ENVIRONMENT_NON_MODULE_BYTES)
HELPER_EXCEPTION_PREFIX = "PANGOPUP_HELPER_EXCEPTION:"

def sanitized_exception(exception_type, _exception, _traceback):
    name = getattr(exception_type, "__name__", "Unknown")
    if (not name or len(name) > 64 or not name.isascii() or
        not (name[0].isalpha() or name[0] == "_") or
        not all(character.isalnum() or character == "_" for character in name)):
        name = "Unknown"
    print(HELPER_EXCEPTION_PREFIX + name, file=sys.stderr, flush=True)

sys.excepthook = sanitized_exception

def canonical_sql_rows(cursor, column_types):
    rows = []
    for source in cursor:
        if type(source) is not sqlite3.Row:
            raise RuntimeError("SQLite query did not return sqlite3.Row")
        row = list(source)
        if len(row) != len(column_types):
            raise RuntimeError("SQLite query returned an unexpected column count")
        for value, allowed in zip(row, column_types, strict=True):
            if type(value) not in allowed:
                raise RuntimeError("SQLite query returned a non-canonical value")
        rows.append(row)
    return rows

def legacy_schema_digest_bytes(rows):
    records = []
    for row in rows:
        # Preserve the historical investigation digest exactly: four fixed
        # columns, NULL as an empty field, pipes between fields, LF between
        # rows, and no final LF. Database bytes remain the primary identity;
        # this secondary digest deliberately permits SQL-internal newlines.
        fields = []
        for value in row:
            if value is None:
                fields.append("")
            elif type(value) is str and value and "|" not in value:
                fields.append(value)
            else:
                raise RuntimeError("SQLite schema cannot use the legacy digest encoding")
        records.append("|".join(fields))
    return "\n".join(records).encode("utf-8")

def sqlite_row_control():
    connection = sqlite3.connect(":memory:")
    try:
        connection.row_factory = sqlite3.Row
        source = connection.execute(
            "SELECT 7 AS duplicate, 8 AS duplicate, NULL AS optional").fetchone()
        if (type(source) is not sqlite3.Row or
            list(source.keys()) != ["duplicate", "duplicate", "optional"]):
            raise RuntimeError("SQLite row-factory control drifted")
        values = canonical_sql_rows(
            (source,),
            ((int,), (int,), (type(None),)))
        schema = canonical_sql_rows(
            connection.execute(
                "SELECT 'table' AS type, 'features' AS name, "
                "'features' AS tbl_name, NULL AS sql"),
            ((str,), (str,), (str,), (str, type(None))))
    finally:
        connection.close()
    values_bytes = json.dumps(
        values, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    schema_bytes = legacy_schema_digest_bytes(schema)
    expected = b'[[7,8,null]]\ntable|features|features|'
    observed = values_bytes + b"\n" + schema_bytes
    if observed != expected:
        raise RuntimeError("SQLite row normalization control drifted")
    return hashlib.sha256(observed).hexdigest()

def digest_file(path):
    h = hashlib.sha256()
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink < 1:
            raise RuntimeError("module source is not a regular file")
        if before.st_size < 0 or before.st_size > MAX_PYTHON_BYTES:
            raise RuntimeError("module source exceeds its byte bound")
        with os.fdopen(os.dup(descriptor), "rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                h.update(block)
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_nlink, before.st_size,
            before.st_mtime_ns, before.st_ctime_ns) != \
           (after.st_dev, after.st_ino, after.st_nlink, after.st_size,
            after.st_mtime_ns, after.st_ctime_ns):
            raise RuntimeError("module source changed during authentication")
    finally:
        os.close(descriptor)
    return {"bytes": int(after.st_size), "sha256": h.hexdigest(),
            "device": int(after.st_dev), "inode": int(after.st_ino),
            "links": int(after.st_nlink), "modified_ns": int(after.st_mtime_ns),
            "changed_ns": int(after.st_ctime_ns)}

def module_identity(name, module):
    if not name or len(name.encode("utf-8")) > MAX_MODULE_IDENTITY_BYTES:
        raise RuntimeError("module identity exceeds its byte bound")
    origin = getattr(getattr(module, "__spec__", None), "origin", None)
    if origin in ("built-in", "frozen"):
        marker = (name + "\0" + origin).encode()
        identity = {"name": name, "kind": "interpreter", "path": origin,
                    "bytes": len(marker), "sha256": hashlib.sha256(marker).hexdigest(),
                    "device": 0, "inode": 0, "links": 0, "modified_ns": 0,
                    "changed_ns": 0}
    else:
        path = getattr(module, "__file__", None)
        if path is None:
            raise RuntimeError("imported module has no authenticated source")
        observed = digest_file(path)
        identity = {"name": name, "kind": "file", "path": os.path.realpath(path),
                    **observed}
    if len(canonical_json_bytes(identity)) > MAX_MODULE_IDENTITY_BYTES:
        raise RuntimeError("module identity exceeds its byte bound")
    return identity

def canonical_json_bytes(value):
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")

def emit(value):
    print(canonical_json_bytes(value).decode("utf-8"), flush=True)

database = sys.argv[1]
gtf_path = sys.argv[2]
mode = sys.argv[3]
logical_launcher = sys.argv[4]
logical_prefix = sys.argv[5]
logical_base_prefix = sys.argv[6]
held_prefix = sys.argv[7]
logical_base_executable = sys.argv[8]
if (os.path.realpath(held_prefix) != os.path.realpath(logical_prefix) or
    not os.path.samefile(sys.executable, logical_launcher) or
    not os.path.samefile(sys._base_executable, logical_base_executable) or
    os.path.realpath(sys.base_prefix) != os.path.realpath(logical_base_prefix)):
    raise RuntimeError("authenticated Python prefix selection failed")
site_packages = os.path.join(
    held_prefix, "lib", f"python{sys.version_info.major}.{sys.version_info.minor}",
    "site-packages")
if not os.path.isdir(site_packages):
    raise RuntimeError("authenticated Python site-packages directory is unavailable")
sys.path.insert(0, site_packages)
import _sqlite3
sql_row_control_sha256 = sqlite_row_control()
import gffutils
with warnings.catch_warnings():
    warnings.simplefilter("ignore", DeprecationWarning)
    sqlite3_module_version = sqlite3.version
db = gffutils.FeatureDB(database, keep_order=True)
schema_rows = canonical_sql_rows(
    db.conn.execute(
        "SELECT type,name,tbl_name,sql FROM sqlite_master "
        "ORDER BY type,name,tbl_name,sql"),
    ((str,), (str,), (str,), (str, type(None))))
schema_bytes = legacy_schema_digest_bytes(schema_rows)

first = next(db.features_of_type("gene"), None)
if first is None:
    raise RuntimeError("empty annotation database")
# Exercise both query paths before authenticating every imported helper module.
list(db.children(first, featuretype="exon"))

trace = []
db.conn.set_trace_callback(trace.append)
list(db.region((first.seqid, first.start, first.start), featuretype="gene"))
db.conn.set_trace_callback(None)
selects = [statement for statement in trace if statement.lstrip().upper().startswith("SELECT")]
if not selects:
    raise RuntimeError("region query was not observed")
region_sql = selects[-1]
plan = canonical_sql_rows(
    db.conn.execute("EXPLAIN QUERY PLAN " + region_sql),
    ((int,), (int,), (int,), (str,)))

def environment_payload(kind):
    compile_options = [
        row[0] for row in canonical_sql_rows(
            db.conn.execute("PRAGMA compile_options"), ((str,),))]
    imported = []
    for name, module in sorted(sys.modules.items()):
        if module is not None and (
            getattr(module, "__file__", None) is not None or
            getattr(getattr(module, "__spec__", None), "origin", None)
                in ("built-in", "frozen")):
            if len(imported) >= MAX_ENVIRONMENT_MODULES:
                raise RuntimeError("environment module count exceeds its bound")
            imported.append(module_identity(name, module))
    payload = {
        "kind": kind, "schema": "pangopup-mask-observation-v3",
        "python": sys.version.split()[0], "gffutils": getattr(gffutils, "__version__", ""),
        "executable": logical_launcher, "prefix": logical_prefix,
        "base_prefix": logical_base_prefix,
        "base_executable": logical_base_executable,
        "sqlite3_module": sqlite3_module_version,
        "sqlite_library": sqlite3.sqlite_version,
        "sql_row_control_sha256": sql_row_control_sha256,
        "sqlite_compile_options": sorted(compile_options),
        "schema_sha256": hashlib.sha256(schema_bytes).hexdigest(),
        "query_shape": "gtf.region((contig,pos-1,pos-1),featuretype=gene)",
        "region_sql": region_sql, "query_plan": plan,
        "modules": sorted(imported, key=lambda value: value["name"]),
    }
    non_module = dict(payload)
    del non_module["modules"]
    if len(canonical_json_bytes(non_module)) + 1 > MAX_ENVIRONMENT_NON_MODULE_BYTES:
        raise RuntimeError("environment envelope exceeds its byte bound")
    if len(canonical_json_bytes(payload)) + 1 > MAX_ENVIRONMENT_BYTES:
        raise RuntimeError("environment payload exceeds its byte bound")
    return payload

emit(environment_payload("environment"))
if mode == "environment":
    raise SystemExit(0)
if mode != "full":
    raise RuntimeError("invalid observation mode")

genes = []
facts = {}
total_boundaries = 0
for gene in db.features_of_type("gene"):
    if gene.seqid not in PRIMARY:
        raise RuntimeError("unsupported primary-contig policy")
    gene_id = gene["gene_id"][0]
    boundaries = []
    for exon in db.children(gene, featuretype="exon"):
        boundaries.extend([int(exon.start), int(exon.end)])
    boundaries = sorted(set(boundaries))
    if len(genes) >= MAX_GENES or len(boundaries) > MAX_BOUNDARIES_PER_GENE:
        raise RuntimeError("annotation resource bound exceeded")
    total_boundaries += len(boundaries)
    if total_boundaries > MAX_BOUNDARIES or len(gene_id.encode()) > 64:
        raise RuntimeError("annotation resource bound exceeded")
    fact = {"id": gene_id, "contig": gene.seqid, "strand": gene.strand,
            "start": int(gene.start), "end": int(gene.end), "boundaries": boundaries}
    if gene_id in facts:
        raise RuntimeError("duplicate exact gene identity")
    facts[gene_id] = fact
    genes.append(fact)
for fact in genes:
    emit({"kind": "gene", **fact})

by_contig = {contig: [] for contig in PRIMARY}
for fact in genes:
    by_contig[fact["contig"]].append(fact)
domain_count = 0
for contig in PRIMARY:
    events = set()
    for fact in by_contig[contig]:
        if fact["start"] < fact["end"]:
            events.add(fact["start"] + 1)
            if fact["end"] < 4294967295:
                events.add(fact["end"] + 1)
    events = sorted(events)
    for index, begin in enumerate(events):
        end = events[index + 1] - 1 if index + 1 < len(events) else 4294967295
        plus, minus = [], []
        for gene in db.region((contig, begin - 1, begin - 1), featuretype="gene"):
            if gene.start > begin or gene.end < begin:
                continue
            gene_id = gene["gene_id"][0]
            target = plus if gene.strand == "+" else minus if gene.strand == "-" else None
            if target is not None:
                target.append(gene_id)
        if plus or minus:
            emit({"kind": "domain", "contig": contig, "begin": begin, "end": end,
                  "plus": plus, "minus": minus})
            domain_count += 1
            if domain_count > 2 * MAX_GENES + len(PRIMARY):
                raise RuntimeError("domain resource bound exceeded")
emit(environment_payload("environment_end"))
emit({"kind": "summary", "genes": len(genes), "domains": domain_count})
"#;

#[derive(Debug)]
pub struct MaskBuildError {
    code: &'static str,
    message: String,
}

impl MaskBuildError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        let mut message = message.into();
        if message.len() > MAX_ERROR_BYTES {
            message.truncate(MAX_ERROR_BYTES);
        }
        Self { code, message }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MaskBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MaskBuildError {}

impl From<io::Error> for MaskBuildError {
    fn from(_: io::Error) -> Self {
        Self::new("IO", "mask maintenance I/O failed")
    }
}

impl From<pangopup_index::mask_candidates::MaskCandidateError> for MaskBuildError {
    fn from(error: pangopup_index::mask_candidates::MaskCandidateError) -> Self {
        if matches!(
            error,
            pangopup_index::mask_candidates::MaskCandidateError::Cancelled
        ) {
            Self::new("CANCELLED", "qualification was cancelled")
        } else {
            Self::new("CANDIDATE", "mask candidate operation failed")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Capture,
    Prepare,
    Benchmark,
}

impl Phase {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Prepare => "prepare",
            Self::Benchmark => "benchmark",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseReceipt {
    pub schema: String,
    pub profile: String,
    pub contract_id: String,
    pub phase: Phase,
    pub builder_source_sha256: String,
    pub inputs: BTreeMap<String, Identity>,
    pub outputs: BTreeMap<String, Identity>,
    pub next_phase: Option<Phase>,
    pub reused_from: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FailureReceipt {
    pub schema: String,
    pub profile: String,
    pub contract_id: String,
    pub failed_phase: Phase,
    pub code: String,
    pub message: String,
    pub sealed_phases: Vec<Phase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleIdentity {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub device: u64,
    pub inode: u64,
    pub links: u64,
    pub modified_ns: i64,
    pub changed_ns: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationEnvironment {
    pub kind: String,
    pub schema: String,
    pub python: String,
    pub gffutils: String,
    pub executable: String,
    pub prefix: String,
    pub base_prefix: String,
    pub base_executable: String,
    pub sqlite3_module: String,
    pub sqlite_library: String,
    pub sql_row_control_sha256: String,
    pub sqlite_compile_options: Vec<String>,
    pub schema_sha256: String,
    pub query_shape: String,
    pub region_sql: String,
    pub query_plan: Vec<Vec<serde_json::Value>>,
    pub modules: Vec<ModuleIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedGene {
    pub kind: String,
    pub id: String,
    pub contig: String,
    pub strand: String,
    pub start: u32,
    pub end: u32,
    pub boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedDomain {
    pub kind: String,
    pub contig: String,
    pub begin: u32,
    pub end: u32,
    pub plus: Vec<String>,
    pub minus: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObservationSummary {
    kind: String,
    genes: usize,
    domains: usize,
}

#[derive(Debug)]
pub struct Observation {
    pub environment: ObservationEnvironment,
    pub genes: Vec<ObservedGene>,
    pub domains: Vec<ObservedDomain>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalGeneLine {
    pub schema: String,
    pub id: String,
    pub stable_id: String,
    pub contig: String,
    pub strand: String,
    pub start: u32,
    pub end: u32,
    pub rank: u32,
    pub boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalDomainLine {
    pub schema: String,
    pub contig: String,
    pub begin: u32,
    pub end: u32,
    pub plus: Vec<String>,
    pub minus: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema: String,
    pub profile: String,
    pub builder_source_sha256: String,
    pub genes: u64,
    pub plus_genes: u64,
    pub minus_genes: u64,
    pub primary_contigs: u64,
    pub boundaries: u64,
    pub maximum_boundaries_per_gene: u64,
    pub empty_boundary_genes: u64,
    pub versioned_genes: u64,
    pub par_y_genes: u64,
    pub distinct_stable_ids: u64,
    pub stable_collisions: u64,
    pub duplicate_exact_ids: u64,
    pub domains: u64,
    pub same_strand_multi_domains: u64,
    pub opposite_strand_multi_domains: u64,
    pub canonical_stream: Identity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceQuery {
    pub ordinal: u16,
    pub stratum: String,
    pub contig: String,
    pub position: u32,
    pub expected_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceManifest {
    pub schema: String,
    pub profile: String,
    pub strata: Vec<PerformanceStratum>,
    pub queries: Vec<PerformanceQuery>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceStratum {
    pub name: String,
    pub requested: u16,
    pub eligible: u64,
    pub distinct: u16,
    pub repeated: u16,
}

#[derive(Clone, Debug)]
struct HeldIdentity {
    size: u64,
    device: u64,
    inode: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[derive(Debug)]
struct HeldFile {
    file: File,
    identity: HeldIdentity,
}

struct CaptureSources {
    database: HeldFile,
    gtf: HeldFile,
    python: HeldFile,
    python_environment: HeldPythonEnvironment,
    environment: ObservationEnvironment,
}

struct PreparedCapture {
    sources: CaptureSources,
    helper_identity: Identity,
    contract_bytes: Vec<u8>,
    contract_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonEnvironmentIdentity {
    pub launcher: String,
    pub prefix: String,
    pub base_prefix: String,
    pub base_executable: String,
    pub launcher_link: Identity,
    pub pyvenv_config: Identity,
}

#[derive(Clone, Debug)]
struct HeldSymlinkIdentity {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

struct HeldPythonEnvironment {
    launcher: PathBuf,
    prefix: PathBuf,
    base_prefix: PathBuf,
    base_executable: PathBuf,
    prefix_directory: File,
    prefix_directory_device: u64,
    prefix_directory_inode: u64,
    launcher_directory: File,
    launcher_directory_device: u64,
    launcher_directory_inode: u64,
    launcher_name: OsString,
    launcher_symlink: HeldSymlinkIdentity,
    launcher_link: Identity,
    pyvenv_config: HeldFile,
    pyvenv_identity: Identity,
}

impl HeldPythonEnvironment {
    fn evidence(&self) -> Result<PythonEnvironmentIdentity, MaskBuildError> {
        Ok(PythonEnvironmentIdentity {
            launcher: exact_path_text(&self.launcher, "Python launcher")?,
            prefix: exact_path_text(&self.prefix, "Python prefix")?,
            base_prefix: exact_path_text(&self.base_prefix, "Python base prefix")?,
            base_executable: exact_path_text(&self.base_executable, "Python executable")?,
            launcher_link: self.launcher_link.clone(),
            pyvenv_config: self.pyvenv_identity.clone(),
        })
    }
}

struct StageLease {
    parent: File,
    directory: File,
    original_name: OsString,
    current_name: OsString,
    device: u64,
    inode: u64,
}

impl StageLease {
    fn open(stage: &Path) -> Result<Self, MaskBuildError> {
        let parent_path = stage
            .parent()
            .ok_or_else(|| MaskBuildError::new("STAGE", "stage parent is missing"))?;
        let name = stage
            .file_name()
            .ok_or_else(|| MaskBuildError::new("STAGE", "stage name is missing"))?
            .to_owned();
        let parent = open_absolute_directory(parent_path)?;
        let descriptor = rustix::fs::openat(
            &parent,
            &name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| MaskBuildError::new("STAGE", "private stage is unavailable"))?;
        let directory = File::from(descriptor);
        let metadata = directory
            .metadata()
            .map_err(|_| MaskBuildError::new("STAGE", "stage metadata is unavailable"))?;
        // SAFETY: geteuid has no preconditions.
        let effective_uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 || metadata.uid() != effective_uid
        {
            return Err(MaskBuildError::new(
                "STAGE",
                "stage must be a private owned directory",
            ));
        }
        let lease = Self {
            parent,
            directory,
            original_name: name.clone(),
            current_name: name,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        lease.verify_current()?;
        Ok(lease)
    }

    fn verify_current(&self) -> Result<(), MaskBuildError> {
        let descriptor = rustix::fs::openat(
            &self.parent,
            &self.current_name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| MaskBuildError::new("STAGE_LOCATION", "stage entry is unavailable"))?;
        let metadata = File::from(descriptor)
            .metadata()
            .map_err(|_| MaskBuildError::new("STAGE_LOCATION", "stage entry metadata failed"))?;
        if metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(MaskBuildError::new(
                "STAGE_LOCATION",
                "stage entry identity changed",
            ));
        }
        Ok(())
    }

    fn member_exists(&self, name: &str) -> Result<bool, MaskBuildError> {
        match rustix::fs::statat(&self.directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
            Err(_) => Err(MaskBuildError::new(
                "STAGE",
                "stage member state is unavailable",
            )),
        }
    }

    fn write_member(&self, name: &str, bytes: &[u8], mode: u32) -> Result<(), MaskBuildError> {
        let descriptor = rustix::fs::openat(
            &self.directory,
            name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from(mode),
        )
        .map_err(|_| MaskBuildError::new("STAGE", "stage member creation failed"))?;
        let mut file = File::from(descriptor);
        file.write_all(bytes)?;
        file.sync_all()?;
        self.directory.sync_all()?;
        Ok(())
    }

    fn publish(&mut self, destination: &OsStr) -> Result<(), MaskBuildError> {
        self.publish_with_parent_sync(destination, |parent| parent.sync_all())
    }

    fn publish_with_parent_sync(
        &mut self,
        destination: &OsStr,
        mut sync_parent: impl FnMut(&File) -> io::Result<()>,
    ) -> Result<(), MaskBuildError> {
        self.verify_current()?;
        check_cancellation()?;
        rustix::fs::renameat_with(
            &self.parent,
            &self.current_name,
            &self.parent,
            destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| MaskBuildError::new("PUBLICATION", "no-replace publication failed"))?;
        self.current_name = destination.to_owned();
        if let Err(error) = self.verify_current().and_then(|_| {
            sync_parent(&self.parent)
                .map_err(|_| MaskBuildError::new("PUBLICATION", "publication parent sync failed"))
        }) {
            let rollback = rustix::fs::renameat_with(
                &self.parent,
                &self.current_name,
                &self.parent,
                &self.original_name,
                rustix::fs::RenameFlags::NOREPLACE,
            );
            if rollback.is_ok() {
                self.current_name = self.original_name.clone();
                if sync_parent(&self.parent).is_err() {
                    return Err(MaskBuildError::new(
                        "DURABILITY_UNCERTAIN",
                        "publication rollback succeeded but parent sync failed",
                    ));
                }
            }
            return Err(if rollback.is_ok() {
                error
            } else {
                MaskBuildError::new(
                    "DURABILITY_UNCERTAIN",
                    "publication rollback failed; held stage retained",
                )
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CaptureArguments {
    pub database: PathBuf,
    pub gtf: PathBuf,
    pub python: PathBuf,
    pub python_launcher: PathBuf,
    pub output_parent: PathBuf,
    pub expected_database: Identity,
    pub expected_gtf: Identity,
    pub expected_python: Option<Identity>,
    pub expected_launcher_link: Identity,
    pub expected_pyvenv_config: Identity,
    pub environment_policy: EnvironmentPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentPolicy {
    pub python: String,
    pub gffutils: String,
    pub sqlite3_module: String,
    pub sqlite_library: String,
    pub schema_sha256: String,
    pub query_plan_contains: String,
}

impl EnvironmentPolicy {
    pub fn production() -> Self {
        Self {
            python: "3.13.5".into(),
            gffutils: "0.14".into(),
            sqlite3_module: "2.6.0".into(),
            sqlite_library: "3.49.1".into(),
            schema_sha256: SCHEMA_SHA256.into(),
            query_plan_contains: "USING INDEX seqidstartend".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CapturePreflightContract {
    schema: String,
    profile: String,
    builder_source_sha256: String,
    helper: Identity,
    database: Identity,
    gtf: Identity,
    python: Identity,
    launcher_link: Identity,
    pyvenv_config: Identity,
    environment_policy: EnvironmentPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PreflightFailureReceipt {
    schema: String,
    preflight_id: String,
    contract: CapturePreflightContract,
    failed_phase: Phase,
    code: String,
    message: String,
    sealed_phases: Vec<Phase>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaptureOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contract_id: String,
    pub observation: Identity,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrepareOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contract_id: String,
    pub genes: u64,
    pub domains: u64,
    pub queries: u64,
    pub candidates: BTreeMap<String, Identity>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contract_id: String,
    pub sealed_phases: Vec<Phase>,
    pub failed: bool,
}

fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, MaskBuildError> {
    let mut bytes = serde_jcs::to_vec(value)
        .map_err(|_| MaskBuildError::new("JSON", "canonical JSON encoding failed"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonical_environment_bytes(
    environment: &ObservationEnvironment,
) -> Result<Vec<u8>, MaskBuildError> {
    if environment.modules.len() > MAX_ENVIRONMENT_MODULES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "complete environment evidence exceeds its byte bound",
        ));
    }
    for module in &environment.modules {
        let bytes = serde_jcs::to_vec(module)
            .map_err(|_| MaskBuildError::new("JSON", "canonical JSON encoding failed"))?;
        if bytes.len() > MAX_MODULE_IDENTITY_BYTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "complete environment evidence exceeds its byte bound",
            ));
        }
    }
    let mut non_module = serde_json::to_value(environment)
        .map_err(|_| MaskBuildError::new("JSON", "canonical JSON encoding failed"))?;
    non_module
        .as_object_mut()
        .ok_or_else(|| MaskBuildError::new("JSON", "environment JSON is not an object"))?
        .remove("modules")
        .ok_or_else(|| MaskBuildError::new("JSON", "environment modules are missing"))?;
    if canonical(&non_module)?.len() > MAX_METADATA_BYTES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "complete environment evidence exceeds its byte bound",
        ));
    }
    let bytes = canonical(environment)?;
    if bytes.len() > MAX_ENVIRONMENT_BYTES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "complete environment evidence exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn validate_capture_contract_bytes(
    contract: &impl Serialize,
    environment: &ObservationEnvironment,
    bytes: &[u8],
) -> Result<(), MaskBuildError> {
    canonical_environment_bytes(environment)?;
    let mut envelope = serde_json::to_value(contract)
        .map_err(|_| MaskBuildError::new("JSON", "canonical JSON encoding failed"))?;
    envelope
        .as_object_mut()
        .ok_or_else(|| MaskBuildError::new("JSON", "capture contract is not an object"))?
        .remove("environment")
        .ok_or_else(|| MaskBuildError::new("JSON", "capture contract environment is missing"))?;
    if canonical(&envelope)?.len() > MAX_METADATA_BYTES || bytes.len() > MAX_CAPTURE_CONTRACT_BYTES
    {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "capture contract exceeds its byte bound",
        ));
    }
    Ok(())
}

fn identity(bytes: &[u8]) -> Identity {
    Identity {
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Serialize)]
struct CaptureContract<'a> {
    schema: &'static str,
    profile: &'static str,
    builder_source_sha256: &'static str,
    helper: Identity,
    database: &'a Identity,
    gtf: &'a Identity,
    python: &'a Identity,
    python_environment: &'a PythonEnvironmentIdentity,
    environment: &'a ObservationEnvironment,
}

/// Authenticate, privately snapshot, and observe the exact annotation inputs.
/// A created stage is preserved with a failure receipt on every handled error.
pub fn capture_phase(arguments: &CaptureArguments) -> Result<CaptureOutcome, MaskBuildError> {
    require_absolute(&arguments.database, "database")?;
    require_absolute(&arguments.gtf, "GTF")?;
    require_absolute(&arguments.python, "Python")?;
    require_absolute(&arguments.python_launcher, "Python launcher")?;
    require_absolute(&arguments.output_parent, "output parent")?;
    validate_expected_identity(&arguments.expected_database, MAX_DATABASE_BYTES)?;
    validate_expected_identity(&arguments.expected_gtf, MAX_GTF_BYTES)?;
    let expected_python = arguments
        .expected_python
        .as_ref()
        .ok_or_else(|| MaskBuildError::new("CONTRACT", "Python identity is required"))?;
    validate_expected_identity(expected_python, MAX_PYTHON_BYTES)?;
    validate_expected_identity(&arguments.expected_launcher_link, MAX_PYTHON_LAUNCHER_BYTES)?;
    validate_expected_identity(&arguments.expected_pyvenv_config, MAX_PYVENV_CONFIG_BYTES)?;
    let output_parent = open_absolute_directory(&arguments.output_parent)?;
    let preflight_contract = capture_preflight_contract(arguments, expected_python);
    let preflight_bytes = canonical(&preflight_contract)?;
    let preflight_id = identity(&preflight_bytes).sha256;
    let prepared = capture_preflight_or_preserve(
        arguments,
        &output_parent,
        &preflight_id,
        preflight_contract,
        |python, database, environment| {
            probe_observation_environment(python, database, environment)
        },
    )?;
    let PreparedCapture {
        mut sources,
        helper_identity,
        contract_bytes,
        contract_id,
    } = prepared;
    let stage = arguments
        .output_parent
        .join(format!("{STAGE_PREFIX}{contract_id}"));
    create_private_stage(&arguments.output_parent, &stage)?;
    let lease = StageLease::open(&stage)?;
    let result = capture_into_stage(
        arguments,
        &stage,
        &contract_id,
        &contract_bytes,
        &helper_identity,
        &mut sources,
    )
    .and_then(|observation| {
        lease.verify_current()?;
        Ok(observation)
    });
    match result {
        Ok(observation) => Ok(CaptureOutcome {
            ok: true,
            command: "capture",
            contract_id,
            observation,
        }),
        Err(error) => {
            preserve_failure_held(&lease, &contract_id, Phase::Capture, &error)?;
            Err(error)
        }
    }
}

fn capture_preflight_or_preserve(
    arguments: &CaptureArguments,
    output_parent: &File,
    preflight_id: &str,
    contract: CapturePreflightContract,
    probe: impl FnOnce(
        &mut HeldFile,
        &mut HeldFile,
        &mut HeldPythonEnvironment,
    ) -> Result<ObservationEnvironment, MaskBuildError>,
) -> Result<PreparedCapture, MaskBuildError> {
    let result = authenticate_capture_preflight(arguments, probe)
        .and_then(|sources| prepare_capture_contract(arguments, sources));
    preserve_capture_preflight_result(output_parent, preflight_id, contract, result)
}

fn preserve_capture_preflight_result(
    output_parent: &File,
    preflight_id: &str,
    contract: CapturePreflightContract,
    result: Result<PreparedCapture, MaskBuildError>,
) -> Result<PreparedCapture, MaskBuildError> {
    match result {
        Ok(prepared) => Ok(prepared),
        Err(error) => {
            preserve_preflight_failure_held(output_parent, preflight_id, contract, &error)?;
            Err(error)
        }
    }
}

fn prepare_capture_contract(
    arguments: &CaptureArguments,
    sources: CaptureSources,
) -> Result<PreparedCapture, MaskBuildError> {
    let expected_python = arguments
        .expected_python
        .as_ref()
        .ok_or_else(|| MaskBuildError::new("CONTRACT", "Python identity is required"))?;
    let python_environment = sources.python_environment.evidence()?;
    let helper_identity = identity(OBSERVATION_HELPER.as_bytes());
    let contract = CaptureContract {
        schema: CAPTURE_CONTRACT_SCHEMA,
        profile: MASK_PROFILE,
        builder_source_sha256: BUILDER_SOURCE_SHA256,
        helper: helper_identity.clone(),
        database: &arguments.expected_database,
        gtf: &arguments.expected_gtf,
        python: expected_python,
        python_environment: &python_environment,
        environment: &sources.environment,
    };
    let contract_bytes = canonical(&contract)?;
    validate_capture_contract_bytes(&contract, &sources.environment, &contract_bytes)?;
    let contract_id = identity(&contract_bytes).sha256;
    Ok(PreparedCapture {
        sources,
        helper_identity,
        contract_bytes,
        contract_id,
    })
}

fn authenticate_capture_preflight(
    arguments: &CaptureArguments,
    probe: impl FnOnce(
        &mut HeldFile,
        &mut HeldFile,
        &mut HeldPythonEnvironment,
    ) -> Result<ObservationEnvironment, MaskBuildError>,
) -> Result<CaptureSources, MaskBuildError> {
    reject_database_sidecars(&arguments.database)?;
    let mut database = open_held(&arguments.database, MAX_DATABASE_BYTES)?;
    let mut gtf = open_held(&arguments.gtf, MAX_GTF_BYTES)?;
    let mut python = open_held(&arguments.python, MAX_PYTHON_BYTES)?;
    let database_identity = authenticate_held(&mut database)?;
    let gtf_identity = authenticate_held(&mut gtf)?;
    let python_identity = authenticate_held(&mut python)?;
    if database_identity != arguments.expected_database
        || gtf_identity != arguments.expected_gtf
        || Some(&python_identity) != arguments.expected_python.as_ref()
    {
        return Err(MaskBuildError::new(
            "SOURCE_IDENTITY",
            "authenticated input identity mismatch",
        ));
    }
    let mut python_environment = open_python_environment(arguments, &python)?;
    verify_python_environment(&mut python_environment, &python)?;
    let exact_environment = probe(&mut python, &mut database, &mut python_environment)?;
    let environment_evidence = python_environment.evidence()?;
    validate_environment(
        &exact_environment,
        &arguments.environment_policy,
        &environment_evidence,
    )?;
    verify_held(&database)?;
    verify_held(&gtf)?;
    verify_held(&python)?;
    verify_python_environment(&mut python_environment, &python)?;
    reject_database_sidecars(&arguments.database)?;
    Ok(CaptureSources {
        database,
        gtf,
        python,
        python_environment,
        environment: exact_environment,
    })
}

fn capture_into_stage(
    arguments: &CaptureArguments,
    stage: &Path,
    contract_id: &str,
    contract_bytes: &[u8],
    helper_identity: &Identity,
    sources: &mut CaptureSources,
) -> Result<Identity, MaskBuildError> {
    create_private_directory(&stage.join("source"))?;
    create_private_directory(&stage.join("capture"))?;
    write_synced(&stage.join("contract.json"), contract_bytes, 0o400)?;

    let database_observed = copy_held_authenticated(
        &mut sources.database,
        &stage.join(SNAPSHOT_DATABASE),
        &arguments.expected_database,
    )?;
    let gtf_observed = copy_held_authenticated(
        &mut sources.gtf,
        &stage.join(SNAPSHOT_GTF),
        &arguments.expected_gtf,
    )?;
    let pyvenv_observed = copy_held_authenticated(
        &mut sources.python_environment.pyvenv_config,
        &stage.join(SNAPSHOT_PYVENV_CONFIG),
        &arguments.expected_pyvenv_config,
    )?;
    let python_observed = authenticate_held(&mut sources.python)?;
    if Some(&python_observed) != arguments.expected_python.as_ref() {
        return Err(MaskBuildError::new(
            "PYTHON_IDENTITY",
            "Python executable identity mismatch",
        ));
    }
    reject_database_sidecars(&arguments.database)?;
    verify_python_environment(&mut sources.python_environment, &sources.python)?;
    run_observation_helper(stage, &mut sources.python, &mut sources.python_environment)?;
    verify_held(&sources.database)?;
    verify_held(&sources.gtf)?;
    verify_held(&sources.python)?;
    verify_python_environment(&mut sources.python_environment, &sources.python)?;
    reject_database_sidecars(&stage.join(SNAPSHOT_DATABASE))?;
    verify_file_identity(&stage.join(SNAPSHOT_DATABASE), &database_observed)?;
    verify_file_identity(&stage.join(SNAPSHOT_GTF), &gtf_observed)?;
    verify_file_identity(&stage.join(SNAPSHOT_PYVENV_CONFIG), &pyvenv_observed)?;

    let observation_path = stage.join(OBSERVATION_MEMBER);
    let observation = parse_observation(&observation_path, &sources.environment)?;
    let environment_bytes = canonical_environment_bytes(&observation.environment)?;
    write_synced(&stage.join(ENVIRONMENT_MEMBER), &environment_bytes, 0o400)?;
    let observation_identity = hash_file(&observation_path, MAX_OBSERVATION_BYTES)?;
    let mut inputs = BTreeMap::new();
    inputs.insert("database".into(), database_observed);
    inputs.insert("gtf".into(), gtf_observed);
    inputs.insert("helper".into(), helper_identity.clone());
    inputs.insert("python".into(), python_observed);
    inputs.insert(
        "python_launcher_link".into(),
        sources.python_environment.launcher_link.clone(),
    );
    inputs.insert("pyvenv_config".into(), pyvenv_observed);
    inputs.insert("contract".into(), identity(contract_bytes));
    let mut outputs = BTreeMap::new();
    outputs.insert("observation".into(), observation_identity.clone());
    outputs.insert("environment".into(), identity(&environment_bytes));
    seal_phase(
        stage,
        CAPTURE_RECEIPT,
        PhaseReceipt {
            schema: PHASE_RECEIPT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            contract_id: contract_id.into(),
            phase: Phase::Capture,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            inputs,
            outputs,
            next_phase: Some(Phase::Prepare),
            reused_from: None,
        },
    )?;
    Ok(observation_identity)
}

fn require_absolute(path: &Path, label: &'static str) -> Result<(), MaskBuildError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(MaskBuildError::new(
            "USAGE",
            format!("{label} path must be absolute"),
        ))
    }
}

fn open_absolute_directory(path: &Path) -> Result<File, MaskBuildError> {
    require_absolute(path, "directory")?;
    let descriptor = rustix::fs::open(
        "/",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| MaskBuildError::new("STAGE", "root directory is unavailable"))?;
    let mut current = File::from(descriptor);
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let descriptor = rustix::fs::openat(
                    &current,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| MaskBuildError::new("STAGE", "directory component is unavailable"))?;
                current = File::from(descriptor);
            }
            _ => {
                return Err(MaskBuildError::new(
                    "STAGE",
                    "directory path component is invalid",
                ));
            }
        }
    }
    Ok(current)
}

fn validate_expected_identity(expected: &Identity, maximum: u64) -> Result<(), MaskBuildError> {
    if expected.bytes == 0 || expected.bytes > maximum || !valid_digest(&expected.sha256) {
        Err(MaskBuildError::new("CONTRACT", "input identity is invalid"))
    } else {
        Ok(())
    }
}

fn exact_path_text(path: &Path, label: &'static str) -> Result<String, MaskBuildError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| MaskBuildError::new("USAGE", format!("{label} path must be valid UTF-8")))
}

fn python_environment_paths(
    python: &Path,
    launcher: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf), MaskBuildError> {
    require_absolute(launcher, "Python launcher")?;
    let launcher_directory = launcher
        .parent()
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher parent is missing"))?;
    if launcher_directory.file_name() != Some(OsStr::new("bin")) {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "launcher must use the standard prefix/bin layout",
        ));
    }
    let prefix = launcher_directory
        .parent()
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher prefix is missing"))?
        .to_owned();
    let base_prefix = python
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "Python prefix is missing"))?
        .to_owned();
    Ok((prefix.clone(), base_prefix, prefix.join("pyvenv.cfg")))
}

fn capture_preflight_contract(
    arguments: &CaptureArguments,
    expected_python: &Identity,
) -> CapturePreflightContract {
    CapturePreflightContract {
        schema: "pangopup-mask-capture-preflight-v1".into(),
        profile: MASK_PROFILE.into(),
        builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
        helper: identity(OBSERVATION_HELPER.as_bytes()),
        database: arguments.expected_database.clone(),
        gtf: arguments.expected_gtf.clone(),
        python: expected_python.clone(),
        launcher_link: arguments.expected_launcher_link.clone(),
        pyvenv_config: arguments.expected_pyvenv_config.clone(),
        environment_policy: arguments.environment_policy.clone(),
    }
}

fn held_symlink_identity(stat: &rustix::fs::Stat) -> HeldSymlinkIdentity {
    HeldSymlinkIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
        size: stat.st_size as u64,
        modified_seconds: stat.st_mtime,
        modified_nanoseconds: stat.st_mtime_nsec as i64,
        changed_seconds: stat.st_ctime,
        changed_nanoseconds: stat.st_ctime_nsec as i64,
    }
}

fn same_symlink_identity(left: &HeldSymlinkIdentity, right: &HeldSymlinkIdentity) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && left.size == right.size
        && left.modified_seconds == right.modified_seconds
        && left.modified_nanoseconds == right.modified_nanoseconds
        && left.changed_seconds == right.changed_seconds
        && left.changed_nanoseconds == right.changed_nanoseconds
}

fn metadata_is_held_file(metadata: &fs::Metadata, held: &HeldIdentity) -> bool {
    metadata.file_type().is_file()
        && metadata.len() == held.size
        && metadata.dev() == held.device
        && metadata.ino() == held.inode
        && metadata.mtime() == held.modified_seconds
        && metadata.mtime_nsec() == held.modified_nanoseconds
        && metadata.ctime() == held.changed_seconds
        && metadata.ctime_nsec() == held.changed_nanoseconds
}

fn read_launcher_link(
    directory: &File,
    name: &OsStr,
) -> Result<(HeldSymlinkIdentity, Identity), MaskBuildError> {
    let stat = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher is unavailable"))?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::Symlink {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "launcher must be a symbolic link",
        ));
    }
    let target = rustix::fs::readlinkat(directory, name, Vec::new())
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher link is unreadable"))?;
    let bytes = target.as_bytes();
    if bytes.is_empty() || bytes.len() as u64 > MAX_PYTHON_LAUNCHER_BYTES {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "launcher link exceeds its byte bound",
        ));
    }
    Ok((held_symlink_identity(&stat), identity(bytes)))
}

fn read_small_held(held: &mut HeldFile, maximum: u64) -> Result<Vec<u8>, MaskBuildError> {
    held.file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(held.identity.size).unwrap_or(0));
    Read::by_ref(&mut held.file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "pyvenv config exceeds its byte bound",
        ));
    }
    verify_held(held)?;
    Ok(bytes)
}

fn validate_pyvenv_config(
    bytes: &[u8],
    python: &Path,
    policy: &EnvironmentPolicy,
) -> Result<(), MaskBuildError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "pyvenv config is not UTF-8"))?;
    let mut facts = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(MaskBuildError::new(
                "PYTHON_ENVIRONMENT",
                "pyvenv config line is malformed",
            ));
        };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty()
            || value.is_empty()
            || facts.insert(name.to_owned(), value.to_owned()).is_some()
        {
            return Err(MaskBuildError::new(
                "PYTHON_ENVIRONMENT",
                "pyvenv config facts are invalid",
            ));
        }
    }
    let python_home = python
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "Python home is invalid"))?;
    if facts.get("home").map(String::as_str) != Some(python_home)
        || facts.get("implementation").map(String::as_str) != Some("CPython")
        || facts.get("version_info").map(String::as_str) != Some(policy.python.as_str())
        || facts
            .get("include-system-site-packages")
            .map(String::as_str)
            != Some("false")
    {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "pyvenv config facts do not select the pinned interpreter",
        ));
    }
    Ok(())
}

fn open_python_environment(
    arguments: &CaptureArguments,
    python: &HeldFile,
) -> Result<HeldPythonEnvironment, MaskBuildError> {
    let (prefix, base_prefix, pyvenv_path) =
        python_environment_paths(&arguments.python, &arguments.python_launcher)?;
    let _ = exact_path_text(&arguments.python_launcher, "Python launcher")?;
    let _ = exact_path_text(&prefix, "Python prefix")?;
    let _ = exact_path_text(&base_prefix, "Python base prefix")?;
    let _ = exact_path_text(&arguments.python, "Python executable")?;
    let prefix_directory = open_absolute_directory(&prefix)?;
    let prefix_metadata = prefix_directory
        .metadata()
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "prefix metadata failed"))?;
    let launcher_parent = arguments
        .python_launcher
        .parent()
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher parent is missing"))?;
    let launcher_directory = open_absolute_directory(launcher_parent)?;
    let launcher_directory_metadata = launcher_directory
        .metadata()
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher metadata failed"))?;
    let launcher_name = arguments
        .python_launcher
        .file_name()
        .ok_or_else(|| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher name is missing"))?
        .to_owned();
    let (launcher_symlink, launcher_link) =
        read_launcher_link(&launcher_directory, &launcher_name)?;
    if launcher_link != arguments.expected_launcher_link {
        return Err(MaskBuildError::new(
            "SOURCE_IDENTITY",
            "Python launcher identity mismatch",
        ));
    }
    let followed = File::from(
        rustix::fs::openat(
            &launcher_directory,
            &launcher_name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher target failed"))?,
    );
    if !metadata_is_held_file(
        &followed
            .metadata()
            .map_err(|_| MaskBuildError::new("PYTHON_ENVIRONMENT", "launcher target failed"))?,
        &python.identity,
    ) {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "launcher does not select the pinned Python executable",
        ));
    }
    let mut pyvenv_config = open_held(&pyvenv_path, MAX_PYVENV_CONFIG_BYTES)?;
    let pyvenv_identity = authenticate_held(&mut pyvenv_config)?;
    if pyvenv_identity != arguments.expected_pyvenv_config {
        return Err(MaskBuildError::new(
            "SOURCE_IDENTITY",
            "pyvenv config identity mismatch",
        ));
    }
    let pyvenv_bytes = read_small_held(&mut pyvenv_config, MAX_PYVENV_CONFIG_BYTES)?;
    validate_pyvenv_config(
        &pyvenv_bytes,
        &arguments.python,
        &arguments.environment_policy,
    )?;
    let mut held = HeldPythonEnvironment {
        launcher: arguments.python_launcher.clone(),
        prefix,
        base_prefix,
        base_executable: arguments.python.clone(),
        prefix_directory,
        prefix_directory_device: prefix_metadata.dev(),
        prefix_directory_inode: prefix_metadata.ino(),
        launcher_directory,
        launcher_directory_device: launcher_directory_metadata.dev(),
        launcher_directory_inode: launcher_directory_metadata.ino(),
        launcher_name,
        launcher_symlink,
        launcher_link,
        pyvenv_config,
        pyvenv_identity,
    };
    verify_python_environment(&mut held, python)?;
    Ok(held)
}

fn verify_python_environment(
    environment: &mut HeldPythonEnvironment,
    python: &HeldFile,
) -> Result<(), MaskBuildError> {
    verify_held(python)?;
    verify_held(&environment.pyvenv_config)?;
    let prefix = open_absolute_directory(&environment.prefix)?;
    let prefix_metadata = prefix
        .metadata()
        .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "Python prefix changed"))?;
    if prefix_metadata.dev() != environment.prefix_directory_device
        || prefix_metadata.ino() != environment.prefix_directory_inode
    {
        return Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "Python prefix changed during authentication",
        ));
    }
    let launcher_parent = environment
        .launcher
        .parent()
        .ok_or_else(|| MaskBuildError::new("SOURCE_MUTATION", "launcher parent changed"))?;
    let launcher_directory = open_absolute_directory(launcher_parent)?;
    let launcher_metadata = launcher_directory
        .metadata()
        .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "launcher directory changed"))?;
    if launcher_metadata.dev() != environment.launcher_directory_device
        || launcher_metadata.ino() != environment.launcher_directory_inode
    {
        return Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "launcher directory changed during authentication",
        ));
    }
    let (symlink, link) =
        read_launcher_link(&environment.launcher_directory, &environment.launcher_name)?;
    if !same_symlink_identity(&symlink, &environment.launcher_symlink)
        || link != environment.launcher_link
    {
        return Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "Python launcher changed during authentication",
        ));
    }
    let followed = File::from(
        rustix::fs::openat(
            &environment.launcher_directory,
            &environment.launcher_name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "launcher target changed"))?,
    );
    if !metadata_is_held_file(
        &followed
            .metadata()
            .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "launcher target changed"))?,
        &python.identity,
    ) {
        return Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "launcher target changed during authentication",
        ));
    }
    let config = File::from(
        rustix::fs::openat(
            &environment.prefix_directory,
            "pyvenv.cfg",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "pyvenv config changed"))?,
    );
    if !metadata_is_held_file(
        &config
            .metadata()
            .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "pyvenv config changed"))?,
        &environment.pyvenv_config.identity,
    ) || authenticate_held(&mut environment.pyvenv_config)? != environment.pyvenv_identity
    {
        return Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "pyvenv config changed during authentication",
        ));
    }
    Ok(())
}

fn reject_database_sidecars(database: &Path) -> Result<(), MaskBuildError> {
    let parent = database
        .parent()
        .ok_or_else(|| MaskBuildError::new("SOURCE", "database parent is missing"))?;
    let directory = open_absolute_directory(parent)?;
    let name = database
        .file_name()
        .ok_or_else(|| MaskBuildError::new("SOURCE", "database filename is invalid"))?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = name.to_os_string();
        sidecar.push(suffix);
        match rustix::fs::statat(&directory, &sidecar, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => {
                return Err(MaskBuildError::new(
                    "DATABASE_SIDECAR",
                    "database sidecar state is present",
                ));
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(_) => {
                return Err(MaskBuildError::new(
                    "DATABASE_SIDECAR",
                    "database sidecar state cannot be inspected",
                ));
            }
        }
    }
    Ok(())
}

fn create_private_stage(parent: &Path, stage: &Path) -> Result<(), MaskBuildError> {
    if stage.parent() != Some(parent) {
        return Err(MaskBuildError::new(
            "OUTPUT",
            "stage must be a direct child of its output parent",
        ));
    }
    let directory = open_absolute_directory(parent)?;
    let name = stage
        .file_name()
        .ok_or_else(|| MaskBuildError::new("OUTPUT", "stage name is missing"))?;
    rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::from(0o700))
        .map_err(|_| MaskBuildError::new("OUTPUT", "contract stage already exists"))?;
    directory.sync_all()?;
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), MaskBuildError> {
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)?;
    sync_directory(
        path.parent()
            .ok_or_else(|| MaskBuildError::new("IO", "directory parent is missing"))?,
    )?;
    Ok(())
}

fn open_held(path: &Path, maximum: u64) -> Result<HeldFile, MaskBuildError> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| MaskBuildError::new("SOURCE", "input is unavailable"))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|_| MaskBuildError::new("SOURCE", "input metadata is unavailable"))?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        return Err(MaskBuildError::new(
            "SOURCE",
            "input must be a bounded single-link regular file",
        ));
    }
    Ok(HeldFile {
        identity: HeldIdentity {
            size: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        },
        file,
    })
}

fn verify_held(held: &HeldFile) -> Result<(), MaskBuildError> {
    let metadata = held
        .file
        .metadata()
        .map_err(|_| MaskBuildError::new("SOURCE_MUTATION", "input metadata changed"))?;
    let expected = &held.identity;
    if metadata.file_type().is_file()
        && metadata.nlink() == 1
        && metadata.len() == expected.size
        && metadata.dev() == expected.device
        && metadata.ino() == expected.inode
        && metadata.mtime() == expected.modified_seconds
        && metadata.mtime_nsec() == expected.modified_nanoseconds
        && metadata.ctime() == expected.changed_seconds
        && metadata.ctime_nsec() == expected.changed_nanoseconds
    {
        Ok(())
    } else {
        Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "input changed during authentication",
        ))
    }
}

fn authenticate_held(held: &mut HeldFile) -> Result<Identity, MaskBuildError> {
    held.file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancellation()?;
        let read = held.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "input byte count overflow"))?;
    }
    verify_held(held)?;
    Ok(Identity {
        bytes,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn copy_held_authenticated(
    held: &mut HeldFile,
    destination: &Path,
    expected: &Identity,
) -> Result<Identity, MaskBuildError> {
    held.file.seek(SeekFrom::Start(0))?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(destination)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancellation()?;
        let read = held.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "input byte count overflow"))?;
        if bytes > expected.bytes {
            return Err(MaskBuildError::new(
                "SOURCE_IDENTITY",
                "input identity mismatch",
            ));
        }
    }
    output.sync_all()?;
    verify_held(held)?;
    let observed = Identity {
        bytes,
        sha256: format!("{:x}", hasher.finalize()),
    };
    if &observed != expected {
        return Err(MaskBuildError::new(
            "SOURCE_IDENTITY",
            "input identity mismatch",
        ));
    }
    fs::set_permissions(destination, fs::Permissions::from_mode(0o400))?;
    sync_directory(
        destination
            .parent()
            .ok_or_else(|| MaskBuildError::new("IO", "snapshot parent is missing"))?,
    )?;
    Ok(observed)
}

fn inheritable(file: &File) -> Result<i32, MaskBuildError> {
    let descriptor = file.as_raw_fd();
    // SAFETY: fcntl reads and updates flags on a live held descriptor.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0
    {
        return Err(MaskBuildError::new(
            "PYTHON",
            "helper descriptor could not be inherited",
        ));
    }
    Ok(flags)
}

fn restore_descriptor(file: &File, flags: i32) -> Result<(), MaskBuildError> {
    // SAFETY: fcntl restores flags on the same live held descriptor.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags) } < 0 {
        Err(MaskBuildError::new(
            "PYTHON",
            "helper descriptor flags could not be restored",
        ))
    } else {
        Ok(())
    }
}

fn spawn_observation_helper(
    python: &File,
    python_environment: &HeldPythonEnvironment,
    database: &Path,
    inherited_database: Option<&File>,
    gtf: &Path,
    mode: &str,
    current_dir: &Path,
) -> Result<std::process::Child, MaskBuildError> {
    let python_flags = inheritable(python)?;
    let prefix_flags = match inheritable(&python_environment.prefix_directory) {
        Ok(flags) => flags,
        Err(error) => {
            let _ = restore_descriptor(python, python_flags);
            return Err(error);
        }
    };
    let database_flags = match inherited_database.map(inheritable).transpose() {
        Ok(flags) => flags,
        Err(error) => {
            let prefix_restored =
                restore_descriptor(&python_environment.prefix_directory, prefix_flags);
            let python_restored = restore_descriptor(python, python_flags);
            return Err(prefix_restored
                .err()
                .or_else(|| python_restored.err())
                .unwrap_or(error));
        }
    };
    let executable = format!("/proc/self/fd/{}", python.as_raw_fd());
    let held_prefix = format!(
        "/proc/self/fd/{}",
        python_environment.prefix_directory.as_raw_fd()
    );
    let held_launcher = format!(
        "{held_prefix}/bin/{}",
        python_environment.launcher_name.to_string_lossy()
    );
    let mut command = Command::new(executable);
    command
        .arg0(&held_launcher)
        .arg("-I")
        .arg("-S")
        .arg("-B")
        .arg("-X")
        .arg("pycache_prefix=/dev/null")
        .arg("-c")
        .arg(OBSERVATION_HELPER)
        .arg(database)
        .arg(gtf)
        .arg(mode)
        .arg(&python_environment.launcher)
        .arg(&python_environment.prefix)
        .arg(&python_environment.base_prefix)
        .arg(&held_prefix)
        .arg(&python_environment.base_executable)
        .current_dir(current_dir)
        .env("__PYVENV_LAUNCHER__", &held_launcher)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("NUMEXPR_NUM_THREADS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let spawned = command
        .spawn()
        .map_err(|_| MaskBuildError::new("PYTHON", "observation helper could not start"));
    let python_restored = restore_descriptor(python, python_flags);
    let prefix_restored = restore_descriptor(&python_environment.prefix_directory, prefix_flags);
    let database_restored = match (inherited_database, database_flags) {
        (Some(file), Some(flags)) => restore_descriptor(file, flags),
        _ => Ok(()),
    };
    match (spawned, python_restored, prefix_restored, database_restored) {
        (Ok(mut child), Err(error), _, _)
        | (Ok(mut child), _, Err(error), _)
        | (Ok(mut child), _, _, Err(error)) => {
            terminate_child(&mut child);
            let _ = child.wait();
            Err(error)
        }
        (Ok(child), Ok(()), Ok(()), Ok(())) => Ok(child),
        (Err(_), Err(error), _, _) | (Err(_), _, Err(error), _) | (Err(_), _, _, Err(error)) => {
            Err(error)
        }
        (Err(error), Ok(()), Ok(()), Ok(())) => Err(error),
    }
}

fn terminate_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let process_group = -(child.id() as i32);
        // SAFETY: the child was placed in a process group whose id is its pid.
        let _ = unsafe { libc::kill(process_group, libc::SIGKILL) };
    }
    let _ = child.kill();
}

fn wait_for_helper(
    child: &mut std::process::Child,
    exceeded: &AtomicBool,
) -> Result<std::process::ExitStatus, MaskBuildError> {
    loop {
        if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
            terminate_child(child);
            let _ = child.wait();
            return Err(MaskBuildError::new(
                "CANCELLED",
                "qualification was cancelled",
            ));
        }
        if exceeded.load(Ordering::SeqCst) {
            terminate_child(child);
            let _ = child.wait();
            return Err(MaskBuildError::new(
                "RESOURCE",
                "observation helper output exceeds its byte bound",
            ));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|_| MaskBuildError::new("PYTHON", "observation helper wait failed"))?
        {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn helper_exception_class(stderr: &[u8]) -> Option<&str> {
    let marker = stderr
        .strip_suffix(b"\n")?
        .strip_prefix(HELPER_EXCEPTION_PREFIX)?;
    let name = std::str::from_utf8(marker).ok()?;
    let mut characters = name.chars();
    let first = characters.next()?;
    if name.len() > 64
        || !name.is_ascii()
        || !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        None
    } else {
        Some(name)
    }
}

fn helper_result_error(
    success: bool,
    stderr: &[u8],
    has_output: bool,
    context: &'static str,
) -> Option<MaskBuildError> {
    if !success {
        if let Some(class) = helper_exception_class(stderr) {
            return Some(MaskBuildError::new(
                "PYTHON_EXCEPTION",
                format!("{context} raised {class}"),
            ));
        }
        return Some(MaskBuildError::new(
            "PYTHON_PROCESS",
            format!("{context} process failed"),
        ));
    }
    if !stderr.is_empty() {
        Some(MaskBuildError::new(
            "PYTHON_STDERR",
            format!("{context} wrote unexpected diagnostics"),
        ))
    } else if !has_output {
        Some(MaskBuildError::new(
            "PYTHON_OUTPUT",
            format!("{context} produced no output"),
        ))
    } else {
        None
    }
}

fn drain_bounded(
    mut reader: impl Read,
    maximum: usize,
    exceeded: Arc<AtomicBool>,
) -> io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(maximum.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let available = maximum.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(available)]);
        if read > available {
            exceeded.store(true, Ordering::SeqCst);
        }
    }
    Ok(retained)
}

fn drain_observation(
    mut reader: impl Read,
    mut output: File,
    exceeded: Arc<AtomicBool>,
) -> io::Result<u64> {
    let mut buffer = [0_u8; 64 * 1024];
    let mut written = 0_u64;
    let mut line_bytes = 0_usize;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            if *byte == b'\n' {
                line_bytes = 0;
            } else {
                line_bytes = line_bytes.saturating_add(1);
                if line_bytes > MAX_LINE_BYTES {
                    exceeded.store(true, Ordering::SeqCst);
                }
            }
        }
        let available = MAX_OBSERVATION_BYTES.saturating_sub(written);
        let retained = usize::try_from(available).unwrap_or(usize::MAX).min(read);
        if retained != 0 {
            output.write_all(&buffer[..retained])?;
            written += retained as u64;
        }
        if retained != read {
            exceeded.store(true, Ordering::SeqCst);
        }
    }
    output.sync_all()?;
    Ok(written)
}

fn probe_observation_environment(
    python: &mut HeldFile,
    database: &mut HeldFile,
    python_environment: &mut HeldPythonEnvironment,
) -> Result<ObservationEnvironment, MaskBuildError> {
    check_cancellation()?;
    let database_path = PathBuf::from(format!("/proc/self/fd/{}", database.file.as_raw_fd()));
    let mut child = spawn_observation_helper(
        &python.file,
        python_environment,
        &database_path,
        Some(&database.file),
        Path::new("/dev/null"),
        "environment",
        Path::new("/"),
    )?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| MaskBuildError::new("PYTHON", "helper stdout is unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| MaskBuildError::new("PYTHON", "helper stderr is unavailable"))?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout_exceeded = Arc::clone(&exceeded);
    let stderr_exceeded = Arc::clone(&exceeded);
    let stdout_reader =
        thread::spawn(move || drain_bounded(stdout, MAX_ENVIRONMENT_BYTES, stdout_exceeded));
    let stderr_reader =
        thread::spawn(move || drain_bounded(stderr, MAX_ERROR_BYTES, stderr_exceeded));
    let status = wait_for_helper(&mut child, &exceeded);
    let stdout = stdout_reader
        .join()
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stdout reader failed"))?
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stdout read failed"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stderr reader failed"))?
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stderr read failed"))?;
    let status = status?;
    verify_held(python)?;
    verify_held(database)?;
    verify_python_environment(python_environment, python)?;
    if let Some(error) = helper_result_error(
        status.success(),
        &stderr,
        !stdout.is_empty(),
        "environment probe",
    ) {
        return Err(error);
    }
    let environment: ObservationEnvironment = parse_canonical(&stdout)?;
    if environment.kind != "environment" {
        return Err(MaskBuildError::new(
            "ENVIRONMENT",
            "environment probe output is invalid",
        ));
    }
    Ok(environment)
}

fn run_observation_helper(
    stage: &Path,
    python: &mut HeldFile,
    python_environment: &mut HeldPythonEnvironment,
) -> Result<(), MaskBuildError> {
    check_cancellation()?;
    let observation_path = stage.join(OBSERVATION_MEMBER);
    let stdout_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o400)
        .open(&observation_path)?;
    let mut child = spawn_observation_helper(
        &python.file,
        python_environment,
        &stage.join(SNAPSHOT_DATABASE),
        None,
        &stage.join(SNAPSHOT_GTF),
        "full",
        stage,
    )?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| MaskBuildError::new("PYTHON", "helper stdout is unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| MaskBuildError::new("PYTHON", "helper stderr is unavailable"))?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout_exceeded = Arc::clone(&exceeded);
    let stderr_exceeded = Arc::clone(&exceeded);
    let stdout_reader =
        thread::spawn(move || drain_observation(stdout, stdout_file, stdout_exceeded));
    let stderr_reader =
        thread::spawn(move || drain_bounded(stderr, MAX_ERROR_BYTES, stderr_exceeded));
    let status = wait_for_helper(&mut child, &exceeded);
    let observed = stdout_reader
        .join()
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stdout reader failed"))?
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stdout write failed"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stderr reader failed"))?
        .map_err(|_| MaskBuildError::new("PYTHON", "helper stderr read failed"))?;
    let status = status?;
    verify_held(python)?;
    verify_python_environment(python_environment, python)?;
    if let Some(error) = helper_result_error(
        status.success(),
        &stderr,
        observed != 0,
        "observation helper",
    ) {
        return Err(error);
    }
    sync_directory(
        observation_path
            .parent()
            .ok_or_else(|| MaskBuildError::new("IO", "observation parent is missing"))?,
    )?;
    Ok(())
}

pub fn parse_observation(
    path: &Path,
    expected_environment: &ObservationEnvironment,
) -> Result<Observation, MaskBuildError> {
    let mut held = open_held(path, MAX_OBSERVATION_BYTES)?;
    let result = parse_observation_file(&mut held.file, expected_environment)?;
    verify_held(&held)?;
    Ok(result)
}

fn parse_observation_file(
    input: &mut File,
    expected_environment: &ObservationEnvironment,
) -> Result<Observation, MaskBuildError> {
    input.seek(SeekFrom::Start(0))?;
    let mut reader = BufReader::new(input);
    let mut line = Vec::new();
    let mut environment = None;
    let mut genes = Vec::new();
    let mut domains = Vec::new();
    let mut summary = None;
    let mut seen_domain = false;
    let mut environment_end = false;
    let mut total_boundaries = 0_usize;
    let mut line_count = 0_u64;
    loop {
        if line_count.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        line.clear();
        let read = read_bounded_line(&mut reader, &mut line, MAX_LINE_BYTES)?;
        if read == 0 {
            break;
        }
        line_count = line_count.saturating_add(1);
        let value: serde_json::Value = serde_json::from_slice(trim_newline(&line))
            .map_err(|_| MaskBuildError::new("OBSERVATION", "observation JSON is invalid"))?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| MaskBuildError::new("OBSERVATION", "observation kind is missing"))?;
        match kind {
            "environment" if environment.is_none() && genes.is_empty() && domains.is_empty() => {
                let decoded: ObservationEnvironment = decode_value(value)?;
                validate_exact_environment(&decoded, expected_environment, "environment")?;
                environment = Some(decoded);
            }
            "gene"
                if environment.is_some()
                    && !seen_domain
                    && !environment_end
                    && summary.is_none() =>
            {
                if genes.len() >= MAX_GENES {
                    return Err(MaskBuildError::new("RESOURCE", "gene count exceeds bound"));
                }
                let gene: ObservedGene = decode_value(value)?;
                validate_observed_gene(&gene)?;
                total_boundaries = total_boundaries
                    .checked_add(gene.boundaries.len())
                    .ok_or_else(|| MaskBuildError::new("RESOURCE", "boundary count overflow"))?;
                if total_boundaries > MAX_BOUNDARIES {
                    return Err(MaskBuildError::new(
                        "RESOURCE",
                        "boundary count exceeds bound",
                    ));
                }
                genes.push(gene);
            }
            "domain" if environment.is_some() && !environment_end && summary.is_none() => {
                seen_domain = true;
                let domain: ObservedDomain = decode_value(value)?;
                validate_observed_domain(&domain)?;
                domains.push(domain);
                if domains.len() > 2 * MAX_GENES + MAX_CONTIGS {
                    return Err(MaskBuildError::new(
                        "RESOURCE",
                        "domain count exceeds bound",
                    ));
                }
            }
            "environment_end" if environment.is_some() && !environment_end && summary.is_none() => {
                let decoded: ObservationEnvironment = decode_value(value)?;
                validate_exact_environment(&decoded, expected_environment, "environment_end")?;
                environment_end = true;
            }
            "summary" if environment.is_some() && environment_end && summary.is_none() => {
                summary = Some(decode_value::<ObservationSummary>(value)?);
            }
            _ => {
                return Err(MaskBuildError::new(
                    "OBSERVATION",
                    "observation record order or kind is invalid",
                ));
            }
        }
    }
    let summary = summary
        .ok_or_else(|| MaskBuildError::new("OBSERVATION", "observation summary is missing"))?;
    if summary.kind != "summary"
        || summary.genes != genes.len()
        || summary.domains != domains.len()
        || genes.is_empty()
        || !environment_end
    {
        return Err(MaskBuildError::new(
            "OBSERVATION",
            "observation summary does not match its records",
        ));
    }
    validate_observation_relationships(&genes, &domains)?;
    Ok(Observation {
        environment: environment
            .ok_or_else(|| MaskBuildError::new("OBSERVATION", "environment is missing"))?,
        genes,
        domains,
    })
}

fn validate_exact_environment(
    environment: &ObservationEnvironment,
    expected: &ObservationEnvironment,
    kind: &str,
) -> Result<(), MaskBuildError> {
    canonical_environment_bytes(environment)?;
    canonical_environment_bytes(expected)?;
    let mut expected = expected.clone();
    expected.kind = kind.into();
    if environment == &expected {
        Ok(())
    } else {
        Err(MaskBuildError::new(
            "ENVIRONMENT",
            "exact observation environment drifted",
        ))
    }
}

fn decode_value<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, MaskBuildError> {
    serde_json::from_value(value)
        .map_err(|_| MaskBuildError::new("OBSERVATION", "observation schema is invalid"))
}

fn validate_environment(
    environment: &ObservationEnvironment,
    policy: &EnvironmentPolicy,
    python_environment: &PythonEnvironmentIdentity,
) -> Result<(), MaskBuildError> {
    if validate_environment_shape(environment).is_err()
        || validate_python_environment_identity(python_environment).is_err()
        || validate_environment_launch(environment, python_environment).is_err()
    {
        return Err(MaskBuildError::new(
            "ENVIRONMENT",
            "observation environment or query plan drifted",
        ));
    }
    let plan = environment
        .query_plan
        .iter()
        .flat_map(|row| row.iter())
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    if environment.kind != "environment"
        || environment.schema != OBSERVATION_SCHEMA
        || environment.python != policy.python
        || environment.gffutils != policy.gffutils
        || environment.sqlite3_module != policy.sqlite3_module
        || environment.sqlite_library != policy.sqlite_library
        || environment.sql_row_control_sha256 != SQL_ROW_CONTROL_SHA256
        || environment.schema_sha256 != policy.schema_sha256
        || environment.query_shape != "gtf.region((contig,pos-1,pos-1),featuretype=gene)"
        || !plan.contains(&policy.query_plan_contains)
    {
        return Err(MaskBuildError::new(
            "ENVIRONMENT",
            "observation environment or query plan drifted",
        ));
    }
    Ok(())
}

fn validate_python_environment_identity(
    environment: &PythonEnvironmentIdentity,
) -> Result<(), MaskBuildError> {
    let launcher = Path::new(&environment.launcher);
    let prefix = Path::new(&environment.prefix);
    let base_prefix = Path::new(&environment.base_prefix);
    let base_executable = Path::new(&environment.base_executable);
    if !launcher.is_absolute()
        || !prefix.is_absolute()
        || !base_prefix.is_absolute()
        || !base_executable.is_absolute()
        || launcher.parent().and_then(Path::file_name) != Some(OsStr::new("bin"))
        || launcher.parent().and_then(Path::parent) != Some(prefix)
        || base_executable.parent().and_then(Path::parent) != Some(base_prefix)
        || validate_expected_identity(&environment.launcher_link, MAX_PYTHON_LAUNCHER_BYTES)
            .is_err()
        || validate_expected_identity(&environment.pyvenv_config, MAX_PYVENV_CONFIG_BYTES).is_err()
    {
        return Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "Python environment identity is invalid",
        ));
    }
    Ok(())
}

fn validate_environment_launch(
    environment: &ObservationEnvironment,
    python_environment: &PythonEnvironmentIdentity,
) -> Result<(), MaskBuildError> {
    if environment.executable != python_environment.launcher
        || environment.prefix != python_environment.prefix
        || environment.base_prefix != python_environment.base_prefix
        || environment.base_executable != python_environment.base_executable
    {
        Err(MaskBuildError::new(
            "PYTHON_ENVIRONMENT",
            "Python environment prefix facts drifted",
        ))
    } else {
        Ok(())
    }
}

fn validate_environment_shape(environment: &ObservationEnvironment) -> Result<(), MaskBuildError> {
    let plan_rows_valid = !environment.query_plan.is_empty()
        && environment.query_plan.len() <= 64
        && environment.query_plan.iter().all(|row| {
            row.len() == 4
                && row[0].as_i64().is_some()
                && row[1].as_i64().is_some()
                && row[2].as_i64().is_some()
                && row[3].as_str().is_some_and(|value| !value.is_empty())
        });
    if environment.schema != OBSERVATION_SCHEMA
        || environment.region_sql.is_empty()
        || environment.region_sql.len() > MAX_METADATA_BYTES
        || environment.sqlite_compile_options.is_empty()
        || environment.sqlite_compile_options.len() > 512
        || environment
            .sqlite_compile_options
            .windows(2)
            .any(|values| values[0] >= values[1])
        || environment
            .sqlite_compile_options
            .iter()
            .any(|value| value.is_empty() || value.len() > 1_024)
        || !valid_digest(&environment.sql_row_control_sha256)
        || !plan_rows_valid
        || environment.modules.is_empty()
        || environment.modules.len() > MAX_ENVIRONMENT_MODULES
        || !environment
            .modules
            .iter()
            .any(|module| module.name == "gffutils")
        || !environment
            .modules
            .iter()
            .any(|module| module.name == "_sqlite3")
        || !environment
            .modules
            .iter()
            .any(|module| module.name == "sqlite3")
        || environment
            .modules
            .windows(2)
            .any(|modules| modules[0].name >= modules[1].name)
        || environment.modules.iter().any(|module| {
            module.name.is_empty()
                || module.path.is_empty()
                || module.path.len() > 4_096
                || module.bytes > MAX_PYTHON_BYTES
                || !valid_digest(&module.sha256)
                || match module.kind.as_str() {
                    "file" => {
                        !Path::new(&module.path).is_absolute()
                            || module.device == 0
                            || module.inode == 0
                            || module.links == 0
                            || module.modified_ns < 0
                            || module.changed_ns < 0
                    }
                    "interpreter" => {
                        !matches!(module.path.as_str(), "built-in" | "frozen")
                            || module.device != 0
                            || module.inode != 0
                            || module.links != 0
                            || module.modified_ns != 0
                            || module.changed_ns != 0
                    }
                    _ => true,
                }
        })
        || canonical_environment_bytes(environment).is_err()
    {
        return Err(MaskBuildError::new(
            "ENVIRONMENT",
            "observation environment shape is invalid",
        ));
    }
    Ok(())
}

fn validate_observed_gene(gene: &ObservedGene) -> Result<(), MaskBuildError> {
    GencodeGeneId::from_str(&gene.id)
        .map_err(|_| MaskBuildError::new("OBSERVATION", "gene identity is invalid"))?;
    Grch38Contig::from_str(&gene.contig)
        .map_err(|_| MaskBuildError::new("OBSERVATION", "gene contig is unsupported"))?;
    if gene.kind != "gene"
        || !matches!(gene.strand.as_str(), "+" | "-")
        || gene.start == 0
        || gene.start > gene.end
        || gene.id.len() > 64
        || gene.boundaries.len() > MAX_BOUNDARIES_PER_GENE
    {
        return Err(MaskBuildError::new(
            "OBSERVATION",
            "gene facts are invalid or noncanonical",
        ));
    }
    let mut prior = None;
    for (index, boundary) in gene.boundaries.iter().copied().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if boundary < gene.start
            || boundary > gene.end
            || prior.is_some_and(|value| value >= boundary)
        {
            return Err(MaskBuildError::new(
                "OBSERVATION",
                "gene facts are invalid or noncanonical",
            ));
        }
        prior = Some(boundary);
    }
    Ok(())
}

fn validate_observed_domain(domain: &ObservedDomain) -> Result<(), MaskBuildError> {
    Grch38Contig::from_str(&domain.contig)
        .map_err(|_| MaskBuildError::new("OBSERVATION", "domain contig is unsupported"))?;
    let plus_duplicates = has_duplicates(&domain.plus)?;
    let minus_duplicates = has_duplicates(&domain.minus)?;
    if domain.kind != "domain"
        || domain.begin == 0
        || domain.begin > domain.end
        || (domain.plus.is_empty() && domain.minus.is_empty())
        || domain.plus.len() + domain.minus.len() > MAX_GENES
        || plus_duplicates
        || minus_duplicates
    {
        return Err(MaskBuildError::new(
            "OBSERVATION",
            "domain facts are invalid",
        ));
    }
    Ok(())
}

fn has_duplicates(values: &[String]) -> Result<bool, MaskBuildError> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if !seen.insert(value) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_observation_relationships(
    genes: &[ObservedGene],
    domains: &[ObservedDomain],
) -> Result<(), MaskBuildError> {
    let mut identities = BTreeMap::new();
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if identities.insert(gene.id.as_str(), gene).is_some() {
            return Err(MaskBuildError::new(
                "OBSERVATION",
                "duplicate exact gene identity",
            ));
        }
    }
    let mut prior: Option<(u8, u32)> = None;
    for (domain_index, domain) in domains.iter().enumerate() {
        if domain_index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let contig = Grch38Contig::from_str(&domain.contig)
            .map_err(|_| MaskBuildError::new("OBSERVATION", "domain contig is unsupported"))?;
        let key = (contig.code(), domain.begin);
        if prior.is_some_and(|value| key <= value) {
            return Err(MaskBuildError::new(
                "OBSERVATION",
                "domains are not in canonical coordinate order",
            ));
        }
        prior = Some(key);
        for (strand, ids) in [("+", &domain.plus), ("-", &domain.minus)] {
            for (id_index, id) in ids.iter().enumerate() {
                if id_index.is_multiple_of(1_024) {
                    check_cancellation()?;
                }
                let gene = identities.get(id.as_str()).ok_or_else(|| {
                    MaskBuildError::new("OBSERVATION", "domain references an unknown gene")
                })?;
                if gene.contig != domain.contig
                    || gene.strand != strand
                    || domain.begin <= gene.start
                    || domain.end > gene.end
                {
                    return Err(MaskBuildError::new(
                        "OBSERVATION",
                        "domain membership contradicts gene facts",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    output: &mut Vec<u8>,
    maximum: usize,
) -> Result<usize, MaskBuildError> {
    let read = reader
        .take((maximum + 1) as u64)
        .read_until(b'\n', output)?;
    if output.len() > maximum || (read != 0 && !output.ends_with(b"\n")) {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "input line exceeds its byte bound",
        ));
    }
    Ok(read)
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

fn hash_file(path: &Path, maximum: u64) -> Result<Identity, MaskBuildError> {
    let mut held = open_held(path, maximum)?;
    authenticate_held(&mut held)
}

fn verify_file_identity(path: &Path, expected: &Identity) -> Result<(), MaskBuildError> {
    if &hash_file(path, expected.bytes)? == expected {
        Ok(())
    } else {
        Err(MaskBuildError::new(
            "SOURCE_MUTATION",
            "staged source identity changed",
        ))
    }
}

fn write_synced(path: &Path, bytes: &[u8], mode: u32) -> Result<(), MaskBuildError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_directory(
        path.parent()
            .ok_or_else(|| MaskBuildError::new("IO", "file parent is missing"))?,
    )?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), MaskBuildError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn seal_phase(
    stage: &Path,
    receipt_name: &str,
    receipt: PhaseReceipt,
) -> Result<(), MaskBuildError> {
    let bytes = canonical(&receipt)?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "phase receipt exceeds bound",
        ));
    }
    check_cancellation()?;
    write_synced(&stage.join(receipt_name), &bytes, 0o400)?;
    sync_directory(stage)?;
    Ok(())
}

fn preserve_failure_held(
    lease: &StageLease,
    contract_id: &str,
    failed_phase: Phase,
    error: &MaskBuildError,
) -> Result<(), MaskBuildError> {
    let mut sealed_phases = Vec::new();
    for (phase, name) in [
        (Phase::Capture, CAPTURE_RECEIPT),
        (Phase::Prepare, PREPARE_RECEIPT),
        (Phase::Benchmark, BENCHMARK_RECEIPT),
    ] {
        if lease.member_exists(name)? {
            sealed_phases.push(phase);
        }
    }
    let receipt = FailureReceipt {
        schema: FAILURE_SCHEMA.into(),
        profile: MASK_PROFILE.into(),
        contract_id: contract_id.into(),
        failed_phase,
        code: error.code.into(),
        message: error.message.clone(),
        sealed_phases,
    };
    if lease.member_exists(FAILURE_RECEIPT)? {
        return Err(MaskBuildError::new(
            "FAILURE_RECEIPT",
            "failure receipt already exists; automatic retry is forbidden",
        ));
    }
    lease.write_member(FAILURE_RECEIPT, &canonical(&receipt)?, 0o400)
}

fn preserve_preflight_failure_held(
    output_parent: &File,
    preflight_id: &str,
    contract: CapturePreflightContract,
    error: &MaskBuildError,
) -> Result<(), MaskBuildError> {
    let expected_id = identity(&canonical(&contract)?).sha256;
    if expected_id != preflight_id || !valid_digest(preflight_id) {
        return Err(MaskBuildError::new(
            "PREFLIGHT_FAILURE",
            "preflight failure identity is invalid",
        ));
    }
    let receipt = PreflightFailureReceipt {
        schema: PREFLIGHT_FAILURE_SCHEMA.into(),
        preflight_id: preflight_id.into(),
        contract,
        failed_phase: Phase::Capture,
        code: error.code.into(),
        message: error.message.clone(),
        sealed_phases: Vec::new(),
    };
    let bytes = canonical(&receipt)?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "preflight failure receipt exceeds its byte bound",
        ));
    }
    let stage_name = OsString::from(format!("{PREFLIGHT_FAILURE_PREFIX}{preflight_id}"));
    rustix::fs::mkdirat(output_parent, &stage_name, rustix::fs::Mode::from(0o700)).map_err(
        |error| {
            if error == rustix::io::Errno::EXIST {
                MaskBuildError::new(
                    "FAILURE_RECEIPT",
                    "preflight failure already exists; automatic retry is forbidden",
                )
            } else {
                MaskBuildError::new(
                    "PREFLIGHT_FAILURE",
                    "preflight failure stage creation failed",
                )
            }
        },
    )?;
    output_parent.sync_all()?;
    let stage = File::from(
        rustix::fs::openat(
            output_parent,
            &stage_name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| {
            MaskBuildError::new("PREFLIGHT_FAILURE", "preflight failure stage open failed")
        })?,
    );
    let metadata = stage.metadata().map_err(|_| {
        MaskBuildError::new(
            "PREFLIGHT_FAILURE",
            "preflight failure stage metadata failed",
        )
    })?;
    // SAFETY: geteuid has no preconditions.
    let effective_uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 || metadata.uid() != effective_uid {
        return Err(MaskBuildError::new(
            "PREFLIGHT_FAILURE",
            "preflight failure stage is not private",
        ));
    }
    let descriptor = rustix::fs::openat(
        &stage,
        FAILURE_RECEIPT,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from(0o400),
    )
    .map_err(|_| {
        MaskBuildError::new(
            "PREFLIGHT_FAILURE",
            "preflight failure receipt creation failed",
        )
    })?;
    let mut file = File::from(descriptor);
    file.write_all(&bytes)?;
    file.sync_all()?;
    stage.sync_all()?;
    output_parent.sync_all()?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnnotationFact {
    id: String,
    contig: Grch38Contig,
    strand: MaskStrand,
    start: u32,
    end: u32,
    boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityPoint {
    pub id: String,
    pub contig: String,
    pub position: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityEvidence {
    corpus: Identity,
    points: Identity,
    values: Vec<CompatibilityPoint>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityPointSet<'a> {
    schema: &'static str,
    profile: &'static str,
    cases_jsonl: Identity,
    points: &'a [CompatibilityPoint],
}

/// Authenticate the retained compatibility corpus, then extract exactly the
/// M01--M14 genomic points in case order for the performance manifest.
pub fn load_compatibility_points(root: &Path) -> Result<CompatibilityEvidence, MaskBuildError> {
    require_absolute(root, "compatibility corpus")?;
    let evidence = crate::compatibility::authenticate_corpus(root).map_err(|_| {
        MaskBuildError::new(
            "COMPATIBILITY",
            "compatibility corpus authentication failed",
        )
    })?;
    if evidence.cases_jsonl_bytes.len() > MAX_PERFORMANCE_BYTES {
        return Err(MaskBuildError::new(
            "COMPATIBILITY",
            "case member exceeds its byte bound",
        ));
    }
    let mut reader = BufReader::new(io::Cursor::new(&evidence.cases_jsonl_bytes));
    let mut line = Vec::new();
    let mut result = Vec::with_capacity(14);
    while result.len() < 14 {
        line.clear();
        if read_bounded_line(&mut reader, &mut line, 256 * 1024)? == 0 {
            return Err(MaskBuildError::new(
                "COMPATIBILITY",
                "model compatibility cases are incomplete",
            ));
        }
        let value: serde_json::Value =
            serde_json::from_slice(trim_newline(&line)).map_err(|_| {
                MaskBuildError::new("COMPATIBILITY", "compatibility case JSON is invalid")
            })?;
        let id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| MaskBuildError::new("COMPATIBILITY", "case identity is missing"))?;
        let expected_prefix = format!("M{:02}-", result.len() + 1);
        let input = value
            .get("input")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| MaskBuildError::new("COMPATIBILITY", "case input is missing"))?;
        let contig = input
            .get("contig")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| MaskBuildError::new("COMPATIBILITY", "case contig is missing"))?;
        let position = input
            .get("position")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value != 0)
            .ok_or_else(|| MaskBuildError::new("COMPATIBILITY", "case position is invalid"))?;
        if !id.starts_with(&expected_prefix)
            || value.get("kind").and_then(serde_json::Value::as_str) != Some("model")
            || input.get("assembly").and_then(serde_json::Value::as_str) != Some("GRCh38")
        {
            return Err(MaskBuildError::new(
                "COMPATIBILITY",
                "compatibility case order or profile drifted",
            ));
        }
        Grch38Contig::from_str(contig)
            .map_err(|_| MaskBuildError::new("COMPATIBILITY", "case contig is invalid"))?;
        result.push(CompatibilityPoint {
            id: id.into(),
            contig: contig.into(),
            position,
        });
    }
    let cases_jsonl = Identity {
        bytes: evidence.cases_jsonl_bytes.len() as u64,
        sha256: evidence.cases_jsonl_sha256,
    };
    let point_bytes = canonical(&CompatibilityPointSet {
        schema: "pangopup-mask-compatibility-points-v1",
        profile: MASK_PROFILE,
        cases_jsonl,
        points: &result,
    })?;
    Ok(CompatibilityEvidence {
        corpus: Identity {
            bytes: evidence.corpus_bytes,
            sha256: evidence.corpus_sha256,
        },
        points: identity(&point_bytes),
        values: result,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnedCaptureContract {
    schema: String,
    profile: String,
    builder_source_sha256: String,
    helper: Identity,
    database: Identity,
    gtf: Identity,
    python: Identity,
    python_environment: PythonEnvironmentIdentity,
    environment: ObservationEnvironment,
}

pub fn prepare_phase(
    stage: &Path,
    compatibility: &CompatibilityEvidence,
) -> Result<PrepareOutcome, MaskBuildError> {
    check_cancellation()?;
    require_absolute(stage, "stage")?;
    let lease = StageLease::open(stage)?;
    let (contract_id, contract, capture_receipt) = authenticate_capture_stage(stage)?;
    lease.verify_current()?;
    if stage.join(PREPARE_RECEIPT).exists()
        || stage.join(BENCHMARK_RECEIPT).exists()
        || stage.join(FAILURE_RECEIPT).exists()
        || stage.join("prepare").exists()
    {
        return Err(MaskBuildError::new(
            "PHASE_STATE",
            "prepare phase is not an automatic retry",
        ));
    }
    let result = (|| {
        create_private_directory(&stage.join("prepare"))?;
        create_private_directory(&stage.join(CANDIDATE_DIRECTORY))?;
        prepare_into_stage(
            stage,
            &contract_id,
            &contract,
            &capture_receipt,
            compatibility,
        )
    })();
    match result {
        Ok(outcome) => {
            lease.verify_current()?;
            Ok(outcome)
        }
        Err(error) => {
            preserve_failure_held(&lease, &contract_id, Phase::Prepare, &error)?;
            Err(error)
        }
    }
}

fn prepare_into_stage(
    stage: &Path,
    contract_id: &str,
    contract: &OwnedCaptureContract,
    capture_receipt: &PhaseReceipt,
    compatibility: &CompatibilityEvidence,
) -> Result<PrepareOutcome, MaskBuildError> {
    let compatibility_points = compatibility.values.as_slice();
    if compatibility_points.len() != 14
        || compatibility_points
            .iter()
            .any(|point| point.id.is_empty() || point.position == 0)
    {
        return Err(MaskBuildError::new(
            "COMPATIBILITY",
            "exactly M01-M14 compatibility points are required",
        ));
    }
    let observation = parse_observation(&stage.join(OBSERVATION_MEMBER), &contract.environment)?;
    check_cancellation()?;
    let gtf_facts = parse_gtf_facts(&stage.join(SNAPSHOT_GTF))?;
    check_cancellation()?;
    let observed_facts = observation_facts(&observation.genes)?;
    certify_gtf_facts(&gtf_facts, &observed_facts)?;
    let genes = canonical_genes(&observation)?;
    certify_observed_domains(&observation, &genes)?;
    let oracle = DomainOracle::new(&observation, &genes)?;

    let canonical_path = stage.join(CANONICAL_MEMBER);
    let mut canonical_writer = BufWriter::new(
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&canonical_path)?,
    );
    let mut canonical_hasher = Sha256::new();
    let mut canonical_bytes = 0_u64;
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let bytes = canonical(&CanonicalGeneLine {
            schema: CANONICAL_SCHEMA.into(),
            id: gene.identity().to_string(),
            stable_id: gene.stable_identity().to_string(),
            contig: gene.contig().to_string(),
            strand: match gene.strand() {
                MaskStrand::Plus => "+".into(),
                MaskStrand::Minus => "-".into(),
            },
            start: gene.start().get(),
            end: gene.end().get(),
            rank: gene.query_rank(),
            boundaries: gene.boundaries().iter().map(|value| value.get()).collect(),
        })?;
        canonical_writer.write_all(&bytes)?;
        canonical_hasher.update(&bytes);
        canonical_bytes = canonical_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "canonical stream overflow"))?;
        if canonical_bytes > MAX_OBSERVATION_BYTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "canonical stream exceeds bound",
            ));
        }
    }
    for (index, domain) in observation.domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let bytes = canonical(&CanonicalDomainLine {
            schema: "pangopup-mask-canonical-domain-v1".into(),
            contig: domain.contig.clone(),
            begin: domain.begin,
            end: domain.end,
            plus: domain.plus.clone(),
            minus: domain.minus.clone(),
        })?;
        canonical_writer.write_all(&bytes)?;
        canonical_hasher.update(&bytes);
        canonical_bytes = canonical_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "canonical stream overflow"))?;
        if canonical_bytes > MAX_OBSERVATION_BYTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "canonical stream exceeds bound",
            ));
        }
    }
    canonical_writer.flush()?;
    canonical_writer.get_ref().sync_all()?;
    fs::set_permissions(&canonical_path, fs::Permissions::from_mode(0o400))?;
    sync_directory(&stage.join("prepare"))?;
    let canonical_identity = Identity {
        bytes: canonical_bytes,
        sha256: format!("{:x}", canonical_hasher.finalize()),
    };
    let inventory = inventory(&genes, &observation.domains, canonical_identity.clone())?;
    if contract.database.bytes == DATABASE_BYTES
        && contract.database.sha256 == DATABASE_SHA256
        && (inventory.genes != 60_649
            || inventory.par_y_genes != 44
            || inventory.stable_collisions != 44)
    {
        return Err(MaskBuildError::new(
            "PRODUCTION_COUNTS",
            "production GENCODE identity counts drifted",
        ));
    }
    let inventory_bytes = canonical(&inventory)?;
    write_synced(&stage.join(INVENTORY_MEMBER), &inventory_bytes, 0o400)?;

    let mut candidates = BTreeMap::new();
    for codec in MaskCandidateCodec::ALL {
        check_cancellation()?;
        let path = stage.join(CANDIDATE_DIRECTORY).join(codec.filename());
        write_mask_candidate_with_cancellation(&path, codec, &genes, &|| {
            check_cancellation().is_err()
        })?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
        let member = hash_file(&path, CANDIDATE_MEMBER_MAX)?;
        let reader = MaskCandidateReader::open(&path)?;
        reader.inspect_payload_with_cancellation(&|| check_cancellation().is_err())?;
        certify_candidate(&reader, &observation, &genes, &oracle)?;
        candidates.insert(codec.name().into(), member);
    }
    sync_directory(&stage.join(CANDIDATE_DIRECTORY))?;

    let performance =
        build_performance_manifest(&observation, &genes, &oracle, compatibility_points)?;
    if performance.queries.len() != 1_000 {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "performance manifest does not contain 1,000 queries",
        ));
    }
    let performance_bytes = encode_performance(&performance)?;
    write_synced(&stage.join(PERFORMANCE_MEMBER), &performance_bytes, 0o400)?;

    let mut inputs = BTreeMap::new();
    inputs.insert(
        "capture_receipt".into(),
        hash_file(&stage.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)?,
    );
    inputs.insert(
        "observation".into(),
        capture_receipt
            .outputs
            .get("observation")
            .cloned()
            .ok_or_else(|| MaskBuildError::new("RECEIPT", "observation receipt is missing"))?,
    );
    inputs.insert("gtf".into(), contract.gtf.clone());
    inputs.insert("compatibility_corpus".into(), compatibility.corpus.clone());
    inputs.insert("compatibility_points".into(), compatibility.points.clone());
    let mut outputs = BTreeMap::new();
    outputs.insert("canonical".into(), canonical_identity);
    outputs.insert("inventory".into(), identity(&inventory_bytes));
    outputs.insert("performance".into(), identity(&performance_bytes));
    outputs.extend(
        candidates
            .iter()
            .map(|(name, value)| (format!("candidate:{name}"), value.clone())),
    );
    seal_phase(
        stage,
        PREPARE_RECEIPT,
        PhaseReceipt {
            schema: PHASE_RECEIPT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            contract_id: contract_id.into(),
            phase: Phase::Prepare,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            inputs,
            outputs,
            next_phase: Some(Phase::Benchmark),
            reused_from: None,
        },
    )?;
    Ok(PrepareOutcome {
        ok: true,
        command: "prepare",
        contract_id: contract_id.into(),
        genes: genes.len() as u64,
        domains: observation.domains.len() as u64,
        queries: performance.queries.len() as u64,
        candidates,
    })
}

fn authenticate_capture_stage(
    stage: &Path,
) -> Result<(String, OwnedCaptureContract, PhaseReceipt), MaskBuildError> {
    authenticate_capture_stage_for_builder(stage, BUILDER_SOURCE_SHA256)
}

fn authenticate_capture_stage_for_builder(
    stage: &Path,
    expected_builder_source_sha256: &str,
) -> Result<(String, OwnedCaptureContract, PhaseReceipt), MaskBuildError> {
    let leaf = stage_contract_id(stage)?;
    let contract_bytes = read_bounded(&stage.join("contract.json"), MAX_CAPTURE_CONTRACT_BYTES)?;
    if identity(&contract_bytes).sha256 != leaf {
        return Err(MaskBuildError::new("CONTRACT", "contract identity drifted"));
    }
    let contract: OwnedCaptureContract = parse_canonical(&contract_bytes)?;
    if contract.schema != CAPTURE_CONTRACT_SCHEMA
        || contract.profile != MASK_PROFILE
        || contract.builder_source_sha256 != expected_builder_source_sha256
        || identity(OBSERVATION_HELPER.as_bytes()) != contract.helper
        || validate_expected_identity(&contract.database, MAX_DATABASE_BYTES).is_err()
        || validate_expected_identity(&contract.gtf, MAX_GTF_BYTES).is_err()
        || validate_expected_identity(&contract.python, MAX_PYTHON_BYTES).is_err()
        || validate_python_environment_identity(&contract.python_environment).is_err()
        || contract.environment.kind != "environment"
        || validate_environment_shape(&contract.environment).is_err()
        || validate_environment_launch(&contract.environment, &contract.python_environment).is_err()
        || validate_capture_contract_bytes(&contract, &contract.environment, &contract_bytes)
            .is_err()
    {
        return Err(MaskBuildError::new("CONTRACT", "capture contract drifted"));
    }
    let receipt_bytes = read_bounded(&stage.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES)?;
    let receipt: PhaseReceipt = parse_canonical(&receipt_bytes)?;
    validate_phase_receipt_for_builder(
        &receipt,
        &leaf,
        Phase::Capture,
        Some(Phase::Prepare),
        expected_builder_source_sha256,
    )?;
    authenticate_receipt_reuse_for_builder(stage, &receipt, expected_builder_source_sha256)?;
    let expected_contract = identity(&contract_bytes);
    if receipt.inputs.get("database") != Some(&contract.database)
        || receipt.inputs.get("gtf") != Some(&contract.gtf)
        || receipt.inputs.get("python") != Some(&contract.python)
        || receipt.inputs.get("python_launcher_link")
            != Some(&contract.python_environment.launcher_link)
        || receipt.inputs.get("pyvenv_config") != Some(&contract.python_environment.pyvenv_config)
        || receipt.inputs.get("helper") != Some(&contract.helper)
        || receipt.inputs.get("contract") != Some(&expected_contract)
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "capture receipt input identities drifted",
        ));
    }
    for (name, path, maximum) in [
        ("database", SNAPSHOT_DATABASE, MAX_DATABASE_BYTES),
        ("gtf", SNAPSHOT_GTF, MAX_GTF_BYTES),
        (
            "pyvenv_config",
            SNAPSHOT_PYVENV_CONFIG,
            MAX_PYVENV_CONFIG_BYTES,
        ),
        ("observation", OBSERVATION_MEMBER, MAX_OBSERVATION_BYTES),
        (
            "environment",
            ENVIRONMENT_MEMBER,
            MAX_ENVIRONMENT_BYTES as u64,
        ),
    ] {
        let expected = receipt
            .inputs
            .get(name)
            .or_else(|| receipt.outputs.get(name))
            .ok_or_else(|| MaskBuildError::new("RECEIPT", "phase member identity is missing"))?;
        if hash_file(&stage.join(path), maximum)? != *expected {
            return Err(MaskBuildError::new(
                "RECEIPT",
                "sealed capture member drifted",
            ));
        }
    }
    Ok((leaf, contract, receipt))
}

fn validate_phase_receipt(
    receipt: &PhaseReceipt,
    contract_id: &str,
    phase: Phase,
    next: Option<Phase>,
) -> Result<(), MaskBuildError> {
    validate_phase_receipt_for_builder(receipt, contract_id, phase, next, BUILDER_SOURCE_SHA256)
}

fn validate_phase_receipt_for_builder(
    receipt: &PhaseReceipt,
    contract_id: &str,
    phase: Phase,
    next: Option<Phase>,
    expected_builder_source_sha256: &str,
) -> Result<(), MaskBuildError> {
    let (inputs, outputs): (&[&str], &[&str]) = match phase {
        Phase::Capture => (
            &[
                "contract",
                "database",
                "gtf",
                "helper",
                "python",
                "python_launcher_link",
                "pyvenv_config",
            ],
            &["environment", "observation"],
        ),
        Phase::Prepare => (
            &[
                "capture_receipt",
                "compatibility_corpus",
                "compatibility_points",
                "gtf",
                "observation",
            ],
            &[
                "candidate:binned-postings",
                "candidate:domains",
                "candidate:interval-tree",
                "canonical",
                "inventory",
                "performance",
            ],
        ),
        Phase::Benchmark => (
            &[
                "candidate:binned-postings",
                "candidate:domains",
                "candidate:interval-tree",
                "performance",
                "prepare_receipt",
            ],
            &["report"],
        ),
    };
    let reused = receipt.reused_from.is_some();
    if receipt.schema != PHASE_RECEIPT_SCHEMA
        || receipt.profile != MASK_PROFILE
        || receipt.contract_id != contract_id
        || receipt.phase != phase
        || receipt.builder_source_sha256 != expected_builder_source_sha256
        || receipt.next_phase != next
        || receipt
            .reused_from
            .as_ref()
            .is_some_and(|digest| !valid_digest(digest))
        || !exact_receipt_map(&receipt.inputs, inputs, reused)
        || !exact_receipt_map(&receipt.outputs, outputs, false)
    {
        Err(MaskBuildError::new("RECEIPT", "phase receipt drifted"))
    } else {
        Ok(())
    }
}

fn authenticate_receipt_reuse(stage: &Path, receipt: &PhaseReceipt) -> Result<(), MaskBuildError> {
    authenticate_receipt_reuse_for_builder(stage, receipt, BUILDER_SOURCE_SHA256)
}

fn authenticate_receipt_reuse_for_builder(
    stage: &Path,
    receipt: &PhaseReceipt,
    expected_builder_source_sha256: &str,
) -> Result<(), MaskBuildError> {
    let Some(reused_from) = receipt.reused_from.as_deref() else {
        return Ok(());
    };
    let expected = receipt
        .inputs
        .get("reuse_authorization")
        .ok_or_else(|| MaskBuildError::new("RECEIPT", "reuse authorization is missing"))?;
    let bytes = read_bounded(&stage.join(REUSE_AUTHORIZATION_MEMBER), MAX_METADATA_BYTES)?;
    if identity(&bytes) != *expected {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "reuse authorization identity drifted",
        ));
    }
    if let Ok(authorization) = parse_canonical::<ReuseAuthorization>(&bytes) {
        return authenticate_ordinary_reuse_receipt(
            receipt,
            expected_builder_source_sha256,
            reused_from,
            &authorization,
        );
    }
    if let Ok(authorization) = parse_canonical::<CapturePromotionAuthorization>(&bytes) {
        return authenticate_promoted_capture_receipt(
            receipt,
            expected_builder_source_sha256,
            reused_from,
            &authorization,
        );
    }
    Err(MaskBuildError::new(
        "RECEIPT",
        "reuse authorization schema is invalid",
    ))
}

fn authenticate_ordinary_reuse_receipt(
    receipt: &PhaseReceipt,
    expected_builder_source_sha256: &str,
    reused_from: &str,
    authorization: &ReuseAuthorization,
) -> Result<(), MaskBuildError> {
    let valid_phase_prefix = matches!(
        authorization.sealed_phases.as_slice(),
        [Phase::Capture]
            | [Phase::Capture, Phase::Prepare]
            | [Phase::Capture, Phase::Prepare, Phase::Benchmark]
    );
    let prior_receipt = match receipt.phase {
        Phase::Capture => Some(&authorization.capture_receipt),
        Phase::Prepare => authorization.prepare_receipt.as_ref(),
        Phase::Benchmark => authorization.benchmark_receipt.as_ref(),
    };
    if authorization.schema != "pangopup-mask-reuse-authorization-v1"
        || authorization.decision != "RUN-READY-REUSE"
        || authorization.contract_id != receipt.contract_id
        || authorization.builder_source_sha256 != expected_builder_source_sha256
        || authorization.coordinator.trim().is_empty()
        || authorization.reviewer.trim().is_empty()
        || authorization.coordinator == authorization.reviewer
        || !valid_phase_prefix
        || !authorization.sealed_phases.contains(&receipt.phase)
        || prior_receipt.map(|identity| identity.sha256.as_str()) != Some(reused_from)
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "reuse authorization contract drifted",
        ));
    }
    Ok(())
}

fn authenticate_promoted_capture_receipt(
    receipt: &PhaseReceipt,
    expected_builder_source_sha256: &str,
    reused_from: &str,
    authorization: &CapturePromotionAuthorization,
) -> Result<(), MaskBuildError> {
    let identities_are_valid = [
        &authorization.source_contract,
        &authorization.target_contract,
        &authorization.capture_receipt,
        &authorization.failure_receipt,
    ]
    .into_iter()
    .all(|value| value.bytes != 0 && valid_digest(&value.sha256));
    if authorization.schema != CAPTURE_PROMOTION_AUTHORIZATION_SCHEMA
        || authorization.decision != "RUN-READY-CAPTURE-PROMOTION"
        || !identities_are_valid
        || !valid_digest(&authorization.source_builder_source_sha256)
        || authorization.source_builder_source_sha256 == expected_builder_source_sha256
        || authorization.target_builder_source_sha256 != expected_builder_source_sha256
        || authorization.target_contract.sha256 != receipt.contract_id
        || receipt.inputs.get("contract") != Some(&authorization.target_contract)
        || authorization.coordinator.trim().is_empty()
        || authorization.reviewer.trim().is_empty()
        || authorization.coordinator == authorization.reviewer
        || authorization.sealed_phases != [Phase::Capture]
        || receipt.phase != Phase::Capture
        || authorization.capture_receipt.sha256 != reused_from
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "capture promotion authorization contract drifted",
        ));
    }
    Ok(())
}

fn exact_receipt_map(
    observed: &BTreeMap<String, Identity>,
    required: &[&str],
    include_reuse_authorization: bool,
) -> bool {
    let expected_len = required.len() + usize::from(include_reuse_authorization);
    observed.len() == expected_len
        && required.iter().all(|key| observed.contains_key(*key))
        && (!include_reuse_authorization || observed.contains_key("reuse_authorization"))
        && observed
            .values()
            .all(|identity| identity.bytes != 0 && valid_digest(&identity.sha256))
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, MaskBuildError> {
    let mut held = open_held(path, maximum as u64)?;
    let length = usize::try_from(held.identity.size)
        .map_err(|_| MaskBuildError::new("RESOURCE", "member size is not addressable"))?;
    let mut bytes = Vec::with_capacity(length);
    held.file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancellation()?;
        let read = held.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > length {
            return Err(MaskBuildError::new("IO", "member changed while reading"));
        }
    }
    verify_held(&held)?;
    if bytes.len() != length {
        return Err(MaskBuildError::new("IO", "member changed while reading"));
    }
    Ok(bytes)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, MaskBuildError> {
    let value = serde_json::from_slice(bytes)
        .map_err(|_| MaskBuildError::new("JSON", "closed JSON schema is invalid"))?;
    if canonical(&value)? != bytes {
        return Err(MaskBuildError::new("JSON", "JSON is not canonical"));
    }
    Ok(value)
}

fn parse_gtf_facts(path: &Path) -> Result<BTreeMap<String, AnnotationFact>, MaskBuildError> {
    let mut held = open_held(path, MAX_GTF_BYTES)?;
    let result = parse_gtf_file(&mut held.file)?;
    verify_held(&held)?;
    Ok(result)
}

fn parse_gtf_file(input: &mut File) -> Result<BTreeMap<String, AnnotationFact>, MaskBuildError> {
    input.seek(SeekFrom::Start(0))?;
    let mut reader = BufReader::new(GzDecoder::new(BufReader::new(input)));
    let mut line = Vec::new();
    let mut facts: BTreeMap<String, AnnotationFact> = BTreeMap::new();
    let mut boundaries: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut boundary_count = 0_usize;
    let mut line_count = 0_u64;
    loop {
        if line_count.is_multiple_of(10_000) {
            check_cancellation()?;
        }
        line.clear();
        if read_bounded_line(&mut reader, &mut line, MAX_LINE_BYTES)? == 0 {
            break;
        }
        line_count = line_count.saturating_add(1);
        let text = std::str::from_utf8(trim_newline(&line))
            .map_err(|_| MaskBuildError::new("GTF", "GTF line is not UTF-8"))?;
        if text.starts_with('#') {
            continue;
        }
        let columns = text.split('\t').collect::<Vec<_>>();
        if columns.len() != 9 {
            return Err(MaskBuildError::new("GTF", "GTF row shape is invalid"));
        }
        if !matches!(columns[2], "gene" | "transcript" | "exon") {
            continue;
        }
        let contig = Grch38Contig::from_str(columns[0])
            .map_err(|_| MaskBuildError::new("GTF", "GTF contains an unsupported contig"))?;
        let start = columns[3]
            .parse::<u32>()
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| MaskBuildError::new("GTF", "GTF start is invalid"))?;
        let end = columns[4]
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= start)
            .ok_or_else(|| MaskBuildError::new("GTF", "GTF end is invalid"))?;
        let strand = match columns[6] {
            "+" => MaskStrand::Plus,
            "-" => MaskStrand::Minus,
            _ => return Err(MaskBuildError::new("GTF", "GTF strand is invalid")),
        };
        let attributes = parse_gtf_attributes(columns[8])?;
        let gene_id = attributes
            .get("gene_id")
            .and_then(|values| values.first())
            .ok_or_else(|| MaskBuildError::new("GTF", "GTF gene_id is missing"))?
            .clone();
        GencodeGeneId::from_str(&gene_id)
            .map_err(|_| MaskBuildError::new("GTF", "GTF gene_id is invalid"))?;
        match columns[2] {
            "gene" => {
                if facts.len() >= MAX_GENES
                    || facts
                        .insert(
                            gene_id.clone(),
                            AnnotationFact {
                                id: gene_id,
                                contig,
                                strand,
                                start,
                                end,
                                boundaries: Vec::new(),
                            },
                        )
                        .is_some()
                {
                    return Err(MaskBuildError::new(
                        "GTF",
                        "GTF contains duplicate or too many genes",
                    ));
                }
            }
            "exon"
                if attributes
                    .get("tag")
                    .is_some_and(|tags| tags.iter().any(|tag| tag == "Ensembl_canonical")) =>
            {
                let values = boundaries.entry(gene_id).or_default();
                if values.len() + 2 > MAX_BOUNDARIES_PER_GENE {
                    return Err(MaskBuildError::new(
                        "RESOURCE",
                        "one gene exceeds its boundary bound",
                    ));
                }
                values.extend([start, end]);
                boundary_count = boundary_count
                    .checked_add(2)
                    .ok_or_else(|| MaskBuildError::new("RESOURCE", "boundary count overflow"))?;
                if boundary_count > MAX_BOUNDARIES {
                    return Err(MaskBuildError::new(
                        "RESOURCE",
                        "GTF boundary count exceeds bound",
                    ));
                }
            }
            _ => {}
        }
    }
    for (index, (gene_id, mut values)) in boundaries.into_iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let fact = facts
            .get_mut(&gene_id)
            .ok_or_else(|| MaskBuildError::new("GTF", "exon references an unknown gene"))?;
        if values.len() >= 1_024 {
            check_cancellation()?;
        }
        values.sort_unstable();
        if values.len() >= 1_024 {
            check_cancellation()?;
        }
        values.dedup();
        for (boundary_index, value) in values.iter().enumerate() {
            if boundary_index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            if *value < fact.start || *value > fact.end {
                return Err(MaskBuildError::new(
                    "GTF",
                    "exon boundary falls outside its gene",
                ));
            }
        }
        fact.boundaries = values;
    }
    if facts.is_empty() {
        return Err(MaskBuildError::new("GTF", "GTF has no genes"));
    }
    Ok(facts)
}

fn parse_gtf_attributes(value: &str) -> Result<BTreeMap<String, Vec<String>>, MaskBuildError> {
    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let value = value.trim();
    let body = value
        .strip_suffix(';')
        .ok_or_else(|| MaskBuildError::new("GTF", "GTF attribute terminator is missing"))?;
    if body.trim().is_empty() {
        return Err(MaskBuildError::new("GTF", "GTF attribute list is empty"));
    }
    for (index, raw) in body.split(';').enumerate() {
        if index >= MAX_GTF_ATTRIBUTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "GTF attribute count exceeds its bound",
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(MaskBuildError::new("GTF", "GTF attribute is empty"));
        }
        let (key, value) = raw
            .split_once(' ')
            .ok_or_else(|| MaskBuildError::new("GTF", "GTF attribute is invalid"))?;
        if key.is_empty()
            || key.len() > MAX_GTF_ATTRIBUTE_KEY_BYTES
            || !key.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphabetic()
                    || (index != 0 && (byte.is_ascii_digit() || byte == b'_'))
            })
        {
            return Err(MaskBuildError::new("GTF", "GTF attribute key is invalid"));
        }
        let value = value.trim();
        if value.is_empty() || value.len() > MAX_GTF_ATTRIBUTE_VALUE_BYTES {
            return Err(MaskBuildError::new("GTF", "GTF attribute value is invalid"));
        }
        let parsed = if value.starts_with('"') || value.ends_with('"') {
            let Some(inner) = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
            else {
                return Err(MaskBuildError::new(
                    "GTF",
                    "GTF attribute quoting is invalid",
                ));
            };
            if inner.is_empty() || inner.contains('"') {
                return Err(MaskBuildError::new(
                    "GTF",
                    "GTF attribute quoting is invalid",
                ));
            }
            inner
        } else {
            if !matches!(key, "level" | "exon_number")
                || value.len() > 1 && value.starts_with('0')
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || value
                    .parse::<u32>()
                    .ok()
                    .filter(|number| *number != 0)
                    .is_none()
                || key == "level" && !matches!(value, "1" | "2" | "3")
            {
                return Err(MaskBuildError::new(
                    "GTF",
                    "GTF unquoted attribute is invalid",
                ));
            }
            value
        };
        if parsed.len() > MAX_GTF_ATTRIBUTE_VALUE_BYTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "GTF attribute value exceeds its bound",
            ));
        }
        result.entry(key.into()).or_default().push(parsed.into());
    }
    Ok(result)
}

fn observation_facts(
    genes: &[ObservedGene],
) -> Result<BTreeMap<String, AnnotationFact>, MaskBuildError> {
    let mut result = BTreeMap::new();
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let fact = AnnotationFact {
            id: gene.id.clone(),
            contig: Grch38Contig::from_str(&gene.contig)
                .map_err(|_| MaskBuildError::new("OBSERVATION", "gene contig is invalid"))?,
            strand: if gene.strand == "+" {
                MaskStrand::Plus
            } else {
                MaskStrand::Minus
            },
            start: gene.start,
            end: gene.end,
            boundaries: gene.boundaries.clone(),
        };
        if result.insert(gene.id.clone(), fact).is_some() {
            return Err(MaskBuildError::new(
                "OBSERVATION",
                "duplicate observed gene",
            ));
        }
    }
    Ok(result)
}

fn certify_gtf_facts(
    gtf: &BTreeMap<String, AnnotationFact>,
    observed: &BTreeMap<String, AnnotationFact>,
) -> Result<(), MaskBuildError> {
    let mismatch = || {
        MaskBuildError::new(
            "GTF_FACTS",
            "independent GTF facts differ from the database observation",
        )
    };
    if gtf.len() != observed.len() {
        return Err(mismatch());
    }
    for (index, ((gtf_id, gtf_fact), (observed_id, observed_fact))) in
        gtf.iter().zip(observed).enumerate()
    {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if gtf_id != observed_id
            || gtf_fact.id != observed_fact.id
            || gtf_fact.contig != observed_fact.contig
            || gtf_fact.strand != observed_fact.strand
            || gtf_fact.start != observed_fact.start
            || gtf_fact.end != observed_fact.end
            || gtf_fact.boundaries.len() != observed_fact.boundaries.len()
        {
            return Err(mismatch());
        }
        for (boundary_index, (gtf_boundary, observed_boundary)) in gtf_fact
            .boundaries
            .iter()
            .zip(&observed_fact.boundaries)
            .enumerate()
        {
            if boundary_index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            if gtf_boundary != observed_boundary {
                return Err(mismatch());
            }
        }
    }
    Ok(())
}

fn canonical_genes(observation: &Observation) -> Result<Vec<CanonicalMaskGene>, MaskBuildError> {
    let facts = observation_facts(&observation.genes)?;
    let mut ranks = BTreeMap::new();
    for code in 1..=25 {
        check_cancellation()?;
        let contig = Grch38Contig::from_code(code)
            .map_err(|_| MaskBuildError::new("RANK", "contig code is invalid"))?;
        let mut ids = Vec::new();
        for (index, fact) in facts.values().enumerate() {
            if index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            if fact.contig == contig {
                ids.push(fact.id.clone());
            }
        }
        if ids.is_empty() {
            continue;
        }
        let local = topological_ranks(contig, &ids, &observation.domains)?;
        ranks.extend(local.into_iter().map(|(id, rank)| ((code, id), rank)));
    }
    let mut result = Vec::with_capacity(facts.len());
    for (index, fact) in facts.values().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let rank = *ranks
            .get(&(fact.contig.code(), fact.id.clone()))
            .ok_or_else(|| MaskBuildError::new("RANK", "gene rank is missing"))?;
        result.push(CanonicalMaskGene::new(
            GencodeGeneId::from_str(&fact.id)
                .map_err(|_| MaskBuildError::new("RANK", "gene identity is invalid"))?,
            fact.contig,
            fact.strand,
            GenomicPosition::new(fact.start)
                .map_err(|_| MaskBuildError::new("RANK", "gene start is invalid"))?,
            GenomicPosition::new(fact.end)
                .map_err(|_| MaskBuildError::new("RANK", "gene end is invalid"))?,
            rank,
            fact.boundaries
                .iter()
                .map(|value| {
                    GenomicPosition::new(*value)
                        .map_err(|_| MaskBuildError::new("RANK", "boundary is invalid"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )?);
    }
    check_cancellation()?;
    result.sort_unstable_by_key(|gene| (gene.contig().code(), gene.query_rank()));
    check_cancellation()?;
    Ok(result)
}

fn topological_ranks(
    contig: Grch38Contig,
    ids: &[String],
    domains: &[ObservedDomain],
) -> Result<BTreeMap<String, u32>, MaskBuildError> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut indegree: BTreeMap<String, usize> = BTreeMap::new();
    for (index, id) in ids.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        edges.insert(id.clone(), BTreeSet::new());
        indegree.insert(id.clone(), 0);
    }
    let mut first_seen = BTreeMap::new();
    let mut ordinal = 0_usize;
    let contig_name = contig.to_string();
    for (domain_index, domain) in domains.iter().enumerate() {
        if domain_index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if domain.contig != contig_name {
            continue;
        }
        for ordered in [&domain.plus, &domain.minus] {
            for (id_index, id) in ordered.iter().enumerate() {
                if id_index.is_multiple_of(1_024) {
                    check_cancellation()?;
                }
                first_seen.entry(id.clone()).or_insert_with(|| {
                    let value = ordinal;
                    ordinal += 1;
                    value
                });
            }
            for (pair_index, pair) in ordered.windows(2).enumerate() {
                if pair_index.is_multiple_of(1_024) {
                    check_cancellation()?;
                }
                if !edges.contains_key(&pair[0]) || !edges.contains_key(&pair[1]) {
                    return Err(MaskBuildError::new(
                        "RANK",
                        "domain ordering references an unknown gene",
                    ));
                }
                if edges
                    .get_mut(&pair[0])
                    .ok_or_else(|| MaskBuildError::new("RANK", "rank edge is missing"))?
                    .insert(pair[1].clone())
                {
                    *indegree
                        .get_mut(&pair[1])
                        .ok_or_else(|| MaskBuildError::new("RANK", "rank node is missing"))? += 1;
                }
            }
        }
    }
    let mut sorted_ids = ids.to_vec();
    sorted_ids.sort();
    for (index, id) in sorted_ids.into_iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        first_seen.entry(id).or_insert_with(|| {
            let value = ordinal;
            ordinal += 1;
            value
        });
    }
    let mut ready = BTreeSet::new();
    for (id, degree) in &indegree {
        if *degree == 0 {
            ready.insert((first_seen[id], id.clone()));
        }
    }
    let mut result = BTreeMap::new();
    while let Some((seen, id)) = ready.pop_first() {
        if result.len().is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let _ = seen;
        let rank = u32::try_from(result.len())
            .map_err(|_| MaskBuildError::new("RANK", "rank count overflow"))?;
        result.insert(id.clone(), rank);
        for next in edges
            .get(&id)
            .ok_or_else(|| MaskBuildError::new("RANK", "rank node is missing"))?
        {
            let degree = indegree
                .get_mut(next)
                .ok_or_else(|| MaskBuildError::new("RANK", "rank degree is missing"))?;
            *degree -= 1;
            if *degree == 0 {
                ready.insert((first_seen[next], next.clone()));
            }
        }
    }
    if result.len() != ids.len() {
        return Err(MaskBuildError::new(
            "RANK",
            "upstream point-query orders do not admit one stable rank",
        ));
    }
    Ok(result)
}

#[derive(Default)]
struct OracleWork {
    event_updates: u64,
    emitted_memberships: u64,
    binary_search_steps: u64,
    returned_records: u64,
}

#[derive(Default)]
struct SweepEvent {
    add: Vec<usize>,
    remove: Vec<usize>,
}

fn sweep_domains(
    genes: &[CanonicalMaskGene],
    work: &mut OracleWork,
) -> Result<Vec<ObservedDomain>, MaskBuildError> {
    let mut result = Vec::new();
    for code in 1..=25 {
        check_cancellation()?;
        let contig = Grch38Contig::from_code(code)
            .map_err(|_| MaskBuildError::new("DOMAIN", "contig code is invalid"))?;
        let mut events: BTreeMap<u32, SweepEvent> = BTreeMap::new();
        for (index, gene) in genes.iter().enumerate() {
            if index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            if gene.contig() != contig {
                continue;
            }
            if gene.start() < gene.end() {
                let begin = gene
                    .start()
                    .get()
                    .checked_add(1)
                    .ok_or_else(|| MaskBuildError::new("DOMAIN", "domain start overflow"))?;
                events.entry(begin).or_default().add.push(index);
                if let Some(after) = gene.end().get().checked_add(1) {
                    events.entry(after).or_default().remove.push(index);
                }
            }
        }
        let positions = events.keys().copied().collect::<Vec<_>>();
        let mut plus = BTreeSet::new();
        let mut minus = BTreeSet::new();
        for (event_index, begin) in positions.iter().copied().enumerate() {
            check_cancellation()?;
            let event = events
                .get(&begin)
                .ok_or_else(|| MaskBuildError::new("DOMAIN", "sweep event is missing"))?;
            for index in &event.remove {
                let gene = genes
                    .get(*index)
                    .ok_or_else(|| MaskBuildError::new("DOMAIN", "sweep gene is missing"))?;
                let active = match gene.strand() {
                    MaskStrand::Plus => &mut plus,
                    MaskStrand::Minus => &mut minus,
                };
                if !active.remove(&(gene.query_rank(), *index)) {
                    return Err(MaskBuildError::new(
                        "DOMAIN",
                        "sweep removal was not active",
                    ));
                }
                work.event_updates = work.event_updates.saturating_add(1);
            }
            for index in &event.add {
                let gene = genes
                    .get(*index)
                    .ok_or_else(|| MaskBuildError::new("DOMAIN", "sweep gene is missing"))?;
                let active = match gene.strand() {
                    MaskStrand::Plus => &mut plus,
                    MaskStrand::Minus => &mut minus,
                };
                if !active.insert((gene.query_rank(), *index)) {
                    return Err(MaskBuildError::new("DOMAIN", "duplicate sweep activation"));
                }
                work.event_updates = work.event_updates.saturating_add(1);
            }
            let end = positions
                .get(event_index + 1)
                .map_or(u32::MAX, |value| value - 1);
            if !plus.is_empty() || !minus.is_empty() {
                let collect = |active: &BTreeSet<(u32, usize)>| {
                    let mut result = Vec::with_capacity(active.len());
                    for (index, (_, gene_index)) in active.iter().enumerate() {
                        if index.is_multiple_of(1_024) {
                            check_cancellation()?;
                        }
                        result.push(genes[*gene_index].identity().to_string());
                    }
                    Ok::<_, MaskBuildError>(result)
                };
                let plus = collect(&plus)?;
                let minus = collect(&minus)?;
                work.emitted_memberships = work
                    .emitted_memberships
                    .saturating_add((plus.len() + minus.len()) as u64);
                result.push(ObservedDomain {
                    kind: "domain".into(),
                    contig: contig.to_string(),
                    begin,
                    end,
                    plus,
                    minus,
                });
            }
        }
    }
    Ok(result)
}

fn certify_observed_domains(
    observation: &Observation,
    genes: &[CanonicalMaskGene],
) -> Result<(), MaskBuildError> {
    let mut work = OracleWork::default();
    if sweep_domains(genes, &mut work)? == observation.domains {
        Ok(())
    } else {
        Err(MaskBuildError::new(
            "DOMAIN",
            "ordered upstream domains are incomplete or inconsistent",
        ))
    }
}

struct DomainOracle<'a> {
    domains: &'a [ObservedDomain],
    ranges: [Range<usize>; MAX_CONTIGS],
    genes: BTreeMap<String, (MaskStrand, QueryGeneValue)>,
}

impl<'a> DomainOracle<'a> {
    fn new(
        observation: &'a Observation,
        genes: &[CanonicalMaskGene],
    ) -> Result<Self, MaskBuildError> {
        let mut ranges: [Range<usize>; MAX_CONTIGS] = std::array::from_fn(|_| 0..0);
        for (index, domain) in observation.domains.iter().enumerate() {
            if index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            let code = Grch38Contig::from_str(&domain.contig)
                .map_err(|_| MaskBuildError::new("DOMAIN", "domain contig is invalid"))?
                .code() as usize;
            let slot = ranges
                .get_mut(code - 1)
                .ok_or_else(|| MaskBuildError::new("DOMAIN", "domain contig is invalid"))?;
            if Range::is_empty(slot) {
                *slot = index..index + 1;
            } else if slot.end == index {
                slot.end += 1;
            } else {
                return Err(MaskBuildError::new(
                    "DOMAIN",
                    "domains for one contig are not contiguous",
                ));
            }
        }
        let mut facts = BTreeMap::new();
        for (index, gene) in genes.iter().enumerate() {
            if index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            if facts
                .insert(
                    gene.identity().to_string(),
                    (
                        gene.strand(),
                        QueryGeneValue {
                            id: gene.identity().to_string(),
                            boundaries: gene.boundaries().iter().map(|value| value.get()).collect(),
                        },
                    ),
                )
                .is_some()
            {
                return Err(MaskBuildError::new(
                    "DOMAIN",
                    "duplicate exact gene identity in oracle",
                ));
            }
        }
        for domain in &observation.domains {
            for (strand, ids) in [
                (MaskStrand::Plus, &domain.plus),
                (MaskStrand::Minus, &domain.minus),
            ] {
                for id in ids {
                    if facts.get(id).map(|(actual, _)| *actual) != Some(strand) {
                        return Err(MaskBuildError::new(
                            "DOMAIN",
                            "domain identity or strand is inconsistent",
                        ));
                    }
                }
            }
        }
        Ok(Self {
            domains: &observation.domains,
            ranges,
            genes: facts,
        })
    }

    fn query(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
    ) -> Result<QueryValue, MaskBuildError> {
        self.query_counted(contig, position, &mut OracleWork::default())
    }

    fn query_counted(
        &self,
        contig: Grch38Contig,
        position: GenomicPosition,
        work: &mut OracleWork,
    ) -> Result<QueryValue, MaskBuildError> {
        let range = self
            .ranges
            .get(contig.code() as usize - 1)
            .ok_or_else(|| MaskBuildError::new("DOMAIN", "oracle contig is invalid"))?
            .clone();
        let domains = &self.domains[range];
        let mut low = 0_usize;
        let mut high = domains.len();
        while low < high {
            work.binary_search_steps = work.binary_search_steps.saturating_add(1);
            let middle = low + (high - low) / 2;
            if domains[middle].begin <= position.get() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let Some(domain) = low.checked_sub(1).and_then(|index| domains.get(index)) else {
            return Ok(QueryValue::default());
        };
        if position.get() > domain.end {
            return Ok(QueryValue::default());
        }
        let resolve = |ids: &[String], expected_strand: MaskStrand| {
            let mut result = Vec::with_capacity(ids.len());
            for (index, id) in ids.iter().enumerate() {
                if index.is_multiple_of(1_024) {
                    check_cancellation()?;
                }
                let (strand, value) = self.genes.get(id).ok_or_else(|| {
                    MaskBuildError::new("DOMAIN", "oracle gene identity is missing")
                })?;
                if *strand != expected_strand {
                    return Err(MaskBuildError::new(
                        "DOMAIN",
                        "oracle gene strand is inconsistent",
                    ));
                }
                result.push(value.clone());
            }
            Ok::<_, MaskBuildError>(result)
        };
        let plus = resolve(&domain.plus, MaskStrand::Plus)?;
        let minus = resolve(&domain.minus, MaskStrand::Minus)?;
        work.returned_records = work
            .returned_records
            .saturating_add((plus.len() + minus.len()) as u64);
        Ok(QueryValue { plus, minus })
    }
}

fn certify_candidate(
    reader: &MaskCandidateReader,
    observation: &Observation,
    genes: &[CanonicalMaskGene],
    oracle: &DomainOracle<'_>,
) -> Result<(), MaskBuildError> {
    if reader.inspect_genes_with_cancellation(&|| check_cancellation().is_err())? != genes {
        return Err(MaskBuildError::new(
            "CANDIDATE",
            "candidate logical stream differs from the canonical export",
        ));
    }
    let mut points = BTreeSet::new();
    for (index, domain) in observation.domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        points.insert((
            Grch38Contig::from_str(&domain.contig)
                .map_err(|_| MaskBuildError::new("CANDIDATE", "domain contig is invalid"))?,
            domain.begin,
        ));
    }
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        for value in [
            Some(gene.start().get()),
            gene.start().get().checked_add(1),
            Some(gene.end().get()),
            gene.end().get().checked_add(1),
        ]
        .into_iter()
        .flatten()
        {
            points.insert((gene.contig(), value));
        }
    }
    let mut output = MaskQueryBuffer::with_capacity(64, 4096);
    for (index, (contig, value)) in points.into_iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let position = GenomicPosition::new(value)
            .map_err(|_| MaskBuildError::new("CANDIDATE", "query position is invalid"))?;
        reader.query(contig, position, &mut output)?;
        let expected = oracle.query(contig, position)?;
        let actual = query_value(&output)?;
        if actual != expected {
            return Err(MaskBuildError::new(
                "CANDIDATE",
                "candidate differs from the independent domain oracle",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct QueryValue {
    plus: Vec<QueryGeneValue>,
    minus: Vec<QueryGeneValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct QueryGeneValue {
    id: String,
    boundaries: Vec<u32>,
}

fn query_value(output: &MaskQueryBuffer) -> Result<QueryValue, MaskBuildError> {
    let strand = |values: &[pangopup_index::mask_candidates::MaskQueryGene]| {
        let mut result = Vec::with_capacity(values.len());
        for (index, gene) in values.iter().enumerate() {
            if index.is_multiple_of(1_024) {
                check_cancellation()?;
            }
            let mut boundaries = Vec::with_capacity(output.boundaries(gene).len());
            for (boundary_index, value) in output.boundaries(gene).iter().enumerate() {
                if boundary_index.is_multiple_of(1_024) {
                    check_cancellation()?;
                }
                boundaries.push(value.get());
            }
            result.push(QueryGeneValue {
                id: gene.identity().to_string(),
                boundaries,
            });
        }
        Ok::<_, MaskBuildError>(result)
    };
    Ok(QueryValue {
        plus: strand(output.plus())?,
        minus: strand(output.minus())?,
    })
}

fn inventory(
    genes: &[CanonicalMaskGene],
    domains: &[ObservedDomain],
    canonical_stream: Identity,
) -> Result<Inventory, MaskBuildError> {
    let mut boundaries = 0_u64;
    let mut plus_genes = 0_u64;
    let mut minus_genes = 0_u64;
    let mut contigs = BTreeSet::new();
    let mut maximum_boundaries_per_gene = 0_u64;
    let mut empty_boundary_genes = 0_u64;
    let mut par_y_genes = 0_u64;
    let mut stable: BTreeMap<String, u64> = BTreeMap::new();
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        boundaries = boundaries
            .checked_add(gene.boundaries().len() as u64)
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "boundary count overflow"))?;
        match gene.strand() {
            MaskStrand::Plus => plus_genes += 1,
            MaskStrand::Minus => minus_genes += 1,
        }
        contigs.insert(gene.contig());
        maximum_boundaries_per_gene =
            maximum_boundaries_per_gene.max(gene.boundaries().len() as u64);
        empty_boundary_genes += u64::from(gene.boundaries().is_empty());
        par_y_genes += u64::from(gene.identity().is_par_y());
        *stable
            .entry(gene.stable_identity().to_string())
            .or_default() += 1;
    }
    let mut same_strand_multi_domains = 0_u64;
    let mut opposite_strand_multi_domains = 0_u64;
    for (index, domain) in domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        same_strand_multi_domains += u64::from(domain.plus.len() > 1 || domain.minus.len() > 1);
        opposite_strand_multi_domains +=
            u64::from(!domain.plus.is_empty() && !domain.minus.is_empty());
    }
    Ok(Inventory {
        schema: INVENTORY_SCHEMA.into(),
        profile: MASK_PROFILE.into(),
        builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
        genes: genes.len() as u64,
        plus_genes,
        minus_genes,
        primary_contigs: contigs.len() as u64,
        boundaries,
        maximum_boundaries_per_gene,
        empty_boundary_genes,
        versioned_genes: genes.len() as u64,
        par_y_genes,
        distinct_stable_ids: stable.len() as u64,
        stable_collisions: stable.values().filter(|count| **count > 1).count() as u64,
        duplicate_exact_ids: 0,
        domains: domains.len() as u64,
        same_strand_multi_domains,
        opposite_strand_multi_domains,
        canonical_stream,
    })
}

fn build_performance_manifest(
    observation: &Observation,
    genes: &[CanonicalMaskGene],
    oracle: &DomainOracle<'_>,
    compatibility_points: &[CompatibilityPoint],
) -> Result<PerformanceManifest, MaskBuildError> {
    check_cancellation()?;
    let mut points: Vec<(String, Grch38Contig, u32)> = Vec::with_capacity(1_000);
    let mut eligible = BTreeMap::new();
    let single_eligible = observation
        .domains
        .iter()
        .filter(|domain| domain.plus.len() + domain.minus.len() == 1)
        .count();
    append_single_gene_queries(&mut points, &observation.domains)?;
    eligible.insert("single-gene", single_eligible);
    let no_gene = no_gene_points(&observation.domains)?;
    append_cycled(&mut points, "no-gene", &no_gene, 100)?;
    eligible.insert("no-gene", no_gene.len());
    let same = domain_points(&observation.domains, |domain| {
        domain.plus.len() > 1 || domain.minus.len() > 1
    })?;
    append_cycled(&mut points, "same-strand-multi", &same, 100)?;
    eligible.insert("same-strand-multi", same.len());
    let opposite = domain_points(&observation.domains, |domain| {
        !domain.plus.is_empty() && !domain.minus.is_empty()
    })?;
    append_cycled(&mut points, "opposite-strand-multi", &opposite, 100)?;
    eligible.insert("opposite-strand-multi", opposite.len());
    for (name, selector) in [
        ("boundary-start", 0_u8),
        ("boundary-start-plus-one", 1),
        ("boundary-end", 2),
        ("boundary-end-plus-one", 3),
    ] {
        let boundary = boundary_points(genes, selector)?;
        append_cycled(&mut points, name, &boundary, 25)?;
        eligible.insert(name, boundary.len());
    }
    let par = par_points(genes)?;
    append_cycled(&mut points, "par-pair", &par, 88)?;
    eligible.insert("par-pair", par.len());
    for (index, point) in compatibility_points.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        points.push((
            "compatibility".into(),
            Grch38Contig::from_str(&point.contig).map_err(|_| {
                MaskBuildError::new("PERFORMANCE_MANIFEST", "compatibility contig is invalid")
            })?,
            point.position,
        ));
    }
    eligible.insert("compatibility", compatibility_points.len());
    let mut extreme = observation.domains.iter().collect::<Vec<_>>();
    check_cancellation()?;
    extreme.sort_by_key(|domain| {
        (
            std::cmp::Reverse(domain.plus.len() + domain.minus.len()),
            Grch38Contig::from_str(&domain.contig).map_or(255, |contig| contig.code()),
            domain.begin,
            domain.plus.join(","),
            domain.minus.join(","),
        )
    });
    check_cancellation()?;
    let extreme_eligible = extreme.len();
    let extreme = extreme
        .into_iter()
        .take(12)
        .map(|domain| {
            Ok((
                Grch38Contig::from_str(&domain.contig).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "domain contig is invalid")
                })?,
                domain.begin,
            ))
        })
        .collect::<Result<Vec<_>, MaskBuildError>>()?;
    append_cycled(&mut points, "extreme-cardinality", &extreme, 12)?;
    eligible.insert("extreme-cardinality", extreme_eligible);
    if points.len() != 1_000 {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "performance stratum counts are invalid",
        ));
    }
    let strata = performance_strata(&points, &eligible)?;
    let mut queries = Vec::with_capacity(points.len());
    for (ordinal, (stratum, contig, value)) in points.into_iter().enumerate() {
        if ordinal.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let position = GenomicPosition::new(value).map_err(|_| {
            MaskBuildError::new("PERFORMANCE_MANIFEST", "query position is invalid")
        })?;
        let expected = canonical(&oracle.query(contig, position)?)?;
        queries.push(PerformanceQuery {
            ordinal: u16::try_from(ordinal).map_err(|_| {
                MaskBuildError::new("PERFORMANCE_MANIFEST", "query ordinal overflow")
            })?,
            stratum,
            contig: contig.to_string(),
            position: value,
            expected_sha256: identity(&expected).sha256,
        });
    }
    Ok(PerformanceManifest {
        schema: PERFORMANCE_SCHEMA.into(),
        profile: MASK_PROFILE.into(),
        strata,
        queries,
    })
}

fn performance_strata(
    points: &[(String, Grch38Contig, u32)],
    eligible: &BTreeMap<&str, usize>,
) -> Result<Vec<PerformanceStratum>, MaskBuildError> {
    const ORDER: [&str; 11] = [
        "single-gene",
        "no-gene",
        "same-strand-multi",
        "opposite-strand-multi",
        "boundary-start",
        "boundary-start-plus-one",
        "boundary-end",
        "boundary-end-plus-one",
        "par-pair",
        "compatibility",
        "extreme-cardinality",
    ];
    ORDER
        .into_iter()
        .map(|name| {
            let selected = points
                .iter()
                .filter(|(stratum, _, _)| stratum == name)
                .map(|(_, contig, position)| (contig.code(), *position))
                .collect::<Vec<_>>();
            let distinct = selected.iter().copied().collect::<BTreeSet<_>>().len();
            let eligible = *eligible.get(name).ok_or_else(|| {
                MaskBuildError::new("PERFORMANCE_MANIFEST", "stratum evidence is missing")
            })?;
            if eligible == 0 || distinct == 0 || distinct > selected.len() || distinct > eligible {
                return Err(MaskBuildError::new(
                    "PERFORMANCE_MANIFEST",
                    "stratum repetition evidence is invalid",
                ));
            }
            Ok(PerformanceStratum {
                name: name.into(),
                requested: u16::try_from(selected.len()).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "stratum count overflow")
                })?,
                eligible: eligible as u64,
                distinct: distinct as u16,
                repeated: (selected.len() - distinct) as u16,
            })
        })
        .collect()
}

fn append_single_gene_queries(
    output: &mut Vec<(String, Grch38Contig, u32)>,
    domains: &[ObservedDomain],
) -> Result<(), MaskBuildError> {
    let mut by_contig: BTreeMap<u8, Vec<&ObservedDomain>> = BTreeMap::new();
    for (index, domain) in domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if domain.plus.len() + domain.minus.len() == 1 {
            let contig = Grch38Contig::from_str(&domain.contig).map_err(|_| {
                MaskBuildError::new("PERFORMANCE_MANIFEST", "domain contig is invalid")
            })?;
            by_contig.entry(contig.code()).or_default().push(domain);
        }
    }
    let total = by_contig.values().map(Vec::len).sum::<usize>();
    if total == 0 {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "single-gene stratum is empty",
        ));
    }
    let mut allocations = Vec::new();
    let mut assigned = 0_usize;
    for (code, values) in &by_contig {
        let numerator = 486_usize
            .checked_mul(values.len())
            .ok_or_else(|| MaskBuildError::new("RESOURCE", "allocation overflow"))?;
        let slots = numerator / total;
        assigned += slots;
        allocations.push((*code, slots, numerator % total));
    }
    allocations.sort_by_key(|(code, _, remainder)| (std::cmp::Reverse(*remainder), *code));
    for allocation in allocations
        .iter_mut()
        .take(486_usize.saturating_sub(assigned))
    {
        allocation.1 += 1;
    }
    allocations.sort_by_key(|(code, _, _)| *code);
    for (code, slots, _) in allocations {
        let values = &by_contig[&code];
        for slot in 0..slots {
            let numerator = (2 * slot + 1)
                .checked_mul(values.len())
                .ok_or_else(|| MaskBuildError::new("RESOURCE", "sampling overflow"))?;
            let index = numerator / (2 * slots);
            let domain = values[index];
            output.push((
                "single-gene".into(),
                Grch38Contig::from_code(code).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "contig code is invalid")
                })?,
                domain.begin + (domain.end - domain.begin) / 2,
            ));
        }
    }
    Ok(())
}

fn domain_points(
    domains: &[ObservedDomain],
    predicate: impl Fn(&ObservedDomain) -> bool,
) -> Result<Vec<(Grch38Contig, u32)>, MaskBuildError> {
    let mut result = Vec::new();
    for (index, domain) in domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if predicate(domain) {
            result.push((
                Grch38Contig::from_str(&domain.contig).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "domain contig is invalid")
                })?,
                domain.begin,
            ));
        }
    }
    Ok(result)
}

fn no_gene_points(domains: &[ObservedDomain]) -> Result<Vec<(Grch38Contig, u32)>, MaskBuildError> {
    let mut result = Vec::new();
    let mut by_contig: BTreeMap<u8, Vec<&ObservedDomain>> = BTreeMap::new();
    for (index, domain) in domains.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let contig = Grch38Contig::from_str(&domain.contig)
            .map_err(|_| MaskBuildError::new("PERFORMANCE_MANIFEST", "domain contig is invalid"))?;
        by_contig.entry(contig.code()).or_default().push(domain);
    }
    for (code, values) in by_contig {
        let contig = Grch38Contig::from_code(code)
            .map_err(|_| MaskBuildError::new("PERFORMANCE_MANIFEST", "contig code is invalid"))?;
        if values[0].begin > 1 {
            result.push((contig, 1));
        }
        for pair in values.windows(2) {
            if let Some(begin) = pair[0].end.checked_add(1)
                && begin < pair[1].begin
            {
                result.push((contig, begin));
            }
        }
        if let Some(after) = values.last().and_then(|domain| domain.end.checked_add(1)) {
            result.push((contig, after));
        }
    }
    Ok(result)
}

fn boundary_points(
    genes: &[CanonicalMaskGene],
    selector: u8,
) -> Result<Vec<(Grch38Contig, u32)>, MaskBuildError> {
    let mut result = BTreeSet::new();
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        let position = match selector {
            0 => Some(gene.start().get()),
            1 => gene.start().get().checked_add(1),
            2 => Some(gene.end().get()),
            _ => gene.end().get().checked_add(1),
        };
        if let Some(position) = position {
            result.insert((gene.contig(), position));
        }
    }
    Ok(result.into_iter().collect())
}

fn par_points(genes: &[CanonicalMaskGene]) -> Result<Vec<(Grch38Contig, u32)>, MaskBuildError> {
    let mut stable: BTreeMap<String, Vec<&CanonicalMaskGene>> = BTreeMap::new();
    for (index, gene) in genes.iter().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        stable
            .entry(gene.stable_identity().to_string())
            .or_default()
            .push(gene);
    }
    let mut result = Vec::new();
    for (index, values) in stable.values().enumerate() {
        if index.is_multiple_of(1_024) {
            check_cancellation()?;
        }
        if values.iter().any(|gene| gene.contig() == Grch38Contig::X)
            && values.iter().any(|gene| gene.contig() == Grch38Contig::Y)
        {
            for gene in values
                .iter()
                .filter(|gene| matches!(gene.contig(), Grch38Contig::X | Grch38Contig::Y))
            {
                result.push((
                    gene.contig(),
                    gene.start().get().checked_add(1).ok_or_else(|| {
                        MaskBuildError::new("PERFORMANCE_MANIFEST", "PAR position overflow")
                    })?,
                ));
            }
        }
    }
    result.sort_unstable();
    Ok(result)
}

fn append_cycled(
    output: &mut Vec<(String, Grch38Contig, u32)>,
    name: &str,
    values: &[(Grch38Contig, u32)],
    count: usize,
) -> Result<(), MaskBuildError> {
    if values.is_empty() {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            format!("{name} stratum is empty"),
        ));
    }
    for index in 0..count {
        let (contig, position) = values[index % values.len()];
        output.push((name.into(), contig, position));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceHeader {
    schema: String,
    profile: String,
    queries: u16,
    strata: Vec<PerformanceStratum>,
}

fn encode_performance(manifest: &PerformanceManifest) -> Result<Vec<u8>, MaskBuildError> {
    let mut bytes = canonical(&PerformanceHeader {
        schema: manifest.schema.clone(),
        profile: manifest.profile.clone(),
        queries: u16::try_from(manifest.queries.len())
            .map_err(|_| MaskBuildError::new("RESOURCE", "query count overflow"))?,
        strata: manifest.strata.clone(),
    })?;
    for query in &manifest.queries {
        bytes.extend(canonical(query)?);
        if bytes.len() > MAX_PERFORMANCE_BYTES {
            return Err(MaskBuildError::new(
                "RESOURCE",
                "performance manifest exceeds bound",
            ));
        }
    }
    Ok(bytes)
}

pub fn read_performance(path: &Path) -> Result<PerformanceManifest, MaskBuildError> {
    let bytes = read_bounded(path, MAX_PERFORMANCE_BYTES)?;
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    let header: PerformanceHeader = parse_canonical(
        lines
            .next()
            .ok_or_else(|| MaskBuildError::new("PERFORMANCE_MANIFEST", "header is missing"))?,
    )?;
    if header.schema != PERFORMANCE_SCHEMA || header.profile != MASK_PROFILE {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "performance header drifted",
        ));
    }
    let mut queries = Vec::new();
    for line in lines {
        let query: PerformanceQuery = parse_canonical(line)?;
        if query.ordinal as usize != queries.len()
            || query.position == 0
            || !valid_digest(&query.expected_sha256)
        {
            return Err(MaskBuildError::new(
                "PERFORMANCE_MANIFEST",
                "performance query is invalid",
            ));
        }
        Grch38Contig::from_str(&query.contig).map_err(|_| {
            MaskBuildError::new("PERFORMANCE_MANIFEST", "performance contig is invalid")
        })?;
        queries.push(query);
    }
    if queries.len() != header.queries as usize || queries.len() != 1_000 {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "performance query count drifted",
        ));
    }
    let observed = queries
        .iter()
        .map(|query| {
            Ok((
                query.stratum.clone(),
                Grch38Contig::from_str(&query.contig).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "performance contig is invalid")
                })?,
                query.position,
            ))
        })
        .collect::<Result<Vec<_>, MaskBuildError>>()?;
    let eligible = header
        .strata
        .iter()
        .map(|stratum| {
            usize::try_from(stratum.eligible)
                .map(|count| (stratum.name.as_str(), count))
                .map_err(|_| MaskBuildError::new("PERFORMANCE_MANIFEST", "eligible count overflow"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if performance_strata(&observed, &eligible)? != header.strata {
        return Err(MaskBuildError::new(
            "PERFORMANCE_MANIFEST",
            "stratum repetition evidence drifted",
        ));
    }
    Ok(PerformanceManifest {
        schema: header.schema,
        profile: header.profile,
        strata: header.strata,
        queries,
    })
}

pub const BENCHMARK_PERMUTATIONS: [[MaskCandidateCodec; 3]; 6] = [
    [
        MaskCandidateCodec::IntervalTree,
        MaskCandidateCodec::Domains,
        MaskCandidateCodec::BinnedPostings,
    ],
    [
        MaskCandidateCodec::IntervalTree,
        MaskCandidateCodec::BinnedPostings,
        MaskCandidateCodec::Domains,
    ],
    [
        MaskCandidateCodec::Domains,
        MaskCandidateCodec::IntervalTree,
        MaskCandidateCodec::BinnedPostings,
    ],
    [
        MaskCandidateCodec::Domains,
        MaskCandidateCodec::BinnedPostings,
        MaskCandidateCodec::IntervalTree,
    ],
    [
        MaskCandidateCodec::BinnedPostings,
        MaskCandidateCodec::IntervalTree,
        MaskCandidateCodec::Domains,
    ],
    [
        MaskCandidateCodec::BinnedPostings,
        MaskCandidateCodec::Domains,
        MaskCandidateCodec::IntervalTree,
    ],
];

/// Fixed pre-measurement tie order: a domain table has the shortest lookup
/// proof, an interval tree is next, and duplicated binned postings carry the
/// greatest canonicality/checking burden.
pub const MASK_SIMPLICITY_ORDER: [MaskCandidateCodec; 3] = [
    MaskCandidateCodec::Domains,
    MaskCandidateCodec::IntervalTree,
    MaskCandidateCodec::BinnedPostings,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkMethod {
    pub manifest_queries: u64,
    pub rounds: u64,
    pub warmups_per_candidate_round: u64,
    pub timed_queries_per_candidate_round: u64,
    pub quantile: String,
    pub headline: String,
    pub logical_page_bytes: u64,
    pub schedule: Vec<Vec<MaskCandidateCodec>>,
    pub simplicity_order: Vec<MaskCandidateCodec>,
}

impl BenchmarkMethod {
    pub fn ticket_012() -> Self {
        Self {
            manifest_queries: 1_000,
            rounds: 6,
            warmups_per_candidate_round: 10_000,
            timed_queries_per_candidate_round: 100_000,
            quantile: "nearest-rank".into(),
            headline: "nearest-rank-p50-of-six-round-quantiles-index-2".into(),
            logical_page_bytes: 4_096,
            schedule: BENCHMARK_PERMUTATIONS
                .iter()
                .map(|round| round.to_vec())
                .collect(),
            simplicity_order: MASK_SIMPLICITY_ORDER.to_vec(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RoundMeasurement {
    pub round: u8,
    pub schedule_position: u8,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub open_ns: u64,
    pub open_peak_heap_bytes: u64,
    pub warmed_allocation_calls: u64,
    pub warmed_allocation_bytes: u64,
    pub maximum_rss_bytes: u64,
    pub minor_faults: u64,
    pub major_faults: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateMeasurement {
    pub codec: MaskCandidateCodec,
    pub member: Identity,
    pub pinned_zstandard_bytes: u64,
    pub pinned_zstandard: String,
    pub semantic_certified: bool,
    pub corruption_controls_passed: bool,
    pub allocation_contract_passed: bool,
    pub page_trace_sha256: String,
    pub metadata_pages: u64,
    pub median_payload_pages: u64,
    pub p95_payload_pages: u64,
    pub headline_p50_ns: u64,
    pub headline_p95_ns: u64,
    pub rounds: Vec<RoundMeasurement>,
}

impl CandidateMeasurement {
    fn maximum_open_peak_heap(&self) -> u64 {
        self.rounds
            .iter()
            .map(|round| round.open_peak_heap_bytes)
            .max()
            .unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkHost {
    pub selected_cpu: u32,
    pub allowed_cpu_count_before_pin: u32,
    pub cpu_model: String,
    pub kernel: String,
    pub governor: String,
    pub power_state: String,
    pub rustc: String,
    pub target: String,
    pub build_profile: String,
    pub executable: Identity,
    pub logical_page_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkResources {
    pub maximum_rss_bytes: u64,
    pub minor_faults: u64,
    pub major_faults: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionStep {
    pub criterion: String,
    pub minimum: u64,
    pub survivors: Vec<MaskCandidateCodec>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionDecision {
    pub selected: MaskCandidateCodec,
    pub steps: Vec<SelectionStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaskBenchmarkReport {
    pub schema: String,
    pub profile: String,
    pub contract_id: String,
    pub builder_source_sha256: String,
    pub performance_manifest: Identity,
    pub method: BenchmarkMethod,
    pub host: BenchmarkHost,
    pub resources: BenchmarkResources,
    pub candidates: Vec<CandidateMeasurement>,
    pub selection: SelectionDecision,
}

fn headline(rounds: &[RoundMeasurement], metric: impl Fn(&RoundMeasurement) -> u64) -> u64 {
    let mut values = rounds.iter().map(metric).collect::<Vec<_>>();
    values.sort_unstable();
    values[2]
}

fn candidate_for(
    candidates: &[CandidateMeasurement],
    codec: MaskCandidateCodec,
) -> &CandidateMeasurement {
    candidates
        .iter()
        .find(|candidate| candidate.codec == codec)
        .expect("validated candidate set")
}

fn survivor_codes(survivors: &[&CandidateMeasurement]) -> Vec<MaskCandidateCodec> {
    MaskCandidateCodec::ALL
        .into_iter()
        .filter(|codec| survivors.iter().any(|candidate| candidate.codec == *codec))
        .collect()
}

fn retain_minimum(
    survivors: &mut Vec<&CandidateMeasurement>,
    metric: impl Fn(&CandidateMeasurement) -> u64,
) -> u64 {
    let minimum = survivors
        .iter()
        .map(|candidate| metric(candidate))
        .min()
        .expect("validated nonempty candidate set");
    survivors.retain(|candidate| metric(candidate) == minimum);
    minimum
}

/// Apply Ticket 012's ordered, non-pairwise selector. Missing or internally
/// inconsistent evidence fails closed and produces no selected format.
pub fn evaluate_mask_candidates(
    candidates: &[CandidateMeasurement],
) -> Result<SelectionDecision, MaskBuildError> {
    if candidates.len() != MaskCandidateCodec::ALL.len()
        || MaskCandidateCodec::ALL.iter().any(|codec| {
            candidates
                .iter()
                .filter(|candidate| candidate.codec == *codec)
                .count()
                != 1
        })
    {
        return Err(MaskBuildError::new(
            "BENCHMARK_EVIDENCE",
            "candidate evidence set is incomplete",
        ));
    }
    for candidate in candidates {
        if !candidate.semantic_certified
            || !candidate.corruption_controls_passed
            || !candidate.allocation_contract_passed
            || candidate.rounds.len() != 6
            || candidate.member.bytes == 0
            || candidate.pinned_zstandard_bytes == 0
            || candidate.pinned_zstandard != PINNED_MASK_ZSTANDARD
            || candidate.metadata_pages == 0
            || candidate.median_payload_pages == 0
            || candidate.p95_payload_pages == 0
            || candidate.headline_p50_ns == 0
            || candidate.headline_p95_ns == 0
            || !valid_digest(&candidate.member.sha256)
            || !valid_digest(&candidate.page_trace_sha256)
            || candidate.rounds.iter().any(|round| {
                round.p50_ns == 0
                    || round.p95_ns == 0
                    || round.open_ns == 0
                    || round.maximum_rss_bytes == 0
                    || round.p50_ns > round.p95_ns
            })
        {
            return Err(MaskBuildError::new(
                "BENCHMARK_EVIDENCE",
                "candidate evidence is missing or invalid",
            ));
        }
        for (round_index, permutation) in BENCHMARK_PERMUTATIONS.iter().enumerate() {
            let expected_position = permutation
                .iter()
                .position(|codec| *codec == candidate.codec)
                .expect("fixed permutation is complete") as u8;
            let round = &candidate.rounds[round_index];
            if round.round as usize != round_index
                || round.schedule_position != expected_position
                || round.warmed_allocation_calls != 0
                || round.warmed_allocation_bytes != 0
            {
                return Err(MaskBuildError::new(
                    "BENCHMARK_EVIDENCE",
                    "round schedule or allocation evidence drifted",
                ));
            }
        }
        if candidate.headline_p50_ns != headline(&candidate.rounds, |round| round.p50_ns)
            || candidate.headline_p95_ns != headline(&candidate.rounds, |round| round.p95_ns)
        {
            return Err(MaskBuildError::new(
                "BENCHMARK_EVIDENCE",
                "candidate headline quantiles are inconsistent",
            ));
        }
    }

    let mut survivors = MaskCandidateCodec::ALL
        .iter()
        .map(|codec| candidate_for(candidates, *codec))
        .collect::<Vec<_>>();
    let mut steps = Vec::new();
    let minimum_p95 = survivors
        .iter()
        .map(|candidate| candidate.headline_p95_ns)
        .min()
        .expect("candidate set is nonempty");
    survivors.retain(|candidate| {
        (candidate.headline_p95_ns as u128) * 100 <= (minimum_p95 as u128) * 105
    });
    steps.push(SelectionStep {
        criterion: "headline-p95-within-five-percent".into(),
        minimum: minimum_p95,
        survivors: survivor_codes(&survivors),
    });

    let minimum_p50 = survivors
        .iter()
        .map(|candidate| candidate.headline_p50_ns)
        .min()
        .expect("p95 survivors are nonempty");
    survivors.retain(|candidate| {
        (candidate.headline_p50_ns as u128) * 100 <= (minimum_p50 as u128) * 105
    });
    steps.push(SelectionStep {
        criterion: "headline-p50-within-five-percent".into(),
        minimum: minimum_p50,
        survivors: survivor_codes(&survivors),
    });

    for (criterion, metric) in [
        (
            "median-logical-payload-pages",
            (|candidate: &CandidateMeasurement| candidate.median_payload_pages)
                as fn(&CandidateMeasurement) -> u64,
        ),
        (
            "p95-logical-payload-pages",
            (|candidate: &CandidateMeasurement| candidate.p95_payload_pages)
                as fn(&CandidateMeasurement) -> u64,
        ),
        (
            "maximum-open-peak-heap",
            (|candidate: &CandidateMeasurement| candidate.maximum_open_peak_heap())
                as fn(&CandidateMeasurement) -> u64,
        ),
        (
            "member-bytes",
            (|candidate: &CandidateMeasurement| candidate.member.bytes)
                as fn(&CandidateMeasurement) -> u64,
        ),
        (
            "pinned-zstandard-bytes",
            (|candidate: &CandidateMeasurement| candidate.pinned_zstandard_bytes)
                as fn(&CandidateMeasurement) -> u64,
        ),
    ] {
        let minimum = retain_minimum(&mut survivors, metric);
        steps.push(SelectionStep {
            criterion: criterion.into(),
            minimum,
            survivors: survivor_codes(&survivors),
        });
    }
    let selected = MASK_SIMPLICITY_ORDER
        .into_iter()
        .find(|codec| survivors.iter().any(|candidate| candidate.codec == *codec))
        .ok_or_else(|| MaskBuildError::new("BENCHMARK_EVIDENCE", "selection is empty"))?;
    steps.push(SelectionStep {
        criterion: "fixed-simplicity-order".into(),
        minimum: MASK_SIMPLICITY_ORDER
            .iter()
            .position(|codec| *codec == selected)
            .expect("selected codec is in simplicity order") as u64,
        survivors: vec![selected],
    });
    Ok(SelectionDecision { selected, steps })
}

pub fn validate_benchmark_report(report: &MaskBenchmarkReport) -> Result<(), MaskBuildError> {
    if report.schema != REPORT_SCHEMA
        || report.profile != MASK_PROFILE
        || !valid_digest(&report.contract_id)
        || report.builder_source_sha256 != BUILDER_SOURCE_SHA256
        || !valid_digest(&report.performance_manifest.sha256)
        || report.method != BenchmarkMethod::ticket_012()
        || report.host.logical_page_bytes != 4_096
        || report.host.allowed_cpu_count_before_pin == 0
        || report.host.cpu_model.is_empty()
        || report.host.kernel.is_empty()
        || report.host.rustc.is_empty()
        || report.host.target.is_empty()
        || report.host.build_profile != "release"
        || report.host.executable.bytes == 0
        || !valid_digest(&report.host.executable.sha256)
        || report.resources.maximum_rss_bytes == 0
    {
        return Err(MaskBuildError::new(
            "BENCHMARK_REPORT",
            "benchmark report contract drifted",
        ));
    }
    let expected = evaluate_mask_candidates(&report.candidates)?;
    if expected != report.selection {
        return Err(MaskBuildError::new(
            "BENCHMARK_REPORT",
            "benchmark selection does not match the closed evaluator",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct CandidateRunInput {
    pub codec: MaskCandidateCodec,
    pub path: PathBuf,
    pub identity: Identity,
}

/// One no-follow candidate descriptor retained across qualification work.
/// Callers clone this descriptor for mmap/compression and reauthenticate the
/// retained descriptor after the measured operation.
pub struct AuthenticatedCandidate {
    held: HeldFile,
    expected: Identity,
}

impl AuthenticatedCandidate {
    pub fn reader_file(&self) -> Result<File, MaskBuildError> {
        self.held
            .file
            .try_clone()
            .map_err(|_| MaskBuildError::new("CANDIDATE_IDENTITY", "candidate clone failed"))
    }

    pub fn file(&self) -> &File {
        &self.held.file
    }

    pub fn reauthenticate(&mut self) -> Result<(), MaskBuildError> {
        if authenticate_held(&mut self.held)? != self.expected {
            return Err(MaskBuildError::new(
                "CANDIDATE_IDENTITY",
                "candidate identity changed",
            ));
        }
        verify_held(&self.held)
    }
}

#[derive(Clone, Debug)]
pub struct BenchmarkRunInput {
    pub contract_id: String,
    pub stage: PathBuf,
    pub performance_manifest: PerformanceManifest,
    pub performance_identity: Identity,
    pub candidates: Vec<CandidateRunInput>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contract_id: String,
    pub selected: MaskCandidateCodec,
    pub published: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueryOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub codec: MaskCandidateCodec,
    pub contig: String,
    pub position: u32,
    pub plus: Vec<QueryOutcomeGene>,
    pub minus: Vec<QueryOutcomeGene>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueryOutcomeGene {
    pub id: String,
    pub stable_id: String,
    pub start: u32,
    pub end: u32,
    pub rank: u32,
    pub boundaries: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReuseAuthorization {
    pub schema: String,
    pub decision: String,
    pub contract_id: String,
    pub builder_source_sha256: String,
    pub coordinator: String,
    pub reviewer: String,
    pub sealed_phases: Vec<Phase>,
    pub capture_receipt: Identity,
    pub prepare_receipt: Option<Identity>,
    pub benchmark_receipt: Option<Identity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapturePromotionAuthorization {
    pub schema: String,
    pub decision: String,
    pub source_contract: Identity,
    pub source_builder_source_sha256: String,
    pub target_contract: Identity,
    pub target_builder_source_sha256: String,
    pub coordinator: String,
    pub reviewer: String,
    pub sealed_phases: Vec<Phase>,
    pub capture_receipt: Identity,
    pub failure_receipt: Identity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapturePromotionPlan {
    pub source_contract: Identity,
    pub source_builder_source_sha256: String,
    pub target_contract: Identity,
    pub target_builder_source_sha256: String,
    pub sealed_phases: Vec<Phase>,
    pub capture_receipt: Identity,
    pub failure_receipt: Identity,
}

#[derive(Clone, Debug, Serialize)]
pub struct CapturePromotionOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub source_contract_id: String,
    pub target_contract_id: String,
    pub sealed_phases: Vec<Phase>,
    pub published: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReuseOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub contract_id: String,
    pub sealed_phases: Vec<Phase>,
    pub published: bool,
}

struct PreparedStage {
    contract_id: String,
    prepare_receipt: PhaseReceipt,
    performance_manifest: PerformanceManifest,
    performance_identity: Identity,
    candidates: Vec<CandidateRunInput>,
}

struct CapturePromotionMaterial {
    plan: CapturePromotionPlan,
    target_contract_bytes: Vec<u8>,
    source_capture_receipt: PhaseReceipt,
}

fn stage_contract_id(stage: &Path) -> Result<String, MaskBuildError> {
    let leaf = stage
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| MaskBuildError::new("STAGE", "stage name is invalid"))?;
    let value = leaf.strip_prefix(STAGE_PREFIX).unwrap_or(leaf);
    let Some(digest) = value.get(..64) else {
        return Err(MaskBuildError::new(
            "STAGE",
            "stage is not contract-addressed",
        ));
    };
    let Some(suffix) = value.get(64..) else {
        return Err(MaskBuildError::new(
            "STAGE",
            "stage is not contract-addressed",
        ));
    };
    let valid_suffix = suffix.is_empty()
        || suffix.strip_prefix("-reuse-").is_some_and(valid_digest)
        || suffix.strip_prefix("-promotion-").is_some_and(valid_digest);
    if !valid_digest(digest) || !valid_suffix {
        Err(MaskBuildError::new(
            "STAGE",
            "stage is not contract-addressed",
        ))
    } else {
        Ok(digest.into())
    }
}

fn authenticate_prepared_stage(stage: &Path) -> Result<PreparedStage, MaskBuildError> {
    let (contract_id, contract, capture) = authenticate_capture_stage(stage)?;
    let receipt_bytes = read_bounded(&stage.join(PREPARE_RECEIPT), MAX_METADATA_BYTES)?;
    let receipt: PhaseReceipt = parse_canonical(&receipt_bytes)?;
    validate_phase_receipt(
        &receipt,
        &contract_id,
        Phase::Prepare,
        Some(Phase::Benchmark),
    )?;
    authenticate_receipt_reuse(stage, &receipt)?;
    let capture_receipt = hash_file(&stage.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)?;
    if receipt.inputs.get("capture_receipt") != Some(&capture_receipt) {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "prepare capture receipt identity drifted",
        ));
    }
    if receipt.inputs.get("gtf") != Some(&contract.gtf)
        || receipt.inputs.get("observation") != capture.outputs.get("observation")
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "prepare source identities drifted",
        ));
    }
    for (key, relative, maximum) in [
        ("canonical", CANONICAL_MEMBER, MAX_OBSERVATION_BYTES),
        ("inventory", INVENTORY_MEMBER, MAX_METADATA_BYTES as u64),
        (
            "performance",
            PERFORMANCE_MEMBER,
            MAX_PERFORMANCE_BYTES as u64,
        ),
    ] {
        let expected = receipt
            .outputs
            .get(key)
            .ok_or_else(|| MaskBuildError::new("RECEIPT", "prepare member is missing"))?;
        if hash_file(&stage.join(relative), maximum)? != *expected {
            return Err(MaskBuildError::new(
                "RECEIPT",
                "sealed prepare member drifted",
            ));
        }
    }
    let inventory_bytes = read_bounded(&stage.join(INVENTORY_MEMBER), MAX_METADATA_BYTES)?;
    let inventory: Inventory = parse_canonical(&inventory_bytes)?;
    if inventory.schema != INVENTORY_SCHEMA
        || inventory.profile != MASK_PROFILE
        || inventory.builder_source_sha256 != BUILDER_SOURCE_SHA256
        || receipt.outputs.get("canonical") != Some(&inventory.canonical_stream)
    {
        return Err(MaskBuildError::new(
            "INVENTORY",
            "sealed inventory contract drifted",
        ));
    }
    let performance_path = stage.join(PERFORMANCE_MEMBER);
    let performance_identity = hash_file(&performance_path, MAX_PERFORMANCE_BYTES as u64)?;
    let performance_manifest = read_performance(&performance_path)?;
    let mut candidates = Vec::with_capacity(3);
    for codec in MaskCandidateCodec::ALL {
        let key = format!("candidate:{}", codec.name());
        let expected = receipt
            .outputs
            .get(&key)
            .cloned()
            .ok_or_else(|| MaskBuildError::new("RECEIPT", "candidate identity is missing"))?;
        let path = stage.join(CANDIDATE_DIRECTORY).join(codec.filename());
        if hash_file(&path, CANDIDATE_MEMBER_MAX)? != expected {
            return Err(MaskBuildError::new(
                "RECEIPT",
                "sealed candidate member drifted",
            ));
        }
        candidates.push(CandidateRunInput {
            codec,
            path,
            identity: expected,
        });
    }
    Ok(PreparedStage {
        contract_id,
        prepare_receipt: receipt,
        performance_manifest,
        performance_identity,
        candidates,
    })
}

fn authenticate_benchmark_stage(
    stage: &Path,
    prepared: &PreparedStage,
) -> Result<(), MaskBuildError> {
    let receipt_bytes = read_bounded(&stage.join(BENCHMARK_RECEIPT), MAX_METADATA_BYTES)?;
    let receipt: PhaseReceipt = parse_canonical(&receipt_bytes)?;
    validate_phase_receipt(&receipt, &prepared.contract_id, Phase::Benchmark, None)?;
    authenticate_receipt_reuse(stage, &receipt)?;
    let prepare_identity = hash_file(&stage.join(PREPARE_RECEIPT), MAX_METADATA_BYTES as u64)?;
    if receipt.inputs.get("prepare_receipt") != Some(&prepare_identity)
        || receipt.inputs.get("performance") != Some(&prepared.performance_identity)
        || prepared.candidates.iter().any(|candidate| {
            receipt
                .inputs
                .get(&format!("candidate:{}", candidate.codec.name()))
                != Some(&candidate.identity)
        })
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "benchmark input identities drifted",
        ));
    }
    let report_bytes = read_bounded(&stage.join("benchmark/report.json"), MAX_METADATA_BYTES)?;
    let report: MaskBenchmarkReport = parse_canonical(&report_bytes)?;
    validate_benchmark_report(&report)?;
    if receipt.outputs.get("report") != Some(&identity(&report_bytes))
        || report.contract_id != prepared.contract_id
        || report.performance_manifest != prepared.performance_identity
        || report.candidates.iter().any(|measurement| {
            prepared
                .candidates
                .iter()
                .find(|candidate| candidate.codec == measurement.codec)
                .map(|candidate| &candidate.identity)
                != Some(&measurement.member)
        })
    {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "benchmark report identities drifted",
        ));
    }
    Ok(())
}

/// Open and authenticate one prepared candidate through a held no-follow
/// descriptor. The returned descriptor is rewound and may be mmaped without
/// reopening its pathname.
pub fn open_candidate_for_benchmark(
    input: &CandidateRunInput,
) -> Result<AuthenticatedCandidate, MaskBuildError> {
    let mut held = open_held(&input.path, CANDIDATE_MEMBER_MAX)?;
    if authenticate_held(&mut held)? != input.identity {
        return Err(MaskBuildError::new(
            "CANDIDATE_IDENTITY",
            "candidate identity changed",
        ));
    }
    held.file.seek(SeekFrom::Start(0))?;
    verify_held(&held)?;
    Ok(AuthenticatedCandidate {
        held,
        expected: input.identity.clone(),
    })
}

pub fn query_prepared_candidate(
    stage: &Path,
    codec: MaskCandidateCodec,
    contig: Grch38Contig,
    position: GenomicPosition,
    stable: Option<pangopup_core::EnsemblGeneId>,
) -> Result<QueryOutcome, MaskBuildError> {
    require_absolute(stage, "stage")?;
    let prepared = authenticate_prepared_stage(stage)?;
    let input = prepared
        .candidates
        .iter()
        .find(|input| input.codec == codec)
        .ok_or_else(|| MaskBuildError::new("CANDIDATE", "candidate is missing"))?;
    let mut authenticated = open_candidate_for_benchmark(input)?;
    let reader = MaskCandidateReader::open_held(authenticated.reader_file()?)?;
    let mut output = MaskQueryBuffer::with_capacity(64, 4_096);
    reader.query_stable(contig, position, stable, &mut output)?;
    let convert = |values: &[pangopup_index::mask_candidates::MaskQueryGene]| {
        values
            .iter()
            .map(|gene| QueryOutcomeGene {
                id: gene.identity().to_string(),
                stable_id: gene.stable_identity().to_string(),
                start: gene.start().get(),
                end: gene.end().get(),
                rank: gene.query_rank(),
                boundaries: output
                    .boundaries(gene)
                    .iter()
                    .map(|boundary| boundary.get())
                    .collect(),
            })
            .collect()
    };
    let outcome = QueryOutcome {
        ok: true,
        command: "query",
        codec,
        contig: contig.to_string(),
        position: position.get(),
        plus: convert(output.plus()),
        minus: convert(output.minus()),
    };
    authenticated.reauthenticate()?;
    Ok(outcome)
}

/// Run the optimized measurement closure inside the sealed three-phase
/// lifecycle. Any handled closure, report, receipt, or publication failure
/// preserves the stage and its prior sealed phases with a failure receipt.
pub fn benchmark_phase<F>(stage: &Path, runner: F) -> Result<BenchmarkOutcome, MaskBuildError>
where
    F: FnOnce(&BenchmarkRunInput) -> Result<MaskBenchmarkReport, MaskBuildError>,
{
    require_absolute(stage, "stage")?;
    let mut lease = StageLease::open(stage)?;
    let prepared = authenticate_prepared_stage(stage)?;
    lease.verify_current()?;
    let contract_id = prepared.contract_id.clone();
    if stage.join(BENCHMARK_RECEIPT).exists()
        || stage.join(FAILURE_RECEIPT).exists()
        || stage.join("benchmark").exists()
    {
        return Err(MaskBuildError::new(
            "PHASE_STATE",
            "benchmark phase is not an automatic retry",
        ));
    }
    let input = BenchmarkRunInput {
        contract_id: contract_id.clone(),
        stage: stage.to_owned(),
        performance_manifest: prepared.performance_manifest,
        performance_identity: prepared.performance_identity,
        candidates: prepared.candidates,
    };
    let result = (|| {
        create_private_directory(&stage.join("benchmark"))?;
        benchmark_into_stage(stage, &mut lease, &prepared.prepare_receipt, &input, runner)
    })();
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            preserve_failure_held(&lease, &contract_id, Phase::Benchmark, &error)?;
            Err(error)
        }
    }
}

fn benchmark_into_stage<F>(
    stage: &Path,
    lease: &mut StageLease,
    prepare_receipt: &PhaseReceipt,
    input: &BenchmarkRunInput,
    runner: F,
) -> Result<BenchmarkOutcome, MaskBuildError>
where
    F: FnOnce(&BenchmarkRunInput) -> Result<MaskBenchmarkReport, MaskBuildError>,
{
    let report = runner(input)?;
    validate_benchmark_report(&report)?;
    if report.contract_id != input.contract_id
        || report.performance_manifest != input.performance_identity
    {
        return Err(MaskBuildError::new(
            "BENCHMARK_REPORT",
            "benchmark report input identities drifted",
        ));
    }
    for candidate in &report.candidates {
        let expected = input
            .candidates
            .iter()
            .find(|value| value.codec == candidate.codec)
            .map(|value| &value.identity)
            .ok_or_else(|| MaskBuildError::new("BENCHMARK_REPORT", "candidate is missing"))?;
        if &candidate.member != expected {
            return Err(MaskBuildError::new(
                "BENCHMARK_REPORT",
                "candidate report identity drifted",
            ));
        }
    }
    let report_bytes = canonical(&report)?;
    if report_bytes.len() > MAX_METADATA_BYTES {
        return Err(MaskBuildError::new(
            "RESOURCE",
            "benchmark report exceeds metadata bound",
        ));
    }
    write_synced(&stage.join("benchmark/report.json"), &report_bytes, 0o400)?;
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "prepare_receipt".into(),
        hash_file(&stage.join(PREPARE_RECEIPT), MAX_METADATA_BYTES as u64)?,
    );
    inputs.insert("performance".into(), input.performance_identity.clone());
    for candidate in &input.candidates {
        inputs.insert(
            format!("candidate:{}", candidate.codec.name()),
            candidate.identity.clone(),
        );
    }
    if prepare_receipt.outputs.get("performance") != Some(&input.performance_identity) {
        return Err(MaskBuildError::new(
            "RECEIPT",
            "prepared performance identity drifted",
        ));
    }
    let mut outputs = BTreeMap::new();
    outputs.insert("report".into(), identity(&report_bytes));
    seal_phase(
        stage,
        BENCHMARK_RECEIPT,
        PhaseReceipt {
            schema: PHASE_RECEIPT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            contract_id: input.contract_id.clone(),
            phase: Phase::Benchmark,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            inputs,
            outputs,
            next_phase: None,
            reused_from: None,
        },
    )?;
    sync_directory(&stage.join("benchmark"))?;
    lease.publish(OsStr::new(&input.contract_id))?;
    Ok(BenchmarkOutcome {
        ok: true,
        command: "benchmark",
        contract_id: input.contract_id.clone(),
        selected: report.selection.selected,
        published: true,
    })
}

pub fn inspect_phase(stage: &Path) -> Result<InspectOutcome, MaskBuildError> {
    require_absolute(stage, "stage")?;
    let contract_id = stage_contract_id(stage)?;
    let _ = authenticate_capture_stage(stage)?;
    let mut sealed_phases = vec![Phase::Capture];
    let mut prepared = None;
    if stage.join(PREPARE_RECEIPT).exists() {
        let authenticated = authenticate_prepared_stage(stage)?;
        if authenticated.contract_id != contract_id {
            return Err(MaskBuildError::new("STAGE", "stage contract drifted"));
        }
        prepared = Some(authenticated);
        sealed_phases.push(Phase::Prepare);
    }
    if stage.join(BENCHMARK_RECEIPT).exists() {
        if sealed_phases.last() != Some(&Phase::Prepare) {
            return Err(MaskBuildError::new(
                "RECEIPT",
                "benchmark receipt has no prepared phase",
            ));
        }
        authenticate_benchmark_stage(
            stage,
            prepared
                .as_ref()
                .ok_or_else(|| MaskBuildError::new("RECEIPT", "prepared phase is missing"))?,
        )?;
        sealed_phases.push(Phase::Benchmark);
    }
    let failed = stage.join(FAILURE_RECEIPT).exists();
    if failed {
        let bytes = read_bounded(&stage.join(FAILURE_RECEIPT), MAX_METADATA_BYTES)?;
        let failure: FailureReceipt = parse_canonical(&bytes)?;
        if failure.schema != FAILURE_SCHEMA
            || failure.profile != MASK_PROFILE
            || failure.contract_id != contract_id
            || failure.sealed_phases != sealed_phases
        {
            return Err(MaskBuildError::new(
                "FAILURE_RECEIPT",
                "failure receipt drifted",
            ));
        }
    }
    Ok(InspectOutcome {
        ok: true,
        command: "inspect",
        contract_id,
        sealed_phases,
        failed,
    })
}

/// Derive the exact current-builder capture contract and authorization facts
/// from one failed, sealed capture without creating or changing any file.
pub fn plan_capture_promotion(
    prior_stage: &Path,
    source_builder_source_sha256: &str,
) -> Result<CapturePromotionPlan, MaskBuildError> {
    require_absolute(prior_stage, "prior stage")?;
    let lease = StageLease::open(prior_stage)?;
    let material =
        authenticate_capture_promotion_source(prior_stage, source_builder_source_sha256)?;
    lease.verify_current()?;
    Ok(material.plan)
}

fn authenticate_capture_promotion_source(
    prior_stage: &Path,
    source_builder_source_sha256: &str,
) -> Result<CapturePromotionMaterial, MaskBuildError> {
    if !valid_digest(source_builder_source_sha256)
        || source_builder_source_sha256 == BUILDER_SOURCE_SHA256
    {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "source builder identity is invalid for promotion",
        ));
    }
    let (source_contract_id, source_contract, source_capture_receipt) =
        authenticate_capture_stage_for_builder(prior_stage, source_builder_source_sha256)?;
    let source_contract_bytes = read_bounded(
        &prior_stage.join("contract.json"),
        MAX_CAPTURE_CONTRACT_BYTES,
    )?;
    let source_contract_identity = identity(&source_contract_bytes);
    if source_contract_identity.sha256 != source_contract_id {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "source contract identity drifted",
        ));
    }
    if prior_stage.join(PREPARE_RECEIPT).exists() || prior_stage.join(BENCHMARK_RECEIPT).exists() {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "source stage contains a sealed phase beyond capture",
        ));
    }
    let capture_receipt = hash_file(
        &prior_stage.join(CAPTURE_RECEIPT),
        MAX_METADATA_BYTES as u64,
    )?;
    let failure_bytes = read_bounded(&prior_stage.join(FAILURE_RECEIPT), MAX_METADATA_BYTES)?;
    let failure_receipt = identity(&failure_bytes);
    let failure: FailureReceipt = parse_canonical(&failure_bytes)?;
    if failure.schema != FAILURE_SCHEMA
        || failure.profile != MASK_PROFILE
        || failure.contract_id != source_contract_id
        || failure.failed_phase != Phase::Prepare
        || failure.sealed_phases != [Phase::Capture]
        || failure.code.trim().is_empty()
        || failure.message.trim().is_empty()
        || failure.code.len() > MAX_ERROR_BYTES
        || failure.message.len() > MAX_ERROR_BYTES
    {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "source failure receipt drifted",
        ));
    }

    let mut target_contract = source_contract.clone();
    target_contract.builder_source_sha256 = BUILDER_SOURCE_SHA256.into();
    if !capture_contract_differs_only_by_builder(
        &source_contract,
        &target_contract,
        source_builder_source_sha256,
        BUILDER_SOURCE_SHA256,
    ) {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "target contract changed beyond builder provenance",
        ));
    }
    let target_contract_bytes = canonical(&target_contract)?;
    validate_capture_contract_bytes(
        &target_contract,
        &target_contract.environment,
        &target_contract_bytes,
    )?;
    let target_contract_identity = identity(&target_contract_bytes);
    if target_contract_identity.sha256 == source_contract_identity.sha256 {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "target contract identity did not change",
        ));
    }
    Ok(CapturePromotionMaterial {
        plan: CapturePromotionPlan {
            source_contract: source_contract_identity,
            source_builder_source_sha256: source_builder_source_sha256.into(),
            target_contract: target_contract_identity,
            target_builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            sealed_phases: vec![Phase::Capture],
            capture_receipt,
            failure_receipt,
        },
        target_contract_bytes,
        source_capture_receipt,
    })
}

fn capture_contract_differs_only_by_builder(
    source: &OwnedCaptureContract,
    target: &OwnedCaptureContract,
    source_builder_source_sha256: &str,
    target_builder_source_sha256: &str,
) -> bool {
    source.builder_source_sha256 == source_builder_source_sha256
        && target.builder_source_sha256 == target_builder_source_sha256
        && source_builder_source_sha256 != target_builder_source_sha256
        && source.schema == target.schema
        && source.profile == target.profile
        && source.helper == target.helper
        && source.database == target.database
        && source.gtf == target.gtf
        && source.python == target.python
        && source.python_environment == target.python_environment
        && source.environment == target.environment
}

fn validate_capture_promotion_authorization(
    authorization: &CapturePromotionAuthorization,
    plan: &CapturePromotionPlan,
) -> Result<(), MaskBuildError> {
    for value in [
        &authorization.source_contract,
        &authorization.target_contract,
        &authorization.capture_receipt,
        &authorization.failure_receipt,
    ] {
        validate_expected_identity(value, MAX_CAPTURE_CONTRACT_BYTES as u64)?;
    }
    if authorization.schema != CAPTURE_PROMOTION_AUTHORIZATION_SCHEMA
        || authorization.decision != "RUN-READY-CAPTURE-PROMOTION"
        || authorization.source_contract != plan.source_contract
        || authorization.source_builder_source_sha256 != plan.source_builder_source_sha256
        || authorization.target_contract != plan.target_contract
        || authorization.target_builder_source_sha256 != plan.target_builder_source_sha256
        || authorization.coordinator.trim().is_empty()
        || authorization.reviewer.trim().is_empty()
        || authorization.coordinator == authorization.reviewer
        || authorization.sealed_phases != [Phase::Capture]
        || authorization.sealed_phases != plan.sealed_phases
        || authorization.capture_receipt != plan.capture_receipt
        || authorization.failure_receipt != plan.failure_receipt
    {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "authorization does not match the derived promotion",
        ));
    }
    Ok(())
}

/// Copy one independently authorized sealed capture into a new contract whose
/// only semantic change is the current builder fingerprint. The source stage,
/// its failure receipt, and all unsealed prepare output remain untouched.
pub fn promote_sealed_capture(
    prior_stage: &Path,
    output_parent: &Path,
    authorization_path: &Path,
) -> Result<CapturePromotionOutcome, MaskBuildError> {
    require_absolute(prior_stage, "prior stage")?;
    require_absolute(output_parent, "output parent")?;
    require_absolute(authorization_path, "capture promotion authorization")?;
    let (authorization_identity, authorization_bytes) =
        read_reuse_authorization(authorization_path)?;
    let authorization: CapturePromotionAuthorization = parse_canonical(&authorization_bytes)?;
    let source_lease = StageLease::open(prior_stage)?;
    let material = authenticate_capture_promotion_source(
        prior_stage,
        &authorization.source_builder_source_sha256,
    )?;
    validate_capture_promotion_authorization(&authorization, &material.plan)?;
    source_lease.verify_current()?;

    let target_contract_id = material.plan.target_contract.sha256.clone();
    let stage = output_parent.join(format!(
        "{STAGE_PREFIX}{target_contract_id}-promotion-{}",
        authorization_identity.sha256
    ));
    create_private_stage(output_parent, &stage)?;
    let lease = StageLease::open(&stage)?;
    let result = promote_capture_into_stage(
        prior_stage,
        &stage,
        &authorization_identity,
        &authorization_bytes,
        &material,
        &source_lease,
        &lease,
    );
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            preserve_failure_held(&lease, &target_contract_id, Phase::Capture, &error)?;
            Err(error)
        }
    }
}

fn promote_capture_into_stage(
    prior_stage: &Path,
    stage: &Path,
    authorization_identity: &Identity,
    authorization_bytes: &[u8],
    material: &CapturePromotionMaterial,
    source_lease: &StageLease,
    target_lease: &StageLease,
) -> Result<CapturePromotionOutcome, MaskBuildError> {
    check_cancellation()?;
    create_private_directory(&stage.join("source"))?;
    create_private_directory(&stage.join("capture"))?;
    if identity(authorization_bytes) != *authorization_identity {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "authorization identity drifted",
        ));
    }
    write_synced(
        &stage.join(REUSE_AUTHORIZATION_MEMBER),
        authorization_bytes,
        0o400,
    )?;
    write_synced(
        &stage.join("contract.json"),
        &material.target_contract_bytes,
        0o400,
    )?;
    for (key, relative, maximum) in [
        ("database", SNAPSHOT_DATABASE, MAX_DATABASE_BYTES),
        ("gtf", SNAPSHOT_GTF, MAX_GTF_BYTES),
        (
            "pyvenv_config",
            SNAPSHOT_PYVENV_CONFIG,
            MAX_PYVENV_CONFIG_BYTES,
        ),
        ("observation", OBSERVATION_MEMBER, MAX_OBSERVATION_BYTES),
        (
            "environment",
            ENVIRONMENT_MEMBER,
            MAX_ENVIRONMENT_BYTES as u64,
        ),
    ] {
        check_cancellation()?;
        let expected = material
            .source_capture_receipt
            .inputs
            .get(key)
            .or_else(|| material.source_capture_receipt.outputs.get(key))
            .ok_or_else(|| {
                MaskBuildError::new("CAPTURE_PROMOTION", "capture member identity is missing")
            })?;
        copy_reauthenticated(
            &prior_stage.join(relative),
            &stage.join(relative),
            expected,
            maximum,
        )?;
    }

    source_lease.verify_current()?;
    if hash_file(
        &prior_stage.join("contract.json"),
        MAX_CAPTURE_CONTRACT_BYTES as u64,
    )? != material.plan.source_contract
        || hash_file(
            &prior_stage.join(CAPTURE_RECEIPT),
            MAX_METADATA_BYTES as u64,
        )? != material.plan.capture_receipt
        || hash_file(
            &prior_stage.join(FAILURE_RECEIPT),
            MAX_METADATA_BYTES as u64,
        )? != material.plan.failure_receipt
    {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "source evidence changed during promotion",
        ));
    }

    let mut capture = material.source_capture_receipt.clone();
    capture.contract_id = material.plan.target_contract.sha256.clone();
    capture.builder_source_sha256 = BUILDER_SOURCE_SHA256.into();
    capture
        .inputs
        .insert("contract".into(), material.plan.target_contract.clone());
    capture
        .inputs
        .insert("reuse_authorization".into(), authorization_identity.clone());
    capture.reused_from = Some(material.plan.capture_receipt.sha256.clone());
    validate_phase_receipt(
        &capture,
        &material.plan.target_contract.sha256,
        Phase::Capture,
        Some(Phase::Prepare),
    )?;
    seal_phase(stage, CAPTURE_RECEIPT, capture)?;
    target_lease.verify_current()?;
    let inspected = inspect_phase(stage)?;
    if inspected.contract_id != material.plan.target_contract.sha256
        || inspected.sealed_phases != [Phase::Capture]
        || inspected.failed
    {
        return Err(MaskBuildError::new(
            "CAPTURE_PROMOTION",
            "promoted capture failed current-builder authentication",
        ));
    }
    Ok(CapturePromotionOutcome {
        ok: true,
        command: "promote-capture",
        source_contract_id: material.plan.source_contract.sha256.clone(),
        target_contract_id: material.plan.target_contract.sha256.clone(),
        sealed_phases: vec![Phase::Capture],
        published: false,
    })
}

/// Create a new absent stage from explicitly authorized, fully sealed prior
/// phases. Partial output and the prior failure receipt are never copied.
fn read_reuse_authorization(
    authorization_path: &Path,
) -> Result<(Identity, Vec<u8>), MaskBuildError> {
    let mut authorization_file = open_held(authorization_path, MAX_METADATA_BYTES as u64)?;
    let authorization_identity = authenticate_held(&mut authorization_file)?;
    authorization_file.file.seek(SeekFrom::Start(0))?;
    let mut authorization_bytes = Vec::with_capacity(authorization_identity.bytes as usize);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancellation()?;
        let read = authorization_file.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        authorization_bytes.extend_from_slice(&buffer[..read]);
        if authorization_bytes.len() as u64 > authorization_identity.bytes {
            return Err(MaskBuildError::new(
                "REUSE_AUTHORIZATION",
                "reuse authorization changed while reading",
            ));
        }
    }
    verify_held(&authorization_file)?;
    if authorization_bytes.len() as u64 != authorization_identity.bytes {
        return Err(MaskBuildError::new(
            "REUSE_AUTHORIZATION",
            "reuse authorization changed while reading",
        ));
    }
    Ok((authorization_identity, authorization_bytes))
}

pub fn reuse_sealed_phases(
    prior_stage: &Path,
    output_parent: &Path,
    authorization_path: &Path,
) -> Result<ReuseOutcome, MaskBuildError> {
    require_absolute(prior_stage, "prior stage")?;
    require_absolute(output_parent, "output parent")?;
    require_absolute(authorization_path, "reuse authorization")?;
    let inspected = inspect_phase(prior_stage)?;
    if !inspected.failed {
        return Err(MaskBuildError::new(
            "REUSE_AUTHORIZATION",
            "only a preserved failed stage may be reused",
        ));
    }
    let (authorization_identity, authorization_bytes) =
        read_reuse_authorization(authorization_path)?;
    let authorization: ReuseAuthorization = parse_canonical(&authorization_bytes)?;
    if authorization.schema != "pangopup-mask-reuse-authorization-v1"
        || authorization.decision != "RUN-READY-REUSE"
        || authorization.contract_id != inspected.contract_id
        || authorization.builder_source_sha256 != BUILDER_SOURCE_SHA256
        || authorization.coordinator.trim().is_empty()
        || authorization.reviewer.trim().is_empty()
        || authorization.coordinator == authorization.reviewer
        || authorization.sealed_phases != inspected.sealed_phases
        || authorization.sealed_phases.is_empty()
    {
        return Err(MaskBuildError::new(
            "REUSE_AUTHORIZATION",
            "reuse authorization does not match the sealed run",
        ));
    }
    let capture_identity = hash_file(
        &prior_stage.join(CAPTURE_RECEIPT),
        MAX_METADATA_BYTES as u64,
    )?;
    let prepare_identity = prior_stage
        .join(PREPARE_RECEIPT)
        .is_file()
        .then(|| {
            hash_file(
                &prior_stage.join(PREPARE_RECEIPT),
                MAX_METADATA_BYTES as u64,
            )
        })
        .transpose()?;
    let benchmark_identity = prior_stage
        .join(BENCHMARK_RECEIPT)
        .is_file()
        .then(|| {
            hash_file(
                &prior_stage.join(BENCHMARK_RECEIPT),
                MAX_METADATA_BYTES as u64,
            )
        })
        .transpose()?;
    if authorization.capture_receipt != capture_identity
        || authorization.prepare_receipt != prepare_identity
        || authorization.benchmark_receipt != benchmark_identity
    {
        return Err(MaskBuildError::new(
            "REUSE_AUTHORIZATION",
            "authorized receipt identities drifted",
        ));
    }
    let contract_id = inspected.contract_id;
    let stage = output_parent.join(format!(
        "{STAGE_PREFIX}{contract_id}-reuse-{}",
        authorization_identity.sha256
    ));
    create_private_stage(output_parent, &stage)?;
    let mut lease = StageLease::open(&stage)?;
    let result = reuse_into_stage(
        prior_stage,
        &stage,
        &contract_id,
        &authorization,
        &authorization_identity,
        &authorization_bytes,
        &mut lease,
    );
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            let failed_phase = match authorization.sealed_phases.last() {
                Some(Phase::Benchmark) => Phase::Benchmark,
                Some(Phase::Prepare) => Phase::Prepare,
                _ => Phase::Capture,
            };
            preserve_failure_held(&lease, &contract_id, failed_phase, &error)?;
            Err(error)
        }
    }
}

fn reuse_into_stage(
    prior: &Path,
    stage: &Path,
    contract_id: &str,
    authorization: &ReuseAuthorization,
    authorization_identity: &Identity,
    authorization_bytes: &[u8],
    lease: &mut StageLease,
) -> Result<ReuseOutcome, MaskBuildError> {
    check_cancellation()?;
    create_private_directory(&stage.join("source"))?;
    create_private_directory(&stage.join("capture"))?;
    if identity(authorization_bytes) != *authorization_identity {
        return Err(MaskBuildError::new(
            "REUSE_AUTHORIZATION",
            "reuse authorization identity drifted",
        ));
    }
    write_synced(
        &stage.join(REUSE_AUTHORIZATION_MEMBER),
        authorization_bytes,
        0o400,
    )?;
    let contract_identity = hash_file(
        &prior.join("contract.json"),
        MAX_CAPTURE_CONTRACT_BYTES as u64,
    )?;
    if contract_identity.sha256 != contract_id {
        return Err(MaskBuildError::new("REUSE", "contract identity drifted"));
    }
    copy_reauthenticated(
        &prior.join("contract.json"),
        &stage.join("contract.json"),
        &contract_identity,
        MAX_CAPTURE_CONTRACT_BYTES as u64,
    )?;
    let old_capture_bytes = read_bounded(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES)?;
    let mut capture: PhaseReceipt = parse_canonical(&old_capture_bytes)?;
    validate_phase_receipt(&capture, contract_id, Phase::Capture, Some(Phase::Prepare))?;
    for (key, relative, maximum) in [
        ("database", SNAPSHOT_DATABASE, MAX_DATABASE_BYTES),
        ("gtf", SNAPSHOT_GTF, MAX_GTF_BYTES),
        (
            "pyvenv_config",
            SNAPSHOT_PYVENV_CONFIG,
            MAX_PYVENV_CONFIG_BYTES,
        ),
        ("observation", OBSERVATION_MEMBER, MAX_OBSERVATION_BYTES),
        (
            "environment",
            ENVIRONMENT_MEMBER,
            MAX_ENVIRONMENT_BYTES as u64,
        ),
    ] {
        check_cancellation()?;
        let expected = capture
            .inputs
            .get(key)
            .or_else(|| capture.outputs.get(key))
            .ok_or_else(|| MaskBuildError::new("REUSE", "capture member identity is missing"))?;
        copy_reauthenticated(
            &prior.join(relative),
            &stage.join(relative),
            expected,
            maximum,
        )?;
    }
    capture.reused_from = Some(authorization.capture_receipt.sha256.clone());
    capture
        .inputs
        .insert("reuse_authorization".into(), authorization_identity.clone());
    seal_phase(stage, CAPTURE_RECEIPT, capture)?;

    if authorization.sealed_phases.contains(&Phase::Prepare) {
        check_cancellation()?;
        create_private_directory(&stage.join("prepare"))?;
        create_private_directory(&stage.join(CANDIDATE_DIRECTORY))?;
        let old_prepare_bytes = read_bounded(&prior.join(PREPARE_RECEIPT), MAX_METADATA_BYTES)?;
        let mut prepare: PhaseReceipt = parse_canonical(&old_prepare_bytes)?;
        validate_phase_receipt(
            &prepare,
            contract_id,
            Phase::Prepare,
            Some(Phase::Benchmark),
        )?;
        for (key, relative, maximum) in [
            ("canonical", CANONICAL_MEMBER, MAX_OBSERVATION_BYTES),
            ("inventory", INVENTORY_MEMBER, MAX_METADATA_BYTES as u64),
            (
                "performance",
                PERFORMANCE_MEMBER,
                MAX_PERFORMANCE_BYTES as u64,
            ),
        ] {
            check_cancellation()?;
            let expected = prepare.outputs.get(key).ok_or_else(|| {
                MaskBuildError::new("REUSE", "prepare member identity is missing")
            })?;
            copy_reauthenticated(
                &prior.join(relative),
                &stage.join(relative),
                expected,
                maximum,
            )?;
        }
        for codec in MaskCandidateCodec::ALL {
            check_cancellation()?;
            let key = format!("candidate:{}", codec.name());
            let expected = prepare
                .outputs
                .get(&key)
                .ok_or_else(|| MaskBuildError::new("REUSE", "candidate identity is missing"))?;
            let relative = PathBuf::from(CANDIDATE_DIRECTORY).join(codec.filename());
            copy_reauthenticated(
                &prior.join(&relative),
                &stage.join(&relative),
                expected,
                CANDIDATE_MEMBER_MAX,
            )?;
        }
        prepare.inputs.insert(
            "capture_receipt".into(),
            hash_file(&stage.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)?,
        );
        prepare
            .inputs
            .insert("reuse_authorization".into(), authorization_identity.clone());
        prepare.reused_from = authorization
            .prepare_receipt
            .as_ref()
            .map(|identity| identity.sha256.clone());
        seal_phase(stage, PREPARE_RECEIPT, prepare)?;
    }

    let mut published = false;
    if authorization.sealed_phases.contains(&Phase::Benchmark) {
        check_cancellation()?;
        create_private_directory(&stage.join("benchmark"))?;
        let old_benchmark_bytes = read_bounded(&prior.join(BENCHMARK_RECEIPT), MAX_METADATA_BYTES)?;
        let mut benchmark: PhaseReceipt = parse_canonical(&old_benchmark_bytes)?;
        validate_phase_receipt(&benchmark, contract_id, Phase::Benchmark, None)?;
        let expected = benchmark
            .outputs
            .get("report")
            .ok_or_else(|| MaskBuildError::new("REUSE", "benchmark report identity is missing"))?;
        copy_reauthenticated(
            &prior.join("benchmark/report.json"),
            &stage.join("benchmark/report.json"),
            expected,
            MAX_METADATA_BYTES as u64,
        )?;
        benchmark.inputs.insert(
            "prepare_receipt".into(),
            hash_file(&stage.join(PREPARE_RECEIPT), MAX_METADATA_BYTES as u64)?,
        );
        benchmark
            .inputs
            .insert("reuse_authorization".into(), authorization_identity.clone());
        benchmark.reused_from = authorization
            .benchmark_receipt
            .as_ref()
            .map(|identity| identity.sha256.clone());
        seal_phase(stage, BENCHMARK_RECEIPT, benchmark)?;
        lease.publish(OsStr::new(contract_id))?;
        published = true;
    }
    Ok(ReuseOutcome {
        ok: true,
        command: "reuse",
        contract_id: contract_id.into(),
        sealed_phases: authorization.sealed_phases.clone(),
        published,
    })
}

fn copy_reauthenticated(
    source: &Path,
    destination: &Path,
    expected: &Identity,
    maximum: u64,
) -> Result<(), MaskBuildError> {
    if expected.bytes == 0 || expected.bytes > maximum || !valid_digest(&expected.sha256) {
        return Err(MaskBuildError::new("REUSE", "member identity is invalid"));
    }
    let mut held = open_held(source, maximum)?;
    copy_held_authenticated(&mut held, destination, expected)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    struct TestCancellation;

    impl TestCancellation {
        fn after(checks: usize) -> Self {
            clear_test_cancellation();
            test_cancel_after(checks);
            Self
        }
    }

    impl Drop for TestCancellation {
        fn drop(&mut self) {
            clear_test_cancellation();
        }
    }

    impl Scratch {
        fn new() -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pangopup-mask-build-test-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create scratch");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(value: u8) -> String {
        format!("{value:064x}")
    }

    fn replace_canonical(path: &Path, value: &impl Serialize) {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("make fixture writable");
        fs::write(path, canonical(value).expect("canonical replacement")).expect("replace fixture");
        fs::set_permissions(path, fs::Permissions::from_mode(0o400)).expect("restore fixture mode");
    }

    fn environment() -> (ObservationEnvironment, EnvironmentPolicy) {
        let schema = digest(9);
        let observed = ObservationEnvironment {
            kind: "environment".into(),
            schema: OBSERVATION_SCHEMA.into(),
            python: "test-python".into(),
            gffutils: "test-gffutils".into(),
            executable: "/fixture/venv/bin/python".into(),
            prefix: "/fixture/venv".into(),
            base_prefix: "/fixture/base".into(),
            base_executable: "/fixture/base/bin/python".into(),
            sqlite3_module: "test-module".into(),
            sqlite_library: "test-library".into(),
            sql_row_control_sha256: SQL_ROW_CONTROL_SHA256.into(),
            sqlite_compile_options: vec!["THREADSAFE=1".into()],
            schema_sha256: schema.clone(),
            query_shape: "gtf.region((contig,pos-1,pos-1),featuretype=gene)".into(),
            region_sql: "SELECT * FROM features WHERE seqid='chr1'".into(),
            query_plan: vec![vec![
                serde_json::Value::from(1),
                serde_json::Value::from(0),
                serde_json::Value::from(0),
                serde_json::Value::String("SEARCH features USING INDEX fixture".into()),
            ]],
            modules: vec![
                ModuleIdentity {
                    name: "_sqlite3".into(),
                    kind: "interpreter".into(),
                    path: "built-in".into(),
                    bytes: 16,
                    sha256: digest(7),
                    device: 0,
                    inode: 0,
                    links: 0,
                    modified_ns: 0,
                    changed_ns: 0,
                },
                ModuleIdentity {
                    name: "gffutils".into(),
                    kind: "file".into(),
                    path: "/fixture/venv/gffutils/__init__.py".into(),
                    bytes: 10,
                    sha256: digest(8),
                    device: 1,
                    inode: 2,
                    links: 3,
                    modified_ns: 4,
                    changed_ns: 5,
                },
                ModuleIdentity {
                    name: "sqlite3".into(),
                    kind: "file".into(),
                    path: "/fixture/base/sqlite3/__init__.py".into(),
                    bytes: 10,
                    sha256: digest(9),
                    device: 1,
                    inode: 3,
                    links: 1,
                    modified_ns: 4,
                    changed_ns: 5,
                },
            ],
        };
        let policy = EnvironmentPolicy {
            python: observed.python.clone(),
            gffutils: observed.gffutils.clone(),
            sqlite3_module: observed.sqlite3_module.clone(),
            sqlite_library: observed.sqlite_library.clone(),
            schema_sha256: schema,
            query_plan_contains: "USING INDEX fixture".into(),
        };
        (observed, policy)
    }

    fn representative_large_environment() -> ObservationEnvironment {
        let mut observed = environment().0;
        for index in 0..251_u64 {
            observed.modules.push(ModuleIdentity {
                name: format!("module_{index:03}"),
                kind: "file".into(),
                path: format!(
                    "/fixture/venv/site-packages/module-{index:03}/{}",
                    "authenticated-source-identity/".repeat(4)
                ),
                bytes: 10 + index,
                sha256: format!("{index:064x}"),
                device: 1,
                inode: 100 + index,
                links: 1,
                modified_ns: 4,
                changed_ns: 5,
            });
        }
        observed
            .modules
            .sort_by(|left, right| left.name.cmp(&right.name));
        assert_eq!(observed.modules.len(), 254);
        let bytes = canonical_environment_bytes(&observed).expect("large environment");
        assert!(bytes.len() > MAX_METADATA_BYTES);
        assert!(bytes.len() < MAX_ENVIRONMENT_BYTES);
        observed
    }

    fn environment_non_module_len(environment: &ObservationEnvironment) -> usize {
        let mut value = serde_json::to_value(environment).expect("environment value");
        value
            .as_object_mut()
            .expect("environment object")
            .remove("modules")
            .expect("module member");
        canonical(&value).expect("canonical envelope").len()
    }

    fn pad_module_identity(module: &mut ModuleIdentity, target: usize) {
        loop {
            let length = serde_jcs::to_vec(&module).expect("canonical module").len();
            if length == target {
                break;
            }
            assert!(length < target, "module padding crossed its target");
            module.path.push('x');
        }
    }

    fn capture_arguments(root: &Path) -> CaptureArguments {
        use std::os::unix::fs::symlink;

        let database = root.join("annotation.db");
        let gtf = root.join("annotation.gtf.gz");
        let base_prefix = root.join("base");
        let base_bin = base_prefix.join("bin");
        let python = base_bin.join("python3");
        let prefix = root.join("venv");
        let launcher_bin = prefix.join("bin");
        let launcher = launcher_bin.join("python");
        let output_parent = root.join("output");
        fs::create_dir(&base_prefix).expect("base prefix");
        fs::create_dir(&base_bin).expect("base bin");
        fs::create_dir(&prefix).expect("venv prefix");
        fs::create_dir(&launcher_bin).expect("launcher bin");
        fs::create_dir(&output_parent).expect("output parent");
        fs::write(&database, b"fixture database").expect("database");
        fs::write(&gtf, b"fixture gtf").expect("GTF");
        fs::write(&python, b"fixture python").expect("Python");
        symlink(&python, &launcher).expect("launcher symlink");
        let config = format!(
            "home = {}\nimplementation = CPython\nversion_info = test-python\ninclude-system-site-packages = false\n",
            base_bin.display()
        );
        fs::write(prefix.join("pyvenv.cfg"), config.as_bytes()).expect("pyvenv config");
        let launcher_target = python.to_str().expect("UTF-8 fixture path").as_bytes();
        CaptureArguments {
            expected_database: identity(b"fixture database"),
            expected_gtf: identity(b"fixture gtf"),
            expected_python: Some(identity(b"fixture python")),
            expected_launcher_link: identity(launcher_target),
            expected_pyvenv_config: identity(config.as_bytes()),
            database,
            gtf,
            python,
            python_launcher: launcher,
            output_parent,
            environment_policy: environment().1,
        }
    }

    fn selected_environment(python_environment: &HeldPythonEnvironment) -> ObservationEnvironment {
        let mut observed = environment().0;
        let evidence = python_environment.evidence().expect("environment evidence");
        observed.executable = evidence.launcher;
        observed.prefix = evidence.prefix;
        observed.base_prefix = evidence.base_prefix;
        observed.base_executable = evidence.base_executable;
        observed
    }

    fn genes() -> Vec<ObservedGene> {
        [
            ("ENSG00000000001.1", "chr1", "+", 1, 4, vec![2, 4]),
            (
                "ENSG00000000002.3",
                "chr2",
                "+",
                10,
                20,
                vec![11, 12, 15, 18, 20],
            ),
            ("ENSG00000000003.1", "chr2", "+", 12, 18, vec![13, 14, 18]),
            ("ENSG00000000004.2", "chr2", "-", 11, 20, vec![12, 19]),
            ("ENSG00000000005.1", "chr2", "+", 14, 16, vec![]),
            ("ENSG00000228572.7", "chrX", "+", 100, 110, vec![101, 110]),
            (
                "ENSG00000228572.7_PAR_Y",
                "chrY",
                "+",
                200,
                210,
                vec![201, 210],
            ),
        ]
        .into_iter()
        .map(
            |(id, contig, strand, start, end, boundaries)| ObservedGene {
                kind: "gene".into(),
                id: id.into(),
                contig: contig.into(),
                strand: strand.into(),
                start,
                end,
                boundaries,
            },
        )
        .collect()
    }

    fn domain(contig: &str, begin: u32, end: u32, plus: &[&str], minus: &[&str]) -> ObservedDomain {
        ObservedDomain {
            kind: "domain".into(),
            contig: contig.into(),
            begin,
            end,
            plus: plus.iter().map(|value| (*value).into()).collect(),
            minus: minus.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn domains() -> Vec<ObservedDomain> {
        let g2 = "ENSG00000000002.3";
        let g3 = "ENSG00000000003.1";
        let g4 = "ENSG00000000004.2";
        let g5 = "ENSG00000000005.1";
        vec![
            domain("chr1", 2, 4, &["ENSG00000000001.1"], &[]),
            domain("chr2", 11, 11, &[g2], &[]),
            domain("chr2", 12, 12, &[g2], &[g4]),
            domain("chr2", 13, 14, &[g2, g3], &[g4]),
            domain("chr2", 15, 16, &[g2, g3, g5], &[g4]),
            domain("chr2", 17, 18, &[g2, g3], &[g4]),
            domain("chr2", 19, 20, &[g2], &[g4]),
            domain("chrX", 101, 110, &["ENSG00000228572.7"], &[]),
            domain("chrY", 201, 210, &["ENSG00000228572.7_PAR_Y"], &[]),
        ]
    }

    fn write_observation_with_environment(
        path: &Path,
        environment: &ObservationEnvironment,
    ) -> ObservationEnvironment {
        let genes = genes();
        let domains = domains();
        let mut bytes = canonical(environment).expect("environment");
        for gene in &genes {
            bytes.extend(canonical(gene).expect("gene"));
        }
        for domain in &domains {
            bytes.extend(canonical(domain).expect("domain"));
        }
        let mut environment_end = environment.clone();
        environment_end.kind = "environment_end".into();
        bytes.extend(canonical(&environment_end).expect("environment end"));
        bytes.extend(
            canonical(&ObservationSummary {
                kind: "summary".into(),
                genes: genes.len(),
                domains: domains.len(),
            })
            .expect("summary"),
        );
        fs::write(path, bytes).expect("write observation");
        environment.clone()
    }

    fn write_observation(path: &Path) -> ObservationEnvironment {
        write_observation_with_environment(path, &environment().0)
    }

    fn write_gtf(path: &Path) {
        let mut text = String::new();
        for gene in genes() {
            text.push_str(&format!(
                "{}\ttest\tgene\t{}\t{}\t.\t{}\t.\tgene_id \"{}\"; level 2;\n",
                gene.contig, gene.start, gene.end, gene.strand, gene.id
            ));
            let exons: Vec<(u32, u32)> = match gene.id.as_str() {
                "ENSG00000000001.1" => vec![(2, 4)],
                "ENSG00000000002.3" => vec![(11, 12), (15, 18), (20, 20)],
                "ENSG00000000003.1" => vec![(13, 14), (18, 18)],
                "ENSG00000000004.2" => vec![(12, 19)],
                "ENSG00000228572.7" => vec![(101, 110)],
                "ENSG00000228572.7_PAR_Y" => vec![(201, 210)],
                _ => Vec::new(),
            };
            for (start, end) in exons {
                text.push_str(&format!(
                    "{}\ttest\texon\t{}\t{}\t.\t{}\t.\tgene_id \"{}\"; exon_number 1; level 2; tag \"Ensembl_canonical\"; tag \"basic\";\n",
                    gene.contig, start, end, gene.strand, gene.id,
                ));
            }
        }
        let output = File::create(path).expect("create GTF");
        let mut encoder = GzEncoder::new(output, Compression::default());
        encoder.write_all(text.as_bytes()).expect("write GTF");
        encoder.finish().expect("finish GTF");
    }

    fn compatibility_evidence_with_count(count: usize) -> CompatibilityEvidence {
        let values = (1..=count)
            .map(|index| CompatibilityPoint {
                id: format!("M{index:02}-fixture"),
                contig: "chr2".into(),
                position: 13,
            })
            .collect::<Vec<_>>();
        let cases_jsonl = Identity {
            bytes: 1,
            sha256: digest(91),
        };
        let point_bytes = canonical(&CompatibilityPointSet {
            schema: "pangopup-mask-compatibility-points-v1",
            profile: MASK_PROFILE,
            cases_jsonl,
            points: &values,
        })
        .expect("compatibility points");
        CompatibilityEvidence {
            corpus: Identity {
                bytes: 1,
                sha256: digest(90),
            },
            points: identity(&point_bytes),
            values,
        }
    }

    fn compatibility_evidence() -> CompatibilityEvidence {
        compatibility_evidence_with_count(14)
    }

    fn synthetic_capture_with_environment(
        parent: &Path,
        observed_environment: ObservationEnvironment,
    ) -> PathBuf {
        synthetic_capture_with_environment_and_builder(
            parent,
            observed_environment,
            BUILDER_SOURCE_SHA256,
        )
    }

    fn synthetic_capture_with_environment_and_builder(
        parent: &Path,
        observed_environment: ObservationEnvironment,
        builder_source_sha256: &str,
    ) -> PathBuf {
        let temporary_gtf = parent.join("fixture.gtf.gz");
        write_gtf(&temporary_gtf);
        let gtf_bytes = fs::read(&temporary_gtf).expect("read GTF bytes");
        fs::remove_file(&temporary_gtf).expect("remove temporary GTF");
        let database_bytes = b"fixture database";
        let contract = OwnedCaptureContract {
            schema: CAPTURE_CONTRACT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            builder_source_sha256: builder_source_sha256.into(),
            helper: identity(OBSERVATION_HELPER.as_bytes()),
            database: identity(database_bytes),
            gtf: identity(&gtf_bytes),
            python: identity(b"fixture python"),
            python_environment: PythonEnvironmentIdentity {
                launcher: observed_environment.executable.clone(),
                prefix: observed_environment.prefix.clone(),
                base_prefix: observed_environment.base_prefix.clone(),
                base_executable: observed_environment.base_executable.clone(),
                launcher_link: identity(b"/fixture/base/bin/python"),
                pyvenv_config: identity(b"fixture pyvenv"),
            },
            environment: observed_environment.clone(),
        };
        let contract_bytes = canonical(&contract).expect("contract");
        validate_capture_contract_bytes(&contract, &observed_environment, &contract_bytes)
            .expect("bounded contract");
        let contract_id = identity(&contract_bytes).sha256;
        let stage = parent.join(format!("{STAGE_PREFIX}{contract_id}"));
        create_private_stage(parent, &stage).expect("create stage");
        create_private_directory(&stage.join("source")).expect("source");
        create_private_directory(&stage.join("capture")).expect("capture");
        write_synced(&stage.join("contract.json"), &contract_bytes, 0o400).expect("contract");
        write_synced(&stage.join(SNAPSHOT_DATABASE), database_bytes, 0o400).expect("database");
        write_synced(&stage.join(SNAPSHOT_GTF), &gtf_bytes, 0o400).expect("GTF");
        write_synced(
            &stage.join(SNAPSHOT_PYVENV_CONFIG),
            b"fixture pyvenv",
            0o400,
        )
        .expect("pyvenv config");
        let _ = write_observation_with_environment(
            &stage.join(OBSERVATION_MEMBER),
            &observed_environment,
        );
        let environment_bytes =
            canonical_environment_bytes(&observed_environment).expect("environment");
        write_synced(&stage.join(ENVIRONMENT_MEMBER), &environment_bytes, 0o400)
            .expect("environment");
        let mut inputs = BTreeMap::new();
        inputs.insert("database".into(), contract.database.clone());
        inputs.insert("gtf".into(), contract.gtf.clone());
        inputs.insert("helper".into(), contract.helper.clone());
        inputs.insert("python".into(), contract.python.clone());
        inputs.insert(
            "python_launcher_link".into(),
            contract.python_environment.launcher_link.clone(),
        );
        inputs.insert(
            "pyvenv_config".into(),
            contract.python_environment.pyvenv_config.clone(),
        );
        inputs.insert("contract".into(), identity(&contract_bytes));
        let mut outputs = BTreeMap::new();
        outputs.insert(
            "observation".into(),
            hash_file(&stage.join(OBSERVATION_MEMBER), MAX_OBSERVATION_BYTES)
                .expect("observation identity"),
        );
        outputs.insert("environment".into(), identity(&environment_bytes));
        seal_phase(
            &stage,
            CAPTURE_RECEIPT,
            PhaseReceipt {
                schema: PHASE_RECEIPT_SCHEMA.into(),
                profile: MASK_PROFILE.into(),
                contract_id,
                phase: Phase::Capture,
                builder_source_sha256: builder_source_sha256.into(),
                inputs,
                outputs,
                next_phase: Some(Phase::Prepare),
                reused_from: None,
            },
        )
        .expect("capture receipt");
        stage
    }

    fn synthetic_capture(parent: &Path) -> PathBuf {
        synthetic_capture_with_environment(parent, environment().0)
    }

    fn synthetic_failed_capture_with_builder(parent: &Path, builder: &str) -> PathBuf {
        let stage =
            synthetic_capture_with_environment_and_builder(parent, environment().0, builder);
        let lease = StageLease::open(&stage).expect("failed capture lease");
        preserve_failure_held(
            &lease,
            &stage_contract_id(&stage).expect("source contract id"),
            Phase::Prepare,
            &MaskBuildError::new("GTF", "GTF attribute quoting is invalid"),
        )
        .expect("preserve synthetic prepare failure");
        stage
    }

    fn capture_promotion_authorization(
        plan: &CapturePromotionPlan,
    ) -> CapturePromotionAuthorization {
        CapturePromotionAuthorization {
            schema: CAPTURE_PROMOTION_AUTHORIZATION_SCHEMA.into(),
            decision: "RUN-READY-CAPTURE-PROMOTION".into(),
            source_contract: plan.source_contract.clone(),
            source_builder_source_sha256: plan.source_builder_source_sha256.clone(),
            target_contract: plan.target_contract.clone(),
            target_builder_source_sha256: plan.target_builder_source_sha256.clone(),
            coordinator: "/root/coordinator".into(),
            reviewer: "/root/reviewer".into(),
            sealed_phases: vec![Phase::Capture],
            capture_receipt: plan.capture_receipt.clone(),
            failure_receipt: plan.failure_receipt.clone(),
        }
    }

    fn write_authorization(path: &Path, authorization: &CapturePromotionAuthorization) -> Identity {
        let bytes = canonical(authorization).expect("promotion authorization");
        fs::write(path, &bytes).expect("write promotion authorization");
        identity(&bytes)
    }

    fn promotion_stage(
        parent: &Path,
        plan: &CapturePromotionPlan,
        authorization_identity: &Identity,
    ) -> PathBuf {
        parent.join(format!(
            "{STAGE_PREFIX}{}-promotion-{}",
            plan.target_contract.sha256, authorization_identity.sha256
        ))
    }

    #[test]
    fn synthetic_observation_gtf_and_all_candidates_prepare_exactly() {
        let scratch = Scratch::new();
        let stage = synthetic_capture(&scratch.0);
        let compatibility = compatibility_evidence();
        let outcome = prepare_phase(&stage, &compatibility).expect("prepare miniature");
        assert_eq!(outcome.genes, 7);
        assert_eq!(outcome.domains, 9);
        assert_eq!(outcome.queries, 1_000);
        assert_eq!(outcome.candidates.len(), 3);
        let receipt: PhaseReceipt =
            parse_canonical(&fs::read(stage.join(PREPARE_RECEIPT)).expect("prepare receipt bytes"))
                .expect("prepare receipt");
        assert_eq!(
            receipt.inputs.get("compatibility_corpus"),
            Some(&compatibility.corpus)
        );
        assert_eq!(
            receipt.inputs.get("compatibility_points"),
            Some(&compatibility.points)
        );
        let inventory: Inventory =
            parse_canonical(&fs::read(stage.join(INVENTORY_MEMBER)).expect("inventory bytes"))
                .expect("inventory");
        assert_eq!(inventory.plus_genes, 6);
        assert_eq!(inventory.minus_genes, 1);
        assert_eq!(inventory.primary_contigs, 4);
        assert_eq!(inventory.maximum_boundaries_per_gene, 5);
        assert_eq!(inventory.distinct_stable_ids, 6);
        assert_eq!(inventory.stable_collisions, 1);
        assert_eq!(inventory.duplicate_exact_ids, 0);
        assert_eq!(
            fs::read(stage.join(CANONICAL_MEMBER))
                .expect("canonical stream")
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
            16,
            "seven genes plus nine complete ordered domains"
        );
        let manifest = read_performance(&stage.join(PERFORMANCE_MEMBER)).expect("manifest");
        let strata = manifest
            .queries
            .iter()
            .fold(BTreeMap::new(), |mut map, query| {
                *map.entry(query.stratum.clone()).or_insert(0_usize) += 1;
                map
            });
        assert_eq!(strata["single-gene"], 486);
        assert_eq!(strata["no-gene"], 100);
        assert_eq!(strata["same-strand-multi"], 100);
        assert_eq!(strata["opposite-strand-multi"], 100);
        assert_eq!(strata["par-pair"], 88);
        assert_eq!(strata["compatibility"], 14);
        assert_eq!(strata["extreme-cardinality"], 12);
        assert_eq!(manifest.strata.len(), 11);
        for stratum in &manifest.strata {
            assert_eq!(
                stratum.requested,
                stratum.distinct + stratum.repeated,
                "retained repetition accounting for {}",
                stratum.name
            );
            assert!(stratum.eligible >= u64::from(stratum.distinct));
        }
        for codec in MaskCandidateCodec::ALL {
            let reader =
                MaskCandidateReader::open(&stage.join(CANDIDATE_DIRECTORY).join(codec.filename()))
                    .expect("open candidate");
            reader.inspect_payload().expect("inspect candidate");
        }
    }

    #[test]
    fn gtf_attributes_accept_only_quoted_values_and_pinned_bare_decimals() {
        let attributes = parse_gtf_attributes(
            "gene_id \"ENSG00000000001.1\"; exon_number 12; level 2; tag \"basic\"; tag \"Ensembl_canonical\";",
        )
        .expect("GENCODE attribute grammar");
        assert_eq!(
            attributes.get("gene_id"),
            Some(&vec!["ENSG00000000001.1".into()])
        );
        assert_eq!(attributes.get("exon_number"), Some(&vec!["12".into()]));
        assert_eq!(attributes.get("level"), Some(&vec!["2".into()]));
        assert_eq!(
            attributes.get("tag"),
            Some(&vec!["basic".into(), "Ensembl_canonical".into()])
        );

        for invalid in [
            "",
            "gene_id \"ENSG00000000001.1\"",
            "gene_id \"\";",
            "level ;",
            "level -1;",
            "level 1.0;",
            "level 0;",
            "level 4;",
            "level 01;",
            "exon_number 0;",
            "exon_number 01;",
            "gene_id ENSG00000000001.1;",
            "tag basic;",
            "gene_id \"unterminated;",
            "gene_id terminated\";",
            "gene_id \"quoted\" garbage;",
            "gene_id \"bad\"quote\";",
            "gene_id \"valid\";; tag \"basic\";",
            "bad-key \"value\";",
            "1bad \"value\";",
            "gene_id \"valid\"; trailing-garbage;",
        ] {
            assert!(
                parse_gtf_attributes(invalid).is_err(),
                "invalid attribute grammar was accepted: {invalid:?}"
            );
        }
        let too_many = (0..=MAX_GTF_ATTRIBUTES)
            .map(|index| format!("tag \"value-{index}\";"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            parse_gtf_attributes(&too_many)
                .expect_err("attribute count bound")
                .code(),
            "RESOURCE"
        );
        let oversized = format!("tag \"{}\";", "x".repeat(MAX_GTF_ATTRIBUTE_VALUE_BYTES + 1));
        assert!(parse_gtf_attributes(&oversized).is_err());
    }

    #[test]
    fn compatibility_points_are_extracted_from_the_authenticated_held_corpus() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/pangolin-compat-v1")
            .canonicalize()
            .expect("compatibility fixture");
        let evidence = load_compatibility_points(&fixture).expect("authenticated points");
        assert_eq!(evidence.values.len(), 14);
        assert!(evidence.values[0].id.starts_with("M01-"));
        assert!(evidence.values[13].id.starts_with("M14-"));
        assert_eq!(evidence.corpus.bytes, 227_060);
        assert_eq!(
            evidence.corpus.sha256,
            "c077d400230fc7df83242d2737a850b2709299be990f521599b0e55735ff55e3"
        );
        assert_eq!(
            evidence.points.sha256,
            "2356eeaad935f3cbb572ab4fe333f7009b4f053c6069b89825725645bf813d32"
        );
    }

    fn scale_genes(count: usize) -> Vec<CanonicalMaskGene> {
        let contig = Grch38Contig::autosome(1).expect("chr1");
        (0..count)
            .map(|index| {
                let start = u32::try_from(index * 4 + 1).expect("small scale coordinate");
                CanonicalMaskGene::new(
                    GencodeGeneId::from_str(&format!("ENSG{:011}.1", index + 1))
                        .expect("scale gene identity"),
                    contig,
                    MaskStrand::Plus,
                    GenomicPosition::new(start).expect("scale start"),
                    GenomicPosition::new(start + 2).expect("scale end"),
                    u32::try_from(index).expect("scale rank"),
                    vec![GenomicPosition::new(start + 1).expect("scale boundary")],
                )
                .expect("scale gene")
            })
            .collect()
    }

    #[test]
    fn domain_oracle_work_scales_with_events_queries_and_returned_memberships() {
        let genes = scale_genes(4_096);
        let mut work = OracleWork::default();
        let domains = sweep_domains(&genes, &mut work).expect("sweep domains");
        assert_eq!(domains.len(), genes.len());
        let observation = Observation {
            environment: environment().0,
            genes: Vec::new(),
            domains,
        };
        let oracle = DomainOracle::new(&observation, &genes).expect("indexed oracle");
        let contig = Grch38Contig::autosome(1).expect("chr1");
        for domain in &observation.domains {
            let position = GenomicPosition::new(domain.begin).expect("domain witness");
            let value = oracle
                .query_counted(contig, position, &mut work)
                .expect("oracle query");
            assert_eq!(value.plus.len(), 1);
        }
        assert_eq!(work.event_updates, (genes.len() * 2) as u64);
        assert_eq!(work.emitted_memberships, genes.len() as u64);
        assert_eq!(work.returned_records, genes.len() as u64);
        assert!(work.binary_search_steps <= (genes.len() * 13) as u64);
        let old_full_scan_lower_bound = (genes.len() as u64) * (genes.len() as u64);
        let indexed_work = work.event_updates
            + work.emitted_memberships
            + work.binary_search_steps
            + work.returned_records;
        assert!(old_full_scan_lower_bound > indexed_work * 100);
    }

    #[test]
    fn domain_comparator_honors_bounded_cancellation() {
        let genes = scale_genes(4_096);
        let _guard = TestCancellation::after(2);
        assert_eq!(
            sweep_domains(&genes, &mut OracleWork::default())
                .expect_err("cancel comparator")
                .code(),
            "CANCELLED"
        );
    }

    #[test]
    fn observation_environment_order_and_plan_drift_fail_closed() {
        let scratch = Scratch::new();
        let path = scratch.0.join("observation.jsonl");
        let expected = write_observation(&path);
        let parsed = parse_observation(&path, &expected).expect("parse observation");
        assert_eq!(parsed.genes.len(), 7);
        let mut wrong = expected.clone();
        wrong.query_plan[0][3] = serde_json::Value::String("USING INDEX absent".into());
        assert_eq!(
            parse_observation(&path, &wrong)
                .expect_err("plan drift")
                .code(),
            "ENVIRONMENT"
        );

        let (environment, _) = environment();
        let mut bytes = canonical(&environment).expect("environment");
        for gene in genes() {
            bytes.extend(canonical(&gene).expect("gene"));
        }
        let mut reordered = domains();
        reordered.swap(0, 1);
        for domain in reordered {
            bytes.extend(canonical(&domain).expect("domain"));
        }
        let mut environment_end = environment.clone();
        environment_end.kind = "environment_end".into();
        bytes.extend(canonical(&environment_end).expect("environment end"));
        bytes.extend(
            canonical(&ObservationSummary {
                kind: "summary".into(),
                genes: 7,
                domains: 9,
            })
            .expect("summary"),
        );
        fs::write(&path, bytes).expect("rewrite observation");
        assert_eq!(
            parse_observation(&path, &expected)
                .expect_err("domain order")
                .code(),
            "OBSERVATION"
        );

        write_observation(&path);
        let missing_postflight = String::from_utf8(fs::read(&path).expect("observation bytes"))
            .expect("UTF-8 observation")
            .replace(
                "\"kind\":\"environment_end\"",
                "\"kind\":\"environment_lost\"",
            );
        fs::write(&path, missing_postflight).expect("remove postflight");
        assert_eq!(
            parse_observation(&path, &expected)
                .expect_err("postflight required")
                .code(),
            "OBSERVATION"
        );
    }

    #[test]
    fn helper_pumps_enforce_memory_total_and_line_bounds() {
        for (input, expected_exceeded) in [
            (MAX_ENVIRONMENT_BYTES - 1, false),
            (MAX_ENVIRONMENT_BYTES, false),
            (MAX_ENVIRONMENT_BYTES + 1, true),
        ] {
            let exceeded = Arc::new(AtomicBool::new(false));
            let retained = drain_bounded(
                io::Cursor::new(vec![b'x'; input]),
                MAX_ENVIRONMENT_BYTES,
                Arc::clone(&exceeded),
            )
            .expect("bounded drain");
            assert_eq!(retained.len(), input.min(MAX_ENVIRONMENT_BYTES));
            assert_eq!(exceeded.load(Ordering::SeqCst), expected_exceeded);
        }

        let scratch = Scratch::new();
        let output = File::create(scratch.0.join("observation")).expect("output");
        let exceeded = Arc::new(AtomicBool::new(false));
        let bytes = drain_observation(
            io::Cursor::new(vec![b'x'; MAX_LINE_BYTES + 1]),
            output,
            Arc::clone(&exceeded),
        )
        .expect("observation drain");
        assert_eq!(bytes, (MAX_LINE_BYTES + 1) as u64);
        assert!(exceeded.load(Ordering::SeqCst));
    }

    #[test]
    fn environment_schema_bounds_every_module_envelope_and_complete_payload() {
        let representative = representative_large_environment();
        let representative_bytes =
            canonical_environment_bytes(&representative).expect("representative environment");
        assert!(representative_bytes.len() > MAX_METADATA_BYTES);
        assert!(representative_bytes.len() < MAX_ENVIRONMENT_BYTES);
        assert!(representative.modules.iter().all(|module| {
            serde_jcs::to_vec(module).expect("canonical module").len() <= MAX_MODULE_IDENTITY_BYTES
        }));
        validate_environment_shape(&representative).expect("representative shape");

        let mut module_edge = environment().0;
        pad_module_identity(&mut module_edge.modules[1], MAX_MODULE_IDENTITY_BYTES);
        canonical_environment_bytes(&module_edge).expect("512-byte module identity");
        module_edge.modules[1].path.push('x');
        assert_eq!(
            canonical_environment_bytes(&module_edge)
                .expect_err("513-byte module identity")
                .code(),
            "RESOURCE"
        );

        let mut envelope_edge = environment().0;
        envelope_edge.region_sql.clear();
        let base = environment_non_module_len(&envelope_edge);
        envelope_edge
            .region_sql
            .push_str(&"s".repeat(MAX_METADATA_BYTES - base));
        assert_eq!(
            environment_non_module_len(&envelope_edge),
            MAX_METADATA_BYTES
        );
        canonical_environment_bytes(&envelope_edge).expect("64 KiB non-module envelope");
        envelope_edge.region_sql.push('s');
        assert_eq!(
            canonical_environment_bytes(&envelope_edge)
                .expect_err("oversized non-module envelope")
                .code(),
            "RESOURCE"
        );

        let mut total_overflow = environment().0;
        total_overflow.modules = (0..MAX_ENVIRONMENT_MODULES)
            .map(|index| {
                let mut module = ModuleIdentity {
                    name: format!("module_{index:03}"),
                    kind: "file".into(),
                    path: format!("/fixture/module-{index:03}/"),
                    bytes: 1,
                    sha256: format!("{index:064x}"),
                    device: 1,
                    inode: 1 + index as u64,
                    links: 1,
                    modified_ns: 1,
                    changed_ns: 1,
                };
                pad_module_identity(&mut module, MAX_MODULE_IDENTITY_BYTES);
                module
            })
            .collect();
        total_overflow.region_sql.clear();
        let base = environment_non_module_len(&total_overflow);
        total_overflow
            .region_sql
            .push_str(&"s".repeat(MAX_METADATA_BYTES - base));
        assert_eq!(
            environment_non_module_len(&total_overflow),
            MAX_METADATA_BYTES
        );
        assert!(total_overflow.modules.iter().all(|module| {
            serde_jcs::to_vec(module).expect("canonical module").len() == MAX_MODULE_IDENTITY_BYTES
        }));
        assert_eq!(
            canonical_environment_bytes(&total_overflow)
                .expect_err("oversized complete environment")
                .code(),
            "RESOURCE"
        );
    }

    #[test]
    fn helper_pins_real_sqlite_row_normalization_and_schema_serialization() {
        let control = b"[[7,8,null]]\ntable|features|features|";
        assert_eq!(identity(control).sha256, SQL_ROW_CONTROL_SHA256);
        assert!(OBSERVATION_HELPER.contains("connection.row_factory = sqlite3.Row"));
        assert!(OBSERVATION_HELPER.contains("type(source) is not sqlite3.Row"));
        assert!(OBSERVATION_HELPER.contains("[\"duplicate\", \"duplicate\", \"optional\"]"));
        assert!(OBSERVATION_HELPER.contains("row = list(source)"));
        assert!(
            OBSERVATION_HELPER.contains("schema_bytes = legacy_schema_digest_bytes(schema_rows)")
        );
        assert!(OBSERVATION_HELPER.contains("plan = canonical_sql_rows("));
        assert!(OBSERVATION_HELPER.contains("db.conn.execute(\"PRAGMA compile_options\")"));
        assert!(!OBSERVATION_HELPER.contains("json.dumps(schema_rows"));
        assert!(
            OBSERVATION_HELPER
                .find("if before.st_size < 0 or before.st_size > MAX_PYTHON_BYTES")
                .expect("module fstat bound")
                < OBSERVATION_HELPER
                    .find("with os.fdopen(os.dup(descriptor), \"rb\") as stream")
                    .expect("module hash read"),
            "oversized module files must fail from fstat before read/hash"
        );
        assert!(
            OBSERVATION_HELPER
                .find("if len(imported) >= MAX_ENVIRONMENT_MODULES")
                .expect("module count bound")
                < OBSERVATION_HELPER
                    .find("imported.append(module_identity(name, module))")
                    .expect("module authentication"),
            "module 513 must fail before its authentication/hash"
        );
        assert!(
            OBSERVATION_HELPER
                .contains("if len(canonical_json_bytes(identity)) > MAX_MODULE_IDENTITY_BYTES")
        );
        assert!(OBSERVATION_HELPER.contains(
            "if len(canonical_json_bytes(non_module)) + 1 > MAX_ENVIRONMENT_NON_MODULE_BYTES"
        ));
        assert!(
            OBSERVATION_HELPER
                .contains("if len(canonical_json_bytes(payload)) + 1 > MAX_ENVIRONMENT_BYTES")
        );

        let (observed, policy) = environment();
        let python_environment = PythonEnvironmentIdentity {
            launcher: observed.executable.clone(),
            prefix: observed.prefix.clone(),
            base_prefix: observed.base_prefix.clone(),
            base_executable: observed.base_executable.clone(),
            launcher_link: identity(b"fixture launcher"),
            pyvenv_config: identity(b"fixture config"),
        };
        validate_environment(&observed, &policy, &python_environment).expect("pinned row control");
        let mut drifted = observed;
        drifted.sql_row_control_sha256 = digest(42);
        assert_eq!(
            validate_environment(&drifted, &policy, &python_environment)
                .expect_err("row-control drift")
                .code(),
            "ENVIRONMENT"
        );
    }

    #[test]
    fn helper_failures_are_classified_without_retaining_diagnostics() {
        let typed = helper_result_error(
            false,
            b"PANGOPUP_HELPER_EXCEPTION:TypeError\n",
            false,
            "environment probe",
        )
        .expect("typed helper failure");
        assert_eq!(typed.code(), "PYTHON_EXCEPTION");
        assert_eq!(typed.message(), "environment probe raised TypeError");

        let hostile = helper_result_error(
            false,
            b"PANGOPUP_HELPER_EXCEPTION:TypeError /private/path\n",
            false,
            "environment probe",
        )
        .expect("hostile helper failure");
        assert_eq!(hostile.code(), "PYTHON_PROCESS");
        assert_eq!(hostile.message(), "environment probe process failed");
        assert!(!hostile.message().contains("private"));

        assert_eq!(
            helper_result_error(true, b"warning\n", true, "observation helper")
                .expect("unexpected diagnostics")
                .code(),
            "PYTHON_STDERR"
        );
        assert_eq!(
            helper_result_error(true, b"", false, "observation helper")
                .expect("missing output")
                .code(),
            "PYTHON_OUTPUT"
        );
        assert!(helper_result_error(true, b"", true, "observation helper").is_none());
    }

    #[test]
    fn authenticated_launcher_selects_the_expected_environment_without_python() {
        let scratch = Scratch::new();
        let arguments = capture_arguments(&scratch.0);
        let sources =
            authenticate_capture_preflight(&arguments, |_python, _database, python_environment| {
                Ok(selected_environment(python_environment))
            })
            .expect("authenticated synthetic environment");
        let evidence = sources
            .python_environment
            .evidence()
            .expect("environment evidence");
        assert_eq!(
            evidence.launcher,
            arguments.python_launcher.display().to_string()
        );
        assert_eq!(evidence.pyvenv_config, arguments.expected_pyvenv_config);
        assert_eq!(evidence.launcher_link, arguments.expected_launcher_link);
        assert_eq!(sources.environment.executable, evidence.launcher);
        assert_eq!(sources.environment.prefix, evidence.prefix);
        assert_eq!(sources.environment.base_prefix, evidence.base_prefix);
        assert_eq!(
            sources.environment.base_executable,
            evidence.base_executable
        );
    }

    #[test]
    #[ignore = "review-only pinned production environment probe; never part of normal gates"]
    fn review_only_pinned_environment_probe_crosses_rust_bound_without_a_stage() {
        fn required_path(name: &str) -> PathBuf {
            std::env::var_os(name)
                .map(PathBuf::from)
                .unwrap_or_else(|| panic!("{name} must name the pinned review input"))
        }

        let arguments = CaptureArguments {
            database: required_path("PANGOPUP_REVIEW_MASK_DATABASE"),
            gtf: required_path("PANGOPUP_REVIEW_MASK_GTF"),
            python: required_path("PANGOPUP_REVIEW_MASK_PYTHON"),
            python_launcher: required_path("PANGOPUP_REVIEW_MASK_PYTHON_LAUNCHER"),
            output_parent: PathBuf::from("/review-only-no-stage-is-created"),
            expected_database: Identity {
                bytes: DATABASE_BYTES,
                sha256: DATABASE_SHA256.into(),
            },
            expected_gtf: Identity {
                bytes: GTF_BYTES,
                sha256: GTF_SHA256.into(),
            },
            expected_python: Some(Identity {
                bytes: 34_679_464,
                sha256: "c243a3ad6dc86fcde244245aca621adee9766759c7524ca89f1b3a44ff4fdc24".into(),
            }),
            expected_launcher_link: Identity {
                bytes: 79,
                sha256: "b407404b75f49e4f39686a8a060bdcbadae49e5c5f1ecd5d6e593f9468bc8ffe".into(),
            },
            expected_pyvenv_config: Identity {
                bytes: 171,
                sha256: "b39b62bae0935628201c24541bebd3011ff5527b55070543a4c565542d8b2ba9".into(),
            },
            environment_policy: EnvironmentPolicy::production(),
        };
        let sources =
            authenticate_capture_preflight(&arguments, |python, database, python_environment| {
                probe_observation_environment(python, database, python_environment)
            })
            .expect("pinned environment preflight");
        let environment = sources.environment.clone();
        let environment_bytes =
            canonical_environment_bytes(&environment).expect("bounded environment evidence");
        assert_eq!(environment_bytes.len(), 79_641);
        assert_eq!(environment.modules.len(), 254);
        assert_eq!(environment.sql_row_control_sha256, SQL_ROW_CONTROL_SHA256);
        assert_eq!(environment.schema_sha256, SCHEMA_SHA256);
        assert!(environment.query_plan.iter().flatten().any(|value| {
            value
                .as_str()
                .is_some_and(|text| text.contains("USING INDEX seqidstartend"))
        }));

        let prepared =
            prepare_capture_contract(&arguments, sources).expect("bounded final contract");
        assert!(prepared.contract_bytes.len() > MAX_METADATA_BYTES);
        assert!(prepared.contract_bytes.len() <= MAX_CAPTURE_CONTRACT_BYTES);
        assert_eq!(
            identity(&prepared.contract_bytes).sha256,
            prepared.contract_id
        );
        let contract: OwnedCaptureContract =
            parse_canonical(&prepared.contract_bytes).expect("canonical final contract");
        assert_eq!(contract.environment, environment);
        eprintln!(
            "environment_bytes={} modules={} contract_bytes={} contract_id={}",
            environment_bytes.len(),
            environment.modules.len(),
            prepared.contract_bytes.len(),
            prepared.contract_id
        );
        assert!(
            !arguments.output_parent.exists(),
            "review-only preflight must not create a stage or output parent"
        );
    }

    #[test]
    fn missing_gffutils_preflight_is_preserved_once_without_a_scan_stage() {
        let scratch = Scratch::new();
        let arguments = capture_arguments(&scratch.0);
        let output_parent =
            open_absolute_directory(&arguments.output_parent).expect("output parent");
        let expected_python = arguments.expected_python.as_ref().expect("Python identity");
        let contract = capture_preflight_contract(&arguments, expected_python);
        let preflight_id = identity(&canonical(&contract).expect("preflight contract")).sha256;
        let error = capture_preflight_or_preserve(
            &arguments,
            &output_parent,
            &preflight_id,
            contract.clone(),
            |_python, _database, _python_environment| {
                Err(MaskBuildError::new("PYTHON", "environment probe failed"))
            },
        )
        .err()
        .expect("bare environment must fail");
        assert_eq!(error.code(), "PYTHON");
        let stage = arguments
            .output_parent
            .join(format!("{PREFLIGHT_FAILURE_PREFIX}{preflight_id}"));
        let metadata = fs::metadata(&stage).expect("preflight stage");
        assert_eq!(metadata.mode() & 0o777, 0o700);
        let members = fs::read_dir(&stage)
            .expect("preflight members")
            .collect::<Result<Vec<_>, _>>()
            .expect("preflight entries");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].file_name(), OsStr::new(FAILURE_RECEIPT));
        let receipt_path = stage.join(FAILURE_RECEIPT);
        assert_eq!(
            fs::metadata(&receipt_path).expect("receipt mode").mode() & 0o777,
            0o400
        );
        let receipt_bytes = fs::read(&receipt_path).expect("failure receipt");
        let receipt: PreflightFailureReceipt =
            parse_canonical(&receipt_bytes).expect("canonical failure receipt");
        assert_eq!(receipt.schema, PREFLIGHT_FAILURE_SCHEMA);
        assert_eq!(receipt.preflight_id, preflight_id);
        assert_eq!(receipt.contract, contract);
        assert_eq!(receipt.code, "PYTHON");
        assert!(receipt.sealed_phases.is_empty());
        assert!(
            !String::from_utf8(receipt_bytes.clone())
                .expect("receipt UTF-8")
                .contains(scratch.0.to_str().expect("scratch path")),
            "sanitized preflight evidence must not expose local paths"
        );
        assert!(!stage.join("source").exists());
        assert!(!stage.join("capture").exists());
        assert!(!stage.join(CAPTURE_RECEIPT).exists());

        let second = capture_preflight_or_preserve(
            &arguments,
            &output_parent,
            &preflight_id,
            capture_preflight_contract(&arguments, expected_python),
            |_python, _database, _python_environment| {
                Err(MaskBuildError::new("PYTHON", "environment probe failed"))
            },
        )
        .err()
        .expect("automatic retry must be refused");
        assert_eq!(second.code(), "FAILURE_RECEIPT");
        assert_eq!(
            fs::read(receipt_path).expect("preserved receipt"),
            receipt_bytes
        );
    }

    #[test]
    fn final_contract_resource_failure_remains_inside_preflight_preservation() {
        let scratch = Scratch::new();
        let arguments = capture_arguments(&scratch.0);
        let output_parent =
            open_absolute_directory(&arguments.output_parent).expect("output parent");
        let expected_python = arguments.expected_python.as_ref().expect("Python identity");
        let contract = capture_preflight_contract(&arguments, expected_python);
        let preflight_id = identity(&canonical(&contract).expect("preflight contract")).sha256;
        let error = preserve_capture_preflight_result(
            &output_parent,
            &preflight_id,
            contract,
            Err(MaskBuildError::new(
                "RESOURCE",
                "capture contract exceeds its byte bound",
            )),
        )
        .err()
        .expect("final contract validation must fail");
        assert_eq!(error.code(), "RESOURCE");
        let stage = arguments
            .output_parent
            .join(format!("{PREFLIGHT_FAILURE_PREFIX}{preflight_id}"));
        let members = fs::read_dir(&stage)
            .expect("failure-only stage")
            .collect::<Result<Vec<_>, _>>()
            .expect("failure-only members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].file_name(), OsStr::new(FAILURE_RECEIPT));
        let receipt: PreflightFailureReceipt = parse_canonical(
            &fs::read(stage.join(FAILURE_RECEIPT)).expect("preflight failure receipt"),
        )
        .expect("canonical preflight failure receipt");
        assert_eq!(receipt.code, "RESOURCE");
        assert!(receipt.sealed_phases.is_empty());
        assert!(!stage.join("contract.json").exists());
        assert!(!stage.join("source").exists());
        assert!(!stage.join("capture").exists());
    }

    #[test]
    fn launcher_and_hardlink_descriptor_drift_fail_closed() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new();
        let arguments = capture_arguments(&scratch.0);
        let launcher = arguments.python_launcher.clone();
        let python = arguments.python.clone();
        let error = authenticate_capture_preflight(
            &arguments,
            move |_python, _database, python_environment| {
                let observed = selected_environment(python_environment);
                fs::remove_file(&launcher).expect("remove authenticated launcher");
                symlink(&python, &launcher).expect("replace launcher with same target");
                Ok(observed)
            },
        )
        .err()
        .expect("launcher descriptor replacement");
        assert_eq!(error.code(), "SOURCE_MUTATION");

        let (mut observed, _) = environment();
        validate_environment_shape(&observed).expect("uv-style hardlink is accepted");
        let expected = observed.clone();
        let module = observed
            .modules
            .iter_mut()
            .find(|module| module.name == "gffutils")
            .expect("gffutils module");
        assert_eq!(module.links, 3);
        module.links = 2;
        assert_eq!(
            validate_exact_environment(&observed, &expected, "environment")
                .expect_err("hardlink descriptor drift")
                .code(),
            "ENVIRONMENT"
        );
    }

    #[test]
    fn held_stage_rejects_substitution_and_publishes_no_replace() {
        let scratch = Scratch::new();
        let stage = scratch.0.join("stage");
        create_private_directory(&stage).expect("stage");
        let mut lease = StageLease::open(&stage).expect("lease");
        let destination = scratch.0.join("published");
        fs::create_dir(&destination).expect("existing destination");
        assert_eq!(
            lease
                .publish(OsStr::new("published"))
                .expect_err("no replace")
                .code(),
            "PUBLICATION"
        );
        assert!(stage.is_dir());

        fs::remove_dir(&destination).expect("remove destination");
        lease.publish(OsStr::new("published")).expect("publish");
        assert!(!stage.exists());
        assert!(destination.is_dir());

        let substituted = scratch.0.join("substituted");
        create_private_directory(&substituted).expect("second stage");
        let lease = StageLease::open(&substituted).expect("second lease");
        let moved = scratch.0.join("moved");
        fs::rename(&substituted, &moved).expect("move held stage");
        create_private_directory(&substituted).expect("decoy stage");
        assert_eq!(
            lease.verify_current().expect_err("substitution").code(),
            "STAGE_LOCATION"
        );
        lease
            .write_member("held.txt", b"held", 0o400)
            .expect("write through held descriptor");
        assert_eq!(
            fs::read(moved.join("held.txt")).expect("held bytes"),
            b"held"
        );
        assert!(!substituted.join("held.txt").exists());
    }

    #[test]
    fn cancellation_interrupts_copy_seal_and_prepublication_without_false_success() {
        let scratch = Scratch::new();
        let source = scratch.0.join("large-source");
        let source_bytes = vec![b'x'; 3 * 64 * 1024];
        fs::write(&source, &source_bytes).expect("large source");
        let mut held = open_held(&source, source_bytes.len() as u64).expect("held source");
        let copied = scratch.0.join("copied");
        {
            let _guard = TestCancellation::after(1);
            assert_eq!(
                copy_held_authenticated(&mut held, &copied, &identity(&source_bytes))
                    .expect_err("cancel copy")
                    .code(),
                "CANCELLED"
            );
        }
        assert!(fs::metadata(&copied).expect("partial copy").len() < source_bytes.len() as u64);

        let receipt_stage = scratch.0.join("receipt-stage");
        create_private_directory(&receipt_stage).expect("receipt stage");
        let receipt = PhaseReceipt {
            schema: PHASE_RECEIPT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            contract_id: digest(1),
            phase: Phase::Capture,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            next_phase: Some(Phase::Prepare),
            reused_from: None,
        };
        {
            let _guard = TestCancellation::after(0);
            assert_eq!(
                seal_phase(&receipt_stage, CAPTURE_RECEIPT, receipt)
                    .expect_err("cancel seal")
                    .code(),
                "CANCELLED"
            );
        }
        assert!(!receipt_stage.join(CAPTURE_RECEIPT).exists());

        let publication_stage = scratch.0.join("publication-stage");
        create_private_directory(&publication_stage).expect("publication stage");
        let mut lease = StageLease::open(&publication_stage).expect("publication lease");
        {
            let _guard = TestCancellation::after(0);
            assert_eq!(
                lease
                    .publish(OsStr::new("published-cancelled"))
                    .expect_err("cancel publication")
                    .code(),
                "CANCELLED"
            );
        }
        assert!(publication_stage.is_dir());
        assert!(!scratch.0.join("published-cancelled").exists());
    }

    #[test]
    fn publication_rollback_sync_failure_is_reported_as_durability_uncertain() {
        let scratch = Scratch::new();
        let stage = scratch.0.join("rollback-stage");
        create_private_directory(&stage).expect("rollback stage");
        let mut lease = StageLease::open(&stage).expect("rollback lease");
        let mut calls = 0_u8;
        let error = lease
            .publish_with_parent_sync(OsStr::new("rollback-published"), |_| {
                calls += 1;
                Err(io::Error::other("injected parent sync failure"))
            })
            .expect_err("parent sync and rollback sync fail");
        assert_eq!(calls, 2);
        assert_eq!(error.code(), "DURABILITY_UNCERTAIN");
        assert!(stage.is_dir());
        assert!(!scratch.0.join("rollback-published").exists());
    }

    #[test]
    fn held_sources_reject_symlinks_links_sidecars_and_mutation() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        fs::write(&source, b"source bytes").expect("source");
        let linked = scratch.0.join("linked");
        fs::hard_link(&source, &linked).expect("hard link");
        assert_eq!(
            open_held(&source, 100).expect_err("linked").code(),
            "SOURCE"
        );
        fs::remove_file(&linked).expect("remove link");
        let alias = scratch.0.join("alias");
        symlink(&source, &alias).expect("symlink");
        assert_eq!(
            open_held(&alias, 100).expect_err("symlink").code(),
            "SOURCE"
        );
        let mut held = open_held(&source, 100).expect("held");
        let before = authenticate_held(&mut held).expect("identity");
        assert_eq!(before.bytes, 12);
        fs::write(&source, b"changed data").expect("mutate");
        assert_eq!(
            verify_held(&held).expect_err("mutation").code(),
            "SOURCE_MUTATION"
        );

        let database = scratch.0.join("annotation.db");
        fs::write(&database, b"db").expect("database");
        fs::write(scratch.0.join("annotation.db-wal"), b"wal").expect("sidecar");
        assert_eq!(
            reject_database_sidecars(&database)
                .expect_err("sidecar")
                .code(),
            "DATABASE_SIDECAR"
        );
    }

    fn measurement(codec: MaskCandidateCodec, p50: u64, p95: u64) -> CandidateMeasurement {
        let rounds = BENCHMARK_PERMUTATIONS
            .iter()
            .enumerate()
            .map(|(round, permutation)| RoundMeasurement {
                round: round as u8,
                schedule_position: permutation
                    .iter()
                    .position(|value| *value == codec)
                    .expect("codec position") as u8,
                p50_ns: p50,
                p95_ns: p95,
                open_ns: 10,
                open_peak_heap_bytes: 100,
                warmed_allocation_calls: 0,
                warmed_allocation_bytes: 0,
                maximum_rss_bytes: 1_000,
                minor_faults: 0,
                major_faults: 0,
            })
            .collect();
        CandidateMeasurement {
            codec,
            member: Identity {
                bytes: 1_000,
                sha256: digest(codec.code()),
            },
            pinned_zstandard_bytes: 500,
            pinned_zstandard: PINNED_MASK_ZSTANDARD.into(),
            semantic_certified: true,
            corruption_controls_passed: true,
            allocation_contract_passed: true,
            page_trace_sha256: digest(codec.code() + 10),
            metadata_pages: 1,
            median_payload_pages: 2,
            p95_payload_pages: 3,
            headline_p50_ns: p50,
            headline_p95_ns: p95,
            rounds,
        }
    }

    fn fixture_report(input: &BenchmarkRunInput) -> Result<MaskBenchmarkReport, MaskBuildError> {
        let candidates = MaskCandidateCodec::ALL
            .into_iter()
            .map(|codec| {
                let mut value = measurement(codec, 100 + u64::from(codec.code()), 200);
                value.member = input
                    .candidates
                    .iter()
                    .find(|candidate| candidate.codec == codec)
                    .expect("candidate")
                    .identity
                    .clone();
                value
            })
            .collect::<Vec<_>>();
        let selection = evaluate_mask_candidates(&candidates)?;
        Ok(MaskBenchmarkReport {
            schema: REPORT_SCHEMA.into(),
            profile: MASK_PROFILE.into(),
            contract_id: input.contract_id.clone(),
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            performance_manifest: input.performance_identity.clone(),
            method: BenchmarkMethod::ticket_012(),
            host: BenchmarkHost {
                selected_cpu: 0,
                allowed_cpu_count_before_pin: 1,
                cpu_model: "fixture".into(),
                kernel: "fixture".into(),
                governor: "fixture".into(),
                power_state: "fixture".into(),
                rustc: "fixture".into(),
                target: "fixture".into(),
                build_profile: "release".into(),
                executable: Identity {
                    bytes: 1,
                    sha256: digest(42),
                },
                logical_page_bytes: 4_096,
            },
            resources: BenchmarkResources {
                maximum_rss_bytes: 1,
                minor_faults: 0,
                major_faults: 0,
            },
            candidates,
            selection,
        })
    }

    #[test]
    fn selector_is_ordered_transitive_and_has_a_closed_exact_tie() {
        let mut candidates = vec![
            measurement(MaskCandidateCodec::IntervalTree, 100, 100),
            measurement(MaskCandidateCodec::Domains, 96, 104),
            measurement(MaskCandidateCodec::BinnedPostings, 50, 106),
        ];
        candidates[0].median_payload_pages = 3;
        candidates[1].median_payload_pages = 2;
        let decision = evaluate_mask_candidates(&candidates).expect("selection");
        assert_eq!(decision.selected, MaskCandidateCodec::Domains);
        assert_eq!(decision.steps[0].survivors.len(), 2);
        assert_eq!(decision.steps[1].survivors.len(), 2);

        for candidate in &mut candidates {
            *candidate = measurement(candidate.codec, 100, 100);
        }
        assert_eq!(
            evaluate_mask_candidates(&candidates)
                .expect("exact tie")
                .selected,
            MaskCandidateCodec::Domains
        );
        candidates[0].rounds[0].warmed_allocation_calls = 1;
        assert_eq!(
            evaluate_mask_candidates(&candidates)
                .expect_err("allocation evidence")
                .code(),
            "BENCHMARK_EVIDENCE"
        );
    }

    #[test]
    fn benchmark_phase_validates_report_and_publishes_the_held_stage() {
        let scratch = Scratch::new();
        let stage = synthetic_capture(&scratch.0);
        prepare_phase(&stage, &compatibility_evidence()).expect("prepare");
        let contract_id = stage_contract_id(&stage).expect("contract");
        let outcome = benchmark_phase(&stage, fixture_report).expect("benchmark lifecycle");
        assert!(outcome.published);
        assert_eq!(outcome.contract_id, contract_id);
        assert!(!stage.exists());
        let published = scratch.0.join(&contract_id);
        let inspected = inspect_phase(&published).expect("inspect publication");
        assert_eq!(
            inspected.sealed_phases,
            vec![Phase::Capture, Phase::Prepare, Phase::Benchmark]
        );
        assert!(!inspected.failed);
    }

    #[test]
    fn phase_inspection_rejects_missing_extra_and_cross_phase_identity_drift() {
        let scratch = Scratch::new();
        let capture_parent = scratch.0.join("capture-parent");
        fs::create_dir(&capture_parent).expect("capture parent");
        let capture_stage = synthetic_capture(&capture_parent);
        let capture_bytes = fs::read(capture_stage.join(CAPTURE_RECEIPT)).expect("capture receipt");
        let capture: PhaseReceipt = parse_canonical(&capture_bytes).expect("capture receipt");
        validate_phase_receipt(
            &capture,
            &stage_contract_id(&capture_stage).expect("capture contract"),
            Phase::Capture,
            Some(Phase::Prepare),
        )
        .expect("exact capture receipt");
        let mut extra = capture.clone();
        extra.inputs.insert(
            "unexpected".into(),
            Identity {
                bytes: 1,
                sha256: digest(55),
            },
        );
        assert_eq!(
            validate_phase_receipt(
                &extra,
                &stage_contract_id(&capture_stage).expect("capture contract"),
                Phase::Capture,
                Some(Phase::Prepare),
            )
            .expect_err("extra receipt input")
            .code(),
            "RECEIPT"
        );
        let mut missing = capture;
        missing.outputs.remove("observation");
        assert_eq!(
            validate_phase_receipt(
                &missing,
                &stage_contract_id(&capture_stage).expect("capture contract"),
                Phase::Capture,
                Some(Phase::Prepare),
            )
            .expect_err("missing receipt output")
            .code(),
            "RECEIPT"
        );

        let prepare_parent = scratch.0.join("prepare-parent");
        fs::create_dir(&prepare_parent).expect("prepare parent");
        let prepare_stage = synthetic_capture(&prepare_parent);
        prepare_phase(&prepare_stage, &compatibility_evidence()).expect("prepare");
        let prepare_path = prepare_stage.join(PREPARE_RECEIPT);
        let mut prepare: PhaseReceipt =
            parse_canonical(&fs::read(&prepare_path).expect("prepare receipt"))
                .expect("prepare receipt");
        prepare.inputs.insert(
            "gtf".into(),
            Identity {
                bytes: 1,
                sha256: digest(56),
            },
        );
        replace_canonical(&prepare_path, &prepare);
        assert_eq!(
            inspect_phase(&prepare_stage)
                .expect_err("cross-phase GTF drift")
                .code(),
            "RECEIPT"
        );
    }

    #[test]
    fn benchmark_inspection_reauthenticates_every_input_identity() {
        let scratch = Scratch::new();
        let stage = synthetic_capture(&scratch.0);
        prepare_phase(&stage, &compatibility_evidence()).expect("prepare");
        let outcome = benchmark_phase(&stage, fixture_report).expect("benchmark");
        let published = scratch.0.join(outcome.contract_id);
        let receipt_path = published.join(BENCHMARK_RECEIPT);
        let mut receipt: PhaseReceipt =
            parse_canonical(&fs::read(&receipt_path).expect("benchmark receipt"))
                .expect("benchmark receipt");
        receipt.inputs.insert(
            "performance".into(),
            Identity {
                bytes: 1,
                sha256: digest(57),
            },
        );
        replace_canonical(&receipt_path, &receipt);
        assert_eq!(
            inspect_phase(&published)
                .expect_err("benchmark input drift")
                .code(),
            "RECEIPT"
        );
    }

    #[test]
    fn failure_receipt_preserves_existing_sealed_phase_and_forbids_retry() {
        let scratch = Scratch::new();
        let stage = scratch.0.join("stage");
        create_private_directory(&stage).expect("stage");
        fs::write(stage.join(CAPTURE_RECEIPT), b"sealed").expect("capture marker");
        let error = MaskBuildError::new("INJECTED", "bounded failure");
        let lease = StageLease::open(&stage).expect("held stage");
        preserve_failure_held(&lease, &digest(7), Phase::Prepare, &error).expect("preserve");
        assert_eq!(
            fs::read(stage.join(CAPTURE_RECEIPT)).expect("capture remains"),
            b"sealed"
        );
        let failure: FailureReceipt =
            parse_canonical(&fs::read(stage.join(FAILURE_RECEIPT)).expect("failure receipt"))
                .expect("parse failure");
        assert_eq!(failure.sealed_phases, vec![Phase::Capture]);
        assert_eq!(
            preserve_failure_held(&lease, &digest(7), Phase::Prepare, &error)
                .expect_err("retry forbidden")
                .code(),
            "FAILURE_RECEIPT"
        );
    }

    #[test]
    fn authorized_reuse_copies_only_sealed_phases_into_an_absent_stage() {
        let scratch = Scratch::new();
        let prior_parent = scratch.0.join("prior");
        let reuse_parent = scratch.0.join("reuse");
        fs::create_dir(&prior_parent).expect("prior parent");
        fs::create_dir(&reuse_parent).expect("reuse parent");
        let prior = synthetic_capture(&prior_parent);
        assert_eq!(
            prepare_phase(&prior, &compatibility_evidence_with_count(0))
                .expect_err("injected prepare failure")
                .code(),
            "COMPATIBILITY"
        );
        let inspected = inspect_phase(&prior).expect("inspect failed stage");
        assert_eq!(inspected.sealed_phases, vec![Phase::Capture]);
        let capture_receipt = hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("capture receipt identity");
        let authorization = ReuseAuthorization {
            schema: "pangopup-mask-reuse-authorization-v1".into(),
            decision: "RUN-READY-REUSE".into(),
            contract_id: inspected.contract_id,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            coordinator: "/root".into(),
            reviewer: "/root/reviewer".into(),
            sealed_phases: vec![Phase::Capture],
            capture_receipt,
            prepare_receipt: None,
            benchmark_receipt: None,
        };
        let authorization_path = scratch.0.join("authorization.json");
        let authorization_bytes = canonical(&authorization).expect("authorization");
        let authorization_identity = identity(&authorization_bytes);
        fs::write(&authorization_path, &authorization_bytes).expect("write authorization");
        let outcome = reuse_sealed_phases(&prior, &reuse_parent, &authorization_path)
            .expect("authorized reuse");
        assert_eq!(outcome.sealed_phases, vec![Phase::Capture]);
        assert!(!outcome.published);
        let reused = reuse_parent.join(format!(
            "{STAGE_PREFIX}{}-reuse-{}",
            outcome.contract_id, authorization_identity.sha256
        ));
        assert!(!reused.join(FAILURE_RECEIPT).exists());
        assert!(!reused.join("prepare").exists());
        assert_eq!(
            hash_file(
                &reused.join(REUSE_AUTHORIZATION_MEMBER),
                MAX_METADATA_BYTES as u64
            )
            .expect("copied authorization"),
            authorization_identity
        );
        let receipt: PhaseReceipt =
            parse_canonical(&fs::read(reused.join(CAPTURE_RECEIPT)).expect("reused receipt"))
                .expect("parse reused receipt");
        assert_eq!(
            receipt.reused_from,
            Some(authorization.capture_receipt.sha256.clone())
        );
        assert!(receipt.inputs.contains_key("reuse_authorization"));
        let staged_authorization = reused.join(REUSE_AUTHORIZATION_MEMBER);
        let mut changed_authorization = authorization.clone();
        changed_authorization.coordinator = "/root/changed".into();
        replace_canonical(&staged_authorization, &changed_authorization);
        assert_eq!(
            inspect_phase(&reused)
                .expect_err("changed staged authorization")
                .code(),
            "RECEIPT"
        );
        replace_canonical(&staged_authorization, &authorization);
        inspect_phase(&reused).expect("restored reused capture");
        prepare_phase(&reused, &compatibility_evidence()).expect("prepare reused capture");
    }

    #[test]
    fn capture_promotion_changes_only_builder_provenance_and_drops_unsealed_work() {
        let scratch = Scratch::new();
        let prior_parent = scratch.0.join("prior-promotion");
        let target_parent = scratch.0.join("target-promotion");
        fs::create_dir(&prior_parent).expect("prior parent");
        fs::create_dir(&target_parent).expect("target parent");
        let old_builder = digest(70);
        let prior = synthetic_failed_capture_with_builder(&prior_parent, &old_builder);
        create_private_directory(&prior.join("prepare")).expect("partial prepare");
        create_private_directory(&prior.join(CANDIDATE_DIRECTORY)).expect("partial candidates");
        fs::write(prior.join("prepare/unsealed.txt"), b"never copy me").expect("partial output");

        assert_eq!(
            inspect_phase(&prior)
                .expect_err("ordinary current-builder inspect must reject old builder")
                .code(),
            "CONTRACT"
        );
        let ordinary_authorization = ReuseAuthorization {
            schema: "pangopup-mask-reuse-authorization-v1".into(),
            decision: "RUN-READY-REUSE".into(),
            contract_id: stage_contract_id(&prior).expect("old contract id"),
            builder_source_sha256: old_builder.clone(),
            coordinator: "/root/coordinator".into(),
            reviewer: "/root/reviewer".into(),
            sealed_phases: vec![Phase::Capture],
            capture_receipt: hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
                .expect("capture receipt"),
            prepare_receipt: None,
            benchmark_receipt: None,
        };
        let ordinary_path = scratch.0.join("ordinary-reuse.json");
        fs::write(
            &ordinary_path,
            canonical(&ordinary_authorization).expect("ordinary authorization"),
        )
        .expect("write ordinary authorization");
        assert_eq!(
            reuse_sealed_phases(&prior, &target_parent, &ordinary_path)
                .expect_err("ordinary reuse must remain current-builder strict")
                .code(),
            "CONTRACT"
        );

        let before_contract = hash_file(
            &prior.join("contract.json"),
            MAX_CAPTURE_CONTRACT_BYTES as u64,
        )
        .expect("old contract");
        let before_capture = hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("old capture");
        let before_failure = hash_file(&prior.join(FAILURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("old failure");
        let material = authenticate_capture_promotion_source(&prior, &old_builder)
            .expect("authenticated promotion material");
        let plan = plan_capture_promotion(&prior, &old_builder).expect("promotion plan");
        assert_eq!(plan, material.plan);
        let source_contract: OwnedCaptureContract =
            parse_canonical(&fs::read(prior.join("contract.json")).expect("source contract bytes"))
                .expect("source contract");
        let target_contract: OwnedCaptureContract =
            parse_canonical(&material.target_contract_bytes).expect("target contract");
        assert!(capture_contract_differs_only_by_builder(
            &source_contract,
            &target_contract,
            &old_builder,
            BUILDER_SOURCE_SHA256,
        ));

        let authorization = capture_promotion_authorization(&plan);
        let authorization_path = scratch.0.join("promotion.json");
        let authorization_identity = write_authorization(&authorization_path, &authorization);
        let outcome = promote_sealed_capture(&prior, &target_parent, &authorization_path)
            .expect("promote sealed capture");
        assert_eq!(outcome.source_contract_id, plan.source_contract.sha256);
        assert_eq!(outcome.target_contract_id, plan.target_contract.sha256);
        assert_eq!(outcome.sealed_phases, vec![Phase::Capture]);
        assert!(!outcome.published);
        let promoted = promotion_stage(&target_parent, &plan, &authorization_identity);
        assert_eq!(
            stage_contract_id(&promoted).expect("promotion suffix"),
            outcome.target_contract_id
        );
        assert!(
            stage_contract_id(&target_parent.join(format!(
                "{STAGE_PREFIX}{}-promotion-not-a-digest",
                outcome.target_contract_id
            )))
            .is_err()
        );
        assert!(!promoted.join(FAILURE_RECEIPT).exists());
        assert!(!promoted.join("prepare").exists());
        assert!(!promoted.join("prepare/unsealed.txt").exists());
        assert!(prior.join("prepare/unsealed.txt").is_file());
        assert_eq!(
            hash_file(
                &prior.join("contract.json"),
                MAX_CAPTURE_CONTRACT_BYTES as u64
            )
            .expect("unchanged contract"),
            before_contract
        );
        assert_eq!(
            hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
                .expect("unchanged capture"),
            before_capture
        );
        assert_eq!(
            hash_file(&prior.join(FAILURE_RECEIPT), MAX_METADATA_BYTES as u64)
                .expect("unchanged failure"),
            before_failure
        );
        let inspected = inspect_phase(&promoted).expect("inspect promoted capture");
        assert_eq!(inspected.sealed_phases, vec![Phase::Capture]);
        assert!(!inspected.failed);
        let receipt: PhaseReceipt = parse_canonical(
            &fs::read(promoted.join(CAPTURE_RECEIPT)).expect("promoted receipt bytes"),
        )
        .expect("promoted receipt");
        assert_eq!(receipt.builder_source_sha256, BUILDER_SOURCE_SHA256);
        assert_eq!(receipt.contract_id, plan.target_contract.sha256);
        assert_eq!(receipt.inputs.get("contract"), Some(&plan.target_contract));
        assert_eq!(
            receipt.inputs.get("reuse_authorization"),
            Some(&authorization_identity)
        );
        assert_eq!(receipt.reused_from, Some(plan.capture_receipt.sha256));
        prepare_phase(&promoted, &compatibility_evidence())
            .expect("current builder prepares promoted capture");
    }

    #[test]
    fn capture_promotion_authorization_is_closed_and_exact() {
        let scratch = Scratch::new();
        let prior_parent = scratch.0.join("prior-authorization");
        let target_parent = scratch.0.join("target-authorization");
        fs::create_dir(&prior_parent).expect("prior parent");
        fs::create_dir(&target_parent).expect("target parent");
        let old_builder = digest(71);
        let prior = synthetic_failed_capture_with_builder(&prior_parent, &old_builder);
        let plan = plan_capture_promotion(&prior, &old_builder).expect("promotion plan");
        let baseline = capture_promotion_authorization(&plan);
        let mut attacks = Vec::new();

        let mut changed = baseline.clone();
        changed.source_contract.sha256 = digest(1);
        attacks.push(("source-contract", changed));
        let mut changed = baseline.clone();
        changed.source_builder_source_sha256 = digest(2);
        attacks.push(("source-builder", changed));
        let mut changed = baseline.clone();
        changed.target_contract.sha256 = digest(3);
        attacks.push(("target-contract", changed));
        let mut changed = baseline.clone();
        changed.target_builder_source_sha256 = digest(4);
        attacks.push(("target-builder", changed));
        let mut changed = baseline.clone();
        changed.capture_receipt.sha256 = digest(5);
        attacks.push(("capture-receipt", changed));
        let mut changed = baseline.clone();
        changed.failure_receipt.sha256 = digest(6);
        attacks.push(("failure-receipt", changed));
        let mut changed = baseline.clone();
        changed.sealed_phases.push(Phase::Prepare);
        attacks.push(("sealed-prefix", changed));
        let mut changed = baseline.clone();
        changed.reviewer = changed.coordinator.clone();
        attacks.push(("same-reviewer", changed));

        for (name, attack) in attacks {
            let path = scratch.0.join(format!("attack-{name}.json"));
            let authorization_identity = write_authorization(&path, &attack);
            assert!(
                promote_sealed_capture(&prior, &target_parent, &path).is_err(),
                "{name} authorization unexpectedly passed"
            );
            assert!(!promotion_stage(&target_parent, &plan, &authorization_identity).exists());
        }

        let mut extra = serde_json::to_value(&baseline).expect("authorization value");
        extra
            .as_object_mut()
            .expect("authorization object")
            .insert("unexpected".into(), serde_json::Value::Bool(true));
        let mut extra_bytes = serde_jcs::to_vec(&extra).expect("canonical attack");
        extra_bytes.push(b'\n');
        let extra_path = scratch.0.join("attack-extra.json");
        fs::write(&extra_path, extra_bytes).expect("write extra field");
        assert_eq!(
            promote_sealed_capture(&prior, &target_parent, &extra_path)
                .expect_err("extra field")
                .code(),
            "JSON"
        );

        let pretty_path = scratch.0.join("attack-noncanonical.json");
        fs::write(
            &pretty_path,
            serde_json::to_vec_pretty(&baseline).expect("pretty authorization"),
        )
        .expect("write noncanonical authorization");
        assert_eq!(
            promote_sealed_capture(&prior, &target_parent, &pretty_path)
                .expect_err("noncanonical authorization")
                .code(),
            "JSON"
        );
    }

    #[test]
    fn capture_promotion_reauthenticates_every_source_authority() {
        let scratch = Scratch::new();
        let old_builder = digest(72);

        let wrong_builder_parent = scratch.0.join("wrong-builder");
        fs::create_dir(&wrong_builder_parent).expect("wrong builder parent");
        let wrong_builder =
            synthetic_failed_capture_with_builder(&wrong_builder_parent, &old_builder);
        assert_eq!(
            plan_capture_promotion(&wrong_builder, &digest(73))
                .expect_err("contract/receipt builder disagreement")
                .code(),
            "CONTRACT"
        );

        let contract_parent = scratch.0.join("changed-contract");
        fs::create_dir(&contract_parent).expect("contract parent");
        let changed_contract =
            synthetic_failed_capture_with_builder(&contract_parent, &old_builder);
        let contract_path = changed_contract.join("contract.json");
        let mut contract: OwnedCaptureContract =
            parse_canonical(&fs::read(&contract_path).expect("contract bytes")).expect("contract");
        contract.profile = "changed-profile".into();
        replace_canonical(&contract_path, &contract);
        assert_eq!(
            plan_capture_promotion(&changed_contract, &old_builder)
                .expect_err("altered old contract")
                .code(),
            "CONTRACT"
        );

        let helper_parent = scratch.0.join("changed-helper");
        fs::create_dir(&helper_parent).expect("helper parent");
        let changed_helper = synthetic_failed_capture_with_builder(&helper_parent, &old_builder);
        let contract_path = changed_helper.join("contract.json");
        let mut contract: OwnedCaptureContract =
            parse_canonical(&fs::read(&contract_path).expect("helper contract bytes"))
                .expect("helper contract");
        contract.helper = Identity {
            bytes: 1,
            sha256: digest(74),
        };
        replace_canonical(&contract_path, &contract);
        assert_eq!(
            plan_capture_promotion(&changed_helper, &old_builder)
                .expect_err("altered helper authority")
                .code(),
            "CONTRACT"
        );

        let receipt_parent = scratch.0.join("changed-receipt");
        fs::create_dir(&receipt_parent).expect("receipt parent");
        let changed_receipt = synthetic_failed_capture_with_builder(&receipt_parent, &old_builder);
        let receipt_path = changed_receipt.join(CAPTURE_RECEIPT);
        let mut receipt: PhaseReceipt =
            parse_canonical(&fs::read(&receipt_path).expect("receipt bytes")).expect("receipt");
        receipt.builder_source_sha256 = digest(75);
        replace_canonical(&receipt_path, &receipt);
        assert_eq!(
            plan_capture_promotion(&changed_receipt, &old_builder)
                .expect_err("altered receipt builder")
                .code(),
            "RECEIPT"
        );

        let member_parent = scratch.0.join("changed-member");
        fs::create_dir(&member_parent).expect("member parent");
        let changed_member = synthetic_failed_capture_with_builder(&member_parent, &old_builder);
        let observation_path = changed_member.join(OBSERVATION_MEMBER);
        fs::set_permissions(&observation_path, fs::Permissions::from_mode(0o600))
            .expect("make observation writable");
        let mut observation = fs::read(&observation_path).expect("observation bytes");
        observation.push(b'\n');
        fs::write(&observation_path, observation).expect("alter observation");
        fs::set_permissions(&observation_path, fs::Permissions::from_mode(0o400))
            .expect("restore observation mode");
        assert_eq!(
            plan_capture_promotion(&changed_member, &old_builder)
                .expect_err("altered sealed member")
                .code(),
            "RECEIPT"
        );

        let failure_parent = scratch.0.join("changed-failure");
        fs::create_dir(&failure_parent).expect("failure parent");
        let changed_failure = synthetic_failed_capture_with_builder(&failure_parent, &old_builder);
        let failure_path = changed_failure.join(FAILURE_RECEIPT);
        let mut failure: FailureReceipt =
            parse_canonical(&fs::read(&failure_path).expect("failure bytes")).expect("failure");
        failure.failed_phase = Phase::Capture;
        replace_canonical(&failure_path, &failure);
        assert_eq!(
            plan_capture_promotion(&changed_failure, &old_builder)
                .expect_err("altered failure receipt")
                .code(),
            "CAPTURE_PROMOTION"
        );
    }

    #[test]
    fn capture_promotion_is_no_replace_and_preserves_a_cancelled_target() {
        let scratch = Scratch::new();
        let prior_parent = scratch.0.join("prior-no-replace");
        fs::create_dir(&prior_parent).expect("prior parent");
        let old_builder = digest(76);
        let prior = synthetic_failed_capture_with_builder(&prior_parent, &old_builder);
        let plan = plan_capture_promotion(&prior, &old_builder).expect("promotion plan");
        let authorization = capture_promotion_authorization(&plan);
        let authorization_path = scratch.0.join("promotion-no-replace.json");
        let authorization_identity = write_authorization(&authorization_path, &authorization);
        let before_contract = hash_file(
            &prior.join("contract.json"),
            MAX_CAPTURE_CONTRACT_BYTES as u64,
        )
        .expect("source contract");
        let before_capture = hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("source capture");
        let before_failure = hash_file(&prior.join(FAILURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("source failure");

        let collision_parent = scratch.0.join("collision");
        fs::create_dir(&collision_parent).expect("collision parent");
        let collision = promotion_stage(&collision_parent, &plan, &authorization_identity);
        create_private_stage(&collision_parent, &collision).expect("collision stage");
        fs::write(collision.join("marker"), b"owned").expect("collision marker");
        assert_eq!(
            promote_sealed_capture(&prior, &collision_parent, &authorization_path)
                .expect_err("target collision")
                .code(),
            "OUTPUT"
        );
        assert_eq!(
            fs::read(collision.join("marker")).expect("marker"),
            b"owned"
        );

        let mut preserved = None;
        for checks in 0..128 {
            let target_parent = scratch.0.join(format!("cancel-{checks}"));
            fs::create_dir(&target_parent).expect("cancellation parent");
            let target = promotion_stage(&target_parent, &plan, &authorization_identity);
            let result = {
                let _guard = TestCancellation::after(checks);
                promote_sealed_capture(&prior, &target_parent, &authorization_path)
            };
            if target.exists() {
                let error = result.expect_err("cancellation after target creation");
                assert_eq!(error.code(), "CANCELLED");
                let failure: FailureReceipt = parse_canonical(
                    &fs::read(target.join(FAILURE_RECEIPT)).expect("preserved failure"),
                )
                .expect("failure receipt");
                assert_eq!(failure.failed_phase, Phase::Capture);
                assert!(failure.sealed_phases.is_empty());
                preserved = Some(target);
                break;
            }
            assert_eq!(
                result.expect_err("pre-stage cancellation").code(),
                "CANCELLED"
            );
        }
        assert!(
            preserved.is_some(),
            "no post-creation cancellation point found"
        );
        assert_eq!(
            hash_file(
                &prior.join("contract.json"),
                MAX_CAPTURE_CONTRACT_BYTES as u64
            )
            .expect("unchanged contract"),
            before_contract
        );
        assert_eq!(
            hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
                .expect("unchanged capture"),
            before_capture
        );
        assert_eq!(
            hash_file(&prior.join(FAILURE_RECEIPT), MAX_METADATA_BYTES as u64)
                .expect("unchanged failure"),
            before_failure
        );
    }

    #[test]
    fn large_environment_authenticates_inspects_and_reuses_without_a_legacy_metadata_cap() {
        let scratch = Scratch::new();
        let prior_parent = scratch.0.join("prior-large");
        let reuse_parent = scratch.0.join("reuse-large");
        fs::create_dir(&prior_parent).expect("prior parent");
        fs::create_dir(&reuse_parent).expect("reuse parent");
        let prior =
            synthetic_capture_with_environment(&prior_parent, representative_large_environment());

        let environment_bytes = fs::read(prior.join(ENVIRONMENT_MEMBER)).expect("environment");
        assert!(environment_bytes.len() > MAX_METADATA_BYTES);
        assert!(environment_bytes.len() <= MAX_ENVIRONMENT_BYTES);
        let contract_bytes = fs::read(prior.join("contract.json")).expect("contract");
        assert!(contract_bytes.len() > MAX_METADATA_BYTES);
        assert!(contract_bytes.len() <= MAX_CAPTURE_CONTRACT_BYTES);
        assert!(
            fs::metadata(prior.join(CAPTURE_RECEIPT))
                .expect("capture receipt")
                .len()
                <= MAX_METADATA_BYTES as u64
        );

        let inspected = inspect_phase(&prior).expect("inspect large capture");
        assert_eq!(inspected.sealed_phases, vec![Phase::Capture]);
        assert!(!inspected.failed);
        assert_eq!(
            prepare_phase(&prior, &compatibility_evidence_with_count(0))
                .expect_err("preserve post-capture failure")
                .code(),
            "COMPATIBILITY"
        );
        let failed = inspect_phase(&prior).expect("inspect preserved large capture");
        assert_eq!(failed.sealed_phases, vec![Phase::Capture]);
        assert!(failed.failed);

        let capture_receipt = hash_file(&prior.join(CAPTURE_RECEIPT), MAX_METADATA_BYTES as u64)
            .expect("capture receipt identity");
        let authorization = ReuseAuthorization {
            schema: "pangopup-mask-reuse-authorization-v1".into(),
            decision: "RUN-READY-REUSE".into(),
            contract_id: failed.contract_id,
            builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
            coordinator: "/root".into(),
            reviewer: "/root/reviewer".into(),
            sealed_phases: vec![Phase::Capture],
            capture_receipt,
            prepare_receipt: None,
            benchmark_receipt: None,
        };
        let authorization_path = scratch.0.join("large-authorization.json");
        let authorization_bytes = canonical(&authorization).expect("authorization");
        let authorization_identity = identity(&authorization_bytes);
        fs::write(&authorization_path, &authorization_bytes).expect("write authorization");
        let outcome = reuse_sealed_phases(&prior, &reuse_parent, &authorization_path)
            .expect("reuse large capture");
        let reused = reuse_parent.join(format!(
            "{STAGE_PREFIX}{}-reuse-{}",
            outcome.contract_id, authorization_identity.sha256
        ));
        let reused_inspection = inspect_phase(&reused).expect("inspect reused large capture");
        assert_eq!(reused_inspection.sealed_phases, vec![Phase::Capture]);
        assert!(!reused_inspection.failed);
        assert_eq!(
            fs::read(reused.join(ENVIRONMENT_MEMBER)).expect("reused environment"),
            environment_bytes
        );
        assert_eq!(
            fs::read(reused.join("contract.json")).expect("reused contract"),
            contract_bytes
        );
    }
}
