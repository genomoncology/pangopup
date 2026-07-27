//! Disposable, persistent SQLite cache for complete modeled score records.
//!
//! Cached bytes are never authoritative. Every row is matched against its
//! complete typed key and decoded through Pangopup's public constructors.

use pangopup_core::{
    GencodeGeneId, GenomicPosition, Grch38Contig, Grch38Variant, ModelGeneScoreRecord,
    ModelWarning, PangolinScore, RelativePosition, ScoreMagnitude,
};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt, fs, io,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

const APPLICATION_ID: i32 = 0x5047_5043; // PGPC
const USER_VERSION: i32 = 1;
const VALUE_SCHEMA: &str = "pangopup-model-cache-value-v1";
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_KEY_BYTES: usize = 16 * 1024;
const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_RECORDS: usize = 1_024;
const SCORING_SEMANTICS: &str = "pangopup-variant-score-v1";
const MASKING_POLICY: &str = "pangolin-gencode-v38-order-sensitive-v1";
const DISTANCE_WINDOW: u32 = 50;
type BoundedStoredRow = (Option<Vec<u8>>, Option<Vec<u8>>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryLimit {
    Bounded(u64),
    Unlimited,
}

impl Default for EntryLimit {
    fn default() -> Self {
        Self::Bounded(10_000)
    }
}

impl FromStr for EntryLimit {
    type Err = CacheError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "unlimited" {
            return Ok(Self::Unlimited);
        }
        let value = value.parse::<u64>().ok().filter(|value| *value > 0).ok_or(
            CacheError::Configuration(
                "model cache maximum must be a positive integer or unlimited",
            ),
        )?;
        Ok(Self::Bounded(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIdentity {
    model_bundle_id: String,
    model_profile: String,
    model_representation: String,
    cpu_policy: String,
    reference_bundle_id: String,
    reference_profile: String,
    reference_sequence_set_sha256: String,
    mask_bytes: u64,
    mask_sha256: String,
}

impl CacheIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_bundle_id: &str,
        model_profile: &str,
        model_representation: &str,
        cpu_policy: &str,
        reference_bundle_id: &str,
        reference_profile: &str,
        reference_sequence_set_sha256: &str,
        mask_bytes: u64,
        mask_sha256: &str,
    ) -> Result<Self, CacheError> {
        if !valid_sha256(model_bundle_id)
            || !valid_profile(model_profile)
            || !matches!(
                model_representation,
                "singleton" | "zero-padded-batch" | "paired-strand-batch"
            )
            || !matches!(
                cpu_policy,
                "sequential:auto/1"
                    | "sequential:1/1"
                    | "sequential:2/1"
                    | "sequential:4/1"
                    | "sequential:8/1"
                    | "parallel:1/2"
                    | "parallel:1/4"
                    | "parallel:1/8"
            )
            || !valid_sha256(reference_bundle_id)
            || !valid_profile(reference_profile)
            || !valid_sha256(reference_sequence_set_sha256)
            || mask_bytes == 0
            || !valid_sha256(mask_sha256)
        {
            return Err(CacheError::Configuration(
                "model cache scoring identity is invalid",
            ));
        }
        Ok(Self {
            model_bundle_id: model_bundle_id.to_owned(),
            model_profile: model_profile.to_owned(),
            model_representation: model_representation.to_owned(),
            cpu_policy: cpu_policy.to_owned(),
            reference_bundle_id: reference_bundle_id.to_owned(),
            reference_profile: reference_profile.to_owned(),
            reference_sequence_set_sha256: reference_sequence_set_sha256.to_owned(),
            mask_bytes,
            mask_sha256: mask_sha256.to_owned(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheKey {
    contig: String,
    position: u32,
    reference: String,
    alternate: String,
    scoring_semantics: &'static str,
    model_bundle_id: String,
    model_profile: String,
    model_representation: String,
    cpu_policy: String,
    reference_bundle_id: String,
    reference_profile: String,
    reference_sequence_set_sha256: String,
    mask_bytes: u64,
    mask_sha256: String,
    masking_policy: &'static str,
    window: u32,
}

impl CacheKey {
    pub fn new(variant: &Grch38Variant, identity: CacheIdentity) -> Self {
        Self {
            contig: variant.contig().to_string(),
            position: variant.position().get(),
            reference: variant.reference().to_owned(),
            alternate: variant.alternate().to_owned(),
            scoring_semantics: SCORING_SEMANTICS,
            model_bundle_id: identity.model_bundle_id,
            model_profile: identity.model_profile,
            model_representation: identity.model_representation,
            cpu_policy: identity.cpu_policy,
            reference_bundle_id: identity.reference_bundle_id,
            reference_profile: identity.reference_profile,
            reference_sequence_set_sha256: identity.reference_sequence_set_sha256,
            mask_bytes: identity.mask_bytes,
            mask_sha256: identity.mask_sha256,
            masking_policy: MASKING_POLICY,
            window: DISTANCE_WINDOW,
        }
    }

    pub fn variant(&self) -> Grch38Variant {
        Grch38Variant::new(
            self.contig
                .parse::<Grch38Contig>()
                .expect("CacheKey contig came from Grch38Variant"),
            GenomicPosition::new(self.position).expect("CacheKey position came from Grch38Variant"),
            &self.reference,
            &self.alternate,
        )
        .expect("CacheKey alleles came from Grch38Variant")
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, CacheError> {
        let bytes = serde_jcs::to_vec(self).map_err(|_| CacheError::InvalidRow)?;
        if bytes.len() > MAX_KEY_BYTES {
            return Err(CacheError::InvalidRow);
        }
        Ok(bytes)
    }

    fn digest(&self) -> Result<String, CacheError> {
        Ok(format!("{:x}", Sha256::digest(self.canonical_bytes()?)))
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_profile(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CacheCounters {
    pub hits: u64,
    pub misses: u64,
    pub fills: u64,
    pub evictions: u64,
    pub invalid_rows: u64,
    pub write_failures: u64,
}

#[derive(Debug)]
pub enum CacheError {
    Configuration(&'static str),
    UnsafePath(&'static str),
    Incompatible,
    Busy,
    Io(io::Error),
    Sqlite(rusqlite::Error),
    InvalidRow,
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(f, "invalid model cache configuration: {reason}"),
            Self::UnsafePath(reason) => write!(f, "unsafe model cache path: {reason}"),
            Self::Incompatible => f.write_str("model cache schema is incompatible"),
            Self::Busy => f.write_str("model cache is busy"),
            Self::Io(error) => write!(f, "model cache I/O failed: {error}"),
            Self::Sqlite(error) => write!(f, "model cache operation failed: {error}"),
            Self::InvalidRow => f.write_str("model cache row is invalid"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<io::Error> for CacheError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub struct ModelResultCache {
    connection: Connection,
    path: PathBuf,
    limit: EntryLimit,
    disposable_default: bool,
    counters: CacheCounters,
    pending_checkpoint: bool,
}

impl ModelResultCache {
    /// Open an explicitly selected cache. Incompatible/corrupt explicit
    /// databases are returned to the caller rather than deleted.
    pub fn open_explicit(path: &Path, limit: EntryLimit) -> Result<Self, CacheError> {
        Self::open_inner(path, limit, false)
    }

    /// Open the disposable default cache, recreating it once if incompatible
    /// or corrupt.
    pub fn open_default(path: &Path, limit: EntryLimit) -> Result<Self, CacheError> {
        match Self::open_inner(path, limit, true) {
            Ok(cache) => Ok(cache),
            Err(CacheError::Incompatible | CacheError::Sqlite(_)) => {
                remove_database_family(path)?;
                Self::open_inner(path, limit, true)
            }
            Err(error) => Err(error),
        }
    }

    fn open_inner(path: &Path, limit: EntryLimit, create_parent: bool) -> Result<Self, CacheError> {
        validate_absolute(path)?;
        let parent = path
            .parent()
            .ok_or(CacheError::UnsafePath("database has no parent"))?;
        if create_parent {
            fs::create_dir_all(parent)?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        validate_private_directory(parent)?;
        validate_database_path(path)?;
        let wal = PathBuf::from(format!("{}-wal", path.display()));
        let shm = PathBuf::from(format!("{}-shm", path.display()));
        validate_database_path(&wal)?;
        validate_database_path(&shm)?;
        if !path.exists() && (wal.exists() || shm.exists()) {
            if !create_parent {
                return Err(CacheError::Incompatible);
            }
            for sidecar in [&wal, &shm] {
                match fs::remove_file(sidecar) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        if !path.exists() {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?;
        }
        let connection = Connection::open(path).map_err(map_sqlite)?;
        connection.busy_timeout(BUSY_TIMEOUT).map_err(map_sqlite)?;
        let application_id: i32 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .map_err(map_sqlite)?;
        let user_version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(map_sqlite)?;
        if (application_id != 0 && application_id != APPLICATION_ID)
            || (user_version != 0 && user_version != USER_VERSION)
        {
            return Err(CacheError::Incompatible);
        }
        let initialized = application_id == APPLICATION_ID && user_version == USER_VERSION;
        connection
            .execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(map_sqlite)?;
        if initialized {
            let journal_mode: String = connection
                .pragma_query_value(None, "journal_mode", |row| row.get(0))
                .map_err(map_sqlite)?;
            if journal_mode != "wal" {
                return Err(CacheError::Incompatible);
            }
        } else {
            connection
                .execute_batch(&format!(
                    "PRAGMA journal_mode=WAL;
                     PRAGMA application_id={APPLICATION_ID};
                 PRAGMA user_version={USER_VERSION};
                     CREATE TABLE IF NOT EXISTS metadata (
                   singleton INTEGER PRIMARY KEY CHECK(singleton=1),
                   next_write_sequence INTEGER NOT NULL CHECK(next_write_sequence > 0)
                 ) STRICT;
                     INSERT OR IGNORE INTO metadata VALUES(1, 1);
                     CREATE TABLE IF NOT EXISTS entries (
                   key_digest TEXT PRIMARY KEY,
                   key_json BLOB NOT NULL,
                   contig TEXT NOT NULL,
                   position INTEGER NOT NULL,
                   reference TEXT NOT NULL,
                   alternate TEXT NOT NULL,
                   scoring_semantics TEXT NOT NULL,
                   model_bundle_id TEXT NOT NULL,
                   model_profile TEXT NOT NULL,
                   model_representation TEXT NOT NULL,
                   cpu_policy TEXT NOT NULL,
                   reference_bundle_id TEXT NOT NULL,
                   reference_profile TEXT NOT NULL,
                   reference_sequence_set_sha256 TEXT NOT NULL,
                   mask_bytes INTEGER NOT NULL,
                   mask_sha256 TEXT NOT NULL,
                   masking_policy TEXT NOT NULL,
                   window INTEGER NOT NULL,
                   value_json BLOB NOT NULL,
                   write_sequence INTEGER NOT NULL CHECK(write_sequence > 0)
                 ) STRICT;"
                ))
                .map_err(map_sqlite)?;
        }
        validate_schema(&connection)?;
        set_family_permissions(path)?;
        let mut cache = Self {
            connection,
            path: path.to_owned(),
            limit,
            disposable_default: create_parent,
            counters: CacheCounters::default(),
            pending_checkpoint: !initialized,
        };
        cache.evict_to_limit().map_err(map_sqlite)?;
        Ok(cache)
    }

    pub fn get(&mut self, key: &CacheKey) -> Result<Option<Vec<ModelGeneScoreRecord>>, CacheError> {
        match self.get_inner(key) {
            Err(CacheError::Sqlite(_)) if self.disposable_default => {
                self.recreate_default()?;
                self.counters.misses += 1;
                Ok(None)
            }
            result => result,
        }
    }

    fn get_inner(
        &mut self,
        key: &CacheKey,
    ) -> Result<Option<Vec<ModelGeneScoreRecord>>, CacheError> {
        let digest = key.digest()?;
        let mask_bytes = i64::try_from(key.mask_bytes).map_err(|_| CacheError::InvalidRow)?;
        let row: Option<BoundedStoredRow> = self
            .connection
            .query_row(
                "SELECT
                   CASE WHEN length(key_json)<=16384 THEN key_json END,
                   CASE WHEN length(value_json)<=1048576 THEN value_json END
                 FROM entries WHERE
                   key_digest=?1 AND contig=?2 AND position=?3 AND reference=?4
                   AND alternate=?5 AND scoring_semantics=?6 AND model_bundle_id=?7
                   AND model_profile=?8 AND model_representation=?9 AND cpu_policy=?10
                   AND reference_bundle_id=?11 AND reference_profile=?12
                   AND reference_sequence_set_sha256=?13 AND mask_bytes=?14
                   AND mask_sha256=?15 AND masking_policy=?16 AND window=?17",
                params![
                    digest,
                    key.contig,
                    key.position,
                    key.reference,
                    key.alternate,
                    key.scoring_semantics,
                    key.model_bundle_id,
                    key.model_profile,
                    key.model_representation,
                    key.cpu_policy,
                    key.reference_bundle_id,
                    key.reference_profile,
                    key.reference_sequence_set_sha256,
                    mask_bytes,
                    key.mask_sha256,
                    key.masking_policy,
                    key.window
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sqlite)?;
        let Some((stored_key, value)) = row else {
            self.counters.misses += 1;
            return Ok(None);
        };
        let (Some(stored_key), Some(value)) = (stored_key, value) else {
            self.connection
                .execute("DELETE FROM entries WHERE key_digest=?1", [&digest])
                .map_err(map_sqlite)?;
            self.pending_checkpoint = true;
            self.counters.invalid_rows += 1;
            self.counters.misses += 1;
            return Ok(None);
        };
        if stored_key != key.canonical_bytes()? {
            self.counters.misses += 1;
            return Ok(None);
        }
        match decode_value(&value) {
            Ok(records) => {
                self.counters.hits += 1;
                Ok(Some(records))
            }
            Err(_) => {
                self.connection
                    .execute("DELETE FROM entries WHERE key_digest=?1", [&digest])
                    .map_err(map_sqlite)?;
                self.pending_checkpoint = true;
                self.counters.invalid_rows += 1;
                self.counters.misses += 1;
                Ok(None)
            }
        }
    }

    fn recreate_default(&mut self) -> Result<(), CacheError> {
        let placeholder = Connection::open_in_memory().map_err(map_sqlite)?;
        let old = std::mem::replace(&mut self.connection, placeholder);
        drop(old);
        remove_database_family(&self.path)?;
        let mut replacement = Self::open_inner(&self.path, self.limit, true)?;
        self.connection = std::mem::replace(
            &mut replacement.connection,
            Connection::open_in_memory().map_err(map_sqlite)?,
        );
        self.pending_checkpoint = replacement.pending_checkpoint;
        Ok(())
    }

    /// Store a successful complete result. A write failure is deliberately
    /// reported separately so callers can retain their already-computed answer.
    pub fn put(
        &mut self,
        key: &CacheKey,
        records: &[ModelGeneScoreRecord],
    ) -> Result<(), CacheError> {
        let result = self.put_inner(key, records);
        if result.is_err() {
            self.counters.write_failures += 1;
        }
        result
    }

    fn put_inner(
        &mut self,
        key: &CacheKey,
        records: &[ModelGeneScoreRecord],
    ) -> Result<(), CacheError> {
        let digest = key.digest()?;
        let key_json = key.canonical_bytes()?;
        let value_json = encode_value(records)?;
        let mask_bytes = i64::try_from(key.mask_bytes).map_err(|_| CacheError::InvalidRow)?;
        let transaction = self.connection.transaction().map_err(map_sqlite)?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT key_json FROM entries WHERE key_digest=?1",
                [&digest],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(map_sqlite)?
            && existing != key_json
        {
            return Ok(());
        }
        let sequence = take_write_sequence(&transaction).map_err(map_sqlite)?;
        transaction
            .execute(
                "INSERT INTO entries(
                   key_digest,key_json,contig,position,reference,alternate,
                   scoring_semantics,model_bundle_id,model_profile,model_representation,
                   cpu_policy,reference_bundle_id,reference_profile,
                   reference_sequence_set_sha256,mask_bytes,mask_sha256,masking_policy,
                   window,value_json,write_sequence
                 )
                 VALUES(
                   ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20
                 )
                 ON CONFLICT(key_digest) DO UPDATE SET
                   value_json=excluded.value_json,
                   write_sequence=excluded.write_sequence",
                params![
                    digest,
                    key_json,
                    key.contig,
                    key.position,
                    key.reference,
                    key.alternate,
                    key.scoring_semantics,
                    key.model_bundle_id,
                    key.model_profile,
                    key.model_representation,
                    key.cpu_policy,
                    key.reference_bundle_id,
                    key.reference_profile,
                    key.reference_sequence_set_sha256,
                    mask_bytes,
                    key.mask_sha256,
                    key.masking_policy,
                    key.window,
                    value_json,
                    sequence
                ],
            )
            .map_err(map_sqlite)?;
        let evictions = evict_transaction(&transaction, self.limit).map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        self.pending_checkpoint = true;
        self.counters.fills += 1;
        self.counters.evictions += evictions;
        set_family_permissions(&self.path)?;
        Ok(())
    }

    fn evict_to_limit(&mut self) -> Result<(), rusqlite::Error> {
        let EntryLimit::Bounded(limit) = self.limit else {
            return Ok(());
        };
        let count = self
            .connection
            .query_row("SELECT count(*) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })? as u64;
        if count <= limit {
            return Ok(());
        }
        let transaction = self.connection.transaction()?;
        let count = evict_transaction(&transaction, self.limit)?;
        transaction.commit()?;
        self.pending_checkpoint = true;
        self.counters.evictions += count;
        Ok(())
    }

    #[cfg(test)]
    fn counters(&self) -> CacheCounters {
        self.counters
    }

    #[cfg(test)]
    fn entry_count(&self) -> Result<u64, CacheError> {
        self.connection
            .query_row("SELECT count(*) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|value| value as u64)
            .map_err(map_sqlite)
    }

    #[cfg(test)]
    fn checkpoint(&mut self) {
        if !self.pending_checkpoint {
            return;
        }
        if self
            .connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .is_ok()
        {
            self.pending_checkpoint = false;
        }
    }
}

fn validate_schema(connection: &Connection) -> Result<(), CacheError> {
    validate_table_shape(
        connection,
        "metadata",
        &[
            ColumnShape::new("singleton", "INTEGER", false, 1),
            ColumnShape::new("next_write_sequence", "INTEGER", true, 0),
        ],
    )?;
    let expected = [
        ColumnShape::new("key_digest", "TEXT", true, 1),
        ColumnShape::new("key_json", "BLOB", true, 0),
        ColumnShape::new("contig", "TEXT", true, 0),
        ColumnShape::new("position", "INTEGER", true, 0),
        ColumnShape::new("reference", "TEXT", true, 0),
        ColumnShape::new("alternate", "TEXT", true, 0),
        ColumnShape::new("scoring_semantics", "TEXT", true, 0),
        ColumnShape::new("model_bundle_id", "TEXT", true, 0),
        ColumnShape::new("model_profile", "TEXT", true, 0),
        ColumnShape::new("model_representation", "TEXT", true, 0),
        ColumnShape::new("cpu_policy", "TEXT", true, 0),
        ColumnShape::new("reference_bundle_id", "TEXT", true, 0),
        ColumnShape::new("reference_profile", "TEXT", true, 0),
        ColumnShape::new("reference_sequence_set_sha256", "TEXT", true, 0),
        ColumnShape::new("mask_bytes", "INTEGER", true, 0),
        ColumnShape::new("mask_sha256", "TEXT", true, 0),
        ColumnShape::new("masking_policy", "TEXT", true, 0),
        ColumnShape::new("window", "INTEGER", true, 0),
        ColumnShape::new("value_json", "BLOB", true, 0),
        ColumnShape::new("write_sequence", "INTEGER", true, 0),
    ];
    validate_table_shape(connection, "entries", &expected)?;
    let metadata = connection
        .query_row(
            "SELECT count(*), min(singleton), max(singleton),
                    min(next_write_sequence), max(next_write_sequence)
             FROM metadata",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .map_err(map_sqlite)?;
    let maximum_sequence: i64 = connection
        .query_row(
            "SELECT coalesce(max(write_sequence), 0) FROM entries",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if metadata.0 != 1
        || metadata.1 != Some(1)
        || metadata.2 != Some(1)
        || metadata.3 != metadata.4
        || metadata.3.is_none_or(|next| next <= maximum_sequence)
    {
        return Err(CacheError::Incompatible);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ColumnShape {
    name: &'static str,
    declared_type: &'static str,
    not_null: bool,
    primary_key: i64,
}

impl ColumnShape {
    const fn new(
        name: &'static str,
        declared_type: &'static str,
        not_null: bool,
        primary_key: i64,
    ) -> Self {
        Self {
            name,
            declared_type,
            not_null,
            primary_key,
        }
    }
}

fn validate_table_shape(
    connection: &Connection,
    table: &str,
    expected: &[ColumnShape],
) -> Result<(), CacheError> {
    debug_assert!(matches!(table, "metadata" | "entries"));
    let strict: Option<i64> = connection
        .query_row(
            "SELECT strict FROM pragma_table_list
             WHERE schema='main' AND name=?1 AND type='table'",
            [table],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if strict != Some(1) {
        return Err(CacheError::Incompatible);
    }
    let mut statement = connection
        .prepare(&format!("PRAGMA table_xinfo({table})"))
        .map_err(map_sqlite)?;
    let observed = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(map_sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite)?;
    let expected = expected
        .iter()
        .map(|column| {
            (
                column.name.to_owned(),
                column.declared_type.to_owned(),
                column.not_null,
                column.primary_key,
                0_i64,
            )
        })
        .collect::<Vec<_>>();
    if observed != expected {
        return Err(CacheError::Incompatible);
    }
    Ok(())
}

impl Drop for ModelResultCache {
    fn drop(&mut self) {
        if self.pending_checkpoint {
            let _ = self
                .connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        }
    }
}

fn take_write_sequence(transaction: &rusqlite::Transaction<'_>) -> Result<i64, rusqlite::Error> {
    let next: i64 = transaction.query_row(
        "SELECT next_write_sequence FROM metadata WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    if next == i64::MAX {
        transaction.execute_batch(
            "CREATE TEMP TABLE renumber(digest TEXT PRIMARY KEY, sequence INTEGER) STRICT;
             INSERT INTO renumber
               SELECT key_digest, row_number() OVER (ORDER BY write_sequence,key_digest)
               FROM entries;
             UPDATE entries SET write_sequence=(
               SELECT sequence FROM renumber WHERE digest=entries.key_digest
             );
             DROP TABLE renumber;
             UPDATE metadata SET next_write_sequence=
               (SELECT count(*) + 1 FROM entries) WHERE singleton=1;",
        )?;
        let renumbered: i64 = transaction.query_row(
            "SELECT next_write_sequence FROM metadata WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE metadata SET next_write_sequence=?1 WHERE singleton=1",
            [renumbered + 1],
        )?;
        return Ok(renumbered);
    }
    transaction.execute(
        "UPDATE metadata SET next_write_sequence=?1 WHERE singleton=1",
        [next + 1],
    )?;
    Ok(next)
}

fn evict_transaction(
    transaction: &rusqlite::Transaction<'_>,
    limit: EntryLimit,
) -> Result<u64, rusqlite::Error> {
    let EntryLimit::Bounded(limit) = limit else {
        return Ok(0);
    };
    let count = transaction.query_row("SELECT count(*) FROM entries", [], |row| {
        row.get::<_, i64>(0)
    })? as u64;
    let remove = count.saturating_sub(limit);
    if remove != 0 {
        transaction.execute(
            "DELETE FROM entries WHERE key_digest IN (
               SELECT key_digest FROM entries
               ORDER BY write_sequence,key_digest LIMIT ?1
             )",
            [remove as i64],
        )?;
    }
    Ok(remove)
}

#[derive(Serialize)]
struct CacheValue<'a> {
    schema: &'static str,
    records: Vec<StoredRecord<'a>>,
}

#[derive(Serialize)]
struct StoredRecord<'a> {
    gene: String,
    gain: u16,
    gain_position: i16,
    loss: u16,
    loss_position: i16,
    warnings: Vec<&'a str>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DecodedValue {
    schema: String,
    records: Vec<DecodedRecord>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DecodedRecord {
    gene: String,
    gain: u16,
    gain_position: i16,
    loss: u16,
    loss_position: i16,
    warnings: Vec<String>,
}

fn encode_value(records: &[ModelGeneScoreRecord]) -> Result<Vec<u8>, CacheError> {
    if records.len() > MAX_RECORDS {
        return Err(CacheError::InvalidRow);
    }
    let records = records
        .iter()
        .map(|record| {
            let score = record.score();
            StoredRecord {
                gene: record.gene().to_string(),
                gain: u16::from(score.gain().hundredths()),
                gain_position: score.gain_position().get(),
                loss: u16::from(score.loss().hundredths()),
                loss_position: score.loss_position().get(),
                warnings: record
                    .warnings()
                    .iter()
                    .map(|warning| match warning {
                        ModelWarning::NoAnnotatedSites => "no_annotated_sites",
                    })
                    .collect(),
            }
        })
        .collect();
    let bytes = serde_jcs::to_vec(&CacheValue {
        schema: VALUE_SCHEMA,
        records,
    })
    .map_err(|_| CacheError::InvalidRow)?;
    if bytes.len() > MAX_VALUE_BYTES {
        return Err(CacheError::InvalidRow);
    }
    Ok(bytes)
}

fn decode_value(bytes: &[u8]) -> Result<Vec<ModelGeneScoreRecord>, CacheError> {
    if bytes.len() > MAX_VALUE_BYTES {
        return Err(CacheError::InvalidRow);
    }
    let decoded: DecodedValue =
        serde_json::from_slice(bytes).map_err(|_| CacheError::InvalidRow)?;
    if decoded.schema != VALUE_SCHEMA
        || decoded.records.len() > MAX_RECORDS
        || serde_jcs::to_vec(&decoded).map_err(|_| CacheError::InvalidRow)? != bytes
    {
        return Err(CacheError::InvalidRow);
    }
    let mut genes = BTreeSet::new();
    decoded
        .records
        .into_iter()
        .map(|record| {
            let gene = GencodeGeneId::from_str(&record.gene).map_err(|_| CacheError::InvalidRow)?;
            if !genes.insert(gene.to_string()) {
                return Err(CacheError::InvalidRow);
            }
            let gain = ScoreMagnitude::new(record.gain).map_err(|_| CacheError::InvalidRow)?;
            let loss = ScoreMagnitude::new(record.loss).map_err(|_| CacheError::InvalidRow)?;
            let gain_position =
                RelativePosition::new(record.gain_position).map_err(|_| CacheError::InvalidRow)?;
            let loss_position =
                RelativePosition::new(record.loss_position).map_err(|_| CacheError::InvalidRow)?;
            if record.warnings.len() > 1 {
                return Err(CacheError::InvalidRow);
            }
            let warnings = record
                .warnings
                .into_iter()
                .map(|warning| match warning.as_str() {
                    "no_annotated_sites" => Ok(ModelWarning::NoAnnotatedSites),
                    _ => Err(CacheError::InvalidRow),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ModelGeneScoreRecord::new(
                gene,
                PangolinScore::new(gain, gain_position, loss, loss_position),
                warnings,
            ))
        })
        .collect()
}

fn validate_absolute(path: &Path) -> Result<(), CacheError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(CacheError::Configuration(
            "model cache path must be an absolute file path",
        ));
    }
    Ok(())
}

fn validate_private_directory(path: &Path) -> Result<(), CacheError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(CacheError::UnsafePath(
            "parent must be an owned private directory",
        ));
    }
    Ok(())
}

fn validate_database_path(path: &Path) -> Result<(), CacheError> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0 =>
        {
            Err(CacheError::UnsafePath(
                "database must be an owned private regular file",
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn set_family_permissions(path: &Path) -> Result<(), CacheError> {
    for candidate in [
        path.to_owned(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            fs::set_permissions(candidate, fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

fn remove_database_family(path: &Path) -> Result<(), CacheError> {
    for candidate in [
        path.to_owned(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn map_sqlite(error: rusqlite::Error) -> CacheError {
    match &error {
        rusqlite::Error::SqliteFailure(value, _)
            if matches!(
                value.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            ) =>
        {
            CacheError::Busy
        }
        _ => CacheError::Sqlite(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn private_temp() -> TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("mode");
        temp
    }

    fn key(position: u32) -> CacheKey {
        let variant = Grch38Variant::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(position).expect("position"),
            "A",
            "AC",
        )
        .expect("variant");
        CacheKey::new(
            &variant,
            CacheIdentity::new(
                &format!("sha256:{:064x}", 1),
                "model",
                "singleton",
                "sequential:1/1",
                &format!("sha256:{:064x}", 2),
                "reference",
                &format!("sha256:{:064x}", 3),
                260,
                &format!("sha256:{:064x}", 4),
            )
            .expect("identity"),
        )
    }

    fn records() -> Vec<ModelGeneScoreRecord> {
        vec![ModelGeneScoreRecord::new(
            "ENSG00000000001.1".parse().expect("gene"),
            PangolinScore::new(
                ScoreMagnitude::new(33).expect("gain"),
                RelativePosition::new(-3).expect("gain position"),
                ScoreMagnitude::new(12).expect("loss"),
                RelativePosition::new(4).expect("loss position"),
            ),
            vec![ModelWarning::NoAnnotatedSites],
        )]
    }

    #[test]
    fn persists_exact_records_across_reopen() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        {
            let mut cache =
                ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
            assert_eq!(cache.get(&key(10)).expect("miss"), None);
            cache.put(&key(10), &records()).expect("put");
            assert_eq!(cache.counters().misses, 1);
        }
        let mut reopened =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("reopen");
        assert_eq!(reopened.get(&key(10)).expect("hit"), Some(records()));
        assert_eq!(reopened.counters().hits, 1);
    }

    #[test]
    fn valid_hits_do_not_mutate_database_wal_or_write_sequence() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let wal = PathBuf::from(format!("{}-wal", path.display()));
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        cache.put(&key(10), &records()).expect("put");
        cache.checkpoint();
        let next_sequence = |cache: &ModelResultCache| {
            cache
                .connection
                .query_row(
                    "SELECT next_write_sequence FROM metadata WHERE singleton=1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("write sequence")
        };
        let before_sequence = next_sequence(&cache);
        let before_database = fs::read(&path).expect("database bytes");
        let before_wal = fs::read(&wal).ok();

        assert_eq!(cache.get(&key(10)).expect("valid hit"), Some(records()));

        assert_eq!(next_sequence(&cache), before_sequence);
        assert_eq!(
            fs::read(&path).expect("database bytes after"),
            before_database
        );
        assert_eq!(fs::read(&wal).ok(), before_wal);
        assert_eq!(cache.counters().write_failures, 0);
    }

    #[test]
    fn bounded_cache_uses_write_order_and_limit_reduction_applies_on_open() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::Bounded(2)).expect("open");
        cache.put(&key(1), &records()).expect("one");
        cache.put(&key(2), &records()).expect("two");
        assert!(cache.get(&key(1)).expect("read without refresh").is_some());
        cache.put(&key(3), &records()).expect("three");
        assert!(cache.get(&key(1)).expect("oldest write evicted").is_none());
        assert!(cache.get(&key(2)).expect("second write retained").is_some());
        cache.put(&key(2), &records()).expect("explicit refresh");
        cache.put(&key(4), &records()).expect("four");
        assert!(cache.get(&key(3)).expect("older write evicted").is_none());
        assert!(
            cache
                .get(&key(2))
                .expect("updated write retained")
                .is_some()
        );
        drop(cache);

        let mut reduced =
            ModelResultCache::open_explicit(&path, EntryLimit::Bounded(1)).expect("reduced");
        assert_eq!(reduced.entry_count().expect("count"), 1);
        assert!(
            reduced
                .get(&key(4))
                .expect("newest write retained")
                .is_some()
        );
    }

    #[test]
    fn unlimited_does_not_evict() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::Unlimited).expect("open");
        for position in 1..=12 {
            cache.put(&key(position), &records()).expect("put");
        }
        assert_eq!(cache.entry_count().expect("count"), 12);
        assert_eq!(cache.counters().evictions, 0);
    }

    #[test]
    fn write_sequence_renumbers_before_integer_exhaustion() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::Bounded(2)).expect("open");
        cache.put(&key(1), &records()).expect("one");
        cache.put(&key(2), &records()).expect("two");
        cache
            .connection
            .execute("UPDATE metadata SET next_write_sequence=?1", [i64::MAX])
            .expect("near exhaustion");
        cache
            .put(&key(1), &records())
            .expect("update through renumber");
        cache.put(&key(3), &records()).expect("put after renumber");
        assert!(cache.get(&key(1)).expect("newest retained").is_some());
        assert!(cache.get(&key(2)).expect("oldest evicted").is_none());
    }

    #[test]
    fn malformed_canonical_value_is_deleted_and_becomes_miss() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("put");
        cache
            .connection
            .execute(
                "UPDATE entries SET value_json=?1",
                [b"{\"schema\":\"wrong\",\"records\":[]}".as_slice()],
            )
            .expect("corrupt row");
        assert_eq!(cache.get(&key(1)).expect("safe miss"), None);
        assert_eq!(cache.entry_count().expect("deleted"), 0);
        assert_eq!(cache.counters().invalid_rows, 1);
    }

    #[test]
    fn explicit_incompatible_database_is_not_deleted_but_default_is_recreated() {
        let explicit = private_temp();
        let explicit_path = explicit.path().join("cache.sqlite3");
        let connection = Connection::open(&explicit_path).expect("open foreign");
        connection
            .pragma_update(None, "application_id", 123_i32)
            .expect("foreign id");
        drop(connection);
        fs::set_permissions(&explicit_path, fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(matches!(
            ModelResultCache::open_explicit(&explicit_path, EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));
        let connection = Connection::open(&explicit_path).expect("still exists");
        let id: i32 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .expect("id");
        assert_eq!(id, 123);

        let disposable = private_temp();
        let disposable_path = disposable.path().join("cache.sqlite3");
        let connection = Connection::open(&disposable_path).expect("open foreign");
        connection
            .pragma_update(None, "application_id", 123_i32)
            .expect("foreign id");
        drop(connection);
        fs::set_permissions(&disposable_path, fs::Permissions::from_mode(0o600)).expect("mode");
        let cache = ModelResultCache::open_default(&disposable_path, EntryLimit::default())
            .expect("recreated");
        assert_eq!(cache.entry_count().expect("empty"), 0);

        let corrupt = private_temp();
        let corrupt_path = corrupt.path().join("cache.sqlite3");
        fs::write(&corrupt_path, b"not sqlite").expect("corrupt bytes");
        fs::set_permissions(&corrupt_path, fs::Permissions::from_mode(0o600)).expect("mode");
        let cache = ModelResultCache::open_default(&corrupt_path, EntryLimit::default())
            .expect("corrupt default recreated");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn established_cache_reopen_validates_metadata_contract() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        drop(
            ModelResultCache::open_explicit(&path, EntryLimit::default())
                .expect("initialize cache"),
        );
        let connection = Connection::open(&path).expect("reopen raw database");
        connection
            .execute("DROP TABLE metadata", [])
            .expect("remove required metadata");
        drop(connection);
        assert!(matches!(
            ModelResultCache::open_explicit(&path, EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));

        let sequence = private_temp();
        let sequence_path = sequence.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&sequence_path, EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("put");
        cache
            .connection
            .execute("UPDATE metadata SET next_write_sequence=1", [])
            .expect("invalidate sequence");
        drop(cache);
        assert!(matches!(
            ModelResultCache::open_explicit(&sequence_path, EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));
    }

    #[test]
    fn same_named_wrong_shape_schema_is_rejected_or_recreated() {
        fn mutate_declared_type(path: &Path) {
            drop(
                ModelResultCache::open_explicit(path, EntryLimit::default())
                    .expect("initialize cache"),
            );
            let connection = Connection::open(path).expect("raw open");
            connection
                .execute_batch(
                    "PRAGMA writable_schema=ON;
                     UPDATE sqlite_schema
                     SET sql=replace(sql, 'key_digest TEXT PRIMARY KEY',
                                         'key_digest BLOB PRIMARY KEY')
                     WHERE type='table' AND name='entries';
                     PRAGMA schema_version=2;
                     PRAGMA writable_schema=OFF;",
                )
                .expect("mutate declared type without changing names");
        }

        let explicit = private_temp();
        let explicit_path = explicit.path().join("cache.sqlite3");
        mutate_declared_type(&explicit_path);
        assert!(matches!(
            ModelResultCache::open_explicit(&explicit_path, EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));

        let disposable = private_temp();
        let disposable_path = disposable.path().join("cache.sqlite3");
        mutate_declared_type(&disposable_path);
        let cache = ModelResultCache::open_default(&disposable_path, EntryLimit::default())
            .expect("default recreates wrong-shape schema");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn busy_write_is_reported_without_damaging_existing_rows() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("existing");
        let blocker = Connection::open(&path).expect("blocker");
        blocker
            .execute_batch("BEGIN IMMEDIATE")
            .expect("hold write lock");
        assert!(matches!(
            cache.put(&key(2), &records()),
            Err(CacheError::Busy)
        ));
        blocker.execute_batch("ROLLBACK").expect("release");
        assert_eq!(
            cache.get(&key(1)).expect("existing intact"),
            Some(records())
        );
        assert_eq!(cache.get(&key(2)).expect("failed fill absent"), None);
        assert_eq!(cache.counters().write_failures, 1);
    }

    #[test]
    fn disposable_default_recovers_from_runtime_sqlite_failure_as_a_miss() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache = ModelResultCache::open_default(&path, EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("put");
        cache
            .connection
            .execute_batch("DROP TABLE entries")
            .expect("simulate damaged database");
        assert_eq!(cache.get(&key(1)).expect("recovered miss"), None);
        assert_eq!(cache.entry_count().expect("recreated"), 0);
    }

    #[test]
    fn unsafe_paths_are_rejected_and_family_is_private() {
        let temp = private_temp();
        let target = temp.path().join("target.sqlite3");
        fs::write(&target, []).expect("target");
        let link = temp.path().join("link.sqlite3");
        symlink(&target, &link).expect("symlink");
        assert!(matches!(
            ModelResultCache::open_explicit(&link, EntryLimit::default()),
            Err(CacheError::UnsafePath(_))
        ));

        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("put");
        for candidate in [
            path.clone(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            if candidate.exists() {
                assert_eq!(
                    fs::metadata(candidate).expect("metadata").mode() & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn orphan_sidecars_are_rejected_explicitly_and_removed_for_default() {
        let explicit = private_temp();
        let explicit_path = explicit.path().join("cache.sqlite3");
        let explicit_wal = PathBuf::from(format!("{}-wal", explicit_path.display()));
        fs::write(&explicit_wal, b"stale").expect("stale WAL");
        fs::set_permissions(&explicit_wal, fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(matches!(
            ModelResultCache::open_explicit(&explicit_path, EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));

        let disposable = private_temp();
        let disposable_path = disposable.path().join("cache.sqlite3");
        let disposable_wal = PathBuf::from(format!("{}-wal", disposable_path.display()));
        fs::write(&disposable_wal, b"stale").expect("stale WAL");
        fs::set_permissions(&disposable_wal, fs::Permissions::from_mode(0o600)).expect("mode");
        let cache = ModelResultCache::open_default(&disposable_path, EntryLimit::default())
            .expect("default replaces orphan");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn every_key_identity_causes_an_actual_cache_miss() {
        fn changed_keys(original: &CacheKey) -> Vec<CacheKey> {
            let mut variants = Vec::new();
            macro_rules! changed {
                ($field:ident, $value:expr) => {{
                    let mut changed = original.clone();
                    changed.$field = $value;
                    variants.push(changed);
                }};
            }
            changed!(contig, "chr2".to_owned());
            changed!(position, 2);
            changed!(reference, "C".to_owned());
            changed!(alternate, "AG".to_owned());
            changed!(scoring_semantics, "other-score-v1");
            changed!(model_bundle_id, format!("sha256:{:064x}", 11));
            changed!(model_profile, "other-model".to_owned());
            changed!(model_representation, "zero-padded-batch".to_owned());
            changed!(cpu_policy, "sequential:8/1".to_owned());
            changed!(reference_bundle_id, format!("sha256:{:064x}", 12));
            changed!(reference_profile, "other-reference".to_owned());
            changed!(reference_sequence_set_sha256, format!("sha256:{:064x}", 13));
            changed!(mask_bytes, 261);
            changed!(mask_sha256, format!("sha256:{:064x}", 14));
            changed!(masking_policy, "other-mask-v1");
            changed!(window, 49);
            variants
        }

        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        let original = key(1);
        cache.put(&original, &records()).expect("put original");
        for changed in changed_keys(&original) {
            assert_eq!(cache.get(&changed).expect("changed-key lookup"), None);
        }
        assert_eq!(
            cache.get(&original).expect("original remains"),
            Some(records())
        );
        assert_eq!(cache.entry_count().expect("one row"), 1);
    }

    #[test]
    fn public_key_construction_rejects_invalid_scoring_identity() {
        let good_sha = format!("sha256:{:064x}", 1);
        let build = |model_bundle: &str,
                     model_profile: &str,
                     representation: &str,
                     cpu: &str,
                     reference_bundle: &str,
                     reference_profile: &str,
                     sequence_sha: &str,
                     mask_bytes: u64,
                     mask_sha: &str| {
            CacheIdentity::new(
                model_bundle,
                model_profile,
                representation,
                cpu,
                reference_bundle,
                reference_profile,
                sequence_sha,
                mask_bytes,
                mask_sha,
            )
        };
        for result in [
            build(
                "sha256:BAD",
                "model",
                "singleton",
                "sequential:1/1",
                &good_sha,
                "reference",
                &good_sha,
                1,
                &good_sha,
            ),
            build(
                &good_sha,
                "",
                "singleton",
                "sequential:1/1",
                &good_sha,
                "reference",
                &good_sha,
                1,
                &good_sha,
            ),
            build(
                &good_sha,
                "model",
                "unknown",
                "sequential:1/1",
                &good_sha,
                "reference",
                &good_sha,
                1,
                &good_sha,
            ),
            build(
                &good_sha,
                "model",
                "singleton",
                "fast",
                &good_sha,
                "reference",
                &good_sha,
                1,
                &good_sha,
            ),
            build(
                &good_sha,
                "model",
                "singleton",
                "sequential:1/1",
                &good_sha,
                "reference",
                &good_sha,
                0,
                &good_sha,
            ),
        ] {
            assert!(matches!(result, Err(CacheError::Configuration(_))));
        }
    }

    #[test]
    fn same_digest_with_different_full_key_neither_aliases_nor_overwrites() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, EntryLimit::default()).expect("open");
        let original = key(1);
        let other = key(2);
        cache.put(&original, &records()).expect("put original");
        let other_digest = other.digest().expect("other digest");
        cache
            .connection
            .execute(
                "UPDATE entries SET key_digest=?1 WHERE key_digest=?2",
                params![other_digest, original.digest().expect("original digest")],
            )
            .expect("force synthetic digest collision");

        assert_eq!(cache.get(&other).expect("collision is a miss"), None);
        cache
            .put(&other, &[])
            .expect("collision does not become a cache failure");
        assert_eq!(cache.entry_count().expect("one retained row"), 1);
        let stored_key: Vec<u8> = cache
            .connection
            .query_row(
                "SELECT key_json FROM entries WHERE key_digest=?1",
                [other_digest],
                |row| row.get(0),
            )
            .expect("stored collision row");
        assert_eq!(
            stored_key,
            original.canonical_bytes().expect("original key")
        );
    }
}
