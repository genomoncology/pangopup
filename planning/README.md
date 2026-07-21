# Pangopup Planning

This directory is the single source of truth for unfinished Pangopup work.
Durable technical rules belong in `../architecture/`; executable behavior
belongs in Rust tests and `../spec/`.

## Structure

- [`goals.md`](goals.md) — durable outcomes and non-goals.
- [`faq.md`](faq.md) — settled explanations and open product choices.
- [`frontier.md`](frontier.md) — current boundary and the few nearby fronts.
- `issues/` — observed problems not yet shaped into work.
- `tickets/` — one implementation-ready vertical slice at a time.
- `templates/` — the required ticket structure and review record.
- `artifacts/` — measurements and evidence that must survive a ticket.
- `failures/` — durable failure analyses.
- `handoffs/` — temporary continuity notes, closed after pickup.

Architecture decisions live in `../architecture/decisions/`.

## Working rule

Do not create a backlog for the whole product. Select the next smallest slice
from the frontier only after its input evidence, observable acceptance test, and
inside-out failure tests are known. File-format work must also state the size and
performance evidence that can accept or reject it.

## Ticket lifecycle

```text
proposed -> independently approved -> ready -> in-progress -> review
         -> independently approved -> complete
```

One coordinating agent owns the lifecycle but not the implementation. Three
different sub-agents provide the independent ticket review, development, and
adversarial code review. The two reviewers are read-only; only the development
agent edits product files. Findings and their dispositions are recorded in the
ticket and returned to the same reviewer. A stage advances only after that
reviewer records approval.

The reviewed `ready` ticket is the development agent's complete instruction.
Do not rely on chat history, sibling tickets, or unstated conventions. After
code review, material fixes go back to the developer and then back through
review. Run the three final gates and commit only after review is clear.

Active tickets are working instructions, not a permanent release ledger. When
the reviewed implementation ships, preserve durable behavior in code, tests,
specs, architecture, and any required artifact. Commit the independently
approved `ready` ticket before dispatch. After code-review approval and final
gates, commit the implementation together with the ticket marked `complete` and
its full evidence. Then immediately remove the completed ticket in a separate
planning-cleanup commit. Git retains both reviewed states without turning the
live planning tree into a duplicate completion log.
