# Runtime asset trust and durability

Status: open
Found by: 2026-07-24 adversarial project review
Priority: before compatible-profile activation and HTTP readiness

## Observation

The installed SNV path opens descriptor-held, no-follow members. Explicit SNV
bundle opens and the current public reference bundle open first inspect path
metadata and then reopen names, which is safe only under the documented
immutable/trusted-directory contract. A writable directory can introduce a
check/use substitution; same-inode truncation after mmap can terminate the
process.

The only complete production reference bundle and machine-readable
qualification roots are also retained at workstation-local absolute paths.
The checked planning artifact records their identities, but losing those roots
would force a needless rebuild and discard inspectable small receipts.

## Required resolution

- Give the production reference provider a non-feature-gated held-member
  constructor and make future installed/service opens descriptor-relative and
  no-follow.
- Keep explicit ambient-path opens documented as trusted development input, or
  harden them to the same contract.
- Atomically activate one manifest binding compatible SNV, model, reference,
  and mask identities rather than four unrelated pointers.
- Preserve the existing reference build. Put its small canonical manifest,
  qualification report, commands, and resource receipts in durable tracked or
  remotely backed-up evidence; publish/transport the large member later without
  rebuilding it.
- Define bounded provisioning cancellation and durable progress before service
  startup can automatically sync assets.
