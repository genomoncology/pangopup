# 030 — Package the qualified model-side runtime assets

Status: ready

## Why

Pangopup already has the exact qualified model, compact RefSeq sequence, and
GENCODE splice-mask members needed when an SNV lookup misses or the input is not
an SNV. One canonical runtime profile binds those three members to the shipped
SNV index. Today those model-side bytes exist only as retained local build
outputs. A clean machine cannot receive them in a deterministic, inspectable
form.

This ticket creates the local packaging boundary around those accepted bytes.
It is intentionally before, and separate from, any public GitHub upload. That
keeps a reversible filesystem operation independent from the unresolved
repository-security and release-process blockers.

## Scope

- Add a `pangopup-build runtime-transport` command family:
  - `pack --profile <FILE> --model-bundle <DIR> --reference-bundle <DIR>
    --mask <FILE> --output <ABSENT_DIR>`;
  - `verify --transport <DIR>`;
  - `unpack --transport <DIR> --output <ABSENT_DIR>`.
- `pack` must authenticate a canonical, internally consistent runtime profile
  and prove that the exact supplied model, reference, and mask members match
  that profile with their existing inspectors before writing output. This
  generic integrity operation does not promote a profile to trusted
  production; the existing installer continues to own that admission check.
  `pack` must not open, hash, copy, compress, or otherwise touch the 15 GB SNV
  score member; the profile's SNV identity is metadata only in this transport.
- Produce one flat, closed local transport directory containing exactly these
  ten regular files:
  - `runtime-transport.json`;
  - `runtime-profile.json`;
  - `model-manifest.json`;
  - `model-NOTICE`;
  - `model.onnx.zst`;
  - `reference-manifest.json`;
  - `reference-NOTICE`;
  - `reference.pgr.zst`;
  - `mask-NOTICE`;
  - `domains.pgm.zst`.
  `runtime-profile.json`, both component manifests, and both component notices
  are byte-exact copies of their authenticated inputs. `mask-NOTICE` is the
  byte-exact checked repository file
  `assets/notices/GENCODE-v38-NOTICE`. The three `.zst` files are deterministic
  single frames of their correspondingly named runtime members. Filesystem
  directory enumeration order is irrelevant; only the canonical ordered
  member array in `runtime-transport.json` defines order and roles. That
  canonical manifest has one fixed-schema ordered member array declaring the
  other nine files and owns their exact names, roles, uncompressed sizes and
  SHA-256 digests, compressed sizes and SHA-256 digests, encoder identity,
  runtime-profile identity, and attribution-member identities.
  `runtime-transport.json` is the schema-implied tenth file rather than a
  member of its own array; the SHA-256 of its exact canonical bytes is the
  transport ID. This avoids an impossible self-hash while still closing the
  complete directory inventory. Extra, missing, substituted, symlinked, or
  non-regular entries and noncanonical manifest member order fail closed.
- Reuse the repository's pinned bundled libzstd 1.5.7 transport settings:
  level 9, checksum and content size enabled, dictionary ID disabled, no
  long-distance mode, and zero workers. Each payload is a separate frame so a
  later release can keep model, reference, and mask independently named and a
  caller can avoid downloading assets it does not need. Do not add tar, zip,
  an archive library, a second compression implementation, or generic
  filesystem recursion.
- `verify` must stream and authenticate every declared compressed and
  decompressed byte without materializing the outputs or running inference.
  It proves transport integrity and exact identity, not publisher identity or
  model semantics.
- `unpack` must reconstruct byte-identical inputs in a private same-filesystem
  stage while each compressed stream is read and decompressed exactly once.
  The same streaming pass authenticates compressed and reconstructed sizes and
  digests; only after all ten members and all staged outputs verify may it
  publish:
  - `runtime-profile.json`;
  - `model/{manifest.json,NOTICE,model.onnx}`;
  - `reference/{manifest.json,NOTICE,reference.pgr}`;
  - `mask/{NOTICE,domains.pgm}`.
  Publish only by an atomic no-replace rename into an absent destination.
  Partial, corrupt, incompatible, or interrupted work must not expose a final
  output. The retained production unpacked output must be accepted unchanged
  by the existing offline runtime-profile installer. Miniature transports
  prove generic integrity and round-trip behavior but are deliberately not
  admitted as trusted production profiles.
