# 038 — Publish and qualify the immutable Linux executable

Status: ready

## Why

Ticket 037 prepared a checksum-verifying installer and a read-only GitHub
workflow that builds and qualifies one Linux x86_64 executable from an exact
commit. Users still cannot use that installer because no `v0.1.0` executable
release exists. The next bounded outcome is to publish the already-defined
six-file executable release once, prove that an isolated Linux user can install
it, synchronize the existing immutable data releases, and obtain both an SNV
lookup result and a model-fallback result.

This is an exceptional public-effect ticket. Preparation and review are normal
repository work. Only the coordinator may dispatch the packaging workflow,
create/upload/publish the GitHub release, or change public release state, and
only after the publication-ready commit's exact remote `ci`/`gate` is green.

## Scope

- Publish one immutable GitHub release in `genomoncology/pangopup`:
  - tag: `v0.1.0`;
  - title: `Pangopup v0.1.0`;
  - target: the exact publication-ready commit produced by this ticket;
  - body: one reviewed, checked-in `planning/artifacts/038-release-notes.md`.
- Use the existing manual `package-linux` workflow as the only binary build.
  The first dispatch, run `30648307402` for preparation commit `a82968c`,
  stopped safely during linking before producing an artifact or changing any
  release state. After this correction is independently reviewed, dispatch
  the corrected build from the new publication-ready commit. That attempt is
  now consumed and recorded below. The only remaining authorization is the
  independently reviewed smoke-corrected dispatch described next.
  Require its successful completion, download its private Actions artifact
  once, and run
  `scripts/qualify-linux-release.sh` again locally against that commit and
  workspace version before any release mutation.
  The first corrected-baseline dispatch, run `30649914623` for commit
  `dc9cf1c`, passed the full gate, build, SBOM, release preparation, and GLIBC
  qualification, then stopped in the clean-container smoke because its shell
  quoting removed the JSON quotation marks from the `grep` patterns. It
  produced no artifact and changed no release state. After the smoke command
  and its execution test are independently reviewed, exactly one
  smoke-corrected dispatch from a new publication-ready commit is permitted.
  That dispatch, run `30651619497` for commit `6f38173`, passed through release
  qualification and executed the shared smoke correctly, then stopped when the
  real CLI rejected `/tmp/model.sqlite3` because `/tmp` is not an owned private
  cache directory. It produced no artifact and changed no release state. After
  review, exactly one cache-environment-corrected dispatch is permitted.
  That dispatch, run `30652858960` for commit `8b271c0`, stopped in the package
  workflow's redundant `make test` after the already-green exact-commit CI gate
  because an unrelated allocation-count test observed 184 instead of 180.
  It produced no artifact and changed no release state.
- Publish exactly the six regular files admitted by the qualifier, without
  renaming or recompressing them:
  `pangopup-linux-x86_64`, `pangopup-linux-x86_64.sha256`,
  `pangopup-linux-x86_64.cdx.json`, `release-manifest.json`, `LICENSE`, and
  `NOTICE`.
- Before mutation, require:
  - public repository visibility;
  - immutable releases enabled;
  - Cargo/workspace version exactly `0.1.0`;
  - no existing `v0.1.0` release or tag;
  - exact publication-ready commit reachable from public `main`;
  - its completed successful `ci` workflow with exactly one successful job
    named `gate`;
  - the successful `package-linux` run bound to the same commit and input;
  - the locally downloaded exact-six artifact still passing the shared
    qualifier, with the manifest's target commit equal to that commit.
- Build and qualify the executable on pinned Ubuntu 24.04, and admit a final
  stripped executable only when its maximum imported GLIBC symbol version is
  no newer than 2.39. The manifest and user documentation must report the
  measured final-binary requirement. The supported platform is Linux
  x86_64/amd64 with GLIBC 2.39 or newer.
- Immediately before every independently authorized `package-linux` dispatch,
  including the remaining smoke-corrected dispatch, repeat and
  record the complete live publication-security audit required by
  `architecture/delivery.md`: read-only default Actions token, disabled Actions
  pull-request approval, enabled Dependabot security updates, enabled secret
  scanning/push protection/non-provider-pattern controls, no open secret
  alerts, and both active `main` rulesets with the reviewed deletion/
  non-fast-forward and pull-request/`gate` policies and only the approved
  administrator-role bypass. Any drift stops the ticket before dispatch.
  Record the fresh audit separately for each attempt; an earlier passing audit
  cannot authorize a later workflow run.
