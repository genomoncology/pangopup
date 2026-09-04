# ADR 0021: Atomically select one locally installed runtime profile

Supersession note (2026-09-04): Ticket 0003 extended this descriptor-relative installation and status boundary to native macOS. The original Linux-only consequence below remains as history. Direct uninstall remains Linux-only.

## Decision

Pangopup installs the model, compiled RefSeq reference, and GENCODE mask from
trusted local inputs into private immutable XDG data. It reuses the already
certified active SNV object and changes a separate `runtime/active.json` pointer
only after every referenced component and receipt is durable.

The runtime installer shares the existing root `.install.lock` with SNV
installation. Component identities come from the canonical four-asset profile.
Published objects are never overwritten; exact reinstall validates and reuses
them.

## Why

Four unrelated caller paths can change independently and cannot prove a
coherent scoring tuple. Recopying or rehashing the 15 GB SNV score member would
repeat certification work. One immutable filesystem graph plus one atomic
pointer gives a small crash boundary without adding a database.

## Consequences

- Installation is Linux-only, offline, and explicit.
- Source paths are never retained in receipts or status.
- Status reads bounded metadata and member sizes, not payload hashes or model
  sessions.
- Ticket 025 deliberately stopped before lookup consumption. Ticket 028 now
  discovers `runtime/active.json` only after inference is required and admits
  the selected model-side tuple through held descriptors.
- Network delivery, publication, rollback/GC, HTTP, Docker, and service
  lifecycle remain separate work.
