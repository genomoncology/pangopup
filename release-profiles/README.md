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
fallback. This observed immutable contract is the input boundary for the next
remote-sync slice.
