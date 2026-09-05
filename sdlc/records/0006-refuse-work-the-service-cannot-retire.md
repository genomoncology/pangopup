---
base: 14d2fcd64577337e28465a4e7b4ea92607c22e9c
head: 0791b1d4572357651fc67c02e241909bd72ac3fa
---

# Bound admitted model work by variant

The service now bounds all running and waiting uncached model work in variant units. The default capacity is 20 units. Whole requests receive the existing 429 response immediately when their admission would exceed the bound. Index hits and completed SQLite cache hits stay outside admission. Status output names the work unit and reports running, queued, and capacity values in the same units. Documentation records the measured basis for the default and describes the resulting wait as a planning estimate.

Code review found that a contended cache gate could cause completed SQLite hits to consume capacity and receive 429. Remediation now waits for the cache gate asynchronously and runs the cache read outside the async runtime thread. The implementation also pins the exact saturation response in a test. The same reviewer accepted the correction. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 275 passed and 7 skipped.
