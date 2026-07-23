# 007 — Publish the certified GRCh38 SNV transport as a public data release

Status: ready

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
  at every object level, RFC 8785 byte-canonicality, JSON-safe integers, exact
  array lengths/order, and exact pinned values. The closed v1 shape is:

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

  The checked-in production receipt is the complete JSON example and exact
  production value contract: object keys, field types, array cardinalities and
  order, encoder values, verification argv values, and every source/reference/
  bundle/transport identity must match it. The parser belongs to
  `pangopup-assets`; the release preparer compares its typed result with the
  same crate's typed transport inspection result. Internal miniature tests use
  the same closed schema but inject a miniature contract containing their own
  exact receipt bytes, whole-file SHA-256, identities, encoder values, and argv
  arrays; they do not weaken production validation.
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
  reviewed notes. Upload the eight assets. There is no local pre-read, hash,
  copy, recompression, semantic verification, or redownload of a large part;
  large bytes are read only as upload request bodies. Upload retries are
  bounded to two attempts per expected asset and the artifact records attempts
  and byte counts.
- Treat the draft as the transaction. On resume, first fetch its exact asset
  inventory. Reuse an expected `uploaded` asset only when name, size, and
  non-null `sha256:` digest match. For an expected asset in `open` state or
  with wrong size/digest, delete only that asset from the still-draft release
  and retry within the bound. An unexpected asset, ambiguous duplicate,
  changed target/title/body, published release, or exhausted retry stops for
  manual review. Never use `--clobber`; never delete or replace a published
  asset or tag.
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
- Excluded: `pangopup assets sync`, HTTP clients, download cache/resume,
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

Revision 3: approved. The pushed-commit audit is now retained outside Git until
the completion commit; all four lifecycle documents are in scope; source and
compatibility facts are separate; and the exact canonical production receipt
is the checked-in schema/value example. The reviewer found no new blocker in
the parser boundary, CI/hygiene procedure, immutable release contract, manual
five-file install path, draft recovery, attribution, or large-I/O limits.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Publication Evidence

Coordinator: pending

## Coordinator Final Check

Coordinator: pending
