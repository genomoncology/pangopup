# The repository-sourcing check runs on macOS and Linux

Eight fixture edits now use portable checked replacements. The all-dates cases still alter every capture date, and the separate service-only case alters one. A missing match fails the edit; the existing fixture fingerprint rejects a changed-nothing case. The independent code reviewer accepted the restored negative-test coverage. Published source, licence, and explanation claims did not change.

`bash tests/repository-sourcing.sh` exited 0 on macOS and in a network-disabled GNU/Linux container with a read-only checkout. The related `spec/repository-motivation.md` passed both blocks on Mac. No container was retained.

After the change, Mac `make lint` exited 0. `make test` passed this check and stopped later in `tests/route-disagreement-rate.sh` because Bash 3.2 rejects `declare -A`. `make spec` reported 187 passed and two separate failures: the container-image control and a model-kernel x86-64 expectation on Apple Silicon. Broad Mac and remote release gates remain open.
