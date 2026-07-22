# Active Tickets

This directory holds at most one active implementation ticket. Create it from
[`../templates/ticket.md`](../templates/ticket.md) through a dedicated
ticket-author sub-agent, obtain review from a different read-only sub-agent,
and dispatch that reviewed file as the development agent's complete instruction.
The coordinator marks it `ready` and commits and pushes it before development
starts.

When Ian explicitly requests a reviewed ticket set, dependency-gated drafts may
coexist here with status `proposed`. They are design evidence, not a backlog:
only the immediate dependency-free ticket may become `ready`, and every later
draft must be reauthored or rechecked by its ticket author and then approved by
its ticket reviewer against the shipped dependency before promotion.

Development and adversarial code review use two more distinct sub-agents. Code
findings go back to the same developer and then the same code reviewer. Material
changes to reviewed scope go back to the same ticket author and ticket reviewer.
All four identities, findings, dispositions, documentation proof, and approvals
are recorded in the ticket. Ticket authors and developers never commit or push;
the coordinator alone does so after independent approval.

After independent code-review approval and final gates, the coordinator commits
and pushes the implementation together with the ticket marked `complete` and
its evidence. A material final-gate or stale-documentation finding returns to
the same developer and code reviewer; a scope defect returns to the same ticket
author and ticket reviewer.
Then remove the ticket in an immediate planning-cleanup commit and push again.
Shipped behavior remains in code, tests, specs, architecture, durable artifacts,
and git history—not a second completed-ticket roadmap.
