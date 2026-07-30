# 033 — Prepare the exact model-side runtime release

Status: complete

## Why

Ticket 030 built and qualified the exact ten-file model-side runtime transport,
but its retained directory is a mutable local build output, not a controlled
GitHub upload source. The next public release must reuse those bytes without
rebuilding or recompressing them and must bind their identities, attribution,
tag, and target commit before any network mutation.

This ticket creates that deterministic offline preparation boundary. It also
closes the stale claim that secret-scanning validity checks block publication:
Pangopup has no configured repository secrets or open secret alerts, while
ordinary GitHub secret scanning and push protection are already enabled.

## Scope

- Add:

  ```text
  pangopup-build runtime-release prepare \
    --transport <DIR> \
    --target-commit <40_LOWERCASE_HEX> \
    --output <ABSENT_DIR>
  ```

- The command is production-only and fixed to:
  - repository `genomoncology/pangopup`;
  - tag `runtime-grch38-v1`;
  - title `Pangopup GRCh38 Pangolin runtime v1`;
  - transport ID
    `sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`;
  - runtime-profile ID
    `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
- Authenticate the supplied transport through the existing streaming verifier.
  Copy its exact ten regular, singly linked members into a new private staging
  directory; never hardlink, rebuild, recompress, or accept an alternate
  production identity.
- Generate three publication files:
  - canonical `runtime-release-profile.json`;
  - deterministic `SHA256SUMS`;
  - deterministic `RELEASE-NOTES.md`.
- The release profile schema is
  `pangopup.runtime-release-profile.v1`. It is canonical RFC 8785/JCS JSON with
  no trailing LF and this closed shape:

  ```json
  {
    "schema": "pangopup.runtime-release-profile.v1",
    "profile": "runtime-grch38-v1",
    "repository": "genomoncology/pangopup",
    "release": {
      "tag": "runtime-grch38-v1",
      "title": "Pangopup GRCh38 Pangolin runtime v1",
      "target_commit": "<40 lowercase hex>",
      "page_url": "https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v1"
    },
    "runtime": {
      "profile_id": "sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c",
      "snv_bundle_id": "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3",
      "model_bundle_id": "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43",
      "reference_bundle_id": "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f",
      "mask_sha256": "sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"
    },
    "model_source": {
      "license": "GPL-3.0-only",
      "upstream_repository": "https://github.com/tkzeng/Pangolin",
      "upstream_commit": "5cf94b8db938c658391b4305cd7ce33297d44ff7",
      "model_py": {
        "path": "pangolin/model.py",
        "size": 3011,
        "sha256": "sha256:4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8",
        "url": "https://raw.githubusercontent.com/tkzeng/Pangolin/5cf94b8db938c658391b4305cd7ce33297d44ff7/pangolin/model.py"
      },
      "license_file": {
        "path": "LICENSE",
        "size": 35149,
        "sha256": "sha256:3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986",
        "url": "https://raw.githubusercontent.com/tkzeng/Pangolin/5cf94b8db938c658391b4305cd7ce33297d44ff7/LICENSE"
      },
      "checkpoint_set": "pangolin-1.0.2-5cf94b8-checkpoints-v1",
      "checkpoints": ["<the exact twelve name/size/sha256 records from Ticket 018, in ordinal order>"],
      "converter_repository": "https://github.com/genomoncology/pangopup",
      "converter_commit": "<same release target commit>",
      "converter_paths": ["tools/pangolin-model", "crates/pangopup-build"]
    },
    "transport": {
      "schema": "pangopup.runtime-transport.v1",
      "transport_id": "sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3",
      "members": ["<the ten name/role/size/sha256/url records in runtime-transport order>"]
    }
  }
  ```

  Each checkpoint record is a closed object with `ordinal`, `name`, `size`,
  `sha256`, and `url`; use the twelve literal name/size/digest records and
  order in `planning/artifacts/018-authenticated-cpu-model-kernel.md`. Its URL
  is
  `https://raw.githubusercontent.com/tkzeng/Pangolin/5cf94b8db938c658391b4305cd7ce33297d44ff7/pangolin/models/<name>`.
  Each transport
  member is a closed object with `logical_path`, `role`, `asset_name`, `size`,
  `sha256`, and `url`. `logical_path` and `asset_name` both equal the retained
  filename. The first member is `runtime-transport.json` with literal role
  `runtime-transport-manifest`; the remaining nine retain their authenticated
  manifest roles exactly. URLs are exactly
  `https://github.com/genomoncology/pangopup/releases/download/runtime-grch38-v1/<asset_name>`.
  The parser rejects extensions, duplicates, noncanonical bytes, wrong order,
  unsafe names, a target outside `[0-9a-f]{40}`, and any disagreement with the
  production identities. The document contains no local paths, timestamps,
  hostnames, credentials, aliases, or optional fields.
