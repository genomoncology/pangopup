//! Feature-gated retained-data preparation for the inactive runtime v2.

use crate::{CommandError, runtime_profile::prepare_qualified_sparse_runtime_profile};
use pangopup_assets::{
    VerifyRuntimeTransportOutcome, canonical_runtime_profile_bytes,
    install_qualified_runtime_v2_transport, open_qualified_runtime_v2_profile,
    pack_runtime_transport, runtime_profile_id, unpack_runtime_transport,
    verify_production_runtime_transport, verify_qualified_runtime_v2_transport,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = concat!(
    "Usage: pangopup-runtime-v2-qualify prepare --v1-transport <ABSOLUTE_DIR> --sparse-bundle <ABSOLUTE_DIR> --scratch <ABSENT_ABSOLUTE_DIR> --output <ABSENT_ABSOLUTE_DIR>\n",
    "       pangopup-runtime-v2-qualify verify --transport <ABSOLUTE_DIR>\n",
    "       pangopup-runtime-v2-qualify install --transport <ABSOLUTE_DIR> --data-dir <ABSOLUTE_DIR>\n",
    "       pangopup-runtime-v2-qualify admit --data-dir <ABSOLUTE_DIR> --expected-snv <SHA256_ID>",
);
const TRANSPORT_MEMBERS: [&str; 10] = [
    "runtime-transport.json",
    "runtime-profile.json",
    "model-manifest.json",
    "model-NOTICE",
    "model.onnx.zst",
    "reference-manifest.json",
    "reference-NOTICE",
    "reference.pgr.zst",
    "mask-NOTICE",
    "domains.pgm.zst",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrepareRuntimeV2QualificationOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub transport_id: String,
    pub runtime_profile_id: String,
    pub compressed_bytes: u64,
    pub unchanged_model_side_members: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AdmitRuntimeV2QualificationOutcome {
    status: &'static str,
    command: &'static str,
    runtime_profile_id: String,
    snv_bundle_id: String,
    model_bundle_id: String,
    reference_bundle_id: String,
    mask_sha256: String,
}

enum Invocation {
    Prepare(Inputs),
    Verify { transport: PathBuf },
    Install { transport: PathBuf, data: PathBuf },
    Admit { data: PathBuf, expected_snv: String },
}

pub fn main(arguments: impl Iterator<Item = OsString>) -> ExitCode {
    let arguments: Vec<_> = arguments.collect();
    if arguments.as_slice() == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(&arguments).and_then(run) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&error),
    }
}

struct Inputs {
    v1_transport: PathBuf,
    sparse_bundle: PathBuf,
    scratch: PathBuf,
    output: PathBuf,
}

fn run(invocation: Invocation) -> Result<impl Serialize, CommandError> {
    let value = match invocation {
        Invocation::Prepare(inputs) => serde_json::to_value(prepare_runtime_v2_transport(&inputs)?),
        Invocation::Verify { transport } => serde_json::to_value(
            verify_qualified_runtime_v2_transport(&transport).map_err(asset_error)?,
        ),
        Invocation::Install { transport, data } => serde_json::to_value(
            install_qualified_runtime_v2_transport(&transport, &data).map_err(asset_error)?,
        ),
        Invocation::Admit { data, expected_snv } => {
            let installed =
                open_qualified_runtime_v2_profile(&data, &expected_snv).map_err(asset_error)?;
            let profile = installed.profile();
            let bytes = canonical_runtime_profile_bytes(profile)
                .map_err(|error| CommandError::new("QUALIFICATION_AUTHORITY", error.to_string()))?;
            let profile_id = runtime_profile_id(&bytes)
                .map_err(|error| CommandError::new("QUALIFICATION_AUTHORITY", error.to_string()))?;
            serde_json::to_value(AdmitRuntimeV2QualificationOutcome {
                status: "ok",
                command: "runtime-v2-qualification.admit",
                runtime_profile_id: profile_id.to_string(),
                snv_bundle_id: profile.snv.bundle_id.clone(),
                model_bundle_id: profile.model.bundle_id.clone(),
                reference_bundle_id: profile.reference.bundle_id.clone(),
                mask_sha256: profile.mask.member_sha256.clone(),
            })
        }
    }
    .map_err(|error| CommandError::new("QUALIFICATION_JSON", error.to_string()))?;
    Ok(value)
}

fn parse(arguments: &[OsString]) -> Result<Invocation, CommandError> {
    match arguments.first().and_then(|value| value.to_str()) {
        Some("prepare") => parse_prepare(arguments).map(Invocation::Prepare),
        Some("verify") => {
            let values = parse_pairs(arguments, &["--transport"])?;
            let transport = values.into_iter().next().expect("one required value");
            require_absolute(&transport)?;
            Ok(Invocation::Verify { transport })
        }
        Some("install") => {
            let values = parse_pairs(arguments, &["--transport", "--data-dir"])?;
            let mut values = values.into_iter();
            let transport = values.next().expect("first required value");
            let data = values.next().expect("second required value");
            require_absolute(&transport)?;
            require_absolute(&data)?;
            Ok(Invocation::Install { transport, data })
        }
        Some("admit") => {
            let values = parse_pairs(arguments, &["--data-dir", "--expected-snv"])?;
            let mut values = values.into_iter();
            let data = values.next().expect("first required value");
            let expected_snv = values
                .next()
                .expect("second required value")
                .into_os_string()
                .into_string()
                .map_err(|_| CommandError::new("CLI_USAGE", USAGE))?;
            require_absolute(&data)?;
            Ok(Invocation::Admit { data, expected_snv })
        }
        _ => Err(CommandError::new("CLI_USAGE", USAGE)),
    }
}

