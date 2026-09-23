# Route corpus check counts substitution classes

The existing route-disagreement check now parses every submitted SNV in the committed manifest. It rejects malformed rows, equal alleles, and non-SNV alleles with a substitution-class diagnostic. It derives transition, transversion, and four directed substitution counts from the actual set and compares them with the retained measurement block. The published coverage sentence now states the two class counts and remains bound word for word to both public documents.

The committed set contains 2,615 transversions and zero transitions: A→C 760, C→G 541, G→T 532, and T→A 782. A fixture changes a manifest variant and every matching raw record to A→G. The set-membership check remains satisfied; the new class-count assertion rejects it. Focused fixtures also reject malformed, equal-base, non-SNV, missing-count, and drifted-count cases. The same route check now runs from `make lint` and retains its `make test` invocation.

`make lint`, `make test`, and `make spec` passed on macOS. The spec gate ran 207 blocks. `bash -n` and `git diff --check` passed. Draft 0105 and Ticket 0145 are archived.