- Use only the authenticated official `gh` executable for GitHub mutation. Do
  not add product upload code, token handling, release retries, or a publishing
  workflow.
- The package workflow must not rerun `make lint`, `make test`, or `make spec`.
  The runbook already requires the exact commit's single green CI `gate`; the
  package workflow owns only exact checkout authentication, release build,
  SBOM, preparation, qualification, clean-container smoke, and private Actions
  artifact upload. Remove prerequisites used only by the duplicate gate and
  retain every prerequisite needed by packaging/qualification.
- Create a private draft first. Upload each of the six admitted files once,
  without replacement or `--clobber`, and compare the complete remote
  name/size/GitHub SHA-256 inventory after every upload. Stop on any mismatch.
- Immediately before publication, recheck the exact release ID, draft state,
  absent tag, target, title, byte-exact body, and closed six-asset inventory.
  Publish once, then require `draft=false`, `immutable=true`, and an exact
  public tag ref resolving directly to the publication-ready commit. Explicitly
  mark this executable release as GitHub's `Latest` release and require the
  `/releases/latest` endpoint to resolve to `v0.1.0`; the existing data-release
  tags remain independently pinned and are never selected through `latest`.
- Before publication only, a failed draft may be deleted by its exact release
  ID after reauthentication if and only if the tag remains absent. Never
  delete or mutate a tag. Stop after cleanup; do not retry in the same
  operation. After publication, never replace, delete, or modify the release.
- After publication, perform bounded unauthenticated verification of release
  and tag metadata plus byte-exact downloads of the five small assets. Verify
  the executable through GitHub's reported size/digest rather than downloading
  it a second time.
- Before making the draft public, run the downloaded, locally requalified
  executable in a pinned Linux x86_64 container with isolated HOME/XDG data and
  cache volumes. It must perform online `pangopup sync`, offline
  `pangopup sync --offline` reuse, combined ready `pangopup status`, and the
  two exact semantic oracles below. This is the production-data compatibility
  gate while rollback is still possible; it must pass before publication.
- Pin the SNV oracle to the exact publication-ready tag source files
  `tests/fixtures/snv-regression/requests.tsv` and
  `tests/fixtures/snv-regression/expected/*.jsonl`: run the same seven ordered
  CLI batches and compare byte-for-byte after removing only
  `provenance.bundle_id` from both actual production-index output and the
  existing independently generated expected JSON. No score, position, record,
  warning, order, source DOI/archive identity, mask/window value, or status may
  be removed from the comparison.
- Pin the model oracle to compatibility case `M09-insertion-short-plus`, input
  `GRCh38:chr12:6801303:G:GA`. Its complete JSONL output must match a new
  reviewed checked-in oracle byte-for-byte: one ordered
  `ENSG00000010610.10` record, gain `0.00` at `0`, loss `0.00` at `-50`, no
  warnings, and the production model/reference/mask identities already fixed
  in `planning/artifacts/020-lookup-first-cli-model-routing.md`. The oracle is
  derived from that retained evidence and the frozen compatibility case, not
  captured from the candidate release binary.
- After publication, use a second fresh pinned Linux container and isolated
  install directory to download `install.sh` from the exact public tag and
  install through the normal public checksum path. Reuse the already-qualified
  isolated XDG asset volumes, run offline sync, status, one retained SNV oracle
  request, and the exact M09 oracle again. This post-publication check proves
  the public installer/download boundary without downloading or reconstructing
  the multi-gigabyte data a second time.
- Preserve the host's existing Pangopup data. Neither container may rebuild,
  republish, or modify either immutable runtime-data release.
- Record a credential-free operation/evidence boundary in
  `planning/artifacts/038-public-linux-release.md`, including the exact commit,
  workflow/run IDs, artifact inventory/sizes/digests, release/tag IDs, upload
  attempt count, immutable state, bounded public checks, and clean-container
  result. It must contain no token, raw authentication output, or sensitive
  URL query parameters.
