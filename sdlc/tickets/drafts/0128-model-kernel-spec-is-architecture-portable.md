---
flow: build
priority: 5
---
# The model-kernel specification passes on supported CPU architectures

Mac `make spec` now reports 188 passing blocks and one failure. The model qualification output correctly names the Apple Silicon host as `aarch64`, while `spec/model-kernel.md` pins `x86_64` even though the surrounding contract says host strings are normalized. The portable specification must accept the supported runtime architectures, refuse an unknown architecture, and compare the rest of the qualification record exactly.

Done, observably:

- The qualification block succeeds with the real `x86_64` Linux output and the real `aarch64` macOS output. Those two strings are the exact accepted set.
- The block validates the raw architecture against `{x86_64, aarch64}` before replacing it with a stable placeholder. Negative cases prove a missing value and every value outside that pair fail rather than disappearing through normalization.
- Bundle identity, score oracle, runtime policy, provider, thread counts, versions, comparison counts, and every other field remain exact.
- Mac `make lint`, `make test`, and `make spec` exit 0. The focused specification also passes on Linux.

Boundary: do not change model inference, runtime reporting, the admitted architecture set, model assets, or score precision. Record Mac and Linux evidence in `sdlc/records/`.
