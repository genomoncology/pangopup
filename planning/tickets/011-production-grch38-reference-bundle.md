# 011 — Ship the production GRCh38 reference bundle and provider

Status: ready
Accepted contract identity: `sha256:c763eab142754e921981943b7c7dc8702f41097146abde08b35e69be12eedffc`
Base revision: `a11de87a27c2f23d6a9fa14637f2273e567608ab`

## Why

Ticket 010 selected `acgt2-rle-v1` by measured speed, but its `PGRBEN01`
container remains benchmark-only. Model fallback cannot proceed until a
long-lived Rust process can mmap an exact complete GRCh38.p14 sequence asset
and copy a Pangolin-sized window without parsing FASTA, loading the genome into
heap memory, or allocating per request.

The observable outcome is one production `PGRREF01` bundle over all 25 required
RefSeq GRCh38.p14 sequences, a typed caller-buffer provider, maintenance CLI
proof, and one preserved full build/qualification. This ticket does not package
models or GENCODE masking data and does not publish or install the asset.

## Current facts and provenance

- ADR 0009 permits production hardening only of `acgt2-rle-v1`. Its retained
  six-contig warm M01–M14 result was p50/p95 `4469/4880 ns`, zero allocations,
  member size `165759160`, and pinned-Zstandard size `144828782` bytes.
- The exact local NCBI source is
  `GCF_000001405.40_GRCh38.p14_genomic.fna.gz`, `972898531` bytes, SHA-256
  `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`.
  It is under the existing read-only GRCh38.p14 assembly directory in
  `/home/ian/foss/uta/ncbi-data/genomes/refseq/vertebrate_mammalian/`.
- The adjacent assembly report is `80454` bytes with SHA-256
  `64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38`.
  Its 25 required `assembled-molecule` rows are chr1–22, X, Y, plus chrM in
  NCBI's non-nuclear assembly unit.
- Independently retained full-source evidence establishes 25 sequences,
  `3088286401` bases, canonical sequence SHA-256
  `2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`,
  680 ignored records, and sorted-extra-accession SHA-256
  `0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb`.
- M01–M14 in `tests/fixtures/pangolin-compat-v1/cases.jsonl` are independent
  literal sequence expectations. Their member SHA-256 is
  `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`.
- `pangopup-core`, `pangopup-index`, and `pangopup-build` already own public
  provider capabilities, mmap format/validation, and bounded streaming
  FASTA/gzip plus atomic publication respectively. No legacy production
  reference format exists; the invariant protected here is exact bounded
  sequence access without runtime FASTA parsing or a payload-wide open scan.

## Scope

### Included

- A distinct production reference bundle and `PGRREF01` v1 mmap member.
- A typed `ReferenceProvider` in core and production `ReferenceBundleOpen` in
  index.
- An authenticated one-pass full-source builder with integrated private
  certification and atomic publication.
- `pangopup-build reference build|inspect|window` canonical JSON commands.
- A registered 25-sequence synthetic miniature used by all normal tests/specs.
- One retained full production build and exact M01–M14 performance/resource
  qualification.
- Update `README.md`, `architecture/README.md`, `architecture/design.md`,
  `architecture/runtime-data.md`, `architecture/delivery.md`; add
  `architecture/reference.md` and
  `architecture/decisions/0010-production-reference-bundle.md`; update
  `planning/frontier.md`; add
  `planning/artifacts/011-production-reference.md`; add the executable
  reference maintenance spec.

### Excluded

- Pangolin checkpoints, conversion, tensor runtime, CPU inference, or routing.
- GENCODE, gene/exon masking, gffutils, SQLite, or general annotation.
- Reference XDG installation, transport, sync, GitHub release, or publication.
- Changes to the shipped SNV score bundle or main `pangopup` scoring CLI.
- HTTP, Docker, cache, service lifecycle, MPS/CUDA/ONNX, quantization.
- General FASTA service, HGVS, transcript/protein projection, or gene knowledge.
- A reusable exhaustive `reference verify` command or any routine full-data CI.
- Drafting Ticket 012.

## Decisions

### Closed bundle and manifest

The bundle contains exactly three regular non-symlink files:

```text
NOTICE
manifest.json
reference.pgr
```

`manifest.json` is at most `65536` bytes, contains no trailing newline, rejects
duplicate/unknown fields, and is RFC 8785/`serde_jcs` canonical bytes of this
closed shape:

