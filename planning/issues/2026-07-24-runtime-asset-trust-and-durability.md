# Runtime asset trust and durability

Status: closed
Found by: 2026-07-24 adversarial project review
Priority: before compatible-profile activation and HTTP readiness

## Observation

The installed SNV path opens descriptor-held, no-follow members. Explicit SNV
bundle opens and the public `ReferenceBundleOpen::open` API for compiled
sequence-index bundles first inspect path metadata and then reopen names, which
is safe only under the documented immutable/trusted-directory contract. A
writable directory can introduce a check/use substitution; same-inode
truncation after mmap can terminate the process.

The only complete production GRCh38 sequence-index bundle and machine-readable
qualification roots are also retained at workstation-local absolute paths.
The checked planning artifact records their identities, but losing those roots
would force a needless rebuild and discard inspectable small receipts.

## Required resolution

- Give the production `ReferenceProvider` a non-feature-gated held-member
  constructor and make future installed/service opens descriptor-relative and
  no-follow.
- Keep explicit ambient-path opens documented as trusted development input, or
  harden them to the same contract.
- Atomically activate one manifest binding compatible SNV, model, compiled
  GRCh38 sequence index, and mask identities rather than four unrelated
  pointers.
- Preserve the existing compiled GRCh38 sequence index build. Put its small
  canonical manifest, qualification report, commands, and resource receipts in
  durable tracked or remotely backed-up evidence; publish/transport the
  compiled member later without rebuilding it. Never mirror the raw NCBI FASTA
  or assembly report.
- Define bounded provisioning cancellation and durable progress before service
  startup can automatically sync assets.

## Resolution

Closed 2026-08-05. Installed sequence and mask providers now open admitted
members through held, descriptor-relative, no-follow boundaries. One atomic
receipt binds the compatible SNV, model, compiled GRCh38 reference, and mask
identities, so readers cannot activate a mixed profile. The derived SNV and
runtime transports are durable immutable public releases; the raw Zenodo,
NCBI, and GENCODE inputs are not mirrored. Synchronization has bounded retry,
safe resumable downloads, durable progress, checksum admission, and atomic
installation. It remains an explicit operator action: `pangopup serve` never
syncs or performs network access during startup. These contracts are covered
by the asset/provider tests, executable specs, public-release qualification,
and the retained Apple volume qualification.
