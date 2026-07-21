# NNN — Short observable outcome

Status: proposed

## Why

Explain the user or system problem and why this is the next coherent slice.

## Scope

- Included responsibility.
- Explicit exclusions and boundaries.
- Exact durable docs that change with the behavior.

## Success Checklist

- Observable CLI or service behavior and its `make spec` proof.
- Inside-out unit/integration tests, including malformed and failure paths.
- Exactness or retained-corpus proof.
- Representative latency, allocation, page-fault, memory, or size proof when
  the ticket touches a query path or artifact format.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

Record at least three hard choices. For each, state the consideration, options,
trade-offs, decision, and why. Link an accepted ADR when it already owns the
choice.

## Dependencies

Ticket IDs or `None`.

## Notes

- Working-in-isolation facts the development agent cannot infer from the ticket.
- Exact commands, fixture/source constraints, and public-repository hygiene.
- Evidence shown here is illustrative unless explicitly named as an artifact.

## Independent Ticket Review

Reviewer: pending

Record findings and the coordinator's disposition before changing status to
`ready`. Every material response returns to this reviewer, who must record
approval before dispatch. The reviewer is read-only and cannot be the
implementation or code-review agent.

## Implementation Evidence

Developer: pending

Record focused tests, measurements, generated artifact identities, and any
scope-relevant deviation, then set status to `review`. The developer cannot be
either reviewer.

## Adversarial Code Review

Reviewer: pending

Record diff/test findings and their disposition before completion. The reviewer
is read-only and cannot be the ticket reviewer or developer. Material fixes are
returned to this reviewer. The ticket may become `complete` and enter final
gates only after the reviewer records approval.
