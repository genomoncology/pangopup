# 026 — Use the installed runtime profile for model fallback

Status: deferred — reference reader/provenance prerequisite required

## Why

Ticket 025 can install and atomically select the exact SNV, model, compiled
GRCh38 reference, mask, and scoring policy that belong together. Ordinary
lookup still ignores that installation and requires three explicit fallback
paths. The installed profile therefore cannot yet produce a splice score.

The next bounded outcome is local consumption, not delivery: when lookup needs
the model and no explicit fallback tuple was supplied, Pangopup discovers the
coherent installed profile, opens its immutable members, and returns the same
typed result as the explicit-path route. A second offline process can reuse the
same installation and persistent SQLite result without source paths or network
access.

## Scope

- Add a typed installed-runtime opener in `pangopup-assets`. It must:
  - resolve the same Linux/XDG data root and active runtime pointer as Ticket
    025;
  - accept the exact bundle identity returned with the already-open active SNV
    provider, validate the canonical profile/receipt and exact production
    tuple against that identity, and never reread root `active.json` to choose
    a second SNV identity;
  - retain no-follow descriptors for the selected model, reference, and mask
    members and recheck their named inode chain before returning;
  - enforce installed ownership, mode, link-count, member shape, declared
    size, and structural identity without scanning or rehashing the 15 GB SNV
    payload or 772 MB reference payload;
  - return typed component identities plus held inputs suitable for the
    existing providers, not raw caller-controlled paths.
- Extend only the existing provider admission/open APIs needed to consume held
  installed members. Preserve their explicit-path behavior. Do not add a
  second model scorer, router, cache, reference format, mask format, or trust
  vocabulary.
- Change `pangopup lookup` routing:
  - an authoritative SNV hit remains completely lazy: it does not inspect
    runtime-profile state, SQLite, reference, mask, or model;
  - if a request needs model fallback and all three explicit fallback flags
    are present, keep the existing explicit behavior byte-for-byte and do not
    inspect installed runtime state;
  - if a request needs model fallback, no explicit fallback flag is present,
    and lookup uses the active installed SNV bundle, discover and use the
    installed coherent profile;
  - never combine an explicit `--bundle` SNV override with an implicitly
    discovered model profile. A model-required request in that shape keeps the
    stable model-assets-required failure unless the complete explicit fallback
    tuple is supplied;
  - partial explicit fallback grammar remains an immediate `CLI_USAGE` error.
- Permit `--model-cache` and `--model-cache-max-entries` with the implicit
  installed route as well as the explicit route. Keep the same XDG cache
  default, key, value, bounds, corruption behavior, and SQLite implementation.
  Cache options alone do not force runtime discovery for an authoritative SNV
  hit. `--bundle` plus cache options but no complete explicit fallback tuple
  remains the current immediate `CLI_USAGE` error with message `model cache
  options require --model-bundle, --reference-bundle, and --mask`. For the
  implicit route, resolve XDG/default/environment cache configuration only
  after the router requires modeling and the installed profile is admitted;
  malformed cache environment cannot break an authoritative SNV hit.
- Freeze these installed-route failures as compact stderr JSON with
  `details:null`:
  - absent `runtime/active.json`: exit 1, `ASSETS_MISSING`, `installed runtime
    profile is missing`;
  - symlink, hardlink, ownership/mode, or hostile entry: exit 1,
    `PROFILE_UNSAFE`, `installed runtime state is unsafe`;
  - malformed metadata, member structure, or an inode-chain change before
    return: exit 1, `PROFILE_CORRUPT`, `installed runtime profile is invalid`;
  - production-tuple or expected already-open SNV identity mismatch: exit 1,
    `PROFILE_INCOMPATIBLE`, `installed runtime profile is incompatible`.
  Existing invalid data-root behavior remains `PATH_INVALID`/exit 2. An
  explicit `--bundle` model-required request without the full explicit tuple
  retains `MODEL_ASSETS_REQUIRED`/exit 2 and the existing message. Never expose
  paths, JSON contents, or OS diagnostics.
- Add inside-out tests proving:
  - managed and explicit routes return byte-identical JSONL/table results and
    identical provenance for checked miniature SNV-miss and non-SNV cases;
  - authoritative SNV hits do not inspect a missing or corrupt installed
    runtime profile;
  - explicit fallback wins without inspecting installed state;
  - implicit fallback requires the active SNV/profile pair and rejects
    `--bundle` mixing, missing/malformed state, identity mismatch, symlink,
    hardlink, mode drift, and pathname replacement;
  - replacement before the final inode-chain check fails closed, while
    replacement after a successful capability return leaves queries using the
    held admitted inodes;
  - a first modeled request fills SQLite and a second independently composed
    CLI run returns the exact result without initializing ONNX or reading the
    dense reference/model payload;
  - normal tests use checked miniature assets through this exact private split:
    `pangopup-assets` unit tests authenticate/open real miniature held
    descriptors through a crate-private trusted-profile parameter; CLI unit or
    integration tests inject a crate-private already-open runtime capability
    or opener and independently compose the command twice. Executable specs
    exercise only production-reachable failures, laziness, explicit
    precedence, and grammar. No public synthetic profile bypass, environment
    backdoor, dev-only binary mode, or production-size copy enters the shipped
    binary or normal gates.
- Add executable specs for grammar, lazy authoritative hits, stable installed
  state failures, explicit precedence, and exact output. The successful
  miniature managed-route/restart proof may remain a Rust integration test
  because the production binary accepts only the production profile.