- Update `README.md`, `architecture/delivery.md`, `planning/frontier.md`, and
  `planning/faq.md` so installation prerequisites, supported platform,
  installer command, XDG asset behavior, first-run download, offline reuse,
  and current-versus-future service behavior are plain and accurate. The
  publication-ready commit must honestly say the executable is prepared but
  not yet public. After the external effect, the same developer applies the
  ticket's pre-reviewed current-state wording and records the exact public URL
  and evidence; the same code reviewer must accept that completion-only diff
  before the coordinator marks the ticket complete and pushes it.
- Exclude macOS/Windows/other architectures, package managers, Docker, HTTP,
  service lifecycle commands, shell-profile edits, root installation, runtime
  asset rebuild/publication, format changes, executable signing, and new model
  optimization. GitHub build-provenance attestations are also excluded from
  this first release: the release already carries a generated CycloneDX SBOM,
  exact manifest, checksum, immutable target, and remote digest proof; add
  attestations only as a separately reviewed hardening outcome if needed.

## Success Checklist

- The publication-ready diff contains a complete executable, credential-free
  runbook/evidence skeleton and honest prepublication documentation; it causes
  no GitHub mutation.
- `make lint`, `make test`, and `make spec` pass locally, and the exact
  publication-ready commit's remote `ci`/`gate` passes before mutation.
- The exact-commit `package-linux` run succeeds and its downloaded artifact
  passes the shared exact-six qualifier locally.
- The corrected workflow runs on Ubuntu 24.04, every clean-container check
  uses the reviewed pinned Ubuntu 24.04 image, and focused tests reject drift
  back to Ubuntu 22.04 or a GLIBC ceiling other than 2.39.
- The workflow smoke commands are executed by a focused test using the same
  shell nesting as Actions. A valid JSON status, SNV lookup, and model lookup
  must pass; a changed expected JSON value must fail. Text-presence checks
  alone do not satisfy this boundary.
- The shared smoke must create an owned mode-0700 cache parent under the
  container's temporary filesystem before model lookup, use the SQLite path
  beneath it, and be exercised with the real CLI/model fixture so an unsafe
  cache parent cannot pass through a fake-only test.
- Before publication, that artifact passes production-data online/offline
  sync, the exact seven-batch canonicalized 1,000-SNV oracle, exact M09 JSONL,
  and combined status in an isolated pinned Linux container.
- The public `v0.1.0` release targets the reviewed commit, contains exactly six
  reviewed assets with matching sizes/digests, and reports `immutable=true`.
- The tag resolves directly to the reviewed commit. Bounded unauthenticated
  release/tag/small-asset verification passes, and GitHub `Latest` resolves to
  `v0.1.0`.
- A second fresh pinned Linux container installs through the public installer,
  reuses the qualified XDG assets offline, repeats one SNV plus exact M09, and
  reports ready status.
- No runtime data is rebuilt or republished, no release asset is replaced, and
  the host's existing installed data remains untouched.

## Decisions

1. **Release build source.** A local binary would be convenient but would not
   prove reproducibility on the supported baseline. Use the reviewed
   exact-commit GitHub packaging workflow, then independently requalify the
   downloaded private artifact before publication.
2. **Draft-first or direct publication.** Direct publication exposes partial
   state. Use one private draft, verify its complete inventory while correction
   is still possible, and publish once into mandatory immutable state.
3. **End-to-end breadth.** A version/help smoke alone would not prove the user
   path. Before irreversible publication, run the candidate through first
   online sync, offline reuse, the exact retained 1,000-SNV corpus, exact M09,
   and combined status. After publication, separately prove the public
   installer and repeat bounded inference using those qualified asset volumes.
4. **Attestation now or later.** GitHub attestations would require a new
   write-capable workflow/security boundary. Keep this release bounded to the
   reviewed SBOM, checksum, manifest, exact commit, immutable release, and
   GitHub digest proof; evaluate attestation independently after users have a
   working install path.
5. **Re-download the executable or trust the remote digest.** The uploaded
   executable is already locally authenticated before upload and GitHub reports
   its SHA-256. Avoid a redundant large download; byte-check all five small
   assets and bind the executable through the remote digest and clean installer
   execution.
6. **Linux compatibility after the stopped build.** The selected Pyke static
   ONNX Runtime 1.24.2 archive itself imports GLIBC 2.38 C23 conversion symbols,
   so it cannot link on Ubuntu 22.04/GLIBC 2.35. Use Ubuntu 24.04/GLIBC 2.39 for
   the first release. This keeps the already qualified static model runtime,
   direct single-executable installer, six-file release, and model behavior.
   A dynamic ONNX Runtime could support an older baseline, but would add a
   shared-library asset and loader/install contract; a custom static build
   would add a C/C++ build and new provenance and qualification. Either is a
   separate measured compatibility outcome, not a repair to this publication.

