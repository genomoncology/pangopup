---
base: 49bc05e
head: 3df0e80
---

# The allocation gate measures only the query thread

The mask allocation gate now enables a thread-local counter only around the measured operation. Allocations from Rust's test harness or another worker thread cannot change the result. The warmed query still runs ten thousand times and requires exactly zero allocations.

One control allocates on the measured thread and proves that the counter detects it. A barrier-controlled worker allocates while measurement is active and proves that unrelated thread activity stays excluded. The original flaky test reproduced once in one hundred isolated runs before the fix with the same four-allocation difference reported by Linux CI run `34042021321`.

Independent design and code reviews accepted the result. The focused test binary passed 100 repeated original-test runs during implementation and 20 complete three-test runs during review. `make lint`, `make test`, and `make spec` passed with 283 specifications passing and 7 retained-asset specifications skipped by design.
