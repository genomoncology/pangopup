# What a gate expected, where it looked, and what stood there instead.
# Sourced, never run.
#
# A qualification harness that asserts with a bare command under
# `set -euo pipefail` stops on status 1 and prints nothing at all. Measured on
# 2026-09-10 against `tests/executable-delivery.sh`: changing `install.sh` line
# 23 from `unknown argument: $1` to `unrecognised option: $1` made the harness
# exit 1 after printing two lines, both the version banner from earlier work.
# Nothing said which assertion failed, what it wanted, or what line it stood
# on, so a maintainer reads an exit code and bisects.
#
# Four forms, each naming the expectation and what was found:
#
#     require_text <file> <text>       the text stands somewhere in the file
#     require_line <file> <line>       the line stands whole in the file
#     require_pattern <file> <ere>     the extended pattern matches somewhere
#     equal <what> <expected> <actual> the two strings are the same
#
# Each exits non-zero itself, so a caller that writes nothing after it still
# stops, and the repair cannot be undone by dropping a `|| fail` from one site.
#
# A file a gate cannot read is refused by the first three. `grep` answers "not
# found" for a path that does not exist, so a mistyped path or a file the
# harness failed to write would otherwise report a plain assertion failure
# rather than the thing that actually went wrong.
#
# `tests/support/forbidden-text.sh` is the other half: `refuse_text` for a text
# that must not be there. `tests/negative-assertion-strength.sh` holds both,
# and holds that these harnesses carry no bare assertion any more.

__expected_refuse() {
    printf 'expectation failed: %s\n' "$1" >&2
    shift
    while (($#)); do
        printf '  %s\n' "$1" >&2
        shift
    done
    exit 1
}

__expected_readable() {
    if [[ ! -f "$1" ]]; then
        __expected_refuse "a gate looked for something in a file it cannot read" \
            "wanted: $2" \
            "in:     $1, which is not a readable file"
    fi
}

require_text() {
    __expected_readable "$1" "$2"
    if ! grep -Fq -- "$2" "$1"; then
        __expected_refuse 'a text a gate requires is not in the file it reads' \
            "wanted: $2" \
            "in:     $1"
    fi
}

require_line() {
    __expected_readable "$1" "$2"
    if ! grep -Fxq -- "$2" "$1"; then
        __expected_refuse 'a whole line a gate requires is not in the file it reads' \
            "wanted the whole line: $2" \
            "in:                    $1"
    fi
}

require_pattern() {
    __expected_readable "$1" "$2"
    if ! grep -Eq -- "$2" "$1"; then
        __expected_refuse 'a pattern a gate requires matches nothing in the file it reads' \
            "wanted a match for: $2" \
            "in:                 $1"
    fi
}

equal() {
    if [[ "$3" != "$2" ]]; then
        __expected_refuse "$1" \
            "expected: $2" \
            "found:    $3"
    fi
}