## Dependencies

- Ticket 037, complete at implementation commit
  `163346ef0ea15bc269199698894ff445354f9a2f`.
- Immutable public `snv-grch38-v1` and `runtime-grch38-v1` data releases.

## Notes

- `install.sh` is source fetched from the exact tag, not a seventh release
  asset. It downloads the executable and checksum from the release.
- The GitHub Actions artifact is temporary transport between the reviewed
  build and the coordinator; it is not a public product asset.
- The clean-container proof may download the existing public runtime assets,
  but must use a temporary container/XDG mount and leave retained host data
  untouched.
- Record the exact container digest, the tagged oracle-file identities, the
  canonicalization operation, the checked M09 oracle SHA-256, and both output
  SHA-256 values in the durable evidence.
- Never print authentication headers, environment variables, raw `gh auth`
  output, or credential-bearing URLs.
- Preserve the failed package workflow run `30648307402` as evidence. It is
  the one stopped original-baseline attempt and does not authorize repeated
  retries. Its former corrected-commit authorization was consumed by the next
  recorded attempt.
- Preserve corrected-baseline package run `30649914623` as evidence. It passed
  through release qualification and failed only in the smoke's incorrectly
  dequoted `grep` pattern; artifact upload was skipped and no release state
  exists. The prior corrected authorization is consumed. This revision permits
  exactly one smoke-corrected dispatch after review.
- Preserve smoke-corrected package run `30651619497` as evidence. It passed the
  shared-script status and SNV checks, then the real model lookup rejected the
  unsafe `/tmp` cache parent. Artifact upload was skipped and no release state
  exists. That authorization is consumed; exactly one independently reviewed
  cache-environment-corrected dispatch remains.
- Preserve package run `30652858960` as evidence. The exact commit CI was green,
  but the workflow redundantly reran the full gate and an unrelated allocation
  test flaked before the release build. No artifact or release state exists.
  That authorization is consumed. After review, exactly one package-only
  dispatch remains.

## Coordinator Authorship

Coordinator: `/root`

Drafted from the shipped Ticket 037 installer, qualifier, and read-only
packaging workflow plus the two already-public immutable runtime-data releases.
At original authorship, no workflow was dispatched and no tag, release, or
asset was created. The later stopped attempt is recorded under External Effect
Evidence.

## Independent Ticket Review

Reviewer: `/root/ticket031_code_review`

First review: REJECT.

- The ticket omitted the mandatory live repository-security audit before its
  first public effect.
- Its production-data compatibility proof occurred only after immutable
  publication, when rollback was no longer possible.
- The 1,000-SNV and model checks lacked exact semantic oracles.
- The documentation requirements did not define an honest transition from the
  publication-ready state to the completed public state.

The coordinator accepted all four findings. The revised ticket requires the
complete delivery-architecture security audit before workflow dispatch; moves
online/offline sync and exact production-data qualification before publication;
pins the seven-batch SNV comparison to the existing independently generated
oracle with only bundle identity canonicalized; pins model output to an
independently derived exact M09 JSONL oracle; and requires the same developer
and code reviewer to complete and review the post-effect documentation
transition. It also makes `v0.1.0` explicitly become GitHub `Latest`, which is
necessary for the installer's default URL.

Final re-review of the original preparation: ACCEPT. The reviewer confirmed that the complete live security
audit precedes the first effect; exact production online/offline sync, ready
status, seven-batch SNV semantics, and independently derived M09 output are
gated before immutable publication; the public installer is proved separately
without repeating the data download; the completion documentation has an
independent review owner; and explicit GitHub `Latest` verification preserves
the default installer URL. The exact-six, rollback, tag/target, immutable,
bounded-public-verification, and credential-hygiene boundaries remain coherent.

