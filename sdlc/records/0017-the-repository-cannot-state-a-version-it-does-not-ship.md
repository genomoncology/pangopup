---
base: 6f1d8b4c9b40b0eba5c918724b33c1cfe74f350b
head: 08bbef176a2469f3b801dae3dfaeed2037f1ee81
---

# The repository prepares one coherent 0.4.0 application candidate

The workspace, all eight PangoPup lockfile packages, executable and service expectations, qualification checks, and container staging workflow now identify the 0.4.0 application candidate. Public installation and citation material still identifies 0.3.0. Historical publication evidence and fixed-version fixtures remain unchanged. Service architecture states the transition, and the active scoring identity changed because application version remains part of its input.

A normal lint gate now parses structured version sources and checks purpose-specific candidate, current-public, historical, and fixed-fixture claims. Code review rejected two incomplete versions of the gate. The final review accepted the claim-specific checks after mutation tests proved that unrelated matching text cannot hide drift. Focused identity, service, executable-delivery, production-qualification, and gate tests passed. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 282 passed and 7 skipped.

No tag, release, image, public 0.4.0 claim, or moving container tag was created.
