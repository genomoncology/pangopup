---
base: 40b57df080bd591562563a6729fb03f81e37a329
head: bff2f2505511d77d9eed6ead0fde61f086afcccb
---

# Strict JSON consumers adopt stable_gene before deployment

Public compatibility guidance now states that `stable_gene` adds a property to command-line JSONL and HTTP score records. Permissive readers remain compatible. Strict readers must accept the field before a PangoPup revision that emits it is deployed. The v0.4.0 candidate release notes repeat that consumer-first order.

The normal version-consistency gate protects the shape warning, deployment order, and `gene` and `stable_gene` meanings. Python 3.9 mutation tests prove that removal of either critical warning fails the gate. No product behavior changed.

Ticket 0025 did not verify its named `GRCh38:chr7:140753336:A:T` production example against production assets. The reviewed evidence contained no supporting downstream comment about silent drops. Neither premise supports this remediation. The additive JSON shape and strict-reader behavior provide the evidence for the compatibility policy. The archived 0025 ticket and record remain unchanged.

Independent code review accepted the implementation. The version gate, Python 3.9 compatibility and mutation harness, formatting, and `git diff --check` passed.