- `SHA256SUMS` covers the ten copied members plus
  `runtime-release-profile.json`, in that order, and does not list itself or
  `RELEASE-NOTES.md`.
- Release notes identify the compatible SNV bundle/release, the model,
  reference, and mask identities, the three included notice files, and state
  explicitly that raw Zenodo, NCBI FASTA, GENCODE GTF/SQLite, original
  checkpoint containers, and qualification fixtures are not included.
- Release notes also define the preferred model-modification source: the
  twelve exact `.v2` checkpoint containers and `pangolin/model.py` are tracked
  at upstream commit `5cf94b8…`, while Pangopup's authenticated converter and
  lockfile are in the release target's `tools/pangolin-model` and
  `crates/pangopup-build`. Ticket 034 must prove every pinned upstream
  checkpoint URL, `model.py` URL, and upstream `LICENSE` URL is publicly
  readable with the recorded size/SHA-256 and the converter paths are present
  in the exact public target commit. Pangopup does not mirror those
  already-public source inputs as runtime assets.
- Open the source directory and every member with no-follow descriptor-relative
  operations. Require a regular singly linked file; retain the descriptor;
  capture device/inode/size/change metadata; copy and SHA-256 through that
  descriptor; compare the copied digest and size with the authenticated
  manifest; and require stable metadata afterward. Before publication, repeat
  the closed source-directory inventory and prove each pathname still names
  the captured device/inode. A replacement or mutation fails.
- Sync and chmod every staged file to `0400`, chmod and sync the stage directory
  to `0500`, then publish by atomic no-replace rename and sync the parent. Every
  output file has link count one. A failure before rename leaves no final
  directory. A parent-sync failure after rename is reported explicitly as
  published-but-durability-unconfirmed and does not delete or claim absence of
  the visible final directory.
- `0500`/`0400` is a coordinator-controlled stable source, not protection from
  the owning user. Ticket 034 must repeat the complete name/type/mode/link/
  size/SHA-256 check immediately before uploading.
- Production preparation is coordinator-owned after the implementation commit
  and its remote gate pass. Retain it outside Git at:

  ```text
  /home/ian/workspace/data/pangopup-runtime-release-033/<TARGET_COMMIT>
  ```

- Update `README.md`, `architecture/delivery.md`,
  `architecture/runtime-data.md`, `planning/frontier.md`, and
  `planning/issues/2026-07-24-publication-security-baseline.md`. Mark the
  security issue closed and retain validity checks only as an optional GitHub
  feature, not a release requirement.
- Record the retained production result in
  `planning/artifacts/033-runtime-release-preparation.md`. The tracked artifact
  contains only identities, inventory, modes/link counts, canonical outcome,
  resource measurements, and pass/fail statements; the 691 MB staging stays
  outside Git.
- Do not upload, create a release/tag, download a large public asset, rebuild
  any runtime member, change installed formats, add an executable SBOM, or
  implement remote sync.

## Success Checklist

- Executable spec pins only the production CLI help, grammar, duplicate/missing
  arguments, target validation, and fail-closed rejection of a nonproduction
  transport. Normal CLI success is production-only.
- Library unit/integration tests use a hidden, test-build-only
  `RuntimeReleasePreparationContract`, analogous to the existing
  `ReleasePreparationContract`, to prove miniature success, exact
  profile/SHA256SUMS/notes bytes, final modes/link counts, and byte-identical
  repeated preparation. No user-facing arbitrary-contract option or
  environment variable exists.
- Tests reject invalid target commits; wrong production identity; missing,
  extra, corrupt, truncated, symlinked, multiply linked, or non-regular
  transport members; occupied output; and injected copy/sync/publication
  failures without exposing a partial final directory.
- A test proves source mutation or replacement during preparation fails rather
  than producing a mixed staging set.
- The preparation copies each retained member once and keeps memory bounded;
  it does not materialize uncompressed model/reference/mask bytes.
- Coordinator production evidence records the exact thirteen-file staging
  inventory, sizes, SHA-256 values, modes/link counts, command outcome,
  `/usr/bin/time -v` elapsed time, maximum RSS, filesystem input/output counts,
  total copied bytes, and byte equality with Ticket 030's ten retained files.