Material assumption failure after publication-ready preparation: the
coordinator completed the live audit and dispatched package workflow run
`30648307402` for commit `a82968cfc9c29c4e95f647ecaac7452e6ef78da2`.
The workflow failed during linking, before artifact creation, download,
qualification, draft creation, tag creation, upload, or publication. The
downloaded static ONNX Runtime archive requires
`__isoc23_strtol`, `__isoc23_strtoll`, and `__isoc23_strtoull`; those symbols
are unavailable in Ubuntu 22.04's GLIBC 2.35. Local inspection confirms the
resulting linked executable requires GLIBC 2.38/2.39. The ticket therefore
returns to design review with a truthful Ubuntu 24.04/GLIBC 2.39 baseline and
one corrected build authorization.

Correction review: REJECT, then ACCEPT after remediation. The reviewer agreed
that Ubuntu 24.04/GLIBC 2.39 is the smallest truthful first-release baseline,
but found contradictory effect history in the first revision. The coordinator
then recorded the stopped run in External Effect Evidence, time-scoped the old
no-dispatch statements, named the next dispatch the first corrected-baseline
effect, and retained exactly one corrected-commit authorization. Final
re-review confirmed the stopped-effect accounting, corrected platform tests,
unchanged static-runtime/single-executable/six-file contract, and explicit
exclusion of older-GLIBC runtime work. ACCEPT.

Smoke-correction review: REJECT, then ACCEPT after remediation. The reviewer
confirmed the second stopped attempt, consumed authorization, singular
smoke-corrected dispatch, and required positive/negative execution-level smoke
test. The first revision reused the earlier live security audit; the
coordinator corrected the ticket to require and separately record a complete
fresh audit immediately before every authorized package dispatch. Final
re-review: ACCEPT.

Cache-environment correction review: ACCEPT. The reviewer confirmed the exact
safe failure and absent artifact/public state, the consumed authorization, and
the narrow correction: preserve cache-path security by creating an owned
mode-0700 immediate parent under container tmpfs and prove the shared smoke
with the real CLI/model fixture. A fresh audit, new green exact commit, and one
cache-environment-corrected dispatch remain required.

Package/CI separation review: ACCEPT. The reviewer confirmed the exact commit's
full green CI gate remains authenticated before dispatch and rechecked before
release mutation, while the package workflow retains every package-specific
build, SBOM, qualification, smoke, and artifact boundary. The stopped flaky
duplicate-gate run, consumed authorization, and one package-only dispatch are
accurately bounded.

## Implementation Evidence

Developer: `/root/ticket036_implementation`

Publication-ready preparation added:

- a shell-only isolated-XDG production runner covering online sync, offline
  reuse, combined status, the exact seven ordered 1,000-SNV batches, and M09;
- a separate `uv` checker that removes only SNV
  `provenance.bundle_id`, compares all remaining SNV semantics, and compares
  M09 byte-for-byte without reading production asset members;
- the independently derived exact M09 oracle, SHA-256
  `16bbc2256a07104b576fa7c5cd81378b900dd0920e20c8f1cb53c286414a91e9`;
- a normal-test fake executable proving the exact orchestration, ready-state
  checks, oracle acceptance, and rejection of a changed splice score;
- exact release notes and a credential-free PREPUBLICATION operation/evidence
  skeleton pinned to the packaging workflow's Ubuntu image digest; and
- honest prepublication updates to the README, delivery architecture,
  frontier, and FAQ.

The retained request file is SHA-256
`042fcc0e550f7dfccad742a6a2e6a89b0c4e245673b0222bcefb7d42b1ffe52d`.
The deterministic fake-run self-test produced matching canonical SNV hashes
`06a2c7e166f29f8e8eb4c1106e92a67dc4b2f6a6955f45f491b76a6156ca9129`
and matching M09 hashes equal to the oracle above. Those are harness tests,
not production-data qualification evidence.

Local gates after the completed preparation:

```text
make lint
  passed (existing duplicate-dependency warnings only)
make test
  passed (workspace tests plus executable-delivery and qualification scripts)
make spec
  236 passed, 6 skipped
git diff --check
  passed
```

During the original implementation preparation, no GitHub workflow was
dispatched and no tag, release, upload, repository setting, or other external
state was changed. The later stopped attempt is recorded under External Effect
Evidence.

Review remediation tightened all four rejected boundaries. SNV comparison now
deletes one exact raw `bundle_id` byte span and preserves every other byte,
including whitespace and final-newline framing. The checker admits only the
fixed request, seven expected-oracle, and M09 hashes; independently enforces
the exact group order, per-group order/counts, and 1,000 total; and has
negative tests for formatting drift, truncation, request substitution, SNV
oracle substitution, M09 substitution, and score mutation. The runner now
requires all three output/XDG directories to be absent, replaces inherited
HOME with a private qualification home, clears Pangopup path/cache overrides,
and requires the first online sync to install both components. Its fake CLI
asserts the isolated HOME/XDG environment.

