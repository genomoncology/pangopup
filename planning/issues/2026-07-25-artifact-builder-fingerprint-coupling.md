# Artifact builder fingerprint coupling

Status: closed by Ticket 013
Found by: Ticket 012 closeout frontier review
Priority: before production mask hardening

## Observation

The existing SNV and production-reference builders derive provenance from one
repository-wide Rust-source fingerprint. Changes to unrelated code therefore
change both builder identities. Ticket 012 added private mask qualification
code and had to regenerate the checked 1,000-request SNV fixture even though
`scores.pgi` and its notice remained byte-identical.

The mask qualification lifecycle already uses a separate source inventory, but
that does not stop future mask work from churning the older SNV/reference
identities through the global fingerprint.

## Required resolution

- Give the SNV and reference artifact families separate, deterministic causal
  source inventories and explicit fingerprint-version identities.
- Prove that an unrelated source change leaves each fingerprint unchanged and
  a declared causal source change alters it.
- Keep existing shipped SNV and qualified reference bundles readable without a
  production rebuild, download, repack, or republication.
- Perform any miniature-fixture provenance migration once, retaining proof that
  its payload members remain byte-identical.
- Preserve the mask qualification lifecycle's separate fingerprint boundary.

This issue does not authorize a production mask format/provider, GTF/database
replay, model work, transport, installation, public CLI changes, or release
publication. Close it only after the ordinary lint/test/spec gate and focused
fingerprint-isolation evidence pass.

## Resolution

Ticket 013 replaced the shared hash for future builds with two independent
source identities:

- `pangopup.snv-builder-source.v1`;
- `pangopup.reference-builder-source.v1`.

In plain English, changing SNV construction code now changes the SNV builder
identity without pretending the reference changed, and changing reference
construction code does the opposite. Shared causal code changes both. Mask,
candidate, sync, release, and CLI changes affect neither.

The evidence is compiled into the builder, includes exact checked dependency
closures/features, and does not inspect the checkout during an artifact build.
Checked legacy manifests prove the already shipped SNV and retained reference
remain readable with unchanged payloads. The 1,000-case SNV fixture and
miniature reference prove the migration changed only descriptive manifest
provenance. No production asset was opened or rebuilt, and the Ticket 012 mask
identity stayed exact.

The durable evidence is
[`../artifacts/013-artifact-builder-provenance.md`](../artifacts/013-artifact-builder-provenance.md).
Independent code review accepted the complete implementation and the bounded
test-only correction exposed by the first broad test run. The final
`make lint`, `make test`, and `make spec` gate passed, so this issue is closed.
