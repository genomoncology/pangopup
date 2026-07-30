# ADR 0023: Derived runtime transport

Status: accepted

Pangopup packages the qualified model, compact RefSeq reference, and GENCODE
mask as one closed ten-file local transport. A canonical
`runtime-transport.json` declares the other nine files in fixed role order and
binds the runtime-profile identity, exact copied metadata/notices, stored sizes
and SHA-256 identities, reconstructed sizes and SHA-256 identities, and the
encoder contract. The manifest is schema-implied rather than self-declared;
its exact canonical-byte SHA-256 is the transport identity.

The three runtime payloads use separate deterministic Zstandard 1.5.7 frames
at level 9 with checksum and content size enabled, dictionary ID and
long-distance mode disabled, and zero workers. Separate frames let future
delivery avoid unrelated downloads while the one manifest prevents a mixed
runtime tuple. Transport compression is removed before mmap or model use.

Pack authenticates a canonical internally consistent profile and the supplied
component bytes. This generic integrity operation does not grant the
production trust authority owned by the existing installer. The profile's SNV
identity is metadata only: pack never opens, hashes, copies, or compresses the
15 GB score member.

Verify streams every stored and reconstructed byte without writing an unpacked
payload. Unpack performs the same checks while decoding each frame exactly once
into a private same-filesystem stage, then publishes only the complete layout
with atomic no-replace rename. Extra entries, unsafe file types, noncanonical
metadata, altered bytes, second frames, and trailing bytes fail closed.

The checked mask notice attributes the archived GENCODE v38 annotation and
describes `domains.pgm` as Pangopup's transformed runtime representation. The
transport does not contain the upstream GTF, intermediate SQLite database,
NCBI FASTA/report, Zenodo archive, or original checkpoint containers.

This decision establishes local reversible packaging only. Public release,
remote sync, XDG install-from-transport integration, and publisher
authentication remain separately reviewed effects.
