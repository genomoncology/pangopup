---
flow: build
priority: 6
---
# The repository cannot state a version it does not ship

The released version is written out by hand in more than a dozen places and
nothing checks that the copies agree. A release bump is a search rather than an
edit. A missed copy is found by a failing release rather than by a gate.

The workspace declares `version = "0.3.0"` in `Cargo.toml`. The executable
reports it: `pangopup --version` prints `pangopup 0.3.0`. Everything else that
states that version is a separate hand-written string. These files each carry at
least one, and each would have to be found and edited for a bump to be complete:

```
README.md
CITATION.cff
crates/pangopup-cli/tests/citation.rs
crates/pangopup-cli/src/service_tests.rs
spec/cli.md
spec/container-image.md
spec/readme-first-use.md
architecture/delivery.md
architecture/service.md
scripts/check-production-qualification.py
tests/production-release-qualification.sh
tests/executable-delivery.sh
.github/workflows/publish-container.yml
```

Two of them decide whether a release succeeds.
`scripts/check-production-qualification.py` compares the version the running
image reports against the literal string `0.3.0`. It accepts no version input.
`scripts/qualify-linux-release.sh` sits beside it in the same procedure and
takes the version as its second argument. Two scripts therefore disagree about
whether the expected version is an input or a constant.
`.github/workflows/publish-container.yml` sets `VERSION: 0.3.0` as a workflow
environment value. A forgotten edit there would publish new software under the
old tag.

One copy already disagrees with the repository. `architecture/service.md` says
the public index "currently identifies application v0.2.0; the repository
prepares v0.3.0 as one coherent executable/container candidate".
`architecture/delivery.md` says the current public set is `0.3.0`, `v0.3.0`, and
`latest`, against a named index digest. Both cannot be true. The `service.md`
sentence arrived with commit `738d38f`, which prepared the v0.3.0 candidate.
Nobody updated it when the release published, because no check reads it.

The behavior this ticket requires is a gate. Every statement about the version
this repository ships now either agrees with the workspace version or fails a
gate that names the file and the disagreement. Deriving each copy is not
required. Some copies read better as literals. What must not happen is a literal
that disagrees in silence.

Four choices this ticket settles, because the agent cannot make them alone.

The version stays `0.3.0`. This ticket adds the gate and repairs the statement
that is wrong today. It does not bump anything. A repository claiming `0.4.0`
while no such release exists would tell a reader to download a tag that is not
there. The bump belongs to the release procedure, beside the release notes and
the publication record that the same procedure authors.

Records of earlier releases are append-only and are never rewritten to satisfy
the gate. `planning/artifacts/054-release-notes.md`,
`planning/artifacts/055-public-v0.3.0.md`,
`planning/artifacts/056-independent-public-v0.3.0.md`, the release artifacts
beside them, and the assertions in `tests/executable-delivery.sh` that read
those artifacts back all name `0.3.0` and its predecessors as history. Those
statements are correct and must keep saying what they say.

Two version strings are neither a current claim nor history, and the gate must
leave them alone. `crates/pangopup-assets/src/active_identity.rs` passes `0.3.0`
as an explicit test input beside a pinned canonical preimage and a pinned
digest. Forcing that string to follow the workspace version would make the
pinned digest wrong. `tests/container-tag-absence.sh` passes `0.3.0` as a sample
argument while unit-testing a helper, and any version would serve.

The gate belongs where the existing gates run. It is not a new command and not a
step an operator must remember.

Done, observably:

- Changing the workspace version and running the repository gates either brings
  every current claim with it or fails, naming each file that still states the
  old version.
- An image reporting a version other than the one this repository declares fails
  production qualification.
- No document in the repository claims a published state that contradicts
  another document.
- The gate reads records of earlier releases, and the two deliberate test
  strings above, without demanding that any of them change.
- With the version left at `0.3.0`, `make lint`, `make test`, and `make spec`
  stay green and the executable specification count does not fall.
- A test proves the gate by making one current claim disagree and observing the
  named failure. That test fails before the change.

Boundary: do not change the version. Do not change what the CLI, the service, or
any released artifact reports. Do not change how the publish workflow builds,
qualifies, stages, or tags anything; the gate reads its version value and
nothing else about it. Do not change publication evidence or any file under
`planning/artifacts/`. Do not change the active scoring identity, which takes the
software version as an input and is expected to change when the version changes.
Do not add a new configuration option, command, or release step.
