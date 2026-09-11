---
---
# The spec download cache lives where cargo clean collects it

Ticket 0092 moved the ONNX Runtime static library the `spec` recipe downloads
out of `target/spec-cache`, which that recipe removes on every run, and into
`target/ort-cache`, which it does not. That closed the defect the ticket named.
It left the 90647244-byte file inside `target/`.

Measured in this checkout on 2026-09-11:

- `target/ort-cache/dfbin/x86_64-unknown-linux-gnu/<digest>/libonnxruntime.a`
  is 90647244 bytes, and `.gitignore` line 1 is `target/`, so it is ignored.
- No `clean` target exists in the `Makefile`, and no script in the repository
  runs `cargo clean`. Nothing in the repository removes the directory.
- `cargo clean` removes the whole of `target/`, including this file. The next
  `make spec` that rebuilds `ort-sys` then fetches those 90647244 bytes again
  over the network. `cargo clean` is a routine command, and a rebuild is what
  an operator runs it for.

The same run also leaves two copies of the library on a machine that runs both
gates. `make lint` and `make test` build `ort-sys` under the cache the operator
resolves, normally `~/.cache/ort.pyke.io`, and `make spec` builds it under
`target/ort-cache`. Measured: 275 MB in the first and 87 MB in the second, with
one digest present in both. They do not churn each other -- `ort-sys` emits no
`cargo:rerun-if-env-changed` for `ORT_CACHE_DIR`, so alternating the two gates
rebuilds nothing, measured as a 13.03-second `make spec` straight after a
`make test`.

A durable location outside `target/` -- an `XDG_CACHE_HOME` subdirectory the
recipe owns rather than one it deletes -- would survive `cargo clean` and hold
one copy. Whether that is worth the second cache location is the decision this
draft asks for; 0092 did not consider it, because the directory it had to leave
was the one the recipe removes each run.
