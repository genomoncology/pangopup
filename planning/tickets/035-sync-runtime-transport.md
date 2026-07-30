# 035 — Sync the pinned runtime transport directly into installation

Status: complete

## Why

The immutable `runtime-grch38-v1` model/reference/mask release is public.
Pangopup can install those assets from explicit decoded local paths, but it
cannot yet download the published transport. The ordinary top-level
`pangopup sync` and combined status interface should be a thin composition of
two already-proven delivery primitives, not the first test of a new downloader
and a new large-member installation path.

This ticket adds the missing model-side primitive: pinned resumable download
of the exact public ten-file runtime download set and one-pass installation into
the existing atomic runtime store. It deliberately leaves the CLI grammar
unchanged so the following ticket can define one small combined command/status
contract over established SNV and runtime operations.

## Scope

- Add a production library operation in `pangopup-assets` that synchronizes
  the exact immutable `runtime-grch38-v1` transport into an already installed,
  compatible SNV data root.
- Check in the exact published `runtime-release-profile.json` and
  `runtime-transport.json` under `release-profiles/` as binary authority.
  Statically prove that the release profile pins the transport manifest's
  exact byte hash, then bind the two canonical files to:
  - release tag, page, repository, title, and target commit
    `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`;
  - runtime transport identity
    `sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`;
  - runtime profile identity
    `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`;
  - the release profile's exact ten downloadable file URLs, roles, names,
    stored sizes/hashes, consisting of `runtime-transport.json` plus its nine
    stored transport members;
  - the transport manifest's nine-member inventory, reconstructed
    sizes/hashes, encoder contract, and the existing trusted four-asset
    profile.
  Never query “latest,” discovery metadata, or an operator-supplied URL.
- Reuse or generalize the existing SNV sync machinery for:
  - bounded rustls HTTPS and redirects restricted to the reviewed
    GitHub/release-download origin;
  - nonblocking cache locking;
  - exact size and SHA-256 checks;
  - resumable partials accepted only with the exact strong ETag, HTTP Range,
    and response contract;
  - private XDG cache publication, offline reuse, timeout bounds, and safe
    hostile-entry handling.
  Do not introduce a second HTTP client or copy a weaker runtime-specific
  downloader.
- Cache the ten release files: the raw runtime transport manifest and its nine
  raw/compressed members. A complete cached transport installs offline; an
  incomplete offline transport returns one bounded typed error naming every
  missing release file. This primitive does not inspect the SNV release cache.
- Join authenticated runtime-transport decoding to the existing atomic runtime
  installation staging boundary:
  - decode each successful compressed model/reference/mask frame exactly once
    into its staged immutable destination;
  - do not create a complete decoded intermediate transport directory and do
    not copy a decoded large member a second time;
  - authenticate stored and reconstructed bytes in the same stream;
  - preserve held-descriptor/path-replacement protections and validate model,
    reference, mask, runtime profile, and active SNV compatibility before
    activation;
  - publish components/profile/active state through the established durable
    transitions.
- If download, cache publication, decode, validation, or activation fails:
  - the already installed SNV bundle remains active and usable;
  - any previously active valid runtime profile remains selected;
  - complete cached members remain reusable and safe partials remain resumable;
  - no partial runtime profile becomes visible.
- Return a typed runtime-sync outcome with status (`installed` or `reused`),
  runtime transport/profile identities, installed component identities, path,
  downloaded bytes, and resumed bytes. Errors remain bounded and contain no
  credentials, response headers, or signed redirect URLs.
  `path` is the immutable activated runtime profile directory containing the
  canonical profile and receipt, not a cache path or component payload path.
- The operation owns only its runtime cache lock and then uses the existing
  data-root install lock during installation. It never holds the data-root
  lock while downloading. Lock order is therefore runtime-cache lock before
  data-root install lock, with ordinary process/descriptor release on failure
  or crash. Whole-product provisioning state and cross-release lock/status
  policy belong to the following combined CLI ticket.
- Normal tests use miniature local assets and loopback HTTP only. They never
  contact GitHub, download/decode the production payload, open the 15 GB SNV
  member, or run production ONNX inference.
- Update `AGENTS.md`, `README.md`, `architecture/delivery.md`,
  `architecture/runtime-data.md`, `planning/frontier.md`, and
  `planning/faq.md`.
