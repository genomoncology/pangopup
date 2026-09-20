# The cache durability check does not parse multiple command lists

`tests/spec-download-cache-durability.sh` treats one Make recipe line as one command prefix. It stops at the first command word. A later command in the same shell list can therefore decide the ONNX Runtime download cache without being read.

A fixture shaped like `true; env ORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort" cargo build` is accepted when the same recipe removes `target/spec-cache`. The shell runs the later build with a cache inside the removed directory, but the check classifies only the environment before `true`.

What a successor must prove: supported command-list separators cannot hide a later cache-deciding build. The check must either parse every admitted simple command or reject multi-command recipe lines with a named diagnostic. It must preserve the bounded static grammar and must not execute recipe text.