fn parse_prepare(arguments: &[OsString]) -> Result<Inputs, CommandError> {
    let mut values: [Option<PathBuf>; 4] = [None, None, None, None];
    let mut index = 1;
    while index < arguments.len() {
        let Some(flag) = arguments[index].to_str() else {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        };
        let slot = match flag {
            "--v1-transport" => 0,
            "--sparse-bundle" => 1,
            "--scratch" => 2,
            "--output" => 3,
            _ => return Err(CommandError::new("CLI_USAGE", USAGE)),
        };
        let Some(value) = arguments.get(index + 1) else {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        };
        if values[slot].replace(PathBuf::from(value)).is_some() {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        }
        index += 2;
    }
    let [
        Some(v1_transport),
        Some(sparse_bundle),
        Some(scratch),
        Some(output),
    ] = values
    else {
        return Err(CommandError::new("CLI_USAGE", USAGE));
    };
    let inputs = Inputs {
        v1_transport,
        sparse_bundle,
        scratch,
        output,
    };
    validate_paths(&inputs)?;
    Ok(inputs)
}

fn parse_pairs(arguments: &[OsString], flags: &[&str]) -> Result<Vec<PathBuf>, CommandError> {
    if arguments.len() != 1 + flags.len() * 2 {
        return Err(CommandError::new("CLI_USAGE", USAGE));
    }
    let mut values = vec![None; flags.len()];
    let mut index = 1;
    while index < arguments.len() {
        let Some(flag) = arguments[index].to_str() else {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        };
        let Some(slot) = flags.iter().position(|expected| *expected == flag) else {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        };
        let Some(value) = arguments.get(index + 1) else {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        };
        if values[slot].replace(PathBuf::from(value)).is_some() {
            return Err(CommandError::new("CLI_USAGE", USAGE));
        }
        index += 2;
    }
    values
        .into_iter()
        .map(|value| value.ok_or_else(|| CommandError::new("CLI_USAGE", USAGE)))
        .collect()
}

fn require_absolute(path: &Path) -> Result<(), CommandError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(CommandError::new(
            "QUALIFICATION_PATH",
            "qualification paths must be absolute",
        ))
    }
}

fn validate_paths(inputs: &Inputs) -> Result<(), CommandError> {
    for path in [
        &inputs.v1_transport,
        &inputs.sparse_bundle,
        &inputs.scratch,
        &inputs.output,
    ] {
        require_absolute(path)?;
    }
    if inputs.scratch == inputs.output
        || inputs.scratch.starts_with(&inputs.output)
        || inputs.output.starts_with(&inputs.scratch)
    {
        return Err(CommandError::new(
            "QUALIFICATION_PATH",
            "scratch and output paths must not overlap",
        ));
    }
    for path in [&inputs.scratch, &inputs.output] {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(CommandError::new(
                    "QUALIFICATION_OUTPUT_CONFLICT",
                    "scratch and output paths must be absent",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(io_error("inspect qualification output path", error));
            }
        }
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .ok_or_else(|| CommandError::new("QUALIFICATION_PATH", "path has no parent"))?;
        let metadata = fs::symlink_metadata(parent).map_err(|error| {
            CommandError::new(
                "QUALIFICATION_PATH",
                format!("qualification parent is unavailable: {error}"),
            )
        })?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(CommandError::new(
                "QUALIFICATION_PATH",
                "qualification parent must be a directory, not a symbolic link",
            ));
        }
    }
    Ok(())
}

fn prepare_runtime_v2_transport(
    inputs: &Inputs,
) -> Result<PrepareRuntimeV2QualificationOutcome, CommandError> {
    prepare_with(
        inputs,
        |path| verify_production_runtime_transport(path).map_err(asset_error),
        |sparse, model, reference, mask, output| {
            prepare_qualified_sparse_runtime_profile(sparse, model, reference, mask, output)
        },
        |path| verify_qualified_runtime_v2_transport(path).map_err(asset_error),
    )
}

fn prepare_with<V, P, Q>(
    inputs: &Inputs,
    verify_v1: V,
    prepare_profile: P,
    verify_v2: Q,
) -> Result<PrepareRuntimeV2QualificationOutcome, CommandError>
where
    V: FnOnce(&Path) -> Result<VerifyRuntimeTransportOutcome, CommandError>,
    P: FnOnce(
        &Path,
        &Path,
        &Path,
        &Path,
        &Path,
    ) -> Result<crate::runtime_profile::PrepareRuntimeProfileOutcome, CommandError>,
    Q: FnOnce(&Path) -> Result<VerifyRuntimeTransportOutcome, CommandError>,
{
    verify_v1(&inputs.v1_transport)?;
    let mut scratch = Scratch::create(&inputs.scratch)?;
    let result = prepare_in_scratch(inputs, &mut scratch, prepare_profile, verify_v2);
    match result {
        Ok(outcome) => Ok(outcome),
        Err(primary) => match scratch.cleanup() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(retained_invocation_error(primary, cleanup, &inputs.scratch)),
        },
    }
}

