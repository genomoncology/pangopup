# Production qualification uses an owned port

The production qualification now starts its child with `--listen 127.0.0.1:0`, waits at most 30 seconds for the existing JSON listening event, and sends every HTTP request to the validated child-reported address. It accepts only canonical loopback IPv4 addresses with a nonzero port. Early exit, timeout, malformed events, non-loopback addresses, and zero ports fail before a request. The exit trap stops and waits for every started child.

The Python fixture now honors explicit `--listen` input and emits the production event. Its focused cases hold request isolation while another fixture owns port 18080, the exact port-zero argument, early exit, malformed and unsafe events, bounded waiting, and cleanup after successful and failed starts. Production service code did not change.

On macOS, `make lint`, `make test`, and `make spec` passed. The portable focused shell checks passed, and the generated fixture bound an ephemeral loopback port and emitted its actual address. The repository excludes `tests/production-release-qualification.sh` outside Linux because it uses GNU tools. Run `bash tests/production-release-qualification.sh` on Linux from this exact candidate before merge.
