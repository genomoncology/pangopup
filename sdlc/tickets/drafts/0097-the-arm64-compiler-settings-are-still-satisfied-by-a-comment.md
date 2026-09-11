---
---
# The ARM64 compiler settings are still satisfied by a comment

Ticket 0053 named six lines of `tests/ci-platform-support.sh` that prove a workflow step exists by searching for its text. Five now go through `require_workflow_command`, which refuses a match that stands behind a `#`. Two do not: the `env:` keys of the ARM64 cross-compile step in `.github/workflows/ci.yml`.

```
require_text 'CC_aarch64_unknown_linux_gnu: aarch64-linux-gnu-gcc' "$arm_step" 'bundled C dependency compiler'
require_text 'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc' "$arm_step" 'Rust target linker'
```

`require_text` is a `case` substring test over a joined slice, so commenting out either line in the workflow leaves the gate green. Measured on 2026-09-10.

Ticket 0053's observable-done list speaks of commands, and an `env:` key is configuration rather than a command, so the code stage left these two on the older check and said so. The exposure is smaller than the one 0053 closed: `cargo check` does not link, and the `cc` crate derives `aarch64-linux-gnu-gcc` from the target on its own, so removing the two keys most likely leaves the cross-compile doing its work or failing loudly rather than passing on nothing. What is left is a gate that still reads a comment as proof.

Closing it means either extending `require_workflow_command` to configuration text, which would make its refusal message ("workflow command does not stand in code") read wrongly, or giving `tests/support/workflow-commands.sh` a second entry point for a setting rather than a command. The choice belongs in a ticket of its own.

Found during code review of ticket 0053.
