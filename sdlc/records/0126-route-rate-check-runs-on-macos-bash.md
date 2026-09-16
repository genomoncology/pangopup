# The route-disagreement evidence check runs on macOS and Linux

The check now stores its fixed measurement fields and recomputed counts in Bash 3.2 indexed arrays. Copied fixture edits use portable temporary-file replacements, and portable checksums prove that every negative fixture changed its input. A hostile copied count containing shell syntax is refused as a malformed count and cannot create its marker file. The recorded scores, populations, percentages, and published claims did not change.

`/bin/bash tests/route-disagreement-rate.sh` exited 0 on macOS and in a read-only GNU/Linux container. Both reported 2 value disagreements, 17 position disagreements, and 379 position-comparable records. The independent code reviewer also confirmed the real 2,615-variant and 2,790-record evidence remains intact. Bash syntax validation and `git diff --check` passed.

Mac `make lint` and `make test` exited 0. `make spec` reported 187 passed and two separate Mac portability failures. `spec/container-image.md` uses a Linux path for `true`, and `spec/model-kernel.md` expects `x86_64` on an Apple Silicon host. Those two failures remain separate work.
