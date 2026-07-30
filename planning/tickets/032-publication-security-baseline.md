# 032 — Enforce the publication security baseline

Status: publication-ready

## Why

Pangopup's public repository currently gives GitHub Actions a writable token by
default, permits Actions to approve pull requests, uses a moving
`actions/checkout@v4` reference, installs `mustmatch==0.1.0` without authenticating
its bytes, has no enforced Rust advisory/license/source policy, and has no main
ruleset. GitHub also reports Dependabot security updates, secret scanning,
non-provider scanning, push protection, and validity checks disabled.

The model, compact reference, and mask runtime transport is already built and
qualified locally. Public distribution must not begin until repository and
dependency controls are explicit and observable. This ticket establishes that
baseline without uploading an asset, changing a release, or mixing runtime
download behavior into repository administration.

## Scope

- Harden `.github/workflows/ci.yml`:
  - declare top-level `permissions: contents: read`;
  - pin `actions/checkout` to full commit
    `11d5960a326750d5838078e36cf38b85af677262`;
  - retain the already-full-SHA `astral-sh/setup-uv` reference
    `08807647e7069bb48b6ef5acd8ec9567f424441b`;
  - replace the unverified PyPI `mustmatch` resolution with the dependency-free
    Linux x86-64 wheel from the versioned `v0.1.0` GitHub release,
    authenticated as 1,469,293 bytes with SHA-256
    `86c6c250578413009eed9ff591d5fa3107c9695176d418dbfc2ccae87d472e77`,
    then install that exact local wheel with `uv`;
  - retain the existing checksum-authenticated ripgrep installation; and
  - install `cargo-deny` 0.19.4 from the 4,965,853-byte official
    `cargo-deny-0.19.4-x86_64-unknown-linux-musl.tar.gz` release asset with
    SHA-256
    `3bd58b784e83715b86ddbc9deac591890372ec77fda5741bb0826970b958506f`,
    then run the ordinary repository gate.
