---
flow: build
priority: 1
deps: []
---
# Production qualification uses an owned port

## Outcome

The production qualification owns the loopback port it tests. An unrelated local service cannot make a correct candidate fail or receive the qualification requests.

## Done, observably

- Start the qualification service on `127.0.0.1:0` and read the existing JSON listening event to learn the port that the service actually bound.
- Make the fixture server accept the same listen input and emit the same listening event. The runner uses only the address from that child event.
- A fixture that occupies port 18080 still passes. A child that exits or emits a malformed or non-loopback address fails before any request and all started processes are cleaned up.
- Preserve the existing explicit-listen input for direct maintainer use.
- Archive draft 0117. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the qualification runner and its fixture server only. Reuse the production service's existing port-zero and listening-event behavior. Do not add port reservation or retry machinery, change public defaults, start a retained service, or use a non-loopback interface.
