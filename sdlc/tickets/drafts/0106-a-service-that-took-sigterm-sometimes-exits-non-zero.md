---
---
# A service that took SIGTERM sometimes exits non-zero

`status_under_threads` in `crates/pangopup-cli/tests/http_service_lifecycle.rs`
starts a real `pangopup serve`, reads `/v1/status`, sends `SIGTERM`, and
asserts the process exited successfully:

    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    assert!(child.wait().expect("service exit").success());

That last assertion failed once on 2026-09-11, inside
`installed_success::pinned_values_hold_across_cpu_policies_and_move_with_the_scored_inputs`,
run from the `## What a consumer pins` block of `spec/http-service.md`.
`make spec` reported `313 passed, 1 failed` and exited 2. The panic is at
`http_service_lifecycle.rs:487` and names nothing but the assertion, so the
exit status the service actually returned was not recorded and nothing in the
failure says whether the process was still starting, still draining, or had
already stopped.

It does not reproduce. Measured on the same working tree, same binaries, same
private cache root:

- `make spec` run four times: one failure, three runs of `314 passed`.
- The named test run on its own 15 times: 15 passes.
- The named test run 12 more times with every core held busy by a spin loop:
  12 passes.
- `scripts/run-service-fixture-tests.sh`: `20 tests matched installed_success`,
  all passing.

Nothing about it belongs to the change it was found under. The failing test and
the service it starts carry the same git blobs on `origin/main` as on the
candidate: `crates/pangopup-cli/tests/http_service_lifecycle.rs` is
`10d24ad3e94057f1e7610044a2ecbc2b9c2fd7e7` on both, and
`crates/pangopup-cli/src/service.rs` is
`3bcc686a2beaf6b12a50369a2fbf1b67fe1bf8dc` on both.

Two things are worth separating. One is whether `pangopup serve` can exit
non-zero on a `SIGTERM` it accepted, which would be a product defect in
shutdown. The other is that this assertion cannot tell a reader which happened:
it prints no exit status, no signal, and none of the service's own standard
error, so a rare failure in a gate arrives with nothing to act on. The second
is fixable on its own and makes the first diagnosable the next time it happens.

Found while verifying ticket 0084.
