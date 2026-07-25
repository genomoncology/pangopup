# Ticket 013 — Artifact-specific builder provenance

Date: 2026-07-25

## Outcome

Future SNV and production-reference manifests no longer share the old
repository-wide Rust-source identity
`fa5d9fc3c3482aeca671e90e75752738019b911c3cba1549bd847856bf3986af`.
They now derive independent identities from only the implementation and locked
dependency evidence that can affect each artifact:

- miniature SNV:
  `85126cbb4bbc008a475b0b941447fb7a24f299abb1754a1c10582912a522eb2d`;
- miniature reference:
  `252f60fd8ea809fa0a3b583bf3a7ddb99601fef67b21a227264e8fa55b873e24`.

These values describe this source revision's future miniature builds. They do
not replace the identity inside any existing production asset.

## Fingerprint contract

The shared `pangopup.builder-source-fingerprint.v1` algorithm hashes
length-framed bytes in this order:

1. algorithm declaration;
2. artifact domain;
3. complete inventory declaration;
4. logical path and contents for every entry, sorted by path.

Paths must be nonempty and unique. The domains are
`pangopup.snv-builder-source.v1` and
`pangopup.reference-builder-source.v1`. The source inventories and their
Linux dependency root/closure/feature projections are checked files compiled
into the builder. Artifact construction never inspects a checkout and never
starts Cargo recursively.

Each artifact carries exact direct-root version requirements, resolved
versions, default-feature policy, explicit features, an isolated Cargo lock
snapshot, and a flattened closure projection. A test independently scans the
selected causal Rust bytes for external path heads and import roots while
excluding comments and normal/raw/byte/C-string literals. Aliases introduced
by ordinary `use`, use trees, and `extern crate` are resolved transitively,
while the raw candidates remain conservative against shadowing. It maps every
source path to its owning crate, then reads the actual Pangopup package
declarations with:

```text
cargo metadata --locked --no-deps \
  --filter-platform x86_64-unknown-linux-gnu \
  --format-version 1
```

Cargo's metadata exposes the effective requirement/default-feature/feature
settings even when the owning manifest inherits them from the workspace. The
derived package-name set and settings must exactly equal the checked artifact
roots. Target-qualified causal dependencies are rejected by this v1 proof.
The checked source-use maps remain useful diagnostics, but changing them does
not change either fingerprint and they are not accepted as proof authority.
Alias-specific controls derive `flate2` through `gzip::`, reject simultaneous
root/witness omission, and prove nearby comment and literal decoys cannot
invent `serde_json`.

Selected code now uses direct private-module paths within each crate. Separate
checked SNV/reference root-wiring projections are fingerprint inputs. Tests
derive them from the real mixed crate roots: each selected module file and
same-crate module path requires its exact top-level module declaration,
including attributes, and every cross-crate root symbol requires the real
`pub mod` or `pub use` item that exposes it. Cargo metadata supplies workspace
dependency imports and renames. A private module `path` rebind and a
cross-crate wildcard-reexport rebind fail or change the projection; appending
an unrelated root item changes neither projection. Whole mixed `lib.rs` files
are not fingerprint inputs.

The isolated-resolution control separately writes only that artifact's roots
and lock into a standalone temporary Cargo project and invokes:

```text
cargo metadata --locked --offline \
  --filter-platform x86_64-unknown-linux-gnu \
  --manifest-path <isolated-artifact-project>/Cargo.toml
```

The temporary manifest is its own workspace and has no Pangopup workspace
member or consumer. Cargo therefore resolves each artifact's roots, transitive
closure, and enabled features without workspace-global feature unification.
The result must match both the artifact lock/projection and independently
parsed repository `Cargo.lock` checksums. The two projections happen to be
byte-identical today because the two builders use the same external closure,
but remain separate named inputs. The isolated result removes unrelated
workspace-enabled `serde`/`serde_core` `alloc` features. SNV direct roots now
explicitly include the `libc` used by no-follow opens.

One control removes `libc` from both the checked root declaration and
diagnostic witness. The former declaration-to-witness comparison would accept
that simultaneous omission, while source/manifest derivation still finds the
actual `libc::` use and fails. Three separate controls mutate the actual
owning-manifest metadata seen by the proof: version requirement,
`default-features`, and explicit feature-list drift each fail independently.
Unrelated `ureq` and `zstd` declarations in the assets crate are not derived
from the selected causal sources.

The production incremental hasher and an independent oracle are both pinned to
hard expected digests. The oracle builds the complete framed preimage in a
`BTreeMap`, hashes it once, and does not call the production fingerprint
function.

