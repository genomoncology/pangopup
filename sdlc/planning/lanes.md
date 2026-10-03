# Current manual lanes

Updated 2026-10-03. The documentation candidate is prepared on its own branch. Root owns assigning fresh independent review and authorizing landing. No reviewer or product worker is assigned by this file. Historical worktrees are preserved and independently managed; their existence establishes no current claim.

## Lanes

| Lane | State | Owner | Worktree | Holds | Next action |
| --- | --- | --- | --- | --- | --- |
| Documentation | done | Coordinator | no active worktree | free | Assign future administrative work separately |
| Product | idle | Unassigned | no active worktree | free | Coordinator selects a bounded ticket from the current plan |
| Independent review | idle | Unassigned | no active worktree | free | Coordinator assigns the next fresh review |

The documentation claim covers only this change. It grants no product implementation or publication authority. Idle and blocked are explicit human states. The current `pm` lane schema reads Worktree and Holds and omits State and Owner. Update this table when an actual assignment changes. Do not treat `free` as approval or review availability. Bare worktree names resolve through Git's registered worktrees without public absolute machine paths.
