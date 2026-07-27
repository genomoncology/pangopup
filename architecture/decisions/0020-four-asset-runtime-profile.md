# ADR 0020: One canonical four-asset runtime profile

## Decision

Pangopup uses one path-free `pangopup.runtime-profile.v1` JSON document to bind
the exact production SNV index, singleton ONNX model, RefSeq GRCh38.p14
sequence bundle, selected GENCODE v38 mask member, and scoring policy.
Canonical RFC 8785/JCS bytes are the authority; the external profile identity
is the SHA-256 of those exact bytes.

The maintainer-only `pangopup-build runtime-profile prepare` command accepts
four explicit local inputs and only succeeds for the jointly qualified tuple.
It authenticates model, reference, and mask bytes through retained
descriptors. SNV admission reads canonical bounded metadata and checks the held
score member's declared size without scanning or hashing the 15 GB payload;
existing installation/certification remains authoritative for that digest.

## Why

Four independently valid assets are not automatically compatible. A small
immutable compatibility statement lets later installation, activation,
readiness, and delivery reject accidental mixtures without embedding
host-specific paths or mutable download locations.

## Consequences

- Future compatible tuples need an explicitly reviewed profile version or
  accepted tuple.
- The profile contains no paths, URLs, credentials, timestamps, aliases, or
  recursive self-ID.
- Profile preparation performs no ONNX initialization or inference and makes
  no network or publication change.
- Installation, activation, sync, HTTP, Docker, and systemd remain separate
  future outcomes.
