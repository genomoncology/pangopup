# The build-directory residue check runs on macOS and Linux

The check now uses `find` forms accepted by BSD and GNU implementations. It still counts every directory under `target/`, detects the two directories left by its disposable leaving harness, accepts the restoring harness, and refuses owner-unwritable residue without running `cargo clean` on the build tree.

Observed focused result: `bash tests/build-directory-residue.sh` exited 0 on macOS and reported 1,109 build directories without residue. A Debian Bookworm container ran `docker run --rm --read-only --user 501:20 --tmpfs /tmp:rw,mode=1777 -v "$PWD:/src:ro" -w /src debian:bookworm-slim bash tests/build-directory-residue.sh` from this worktree. It exited 0 and reported the same 1,109 directories.

Observed gates on macOS after the change: `make lint` exited 0. `make test` passed this check and several following checks, then stopped in `tests/inherited-cache-variables.sh` at `chmod: --: No such file or directory`. `make spec` reported 186 passed and three existing Mac portability failures: the container-image block, the x86-64 model-kernel expectation on Apple Silicon, and GNU `sed -i` in repository-sourcing qualification. These failures remain separate work. This record does not claim green Mac gates.