- Retain one concise evidence note in
  `planning/artifacts/026-installed-runtime-profile-consumption.md` describing
  opened components, lazy boundaries, route parity, and restart/cache proof.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/runtime-data.md`, `architecture/service.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add or amend an ADR only if
  implementation establishes a durable boundary not already owned by ADR
  0021. State plainly that local installed inference works but remote delivery,
  HTTP, container, and service lifecycle remain future.

Explicit exclusions: network sync for model-side assets, transport packing,
GitHub release assets, publication, production clean-machine install,
production-size installation in tests, SNV transport changes, new inference
or batching policy, cache redesign, in-memory LRU, cache fill coalescing,
HTTP, `serve`, status HTTP, start/stop/restart commands, Docker, systemd,
rollback/GC/repair, signing, SBOM, GPU/MPS/CUDA, HGVS, transcript projection,
and public external effects.

## Success Checklist

- With one coherent local installation, ordinary lookup can return an exact
  precomputed SNV hit or transparently model a supported miss/non-SNV without
  three explicit asset paths.
- Lookup never mixes an explicit SNV bundle with an unrelated implicit model
  profile and never observes replaced or mutable installed members.
- The hit path remains lazy and unchanged; the managed model path is
  byte-identical to explicit fallback.
- A checked second-process composition proves offline persistent-cache reuse
  without ONNX initialization or dense payload reads.
- Fast unit/integration/spec coverage proves corrupt and hostile state without
  production assets or a long-running verifier.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Explicit fallback wins.** Explicit paths are an existing diagnostic and
   maintenance contract. Mixing some explicit components with installed ones
   would create an unprofiled tuple, so only all-explicit or all-installed is
   accepted.
2. **Implicit fallback belongs only to the active SNV pair.** The runtime
   profile binds the active installed SNV identity. Combining it with
   `--bundle` could silently route one request through incompatible sources;
   callers using an SNV override must also provide the full explicit fallback
   tuple when modeling is required.
   Admission receives the identity coupled to the already-open SNV provider;
   it does not independently reread the mutable SNV active pointer.
3. **Keep lookup-first laziness.** Installed discovery is deferred until the
   router says a request needs the model. This preserves the fast mmap SNV path
   and prevents corrupt optional model assets from breaking authoritative
   precomputed answers.
4. **Trust immutable installation, then structurally open.** Ticket 025
   authenticated every copied byte before activation. Runtime admission
   revalidates the small coherent metadata, immutable inode policy, sizes, and
   provider structure; it does not pay a 772 MB hash on every process start.
5. **Reuse SQLite across process restarts.** The persistent exact cache already
   binds the complete scoring identity. Installed discovery supplies those
   same identities; no in-memory LRU or second cache is introduced.

## Dependencies

Tickets 019, 020, 023, 024, and 025.

## Notes

- The production authority remains
  `planning/artifacts/024-four-asset-runtime-profile.json`.
- Ticket 025's installer and status reader already own layout and activation.
  Extend that module or a narrow sibling; do not copy its parser or pathname
  logic into the CLI.
- Existing explicit fallback tests are the output and failure-order oracle.
- No retained production asset is copied, installed, initialized, or
  published in this ticket.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 025 outcome and the rolling frontier. This is
the smallest slice that turns local coherent installation into useful splice
scoring while keeping delivery and service work separate.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket026_design_review`

Initial verdict: **REJECT**. The reviewer found that the draft could reread the
SNV active pointer and combine a newly selected identity with an already-open
provider, did not define a feasible cross-crate miniature seam, left
`--bundle` cache grammar and cache-environment laziness ambiguous, and did not
freeze installed-state errors. The reviewer also required explicit semantics
for pathname replacement before and after capability return.

The coordinator bound admission to the already-open provider identity, defined
the assets/CLI crate-private test split, preserved the existing `--bundle`
cache usage error, deferred implicit cache environment resolution until a
model-required route, pinned exact installed error JSON codes/messages/exits,
and split pre-return failure from post-return held-inode behavior.

Revised verdict: **ACCEPT**. The reviewer confirmed that the identity race,
private seam, cache grammar/laziness, exact errors, and pathname-replacement
semantics are now closed and that the outcome is the correct next bounded
slice. The CLI-private recomposition proof is a unit test (a separate Rust
integration-test crate cannot access crate-private items); it must fully drop
and rebuild first-run state and be described as process-equivalent rather than
as a spawned production binary.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket026_implementation`

The attempted implementation proved the routing, error, parity, and persistent
cache behavior, but exposed an unreviewed reference boundary. The installed
runtime must mmap the exact descriptor admitted by Ticket 025. The existing
held-descriptor reader lives in the same 1,829-line module as the
byte-producing writer, and the v1 builder fingerprint hashes that entire
module.

The first workaround reopened the member by pathname and retained a race. The
second copied roughly 500 lines of PGRREF01 parsing/decoding into runtime
admission. Independent code review rejected both. The rejected working diff is
preserved locally as stash `ticket-026-rejected-runtime-consumption`; none of
it was committed, pushed, published, or used to rebuild a production asset.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket026_code_review`

Verdict: **REJECT**. The final attempted diff removed the pathname race but
introduced a second reference parser/decoder with narrow coverage and a public
unsafe synthetic-provenance seam. That violated the single-reader and opaque
admission requirements.

The ticket returned to the original design reviewer. Revised design verdict:
do not expand Ticket 026. First deliver a separate prerequisite that separates
builder-causal reference code from runtime reader code, defines a precise v2
builder fingerprint, and adds one shared held-descriptor reader constructor.
The prerequisite must preserve the existing production reference bundle and
profile without reading, copying, repacking, or rebuilding them. Installed
runtime consumption resumes afterward as a new reviewed ticket.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
