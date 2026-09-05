---
flow: build
priority: 3
---
# The scoring route requires a JSON content type

`/v1/score` scores a request that declares `content-type: text/plain`, and one
that declares no content type at all. Both answer 200.

Cross-origin JavaScript can send an exact JSON body as `text/plain` without a CORS preflight, although browser policy prevents the script from reading the response. Expensive scoring makes the request itself relevant. The service ships with no authentication and the README directs operators to keep it on loopback or behind an authenticated proxy, so what is reachable today is small. The check stops the surface widening quietly if an operator exposes the port.

The status this ticket requires is 415, and a JSON content type means
`application/json`, with any legal parameter such as `charset=utf-8` accepted.
Both are settled here because they are an outward-facing contract rather than an
implementation choice.

The request must carry exactly one `Content-Type` field. Media-type matching is case-insensitive and uses parsed media-type syntax. `application/json; charset=utf-8` is accepted. A missing value, malformed value, `text/plain`, structured suffix types such as `application/json-patch+json`, and repeated fields are rejected. Identical repeated fields do not receive an exception. The strict subtype avoids silently widening this route to contracts it does not implement. The accepted cost is that a caller with a compatible custom JSON media type must send `application/json` instead.

The refusal uses `UNSUPPORTED_MEDIA_TYPE` and the message `content-type must be application/json` in the existing error envelope.

Issue: `sdlc/issues/2026-09-04-require-a-json-content-type.md`

Done, observably:

- A scoring request declaring `application/json` is served as it is today.
- That request is served whether or not the content type carries a parameter.
- A scoring request declaring any other content type, or none, is refused with
  status 415 and the defined error code and message.
- Media-type validation runs after route and method selection but before readiness, body limits, body reads, and admission. An invalid scoring request therefore receives 415 even while the service is draining or failed and even when its body would exceed the size limit. The accepted cost is that the caller must correct its media type before this route can reveal scoring readiness.
- The health and status routes are unaffected and still answer without a body.
- `README.md` documents the required media type. `spec/http-service.md` pins it through the real executable.
- Tests cover mixed-case `application/json`, legal parameters, a missing value, a malformed value, `text/plain`, `application/json-patch+json`, identical and conflicting repeated fields, precedence over readiness and body limits, and no effect on health or status routes. Real-listener score helpers send deliberate headers rather than inheriting an implicit default.

Boundary: do not accept `application/*+json`. Do not change the request or response shape, the size limit, the existing validation of a well-formed request, or any behavior once a request is accepted. Do not add authentication, authorization, an origin check, or any other header requirement.
