# PangoPup v0.4.1 publication record

State: **COMPLETE — immutable v0.4.1 executable and native container are public and qualified.**

Ian Maurer authorized the v0.4.1 Linux x86-64 executable release and native Linux AMD64/ARM64 container index on 2026-09-06. The out-of-commit authorization receipt bound that scope to final `origin/main` commit `ba8b62180ecd5750a575944d2070f83ca585f4ed` before the first remote mutation. The retained receipt has SHA-256 `1349dfa3968706ae446ebe23ed5c38f2b44c430b999664a793d1b1658a2c58c7`. The UTC publication date matched `CITATION.cff`.

## Commit-bound qualification

The clean publication checkout, expected repository origin, absent Git replacement refs, and exact `origin/main` commit were authenticated. Local `make lint`, `make test`, and `make spec` passed. The repository remained public with read-only default Actions permissions, disabled Actions pull-request approval, enabled Dependabot security updates, enabled secret scanning and push protection, no open secret alerts, and the expected active main rulesets.

Push-triggered CI run `34050860424` and native-container run `34050860367` succeeded for the publication commit before staging. The container run passed native AMD64 and ARM64 smoke jobs.

Container stage run `34051202792` succeeded for the same commit. Receipt artifact ID `9994644140`, named `pangopup-container-stage-ba8b62180ecd5750a575944d2070f83ca585f4ed-34051202792`, had API and downloaded-archive SHA-256 `06d7334e32b3fbe5f0782517bf922316a7a66bf7abe39d5020c3a766287c13a8`. Its canonical receipt bound AMD64 leaf `sha256:6da7aa07432fe5d24166ce0157bad266ccee42804b8600fb4501de7f4c9b4852` and ARM64 leaf `sha256:4d6a025f65416289b5fce865fb369d401161bd4e4f02b7abf9b16eaeb04afb91` to that run, workflow commit, and publication commit. Both leaves passed native qualification while every v0.4.1 public container alias remained absent.

Executable package run `34051204175` succeeded for the publication commit. Artifact ID `9994637970`, named `pangopup-linux-ba8b62180ecd5750a575944d2070f83ca585f4ed`, had API and downloaded-archive SHA-256 `e3328a12b279dbf8662805f33ff3e813b6c69ac41fbf0fccff0615bf14831bdc`. Qualification admitted exactly these six public files:

```text
LICENSE	35149	sha256:3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986
NOTICE	2365	sha256:516f3c44d00eb2840a1a1a7e1127b027bb47190d3fe36a4c918572f39d7ad1c1
pangopup-linux-x86_64	29035720	sha256:06ad5c0e3aff8f030116262045cede3bc97cdfc716b3e72029f02b6d58d6f822
pangopup-linux-x86_64.cdx.json	201980	sha256:36dc596676a6fd0848d0bb7cbca6d31e07d3cc892ce8f13a68a1317273b8c716
pangopup-linux-x86_64.sha256	88	sha256:53e339cfdf6c18c08c3ae1216da7e4e8378bf8680df06d05f70c87cd2b4b8ece
release-manifest.json	950	sha256:17656b9613d3f52c179e80afd57f40f1af2e769ed032ea6b60b0f43a1517ad05
```

The manifest version and target commit, executable checksum and version, SBOM, notices, dynamic-library allowlist, and maximum GLIBC 2.38 check passed.

The private draft target, title, tag, non-prerelease state, release-note body, and six-member inventory matched the admitted inputs immediately before publication. GitHub Latest remained immutable v0.4.0 at its pinned commit, GHCR `latest` remained the pinned v0.3.0 predecessor, the v0.4.1 direct tag and release were absent outside the draft, the v0.4.1 container tags were absent, and the UTC date still matched `CITATION.cff`.

## Public executable

GitHub release ID `383676522` is immutable, Latest, non-draft, and non-prerelease with tag `v0.4.1`, title `PangoPup v0.4.1`, target commit `ba8b62180ecd5750a575944d2070f83ca585f4ed`, and publication time `2026-09-06T18:23:57Z`. The direct `refs/tags/v0.4.1` ref resolves to the same commit. The public body is byte-identical to `planning/artifacts/059-release-notes.md`, whose SHA-256 is `a2e481810f3e9095c5a06437fc47b96162c79f6c66147d08b3c0f2711e5e1abe`. Fresh anonymous downloads matched every admitted name, size, and SHA-256 above.

The tagged installer produced `pangopup 0.4.1` with clean stderr. Offline asset reuse, status, representative lookup, model, cache, HTTP, and release checks passed against disposable retained assets. Code-only uninstall removed the executable and preserved data and cache. Full uninstall removed the executable, normal read-only managed profile, and cache. Neither check touched retained original assets.

## Public container

Finalize run `34051969820` succeeded. Finalize receipt artifact ID `9994810489`, named `pangopup-container-finalize-receipt-ba8b62180ecd5750a575944d2070f83ca585f4ed-34051969820`, had size 371 bytes and API and downloaded-archive SHA-256 `8f4826bdc99a300984d12b24254ef7d50c17d7de7fac143391700b0c6633f51e`. The run re-admitted stage run `34051202792` and its retained receipt, requalified both exact native leaves, authenticated the immutable public v0.4.1 release and direct tag at the staged publication commit, and preserved the v0.3.0 predecessor until index creation.

Fresh anonymous reads show `0.4.1`, `v0.4.1`, and `latest` resolving to OCI index `sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8`. The index contains exactly the held Linux AMD64 and ARM64 leaf digests above and reports source `https://github.com/genomoncology/pangopup`, revision `ba8b62180ecd5750a575944d2070f83ca585f4ed`, version `0.4.1`, and license `GPL-3.0-only`. Host qualification of the public AMD64 image reported `container qualified architecture=amd64 image_size=56412629`; the retained output has SHA-256 `4e55aae769ba6d8e7e73a6c5d75a13d79a589b5c355861c7cb4906fd76a857a2`.

The executable and container now form one coherent public v0.4.1 release. The v0.4.0 executable-only record, v0.3.0 publication record, earlier release bodies, fixed-version fixtures, scoring asset releases, and abandoned v0.4.0 leaves remain unchanged.

This record contains no token, request header, authenticated download URL, or credential path. The public release is immutable. The versioned container aliases are historical deployment names and must not be deleted or retagged.
