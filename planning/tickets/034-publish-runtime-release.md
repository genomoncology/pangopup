# 034 — Publish the exact immutable model-side runtime release

Status: ready

## Why

Ticket 033 prepared and qualified one closed, read-only release stage for the
model, compact GRCh38 reference, and compact splice mask. Users still cannot
download those assets from Pangopup. The next bounded outcome is to publish
those exact derived runtime bytes once, without rebuilding or recompressing
them and without republishing any raw NCBI, GENCODE, or Zenodo source.
Pangolin preferred source is intentionally included beside the GPL-derived
model.

This ticket is an exceptional public-effect ticket. Preparation and review are
ordinary repository changes; only the coordinator may create or publish the
GitHub release, and only after the publication-ready commit passes its exact
remote gate.

## Scope

- Publish one immutable GitHub release in `genomoncology/pangopup`:
  - tag: `runtime-grch38-v1`;
  - title: `Pangopup GRCh38 Pangolin runtime v1`;
  - target commit:
    `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`;
  - body: the exact amended Ticket 034 `RELEASE-NOTES.md`.
- Use this retained, qualified stage as the only runtime-byte source:
  `/home/ian/workspace/data/pangopup-runtime-release-033/e6d8497aaf1e3db521360ad969252a2ec6fd14e4`.
- Upload the exact 12 runtime files:
  `runtime-transport.json`, `runtime-profile.json`,
  `model-manifest.json`, `model-NOTICE`, `model.onnx.zst`,
  `reference-manifest.json`, `reference-NOTICE`, `reference.pgr.zst`,
  `mask-NOTICE`, `domains.pgm.zst`, `runtime-release-profile.json`, and
  `SHA256SUMS`.
- Also upload exactly three GPL preferred-source files prepared and qualified
  before code review:
  - `pangolin-5cf94b8-source.tar.zst`: a deterministic archive of the complete
    tracked upstream Pangolin tree at
    `5cf94b8db938c658391b4305cd7ce33297d44ff7`, including all 12 checkpoints,
    `pangolin/model.py`, build metadata, and the license;
  - `pangopup-e6d8497-source.tar.zst`: a deterministic archive of the complete
    tracked Pangopup tree at
    `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`, including the converter,
    manifests, lockfile, build instructions, and model-qualification source;
  - `Pangolin-GPL-3.0.txt`: a byte-identical copy of upstream `LICENSE`.
- `RELEASE-NOTES.md` supplies the release body and is not an asset. Copy the
  Ticket 033 notes into the new Ticket 034 preferred-source stage, append a
  deterministic section identifying all three preferred-source assets and
  explaining that no NCBI FASTA, GENCODE input, or Zenodo source dataset is
  mirrored, and leave the read-only Ticket 033 stage unchanged.
- Before any GitHub mutation, reauthenticate both complete retained inputs:
  - exact 13-name runtime-stage inventory, regular-file types, link counts,
    owner, modes, sizes, and all SHA-256 values from Ticket 033, including
    `SHA256SUMS`
    `ad5dfff9838b2d17a2d216954b241002b9717dd6215590f28ba3edf0ba2be59f`
    and the pre-amendment `RELEASE-NOTES.md`
    `dba5e20693435d0b94368d7993f18e39e2bd31c4609de10602e4c5e204b5ba1f`;
  - all `SHA256SUMS` entries;
  - byte equality of the ten copied members against the retained Ticket 030
    transport;
  - exact four-name Ticket 034 supplement inventory (three upload assets plus
    amended `RELEASE-NOTES.md`), root and member types, coordinator ownership,
    private read-only modes, link counts, sizes, and SHA-256 values recorded in
    the reviewed durable artifact;
  - one fresh bounded reconstruction of each retained source archive proving
    the reviewed closed inventory and required member identities, followed by
    pathname/inode/metadata revalidation so upload uses those admitted regular
    files;
  - exact target commit present on public `main` with its GitHub `gate`
    successful;
  - public repository visibility and GitHub immutable releases enabled;
  - no existing `runtime-grch38-v1` tag or release.
