---
flow: build
priority: 1
deps: []
---
# Rust literal lint catches embedded hand wraps

## Outcome

The Rust text check catches an accidental physical newline and indentation inside an ordinary string literal, including the form recorded in draft 0103. It continues to allow deliberate multiline and raw strings through explicit, reviewable rules.

## Done, observably

- Add a red fixture for an ordinary string whose runtime value gains a newline and indentation after a hand wrap. The current checker must miss the fixture before the repair.
- Track ordinary Rust string boundaries across physical lines closely enough to detect the recorded defect. Handle escapes and byte strings needed by the current crate tree. Refuse an unsupported form with a named diagnostic instead of claiming it is clean.
- Keep deliberate raw and multiline content available through a narrow documented exemption. Existing source remains accepted without broad path exemptions.
- Keep work linear in checked source bytes and report the number of Rust sources checked.
- Archive draft 0103. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the repository lint check, its fixtures, and documentation only. Do not rewrite product messages, parse Rust generally, or change product behavior and public output.