- Production evidence confirms 691,860,158 compressed frame bytes and no
  network, rebuild, recompression, or raw upstream input.
- `make lint`, `make test`, and `make spec` pass.

The exact successful stdout is canonical compact JSON plus one LF:

```json
{"status":"ok","command":"runtime-release.prepare","tag":"runtime-grch38-v1","target_commit":"<40 lowercase hex>","transport_id":"sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3","runtime_profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","upload_asset_count":12}
```

## Decisions

1. **Reuse versus rebuild.** Rebuilding would create a new transport identity
   without a scoring need. Copy and authenticate Ticket 030's retained bytes.
2. **Stable upload source.** The official `gh` CLI opens pathnames, so Ticket
   034 needs a closed, read-only, single-link staging directory. Create one
   deterministic copy rather than reviving a custom uploader or relying on the
   mutable retained transport.
3. **Release metadata.** Publish the ten runtime transport members directly,
   plus a release profile and checksums. Keep release notes as the release body,
   not an uploaded asset.
4. **Inventory versus SBOM.** The release contains data artifacts, not a new
   executable. The canonical release profile, component manifests, notices,
   and checksums are its inventory/provenance. Executable/container SBOM work
   stays with executable/container publication.
5. **Secret validity.** Keep GitHub scanning and push protection. Do not request
   organization authority for validity checks because Pangopup has no runtime
   secret and that triage feature does not affect these public assets.
6. **Model source availability.** The exact preferred checkpoint inputs already
   live in the pinned public upstream Git commit; the converter and lockfile
   live in Pangopup's public target commit. Freeze their names/digests/URLs in
   the release profile and require Ticket 034 to verify them. Do not duplicate
   them in the runtime download.

## Dependencies

Tickets 030–032.

## Notes

- Retained source:
  `/home/ian/workspace/data/pangopup-runtime-transport-030/transport`.
- Ticket 030 recorded 691,874,664 total directory bytes and the exact ten
  names/digests in
  `planning/artifacts/030-derived-runtime-transport.md`.
- `runtime-release-profile.json` and `SHA256SUMS` are the two additional upload
  assets. `RELEASE-NOTES.md` is local publication input only.
- Normal tests use only checked miniature transports. Production files never
  enter Git or routine CI.
- `pangopup-build` owns only argument parsing and JSON adaptation.
  `pangopup-assets` owns the closed runtime-release profile types/parser,
  production contract, held-descriptor preparation, and the hidden test
  contract.
- Public-repository hygiene forbids local absolute paths, credentials, raw API
  responses, generated production payloads, and retained scan reports in
  tracked files.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-07-30

Drafted from the accepted Tickets 033–035 plan, Ticket 030's retained exact
transport, Ticket 031's removal of the custom uploader, and Ticket 032's
completed repository hardening. The coordinator does not implement or approve
the product diff.

## Independent Ticket Review

Reviewer: `/root/ticket033_design_review`

First review: REJECT.

- The draft did not settle the repository's existing preferred model-source
  prerequisite.
- A production-only CLI could not satisfy the requested miniature executable
  success.
- The release-profile wire shape and command outcome were not exact.
- Verification and later pathname copying left a replacement/mutation race.
- Permission ordering, post-rename parent-sync behavior, durable evidence, and
  crate ownership were underspecified.

The coordinator accepted every finding. The revision freezes the exact public
upstream checkpoints/model source and public Pangopup converter source in the
profile and makes Ticket 034 verify them; moves miniature success behind a
test-only library contract; defines the closed canonical schema and stdout;
requires held-descriptor copy/hash plus pathname revalidation; applies modes
before no-replace publication; distinguishes post-publication parent-sync
failure; names the durable artifact and resource fields; and retains the
existing build-CLI/assets-library split.

The second review found two remaining contract holes. The coordinator assigned
the transport manifest the literal `runtime-transport-manifest` role, retained
the other nine authenticated roles, and added exact `model.py` and upstream
`LICENSE` verification to Ticket 034's source gate.

Final re-review: ACCEPT. No material finding remains. The reviewer confirmed
the exact source objects and future verification gate, production/test seam,
closed public schema/stdout, literal transport-manifest role, held-descriptor
copy/revalidation, truthful atomic publication, evidence contract, and crate
ownership.

## Implementation Evidence

Developer: `/root/ticket033_implementation`

