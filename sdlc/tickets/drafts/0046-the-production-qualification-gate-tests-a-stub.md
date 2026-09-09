---
---
# The production qualification gate tests a stub

`tests/production-release-qualification.sh` exercises `scripts/check-production-qualification.py` against a stubbed binary, so the local gate reports the checker's behavior and never the release's. The real qualification runs the checker against the built executable.

Ticket 0045 found the gap by falling into it. The checker compared release SNV output, two model oracles and HTTP responses byte for byte against fixtures carrying no gene names. v0.5.0 names every record, so the real production qualification would have failed on all four comparisons while `make test` stayed green. The code review repaired the checker and taught the stub to name, then added three negative cases proving the checker now rejects unnamed output.

The repair closes this instance. The shape stays. Any future change to what the executable emits can pass the stub and fail the release, and the failure appears at qualification time rather than at the gate that exists to prevent it.

`scripts/run-linux-tests-with-public-failure.sh` hardcodes `file=Makefile,line=30` for the `test:` target, and ticket 0045 added a mechanical check for that line. The same class of hidden coupling is worth a sweep.
