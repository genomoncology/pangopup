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
};

const SAID: &str = "the-service-said-boom";

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
    let mut clean = child("exit 0");
    assert!(
        report_for(&mut clean, "clean exit").is_none(),
        "a service that exited successfully must not be reported as a failed shutdown"
    );
}

#[test]
fn a_non_zero_exit_is_reported_with_its_status_and_standard_error() {
    let mut failed = child(&format!("echo {SAID} >&2; exit 3"));
    let report = report_for(&mut failed, "service exit")
        .expect("a service that exited 3 must fail the shutdown assertion");
    assert!(
        report.contains("service exit"),
        "the report does not say which shutdown failed: {report}"
    );
    assert!(
        report.contains('3'),
        "the report does not carry the exit status: {report}"
    );
    assert!(
        report.contains(SAID),
        "the report does not carry what the service wrote to standard error: {report}"
    );
}

#[test]
fn a_signalled_exit_is_reported_with_its_signal_and_standard_error() {
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
        report.contains('9'),
        "the report does not name the signal that ended the service: {report}"
    );
    assert!(
        report.contains(SAID),
        "the report does not carry what the service wrote to standard error: {report}"
    );
}
