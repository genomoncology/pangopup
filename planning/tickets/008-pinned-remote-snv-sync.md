# 008 — Sync the pinned public SNV transport into local installation

Status: complete

## Why

Ticket 007 published the exact Ticket 005 transport as the immutable public
`snv-grch38-v1` GitHub release. Ticket 006 already installs that transport
safely when its five files are present locally. The remaining gap is a normal
user command that obtains those exact files without asking the user to run five
download commands by hand.

This slice adds a pinned, resumable downloader in front of the shipped local
installer. It does not select “latest,” rebuild or semantically rescan the
production index, change lookup behavior, add automatic network access to
`lookup`, or introduce the model/HTTP/container work.

## Scope

- Add this Linux CLI command:

  ```text
  pangopup assets sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
  ```

  With no active production bundle, online sync downloads the exact five
  transport members in checked
  `release-profiles/snv-grch38-v1.json`, verifies each member's declared size
  and SHA-256, and calls the existing `install_transport` boundary. A successful
  command emits one compact JSON object on stdout containing `status`,
  `profile`, `bundle_id`, `transport_id`, `path`, `downloaded_bytes`, and
  `resumed_bytes`, followed by one LF. It emits no progress chatter on stdout.
  `status` is `installed` when a bundle was newly published and `reused` when
  the exact active bundle or an existing immutable installed bundle was reused.
- Lexically validate every present explicit/cache environment path, then perform
  the existing cheap active-install validation before requiring a cache root or
  creating an HTTP client. If the active bundle and transport identities equal
  the pinned profile, return `reused` with both byte counters zero. This path
  performs no network or cache-filesystem operation and does not open or hash
  any cache member. An invalid present `--cache-dir`,
  `PANGOPUP_CACHE_DIR`, `XDG_CACHE_HOME`, or `HOME` still fails on this path;
  absence of every cache-root source does not matter when the exact active
  bundle is already usable.
- Resolve the cache root in this strict order:

  ```text
  --cache-dir
  PANGOPUP_CACHE_DIR
  XDG_CACHE_HOME/pangopup
  HOME/.cache/pangopup
  ```

  A present empty, relative, or non-UTF-8 value fails rather than falling
  through. Cache data is disposable and never becomes the authoritative lookup
  path. Durable installed data continues to use the existing independent XDG
  data-root rules.
- Store this profile under a private cache namespace derived from the complete
  profile identity, not merely its human tag. The profile identity is the
  checked production profile's exact SHA-256. Keep partials/sidecars and
  individually verified completed members in distinct private working
  namespaces. Once all five completed members exist, sync the files and
  directory, atomically rename that exact five-member completed directory to
  the absent authoritative cache name `transport`, and sync its parent. Only
  that published `transport` directory may be passed to offline mode or the
  installer. A crash before the one directory rename never presents a partial
  or five-file-looking working directory as a complete transport.
- Add one nonblocking, no-follow Linux cache lock. A sync owns that lock while it
  inspects or changes this profile's cache. It may keep the lock while calling
  the installer because the existing local installer never acquires a cache
  lock; the global order is therefore cache lock then data install lock. A
  competing sync fails with the existing `ASSET_LOCKED` class and never waits.
- Treat the checked production release profile as executable input, using the
  existing `pangopup-assets` parser and its exact compiled-in bytes and digest.
  Do not fetch a second profile, call GitHub's API, follow a release page, accept
  arbitrary URLs, or derive a tag named `latest`. Production accepts only the
  literal HTTPS member URLs already reviewed in that profile.
- Use an in-process Rust HTTP client with TLS certificate verification and no
  runtime dependency on `curl`, `gh`, Python, or a shell. Disable the client's
  automatic redirect policy and implement a bounded redirect loop. Production
  accepts only HTTPS and only the original `github.com` host plus the observed
  `release-assets.githubusercontent.com` download host; reject userinfo,
  fragments, HTTPS-to-HTTP downgrade, missing/relative `Location`, every other
  host, and more than five redirects. Never send credentials or ambient GitHub
  authentication. Send `Accept-Encoding: identity`, disable transparent content
  decoding, reject every non-identity `Content-Encoding`, and propagate the
  same `Range` and `If-Range` headers through every allowed redirect.
