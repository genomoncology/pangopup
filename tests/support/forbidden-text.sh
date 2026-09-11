# The one way a gate forbids a text. Sourced, never run.
#
# `set -e` does not exit on a command whose status is inverted with `!`, so a
# gate written as `! grep -Fq 'contents: write' workflow.yml` under
# `set -euo pipefail` returns 1, carries on, and ends 0 with the forbidden
# thing standing. Thirteen assertions were written that way. This file holds
# the answer for them, in one place so they cannot drift apart again.
#
#     refuse_text <file> <text> <description>
#
# The text is matched literally, anywhere in the file, so a `.` in a version
# number matches a `.` and nothing else and the rule never forbids more than
# the text it names. The whole file is read: a forbidden text standing behind
# a `#` is still forbidden, because a gate forbids a thing being written down
# where a reader will copy it, not a thing being run.
#
# On refusal the message names the text, the file it was found in and what the
# text means, and the caller exits non-zero -- so a gate that writes nothing
# after the call still stops.
#
# `tests/negative-assertion-strength.sh` holds both sides: that no shell gate
# carries a bare `!`-led statement any more, and that every text these gates
# forbid refuses when it is put back.

refuse_text() {
    local file=$1
    local text=$2
    local description=$3

    if grep -Fq -- "$text" "$file"; then
        printf 'forbidden text stands in a file a gate reads: %s\n' "$text" >&2
        printf '  %s holds it, and it is %s\n' "$file" "$description" >&2
        exit 1
    fi
}