- Add a checked `deny.toml` covering the complete locked Rust graph:
  - deny vulnerable, unsound, yanked, and unused-ignored advisory conditions;
    warn on unmaintained advisories and carry no advisory ignores;
  - allow `0BSD`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`,
    `BSD-3-Clause`, `CDLA-Permissive-2.0`, `GPL-3.0-only`, `ISC`, `MIT`,
    `Unicode-3.0`, `Unlicense`, and `Zlib`, at confidence threshold `0.93`;
    carry no package-specific license exception unless ticket design review is
    repeated with concrete locked-graph evidence;
  - allow Cargo wildcard requirements because `cargo-deny` 0.19.4 classifies
    Pangopup's versionless local workspace path dependencies as public
    wildcards and cannot exempt them with `allow-wildcard-paths`; the exact
    `Cargo.lock`, registry checksums, source policy, and reviewed manifest diff
    remain the external-dependency controls; report multiple versions as
    warnings because the current locked graph legitimately contains parallel
    versions such as `getrandom`; do not add package skip/skip-tree
    exceptions; and
  - deny unknown registries and Git sources, allowing only the canonical
    crates.io registry plus workspace packages.
- Make `cargo deny check advisories bans licenses sources` part of `make lint`.
  The developer documents the exact `cargo-deny` version required locally;
  CI installs that same version. Do not add a second generic `check` target.
- Update `README.md`, `architecture/delivery.md`, `planning/frontier.md`, and
  `planning/issues/2026-07-24-publication-security-baseline.md` with the shipped
  code-side and repository-side baseline. Do not claim runtime assets, an
  executable, container, SBOM, or release provenance has been published.
- After independent code review, commit/push, and a green remote gate for that
  exact commit, the coordinator alone applies and verifies these GitHub
  settings through `gh api`:
  - Actions default workflow permission `read` and permission to approve pull
    requests `false`;
  - Dependabot security updates enabled;
  - secret scanning, push protection, and non-provider patterns enabled;
    validity checks remain explicitly observed and reported but are not a
    completion condition because GitHub's current repository-local write API
    exposes no field for them and no code-security configuration is attached;
  - one active `pangopup-main-history` ruleset with no bypass actors that
    targets `main` and rejects deletion and non-fast-forward updates; and
  - one active `pangopup-main-contributions` ruleset targeting `main` that
    requires pull requests and the `gate` status check for ordinary
    contributors, with exactly one bypass actor:
    `actor_type=RepositoryRole`, `actor_id=5` (repository administrator), and
    `bypass_mode=always`, so Pangopup's coordinator-owned reviewed fast-forward
    direct-push lifecycle remains operable.
- Before code review, create
  `planning/artifacts/032-publication-security-baseline.md` containing the
  sanitized captured pre-state, exact intended REST payloads in mutation
  order, the exact bypass actor tuple above, expected after-state, and a
  per-operation rollback table. The coordinator must re-read the same pre-state
  immediately before mutation and abort on drift. Afterward, append the
  sanitized observed after-state and created opaque ruleset IDs. Never record
  tokens, authorization headers, cookies, credentials, or secret values.
- Apply settings in this order: Actions defaults; Dependabot update setting;
  secret-scanning controls; unbypassed history ruleset; administrator-bypassed
  contribution ruleset. On any failure, roll back successful operations in
  exact reverse order: delete only rulesets created by this ticket, then
  restore each feature and Actions field to its captured value. Verify the
  restored pre-state before reporting failure.
- If any independently writable requested feature is unavailable or either
  ruleset cannot express the reviewed semantics, stop, roll back, and return
  the observed limitation to design review. The already-proved
  validity-check write limitation does not block applying the other independent
  controls; retain it precisely in the open publication-security issue and do
  not claim the complete publication blocker is closed.
- Do not publish, draft, upload, delete, or modify any GitHub release or asset.
  Do not open/rebuild production assets, implement runtime sync, add Dependabot
  version-update PRs, automatically merge dependencies, or change scoring,
  installation, CLI output, model behavior, or data formats.

## Success Checklist

- The checked workflow contains no moving action tag, has explicit read-only
  permissions, cannot approve pull requests, and installs each downloaded
  maintenance tool through an exact version plus content digest or equivalent
  reviewed content-authenticated boundary.
- A controlled wrong-wheel digest test or shell-level fixture proves the CI
  installation step fails before installing altered `mustmatch` bytes; normal
  CI installs the exact dependency-free wheel and `make spec` remains green.
- `deny.toml` is minimal and complete for the locked graph. Focused tests or
  commands prove:
  - current advisories, licenses, bans, and sources pass;
  - a synthetic or temporary disallowed license/source/advisory policy change
    fails without modifying `Cargo.lock`; and
  - no broad `allow-git`, `allow-registry`, advisory ignore, package skip, or
    unbounded license confidence exception silently defeats the policy.
- `make lint` invokes the exact deny groups before succeeding. `make test` and
  `make spec` retain their existing ownership and behavior.
- The exact reviewed commit's GitHub `gate` workflow is green before any
  repository-setting mutation.
- After the external effect, GitHub API evidence shows:
  - default workflow permission is read-only;
  - Actions cannot approve pull requests;
  - Dependabot security updates are enabled;
  - the three writable requested secret-scanning controls are enabled;
    validity checks still report disabled with the documented API limitation;
    and
  - the two active rulesets target `main`; history protection has no bypass,
    while contribution rules have only the reviewed administrator bypass.
- A post-effect ordinary coordinator fast-forward documentation/evidence commit
  succeeds through the administrator bypass and receives a green `gate`.
  Evidence also proves the unbypassed history ruleset disallows force pushes
  and branch deletion for every actor, including administrators; do not perform
  either destructive operation as a test.
- Current documentation says precisely what is protected and what remains:
  release-specific dependency inventory, SBOM, provenance, controlled
  stable-source upload, remote digest comparison, immutable finalization,
  runtime sync, and clean-machine inference are still future publication work.
- The publication-security issue remains open after this ticket, narrowed to
  the validity-check limitation plus release-specific inventory/SBOM/
  provenance and publication evidence. This ticket is complete when all
  independently writable code/repository controls match the reviewed
  after-state; it must not claim the overall publication blocker is closed.
- `make lint`, `make test`, and `make spec` pass locally; both the
  pre-effect reviewed commit and post-effect completion commit have green
  remote `gate` runs.

## Decisions

### Make dependency policy part of the ordinary lint gate

- **Consideration:** A policy file that CI or maintainers can forget to run
  does not block an unsafe release.
- **Options:** Document an optional audit command; add a fourth top-level gate;
  run it only in GitHub Actions; or include it in `make lint`.
- **Trade-offs:** Including it in lint requires contributors to install the
  pinned tool, but preserves Pangopup's established three-command gate and
  makes local and remote policy identical.
- **Decision:** `make lint` runs
  `cargo deny check advisories bans licenses sources`. README names the exact
  supported `cargo-deny` version. CI downloads the exact 0.19.4 Linux-musl
  archive, checks its pinned size and SHA-256, and installs only its binary.
  The policy denies advisories/unsound/yanked/unused ignores, warns on
  unmaintained crates, has no ignores, allows wildcard syntax only because
  cargo-deny cannot distinguish the existing publishable workspace path
  dependencies, denies unknown sources, warns on duplicate versions without
  skips, and uses only the explicit reviewed license allowlist in Scope.

### Authenticate the existing mustmatch binary wheel

- **Consideration:** CI needs the project-standard `mustmatch` binary, but
  `uv tool install mustmatch==0.1.0` authenticates a version choice less
  explicitly than Pangopup's other downloaded tools.
- **Options:** Keep PyPI resolution; build mustmatch from Git source on every
  run; copy a binary into Pangopup; or download the dependency-free official
  release wheel and verify its exact size/SHA-256.
- **Trade-offs:** Source builds add another Rust build and toolchain boundary.
  Vendoring duplicates another OSS project. The wheel is small, has no Python
  dependencies, and can be verified like ripgrep.
- **Decision:** Download the exact manylinux x86-64 `v0.1.0` wheel, check the
  pinned digest before installation, and install that local file. Do not
  commit the wheel.

### Use least-privilege workflow tokens

- **Consideration:** The gate only reads repository contents and does not need
  to write code, releases, checks, packages, or pull-request approvals.
- **Options:** Keep GitHub's writable default; set job-specific permissions; or
  set a workflow-wide read-only contents permission.
- **Trade-offs:** A workflow-wide declaration is simpler and fails closed for
  future jobs; a future publishing job would need an explicit separately
  reviewed permission increase.
- **Decision:** Set top-level `permissions: contents: read`, repository default
  workflow permissions to `read`, and PR-approval permission to `false`.

### Protect contributors without breaking the reviewed direct-push lifecycle

- **Consideration:** Pangopup deliberately commits reviewed ticket outcomes
  directly to `main`; a universal pull-request rule would invalidate the
  documented SDLC. Leaving `main` unprotected permits accidental contributor
  pushes and unreviewed history changes.
- **Options:** Convert the whole project to pull requests; leave `main`
  unprotected; put every rule behind one administrator bypass; split
  unbypassed history rules from bypassed contribution rules; or bypass by a
  personal actor/application.
- **Trade-offs:** Administrator bypass is broader than a dedicated GitHub App,
  but it is stable, does not bind the OSS repository to one person, and matches
  the current coordinator authority. Pull requests remain required for
  ordinary contributors.
- **Decision:** Use two rulesets. The history ruleset rejects deletion and
  non-fast-forward updates without any bypass. The contribution ruleset
  requires PR plus green `gate` for ordinary contributors and preserves exactly
  one `RepositoryRole` actor ID 5 `always` bypass. No personal actor, deploy
  key, or custom App is introduced.

### Review and roll back GitHub mutations as one bounded operation

- **Consideration:** Repository settings are external state; reviewing only a
  prose goal or recording the payload after mutation makes mistakes hard to
  detect and reversal ambiguous.
- **Options:** Construct API payloads interactively; let the coordinator infer
  rollback; or check the sanitized pre-state, exact ordered payloads, expected
  state, and reverse rollback mapping before code approval.
- **Trade-offs:** Pre-authoring the payload artifact adds a small amount of
  documentation but makes the public effect reviewable and drift-detectable.
- **Decision:** The implementation diff contains the complete sanitized
  operation plan before code review. The coordinator aborts if pre-state
  changed, applies the reviewed order, and on failure reverses only completed
  ticket operations in exact reverse order and proves the captured pre-state
  was restored.

### Separate repository hardening from asset publication evidence

- **Consideration:** SBOM and provenance must describe the exact artifacts and
  source commit actually released. No public runtime release is created here.
- **Options:** Generate placeholder release evidence now; combine publication
  with repository settings; or establish code/repository controls now and
  generate release-specific evidence in the public-effect ticket.
- **Trade-offs:** Placeholder evidence becomes stale. Combining effects makes
  rollback and review harder. Separation leaves publication blocked for one
  more ticket but keeps every claim tied to real bytes.
- **Decision:** This ticket closes repository and dependency-policy controls
  only. The next publication ticket must generate and review the exact
  dependency inventory, SBOM/provenance, attribution, stable-source upload,
  remote digest comparison, and immutable finalization before publishing.

### Apply independent controls despite the validity-check API gap

- **Consideration:** GitHub reports validity checks as disabled but its current
  repository-update schema exposes no repository-local write field, and this
  repository has no attached code-security configuration. The other controls
  are independently writable and reversible.
- **Options:** Make no security improvement until all four controls share one
  API; guess an undocumented field; or apply every reviewed writable control
  while retaining the exact residual blocker.
- **Trade-offs:** Partial hardening does not clear publication, but withholding
  Actions, Dependabot, scanning, push protection, and history protections adds
  risk without helping validity checks. An undocumented field is unacceptable.
- **Decision:** Apply and verify all independently writable controls and both
  rulesets. Leave validity checks disabled and keep the publication-security
  issue open. GitHub does expose validity checks through organization
  code-security configurations, but the current token lacks required
  `write:org`; the existing recommended configuration enables additional
  out-of-scope features; and detach/delete retain repository settings, so a
  custom selected-repository configuration needs separately reviewed authority,
  asynchronous verification, and rollback. Do not publish assets under this
  residual blocker.

## Dependencies

- Ticket 031 is complete: Pangopup no longer ships or tests a custom GitHub
  uploader, and official `gh` publication remains an explicit later
  coordinator effect.
- Ticket 030 is complete: the exact local derived runtime transport and
  identities exist without requiring a rebuild.

## Notes

- The exact repository gate is `make lint`, `make test`, and `make spec`; there
  is no `make check`.
- The current repository is public, the coordinator has admin permission, the
  vulnerability-alert endpoint is enabled, no ruleset/branch protection
  exists, Actions defaults to `write` with PR approval enabled, and all listed
  secret-scanning/Dependabot security-update controls report disabled.
- `actions/checkout` tag `v4` currently resolves to full commit
  `11d5960a326750d5838078e36cf38b85af677262`. The implementation must use the
  full SHA, not the tag. Do not silently move it during this ticket.
- `astral-sh/setup-uv@08807647e7069bb48b6ef5acd8ec9567f424441b`
  is already a full, GitHub-verified commit. Retain it unless review finds
  direct evidence requiring a separately reviewed change.
- The checked mustmatch wheel has no package dependencies. Its release URL is
  `https://github.com/genomoncology/mustmatch/releases/download/v0.1.0/`
  followed by
  `mustmatch-0.1.0-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl`.
