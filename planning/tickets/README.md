# Active Tickets

This directory holds at most one active implementation ticket. Create it from
[`../templates/ticket.md`](../templates/ticket.md), obtain and record independent
ticket review before marking it `ready`, and dispatch that reviewed file as the
development agent's complete instruction. Commit and push the `ready` ticket
before development starts.

When Ian explicitly requests a reviewed ticket set, dependency-gated drafts may
coexist here with status `proposed`. They are design evidence, not a backlog:
only the immediate dependency-free ticket may become `ready`, and every later
draft must be rechecked against the shipped dependency before promotion.

After independent code-review approval and final gates, commit and push the
implementation together with the ticket marked `complete` and its evidence.
Then remove the ticket in an immediate planning-cleanup commit and push again.
Shipped behavior remains in code, tests, specs, architecture, durable artifacts,
and git history—not a second completed-ticket roadmap.
