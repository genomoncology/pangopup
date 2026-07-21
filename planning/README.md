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
- `artifacts/` — measurements and evidence that must survive a ticket.
- `failures/` — durable failure analyses.
- `handoffs/` — temporary continuity notes, closed after pickup.

Architecture decisions live in `../architecture/decisions/`.

## Working rule

Do not create a backlog for the whole product. Select the next smallest slice
from the frontier only after its input evidence, observable acceptance test, and
inside-out failure tests are known. File-format work must also state the size and
performance evidence that can accept or reject it.
