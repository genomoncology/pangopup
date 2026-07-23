# 007 — Publish the certified GRCh38 SNV transport as a public data release

Status: publication-ready

## Why

Ticket 006 shipped safe Linux installation from an already available local
transport. The next dependency is a real, immutable public origin for those
exact bytes. GitHub reports `genomoncology/pangopup` as private with no
releases, so the remote-sync contract should not be written against guessed
URLs or mutable placeholders.

This slice prepares, reviews, and publishes the preserved Ticket 005 transport.
It first lands all code, metadata, CI repair, and the pinned public-hygiene
procedure in the still-private repository. Only after independent code review,
all local gates, a pushed commit, a green GitHub Actions run, and a retained
zero-finding audit of that exact commit may the coordinator make the repository
public and publish the release. It does not rebuild, recompress, semantically
verify, download, or install the production payload, and it adds no network
behavior to the Pangopup runtime.

## Scope

### Phase A — reviewed preparation while the repository stays private

- Add this deterministic maintenance command:

  ```text
  pangopup-build release prepare \
    --transport <TRANSPORT_DIR> \
    --receipt <PROOF_RECEIPT_JSON> \
    --output <ABSENT_DIR>
  ```

  It accepts only the pinned production contract below at the public CLI. The
  implementation exposes an internal contract-injection seam for miniature
  tests; test fixtures do not need production-sized sparse files. It creates
  one private same-filesystem stage, emits exactly `proof-receipt.json`,
  `release-profile.json`, `SHA256SUMS`, and `release-notes.md`, syncs them, and
  publishes the absent output directory atomically with no replacement.
  `proof-receipt.json` is a byte-identical bounded copy of the supplied receipt.
  It never copies, links, opens, hashes, or rewrites a payload part.
- Make `pangopup-assets` the sole parser/validator for transport release
  metadata. Expose a small typed inspection result from its existing bounded
  `inspect_transport` path: canonical transport bytes and identity, validated
  bundle-manifest and notice bytes/identities, score identity, compression
  identity, and ordered part descriptors. Inspection opens only
  `transport.json`, `bundle-manifest.json`, and `NOTICE`; parts are checked by
  directory entry, no-follow regular-file metadata, name, and size. Do not
  duplicate the private transport schema or validation in `pangopup-build`.
- Check the exact safe, bounded 2,194-byte production receipt into
  `release-profiles/proofs/snv-grch38-v1.json`. Its bytes must hash to
  `sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475`.
  The public CLI requires the caller-supplied receipt to be byte-identical to
  this reviewed file; the output copy is byte-identical too. This receipt has
  no local path, host identity, credential, or private source content and is
  part of the public release proof.
- Parse the proof receipt with duplicate-key rejection, `deny_unknown_fields`
  at every object level, JSON-safe integers, exact array lengths/order, and
  exact pinned values. Its framing is 2,193 bytes of RFC 8785 canonical JSON
  followed by exactly one LF byte, for 2,194 bytes total. Validate canonicality
  against the JSON prefix after removing that one required LF; reject a missing
  LF, CRLF, more than one LF, or any other leading/trailing whitespace. The
  closed v1 shape is:

  ```text
  root: schema, source, reference, bundle, transport, tool, verify
  source: archive_name, archive_size, archive_md5,
          observed_member_count, observed_members_sha256
  reference: assembly_accession, input_size, input_sha256,
             sequence_set_sha256
  bundle: bundle_id, builder_version, builder_source_sha256, manifest, members
  bundle.manifest: size, sha256
  bundle.members[0]: path=NOTICE, size, sha256
  bundle.members[1]: path=scores.pgi, size, sha256
  transport: transport_id, manifest, compressed, parts
  transport.manifest/compressed: size, sha256
  transport.parts[*]: ordinal, path, size, sha256
  tool: implementation_commit, encoder_crate, libzstd_version
  verify: bundle, transport (each a nonempty ordered string array)
  ```

  The checked-in production receipt is the complete LF-framed JSON example and
  exact production value contract: object keys, field types, array cardinalities
  and order, encoder values, verification argv values, and every
  source/reference/bundle/transport identity must match it. The parser belongs to
  `pangopup-assets`; the release preparer compares its typed result with the
  same crate's typed transport inspection result. Internal miniature tests use
  the same closed schema and canonical-JSON-plus-one-LF framing but inject a
  miniature contract containing their own exact receipt bytes, whole-file
  SHA-256, identities, encoder values, and argv arrays; they do not weaken
  production validation.
