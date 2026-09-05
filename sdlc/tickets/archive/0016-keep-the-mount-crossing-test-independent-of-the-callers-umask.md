---
flow: build
priority: 10
---
# The mount-crossing test is independent of the caller's umask

The local asset suite assumes its temporary root has the repository's required mode. The temporary root inherits the caller's umask and can therefore have a mode other than the required `0700`, so the shared root admission rejects the fixture before the test reaches the mount-crossing behavior it claims to prove.

The test must establish its own admitted root and still prove that a child entry from another device is rejected. Production asset behavior does not change.

Done, observably:

- The mount-crossing test reaches the cross-device rejection under caller umask `002` and `077`.
- The complete asset test suite passes under both umasks.
- The repository lint, test, and specification gates remain green.

Boundary: change test setup only. Do not weaken root admission, change production filesystem behavior, or broaden accepted modes.
