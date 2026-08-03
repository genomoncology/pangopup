# 046 — Close resilient-sync qualification

Status: ready

## Why

Ticket 045 is implemented, independently reviewed, pushed, and successfully
retested on an Apple M5 Max. Ordinary GitHub CI is green at
`e5d7d1ae788ac9e1c6c194c60b832ba8837b238f`, but both native container jobs in
run `30851865716` fail before asset setup because
`scripts/qualify-container.sh` still expects the pre-Ticket-045 `sync --help`
line. The executable correctly reports the new `--progress | --quiet` flags.
The rolling frontier also still calls resilient sync the next undrafted
outcome, and the successful Mac retest currently exists only on the test Mac
and in the coordination transcript.

## Scope

- Update the final-image focused-help oracle to the exact shipped `sync` help
  line. Do not loosen exact help checking or derive expected output from the
  image under test.
- Add `planning/artifacts/045-apple-silicon-retest.md`, a compact factual record
  of the supplied exact-commit Mac retest: platform/image identity, intended
  fixes exercised, interrupted/resumed byte evidence, scoring/HTTP regression,
  retained limitations, and the location of the raw evidence on the Mac. Do
  not copy raw logs or claim an automatic in-process retry was induced.
- Update `planning/frontier.md` so resilient synchronization, focused help,
  read-only status, and the Mac retest are established; make the ONNX Runtime
  Apple warning the next bounded outcome, followed by the compact README,
  release, and container-publication slots.
- No CLI, asset, model, HTTP, installer, Dockerfile, or release behavior change.

## Success Checklist

- A local final-image qualification reaches and passes the exact focused-help
  check with `--progress | --quiet` present.
- The qualification still rejects an intentionally stale or broadened sync
  usage line.
- The Mac artifact records exact commit
  `e5d7d1ae788ac9e1c6c194c60b832ba8837b238f`, ARM64 image size 23,380,620
  bytes, 263,438,336 resumed bytes, 2,360,124,034 invocation-downloaded bytes,
  matching monotonic progress/final JSON totals, and the honest absence of a
  deliberately induced within-process retry.
- Frontier searches contain no claim that Ticket 043, 044, or 045 behavior is
  future or next.
- `make lint`, `make test`, and `make spec` pass.
- After the reviewed implementation is pushed, GitHub `ci` and both native
  jobs in `container` pass for the exact commit.

## Decisions

1. **Repair the independent oracle.** The executable, CLI specs, root-help
   fixture, and Mac evidence agree; only the final-image script is stale.
   Updating its literal preserves an independent exact assertion.
2. **Preserve conclusions, not a second raw archive.** The raw evidence remains
   at `/Users/ian/Desktop/pangopup-mac-evidence-ticket045`; the repository keeps
   a bounded audit summary with exact measurements and limitations.
3. **Keep this a qualification closeout.** ONNX Runtime replacement, README
   restructuring, versioning, and publication require their own evidence and
   do not enter this repair.

## Dependencies

- Tickets 043–045: complete.

## Notes

- GitHub failure is exact:
  `sync-h-first-line expected=Usage: pangopup sync [--offline] ... observed=Usage: pangopup sync [--offline] [--progress | --quiet] ...`.
- The Mac retest found no Ticket 043–045 product failure. Its sole remaining
  product friction is ONNX Runtime's `Unknown CPU vendor` warning on Apple
  Silicon, including commands that never initialize a model session.
- This ticket has no public or irreversible external effect.

## Coordinator Authorship

Coordinator: Codex

Drafted from the exact remote failure, shipped CLI catalog, supplied Mac
report, and current frontier. The coordinator does not implement or approve
its own ticket.

## Independent Ticket Review

Reviewer: Herschel the 2nd

Verdict: ACCEPT. The reviewer confirmed the remote failure, shipped help line,
Mac measurements, scope boundary, and two stale frontier locations are all
supported. The artifact must remain a concise record of the supplied report,
not claim that this Linux checkout independently opened the raw Mac evidence.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
