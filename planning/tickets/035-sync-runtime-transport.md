# 035 — Sync the pinned runtime transport directly into installation

Status: ready

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
  production transport. Each HTTP response byte is consumed once, each
  completed cached member is read once per install attempt through its held
  descriptor, and each reconstructed byte is written once to its final staging
  destination while hashing the same streaming bytes as they pass.
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

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