- Bound hostile or stalled peers with a 30-second production connect timeout,
  a 30-second response-header timeout, and a 120-second body read-idle timeout.
  There is no whole-transfer deadline for a valid progressing large body. Tests
  inject short bounds and cover a stalled connect, accepted connection with no
  headers, and headers followed by a stalled body. Timeout errors are typed and
  sanitized and always release the cache lock.
- Download sequentially in profile order. This intentionally caps open files,
  sockets, write buffers, and concurrent bandwidth. Stream through a bounded
  buffer; never materialize an asset in heap. Require an exact successful status,
  declared response length when present, and no bytes beyond the profile size.
  While receiving a fresh member, hash the stream once and count bytes. Only an
  exact size/digest becomes a read-only completed cache member by atomic
  no-replace rename. A wrong digest, short body, long body, I/O failure, or
  malformed response fails without changing an installed bundle.
- Preserve interrupted work as a private regular partial plus an immutable,
  canonical, bounded resume record. The record binds schema, profile digest,
  original URL, asset name, expected size/digest, and the final response's
  strong ETag; it deliberately does **not** contain a changing completed-byte
  count. After receiving and validating response headers, write/sync/publish the
  sidecar first, sync its directory, then create the no-follow partial and
  stream the body. The bounded regular partial's observed file length is the
  authoritative resume offset after a process or machine interruption. On
  recovery, require `0 < length < expected_size`, hash exactly that prefix once,
  and never infer bytes beyond its actual length. A sidecar without a partial
  is harmless fresh state; a partial without its exact sidecar, an oversized
  partial, or invalid/noncanonical metadata is discarded safely. Promotion
  closes and syncs the exact-size/hash file before atomically moving it into the
  completed-members working directory and syncing that directory.
- The resume response matrix is closed:

  - A fresh request accepts only final `200`, identity encoding, and a strong
    ETag, then streams from byte zero.
  - A resume sends `Range: bytes=<N>-` plus the recorded strong `If-Range` to
    the original reviewed URL. Final `206` is appendable only when its strong
    ETag exactly matches and `Content-Range` is exactly
    `bytes <N>-<expected-1>/<expected>`.
  - Final `200` means restart, whether its required strong ETag is unchanged or
    changed. Stream it from byte zero into a distinct fresh file/sidecar and
    preserve the old prefix until the fresh body passes exact size/hash; never
    append its bytes to the old partial.
  - Missing/weak ETag, `416`, malformed/missing range metadata on `206`, or any
    other final status fails without appending. There is no automatic retry
    after a completed body has the wrong hash.

  `downloaded_bytes` counts response-body bytes received from the network in
  this invocation, including a resumed suffix. `resumed_bytes` counts only
  verified prefix bytes reused in a successful final member; a `200` restart,
  completed-cache reuse, or active-install reuse contributes zero.
- `--offline` performs the same active-install fast path, then forbids every
  network operation. It may pass a closed completed cached transport to the
  existing installer. If any cache member is missing or partial, fail with a
  typed `ASSETS_MISSING` error that names the profile and each incomplete asset
  with present and expected bytes. Offline mode never claims that cache metadata
  alone certifies bytes: the installer still performs its existing one-stream
  transport/reconstruction verification before publication.
- Completed cache members are hints, not authority. The installer remains the
  final integrity and semantic-publication boundary. If it returns one of the
  cache-content error kinds (`MANIFEST_INVALID`, `TRANSPORT_INCOMPATIBLE`,
  `PART_SET_INVALID`, `TRANSPORT_HASH_MISMATCH`, `COMPRESSION_INVALID`, or
  `BUNDLE_INVALID`), sync attempts to remove the whole published cached
  `transport` directory through its held private cache root before returning.
  Successful eviction returns the original typed installer failure. If eviction
  itself fails, return that same original error kind with sanitized cleanup
  context and truthfully leave the cache for a later invocation to retry; never
  hide cleanup failure or claim removal. Sync never guesses an offending
  filename from error prose and never removes or replaces a previously
  installed bundle. Data-root, lock, output, or unrelated I/O errors leave the
  completed cache intact.
