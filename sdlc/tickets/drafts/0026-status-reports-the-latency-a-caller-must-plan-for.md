---
flow: build
priority: 5
---
# Status reports the latency a caller must plan for

A caller must pick an HTTP read timeout. Too short and it abandons admitted model work that is still running, which wastes the inference and leaves the outcome unknown. `serve --help` and the service architecture both state the arithmetic: the slowest retained p50 is 10.241 seconds per uncached model variant, and the default capacity of twenty units gives a planning estimate of about 205 seconds.

The service holds that factor as a private constant and uses it for `Retry-After`. It never reports it. A client that wants to size a timeout from the deployment it actually talks to must copy 10.241 out of prose and multiply it by a status field.

Tickets 0012, 0018, and 0021 all removed a copied service constant from consumers. This is the same constant in the same shape.

Report the planning factor and the full-capacity planning estimate in the status model object, computed from the same value that produces `Retry-After`. A caller then sizes its read timeout from the deployment it is talking to and never copies a number out of documentation. Say in the same place that these are planning guidance from a retained measurement and not a latency guarantee, matching the wording the executable already uses.

Done, observably:

- `/v1/status` reports the per-unit planning seconds and the planning estimate for a full queue.
- Both reported values derive from the same constant that computes `Retry-After`, so a saturation header and the reported estimate can never disagree.
- Changing `--model-queue-capacity` changes the reported full-capacity estimate and leaves the per-unit factor unchanged.
- The reported values are documented as planning guidance rather than a guarantee.
- The values do not enter `scoring_identity`, because they do not change a score.
- The reported estimate matches the number `serve --help` states for the default capacity.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the measured constant, the `Retry-After` formula, admission accounting, queue capacity defaults, limits, HTTP statuses, response shapes, scoring, routing, or caching. Do not add a server-side timeout, a latency guarantee, a per-request estimate, or configuration for either value.
