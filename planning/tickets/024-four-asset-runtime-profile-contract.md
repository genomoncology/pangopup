# 024 — Freeze one coherent four-asset runtime profile

Status: complete

## Why

Pangopup can score from four explicit local inputs today, but those inputs are
still unrelated paths. A caller can accidentally combine an SNV index, model,
GRCh38 sequence bundle, and GENCODE mask that were never qualified together.
Installation, offline startup, HTTP readiness, and publication all need one
small canonical statement of the exact compatible tuple before any of them can
activate it safely.

This ticket freezes and proves that statement. It does not install, download,
publish, or activate large assets yet.

## Scope

- Add a strict canonical `pangopup.runtime-profile.v1` JSON contract in
  `pangopup-assets`; do not add another crate. Its identity is
  `sha256:<SHA-256 of the exact canonical profile bytes>`.
- Bind exactly these independently versioned runtime facts:
  - the fixed-v1 SNV bundle identity, lookup format, and score-member
    length/digest;
  - the selected singleton ONNX model bundle identity, profile,
    representation, and model-member length/digest. The bundle identity already
    binds the canonical graph, checkpoint, and inventory declarations; do not
    duplicate those structures in this profile;
  - the production RefSeq GRCh38.p14 sequence bundle identity, profile, format,
    assembly accession, member length/digest, and logical sequence-set digest;
  - the exact selected GENCODE v38 domains member format, length, and SHA-256;
  - the closed scoring contract: GRCh38, distance/window 50,
    `pangopup-variant-score-v1`,
    `pangolin-gencode-v38-order-sensitive-v1`, and the portable ordinary CPU
    policy `sequential:1/1`.
- Freeze this exact typed JSON shape; all objects reject unknown fields:

  ```json
  {
    "schema": "pangopup.runtime-profile.v1",
    "snv": {
      "bundle_id": "sha256:…",
      "format": "pangopup.fixed11.v1",
      "member_bytes": 15033158255,
      "member_sha256": "sha256:…"
    },
    "model": {
      "bundle_id": "sha256:…",
      "profile": "pangolin-1.0.2-5cf94b8-onnx-cpu-v1",
      "representation": "singleton",
      "member_bytes": 33867142,
      "member_sha256": "sha256:…"
    },
    "reference": {
      "bundle_id": "sha256:…",
      "profile": "refseq-grch38p14-primary-v1",
      "format": "pangopup.reference.acgt2-rle.v1",
      "assembly": "GRCh38.p14",
      "assembly_accession": "GCF_000001405.40",
      "sequence_set_sha256": "sha256:…",
      "member_bytes": 772091760,
      "member_sha256": "sha256:…"
    },
    "mask": {
      "format": "pangopup.gencode-v38-domains.v1",
      "member_bytes": 6703320,
      "member_sha256": "sha256:…"
    },
    "scoring": {
      "assembly": "GRCh38",
      "semantics": "pangopup-variant-score-v1",
      "distance": 50,
      "masking_policy": "pangolin-gencode-v38-order-sensitive-v1",
      "cpu_policy": "sequential:1/1"
    }
  }
  ```

- Define compatibility as exact equality with the already jointly qualified
  production tuple:
  - SNV bundle
    `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`,
    score member 15,033,158,255 bytes,
    `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`;
  - model bundle
    `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`,
    `model.onnx` 33,867,142 bytes,
    `sha256:3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b`;
  - reference bundle
    `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
    sequence set
    `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`,
    `reference.pgr` 772,091,760 bytes,
    `sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`;
  - mask 6,703,320 bytes,
    `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
  “Mixed” means any one of these identity, member, format, or policy facts
  differs. Independently valid future rebuilds require a separately named
  runtime-profile schema/version or an explicitly reviewed new accepted tuple.
- Keep the profile data-only and path-free. It contains identities and
  compatibility facts, never local filesystem paths, URLs, mutable aliases,
  credentials, timestamps, hostnames, or a moving “latest” version.
- Add bounded strict parsing and RFC 8785/JCS canonical serialization: UTF-8
  JSON with no BOM or trailing newline and no larger than 64 KiB, closed
  schemas, duplicate-key rejection, JCS key ordering, JSON-safe unsigned
  integers `0..=9_007_199_254_740_991`, lowercase `sha256:` identities, no
  extensions, and byte-identical round trips. Parsing establishes grammatical
  validity; a separate production trust check requires the pinned tuple above.
  The profile ID is derived externally and is not recursively embedded in the
  profile.