- Add a private in-module injected sync contract and transport client compiled
  only under `cfg(test)`. Production and feature-enabled downstream callers
  cannot supply a profile, URL, host allowlist, or insecure transport. Library
  tests run against a bounded local scripted HTTP server and miniature
  transport. A CLI adapter test invokes a
  parameterized in-process runner to prove exact success JSON without adding a
  production flag or environment escape hatch. The ordinary production binary
  remains closed to the compiled profile; executable mustmatch covers exact
  grammar and offline/no-network failures, not a local-server happy path. No
  normal test downloads production assets or contacts GitHub.
- Update behavior and current/future wording in all of:
  `README.md`, `AGENTS.md`, `architecture/README.md`,
  `architecture/delivery.md`, `architecture/runtime-data.md`,
  `architecture/decisions/0005-automatic-version-locked-assets.md`,
  `planning/frontier.md`, `planning/faq.md`, `spec/cli.md`, and add
  `spec/remote-assets.md`. Add retained implementation/test evidence to
  `planning/artifacts/008-pinned-remote-snv-sync.md`.

Explicit exclusions:

- no live production download in tests, development, review, or final gates;
- no full-payload verifier, full semantic rescan, production index rebuild, or
  production cache population;
- no parallel/multipart download, mirror selection, retry loop, bandwidth
  scheduler, progress bar, persistent download-status subcommand, cache garbage
  collection, repair, rollback, or signature framework;
- no implicit download by `lookup`, startup, or status;
- no model, reference, mask, non-SNV inference/cache, HTTP service, Docker,
  macOS, or Windows behavior.

## Success Checklist

- An in-library test against the miniature scripted server downloads
  the checked test transport and installs it through the shipped installer. An
  in-process CLI adapter test emits the exact compact success JSON for that
  result. Lookup through the resulting active bundle matches the existing
  fixture. The executable spec proves the public command grammar and offline
  failures without a network or test-only production-binary override.
- A second sync of the exact active production/test profile proves zero HTTP
  requests, zero cache-member opens, zero downloaded/resumed bytes, and returns
  `reused` through the cheap active validation path.
- A killed/truncated miniature transfer leaves a bounded valid partial whose
  length is authoritative; the next sync uses exact `Range`/`If-Range`, receives
  a valid `206`, and completes with byte-identical content. Fault tests interrupt
  every sidecar-create/sync, partial-create/write/sync, completed-member
  promotion, and final-directory publication boundary. Tests cover redirect on
  both fresh and resumed requests.
- Inside-out tests reject weak/missing ETag and changed ETag on `206`, safely
  restart from byte zero on a strong-ETag `200`, and reject wrong or missing
  `Content-Range`, `416`, short/long bodies, wrong SHA-256, wrong declared size,
  extra response bytes, transparent/non-identity encoding, redirect loops, too
  many redirects, non-HTTPS/unknown production hosts, symlink roots/members,
  nonregular members, malformed/oversized resume JSON, invalid partial lengths,
  concurrent sync, stalled connect/headers/body, and cache/data path errors
  without changing a valid active installation.
- Offline tests prove exact-active reuse with zero network and zero cache I/O,
  complete-cache installation, and deterministic reporting of every
  missing/partial member without a network request. Existing cheap validation
  of the authoritative data-root installation remains expected I/O.
- A test audit proves the fresh body is read once, the resumed prefix is read
  once, no full asset is held in heap, members are downloaded sequentially, and
  `install_transport`—not a duplicated unpacker—publishes the bundle.
- The implementation artifact records the selected HTTP/TLS crates and locked
  versions, complete release-profile identity, miniature transfer sizes,
  request counts and exact byte counters for fresh/resumed/restarted/reused/
  offline cases, configured timeout values, and maximum streaming buffer size.
  It also records a release build's startup/active-reuse latency and heap
  allocation or RSS evidence on the miniature fixture; it makes no claim about
  production network speed.
- `make lint`, `make test`, and `make spec` pass without network access.

## Decisions

### 1. The binary-pinned profile is the network root of trust

