# The scoring route accepts a non-JSON content type

`/v1/score` scores a request declaring `content-type: text/plain`, and one
declaring no content type at all. Both answer 200 with a normal result.

```
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/score \
  -H 'content-type: text/plain' \
  -d '{"variants":["GRCh38:chr12:6801301:G:A"]}'
200
```

## Why this matters

A JSON endpoint that accepts `text/plain` can be reached by a form posted from
a page the operator does not control, because that content type does not oblige
a browser to ask permission first. The service ships without authentication and
`README.md` directs operators to loopback or an authenticated proxy, so what is
reachable today is small. The check is cheap and stops the surface widening
quietly the first time someone exposes the port.

## Suggested direction

Require a JSON content type and refuse anything else before scoring work is
admitted. The exact status and the treatment of content-type parameters are an
outward-facing contract and should be settled in the ticket rather than left to
the implementation.

Ticket: `sdlc/tickets/drafts/0009-require-a-json-content-type-for-scoring.md`
