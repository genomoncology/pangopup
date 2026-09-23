# Run macOS spec bounds portably

The public macOS job for `e75a266` failed four spec examples. The two FIFO examples invoked GNU `timeout`, which the runner did not provide, and exited 127 before checking the transport. The two runtime-v2 examples hit mustmatch 0.1.0's 30-second default block limit. The qualification runs nine feature-specific builds and 24 exact test selections. It completed successfully in 65.82 seconds on the macOS development host.

The FIFO spec now invokes a repository-local Python 3 timeout helper. The helper starts the command in its own process group, preserves its ordinary exit status, sends termination and then kill after a deadline, and returns 124 on timeout. The exact-output pin still requires the `PART_SET_INVALID` refusal and exit 1. The static refutation check requires this portable helper for commands in FIFO-building blocks. A focused test proves ordinary exit, deadline, and missing-command statuses.

The runtime-v2 qualification block now carries a 180-second mustmatch limit. Other blocks retain the 30-second default. Its script, exact selection checks, and output pin remain unchanged.

`bash tests/spec-timeout.sh`, `bash tests/spec-refutation-evidence.sh`, `bash tests/spec-block-execution.sh`, and `git diff --check` passed. On macOS, `make lint`, `make test`, and `make spec` passed from the branch based on main `4458eb4`; the spec gate reported 207 passed. The pushed commit's macOS CI remains the final runner confirmation. Ticket 0155 is archived.
