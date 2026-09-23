# Public claim checks separate tool purpose from clinical meaning

The README's sentence "The scores help identify variants that may alter RNA splicing" remains allowed. It describes the prediction target. It does not assign clinical meaning. The repository motivation specification now states this boundary directly.

`tests/repository-sourcing.sh` checks both `README.md` and `architecture/motivation.md` for four named meanings: pathogenicity, clinical significance, diagnosis, and stand-alone evidence. It accepts the direct limitation "A score alone does not establish a diagnosis" and rejects its affirmative counterpart on either page. Focused fixtures also accept the splice-target sentence and reject one affirmative claim for each clinical meaning on each page. The check is lexical. It does not claim general prose understanding or extend to other pages. The legal-advice check and public pages did not change.

Draft 0120 and Ticket 0148 are archived. Draft 0118 remains open. The focused `bash tests/repository-sourcing.sh` passed. `make lint`, `make test`, and `make spec` passed on macOS. The specification gate reported 207 passed. The `make test` gate excludes `pangopup-build`; the separate source-fingerprint failure at the starting commit belongs to the release gate repair and is not evidence for this ticket.
