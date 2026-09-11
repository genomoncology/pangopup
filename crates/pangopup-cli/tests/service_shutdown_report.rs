#![cfg(unix)]

//! A shutdown assertion that fails has to say what happened.
//!
//! `status_under_threads` asserted `child.wait().expect("service exit")
//! .success()` and printed nothing else. That assertion failed once, inside a
//! `make spec` run, and has not reproduced. Two questions sat inside the one
//! failure and the assertion could not separate them: whether the service can
//! exit non-zero on a `SIGTERM` it accepted, and whether a reader can tell what
//! happened. Nothing here changes shutdown. It makes the second question
//! answerable, so the next occurrence arrives with the exit status, the signal
//! if there was one, and what the service wrote to its standard error.
//!
//! The report is proved against children this module starts and ends on
//! purpose, because a shutdown failure cannot be forced out of the real
//! service. A child that exits 3 and a child killed with `SIGKILL` are real
//! exit statuses of the same kind the service would produce.

mod support;

use std::{
    io::{BufRead, BufReader},
    panic,
    process::{Child, Command, Stdio},
    sync::{Mutex, MutexGuard, OnceLock},
};

const SAID: &str = "the-service-said-boom";

/// Held for the whole of a test that replaces the panic hook.
///
/// `panic::set_hook` is process-wide and the test harness runs this file's
/// tests in parallel threads, so a silenced hook silences every test running
/// beside it. Measured: a test that panics while another test holds a no-op
/// hook is reported as failed with no message, no assertion text and no
/// location -- which is the illegibility this file exists to refuse.
///
/// Every test here takes the seat on its first line rather than only around
/// the hook, because a test's own assertion fires after its hook is restored
/// and would otherwise land inside another test's silent window.
fn hook_seat() -> MutexGuard<'static, ()> {
    static SEAT: OnceLock<Mutex<()>> = OnceLock::new();
    match SEAT.get_or_init(|| Mutex::new(())).lock() {
        Ok(seat) => seat,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Whether `report` names `number` as a number of its own rather than as a
/// digit inside a larger one.
///
/// `contains('9')` would be satisfied by a process id, so it would pass on a
/// report that never named the signal at all.
fn names_number(report: &str, number: i32) -> bool {
    let needle = number.to_string();
    report.match_indices(&needle).any(|(at, _)| {
        let before = report[..at].chars().next_back();
        let after = report[at + needle.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// A child with its standard error piped, so the report has something to
/// read. `script` runs under `sh`.
fn child(script: &str) -> Child {
    Command::new("/bin/sh")
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn a child to end")
}

/// The message `assert_shutdown_succeeded` panics with, or `None` when it
/// did not panic. The panic hook is silenced across the call so a proof
/// that a failure is legible does not print a backtrace of its own.
fn report_for(child: &mut Child, what: &str) -> Option<String> {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let outcome = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        support::assert_shutdown_succeeded(child, what);
    }));
    panic::set_hook(previous);
    match outcome {
        Ok(()) => None,
        Err(payload) => Some(
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|text| (*text).to_owned())
                })
                .expect("a panic message"),
        ),
    }
}

#[test]
fn a_clean_shutdown_asserts_nothing() {
    let _seat = hook_seat();
    let mut clean = child("exit 0");
    assert!(
        report_for(&mut clean, "clean exit").is_none(),
        "a service that exited successfully must not be reported as a failed shutdown"
    );
}

#[test]
fn a_non_zero_exit_is_reported_with_its_status_and_standard_error() {
    let _seat = hook_seat();
    let mut failed = child(&format!("echo {SAID} >&2; exit 3"));
    let report = report_for(&mut failed, "service exit")
        .expect("a service that exited 3 must fail the shutdown assertion");
    assert!(
        report.contains("service exit"),
        "the report does not say which shutdown failed: {report}"
    );
    assert!(
        names_number(&report, 3),
        "the report does not carry the exit status: {report}"
    );
    assert!(
        report.contains(SAID),
        "the report does not carry what the service wrote to standard error: {report}"
    );
}

/// A service that fails quietly is the common case. `pangopup serve` writes
/// its listening line to standard output and writes to standard error only
/// when it has something to report, so most shutdown failures carry nothing
/// there. A report that ended on a colon would leave a reader deciding whether
/// the service was silent or the capture had failed.
#[test]
fn a_silent_service_is_reported_as_having_written_nothing() {
    let _seat = hook_seat();
    let mut quiet = child("exit 3");
    let report = report_for(&mut quiet, "service exit")
        .expect("a service that exited 3 must fail the shutdown assertion");
    assert!(
        report.contains("wrote nothing to its standard error"),
        "the report does not say the service was silent: {report}"
    );
    assert!(
        !report.trim_end().ends_with(':'),
        "the report ends on a colon with nothing after it: {report}"
    );
    assert!(
        names_number(&report, 3),
        "the report does not carry the exit status: {report}"
    );
}

#[test]
fn a_signalled_exit_is_reported_with_its_signal_and_standard_error() {
    let _seat = hook_seat();
    let mut killed = child(&format!("echo {SAID} >&2; echo said; sleep 30"));
    // Wait until the child has written, so the signal cannot arrive before
    // there is any standard error for the report to carry.
    let mut ready = String::new();
    BufReader::new(killed.stdout.as_mut().expect("stdout"))
        .read_line(&mut ready)
        .expect("the fixture child reports that it has written");
    assert_eq!(
        ready.trim(),
        "said",
        "the fixture child did not run, so this test proves nothing"
    );
    assert_eq!(
        unsafe { libc::kill(killed.id() as i32, libc::SIGKILL) },
        0,
        "the fixture child could not be signalled, so this test proves nothing"
    );
    let report = report_for(&mut killed, "service exit")
        .expect("a service killed by a signal must fail the shutdown assertion");
    assert!(
        report.contains("signal"),
        "the report does not say the service died of a signal: {report}"
    );
    assert!(
        names_number(&report, libc::SIGKILL),
        "the report does not name the signal that ended the service: {report}"
    );
    assert!(
        report.contains(SAID),
        "the report does not carry what the service wrote to standard error: {report}"
    );
}
