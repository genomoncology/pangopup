---
base: cc6fb3fd8805008ab8902512e1377627662d4311
head: 998e9eec6053cbd69a3b7b96c16adf3b591d3cf6
---
# The release checker accepts one value published under both identity names

`scripts/check-production-qualification.py` is the last reader before a
published artifact and the only one that judges a real deployment rather than a
fixture. It read two of the three digests a status response publishes and
compared neither with the other. It now holds `runtime_profile_id` to the same
digest pattern as `scoring_identity` and `data_set_version`, and refuses a
status response that publishes one digest under any two of the three names,
naming the two that collided. `tests/production-release-qualification.sh` gains
a stub that publishes three distinct digests, a claim over the qualified output
that they stay distinct and well formed, and five mutations that drive each new
refusal. Nothing under `crates/` changed, no response shape changed, and no
field was added or renamed.

Exercised beyond the suite under a private `HOME`, `XDG_DATA_HOME` and
`XDG_CACHE_HOME`, with `CARGO_HOME` and `RUSTUP_HOME` pinned to the real ones.

The headline was run as a release engineer would: one deployment built from the
qualified output, every response file internally consistent, judged by the base
checker at `cc6fb3fd8805008ab8902512e1377627662d4311` and by this candidate side by
side. Five deployments, five identical outcomes on the base and five refusals on
the candidate.

- one digest under `scoring_identity` and `data_set_version`, with all three
  scored items rewritten to agree: base exit 0, candidate exit 1,
  `HTTP status published one digest under both scoring_identity and data_set_version`
- one digest under `scoring_identity` and `runtime_profile_id`: base exit 0,
  candidate exit 1,
  `HTTP status published one digest under both scoring_identity and runtime_profile_id`
- one digest under `data_set_version` and `runtime_profile_id`: base exit 0,
  candidate exit 1,
  `HTTP status published one digest under both data_set_version and runtime_profile_id`
- `runtime_profile_id` deleted: base exit 0, candidate exit 1,
  `HTTP status runtime profile id is invalid`
- `runtime_profile_id` published as `sha256:` and 64 `Z` characters: base exit
  0, candidate exit 1, `HTTP status runtime profile id is invalid`

A correct release still ships. The whole of
`tests/production-release-qualification.sh` passes end to end and prints
`production release qualification tests passed`. The same checker run directly
against the pristine three-distinct-digest deployment, outside the harness,
exits 0 on the base and on the candidate. The candidate gains refusals, not a
reason to reject a correct release.

The real product agrees, which is the one thing no fixture can say. A real
`pangopup serve` was started against a runtime installed from repository
fixtures and `GET /v1/status` was read off it. It published
`scoring_identity` `sha256:76845e29fa487e1fc6347bc522a7ec3b52be1c45582557636f0eda4c4211dbe4`,
`data_set_version` `sha256:017e2cce67438150831152b3e18715528a4cc455fb9e9cce10d41495d97505bb`
and `runtime_profile_id` `sha256:924267d6e88ac78be1b7916e458f58a7d3b9bb23cc778712b96254ec920a99d4`:
three distinct, well-formed digests. Those three values were written into the
qualified deployment in place of the stub's, items included, and the candidate
checker exited 0 on them. The service was stopped and `pgrep -x pangopup` and
`pgrep -x pangopup-build` are both empty.

A maintainer is not obstructed. Every deployment directory the harness leaves
behind, fifty of them, was judged by the base checker and by the candidate. On
forty-five the exit code and the stderr text are byte-identical. The five that
differ are the five new refusals, and the base accepted all five with exit 0 and
no output. No deployment that the base refused refuses with a different message
on the candidate. The existing mutation list is unchanged and passes.

Four costs are accepted rather than filed.

`scripts/check-version-consistency.py` changed although the ticket names two
files. It is the mechanical consequence of the stub publishing a third status
value: that file pins the stub's status claim literally, so without the change
`make lint` fails on work the ticket requires. The pattern now ends `[,}]`, so
the field list stays open while the value token still closes. Proved here: a
stale version string in that line and a `data_set_version` binding renamed to
`data_set_version_old` are each still refused with
`version consistency: candidate: tests/production-release-qualification.sh: expected candidate qualification server status`,
and the unmodified file passes.

The new harness block carries `assert len(values) == len(names) == 3` over a
literal three-tuple, which cannot fail today. The gate that does the work beside
it is `assert len(set(values)) == 3`. Judged a tripwire against someone
shortening the name list, not worth the churn of removing.

Distinctness of the three digests is structural, not a hope. Each is a SHA-256
over a byte string carrying a different schema constant:
`pangopup.runtime-profile.v1` in the canonical profile bytes hashed by
`runtime_profile_id` (`crates/pangopup-assets/src/runtime_profile.rs:18` and
`:228`), `pangopup.active-scoring-identity.v1` and
`pangopup.scoring-data-set-version.v1` in the two preimage structs
(`crates/pangopup-assets/src/active_identity.rs:10` and `:46`). Two of them can
be equal only through a SHA-256 collision, so the new refusal cannot block a
correct release.

One refusal message moved. A deployment that collapses `data_set_version` onto
`scoring_identity` in the status response alone, leaving the scored items
carrying the old version, said `HTTP SNV data-set version mismatch` on the base
and says
`HTTP status published one digest under both scoring_identity and data_set_version`
on the candidate. That deployment is collapsed, which is this ticket's subject,
and it still refuses. The message now names the collapse rather than its first
downstream symptom.

`make lint`, `make test`, `make spec`,
`scripts/run-service-fixture-tests.sh`, all twenty `PORTABLE_QUALIFICATION`
gates run one at a time, all three `SHELL_QUALIFICATION` gates run one at a
time, and `bash sdlc/scripts/lint` pass on this candidate. `make test` took
109.03 seconds against an about 104-second baseline. `make spec` reports
`306 passed, 6 skipped`. `scripts/run-service-fixture-tests.sh` reports
`20 tests matched installed_success in tests/http_service_lifecycle.rs`.
Draft 0098 came in with the candidate; verify filed none.
