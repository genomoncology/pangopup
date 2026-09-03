---
flow: build
priority: 5
---
# State the goal as a behavior, in one line

State the goal: what the system does after this ticket that it does
not do today, and the evidence that it does not do it (run ids,
event seqs, file paths, measured output). Write for an agent that
receives this file and nothing else.

State the required behavior in observable terms. WARNING — a ticket
states outcomes, never designs. Naming internal files, enum values,
field mappings, or exact recovery decisions hands design work to
the ticket, and the design stage owns those. Each design decision
written here becomes a constraint the reviewer must hold the design
to, and an over-specified ticket fights its own flow.

Settle the hard choices the agent cannot make alone, and name what
is out of scope by name when lookalikes exist.

Done, observably:

- Each bullet is something a test can pin or an operator can see.
- Say what becomes true, not how it is implemented.

Boundary: what this ticket must not change, and where the work
stops. Name the neighboring behavior that stays as it is.