- The coordinator prepares preferred source from exact Git objects before code
  review, never from mutable worktrees:
  - require both local repositories' configured public remotes and exact
    commits to match the release-profile records;
  - use `git archive` over each exact commit and one explicitly recorded,
    pinned deterministic Zstandard encoder contract;
  - authenticate the reconstructed archive inventories, including all 12
    checkpoint identities, `pangolin/model.py`, upstream `LICENSE`, the
    Pangopup converter paths, `Cargo.lock`, and build instructions;
  - publish the three source files and amended local-only release notes into an
    absent private read-only retained directory by no-replace rename, record
    their exact sizes and SHA-256 identities in
    `planning/artifacts/034-public-runtime-release.md`, and never regenerate
    them during upload.
- Prove preferred-source consistency before publishing the converted model:
  - fetch and verify the 12 exact upstream Pangolin checkpoints, `pangolin/model.py`,
    and upstream `LICENSE` from commit
    `5cf94b8db938c658391b4305cd7ce33297d44ff7` against the sizes and SHA-256
    values in `runtime-release-profile.json`;
  - verify the two converter paths named by that profile exist at the exact
    public Pangopup target commit;
  - prove those same objects occur in the retained preferred-source archives.
- Use only the authenticated official `gh` executable for GitHub mutations.
  Do not add an uploader, wrapper, retry supervisor, workflow, token handling,
  or product command.
- Create a draft first. Upload each exact asset once without replacement or
  `--clobber`, checking the complete 15-asset remote draft inventory and GitHub-reported
  size/digest after every upload. A failed upload or mismatch stops the
  operation.
- Immediately before publication, recheck the exact tag, target, title, body,
  closed 15-asset inventory, sizes, and digests. Publish once, then require
  `draft=false` and `immutable=true`. A mutable completed release is failure,
  not a fallback.
- Before publication only, a failed draft may be removed by exact release ID
  and its exact generated tag may be removed by exact ref after confirming both
  still belong to this operation. Stop after cleanup; do not retry in the same
  run. After publication, never delete, replace, or mutate the immutable
  release.
- After publication, perform bounded unauthenticated reads of the public
  repository/release metadata, every small runtime asset, the standalone
  license, and the source-archive headers, checking exact bytes/ranges.
  Do not download either large compressed frame (`model.onnx.zst`,
  `reference.pgr.zst`) or the smaller mask frame a second time; their local
  identities and GitHub-reported digests are the proof.
- Update:
  - `README.md`;
  - `architecture/delivery.md`;
  - `planning/frontier.md`;
  - `planning/faq.md`;
  - `planning/artifacts/034-public-runtime-release.md`.
- Exclude runtime remote sync/install UX, top-level `pangopup sync`/`status`,
  HTTP, Docker, executable releases, signing, SBOM generation, provenance
  attestations, and any runtime-asset rebuild, recompression, or format change.
  Those remain later outcomes.

## Success Checklist

- A reviewed publication plan records exact preferred-source preparation,
  preflight, draft, upload, verification, publication, bounded public-read,
  evidence, and prepublication rollback commands using `gh`.
- The publication-ready repository change contains no credentials, raw API
  dumps, local absolute paths in user-facing docs, or GitHub mutation code.
- The exact publication-ready commit passes `make lint`, `make test`, and
  `make spec` locally and its exact GitHub `gate`.
- The public release resolves to the pinned target, contains exactly the 15
  reviewed assets, and reports `immutable=true`.
- All remote asset sizes and GitHub SHA-256 digests match the retained stage.
  Every bounded unauthenticated small-asset read is byte exact.
