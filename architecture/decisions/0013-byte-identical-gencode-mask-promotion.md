# 0013 — Byte-identical GENCODE mask promotion

Status: accepted

## Decision

Use the exact `PGMBEN01` v1 `domains` member selected and retained by Ticket
012 as Pangopup's runtime GENCODE v38 mask representation. The production API
opens only codec discriminator `domains`; the interval-tree and
binned-postings members remain rejected.

The runtime boundary is `pangopup_index::mask`: one read-only mmap,
`MaskDomainsOpen`, a `Send + Sync` `MaskProvider`, and reusable caller-owned
query storage. It preserves `(start,end]` membership, plus-before-minus order,
authenticated within-strand rank, exact versioned/PAR identities, optional
stable-gene filtering, and normalized exon boundaries.

Ordinary open performs bounded header, section, and directory checks. Queries
validate the records they touch. Whole-file SHA-256 belongs to later
download/install verification; runtime open does not scan the complete member.

## Context

ADR 0011 selected constant-membership domains after exhaustive semantic
certification and a complete retained comparison, but required the winner to be
re-specified under a separate production magic. That would reproduce identical
data and behavior solely to rename an already authenticated internal format.

The selected 6,703,320-byte member is retained with SHA-256
`714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
Its exact 1,000-query oracle, ordering behavior, corruption controls, bounded
open, and allocation-free warmed lookup already passed.

## Consequences

- This decision supersedes only ADR 0011's requirement for a distinct
  production format identity. ADR 0011's representation and semantic
  selection remains accepted.
- No GTF, SQLite database, Python environment, canonical export, builder, or
  second mask member is part of runtime operation.
- The historical candidate writer, alternate codecs, and qualification tools
  were removed after promotion. Their selection report, exactness manifest,
  and git history remain the durable evidence.
- Asset packaging later binds the exact member size and digest. The member is
  immutable after verified installation; concurrent in-place mutation or
  truncation of an open mmap is outside the supported threat model.
- Mask transport, installation, publication, model execution, and service
  routing remain separate outcomes.
