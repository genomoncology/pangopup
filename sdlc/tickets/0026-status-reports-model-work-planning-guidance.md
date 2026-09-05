---
flow: build
priority: 5
---
# Status reports model-work planning guidance

A caller must pick an HTTP read timeout. Too short and it abandons admitted model work that is still running, which wastes the inference and leaves the outcome unknown. `serve --help` and the service architecture both state the available planning arithmetic: the slowest retained p50 is 10.241 seconds per uncached model variant, and the default capacity of twenty units gives a planning estimate of about 205 seconds. A p50 is not an upper bound or a timeout guarantee.

The service holds that factor as a private constant and uses it for `Retry-After`. It never reports it. A client that wants to size a timeout from the deployment it actually talks to must copy 10.241 out of prose and multiply it by a status field.

Tickets 0012, 0018, and 0021 established the same shared-source principle for other service facts. This ticket applies that principle to the retained planning measurement.

Report the planning factor and the conservative full-capacity planning estimate in the status model object, computed from the same value that produces `Retry-After`. Do not divide the estimate by worker count because the retained measurements do not prove linear scaling. A caller can then choose its timeout and safety margin from the deployment it is talking to without copying a number out of documentation. Say in the same place that these values come from a retained measurement and do not guarantee latency, matching the wording the executable already uses.

Done, observably:

- `/v1/status` reports the per-unit planning seconds and the planning estimate for a full queue.
- Both reported values derive from the same constant that computes `Retry-After`, so a saturation header and the reported estimate can never disagree.
- Changing `--model-queue-capacity` changes the reported full-capacity estimate and leaves the per-unit factor unchanged.
- The reported values carry clear units and are documented as planning guidance rather than a guarantee or timeout recommendation.
- The values do not enter `scoring_identity`, because they do not change a score.
- The reported estimate matches the number `serve --help` states for the default capacity.
- Compatibility notes state that strict JSON consumers must adopt the added status fields before deployment.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the measured constant, the `Retry-After` formula, admission accounting, queue capacity defaults, limits, HTTP statuses, existing status fields, non-status response shapes, scoring, routing, or caching. The only status-shape change is the two additive planning fields required above. Do not add a server-side timeout, a latency guarantee, a recommended client timeout, a per-request estimate, or configuration for either value.

## Review

- Design review: approved after remediation at content SHA-256 `31cd7c7da87013336bf19e3a194d5f46f710b45347ea4783bb43d43497f7aa10`. The reviewer verified the retained 10.241-second factor and 205-second default arithmetic. The revision corrects the shared-source precedent, permits only the two required status additions, and requires coordinated adoption by strict JSON consumers.
- Code review: pending.