- Add a coordinator-only maintenance command used in Phase B:

  ```text
  pangopup-build release upload-asset \
    --transport <TRANSPORT_DIR> \
    --prepared <PREPARED_DIR> \
    --gh <ABSOLUTE_PINNED_GH_BINARY> \
    --release-id <POSITIVE_GITHUB_ID> \
    --asset <EXACT_ASSET_NAME>
  ```

  Resolve and validate `--gh` before touching either asset root. The production
  executable is the official GitHub CLI 2.45.0 Linux amd64 binary: release tag
  source commit `3ca179bcdeb46b5e54ddc6cad8feb6addf487d7c`, archive
  `gh_2.45.0_linux_amd64.tar.gz`, archive size 10,716,793 and
  `sha256:79e89a14af6fc69163aee00e764e86d5809d0c6c77e6f229aebe7a4ed115ee67`,
  extracted executable size 43,495,424 and
  `sha256:d4a46368912cfc7b9f0a897a613910e34562ef033fc6029e0bea52c43b440fa4`.
  Open its root/components without symlinks and the executable itself no-follow.
  Copy that held source into a Linux `memfd_create(MFD_CLOEXEC |
  MFD_ALLOW_SEALING)` snapshot, require the snapshot to have the pinned
  size/digest, then apply `F_SEAL_WRITE | F_SEAL_SHRINK | F_SEAL_GROW |
  F_SEAL_SEAL` and rewind it. Execute the sealed snapshot, never the source
  pathname or mutable source descriptor, with `execveat(..., AT_EMPTY_PATH)`.
  Keep the sealed descriptor valid through child exec and do not resolve
  `--gh` again. Its `api --input -` sends the supplied reader once with an
  explicit content length and performs no automatic POST retry.

  Accept only one of the exact eight production asset names and derive its
  source root/member from that closed set. Open the chosen transport or
  prepared root with Linux `openat2`/component walking that rejects a symlink
  root or component, then open the direct member dirfd-relative with
  `O_NOFOLLOW|O_CLOEXEC` before contract validation. Hold that one File for the
  rest of the operation; never resolve the selected pathname again. For either
  large payload part, acquire a Linux read lease on that held read-only file
  immediately after open and before `fstat` or any contract validation. An
  existing writer, unsupported lease/filesystem, lease error, or later lease-
  break notification fails closed; there is no lock-free fallback. On the
  supervising thread, establish the combined blocked-signal/`signalfd`
  supervision described below before acquiring the lease or creating any
  helper-owned worker thread.
  Acquire the lease first while `SIGIO` is blocked; Linux lease acquisition may
  install its own process owner. Then route notifications explicitly to the
  supervising thread with `F_SETOWN_EX(F_OWNER_TID, gettid())`, verify the owner
  with `F_GETOWN_EX`, and finally confirm `F_RDLCK`. The blocked signal covers
  the brief acquisition-to-routing window.
  Capture workers inherit the blocked mask. The parent retains the lease while
  the duplicate descriptor is child stdin and monitors the signal descriptor
  in the same wait loop as child exit and the monotonic deadline. A break
  notification starts immediate process-group kill and direct-child reap; the
  lease is released only afterward. Before accepting a successful child,
  nonblockingly drain pending lease notifications and require
  `F_GETLEASE == F_RDLCK`.

  Read `/proc/sys/fs/lease-break-time` before acquisition and fail if it is
  unavailable, invalid, or does not leave at least ten seconds for cleanup.
  Lease-break cleanup has a five-second monotonic ceiling, comfortably below
  that accepted kernel window. The command guarantees fail-closed behavior
  only while the kernel lease remains held: Linux may forcibly break a lease
  after its configured window, so failure to kill the upload child inside that
  window is a fatal condition and cannot honestly guarantee that later bytes
  stayed frozen. This is a race-safety boundary over the already certified
  retained inode, not a new content certification: the command still does not
  pre-read or hash a large part.

  Require a regular file and reviewed `fstat` size. Revalidate bounded
  transport metadata and the closed four-file prepared directory using
  dirfd-relative no-follow opens. If the selected asset is small metadata,
  copy its held bytes into a
  separate sealed `memfd`, validate that immutable snapshot, rewind it, and use
  that same sealed snapshot as child stdin. If it is a payload part, inspect
  only its leased held-fd metadata. Validation includes byte-exact proof/profile,
  regenerated notes/checksum bytes, and expected name/size/digest declarations.

  Spawn the pinned executable directly without a shell and with the selected
  stable descriptor as stdin. The exact argv is:

  ```text
  gh api
    https://uploads.github.com/repos/genomoncology/pangopup/releases/<id>/assets?name=<percent-encoded-reviewed-name>
    --method POST
    --header Accept:application/vnd.github+json
    --header X-GitHub-Api-Version:2022-11-28
    --header Content-Type:application/octet-stream
    --header Content-Length:<reviewed-size>
    --input -
    --jq {"name":.name,"size":.size,"state":.state,"digest":.digest}
  ```

  Set stdin to the selected sealed or leased stable descriptor, stdout/stderr
  to pipes, and no TTY. Clear the
  child environment, copy only `HOME`, `XDG_CONFIG_HOME`, `GH_CONFIG_DIR`,
  `GH_TOKEN`, `GITHUB_TOKEN`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `HTTPS_PROXY`,
  `NO_PROXY`, `LANG`, and `LC_ALL` when present, then force
  `GH_PROMPT_DISABLED=1`, `GH_PAGER=cat`, `PAGER=cat`, and `NO_COLOR=1`.
  `GH_DEBUG`, `DEBUG`, `GH_FORCE_TTY`, browser variables, and every unrelated
  variable are absent. Bound captured stdout and stderr to 64 KiB each and
  bound the complete child request to 21,600 seconds. Tests inject a shorter
  deadline. On output overflow, deadline, lease break, or any supervision
  error, send `SIGKILL` to the child's process group, close/drain bounded pipes,
  wait for and reap the direct child, then release held leases/descriptors and
  fail closed. Never claim to reap grandchildren: Linux process-group signaling
  and direct-child reaping are the enforced boundary. Never echo raw child
  output.

  The coordinator must not orphan an authenticated request when interrupted.
  Before acquiring a payload lease, and in all cases before spawning the child
  or any capture worker, the supervising thread saves its signal mask, blocks
  `SIGINT` and `SIGTERM` together with the lease's `SIGIO`, and creates one
  nonblocking `signalfd` monitoring all three. Capture workers inherit that
  mask. Drain and classify pending signals immediately before child spawn; an
  already-pending interrupt or lease break starts no child. Receipt of
  `SIGINT` or `SIGTERM` enters the same
  process-group `SIGKILL`, pipe cancellation, direct-child reap, and lease-
  release path as deadline failure, then restores the original mask and returns
  a sanitized nonzero interrupted result. Every normal/error return restores
  the original mask after child cleanup.

  Create the signal descriptor with `SFD_NONBLOCK | SFD_CLOEXEC`. As defense
  against abrupt parent death that cannot run cleanup, capture the expected
  parent PID in the parent before `fork`/`spawn` and pass that value into the
  child pre-exec closure. The child creates its own process group, calls
  `prctl(PR_SET_PDEATHSIG, SIGKILL)`, then requires `getppid()` still equals
  the parent-captured PID; mismatch exits without executing `gh`. After those
  steps succeed, restore the parent's original signal mask in the child
  immediately before `execveat`, so the sealed `gh` process does not inherit
  blocked `SIGINT`, `SIGTERM`, or `SIGIO`. This closes the parent-death race for
  the direct upload child. Do not
  overclaim that `PR_SET_PDEATHSIG` reaps or independently protects arbitrary
  grandchildren; orderly `SIGINT`/`SIGTERM` handling kills the complete child
  process group, while abrupt uncatchable parent death guarantees the direct
  child receives `SIGKILL`.

  Exit 0 plus exactly one closed reduced JSON object is required. Its name and
  size must equal the reviewed asset, state must be `uploaded`, and digest may
  be null only pending the mandatory draft-inventory poll; a present digest
  must exactly equal the reviewed selected-asset `sha256:` identity. A
  well-formed but different digest fails closed. Emit only Pangopup's own bounded JSON
  summary. One command performs one child invocation/request and never retries,
  deletes, or clobbers. Authentication remains owned by `gh`; Pangopup never
  reads, prints, persists, or places a token in argv.

  Tests inject a fake executable before production-binary enforcement and use
  miniature data without network. They assert exact argv/URL/header/length,
  one invocation, byte-exact stdin, bounded output/error behavior, closed
  response parsing, sanitized environment, and absence of source path or token
  in argv. A compiled fake-child race replaces/symlinks the `--gh` pathname
  after validation and proves the originally sealed executable runs; tests also
  reject executable-root/component/member symlinks. Asset race hooks cover
  transport metadata, prepared metadata, and payload
  members; symlink roots/components/members; and pathname replacement both
  after the held open but before validation and after validation. For small
  assets the sealed bytes remain authoritative in every accepted case. For
  payload parts, same-inode overwrite and truncate attempts made after lease
  acquisition must trigger cancellation, leave the writer blocked until the
  child is killed and reaped, and never produce an accepted upload result. A
  pre-existing writer must make lease acquisition fail before child spawn.
  Injected platform tests also cover `F_SETOWN_EX` failure, `F_GETOWN_EX`
  failure or mismatch, unavailable/malformed/less-than-ten-second
  `lease-break-time`, the post-owner `F_GETLEASE` error/loss branch before child
  spawn, final `F_GETLEASE` loss without a readable notification,
  and lease-break cleanup exceeding its five-second ceiling. They also cover
  nonregular inputs, fstat-size mismatch, unreviewed names, malformed prepared
  files, child failure, a silent child past the injected deadline, and zero payload
  pre-read. The large-payload owner is a private opaque type that implements
  neither `Read` nor `Seek` and does not expose `File` or a raw descriptor
  outside one Linux supervision module. Every operation on that descriptor
  goes through one closed, injected syscall boundary: no-follow open, lease
  signal setup/acquire/query/release, `fstat`, `lseek`, duplication into child
  stdin, and close. There is no content-access operation. The test
  implementation records the exhaustive production call sequence and fails on
  any unclassified operation; a source-boundary test rejects direct payload-fd
  access outside that adapter, including `read`, `pread*`, `readv`, `mmap`,
  `sendfile`, `splice`, `copy_file_range`, or an `io_uring` path. Immediately
  before child spawn, also require `lseek(SEEK_CUR) == 0`. A test hook at that
  boundary asserts the exact allowed operation log and zero offset before the
  fake child is released to consume stdin. This instrumented syscall boundary,
  rather than the offset alone, is the direct zero-parent-read proof.
  A subprocess integration test starts the real coordinator command around a
  fake child plus same-process-group descendant holding stdin and both pipes,
  sends the coordinator `SIGINT` and `SIGTERM` in separate cases, and proves a
  nonzero coordinator result, group termination, direct-child reap, writer
  release, and no surviving request. A separate abrupt-parent-death test proves
  the `PR_SET_PDEATHSIG` plus `getppid` guard kills or prevents the direct fake
  upload child. The injected abrupt-death case includes a pre-exec barrier held
  by the test driver: the driver kills the coordinator before allowing the
  child to call `PR_SET_PDEATHSIG`, proving the parent-captured PID check stops
  execution even when death happens in that earliest window. It separately
  covers death after `PR_SET_PDEATHSIG` and does not claim descendant coverage.
  Developers/reviewers never invoke this command against GitHub.
