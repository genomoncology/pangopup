# 027 — Separate reference reading from byte-producing provenance

Status: complete

## Why

Ticket 026 proved that installed model fallback must mmap the exact
`reference.pgr` descriptor authenticated by the installer. Pangopup already has
one qualified `PGRREF01` reader that can do this safely, but its 1,829-line
module also contains the byte-producing writer. The v1 reference builder
fingerprint hashes that entire mixed file.

Two attempted workarounds were rejected: reopening the member by pathname left
a race, while copying the reader into runtime admission created a second
decoder. The next bounded outcome is the prerequisite boundary correction:
retain one reader, give it one held-descriptor entry point, and make future
builder provenance describe only code that can produce artifact bytes. The
existing 772 MB production member and canonical runtime profile remain
unchanged and must not be opened or rebuilt.

## Scope

- Mechanically split the current reference implementation while preserving
  existing public `pangopup_index::reference::*` paths and behavior:
  - a shared wire/layout module for manifest types, fixed `PGRREF01` layout,
    codecs, and constants used to produce bytes;
  - one byte-producing writer module;
  - one mmap reader/provider module containing the existing structural parser,
    packed decoder, ambiguity overlay, and query implementation;
  - a thin facade that preserves current exports;
  - separate the build adapter's byte-producing path from post-build
    certification, inspection, qualification, compatibility-corpus, and
    evaluation logic. This split is mandatory: the current build file mixes
    those responsibilities and v2 cannot honestly exclude certification-only
    code while it remains mixed.
  Move code mechanically; do not create a second parser, decoder, writer, magic,
  format, or public provider trait.
- Add one crate-private held-descriptor constructor to the existing reader
  structural core. `reference_admission` exposes one opaque installed-reference
  capability. Its raw constructor is an explicitly `unsafe` authority
  boundary (or equivalently sealed): callers must provide canonical bounded
  manifest/NOTICE bytes and the exact immutable member descriptor already
  authenticated by installation. Safe downstream code cannot construct a
  claimed production identity from an arbitrary file.
- The held constructor maps and retains the supplied descriptor and never
  reopens `reference.pgr` or its parents by pathname. It performs the same
  bounded header/directory/padding/ambiguity validation and exposes the same
  zero-allocation `ReferenceProvider` query implementation as ordinary open.
  A deterministic substitute-and-restore hook must prove the mapped provider
  remains on the held inode.
- Introduce `pangopup.reference-builder-source.v2`. Its exact framed inventory
  contains only:
  - shared wire/layout and byte codec inputs;
  - the writer;
  - the byte-producing build adapter;
  - truly shared causal types/errors;
  - exact causal root wiring and locked dependency projections.
  Reader API/visibility, mmap opening, query decoding, certification-only
  evaluation, runtime admission, CLI, delivery, and service code are excluded.
  Do not change SNV provenance.
- Do not hash the mixed `pangopup-core/src/lib.rs` in v2. Instead add a checked
  canonical `reference-core-contract.v2` projection derived from the compiled
  `Grch38Contig` behavior used by byte production: all 25 numeric codes,
  canonical display strings, `from_code` round trips, and the exact chrM
  distinction. The fingerprint includes that projection; independent tests
  derive it from the real core API and reject omission, reordering, duplicate
  codes/names, or behavior drift. Unrelated core API/source edits change
  neither reference v2 nor its behavioral projection. Ticket 027 does not edit
  core and preserves the current SNV fingerprint; future core edits retain
  SNV v1's existing whole-core fingerprint-churn semantics. Do not move or
  edit the core type merely to make the reference inventory smaller.
- Keep v1/legacy provenance readable. Builder provenance is descriptive, not a
  runtime compatibility key. A future build emits v2; no existing manifest,
  bundle ID, member identity, or runtime profile is rewritten.