The publication evidence now contains one extractable, `bash -n`-checked
coordinator program. It has exact live security/ruleset/CI assertions,
single-run workflow discovery and artifact admission, draft/tag/body checks,
descriptor-held private staging, one-upload prefix inventory checks, bounded
draft rollback, immutable/Latest/direct-tag publication checks, unauthenticated
public metadata and five-small-asset verification, and the fresh tagged
installer/offline-reuse/SNV/M09 proof. Placeholder values fail closed before
the first effect. The focused remediation test passed before the final full
gate. No coordinator command was executed.

Corrected-baseline implementation changed the read-only packaging runner to
Ubuntu 24.04 and pinned every clean-container invocation to
`ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`.
The shared qualifier now admits a measured maximum imported GLIBC version of
2.39 and rejects a test executable mutated to require GLIBC 2.40. README,
delivery architecture, frontier, FAQ, release notes, and the operation/evidence
runbook consistently state Linux x86_64/amd64 with GLIBC 2.39 or newer. The
runbook preserves original package run `30648307402`, its exact preparation
commit and successful CI run, its link-failure boundary, and the absence of an
artifact or release mutation. The static ONNX Runtime selection, five admitted
dynamic system dependencies, exact-six inventory, semantic qualification, and
one corrected-dispatch boundary are unchanged.

Focused implementation checks:

```text
bash tests/executable-delivery.sh
  passed
bash tests/production-release-qualification.sh
  passed
bash -n scripts/qualify-linux-release.sh tests/executable-delivery.sh tests/production-release-qualification.sh
  passed
git diff --check
  passed
```

No GitHub operation, workflow dispatch, tag, release, upload, or repository
setting mutation occurred during corrected-baseline implementation.

Smoke-correction implementation moved the complete version/help/status/SNV/
model behavior into `scripts/smoke-linux-release.sh`. The workflow invokes
that one checked script inside the pinned container with the exact executable,
source, data, and model-cache arguments; it no longer duplicates commands or
JSON patterns in nested shell text. The focused executable-delivery test invokes
the same repository script through a fake `docker ... bash -ceu '...'`
boundary and a fake Pangopup executable. Valid status, precomputed-SNV, and
model JSON all pass, all five commands are observed, and a mutated copy changing
only the expected status from `missing` to `ready` fails. An exact-line test
binds the workflow to the shared script and its four reviewed arguments and
rejects inline variant copies. The public operation record preserves
corrected-baseline package run `30649914623`, its passed qualification boundary,
failed smoke boundary, zero artifact/release effect, and consumed authorization.
It requires a separately recorded fresh security audit before the sole
smoke-corrected dispatch.

Focused smoke-correction checks:

```text
bash tests/executable-delivery.sh
  passed
bash tests/production-release-qualification.sh
  passed
bash -n scripts/smoke-linux-release.sh tests/executable-delivery.sh tests/production-release-qualification.sh scripts/qualify-linux-release.sh
  passed
git diff --check
  passed
```

No GitHub operation, workflow dispatch, tag, release, upload, or repository
setting mutation occurred during smoke-correction implementation.

Cache-environment correction changes the shared smoke's fourth argument from a
SQLite file to an absent immediate child of `/tmp`, which is the workflow's
private tmpfs mount. After status and SNV checks, the script creates that parent
atomically at mode `0700`, validates it is a non-symlink directory owned by the
current user with exact mode `0700`, and gives the real model lookup only its
`model.sqlite3` child. The workflow now passes `/tmp/pangopup-smoke-cache`.
The retained fake-container regression still observes all five commands and
the changed-expectation failure, and additionally confirms the child cache path
and parent ownership/mode. A real `target/debug/pangopup` regression with the
existing miniature SNV/reference/mask/model fixtures proves an SQLite path
directly under `/tmp` fails with `MODEL_CACHE_INVALID`, while the exact shared
script creates the private parent, completes model inference, and leaves a real
SQLite database beneath it. The operation record preserves package run
`30651619497`, its passed boundaries, cache-parent failure, absence of an
artifact or release effect, consumed authorization, and the one remaining
fresh-audit-gated cache-environment-corrected dispatch.

