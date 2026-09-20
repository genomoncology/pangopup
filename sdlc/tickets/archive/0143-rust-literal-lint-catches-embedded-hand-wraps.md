---
flow: build
priority: 1
deps: []
---
# Rust literal lint catches embedded hand wraps

## Outcome

The Rust text check catches an accidental physical newline and indentation inside an ordinary string literal, including the form recorded in draft 0103. It continues to allow deliberate multiline and raw strings through explicit, reviewable rules.

## Done, observably

- Add a red fixture for an ordinary string whose runtime value gains a newline and indentation after a hand wrap. The current checker must miss the fixture before the repair. Preserve the existing rejection of collapsed indentation inside a single-line literal and add a positive fixture for a valid backslash continuation.
- Track ordinary Rust string boundaries across physical lines closely enough to detect the recorded defect. Pin escaped quotes and backslashes, ordinary and byte-string continuations, character literals containing `"`, lifetimes, line comments, nested block comments, C strings, and raw and raw-byte strings with differing hash delimiters. Raw-string contents remain data. Unsupported or unterminated forms report path and line.
- Keep deliberate ordinary multiline content through narrow named exemptions. Preserve the existing `LEGACY_USAGE` exception. Accept the current multiline SQL without file-wide or word-prefix exemptions. A defective literal beside an exemption remains rejected, and a stale exemption fails.
- Keep work linear in checked source bytes, prove it with a long fixture, report the number of Rust sources checked, and retain Bash 3.2 portability.
- Archive draft 0103. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the repository lint check, its fixtures, and documentation only. Do not rewrite product messages, parse Rust generally, or change product behavior and public output.