- The developer may inspect GitHub state read-only but performs no GitHub
  mutation. The coordinator alone performs the reviewed external effects.
- Repository rules and APIs can evolve. Treat the semantic after-state in this
  ticket as authoritative; record the exact accepted API payload in the
  artifact without credentials.
- Public-repository hygiene forbids secrets, tokens, credentials,
  machine-specific absolute paths, downloaded wheels, generated production
  payloads, or unredacted API headers in Git.
- Evidence shown here is illustrative unless explicitly named as the durable
  artifact.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-07-30

The coordinator drafted this ticket from the observed public GitHub settings,
the open publication-security issue, Ticket 031's deletion outcome, and the
rolling publication frontier. It does not implement or approve the code diff.

## Independent Ticket Review

Reviewer: `/root/ticket032_design_review`

First review: REJECT.

- One administrator-bypassed ruleset could not honestly claim unbypassed
  force-push/deletion protection.
- The ticket had not selected an exact authenticated `cargo-deny` distribution
  or the substantive advisory/license/bans/source policy.
- GitHub payloads and rollback were not fully reviewable before mutation.
- The mustmatch release was incorrectly called immutable.

The coordinator accepted every finding. The revision uses separate unbypassed
history and administrator-bypassed contribution rulesets; pins the exact
`cargo-deny` 0.19.4 archive and complete policy; requires pre-review payload,
pre-state, mutation order, and reverse rollback evidence; and describes the
mustmatch wheel only as a versioned asset authenticated by exact size/digest.

