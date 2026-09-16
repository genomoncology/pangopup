# The published-claim evidence check runs on macOS

The shell check now groups digits without a GNU-only `sed` label and writes fixture edits through a portable temporary-file replacement. It still fingerprints copied fixtures before and after each edit, so an edit that changes nothing cannot prove a negative case. The independent code reviewer accepted the one-file diff.

On macOS, `bash tests/published-claim-evidence.sh` exited 0 and reported all three claims, including the index-size arithmetic across 1,366,418,555 loci and 30 `REF=N` exceptions. After the change, `make lint` exited 0. `make test` passed this check and stopped next at `tests/qualification-runner-cache-isolation.sh:118` because Mac Bash lacks `mapfile`. `make spec` reported 186 passed and the same three separate portability failures in record 0121. These results do not make the Mac gate green.

Exact pushed-SHA Linux tests remain pending before the ticket is complete.
