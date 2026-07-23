# Active Tickets

This directory holds at most one active implementation ticket. The coordinator
creates it from [`../templates/ticket.md`](../templates/ticket.md) after
reconciling the previous shipped outcome with the rolling frontier, obtains
review from a read-only sub-agent, and dispatches the approved file as the
development agent's complete instruction. The coordinator marks it `ready` and
commits and pushes it before development starts. Future work stays as outcome
slots in `../frontier.md`, not prewritten tickets.

Development and adversarial code review use two more distinct sub-agents. Code
findings go back to the same developer and then the same code reviewer. Material
changes to reviewed scope go back to the coordinator and same ticket reviewer.
All three sub-agent identities, findings, dispositions, documentation proof,
and approvals are recorded in the ticket. Developers never commit or push; the
coordinator alone does so after independent approval.

A ticket that explicitly includes a public or irreversible external effect
uses the narrow final lifecycle below:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator external effect -> complete -> commit/push -> cleanup
```

The coordinator generates production small outputs before code review, pushes
the independently approved preparation, requires that exact commit's green
remote gate and pinned audit, and alone performs the effect. Redacted audit and
effect evidence enter the later completion commit. Developers and reviewers
never mutate external state.

After independent code-review approval and final gates, the coordinator commits
and pushes the implementation together with the ticket marked `complete` and
its evidence. A material final-gate or stale-documentation finding returns to
the same developer and code reviewer; a scope defect returns to the coordinator
and same ticket reviewer. Then remove the ticket in an immediate
planning-cleanup commit and push again. Shipped behavior remains in code, tests,
specs, architecture, durable artifacts, and git history—not a second completed-
ticket roadmap.