Re-review: ACCEPT. No material design finding remains. Implementation must
empirically prove the pinned tool archives and negative cases, GitHub's
acceptance of both exact ruleset payloads, and the post-effect administrator
fast-forward push plus green `gate`.

Implementation-discovered scope revision: `cargo-deny` 0.19.4 still rejects
versionless local path dependencies when every Pangopup crate is publishable;
its `allow-wildcard-paths` switch does not apply. Adding redundant versions to
all internal dependency edges would churn builder-causal manifests without
improving registry security. The revision therefore allows wildcard syntax,
keeps the exact lockfile and registry/source controls authoritative for
external code, and retains no package skips. Ticket re-review: ACCEPT. The
reviewer accepted this as a bounded, honestly documented cargo-deny limitation;
locked gates, registry checksums, crates.io-only sources, no Git dependencies,
and reviewed manifest/lock diffs still control the exact external graph without
builder-causal manifest churn.

Code review exposed a second design revision: GitHub has no reviewed
repository-local validity-check write, but every other setting is independent.
The ticket now authorizes those writable improvements, keeps validity checks
disabled and the publication issue open, and forbids claiming release
readiness. Ticket re-review: ACCEPT. The reviewer accepted the bounded,
reversible partial-hardening outcome because it preserves the exact residual
blocker without withholding unrelated protections or guessing an API.

