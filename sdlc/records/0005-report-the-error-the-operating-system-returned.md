---
base: 806b95080614ce8f7b39bfe2c0fd71f8163bea03
head: c0f774e0bea09a9f30e40131d1531be7a2f8179e
---

# Report the operating system error from local asset failures

Every local asset translation now reports the `io::Error` returned by the failing operation. Raw system-call wrappers capture `errno` immediately. Error kinds, exit behavior, semantic failures, and failure timing remain unchanged.

The compiler-enforced helper signature and a source guard prevent callers from silently discarding a caught error. Deterministic tests distinguish a non-directory parent from a symlink loop. Independent design and code reviews accepted the result without findings. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 277 passed and 7 skipped.