- Add a maintainer command:

  ```text
  pangopup-build runtime-profile prepare \
    --snv-bundle <DIR> \
    --model-bundle <DIR> \
    --reference-bundle <DIR> \
    --mask <FILE> \
    --output <NEW_FILE>
  ```

  It opens the four already-built local assets through production
  readers/admission APIs, requires the exact pinned tuple, and writes one new
  canonical profile without replacing an existing output. It must not copy,
  rebuild, mutate, install, or publish any component.
- Add only the bounded inspection surface needed to compose existing trust
  boundaries:
  - SNV admission reads the canonical manifest and notice through held
    descriptors, checks the exact member set and declared score-member
    type/size, and detects pathname replacement. It may mmap only the fixed
    header if structurally unavoidable, but it must not scan, hash, decode, or
    page through the 15 GB score payload. The installed/certified bundle
    receipt remains the authority for that payload's full digest.
  - model inspection authenticates the bounded manifest and all bounded model
    members, reports the selected bundle/profile/representation/model-member
    facts, and uses held descriptors so replacement of a pathname cannot change
    the inspected bytes. It never constructs `ModelKernel`, initializes ONNX
    Runtime, or runs inference.
  - reference inspection may perform the existing full member authentication
    because this is a one-time maintainer operation and must keep the same
    retained descriptor for identity and verification.
  - mask inspection uses the existing identified/qualification open over the
    retained descriptor and exact selected domains bytes.
- Publish the small output through a same-directory private staged file:
  reject a symlink or existing final target, write and sync the complete
  canonical bytes, atomically rename with no-replace semantics, then sync the
  parent directory. On failure, no partial final path remains. Use stable
  categories for usage, incompatible tuple, unsafe path/member, corrupt input,
  input I/O, output conflict, and output I/O; diagnostics are bounded and do
  not echo untrusted JSON or arbitrary path contents.
- Add normal checked miniature typed facts for grammar/canonical/profile-ID
  tests without a public synthetic-preparation flag. Test the shipped
  production-only command's rejection, stable errors, and atomic output with
  deliberately invalid small paths. The only successful four-path command is
  coordinator-only against the retained production assets.
- Add inside-out tests proving deterministic typed construction, strict
  parse/JCS round trip, every component and compatibility field affects the
  profile ID, production trust rejection of grammar-valid synthetic facts,
  wrong build/profile/format/policy rejection, manifest/member pathname
  replacement detection, full model/reference/mask same-size corruption
  failure through held descriptors, bounded input and diagnostics, read-only
  inputs, output no-replace/durability/atomicity, and no model
  session/inference. Do not promise to rediscover same-size SNV score corruption
  here; that belongs to existing installation/certification.
- Coordinator-only production evidence may generate the small candidate
  profile from the already retained SNV/model/reference/mask assets after
  normal tests pass. Commit the exact canonical bytes as
  `planning/artifacts/024-four-asset-runtime-profile.json`, and retain component
  identities, profile ID, command, size, input paths, and hashes in
  `planning/artifacts/024-four-asset-runtime-profile.md`. The JSON is a small
  compatibility authority, not a large runtime asset, public release manifest,
  or active installation.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/runtime-data.md`, `architecture/service.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add one ADR for the canonical
  profile contract. Clearly say installation/activation, network sync,
  publication, runtime discovery, HTTP, Docker, and systemd remain future.

Explicit exclusions: XDG installation, immutable component-store layout,
`active.json` replacement, rollback, repair/GC, progress/status, networking,
GitHub releases, release uploads, raw-source rebuilds, asset compression or
splitting, lookup CLI profile discovery, model-cache migration, HTTP, service
lifecycle, Docker, systemd, signing, SBOM, and public external effects.

## Success Checklist

- One canonical path-free profile unambiguously binds the exact four compatible
  runtime assets and scoring policy.
- Constructing and serializing twice from the same miniature typed facts
  produces byte-identical bytes and the same profile ID; any material field
  change changes the ID.
