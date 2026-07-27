# Ticket 025 local runtime-profile installation

The checked miniature contract installs the existing SNV regression bundle,
singleton model fixture, route-reference bundle, and domains mask into the
production layout without a shipped synthetic compatibility bypass.

Observed deterministic shape:

```text
runtime/
  active.json
  components/{model,reference,mask}/<content-id>/...
  profiles/<profile-id>/{profile.json,receipt.json}
  .staging/
```

The focused Rust tests prove first install, exact compact receipts, bounded
ready status, immutable member/wrapper modes, and idempotent reuse without
replacing the installed model inode. A five-transition test injects failures
after staged durability, component publication, profile publication, before
active rename, and after active rename. Before the commit point status remains
missing; after it status is coherent; every case retries successfully.

The adversarial cases also prove that same-name/same-size corruption of the
model, reference, or mask is detected before reuse; source replacement,
truncation, symlinks, hardlinks, and extra members fail closed; malformed or
dangling active metadata is never reported as missing; an interrupted
replacement preserves the prior active profile; and orphan cleanup is both
entry-bounded and descriptor-relative. Runtime, component-kind, identity,
bundle, profile, and payload descriptors remain held through activation; a
deterministic intermediate-directory replacement fails before the active
pointer can change.

Normal gates use only checked small fixtures. No production model, reference,
mask, or 15 GB SNV payload was copied, installed, or invoked.