- The durable artifact records preferred-source archive inventories and
  identities, redacted preflight facts, release/tag IDs, upload attempts,
  remote inventory/digests, immutable state, bounded public checks, and the
  public release URL. It records that no NCBI, GENCODE, or Zenodo source,
  rebuild, recompression, replacement, or retry occurred.
- Documentation clearly distinguishes the now-public runtime release from the
  still-future automatic download/install commands.

## Decisions

1. **One release or separate component releases.** Separate releases could
   update components independently but permit users to assemble an unqualified
   tuple. Publish the one compatibility-profile-bound runtime release because
   Pangopup has qualified the model, reference, and mask only as that tuple.
2. **Official CLI or custom uploader.** A custom uploader could add
   instrumentation but previously created unnecessary process and retry
   machinery. Use the authenticated official `gh` executable and retain
   redacted observations in the evidence artifact.
3. **Draft-first or direct publication.** Direct publication exposes partial
   state while large files upload. Use a private draft, prove the closed remote
   inventory, and publish exactly once.
4. **Public proof of large bytes.** Re-downloading roughly 692 MB would consume
   bandwidth without adding identity evidence beyond GitHub's uploaded digest.
   Trust the locally reauthenticated source plus GitHub-reported SHA-256 for
   frames; bound public byte reads to metadata and small assets.
5. **Source links or self-contained preferred source.** Upstream immutable
   links identify the original objects but Pangopup does not control their
   lifetime. Publish exact upstream and Pangopup source archives plus the GPL
   text beside the immutable model, while retaining the upstream URLs as
   provenance. This adds a small download only for people who need source and
   makes the release independently modifiable without mirroring unrelated
   NCBI, GENCODE, or Zenodo inputs.

## Dependencies

- Ticket 033, complete.
- GitHub Actions run `30582599182` for the implementation commit, successful.
- Retained qualified runtime stage and Ticket 033 evidence.

## Notes

- `runtime-release-profile.json` and `SHA256SUMS` are upload assets;
  `RELEASE-NOTES.md` is only the exact release body.
- The exact 12-file runtime upload payload is 691,883,641 bytes. Re-derive that
  value from the retained stage during preflight, then add the three reviewed
  preferred-source sizes to derive the exact 15-asset total.
- Never print authentication headers, environment variables, raw `gh auth`
  output, or URLs containing credentials. Commit only bounded redacted facts.
- The coordinator owns the public effect. The developer prepares documentation
  and the exact operation/evidence skeleton but performs no network mutation.

## Coordinator Authorship

Coordinator: `/root`

Drafted from the qualified Ticket 033 stage and its observed exact production
identities. No GitHub release, tag, or asset mutation occurred during
authorship.

## Independent Ticket Review

Reviewer: `/root/ticket034_design_review`

First review: REJECT.

- Ticket 033 had not durably recorded the hashes of `SHA256SUMS` and
  `RELEASE-NOTES.md`, leaving two mutable publication inputs without an
  independent expected identity.
- The proposed model release carried a notice and upstream URLs but neither a
  GPL license copy nor Pangopup-controlled preferred source for the lifetime of
  the immutable binary.

The coordinator accepted both findings. Ticket 033 evidence now pins the two
missing hashes. This revision adds a standalone GPL copy and complete exact
upstream/Pangopup source archives beside the converted model, with
deterministic preparation, reconstructed-inventory qualification, retained
identities, and exact 15-asset remote verification. The upstream URLs remain
provenance; NCBI, GENCODE, and Zenodo raw sources remain excluded.

Final re-review: ACCEPT. The reviewer confirmed that both retained input sets
are closed and reauthenticated before any GitHub mutation, supplement
generation is coordinator-owned before code review, exact Git-object source
archives and the standalone GPL text complete the preferred-source boundary,
and draft-first publication, bounded rollback, remote digests, immutability,
and public verification are independently testable.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: pending

Use the exceptional lifecycle:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator external effect -> complete -> commit/push -> cleanup
```

## Coordinator Final Check

Coordinator: pending