- Malformed, duplicate, oversized, synthetic, incompatible, replaced, or mixed
  inputs fail before a profile is published and never leave a partial final
  file. Model/reference/mask corruption is authenticated here; SNV payload
  corruption remains covered by its existing installer/certifier.
- Preparation performs no inference and does not scan, hash, decode, or fault
  through the large SNV score payload.
- A small retained coordinator candidate binds the already-qualified
  production assets without publishing or activating them.
- Fast unit/integration tests and executable specs cover the contract and
  maintainer CLI behavior; no long-running verifier enters normal gates.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Freeze the compatibility statement before installation.** Combining
   contract design, large-file installation, activation, rollback, and network
   delivery would make one oversized ticket and hide whether mixed assets are
   rejected. This ticket produces the small authority that later installer and
   runtime tickets consume.
2. **Keep the profile path-free and hash the canonical bytes.** Absolute paths,
   URLs, timestamps, and mutable aliases make identities host-specific or
   moving. Content and compatibility identities are portable; later
   installation maps them to private immutable local paths. RFC 8785/JCS owns
   key ordering rather than a handwritten serializer convention.
3. **Reuse existing production readers and the existing assets crate.** A new
   reader, asset format, or profile crate would duplicate trust boundaries.
   The maintainer command composes already-reviewed identity APIs and adds only
   the small canonical tuple.
4. **Do not package upstream raw inputs.** The profile names only the derived
   SNV index, ONNX bundle, compiled RefSeq sequence bundle, and compiled GENCODE
   mask. Zenodo source TSVs, NCBI FASTA, GENCODE GTF/gffutils data, and original
   checkpoint containers are not runtime profile members.
5. **No recursive self-ID field.** The profile ID is the SHA-256 of its exact
   canonical bytes and is reported beside them. Embedding that digest in the
   bytes it hashes adds no trust and complicates deterministic construction.

## Dependencies

Tickets 006, 008, 011, 014, 018, 019, 021, 022, and 023.

## Notes

- Reuse the retained production assets. Do not rebuild the 15 GB SNV index,
  772 MB reference member, 6.7 MB mask, or 33.9 MB model bundle.
- The SNV release already exists publicly; this ticket performs no network
  access and no GitHub mutation.
- The mask is one selected `domains.pgm` member rather than a new bundle
  format. Later packaging may wrap it, but must preserve its exact qualified
  bytes and identity.
- The production candidate is not a public release manifest. Publication
  prerequisites remain blocking and are handled only by the later publication
  outcome.
- A grammar-valid miniature profile is a library test fixture only. The
  shipped preparation command has one accepted production tuple and no
  `--allow-synthetic`, `--force`, or compatibility-bypass option.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted after Ticket 023 shipped persistent exact model-result caching. The
rolling frontier calls for a coherent installed profile next; this ticket
deliberately freezes only the small compatibility authority so installation,
activation, and delivery can follow as separately reviewed bounded slices.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket024_design_review`

Initial verdict: **REJECT**. The reviewer found that the first draft did not pin
the accepted production tuple or exact JSON shape, redundantly requested model
checkpoint/graph fields already bound by the bundle ID, contradicted the
existing SNV mmap boundary, overclaimed same-size corruption detection for the
15 GB score payload, made miniature CLI success impossible under exact
production rejection, underspecified canonical ordering and small-file
publication durability, and retained only a prose artifact.

The coordinator pinned every selected component/member identity and literal
policy, froze the typed JSON skeleton and RFC 8785/JCS rules, separated grammar
from production trust, scoped SNV admission to bounded metadata while retaining
installer/certifier authority for payload integrity, required held-descriptor
model/reference/mask authentication without inference, separated typed
miniature tests from the sole coordinator production invocation, specified
durable atomic no-replace output and stable errors, and named both exact JSON
and Markdown retained artifacts.

The reviewer found one stale checklist sentence that still implied a successful
miniature CLI preparation. The coordinator corrected it to cover typed
miniature construction/serialization while retaining the production-only
command boundary.

Revised verdict: **ACCEPT**. The reviewer confirmed the ticket is bounded,
internally consistent, feasible against current APIs with the explicitly
scoped admission additions, and implementation-ready.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket024_implementation`