- Keep memory bounded to fixed buffers plus existing bounded inspectors.
  Packaging, verification, and unpacking must stream the 772 MB reference
  member and may not hold a complete compressed or uncompressed payload in
  memory.
- Add a checked GENCODE v38 attribution notice that identifies the exact
  archived annotation source used for the derived mask, explains that
  `domains.pgm` is Pangopup's transformed runtime representation, and does not
  claim that Pangopup redistributes the upstream GTF or SQLite database.
- Generate one retained local production transport from the three already
  qualified members and the checked four-asset profile. Record its exact file
  inventory, sizes, SHA-256 identities, compression ratios, peak RSS, and
  pack/verify/unpack results in
  `planning/artifacts/030-derived-runtime-transport.md`. The production output
  remains outside Git and outside any public release.
- Add `spec/runtime-transport.md` covering deterministic miniature
  pack/verify/unpack, byte-exact reconstruction, closed inventory, corruption,
  truncation, substitution, symlink/non-regular inputs, conflicting output,
  and stable JSON/exit behavior. Add inside-out unit and integration tests for
  canonical manifest parsing, frame boundaries, streaming verification,
  cleanup, and unsafe filesystem shapes.
- Update the checked maintainer command/help catalog from Ticket 029 and update
  `README.md`, `architecture/delivery.md`, `architecture/runtime-data.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add
  `architecture/decisions/0023-derived-runtime-transport.md` to own the
  transport layout and integrity boundary.

Excluded: rebuilding or republishing the raw Zenodo score dataset; rebuilding
or distributing the NCBI FASTA or assembly report; rebuilding or distributing
the GENCODE GTF or SQLite database; rebuilding or changing the selected model,
reference, mask, SNV index, runtime profile, or score behavior; packaging the
15 GB installed SNV index; XDG installation changes; remote sync; URLs;
GitHub API or CLI calls; release creation or upload; executable packaging;
HTTP; Docker; model-result caching; inference optimization; and resolution of
the public repository/security or upload-process lifecycle issues.

## Success Checklist

- Two packs of the same miniature inputs are byte-for-byte identical in name,
  inventory, transport ID, metadata, notices, and compressed payloads.
- Verify authenticates all compressed and reconstructed bytes without creating
  an unpacked payload; corruption, truncation, substitution, extra entries, and
  unsafe file types fail closed.
- Unpack reconstructs every miniature input byte exactly in one streaming
  decode-and-stage pass. The retained production unpack then passes the
  existing runtime-profile installation boundary without inference or network
  access; the normal executable spec does not admit a synthetic profile as
  trusted production.
- The miniature executable spec fails against the pre-ticket binary and passes
  after implementation.
- The retained production run packages the exact profile
  `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`,
  model member of 33,867,142 bytes, reference member of 772,091,760 bytes, and
  mask member of 6,703,320 bytes with mask SHA-256
  `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
- Production packaging preserves the accepted bytes and stays below GitHub's
  per-asset size ceiling for each compressed payload. This is only a size
  qualification; no publication occurs.
- Peak RSS and output size are measured rather than inferred. No operation
  allocates a payload-sized buffer or reads the SNV score member.
- Current documentation clearly distinguishes shipped local packaging from
  future publication, sync, installation integration, and service work.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Package only derived runtime members.** The raw Zenodo, NCBI, and GENCODE
   sources already have authoritative upstream homes and are not runtime
   inputs. Republishing them would add storage and licensing work without
   helping inference. The accepted compiled bytes plus attribution are the
   package.
2. **Use one manifest-bound delivery set with three independent frames.** A
   single opaque archive would make the compatibility tuple easy to download
   but force every consumer to fetch every payload and would couple later
   release families. Three frames preserve independent model/reference/mask
   delivery while one canonical manifest prevents mixing incompatible bytes.
