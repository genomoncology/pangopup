---
flow: build
priority: 3
---
# The scoring route requires a JSON content type

`/v1/score` scores a request that declares `content-type: text/plain`, and one
that declares no content type at all. Both answer 200.

A JSON endpoint that accepts `text/plain` can be reached by a form submitted
from a page the operator does not control, because that content type does not
oblige a browser to ask permission first. The service ships with no
authentication and the README directs operators to keep it on loopback or behind
an authenticated proxy, so what is reachable today is small. It is a cheap
check, and it stops the surface widening quietly the first time someone exposes
the port.

The status this ticket requires is 415, and a JSON content type means
`application/json`, with any legal parameter such as `charset=utf-8` accepted.
Both are settled here because they are an outward-facing contract rather than an
implementation choice.

Issue: `sdlc/issues/2026-09-04-require-a-json-content-type.md`

Done, observably:

- A scoring request declaring `application/json` is served as it is today.
- That request is served whether or not the content type carries a parameter.
- A scoring request declaring any other content type, or none, is refused with
  status 415 and a message naming the reason.
- The refusal happens before any scoring work is admitted.
- The health and status routes are unaffected and still answer without a body.
- The suite pins the refusal with a case that fails before the change.

Boundary: do not change the request or response shape, the size limit, the
existing validation of a well-formed request, or any behavior once a request is
accepted. Do not add authentication, authorization, an origin check, or any
other header requirement.
