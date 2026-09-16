# The qualification cache-isolation check runs on macOS Bash

The check reads its three fixture paths with Bash 3.2-compatible built-ins. Each relative-path refusal now checks all three original absent absolute candidates and all relative working-tree paths. A planted-path regression proves the absence assertion catches the candidate that the relative argument replaces. The independent code reviewer accepted the final one-file diff.

On macOS, `/bin/bash tests/qualification-runner-cache-isolation.sh` exited 0 and reported eight checks across one recorded spawn. The same check exited 0 in a network-disabled GNU/Linux container with the checkout read-only and executable temporary storage. The test stub and its output stayed under temporary storage; no container was retained.

After the change, `make lint` exited 0. Mac `make test` passed this check and stopped later in `tests/repository-sourcing.sh` because BSD `sed` rejected a fixture edit. Mac `make spec` reported 186 passed and three separate portability failures, including repository sourcing. These are later tickets. This record does not claim green Mac or remote release gates.
