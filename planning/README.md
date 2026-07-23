# Pangopup Planning

This directory is the single source of truth for unfinished Pangopup work.
Durable technical rules belong in `../architecture/`; executable behavior
belongs in Rust tests and `../spec/`.

## Structure

- [`goals.md`](goals.md) — durable outcomes and non-goals.
- [`faq.md`](faq.md) — settled explanations and open product choices.
- [`frontier.md`](frontier.md) — rolling dependency-ordered outcome roadmap,
  not live ticket status or a prewritten backlog.
- `issues/` — observed problems not yet shaped into work.
- `tickets/` — the sole owner of live work status; one implementation-ready
  vertical slice at a time.
- `templates/` — the required ticket structure and review record.
- `artifacts/` — measurements and evidence that must survive a ticket.
- `failures/` — durable failure analyses.
- `handoffs/` — temporary continuity notes, closed after pickup.

Architecture decisions live in `../architecture/decisions/`.

## Working rule

Do not create a ticket backlog for the whole product. The frontier is a rolling
outcome roadmap: it says what capabilities remain and in what dependency order,
without pretending their implementation contracts are already known. Select
the next smallest slice only after its input evidence, observable acceptance
test, and inside-out failure tests are known. File-format work must also state
the size and performance evidence that can accept or reject it.

## Ticket lifecycle

```text
proposed -> independently approved -> ready -> in-progress -> review
         -> independently approved -> complete
```

One coordinating agent owns the lifecycle and writes one ticket at a time from
the previous shipped result and rolling frontier; it does not implement product
code or review its own ticket. Three distinct sub-agents provide independent
ticket review, development, and adversarial code review. The two reviewers are
read-only. Ticket findings return to the coordinator and then the same ticket
reviewer; code findings return to the same developer and then the same code
reviewer. A stage advances only after its reviewer records approval.

Developers never commit or push. The coordinator alone commits and pushes after
the applicable independent approval.

The reviewed `ready` ticket is the development agent's complete instruction.
Do not rely on chat history, sibling tickets, or unstated conventions. After
code review, material fixes go back to the developer and then back through
the same review. A material scope change to a `ready` ticket goes back to the
coordinator and ticket reviewer first. The coordinator records review
identities, ticket revisions, mechanical command results, and status
transitions. Run the three final gates and commit only after review is clear.

Documentation follows the same proof chain: the ticket names it, the developer
updates it with the implementation, the code reviewer checks it, and the
coordinator performs a final stale-claim scan. A material final-gate,
documentation, or implementation defect returns to the same developer and then
the same code reviewer. If it reveals a scope defect instead, it returns to the
coordinator and same ticket reviewer before development resumes.

Active tickets are working instructions, not a permanent release ledger. When
the reviewed implementation ships, preserve durable behavior in code, tests,
specs, architecture, and any required artifact. Commit the independently
approved `ready` ticket before dispatch. After code-review approval and final
gates, commit the implementation together with the ticket marked `complete` and
its full evidence. Then immediately remove the completed ticket in a separate
planning-cleanup commit. Git retains both reviewed states without turning the
live planning tree into a duplicate completion log.