- Prove the boundary with only checked miniature inputs:
  - use the checked current-v1 miniature oracle from
    `pangopup-reference-mini-v1`: `reference.pgr` size 4,560/SHA-256
    `0ef815ffb3fbb897e880e56afcb57e1edb41f3707784f591c0457581c2e9a3d5`,
    `NOTICE` size 279/SHA-256
    `faea3b1976bf4e15f95bad3906144d83b4441f860d3c5b87ab406205e47262db`,
    and current-v1 canonical manifest SHA-256
    `8617204d0678ea23aa00e288e94bbf2622cf3884cf26562f65fb85eda5b18bd2`.
    The v1 oracle is pinned independently before v2 output is compared; the v2
    builder must not generate both sides of its own oracle;
  - the current-v1 oracle and v2 miniature build have byte-identical
    `reference.pgr` and `NOTICE`;
  - their canonical manifests differ only at
    `builder.source_sha256` (and no payload/source/profile fact);
  - existing legacy/v1 manifests still open and return the same windows;
  - mutation controls show every wire/writer/build-input edit changes v2,
    while reader-only API/query edits do not;
  - dependency/root-wiring derivation is complete and independently checked;
  - ordinary open, identified open, held open, build certification, all 25
    synthetic contigs, ambiguity/boundary/corruption, concurrency,
    zero-allocation, and existing route-reference cases remain on the same
    reader implementation;
  - held construction cannot be reached through a safe raw-file
    claimed-identity API.
- Pin static preservation facts without opening production files:
  - production reference bundle ID
    `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
  - member size `772091760` and SHA-256
    `sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`;
  - sequence-set SHA-256
    `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`;
  - Ticket 024 profile identity
    `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
- Retain a concise migration/proof note at
  `planning/artifacts/027-reference-reader-provenance-boundary.md`.
- Amend/supersede ADR 0012 and update `architecture/reference.md`,
  `architecture/index.md`, `planning/frontier.md`, and `AGENTS.md`. Update
  `README.md` only if a current provenance statement becomes false. State
  plainly that this changes future reference-builder provenance only; it does
  not create, rebuild, read, copy, repack, install, or publish a production
  asset. Installed-profile consumption remains the next ticket.

Explicit exclusions: Ticket 026 CLI/runtime routing, restoring the rejected
stash, model/mask/SNV behavior, a new reference format, payload changes,
production build or qualification, opening/copying the retained 772 MB member,
networking, GitHub release assets, publication, HTTP, Docker, service
lifecycle, GPU, and external effects.

## Success Checklist

- Pangopup has one `PGRREF01` parser/decoder/provider and one writer, with
  existing public paths and behavior preserved.
- The exact held descriptor can be mapped through the shared reader behind an
  opaque installed-admission capability, with no pathname reopen or safe
  synthetic identity bypass.
- Reference builder provenance v2 changes for byte-producing inputs and not
  for reader-only/runtime changes.
- Miniature payload and notice bytes remain identical; existing v1/production
  identities remain valid and statically pinned without a production read or
  rebuild.
- Fast focused tests and the normal gate prove the migration.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Split provenance before runtime consumption.** Combining a format-module
   migration with the already-large rejected routing diff would make both
   harder to review. Ticket 026 remains deferred until this prerequisite is
   independently complete.
2. **One reader implementation.** A copied scalar decoder may appear safer
   than changing provenance, but it creates permanent semantic drift. Ordinary,
   identified, qualification, and held opens must converge on one structural
   parser and query path.
3. **Version provenance, not the artifact format.** Payload bytes and
   `PGRREF01` semantics do not change. The new domain is
   `pangopup.reference-builder-source.v2`; existing v1 manifests remain valid.
4. **Fingerprint byte production, not every gatekeeper.** Reader and
   certification changes can decide whether bytes are accepted but cannot
   produce different bytes. V2 binds wire/layout, writer, build adapter, and
   causal dependencies; separate tests continue to qualify readers and
   certification.
5. **No production rebuild.** Static constants, legacy fixtures, and
   byte-identical miniature builds prove compatibility. Reading or rebuilding
   the retained production member would add no useful evidence.

## Dependencies

Tickets 011 and 013. Design finding from deferred Ticket 026.

## Notes

- The rejected Ticket 026 implementation is preserved locally as stash
  `ticket-026-rejected-runtime-consumption`; do not apply it during this
  prerequisite.
- Start from clean `main`. Do not use the rejected duplicate reader as source.
- The v1 algorithm and fixtures remain historical compatibility evidence. V2
  should reuse the established length-framed, inventory-checked machinery
  where semantics still fit rather than inventing an unrelated hasher.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted after Ticket 026 implementation and code review exposed that the
