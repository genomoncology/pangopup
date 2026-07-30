# Ticket 033 runtime release preparation evidence

## Shipped implementation

`pangopup-build runtime-release prepare` is the offline, production-only
boundary between the retained qualified ten-file runtime transport and a later
GitHub draft release. It accepts only the pinned transport/runtime-profile
identities, copies each retained member exactly once through a held no-follow
descriptor, and atomically publishes a private read-only stage.

Before copying, it streams each Zstandard frame once through bounded
decompression to authenticate both stored and reconstructed identities. It
does not materialize an uncompressed runtime file or payload-sized heap value;
the retained stored members are subsequently reread once for the byte-exact
copy.

The stage contains the ten byte-identical transport files plus canonical
`runtime-release-profile.json`, deterministic `SHA256SUMS`, and deterministic
`RELEASE-NOTES.md`. The profile records exact component identities, immutable
future URLs, the twelve upstream checkpoint source records, `model.py` and
license records, and converter paths at the target commit. No raw upstream
input, local path, timestamp, hostname, credential, rebuild, recompression,
materialized decoded payload, network operation, tag, release, or upload is
involved.

## Miniature implementation proof

The focused Rust suite uses the hidden test-build-only preparation contract and
proves:

- two preparations are byte-identical closed thirteen-file sets;
- every file is mode `0400`, link count one, and the directory is mode `0500`;
- profile parsing is closed and canonical, checksums have exact order, and
  release notes state the excluded raw inputs;
- the public production entry rejects the valid miniature transport;
- invalid commits, wrong identities, missing/extra/corrupt/truncated,
  symlinked, multiply linked, and non-regular members fail closed;
- injected copy, file-sync, directory-sync, and publication failures leave no
  final output;
- source pathname replacement is detected through retained descriptors; and
- FIFO replacement between nonblocking path admission and readable open is
  rejected without blocking; and
- post-rename parent-sync failure reports a visible publication whose
  durability is unconfirmed.

## Production result

Coordinator-owned after the reviewed implementation commit and its green
remote gate. Record the exact target commit, command outcome, thirteen-file
inventory, modes/link counts, SHA-256 values, copied-byte equality, compressed
frame total, `/usr/bin/time -v` resource measurements (including maximum RSS
and filesystem reads from streaming semantic verification plus stored-byte
copy), and no-network/no-build statements here before Ticket 034.
