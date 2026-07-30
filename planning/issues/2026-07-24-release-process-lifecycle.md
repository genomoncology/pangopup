# Release upload process lifecycle

Status: closed
Found by: 2026-07-24 adversarial project review
Priority: block the next public asset upload

## Observation

Two descendants of an interrupted release-upload integration test remained
alive under PID 1 for more than a day at review time:

- the test helper was blocked before setting its parent-death signal while it
  retained both ends of its barrier pipe, so parent death could not produce
  EOF; and
- a fake `gh` descendant survived abrupt coordinator death after its direct
  process-group leader exited.

Orderly interrupt and deadline paths group-kill and reap correctly. The first
case is a confirmed test/gate hygiene defect. The second is a production risk
only if the authenticated GitHub CLI creates descendants, but the existing
abrupt-death proof covers only the direct child and is not sufficient to rule
that out.

## Resolution

Ticket 031 deleted the custom release uploader instead of repairing its second
process supervisor. Pangopup no longer contains the upload command, public or
test upload API, fake-`gh` helper, process-group/parent-death supervision,
signal handling, payload leases, or uploader subprocess tests. Therefore the
observed helper leak and the unbounded descendant risk have no remaining code
path in the repository.

Deterministic `release prepare` remains unchanged, but it does not make a local
pathname immutable. A later independently reviewed publication ticket must
define a controlled stable-source and draft-first lifecycle around direct
coordinator use of the authenticated official `gh` executable before any new
public effect. This closure does not claim that runtime-asset publication or
the separate repository-security baseline is complete.