```text
{
  "schema": string,
  "reference_format": string,
  "profile": string,
  "builder": {"version":string,"source_sha256":sha256},
  "source": {
    "assembly":string,"assembly_accession":string,
    "fasta":{"url":string,"compression":string,"bytes":u64,"sha256":sha256},
    "assembly_report":{"url":string,"bytes":u64,"sha256":sha256}
  },
  "sequences": {
    "total_bases":u64,"sequence_set_sha256":sha256,
    "extra_record_count":u64,"extra_accessions_sha256":sha256,
    "ambiguity_runs":u64,
    "aliases":[{"contig":string,"accession":string,"length":u64,
                "ambiguity_runs":u64}]
  },
  "members":[{"path":string,"size":u64,"sha256":sha256,
              "media_type":string}],
  "attribution":{"notice_path":string,"policy_url":string,"transformed":bool}
}
```

All SHA values are `sha256:` plus 64 lowercase hexadecimal digits. Member order
is `NOTICE`, `reference.pgr`; alias order is contig codes 1 through 25. The
canonical manifest SHA-256 is the bundle ID and is not embedded in the
manifest.

Production literals are:

```text
schema             pangopup.reference.bundle.v1
reference_format   pangopup.reference.acgt2-rle.v1
profile            refseq-grch38p14-primary-v1
assembly           GRCh38.p14
assembly_accession GCF_000001405.40
fasta compression  gzip
```

The exact source/report URLs, sizes, hashes, ordered canonical contig/accession/
length table already enforced by the score builder, total/digests/counts above,
and NCBI policy URL are fixed values, not caller declarations.

The sequence-set hash framing remains the independently established grammar:
for each required accession in contig-code order, hash LE `u64` accession byte
length, accession bytes, LE `u64` base length, and uppercase IUPAC bases. The
extra digest hashes sorted accession strings as LE `u64` byte length followed
by bytes. Builder source identity uses the repository's existing length-framed
source identity.

`NOTICE` is at most `16384` bytes and production uses this exact text:

```text
Pangopup RefSeq GRCh38.p14 reference bundle

NCBI RefSeq GRCh38.p14 sequence
Assembly: GRCh38.p14 (GCF_000001405.40)
Source: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz
Assembly report: https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt
Policy and acknowledgment/disclaimer: https://www.ncbi.nlm.nih.gov/home/about/policies/

Pangopup selected the 25 required assembled-molecule sequences (chr1–chr22, chrX, chrY, and non-nuclear chrM), renamed them to canonical chr aliases, uppercased exact IUPAC bases, and encoded them as pangopup.reference.acgt2-rle.v1. Pangopup does not claim a Creative Commons license for NCBI data.
```

### Exact production member

`reference.pgr` uses little-endian integers and this 64-byte header:

```text
0..8    ASCII PGRREF01
8..10   u16 version = 1
10      encoding = 1
11      contig count = 25
12..16  zero
16..24  exact file length
24..32  directory offset = 64
32..40  directory length = 1200
40..48  dense offset = 4096
48..56  ambiguity offset = align8(4096 + sum(ceil(contig_bases/4)))
56..64  total ambiguity-run count
```

Each ordered 48-byte entry is `u8 contig_code`, seven zero bytes, then LE `u64`
base count, dense offset, dense length, ambiguity offset, and ambiguity count.
Bytes `1264..4096` are zero. Dense sections are contiguous in code order with
no inter-contig padding; `dense_length = ceil(bases/4)`. Alignment padding
before the one global ambiguity region is zero. A zero-run entry has ambiguity
offset zero; otherwise its offset is global ambiguity offset plus prior run
counts times 16. Exact file length is ambiguity offset plus total runs times
16. Run count is at most `65536`, run bytes at most `1048576`, and the member is
at most `773124288` bytes.

ACGT uses `A=0,C=1,G=2,T=3`, earliest base in the low pair. Unused high pairs
in each contig's final byte are zero. Ambiguous positions use dense `A/00` and
are restored with 16-byte records: LE `u32` zero-based start, LE `u32` nonzero
length, `u8` code `R/Y/S/W/K/M/B/D/H/V/N = 4..14`, seven zero bytes. Runs are
grouped by contig, sorted, in bounds, nonoverlapping, and adjacent equal-code
runs are rejected rather than normalized by a reader. Code 15 is invalid.

