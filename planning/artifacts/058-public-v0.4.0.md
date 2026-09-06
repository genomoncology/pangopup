# PangoPup v0.4.0 publication record

State: **PARTIAL — immutable v0.4.0 executable public; v0.4.0 container aliases absent.**

Ian Maurer authorized the v0.4.0 executable and native container publication on 2026-09-06. The authorization was bound to exact publication commit `ea4438e50762e32f09052b364060c89201ed78bc` before the first remote mutation.

## Public executable outcome

GitHub release ID `383614742` is immutable, Latest, non-draft, and non-prerelease. It has tag and title `v0.4.0` and `PangoPup v0.4.0`. The release and direct `refs/tags/v0.4.0` both resolve to `ea4438e50762e32f09052b364060c89201ed78bc`. Publication completed at `2026-09-06T14:33:59Z`.

The public body is byte-identical to `planning/artifacts/057-release-notes.md`, whose SHA-256 is `729fa6ed9ddb641501f2abdf5e63cd2fd9861154a46f02967bea7ff408ce4aa9`. Anonymous downloads produced this exact six-file inventory:

```text
LICENSE	35149	sha256:3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986
NOTICE	2365	sha256:516f3c44d00eb2840a1a1a7e1127b027bb47190d3fe36a4c918572f39d7ad1c1
pangopup-linux-x86_64	29034216	sha256:6340f95f55b122f4d4f1b914cb59817239664ae36ee9d23924a9db3d8bb929c5
pangopup-linux-x86_64.cdx.json	201980	sha256:6b83db1c68d99354f4ea21ebcfc37debaaeaedcc9389a4461eef9c697939ffeb
pangopup-linux-x86_64.sha256	88	sha256:0fb7f8b3f559e9b516578a1172986effeaae34828ae034b669d96cee2c802a51
release-manifest.json	950	sha256:34e889cbc8427e299e0bc87f0c3c76f3fc8b7e0d88fe7ffeeb8fb1dbc061e521
```

The admitted artifact, private draft, published release, and fresh anonymous downloads matched this inventory. The tagged installer produced `pangopup 0.4.0`. Offline synchronization, status, lookup, model, cache, HTTP, and code-only uninstall checks passed against disposable copies of the retained production profile.

## Container outcome and stop condition

Native container stage run `34039157332` succeeded at the same commit and retained AMD64 leaf `sha256:8350078aebf6542e976ad0219cf130cd6228b13b984867dc9dc36605f50e7d96` and ARM64 leaf `sha256:484b71f80b46f94080a13caa64c44b7a2d99de059a3ba92115cc3dae89e1deec`. Those untagged leaves are abandoned and must not be finalized or reused for another commit.

The required public full-uninstall check failed against the normal read-only managed profile with code `UNINSTALL_IO` and message `remove entry receipt.json: Permission denied (os error 13)`. Publication stopped before container finalization. No v0.4.0 container index was created.

Fresh anonymous registry reads on 2026-09-06 returned canonical `MANIFEST_UNKNOWN` responses for GHCR `0.4.0` and `v0.4.0`. GHCR `latest`, `0.3.0`, and `v0.3.0` remained OCI index `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`.

Ticket 0034 fixed full uninstall on read-only managed profiles. Tickets 0031 through 0033 and 0035 corrected release qualification, finalization recovery, and portable allocation-test behavior. A later release must build and qualify fresh executable and container artifacts from its own exact commit.

This record contains no token, request header, authenticated download URL, or credential path. The v0.4.0 release and its six assets are immutable history and must not be edited or deleted.