Focused cache-environment checks:

```text
bash tests/executable-delivery.sh
  passed
bash tests/production-release-qualification.sh
  passed
bash -n scripts/smoke-linux-release.sh tests/executable-delivery.sh tests/production-release-qualification.sh scripts/qualify-linux-release.sh
  passed
git diff --check
  passed
```

No GitHub operation, workflow dispatch, tag, release, upload, or repository
setting mutation occurred during cache-environment implementation.

## Adversarial Code Review

Reviewer: `/root/ticket037_implementation`

First review: REJECT.

- The SNV checker parsed and reserialized JSON, so formatting drift could pass
  despite the promised byte equality after removing only `bundle_id`.
- Corpus and oracle hashes/counts/order were printed but not enforced; a
  substituted seven-record corpus passed.
- The publication artifact described most GitHub operations in prose rather
  than providing the required executable commands and exact assertions.
- The production runner accepted existing XDG directories, inherited HOME,
  and allowed first online sync to report reuse, so it did not prove a fresh
  isolated installation.

Remediation and re-review: pending.

The developer remediated all four findings. The checker now removes one exact
validated raw field span without reserializing anything else, authenticates
the fixed request/M09/all-seven-oracle identities, enforces group order/counts
and exactly 1,000 requests, and rejects formatting drift, truncation, score
changes, and substituted oracles. The runner requires absent XDG/output roots,
isolates HOME, and requires the first sync to report installation. The durable
artifact now contains one extracted, syntax-checked official-`gh` coordinator
program covering the full audit, build binding, exact-six qualification,
draft/upload/prefix checks, bounded rollback, immutable/Latest publication,
bounded public reads, and tagged-installer proof.

Final re-review: ACCEPT. The reviewer independently confirmed the byte-level
comparison, fixed corpus identities and counts, exact M09 oracle, fresh runtime
environment, transitive exact build-input binding, one-time upload and closed
inventory checks, safe prepublication-only rollback, immutable direct-tag and
Latest checks, unauthenticated verification, installer proof, negative tests,
clean diff, and extracted-runbook Bash syntax. No source or GitHub mutation was
performed during review.

Corrected-baseline code review: ACCEPT. The reviewer independently confirmed
the Ubuntu 24.04 runner and pinned amd64 image, the measured GLIBC 2.39 import
and explicit 2.40 rejection, the unchanged exact five system dependencies and
static ONNX Runtime, and the preserved exact-six, manifest, SBOM, semantic,
isolated-XDG, release-security, rollback, immutable/Latest, and installer
boundaries. Read-only GitHub inspection also confirmed the recorded failed-run
stage, zero workflow artifacts, and absence of a `v0.1.0` release or tag. The
focused suites, shell syntax, and diff checks passed. No material findings.

Smoke-correction code review: REJECT, then ACCEPT after remediation. The first
test copied workflow commands and bound only their `grep` suffixes, allowing
the workflow's executable, variants, bundles, mask, or cache arguments to drift
untested. The developer replaced both copies with one strict shared smoke
script invoked exactly once by the workflow inside the pinned container and by
the focused fake-Docker/inner-Bash test. The reviewer confirmed all five
commands are observed, a changed expected status fails, the exact four-argument
workflow invocation is bound, and no inline command copy remains. Focused
suites, shell syntax, and diff checks pass; all release and runtime boundaries
remain unchanged. Final re-review: ACCEPT.

Cache-environment code review: ACCEPT. The reviewer confirmed the strict
absent-immediate-child grammar, atomic mode-0700 creation, non-symlink/current
owner/exact-mode validation, child SQLite path, and the real CLI proof that a
direct `/tmp` cache fails while the shared smoke completes real model inference
and creates the SQLite file privately. Fake five-command coverage, exact
workflow invocation, stopped-run evidence, and all release/runtime boundaries
remain intact. No material findings.

Cache-environment code review: pending.

## External Effect Evidence

Coordinator: `/root`

Stopped original-baseline attempt:

- target commit: `a82968cfc9c29c4e95f647ecaac7452e6ef78da2`;
- exact remote `ci` run `30648083892`: completed successfully;
- package workflow run: `30648307402`, completed with `failure`;
- failure boundary: `Gate and build`, while linking the selected static ONNX
  Runtime on Ubuntu 22.04/GLIBC 2.35;
