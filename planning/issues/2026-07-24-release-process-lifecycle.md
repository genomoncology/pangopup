# Release upload process lifecycle

Status: open
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

## Required resolution

- Set and prove correct close-on-exec ownership for every test barrier endpoint,
  close unused ends explicitly, and use RAII cleanup for panic/interruption.
- Add an outer-grandparent regression that terminates the test runner and proves
  no helper remains.
- Extend abrupt coordinator-death coverage to a descriptor-holding descendant.
- Either supervise the entire upload process group from a watchdog that survives
  the coordinator or prove and enforce that the pinned executable cannot create
  descendants.
- Confirm a cancelled `make test` does not retain executables, payload
  descriptors, leases, locks, or child processes.

Do not rerun public upload machinery merely to close this issue. Use controlled
fake subprocesses; no GitHub mutation belongs in the remediation ticket.