Code review then identified the organization code-security-configuration API.
That route is not available through the current `read:org` token, the existing
recommended configuration has broader side effects, and detach retains applied
settings. The ticket therefore retains the accepted repository-local
partial-hardening boundary but now records the known organization route and
the exact authority/rollback reasons it is separate follow-up work. Ticket
re-review: ACCEPT. The reviewer confirmed that the organization route requires
new authority and broader asynchronous, non-equivalently reversible effects,
so it belongs outside this repository-local operation.

## Implementation Evidence

Developer: `/root/ticket032_implementation`, 2026-07-30

- The checked workflow now has only `contents: read`, pins both actions to the
  reviewed full commits, and authenticates the exact mustmatch 0.1.0 wheel
  (`1,469,293` bytes,
  `86c6c250…d472e77`) and cargo-deny 0.19.4 archive (`4,965,853` bytes,
  `3bd58b78…8506f`) before installation. A same-size one-byte wheel mutation
  failed SHA-256 verification and the controlled `uv` install root remained
  absent, proving installation was not entered.
- Cargo-deny 0.19.4 passes the complete locked graph for
  `advisories bans licenses sources`. Temporary policy copies that removed
  `MIT`, replaced the canonical crates.io registry, or added an unused advisory
  ignore each failed at the intended policy boundary. `Cargo.lock` remained
  byte-identical at
  `1d8bd9db…b9feab`; the checked config has no advisory ignore, license
  exception, Git allowance, package skip, or skip tree.
- The independently accepted wildcard revision leaves every workspace
  `Cargo.toml` unchanged. Focused production-identity and canonical dependency
  projection tests passed, as did the complete builder provenance suite in
  `make test`.
- `make lint`, `make test`, and `make spec` pass locally with the exact
  authenticated tools; mustmatch reports `218 passed, 4 skipped`.
- Read-only GitHub API capture matched the ticket's pre-state. The sanitized
  ordered payloads, exact `RepositoryRole/5/always` bypass, expected state, and
  reverse rollback are checked in
  `planning/artifacts/032-publication-security-baseline.md`. The current
  official repository-update schema has no write field for the validity-check
  status returned by repository reads. After independent design re-review, the
  artifact authorizes the other writable controls, expects validity checks to
  remain disabled, and keeps the issue and asset-publication block open. No
  repository setting, ruleset, release, or asset was mutated.

## Adversarial Code Review

Reviewer: `/root/ticket032_code_review`

First review: REJECT. The operation artifact correctly established that the
repository-update endpoint has no validity-check write, but its broader
no-REST-path conclusion was incomplete. Organization code-security
configurations can carry validity checks and attach to selected repositories;
the live recommended configuration enables additional out-of-scope features,
while a custom configuration requires expanded organization authority,
asynchronous verification, and non-equivalent detach rollback. The finding
returned the external-effect boundary to design review. All checked-code,
authenticated-tool, dependency-policy, fingerprint, ruleset-payload, and
existing reverse-rollback review points were otherwise clean.

Design remediation: the coordinator returned the limitation to the independent
ticket reviewer. The accepted revision authorizes every documented writable
control, expects validity checks to remain disabled, keeps the
publication-security issue and asset-publication block open, and forbids any
claim of complete publication readiness. The organization-configuration route
is accurately retained as a separate future authority/design problem rather
than called nonexistent or guessed into this ticket. The developer updated the
artifact, durable docs, and implementation evidence to that reviewed boundary.

Re-review: ACCEPT. The revised artifact, ticket, and durable docs accurately
authorize the five repository-local writable operations, require validity
checks to remain disabled, keep the issue and publication blocked, and retain
the organization-configuration route with its authority, asynchronous
application, broader-feature, and detach-retains-settings constraints. Reverse
rollback and both ruleset semantics remain sound. No material finding remains.

## External Effect Evidence

Coordinator: pending. This ticket uses the exceptional reviewed external-effect
lifecycle:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator GitHub settings -> complete -> commit/push -> cleanup
```

No release or asset mutation is authorized.

## Coordinator Final Check

Coordinator: pending
