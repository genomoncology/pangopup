---
base: 79968d7ce6fcde2b1a8f804e62f288ca943c10d4
head: ce22185941313bb152a6b9c32deb72bcaa216626
---

# Accept reported gene identifiers as filters

CLI and HTTP gene filters now accept stable Ensembl identifiers and the versioned GENCODE forms that PangoPup reports, including `_PAR_Y`. Both adapters normalize accepted input to the stable identity used by matching. Reported identifiers, output bytes, and existing stable-filter behavior remain unchanged. Malformed input receives an accurate description of every accepted form.

Design review found that the original ticket omitted the HTTP filter and `_PAR_Y` output. Code review then rejected misleading stable-only error messages after the parser was widened. The implementation now uses one shared adapter parser and truthful public errors. The same reviewer accepted the correction. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 280 passed and 7 skipped.
