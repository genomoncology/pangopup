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
    collections::{BTreeSet, HashSet},
    fmt, fs, io,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Mutex, OnceLock},
    time::Duration,
};

const APPLICATION_ID: i32 = 0x5047_5043; // PGPC
const USER_VERSION: i32 = 2;
/// Layouts earlier releases wrote. A file stamped with one is this
/// software's own file from another version, not a foreign or a damaged
/// database, so it is discarded whole rather than refused.
const EARLIER_USER_VERSIONS: [i32; 1] = [1];
const VALUE_SCHEMA: &str = "pangopup-model-cache-value-v1";
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_KEY_BYTES: usize = 16 * 1024;
const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_RECORDS: usize = 1_024;
const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
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

/// The setup a run reaches: the assets it scores with, beside the build
/// constants and software version the file records for itself. It is not the
/// effective CPU policy. Ticket 0040 measured that no thread or worker setting
/// moves any score, position, status, reason or provenance field, so keying or
/// stamping on one throws paid-for rows away on a thread change and still
/// fails to notice a software change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIdentity {
    model_bundle_id: String,
    model_profile: String,
    model_representation: String,
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
            reference_bundle_id: reference_bundle_id.to_owned(),
            reference_profile: reference_profile.to_owned(),
            reference_sequence_set_sha256: reference_sequence_set_sha256.to_owned(),
            mask_bytes,
            mask_sha256: mask_sha256.to_owned(),
        })
    }
}

/// What one cache file records about the setup that filled it, as the readable
/// text a reader sees when the file is opened. The judge reads the same text,
/// so a match is never decided from a digest a reader cannot check against the
/// values beside it.
#[derive(Debug, Eq, PartialEq)]
struct RecordedSetup {
    software_version: String,
    model_bundle_id: String,
    model_profile: String,
    model_representation: String,
    reference_bundle_id: String,
    reference_profile: String,
    reference_sequence_set_sha256: String,
    mask_bytes: i64,
    mask_sha256: String,
    scoring_semantics: String,
    masking_policy: String,
    window: i64,
}

impl RecordedSetup {
    fn running(identity: &CacheIdentity) -> Result<Self, CacheError> {
        Ok(Self {
            software_version: SOFTWARE_VERSION.to_owned(),
            model_bundle_id: identity.model_bundle_id.clone(),
            model_profile: identity.model_profile.clone(),
            model_representation: identity.model_representation.clone(),
            reference_bundle_id: identity.reference_bundle_id.clone(),
            reference_profile: identity.reference_profile.clone(),
            reference_sequence_set_sha256: identity.reference_sequence_set_sha256.clone(),
            mask_bytes: i64::try_from(identity.mask_bytes).map_err(|_| CacheError::InvalidRow)?,
            mask_sha256: identity.mask_sha256.clone(),
            scoring_semantics: SCORING_SEMANTICS.to_owned(),
            masking_policy: MASKING_POLICY.to_owned(),
            window: i64::from(DISTANCE_WINDOW),
        })
    }

    fn read(connection: &Connection) -> Result<Self, CacheError> {
        connection
            .query_row("SELECT * FROM setup WHERE singleton=1", [], |row| {
                Ok(Self {
                    software_version: row.get(1)?,
                    model_bundle_id: row.get(2)?,
                    model_profile: row.get(3)?,
                    model_representation: row.get(4)?,
                    reference_bundle_id: row.get(5)?,
                    reference_profile: row.get(6)?,
                    reference_sequence_set_sha256: row.get(7)?,
                    mask_bytes: row.get(8)?,
                    mask_sha256: row.get(9)?,
                    scoring_semantics: row.get(10)?,
                    masking_policy: row.get(11)?,
                    window: row.get(12)?,
                })
            })
            .map_err(map_sqlite)
    }

    fn record(&self, connection: &Connection) -> Result<(), CacheError> {
        connection
            .execute(
                "INSERT OR IGNORE INTO setup VALUES(
                   1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12
                 )",
                params![
                    self.software_version,
                    self.model_bundle_id,
                    self.model_profile,
                    self.model_representation,
                    self.reference_bundle_id,
                    self.reference_profile,
                    self.reference_sequence_set_sha256,
                    self.mask_bytes,
                    self.mask_sha256,
                    self.scoring_semantics,
                    self.masking_policy,
                    self.window
                ],
            )
            .map_err(map_sqlite)?;
        Ok(())
    }
}

/// A row is found by the submitted variant alone. The setup is judged once,
/// when the cache opens, not once per row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheKey {
    contig: String,
    position: u32,
    reference: String,
    alternate: String,
}