Consideration: sync must obtain the published release reproducibly without
turning GitHub's current state into a scoring decision.

Options: ask the GitHub API for the latest release; download and trust a remote
manifest; accept a user URL; compile the already reviewed exact profile into
the runtime.

Trade-offs: remote discovery is more flexible but mutable and adds API/schema
surface. A compiled profile requires a new Pangopup release to select new data,
which is exactly the desired compatibility control.

Decision: compile and validate the checked `snv-grch38-v1` profile and use only
its five literal URLs, sizes, and hashes. Do not fetch or discover metadata.

### 2. Resume uses HTTP validators, not length alone

Consideration: appending the wrong remote bytes to a preserved prefix can waste
the full transfer and can create a mixed-release cache.

Options: always restart; resume from length only; bind the prefix to a strong
ETag and require an exact ranged response.

Trade-offs: restarting is simplest but discards up to almost 2 GB of useful
work. Length-only resume is unsafe. Strong-validator resume adds a small sidecar
and strict response handling.

Decision: publish an immutable header-derived sidecar before body writes and
make the bounded regular partial length authoritative. Resume only on an exact
`206`/ETag/Content-Range match. Treat a strong-ETag complete `200` as a fresh
restart into a separate file, never as append data, and preserve the old prefix
until the replacement is fully verified.

### 3. Downloads are sequential and bounded

Consideration: the production transport has only five members and two large
parts. Query performance is the product priority; sync is a one-time setup path.

Options: parallel multipart/range download; one concurrent request per asset;
one sequential streaming request.

Trade-offs: parallel download may reduce elapsed transfer on some links but
multiplies sockets, file-state transitions, retry cases, and server assumptions.
Sequential streaming has bounded resources and a much smaller correctness
surface.

Decision: download one member at a time in profile order with a fixed bounded
buffer. Parallelism remains a measured future option, not a v1 requirement.

### 4. Cache and installed data have different authority

Consideration: users should be able to clear XDG cache without breaking lookup,
and an incomplete download must never look installed.

Options: mmap directly from cache; move the transport into data storage; retain
transport in cache and call the existing installer.

Trade-offs: direct cache lookup is smaller on disk but makes disposable files
authoritative and keeps compression on the query path. Moving destroys resumable
inputs. Retaining both costs disk during installation but preserves existing
atomic semantics and allows later cache deletion.

Decision: cache only transport/resume material; individually verified working
members become a visible five-member cache transport in one atomic directory
rename, and the existing receipt-bound XDG data bundle remains authoritative.
Sync calls `install_transport` unchanged.

### 5. Explicit sync owns network behavior

Consideration: scripts, air-gapped machines, and lookup latency must not depend
on network availability.

Options: auto-download from every lookup/startup; auto-download only from a
future service; explicit provisioning command.

Trade-offs: implicit download is convenient on a blank machine but makes an
otherwise local command slow and failure-prone. Explicit sync is observable and
composable with containers and external service managers.

Decision: only `pangopup assets sync` uses the network in this slice. Lookup and
status remain local. A future service may explicitly invoke the shared sync
adapter during provisioning after its own reviewed contract.

## Dependencies

- Ticket 005 deterministic SNV transport.
- Ticket 006 Linux/XDG local installation, active discovery, locking, and fast
  regression corpus.
- Ticket 007 checked release profile and immutable public `snv-grch38-v1`
  release.

## Notes

- The exact production release-profile whole-file identity is
  `sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6`
  over 2,821 bytes. Recheck this against the checked file in focused tests; do
  not trust this prose if it disagrees with executable production validation.
- The two production payload members are 1,000,000,000 and 931,687,706 bytes.
  They are already preserved and published. Do not read, copy, download, hash,
  rebuild, or verify them while implementing or testing this ticket.
- An unauthenticated request to a reviewed GitHub release URL currently returns
  a redirect to `release-assets.githubusercontent.com`; the final response
  advertises byte ranges and a strong ETag. That observation motivates the
  resume protocol but is not a test dependency or permission to broaden the
  production host allowlist silently.
