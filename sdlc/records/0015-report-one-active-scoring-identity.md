---
base: dad7162925c5dfbb8b3afa2d905ebd07246a3a5a
head: ea84e467e8266cf6556a951c54bcf80194262722
---

# Report one active scoring identity

The HTTP service now reports one full SHA-256 `scoring_identity` through `/v1/status` and every result item returned by `/v1/score`. The identity uses RFC 8785 canonical JSON over the software version, admitted runtime-profile identity, and effective CPU policy. Precomputed, modeled, cached, ambiguous, mixed, and item-level rejected outcomes share the same service identity.

Design review narrowed the contract to HTTP because standalone CLI lookup can run without a complete service profile. It also defined the exact preimage, all result shapes, excluded operational and request details, asset-layer ownership, and separation from route provenance and cache identity. This accepts identity churn for any software release. The cost prevents consumers from treating different service builds as one active environment.

Code review accepted the implementation without findings. The asset layer owns typed construction and canonical hashing. The service computes the identity once during startup and adds it at the HTTP rendering boundary. Standalone CLI output, `RoutedResult`, detailed provenance, cache identity and schema, asset formats, scoring, routing, limits, and readiness remain unchanged.

Root verification passed `git diff --check`, `make lint`, `make test`, and `make spec`. The executable specification reported 282 passed and 7 skipped. The README remains inside its original compactness limit at 258 lines and 1,683 words. Existing dependency-duplication warnings remained non-failing. The shared hardening issue remains because it contains held observations beyond this ticket.