- Exclude all CLI grammar/output changes, combined SNV/runtime orchestration,
  whole-product provisioning lock/status, a real clean-machine production
  download, persistent progress, repair/GC/rollback, HTTP, Docker/systemd,
  executable releases, signing/SBOM/provenance, and asset rebuild,
  recompression, format change, or publication.

## Success Checklist

- A miniature empty runtime cache downloads all ten members over the real
  bounded client and installs/activates a compatible runtime profile beside a
  miniature active SNV bundle.
- A second online call and an offline call reuse the installed runtime with
  zero network bytes. Offline mode can install a complete cached transport and
  reports the full missing runtime-member set when incomplete.
- Interrupted model/reference/mask members resume only under the exact
  validator/range contract. Wrong status, redirect, validator, range, declared
  length, stored size/hash, truncation, trailing/second frame, and
  reconstructed size/hash fail closed.
- Instrumented tests prove one decode and one destination write for each
  compressed runtime member, no decoded intermediate transport tree, and no
  second decoded-member copy.
- Path substitution, symlink, hardlink, hostile cache entry, and cache/data
  root shape tests fail closed without changing installed state.
- Failure injection covers durable cache-member publication and the existing
  runtime component/profile/active transitions. Every failure preserves an
  active SNV and prior runtime; retry converges without exposing a partial
  profile.
- Concurrent first runtime sync has one nonblocking cache-lock owner and one
  typed loser. A cache-lock loser never enters installation.
- Static production-contract tests bind the exact checked release profile,
  its exact checked transport manifest, and ten public members without network
  or production-payload reads.
- Resource tests bound read buffers and heap independently of the 692 MB
  production transport. Each HTTP response byte is consumed once. On a
  successful install path, each completed cached member is read once per
  attempt through its held descriptor and each reconstructed byte is written
  once to its final staging destination while hashing the same streaming bytes
  as they pass. After a content failure, recovery may reread completed members
  once to retain authenticated good files and discard only corrupt files.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Runtime primitive first or whole-product CLI first.** A combined command
   would mix new transport security and large-file installation with public
   JSON, operation locking, status truth tables, and breaking grammar changes.
   Establish the runtime primitive here; compose it with the proven SNV
   primitive in the next ticket.
2. **Reuse the local transport or invent direct asset installation.** The
   published manifest plus its nine members already bind stored and
   reconstructed identities. Reuse that contract, but join decode to
   installation staging so large decoded members are neither materialized nor
   copied twice.
3. **Share the downloader or fork it.** The existing SNV client already owns
   HTTPS origin, redirect, timeout, resume, cache-shape, and response rules.
   Generalize that machinery behind closed profiles rather than duplicating a
   security-sensitive implementation.
4. **Hold the install lock during download or only during installation.**
   Holding it across a 692 MB transfer would make local status and unrelated
   installation appear blocked. Hold only the runtime cache lock during
   download, then acquire the existing install lock for durable publication.
5. **Production download in CI or miniature contract tests.** Repeated large
   downloads test GitHub availability and waste bandwidth. Pin production
   identity statically and prove network, resume, decode, atomicity, and
   resources with exact miniature transports. A clean-machine production
   qualification follows the combined CLI.

## Dependencies

- Ticket 034, complete: immutable public `runtime-grch38-v1`.
- Existing pinned SNV downloader/cache, deterministic runtime transport,
  atomic runtime installer, and installed-profile consumption.

## Notes

- The release profile downloads ten files:
  `runtime-transport.json` plus the nine stored members listed inside that
  manifest. The manifest describes the nine installed transport members and
  their reconstructed identities. `runtime-release-profile.json`,
  `SHA256SUMS`, preferred-source archives, and the standalone GPL text are
  publication/source-compliance assets and are not installed runtime members.
- The public release is an observed immutable input. This ticket performs no
  GitHub mutation and publishes no raw Zenodo, NCBI, or GENCODE input.
- The next ticket owns exact top-level `pangopup sync`/`status` JSON, complete
  offline missing-state aggregation across both releases, root-scoped
  provisioning state, nested-command removal, and clean composition of the two
  typed sync outcomes.

## Coordinator Authorship

Coordinator: `/root`

Revised after independent review separated the security/resource-sensitive
runtime primitive from public combined CLI orchestration. The coordinator does
not implement or approve this ticket.

## Independent Ticket Review

Reviewer: `/root/ticket035_design_review`

First review: REJECT.

- The combined ticket did not define how status could observe long downloads
  that hold only cache locks, nor lock ordering or ready-state behavior.