fn prepare_in_scratch<P, Q>(
    inputs: &Inputs,
    scratch: &mut Scratch,
    prepare_profile: P,
    verify_v2: Q,
) -> Result<PrepareRuntimeV2QualificationOutcome, CommandError>
where
    P: FnOnce(
        &Path,
        &Path,
        &Path,
        &Path,
        &Path,
    ) -> Result<crate::runtime_profile::PrepareRuntimeProfileOutcome, CommandError>,
    Q: FnOnce(&Path) -> Result<VerifyRuntimeTransportOutcome, CommandError>,
{
    let unpacked = inputs.scratch.join("v1-unpacked");
    unpack_runtime_transport(&inputs.v1_transport, &unpacked).map_err(asset_error)?;
    let profile_path = inputs.scratch.join("runtime-profile-v2.json");
    let prepared = prepare_profile(
        &inputs.sparse_bundle,
        &unpacked.join("model"),
        &unpacked.join("reference"),
        &unpacked.join("mask/domains.pgm"),
        &profile_path,
    )?;
    let candidate = inputs.scratch.join("runtime-v2-transport");
    let packed = pack_runtime_transport(
        &profile_path,
        &unpacked.join("model"),
        &unpacked.join("reference"),
        &unpacked.join("mask/domains.pgm"),
        &candidate,
    )
    .map_err(asset_error)?;
    let candidate_guard = CandidateGuard::open(&candidate)?;
    let verification_swap = begin_verification_swap(inputs, &candidate)?;
    let verification = verify_v2(&candidate);
    let restoration = finish_verification_swap(verification_swap, &candidate);
    let verified = match (verification, restoration) {
        (_, Err(error)) => return Err(error),
        (result, Ok(())) => result?,
    };
    if packed.transport_id != verified.transport_id
        || packed.runtime_profile_id != verified.runtime_profile_id
        || prepared.profile_id != verified.runtime_profile_id
        || packed.compressed_bytes != verified.compressed_bytes
    {
        return Err(CommandError::new(
            "QUALIFICATION_IDENTITY",
            "prepared runtime v2 identities disagree",
        ));
    }
    candidate_guard.require_staged_at(&candidate)?;
    inject_post_verification_fault(inputs, &candidate)?;
    candidate_guard.require_staged_at(&candidate)?;

    remove_owned_directory(&unpacked)?;
    remove_owned_file(&profile_path)?;
    sync_directory(&inputs.scratch)?;
    let mut published = candidate_guard.publish(&candidate, &inputs.output)?;
    let finalize = sync_directory(
        inputs
            .output
            .parent()
            .expect("validated absolute output has a parent"),
    )
    .and_then(|()| published.require_owned())
    .and_then(|()| scratch.remove_empty())
    .and_then(|()| published.require_owned());
    if let Err(primary) = finalize {
        return match published.cleanup() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(retained_output_error(primary, cleanup, &inputs.output)),
        };
    }
    published.disarm();

    Ok(PrepareRuntimeV2QualificationOutcome {
        status: "ok",
        command: "runtime-v2-qualification.prepare",
        transport_id: verified.transport_id,
        runtime_profile_id: verified.runtime_profile_id,
        compressed_bytes: verified.compressed_bytes,
        unchanged_model_side_members: 8,
    })
}

#[derive(Clone, Copy)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn matches(self, metadata: &fs::Metadata) -> bool {
        metadata.dev() == self.device
            && metadata.ino() == self.inode
            && metadata.len() == self.bytes
            && metadata.mtime() == self.modified_seconds
            && metadata.mtime_nsec() == self.modified_nanoseconds
            && metadata.ctime() == self.changed_seconds
            && metadata.ctime_nsec() == self.changed_nanoseconds
    }

    fn same_object(self, metadata: &fs::Metadata) -> bool {
        metadata.dev() == self.device && metadata.ino() == self.inode
    }
}

struct HeldMember {
    name: &'static str,
    file: File,
    identity: ObjectIdentity,
}

struct CandidateGuard {
    directory: File,
    identity: ObjectIdentity,
    members: Vec<HeldMember>,
}

impl CandidateGuard {
    fn open(path: &Path) -> Result<Self, CommandError> {
        let directory = open_directory(path, "open qualified runtime v2 candidate")?;
        let metadata = directory
            .metadata()
            .map_err(|error| io_error("inspect held runtime v2 candidate", error))?;
        let identity = ObjectIdentity::from_metadata(&metadata);
        let mut members = Vec::with_capacity(TRANSPORT_MEMBERS.len());
        for name in TRANSPORT_MEMBERS {
            let file = open_member(&directory, name)?;
            let metadata = file
                .metadata()
                .map_err(|error| io_error("inspect held runtime v2 member", error))?;
            if !metadata.file_type().is_file() || metadata.nlink() != 1 {
                return Err(CommandError::new(
                    "QUALIFICATION_STAGE_IDENTITY",
                    "runtime v2 candidate member is not a single-link regular file",
                ));
            }
            members.push(HeldMember {
                name,
                file,
                identity: ObjectIdentity::from_metadata(&metadata),
            });
        }
        let guard = Self {
            directory,
            identity,
            members,
        };
        guard.require_staged_at(path)?;
        Ok(guard)
    }

    fn require_staged_at(&self, path: &Path) -> Result<(), CommandError> {
        self.require_at(path, true)
    }

    fn require_published_at(&self, path: &Path) -> Result<(), CommandError> {
        self.require_at(path, false)
    }

