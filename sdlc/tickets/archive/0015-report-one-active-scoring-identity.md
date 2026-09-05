---
flow: build
priority: 6
---
# Every score reports one active scoring identity

PangoPup reports detailed route provenance and lists its active asset identities through the status route. A downstream annotation system has room for one data-set version. It therefore asks an operator to configure a second version string by hand. That string can disagree with the image, installed assets, scoring semantics, or CPU policy that produced the score.

PangoPup must report one concise active scoring identity through `/v1/status` and every result item returned by `/v1/score`. The standalone CLI remains unchanged because it can run with only an SNV bundle and cannot always know a complete service environment. The identity changes whenever an installed input, running software release, or active policy capable of changing an answer changes. Detailed precomputed and modeled provenance remains available and authoritative. The concise identity names the complete active service environment so a consumer with one version field can record the truth without reconstructing it.

Every HTTP result item carries the same identity regardless of whether its status is `found`, `not_found`, `ambiguous_source_reference`, `mixed`, or `rejected`, and regardless of whether precomputed lookup, model inference, or cache reuse answered it. An all-rejected HTTP 422 remains a request-level error with no result item and no scoring identity.

The reported value is the full lowercase `sha256:<64 hex>` digest of this RFC 8785 canonical JSON preimage:

```json
{"effective_cpu_policy":"sequential:<threads>/1","runtime_profile_id":"sha256:<64 lowercase hex>","schema":"pangopup.active-scoring-identity.v1","software_version":"<CARGO_PKG_VERSION>"}
```

The admitted runtime profile identity already commits to the complete canonical runtime profile: the SNV bundle; model bundle, profile, and representation; reference bundle, profile, and sequence set; mask member; scoring semantics; masking policy; distance; and declared CPU policy. The separate effective CPU policy captures the service's thread override. The software version binds the running release. This deliberately changes the identity for a documentation-only release. That cost prevents a consumer from silently treating different service builds as the same active environment.

The identity must remain stable across restarts and machines that use the same software version, admitted runtime profile, and effective CPU policy. Worker count, queue capacity and state, cache path, cache limit and contents, listener address, filesystem paths, process identity, host details, request variant, gene filter, and `model_only` must not affect it. Worker count and queue policy do not change score calculation. Variant, filter, and route selection belong to the request. Existing detailed provenance records which route answered.

`pangopup-assets` owns a typed active-identity preimage and its canonical hashing beside the canonical runtime-profile identity. Construction accepts the software version, admitted runtime profile identity, and effective CPU policy as explicit inputs. The service calculates the value once during startup, retains it in service state, exposes it as top-level `scoring_identity` in `/v1/status`, and adds it to each `/v1/score` result object. The engine and route provenance remain unchanged.

The active scoring identity is an environment-level superset of the model cache identity. It does not replace `CacheIdentity`, enter a cache key, or change the cache schema. The cache correctly excludes the SNV route and software release while retaining every model-answer input.

This changes a public contract consumed by scoring clients. A downstream annotation service can remove its manually configured PangoPup data-set version after it adopts a release carrying this identity.

Done, observably:

- One pinned canonical preimage produces one exact full digest. Changing the software version, runtime profile identity, or effective CPU policy changes the digest. Repeated construction produces identical canonical bytes and identity.
- The status response and precomputed, modeled, cached, ambiguous, mixed, and mixed-batch rejected result items report the same active scoring identity.
- Two services with the same software version, admitted runtime profile, and effective CPU policy report the same identity. Restarting one of them does not change it.
- Changing an admitted asset profile or effective thread policy changes the identity.
- Worker count, queue capacity and state, cache path, cache limit and contents, listener address, filesystem paths, process identity, host details, request variant, gene filter, and `model_only` do not affect or appear in the identity.
- Detailed route provenance remains unchanged and lets a caller audit the components represented by the concise identity.
- One real HTTP lifecycle test proves that `/v1/status` and `/v1/score` report the same identity.
- User documentation tells consumers when to store the concise identity and when to retain the detailed route provenance.

Boundary: do not change standalone CLI output, score calculation, provenance values, asset formats, cache identity or schema, request limits, input parsing, routing, or readiness. Do not add the active identity to `RoutedResult` or route provenance. Do not remove or rename an existing status or result field.