- Its promised stable JSON lacked an exact state/output truth table, including
  partial success and concurrent-loss behavior.
- Sequential offline sync could not report both release families' missing
  members as required.
- Downloader generalization, direct streaming installation, combined
  orchestration/status, breaking grammar, and production authority were too
  large for one independently reviewable ticket.

The coordinator accepted the findings and split at the typed operation
boundary. This ticket now owns only pinned runtime download, cache, one-pass
install, atomicity, resource behavior, and a typed library result. The next
ticket will define combined operation state, exact JSON/truth table, full
offline aggregation, and CLI migration over two established primitives.

Second review: REJECT.

- Cache-first installation necessarily consumes each response byte into cache
  and later reads each completed cache member once during installation; the
  draft incorrectly prohibited that required second source read.
- Reconstructed identities live in `runtime-transport.json`, not
  `runtime-release-profile.json`, so one checked profile alone could not
  statically bind all claimed facts.
- The typed outcome's `path` field was ambiguous.

The coordinator corrected the resource contract to one response consumption,
one held cached-member read per install attempt, and one reconstructed
destination write. Both exact published small manifests are now checked
authority with an explicit byte-hash relationship, and `path` names the
activated immutable runtime-profile directory.

Third review: REJECT.

- The draft called all ten downloaded files entries in
  `runtime-transport.json`; in fact the release profile downloads that
  manifest plus the manifest's nine members.

The coordinator corrected the inventory throughout: ten release-profile
download entries, comprising the exact transport manifest and its nine stored
members; the inner manifest alone owns the nine reconstructed-member facts.

Final re-review: ACCEPT. The reviewer confirmed that the ticket is now
appropriately bounded, distinguishes the ten release downloads from the inner
nine-member transport, defines an achievable one-pass install/resource
contract, preserves atomic state, and leaves combined CLI/status semantics to
the following ticket.

## Implementation Evidence

Developer: `/root/ticket035_implementation`

- Checked in the exact 8,043-byte public
  `release-profiles/runtime-release-profile.json`
  (`sha256:d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e`)
  and exact 3,179-byte public `release-profiles/runtime-transport.json`
  (`sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`).
  A static closed-contract test binds the release identity, pinned target,
  ten outer download entries, exact inner-manifest byte identity, nine inner
  members, stored/reconstructed identities, and canonical ordering without
  network or production-payload reads.
- Added typed `sync_runtime_assets` and `RuntimeSyncOutcome`. It reuses the
  existing bounded rustls client, allowlisted redirects, strong-ETag resume,
  private descriptor-relative cache, exact size/SHA-256 checks, nonblocking
  cache lock, offline missing aggregation, and cache publication. It never
  queries release discovery metadata or accepts a URL.
- Refactored the established runtime installer at its staging-source seam.
  Local explicit inputs retain their prior behavior; authenticated cached
  transport members now stream directly into the same model/reference/mask
  staging directories. Raw bounded members are retained from their single
  authenticated read, and each Zstandard frame is read/decompressed once to
  its final staging destination while stored and reconstructed identities are
  checked. No decoded intermediate transport tree or second decoded-member
  copy exists.
- The runtime cache lock is held before the existing data-root install lock,
  but no network response remains open and the data-root lock is never held
  during download. Active runtime reuse performs zero network work. Runtime
  compatibility errors preserve a complete cache; proven cached-content
  corruption is evicted.
- Added miniature/offline coverage for all ten release downloads, direct
  atomic installation beside an active SNV bundle, online and offline reuse,
  zero offline requests, exact byte counters, absence of a decoded cache tree,
  and a bounded error naming every missing release member. Existing generic
  sync tests continue to cover real loopback HTTP, redirect/status/length
  matrices, exact resume validators/ranges, timeouts, hostile entries,
  symlink/hardlink/path replacement, nonblocking concurrency, failure
  injection, atomic publication, and constant buffers; existing runtime
  transport/installer tests cover truncation, trailing/second frames,
  reconstructed hash/size failures, transition failures, prior-active
  preservation, and retry convergence.
- Updated `AGENTS.md`, `README.md`, `release-profiles/README.md`,
  `architecture/README.md`, `architecture/delivery.md`,
  `architecture/runtime-data.md`, `planning/frontier.md`, and
  `planning/faq.md` to distinguish the shipped typed runtime primitive from
  the still-future combined CLI.
- Focused `cargo test -p pangopup-assets`: 82 passed.
- `make lint`: passed (the existing dependency-duplicate reports remain
  configured warnings).