Cheap open validates metadata, directory closure, zero padding, exact bounds,
and the complete bounded ambiguity table before allocating/copying it. It does
not touch dense bytes, validate dense final padding, or hash `reference.pgr`.
Private build certification owns those exhaustive checks.

### Public provider

`pangopup-core` adds:

```rust
pub trait ReferenceProvider: Send + Sync {
    fn copy_window(
        &self,
        contig: Grch38Contig,
        start: GenomicPosition,
        destination: &mut [u8],
    ) -> Result<(), ReferenceError>;

    fn provenance(&self) -> &ReferenceProvenance;
}
```

`ReferenceError` is non-exhaustive with `EmptyWindow`, `OutOfBounds`, and
`CorruptProviderData`. `ReferenceProvenance` has exact accessors for bundle ID,
profile, format, assembly, assembly accession, and sequence-set SHA-256.

`ReferenceBundleOpen` in index owns the sole long-lived read-only mmap and
implements this trait. Coordinates are one-based inclusive; destination length
is the requested length. Empty and checked end-out-of-contig windows fail
without changing destination. Reads return uppercase exact IUPAC and never
truncate, pad, circularly wrap chrM, or reverse complement. Copy performs zero
heap allocations. The mapped regular file must remain immutable and untruncated
for the reader lifetime; mutation is unsupported and truncation may terminate
the process rather than return a typed error.

Bundle alias resolution accepts only bare or `chr` 1–22/X/Y/M plus the exact 25
versioned RefSeq accessions. It rejects `MT`, `chrMT`, lowercase, leading-zero
chromosomes, and versionless accessions. Paths, aliases, manifests, offsets,
and mmap types do not enter the core trait.

### Registered miniature

`pangopup-reference-mini-v1` is a new checked 25-short-sequence profile using
the production container. It is not the prior two-contig benchmark fixture.
Its fixed manifest values include:

```text
assembly             synthetic-mini
assembly_accession   pangopup-reference-mini-v1
FASTA URL            urn:pangopup:fixture:reference-mini-v1:source
report URL           urn:pangopup:fixture:reference-mini-v1:assembly-report
policy URL           https://www.gnu.org/licenses/gpl-3.0.html
contexts_verified    4
```

Its separate exact NOTICE identifies synthetic Pangopup GPL-3.0-only fixture
data and disclaims biological-reference status. Its exact plain FASTA, a
deterministic single-member gzip of the same bases, and report are the only
accepted input identities. Plain/gzip produce identical `reference.pgr` and
logical digest but intentionally different manifest/bundle identity because
observed compression/size/hash differ. The four independent certification
windows cover all-IUPAC, first edge, final edge, and a cross-run/contig case.
Maintenance commands accept this profile; later model admission must require
the exact production profile, format, assembly accession, and sequence digest.

### Integrity policy

No per-page checksum is added: speed is the first priority, and every two-bit
pattern is a valid A/C/G/T base. Current integrated build certification catches
a dense substitution before publication; cheap open/window do not. No
reference installer exists in this ticket. Future transport/install work must
authenticate the manifest-bound member SHA-256 before activation. The tests
must state this boundary by proving a same-size dense bit flip passes cheap
open but fails private certification.

## Maintenance CLI

Exact grammar is:

```text
pangopup-build reference build --profile <EXACT_PROFILE> --source <PATH> \
  --assembly-report <PATH> --output <ABSENT_DIR>
pangopup-build reference inspect --bundle <DIR>
pangopup-build reference window --bundle <DIR> --contig <ALIAS> \
  --start <U32> --length <1..=1048576>
```

Missing, unknown, or duplicate flags; unsupported profile/alias; nonnumeric,
zero, or over-cap numeric values exit 2 with `CLI_USAGE`. A typed valid window
that crosses a contig end is operational `REFERENCE_WINDOW`. Operational
failures exit 1 with one of `REFERENCE_INPUT`, `REFERENCE_BUNDLE`,
`REFERENCE_WINDOW`, `ALREADY_EXISTS`, or `IO`. Every command writes exactly one
JCS JSON object plus LF to stdout, keeps stderr empty, and does not expose a
supplied path, OS error, backtrace, source byte, or partial hash.

Success objects have exactly these fields:

