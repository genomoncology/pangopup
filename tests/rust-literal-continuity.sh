#!/usr/bin/env bash
set -euo pipefail

# Rust permits an ordinary string to cross a physical line. Deleting the
# trailing backslash from a hand-wrapped message therefore compiles while it
# adds a newline and indentation to the runtime value. This gate tracks the
# token boundaries that can hide a quote and refuses that trace. It also keeps
# the older check for three or more spaces between word characters inside an
# ordinary string.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scanner="$repository/tests/support/rust-literal-scan.awk"
exemptions="$repository/tests/rust-literal-continuity-exemptions.tsv"

fail() { printf 'rust literal continuity: %s\n' "$*" >&2; exit 1; }

[[ -r "$scanner" ]] || fail "scanner is not readable: $scanner"
[[ -r "$exemptions" ]] || fail "exemption list is not readable: $exemptions"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

scan() {
    local root=$1
    local allowed=$2
    shift 2
    LC_ALL=C awk -v root="$root" -v exemptions="$allowed" -f "$scanner" "$@"
}

scan_accounted() {
    local root=$1
    local allowed=$2
    local report=$3
    shift 3
    LC_ALL=C awk -v root="$root" -v exemptions="$allowed" -v work_report="$report" \
        -f "$scanner" "$@"
}

empty_exemptions="$work/empty-exemptions.tsv"
: >"$empty_exemptions"

# --- 1. exact red and positive fixtures ------------------------------------
fixture="$work/fixture"
mkdir -p "$fixture/crates/other/src" "$fixture/crates/pangopup-build/src"

{
    printf 'fn embedded() {\n'
    printf '    panic!("the recorded setup no longer matches\n'
    printf '             so the cache was discarded");\n'
    printf '}\n'
    printf 'const COLLAPSED: &str = "the recorded setup no longer matches   so the cache was discarded";\n'
    printf 'const BYTE_WRAP: &[u8] = b"the bytes continue\n'
    printf '                              with indentation";\n'
} >"$fixture/crates/other/src/bad.rs"

{
    printf 'const BAD: &str = "vertical tab follows \\\n'
    printf '\v\n'
    printf 'text";\n'
} >"$fixture/crates/other/src/vertical-tab.rs"

{
    printf 'const BAD: &str = "form feed follows \\\n'
    printf '\f\n'
    printf 'text";\n'
} >"$fixture/crates/other/src/form-feed.rs"

{
    printf 'const SQL: &str = "SELECT key\n'
    printf '                        FROM entries";\n'
    printf 'const NEIGHBOR: &str = "this defect remains\n'
    printf '                         visible beside the exemption";\n'
} >"$fixture/crates/other/src/exempt-and-bad.rs"

printf 'const LEGACY_USAGE: &str = "Usage: inspect <DIR>\\n       open <FILE>";\n' \
    >"$fixture/crates/pangopup-build/src/main.rs"
printf 'const LEGACY_USAGE: &str = "Usage: inspect <DIR>\\n       open <FILE>";\n' \
    >"$fixture/crates/other/src/borrowed.rs"

fixture_exemptions="$work/fixture-exemptions.tsv"
{
    printf 'fixture-sql\tmultiline\tcrates/other/src/exempt-and-bad.rs\t1\tconst SQL: &str = "SELECT key\n'
    printf 'fixture-legacy\tcollapsed\tcrates/pangopup-build/src/main.rs\t1\tconst LEGACY_USAGE: &str = "Usage: inspect <DIR>\\n       open <FILE>";\n'
} >"$fixture_exemptions"