impl CacheKey {
    pub fn new(variant: &Grch38Variant) -> Self {
        Self {
            contig: variant.contig().to_string(),
            position: variant.position().get(),
            reference: variant.reference().to_owned(),
            alternate: variant.alternate().to_owned(),
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
    setup: CacheIdentity,
    limit: EntryLimit,
    disposable_default: bool,
    counters: CacheCounters,
    pending_checkpoint: bool,
    discarded: bool,
    unreadable: bool,
    earlier_layout: bool,
    /// The file this cache judged, as `(dev, ino)` read just after the open.
    judged: (u64, u64),
    retired: bool,
}

/// What one open found. A cache is handed back for the two cases that opened
/// one; an earlier layout is recognized from the file's stamp before any table
/// is read, so nothing is opened for it.
enum Opened {
    Matching(ModelResultCache),
    OtherSetup(ModelResultCache),
    EarlierLayout,
}

impl ModelResultCache {
    /// Open an explicitly selected cache. Incompatible/corrupt explicit
    /// databases are returned to the caller rather than deleted. A cache a
    /// different setup filled, including one an earlier release wrote, is
    /// discarded whole, the way a disposable one is: the caller is told so it
    /// can report it, not left reading rows the running setup would never
    /// write.
    pub fn open_explicit(
        path: &Path,
        setup: &CacheIdentity,
        limit: EntryLimit,
    ) -> Result<Self, CacheError> {
        Self::open_matching(path, setup, limit, false)
    }

    /// Open the disposable default cache, recreating it once if incompatible
    /// or corrupt.
    pub fn open_default(
        path: &Path,
        setup: &CacheIdentity,
        limit: EntryLimit,
    ) -> Result<Self, CacheError> {
        match Self::open_matching(path, setup, limit, true) {
            Ok(cache) => Ok(cache),
            Err(CacheError::Incompatible | CacheError::Sqlite(_)) => {
                // The file could not be read at all: not a database, another
                // application's, or a layout no release of this software wrote.
                // The reopen creates a fresh file and so matches, which is why
                // the discard has to be recorded here rather than in `replace`.
                remove_database_family(path)?;
                let mut cache = Self::open_matching(path, setup, limit, true)?;
                cache.unreadable = true;
                Ok(cache)
            }
            Err(error) => Err(error),
        }
    }

    /// Whether opening this cache discarded a file another setup had filled.
    pub fn discarded_earlier_setup(&self) -> bool {
        self.discarded
    }

    /// Whether opening this cache destroyed a file this build could not read.
    /// A separate cause from a setup change and reported separately: an
    /// operator told that another setup filled a corrupt file goes looking for
    /// an upgrade nobody made.
    pub fn discarded_unreadable_file(&self) -> bool {
        self.unreadable
    }

    /// Whether opening this cache discarded a file an earlier layout wrote. Its
    /// own cause and reported separately: nothing about the assets changed, the
    /// release did, and an operator told another setup filled the file goes
    /// looking for an upgrade nobody made instead of reading the release note.
    pub fn discarded_earlier_layout(&self) -> bool {
        self.earlier_layout
    }

    /// Judge the recorded setup once, at open. On any difference the whole file
    /// goes, leaving no earlier row readable, and the reopened file records the
    /// running setup. No migration keeps old rows readable and none is wanted.
    fn open_matching(
        path: &Path,
        setup: &CacheIdentity,
        limit: EntryLimit,
        create_parent: bool,
    ) -> Result<Self, CacheError> {
        match Self::open_inner(path, setup, limit, create_parent)? {
            Opened::Matching(cache) => Ok(cache),
            Opened::OtherSetup(cache) => {
                drop(cache);
                let mut cache = Self::replace(path, setup, limit, create_parent)?;
                cache.discarded = true;
                Ok(cache)
            }
            Opened::EarlierLayout => {
                let mut cache = Self::replace(path, setup, limit, create_parent)?;
                cache.earlier_layout = true;
                Ok(cache)
            }
        }
    }

    /// Throw the whole file away and open a fresh one recording the running
    /// setup. The caller raises the flag for the cause it found, so the report
    /// names that cause rather than standing for both.
    fn replace(
        path: &Path,
        setup: &CacheIdentity,
        limit: EntryLimit,
        create_parent: bool,
    ) -> Result<Self, CacheError> {
        remove_database_family(path)?;
        let Opened::Matching(cache) = Self::open_inner(path, setup, limit, create_parent)? else {
            return Err(CacheError::Incompatible);
        };
        Ok(cache)
    }

    /// Open once and report whether the file's recorded setup is the running
    /// setup. A file this call created records the running setup and matches.
    fn open_inner(
        path: &Path,
        setup: &CacheIdentity,
        limit: EntryLimit,
        create_parent: bool,
    ) -> Result<Opened, CacheError> {
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
        if application_id != 0 && application_id != APPLICATION_ID {
            return Err(CacheError::Incompatible);
        }
        if user_version != 0 && user_version != USER_VERSION {
            // Our own file under a layout another release wrote. A software
            // change is what the recorded setup exists to catch, so discard it
            // whole and say so rather than refusing the run. Anything else --
            // a foreign database, a damaged one, a layout this build does not
            // know -- stays incompatible.
            if application_id == APPLICATION_ID && EARLIER_USER_VERSIONS.contains(&user_version) {
                return Ok(Opened::EarlierLayout);
            }
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
                     CREATE TABLE IF NOT EXISTS setup (
                   singleton INTEGER PRIMARY KEY CHECK(singleton=1),
                   software_version TEXT NOT NULL,
                   model_bundle_id TEXT NOT NULL,
                   model_profile TEXT NOT NULL,
                   model_representation TEXT NOT NULL,
                   reference_bundle_id TEXT NOT NULL,
                   reference_profile TEXT NOT NULL,
                   reference_sequence_set_sha256 TEXT NOT NULL,
                   mask_bytes INTEGER NOT NULL,
                   mask_sha256 TEXT NOT NULL,
                   scoring_semantics TEXT NOT NULL,
                   masking_policy TEXT NOT NULL,
                   window INTEGER NOT NULL
                 ) STRICT;
                     CREATE TABLE IF NOT EXISTS entries (
                   key_digest TEXT PRIMARY KEY,
                   key_json BLOB NOT NULL,
                   contig TEXT NOT NULL,
                   position INTEGER NOT NULL,
                   reference TEXT NOT NULL,
                   alternate TEXT NOT NULL,
                   value_json BLOB NOT NULL,
                   write_sequence INTEGER NOT NULL CHECK(write_sequence > 0)
                 ) STRICT;"
                ))
                .map_err(map_sqlite)?;
        }
        let running = RecordedSetup::running(setup)?;
        if !initialized {
            running.record(&connection)?;
        }
        validate_schema(&connection)?;
        let matched = RecordedSetup::read(&connection)? == running;
        set_family_permissions(path)?;
        let judged = file_identity(path)?;
        let mut cache = Self {
            connection,
            path: path.to_owned(),
            setup: setup.clone(),
            limit,
            disposable_default: create_parent,
            counters: CacheCounters::default(),
            pending_checkpoint: !initialized,
            discarded: false,
            unreadable: false,
            earlier_layout: false,
            judged,
            retired: false,
        };
        if !matched {
            // Nothing this open may keep. `open_matching` throws this file
            // away whole and `holds_the_file_at_its_path` only reads the
            // verdict off it, so trimming it to the running limit would only
            // destroy rows -- rows that belong to whichever process actually
            // judged the file, when the caller is a running cache probing a
            // replacement it is about to walk away from.
            return Ok(Opened::OtherSetup(cache));
        }
        cache.evict_to_limit().map_err(map_sqlite)?;
        Ok(Opened::Matching(cache))
    }