    fn require_at(&self, path: &Path, require_full_directory: bool) -> Result<(), CommandError> {
        let directory = fs::symlink_metadata(path)
            .map_err(|error| io_error("inspect runtime v2 candidate path", error))?;
        if !directory.file_type().is_dir()
            || directory.file_type().is_symlink()
            || !self.identity.same_object(&directory)
            || (require_full_directory && !self.identity.matches(&directory))
            || !self.identity.same_object(
                &self
                    .directory
                    .metadata()
                    .map_err(|error| io_error("reinspect held runtime v2 candidate", error))?,
            )
        {
            return Err(CommandError::new(
                "QUALIFICATION_STAGE_IDENTITY",
                "verified runtime v2 candidate directory was replaced or changed",
            ));
        }
        let observed: BTreeSet<_> = fs::read_dir(path)
            .map_err(|error| io_error("list runtime v2 candidate", error))?
            .map(|entry| {
                entry
                    .map_err(|error| io_error("read runtime v2 candidate entry", error))?
                    .file_name()
                    .into_string()
                    .map_err(|_| {
                        CommandError::new(
                            "QUALIFICATION_STAGE_IDENTITY",
                            "runtime v2 candidate member name is not UTF-8",
                        )
                    })
            })
            .collect::<Result<_, _>>()?;
        let expected: BTreeSet<_> = TRANSPORT_MEMBERS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        if observed != expected {
            return Err(CommandError::new(
                "QUALIFICATION_STAGE_IDENTITY",
                "verified runtime v2 candidate member set changed",
            ));
        }
        for member in &self.members {
            let held = member
                .file
                .metadata()
                .map_err(|error| io_error("reinspect held runtime v2 member", error))?;
            let named = fs::symlink_metadata(path.join(member.name))
                .map_err(|error| io_error("reinspect runtime v2 member path", error))?;
            if !named.file_type().is_file()
                || named.nlink() != 1
                || !member.identity.matches(&held)
                || !member.identity.matches(&named)
            {
                return Err(CommandError::new(
                    "QUALIFICATION_STAGE_IDENTITY",
                    format!("verified runtime v2 member changed: {}", member.name),
                ));
            }
        }
        Ok(())
    }

    fn publish(&self, source: &Path, output: &Path) -> Result<PublishedOutput<'_>, CommandError> {
        self.require_staged_at(source)?;
        inject_prepublication_fault(source)?;
        rename_noreplace(source, output)?;
        if let Err(primary) = self.require_published_at(output) {
            return match self.directory_owned_at(output) {
                Ok(true) => match fs::remove_dir_all(output)
                    .map_err(|error| io_error("remove rejected runtime v2 output", error))
                {
                    Ok(()) => Err(primary),
                    Err(cleanup) => Err(retained_output_error(primary, cleanup, output)),
                },
                Ok(false) => Err(retained_unknown_output_error(primary, output)),
                Err(inspection) => Err(unknown_output_ownership_error(primary, inspection, output)),
            };
        }
        Ok(PublishedOutput {
            path: output.to_owned(),
            candidate: self,
            owned: true,
        })
    }

    fn directory_owned_at(&self, path: &Path) -> Result<bool, CommandError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                Ok(metadata.file_type().is_dir() && self.identity.same_object(&metadata))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error("inspect rejected runtime v2 output", error)),
        }
    }
}

fn open_directory(path: &Path, action: &str) -> Result<File, CommandError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| io_error(action, error))
}

fn open_member(directory: &File, name: &str) -> Result<File, CommandError> {
    let name = std::ffi::CString::new(name).expect("fixed member name has no NUL");
    // SAFETY: the held directory descriptor and fixed C string remain valid
    // for this call. A successful call returns one newly owned descriptor.
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(io_error(
            "open held runtime v2 member",
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: `openat` returned a newly owned descriptor above.
    Ok(unsafe { std::os::fd::FromRawFd::from_raw_fd(descriptor) })
}

struct PublishedOutput<'a> {
    path: PathBuf,
    candidate: &'a CandidateGuard,
    owned: bool,
}

impl PublishedOutput<'_> {
    fn require_owned(&self) -> Result<(), CommandError> {
        self.candidate.require_published_at(&self.path)
    }

    fn disarm(&mut self) {
        self.owned = false;
    }

    fn cleanup(&mut self) -> Result<(), CommandError> {
        if !self.owned {
            return Ok(());
        }
        self.candidate.require_published_at(&self.path)?;
        fs::remove_dir_all(&self.path)
            .map_err(|error| io_error("remove published runtime v2 output", error))?;
        self.owned = false;
        sync_directory(
            self.path
                .parent()
                .expect("validated absolute output has a parent"),
        )
    }
}

struct Scratch {
    path: PathBuf,
    device: u64,
    inode: u64,
    owned: bool,
}

impl Scratch {
    fn create(path: &Path) -> Result<Self, CommandError> {
        fs::create_dir(path).map_err(|error| io_error("create qualification scratch", error))?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let primary = io_error("inspect qualification scratch", error);
                return match fs::remove_dir(path)
                    .map_err(|error| io_error("remove uninspected qualification scratch", error))
                {
                    Ok(()) => Err(primary),
                    Err(cleanup) => Err(retained_invocation_error(primary, cleanup, path)),
                };
            }
        };
        let scratch = Self {
            path: path.to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
            owned: true,
        };
        if let Err(primary) = fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error("set qualification scratch permissions", error))
            .and_then(|()| scratch.require_owned())
        {
            return match fs::remove_dir(path)
                .map_err(|error| io_error("remove rejected qualification scratch", error))
            {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(retained_invocation_error(primary, cleanup, path)),
            };
        }
        Ok(scratch)
    }

    fn remove_empty(&mut self) -> Result<(), CommandError> {
        self.require_owned()?;
        fs::remove_dir(&self.path)
            .map_err(|error| io_error("remove empty qualification scratch", error))?;
        self.owned = false;
        Ok(())
    }

    fn require_owned(&self) -> Result<(), CommandError> {
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|error| io_error("inspect qualification scratch", error))?;
        if !metadata.file_type().is_dir()
            || metadata.dev() != self.device
            || metadata.ino() != self.inode
        {
            return Err(CommandError::new(
                "QUALIFICATION_SCRATCH",
                "qualification scratch was replaced",
            ));
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<(), CommandError> {
        if !self.owned {
            return Ok(());
        }
        inject_cleanup_failure()?;
        self.require_owned()?;
        fs::remove_dir_all(&self.path)
            .map_err(|error| io_error("remove qualification invocation directory", error))?;
        self.owned = false;
        Ok(())
    }
}

