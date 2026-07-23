//! Linux-only, content-blind ownership and supervision of a leased payload fd.
//!
//! `LeasedPayload` deliberately exposes neither `File` nor a raw descriptor and
//! implements neither `Read` nor `Seek`. Every operation on the payload fd is
//! classified here; there is no content-access operation in this boundary.

use std::{
    ffi::{CString, OsStr},
    fs::File,
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
    time::Duration,
};

const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;
const F_SETOWN_EX: libc::c_int = 15;
const F_GETOWN_EX: libc::c_int = 16;
const F_OWNER_TID: libc::c_int = 0;
pub(crate) const LEASE_CLEANUP_DEADLINE: Duration = Duration::from_secs(5);

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadOperation {
    BlockUploadSignals,
    CreateSignalFd,
    ReadLeaseBreakTime,
    OpenNoFollow,
    AcquireReadLease,
    SetOwnerThread,
    GetOwnerThread,
    QueryLeaseAfterOwner,
    Fstat,
    QueryOffsetBeforeSpawn,
    DuplicateChildStdin,
    DrainSignalsBeforeSpawn,
    PollLeaseBreak,
    DrainLeaseBreak,
    FinalQueryLease,
    ReleaseLease,
    ClosePayload,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg(any(test, feature = "test-read-audit"))]