```text
build   {ok:true,command:"reference.build",profile,bundle_id,
         members:[{path,size,sha256}],
         certification:{total_bases,sequence_set_sha256,contexts_verified}}
inspect {ok:true,command:"reference.inspect",profile,format,bundle_id,
         sequences,total_bases,integrity:"structural_only",
         member_sha256_checked:false}
window  {ok:true,command:"reference.window",contig,start,length,bases,
         provenance:{bundle_id,profile,format,assembly,assembly_accession,
                     sequence_set_sha256}}
```

`sequences` is integer 25 for both registered profiles. Failure is exactly
`{ok:false,command,error:{code,message}}`. `inspect` never implies dense-member
integrity. There is no public exhaustive verification command.

## Builder and private certification

- Production profile accepts only the exact pinned single-member gzip/report;
  mini accepts only its two exact source forms/report. Profile selection is the
  required flag and is never inferred from a long scan.
- Open and authenticate held regular non-symlink inputs. Parse the report as
  independent plan authority before FASTA. Bound lines, parser buffers, file
  descriptors, arithmetic, and scratch resources; detect input identity/length
  change while reading.
- Stream FASTA once, select exact accessions wherever they occur, validate and
  skip all 680 extras, uppercase/validate IUPAC, hash the source-side canonical
  stream, write dense bytes directly at precomputed offsets, and spool only
  ambiguity runs. Never materialize/spool the 3.09-billion-base ASCII genome.
- Write header/directory last, sync members and directory, and use private
  same-parent staging plus no-replace rename. An existing output returns
  `ALREADY_EXISTS`; cleanup unpublished staging on handled failure and roll
  back a parent-sync failure.
- Before publish, hash both members, reopen the production provider, decode all
  sequences in bounded chunks, reproduce the independently established count/
  digest, validate canonical dense padding/placeholders, and replay the 14
  compile-time literal contexts (four for mini). Only a certified stage is
  published.

## Red and discriminating controls

Normal gates use only the independently authored mini source/report/windows.
Before the production build exists, controls must prove:

- all 15 IUPAC symbols, lowercase normalization, packed-byte/page boundaries,
  first/final bases, multi-contig isolation, run crossings/coalescing/padding;
- exact accepted/rejected aliases; empty, start/end, overflow, terminal, and
  one-past-contig window behavior with destination unchanged on failure;
- missing/duplicate/wrong-length/malformed report rows, reordered FASTA,
  missing/duplicate records, invalid symbols, changed input, malformed or
  trailing gzip, and extra-record mismatch;
- bad magic/version/encoding/count/file length/reserved metadata, directory
  order/overlap/gaps, dense padding, run codes/length/order/overlap/coalescing,
  truncation and trailing bytes;
- cheap-open instrumentation reads zero dense logical bytes/pages, and window
  reads only its exact dense pages after ambiguity metadata is resident;
- valid dense substitution passes cheap open and fails private certification;
- zero calls/bytes allocated for a warmed copy; open peak outstanding heap at
  most `2097152` bytes excluding mmap/allocator startup; scaled builder peak at
  most `16777216` bytes; `Arc<ReferenceBundleOpen>` concurrent exact reads;
- read-only inputs, private cleanup, existing/concurrent output no-replace,
  parent-sync rollback, canonical JSON, exit codes, empty stderr, path redaction;
- plain/gzip and reordered mini inputs yield the same `reference.pgr` while
  their source-bound manifests differ as designed.

The exact 25-contig offset formula and literal contexts independently predict
that M01–M14 address exactly these eight unique dense logical 4096-byte pages:

```text
31748 109204 119053 119054 133714 133715 152494 152495
```

The per-case page-count sum is 20 and every case touches one or two pages. A
unit test calculates this set without reading candidate payload bytes.

## Acceptance and retained qualification

Focused tests, then normal gates, must remain full-data-free. Once the distinct
code reviewer records `qualification-ready` with no Major finding, the
coordinator launches one full retained job against that exact code/source/
harness identity under:

```text
/home/ian/workspace/data/pangopup-reference-production-011/<contract-id>/
```

