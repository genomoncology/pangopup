# PangoPup work records

PangoPup is ACTIVE. This folder owns current work status. `planning/current.md` reconciles the retained product roadmap and evidence. The older root `planning/` tree remains historical evidence and reference material. No current product ticket is assigned by this documentation change. Record 0152 retains the public release evidence; this onboarding makes no new release, runtime, or publication claim.

## Current authority

Workspace AGENTS governs manual delivery. This repository's AGENTS and this folder describe local boundaries. Use ticket review, implementation, independent code review, relevant verification, a record, and root-approved landing. A documentation or configuration Quick Fix may omit the ticket and ticket review. Push whole fixes to their isolated ticket branch. An unlanded candidate awaits independent review even when checks pass.

`issues/` records problems. `tickets/` owns active ticket instructions and explicit status. `planning/` owns current plans, decisions, and [declared lanes](planning/lanes.md). `records/` retains outcomes and proof. Preserve archived tickets, drafts, artifacts, historical factory scripts, and unrelated worktrees. The retired factory scripts in `project/`, where present, are historical mechanisms. They do not authorize unattended dispatch, landing, or cleanup.

## Installed pm

Run from this checkout, or pass `--repo` with its path:

```text
pm status
pm next
pm daily
pm item <ticket number>
pm lanes
```

Each command also accepts `--json`. `pm.json` uses the configuration declared by the installed tool's `src/records/load.ts` and `src/records/lanes.ts`. The mailroom names the sibling `sdlc` repository and this repository's own name. No mail is sent by these read commands.

`recordNaming: ticket` admits the retained numbered records and date-prefixed records. The three mapping objects are empty because no alternate status meaning has been established for current records. Use explicit canonical statuses for new work. Built, reviewed, and accepted describe their respective stages. They do not establish completion or landing. Keep the default landed-proof requirement from ticket 1; this configuration does not relax it. `origin/main` is the landing target.

The Evidence-label adoption boundary is ticket 0159. It applies to new numbered tickets from onboarding. Earlier records retain their original proof. Current live tickets receive explicit status and Evidence metadata where present. This boundary changes no landing-proof requirement.

## Verification

Product changes require make lint, make test, and make spec under the repository's ordinary rules. This documentation and configuration candidate uses JSON parsing, `git diff --check`, local link checks, and comparisons of pm human and JSON output with known records. It runs no product gates or hosted CI. Push candidate commits with `[skip ci]`.

Read [the onboarding record](records/qf-pm-onboarding.md) for observed limitations. Findings remain visible. A zero exit from next or daily alone does not prove that records are complete or a ticket is approved. Root owns feedback to the tool repository and the independent review of this candidate. Ian can overturn these local declarations.