pub enum LeaseBreakTimeTest {
    #[default]
    System,
    Unavailable,
    Malformed,
    Seconds(u64),
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct PayloadTestFaults {
    pub lease_break_time: LeaseBreakTimeTest,
    pub set_owner_error: bool,
    pub get_owner_error: bool,
    pub owner_mismatch: bool,
    pub post_owner_query_error: bool,
    pub post_owner_lease_lost: bool,
    pub final_lease_lost: bool,
    pub cleanup_deadline_exhausted: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PayloadConfig {
    #[cfg(any(test, feature = "test-read-audit"))]
    pub faults: PayloadTestFaults,
}

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[repr(C)]
struct FOwnerEx {
    kind: libc::c_int,
    pid: libc::pid_t,
}

pub(crate) struct LeasedPayload {
    payload: Option<OwnedFd>,
    lease_active: bool,
    operations: Vec<PayloadOperation>,
    #[cfg(any(test, feature = "test-read-audit"))]
    config: PayloadConfig,
}

pub(crate) struct UploadSignals {
    signal: Option<OwnedFd>,
    old_mask: libc::sigset_t,
    mask_active: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PendingUploadSignals {
    pub interrupt: Option<libc::c_int>,
    pub lease_break: bool,
}

impl UploadSignals {
    pub(crate) fn block() -> io::Result<Self> {
        let (signal_set, old_mask) = block_upload_signals()?;
        let signal = match create_signal_fd(&signal_set) {
            Ok(signal) => signal,
            Err(error) => {
                restore_signal_mask(&old_mask);
                return Err(error);
            }
        };
        Ok(Self {
            signal: Some(signal),
            old_mask,
            mask_active: true,
        })
    }

    pub(crate) fn drain(&mut self) -> io::Result<PendingUploadSignals> {
        drain_signal_fd(self.signal_fd())
    }

    pub(crate) fn original_mask(&self) -> libc::sigset_t {
        self.old_mask
    }

    fn signal_fd(&self) -> libc::c_int {
        self.signal
            .as_ref()
            .expect("signal fd exists while upload supervisor is active")
            .as_raw_fd()
    }
}

impl Drop for UploadSignals {
    fn drop(&mut self) {
        if let Some(signal) = self.signal.take() {
            let _ = drain_signal_fd(signal.as_raw_fd());
            drop(signal);
        }
        if self.mask_active {
            restore_signal_mask(&self.old_mask);
            self.mask_active = false;
        }
    }
}

impl LeasedPayload {
    pub(crate) fn open(root: &File, name: &str, config: PayloadConfig) -> io::Result<Self> {
        #[cfg(not(any(test, feature = "test-read-audit")))]
        let _ = config;
        let operations = vec![
            PayloadOperation::BlockUploadSignals,
            PayloadOperation::CreateSignalFd,
        ];
        let mut leased = Self {
            payload: None,
            lease_active: false,
            operations,
            #[cfg(any(test, feature = "test-read-audit"))]
            config,
        };
        leased.record(PayloadOperation::ReadLeaseBreakTime);
        leased.validate_lease_break_time()?;
        leased.record(PayloadOperation::OpenNoFollow);
        leased.payload = Some(open_payload(root, name)?);
        leased.record(PayloadOperation::AcquireReadLease);
        if unsafe { libc::fcntl(leased.fd(), libc::F_SETLEASE, libc::F_RDLCK) } == -1 {
            return Err(io::Error::last_os_error());
        }
        leased.lease_active = true;

        leased.record(PayloadOperation::SetOwnerThread);
        #[cfg(any(test, feature = "test-read-audit"))]
        if leased.config.faults.set_owner_error {
            return Err(io::Error::other("injected F_SETOWN_EX failure"));
        }
        let expected_tid = gettid();
        let owner = FOwnerEx {
            kind: F_OWNER_TID,
            pid: expected_tid,
        };
        if unsafe { libc::fcntl(leased.fd(), F_SETOWN_EX, &owner) } == -1 {
            return Err(io::Error::last_os_error());
        }

        leased.record(PayloadOperation::GetOwnerThread);
        #[cfg(any(test, feature = "test-read-audit"))]
        if leased.config.faults.get_owner_error {
            return Err(io::Error::other("injected F_GETOWN_EX failure"));
        }
        let mut actual = FOwnerEx { kind: 0, pid: 0 };
        if unsafe { libc::fcntl(leased.fd(), F_GETOWN_EX, &mut actual) } == -1 {
            return Err(io::Error::last_os_error());
        }
        #[cfg(any(test, feature = "test-read-audit"))]
        if leased.config.faults.owner_mismatch {
            actual.pid = actual.pid.saturating_add(1);
        }
        if actual.kind != F_OWNER_TID || actual.pid != expected_tid {
            return Err(io::Error::other("lease signal owner mismatch"));
        }

        leased.record(PayloadOperation::QueryLeaseAfterOwner);
        #[cfg(any(test, feature = "test-read-audit"))]
        if leased.config.faults.post_owner_query_error {
            return Err(io::Error::other("injected post-owner F_GETLEASE failure"));
        }
        let lease = unsafe { libc::fcntl(leased.fd(), libc::F_GETLEASE) };
        if lease == -1 {
            return Err(io::Error::last_os_error());
        }
        #[cfg(any(test, feature = "test-read-audit"))]
        let lease = if leased.config.faults.post_owner_lease_lost {
            libc::F_UNLCK
        } else {
            lease
        };
        if lease != libc::F_RDLCK {
            return Err(io::Error::other("read lease lost during owner routing"));
        }
        Ok(leased)
    }

    pub(crate) fn size(&mut self) -> io::Result<u64> {
        self.record(PayloadOperation::Fstat);
        let mut stat = MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(self.fd(), stat.as_mut_ptr()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG || stat.st_size < 0 {
            return Err(io::Error::other("leased payload is not a regular file"));
        }
        Ok(stat.st_size as u64)
    }

    pub(crate) fn verify_zero_offset(&mut self) -> io::Result<i64> {
        self.record(PayloadOperation::QueryOffsetBeforeSpawn);
        let offset = unsafe { libc::lseek(self.fd(), 0, libc::SEEK_CUR) };
        if offset == -1 {
            return Err(io::Error::last_os_error());
        }
        if offset != 0 {
            return Err(io::Error::other(
                "payload offset changed before child spawn",
            ));
        }
        Ok(offset)
    }

    pub(crate) fn child_stdin(&mut self) -> io::Result<std::process::Stdio> {
        self.record(PayloadOperation::DuplicateChildStdin);
        let duplicate = unsafe { libc::fcntl(self.fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate == -1 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(duplicate) };
        Ok(std::process::Stdio::from(file))
    }

    pub(crate) fn drain_before_spawn(
        &mut self,
        signals: &mut UploadSignals,
    ) -> io::Result<PendingUploadSignals> {
        self.record(PayloadOperation::DrainSignalsBeforeSpawn);
        signals.drain()
    }

    pub(crate) fn break_pending(
        &mut self,
        signals: &mut UploadSignals,
    ) -> io::Result<PendingUploadSignals> {
        self.record(PayloadOperation::PollLeaseBreak);
        signals.drain()
    }

    pub(crate) fn final_check(
        &mut self,
        signals: &mut UploadSignals,
    ) -> io::Result<PendingUploadSignals> {
        self.record(PayloadOperation::DrainLeaseBreak);
        let pending = signals.drain()?;
        if pending.interrupt.is_some() || pending.lease_break {
            return Ok(pending);
        }
        self.record(PayloadOperation::FinalQueryLease);
        #[cfg(any(test, feature = "test-read-audit"))]
        if self.config.faults.final_lease_lost {
            return Err(io::Error::other("injected final lease loss"));
        }
        if unsafe { libc::fcntl(self.fd(), libc::F_GETLEASE) } != libc::F_RDLCK {
            return Err(io::Error::other("payload read lease lost"));
        }
        Ok(pending)
    }

    pub(crate) fn cleanup_deadline_exhausted(&self) -> bool {
        #[cfg(any(test, feature = "test-read-audit"))]
        {
            self.config.faults.cleanup_deadline_exhausted
        }
        #[cfg(not(any(test, feature = "test-read-audit")))]
        {
            false
        }
    }

    pub(crate) fn operations(&self) -> &[PayloadOperation] {
        &self.operations
    }

    pub(crate) fn release(&mut self) -> io::Result<()> {
        if self.lease_active {
            self.record(PayloadOperation::ReleaseLease);
            if unsafe { libc::fcntl(self.fd(), libc::F_SETLEASE, libc::F_UNLCK) } == -1 {
                return Err(io::Error::last_os_error());
            }
            self.lease_active = false;
        }
        Ok(())
    }

    fn fd(&self) -> libc::c_int {
        self.payload
            .as_ref()
            .expect("payload exists after open")
            .as_raw_fd()
    }

    fn record(&mut self, operation: PayloadOperation) {
        self.operations.push(operation);
    }

    fn validate_lease_break_time(&self) -> io::Result<()> {
        #[cfg(any(test, feature = "test-read-audit"))]
        let value = match self.config.faults.lease_break_time {
            LeaseBreakTimeTest::System => std::fs::read_to_string("/proc/sys/fs/lease-break-time")?,
            LeaseBreakTimeTest::Unavailable => {
                return Err(io::Error::other("injected unavailable lease-break-time"));
            }
            LeaseBreakTimeTest::Malformed => "not-a-number".to_owned(),
            LeaseBreakTimeTest::Seconds(seconds) => seconds.to_string(),
        };
        #[cfg(not(any(test, feature = "test-read-audit")))]
        let value = std::fs::read_to_string("/proc/sys/fs/lease-break-time")?;
        let seconds = value
            .trim()
            .parse::<u64>()
            .map_err(|_| io::Error::other("invalid lease-break-time"))?;
        if seconds < 10 {
            return Err(io::Error::other("lease-break-time is below ten seconds"));
        }
        Ok(())
    }
}

impl Drop for LeasedPayload {
    fn drop(&mut self) {
        let _ = self.release();
        if self.payload.is_some() {
            self.record(PayloadOperation::ClosePayload);
            drop(self.payload.take());
        }
    }
}

fn block_upload_signals() -> io::Result<(libc::sigset_t, libc::sigset_t)> {
    let mut set = MaybeUninit::<libc::sigset_t>::uninit();
    let mut old = MaybeUninit::<libc::sigset_t>::uninit();
    if unsafe { libc::sigemptyset(set.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut set = unsafe { set.assume_init() };
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGIO] {
        if unsafe { libc::sigaddset(&mut set, signal) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    let result = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, old.as_mut_ptr()) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    Ok((set, unsafe { old.assume_init() }))
}

fn restore_signal_mask(old: &libc::sigset_t) {
    let _ = unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, old, std::ptr::null_mut()) };
}

fn create_signal_fd(set: &libc::sigset_t) -> io::Result<OwnedFd> {
    let fd = unsafe { libc::signalfd(-1, set, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK) };
    if fd == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn open_payload(root: &File, name: &str) -> io::Result<OwnedFd> {
    let name = OsStr::new(name);
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(io::Error::other("invalid payload member"));
    }
    let name = CString::new(bytes).map_err(|_| io::Error::other("NUL in payload path"))?;
    let how = OpenHow {
        flags: (libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC) as u64,
        mode: 0,
        resolve: RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS | RESOLVE_BENEATH,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            root.as_raw_fd(),
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        ) as libc::c_int
    };
    if fd == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn drain_signal_fd(fd: libc::c_int) -> io::Result<PendingUploadSignals> {
    let mut pending = PendingUploadSignals::default();
    loop {
        let mut info = MaybeUninit::<libc::signalfd_siginfo>::uninit();
        let read = unsafe {
            libc::read(
                fd,
                info.as_mut_ptr().cast(),
                std::mem::size_of::<libc::signalfd_siginfo>(),
            )
        };
        if read == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(pending);
            }
            return Err(error);
        }
        if read == 0 {
            return Ok(pending);
        }
        if read as usize != std::mem::size_of::<libc::signalfd_siginfo>() {
            return Err(io::Error::other("short signalfd record"));
        }
        let info = unsafe { info.assume_init() };
        match info.ssi_signo as libc::c_int {
            libc::SIGINT => pending.interrupt = Some(libc::SIGINT),
            libc::SIGTERM => pending.interrupt = Some(libc::SIGTERM),
            libc::SIGIO => pending.lease_break = true,
            _ => return Err(io::Error::other("unexpected signalfd signal")),
        }
    }
}

fn gettid() -> libc::pid_t {
    unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t }
}