- Use the same miniature bundle/transport builders already used by
  `spec/local-assets.md` and Rust integration tests. The scripted server must be
  implemented within the Rust test process; no Python or shell server becomes a
  runtime dependency.
- Keep HTTP/TLS code below the CLI adapter, preferably in a new focused module
  in `pangopup-assets`. `pangopup-core` and `pangopup-index` remain network- and
  path-discovery-free. Do not move builder-only transport codecs into the CLI.
- Treat all HTTP headers, redirects, body lengths, cache metadata, directory
  entries, and filesystem state as hostile input. Errors must be typed and
  sanitized; never include a signed redirect URL or raw response body in user
  output.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-07-23

Drafted from the observed immutable Ticket 007 release, the shipped Ticket 006
installer contract, ADR 0005, and the rolling frontier. No product code was
changed while authoring this ticket.

## Independent Ticket Review

Reviewer: Codex sub-agent `ticket_008_design_review` (independent, read-only)

Initial review: changes requested, 2026-07-23.

- **Crash resume blocker:** a mutable completed-byte count could disagree with
  the partial after interruption. Resolved by an immutable, header-derived
  sidecar published before body writes, authoritative bounded file length, and
  explicit durability/fault boundaries.
- **Test-boundary blocker:** the production-only CLI could not point at the
  miniature HTTP server claimed by the checklist. Resolved by assigning HTTP
  behavior to injected library integration tests, exact success rendering to a
  parameterized CLI adapter test, and only grammar/offline failure behavior to
  the ordinary executable spec.
- **Wire ambiguity:** `200`, changed validators, content encoding, redirects,
  and byte counters were inconsistent or unstated. Resolved with the closed
  response matrix, distinct restart file, identity encoding, redirect header
  propagation, and exact counter definitions.
- **Network liveness:** hostile peers had no finite bound. Resolved with exact
  connect, response-header, and read-idle production timeouts plus injected
  stalled-peer tests.
- **Cache publication:** per-member completion did not prove a closed visible
  transport. Resolved with distinct working namespaces and one synced atomic
  directory publication; cache-content rejection removes the whole published
  cache transport without parsing error prose.
- **Fast-path path validation:** strict cache-path errors conflicted with
  zero-cache active reuse. Resolved by lexically validating every present path
  before active reuse while requiring and touching no cache root on that path.

Re-review: approved after one final wording correction, 2026-07-23. The reviewer
identified that “zero-I/O active reuse” contradicted the existing receipt,
manifest, and cheap index-header validation. The checklist now requires zero
network and zero cache I/O while explicitly retaining authoritative data-root
validation. The injected client may simulate connect timeout deterministically;
the loopback scripted server owns real header/body stalls. No unroutable
external host enters tests.

The reviewer approved the revised ticket for development.

Narrow contract re-review: approved, 2026-07-23. During code re-review the
coordinator corrected the unconditional cache-eviction promise: eviction is
attempted, and failure preserves the original installer error kind while adding
sanitized cleanup context and leaving the cache for later retry. The same ticket
reviewer found this truthful failure rule consistent with the accepted cache-
authority design and approved implementation remediation to resume.

## Implementation Evidence

Developer: Codex sub-agent `ticket_008_developer` (independent; no commit or
push), 2026-07-23.

- Added the production-only compiled-profile sync boundary in
  `pangopup-assets`, a rustls-backed ureq 3.2.0 client, strict redirect/encoding
  handling, phase timeouts, sequential bounded streaming, private cache
  locking, canonical strong-ETag resume state, no-replace member/directory
  publication, offline recovery, and installer-owned final validation.
- Added `pangopup assets sync` grammar, strict cache-root discovery, stable
  compact success JSON, exact typed failure mapping, and a private injected CLI
  runner test. Lookup/status retain their no-network behavior.
- Added in-library unit/fault tests and a real loopback HTTP streaming/stall
  test that exercise the private miniature contract through the real installer.
  The first adversarial review found that exposing that seam under the existing
  downstream `test-read-audit` feature would let a feature-enabled caller
  supply arbitrary profiles, hosts, and insecure transport. The public seam and
  its external consumer were removed; only private `cfg(test)` injection
  remains. No production binary flag, environment override, profile injection,
  insecure transport, or GitHub contact was added.
