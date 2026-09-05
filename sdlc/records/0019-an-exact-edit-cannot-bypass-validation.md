---
base: 94971caba3c0b85cf848179aedef4116f2426555
head: 62494d23c2ceda55a7f1093b5c09d0d8d3e6864d
---

# An exact edit cannot bypass validation

The public exact-edit value now keeps its representation private and admits values only through the existing fallible insertion and deletion constructors. Conversion uses checked error returns instead of caller-dependent subtraction and assertions. Valid command-line, HTTP, routing, cache, and scoring behavior did not change.

Independent design and code reviews accepted the change without findings. The red compile-fail proof failed before the implementation and passed afterward. Focused engine and CLI tests passed. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 282 passed and 7 skipped.
