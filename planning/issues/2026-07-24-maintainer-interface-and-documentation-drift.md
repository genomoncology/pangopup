# Maintainer interface and documentation drift

Status: closed
Found by: 2026-07-24 adversarial project review
Priority: before adding more public maintenance commands

## Observation

`pangopup-build` has accumulated production build, verification, reference,
transport, release, compatibility, and candidate commands, but its root usage
string still advertises only four early prototype commands. `--help`,
`--version`, and nested `--help` probes exit as usage errors. No executable spec
checks the complete maintenance command catalog.

At discovery, several documents also retained pre-Ticket-011 or pre-sync
claims:

- `planning/faq.md` says the reference builder/runtime is not implemented and
  the selected representation still needs production hardening;
- `architecture/design.md` says `pangopup-assets` has no network access even
  though that crate owns pinned remote sync;
- the same design document refers to a nonexistent CLI streaming mode;
- `release-profiles/README.md` calls shipped remote sync the next slice;
- `planning/artifacts/011-production-reference.md` points to an active Ticket
  011 that was correctly removed; and
- README outcome 11 is not marked complete.

Ticket 012 reconciled those listed stale current/future claims and then updated
the same surfaces again after the retained mask comparison selected `domains`.
The private feature-gated `pangopup-mask-candidates` binary is deliberately
excluded from ordinary installation and does not expand the supported
maintainer interface. Its closed grammar is executable-spec tested. Historical
ADR consequences remain snapshots of the decision at acceptance unless a
document explicitly presents itself as current state.

The unresolved debt is the public `pangopup-build` help/version behavior and a
narrow repeatable check of its supported command catalog. Release-upload
subprocess lifecycle is tracked separately and is not part of this issue.

## Required resolution

- Make top-level and nested maintainer help complete, conventional, and
  executable-spec tested without changing established operational error
  contracts.
- Reconcile every listed current/future claim against shipped behavior.
- State whether accepted ADR consequences are historical snapshots or current
  survivor documentation, then label historical statements where needed.
- Add a repeatable stale-command/stale-current-claim check that is narrow
  enough to remain useful rather than becoming another long-running verifier.

## Resolution

Ticket 029 made one checked catalog the source of both maintenance-command
recognition and root, namespace, and leaf help. Conventional help and version
paths are successful, stdout-only, and side-effect free; malformed operational
requests retain their exact previous JSON, exit class, and stream, including
the reference namespace's stdout exception. Unit tests bind every dispatch leaf
to one unique catalog path, and `spec/build-cli.md` executes the complete
catalog plus representative legacy failures.

Ticket 012 had already reconciled every current-state document named above.
Accepted ADR consequence sections remain historical acceptance-time snapshots;
README, FAQ, frontier, architecture overviews, and open issues are the
current-state surfaces. The narrow drift is therefore resolved before any new
production maintenance command is added.