- Replaced pathname-based cache mutation with held directory descriptors,
  component-by-component `openat2` no-symlink traversal, mode-0700 `mkdirat`,
  and descriptor-relative create/open/rename/unlink. Tests reject final and
  intermediate cache-root symlinks and member symlinks without touching their
  targets. Installer cache-content failures evict through the held descriptor;
  a cleanup failure preserves the original installer error kind and adds
  sanitized cleanup context.
- Closed the response and recovery matrix with exact strong-ETag grammar,
  changed/missing/weak validators on actual `206` responses, missing/wrong
  `Content-Range`, `416`, fresh and resumed redirects, malformed/noncanonical
  resume JSON, invalid partial lengths, and deterministic connect-timeout
  classification. Discriminating controls prove that wrong declared lengths
  neither promote a fresh member nor append to a retained resume prefix, that a
  timeout while `sync_with` owns the cache lock releases it before a successful
  retry, and that same-root cache and install-lock path failures preserve a
  valid nonmatching active receipt and bundle. Byte-read audits cover fresh
  bodies, resumed prefixes, and suffixes.
- Retained exact dependency, profile, transfer-counter, timeout, fault-window,
  buffer, release-mode latency, and RSS evidence in
  `planning/artifacts/008-pinned-remote-snv-sync.md`. The measured miniature
  transport is 7,948 bytes over four members; fresh sync made four sequential
  requests, active/offline reuse made zero, and the release-mode active reuse
  call measured 375,487 ns in the retained final run. These are local miniature
  measurements, not production-network claims.
- Regenerated the checked miniature regression fixture with its documented Rust
  tool because Ticket 008 changed the builder source identity. The request TSV,
  `scores.pgi`, notice, and all expected query records were unchanged; expected
  JSON is byte-identical after normalizing only the derived bundle ID. The final
  miniature bundle is
  `sha256:9c03f5c7397f2b6203d23a2bc373ddfc345aac1008fe3f77e6cdc0d2715993bf`
  with builder source
  `sha256:28d863ac7997ac15cc1ff7e2943d8a2148678a6420c2589701ab19d40dd42e56`.
- Updated every named durable/user document and added executable
  `spec/remote-assets.md`. A stale-claim scan found no remaining statement that
  pinned SNV sync is future or unimplemented; model/HTTP/container/progress
  work remains correctly future.
- Focused tests passed:
  `cargo test --locked -p pangopup-assets sync::tests` (23 tests) and
  `cargo test --locked -p pangopup-build --test snv_regression_fixture`.
- Developer gate passed after the final code/doc diff: `make lint`, `make test`,
  and `make spec`. Normal gates used only checked miniature fixtures and
  isolated local files; they did not read, hash, rebuild, verify, or download
  production payload members.

No reviewed-scope deviation is known. The implementation is ready for the
independent adversarial code review.

## Adversarial Code Review

Reviewer: Codex sub-agent `ticket_008_code_review` (independent, read-only)

Initial review: rejected, 2026-07-23.

- **Broad gate failure:** the fixture regeneration test exposed a changed
  builder provenance identity. Resolved by regenerating only the checked
  miniature fixture with its documented tool and proving its request corpus,
  index payload, notice, and normalized expected results unchanged.
- **Production caller boundary:** the feature-gated public test seam admitted
  arbitrary profiles, hosts, and insecure transport. Resolved by deleting the
  public exports and external consumer and retaining injection only inside the
  module's `cfg(test)` boundary.
- **Cache traversal and race safety:** cache directories and members were
  operated on by path after one metadata check. Resolved with descriptor-held,
  no-follow component traversal and descriptor-relative operations, plus
  intermediate/final/member symlink controls.
- **Cleanup error loss:** cache eviction errors were discarded. Resolved by
  preserving the installer's typed error kind and appending sanitized cleanup
  context; the injected cleanup-failure test proves the cache is retained.
- **ETag grammar:** the parser admitted an embedded quote. Resolved with the
  conservative ASCII grammar, including the valid empty strong tag.