- Pin this release contract:

  ```text
  repository: genomoncology/pangopup
  tag: snv-grch38-v1
  title: Pangopup GRCh38 SNV scores v1
  target commit: 851f57d6ffb75a2c099a3d1263b1e94b60aad0e8
  producer implementation commit: 4161679b362805b706a5bfd2a8b24a25df5e23fb
  builder source: sha256:10fd5d7715a611f9b7f20040887391502535ac7860bc6a1eda2bfdda79682b64
  bundle ID: sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3
  transport ID: sha256:3a2f4901b8f3dece302640d0257cc98aa50010a45fe61c5ef77c64a62f4660aa
  proof receipt: 2,194 bytes,
    sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475
  ```

  The release tag points to the clean Ticket 006 shipped commit so its source
  archive contains the documented local installer. The older producer commit
  remains independently bound by the proof receipt. The tag deliberately does
  not point to the Ticket 007 commit because a checked-in profile cannot
  contain the hash of the commit that contains itself.
- Publish exactly these eight assets, in this logical order:

  | Asset | Exact bytes/source |
  |---|---|
  | `transport.json` | transport copy; 1,266 bytes; `sha256:f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a` |
  | `bundle-manifest.json` | transport copy; 3,589 bytes; bundle ID above |
  | `NOTICE` | transport copy; 1,709 bytes; `sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
  | `payload.pgi.zst.part0000` | 1,000,000,000 bytes; `sha256:07c1f9a2e33e1a5bd929500eefd00b84764c82d56e3f573c35d380419e4ed42a` |
  | `payload.pgi.zst.part0001` | 931,687,706 bytes; `sha256:87580144fd828676d7adb269059cf2b425b342fe5ccee442888e0b93994adc74` |
  | `proof-receipt.json` | generated output copy; identity above |
  | `release-profile.json` | generated canonical profile |
  | `SHA256SUMS` | generated LF-terminated digest list |

  `SHA256SUMS` lists the other seven assets in table order. Each line is one
  lowercase 64-hex SHA-256, two ASCII spaces, the exact filename, and LF. It
  does not list itself. Known part hashes come only from the mutually matching
  canonical transport and proof receipt.
- Generate and check in byte-identical
  `release-profiles/snv-grch38-v1.json`. It is canonical RFC 8785 JSON with no
  trailing LF. Every object is closed, array order is semantic, integers are
  `0..=2^53-1`, identity strings include their algorithm prefix, and URLs are
  literal HTTPS strings. Its exact object shape is:

  ```json
  {
    "schema": "pangopup.release-profile.v1",
    "profile": "snv-grch38-v1",
    "repository": "genomoncology/pangopup",
    "release": {
      "tag": "snv-grch38-v1",
      "title": "Pangopup GRCh38 SNV scores v1",
      "target_commit": "851f57d6ffb75a2c099a3d1263b1e94b60aad0e8",
      "page_url": "https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v1"
    },
    "source": {
      "title": "Pangolin precomputed scores",
      "creators": ["Nils Wagner", "Aleksandr Neverov"],
      "doi": "10.5281/zenodo.15649338",
      "license": "CC-BY-4.0",
      "archive": {
        "name": "Pangolin_hg38_snvs_masked.zip",
        "size": 12988141317,
        "md5": "md5:679ef0b50e511b6102b4b88fbf811108"
      },
      "assembly": "GRCh38",
      "masked": true,
      "window": 50
    },
    "reference_compatibility": {
      "assembly": "GRCh38.p14",
      "assembly_accession": "GCF_000001405.40",
      "input_size": 972898531,
      "input_sha256": "sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3",
      "sequence_set_sha256": "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4",
      "ordinary_ref_mismatches": 0,
      "preserved_ref_n_loci": 30
    },
    "bundle": {
      "schema": "pangopup.bundle.v1",
      "index_format": "pangopup.fixed11.v1",
      "bundle_id": "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3"
    },
    "transport": {
      "schema": "pangopup.snv-transport.v1",
      "transport_id": "sha256:3a2f4901b8f3dece302640d0257cc98aa50010a45fe61c5ef77c64a62f4660aa",
      "members": [
        {"logical_path":"transport.json","asset_name":"transport.json","size":1266,"sha256":"sha256:f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a","url":"https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/transport.json"},
        {"logical_path":"bundle-manifest.json","asset_name":"bundle-manifest.json","size":3589,"sha256":"sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3","url":"https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/bundle-manifest.json"},
        {"logical_path":"NOTICE","asset_name":"NOTICE","size":1709,"sha256":"sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7","url":"https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/NOTICE"},
        {"logical_path":"payload.pgi.zst.part0000","asset_name":"payload.pgi.zst.part0000","size":1000000000,"sha256":"sha256:07c1f9a2e33e1a5bd929500eefd00b84764c82d56e3f573c35d380419e4ed42a","url":"https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/payload.pgi.zst.part0000"},
        {"logical_path":"payload.pgi.zst.part0001","asset_name":"payload.pgi.zst.part0001","size":931687706,"sha256":"sha256:87580144fd828676d7adb269059cf2b425b342fe5ccee442888e0b93994adc74","url":"https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/payload.pgi.zst.part0001"}
      ]
    },
    "proof": {
      "schema": "pangopup.proof-receipt.v1",
      "asset_name": "proof-receipt.json",
      "size": 2194,
      "sha256": "sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475"
    }
  }
  ```

  The formatted example fixes names, nesting, and values; the emitted bytes use
  RFC 8785 key ordering and no insignificant whitespace.
- Generate release notes that retain the title, creators, DOI, and CC BY 4.0;
  say the publisher identifies this as masked, window-50 precomputed SNV data
  for hg38 but does not name an exact FASTA/patch or GENCODE release; separately
  say Pangopup exhaustively certified ordinary reference alleles against RefSeq
  GRCh38.p14/GCF_000001405.40 while preserving the 30 published `REF=N` loci;
  describe the per-gene TSV-to-fixed-v1 transformation; print bundle,
  transport, and proof identities; and state that model weights, reference/mask
  assets, binaries, non-SNV inference, remote sync, HTTP, and Docker are absent.
  The notes include an exact manual path that downloads only the five transport
  members into a new `<TRANSPORT_DIR>` and keeps the proof/profile/SHA assets
  outside it, then runs:

  ```text
  pangopup assets install --transport <TRANSPORT_DIR>
  ```

  Downloading all eight assets into the transport directory is explicitly
  documented as invalid because the installer enforces a closed five-file set.
- Add checked-in miniature fixtures and inside-out tests for strict receipt and
  profile parsing, deterministic outputs, exact names/order/URLs, pinned public
  CLI rejection of another contract, internal miniature-contract success,
  no-follow regular-file checks, missing/extra/wrong-size parts, malformed or
  mismatched receipt metadata, output conflict/atomicity, and an audit hook
  proving zero payload-part opens/reads. Add `spec/snv-release.md` for exact CLI
  grammar, JSON success, deterministic failures, and output shape.
- Repair CI before public visibility. Install ripgrep 15.2.0 in the workflow
  from `ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz`, requiring archive
  `sha256:33e15bcf1624b25cdd2a55813a47a2f95dbe126268203e76aa6a585d1e7b149c`
  before extraction. Do not depend on an unpinned package-manager version. All
  three local gates and the pushed GitHub Actions gate must be green before
  Phase B.
- Add `planning/artifacts/007-public-snv-release.md` containing the pinned audit
  procedure and empty completion-evidence headings, not a claim about a future
  commit. Before Phase B, the coordinator audits the exact pushed
  publication-ready commit and all
  reachable history with gitleaks v8.30.1's
  `gitleaks_8.30.1_linux_x64.tar.gz` only after checking
  `sha256:551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb`.
  Run its default pinned rules against Git history and a no-git scan against
  temporary copies of GitHub-hosted textual state. Use full redaction; never
  retain raw scanner JSON, API responses, logs, or suspected secrets. The
  retained result records only tool/version/archive digest, exact commands with
  non-secret placeholders, scanned commit, aggregate object counts, exit
  status, finding count, and disposition. Store that redacted result durably
  outside Git under `$PANGOPUP_PUBLICATION_EVIDENCE`; Phase B may consume this
  reviewed-format, retained-but-not-yet-committed evidence. Never put a secret
  or raw scanner/API output there.
- The hosted-state audit is a closed inventory: repository settings and topics;
  branch/rulesets; Actions workflow/run logs and artifacts; issues, pull
  requests, comments, and discussions; wiki refs/pages; projects; releases and
  assets; Pages; deployments/environments; webhooks; and deploy keys. Record
  only counts and pass/fail. Two failed Actions runs and zero Actions artifacts
  were observed before this ticket; their logs must be included. Any credential,
  private key, non-public dataset, customer identifier, or actual dependency on
  non-public software blocks visibility. Historical absolute developer paths
  and a sentence denying a dependency are benign context and do not justify a
  history rewrite.
- Update every duplicated workflow authority—`AGENTS.md`, `planning/README.md`,
  `planning/templates/ticket.md`, and `planning/tickets/README.md`—with this
  narrow external-effect lifecycle:

  ```text
  review -> publication-ready -> commit/push -> green remote gate
         -> coordinator external effect -> complete -> commit/push -> cleanup
  ```

  The coordinator generates the production small outputs before code review.
  After code-review approval and all local gates, it marks the ticket
  `publication-ready`, commits and pushes the reviewed code/profile/tests/audit
  procedure, and waits for that exact commit's green Actions run. It then
  audits that exact pushed commit, retains the redacted result outside Git, and
  only then may perform Phase B. The post-publication completion commit appends
  both audit and release evidence. This avoids asking a commit to contain an
  audit naming its own hash. It is not permission for developers to mutate
  external state.

### Phase B — coordinator-only public publication

- Reconfirm the publication-ready commit is the remote `main`, its Actions run
  is green, the retained outside-Git hygiene result covers that exact commit,
  and no tag/release named
  `snv-grch38-v1` exists. If the repository is already public, record that and
  do not toggle it; otherwise change visibility once from private to public.
- Before creating the release, require GitHub immutable releases:

  ```text
  PUT /repos/genomoncology/pangopup/immutable-releases
  GET /repos/genomoncology/pangopup/immutable-releases
  X-GitHub-Api-Version: 2026-03-10
  ```

  GET must return `enabled=true`; absence, denial, or conflict blocks release
  creation. Retain only the non-secret enabled/enforced setting evidence.
- Create one draft release with the pinned tag, title, target commit, and exact
  reviewed notes. Upload each of the eight assets only through
  `pangopup-build release upload-asset`; never use pathname-reopening
  `gh release upload`. There is no local pre-read, hash, copy, link,
  recompression, semantic verification, or redownload of a large part; large
  bytes are read only by the child from its duplicate of the leased descriptor
  as upload request bodies.
  Coordinator retries are bounded to two command invocations per expected
  asset and the artifact records attempts and byte counts. After every command
  result—including nonzero exit, bounded-output failure, lost response, or
  invalid response—the coordinator re-fetches the complete draft inventory
  before any reuse, deletion, or second invocation. It never assumes a failed
  client observed a failed server write.
- Treat the draft as the transaction. On resume, first fetch its exact asset
  inventory. Reuse an expected `uploaded` asset only when name, size, and
  non-null `sha256:` digest match. For an expected asset in `open` state or
  with wrong size/digest, delete only that asset from the still-draft release
  and retry within the bound. An unexpected asset, ambiguous duplicate,
  changed target/title/body, published release, or exhausted retry stops for
  manual review. Never use `--clobber`; never delete or replace a published
  asset or tag. A null digest from a successful upload command is accepted only
  as a pending state for the bounded inventory poll, never as publication
  evidence. The helper itself never performs inventory, retry, or deletion.
- Poll the draft asset endpoint a bounded maximum of 120 times per upload until
  `state=uploaded` and `digest` is non-null. Before publication, compare the
  exact closed eight-name set, sizes, and server-reported SHA-256 digests with
  the reviewed `SHA256SUMS`. A missing/null digest blocks publication.
- Publish the draft once. Then require the release REST object to report
  `draft=false`, `immutable=true`, the exact tag/target/title/body, and the
  exact eight verified assets. Prove unauthenticated bounded reads of the
  repository, release page, `release-profile.json`, `transport.json`,
  `bundle-manifest.json`, `NOTICE`, `proof-receipt.json`, and `SHA256SUMS`.
  Do not fetch either payload part.
- Append the retained redacted audit result and redacted external-publication
  evidence to the artifact, mark the ticket complete,
  run the documentation/current-state scan, commit and push the completed
  ticket/evidence, and remove the ticket in the normal cleanup commit.
- Update `README.md`, `architecture/delivery.md`,
  `architecture/source-data.md`, `release-profiles/README.md`,
  `planning/faq.md`, and `planning/frontier.md` so Phase A describes exactly
  what is prepared and Phase B changes future/current claims only after the
  external evidence exists.
- Excluded: `pangopup assets sync`, runtime HTTP clients, download cache/resume,
  clean-machine multi-gigabyte download/install, executable releases, models,
  reference/mask publication, containers, signing/SBOM, history rewrite, and
  any production rebuild/recompression/full verification.

## Success Checklist

- The publication-ready commit has independent approval, green local gates,
  green pushed CI, and no production part read. A pinned zero-finding audit of
  that exact pushed commit is retained outside Git before visibility changes
  and committed with the completion evidence afterward.
- `pangopup-build release prepare` deterministically reproduces the reviewed
  proof copy, profile, SHA list, and notes from bounded metadata while opening
  neither production part.
- `pangopup-build release upload-asset` validates the exact release/prepared
  contract, snapshots small inputs into sealed memfds, holds a kernel read
  lease over a selected large part, and streams only that stable source into
  one authenticated child request without reopening its pathname;
  replacement, same-inode mutation, timeout, cleanup, zero-pre-read, and
  symlink-race tests pass without network access.
- The public `snv-grch38-v1` immutable release targets the Ticket 006 commit,
  contains exactly eight assets, and GitHub reports every reviewed size and
  SHA-256 digest.
- The public profile, notice, receipt, and notes preserve source/creator/DOI/
  CC BY attribution and distinguish lookup data from future model/runtime
  assets.
- The documented manual download puts only five members in the transport
  directory and feeds the already-shipped installer.
- Unauthenticated bounded requests succeed without downloading large parts;
  large local bytes were read only as upload bodies with bounded recorded
  retry counts.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Publish before implementing sync.** The downloader should target an
   observed immutable release contract, not invented URLs.
2. **Use the Ticket 006 commit for the data tag.** It contains the installer;
   the proof receipt separately preserves the transport producer commit.
3. **Land and prove preparation before visibility changes.** Public release is
   a second coordinator-only phase after reviewed code, local gates, pushed CI,
   and history/hosted-state hygiene all pass.
4. **Use strict, reusable metadata parsing below the CLI.** One bounded parser
   prevents release tooling and the future downloader from interpreting the
   same transport differently.
5. **Release transport members directly.** Another tar or recompression layer
   would add format and large-I/O work without helping runtime lookup.
6. **Require platform-enforced immutability.** GitHub's repository setting is
   enabled before release creation, and the published release must report
   `immutable=true`; procedural promises are not a fallback.
7. **Trust retained certification locally and GitHub digests remotely.** No
   repeated full semantic scan or multi-gigabyte redownload is useful here.
8. **Keep remote sync separate.** This ticket establishes the real public
   contract; the next independently reviewed ticket can implement bounded
   network/cache behavior against it.
9. **Freeze upload inputs without re-verifying large payloads.** Small inputs
   become sealed memfds; already-certified multi-gigabyte parts use a fail-
   closed Linux read lease and child-stdin handoff. This closes in-flight
   same-inode races without restoring the discarded full-read verifier.
10. **Bound the upload subprocess.** One request has a six-hour operational
    ceiling and process-group kill plus direct-child reap cleanup. Lease-break
    cleanup has a separate five-second ceiling beneath the verified kernel
    forced-break window. A stuck helper cannot hold an asset lease or
    coordinator workflow forever.
11. **Treat coordinator interruption as upload cancellation.** Catchable
    termination signals go through the normal group-kill/reap/release path;
    child-side parent-death protection covers abrupt parent loss for the direct
    upload process. Neither path may report or leave an accepted request.

## Dependencies

Tickets 005 and 006, shipped. The coordinator supplies the retained inputs via
`PANGOPUP_RETAINED_TRANSPORT` and `PANGOPUP_PROOF_RECEIPT`; their host paths
must not enter Git, generated metadata, notes, logs, or review evidence.

## Notes

- The repository was observed private with zero releases/issues/pull requests/
  artifacts on 2026-07-23. Two failed Actions runs were caused by the missing
  `rg` executable in `make spec`; that is a public-readiness defect fixed here.
- The coordinator has authenticated repository administration and release
  access. Never print or retain authentication material.
- Developer/reviewer tests use only checked-in miniature fixtures. The
  coordinator alone invokes preparation against production metadata and later
  uploads the preserved large parts.
- The exact gate is `make lint`, `make test`, and `make spec`; there is no
  `make check`.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-07-23

This is the only active ticket. It reacts to Ticket 006's shipped local
installer by creating the real public origin the next downloader ticket will
consume.

## Independent Ticket Review

Reviewer: Newton `/root/ticket_007_design_review`

Revision 1 was not approved. The coordinator resolved all eight findings:
preparation now lands before publication; the tag targets Ticket 006; profile
and receipt contracts are closed and pinned; CI and all GitHub-hosted state are
audited; immutable releases are mandatory; manual installation keeps auxiliary
assets outside the transport directory; draft digest/recovery behavior is
exact; and the large-I/O invariant allows only bounded upload-body retries.

Revision 2 was not approved because the audit result still referred to the
commit intended to contain it, publisher hg38 provenance and Pangopup RefSeq
compatibility were conflated, and the proof-receipt value contract remained
incomplete.

Revision 3 was approved. The pushed-commit audit is now retained outside Git
until the completion commit; all four lifecycle documents are in scope; source
and compatibility facts are separate; and the exact canonical production
receipt is the checked-in schema/value example. The reviewer found no new
blocker in the parser boundary, CI/hygiene procedure, immutable release
contract, manual five-file install path, draft recovery, attribution, or
large-I/O limits.

During development, the exact retained receipt exposed one internal
contradiction: its reviewed 2,194-byte identity includes a final LF, while its
RFC 8785 JSON payload without that LF is 2,193 bytes. Revision 4 preserves the
exact public artifact and makes the parser rule explicit: canonical JSON plus
exactly one LF.

Revision 4: approved. The reviewer independently confirmed the retained byte
count, final `0a`, canonical 2,193-byte JSON prefix, whole-file identity, and
the deterministic rejection rules. No new scope blocker was found.

The first adversarial code review found that a later raw `gh release upload`
would reopen a payload pathname after metadata inspection. A replacement or
symlink could therefore leak unrelated local bytes into the draft before the
server digest blocked publication. Revision 5 adds the held-no-follow-file
`release upload-asset` boundary above and forbids pathname-based upload. This
material security amendment is pending the same ticket review before the
developer remediates code-review findings.

Revision 5 was not approved because selected-file open ordering, the `gh`
process contract, and post-attempt ambiguity handling were incomplete.
Revision 6 opened the selected dirfd-relative no-follow source before
validation, used that same held File for validation and child stdin, pinned the
exact GitHub CLI binary/argv/environment/bounds, and required a complete
inventory after every attempt before any recovery.

Revision 6 was not approved because the validated GitHub CLI was still executed
by a reopenable pathname and a present response digest was not compared with
the reviewed asset digest. Revision 7 executes the held executable descriptor
and requires exact digest equality.

Revision 7: approved. The reviewer found no remaining blocker in held asset or
executable security, auth/process behavior, bounded output, fake-child
testability, exact response validation, inventory/retry semantics, scope, or
the no-large-preread invariant.

The second adversarial code review found three residual process-boundary gaps.
A held descriptor still permits an in-place overwrite or truncate of its inode;
the child had no deadline; and the payload test proved eventual stdin bytes but
did not observe the pre-spawn offset/read boundary. Revision 8 freezes the
executable and small assets in sealed memfds, requires a monitored Linux read
lease for a large part, adds a six-hour production deadline with process-group
kill/reap cleanup, and makes the zero-parent-read boundary directly auditable.
This material amendment is pending the same ticket reviewer before development
continues. The reviewer did not approve the first Revision 8 wording: it did
not completely route and verify lease notifications, overstated guarantees
beyond Linux's forced-break window and process reaping, and treated an offset
plus local counter as proof against offset-preserving reads. Revision 9 adds
thread-targeted signal setup and final lease checks, a five-second cleanup
ceiling below a validated kernel window, exact process-group/direct-child
language, and an exhaustive injected syscall boundary for the no-pre-read
proof. Revision 9 was not approved because lease acquisition can overwrite a
preselected signal owner and the new failure branches lacked explicit tests.
Revision 10 blocks the signal and creates `signalfd`, acquires the lease, then
sets/verifies the thread owner and reconfirms the lease; it adds injected tests
for owner setup/readback, kernel-window parsing, silent final lease loss, and
cleanup-deadline exhaustion.

Revision 10: approved. The reviewer confirmed the lease acquisition/routing
order is Linux-correct, the blocked signal covers the interim, final success
rechecks the lease, and every new fail-closed branch has an explicit injected
test. The process-cleanup and exhaustive zero-pre-read boundaries are
decision-complete and remain consistent with the ban on large pre-reads.

The Revision 10 implementation rereview closed the original three findings but
found that placing `gh` in its own process group without coordinator signal
handling could orphan an authenticated request on Ctrl-C or termination. It
also found no injected test for the post-owner lease query and one misleading
documentation label that called all eight release assets the installable
transport. Revision 11 adds supervised `SIGINT`/`SIGTERM`, child-side
`PR_SET_PDEATHSIG` with the parent-PID race check, exact integration/fault
tests, and requires `architecture/delivery.md` to distinguish the eight-file
release asset set from the five-file installable transport. Revision 11 was not
approved because it could capture an already-reparented PID inside the child
and would carry the parent's blocked signals through exec. Revision 12 captures
the expected PID in the parent before spawn, uses a pre-protection death-barrier
test, requires `SFD_CLOEXEC`, and restores the original child signal mask
immediately before `execveat`.

Revision 12: approved. The reviewer confirmed both sides of the parent-death
race are covered, child setup happens in the safe order, the original signal
mask is restored only immediately before sealed execution, and
`SFD_CLOEXEC` prevents descriptor leakage. The abrupt-death limitation is
accurate and the interruption contract is decision-complete.

## Implementation Evidence

Developer: Codex `/root/ticket_007_developer`, 2026-07-23

Implemented Phase A only. `pangopup-assets` now owns closed canonical parsing
for the LF-framed proof receipt and no-LF release profile plus a public bounded
transport inspection result. Inspection reads only `transport.json`,
`bundle-manifest.json`, and `NOTICE`; it validates payload parts using no-follow
regular-file name/size metadata. `pangopup-build release prepare` accepts only
the pinned production receipt/transport contract, creates a private
same-filesystem stage, and atomically emits the exact proof copy, generated
byte-identical profile, ordered `SHA256SUMS`, and release notes.

The checked proof is exactly 2,194 bytes and hashes to
`sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475`.
The canonical no-LF profile is 2,821 bytes and hashes to
`sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6`.
Miniature injected-contract preparation was byte-deterministic across two
outputs. Its input-open audit recorded only the receipt and three small
transport metadata members and zero payload-part opens. Tests also covered
missing/multiple/CRLF receipt framing, duplicates, unknown fields,
noncanonical JSON, pinned-public rejection of a miniature contract,
well-formed metadata mismatch, symlinked receipt/part, wrong-size part,
closed member order/URLs, checksum order, notes content, private stage mode,
output conflict, and stage cleanup.

Focused evidence:

```text
cargo test --locked -p pangopup-assets release::tests -- --nocapture
  2 passed
