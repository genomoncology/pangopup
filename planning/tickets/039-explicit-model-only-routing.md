# 039 — Explicit model-only routing

Status: ready

## Why

Pangopup's shipped default is intentionally lookup-first: a covered GRCh38 SNV
returns the exact Zenodo precomputed result, while a lookup miss or supported
non-SNV uses the pinned Pangolin-compatible model. The lookup and model are
separately versioned prediction authorities and can differ slightly because the
Zenodo bulk generator is not the public Pangolin CLI. A caller therefore needs
one explicit way to bypass the SNV lookup and ask the model to score the whole
request batch. Today the CLI cannot do that for a covered SNV.

This is the next coherent slice because the same request choice must be settled
before the HTTP contract is built. The ordinary fast path and every existing
default response remain unchanged.

## Scope

- Add one command-level `--model-only` boolean flag to `pangopup lookup`.
- Put the bypass behavior in the typed routing layer owned by
  `pangopup-engine`, so the future HTTP adapter can reuse it without invoking
  CLI code.
- Let a complete explicit `--model-bundle`/`--reference-bundle`/`--mask`
  model-only invocation run with no `--bundle`, no `--data-dir`, no active
  installation, and no SNV asset on disk. Reject `--bundle` with
  `--model-only` as contradictory rather than opening or silently ignoring it.
- Let model-only without an explicit model tuple resolve the model, reference,
  and mask from the canonical activated profile selected by the ordinary XDG
  location or `--data-dir`. This installed path retains the coherent profile
  trust check; it does not perform a score lookup.
- When the flag is present, bypass precomputed lookup for every variant in the
  batch and use the existing model fallback, exact SQLite cache identity, gene
  filtering, modeled result, errors, and provenance.
- Preserve default lookup-first routing, lazy model opening, batch ordering,
  transactional stdout, JSONL, and table behavior byte-for-byte.
- Add executable CLI coverage to `spec/model-routing.md` and inside-out
  routing/parser/cache tests in the owning crates.
- Update the shipped and durable behavior descriptions in `README.md`,
  `AGENTS.md`, `architecture/design.md`, `architecture/service.md`,
  `architecture/decisions/0016-lookup-first-cli-model-routing.md`,
  `planning/frontier.md`, and `planning/faq.md`. These updates must also remove
  stale claims in the touched passages that model assets, combined activation,
  persistent model caching, or clean-machine CLI inference are still future.
- Exclude HTTP serving, Docker, automatic threshold policies, comparison
  envelopes, lookup-only flags, model changes, cache-format changes, asset
  publication, and new public external effects.

## Success Checklist

- `pangopup lookup` without `--model-only` retains its current exact observable
  behavior: a covered SNV is precomputed, an installed-profile lookup miss or
  supported non-SNV uses the model, and an explicit lookup-only bundle retains
  its existing SNV-miss behavior.
- `pangopup lookup --model-only` sends a covered SNV through the model and emits
  the existing modeled JSONL/table shape with `provenance.kind` equal to
  `model`; it does not emit or merge the available precomputed record.
- A routing-layer spy test proves model-only construction does not call the
  `ScoreProvider`, rather than merely observing model-shaped fixture output.
- A CLI/admission test proves an explicit model-only request neither opens nor
  reads an SNV bundle: it succeeds with a complete valid explicit model tuple,
  an absent active installation, and no SNV bundle anywhere in the test root.
  A missing or deliberately invalid unrelated SNV path cannot affect it.
- A default-route spy/observer test proves a covered SNV still does not open
  the reference, mask, or model and does not touch the SQLite cache.
- Model-only works for ordered multi-variant batches and retains transactional
  failure: a rejected or operationally failed model request writes no partial
  stdout.
- Model-only uses the existing complete-result SQLite cache without changing
  its schema or identity. Two CLI processes return byte-identical modeled
  output and the second request is a cache hit under focused test observation.
- Model-only with an activated installed profile works and admits the exact
  model/reference/mask members bound by that canonical profile. Model-only
  with the complete explicit tuple works without any installed profile. The
  two sources are never mixed.
- Missing model assets, duplicate `--model-only`, and incompatible combinations
  produce stable typed errors on stderr and no stdout. The exact error codes are
  selected from existing codes unless a genuinely distinct failure requires a
  new documented code. In particular, `--bundle` plus `--model-only`,
  `--data-dir` plus an explicit model tuple, and partial explicit tuples are
  usage errors.
- Existing default-routing specs remain unchanged and green. New executable
  specs demonstrate the same covered SNV returning `precomputed` by default and
  `model` with the flag, using bounded checked fixtures rather than production
  assets or network access.
- No threshold such as `0.2`, `0.5`, or `0.8` appears in routing logic. The
  caller may compare a normal lookup call with a separate model-only call.
- The focused tests and `make lint`, `make test`, and `make spec` pass.

## Decisions

### The public request control is one boolean flag

**Consideration:** callers need only one override: bypass the lookup and invoke
the model. A general mode grammar would advertise choices Pangopup does not
need.

**Options:** add `--model-only`; add a string-valued `--scoring-mode`; or add
several lookup/model/compare modes.

**Trade-offs:** an enum could grow without a later grammar change, but it makes
the current interface harder to explain and creates unsupported combinations.
A flag precisely names the sole exceptional behavior.

**Decision and why:** add `--model-only` as a command-level boolean accepted at
most once. The future HTTP request will use the equivalent optional
`model_only` boolean, but HTTP implementation is not part of this ticket.