Implemented the strict `pangopup.runtime-profile.v1` typed/JCS contract,
external content identity, exact pinned production trust check, and bounded
held-descriptor SNV metadata admission in `pangopup-assets`. Added
model-bundle inspection that authenticates every bounded member without
constructing an ONNX session, reusing the existing identified reference and
mask opens for their exact retained descriptors.

Added the production-only `pangopup-build runtime-profile prepare` command. It
composes the four authenticated inputs, rejects every mixed tuple, and
publishes canonical bytes through descriptor-relative private staging,
`renameat2(RENAME_NOREPLACE)`, file sync, and held-parent-directory sync. It
does not read the SNV score payload, initialize ONNX, infer, install, download,
activate, or publish assets.

Inside-out coverage proves deterministic canonical round trips, duplicate and
extension rejection, JSON-safe bounds, external identity sensitivity for
every compatibility leaf, synthetic trust rejection, bounded SNV metadata
inspection, held-path replacement detection, model inspection without a
session, private no-replace output, and no partial final output. Existing
identified reference/mask tests continue to cover full authentication,
same-size corruption, and pathname replacement. The executable spec covers
the closed production-only CLI grammar, stable bounded errors, and absence of
partial output.

Documentation now includes ADR 0020 and updates the README, architecture,
service boundary, FAQ, frontier, and repository contract. It explicitly keeps
installation/activation, network sync, publication, runtime discovery, HTTP,
Docker, and systemd as future work.

Evidence:

- focused runtime-profile, publisher, and model replacement tests: green;
- `make lint`: green;
- `make test`: green across the workspace; six pre-existing
  coordinator/maintainer production tests were ignored as intended;
- `make spec`: 178 passed, 3 skipped;
- `git diff --check`: green.

The coordinator then ran the production-only command once against the retained
qualified assets. It produced the exact 1,366-byte mode-`0600` canonical file
`planning/artifacts/024-four-asset-runtime-profile.json`; its profile ID and
file SHA-256 are both
`0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
The final byte is `}` with no trailing newline. The exact command, input paths,
component identities, output receipt, and scope boundaries are retained in
`planning/artifacts/024-four-asset-runtime-profile.md`. No runtime asset was
copied, rebuilt, installed, activated, downloaded, or published.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket024_code_review`

Initial verdict: **REJECT**. The reviewer found two blockers:

1. SNV runtime-profile admission and the model's full authenticated inspection
   enumerated an untrusted directory into a collection without stopping at the
   fixed three-member contract.
2. Model, reference, and mask inspection failures were all collapsed into
   `PROFILE_INCOMPATIBLE`, hiding missing/I/O, unsafe path/member shape, and
   malformed/hash/structural corruption.

The developer remediated only those findings. Both enumerators now reject on
the fourth observed entry before inserting it, retain the exact three-member
set check, and have fast 64-entry regression tests. Preparation now validates
each authenticated component against the pinned tuple before opening the next
and maps missing/I/O to `INPUT_IO`, true tuple/version/profile differences to
`PROFILE_INCOMPATIBLE`, symlink/path/member-shape failures to `PROFILE_UNSAFE`,
and malformed/hash/structural failures to `PROFILE_CORRUPT`. Fixed redacted
messages never include untrusted paths, reasons, or contents.

Focused library tests cover all four categories for model, reference, and mask.
Executable CLI cases cover representative missing, incompatible, unsafe, and
corrupt SNV inputs and prove later paths remain unopened. Post-remediation
gates are green: `make lint`, complete `make test`, `make spec` with 184
passed/3 skipped, and `git diff --check`.

The retained production JSON and Markdown were not regenerated or edited
during remediation. The JSON remains 1,366 bytes with SHA-256/profile ID
`0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
Re-review verdict: **ACCEPT**. The same reviewer confirmed both blockers are
resolved, the fourth-entry bounds and distinct redacted error categories have
focused and CLI coverage, no material findings remain, and the production
artifact hash/size/mode are unchanged.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

Final coordinator gate after accepted code review: `make lint`, complete
`make test`, `make spec` (184 passed, 3 skipped), and `git diff --check` all
pass. The retained JSON remains 1,366 bytes with profile ID/file SHA-256
`0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
Documentation consistently leaves installation, activation, sync,
publication, runtime discovery, HTTP, Docker, and systemd as future work.
