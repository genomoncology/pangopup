---
flow: build
priority: 1
deps: []
---
# Production qualification uses an owned port

## Outcome

The production qualification owns the loopback port it tests. An unrelated local service cannot make a correct candidate fail or receive the qualification requests.

## Done, observably

- Replace the duplicated fixed port with one source of truth passed between the fixture server and qualification runner.
- Prefer a port reserved by binding loopback port zero and preserve ownership until the intended server takes it. If the platform cannot transfer that reservation safely, use a bounded retry that proves the started process owns the selected port before sending a request.
- A fixture that occupies port 18080 must still pass. A fixture that races for the selected port must fail with a message naming the port and ownership problem, never contact the competing service, and clean up every process it started.
- Preserve direct maintainer use with a documented explicit-port input when needed.
- Keep the implementation portable across supported macOS and Linux hosts.
- Archive draft 0117. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the qualification harness and its fixtures only. Do not change the production service's public address defaults, start a retained service, or use a non-loopback interface.
