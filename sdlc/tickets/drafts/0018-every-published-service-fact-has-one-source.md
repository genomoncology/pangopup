---
flow: build
priority: 4
---
# Every fact the service publishes comes from one source

Ticket 0012 gave clients a machine-readable contract. Clients no longer need to
copy PangoPup's limits and vocabulary into their own code. Three facts inside
that contract are still written twice inside PangoPup itself. Every copy agrees
today. Each one can drift. A drifted copy publishes a contract the service does
not honor.

The effective CPU policy is written twice.
`crates/pangopup-cli/src/service.rs:663` computes it once as `policy.to_string()`
and feeds it to result provenance, to the model cache identity, and to the
active scoring identity that ticket 0015 added.
`crates/pangopup-cli/src/service.rs:1031` builds the same fact again for
`/v1/status` as `format!("sequential:{}/1", state.dispatcher.threads)`. The two
agree because the service admits exactly one execution mode and one inter-op
thread. A second mode, or a configurable inter-op count, would make the status
route publish a policy that no score was produced under. The identity digest
would disagree in silence.

The mitochondrial contig aliases are written twice.
`crates/pangopup-cli/src/main.rs` owns the accepted spellings and serves them
into the published `request_contract.variants.contigs` array. That array reports
`M`, `MT`, `chrM`, `chrMT`, and `NC_012920.1` for canonical `chrM`.
`crates/pangopup-build/src/main.rs:484` restates `"MT" | "chrMT" => Some("chrM")`
for the maintenance reference-window command. Ticket 0010 required one public
alias policy across every user-input adapter. Two lists now decide it.

Two enforced request limits are written twice.
`crates/pangopup-cli/src/service.rs:56` holds `max_variants: 100` and
`max_uncached_model_variants: 10`, and the status contract publishes both.
The refusals that enforce them state the same numbers as prose at
`crates/pangopup-cli/src/service.rs:1234` and
`crates/pangopup-cli/src/service.rs:1342`. Lowering a limit would leave a caller
reading one number in the contract and a different number in the refusal that
just rejected it.

The behavior this ticket requires is one source for each of those three facts.
Enforcement, serialization, and message text read that source. A change to the
source reaches every place the fact appears, and a check proves it rather than a
reader remembering to look.

Three choices this ticket settles, because the agent cannot make them alone.

Nothing the service publishes changes. `sequential:1/1`, the five accepted
mitochondrial spellings, `10`, `100`, every error code, every HTTP status, and
every response shape stay exactly as they are. Refusal messages may be built
from the source values, and for the current limits they must still read byte for
byte as they read today. A consumer that upgrades sees no difference.

`pangopup-core` does not change. Its source contributes to immutable asset
fingerprint evidence, and ticket 0010 already had to move an alias out of it
after the fingerprint gate caught the change. A shared contig-spelling source
therefore belongs somewhere both binaries already depend on, or somewhere new.
`crates/pangopup-index` already supplies `required_accession` to both.

Source ingestion stays stricter than caller input. Stored source identities
require canonical contig names, and ticket 0010 kept `MT` and `chrMT` refused
there on purpose. Single-sourcing the caller-facing aliases must not widen it.

Done, observably:

- The status route, result provenance, the model cache identity, and the active
  scoring identity report one effective CPU policy that the service computes
  once.
- One contig-spelling source drives the scoring adapters, the published
  contract, and the maintenance reference-window command.
- Each request-limit refusal states the number the published contract reports
  for that limit.
- Changing one of these sources changes every place its fact appears. A test
  proves that for the CPU policy, for a contig spelling, and for a request
  limit.
- Source ingestion still refuses `MT` and `chrMT`.
- Every value, message, code, status, and response shape the service publishes
  today is byte-identical afterwards. The existing status, contract, saturation,
  and rejection tests keep passing unchanged in meaning.

Boundary: do not change any limit, alias, policy string, error code, HTTP
status, response shape, accepted input, or reported gene identity. Do not change
`pangopup-core`, an artifact builder-source fingerprint, a checked bundle
manifest, or an immutable asset identity. Do not add a configuration option, a
status field, an execution mode, or an inter-op thread control. Do not widen
source ingestion. Do not change admission, scoring, routing, or caching.
