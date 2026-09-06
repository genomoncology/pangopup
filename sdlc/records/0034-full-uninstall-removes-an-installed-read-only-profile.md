---
base: af1935c
head: cf91bde
---

# Full uninstall removes an installed read-only profile

Full uninstall now widens permissions only through already admitted directory descriptors. Successful removal handles the normal `0555` directory and `0444` file layout. Ordinary traversal failures restore every surviving directory mode before exchanging the detached tree back into its public path.

Permission-restoration failures follow a separate fail-closed path. The public path remains occupied by its blocker, the potentially writable survivor stays under the detached private name for reviewed recovery, and the executable remains installed. Root symlinks remain rejected. Nested symlinks remain safely unlinked without following their targets. Existing ownership, hard-link, mount, replacement, special-entry, nesting, protected-root, and executable-last boundaries remain intact.

The portable suite reproduced the retained permission failure before the fix. It now has 27 passing uninstall tests, including realistic read-only success, normal rollback with exact mode restoration, and root and nested restoration failures. Independent design and code reviews accepted the result. `make lint`, `make test`, and `make spec` passed with 283 specifications passing and 7 retained-asset specifications skipped by design.
