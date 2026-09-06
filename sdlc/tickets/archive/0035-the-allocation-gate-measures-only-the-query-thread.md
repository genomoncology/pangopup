---
flow: build
priority: 10
---
# The allocation gate measures only the query thread

Linux CI run `34042021321` failed the warmed mask-query allocation gate with four allocations between the measured counter reads. The same test then passed ten consecutive local runs and failed once during one hundred additional isolated runs with the same four-allocation difference. The gate uses one process-global allocation counter, so unrelated Rust test-harness work on another thread can change the measurement. The failed test lives in a separate `pangopup-index` test executable and does not link the uninstall code changed immediately before the failure.

The gate must count allocations made by the measured query thread while measurement is explicitly active. It must keep the exact zero-allocation requirement for ten thousand warmed queries. The test must also prove that its counter detects an intentional allocation on the measured thread. Work outside the measured thread must not change the result.

Done, observably:

- The former process-wide measurement can no longer fail because another thread allocates between its counter reads.
- Ten thousand sufficiently reserved warmed mask queries still require exactly zero allocations.
- A negative control enables measurement, performs an intentional allocation on the measured thread, and observes a nonzero count.
- Repeated isolated runs pass without weakening the zero-allocation assertion.
- The normal lint, test, and executable specification gates pass.

Boundary: Change only test measurement machinery. Do not permit any measured query allocation, add an allocation tolerance, change mask behavior, change production allocation behavior, or publish or alter a release, tag, executable asset, container alias, staged leaf, or scoring asset.
