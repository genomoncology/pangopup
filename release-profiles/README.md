# Release profiles

This directory contains reviewed, immutable remote identity contracts. A
profile is canonical RFC 8785 JSON with no trailing newline. It pins one
repository, tag, target commit, source attribution, reference-compatibility
evidence, bundle/transport identities, exact asset names/sizes/digests, and
literal HTTPS URLs.

`snv-grch38-v1.json` is the checked publication profile. Its proof receipt is
under `proofs/`; that exceptional retained format is exactly 2,193 bytes of
canonical JSON followed by one required LF, and its whole 2,194-byte identity
is pinned by the profile. The release preparer validates both framing and
content before copying the bytes.

Profiles are data contracts; remote existence is separately observed. The
`snv-grch38-v1` contract is now published at its pinned URL with exactly eight
matching assets and `immutable=true`. `pangopup-build release prepare`
reproduces the small publication outputs from bounded local metadata without
opening payload parts or contacting GitHub. A mutable release is not a
fallback. This observed immutable contract is the source of truth consumed by
the shipped pinned remote-sync implementation.

Sparse v2 tooling generates local candidate authorities only. `sparse-release assemble` accepts the exact checked v1 bundle as its sole corpus authority and creates a certified three-file sparse bundle from an already-built member. The resulting manifest keeps the original v1 builder under sparse provenance and records the current assembler version at the top level. `sparse-release prepare` verifies that bundle through its complete transport and writes a canonical proof, profile, checksum list, and release notes. The checked v1 bundle manifest beside the v1 proof supplies the complete immutable authority facts used by preparation. Preparation accepts only a tooling commit that equals the executable's compiled commit from a clean checkout; the separately supplied release target may differ.

`snv-grch38-v2.json`, its proof under `proofs/`, its named checksum list, and its named release notes are the byte-exact outputs of the retained two-run qualification at tooling and target commit `8eb8917ff9f788c28b29287ed3a06b6b4762c554`. Production code authenticates the 4,755-byte profile and 5,405-byte proof through independent SHA-256 pins and closed version-specific parsers. One internal accessor returns the qualified sparse SNV descriptor. The ordinary production selector still returns v1. No v2 GitHub release exists, and no compiled sync or active installation selects v2. The generated authorities therefore land after their target commit. The production v1 files in this directory remain byte-for-byte unchanged.

The sparse proof repeats the source, reference, counts, and logical corpus facts and checks them against the pinned v1 bundle manifest. The proof omits attribution and the original authority-builder object. The pinned v1 manifest supplies those immutable facts. The exact sparse bundle identity covers the sparse manifest and its full provenance, including the original authority builder. The checked v2 profile and proof cross-link that bundle identity with the exact transport, members, part order, provenance, and sizing sums.

`runtime-release-profile.json` is the checked outer authority for the public
`runtime-grch38-v1` release and its exact ten-file download set.
`runtime-transport.json` is the checked inner authority for the nine installed
members, including stored and reconstructed identities. The outer authority
pins the inner file's exact byte hash. The shipped runtime-sync library reads
these compiled bytes; it never discovers a latest release or downloads either
authority from an untrusted location.

`pangopup-build runtime-release prepare-v2` is the bounded inactive outer-release preparer for the qualified sparse authority. It derives the exact sparse-bound inner profile, requires all eight model/reference/mask transport members to remain byte-identical to v1, and permits only that profile and the enclosing transport manifest to change. The preparer records the original converter commit, the clean compiled tooling commit, and the independently supplied release target in separate fields. Its `SOURCE-SUPPLEMENT.json` names the three previously verified preferred-source members by role, size, digest, and source commit. It contains no URL verification claim. The command prepares local bytes only. Production selection, sync, admission, installation, publication, and activation remain v1-only.

Before activation, run `tests/runtime-v2-qualification.sh` against the exact pushed tooling and retained assets. The gate counts 1,000 requests and seven nonempty command groups, exercises real fixed and sparse providers, then coordinates reachable model/service routing and separately labeled serializer shapes. A later ticket must also qualify upgrade and rollback before changing compiled authorities.

Retained runtime-v2 construction uses the feature-gated `pangopup-runtime-v2-qualify prepare` binary. The command authenticates the exact checked sparse authority and unchanged v1 model-side transport members before it publishes a candidate transport. The default builder and shipped executable do not contain this route.
