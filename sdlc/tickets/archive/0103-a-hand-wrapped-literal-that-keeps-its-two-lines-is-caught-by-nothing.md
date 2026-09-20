---
---
# A hand-wrapped literal that keeps its two lines is caught by nothing

`tests/rust-literal-continuity.sh`, added by ticket 0085, catches the collapse
as it actually happened: the five messages repaired in commit `6cbe88f` were
each a single source line carrying a fourteen-space gap between two words. The
scan is `[[:alnum:]_] {3,}[[:alnum:]_]` read line by line, which is exactly
that trace.

There is a second way the same accident lands, and it leaves a different trace.
Delete the trailing `\` and leave the two lines where they are:

    "stopped using model cache {}: it no longer holds the file it opened, so restart to
     use what is there now",

That is a valid Rust string literal spanning two lines. The runtime text now
carries a newline and thirteen spaces in the middle of the sentence, which is
the same defect the gate exists to catch, and the source carries no run of
spaces between two word characters -- the indent stands at the start of a line.

Measured on 2026-09-11 against `crates/pangopup-cache/src/lib.rs:703` on the
ticket 0085 candidate: `rustfmt --edition 2024 --check` exited 0,
`cargo fmt --all --check` exited 0, and
`bash tests/rust-literal-continuity.sh` reported
`92 Rust source(s) under crates/, no collapsed wrap` and exited 0.

Catching it means telling a line that continues a string literal from a line
that begins one, which needs the scan to track whether it stands inside a
literal. That is a different reading from the one-line trace, so it is a
decision about what the gate should read rather than a pattern to widen.

A raw string is the neighbouring question. `r#"..."#` cannot be hand-wrapped
with `\` at all, so alignment inside one is always deliberate, and the
one-line scan would refuse it. The crate tree carries no such raw string
today.

Found while verifying ticket 0085.