### Default lookup-first output is immutable

**Consideration:** lookup hits are the speed-critical path and the published
Zenodo values are authoritative for that route. Existing CLI output is already
released in `v0.1.0`.

**Options:** change the default, return both authorities, annotate all existing
responses with a new request mode, or make the override additive only.

**Trade-offs:** richer envelopes could expose comparisons in one response but
would break the stable JSONL/table contract and add work to every ordinary
lookup.

**Decision and why:** the absent flag is byte-for-byte current behavior. The
flag produces the existing modeled result type. No new comparison or wrapper
result is introduced.

### Model-only bypasses lookup rather than looking up and discarding

**Consideration:** the flag must have unambiguous semantics and should not page
through the 15 GB SNV member when the caller explicitly requested inference.

**Options:** perform lookup then ignore it; inspect lookup only to determine
support; or construct a typed model-required request directly after ordinary
variant validation.

**Trade-offs:** reusing the normal router superficially reduces code changes,
but silently performs unwanted I/O and makes the guarantee hard to test.

**Decision and why:** the engine exposes a typed model-only route that cannot
consult the `ScoreProvider`. Existing reference validation, model support,
masking, gene filtering, cache admission, and errors remain authoritative.
The existing documentation that `ModelRequired` can only follow
`LookupFirstRouter::inspect` must remain true or be deliberately revised: the
implementation may add a distinct explicit-model request type rather than
forge a lookup-inspected value.

### Explicit and installed asset selection remain separate

**Consideration:** the current lookup-first CLI opens the SNV bundle before it
knows whether fallback is necessary. Reusing that control flow would make a
model-only user supply or install a 15 GB asset that is never used.

**Options:** preserve the current bundle-first opening; silently ignore a
supplied `--bundle`; or define separate explicit-tuple and installed-profile
model-only paths.

**Trade-offs:** preserving bundle-first is mechanically easy but violates the
flag's meaning. Silently ignored paths conceal user mistakes. Separate paths
add parser/admission coverage but make dependencies and trust explicit.

**Decision and why:** a complete explicit model tuple is self-sufficient and
must not open or require an SNV asset. `--bundle` is rejected with
`--model-only`; `--data-dir` selects the activated profile only when no
explicit tuple is supplied. The installed path retains canonical four-asset
identity coherence while admitting its model-side members; it does not create
or consult a lookup provider.

### Threshold comparison remains caller policy

**Consideration:** lookup and model scores can straddle a caller's threshold,
but thresholds may differ by workflow and by gain/loss/gene record.

**Options:** automatically invoke the model near built-in thresholds; accept a
configurable threshold policy; or let callers make a normal call and an
explicit model-only call.

**Trade-offs:** automatic policies hide seconds of extra CPU work and turn a
splice-scoring service into an interpretation policy engine. Two explicit
calls are simple, observable, and preserve both authorities.

**Decision and why:** Pangopup contains no threshold-triggered routing. It
returns provenance-bearing numeric predictions; callers decide whether and
when to request the other authority.

## Dependencies

- Tickets 020 and 028: shipped typed lookup-first routing and activated-profile
  consumption.
- Ticket 023: shipped persistent exact SQLite model-result cache.
- Ticket 038: shipped `v0.1.0` CLI contract whose default output must remain
  compatible.

## Notes

- Work in `/home/ian/workspace/repos/pangopup` and preserve unrelated changes.
- The repository gate is exactly `make lint`, `make test`, and `make spec`;
  there is no `make check`.
- Observable CLI behavior belongs in `spec/model-routing.md`; unit and
  integration tests own parser, routing, lazy-open, cache, and failure details.
- Normal tests must remain offline and bounded. Do not open production assets,
  invoke Python/PyTorch, download anything, or rerun the 10,000-row conformance
  experiment.
- The existing `RoutedResult::Modeled` and `ModelProvenance` are the output
  contract. If a shared model-required constructor exists when development
  begins and its invariant honestly includes explicit requests, use it;
  otherwise define a distinct narrow typed explicit-model API in this ticket.
- Public-repository hygiene applies: no credentials, private URLs, absolute
  developer paths, raw upstream source data, or experiment output is committed.
- Evidence in this ticket is illustrative; do not commit it as runtime data.
- This ticket intentionally changes CLI grammar and routing behavior. Reviewers
  should not reject the named spec/help/architecture updates as scope creep.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 038 outcome, the current implementation, and
Ian's decision that explicit model inference is a single flag rather than a
general scoring-mode option.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket039_design_review`

Initial verdict: **REJECT**.

The reviewer found that bypassing `ScoreProvider::lookup` alone would still
allow the CLI to open or require the 15 GB SNV bundle before selecting the
model path. The draft also did not fully define explicit-tuple versus installed
profile grammar, did not prove the SNV asset stayed unopened, and risked
forging `ModelRequired` despite its documented lookup-inspection invariant.

Coordinator disposition: accepted. The revised scope makes a complete explicit
model tuple self-sufficient without an SNV bundle or active installation,
rejects contradictory `--bundle` and explicit-tuple/`--data-dir` combinations,
keeps installed-profile selection distinct, requires an absent/invalid-SNV
open/read regression, and requires an honest typed explicit-model engine
boundary.

Re-review verdict: **ACCEPT**. The reviewer confirmed that every initial
finding is resolved, the default and cache contracts are protected, explicit
and installed asset selection are unambiguous, and the bounded outcome is the
correct prerequisite for the HTTP contract.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