    pub fn get(&mut self, key: &CacheKey) -> Result<Option<Vec<ModelGeneScoreRecord>>, CacheError> {
        if !self.holds_the_file_at_its_path() {
            self.counters.misses += 1;
            return Ok(None);
        }
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
        let row: Option<BoundedStoredRow> = self
            .connection
            .query_row(
                "SELECT
                   CASE WHEN length(key_json)<=16384 THEN key_json END,
                   CASE WHEN length(value_json)<=1048576 THEN value_json END
                 FROM entries WHERE
                   key_digest=?1 AND contig=?2 AND position=?3 AND reference=?4
                   AND alternate=?5",
                params![
                    digest,
                    key.contig,
                    key.position,
                    key.reference,
                    key.alternate
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
        let Opened::Matching(mut replacement) =
            Self::open_inner(&self.path, &self.setup, self.limit, true)?
        else {
            return Err(CacheError::Incompatible);
        };
        self.connection = std::mem::replace(
            &mut replacement.connection,
            Connection::open_in_memory().map_err(map_sqlite)?,
        );
        self.pending_checkpoint = replacement.pending_checkpoint;
        // The file at the path is the one this call just created, so the
        // captured identity has to follow it or the next operation would judge
        // its own recovery a foreign replacement and retire.
        self.judged = replacement.judged;
        report_once(&format!(
            "destroyed model cache {}: it could not be read after it opened",
            self.path.display()
        ));
        Ok(())
    }

    /// Stop touching the file at this path for the life of this cache, and say
    /// so. Nothing was destroyed and whatever now sits at the path is left
    /// where it is, so a process that goes cold here has a restart as its lever
    /// and nothing else would say so.
    fn retire(&mut self) -> bool {
        self.retired = true;
        report_once(&format!(
            "stopped using model cache {}: it no longer holds the file it opened, so restart to \
             use what is there now",
            self.path.display()
        ));
        false
    }

    /// Whether this cache may still touch the file at its path.
    ///
    /// The recorded setup is judged once, at the open, and that judgement is
    /// about the file the connection opened. Another process discarding that
    /// file unlinks it and creates a fresh one at the same path, so the
    /// connection goes on reading rows out of a file nobody can reach and
    /// writing rows that die with the process. One `stat` per operation, never
    /// per row, catches it.
    ///
    /// On a difference the path is re-opened once through `open_inner`, which
    /// reads the replacement's recorded setup and destroys nothing. An exact
    /// setup match is taken up, so every row served afterwards was written
    /// under the running setup; that is what a peer recovering from a file it
    /// could not read leaves behind. Any other setup, an earlier layout, an
    /// unreadable file, or a path that no longer names a file retires the cache
    /// for the life of the process. Nothing is created to replace what went.
    fn holds_the_file_at_its_path(&mut self) -> bool {
        if self.retired {
            return false;
        }
        match file_identity(&self.path) {
            Ok(current) if current == self.judged => return true,
            Ok(_) => {}
            Err(_) => return self.retire(),
        }
        match Self::open_inner(&self.path, &self.setup, self.limit, self.disposable_default) {
            Ok(Opened::Matching(mut replacement)) => {
                let Ok(placeholder) = Connection::open_in_memory() else {
                    return self.retire();
                };
                self.connection = std::mem::replace(&mut replacement.connection, placeholder);
                self.judged = replacement.judged;
                self.pending_checkpoint = replacement.pending_checkpoint;
                true
            }
            _ => self.retire(),
        }
    }

    /// Store a successful complete result. A write failure is deliberately
    /// reported separately so callers can retain their already-computed answer.
    pub fn put(
        &mut self,
        key: &CacheKey,
        records: &[ModelGeneScoreRecord],
    ) -> Result<(), CacheError> {
        if !self.holds_the_file_at_its_path() {
            return Ok(());
        }
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
                   value_json,write_sequence
                 )
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
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
    validate_table_shape(
        connection,
        "setup",
        &[
            ColumnShape::new("singleton", "INTEGER", false, 1),
            ColumnShape::new("software_version", "TEXT", true, 0),
            ColumnShape::new("model_bundle_id", "TEXT", true, 0),
            ColumnShape::new("model_profile", "TEXT", true, 0),
            ColumnShape::new("model_representation", "TEXT", true, 0),
            ColumnShape::new("reference_bundle_id", "TEXT", true, 0),
            ColumnShape::new("reference_profile", "TEXT", true, 0),
            ColumnShape::new("reference_sequence_set_sha256", "TEXT", true, 0),
            ColumnShape::new("mask_bytes", "INTEGER", true, 0),
            ColumnShape::new("mask_sha256", "TEXT", true, 0),
            ColumnShape::new("scoring_semantics", "TEXT", true, 0),
            ColumnShape::new("masking_policy", "TEXT", true, 0),
            ColumnShape::new("window", "INTEGER", true, 0),
        ],
    )?;
    let expected = [
        ColumnShape::new("key_digest", "TEXT", true, 1),
        ColumnShape::new("key_json", "BLOB", true, 0),
        ColumnShape::new("contig", "TEXT", true, 0),
        ColumnShape::new("position", "INTEGER", true, 0),
        ColumnShape::new("reference", "TEXT", true, 0),
        ColumnShape::new("alternate", "TEXT", true, 0),
        ColumnShape::new("value_json", "BLOB", true, 0),
        ColumnShape::new("write_sequence", "INTEGER", true, 0),
    ];
    validate_table_shape(connection, "entries", &expected)?;
    let recorded_setups: i64 = connection
        .query_row("SELECT count(*) FROM setup", [], |row| row.get(0))
        .map_err(map_sqlite)?;
    if recorded_setups != 1 {
        return Err(CacheError::Incompatible);
    }
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
    debug_assert!(matches!(table, "metadata" | "setup" | "entries"));
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

/// Which file the path names, as `(dev, ino)`.
///
/// rusqlite exposes no descriptor, so this is a `stat` of the path rather than
/// an `fstat` of the connection's own file: a replacement landing between the
/// open and this call would be captured instead of the file actually held.
/// `st_ino` is reused freely once nothing holds the old file, and is not stable
/// on every filesystem; the cache lives under `XDG_CACHE_HOME`, which is local
/// on the supported platforms, and a cache whose file is being replaced is by
/// definition still holding it, so its number cannot be handed out underneath
/// it.
fn file_identity(path: &Path) -> io::Result<(u64, u64)> {
    let metadata = fs::metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

/// Say once for this process what happened to a cache file, on the stream the
/// reports at open already use. A process opens one cache per worker beside the
/// one its handler holds, and they all share the file, so a report per cache
/// would say the same thing several times over; a report per lookup would
/// repeat it for the rest of the run.
fn report_once(sentence: &str) {
    static SAID: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let mut said = SAID
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if said.insert(sentence.to_owned()) {
        eprintln!("{sentence}");
    }
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

    fn setup() -> CacheIdentity {
        CacheIdentity::new(
            &format!("sha256:{:064x}", 1),
            "model",
            "singleton",
            &format!("sha256:{:064x}", 2),
            "reference",
            &format!("sha256:{:064x}", 3),
            260,
            &format!("sha256:{:064x}", 4),
        )
        .expect("identity")
    }

    fn key(position: u32) -> CacheKey {
        let variant = Grch38Variant::new(
            "chr1".parse().expect("contig"),
            GenomicPosition::new(position).expect("position"),
            "A",
            "AC",
        )
        .expect("variant");
        CacheKey::new(&variant)
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
            let mut cache = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
                .expect("open");
            assert_eq!(cache.get(&key(10)).expect("miss"), None);
            cache.put(&key(10), &records()).expect("put");
            assert_eq!(cache.counters().misses, 1);
        }
        let mut reopened = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
            .expect("reopen");
        assert_eq!(reopened.get(&key(10)).expect("hit"), Some(records()));
        assert_eq!(reopened.counters().hits, 1);
    }

    #[test]
    fn valid_hits_do_not_mutate_database_wal_or_write_sequence() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let wal = PathBuf::from(format!("{}-wal", path.display()));
        let mut cache =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::Bounded(2)).expect("open");
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

        let mut reduced = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::Bounded(1))
            .expect("reduced");
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
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::Unlimited).expect("open");
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
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::Bounded(2)).expect("open");
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
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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
            ModelResultCache::open_explicit(&explicit_path, &setup(), EntryLimit::default()),
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
        let cache =
            ModelResultCache::open_default(&disposable_path, &setup(), EntryLimit::default())
                .expect("recreated");
        assert_eq!(cache.entry_count().expect("empty"), 0);

        let corrupt = private_temp();
        let corrupt_path = corrupt.path().join("cache.sqlite3");
        fs::write(&corrupt_path, b"not sqlite").expect("corrupt bytes");
        fs::set_permissions(&corrupt_path, fs::Permissions::from_mode(0o600)).expect("mode");
        let cache = ModelResultCache::open_default(&corrupt_path, &setup(), EntryLimit::default())
            .expect("corrupt default recreated");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn established_cache_reopen_validates_metadata_contract() {
        for table in ["metadata", "setup"] {
            let temp = private_temp();
            let path = temp.path().join("cache.sqlite3");
            drop(
                ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
                    .expect("initialize cache"),
            );
            let connection = Connection::open(&path).expect("reopen raw database");
            connection
                .execute(&format!("DROP TABLE {table}"), [])
                .expect("remove required table");
            drop(connection);
            assert!(matches!(
                ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()),
                Err(CacheError::Incompatible)
            ));
        }

        let sequence = private_temp();
        let sequence_path = sequence.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&sequence_path, &setup(), EntryLimit::default())
                .expect("open");
        cache.put(&key(1), &records()).expect("put");
        cache
            .connection
            .execute("UPDATE metadata SET next_write_sequence=1", [])
            .expect("invalidate sequence");
        drop(cache);
        assert!(matches!(
            ModelResultCache::open_explicit(&sequence_path, &setup(), EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));
    }

    #[test]
    fn same_named_wrong_shape_schema_is_rejected_or_recreated() {
        fn mutate_declared_type(path: &Path) {
            drop(
                ModelResultCache::open_explicit(path, &setup(), EntryLimit::default())
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
            ModelResultCache::open_explicit(&explicit_path, &setup(), EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));

        let disposable = private_temp();
        let disposable_path = disposable.path().join("cache.sqlite3");
        mutate_declared_type(&disposable_path);
        let cache =
            ModelResultCache::open_default(&disposable_path, &setup(), EntryLimit::default())
                .expect("default recreates wrong-shape schema");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn busy_write_is_reported_without_damaging_existing_rows() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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
        let mut cache =
            ModelResultCache::open_default(&path, &setup(), EntryLimit::default()).expect("open");
        cache.put(&key(1), &records()).expect("put");
        cache
            .connection
            .execute_batch("DROP TABLE entries")
            .expect("simulate damaged database");
        assert_eq!(cache.get(&key(1)).expect("recovered miss"), None);
        assert_eq!(cache.entry_count().expect("recreated"), 0);
    }

    /// Another setup, differing in one recorded value. A cache opened under it
    /// at a filled path discards that file whole, the way ticket 0058 settled.
    fn other_setup() -> CacheIdentity {
        CacheIdentity::new(
            &format!("sha256:{:064x}", 9),
            "model",
            "singleton",
            &format!("sha256:{:064x}", 2),
            "reference",
            &format!("sha256:{:064x}", 3),
            260,
            &format!("sha256:{:064x}", 4),
        )
        .expect("identity")
    }

    // The setup is judged when the file opens, which is what keeps a hit cheap.
    // That judgement covers the file it was made about and nothing else. Once
    // another setup has discarded that file and put its own in its place, the
    // rows the first cache still holds open belong to a file no longer at the
    // path, and it may not answer from them. It must not answer from the
    // replacement either: that file was judged by the process that made it, not
    // by this one.
    #[test]
    fn a_cache_stops_answering_once_its_file_has_been_replaced() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut held =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
        held.put(&key(10), &records()).expect("put");
        assert_eq!(
            held.get(&key(10)).expect("hit"),
            Some(records()),
            "the row has to be found before the file is replaced, or a later miss proves nothing"
        );

        let mut replacing =
            ModelResultCache::open_explicit(&path, &other_setup(), EntryLimit::default())
                .expect("replacing open");
        assert!(
            replacing.discarded_earlier_setup(),
            "the second open must have discarded the file the first one is holding open"
        );
        replacing.put(&key(20), &records()).expect("refill");

        assert_eq!(
            held.get(&key(10)).expect("read after replacement"),
            None,
            "a cache whose file another setup discarded must stop answering from the rows it \
             still holds open"
        );
        assert_eq!(
            held.get(&key(20)).expect("read after replacement"),
            None,
            "and must not reach into the file that took its place, which it never judged"
        );
        assert_eq!(
            replacing.get(&key(20)).expect("hit"),
            Some(records()),
            "the process that judged the file at the path keeps its own rows"
        );
    }

    // Retiring is not destroying. The file the replacing process filled is the
    // one every later open judges, so a cache that has stopped answering must
    // leave it exactly as it found it.
    #[test]
    fn a_cache_that_stopped_answering_leaves_the_file_that_replaced_it_alone() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut held =
            ModelResultCache::open_default(&path, &setup(), EntryLimit::default()).expect("open");
        held.put(&key(10), &records()).expect("put");
        {
            let mut replacing =
                ModelResultCache::open_default(&path, &other_setup(), EntryLimit::default())
                    .expect("replacing open");
            replacing.put(&key(20), &records()).expect("refill");
        }

        assert_eq!(
            held.get(&key(10)).expect("read"),
            None,
            "the held cache has to have stopped answering before what it does next means anything"
        );
        let _ = held.put(&key(30), &records());

        let mut reader =
            ModelResultCache::open_explicit(&path, &other_setup(), EntryLimit::default())
                .expect("reopen");
        assert!(
            !reader.discarded_earlier_setup(),
            "the retired cache must not have destroyed or restamped the file that replaced it"
        );
        assert_eq!(
            reader.get(&key(20)).expect("hit"),
            Some(records()),
            "the row the replacing process paid for must still be there"
        );
        assert_eq!(
            reader.get(&key(30)).expect("read"),
            None,
            "a retired cache must not write into a file it never judged"
        );
    }

    // A disposable default that throws its own file away and opens a fresh one
    // is holding the file at its path, not a replaced one. It has to go on
    // storing and finding rows.
    //
    // A second handle is held on the file across the recreate on purpose. The
    // inode of an unlinked file is freed the moment nothing holds it, and this
    // filesystem hands the very same number straight back, so a recreate with
    // nothing holding the old file lands on the same identity and a cache that
    // never refreshed what it captured would look correct. Holding the old file
    // open pins that number, forcing the recreated file to a different one, so
    // this test fails if the recreate does not refresh the captured identity.
    #[test]
    fn a_disposable_default_that_recreated_itself_still_stores_and_finds_rows() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_default(&path, &setup(), EntryLimit::default()).expect("open");
        let pinned = fs::File::open(&path).expect("pin the file the recreate throws away");
        let before = path.metadata().expect("identity before").ino();
        cache
            .connection
            .execute_batch("DROP TABLE entries")
            .expect("simulate damaged database");
        assert_eq!(cache.get(&key(1)).expect("recovered miss"), None);
        assert_ne!(
            path.metadata().expect("identity after").ino(),
            before,
            "the recreate has to land on a file of another identity, or this test cannot tell a \
             refreshed capture from a stale one"
        );
        drop(pinned);

        cache.put(&key(1), &records()).expect("put after recreate");
        assert_eq!(
            cache.get(&key(1)).expect("hit after recreate"),
            Some(records()),
            "a cache that recreated its own file must go on answering from it"
        );
    }

    // A replacement that records the running setup is not a foreign file. The
    // shape a peer leaves behind when it recovers from a file it could not read
    // (ticket 0067) or from a runtime failure: the file at the path is gone and
    // a fresh one recording this same setup stands in its place. Retiring here
    // would cost a long-running service its cache for the rest of its life
    // because another process repaired something, so the cache takes up the
    // file at its path instead. Every row it serves afterwards was still
    // written under its own setup, which is all ticket 0058 ever promised.
    #[test]
    fn a_cache_takes_up_a_replacement_that_records_its_own_setup() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut held =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
        let judged = path.metadata().expect("the file it judged").ino();
        held.put(&key(10), &records()).expect("put");
        assert_eq!(
            held.get(&key(10)).expect("hit"),
            Some(records()),
            "the row has to be found before the file is replaced, or a later miss proves nothing"
        );

        // What a peer's recovery leaves behind: the file the held cache judged
        // is unlinked and a fresh one recording this same setup is at the path.
        remove_database_family(&path).expect("the peer throws the file away");
        let peer = ModelResultCache::open_default(&path, &setup(), EntryLimit::default())
            .expect("the peer opens a fresh file");
        assert!(
            !peer.discarded_earlier_setup(),
            "the peer created a fresh file rather than discarding another setup's, so this \
             fixture must not be exercising a setup change"
        );
        assert_ne!(
            path.metadata().expect("identity").ino(),
            judged,
            "the file at the path must really be another file, or this test proves nothing"
        );
        drop(peer);

        held.put(&key(20), &records())
            .expect("put after replacement");
        assert_eq!(
            held.get(&key(10)).expect("read after replacement"),
            None,
            "the rows that died with the replaced file are gone, and a cache that answered from \
             them would still be reading a file no longer at its path"
        );
        assert_eq!(
            held.get(&key(20)).expect("read after replacement"),
            Some(records()),
            "a cache whose file was replaced by one recording its own setup goes on working"
        );

        let mut reader = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
            .expect("reopen");
        assert_eq!(
            reader.get(&key(20)).expect("hit"),
            Some(records()),
            "and what it writes lands in the file at its path, not in one only it can see"
        );
    }

    // Retiring reads a verdict off the file at the path; it does not take the
    // file over. The bounded limit belongs to this process, and the rows in a
    // file another setup filled belong to the process that judged it, so the
    // probe that ends in retirement must leave every one of them where it is.
    // A default limit is far above what a fixture writes, so the limit here is
    // small enough that an eviction would show.
    #[test]
    fn a_cache_that_retires_evicts_nothing_from_the_file_it_walked_away_from() {
        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut held =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::Bounded(1)).expect("open");
        held.put(&key(1), &records()).expect("put");
        assert_eq!(held.get(&key(1)).expect("hit"), Some(records()));

        let mut replacing =
            ModelResultCache::open_explicit(&path, &other_setup(), EntryLimit::Unlimited)
                .expect("replacing open");
        assert!(replacing.discarded_earlier_setup());
        for n in 10..15 {
            replacing.put(&key(n), &records()).expect("refill");
        }
        assert_eq!(replacing.entry_count().expect("count"), 5);
        drop(replacing);

        assert_eq!(held.get(&key(1)).expect("read"), None, "must retire");

        let mut reader =
            ModelResultCache::open_explicit(&path, &other_setup(), EntryLimit::Unlimited)
                .expect("reopen");
        assert!(!reader.discarded_earlier_setup());
        assert_eq!(
            reader.entry_count().expect("count"),
            5,
            "the retiring cache must not have evicted rows from a file it never judged"
        );
        for n in 10..15 {
            assert_eq!(
                reader.get(&key(n)).expect("read"),
                Some(records()),
                "row {n} the replacing process paid for is still readable"
            );
        }
    }

    #[test]
    fn unsafe_paths_are_rejected_and_family_is_private() {
        let temp = private_temp();
        let target = temp.path().join("target.sqlite3");
        fs::write(&target, []).expect("target");
        let link = temp.path().join("link.sqlite3");
        symlink(&target, &link).expect("symlink");
        assert!(matches!(
            ModelResultCache::open_explicit(&link, &setup(), EntryLimit::default()),
            Err(CacheError::UnsafePath(_))
        ));

        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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
            ModelResultCache::open_explicit(&explicit_path, &setup(), EntryLimit::default()),
            Err(CacheError::Incompatible)
        ));

        let disposable = private_temp();
        let disposable_path = disposable.path().join("cache.sqlite3");
        let disposable_wal = PathBuf::from(format!("{}-wal", disposable_path.display()));
        fs::write(&disposable_wal, b"stale").expect("stale WAL");
        fs::set_permissions(&disposable_wal, fs::Permissions::from_mode(0o600)).expect("mode");
        let cache =
            ModelResultCache::open_default(&disposable_path, &setup(), EntryLimit::default())
                .expect("default replaces orphan");
        assert_eq!(cache.entry_count().expect("empty"), 0);
    }

    #[test]
    fn every_submitted_variant_field_causes_an_actual_cache_miss() {
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
            variants
        }

        let temp = private_temp();
        let path = temp.path().join("cache.sqlite3");
        let mut cache =
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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

    /// The other half of the same rule. The row key carries no identity any
    /// more, so every identity is judged once, when the file opens. Each
    /// recorded field on its own has to take the whole file with it, including
    /// the software version the key never carried.
    #[test]
    fn every_recorded_setup_field_discards_the_whole_file() {
        use rusqlite::types::Value;

        for (column, replacement) in [
            ("software_version", Value::Text("0.0.0".to_owned())),
            (
                "model_bundle_id",
                Value::Text(format!("sha256:{:064x}", 11)),
            ),
            ("model_profile", Value::Text("other-model".to_owned())),
            (
                "model_representation",
                Value::Text("zero-padded-batch".to_owned()),
            ),
            (
                "reference_bundle_id",
                Value::Text(format!("sha256:{:064x}", 12)),
            ),
            (
                "reference_profile",
                Value::Text("other-reference".to_owned()),
            ),
            (
                "reference_sequence_set_sha256",
                Value::Text(format!("sha256:{:064x}", 13)),
            ),
            ("mask_bytes", Value::Integer(261)),
            ("mask_sha256", Value::Text(format!("sha256:{:064x}", 14))),
            (
                "scoring_semantics",
                Value::Text("other-score-v1".to_owned()),
            ),
            ("masking_policy", Value::Text("other-mask-v1".to_owned())),
            ("window", Value::Integer(49)),
        ] {
            let temp = private_temp();
            let path = temp.path().join("cache.sqlite3");
            let mut cache = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
                .expect("open");
            cache.put(&key(1), &records()).expect("put");
            drop(cache);

            let connection = Connection::open(&path).expect("raw open");
            connection
                .execute(
                    &format!("UPDATE setup SET {column}=?1 WHERE singleton=1"),
                    params![replacement],
                )
                .expect("rewrite one recorded field");
            drop(connection);

            let mut cache = ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default())
                .expect("reopen");
            assert!(
                cache.discarded_earlier_setup(),
                "a changed {column} must be reported as a discard"
            );
            assert_eq!(
                cache.entry_count().expect("row count"),
                0,
                "a changed {column} must leave no earlier row readable"
            );
            assert_eq!(cache.get(&key(1)).expect("lookup"), None);
        }
    }

    #[test]
    fn public_key_construction_rejects_invalid_scoring_identity() {
        let good_sha = format!("sha256:{:064x}", 1);
        let build = |model_bundle: &str,
                     model_profile: &str,
                     representation: &str,
                     reference_bundle: &str,
                     reference_profile: &str,
                     sequence_sha: &str,
                     mask_bytes: u64,
                     mask_sha: &str| {
            CacheIdentity::new(
                model_bundle,
                model_profile,
                representation,
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
            ModelResultCache::open_explicit(&path, &setup(), EntryLimit::default()).expect("open");
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

    /// A row only an earlier release could have written, recognizable wherever
    /// it survives.
    const EARLIER_ROW: &str = "row-written-by-an-earlier-release";

    /// Every layout this build claims to recognize is exercised, not only the
    /// newest one before it. The list is read rather than spelled, so a layout
    /// appended to it is proved the moment it is appended, and
    /// `tests/model-cache-layout-history.sh` holds the list itself to what the
    /// repository's history shows was written.
    ///
    /// The file goes whole, and the cause it goes for is its own. An earlier
    /// layout is this software's own file read by a release that writes another
    /// shape; another setup is an asset or a release change under a run.
    /// `discarded_earlier_setup` answers for the second cause alone, so a
    /// caller reading it can print the sentence it names. The sentence an
    /// earlier layout earns is read from a real run in
    /// `crates/pangopup-cli/tests/model_cache_setup.rs`.
    #[test]
    fn every_layout_an_earlier_release_wrote_is_discarded_whole_and_is_not_another_setup() {
        assert!(
            !EARLIER_USER_VERSIONS.is_empty(),
            "this build stamps layout {USER_VERSION}, so a layout before it was written and is \
             sitting on someone's disk; an empty list makes every one of those files foreign, \
             which is a refusal on a chosen path and a silent deletion on the default one"
        );
        for layout in EARLIER_USER_VERSIONS {
            assert_ne!(
                layout, USER_VERSION,
                "layout {layout} is the layout this build writes, so naming it as an earlier one \
                 throws away every file this build itself wrote"
            );
            let temp = private_temp();
            let path = temp.path().join("cache.sqlite3");
            let connection = Connection::open(&path).expect("create an earlier-layout cache");
            connection
                .execute_batch(&format!(
                    "PRAGMA application_id={APPLICATION_ID};
                     PRAGMA user_version={layout};
                     CREATE TABLE earlier (row TEXT NOT NULL);
                     INSERT INTO earlier VALUES('{EARLIER_ROW}');"
                ))
                .expect("write the earlier layout");
            drop(connection);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode");

            let cache = ModelResultCache::open_default(&path, &setup(), EntryLimit::default())
                .unwrap_or_else(|error| {
                    panic!(
                        "layout {layout} is this software's own file from another release, so it \
                         must be discarded rather than refused: {error}"
                    )
                });
            assert!(
                !cache.discarded_earlier_setup(),
                "the file layout {layout} wrote went because this release reads the on-disk \
                 shape differently from the release that wrote it, and the release note is what \
                 explains that; answering for it here sends the operator after an asset change \
                 nobody made and away from the note that would have answered them"
            );
            assert_eq!(
                cache.entry_count().expect("row count"),
                0,
                "the file layout {layout} wrote is discarded whole"
            );
            drop(cache);
            let bytes = fs::read(&path).expect("read the refilled file");
            assert!(
                !bytes
                    .windows(EARLIER_ROW.len())
                    .any(|window| window == EARLIER_ROW.as_bytes()),
                "no row layout {layout} wrote may stay readable"
            );
        }
    }
}