3. **Reuse pinned Zstandard rather than run another codec experiment.** These
   payload formats were selected for runtime speed and must be reconstructed
   byte-for-byte; transport compression is removed before mmap use. The
   repository already has deterministic, measured Zstandard machinery, and the
   reference and mask have accepted measurements. A new codec contest would
   optimize the third priority—download size—at the cost of the first two:
   performance and implementation simplicity.
4. **Keep SNV data out of this transport.** The public SNV release and XDG
   installation path are complete. The runtime profile binds its identity, but
   opening or repackaging 15 GB would repeat solved work and recreate the
   long-running verification behavior the project explicitly removed.
5. **Separate packaging from publication.** Local pack/verify/unpack is
   deterministic and reversible. GitHub upload is an external effect and
   remains blocked on the recorded security and process-lifecycle issues; it
   receives its own later reviewed ticket.
6. **Make complete integrity verification explicit, not startup behavior.**
   Transport verification necessarily streams every byte once. Normal runtime
   open and lookup remain bounded and mmap-based after unpack/install; this
   ticket does not add a recurring full hash to startup.

## Dependencies

Tickets 011, 012, 018, 024, 025, 027, 028, and 029.

Publication remains blocked by
`planning/issues/2026-07-24-publication-security-baseline.md` and
`planning/issues/2026-07-24-release-process-lifecycle.md`; neither issue is a
dependency for this local-only ticket.

## Notes

- The repository gate is exactly `make lint`, `make test`, and `make spec`.
- Use only the retained accepted production members. Do not regenerate a
  source archive, reference, mask, model, or compatibility corpus.
- The production inputs live outside the repository. Product code, specs, and
  checked documentation must contain no machine-absolute path, credential,
  hostname-specific fact, or production payload.
- The coordinator owns the one large production packaging run after the
  implementation's miniature gates are green and before adversarial code
  review. The developer supplies the exact command and bounded resource
  measurement method but does not publish or commit generated payloads.
- The retained transport is local evidence for the next delivery ticket. It
  must not be added to Git, Git LFS, a GitHub release, or another remote.
- Reuse the existing no-follow, held-descriptor, canonical-JSON, private-stage,
  atomic-publication, and deterministic-Zstandard primitives where they fit.
  Do not create a generic archive framework.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 029 command catalog, the exact Ticket 024
four-asset profile, the retained qualified asset inventory, and the current
frontier. It converts the next known outcome into one local, reversible ticket
while preserving publication as a later independently reviewed effect.

## Independent Ticket Review

Reviewer: Codex `/root/ticket030_design_review`

Initial verdict: **REJECT**. The reviewer found three material contradictions:
the production-only trust rule made a miniature executable pack test
impossible; the closed transport did not name its exact files or the source of
the mask notice; and unpack required a complete verification pass followed by
a second complete decompression.

The coordinator separated generic internally coherent transport packaging from
the existing installer's production-admission policy; limited miniature proof
to deterministic integrity and round trip while reserving installation proof
for the retained production run; fixed the exact ten-file flat inventory and
checked mask-notice path; made canonical manifest order authoritative while
filesystem enumeration order is irrelevant; and changed unpack to authenticate
compressed and reconstructed bytes during one private streaming stage before
atomic publication.

Revised verdict: **ACCEPT**. The reviewer confirmed that generic integrity and
production admission are now correctly separated, the exact layout and notice
source are closed, unpack performs only one streaming decode, and no new
material contradiction remains. Ticket 030 is ready for development.

Development then exposed one material contract defect before any code changed:
the manifest was required to declare exact metadata for all ten files,
including itself, creating an impossible recursive SHA-256/size fixed point.
The coordinator made `runtime-transport.json` the schema-implied file, limited
its ordered member array to the other nine files, and defined the transport ID
as the SHA-256 of the exact canonical manifest bytes. The directory remains
closed at exactly ten files without a self-hash.

Second revised verdict: **ACCEPT**. The reviewer confirmed that the manifest is
now finalizable, the verifier can enforce the exact ten-name set as the
schema-implied manifest plus nine declared members, and the transport ID
remains derived from—not serialized into—the canonical manifest. No related
material contradiction remains.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