- **Missing adversarial controls:** added the reviewer's complete `206`/range/
  `416`, redirect, resume-metadata/length, path-preservation, read-count,
  connect-timeout, and ETag cases.

Developer remediation is complete and the focused and broad gates are green.
Re-review by the same independent reviewer: rejected, 2026-07-23.

- **First-use cache race:** bootstrap/profile/member directories were created
  before the profile lock, so two first syncs could race into `ASSET_IO` instead
  of one owner and one nonblocking `ASSET_LOCKED` loser. Resolved with
  race-idempotent root/profile/lock creation, lock-before-working-state order,
  and a real concurrent absent-cache test: one owner installed while the loser
  returned `ASSET_LOCKED` without issuing a request.
- **Unbounded hostile enumeration:** closed-transport validation and cleanup
  collected every cache entry into a vector. Resolved with callback-based
  `RawDir` streaming for both validation and unlink, plus a 2,048-entry hostile
  transport test that leaves the directory empty without a name collection.
- **Cleanup wording mismatch:** the implementation correctly exposed eviction
  failure while the reviewed scope incorrectly promised unconditional removal.
  The coordinator amended Scope to require attempted eviction, original typed
  failure on success, and the same original kind plus sanitized cleanup context
  when eviction fails. The same ticket reviewer approved that narrow correction
  before this remediation resumed.

The second-review remediation is complete, focused and broad gates are green,
and the diff is ready for the same code reviewer. No code-review approval is
claimed.

Final re-review by the same independent reviewer: rejected, 2026-07-23.

- **Declared-length controls:** the general wire-failure matrix did not
  discriminate fresh promotion from resumed-prefix append. Resolved with
  separate wrong-`Content-Length` cases proving no completed-member promotion
  on fresh transfer and byte-identical retention of the old prefix on resume.
- **Timeout lock release:** timeout classification was covered without proving
  the cache lock owned by `sync_with` was released. Resolved by injecting a
  timeout through `sync_with`, proving no cache transport or active bundle was
  published, immediately reacquiring the cache lock nonblocking, and completing
  a successful retry.
- **Active preservation:** the prior cache-path case accidentally hit exact-
  active reuse and never exercised the broken cache. Resolved by installing a
  second fully valid miniature transport as the nonmatching active bundle. A
  broken cache root and then a complete cache plus hostile same-root install-
  lock path both fail while the original active receipt/path reopens unchanged.

The final-review remediation is complete. The focused sync module has 23
passing tests, fixture regeneration remains byte-exact apart from builder and
derived bundle identity, and the same reviewer approved the exact final diff on
2026-07-23. The reviewer independently verified the fresh/resumed declared-
length controls, timeout-through-`sync_with` lock release and retry, same-root
active preservation, first-use concurrency, descriptor-relative cache safety,
bounded directory streaming, cleanup failure visibility, ETag/range/redirect
behavior, private test seam, exact fixture provenance, every named document,
23 focused tests, byte-exact fixture regeneration, and all broad gates. No
remaining material finding or separately scoped issue was identified.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex (`/root`), 2026-07-23

- Re-ran `make lint`: pass. Cargo emitted only the existing informational
  semver-metadata warning for the pinned `zstd-sys` requirement.
- Re-ran `make test`: pass, including 45 `pangopup-assets` unit tests, the
  byte-exact SNV regression regeneration, transport/resource tests, and all
  workspace tests.
- Re-ran `make spec`: 111 passed.
- Re-ran `git diff --check`: pass.
- Searched `README.md`, `AGENTS.md`, `architecture/`, `planning/`,
  `release-profiles/`, and `spec/` for stale claims that pinned SNV remote sync
  is future or unimplemented: none found. Model/reference/mask sync, inference,
  HTTP, Docker, progress/status, and other explicitly excluded work remain
  correctly future.
- Confirmed the worktree contains only Ticket 008 implementation, tests,
  provenance-only miniature fixture regeneration, its named documentation, and
  retained evidence. No production asset was read and no network effect is part
  of this ticket.

Ticket 008 is complete and approved for commit/push, followed by the required
planning-ticket cleanup commit.