fn retained_invocation_error(
    primary: CommandError,
    cleanup: CommandError,
    scratch: &Path,
) -> CommandError {
    let retained = scratch.display().to_string();
    let mut error = CommandError::new(
        primary.code,
        format!(
            "{}; cleanup failed: {}; retained invocation directory: {retained}",
            primary.message, cleanup.message
        ),
    );
    error.details = Some(serde_json::json!({
        "cleanup": "failed",
        "cleanup_error": cleanup.message,
        "retained_invocation_directory": retained,
    }));
    error
}

fn retained_output_error(
    primary: CommandError,
    cleanup: CommandError,
    output: &Path,
) -> CommandError {
    let retained = output.display().to_string();
    let mut error = CommandError::new(
        primary.code,
        format!(
            "{}; output cleanup failed: {}; retained output directory: {retained}",
            primary.message, cleanup.message
        ),
    );
    error.details = Some(serde_json::json!({
        "cleanup": "failed",
        "cleanup_error": cleanup.message,
        "retained_output_directory": retained,
    }));
    error
}

fn retained_unknown_output_error(primary: CommandError, output: &Path) -> CommandError {
    let retained = output.display().to_string();
    let mut error = CommandError::new(
        primary.code,
        format!(
            "{}; rejected output is not owned and was retained: {retained}",
            primary.message
        ),
    );
    error.details = Some(serde_json::json!({
        "cleanup": "not_attempted_unowned",
        "retained_output_directory": retained,
    }));
    error
}

fn unknown_output_ownership_error(
    primary: CommandError,
    inspection: CommandError,
    output: &Path,
) -> CommandError {
    let retained = output.display().to_string();
    let mut error = CommandError::new(
        primary.code,
        format!(
            "{}; output ownership could not be checked: {}; output was retained: {retained}",
            primary.message, inspection.message
        ),
    );
    error.details = Some(serde_json::json!({
        "cleanup": "not_attempted_ownership_unknown",
        "cleanup_error": inspection.message,
        "retained_output_directory": retained,
    }));
    error
}

fn remove_owned_directory(path: &Path) -> Result<(), CommandError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect qualification directory", error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(CommandError::new(
            "QUALIFICATION_SCRATCH",
            "qualification directory has an unsafe type",
        ));
    }
    fs::remove_dir_all(path).map_err(|error| io_error("remove qualification directory", error))
}

fn remove_owned_file(path: &Path) -> Result<(), CommandError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect qualification file", error))?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(CommandError::new(
            "QUALIFICATION_SCRATCH",
            "qualification file has an unsafe type or link count",
        ));
    }
    fs::remove_file(path).map_err(|error| io_error("remove qualification file", error))
}

fn rename_noreplace(source: &Path, output: &Path) -> Result<(), CommandError> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|error| {
        if matches!(
            error.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty
        ) {
            CommandError::new(
                "QUALIFICATION_OUTPUT_CONFLICT",
                "qualification output already exists",
            )
        } else {
            io_error("publish qualified runtime v2 transport", error)
        }
    })
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync qualification directory", error))
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QualificationFault {
    VerificationSwapAndRestore,
    CandidateDirectoryReplacement,
    CandidateMemberMutation,
    PublicationDirectoryReplacement,
    PublicationMemberMutation,
    PublicationConflict,
    CleanupFailure,
    ScratchReplacement,
}

#[cfg(test)]
thread_local! {
    static QUALIFICATION_FAULT: std::cell::Cell<Option<QualificationFault>> = const { std::cell::Cell::new(None) };
    static FAIL_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn set_qualification_fault(fault: QualificationFault) {
    QUALIFICATION_FAULT.set(Some(fault));
}

#[cfg(test)]
struct VerificationSwap {
    held: Option<PathBuf>,
    identity: Option<ObjectIdentity>,
}

#[cfg(not(test))]
struct VerificationSwap;

#[cfg(test)]
fn begin_verification_swap(
    _inputs: &Inputs,
    candidate: &Path,
) -> Result<VerificationSwap, CommandError> {
    let Some(fault) = QUALIFICATION_FAULT.take() else {
        return Ok(VerificationSwap {
            held: None,
            identity: None,
        });
    };
    if fault != QualificationFault::VerificationSwapAndRestore {
        QUALIFICATION_FAULT.set(Some(fault));
        return Ok(VerificationSwap {
            held: None,
            identity: None,
        });
    }

    let identity = ObjectIdentity::from_metadata(
        &fs::symlink_metadata(candidate)
            .map_err(|error| io_error("inspect candidate before verification swap", error))?,
    );
    let held = candidate.with_extension("verification-held");
    fs::rename(candidate, &held)
        .map_err(|error| io_error("move candidate for verification swap", error))?;
    fs::create_dir(candidate)
        .map_err(|error| io_error("create verification replacement", error))?;
    for member in TRANSPORT_MEMBERS {
        fs::copy(held.join(member), candidate.join(member))
            .map_err(|error| io_error("copy verification replacement member", error))?;
    }
    Ok(VerificationSwap {
        held: Some(held),
        identity: Some(identity),
    })
}

#[cfg(not(test))]
fn begin_verification_swap(
    _inputs: &Inputs,
    _candidate: &Path,
) -> Result<VerificationSwap, CommandError> {
    Ok(VerificationSwap)
}

#[cfg(test)]
fn finish_verification_swap(swap: VerificationSwap, candidate: &Path) -> Result<(), CommandError> {
    let (Some(held), Some(identity)) = (swap.held, swap.identity) else {
        return Ok(());
    };
    fs::remove_dir_all(candidate)
        .map_err(|error| io_error("remove verification replacement", error))?;
    fs::rename(&held, candidate)
        .map_err(|error| io_error("restore candidate after verification swap", error))?;
    let restored = fs::symlink_metadata(candidate)
        .map_err(|error| io_error("inspect restored verification candidate", error))?;
    if !identity.same_object(&restored) {
        return Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            "verification swap did not restore the original directory inode",
        ));
    }
    if identity.matches(&restored) {
        let marker = candidate.join(".verification-swap-marker");
        File::create(&marker)
            .and_then(|file| file.sync_all())
            .and_then(|()| fs::remove_file(&marker))
            .map_err(|error| io_error("mark restored verification candidate", error))?;
    }
    let restored = fs::symlink_metadata(candidate)
        .map_err(|error| io_error("reinspect restored verification candidate", error))?;
    if identity.matches(&restored) {
        return Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            "verification swap did not change captured directory metadata",
        ));
    }
    Ok(())
}

