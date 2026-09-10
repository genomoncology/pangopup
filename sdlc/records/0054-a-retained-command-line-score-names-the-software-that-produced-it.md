---
base: e893e2f84efd72179c2a7faf848a7f15ecfe618f
head: 9f728ab9d5e9e12c97c803b3a868f2da8bc6c850
---
# A retained command-line score names the software that produced it

Every `pangopup lookup` result line now carries `provenance.software_version`,
the plain version the same build reports through `--version`. It is the last key
in `provenance`, beside the asset identifiers that already say where the number
came from. It is not a digest, it names no runtime profile, and it never stands
in for `data_set_version`.

The version reaches the printed line through one seam. `render_requests` takes
the version as an argument; `main.rs` passes `env!("CARGO_PKG_VERSION")` at the
single call the command-line tool renders through, and `render_result_raw`, the
call the HTTP surface reaches the same renderer by, passes nothing. Ticket 0052
settled what an HTTP score item carries and this ticket left it alone. The table
form gained no column.

`architecture/compatibility.md` carries the addition in its response-shape
inventory and now states what a retained command-line score pins. The sentence
ticket 0052 pinned calling this an open question is gone, and the gate in
`spec/http-service.md` that read it moved with it.

The release checker holds the field to the same standard as the other published
fields. It refuses a release that stamps no line, stamps only some lines, stamps
a malformed value, or stamps a version this release is not, and it takes the
field back out before comparing scoring bytes so a field printed on either side
of it still fails.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME` and
`TMPDIR`. All four invocation shapes were run against repository fixtures --
installed data directory, explicit bundle, explicit model assets, and
`--model-only` -- on both routes and in both output forms. Every JSONL line
carried `"software_version":"0.5.0"` as the last key of `provenance`; both table
runs printed the same fifteen columns the base commit prints, byte for byte. A
mixed run whose precomputed and modeled lines went to a file was read back and
both lines still named the version, still last. `pangopup --version` from the
same build reports `pangopup 0.5.0`. A real `pangopup serve` was driven over
HTTP: a precomputed score item, a modeled score item and a rejected item carried
no `software_version` at all, absent rather than null, while `/v1/status`
reported `"version":"0.5.0"` -- the same string the command line stamps, from a
field that already existed. An invalid variant still prints an error object with
no provenance and nothing to stamp.

Two smaller things are accepted rather than filed. `scoring_bytes` in the
release checker keeps a `stamped == 0` form rather than a per-line count;
both model oracles are single-line files, so the two forms are equivalent there.
The new `spec/cli.md` block runs on Linux only, since `cli.md` sits in
`SPEC_LINUX_ONLY`; the inventory bullet is gated on every platform by
`check-version-consistency.py`, and only the two prose statements are
Linux-gated.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
`tests/version-consistency-python39.sh`,
`tests/production-release-qualification.sh` and `sdlc/scripts/lint` all pass on
this candidate. `make test` finished in 77 seconds with 55 suites and no
failure. The operator's model-results cache was untouched: same md5, size and
mtime before and after.
