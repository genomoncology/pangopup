---
base: ba8b621
head: 2cd6228
---

# The repository records the public v0.4.1 delivery

PangoPup v0.4.1 now has one coherent public executable and container release from commit `ba8b62180ecd5750a575944d2070f83ca585f4ed`. GitHub release `383676522` is immutable and Latest. GHCR aliases `0.4.1`, `v0.4.1`, and `latest` resolve to OCI index `sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8`.

The completed publication record names the authorization receipt, source gates, package and container workflows, admitted artifacts, six executable files, native container leaves, public release, tagged installer, isolated uninstall checks, final OCI index, and host-native container qualification. Current architecture and planning documents now describe the public v0.4.1 state.

The version gate pins the public executable and container identities. It protects the v0.4.1 release-note body by exact SHA-256 and rejects stale v0.4.0 executable, v0.3.0 container, and v0.4.1 preparation claims across current-state documents.

Independent ticket review accepted the scope before implementation. Independent code review found three record gaps. The remediation added the finalize receipt artifact, an exact release-note hash check, and negative checks for stale claims that coexist with correct text. The reviewer accepted the remediated result.

`make lint`, `make test`, and `make spec` passed. The specification suite reported 283 passed and 7 skipped. An immutable-file audit confirmed that earlier publication records, release-note bodies, fixed fixtures, product behavior, versions, scoring identity, and scoring assets did not change.