cargo test --locked -p pangopup-build --test transport -- --nocapture
  17 passed
```

Completed local gates on the finished diff:

```text
make lint  pass
make test  pass
make spec  101 passed
```

Documentation changed with behavior: `README.md`, `architecture/delivery.md`,
`architecture/source-data.md`, `release-profiles/README.md`,
`planning/faq.md`, `planning/frontier.md`, all four workflow authorities, and
the pinned `planning/artifacts/007-public-snv-release.md` audit procedure. CI
now installs ripgrep 15.2.0 only after verifying the required archive digest.

No repository setting, tag, release, visibility, immutable-release setting, or
other external state was changed. No production payload part was opened, read,
hashed, copied, linked, verified, or rebuilt. The coordinator must run the
production small-output preparation before dispatching adversarial code review.

Coordinator bounded production preparation completed before code-review
dispatch. It returned `status=prepared`, asset count 8, the pinned transport ID,
and the pinned bundle ID. The four retained small outputs were:

```text
SHA256SUMS           595 bytes  sha256:54c29666c74bb35701d14f10d7d2b2ba3dcadc116a111429274da8aa975dce2e
proof-receipt.json  2194 bytes  sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475
release-notes.md    1594 bytes  sha256:e96a8f702de522292ebb672c6782ecc81b1c134063902de7bc9e38fa78496fb7
release-profile.json 2821 bytes sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6
```

The generated proof and profile were byte-identical to their checked-in
reviewed files. The command used bounded receipt/transport metadata inspection
and part name/type/size metadata only; it did not open a payload part.

Remediation after the first adversarial code review implemented all five
findings under approved Revision 7:

1. Added coordinator-only `release upload-asset`. It validates and holds the
   pinned official `gh` executable before touching either asset root, opens and
   holds the selected no-follow member before contract validation, revalidates
   both closed directories dirfd-relative, and gives that same held File to one
   shell-free child as stdin. The child contract fixes argv, upload URL,
   headers, content length, cleared/allowlisted environment, 64 KiB output
   caps, closed response JSON, and exact present-digest equality. It never
   retries, deletes, reopens the selected path, or echoes child output.
2. The public-hygiene procedure now fetches and prunes public branch, tag, and
   pull-request refs, scans `--log-opts=--all`, and safely extracts Actions log
   ZIPs with traversal, symlink/nonregular, encryption, duplication, entry, and
   byte caps before the no-Git scan.
3. Release-profile parsing now enforces profile/tag, release-page URL,
   proof-asset, exact semantic member order/names, and member URLs. Production
   use additionally pins the complete receipt/profile values and the exact
   2,821-byte profile SHA-256; mutation tests fail closed.
4. Durable docs now make GitHub immutable releases mandatory with no mutable
   fallback and order completed, observed publication before remote-sync work.
5. Generated release notes now contain a copy/paste shell recipe with exactly
   the five reviewed transport-member URLs and install only that closed
   directory; proof, profile, and checksum remain outside it.

The compiled fake-child suite proves exact argv, headers, content length,
single invocation, sanitized environment, byte-exact stdin, null/present
digest handling, closed-response rejection, nonzero/overflow behavior, and no
raw child-error disclosure. Race hooks replace the validated executable and
selected transport metadata, prepared metadata, and payload path both before
and after validation; the originally held descriptors remain authoritative.
Executable and asset root/component/member symlinks, wrong size, an unreviewed
name, malformed/extra prepared state, and mismatched response values fail
before publication. The payload case reaches only held-fd metadata before the
fake child consumes stdin.

Focused and full evidence on the remediated diff:

```text
cargo test -p pangopup-assets release::tests -- --nocapture
  3 passed
