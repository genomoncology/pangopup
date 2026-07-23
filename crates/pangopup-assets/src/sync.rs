//! Pinned, resumable download of the immutable production SNV transport.

use super::release::ProfileMember;
use super::{AssetError, AssetErrorKind, ReleaseProfile, install_transport, open_active_bundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[cfg(test)]
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::PermissionsExt,
};

const CACHE_SCHEMA: &str = "pangopup.asset-resume.v1";
const CACHE_DIR_MODE: u32 = 0o700;
const CACHE_FILE_MODE: u32 = 0o600;
const COMPLETE_FILE_MODE: u32 = 0o444;
const MAX_RESUME_BYTES: u64 = 8 * 1024;
const BUFFER_SIZE: usize = 128 * 1024;
const MAX_REDIRECTS: usize = 5;
static CACHE_NONCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod sync_audit {
    use std::cell::RefCell;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FaultPoint {
        SidecarCreate,
        SidecarSync,
        PartialCreate,
        PartialWrite,
        PartialSync,
        MemberRename,
        MemberDirSync,
        TransportRename,
        TransportDirSync,
        TransportEvict,
    }

    thread_local! {
        static FAULT: RefCell<Option<FaultPoint>> = const { RefCell::new(None) };
        static PREFIX_BYTES: RefCell<u64> = const { RefCell::new(0) };
    }

    pub fn set(point: FaultPoint) {
        FAULT.set(Some(point));
    }

    pub fn hit(point: FaultPoint) {
        let should_crash = FAULT.with_borrow_mut(|fault| {
            if *fault == Some(point) {
                *fault = None;
                true
            } else {
                false
            }
        });
        if should_crash {
            panic!("simulated sync crash at {point:?}");
        }
    }

    pub fn fail(point: FaultPoint) -> bool {
        FAULT.with_borrow_mut(|fault| {
            if *fault == Some(point) {
                *fault = None;
                true
            } else {
                false
            }
        })
    }

    pub fn reset_prefix_bytes() {
        PREFIX_BYTES.set(0);
    }

    pub fn record_prefix_bytes(count: usize) {
        PREFIX_BYTES.with_borrow_mut(|total| *total += count as u64);
    }

    pub fn take_prefix_bytes() -> u64 {
        PREFIX_BYTES.take()
    }
}

#[cfg(test)]
macro_rules! crash_at {
    ($point:ident) => {
        sync_audit::hit(sync_audit::FaultPoint::$point)
    };
}

#[cfg(test)]
macro_rules! fail_at {
    ($point:ident) => {
        sync_audit::fail(sync_audit::FaultPoint::$point)
    };
}

#[cfg(not(test))]
macro_rules! fail_at {
    ($point:ident) => {
        false
    };
}

#[cfg(not(test))]
macro_rules! crash_at {
    ($point:ident) => {};
}

#[derive(Clone, Debug, Default)]
pub struct CachePathInputs {
    pub explicit: Option<OsString>,
    pub pangopup_cache_dir: Option<OsString>,
    pub xdg_cache_home: Option<OsString>,
    pub home: Option<OsString>,
}

impl CachePathInputs {
    pub fn from_environment(explicit: Option<OsString>) -> Self {
        Self {
            explicit,
            pangopup_cache_dir: std::env::var_os("PANGOPUP_CACHE_DIR"),
            xdg_cache_home: std::env::var_os("XDG_CACHE_HOME"),
            home: std::env::var_os("HOME"),
        }
    }
}

/// Validate every present input, then select the first cache root in precedence order.
pub fn resolve_cache_root(inputs: &CachePathInputs) -> Result<Option<PathBuf>, AssetError> {
    let explicit = validate_optional(&inputs.explicit, "--cache-dir")?;
    let direct = validate_optional(&inputs.pangopup_cache_dir, "PANGOPUP_CACHE_DIR")?;
    let xdg = validate_optional(&inputs.xdg_cache_home, "XDG_CACHE_HOME")?;
    let home = validate_optional(&inputs.home, "HOME")?;
    Ok(explicit
        .or(direct)
        .or_else(|| xdg.map(|path| path.join("pangopup")))
        .or_else(|| home.map(|path| path.join(".cache").join("pangopup"))))
}

fn validate_optional(value: &Option<OsString>, label: &str) -> Result<Option<PathBuf>, AssetError> {
    value
        .as_ref()
        .map(|value| absolute_utf8(value, label))
        .transpose()
}

fn absolute_utf8(value: &OsStr, label: &str) -> Result<PathBuf, AssetError> {
    let Some(text) = value.to_str() else {
        return Err(path_invalid(label));
    };
    let path = PathBuf::from(text);
    if text.is_empty() || !path.is_absolute() {
        return Err(path_invalid(label));
    }
    Ok(path)
}

fn path_invalid(label: &str) -> AssetError {
    AssetError::new(
        AssetErrorKind::PathInvalid,
        format!("{label} must be a nonempty absolute UTF-8 path"),
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyncOutcome {
    pub status: &'static str,
    pub profile: String,
    pub bundle_id: String,
    pub transport_id: String,
    pub path: PathBuf,
    pub downloaded_bytes: u64,
    pub resumed_bytes: u64,
}

#[derive(Clone, Debug)]
struct Request {
    url: String,
    range: Option<u64>,
    if_range: Option<String>,
}

struct Response {
    status: u16,
    location: Option<String>,
    etag: Option<String>,
    content_length: Option<u64>,
    content_range: Option<String>,
    content_encoding: Option<String>,
    body: Box<dyn Read + Send>,
}

trait TransportClient {
    fn execute(&self, request: &Request) -> Result<Response, AssetError>;
}

struct UreqClient {
    agent: ureq::Agent,
}

impl UreqClient {
    fn production() -> Self {
        Self::with_timeouts(
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_secs(120),
        )
    }

    fn with_timeouts(connect: Duration, headers: Duration, body_idle: Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .proxy(None)
            .max_redirects(0)
            .http_status_as_error(false)
            .accept_encoding("identity")
            .timeout_connect(Some(connect))
            .timeout_recv_response(Some(headers))
            .timeout_recv_body(Some(body_idle))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl TransportClient for UreqClient {
    fn execute(&self, request: &Request) -> Result<Response, AssetError> {
        let mut builder = self
            .agent
            .get(&request.url)
            .header("Accept-Encoding", "identity");
        if let Some(offset) = request.range {
            builder = builder.header("Range", format!("bytes={offset}-"));
        }
        if let Some(etag) = &request.if_range {
            builder = builder.header("If-Range", etag);
        }
        let response = builder.call().map_err(map_http_error)?;
        let status = response.status().as_u16();
        let headers = response.headers();
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        };
        let content_length = header("content-length")
            .map(|value| value.parse::<u64>())
            .transpose()
            .map_err(|_| download_error("response has invalid Content-Length"))?;
        let location = header("location");
        let etag = header("etag");
        let content_range = header("content-range");
        let content_encoding = header("content-encoding");
        let body = response.into_body().into_reader();
        Ok(Response {
            status,
            location,
            etag,
            content_length,
            content_range,
            content_encoding,
            body: Box::new(body),
        })
    }
}

fn map_http_error(error: ureq::Error) -> AssetError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("timed out") || text.contains("timeout") {
        AssetError::new(AssetErrorKind::AssetTimeout, "asset transfer timed out")
    } else {
        AssetError::new(AssetErrorKind::AssetDownload, "asset transfer failed")
    }
}

#[derive(Clone, Copy)]
struct SyncContract<'a> {
    profile: &'a ReleaseProfile,
    profile_digest: &'a str,
    allowed_hosts: &'a [&'a str],
    require_https: bool,
}

/// Download and install the exact binary-pinned production SNV transport.
pub fn sync_assets(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
) -> Result<SyncOutcome, AssetError> {
    let (_, digest, profile) = super::release::production_profile()?;
    if let Some(outcome) = active_fast_path(data_root, &profile)? {
        return Ok(outcome);
    }
    let client = UreqClient::production();
    sync_with(
        data_root,
        cache_root,
        offline,
        SyncContract {
            profile: &profile,
            profile_digest: digest,
            allowed_hosts: &["github.com", "release-assets.githubusercontent.com"],
            require_https: true,
        },
        &client,
    )
}

fn sync_with(
    data_root: &Path,
    cache_root: Option<&Path>,
    offline: bool,
    contract: SyncContract<'_>,
    client: &dyn TransportClient,
) -> Result<SyncOutcome, AssetError> {
    if let Some(outcome) = active_fast_path(data_root, contract.profile)? {
        return Ok(outcome);
    }
    let cache_root = cache_root.ok_or_else(|| {
        AssetError::new(
            AssetErrorKind::PathUnavailable,
            "no Linux cache directory is available",
        )
    })?;
    let cache = Cache::open(cache_root, contract.profile_digest)?;
    let _lock = cache.lock()?;
    cache.initialize_working_directories()?;

    if let Some(transport) = cache.published_transport(contract.profile)? {
        return install_cached(&cache, &transport, data_root, contract.profile, 0, 0);
    }
    if closed_transport(cache.members_dir()?, contract.profile)? {
        let transport = cache.publish_transport(contract.profile)?;
        return install_cached(&cache, &transport, data_root, contract.profile, 0, 0);
    }
    if offline {
        return Err(missing_error(&cache, contract.profile));
    }

    let mut downloaded = 0_u64;
    let mut resumed = 0_u64;
    for member in &contract.profile.transport.members {
        let result = cache.obtain_member(member, contract, client)?;
        downloaded = downloaded
            .checked_add(result.downloaded)
            .ok_or_else(|| download_error("download byte counter overflow"))?;
        resumed = resumed
            .checked_add(result.resumed)
            .ok_or_else(|| download_error("resume byte counter overflow"))?;
    }
    let transport = cache.publish_transport(contract.profile)?;
    install_cached(
        &cache,
        &transport,
        data_root,
        contract.profile,
        downloaded,
        resumed,
    )
}

fn active_fast_path(
    data_root: &Path,
    profile: &ReleaseProfile,
) -> Result<Option<SyncOutcome>, AssetError> {
    require_linux()?;
    match open_active_bundle(data_root) {
        Ok((active, _))
            if active.bundle_id == profile.bundle.bundle_id
                && active.transport_id == profile.transport.transport_id =>
        {
            Ok(Some(sync_outcome("reused", profile, active.path, 0, 0)))
        }
        Ok(_) => Ok(None),
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => Ok(None),
        Err(error) => Err(error),
    }
}

fn install_cached(
    cache: &Cache,
    transport: &PublishedTransport,
    data_root: &Path,
    profile: &ReleaseProfile,
    downloaded: u64,
    resumed: u64,
) -> Result<SyncOutcome, AssetError> {
    match install_transport(&transport.install_path(), data_root) {
        Ok(installed) => Ok(sync_outcome(
            installed.status,
            profile,
            installed.path,
            downloaded,
            resumed,
        )),
        Err(error) => {
            if cache_content_error(error.kind())
                && let Err(cleanup) = cache.remove_published_transport()
            {
                return Err(AssetError::new(
                    error.kind(),
                    format!("{error}; cached transport eviction also failed: {cleanup}"),
                ));
            }
            Err(error)
        }
    }
}

fn cache_content_error(kind: AssetErrorKind) -> bool {
    matches!(
        kind,
        AssetErrorKind::ManifestInvalid
            | AssetErrorKind::TransportIncompatible
            | AssetErrorKind::PartSetInvalid
            | AssetErrorKind::TransportHashMismatch
            | AssetErrorKind::CompressionInvalid
            | AssetErrorKind::BundleInvalid
    )
}

fn sync_outcome(
    status: &'static str,
    profile: &ReleaseProfile,
    path: PathBuf,
    downloaded_bytes: u64,
    resumed_bytes: u64,
) -> SyncOutcome {
    SyncOutcome {
        status,
        profile: profile.profile.clone(),
        bundle_id: profile.bundle.bundle_id.clone(),
        transport_id: profile.transport.transport_id.clone(),
        path,
        downloaded_bytes,
        resumed_bytes,
    }
}

