# Rust literal continuity scan

`rust-literal-scan.awk` gives `tests/rust-literal-continuity.sh` a bounded lexical view of Rust source under `crates/`. It refuses an unescaped physical newline inside an ordinary, byte, or C string. It also refuses three or more spaces between word characters inside those strings. A final backslash continues an ordinary line and remains accepted.

The scanner tracks ordinary strings, escaped quotes and backslashes, character and byte-character literals, lifetimes and labels, line comments, nested block comments, C strings, and raw, raw-byte, and raw-C strings with exact hash delimiters. Raw-string contents remain data. An unterminated ordinary string, raw string, character literal, or block comment fails with its path and opening line. The scanner does not parse Rust expressions, macros, types, or names.

`tests/rust-literal-continuity-exemptions.tsv` names every deliberate finding by kind, path, line, and exact opening source line. The existing aligned `LEGACY_USAGE` value and the current multiline SQL literals are the complete list. A moved, removed, or changed literal leaves a stale exemption and fails. A nearby literal receives no exemption.

The scanner advances through each source line once. Raw closing-delimiter search jumps directly to the next candidate. The coverage harness scans a generated source larger than one megabyte in addition to its token and failure fixtures. The shell harness uses features available in Bash 3.2, and the scanner uses POSIX Awk features.
