# The model-kernel specification is architecture portable

The model qualification specification now admits the runtime's exact
`x86_64` and `aarch64` architecture strings. A small byte-preserving filter
reads the raw value, refuses a missing value and every value outside that
pair, and only then replaces the architecture, CPU, and Rust compiler strings
with stable placeholders. The expected qualification record still compares
every other byte exactly.

On macOS 26.4 ARM64, the unchanged model qualifier reported `aarch64` and the
focused `spec/model-kernel.md` run passed 20 blocks. Its missing-architecture
and `arm64` controls each exited 1 with the exact refusal. `make lint` and
`make test` exited 0. Full `make spec` exited 0 with 193 passing blocks.
`git diff --check` passed.

A disposable `linux/amd64` Debian Trixie container built the current checkout
with Rust 1.93.1, ran the real model qualifier, and observed raw architecture
`x86_64`. The filter's output matched the complete pinned qualification record
byte for byte. The container ran with `--rm` and retained no project data or
service.