cargo test -p pangopup-build --test transport release_ -- --nocapture
  5 passed
make lint  pass
make test  pass
make spec  105 passed
```

The builder-source identity changed as expected, so the deterministic checked
SNV regression fixture was regenerated with its own repository tool; its
byte-exact regeneration test passes. No network or external state was touched,
and no production payload part was opened, read, hashed, copied, linked,
verified, or rebuilt.

The earlier production prepared-output list above predates the reviewed
five-URL notes recipe and is therefore superseded for dispatch purposes.
`proof-receipt.json`, `release-profile.json`, and `SHA256SUMS` generation are
unchanged, but `release-notes.md` and its prepared-directory identity must be
regenerated by the coordinator through the same bounded metadata-only command
before the next code-review dispatch.

The coordinator preserved that first directory and completed a second bounded
production preparation before redispatch. It again returned the pinned bundle
and transport IDs with asset count 8. The checked proof/profile were
byte-identical and the checksum list was unchanged; the reviewed five-command
notes recipe produced the only expected change:

```text
SHA256SUMS            595 bytes  sha256:54c29666c74bb35701d14f10d7d2b2ba3dcadc116a111429274da8aa975dce2e
proof-receipt.json   2194 bytes  sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475
release-notes.md     2391 bytes  sha256:d82191e1d1dc5f1ecc5c06422aa067426065748083f25a55cf1d5d33f8ef9dae
release-profile.json 2821 bytes  sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6
```

The second command again used only bounded small-file inspection and
part-name/type/size metadata. It did not open a production payload part.

## Adversarial Code Review

Reviewer: Newton `/root/ticket_007_code_review`

Revision 7 implementation: not approved. The reviewer found that pathname-safe
descriptors did not prevent same-inode mutation after validation, a silent
child could run forever, and no test directly observed zero payload reads
before child spawn. Those findings are routed through ticket Revision 8 before
developer remediation.

Revision 10 implementation: not approved. The original findings were closed,
but the reviewer found a high-severity coordinator-interruption orphan, missing
post-owner lease-query injection, and a low-severity transport/release-set docs
error. Those findings are routed through ticket Revision 11 before developer
remediation.

Revision 12 implementation: approved. The reviewer independently verified
orderly `SIGINT`/`SIGTERM` cleanup, both parent-death race windows, restored
child/parent signal masks, close-on-exec signal state, post-owner lease-query
failure coverage, and the corrected release-set terminology. All earlier
sealed-input, content-blind payload, no-pre-read, deadline, response, fixture,
and immutable-publication boundaries remain sound. Reviewer gates passed:
focused uploader 12, release parser 3, `make lint`, full `make test`, `make
spec` 105, and `git diff --check`. The fixture `scores.pgi` remained
byte-identical and JSONL changes were provenance-only. No network, production
payload content, GitHub mutation, or external state was accessed.

Revision 10 remediation is ready for the same reviewer. The GitHub CLI source
is copied into a size/digest-validated sealed memfd and executed with
`execveat(AT_EMPTY_PATH)`. Every small selected asset is copied into its own
sealed memfd before validation. A selected payload is instead owned by the new
private Linux boundary in `release_upload_linux.rs`; the opaque type exposes no
`File` or raw fd and implements neither `Read` nor `Seek`.

The payload boundary blocks `SIGIO`, creates a nonblocking `signalfd`, validates
`lease-break-time >= 10`, opens no-follow, acquires the read lease, then
sets/reads back `F_OWNER_TID` and reconfirms `F_RDLCK` in the approved order.
Only classified boundary operations can fstat, query offset, duplicate child
stdin, poll/drain the lease signal, perform the final lease query, release, and
close. Immediately before spawn the exact injected operation log contains no
content access and `lseek(SEEK_CUR)` is zero. A source-boundary test rejects
payload access through `read`, `pread*`, `readv`, `mmap`, `sendfile`, `splice`,
`copy_file_range`, or `io_uring` escape paths.

The child uses a separate process group and a 21,600-second production
deadline. Tests inject short deadlines. Supervision continues until both the
direct child and both nonblocking bounded output readers finish, so a
same-group descendant cannot hold a pipe open indefinitely after the direct
child exits. Overflow, deadline, lease break, or supervision failure cancels
the readers, sends process-group `SIGKILL`, and reaps the direct child before
the lease is released. Cleanup is checked against the five-second ceiling;
success nonblockingly drains lease notifications and reconfirms `F_RDLCK`.

Focused tests use only miniature data and a compiled fake executable. They
prove sealed gh and all six small selected assets survive same-inode overwrite
or truncate; a pre-existing writer blocks lease acquisition; payload overwrite
and truncate attempts remain blocked until cancellation, process-group kill,
direct-child reap, and lease release; the allowed pre-spawn log and zero offset
are exact; and no parent payload content operation occurs. Injected tests cover
`F_SETOWN_EX` failure, `F_GETOWN_EX` failure/mismatch, unavailable, malformed,
and nine-second kernel windows, silent final lease loss, and cleanup-deadline
exhaustion. The descendant-held-pipe test proves the injected request deadline
kills the group while the direct child is reaped.

Final evidence on the Revision 10 diff:

```text
cargo test --locked -p pangopup-build --test transport release_upload -- --nocapture
  8 passed