- artifact: none; artifact preparation, qualification, smoke, and upload steps
  were skipped;
- public state after the stop: no draft, release, tag, or uploaded release
  asset for `v0.1.0`.

The operation stopped at the failed workflow. It made no release mutation and
did not retry. Its corrected-baseline authorization was consumed by the next
recorded attempt. Corrected qualification and publication evidence remain
pending.

Stopped corrected-baseline attempt:

- target commit: `dc9cf1ccb3b0313042bed333463758da24feb184`;
- exact remote `ci` run `30649639343`: completed successfully;
- package workflow run: `30649914623`, completed with `failure`;
- passed boundary: full gate and release build, deterministic SBOM, exact-six
  preparation, and final GLIBC/dependency/inventory qualification;
- failure boundary: clean-container smoke. The nested shell expression reduced
  the intended JSON pattern `"status":"missing"` to `status:missing`, so it
  rejected the valid JSON output;
- artifact: none; Actions artifact upload was skipped;
- public state after the stop: no draft, release, tag, or uploaded release
  asset for `v0.1.0`.

The operation stopped at the failed workflow and did not retry. The first
corrected authorization is consumed. After independent acceptance of an
executed smoke-command regression and a new exact commit's green remote gate,
exactly one smoke-corrected package dispatch is permitted.

Stopped smoke-corrected attempt:

- target commit: `6f381736df9eda5abbbf1d57641bb40f5e5b024c`;
- exact remote `ci` run `30651379985`: completed successfully;
- package workflow run: `30651619497`, completed with `failure`;
- passed boundary: full gate/build/SBOM/preparation/qualification, shared smoke
  version/help, status, and precomputed SNV lookup;
- failure boundary: real model lookup rejected `/tmp/model.sqlite3` with
  `MODEL_CACHE_INVALID` because `/tmp` is not an owned private directory;
- artifact: none; Actions artifact upload was skipped;
- public state after the stop: no draft, release, tag, or uploaded release
  asset for `v0.1.0`.

That authorization is consumed. After independent acceptance of the private
cache-parent correction, real-CLI smoke regression, fresh security audit, and a
new exact commit's green gate, exactly one cache-environment-corrected package
dispatch is permitted.

Stopped duplicate-gate attempt:

- target commit: `8b271c065be198df2feffc33fbdee27cd49ac463`;
- exact remote `ci` run `30652597423`: completed successfully;
- package workflow run: `30652858960`, completed with `failure`;
- failure boundary: redundant `make test`; unrelated warmed-allocation test
  observed 184 allocations after a 180-allocation baseline;
- release build, artifact creation/upload, draft, tag, and release: none.

After independent acceptance of package/CI responsibility separation, a new
green exact commit, and a fresh security audit, exactly one package-only
dispatch is permitted.

Use the exceptional lifecycle:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator external effect -> complete -> commit/push -> cleanup
```

## Coordinator Final Check

Coordinator: `/root` (publication-ready check)

After final code-review acceptance, the coordinator inspected the complete
diff and reran `make lint`, `make test`, `make spec`, and `git diff --check`.
All passed; Mustmatch reports 236 passed and 6 intentionally skipped. The
normal test gate includes the executable-delivery suite and the hostile
production-release qualification suite. Current documentation consistently
describes the executable as prepared but not yet public. No workflow, tag,
release, upload, or repository-setting mutation occurred before this check.

Final completion evidence and the reviewed current-state documentation
transition remain pending the exact-commit remote gate and coordinator-owned
external effect.

After corrected-baseline code-review acceptance, the coordinator inspected the
complete correction and reran `make lint`, `make test`, `make spec`, and
`git diff --check`. All passed; Mustmatch again reports 236 passed and 6
intentionally skipped. The test gate includes the GLIBC 2.40 rejection, stale
runner/ceiling assertions, pinned-image/runbook assertions, executable delivery
suite, and hostile production-release qualification suite. The corrected
publication-ready documentation remains explicitly prepublication.

After smoke-correction re-review acceptance, the coordinator reran the same
full `make lint`, `make test`, `make spec`, and `git diff --check` gate. All
passed; Mustmatch reports 236 passed and 6 intentionally skipped. The delivery
test executed the shared smoke script through the nested shell boundary,
observed all five commands, and rejected the changed-value mutation.
