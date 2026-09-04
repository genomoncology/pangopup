---
flow: build
priority: 7
---
# The HTTP service answers a rejected request with a client status

The HTTP service returns status 500 for input the model refuses. The `lookup`
command treats the same input as a request error and exits 2.

`MODEL_REJECTED` names a request that will never succeed however many times it
is sent. A 500 states the opposite: that the server failed and the caller should
try again. Proxies, client libraries and job runners act on that, so a submitted
typo becomes repeated inference work on a queue that is already the scarcest
resource in the service. Availability measurements count the same typo as the
service failing.

Two inputs reproduce it against an installed profile. An allele longer than 100
bases, and a REF that does not match GRCh38:

```
CLI : MODEL_REJECTED "model alleles exceed 100 bases (REF 1, ALT 151)"   exit 2
HTTP: 500 MODEL_REJECTED "scoring failed"
```

The same route already answers 400 `INVALID_REQUEST` for a variant string it
cannot parse, so the surface disagrees with itself about which failures are the
caller's.

Ticket 0001 made the failure family visible in the response body. The status
line still reports every family as a server failure: `process_job` returns
`INTERNAL_SERVER_ERROR` for every backend failure code it is given.

The status this ticket requires is 422, and the reply keeps the generic message
it carries today. Both are settled here because they are an outward-facing
contract rather than an implementation choice. 422 is already what this service
answers when it refuses a request on its meaning rather than its syntax, as it
does for a batch carrying too many uncached variants. 400 would merge that
refusal with the parse failures the route already answers 400 for, and the two
are worth telling apart.

Issue: `sdlc/issues/2026-09-04-return-4xx-for-rejected-model-input.md`

Done, observably:

- An HTTP caller that submits input the model refuses receives status 422.
- An HTTP caller that meets a scoring failure or an unusable cache still
  receives a status in the 5xx range.
- A caller can tell those two situations apart from the status line alone,
  without reading the response body.
- The message text carried in a failure reply is the generic one sent today.
- The spec suite pins each family's status with a case that fails before the
  change.

Boundary: do not change which failure family the backend decides, the failure
codes already carried in the response body, the message text those replies
carry, the response shape, or anything on
the success path. Do not change how the `lookup` command reports or what it
exits with. Do not add or remove a failure family. The 400 answer for an
unparseable variant string stays as it is.