fixture_sources=()
while IFS= read -r source; do fixture_sources+=("$source"); done < <(
    find "$fixture/crates" -type f -name '*.rs' | sort
)
(( ${#fixture_sources[@]} == 6 )) \
    || fail "planted 6 red fixture sources and found ${#fixture_sources[@]}"

if fixture_findings=$(scan "$fixture" "$fixture_exemptions" "${fixture_sources[@]}" 2>&1); then
    fail 'the scanner accepted the red embedded-wrap fixtures'
fi

for expected in \
    'crates/other/src/bad.rs:2: ordinary string contains an unescaped physical newline' \
    'crates/other/src/bad.rs:5: ordinary string contains three or more spaces between word characters' \
    'crates/other/src/bad.rs:6: ordinary string contains an unescaped physical newline' \
    'crates/other/src/exempt-and-bad.rs:3: ordinary string contains an unescaped physical newline' \
    'crates/other/src/form-feed.rs:1: ordinary string contains an unescaped physical newline' \
    'crates/other/src/vertical-tab.rs:1: ordinary string contains an unescaped physical newline' \
    'crates/other/src/borrowed.rs:1: ordinary string contains three or more spaces between word characters'; do
    grep -Fq -- "$expected" <<<"$fixture_findings" \
        || fail "the red fixture did not produce '$expected': $fixture_findings"
done

if grep -Fq 'exempt-and-bad.rs:1:' <<<"$fixture_findings"; then
    fail "the exact SQL exemption did not suppress its named literal: $fixture_findings"
fi
if grep -Fq 'pangopup-build/src/main.rs:1:' <<<"$fixture_findings"; then
    fail "the exact LEGACY_USAGE exemption did not suppress its named literal: $fixture_findings"
fi

positive="$work/positive"
mkdir -p "$positive/crates/other/src"
{
    printf 'const TEXT: &str = "a deliberate ordinary \\\n'
    printf '    continuation";\n'
    printf 'const BYTES: &[u8] = b"a deliberate byte \\\n'
    printf '    continuation";\n'
    printf 'const ESCAPES: &str = "an escaped quote: \\" and slash: \\\\";\n'
    printf 'const C_TEXT: &core::ffi::CStr = c"one-line C text";\n'
    printf 'fn tokens<'"'"'a>(value: &'"'"'a str) {\n'
    printf '    let quote = '\''"'\'';\n'
    printf '    let byte_quote = b'\''"'\'';\n'
    printf '    let byte_ascii = b'\''a'\'';\n'
    printf '    '\''outer: loop { break '\''outer; }\n'
    printf '    let _ = value;\n'
    printf '}\n'
    printf '/* outer "ordinary-looking\n'
    printf '   /* nested b"byte-looking */\n'
    printf '   still comment " */\n'
    printf '// "line-comment-looking\n'
    printf 'const RAW: &str = r##"raw text\n'
    printf '    with " and "# non-closing data\n'
    printf '    closes here"##;\n'
    printf 'const RAW_BYTE: &[u8] = br#"raw bytes\n'
    printf '    with an ordinary-looking " quote"#;\n'
    printf 'const RAW_C: &core::ffi::CStr = cr###"raw C\n'
    printf '    data"###;\n'
} >"$positive/crates/other/src/tokens.rs"

{
    printf 'const CRLF: &str = "continued across CRLF \\\r\n'
    printf '    text";\r\n'
    printf 'const BLANKS: &str = "continued across blank lines \\\n'
    printf '    \n'
    printf '\t\n'
    printf '    text";\n'
    printf 'const BYTE_BLANKS: &[u8] = b"continued byte text \\\n'
    printf ' \t \n'
    printf '    bytes";\n'
} >"$positive/crates/other/src/continuations.rs"

{
    printf "const CHAR: char = 'é';\n"
    printf "fn unicode_lifetime<'é>(value: &'é str) -> &'é str { value }\n"
} >"$positive/crates/other/src/unicode.rs"

awk 'BEGIN { for (i = 1; i <= 25000; i++) printf "const LINE_%d: &str = \"checked linear fixture %d\";\n", i, i }' \
    >"$positive/crates/other/src/long.rs"

scale_pairs=(500 1000 2000)
for pair_count in "${scale_pairs[@]}"; do
    awk -v pairs="$pair_count" 'BEGIN {
        printf "const MANY: &[&str] = &["
        for (i = 1; i <= pairs; i++) {
            printf "\"ordinary-%d\",r#\"raw-%d\"# ,", i, i
        }
        print "];"
    }' >"$positive/crates/other/src/long-single-line-$pair_count.rs"
done

positive_sources=()
while IFS= read -r source; do positive_sources+=("$source"); done < <(
    find "$positive/crates" -type f -name '*.rs' | sort
)
positive_findings=$(scan "$positive" "$empty_exemptions" "${positive_sources[@]}" 2>&1) \
    || fail "the scanner refused continuations, comments, characters, lifetimes, raw strings, C strings, or the long fixture: $positive_findings"
[[ -z "$positive_findings" ]] \
    || fail "the positive fixture produced findings: $positive_findings"

rustc --edition 2024 --crate-type lib --emit metadata -A warnings \
    -o "$work/continuations.rmeta" "$positive/crates/other/src/continuations.rs"
rustc --edition 2024 --crate-type lib --emit metadata -A warnings \
    -o "$work/unicode.rmeta" "$positive/crates/other/src/unicode.rs"

work_report="$work/single-line-work.tsv"
previous_bytes=0
previous_work=0
scaling_summary=''
for pair_count in "${scale_pairs[@]}"; do
    single_line="$positive/crates/other/src/long-single-line-$pair_count.rs"
    single_line_findings=$(scan_accounted "$positive" "$empty_exemptions" "$work_report" \
        "$single_line" 2>&1) \
        || fail "the scanner refused the $pair_count-pair single-line fixture: $single_line_findings"
    [[ -z "$single_line_findings" ]] \
        || fail "the $pair_count-pair single-line fixture produced findings: $single_line_findings"
    [[ "$(wc -l <"$single_line" | tr -d '[:space:]')" == 1 ]] \
        || fail "the $pair_count-pair work fixture no longer occupies exactly one physical line"
    read -r single_line_bytes single_line_work <"$work_report"
    [[ "$single_line_bytes" =~ ^[1-9][0-9]*$ && "$single_line_work" =~ ^[1-9][0-9]*$ ]] \
        || fail "the scanner wrote malformed work accounting: $(cat "$work_report")"
    [[ "$single_line_bytes" == "$(wc -c <"$single_line" | tr -d '[:space:]')" ]] \
        || fail "the scanner byte accounting does not equal the $pair_count-pair fixture size"
    (( single_line_work <= single_line_bytes * 12 )) \
        || fail "the $pair_count-pair scan used $single_line_work work units for $single_line_bytes bytes"
    if (( previous_bytes > 0 )); then
        (( single_line_work * previous_bytes * 10 <= previous_work * single_line_bytes * 11 )) \
            || fail "work per byte grew by more than 10 percent from $previous_bytes to $single_line_bytes bytes"
    fi
    if [[ -n "$scaling_summary" ]]; then scaling_summary="$scaling_summary, "; fi
    scaling_summary="${scaling_summary}${pair_count} pairs ${single_line_bytes}/${single_line_work} bytes/work"
    previous_bytes=$single_line_bytes
    previous_work=$single_line_work
done

# --- 2. malformed tokens and stale exemptions fail closed ------------------
malformed="$work/malformed"
mkdir -p "$malformed/crates/other/src"
printf 'const BAD: &str = "never closes;\n' >"$malformed/crates/other/src/string.rs"
printf 'const BAD: &str = r###"never closes;\n' >"$malformed/crates/other/src/raw.rs"
printf 'const BAD: char = '\''\\\n' >"$malformed/crates/other/src/character.rs"
printf 'const BAD: char = '\''!;\n' >"$malformed/crates/other/src/punctuation-character.rs"
printf 'const BAD: char = '\'' ;\n' >"$malformed/crates/other/src/space-character.rs"
printf 'const BAD: u8 = b'\''!;\n' >"$malformed/crates/other/src/punctuation-byte.rs"
printf 'const BAD: u8 = b'\''a;\n' >"$malformed/crates/other/src/ascii-byte.rs"
printf 'const BAD: u8 = b'\''_;\n' >"$malformed/crates/other/src/underscore-byte.rs"
printf 'fn bad() { /* never closes\n' >"$malformed/crates/other/src/comment.rs"
awk 'BEGIN {
    printf "const BAD: &str = r"
    for (i = 1; i <= 256; i++) printf "#"
    print "\"unsupported\";"
}' >"$malformed/crates/other/src/raw-hash-limit.rs"

malformed_sources=()
while IFS= read -r source; do malformed_sources+=("$source"); done < <(
    find "$malformed/crates" -type f -name '*.rs' | sort
)
if malformed_findings=$(scan "$malformed" "$empty_exemptions" "${malformed_sources[@]}" 2>&1); then
    fail 'the scanner accepted unterminated Rust token forms'
fi
for expected in \
    'crates/other/src/ascii-byte.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line' \
    'crates/other/src/character.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line' \
    'crates/other/src/comment.rs:1: unsupported or unterminated Rust source: nested block comment reaches end of file' \
    'crates/other/src/punctuation-byte.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line' \
    'crates/other/src/punctuation-character.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line' \
    "crates/other/src/raw-hash-limit.rs:1: unsupported or unterminated Rust source: raw string delimiter exceeds Rust's 255-hash limit" \
    'crates/other/src/raw.rs:1: unsupported or unterminated Rust source: raw string literal reaches end of file' \
    'crates/other/src/space-character.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line' \
    'crates/other/src/string.rs:1: unsupported or unterminated Rust source: ordinary string literal reaches end of file' \
    'crates/other/src/underscore-byte.rs:1: unsupported or unterminated Rust source: character literal does not close on its physical line'; do
    grep -Fq -- "$expected" <<<"$malformed_findings" \
        || fail "the malformed fixture did not produce '$expected': $malformed_findings"
done

rustc_rejects() {
    local source=$1
    local code=$2
    local name=$3
    local stderr_file="$work/$name.stderr"
    if rustc --edition 2024 --crate-type lib --emit metadata -A warnings \
        -o "$work/$name.rmeta" "$source" 2>"$stderr_file"; then
        fail "rustc accepted the malformed $name fixture"
    fi
    grep -Fq "error[$code]" "$stderr_file" \
        || fail "rustc did not report $code for the malformed $name fixture: $(cat "$stderr_file")"
}

rustc_rejects "$malformed/crates/other/src/punctuation-character.rs" E0762 punctuation-character
rustc_rejects "$malformed/crates/other/src/space-character.rs" E0762 space-character
rustc_rejects "$malformed/crates/other/src/punctuation-byte.rs" E0763 punctuation-byte
rustc_rejects "$malformed/crates/other/src/ascii-byte.rs" E0763 ascii-byte
rustc_rejects "$malformed/crates/other/src/underscore-byte.rs" E0763 underscore-byte

stale_exemptions="$work/stale-exemptions.tsv"
printf 'removed\tmultiline\tcrates/other/src/tokens.rs\t99\tconst REMOVED: &str = "gone\n' \
    >"$stale_exemptions"
if stale_findings=$(scan "$positive" "$stale_exemptions" "${positive_sources[@]}" 2>&1); then
    fail 'the scanner accepted a stale exemption'
fi
grep -Fq "stale Rust literal exemption 'removed'" <<<"$stale_findings" \
    || fail "the stale exemption did not name itself: $stale_findings"

# --- 3. scan the checked crate tree ----------------------------------------
sources=()
while IFS= read -r source; do sources+=("$source"); done < <(
    find "$repository/crates" -type f -name '*.rs' -not -path '*/target/*' | sort
)
(( ${#sources[@]} > 0 )) \
    || fail 'found no Rust source under crates/'

if findings=$(scan "$repository" "$exemptions" "${sources[@]}" 2>&1); then
    [[ -z "$findings" ]] || fail "scanner reported output without failing: $findings"
else
    printf 'rust literal continuity: checked source contains accidental or unsupported literals:\n%s\n' \
        "$findings" >&2
    exit 1
fi

long_bytes=$(wc -c <"$positive/crates/other/src/long.rs" | tr -d '[:space:]')
exemption_count=$(awk -F '\t' '!/^[[:space:]]*(#|$)/ { count++ } END { print count + 0 }' "$exemptions")
printf 'rust literal continuity: %s Rust source(s), no accidental wrap; %s-byte line corpus; single-line scaling: %s; %s exact exemption(s) used\n' \
    "${#sources[@]}" "$long_bytes" "$scaling_summary" "$exemption_count"