`qualification-input.json` is canonical and hashes the release executable,
builder source inventory, production profile, source/report, compatibility
corpus, and benchmark workload; its SHA-256 is `contract-id`. Preflight requires
at least 5 GiB free, absent output, no equivalent active job, and matching
reviewed diff. Retain `command.txt`, PID/session, stdout JSONL, separate empty
application stderr, `/usr/bin/time -v` resource log, progress samples from the
staging member and `/proc/<pid>/{status,io}`, bundle, benchmark report, pinned
Rust-Zstandard measurement, and all relevant identities. The exact launch uses
the release `pangopup-build reference build --profile
refseq-grch38p14-primary-v1` with the pinned absolute input paths and absent
bundle output.

Cancellation sends `TERM` to the process group, waits and records the exit, and
preserves logs/orphan stage. Do not automatically clean/retry. Preserve a
failure and retry only after recording the cause or changing code, assign a new
contract ID, and return any material contract change to the design reviewer.
Never rerun an unchanged successful or failed candidate merely for confidence.

The retained result passes only if:

- exact 25 sequences, `3088286401` bases, 680 extras and both established
  digests reproduce; all 14 literal contexts match;
- the exact Ticket 010 five-round, 20-warmup, 10000-operation M01–M14 workload
  reports headline p50 at most `5586 ns` and p95 at most `6100 ns`;
- every copy allocates zero calls/bytes; production open peak heap is at most
  `2097152`; `reference.pgr` is at most `773124288` bytes;
- open reports zero dense logical reads and the workload reproduces the exact
  eight-page set above;
- the artifact report records actual bundle/member/JCS hashes, source/binary/
  harness identities, installed and measurement-only pinned-Zstandard sizes,
  builder/open heap, RSS, minor/major faults, kernel/CPU/host, and the mmap/RSS
  interpretation.

Missing a threshold leaves the ticket incomplete. Bounded remediation returns
to the same developer/reviewer. A layout, threshold, or other contract change
returns the ticket to proposed and the same design reviewer.

## Dependencies

- Ticket 010 and ADR 0009: complete.
- Exact local source/report and compatibility corpus: present and authenticated.
- No external download, credential, publication, or user decision is required.

## Work ownership

- Pre-existing user changes: none; base was clean and matched `origin/main`.
- Coordinator-authored file: this ticket only before readiness.
- Implementer changes: pending distinct development sub-agent.
- Generated artifacts: checked miniature files in git; full bundle/logs remain
  under the retained data path and never enter git.
- Concurrent unrelated work: none observed.
- Ticket reviewer: `/root/ticket_011_design_review` (read-only; accepted).
- Developer: pending and must differ from reviewer/coordinator.
- Code reviewer: pending and must differ from reviewer/developer/coordinator.

## Independent Ticket Review

Reviewer: `/root/ticket_011_design_review`

Verdict: `ACCEPTED AS READY`; no Major or Minor findings against proposed-file
SHA-256 `c763eab142754e921981943b7c7dc8702f41097146abde08b35e69be12eedffc`
at the named base and dependencies.

The reviewer accepted the fully specified conversational contract after
requiring explicit profiles, exact binary/manifest/CLI contracts, bounded open
memory, provenance profile/format, honest corruption limits, a two-stage code
review around the retained job, size/memory thresholds, and exact production
page calculation. The exact durable file faithfully serialized that contract,
remained reference-only, and introduced no contradiction or scope creep.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

The code review has two recorded verdicts. First, the reviewer must find the
implementation and miniature evidence qualification-ready before the
coordinator starts the retained full job. After the developer records retained
evidence and documentation, the same reviewer reviews the final diff and proof
before acceptance and explicitly answers whether a major separately scoped
issue was revealed.

## External Effect Evidence

Coordinator: not applicable. The retained local build is authorized
implementation evidence, not release publication or deployment.

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Exact mini build/reader/corruption controls | pending focused Rust tests | pending |
| Maintenance JSON behavior | pending `make spec` reference spec | pending |
| Full source correctness and identity | retained qualification | pending |
| Query speed/pages/allocations | retained M01–M14 benchmark | pending |
| Open/builder memory and artifact sizes | tests plus retained resource report | pending |
| Documentation/current-future boundary | named docs plus stale-claim scan | pending |
| Repository gates | `make lint`, `make test`, `make spec` | pending |

## Coordinator Final Check

Coordinator: pending

After final independent approval, inspect the complete diff/artifact inventory,
close this trace, run all three gates, mark this ticket complete, commit/push the
coherent outcome, then remove this live ticket in an immediate cleanup commit
and push. Mark production reference established and leave model/mask packaging
as the next unresolved frontier outcome without drafting another ticket.