fn require_linux() -> Result<(), AssetError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(AssetError::new(
            AssetErrorKind::UnsupportedPlatform,
            "asset sync is supported only on Linux",
        ))
    }
}

#[derive(Debug)]
struct Cache {
    profile_dir: SafeDir,
    partial_dir: OnceLock<SafeDir>,
    members_dir: OnceLock<SafeDir>,
    #[cfg(test)]
    partial: PathBuf,
    #[cfg(test)]
    members: PathBuf,
    #[cfg(test)]
    transport: PathBuf,
}

#[derive(Debug)]
struct PublishedTransport {
    dir: SafeDir,
    #[cfg(test)]
    path: PathBuf,
}

impl PublishedTransport {
    fn install_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.dir.file.as_raw_fd()))
    }
}

#[cfg(test)]
impl std::ops::Deref for PublishedTransport {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[derive(Debug)]
struct CacheLock(File);

impl Drop for CacheLock {
    fn drop(&mut self) {
        // SAFETY: flock operates on this live owned descriptor.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

impl Cache {
    fn open(root: &Path, digest: &str) -> Result<Self, AssetError> {
        let identity = digest.strip_prefix("sha256:").filter(|value| {
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        let Some(identity) = identity else {
            return Err(download_error("invalid pinned profile identity"));
        };
        let root_dir = open_or_create_cache_root(root)?;
        let profiles_dir = ensure_private_child(&root_dir, "profiles")?;
        let profile_dir = ensure_private_child(&profiles_dir, identity)?;
        Ok(Self {
            profile_dir,
            partial_dir: OnceLock::new(),
            members_dir: OnceLock::new(),
            #[cfg(test)]
            partial: root.join("profiles").join(identity).join("partial"),
            #[cfg(test)]
            members: root.join("profiles").join(identity).join("members"),
            #[cfg(test)]
            transport: root.join("profiles").join(identity).join("transport"),
        })
    }

    fn initialize_working_directories(&self) -> Result<(), AssetError> {
        let partial = ensure_private_child(&self.profile_dir, "partial")?;
        let members = ensure_private_child(&self.profile_dir, "members")?;
        self.partial_dir
            .set(partial)
            .map_err(|_| asset_state("partial cache was initialized twice"))?;
        self.members_dir
            .set(members)
            .map_err(|_| asset_state("member cache was initialized twice"))?;
        Ok(())
    }

    fn partial_dir(&self) -> Result<&SafeDir, AssetError> {
        self.partial_dir
            .get()
            .ok_or_else(|| asset_state("partial cache is not initialized"))
    }

    fn members_dir(&self) -> Result<&SafeDir, AssetError> {
        self.members_dir
            .get()
            .ok_or_else(|| asset_state("member cache is not initialized"))
    }

    fn lock(&self) -> Result<CacheLock, AssetError> {
        let file = open_or_create_regular(
            &self.profile_dir,
            ".sync.lock",
            libc::O_RDWR,
            CACHE_FILE_MODE,
        )?;
        validate_private_file(&file, &self.profile_dir, CACHE_FILE_MODE)?;
        // SAFETY: flock operates on this live descriptor and retains no pointer.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let error = io::Error::last_os_error();
            return if matches!(error.raw_os_error(), Some(libc::EWOULDBLOCK)) {
                Err(AssetError::new(
                    AssetErrorKind::AssetLocked,
                    "asset cache is already being synchronized",
                ))
            } else {
                Err(asset_io("lock asset cache"))
            };
        }
        Ok(CacheLock(file))
    }

    fn published_transport(
        &self,
        profile: &ReleaseProfile,
    ) -> Result<Option<PublishedTransport>, AssetError> {
        let Some(dir) = open_private_child_optional(&self.profile_dir, "transport")? else {
            return Ok(None);
        };
        if closed_transport(&dir, profile)? {
            Ok(Some(PublishedTransport {
                dir,
                #[cfg(test)]
                path: self.transport.clone(),
            }))
        } else {
            remove_directory_contents(&dir)?;
            remove_dir(&self.profile_dir, "transport")?;
            sync_dir(&self.profile_dir)?;
            Ok(None)
        }
    }

    fn obtain_member(
        &self,
        member: &ProfileMember,
        contract: SyncContract<'_>,
        client: &dyn TransportClient,
    ) -> Result<MemberTransfer, AssetError> {
        validate_component(&member.asset_name)?;
        let completed = member.asset_name.as_str();
        if complete_member_size(self.members_dir()?, completed)? == Some(member.size) {
            return Ok(MemberTransfer::default());
        }
        if entry_exists(self.members_dir()?, completed)? {
            remove_regular_file(self.members_dir()?, completed)?;
        }
        let partial = format!("{}.partial", member.asset_name);
        let sidecar = format!("{}.resume.json", member.asset_name);
        let resume = load_resume(
            self.partial_dir()?,
            &partial,
            &sidecar,
            member,
            contract.profile_digest,
        )?;
        let mut response = follow_redirects(
            client,
            &member.url,
            resume.as_ref().map(|state| state.length),
            resume.as_ref().map(|state| state.record.etag.as_str()),
            contract,
        )?;
        match (resume, response.status) {
            (Some(state), 206) => {
                validate_common_response(&response)?;
                if response.etag.as_deref().filter(|value| strong_etag(value))
                    != Some(state.record.etag.as_str())
                {
                    return Err(download_error("resumed response ETag changed"));
                }
                let expected_range =
                    format!("bytes {}-{}/{}", state.length, member.size - 1, member.size);
                if response.content_range.as_deref() != Some(expected_range.as_str()) {
                    return Err(download_error("resumed response Content-Range is invalid"));
                }
                let suffix = member.size - state.length;
                if response.content_length.is_some_and(|value| value != suffix) {
                    return Err(download_error("resumed response length is invalid"));
                }
                let mut file = open_partial_append(self.partial_dir()?, &partial)?;
                let mut hasher = hash_prefix(&mut file, state.length)?;
                file.seek(SeekFrom::End(0))
                    .map_err(|_| asset_io("seek partial asset"))?;
                let received = stream_exact(
                    &mut response.body,
                    &mut file,
                    &mut hasher,
                    suffix,
                    "resumed asset",
                )?;
                finish_member(
                    file,
                    MemberDestination {
                        source_dir: self.partial_dir()?,
                        source: &partial,
                        completed_dir: self.members_dir()?,
                        completed,
                        sidecar: &sidecar,
                    },
                    member,
                    hasher,
                )?;
                Ok(MemberTransfer {
                    downloaded: received,
                    resumed: state.length,
                })
            }
            (Some(_), 200) => self.stream_fresh_response(
                member,
                contract.profile_digest,
                response,
                completed,
                ExistingPartial {
                    partial: partial.as_str(),
                    sidecar: sidecar.as_str(),
                    preserve: true,
                },
            ),
            (None, 200) => self.stream_fresh_response(
                member,
                contract.profile_digest,
                response,
                completed,
                ExistingPartial {
                    partial: partial.as_str(),
                    sidecar: sidecar.as_str(),
                    preserve: false,
                },
            ),
            (_, 416) => Err(download_error("asset server rejected resume range")),
            (_, 206) => Err(download_error("unexpected partial asset response")),
            _ => Err(download_error("asset server returned an unexpected status")),
        }
    }

    fn stream_fresh_response(
        &self,
        member: &ProfileMember,
        profile_digest: &str,
        mut response: Response,
        completed: &str,
        existing: ExistingPartial<'_>,
    ) -> Result<MemberTransfer, AssetError> {
        validate_common_response(&response)?;
        let etag = response
            .etag
            .as_deref()
            .filter(|value| strong_etag(value))
            .ok_or_else(|| download_error("fresh response requires a strong ETag"))?;
        if response
            .content_length
            .is_some_and(|value| value != member.size)
        {
            return Err(download_error("fresh response length is invalid"));
        }
        let fresh = if existing.preserve {
            format!("{}.fresh", member.asset_name)
        } else {
            existing.partial.to_owned()
        };
        let fresh_sidecar = if existing.preserve {
            format!("{}.fresh.resume.json", member.asset_name)
        } else {
            existing.sidecar.to_owned()
        };
        remove_regular_if_present(self.partial_dir()?, &fresh)?;
        remove_regular_if_present(self.partial_dir()?, &fresh_sidecar)?;
        let record = ResumeRecord {
            schema: CACHE_SCHEMA.to_owned(),
            profile_sha256: profile_digest.to_owned(),
            url: member.url.clone(),
            asset_name: member.asset_name.clone(),
            expected_size: member.size,
            expected_sha256: member.sha256.clone(),
            etag: etag.to_owned(),
        };
        write_resume(self.partial_dir()?, &fresh_sidecar, &record)?;
        crash_at!(PartialCreate);
        let mut file = create_new_file(self.partial_dir()?, &fresh)?;
        let mut hasher = Sha256::new();
        let received = stream_exact(
            &mut response.body,
            &mut file,
            &mut hasher,
            member.size,
            "fresh asset",
        )?;
        finish_member(
            file,
            MemberDestination {
                source_dir: self.partial_dir()?,
                source: &fresh,
                completed_dir: self.members_dir()?,
                completed,
                sidecar: &fresh_sidecar,
            },
            member,
            hasher,
        )?;
        if existing.preserve {
            remove_regular_if_present(self.partial_dir()?, existing.partial)?;
            remove_regular_if_present(self.partial_dir()?, existing.sidecar)?;
        }
        Ok(MemberTransfer {
            downloaded: received,
            resumed: 0,
        })
    }

    fn publish_transport(
        &self,
        profile: &ReleaseProfile,
    ) -> Result<PublishedTransport, AssetError> {
        if !closed_transport(self.members_dir()?, profile)? {
            return Err(missing_error(self, profile));
        }
        crash_at!(TransportRename);
        rename_noreplace(&self.profile_dir, "members", &self.profile_dir, "transport")?;
        crash_at!(TransportDirSync);
        sync_dir(&self.profile_dir)?;
        let transport = open_private_child(&self.profile_dir, "transport")?;
        let _new_members = create_private_child(&self.profile_dir, "members")?;
        sync_dir(&self.profile_dir)?;
        Ok(PublishedTransport {
            dir: transport,
            #[cfg(test)]
            path: self.transport.clone(),
        })
    }

    fn remove_published_transport(&self) -> Result<(), AssetError> {
        if let Some(transport) = open_private_child_optional(&self.profile_dir, "transport")? {
            if fail_at!(TransportEvict) {
                return Err(asset_io("evict cached transport"));
            }
            remove_directory_contents(&transport)?;
            remove_dir(&self.profile_dir, "transport")?;
            sync_dir(&self.profile_dir)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MemberTransfer {
    downloaded: u64,
    resumed: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeRecord {
    schema: String,
    profile_sha256: String,
    url: String,
    asset_name: String,
    expected_size: u64,
    expected_sha256: String,
    etag: String,
}

struct ResumeState {
    record: ResumeRecord,
    length: u64,
}

#[derive(Clone, Copy)]
struct ExistingPartial<'a> {
    partial: &'a str,
    sidecar: &'a str,
    preserve: bool,
}

fn load_resume(
    dir: &SafeDir,
    partial: &str,
    sidecar: &str,
    member: &ProfileMember,
    profile_digest: &str,
) -> Result<Option<ResumeState>, AssetError> {
    let partial_size = regular_size(dir, partial)?;
    let sidecar_exists = regular_size(dir, sidecar)?.is_some();
    let valid_length = partial_size.filter(|value| *value > 0 && *value < member.size);
    if let (Some(length), true) = (valid_length, sidecar_exists) {
        let bytes = read_bounded(dir, sidecar, MAX_RESUME_BYTES).ok();
        if let Some(bytes) = bytes
            && let Ok(record) = parse_resume(&bytes)
            && record.schema == CACHE_SCHEMA
            && record.profile_sha256 == profile_digest
            && record.url == member.url
            && record.asset_name == member.asset_name
            && record.expected_size == member.size
            && record.expected_sha256 == member.sha256
            && strong_etag(&record.etag)
        {
            return Ok(Some(ResumeState { record, length }));
        }
    }
    remove_regular_if_present(dir, partial)?;
    remove_regular_if_present(dir, sidecar)?;
    Ok(None)
}

fn parse_resume(bytes: &[u8]) -> Result<ResumeRecord, AssetError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| asset_state("resume record is invalid"))?;
    if serde_jcs::to_vec(&value).map_err(|_| asset_state("resume record is invalid"))? != bytes {
        return Err(asset_state("resume record is not canonical"));
    }
    serde_json::from_value(value).map_err(|_| asset_state("resume record is invalid"))
}

fn write_resume(dir: &SafeDir, name: &str, record: &ResumeRecord) -> Result<(), AssetError> {
    let bytes = serde_jcs::to_vec(record).map_err(|_| asset_io("serialize resume record"))?;
    if bytes.len() as u64 > MAX_RESUME_BYTES {
        return Err(asset_state("resume record is too large"));
    }
    validate_component(name)?;
    for _ in 0..64 {
        let nonce = CACHE_NONCE.fetch_add(1, Ordering::Relaxed);
        let temporary = format!(".{name}.writing.{}.{nonce}", std::process::id());
        crash_at!(SidecarCreate);
        let mut file = match create_new_file(dir, &temporary) {
            Ok(file) => file,
            Err(error) if entry_exists(dir, &temporary).unwrap_or(false) => continue,
            Err(error) => return Err(error),
        };
        let result = (|| {
            file.write_all(&bytes)
                .map_err(|_| asset_io("write resume record"))?;
            file.sync_all()
                .map_err(|_| asset_io("sync resume record"))?;
            drop(file);
            rename_noreplace(dir, &temporary, dir, name)?;
            crash_at!(SidecarSync);
            sync_dir(dir)
        })();
        if result.is_err() {
            let _ = remove_regular_if_present(dir, &temporary);
        }
        return result;
    }
    Err(asset_io("create unique resume staging file"))
}

fn follow_redirects(
    client: &dyn TransportClient,
    original: &str,
    range: Option<u64>,
    if_range: Option<&str>,
    contract: SyncContract<'_>,
) -> Result<Response, AssetError> {
    validate_url(original, contract)?;
    let mut current = original.to_owned();
    for redirects in 0..=MAX_REDIRECTS {
        let response = client.execute(&Request {
            url: current.clone(),
            range,
            if_range: if_range.map(ToOwned::to_owned),
        })?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            if redirects == MAX_REDIRECTS {
                return Err(download_error("asset redirect limit exceeded"));
            }
            let location = response
                .location
                .as_deref()
                .ok_or_else(|| download_error("asset redirect is missing Location"))?;
            validate_url(location, contract)?;
            current = location.to_owned();
            continue;
        }
        return Ok(response);
    }
    Err(download_error("asset redirect limit exceeded"))
}

fn validate_url(url: &str, contract: SyncContract<'_>) -> Result<(), AssetError> {
    let parsed =
        ureq::http::Uri::try_from(url).map_err(|_| download_error("asset URL is invalid"))?;
    let scheme = parsed
        .scheme_str()
        .ok_or_else(|| download_error("asset URL has no scheme"))?;
    if (contract.require_https && scheme != "https")
        || (!contract.require_https && scheme != "http")
    {
        return Err(download_error("asset URL scheme is not allowed"));
    }
    let authority = parsed
        .authority()
        .ok_or_else(|| download_error("asset URL has no host"))?;
    if authority.as_str().contains('@') {
        return Err(download_error("asset URL userinfo is forbidden"));
    }
    let host = authority.host();
    if !contract.allowed_hosts.contains(&host) {
        return Err(download_error("asset URL host is not allowed"));
    }
    if contract.require_https && authority.port_u16().is_some_and(|port| port != 443) {
        return Err(download_error("asset URL port is not allowed"));
    }
    if url.contains('#') {
        return Err(download_error("asset URL fragment is forbidden"));
    }
    Ok(())
}

fn validate_common_response(response: &Response) -> Result<(), AssetError> {
    if response
        .content_encoding
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err(download_error(
            "asset response content encoding is not identity",
        ));
    }
    Ok(())
}

fn strong_etag(value: &str) -> bool {
    if value.len() < 2
        || value.len() > 512
        || !value.starts_with('"')
        || !value.ends_with('"')
        || value.starts_with("W/")
    {
        return false;
    }
    value.as_bytes()[1..value.len() - 1]
        .iter()
        .all(|byte| *byte == 0x21 || (0x23..=0x7e).contains(byte))
}

fn stream_exact(
    input: &mut dyn Read,
    output: &mut File,
    hasher: &mut Sha256,
    expected: u64,
    label: &str,
) -> Result<u64, AssetError> {
    let mut remaining = expected;
    let mut buffer = [0_u8; BUFFER_SIZE];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(BUFFER_SIZE as u64))
            .map_err(|_| download_error("asset size is unsupported"))?;
        let count = input
            .read(&mut buffer[..limit])
            .map_err(|error| body_read_error(error, label))?;
        if count == 0 {
            return Err(download_error(format!("{label} body is short")));
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| asset_io("write partial asset"))?;
        crash_at!(PartialWrite);
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    let mut extra = [0_u8; 1];
    if input
        .read(&mut extra)
        .map_err(|error| body_read_error(error, label))?
        != 0
    {
        return Err(download_error(format!("{label} body is long")));
    }
    Ok(expected)
}

fn body_read_error(error: io::Error, label: &str) -> AssetError {
    if error.kind() == io::ErrorKind::TimedOut
        || error.to_string().to_ascii_lowercase().contains("timeout")
    {
        AssetError::new(AssetErrorKind::AssetTimeout, "asset body read timed out")
    } else {
        download_error(format!("{label} body read failed"))
    }
}

fn hash_prefix(file: &mut File, length: u64) -> Result<Sha256, AssetError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| asset_io("seek partial asset"))?;
    let mut hasher = Sha256::new();
    let mut remaining = length;
    let mut buffer = [0_u8; BUFFER_SIZE];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(BUFFER_SIZE as u64))
            .map_err(|_| asset_state("partial asset length is invalid"))?;
        let count = file
            .read(&mut buffer[..limit])
            .map_err(|_| asset_io("read partial asset"))?;
        if count == 0 {
            return Err(asset_state("partial asset changed while hashing"));
        }
        #[cfg(test)]
        sync_audit::record_prefix_bytes(count);
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(hasher)
}

