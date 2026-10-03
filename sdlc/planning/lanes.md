# Current manual lanes

Updated 2026-10-03. The documentation candidate is prepared on its own branch. Root owns assigning fresh independent review and authorizing landing. No reviewer or product worker is assigned by this file. Historical worktrees are preserved and independently managed; their existence establishes no current claim.

## Lanes

| Lane | State | Owner | Worktree | Holds | Next action |
| --- | --- | --- | --- | --- | --- |
| Documentation | awaiting independent review | Current session, documentation Quick Fix only | `pangopup-qf-pm-onboarding` | `AGENTS.md`, `planning/README.md`, `planning/goals.md`, `planning/tickets/README.md`, `sdlc/README.md`, `sdlc/pm.json`, `sdlc/planning/lanes.md`, `sdlc/planning/plan.md`, `sdlc/planning/ticket-template.md`, `sdlc/scripts/README.md`, `sdlc/issues/2026-09-04-smaller-hardening-notes.md`, `sdlc/issues/2026-09-04-splice-consequence-module-research.md`, `sdlc/issues/2026-09-12-snv-neighborhood-indel-prefilter.md`, `sdlc/tickets/qf-pm-onboarding.md`, `sdlc/records/qf-pm-onboarding.md` | Root assigns independent review of exact pushed candidate |
| Product | idle | Unassigned | no active worktree | free | Coordinator selects a bounded ticket from the current plan |
| Independent review | blocked on root assignment | Unassigned | no active worktree | free | Root assigns a fresh read-only reviewer; this session supplies no acceptance |

The documentation claim covers only this change. It grants no product implementation or publication authority. Idle and blocked are explicit human states. The current `pm` lane schema reads Worktree and Holds and omits State and Owner. Update this table when an actual assignment changes. Do not treat `free` as approval or review availability. Bare worktree names resolve through Git's registered worktrees without public absolute machine paths.
