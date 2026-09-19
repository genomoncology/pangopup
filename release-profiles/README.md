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

`snv-grch38-v2.json`, its proof under `proofs/`, its named checksum list, and its named release notes are the byte-exact outputs of the retained two-run qualification at tooling and target commit `8eb8917ff9f788c28b29287ed3a06b6b4762c554`. They describe a local publication candidate. No v2 GitHub release exists, and no compiled sync or active installation selects them. The generated authorities therefore land after their target commit. The production v1 files in this directory remain byte-for-byte unchanged.

`runtime-release-profile.json` is the checked outer authority for the public
`runtime-grch38-v1` release and its exact ten-file download set.
`runtime-transport.json` is the checked inner authority for the nine installed
members, including stored and reconstructed identities. The outer authority
pins the inner file's exact byte hash. The shipped runtime-sync library reads
these compiled bytes; it never discovers a latest release or downloads either
authority from an untrusted location.