struct MemberDestination<'a> {
    source_dir: &'a SafeDir,
    source: &'a str,
    completed_dir: &'a SafeDir,
    completed: &'a str,
    sidecar: &'a str,
}

fn finish_member(
    file: File,
    destination: MemberDestination<'_>,
    member: &ProfileMember,
    hasher: Sha256,
) -> Result<(), AssetError> {
    crash_at!(PartialSync);
    file.sync_all()
        .map_err(|_| asset_io("sync downloaded asset"))?;
    let actual = format!("sha256:{:x}", hasher.finalize());
    if actual != member.sha256 {
        return Err(download_error("downloaded asset SHA-256 mismatch"));
    }
    set_file_mode(&file, COMPLETE_FILE_MODE)?;
    crash_at!(MemberRename);
    rename_noreplace(
        destination.source_dir,
        destination.source,
        destination.completed_dir,
        destination.completed,
    )?;
    crash_at!(MemberDirSync);
    sync_dir(destination.completed_dir)?;
    remove_regular_if_present(destination.source_dir, destination.sidecar)?;
    Ok(())
}

fn closed_transport(dir: &SafeDir, profile: &ReleaseProfile) -> Result<bool, AssetError> {
    validate_private_dir(dir)?;
    let expected: BTreeSet<_> = profile
        .transport
        .members
        .iter()
        .map(|member| member.asset_name.as_str())
        .collect();
    let mut seen = BTreeSet::new();
    let mut closed = true;
    for_each_name(dir, |name| {
        if !expected.contains(name.as_str()) {
            closed = false;
            return Err(DirectoryVisit::ClosedMismatch);
        }
        let member = profile
            .transport
            .members
            .iter()
            .find(|member| member.asset_name == name)
            .ok_or(DirectoryVisit::ClosedMismatch)?;
        if complete_member_size(dir, &name)? != Some(member.size) {
            closed = false;
            return Err(DirectoryVisit::ClosedMismatch);
        }
        seen.insert(name);
        if seen.len() > expected.len() {
            return Err(DirectoryVisit::ClosedMismatch);
        }
        Ok(())
    })
    .or_else(|error| match error {
        DirectoryVisit::ClosedMismatch => Ok(()),
        DirectoryVisit::Asset(error) => Err(error),
    })?;
    if !closed {
        return Ok(false);
    }
    Ok(seen.len() == expected.len())
}

fn missing_error(cache: &Cache, profile: &ReleaseProfile) -> AssetError {
    let details = profile
        .transport
        .members
        .iter()
        .map(|member| {
            let complete = cache
                .members_dir()
                .and_then(|dir| complete_member_size(dir, &member.asset_name))
                .ok()
                .flatten()
                .filter(|size| *size == member.size);
            let partial = cache
                .partial_dir()
                .and_then(|dir| regular_size(dir, &format!("{}.partial", member.asset_name)))
                .ok()
                .flatten()
                .unwrap_or(0);
            let present = complete.unwrap_or(partial);
            format!("{}:{present}/{}", member.asset_name, member.size)
        })
        .collect::<Vec<_>>()
        .join(",");
    AssetError::new(
        AssetErrorKind::AssetsMissing,
        format!("profile {} is incomplete: {details}", profile.profile),
    )
}

#[derive(Debug)]
struct SafeDir {
    file: File,
    dev: u64,
    uid: u32,
}

fn open_or_create_cache_root(path: &Path) -> Result<SafeDir, AssetError> {
    if !path.is_absolute() {
        return Err(asset_state("cache root is not absolute"));
    }
    let root = open_path_directory(Path::new("/")).map_err(|_| asset_io("open filesystem root"))?;
    let mut current = safe_dir(root).map_err(|_| asset_io("inspect filesystem root"))?;
    let mut saw_component = false;
    for component in path.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir) {
                continue;
            }
            return Err(asset_state("cache root contains an invalid component"));
        };
        let name = name
            .to_str()
            .ok_or_else(|| asset_state("cache root is not UTF-8"))?;
        saw_component = true;
        current = open_or_create_path_child(&current, name)?;
    }
    if !saw_component {
        return Err(asset_state("cache root cannot be the filesystem root"));
    }
    if current.uid != effective_uid() {
        return Err(asset_state("cache root is not owned by the current user"));
    }
    validate_private_dir(&current)?;
    Ok(current)
}

fn ensure_private_child(parent: &SafeDir, name: &str) -> Result<SafeDir, AssetError> {
    match open_private_child_optional(parent, name)? {
        Some(dir) => Ok(dir),
        None => {
            match mkdir_at_raw(parent, name, CACHE_DIR_MODE) {
                Ok(()) => sync_dir(parent)?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(asset_io("create cache directory")),
            }
            open_private_child(parent, name)
        }
    }
}

fn create_private_child(parent: &SafeDir, name: &str) -> Result<SafeDir, AssetError> {
    mkdir_at_raw(parent, name, CACHE_DIR_MODE).map_err(|_| asset_io("create cache directory"))?;
    let dir = open_private_child(parent, name)?;
    sync_dir(parent)?;
    Ok(dir)
}

fn open_or_create_path_child(parent: &SafeDir, name: &str) -> Result<SafeDir, AssetError> {
    match open_child_dir_raw(parent, name, false) {
        Ok(dir) => Ok(dir),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match mkdir_at_raw(parent, name, CACHE_DIR_MODE) {
                Ok(()) => sync_dir(parent)?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(asset_io("create cache directory")),
            }
            open_child_dir_raw(parent, name, false)
                .map_err(|_| asset_state("cache root traversal is unsafe"))
        }
        Err(_) => Err(asset_state("cache root traversal is unsafe")),
    }
}

fn open_private_child(parent: &SafeDir, name: &str) -> Result<SafeDir, AssetError> {
    let dir = open_child_dir_raw(parent, name, true)
        .map_err(|error| map_open_error(error, "open private cache directory"))?;
    if dir.dev != parent.dev || dir.uid != parent.uid {
        return Err(asset_state("cache directory crossed its private root"));
    }
    validate_private_dir(&dir)?;
    Ok(dir)
}

