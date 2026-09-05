---
base: 509fd799d8003a2be682bcaa3aec17ad12226ec2
head: d8908f256e49d3cc829ed1ead1065aa9fbc30595
---

# Keep the mount-crossing test independent of the caller's umask

The mount-crossing test now creates its own admitted root with the required mode before it checks a child entry from another device. Production filesystem behavior did not change.

Independent design and code reviews accepted the change without findings. The focused test and the complete 120-test asset suite passed under caller umasks `002` and `077`. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 276 passed and 7 skipped.
