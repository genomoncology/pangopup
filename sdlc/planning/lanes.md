# Manual work lanes

Recorded October 3, 2026. Root owns queue coordination and independent review. This session owns the bounded documentation candidate. No product worker is assigned by this record. Retained worktrees outside this table remain independently managed; their presence does not establish an active worker or a free claim.

## Lanes

| Lane | Worktree | Holds | State | Owner | Next action |
| --- | --- | --- | --- | --- | --- |
| documentation | `pangopup-qf-pm-authority` | `AGENTS.md`, `sdlc/**`, `planning/README.md`, `planning/tickets/README.md` | blocked on independent root review after push | current documentation session | Root reviews the exact pushed commit before landing |
| product | no active worktree | free | idle; no product assignment | unassigned; root owns queue selection | Root reconciles the next ticket and assigns a worker |

The documentation claim covers only this candidate's paths. Idle means no assignment in this declared lane. It does not mean the product is retired or all retained worktrees are idle. Update this table when root assigns work or adopts the candidate. `pm lanes` reads Worktree and Holds; State, Owner, and Next action remain human declarations. `pm next` does not infer approval from those columns.

The installed next surface labels a lane with no worktree as busy. The product row remains idle and unassigned with a free claim. Root must assign a worktree before pm can advertise an available worker. This rendering does not authorize product work.