The transport regression has its own filesystem-backed reconstruction of the
final SNV preimage and exact `85126cbb...22eb2d` identity. It mutates the
checked root-wiring projection, root `NOTICE`, and assets certification source
independently. This replaced its obsolete pre-Ticket-013 repository-wide
digest helper after the first coordinator `make test` attempt exposed the
stale assertion; the emitted manifest was correct.

Pure injected-input tests prove every declared artifact-only entry changes
only its owner, shared core and algorithm changes affect both, order is
irrelevant, duplicates fail, and add/remove/rename/domain/inventory changes
alter a digest. The test universe also mutates representative actual excluded
paths: `build.rs`, the three mixed crate roots, mask qualification, mask
candidates, sync, release, and the CLI. None changes either artifact identity
directly; causal root wiring is represented by the checked projections above.

The exact Ticket 012 mask identity remains:

```text
fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816
```

Neither `crates/pangopup-build/build.rs`, Cargo manifests/lock, nor any current
mask-fingerprint input changed.

## Legacy compatibility and payload invariants

The checked migration fixture retains actual pre-migration manifests for both
families. Current readers open them with unchanged new-build members and
return a real SNV lookup and reference window. This proves builder provenance
is descriptive rather than a runtime compatibility key.

The deterministic SNV regression generator was rerun after the independent
review required the final inventory closure. Its ten causal inputs/outputs
remained byte-identical:

- six attributed source gzip members;
- fixture reference;
- request list;
- bundle `NOTICE`
  (`9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7`);
- `scores.pgi`
  (`fb0a77425456bd39e6aab7ad3447a24757f6889e82f7b27df01c214b78f8a6b9`).

Only the manifest's `builder.source_sha256` changed. Its exact identity moved
from `f7d93978715603eeebb72c7bb1af744e0d3bb5f976c94c3daaeae2c0e6d58fbc`
to `941de4d6662bb20ed5932446beb37a5b9062fc74adcdedc5efd75502cb93c65e`.
Every expected JSONL line changed only at `provenance.bundle_id`; replacing
that one value reconstructs the exact legacy file hashes.

The miniature reference retained:

- `reference.pgr`: 4,560 bytes,
  `0ef815ffb3fbb897e880e56afcb57e1edb41f3707784f591c0457581c2e9a3d5`;
- `NOTICE`: 279 bytes,
  `faea3b1976bf4e15f95bad3906144d83b4441f860d3c5b87ab406205e47262db`.

Only its manifest provenance changed, from exact legacy manifest
`9712bea82a93ae97332c47d5ebc6e2bd4a30385315d170416e0ef95a4caac104`
to `1c47a81751a4b19d861f7b6a55a61b07794e26d68c127781b08a1521c948fad6`.

The unchanged source constants still pin the public SNV builder/bundle/member
and retained reference builder/bundle/member identities. Tests inspect those
constants without opening a production asset. No production bundle, source
archive, FASTA, GTF, SQLite database, transport, or release asset was read,
rebuilt, downloaded, repacked, installed, or published.

## Focused evidence

The implementation-stage controls passed:

```text
cargo check --locked --workspace --all-features
cargo test --locked -p pangopup-build source_fingerprint
cargo test --locked -p pangopup-build --test builder_provenance
cargo test --locked -p pangopup-build --test snv_regression_fixture
cargo test --locked -p pangopup-build --test transport builder_identity_covers_assets_manifest_notice_and_certification_source -- --exact
cargo test --locked -p pangopup-build --test full_bundle
cargo test --locked -p pangopup-build --test reference
cargo test --locked -p pangopup-assets release
cargo test --locked -p pangopup-build --features mask-qualification mask::tests
make lint
```

Observed focused results were 16 source-fingerprint unit controls, four
migration controls, one final-identity transport control, one 1,000-case SNV
regression, 13 full-bundle controls, 15 reference controls, four
release-filter controls, and 29 mask controls; the one intentionally ignored
production mask lifecycle test remained ignored.
Implementation-stage Rustfmt and workspace/all-target Clippy also passed with
warnings denied. Independent code review accepted the complete implementation
and the test-only transport-oracle correction exposed by the first broad test
run. The coordinator's final `make lint`, `make test`, and `make spec` gate
then passed; the executable specification gate reported 150 passing cases.

This evidence is deliberately miniature and deterministic. It proves
provenance isolation and compatibility; it does not qualify a new production
asset or authorize a production rebuild.
