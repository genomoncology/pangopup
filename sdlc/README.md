# PangoPup work records

PangoPup is ACTIVE. This directory owns current work and status. Read repository `AGENTS.md`, then [the current plan](planning/plan.md) and [lanes](planning/lanes.md). The installed `pm` CLI reads `sdlc/pm.json`; it reports evidence and does not authorize work.

## Manual delivery

Build work follows ticket, independent ticket review, implementation, independent code review, checks, record, landing, and push. Documentation and configuration Quick Fixes may omit ticket review. Use an isolated `ticket/` branch and registered worktree. Push whole work in progress with `[skip ci]` for documentation-only changes. Independent review must cover the exact candidate before landing. The current onboarding waits for root independent review.

Place current tickets in `tickets/`, issues in `issues/`, plans and decisions in `planning/`, and outcome records in `records/`. Give current tickets an explicit `Status:` within the first 25 lines, an owner, and evidence labels `Starts from`, `Keeps`, `Changes`, `Proof`, and `Defers`. Use `qf-slug.md` for documentation Quick Fix tracking. Number build tickets with the installed allocation command, `pm ticket new SLUG`, within the authorized manual flow.

The factory is retired. Historical `project/` and `scripts/` files do not schedule work. Promotion onto main no longer authorizes unattended dispatch. Preserve archived tickets, drafts, prior records, and existing worktrees. Their historical status is evidence, not a current assignment.

## Status and landing evidence

`ticketStatusMappings` maps only `awaiting independent review` to `in progress`. That is the current documentation candidate's waiting state. It does not mean landed. `issueStatusMappings` maps `parked` and `held` to `blocked`: the issue text records the input required to revisit each. The research issue remains `open`, without an approved implementation. There is no broad mapping from built, reviewed, accepted, or archived to complete.

`recordNaming: ticket` accepts the existing numbered records and date records. `targetRef: origin/main` makes pushed main the landing target. Landing proof starts at ticket 1. Complete tickets must prove an actual landing; do not raise the threshold to hide missing proof. Historical archive files do not become current tickets merely because a matching record exists. Accepted technical decisions remain decisions, and do not count as completed implementation tickets.

## Checks

For documentation and configuration changes, run `git diff --check`, parse `pm.json` as JSON, confirm declared paths and headings, and compare the read-only surfaces below with the named records. Record exit codes, counts, and discrepancies. The configured light gate checks whitespace only. Product changes still require `make lint`, `make test`, and `make spec`; there is no `make check`. Product gates, hosted CI, production data, and publication are outside this onboarding.

```text
pm status --json
pm next --json
pm daily --json
pm item qf-pm-onboarding --json
pm item 0152 --json
pm lanes --json
```

Repeat without `--json` to check human output. Findings remain visible. An exit code of zero from `next` or `daily` does not prove the loaded records are clean. `pm item 0152` currently cannot resolve the archived ticket even though its publication record exists. Read retained historical evidence directly.

## Ownership and reports

The lane table owns current assignments and path claims. `pm` reads lane names, worktrees, and claims; its output does not preserve our State and Owner columns. Read the table before assigning work. A free claim does not prove a blocked reviewer is available; `next` reports no worktree declared for both idle and blocked unassigned lanes. Never infer a live worker from a branch or worktree. The relative mailroom routes to `sdlc` with sender name `pangopup`; root owns tool feedback and any message sends for this onboarding.

Historical product evidence remains in `planning/` and `architecture/`. [The current plan](planning/plan.md) reconciles those sources into this directory. Do not file new work under the older planning tree.