Implemented the reviewed production-only `runtime-release prepare` CLI and the
closed canonical runtime-release profile in `pangopup-assets`. The preparation
authenticates stored and reconstructed identities through one bounded
streaming decode from the retained descriptor set, then copies the exact
closed stored member set once through those same descriptor-relative,
no-follow file handles, revalidates source path/inode/metadata, writes
deterministic profile/checksum/note bytes, applies `0400`/`0500`, and publishes
with atomic no-replace rename. A post-rename parent-sync failure is reported as
visible but durability-unconfirmed. No production preparation, network,
GitHub, rebuild, recompression, or materialized decode was performed.

The hidden miniature contract and fault seam cover deterministic success,
closed profile parsing, byte equality, modes/link counts, invalid targets and
identities, missing/extra/corrupt/truncated/symlinked/hardlinked/non-regular
inputs, occupied output, source replacement before and after semantic
verification, copy/sync/publication failures,
and truthful post-publication durability failure. The executable spec covers
help, exact grammar, duplicate/missing flags, target spelling, and normal-CLI
rejection of a valid nonproduction transport.

The first adversarial code review rejected three material gaps. Remediation
removed the two-view pathname inspection: one held directory and one retained
descriptor per member now supply the manifest, runtime profile, bounded raw
reads, streaming Zstandard semantic verification, exact stored-byte copy, and
final pathname/inode revalidation. The hidden contract now carries all ten
expected name/role/size/digest records, and the production parser binds every
record—including the runtime-transport manifest digest—to the fixed contract.
Canonical negative tests alter member size, member digest, manifest digest,
member order, component identity, and preferred model source. Documentation
now states that semantic verification performs one bounded streaming decode
per frame without materializing uncompressed payloads.

The same reviewer then found a remaining type-admission race: replacement with
a FIFO between `O_PATH` inspection and readable open could block. The readable
descriptor now adds `O_NONBLOCK` (with no behavior change for regular files),
followed by the existing type/device/inode comparison. A deterministic seam
replaces the admitted regular pathname with a FIFO in that exact gap and proves
prompt rejection without publication.

Focused results:

```text
cargo test --locked -p pangopup-build --test runtime_release
  9 passed
cargo clippy --locked -p pangopup-assets -p pangopup-build --all-targets -- -D warnings
  passed
make spec
  227 passed; 6 skipped
```

`Cargo.toml`, `Cargo.lock`, and all crate manifests are unchanged.

## Adversarial Code Review

Reviewer: `/root/ticket033_code_review`

First review: REJECT.

- Path-based verification and a later independent inspection could observe two
  transport versions.
- The production profile parser accepted syntactically valid but false member
  inventories.
- Documentation denied the streaming decompression actually performed by
  semantic verification.

Remediation moved identity derivation, semantic verification, stored-byte copy,
and final revalidation onto one held directory/ten-member descriptor set;
pinned every production record and added canonical rebinding negatives; and
corrected all resource claims.

Second review: REJECT. A replacement with a FIFO between nonblocking type
admission and the readable open could block. Remediation made the readable
no-follow open nonblocking, retained post-open device/inode/type equality, and
added an exact-gap FIFO replacement regression.

Final re-review: ACCEPT. No material finding remains. The reviewer verified the
single held trust boundary, exact ten-member binding, profile negatives,
bounded-decompression documentation, FIFO-safe admission, atomic publication,
post-rename error truth, unchanged manifests/lockfile, and absence of
production/network/GitHub effects.

## External Effect Evidence

Coordinator: not applicable. This ticket performs no GitHub release, tag, or
asset mutation.

## Coordinator Final Check

Coordinator: `/root`

The full local gate passed: `make lint`, the complete workspace test suite, and
`make spec` (`227 passed; 6 skipped`). The reviewed implementation was committed
as `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`, pushed, and its exact GitHub
Actions run `30582599182` passed.

Only after that green remote gate, the coordinator ran the production
preparer once against the retained Ticket 030 transport. It produced the
closed read-only 13-file stage for `runtime-grch38-v1`. All 11 declared
checksums passed; all ten copied transport members matched the retained source
byte for byte; modes, types, and link counts matched the contract. The complete
inventory, hashes, command result, and `/usr/bin/time -v` resource observations
are recorded in
`planning/artifacts/033-runtime-release-preparation.md`.

No rebuild, recompression, network request, GitHub release, tag, or upload
occurred. Ticket 033 is complete and Ticket 034 may now review publication of
this exact immutable stage.