#[cfg(not(test))]
fn finish_verification_swap(
    _swap: VerificationSwap,
    _candidate: &Path,
) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
fn inject_post_verification_fault(inputs: &Inputs, candidate: &Path) -> Result<(), CommandError> {
    let Some(fault) = QUALIFICATION_FAULT.take() else {
        return Ok(());
    };
    match fault {
        QualificationFault::VerificationSwapAndRestore => Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            "verification swap fault reached the wrong checkpoint",
        )),
        QualificationFault::CandidateDirectoryReplacement => {
            fs::rename(candidate, inputs.scratch.join("verified-candidate"))
                .map_err(|error| io_error("move verified candidate", error))?;
            fs::create_dir(candidate)
                .map_err(|error| io_error("replace verified candidate", error))?;
            Ok(())
        }
        QualificationFault::CandidateMemberMutation => {
            mutate_member_in_place(candidate, "runtime-profile.json")
        }
        QualificationFault::PublicationConflict => fs::create_dir(&inputs.output)
            .map_err(|error| io_error("create publication conflict", error)),
        fault @ (QualificationFault::PublicationDirectoryReplacement
        | QualificationFault::PublicationMemberMutation) => {
            QUALIFICATION_FAULT.set(Some(fault));
            Ok(())
        }
        QualificationFault::CleanupFailure => {
            FAIL_CLEANUP.set(true);
            Err(CommandError::new(
                "QUALIFICATION_INJECTED",
                "injected qualification failure before cleanup",
            ))
        }
        QualificationFault::ScratchReplacement => {
            fs::rename(&inputs.scratch, inputs.scratch.with_extension("verified"))
                .map_err(|error| io_error("move qualification invocation directory", error))?;
            fs::create_dir(&inputs.scratch)
                .map_err(|error| io_error("replace qualification invocation directory", error))?;
            Err(CommandError::new(
                "QUALIFICATION_SCRATCH",
                "qualification invocation directory was replaced",
            ))
        }
    }
}

#[cfg(test)]
fn inject_prepublication_fault(candidate: &Path) -> Result<(), CommandError> {
    let Some(fault) = QUALIFICATION_FAULT.take() else {
        return Ok(());
    };
    match fault {
        QualificationFault::PublicationDirectoryReplacement => {
            fs::rename(candidate, candidate.with_extension("verified-publication"))
                .map_err(|error| io_error("move publication candidate", error))?;
            fs::create_dir(candidate)
                .map_err(|error| io_error("replace publication candidate", error))
        }
        QualificationFault::PublicationMemberMutation => {
            mutate_member_in_place(candidate, "runtime-profile.json")
        }
        other => Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            format!("unexpected prepublication fault: {other:?}"),
        )),
    }
}

#[cfg(test)]
fn mutate_member_in_place(candidate: &Path, name: &str) -> Result<(), CommandError> {
    let member = candidate.join(name);
    let before = fs::symlink_metadata(&member)
        .map_err(|error| io_error("inspect member before injected mutation", error))?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(&member)
        .map_err(|error| io_error("open member for injected mutation", error))?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error("write injected member mutation", error))?;
    let after = file
        .metadata()
        .map_err(|error| io_error("inspect member after injected mutation", error))?;
    if before.dev() != after.dev() || before.ino() != after.ino() || after.len() != before.len() + 1
    {
        return Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            "injected mutation did not change bytes on the same inode",
        ));
    }
    let observed: BTreeSet<_> = fs::read_dir(candidate)
        .map_err(|error| io_error("list candidate after injected mutation", error))?
        .map(|entry| {
            entry
                .map_err(|error| io_error("read candidate after injected mutation", error))?
                .file_name()
                .into_string()
                .map_err(|_| {
                    CommandError::new(
                        "QUALIFICATION_INJECTED",
                        "candidate member name is not UTF-8 after injected mutation",
                    )
                })
        })
        .collect::<Result<_, _>>()?;
    let expected: BTreeSet<_> = TRANSPORT_MEMBERS
        .iter()
        .map(|member| (*member).to_owned())
        .collect();
    if observed != expected {
        return Err(CommandError::new(
            "QUALIFICATION_INJECTED",
            "injected mutation changed the candidate inventory",
        ));
    }
    Ok(())
}

