# 0012 — Artifact-specific builder provenance

Status: accepted

## Decision

Future SNV and production-reference builds use separate, domain-separated
source fingerprints:

- `pangopup.snv-builder-source.v1`
- `pangopup.reference-builder-source.v1`

Both use the same versioned, length-framed SHA-256 algorithm. Each digest
includes its algorithm declaration, domain, complete inventory declaration,
and a sorted, duplicate-free list of logical paths and bytes. The inventory
also contains a checked projection of the exact locked Linux dependency
closure and enabled features used by that artifact's builder.

The evidence is compiled into `pangopup-build`. Building an artifact does not
read a source checkout or run Cargo recursively. Three narrowly mixed modules
were split so the inventories can name only SNV or reference implementation
inputs while preserving their existing public paths. Shared command-error and
asset error/input-audit support were also moved into bounded modules so the
inventories close over actual imports without hashing mixed crate roots.
Causal code now names those private modules directly instead of resolving
through crate-root reexports.

Each artifact also carries a checked projection of only its causal crate-root
wiring. Tests derive that projection from the real mixed `lib.rs` files:
selected file modules require their exact module declaration, including
attributes such as a `path` rebind, while root symbols used across Pangopup
crates require the actual `pub mod` or `pub use` item that exposes them.
Workspace dependency imports and renames come from Cargo metadata. Changing or
rebinding causal wiring therefore fails until its projection is refreshed,
which changes the fingerprint; an unrelated root item changes neither the
projection nor the fingerprint. Whole mixed crate roots remain excluded.

Dependency evidence is resolved in checked, standalone per-artifact Cargo
projects. Each has exact direct version requirements, resolved versions,
default-feature policy, explicit features, and its own lock snapshot;
`cargo metadata --locked --offline` cannot unify features with an unrelated
workspace consumer. Tests independently enumerate external path heads from
the selected causal Rust bytes, associate each file with its owning crate, and
bind every resulting root and setting to that crate's effective declaration
from the actual Pangopup Cargo manifests. Checked source-use maps are
human-readable diagnostics only: they are neither proof authority nor
fingerprint inputs. The source tokenizer follows dependency aliases introduced
by `use`, use trees, and `extern crate`, while comments and normal/raw/byte/
C-string literals cannot create false uses. A one-shot full-preimage digest
oracle and hard expected values independently check the production incremental
hasher.

Builder provenance describes how an artifact was produced. It is not a runtime
compatibility key. Existing v1 readers continue to accept immutable legacy
manifests carrying the former repository-wide fingerprint, provided their
ordinary schema and integrity checks pass.

The implementation and migration evidence are retained in
[`../../planning/artifacts/013-artifact-builder-provenance.md`](../../planning/artifacts/013-artifact-builder-provenance.md).

## Consequences

- Unrelated mask, candidate, sync, release, CLI, model, or HTTP changes do not
  churn future SNV or reference builder identities.
- A causal SNV source change cannot silently alter the reference identity, and
  a causal reference source change cannot silently alter the SNV identity.
- Shared causal vocabulary and the shared fingerprint algorithm intentionally
  change both identities.
- The shipped SNV release and qualified production reference remain byte-for-
  byte immutable and readable. They are not rebuilt or republished for a
  metadata-only provenance migration.
- Adding or changing a causal dependency requires updating the affected
  checked dependency roots, isolated lock, and projection. The source/
  manifest derivation must pass; diagnostic source-use evidence may then be
  refreshed for readers. A whole-workspace consumer or lockfile change is not
  itself an artifact identity change.
- Changing a selected private-module declaration or a cross-crate root
  reexport requires refreshing the affected root-wiring projection. Unrelated
  crate-root items remain outside both artifact identities.
- Ticket 015 retired the completed Ticket 012 mask-qualification fingerprint
  together with its one-time builder. A future production mask bundle defines
  its own delivery identity and provenance when that asset is packaged.
