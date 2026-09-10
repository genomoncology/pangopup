---
---
# Nothing catches a hand-wrapped string literal that lost its continuation

A long Rust string literal in this repo is wrapped by hand: a `\` ends the
line and the next line is indented to sit under the opening quote, so the
runtime text reads as one sentence. When the `\` is missing the wrap collapses
onto one line and the indentation survives as a run of internal spaces inside
the literal. The sentence still compiles, still asserts the same thing, and
reads wrong the moment anyone sees it.

Five assertion messages in `crates/pangopup-cache/src/lib.rs` arrived this way
and were repaired under ticket 0075. No gate saw them. `cargo fmt` does not
reflow string literal contents, clippy has no lint for it, and `sdlc/scripts/lint`
adds nothing that would.

The pattern is mechanical: three or more spaces between two word characters
inside a Rust source file, which the whole crate tree otherwise contains in
exactly one deliberate place (the aligned usage text in
`crates/pangopup-build/src/main.rs`). A check in the lint ladder that rejects
the run, with that one alignment case exempted, would turn a defect that has to
be noticed into one that cannot land.
