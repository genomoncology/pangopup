---
---
# A cache path spelled with a dot or a parent segment is read as somewhere else

`tests/spec-download-cache-durability.sh` decides whether a downloaded-library
cache stands inside a directory a recipe removes by comparing the two paths as
text:

    inside() {
        [[ "$1" == "$2" || "$1" == "$2"/* ]]
    }

It never normalises either side. Measured on 2026-09-11 against a tree rooted
at `$root`:

- `ORT_CACHE_DIR="$(CURDIR)/./target/ort-cache"` resolves to
  `$root/./target/ort-cache`, which `inside` reads as outside `$root/target`.
  The cache is in fact inside the build directory and a routine `cargo clean`
  collects it.
- `ORT_CACHE_DIR="$(CURDIR)/target/../ort-cache"` resolves to
  `$root/target/../ort-cache`, which `inside` reads as inside `$root/target`.
  The cache is in fact a sibling of the build directory and nothing collects
  it.

Both directions are wrong, and the second is the one that refuses a recipe that
is fine. `removed_directories` has the same gap on the other operand: a recipe
spelling `rm -rf ./target/spec-cache` names a directory no comparison here
matches.

Nothing in the `Makefile` spells a path either way today. The exposure arrives
the moment someone writes one, and the gate answers with confidence either way.

What a successor must prove: a cache named through a `.` or a `..` segment is
classified by where it actually lands, in both directions, and a recipe whose
removal is spelled the same way is still read as removing that directory.
