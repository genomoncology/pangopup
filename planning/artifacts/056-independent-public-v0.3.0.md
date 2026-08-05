# Independent public v0.3.0 qualification

Verdict: **PASS**. An anonymous user can obtain the exact v0.3.0 executable
and native container, install the executable without root, reuse compatible
assets offline, reproduce the retained lookup/model/HTTP oracles, persist and
reuse one modeled result, and remove only the paths selected by the uninstall
scope.

This audit used an anonymous clean checkout of tag `v0.3.0`, not Ticket 055's
checkout or summary, as its oracle. The tag resolved to exact commit
`3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`; the checkout was clean and had no
replacement objects. Evidence is retained outside git under
`/home/ian/workspace/data/pangopup-release-056-independent-3a857f7/`.

## Public executable

Anonymous GitHub API reads proved release `v0.3.0` is immutable, public,
non-draft, non-prerelease, and Latest, targets the exact commit, and has the
byte-identical reviewed release body. Its exact six members were independently
downloaded and matched their API sizes and SHA-256 digests:

| Member | Bytes | SHA-256 |
|---|---:|---|
| `LICENSE` | 35,149 | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |
| `NOTICE` | 1,899 | `19d4942d45f87794e304cf8a3d72a7c7a685fb4641a772f9a35acf8b701754c7` |
| `pangopup-linux-x86_64` | 29,069,480 | `cd5a451190c35af1fe5dd481abf64d06f430c4154db94068fc889131ccaa3578` |
| `pangopup-linux-x86_64.cdx.json` | 201,980 | `6b63fa3761f4bd81b3cd949f3fb68a06caf4962d9e6490b2cfda1ac1d62bc1da` |
| `pangopup-linux-x86_64.sha256` | 88 | `976d6b12925f11ce021a44f9a9480684db71c0db0f7df7187ed69fc0212384fe` |
| `release-manifest.json` | 950 | `5b5df42e65d6f35d37147e59e40ef7745212e94e0e1655677dff2b3a26d34d59` |

The tagged qualifier admitted the checksum, manifest, SBOM, notices, version,
commit, and GLIBC boundary. In a fresh Ubuntu 24.04 container, UID 12345 ran
the pinned installer. Both version forms and focused runtime help passed with
empty application stderr, and the installed executable's digest equaled the
admitted public executable.

## Public container

Fresh anonymous GHCR reads proved `0.3.0`, `v0.3.0`, and `latest` all resolve
to OCI index
`sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`.
It contains exactly these native Linux children:

- AMD64: `sha256:cc85a70eb6549e35a3641070217c9252759f2b9e22ddfc7ef83605ca54470aba`
- ARM64: `sha256:d73958dbd3dc7c2252b01b3354968f8d24f9585e7e5b4aa6d76f8b1e5bc5b1c0`

The index annotations identify the public source, version `0.3.0`, exact
publication commit, and `GPL-3.0-only`. The tag's full container qualifier
passed on exact reference
`ghcr.io/genomoncology/pangopup@sha256:cc85a70eb6549e35a3641070217c9252759f2b9e22ddfc7ef83605ca54470aba`
(`56,445,763` image bytes). Retained argv and pre/post `docker image inspect`
records prove the qualifier used that literal digest reference, its
`RepoDigests` contained that exact leaf, and its local image ID remained
`sha256:41e38695f1ce770cbd09bc9bdcae69a7c9be3dbdea6bc348da850b9b74b7fc5e`
across the run. This one Linux host did not execute the ARM64 leaf; the
release's separate native GitHub ARM64 qualification remains supporting rather
than independent local evidence.

## Offline behavior and safe asset handling

Before testing, every regular file in Ticket 055's preserved data and cache
roots received a path-sensitive SHA-256 inventory. Reflinks were unsupported,
so the audit made real copies: 15,845,837,837 bytes of installed data and
2,623,585,318 bytes of cache-tree content including filesystem overhead. All
asset-dependent host scoring and offline-sync commands used only those copies.
Complete after-test inventories of the preserved roots were byte-identical to
the before inventories.

Using the public executable and copied assets, the tagged production harness
passed:

- offline reuse and ready status;
- all seven retained groups totaling 1,000 SNVs, with actual and expected
  canonical digest
  `06a2c7e166f29f8e8eb4c1106e92a67dc4b2f6a6955f45f491b76a6156ca9129`;
- automatic non-SNV model fallback and forced-model SNV exact oracles;
- focused help; and
- HTTP liveness, readiness, status, SNV, automatic-model, and forced-model
  responses.

A separate empty SQLite path proved actual persistence rather than merely
repeating inference. The first process took 8.61 seconds, created one
authenticated `entries` row, and returned the retained exact M09 output. A
second process returned byte-identical output in 0.01 seconds. The primary
database SHA-256 remained
`7c746cc99466882f3af77fe689924f8745e2c7ad852a11c149c32decf1189535`;
read-only inspection created a zero-length WAL and a nonempty SHM sidecar,
both of which disappeared after the connection closed. Those transient
sidecars were not claimed as unchanged durable database bytes. These times
distinguish the paths but are not product benchmarks.

## Uninstall boundaries

Two new isolated trees held copies of the public executable and empty managed
data/cache roots. Each executable parent, data parent, and cache parent had an
outside sentinel.

- `uninstall --yes` displayed all three resolved paths, removed only the
  executable, preserved both managed roots, and preserved all sentinels.
- `uninstall --full --yes` displayed all paths, prompted for nothing, removed
  the executable and the two managed roots, and preserved all sentinels.

No preserved qualification root was an uninstall target.

## Failures kept separate

No product scoring, packaging, installation, service, or uninstall failure was
found. The retained evidence also records bounded test-environment issues:

- this filesystem rejected reflinks, so the required real-copy fallback was
  used;
- the host lacked the `sqlite3` CLI, so Python's standard `sqlite3` module was
  run through `uv` for the read-only integrity/row-count query;
- the container qualifier's optional held-digest assertion assumes exactly one
  local `RepoDigests` value. It rejected a Docker daemon that listed both the
  pulled index and leaf, even though the exact leaf was present. The audit did
  not weaken the identity claim: anonymous registry bytes authenticated the
  leaf, and a fresh full qualifier run retained literal argv plus pre/post
  inspect evidence binding that exact digest reference to one unchanged local
  image ID. The strict single-entry assertion itself was omitted from that
  rerun because its stated local-daemon precondition was false; and
- two preliminary uninstall fixtures were rejected or interrupted before the
  accepted isolated run: one deliberately unknown in-root marker correctly
  triggered `UNINSTALL_UNSAFE`, and one shell loop accidentally overwrote
  zsh's special `path` variable. Neither touched preserved data.

The audit performed no network synchronization, biological-asset rebuild or
certification, upload, release edit, retag, package-setting change, or
biological-asset mutation. Its only sync commands were explicit offline reuse
checks against the disposable copies; it compiled the tagged maintainer helper
needed by the container qualifier.
