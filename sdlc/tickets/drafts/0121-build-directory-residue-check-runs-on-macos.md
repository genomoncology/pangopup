---
flow: build
priority: 5
---
# The build-directory residue check runs on macOS and Linux

On macOS, `make test` stops at `tests/build-directory-residue.sh` because BSD `find` rejects `-writable`. The same check also uses GNU `find -printf`. This is the first observed Mac test failure at the current `0.5.0` source commit. `make lint` passes.

The check must identify build directories without owner-write permission on both supported operating systems. Its disposable harness must still prove that it detects a harness that leaves two such directories and accepts one that restores them. The real build-tree scan must still name residue relative to `target/`, refuse an empty or nearly empty build tree, and never run `cargo clean` on the real tree.

Done, observably:

- The focused shell check passes on macOS and Linux after a build.
- The leaving-harness fixture still produces two detected residue paths, including the deeper bundle path. The restoring-harness fixture produces none. The focused check passes when both assertions hold.
- `make test` on macOS advances beyond this shell check without a `find` syntax error.
- `make lint`, `make test`, and `make spec` run before the change is committed. Report unrelated baseline gate failures rather than claiming they passed.

Boundary: keep installed-bundle permissions and the cleanup duty unchanged. Do not alter `cargo clean`, other qualification checks, or the separate macOS spec failures.

Documentation: record the observed Mac and Linux result in `sdlc/records/` when complete. No public score or operator instruction changes.