fn open_private_child_optional(
    parent: &SafeDir,
    name: &str,
) -> Result<Option<SafeDir>, AssetError> {
    match open_child_dir_raw(parent, name, true) {
        Ok(dir) => {
            if dir.dev != parent.dev || dir.uid != parent.uid {
                return Err(asset_state("cache directory crossed its private root"));
            }
            validate_private_dir(&dir)?;
            Ok(Some(dir))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(asset_state("cache directory is not a safe directory")),
    }
}

fn open_child_dir_raw(parent: &SafeDir, name: &str, no_xdev: bool) -> io::Result<SafeDir> {
    let file = open_at(
        parent.file.as_raw_fd(),
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
        no_xdev,
    )?;
    safe_dir(file)
}

fn safe_dir(file: File) -> io::Result<SafeDir> {
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(io::Error::other("not a directory"));
    }
    Ok(SafeDir {
        file,
        dev: metadata.dev(),
        uid: metadata.uid(),
    })
}

fn validate_private_dir(dir: &SafeDir) -> Result<(), AssetError> {
    let metadata = dir
        .file
        .metadata()
        .map_err(|_| asset_io("inspect private cache directory"))?;
    if !metadata.is_dir()
        || metadata.dev() != dir.dev
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != CACHE_DIR_MODE
    {
        return Err(asset_state("cache path is not a private owned directory"));
    }
    Ok(())
}

fn rename_noreplace(from: &SafeDir, old: &str, to: &SafeDir, new: &str) -> Result<(), AssetError> {
    validate_component(old)?;
    validate_component(new)?;
    rustix::fs::renameat_with(
        &from.file,
        old,
        &to.file,
        new,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|_| asset_io("publish cache entry"))
}

fn create_new_file(dir: &SafeDir, name: &str) -> Result<File, AssetError> {
    let file = open_at(
        dir.file.as_raw_fd(),
        name,
        libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        CACHE_FILE_MODE,
        true,
    )
    .map_err(|_| asset_io("create cache file"))?;
    validate_private_file(&file, dir, CACHE_FILE_MODE)?;
    Ok(file)
}

fn open_partial_append(dir: &SafeDir, name: &str) -> Result<File, AssetError> {
    let file = open_regular(dir, name, libc::O_RDWR)?;
    validate_private_file(&file, dir, CACHE_FILE_MODE)?;
    Ok(file)
}

fn regular_size(dir: &SafeDir, name: &str) -> Result<Option<u64>, AssetError> {
    match open_regular(dir, name, libc::O_RDONLY) {
        Ok(file) => {
            validate_owned_file(&file, dir)?;
            Ok(Some(
                file.metadata()
                    .map_err(|_| asset_io("inspect cache member"))?
                    .len(),
            ))
        }
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => Ok(None),
        Err(error) => Err(error),
    }
}

fn complete_member_size(dir: &SafeDir, name: &str) -> Result<Option<u64>, AssetError> {
    match open_regular(dir, name, libc::O_RDONLY) {
        Ok(file) => {
            validate_private_file(&file, dir, COMPLETE_FILE_MODE)?;
            Ok(Some(
                file.metadata()
                    .map_err(|_| asset_io("inspect completed cache member"))?
                    .len(),
            ))
        }
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_bounded(dir: &SafeDir, name: &str, limit: u64) -> Result<Vec<u8>, AssetError> {
    let mut file = open_regular(dir, name, libc::O_RDONLY)?;
    validate_private_file(&file, dir, CACHE_FILE_MODE)?;
    let size = file
        .metadata()
        .map_err(|_| asset_io("inspect cache metadata"))?
        .len();
    if size > limit {
        return Err(asset_state("cache metadata is too large"));
    }
    let capacity = usize::try_from(size).map_err(|_| asset_state("cache metadata is too large"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|_| asset_io("read cache metadata"))?;
    Ok(bytes)
}

fn open_regular(dir: &SafeDir, name: &str, flags: i32) -> Result<File, AssetError> {
    let file = match open_at(
        dir.file.as_raw_fd(),
        name,
        flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
        true,
    ) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AssetError::new(
                AssetErrorKind::AssetsMissing,
                "cache member is missing",
            ));
        }
        Err(_) => return Err(asset_state("cache member is not a safe regular file")),
    };
    if !file
        .metadata()
        .map_err(|_| asset_io("inspect cache member"))?
        .is_file()
    {
        return Err(asset_state("cache member is not a regular file"));
    }
    Ok(file)
}

fn open_or_create_regular(
    dir: &SafeDir,
    name: &str,
    flags: i32,
    mode: u32,
) -> Result<File, AssetError> {
    match open_regular(dir, name, flags) {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => {
            match open_at(
                dir.file.as_raw_fd(),
                name,
                flags | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                mode,
                true,
            ) {
                Ok(file) => Ok(file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    open_regular(dir, name, flags)
                }
                Err(_) => Err(asset_io("create cache file")),
            }
        }
        Err(error) => Err(error),
    }
}

fn validate_private_file(file: &File, dir: &SafeDir, mode: u32) -> Result<(), AssetError> {
    validate_owned_file(file, dir)?;
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect private cache file"))?;
    if metadata.mode() & 0o777 != mode {
        return Err(asset_state("cache member has unsafe ownership or mode"));
    }
    Ok(())
}

fn validate_owned_file(file: &File, dir: &SafeDir) -> Result<(), AssetError> {
    let metadata = file
        .metadata()
        .map_err(|_| asset_io("inspect private cache file"))?;
    if !metadata.is_file() || metadata.dev() != dir.dev || metadata.uid() != dir.uid {
        return Err(asset_state("cache member has unsafe ownership"));
    }
    Ok(())
}

fn entry_exists(dir: &SafeDir, name: &str) -> Result<bool, AssetError> {
    match open_regular(dir, name, libc::O_RDONLY) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => Ok(false),
        Err(error) => Err(error),
    }
}

fn remove_regular_if_present(dir: &SafeDir, name: &str) -> Result<(), AssetError> {
    match open_regular(dir, name, libc::O_RDONLY) {
        Ok(file) => {
            validate_private_file(
                &file,
                dir,
                file.metadata()
                    .map_err(|_| asset_io("inspect cache file"))?
                    .mode()
                    & 0o777,
            )?;
            drop(file);
            unlink_file(dir, name)
        }
        Err(error) if error.kind() == AssetErrorKind::AssetsMissing => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_regular_file(dir: &SafeDir, name: &str) -> Result<(), AssetError> {
    let file = open_regular(dir, name, libc::O_RDONLY)?;
    drop(file);
    unlink_file(dir, name)
}

fn remove_directory_contents(dir: &SafeDir) -> Result<(), AssetError> {
    for_each_name(dir, |name| {
        unlink_file(dir, &name).map_err(DirectoryVisit::Asset)
    })
    .map_err(|error| match error {
        DirectoryVisit::Asset(error) => error,
        DirectoryVisit::ClosedMismatch => asset_state("unexpected cache cleanup stop"),
    })?;
    sync_dir(dir)
}

#[derive(Debug)]
enum DirectoryVisit {
    Asset(AssetError),
    ClosedMismatch,
}

impl From<AssetError> for DirectoryVisit {
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

fn for_each_name(
    dir: &SafeDir,
    mut visit: impl FnMut(String) -> Result<(), DirectoryVisit>,
) -> Result<(), DirectoryVisit> {
    let cursor = open_dot(
        dir.file.as_raw_fd(),
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
    )
    .map_err(|_| DirectoryVisit::Asset(asset_io("open cache directory cursor")))?;
    let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(cursor, &mut buffer);
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|_| DirectoryVisit::Asset(asset_io("read cache directory")))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = String::from_utf8(bytes.to_vec())
            .map_err(|_| DirectoryVisit::Asset(asset_state("cache child name is not UTF-8")))?;
        visit(name)?;
    }
    Ok(())
}

fn open_dot(dirfd: RawFd, flags: i32) -> io::Result<File> {
    let name = CString::new(".").expect("static directory component");
    let how = OpenHow {
        flags: flags as u64,
        mode: 0,
        resolve: RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV,
    };
    // SAFETY: the component, descriptor, and OpenHow remain live for the syscall.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        ) as i32
    };
    file_from_fd(fd)
}

fn sync_dir(dir: &SafeDir) -> Result<(), AssetError> {
    dir.file
        .sync_all()
        .map_err(|_| asset_io("sync cache directory"))
}

fn set_file_mode(file: &File, mode: u32) -> Result<(), AssetError> {
    // SAFETY: fchmod operates on this live descriptor.
    if unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) } == 0 {
        Ok(())
    } else {
        Err(asset_io("set cache file mode"))
    }
}

fn open_path_directory(path: &Path) -> io::Result<File> {
    use std::os::unix::ffi::OsStrExt;
    let path =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::other("NUL in path"))?;
    // SAFETY: the path is NUL terminated and no variadic mode is required.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(fd)
}

fn open_at(dirfd: RawFd, name: &str, flags: i32, mode: u32, no_xdev: bool) -> io::Result<File> {
    let name = component(name)?;
    let how = OpenHow {
        flags: flags as u64,
        mode: u64::from(mode),
        resolve: RESOLVE_BENEATH
            | RESOLVE_NO_SYMLINKS
            | RESOLVE_NO_MAGICLINKS
            | if no_xdev { RESOLVE_NO_XDEV } else { 0 },
    };
    // SAFETY: the component, descriptor, and OpenHow remain live for the syscall.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        ) as i32
    };
    file_from_fd(fd)
}

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

const RESOLVE_NO_XDEV: u64 = 0x01;
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;

fn file_from_fd(fd: i32) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: a successful open/openat2 returns a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn mkdir_at_raw(parent: &SafeDir, name: &str, mode: u32) -> io::Result<()> {
    let name = component(name)?;
    // SAFETY: the component and parent descriptor remain live for this call.
    if unsafe { libc::mkdirat(parent.file.as_raw_fd(), name.as_ptr(), mode as libc::mode_t) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn unlink_file(parent: &SafeDir, name: &str) -> Result<(), AssetError> {
    let name = component(name).map_err(|_| asset_state("invalid cache component"))?;
    // SAFETY: unlinkat with flags zero unlinks the directory entry and never follows a symlink.
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), name.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(asset_io("remove cache file"))
    }
}

fn remove_dir(parent: &SafeDir, name: &str) -> Result<(), AssetError> {
    let name = component(name).map_err(|_| asset_state("invalid cache component"))?;
    // SAFETY: unlinkat removes only the named directory below the held parent descriptor.
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } == 0 {
        Ok(())
    } else {
        Err(asset_io("remove cache directory"))
    }
}

fn component(name: &str) -> io::Result<CString> {
    validate_component(name).map_err(|_| io::Error::other("invalid path component"))?;
    CString::new(name).map_err(|_| io::Error::other("NUL in path component"))
}

fn validate_component(name: &str) -> Result<(), AssetError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(asset_state("cache component is invalid"));
    }
    Ok(())
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

fn map_open_error(error: io::Error, action: &str) -> AssetError {
    if error.kind() == io::ErrorKind::NotFound {
        AssetError::new(AssetErrorKind::AssetIo, action)
    } else {
        asset_state("cache path is unsafe")
    }
}

fn asset_io(action: &str) -> AssetError {
    AssetError::new(AssetErrorKind::AssetIo, action)
}

fn asset_state(message: &str) -> AssetError {
    AssetError::new(AssetErrorKind::AssetStateInvalid, message)
}