reference builder's causal source inventory includes reader-only code. This
ticket owns only that prerequisite boundary and preservation proof.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket027_design_review`

Initial verdict: **REJECT**. The reviewer found three mistyped production
digests, an incomplete v2 treatment of mixed `pangopup-core` behavior,
optional rather than mandatory build/certification separation, and no explicit
independent current-v1 miniature oracle.

The coordinator corrected all three identities from Ticket 024, required a
checked behavior-derived `Grch38Contig` projection without changing core or SNV
v1, made the byte-production/certification split mandatory, and pinned the
current-v1 miniature payload/notice/manifest oracle independently of v2.

The reviewer found two remaining factual errors: the miniature manifest hash
predated Ticket 020, and the draft overstated SNV v1 isolation from future core
edits. The coordinator replaced the manifest hash with the independently
retained current-v1 value and stated the asymmetric contract precisely:
Ticket 027 preserves today's SNV fingerprint, future SNV v1 keeps its current
whole-core churn behavior, and unrelated core behavior cannot churn reference
v2.

Revised verdict: **ACCEPT**. The reviewer confirmed that the corrected
current-v1 miniature oracle, Ticket 024 production identities, asymmetric core
contract, mandatory build/certification split, single-reader held-descriptor
boundary, preservation tests, and no-production-read scope are coherent and
feasible.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket027_implementation`

- Split `PGRREF01` mechanically behind the existing public facade into shared
  wire/layout, one writer, and one reader/provider. Ordinary, identified,
  qualification, and installed held opens converge on that reader.
- Added the explicit unsafe installed-descriptor admission boundary and opaque
  safe provider. A deterministic substitute-and-restore test proves queries
  remain on the held inode.
- Split byte-producing build code from certification/inspection/qualification
  code. Reference builder provenance v2 contains only causal byte inputs,
  locked dependency/root projections, and the behavior-derived 25-contig core
  contract. SNV v1's hard fingerprint remains unchanged.
- The checked current-v1 miniature oracle and v2 build have identical
  `reference.pgr`/`NOTICE`; changing only `builder.source_sha256` reconstructs
  the pinned v1 canonical manifest. Legacy manifests still open.
- Static tests retain all Ticket 024 production identities without opening the
  production member. No production build/read/copy/install/network or external
  effect occurred.
- Initial code review found that grouped Rust imports escaped the root-wiring
  projection and that the two public reference facades were not causally
  bound. The implementation now derives and checks the
  `source_fingerprint` module edge plus a canonical facade-wiring projection
  for the crate-level public build facade declaration, build entry point, wire
  layout, and writer. Mutation controls prove those rebindings—including an
  exact `#[path="replacement.rs"] pub mod reference` outer rebind—change v2
  while reader-only facade exports remain excluded.
- Focused reader/admission, builder provenance, miniature/reference, resource,
  corruption, concurrency, zero-allocation, and legacy tests pass.
- Full post-remediation developer gate: `make lint` passed; `make test` passed;
  `make spec` passed with 194 passed/3 skipped; `git diff --check` passed.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket027_code_review`

Initial verdict: **REJECT**. The reviewer found that the v2 root projection
missed a grouped `source_fingerprint` module import and that public facade
reexports could redirect the build or index implementation without changing
the digest. Both gaps are remediated as described above; same-reviewer
verification rejected one remaining outer edge.

First re-review verdict: **REJECT**. The inner facade edges were bound, but the
outer `pangopup-build/src/lib.rs` declaration could still redirect the entire
public facade. The checked facade projection now independently derives that
public module edge, and its exact path-rebinding mutation control changes the
v2 digest.

Final verdict: **ACCEPT**. The reviewer confirmed that the outer module edge,
fingerprint-module edge, build/index causal facade edges, and exact mutation
controls close every identified routing bypass while reader-only exports
remain neutral. The final v2 digest is
`09cd44449b77592e4b9948cc0756e736b01ecf5220b3d5312c52b12b6b6e9c65`;
no material finding remains.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

Reviewed the accepted final diff, verified the rejected Ticket 026 stash
remained unapplied, and reran the repository gate on 2026-07-27:
`make lint`, `make test`, `make spec` (`194 passed, 3 skipped`), and
`git diff --check` all passed. Production identities are tested statically;
no production reference member was opened, read, copied, rebuilt, installed,
repacked, or published.