- `make test`: passed across the workspace.
- `make spec`: 227 passed, 6 skipped.

First code-review remediation:

- Production cached installation now requires the cached inner manifest bytes
  to equal the exact compiled inner authority. The compiled-authority loader
  also proves the exact outer profile and its ten facts agree with that inner
  manifest. A same-size canonical substitution that rebinds
  `mask-NOTICE` is rejected before staging.
- A content failure no longer deletes the whole published runtime transport.
  Recovery reauthenticates each completed member through held descriptors,
  moves every good member back into resumable complete cache state, and drops
  only bad content. A miniature corruption test preserves nine good members
  including `reference.pgr.zst`, downloads one bad notice on retry, and
  installs successfully.
- Runtime fast reuse now cheaply opens the currently active SNV bundle and
  compares its actual bundle identity with the runtime release before trusting
  the runtime receipt. A replacement-active-SNV regression proves stale reuse
  is rejected without a score scan.
- Direct-install instrumentation records the actual compressed bytes consumed
  and reconstructed bytes produced by each successful frame; the test proves
  exactly one direct seam event with full stored/decode/final-write byte
  counts for model, reference, and mask. A separate runtime-lock test proves a
  nonblocking cache-lock loser invokes no installer operation.
- Stale current-state claims identified by review were corrected.
  `mask-NOTICE` remains one exact authenticated cached member and explicit
  unpack output. The pre-existing frozen installed mask component remains
  `domains.pgm`-only, matching explicit local installation and its receipt;
  this ticket does not expand that topology.
- Post-remediation `make lint`, `make test`, and `make spec` all pass;
  `make spec` remains 227 passed and 6 skipped.

Second code-review remediation:

- Direct installation now wraps the actual final destination file in an
  independent counting `Write` implementation. The seam evidence compares
  actual successful destination writes with independently observed decoded
  bytes and stored cached reads; an omitted or duplicate write path changes
  that counter and fails the three-member assertion.
- Documentation and the resource checklist now limit the one-cache-read claim
  to a successful install path. Content-failure recovery honestly performs one
  additional authenticated read of each completed member so it can preserve
  good content.
- Recovery retains each authenticated cache descriptor and its metadata across
  the pathname rename, reopens the destination entry, and requires matching
  device, inode, length, and link state before keeping it. A deterministic
  regression replaces the source pathname after hash authentication and
  before rename; recovery rejects the substituted inode with
  `ASSET_STATE_INVALID`.
- The seam regression exposed that post-publication recovery must open the new
  pathname-visible `members` directory rather than reuse the held descriptor
  that was renamed to `transport`; that directory selection is corrected.
- Post-remediation `make lint` and `make test` pass; the full workspace test
  gate reports 337 passing tests. `make spec` remains 227 passed and 6
  skipped.

## Adversarial Code Review

Reviewer: `/root/ticket035_code_review`

First review: REJECT.

- Production install trusted a canonical cached inner manifest without binding
  it to the exact compiled outer/inner authorities.
- One corrupt member deleted the whole cached transport instead of preserving
  the other completed downloads.
- Fast reuse trusted a stored SNV identity without checking the currently
  active SNV bundle.
- The new seam lacked independent read/decode/write and cache-lock-loss
  instrumentation, and current docs retained contradictory future claims.

Second review: REJECT.

- The final-write counter copied the decoded count instead of measuring actual
  destination writes, and the one-read claim omitted recovery rereads.
- Recovery dropped its authenticated descriptor before renaming by pathname,
  leaving a replacement race.

Final re-review: ACCEPT. The reviewer verified exact compiled outer/inner
authority binding, notice-rebinding rejection, nine-member salvage and
one-member retry, live active-SNV validation, zero installer calls for a
cache-lock loser, independent stored/decode/actual-write counters, honest
failure reread documentation, descriptor-held rename identity checks, and the
fail-closed substitution regression. Focused `pangopup-assets` tests passed
all 82 cases; `git diff --check` passed. No review-time edit or external
mutation occurred.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: `/root`

The final independently accepted diff passes `git diff --check`, `make lint`,
`make test` (337 tests), and `make spec` (227 passed, 6 skipped). The
coordinator confirmed that only the accepted asset-layer primitive, exact two
small release authorities, tests, and named current-state documentation
changed; the CLI grammar and external state remain untouched. Current docs
consistently distinguish shipped typed runtime sync from the following
combined CLI/status outcome.