fn download_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::AssetDownload, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        collections::VecDeque,
        net::TcpListener,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Condvar, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        thread,
        time::Instant,
    };

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pangopup-sync-{}-{serial}", std::process::id()));
            fs::create_dir(&path).expect("temp");
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            fn writable(path: &Path) {
                if let Ok(meta) = fs::symlink_metadata(path) {
                    let mode = if meta.is_dir() { 0o700 } else { 0o600 };
                    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
                    if meta.is_dir()
                        && let Ok(entries) = fs::read_dir(path)
                    {
                        for entry in entries.flatten() {
                            writable(&entry.path());
                        }
                    }
                }
            }
            writable(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct ScriptClient {
        responses: RefCell<VecDeque<Response>>,
        requests: RefCell<Vec<Request>>,
    }

    struct CountRead {
        inner: io::Cursor<Vec<u8>>,
        bytes: Arc<AtomicU64>,
    }

    impl Read for CountRead {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = self.inner.read(buffer)?;
            self.bytes.fetch_add(count as u64, Ordering::Relaxed);
            Ok(count)
        }
    }

    impl ScriptClient {
        fn new(responses: Vec<Response>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    impl TransportClient for ScriptClient {
        fn execute(&self, request: &Request) -> Result<Response, AssetError> {
            self.requests.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| download_error("unexpected request"))
        }
    }

    struct TimeoutClient;

    impl TransportClient for TimeoutClient {
        fn execute(&self, _request: &Request) -> Result<Response, AssetError> {
            Err(AssetError::new(
                AssetErrorKind::AssetTimeout,
                "injected asset timeout",
            ))
        }
    }

    struct DynamicClient {
        assets: Vec<(String, Vec<u8>)>,
        requests: RefCell<Vec<Request>>,
    }

    struct BlockingClient {
        inner: DynamicClient,
        gate: Arc<(Mutex<bool>, Condvar)>,
    }

    impl TransportClient for BlockingClient {
        fn execute(&self, request: &Request) -> Result<Response, AssetError> {
            if self.inner.requests.borrow().is_empty() {
                let (state, ready) = &*self.gate;
                let mut blocked = state.lock().expect("gate lock");
                *blocked = true;
                ready.notify_all();
                while *blocked {
                    blocked = ready.wait(blocked).expect("gate wait");
                }
            }
            self.inner.execute(request)
        }
    }

    impl DynamicClient {
        fn new(profile: &ReleaseProfile, transport: &Path) -> Self {
            Self {
                assets: profile
                    .transport
                    .members
                    .iter()
                    .map(|member| {
                        (
                            member.url.clone(),
                            fs::read(transport.join(&member.asset_name)).expect("asset bytes"),
                        )
                    })
                    .collect(),
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    impl TransportClient for DynamicClient {
        fn execute(&self, request: &Request) -> Result<Response, AssetError> {
            self.requests.borrow_mut().push(request.clone());
            let bytes = self
                .assets
                .iter()
                .find(|(url, _)| url == &request.url)
                .map(|(_, bytes)| bytes)
                .ok_or_else(|| download_error("unexpected dynamic request"))?;
            if let Some(offset) = request.range {
                let start = usize::try_from(offset)
                    .map_err(|_| download_error("range offset is invalid"))?;
                let mut response = response(206, bytes[start..].to_vec(), Some("\"dynamic\""));
                response.content_range = Some(format!(
                    "bytes {offset}-{}/{}",
                    bytes.len() - 1,
                    bytes.len()
                ));
                Ok(response)
            } else {
                Ok(response(200, bytes.clone(), Some("\"dynamic\"")))
            }
        }
    }

    fn response(status: u16, bytes: Vec<u8>, etag: Option<&str>) -> Response {
        Response {
            status,
            location: None,
            etag: etag.map(ToOwned::to_owned),
            content_length: Some(bytes.len() as u64),
            content_range: None,
            content_encoding: None,
            body: Box::new(io::Cursor::new(bytes)),
        }
    }

    fn write_private(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("write private fixture");
        fs::set_permissions(path, fs::Permissions::from_mode(CACHE_FILE_MODE))
            .expect("set private fixture mode");
    }

    fn lock_and_initialize(cache: &Cache) -> CacheLock {
        let lock = cache.lock().expect("cache lock");
        cache
            .initialize_working_directories()
            .expect("initialize cache");
        lock
    }

    fn seed_resume(
        cache: &Cache,
        member: &ProfileMember,
        digest: &str,
        bytes: &[u8],
        offset: usize,
        etag: &str,
    ) {
        let partial_name = format!("{}.partial", member.asset_name);
        let sidecar_name = format!("{}.resume.json", member.asset_name);
        write_private(&cache.partial.join(&partial_name), &bytes[..offset]);
        write_resume(
            cache.partial_dir().expect("partial cache"),
            &sidecar_name,
            &ResumeRecord {
                schema: CACHE_SCHEMA.to_owned(),
                profile_sha256: digest.to_owned(),
                url: member.url.clone(),
                asset_name: member.asset_name.clone(),
                expected_size: member.size,
                expected_sha256: member.sha256.clone(),
                etag: etag.to_owned(),
            },
        )
        .expect("seed resume record");
    }

    fn miniature_profile(transport: &Path, base: &str) -> ReleaseProfile {
        let (_, _, mut profile) = super::super::release::production_profile().expect("profile");
        profile.profile = "test-v1".to_owned();
        profile.release.tag = "test-v1".to_owned();
        profile.transport.members.clear();
        for name in [
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
            "payload.pgi.zst.part0000",
        ] {
            let bytes = fs::read(transport.join(name)).expect("transport member");
            profile.transport.members.push(ProfileMember {
                logical_path: name.to_owned(),
                asset_name: name.to_owned(),
                size: bytes.len() as u64,
                sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
                url: format!("{base}/{name}"),
            });
        }
        let manifest = fs::read(transport.join("transport.json")).expect("manifest");
        let value: serde_json::Value = serde_json::from_slice(&manifest).expect("manifest json");
        profile.transport.transport_id = value["transport_id"].as_str().expect("id").to_owned();
        profile.bundle.bundle_id = value["bundle"]["bundle_id"]
            .as_str()
            .expect("bundle id")
            .to_owned();
        profile
    }

    fn fixture(root: &Path) -> PathBuf {
        let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle");
        let transport = root.join("fixture-transport");
        super::super::pack_bundle(&bundle, &transport).expect("pack fixture");
        transport
    }

    fn alternate_fixture(root: &Path) -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/snv-regression/bundle");
        let bundle = root.join("alternate-bundle");
        fs::create_dir(&bundle).expect("alternate bundle");
        fs::copy(source.join("NOTICE"), bundle.join("NOTICE")).expect("copy notice");
        fs::copy(source.join("scores.pgi"), bundle.join("scores.pgi")).expect("copy scores");
        let mut manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(source.join("manifest.json")).expect("read fixture manifest"),
        )
        .expect("parse fixture manifest");
        manifest["builder"]["source_sha256"] = serde_json::Value::String(
            "sha256:abababababababababababababababababababababababababababababababab".to_owned(),
        );
        fs::write(
            bundle.join("manifest.json"),
            serde_jcs::to_vec(&manifest).expect("canonical alternate manifest"),
        )
        .expect("write alternate manifest");
        let transport = root.join("alternate-transport");
        super::super::pack_bundle(&bundle, &transport).expect("pack alternate fixture");
        transport
    }

    #[test]
    fn fresh_sync_installs_then_active_reuse_is_zero_network() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let responses = profile
            .transport
            .members
            .iter()
            .map(|member| {
                response(
                    200,
                    fs::read(transport.join(&member.asset_name)).expect("bytes"),
                    Some("\"fixture\""),
                )
            })
            .collect();
        let client = ScriptClient::new(responses);
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let data = temp.0.join("data");
        let cache = temp.0.join("cache");
        let installed = sync_with(&data, Some(&cache), false, contract, &client).expect("sync");
        assert_eq!(installed.status, "installed");
        assert_eq!(
            installed.downloaded_bytes,
            profile
                .transport
                .members
                .iter()
                .map(|member| member.size)
                .sum::<u64>()
        );
        eprintln!(
            "miniature_member_sizes={:?} fresh_downloaded={} fresh_resumed={}",
            profile
                .transport
                .members
                .iter()
                .map(|member| member.size)
                .collect::<Vec<_>>(),
            installed.downloaded_bytes,
            installed.resumed_bytes
        );
        let requests = client.requests.borrow();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.url.as_str())
                .collect::<Vec<_>>(),
            profile
                .transport
                .members
                .iter()
                .map(|member| member.url.as_str())
                .collect::<Vec<_>>()
        );
        drop(requests);
        let offline_data = temp.0.join("offline-data");
        let offline_client = ScriptClient::new(vec![]);
        let offline = sync_with(&offline_data, Some(&cache), true, contract, &offline_client)
            .expect("offline cached install");
        assert_eq!(offline.status, "installed");
        assert!(offline_client.requests.borrow().is_empty());
        let no_network = ScriptClient::new(vec![]);
        let reuse_started = Instant::now();
        let reused = sync_with(&data, None, true, contract, &no_network).expect("active reuse");
        let reuse_elapsed = reuse_started.elapsed();
        eprintln!("miniature_active_reuse_ns={}", reuse_elapsed.as_nanos());
        assert_eq!(reused.status, "reused");
        assert_eq!((reused.downloaded_bytes, reused.resumed_bytes), (0, 0));
        assert!(no_network.requests.borrow().is_empty());
        let (active, _) = open_active_bundle(&data).expect("active");
        assert_eq!(active.path, installed.path);
    }

    #[test]
    fn concurrent_first_sync_has_one_owner_and_one_locked_loser() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:7373737373737373737373737373737373737373737373737373737373737373",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let data = temp.0.join("data");
        let cache = temp.0.join("cache");
        assert!(!cache.exists());
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let loser_client = DynamicClient::new(&profile, &transport);

        thread::scope(|scope| {
            let winner_gate = Arc::clone(&gate);
            let winner_client = BlockingClient {
                inner: DynamicClient::new(&profile, &transport),
                gate: winner_gate,
            };
            let winner_data = &data;
            let winner_cache = &cache;
            let winner = scope.spawn(move || {
                sync_with(
                    winner_data,
                    Some(winner_cache),
                    false,
                    contract,
                    &winner_client,
                )
            });

            let (state, ready) = &*gate;
            let mut blocked = state.lock().expect("gate lock");
            while !*blocked {
                blocked = ready.wait(blocked).expect("gate wait");
            }
            drop(blocked);

            let loser = sync_with(&data, Some(&cache), false, contract, &loser_client)
                .expect_err("second first-use sync must not wait");
            assert_eq!(loser.kind(), AssetErrorKind::AssetLocked);
            assert!(loser_client.requests.borrow().is_empty());

            let mut blocked = state.lock().expect("gate lock");
            *blocked = false;
            ready.notify_all();
            drop(blocked);

            let installed = winner.join().expect("winner thread").expect("winner sync");
            assert_eq!(installed.status, "installed");
        });
        let (active, _) = open_active_bundle(&data).expect("one active install");
        assert_eq!(active.bundle_id, profile.bundle.bundle_id);
    }

    #[test]
    fn miniature_transport_installs_through_real_streaming_http_client() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
        let address = listener.local_addr().expect("fixture address");
        let base = format!("http://{address}");
        let profile = miniature_profile(&transport, &base);
        let served = Arc::new(AtomicU64::new(0));
        let server_served = Arc::clone(&served);
        let bodies: Vec<_> = profile
            .transport
            .members
            .iter()
            .map(|member| {
                (
                    format!("/{}", member.asset_name),
                    fs::read(transport.join(&member.asset_name)).expect("member bytes"),
                )
            })
            .collect();
        let server = thread::spawn(move || {
            for _ in 0..bodies.len() {
                let (mut stream, _) = listener.accept().expect("accept fixture request");
                let mut request = [0_u8; 4096];
                let count = stream.read(&mut request).expect("read fixture request");
                let text = std::str::from_utf8(&request[..count]).expect("request UTF-8");
                let path = text
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .expect("request path");
                let body = bodies
                    .iter()
                    .find(|(candidate, _)| candidate == path)
                    .map(|(_, body)| body)
                    .expect("known path");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture-http\"\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write response headers");
                stream.write_all(body).expect("write response body");
                stream.flush().expect("flush response");
                server_served.fetch_add(1, Ordering::Relaxed);
            }
        });
        let client = UreqClient::with_timeouts(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        );
        let outcome = sync_with(
            &temp.0.join("data"),
            Some(&temp.0.join("cache")),
            false,
            SyncContract {
                profile: &profile,
                profile_digest:
                    "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                allowed_hosts: &["127.0.0.1"],
                require_https: false,
            },
            &client,
        )
        .expect("real HTTP sync");
        assert_eq!(outcome.status, "installed");
        server.join().expect("fixture server");
        assert_eq!(served.load(Ordering::Relaxed), 4);
        let (_, opened) = open_active_bundle(&temp.0.join("data")).expect("active fixture");
        assert_eq!(opened.bundle_id(), profile.bundle.bundle_id);
    }

    #[test]
    fn resume_requires_exact_validator_and_range() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let cache = Cache::open(
            &temp.0.join("cache"),
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .expect("cache");
        cache
            .initialize_working_directories()
            .expect("working directories");
        let offset = bytes.len() as u64 / 2;
        let partial = cache.partial.join(format!("{}.partial", member.asset_name));
        write_private(&partial, &bytes[..offset as usize]);
        let sidecar = cache
            .partial
            .join(format!("{}.resume.json", member.asset_name));
        write_resume(
            cache.partial_dir().expect("partial cache"),
            sidecar
                .file_name()
                .and_then(OsStr::to_str)
                .expect("sidecar name"),
            &ResumeRecord {
                schema: CACHE_SCHEMA.to_owned(),
                profile_sha256:
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_owned(),
                url: member.url.clone(),
                asset_name: member.asset_name.clone(),
                expected_size: member.size,
                expected_sha256: member.sha256.clone(),
                etag: "\"same\"".to_owned(),
            },
        )
        .expect("sidecar");
        let mut ranged = response(206, bytes[offset as usize..].to_vec(), Some("\"same\""));
        ranged.content_range = Some(format!(
            "bytes {offset}-{}/{}",
            member.size - 1,
            member.size
        ));
        let client = ScriptClient::new(vec![ranged]);
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let transferred = cache
            .obtain_member(member, contract, &client)
            .expect("resume");
        assert_eq!(transferred.resumed, offset);
        assert_eq!(transferred.downloaded, member.size - offset);
        let request = &client.requests.borrow()[0];
        assert_eq!(request.range, Some(offset));
        assert_eq!(request.if_range.as_deref(), Some("\"same\""));
        assert_eq!(
            fs::read(cache.members.join(&member.asset_name)).expect("complete"),
            bytes
        );
    }

    #[test]
    fn interrupted_fresh_body_is_reused_by_exact_range_request() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let split = bytes.len() / 3;
        let mut short = response(200, bytes[..split].to_vec(), Some("\"stable\""));
        short.content_length = None;
        let first = ScriptClient::new(vec![short]);
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let cache = Cache::open(
            &temp.0.join("cache"),
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .expect("cache");
        cache
            .initialize_working_directories()
            .expect("working dirs");
        assert_eq!(
            cache
                .obtain_member(member, contract, &first)
                .expect_err("short body")
                .kind(),
            AssetErrorKind::AssetDownload
        );
        let partial = cache.partial.join(format!("{}.partial", member.asset_name));
        assert_eq!(fs::metadata(&partial).expect("partial").len(), split as u64);
        let mut resumed = response(206, bytes[split..].to_vec(), Some("\"stable\""));
        resumed.content_range = Some(format!("bytes {split}-{}/{}", member.size - 1, member.size));
        let second = ScriptClient::new(vec![resumed]);
        let result = cache
            .obtain_member(member, contract, &second)
            .expect("resume interrupted body");
        assert_eq!(result.resumed, split as u64);
        assert_eq!(second.requests.borrow()[0].range, Some(split as u64));
    }

    #[test]
    fn resume_redirect_preserves_headers_and_full_200_restarts_without_prefix_credit() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let offset = bytes.len() as u64 / 2;
        let digest = "sha256:abababababababababababababababababababababababababababababababab";
        let cache = Cache::open(&temp.0.join("cache"), digest).expect("cache");
        cache
            .initialize_working_directories()
            .expect("working dirs");
        let partial = cache.partial.join(format!("{}.partial", member.asset_name));
        write_private(&partial, &bytes[..offset as usize]);
        let sidecar = cache
            .partial
            .join(format!("{}.resume.json", member.asset_name));
        write_resume(
            cache.partial_dir().expect("partial cache"),
            sidecar
                .file_name()
                .and_then(OsStr::to_str)
                .expect("sidecar name"),
            &ResumeRecord {
                schema: CACHE_SCHEMA.to_owned(),
                profile_sha256: digest.to_owned(),
                url: member.url.clone(),
                asset_name: member.asset_name.clone(),
                expected_size: member.size,
                expected_sha256: member.sha256.clone(),
                etag: "\"old\"".to_owned(),
            },
        )
        .expect("sidecar");
        let redirect = Response {
            status: 302,
            location: Some("http://download.test/member".to_owned()),
            etag: None,
            content_length: Some(0),
            content_range: None,
            content_encoding: None,
            body: Box::new(io::empty()),
        };
        let restarted = response(200, bytes.clone(), Some("\"new\""));
        let client = ScriptClient::new(vec![redirect, restarted]);
        let result = cache
            .obtain_member(
                member,
                SyncContract {
                    profile: &profile,
                    profile_digest: digest,
                    allowed_hosts: &["fixture.test", "download.test"],
                    require_https: false,
                },
                &client,
            )
            .expect("restart");
        assert_eq!((result.downloaded, result.resumed), (member.size, 0));
        let requests = client.requests.borrow();
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            assert_eq!(request.range, Some(offset));
            assert_eq!(request.if_range.as_deref(), Some("\"old\""));
        }
        assert_eq!(
            fs::read(cache.members.join(&member.asset_name)).expect("completed"),
            bytes
        );
        assert!(!partial.exists());
        assert!(!sidecar.exists());
    }

    #[test]
    fn wire_failures_do_not_promote_members() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let cases = [
            response(200, bytes.clone(), None),
            response(200, bytes.clone(), Some("W/\"weak\"")),
            response(200, bytes[..bytes.len() - 1].to_vec(), Some("\"short\"")),
            response(200, [bytes.as_slice(), b"x"].concat(), Some("\"long\"")),
            response(416, Vec::new(), Some("\"range\"")),
        ];
        for (ordinal, mut case) in cases.into_iter().enumerate() {
            if ordinal == 2 || ordinal == 3 {
                case.content_length = None;
            }
            let digest = format!("sha256:{ordinal:064x}");
            let cache =
                Cache::open(&temp.0.join(format!("cache-{ordinal}")), &digest).expect("cache");
            cache
                .initialize_working_directories()
                .expect("working dirs");
            let client = ScriptClient::new(vec![case]);
            let contract = SyncContract {
                profile: &profile,
                profile_digest: &digest,
                allowed_hosts: &["fixture.test"],
                require_https: false,
            };
            assert!(cache.obtain_member(member, contract, &client).is_err());
            assert!(!cache.members.join(&member.asset_name).exists());
        }

        let mut wrong = bytes;
        wrong[0] ^= 1;
        let digest = "sha256:9999999999999999999999999999999999999999999999999999999999999999";
        let cache = Cache::open(&temp.0.join("wrong-hash"), digest).expect("cache");
        cache
            .initialize_working_directories()
            .expect("working dirs");
        let client = ScriptClient::new(vec![response(200, wrong, Some("\"wrong\""))]);
        assert_eq!(
            cache
                .obtain_member(
                    member,
                    SyncContract {
                        profile: &profile,
                        profile_digest: digest,
                        allowed_hosts: &["fixture.test"],
                        require_https: false,
                    },
                    &client,
                )
                .expect_err("wrong hash")
                .kind(),
            AssetErrorKind::AssetDownload
        );
        assert!(!cache.members.join(&member.asset_name).exists());
    }

    #[test]
    fn wrong_declared_lengths_neither_promote_fresh_nor_append_resumed_bytes() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:7575757575757575757575757575757575757575757575757575757575757575",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };

        let fresh_cache =
            Cache::open(&temp.0.join("fresh"), contract.profile_digest).expect("fresh cache");
        let _fresh_lock = lock_and_initialize(&fresh_cache);
        let mut fresh = response(200, bytes.clone(), Some("\"length\""));
        fresh.content_length = Some(member.size - 1);
        let fresh_error = fresh_cache
            .obtain_member(member, contract, &ScriptClient::new(vec![fresh]))
            .expect_err("wrong fresh Content-Length");
        assert_eq!(fresh_error.kind(), AssetErrorKind::AssetDownload);
        assert!(!fresh_cache.members.join(&member.asset_name).exists());
        assert!(
            !fresh_cache
                .partial
                .join(format!("{}.partial", member.asset_name))
                .exists()
        );

        let resumed_cache =
            Cache::open(&temp.0.join("resumed"), contract.profile_digest).expect("resume cache");
        let _resumed_lock = lock_and_initialize(&resumed_cache);
        let offset = bytes.len() / 2;
        seed_resume(
            &resumed_cache,
            member,
            contract.profile_digest,
            &bytes,
            offset,
            "\"length\"",
        );
        let mut resumed = response(206, bytes[offset..].to_vec(), Some("\"length\""));
        resumed.content_length = Some(member.size - offset as u64 + 1);
        resumed.content_range = Some(format!(
            "bytes {offset}-{}/{}",
            member.size - 1,
            member.size
        ));
        let resume_error = resumed_cache
            .obtain_member(member, contract, &ScriptClient::new(vec![resumed]))
            .expect_err("wrong resumed Content-Length");
        assert_eq!(resume_error.kind(), AssetErrorKind::AssetDownload);
        assert_eq!(
            fs::read(
                resumed_cache
                    .partial
                    .join(format!("{}.partial", member.asset_name))
            )
            .expect("preserved resume prefix"),
            bytes[..offset]
        );
        assert!(!resumed_cache.members.join(&member.asset_name).exists());
    }

    #[test]
    fn timeout_releases_sync_lock_and_never_publishes_before_successful_retry() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:7676767676767676767676767676767676767676767676767676767676767676",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let data = temp.0.join("data");
        let cache_root = temp.0.join("cache");
        let error = sync_with(&data, Some(&cache_root), false, contract, &TimeoutClient)
            .expect_err("injected timeout");
        assert_eq!(error.kind(), AssetErrorKind::AssetTimeout);
        assert!(active_bundle_missing(&data));
        let profile_cache = cache_root
            .join("profiles")
            .join(contract.profile_digest.trim_start_matches("sha256:"));
        assert!(!profile_cache.join("transport").exists());

        let cache = Cache::open(&cache_root, contract.profile_digest).expect("reopen cache");
        let lock = cache.lock().expect("timeout released cache lock");
        drop(lock);

        let retry = sync_with(
            &data,
            Some(&cache_root),
            false,
            contract,
            &DynamicClient::new(&profile, &transport),
        )
        .expect("successful retry");
        assert_eq!(retry.status, "installed");
    }

    #[test]
    fn offline_missing_reports_all_members_and_paths_are_strict() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let client = ScriptClient::new(vec![]);
        let error = sync_with(
            &temp.0.join("data"),
            Some(&temp.0.join("cache")),
            true,
            contract,
            &client,
        )
        .expect_err("missing");
        assert_eq!(error.kind(), AssetErrorKind::AssetsMissing);
        for member in &profile.transport.members {
            assert!(error.to_string().contains(&member.asset_name));
        }
        assert!(client.requests.borrow().is_empty());
        assert_eq!(
            resolve_cache_root(&CachePathInputs {
                explicit: Some(temp.0.clone().into_os_string()),
                pangopup_cache_dir: Some("relative".into()),
                ..CachePathInputs::default()
            })
            .expect_err("all present paths validated")
            .kind(),
            AssetErrorKind::PathInvalid
        );
    }

    #[test]
    fn cache_lock_is_nonblocking_and_published_corruption_is_evicted() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let digest = "sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
        let cache = Cache::open(&temp.0.join("cache"), digest).expect("cache");
        let first_lock = cache.lock().expect("first lock");
        assert_eq!(
            cache.lock().expect_err("competing lock").kind(),
            AssetErrorKind::AssetLocked
        );
        drop(first_lock);
        cache
            .initialize_working_directories()
            .expect("working dirs");
        for member in &profile.transport.members {
            fs::copy(
                transport.join(&member.asset_name),
                cache.members.join(&member.asset_name),
            )
            .expect("copy completed member");
            fs::set_permissions(
                cache.members.join(&member.asset_name),
                fs::Permissions::from_mode(COMPLETE_FILE_MODE),
            )
            .expect("member mode");
        }
        let published = cache
            .publish_transport(&profile)
            .expect("publish transport");
        let part = published.join("payload.pgi.zst.part0000");
        fs::set_permissions(&part, fs::Permissions::from_mode(CACHE_FILE_MODE))
            .expect("make corruptible");
        let mut file = OpenOptions::new()
            .write(true)
            .open(&part)
            .expect("open part");
        file.write_all(b"X").expect("corrupt part prefix");
        drop(file);
        fs::set_permissions(&part, fs::Permissions::from_mode(COMPLETE_FILE_MODE))
            .expect("restore mode");
        let client = ScriptClient::new(vec![]);
        let error = sync_with(
            &temp.0.join("data"),
            Some(&temp.0.join("cache")),
            true,
            SyncContract {
                profile: &profile,
                profile_digest: digest,
                allowed_hosts: &["fixture.test"],
                require_https: false,
            },
            &client,
        )
        .expect_err("corrupt published cache");
        assert!(cache_content_error(error.kind()));
        assert!(!published.exists());
    }

    #[test]
    fn hostile_transport_entries_are_validated_and_removed_streamingly() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let digest = "sha256:7474747474747474747474747474747474747474747474747474747474747474";
        let cache = Cache::open(&temp.0.join("cache"), digest).expect("cache");
        let _lock = lock_and_initialize(&cache);
        let hostile = create_private_child(&cache.profile_dir, "transport").expect("transport");
        for ordinal in 0..2_048 {
            create_new_file(&hostile, &format!("junk-{ordinal:04}")).expect("hostile member");
        }
        assert!(!closed_transport(&hostile, &profile).expect("closed validation"));
        remove_directory_contents(&hostile).expect("streaming cleanup");
        let mut remaining = 0_u64;
        for_each_name(&hostile, |_| {
            remaining += 1;
            Ok(())
        })
        .expect("count remaining entries");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn resume_206_matrix_rejects_validator_range_and_status_failures_without_append() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let offset = bytes.len() / 2;
        let digest = "sha256:5656565656565656565656565656565656565656565656565656565656565656";

        let mut cases = Vec::new();
        let valid_range = format!("bytes {offset}-{}/{}", member.size - 1, member.size);
        for etag in [None, Some("W/\"same\""), Some("\"changed\"")] {
            let mut response = response(206, bytes[offset..].to_vec(), etag);
            response.content_range = Some(valid_range.clone());
            cases.push(response);
        }
        let mut missing_range = response(206, bytes[offset..].to_vec(), Some("\"same\""));
        missing_range.content_range = None;
        cases.push(missing_range);
        let mut wrong_range = response(206, bytes[offset..].to_vec(), Some("\"same\""));
        wrong_range.content_range = Some(format!(
            "bytes {}-{}/{}",
            offset + 1,
            member.size - 1,
            member.size
        ));
        cases.push(wrong_range);
        cases.push(response(416, Vec::new(), Some("\"same\"")));

        for (ordinal, candidate) in cases.into_iter().enumerate() {
            let cache =
                Cache::open(&temp.0.join(format!("matrix-{ordinal}")), digest).expect("cache");
            let _lock = lock_and_initialize(&cache);
            seed_resume(&cache, member, digest, &bytes, offset, "\"same\"");
            let client = ScriptClient::new(vec![candidate]);
            let error = cache
                .obtain_member(
                    member,
                    SyncContract {
                        profile: &profile,
                        profile_digest: digest,
                        allowed_hosts: &["fixture.test"],
                        require_https: false,
                    },
                    &client,
                )
                .expect_err("invalid resume response");
            assert_eq!(
                error.kind(),
                AssetErrorKind::AssetDownload,
                "case {ordinal}"
            );
            assert_eq!(
                fs::read(cache.partial.join(format!("{}.partial", member.asset_name)))
                    .expect("preserved prefix"),
                bytes[..offset],
                "case {ordinal}"
            );
            assert!(!cache.members.join(&member.asset_name).exists());
        }
    }

    #[test]
    fn fresh_redirect_is_followed_without_range_headers() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let redirect = Response {
            status: 302,
            location: Some("http://download.test/member".to_owned()),
            etag: None,
            content_length: Some(0),
            content_range: None,
            content_encoding: None,
            body: Box::new(io::empty()),
        };
        let client = ScriptClient::new(vec![
            redirect,
            response(200, bytes, Some("\"fresh-redirect\"")),
        ]);
        let digest = "sha256:5757575757575757575757575757575757575757575757575757575757575757";
        let cache = Cache::open(&temp.0.join("fresh-redirect"), digest).expect("cache");
        let _lock = lock_and_initialize(&cache);
        cache
            .obtain_member(
                member,
                SyncContract {
                    profile: &profile,
                    profile_digest: digest,
                    allowed_hosts: &["fixture.test", "download.test"],
                    require_https: false,
                },
                &client,
            )
            .expect("fresh redirect");
        assert_eq!(client.requests.borrow().len(), 2);
        assert!(
            client
                .requests
                .borrow()
                .iter()
                .all(|request| { request.range.is_none() && request.if_range.is_none() })
        );
    }

    #[test]
    fn fresh_and_resumed_reads_touch_each_network_byte_and_prefix_byte_once() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let digest = "sha256:5858585858585858585858585858585858585858585858585858585858585858";
        let fresh_count = Arc::new(AtomicU64::new(0));
        let fresh = Response {
            status: 200,
            location: None,
            etag: Some("\"audit\"".to_owned()),
            content_length: Some(member.size),
            content_range: None,
            content_encoding: None,
            body: Box::new(CountRead {
                inner: io::Cursor::new(bytes.clone()),
                bytes: Arc::clone(&fresh_count),
            }),
        };
        let fresh_cache = Cache::open(&temp.0.join("fresh-audit"), digest).expect("cache");
        let _fresh_lock = lock_and_initialize(&fresh_cache);
        fresh_cache
            .obtain_member(
                member,
                SyncContract {
                    profile: &profile,
                    profile_digest: digest,
                    allowed_hosts: &["fixture.test"],
                    require_https: false,
                },
                &ScriptClient::new(vec![fresh]),
            )
            .expect("fresh audit");
        assert_eq!(fresh_count.load(Ordering::Relaxed), member.size);

        let offset = bytes.len() / 3;
        let resumed_cache = Cache::open(&temp.0.join("resume-audit"), digest).expect("cache");
        let _resumed_lock = lock_and_initialize(&resumed_cache);
        seed_resume(&resumed_cache, member, digest, &bytes, offset, "\"audit\"");
        let suffix_count = Arc::new(AtomicU64::new(0));
        let mut resumed = Response {
            status: 206,
            location: None,
            etag: Some("\"audit\"".to_owned()),
            content_length: Some(member.size - offset as u64),
            content_range: None,
            content_encoding: None,
            body: Box::new(CountRead {
                inner: io::Cursor::new(bytes[offset..].to_vec()),
                bytes: Arc::clone(&suffix_count),
            }),
        };
        resumed.content_range = Some(format!(
            "bytes {offset}-{}/{}",
            member.size - 1,
            member.size
        ));
        sync_audit::reset_prefix_bytes();
        resumed_cache
            .obtain_member(
                member,
                SyncContract {
                    profile: &profile,
                    profile_digest: digest,
                    allowed_hosts: &["fixture.test"],
                    require_https: false,
                },
                &ScriptClient::new(vec![resumed]),
            )
            .expect("resume audit");
        assert_eq!(sync_audit::take_prefix_bytes(), offset as u64);
        assert_eq!(
            suffix_count.load(Ordering::Relaxed),
            member.size - offset as u64
        );
    }

    #[test]
    fn cache_traversal_and_member_symlinks_fail_closed() {
        let temp = Temp::new();
        let target = temp.0.join("target");
        fs::create_dir(&target).expect("target");
        let intermediate = temp.0.join("intermediate");
        std::os::unix::fs::symlink(&target, &intermediate).expect("intermediate symlink");
        let digest = "sha256:5959595959595959595959595959595959595959595959595959595959595959";
        assert_eq!(
            Cache::open(&intermediate.join("cache"), digest)
                .expect_err("intermediate symlink")
                .kind(),
            AssetErrorKind::AssetStateInvalid
        );

        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let cache = Cache::open(&temp.0.join("member-cache"), digest).expect("cache");
        let _lock = lock_and_initialize(&cache);
        let outside = temp.0.join("outside");
        write_private(&outside, b"outside");
        std::os::unix::fs::symlink(&outside, cache.members.join(&member.asset_name))
            .expect("member symlink");
        let error = cache
            .obtain_member(
                member,
                SyncContract {
                    profile: &profile,
                    profile_digest: digest,
                    allowed_hosts: &["fixture.test"],
                    require_https: false,
                },
                &ScriptClient::new(vec![]),
            )
            .expect_err("member symlink");
        assert_eq!(error.kind(), AssetErrorKind::AssetStateInvalid);
        assert_eq!(fs::read(&outside).expect("outside unchanged"), b"outside");
    }

    #[test]
    fn malformed_resume_records_and_invalid_partial_lengths_are_discarded() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let bytes = fs::read(transport.join(&member.asset_name)).expect("bytes");
        let digest = "sha256:6060606060606060606060606060606060606060606060606060606060606060";
        for (ordinal, length) in [0_usize, bytes.len(), bytes.len() + 1]
            .into_iter()
            .enumerate()
        {
            let cache =
                Cache::open(&temp.0.join(format!("length-{ordinal}")), digest).expect("cache");
            let _lock = lock_and_initialize(&cache);
            let partial_name = format!("{}.partial", member.asset_name);
            let sidecar_name = format!("{}.resume.json", member.asset_name);
            write_private(&cache.partial.join(&partial_name), &vec![0_u8; length]);
            write_private(&cache.partial.join(&sidecar_name), b"not-json");
            assert!(
                load_resume(
                    cache.partial_dir().expect("partial cache"),
                    &partial_name,
                    &sidecar_name,
                    member,
                    digest
                )
                .expect("discard invalid partial")
                .is_none()
            );
            assert!(!cache.partial.join(&partial_name).exists());
            assert!(!cache.partial.join(&sidecar_name).exists());
        }

        let cache = Cache::open(&temp.0.join("noncanonical"), digest).expect("cache");
        let _lock = lock_and_initialize(&cache);
        let partial_name = format!("{}.partial", member.asset_name);
        let sidecar_name = format!("{}.resume.json", member.asset_name);
        write_private(&cache.partial.join(&partial_name), &bytes[..1]);
        let record = ResumeRecord {
            schema: CACHE_SCHEMA.to_owned(),
            profile_sha256: digest.to_owned(),
            url: member.url.clone(),
            asset_name: member.asset_name.clone(),
            expected_size: member.size,
            expected_sha256: member.sha256.clone(),
            etag: "\"same\"".to_owned(),
        };
        write_private(
            &cache.partial.join(&sidecar_name),
            &serde_json::to_vec_pretty(&record).expect("pretty record"),
        );
        assert!(
            load_resume(
                cache.partial_dir().expect("partial cache"),
                &partial_name,
                &sidecar_name,
                member,
                digest
            )
            .expect("discard noncanonical record")
            .is_none()
        );
    }

    #[test]
    fn cache_and_data_path_failures_do_not_change_a_valid_install() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let data = temp.0.join("valid-data");
        let alternate = alternate_fixture(&temp.0);
        install_transport(&alternate, &data).expect("valid nonmatching install");
        let (before, _) = open_active_bundle(&data).expect("active before");
        assert_ne!(before.bundle_id, profile.bundle.bundle_id);
        assert_ne!(before.transport_id, profile.transport.transport_id);
        let bad_cache = temp.0.join("bad-cache");
        write_private(&bad_cache, b"not a directory");
        let digest = "sha256:6161616161616161616161616161616161616161616161616161616161616161";
        let contract = SyncContract {
            profile: &profile,
            profile_digest: digest,
            allowed_hosts: &["fixture.test"],
            require_https: false,
        };
        let cache_error = sync_with(
            &data,
            Some(&bad_cache),
            true,
            contract,
            &ScriptClient::new(vec![]),
        )
        .expect_err("nonmatching active must exercise cache failure");
        assert_eq!(cache_error.kind(), AssetErrorKind::AssetStateInvalid);
        let (after_cache_error, _) = open_active_bundle(&data).expect("active after cache error");
        assert_eq!(after_cache_error, before);

        let cache_root = temp.0.join("complete-cache");
        let cache = Cache::open(&cache_root, digest).expect("complete cache");
        let cache_lock = lock_and_initialize(&cache);
        for member in &profile.transport.members {
            fs::copy(
                transport.join(&member.asset_name),
                cache.members.join(&member.asset_name),
            )
            .expect("copy completed member");
            fs::set_permissions(
                cache.members.join(&member.asset_name),
                fs::Permissions::from_mode(COMPLETE_FILE_MODE),
            )
            .expect("completed member mode");
        }
        cache.publish_transport(&profile).expect("publish cache");
        drop(cache_lock);

        let install_lock = data.join(".install.lock");
        fs::remove_file(&install_lock).expect("remove regular install lock");
        let outside = temp.0.join("outside-install-lock");
        write_private(&outside, b"outside");
        std::os::unix::fs::symlink(&outside, &install_lock).expect("hostile install lock");
        let data_error = sync_with(
            &data,
            Some(&cache_root),
            true,
            contract,
            &ScriptClient::new(vec![]),
        )
        .expect_err("same-root install-lock failure");
        assert_eq!(data_error.kind(), AssetErrorKind::AssetStateInvalid);
        let (after_data_error, _) = open_active_bundle(&data).expect("active after data error");
        assert_eq!(after_data_error, before);
        assert_eq!(
            fs::read(&outside).expect("outside lock unchanged"),
            b"outside"
        );
    }

    #[test]
    fn cleanup_failure_preserves_installer_error_kind_and_reports_context() {
        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let digest = "sha256:6262626262626262626262626262626262626262626262626262626262626262";
        let cache = Cache::open(&temp.0.join("cache"), digest).expect("cache");
        let _lock = lock_and_initialize(&cache);
        for member in &profile.transport.members {
            fs::copy(
                transport.join(&member.asset_name),
                cache.members.join(&member.asset_name),
            )
            .expect("copy member");
            fs::set_permissions(
                cache.members.join(&member.asset_name),
                fs::Permissions::from_mode(COMPLETE_FILE_MODE),
            )
            .expect("member mode");
        }
        let published = cache.publish_transport(&profile).expect("publish");
        let part = published.join("payload.pgi.zst.part0000");
        fs::set_permissions(&part, fs::Permissions::from_mode(CACHE_FILE_MODE))
            .expect("mutable part");
        OpenOptions::new()
            .write(true)
            .open(&part)
            .expect("open part")
            .write_all(b"X")
            .expect("corrupt part");
        fs::set_permissions(&part, fs::Permissions::from_mode(COMPLETE_FILE_MODE))
            .expect("restore part");
        sync_audit::set(sync_audit::FaultPoint::TransportEvict);
        let error = install_cached(&cache, &published, &temp.0.join("data"), &profile, 0, 0)
            .expect_err("installer and cleanup failure");
        assert!(cache_content_error(error.kind()));
        assert!(error.to_string().contains("eviction also failed"));
        assert!(published.exists());
    }

    #[test]
    fn every_durable_sync_boundary_recovers_without_exposing_partial_transport() {
        use sync_audit::FaultPoint;

        let temp = Temp::new();
        let transport = fixture(&temp.0);
        let profile = miniature_profile(&transport, "http://fixture.test");
        let points = [
            FaultPoint::SidecarCreate,
            FaultPoint::SidecarSync,
            FaultPoint::PartialCreate,
            FaultPoint::PartialWrite,
            FaultPoint::PartialSync,
            FaultPoint::MemberRename,
            FaultPoint::MemberDirSync,
            FaultPoint::TransportRename,
            FaultPoint::TransportDirSync,
        ];
        for (ordinal, point) in points.into_iter().enumerate() {
            let digest = format!("sha256:{:064x}", ordinal + 100);
            let cache_root = temp.0.join(format!("fault-cache-{ordinal}"));
            let data_root = temp.0.join(format!("fault-data-{ordinal}"));
            let contract = SyncContract {
                profile: &profile,
                profile_digest: &digest,
                allowed_hosts: &["fixture.test"],
                require_https: false,
            };
            let client = DynamicClient::new(&profile, &transport);
            sync_audit::set(point);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = sync_with(&data_root, Some(&cache_root), false, contract, &client);
                }))
                .is_err(),
                "fault point {point:?} was not reached"
            );
            assert!(active_bundle_missing(&data_root));
            let profile_cache = cache_root
                .join("profiles")
                .join(digest.trim_start_matches("sha256:"));
            if point != FaultPoint::TransportDirSync {
                assert!(
                    !profile_cache.join("transport").exists(),
                    "{point:?} exposed a transport before directory publication"
                );
            }
            let recovered = sync_with(&data_root, Some(&cache_root), false, contract, &client)
                .expect("recover sync boundary");
            assert_eq!(recovered.status, "installed", "{point:?}");
            assert!(profile_cache.join("transport").is_dir());
        }
    }

    fn active_bundle_missing(data_root: &Path) -> bool {
        open_active_bundle(data_root)
            .expect_err("active bundle should be absent")
            .kind()
            == AssetErrorKind::AssetsMissing
    }

    #[test]
    fn response_and_redirect_matrix_fail_closed() {
        for valid in ["\"\"", "\"good\"", "\"!#$%&'()*+,-./:;<=>?@[]^_`{|}~\""] {
            assert!(strong_etag(valid), "valid ETag {valid:?}");
        }
        for invalid in [
            "",
            "\"",
            "W/\"weak\"",
            "\"embedded\"quote\"",
            "\"space here\"",
            "\"tab\there\"",
            "\"obs-\u{80}\"",
        ] {
            assert!(!strong_etag(invalid), "invalid ETag {invalid:?}");
        }
        let (_, _, profile) = super::super::release::production_profile().expect("profile");
        let contract = SyncContract {
            profile: &profile,
            profile_digest: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            allowed_hosts: &["github.com", "release-assets.githubusercontent.com"],
            require_https: true,
        };
        for url in [
            "http://github.com/file",
            "https://evil.example/file",
            "https://user@github.com/file",
            "https://github.com/file#fragment",
            "/relative",
        ] {
            assert!(validate_url(url, contract).is_err(), "{url}");
        }
        let mut encoded = response(200, vec![], Some("\"x\""));
        encoded.content_encoding = Some("gzip".to_owned());
        assert!(validate_common_response(&encoded).is_err());
        let connect_timeout = map_http_error(ureq::Error::Timeout(ureq::Timeout::Connect));
        assert_eq!(connect_timeout.kind(), AssetErrorKind::AssetTimeout);
        assert!(!connect_timeout.to_string().contains("127.0.0.1"));

        let redirects = (0..=MAX_REDIRECTS)
            .map(|_| Response {
                status: 302,
                location: Some("https://github.com/again".to_owned()),
                etag: None,
                content_length: Some(0),
                content_range: None,
                content_encoding: None,
                body: Box::new(io::empty()) as Box<dyn Read + Send>,
            })
            .collect();
        let client = ScriptClient::new(redirects);
        assert!(
            follow_redirects(
                &client,
                "https://github.com/start",
                Some(10),
                Some("\"range\""),
                contract,
            )
            .is_err()
        );
        assert_eq!(client.requests.borrow().len(), MAX_REDIRECTS + 1);
        assert!(client.requests.borrow().iter().all(|request| {
            request.range == Some(10) && request.if_range.as_deref() == Some("\"range\"")
        }));
    }

    #[test]
    fn hostile_cache_shapes_and_oversized_resume_metadata_fail_or_discard_safely() {
        let temp = Temp::new();
        let target = temp.0.join("target");
        fs::create_dir(&target).expect("target");
        std::os::unix::fs::symlink(&target, temp.0.join("cache-link")).expect("cache symlink");
        assert_eq!(
            Cache::open(
                &temp.0.join("cache-link"),
                "sha256:1212121212121212121212121212121212121212121212121212121212121212",
            )
            .expect_err("symlink cache root")
            .kind(),
            AssetErrorKind::AssetStateInvalid
        );

        let transport = fixture(&temp.0);
        let mut profile = miniature_profile(&transport, "http://fixture.test");
        profile.transport.members.truncate(1);
        let member = &profile.transport.members[0];
        let digest = "sha256:3434343434343434343434343434343434343434343434343434343434343434";
        let cache = Cache::open(&temp.0.join("cache"), digest).expect("cache");
        cache
            .initialize_working_directories()
            .expect("working dirs");
        let partial = cache.partial.join(format!("{}.partial", member.asset_name));
        let sidecar = cache
            .partial
            .join(format!("{}.resume.json", member.asset_name));
        write_private(&partial, b"x");
        write_private(&sidecar, &vec![b'x'; MAX_RESUME_BYTES as usize + 1]);
        assert!(
            load_resume(
                cache.partial_dir().expect("partial cache"),
                partial
                    .file_name()
                    .and_then(OsStr::to_str)
                    .expect("partial name"),
                sidecar
                    .file_name()
                    .and_then(OsStr::to_str)
                    .expect("sidecar name"),
                member,
                digest,
            )
            .expect("discard oversized metadata")
            .is_none()
        );
        assert!(!partial.exists());
        assert!(!sidecar.exists());

        fs::create_dir(cache.members.join(&member.asset_name)).expect("nonregular member");
        let client = ScriptClient::new(vec![]);
        assert_eq!(
            cache
                .obtain_member(
                    member,
                    SyncContract {
                        profile: &profile,
                        profile_digest: digest,
                        allowed_hosts: &["fixture.test"],
                        require_https: false,
                    },
                    &client,
                )
                .expect_err("nonregular completed member")
                .kind(),
            AssetErrorKind::AssetStateInvalid
        );
        assert!(client.requests.borrow().is_empty());
    }

    #[test]
    fn real_client_bounds_header_and_body_stalls() {
        let header_listener = TcpListener::bind("127.0.0.1:0").expect("bind header server");
        let header_address = header_listener.local_addr().expect("header address");
        let header_server = thread::spawn(move || {
            let (_stream, _) = header_listener.accept().expect("accept header");
            thread::sleep(Duration::from_millis(80));
        });
        let client = UreqClient::with_timeouts(
            Duration::from_millis(25),
            Duration::from_millis(25),
            Duration::from_millis(25),
        );
        let error = match client.execute(&Request {
            url: format!("http://{header_address}/asset"),
            range: None,
            if_range: None,
        }) {
            Ok(_) => panic!("header request unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), AssetErrorKind::AssetTimeout);
        header_server.join().expect("header server");

        let body_listener = TcpListener::bind("127.0.0.1:0").expect("bind body server");
        let body_address = body_listener.local_addr().expect("body address");
        let body_server = thread::spawn(move || {
            let (mut stream, _) = body_listener.accept().expect("accept body");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nETag: \"stall\"\r\n\r\na")
                .expect("write headers and prefix");
            stream.flush().expect("flush prefix");
            thread::sleep(Duration::from_millis(80));
        });
        let mut response = client
            .execute(&Request {
                url: format!("http://{body_address}/asset"),
                range: None,
                if_range: None,
            })
            .expect("body headers");
        let output_path = temp_path("body-stall");
        let mut output = File::create(&output_path).expect("output");
        let mut hasher = Sha256::new();
        let error = stream_exact(&mut response.body, &mut output, &mut hasher, 2, "stalled")
            .expect_err("body timeout");
        assert_eq!(error.kind(), AssetErrorKind::AssetTimeout);
        body_server.join().expect("body server");
        drop(output);
        fs::remove_file(output_path).expect("remove output");
    }

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pangopup-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