make lint  pass
make test  pass (25 transport integration tests)
make spec  105 passed
git diff --check  pass
```

The builder-source identity changed as expected, and the repository's
deterministic SNV regression generator refreshed the checked fixture; its
byte-exact regeneration test passes. Durable uploader documentation now
describes sealed CLI/small snapshots, the content-blind leased payload,
21,600-second deadline, process-group kill/direct-child reap, and five-second
lease cleanup rather than mutable held descriptors.

No repository setting, GitHub state, network endpoint, or other external state
was touched. No production payload part was opened, read, hashed, copied,
linked, rebuilt, or verified.

Revision 12 remediation is ready for rereview. Upload supervision now saves the
calling thread's mask, blocks `SIGINT`, `SIGTERM`, and `SIGIO` together before
payload lease acquisition, and owns one nonblocking close-on-exec `signalfd`.
It drains and classifies pending signals immediately before spawn, monitors all
three during the request, and consumes catchable interruption through the same
process-group kill, bounded-pipe cancellation, direct-child reap, and lease
release path as the other fail-closed outcomes. The original parent mask is
restored on every return after cleanup.

The parent captures its PID before spawn. In the pre-exec boundary, the child
creates its process group, installs `PR_SET_PDEATHSIG(SIGKILL)`, verifies that
`getppid()` still equals the captured parent, restores the saved original
signal mask, and only then calls `execveat` on the sealed CLI snapshot. The
fake executable proves the three supervised signals are not left blocked and
the close-on-exec signal descriptor is absent. Subprocess barriers cover parent
death before and after `PR_SET_PDEATHSIG`; neither case reaches fake `gh`.

Orderly subprocess tests separately deliver `SIGINT` and `SIGTERM` while a
payload writer is lease-blocked and a same-process-group descendant holds
stdin and both output pipes. Both cases return a sanitized nonzero interrupted
result, kill the group, reap the direct child, release the writer only after
cleanup, and restore the coordinator mask. A pre-spawn pending-interrupt test
starts no child. Injected post-owner `F_GETLEASE` error and lease-loss cases now
also fail before child spawn. `architecture/delivery.md` distinguishes the
eight-file release asset set from its exact five-file installable transport.

Final evidence on the Revision 12 diff:

```text
cargo test --locked -p pangopup-build --test transport release_upload -- --test-threads=1
  12 passed
make lint  pass
cargo build --locked --package pangopup-build --bin pangopup-build  pass
make test  pass (29 transport integration tests)
make spec  105 passed
git diff --check  pass
```

The source-identity change was regenerated with the repository's deterministic
SNV fixture tool, and the byte-exact fixture regeneration test passes. No
repository setting, GitHub state, network endpoint, or other external state was
touched. No production payload part was opened, read, hashed, copied, linked,
rebuilt, or verified.

## External Publication Evidence

Coordinator: pending

## Coordinator Final Check

Coordinator: Codex `/root`, 2026-07-23

The independently approved diff passed the coordinator's final local gate:

```text
make lint  pass
make test  pass (including 29 transport integration tests)
make spec  105 passed
cargo build --locked --package pangopup-build --bin pangopup-build  pass
git diff --check  pass
```

The current-state scan found only expected pre-publication language: bounded
release preparation is shipped, while repository visibility, immutable release
publication, and remote sync remain incomplete. The eight-file release set and
exact five-file installable transport are distinct in user and architecture
documentation. This exact diff is ready to commit and push; the coordinator
must require its remote Actions run and pinned public-hygiene audit to pass
before any visibility or release mutation.
