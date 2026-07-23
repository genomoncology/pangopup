# NNN — Short observable outcome

Status: proposed

## Why

Explain the user or system problem and why this is the next coherent slice.

## Scope

- Included responsibility.
- Explicit exclusions and boundaries.
- Exact durable and user-facing docs that change with the behavior. Name every
  file; documentation is implemented and reviewed in the same diff as behavior.

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

## Coordinator Authorship

Coordinator: pending

The coordinator writes this ticket from the previous shipped outcome and the
rolling frontier. It owns substantive ticket-review remediation but does not
implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: pending

Record findings and the coordinator's disposition before changing status to
`ready`. Every material response returns to the coordinator and then this same
reviewer, who must record approval before dispatch. The reviewer is read-only.
Ticket reviewer, developer, and code reviewer must be three distinct
sub-agents.

## Implementation Evidence

Developer: pending

Record focused tests, measurements, generated artifact identities, and any
scope-relevant deviation, including named documentation changes, then set
status to `review`. The developer cannot be the author or either reviewer and
does not commit or push.

## Adversarial Code Review

Reviewer: pending

Record diff/test findings and their disposition before completion. The reviewer
is read-only and cannot be the author, ticket reviewer, or developer. Material
fixes return to the same developer and then this reviewer. Review includes every
named documentation file and a check that shipped and future behavior are not
confused. The ticket may become `complete` and enter final gates only after the
reviewer records approval.

## External Effect Evidence

Coordinator: not applicable

For the exceptional ticket that names a public or irreversible external
effect, replace `not applicable` with pending evidence and use this lifecycle:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator external effect -> complete -> commit/push -> cleanup
```

The coordinator generates production small outputs before code review. After
approval it pushes the reviewed preparation, requires a green remote gate and
the pinned audit for that exact commit, performs the named external effect,
then records only the reviewed redacted evidence here. Developers and
reviewers never perform the effect.

## Coordinator Final Check

Coordinator: pending

Record final `make lint`, `make test`, and `make spec` results plus a
documentation stale-claim scan. The coordinator authors and remediates the
ticket, orchestrates the independent stages, records evidence, and commits and
pushes approved work; it does not implement product code or review its own
ticket. A material final-gate or documentation finding returns to the same
developer and code reviewer; a scope defect returns to the coordinator and same
ticket reviewer.