#[cfg(not(test))]
fn inject_prepublication_fault(_candidate: &Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(not(test))]
fn inject_post_verification_fault(_inputs: &Inputs, _candidate: &Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
fn inject_cleanup_failure() -> Result<(), CommandError> {
    if FAIL_CLEANUP.replace(false) {
        Err(CommandError::new(
            "QUALIFICATION_CLEANUP",
            "injected cleanup failure",
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn inject_cleanup_failure() -> Result<(), CommandError> {
    Ok(())
}

fn asset_error(error: pangopup_assets::AssetError) -> CommandError {
    CommandError::new(error.kind().code(), error.to_string())
}

fn io_error(action: &str, error: io::Error) -> CommandError {
    CommandError::new("QUALIFICATION_IO", format!("{action}: {error}"))
}

fn json_success(value: &impl Serialize) -> ExitCode {
    match serde_json::to_writer(io::stdout().lock(), value) {
        Ok(()) => {
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => json_failure(&CommandError::new("QUALIFICATION_IO", error.to_string())),
    }
}

fn json_failure(error: &CommandError) -> ExitCode {
    let mut stderr = io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, error);
    let _ = stderr.write_all(b"\n");
    if error.code == "CLI_USAGE" {
        ExitCode::from(2)
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_profile::PrepareRuntimeProfileOutcome;
    use pangopup_assets::{
        MaskProfile, ModelProfile, ReferenceProfile, RuntimeProfile, ScoringProfile, SnvProfile,
        canonical_runtime_profile_bytes, runtime_profile_id, verify_runtime_transport,
    };
    use pangopup_core::ReferenceProvider;
    use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
    use pangopup_model::{ModelRepresentation, inspect_runtime_profile_bundle};
    use tempfile::tempdir;

    const MODEL_SIDE_MEMBERS: [&str; 8] = [
        "model-manifest.json",
        "model-NOTICE",
        "model.onnx.zst",
        "reference-manifest.json",
        "reference-NOTICE",
        "reference.pgr.zst",
        "mask-NOTICE",
        "domains.pgm.zst",
    ];

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
    }

    fn profile(snv_digit: char) -> RuntimeProfile {
        let fixtures = fixtures();
        let model =
            inspect_runtime_profile_bundle(&fixtures.join("pangolin-model-kernel-mini/bundle"))
                .expect("model");
        let reference =
            ReferenceBundleOpen::open_identified(&fixtures.join("reference-route-test/bundle"))
                .expect("reference");
        let mask =
            MaskDomainsOpen::open_identified(&fixtures.join("gencode-mask-mini/domains.pgm"))
                .expect("mask");
        let provenance = reference.provenance();
        RuntimeProfile {
            schema: "pangopup.runtime-profile.v1".to_owned(),
            snv: SnvProfile {
                bundle_id: format!("sha256:{}", snv_digit.to_string().repeat(64)),
                format: "pangopup.sparse-direct.v1".to_owned(),
                member_bytes: 123,
                member_sha256: format!("sha256:{}", snv_digit.to_string().repeat(64)),
            },
            model: ModelProfile {
                bundle_id: model.bundle_id.to_string(),
                profile: model.profile,
                representation: match model.representation {
                    ModelRepresentation::Singleton => "singleton",
                    ModelRepresentation::ZeroPaddedBatch => "zero-padded-batch",
                    ModelRepresentation::PairedStrandBatch => "paired-strand-batch",
                }
                .to_owned(),
                member_bytes: model.member_bytes,
                member_sha256: model.member_sha256,
            },
            reference: ReferenceProfile {
                bundle_id: provenance.bundle_id().to_owned(),
                profile: provenance.profile().to_owned(),
                format: provenance.format().to_owned(),
                assembly: provenance.assembly().to_owned(),
                assembly_accession: provenance.assembly_accession().to_owned(),
                sequence_set_sha256: provenance.sequence_set_sha256().to_owned(),
                member_bytes: reference.identity().bytes(),
                member_sha256: reference.identity().sha256().to_owned(),
            },
            mask: MaskProfile {
                format: "pangopup.gencode-v38-domains.v1".to_owned(),
                member_bytes: mask.identity().bytes(),
                member_sha256: format!("sha256:{}", mask.identity().sha256()),
            },
            scoring: ScoringProfile {
                assembly: "GRCh38".to_owned(),
                semantics: "pangopup-variant-score-v1".to_owned(),
                distance: 50,
                masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
                cpu_policy: "sequential:1/1".to_owned(),
            },
        }
    }

    fn write_profile(path: &Path, profile: &RuntimeProfile) -> PrepareRuntimeProfileOutcome {
        let bytes = canonical_runtime_profile_bytes(profile).expect("profile bytes");
        fs::write(path, &bytes).expect("write profile");
        PrepareRuntimeProfileOutcome {
            status: "ok",
            command: "runtime-profile.prepare",
            profile_id: runtime_profile_id(&bytes).expect("profile id").to_string(),
            bytes: bytes.len() as u64,
        }
    }

    fn packed_v1(root: &Path, profile: &RuntimeProfile) -> PathBuf {
        let fixtures = fixtures();
        let profile_path = root.join("v1-profile.json");
        write_profile(&profile_path, profile);
        let transport = root.join("v1-transport");
        pack_runtime_transport(
            &profile_path,
            &fixtures.join("pangolin-model-kernel-mini/bundle"),
            &fixtures.join("reference-route-test/bundle"),
            &fixtures.join("gencode-mask-mini/domains.pgm"),
            &transport,
        )
        .expect("pack v1");
        transport
    }

    #[test]
    fn runner_publishes_only_the_verified_two_member_change() {
        let temp = tempdir().expect("temp");
        let v1_profile = profile('1');
        let v2_profile = profile('2');
        let v1_transport = packed_v1(temp.path(), &v1_profile);
        let inputs = Inputs {
            v1_transport: v1_transport.clone(),
            sparse_bundle: temp.path().join("sparse-authority"),
            scratch: temp.path().join("scratch"),
            output: temp.path().join("v2-transport"),
        };
        let outcome = prepare_with(
            &inputs,
            |path| verify_runtime_transport(path).map_err(asset_error),
            |_, _, _, _, output| Ok(write_profile(output, &v2_profile)),
            |path| verify_runtime_transport(path).map_err(asset_error),
        )
        .expect("prepare");
        assert_eq!(outcome.unchanged_model_side_members, 8);
        assert!(!inputs.scratch.exists());
        assert!(inputs.output.is_dir());
        for member in MODEL_SIDE_MEMBERS {
            assert_eq!(
                fs::read(v1_transport.join(member)).expect("v1 member"),
                fs::read(inputs.output.join(member)).expect("v2 member"),
                "{member}"
            );
        }
        assert_ne!(
            fs::read(v1_transport.join("runtime-profile.json")).expect("v1 profile"),
            fs::read(inputs.output.join("runtime-profile.json")).expect("v2 profile")
        );
        assert_ne!(
            fs::read(v1_transport.join("runtime-transport.json")).expect("v1 manifest"),
            fs::read(inputs.output.join("runtime-transport.json")).expect("v2 manifest")
        );
    }

    #[test]
    fn failed_final_authority_check_leaves_no_output_or_scratch() {
        let temp = tempdir().expect("temp");
        let v1_profile = profile('1');
        let v2_profile = profile('2');
        let inputs = Inputs {
            v1_transport: packed_v1(temp.path(), &v1_profile),
            sparse_bundle: temp.path().join("sparse-authority"),
            scratch: temp.path().join("scratch"),
            output: temp.path().join("v2-transport"),
        };
        let error = prepare_with(
            &inputs,
            |path| verify_runtime_transport(path).map_err(asset_error),
            |_, _, _, _, output| Ok(write_profile(output, &v2_profile)),
            |_| {
                Err(CommandError::new(
                    "QUALIFICATION_AUTHORITY",
                    "substituted transport",
                ))
            },
        )
        .expect_err("authority rejection");
        assert_eq!(error.code, "QUALIFICATION_AUTHORITY");
        assert!(!inputs.scratch.exists());
        assert!(!inputs.output.exists());
    }

    fn run_with_fault(fault: QualificationFault) -> (tempfile::TempDir, Inputs, CommandError) {
        let temp = tempdir().expect("temp");
        let v1_profile = profile('1');
        let v2_profile = profile('2');
        let inputs = Inputs {
            v1_transport: packed_v1(temp.path(), &v1_profile),
            sparse_bundle: temp.path().join("sparse-authority"),
            scratch: temp.path().join("scratch"),
            output: temp.path().join("v2-transport"),
        };
        set_qualification_fault(fault);
        let error = prepare_with(
            &inputs,
            |path| verify_runtime_transport(path).map_err(asset_error),
            |_, _, _, _, output| Ok(write_profile(output, &v2_profile)),
            |path| verify_runtime_transport(path).map_err(asset_error),
        )
        .expect_err("injected qualification failure");
        (temp, inputs, error)
    }

    #[test]
    fn post_verification_candidate_replacement_and_member_mutation_never_publish() {
        let (_temp, inputs, error) =
            run_with_fault(QualificationFault::CandidateDirectoryReplacement);
        assert_eq!(error.code, "QUALIFICATION_STAGE_IDENTITY");
        assert!(!inputs.output.exists());
        assert!(!inputs.scratch.exists());

        let (_temp, inputs, error) = run_with_fault(QualificationFault::CandidateMemberMutation);
        assert_eq!(error.code, "QUALIFICATION_STAGE_IDENTITY");
        assert_eq!(
            error.message,
            "verified runtime v2 member changed: runtime-profile.json"
        );
        assert!(!inputs.output.exists());
        assert!(!inputs.scratch.exists());
    }

    #[test]
    fn verification_path_swap_and_restore_changes_captured_directory_metadata() {
        let (_temp, inputs, error) = run_with_fault(QualificationFault::VerificationSwapAndRestore);
        assert_eq!(error.code, "QUALIFICATION_STAGE_IDENTITY");
        assert_eq!(
            error.message,
            "verified runtime v2 candidate directory was replaced or changed"
        );
        assert!(!inputs.output.exists());
        assert!(!inputs.scratch.exists());
    }

    #[test]
    fn publication_conflict_preserves_the_other_output_and_cleans_scratch() {
        let (_temp, inputs, error) = run_with_fault(QualificationFault::PublicationConflict);
        assert_eq!(error.code, "QUALIFICATION_OUTPUT_CONFLICT");
        assert!(inputs.output.is_dir());
        assert!(!inputs.scratch.exists());
    }

    #[test]
    fn publication_rechecks_directory_and_members_after_no_replace_rename() {
        let (_temp, inputs, error) = run_with_fault(QualificationFault::PublicationMemberMutation);
        assert_eq!(error.code, "QUALIFICATION_STAGE_IDENTITY");
        assert_eq!(
            error.message,
            "verified runtime v2 member changed: runtime-profile.json"
        );
        assert!(!inputs.output.exists());
        assert!(!inputs.scratch.exists());

        let (_temp, inputs, error) =
            run_with_fault(QualificationFault::PublicationDirectoryReplacement);
        assert_eq!(error.code, "QUALIFICATION_STAGE_IDENTITY");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details["cleanup"].as_str()),
            Some("not_attempted_unowned")
        );
        assert!(inputs.output.is_dir());
        assert!(!inputs.scratch.exists());
    }

    #[test]
    fn cleanup_failure_and_replaced_scratch_report_retained_invocation_directory() {
        for fault in [
            QualificationFault::CleanupFailure,
            QualificationFault::ScratchReplacement,
        ] {
            let (_temp, inputs, error) = run_with_fault(fault);
            assert!(error.message.contains("retained invocation directory"));
            assert!(
                error
                    .message
                    .contains(inputs.scratch.to_str().expect("UTF-8"))
            );
            assert_eq!(
                error
                    .details
                    .as_ref()
                    .and_then(|details| details["cleanup"].as_str()),
                Some("failed")
            );
            assert_eq!(
                error
                    .details
                    .as_ref()
                    .and_then(|details| details["retained_invocation_directory"].as_str()),
                inputs.scratch.to_str()
            );
        }
    }
}
