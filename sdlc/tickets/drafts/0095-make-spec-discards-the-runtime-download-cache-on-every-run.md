---
---
# `make spec` discards the runtime download cache on every run

The `spec` recipe points `XDG_CACHE_HOME` at `target/spec-cache` and removes
that directory first, so the run starts with an empty cache. `ort-sys` builds
with `download-binaries`, and that build script caches the ONNX Runtime static
library under `$XDG_CACHE_HOME/dfbin`. The two together mean the recipe throws
that library away before every run, and any run that rebuilds `ort-sys` fetches
it again over the network.

Measured in this checkout on 2026-09-10. A `make spec` that rebuilt the
registry crates wrote 87 MB to
`target/spec-cache/dfbin/x86_64-unknown-linux-gnu/<digest>/libonnxruntime.a`
and took 157.00 seconds. The next run, with nothing to rebuild, wrote 20480
bytes and took 13.03 seconds. There is no copy of that library in the checkout
or under `CARGO_HOME`, so the 87 MB came from the network.

This is older than ticket 0090. The base commit's recipe already removed
`target/spec-cache` and already pointed `XDG_CACHE_HOME` at it; ticket 0090
pinned `CARGO_HOME` and `RUSTUP_HOME` against the same class of accident for
the toolchain and left this one where it was.

Done, observably:

- A rebuild of `ort-sys` under `make spec` reaches no download, because the
  library cache it reads is not the directory the recipe removes.
- A check proves the recipe does not point a downloaded-artifact cache at a
  directory it deletes.
- `make test`, `make spec` and `make lint` pass.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes, and `make spec` keeps running under a cache home of its own.
