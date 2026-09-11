---
---
# The production qualification port is typed into two files

`scripts/run-production-qualification.sh:123,133` opens and probes
`127.0.0.1:18080`, and `tests/production-release-qualification.sh:198` binds the
same number for its fixture server. Neither reads the port from an environment
variable or picks a free one, so the gate fails with `HTTP service did not
become live` on any machine where something else already holds 18080. Unrelated
work on a developer's machine takes the gate red for a reason that has nothing
to do with the candidate, and the message names no port, so the reader has to
read the script to find out what was in the way.

The fix is to let the port be chosen: bind port 0 and read back what the kernel
gave, or read a variable with 18080 as the default, and name the port in the
failure message either way.
