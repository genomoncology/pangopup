# Maintainer interface and documentation drift

Status: open
Found by: 2026-07-24 adversarial project review
Priority: before adding more public maintenance commands

## Observation

`pangopup-build` has accumulated production build, verification, reference,
transport, release, compatibility, and candidate commands, but its root usage
string still advertises only four early prototype commands. `--help`,
`--version`, and nested `--help` probes exit as usage errors. No executable spec
checks the complete maintenance command catalog.

Several documents also retained pre-Ticket-011 or pre-sync claims:

- `planning/faq.md` says the reference builder/runtime is not implemented and
  the selected representation still needs production hardening;
- `architecture/design.md` says `pangopup-assets` has no network access even
  though that crate owns pinned remote sync;
- the same design document refers to a nonexistent CLI streaming mode;
- `release-profiles/README.md` calls shipped remote sync the next slice;
- `planning/artifacts/011-production-reference.md` points to an active Ticket
  011 that was correctly removed; and
- README outcome 11 is not marked complete.

## Required resolution

- Make top-level and nested maintainer help complete, conventional, and
  executable-spec tested without changing established operational error
  contracts.
- Reconcile every listed current/future claim against shipped behavior.
- State whether accepted ADR consequences are historical snapshots or current
  survivor documentation, then label historical statements where needed.
- Add a repeatable stale-command/stale-current-claim check that is narrow
  enough to remain useful rather than becoming another long-running verifier.

This is real maintenance debt but does not block Ticket 012's local mask
semantics and format work. It must be resolved before new model/mask maintenance
commands are presented as an end-user-supported interface.
