# 011 — Ship the production GRCh38 reference bundle and provider

Status: complete
Accepted v1 format/build contract identity: `sha256:c763eab142754e921981943b7c7dc8702f41097146abde08b35e69be12eedffc`
Replacement v2 lifecycle contract identity: `sha256:df8eb54b29c54b8431fd261e44de3de42c15e1e77aad7d88348b498877d85620`
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

Focused tests and normal gates remain full-data-free. The original v1 contract
authorized exactly one full retained build under:

```text
/home/ian/workspace/data/pangopup-reference-production-011/<contract-id>/
```

That build has already succeeded as contract `1303432912…15b01` after the
required code-review gate. Its canonical input, exact command, PID/progress
checkpoint, stdout, empty stderr, `/usr/bin/time -v` log, heap report, and
atomically published bundle are preserved. It used release `pangopup-build
reference build --profile refseq-grch38p14-primary-v1` with the pinned source
and report and reproduced every full-source identity. These paragraphs are
historical evidence, not authorization to launch another build. Another full
reference build is prohibited.

The v1 cancellation/preservation contract sent `TERM` to the process group,
waited and recorded the exit, and preserved logs or an orphan stage. That rule
was followed. Never rerun that successful build or the failed v1 qualification
candidate merely for confidence.

The combined preserved build plus remaining v2 reuse qualification passes only
if:

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

### Preserved-build qualification replacement

The first retained build succeeded under v1 contract
`1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01`,
but its qualification harness failed before any workload or Zstandard work
because narrow Serde projections rejected unrelated fields in the exact
authenticated corpus. The failure and successful build are immutable retained
evidence. The parser remediation is full-data-free; another full reference
build is prohibited.

The replacement qualification uses a new JCS-canonical input with schema
`pangopup-reference-qualification-reuse-input-v2`. Duplicate and unknown
fields are rejected. Its SHA-256 is the new qualification contract ID. Every
file identity has the existing closed `{bytes:u64,sha256:sha256}` shape. The
closed input shape is:

```text
schema, profile,
source, assembly_report,
prior_build: {
  contract_id, qualification_input, build_command,
  builder_executable, builder_source_sha256,
  builder_stdout, builder_stderr, builder_heap_report,
  builder_resource_log, bundle_id, manifest, notice, reference_member
},
prior_failure: {
  benchmark_executable, harness_source_sha256, qualification_command,
  qualification_report, qualification_stderr, qualification_resource_log
},
replacement: {
  benchmark_executable, harness_source_sha256, source_inventory_sha256,
  corpus, rust_version,
  workload:{rounds,warmups_per_round,operations_per_round,quantile}
}
```

The replacement CLI is exact and maintenance-only:

```text
pangopup-reference-benchmark \
  --reuse-input <canonical-v2.json> \
  --prior-root <retained-v1-root> \
  --corpus <cases.jsonl> \
  --output <absent-v2-contract-root>
```

`replacement.source_inventory_sha256` is the existing length-framed builder
source-inventory contract from `pangopup-build/build.rs`: root `Cargo.toml`,
`Cargo.lock`, root `NOTICE`, the Cargo manifests and all Rust sources in
`pangopup-core`, `pangopup-index`, `pangopup-assets`, and `pangopup-build`.
The running replacement executable's compiled-in inventory identity must equal
that field. It therefore binds the reader, qualification evaluator,
dependencies/lockfile, and harness—not only the benchmark source file.

`prior-root` resolves only the fixed retained paths named by the v1 layout.
Open the absolute root component-by-component as held directory descriptors;
reject symlinked components and use `openat2` with `RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS` where available, or equivalent component-wise
`openat(O_NOFOLLOW)` behavior. Every fixed child is opened relative to those
held descriptors with `O_NOFOLLOW|O_CLOEXEC`, proven regular, and retained for
the operation. The harness never mutates v1 evidence. Before opening/timing the
bundle it must:

Apply the same component-wise no-symlink held-open rule to `--reuse-input`,
`--corpus`, and the `--output` parent. Read the v2 input and corpus from the
same held regular-file descriptors that are authenticated and parsed.

1. Authenticate the prior v1 input as canonical and as contract
   `1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01`,
   and prove that it bound the retained old builder/source/report/corpus/
   workload/benchmark/harness identities.
2. Authenticate the old builder executable and prove the manifest's builder
   source identity is `prior_build.builder_source_sha256`.
3. Authenticate the exact three bundle members, manifest identity, and bundle
   ID against v2 before reader open.
4. Parse canonical build stdout and prove it agrees with the manifest/member
   hashes, 25 sequences, `3088286401` bases, the established logical digest,
   and 14 contexts.
5. Prove builder stderr is empty; authenticate and parse the canonical heap
   report plus `/usr/bin/time -v` resource log; require successful build exit.
6. Prove the prior report is empty; authenticate prior failure command,
   benchmark, harness, stderr, and resource evidence; require stderr to be
   exactly `reference benchmark failed: corpus case\n` and resource exit 1.
7. Authenticate the running executable through a held `/proc/self/exe`
   descriptor, its compiled source-inventory identity, harness source, and the
   exact corpus against the replacement identities. The old failed benchmark
   is evidence only and is never executed again.
8. Run and enforce the unchanged v1 performance, allocation, heap, page,
   member-size, and pinned-Zstandard thresholds.

Small evidence and corpus files are bounded, read exactly once from their held
descriptors into authenticated byte buffers, then parsed from those same
buffers; the harness never hashes one path and reopens it to parse. A
feature-gated qualification reader is constructed from the authenticated held
manifest bytes and held `reference.pgr` descriptor, not from a reopened bundle
path. The descriptor remains live for all timing and pinned-Zstandard work.
Zstandard reads a duplicate of that held descriptor, never the pathname. After
all measurements, re-check the same descriptor's device, inode, size, and
complete SHA-256; any mutation or mismatch fails the run. NOTICE and manifest
receive the same before/after held-identity treatment. This feature-gated
constructor is qualification-only and does not change the normal provider API.

The final report is JCS-canonical, has no trailing newline, and uses the closed
schema `pangopup-reference-production-qualification-reuse-v2`:

```text
schema, qualification_contract_id, prior_build_contract_id, passed,
identities: {
  source, assembly_report,
  prior_build: {
    qualification_input, build_command, builder_executable,
    builder_source_sha256, builder_stdout, builder_stderr,
    builder_heap_report, builder_resource_log, bundle_id,
    manifest, notice, reference_member
  },
  prior_failure: {
    benchmark_executable, harness_source_sha256, qualification_command,
    qualification_report, qualification_stderr, qualification_resource_log
  },
  replacement: {
    benchmark_executable, harness_source_sha256, source_inventory_sha256,
    corpus, rust_version
  }
},
logical, method, performance, storage, resources, host,
mmap_rss_interpretation
```

The nested `logical`, `method`, `performance`, `storage`, `resources`, and
`host` shapes are exactly the v1 `Report` shapes already implemented. Every
field shown is required; duplicate/unknown fields fail deserialization. A
test deserializes the emitted report into this closed type, rejects one
unknown and one duplicate field at each new nesting level, reserializes with
JCS, and requires byte equality. `qualification_contract_id`,
`prior_build_contract_id`, `prior_build.bundle_id`, every scalar source/
harness/inventory digest, and every `Identity.sha256` are explicitly
`sha256:` plus 64 lowercase hex digits.

The final `--output` path must have a basename equal to the v2 contract digest
without the `sha256:` prefix, its parent must exist, and both it and the report
must be absent. The harness creates a mode-0700 private same-parent stage named
`.<contract-id>.qualification-stage-<pid>-<counter>`. It retains the canonical
v2 input, canonical command JSON, copied replacement executable/source,
canonical resource JSON, empty stderr, and final report in that stage. On
success it fsyncs every file and the stage, publishes the stage to the exact
contract root with `renameat2(RENAME_NOREPLACE)` (or equivalent atomic
no-replace), then fsyncs the parent. Existing or concurrent output is an error.

On failure the harness writes canonical failure evidence and publishes the
stage without replacement as
`.<contract-id>.qualification-failed-<pid>-<counter>`, syncs the parent, and
leaves the successful contract root absent. An injected publication test proves
concurrent no-replace, success sync, failure preservation, and that v1 root
bytes are unchanged. The successful v1 bundle/build evidence remains only in
its original root and is referenced by identity; v2 never copies or rewrites
the 772 MB member. Normal tests use only miniature evidence.

The successful root contains exactly these seven regular, non-symlink files
and the one `candidate/` directory; no other entry is permitted:

```text
reuse-input.json
command.json
qualification-report.json
qualification-resource.json
qualification.stderr.log
candidate/pangopup-reference-benchmark
candidate/pangopup-reference-benchmark.rs
```

`qualification.stderr.log` is zero bytes. `reuse-input.json` is the exact v2
input. The executable is mode `0555`; other files are `0444` after staging.
`command.json` is canonical closed schema
`pangopup-reference-qualification-command-v2` with required fields
`working_directory:string` and `argv:[string;9]`; argv is exactly the program
plus the four ordered flag/value pairs shown in the replacement CLI.
`qualification-resource.json` is canonical closed schema
`pangopup-reference-qualification-resource-v2` with required unsigned integer
fields `started_unix_seconds`, `elapsed_ns`, `maximum_rss_bytes`,
`minor_faults`, `major_faults`, `user_cpu_ns`, and `system_cpu_ns`, plus
`outcome`, whose closed values are
`"qualification-passed-before-publication"|"preflight-failed"|
"measurement-failed"|"publication-failed"`. A success root requires the
first value; a failure root requires the value corresponding to its phase.

The final report's top-level `identities` additionally contains this exact
closed field:

```text
retained: {
  reuse_input, command, resource, stderr,
  benchmark_executable, harness_source
}
```

Each value is an `Identity`, and together they bind every retained success
member except the report that contains them. They must agree with the v2
contract/current executable/source identities where duplicated. A test rejects
an extra/missing root entry, wrong mode/type, and an identity mismatch.

A failure root contains exactly the same seven paths plus `failure.json`;
`qualification-report.json` is either zero bytes or the complete canonical
passed-but-unpublished report if only final publication failed.
`qualification.stderr.log` contains exactly one redacted stable error line.
`failure.json` is canonical closed schema
`pangopup-reference-qualification-failure-v2` with required fields:

```text
qualification_contract_id, prior_build_contract_id,
phase:"preflight"|"measurement"|"publication",
code:string, message:string,
report_state:"empty"|"complete_unpublished",
evidence: {
  reuse_input, command, resource, stderr, report,
  benchmark_executable, harness_source
}
```

Every evidence value is an `Identity` and binds every other failure-root file.
The code/message are path-redacted and bounded to 64/256 ASCII bytes. Failure
publication first fsyncs the evidence and stage, atomically publishes without
replacement to the failure name, and fsyncs the parent. If even failure
publication cannot complete, the private stage remains preserved and the
process reports its exact path; it is never deleted or relabeled as success.

The known retained v1 identities that the v2 input must bind are:

```text
qualification-input.json 1029 1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01
command.txt 1409 6c43e18dc6c4ca9839a3a72c5ed2f0d365cadc6a53e8442b9ef2027add54600e
candidate/pangopup-build 5167976 5d503b9dc8e9968e83d7657ac9cf617474c854d13a8f756803757aaab33ed7dc
builder.stdout.jsonl 577 d3a3aa536452484b911f0f64fdf4794c1595963e52810deabbd308604cbd45c2
builder.stderr.log 0 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
builder-heap-report.json 118 eb035a24de0f1a8bc23e316c06be3529803a30e8dfac284f21f71ed5cfd00204
builder-resource.log 1641 0fbafdceb9541fc866fdb55a51c6b35b14b341df157544d1a2c4da68f53622b3
bundle/manifest.json 3719 7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f
bundle/NOTICE 793 1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499
bundle/reference.pgr 772091760 cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82
candidate/pangopup-reference-benchmark 1777632 11adc18be7ee659b08ab79a62d83102ec87585e33d38db75054ac0dc25a71072
candidate/pangopup-reference-benchmark.rs 26804 e359876ab8e7a8b06761ec03c3df4f9bbe60f427db7f8c85c0374c007c2b0a21
qualification-command.txt 1598 4641bb9420eca169d60fd89a6afe3c7f32dedc594a04baf76c549a1d7519ef38
qualification-report.json 0 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
qualification.stderr.log 40 0a4dbf073617ad47c4a525e31559a8c240296259b70e8b858080562008dc9cb2
qualification-resource.log 1823 c5d8349995dc2099104c8dd597fe0c766e7b1f8e4404be7424d7c5b099d07256
```

All hashes in the v2 JSON carry the `sha256:` prefix. The v2 input also binds
the already-pinned source/report identities and prior builder-source digest
`2215dabe7c5e81bde9254a7aa8979c78322647d6a5de803976266d9422ea1d8f`.
Only the same design reviewer may accept this revised lifecycle contract; then
the same developer implements it and the same code reviewer must return
qualification-ready before the replacement run.

## Dependencies

- Ticket 010 and ADR 0009: complete.
- Exact local source/report and compatibility corpus: present and authenticated.
- No external download, credential, publication, or user decision is required.

## Work ownership

- Pre-existing user changes: none; base was clean and matched `origin/main`.
- Coordinator-authored file: this ticket only before readiness.
- Implementer changes: v1 implementation, parser remediation, and bounded v2
  reuse implementation complete by the same developer against the accepted
  `df8eb54b…85620` lifecycle contract; awaiting adversarial code review.
- Generated artifacts: checked miniature files in git; full bundle/logs remain
  under the retained data path and never enter git.
- Concurrent unrelated work: none observed.
- Ticket reviewer: `/root/ticket_011_design_review` (read-only; v1 and accepted
  v2 lifecycle contract reviewed).
- Developer: `/root/ticket_011_developer` (v1/parser/v2 implementation complete;
  distinct from reviewer/coordinator).
- Code reviewer: `/root/ticket_011_code_review` (read-only; distinct from
  reviewer/developer/coordinator).

## Independent Ticket Review

Reviewer: `/root/ticket_011_design_review`

V1 verdict: `ACCEPTED AS READY`; no Major or Minor findings against proposed-file
SHA-256 `c763eab142754e921981943b7c7dc8702f41097146abde08b35e69be12eedffc`
at the named base and dependencies.

The reviewer accepted the fully specified conversational contract after
requiring explicit profiles, exact binary/manifest/CLI contracts, bounded open
memory, provenance profile/format, honest corruption limits, a two-stage code
review around the retained job, size/memory thresholds, and exact production
page calculation. The exact durable file faithfully serialized that contract,
remained reference-only, and introduced no contradiction or scope creep.

V2 lifecycle review history: the first v2 proposal at SHA-256
`d5e430106c5e7f200ab464cee64af97629e67e8bea24c19ef6dd5dcbe352b8f9`
was `REJECT` with six Major findings: stale full-build authorization, missing
replacement source-inventory binding, hash/reopen substitution windows,
unclosed report shape, stale review identity, and unprotected v2 output
publication. The reviewer again explicitly prohibited another full reference
build. The revised proposal closes those findings above and awaits the same
reviewer; no v2 implementation or run is authorized until acceptance.

The second v2 proposal at SHA-256
`7bd7c444d48cd2df9c964fe183f9ceea52f3b037c42d8ef93de43c7c24f74e77`
was also `REJECT`: one historical code-review paragraph still authorized the
old full job, and success/failure evidence filenames and JSON schemas were not
closed. Those gaps are now replaced by the explicit current path and exact
member/schema/receipt contract above. The reviewer otherwise accepted the
source inventory, held-descriptor, same-byte, revalidation, report, and
no-replace design.

V2 lifecycle verdict: `ACCEPTED AS READY` with no Major or Minor findings
against proposed-file SHA-256
`df8eb54b29c54b8431fd261e44de3de42c15e1e77aad7d88348b498877d85620`.
The same reviewer found the reuse path decision-complete, identity-bound,
TOCTOU-resistant, atomically preserved, and implementable without invented
policy. Another full reference build is prohibited; only the v2 reuse
qualification may run after implementation and code-review approval.

## Implementation Evidence

Developer: `/root/ticket_011_developer`

Implemented the accepted production format and provider boundary in
`pangopup-core`/`pangopup-index`, the authenticated streaming builder and
private certifier in `pangopup-build`, the exact maintenance CLI, the retained
qualification harness, registered miniature inputs, executable specs, and the
named architecture/user/planning documentation. The builder compiles and
identity-checks the 220,071-byte compatibility corpus, so production
certification does not depend on a runtime source checkout. No production
FASTA build had started at the initial implementation handoff. The later single
reviewer-authorized build is preserved under the exact contract recorded
below; it has not been repeated.

Exact current-candidate focused evidence:

- `cargo clippy --locked -p pangopup-core -p pangopup-index -p pangopup-build
  --all-targets --all-features -- -D warnings` — passed after remediation.
- `cargo test --locked -p pangopup-index --lib --all-features` — 18 passed,
  including an actual packed decode spanning a 4096-byte mmap page.
- `cargo test --locked -p pangopup-build --lib --test reference --test
  reference_resources --all-features` — 36 passed (22 library, 13 reference,
  1 resource). This covers exact mini provider/builds,
  plain/gzip payload equivalence, strict aliases and provenance, bounds without
  destination mutation, zero dense reads on open, exact packed reads,
  the closed structural corruption matrix, dense-substitution integrity
  separation, held no-follow input authentication and bounds, read-only input,
  concurrent no-replace, durable rollback, zero-copy allocations, heap
  ceilings/reporting, qualification threshold failures, maintenance lookup,
  and the independent production page prediction.
- `cargo test --locked -p pangopup-build --bin
  pangopup-reference-benchmark --features reference-qualification` — 2 passed;
  the exact authenticated corpus produces 14 projected contexts, unrelated
  fields are accepted, and missing/mistyped consumed fields are rejected.
- `PATH="$PWD/target/debug:$PATH" mustmatch test spec/reference.md` after the
  normal debug build — 20 passed, including the complete grammar/redaction
  additions.
- `git diff --check` — passed.

The accepted v2 reuse path is now implemented without touching or copying the
retained production bundle. The exact four-flag maintenance CLI validates the
closed canonical reuse contract and known v1 identities; opens input, corpus,
prior-root components, fixed evidence, and output parent without following
symlinks; parses the same authenticated buffers; constructs the qualification
reader and Zstandard stream from held descriptors; rechecks device/inode/size
and full SHA-256 after measurement; and atomically publishes only small synced
success/failure evidence without replacement. The running executable binds the
complete compiled builder source inventory.

Final v2 developer evidence before code review:

- all-feature Clippy over core/index/build and all targets — passed;
- index library — 18 passed;
- build library/reference/resource suites — 37 passed, including a held
  qualification reader that survives complete bundle-path substitution;
- feature-gated replacement benchmark — 17 passed, covering the exact corpus,
  closed canonical report/failure receipts, held-descriptor substitution and
  mutation, immutable read-only evidence, exact resource/phase wire values,
  exact success members/modes, concurrent no-replace, failure preservation,
  compiled inventory binding, read-once semantic buffers, total late-failure
  sealing, exact preserved-stage reporting, success-report byte authentication,
  descriptor-relative failure repair through concurrent path substitution, and
  direct success/failure inventory/type/mode/every-identity negative controls;
- executable reference spec — 20 passed;
- `git diff --check` — passed.

Neither the v1 nor v2 production qualification was run. The retained v1 root
remained read-only, and no full build was started.

Before the final portability-only corpus embedding and failed-build negative
control, the full-data-free workspace gates also passed: `make lint`, `make
test`, and `make spec` (`131 passed`), with the standalone reference spec at `8
passed`. Those are provisional implementation evidence; repository policy
requires the coordinator to rerun the exact final gates only after independent
code review.

## Adversarial Code Review

Reviewer: `/root/ticket_011_code_review`

First v2 implementation review verdict: `REJECT`; no separately scoped major
issue was revealed. The reviewer returned four bounded findings, now
remediated for re-review:

1. **Total failure lifecycle.** Every operation after private-stage creation is
   now inside one result boundary. Initial retention, preflight, measurement,
   descriptor revalidation, resource/report work, stage validation, and
   success no-replace errors all enter one repair/seal/publish function. If
   that function cannot publish failure evidence, it returns
   `STAGE_PRESERVED` with the exact private stage path. Six injected late-point
   controls plus failure-name exhaustion and actual success no-replace prove
   the funnel.
2. **Read-once small evidence.** Every bounded small retained file and the
   corpus now own the single buffer read for authentication. All semantic
   checks borrow that buffer; held descriptors remain for the required final
   metadata/full-hash pass. A descriptor-pass counter proves five repeated
   semantic parses perform one read pass.
3. **Publication negatives.** Direct success and failure validators now reject
   extra/missing members, wrong type, wrong mode, and every retained/evidence
   identity mismatch. The failure validator also proves exact modes, canonical
   receipt/resource/report state, stderr, and phase/outcome agreement.
4. **Planning overclaim.** `planning/frontier.md` now distinguishes the
   established full build from still-pending query/open/Zstandard
   qualification; the artifact and this ticket retain the same boundary.

Remediation focused evidence: all-feature Clippy passed; index library 18;
build library/reference/resource 37; benchmark 15; reference spec 20; diff
check passed. Neither production qualification path ran and retained v1
evidence was not modified.

Second v2 implementation re-review verdict: `REJECT`; accepted policy and
scope remain unchanged. The reviewer returned two additional bounded findings,
now remediated for the same reviewer:

1. **Success report authentication.** Success validation now opens
   `qualification-report.json` relative to the held stage descriptor, reads one
   authenticated buffer, requires a closed canonical parse, and requires those
   exact bytes to equal the canonical in-memory report before the single
   validate-and-publish path may rename the stage. Direct changed, truncated,
   pretty/noncanonical, duplicate-key, and unknown-field controls prove no
   success root is published.
2. **Descriptor-relative failure repair.** Failure reset no longer enumerates
   or deletes through mutable `stage.path`. It inventories held directory
   descriptors with `rustix::fs::Dir`, opens child/candidate directories with
   `O_NOFOLLOW`, binds device/inode, and removes members only with relative
   `unlinkat`. Before publication or preserved-path reporting, the stage's
   current parent entry is rebound from the held parent descriptor and held
   stage device/inode. Concurrent rename/substitution controls prove the
   substitute tree remains byte-for-byte untouched, a successful failure root
   reports its actual published path, and exhausted failure publication reports
   the actual preserved renamed path.

Second remediation focused evidence: all-feature Clippy passed; index library
18; build library/reference/resource 37; benchmark 17; reference spec 20; diff
check passed. No production or full-build path ran, and retained v1 evidence
was not modified.

Final v2 qualification-readiness verdict: `QUALIFICATION-READY`; no Major or
Minor findings remain and no separately scoped major issue was revealed. The
same reviewer reran the 17 feature-gated benchmark tests, feature-gated Clippy,
and `git diff --check`; accepted the 3,000-plus-line binary as cohesive for this
one-shot maintenance purpose; and authorized exactly one v2 preserved-build
reuse qualification. Another full reference build remains prohibited.

That authorized candidate, contract
`9f22e22e41972db56fbca6d8cbf6367ef756c355f0e3dcec783e11a99660cc8b`,
ran exactly once and failed closed during measurement with code `THRESHOLD`.
Its exact immutable failure root is
`.9f22e22e41972db56fbca6d8cbf6367ef756c355f0e3dcec783e11a99660cc8b.qualification-failed-2232586-0`.
The resource receipt records 17,009,988,205 elapsed ns and 20,766,720 maximum
RSS bytes, but the old failure message was only `qualification threshold
failed`; the precise failed class and observed value were discarded. Therefore
no query, open, allocation, page, storage, or threshold performance result may
be inferred from that run, and neither it nor another candidate is authorized
to run merely to diagnose it.

Bounded post-qualification remediation preserves every predicate, limit,
evaluation order, and failure schema. The evaluator now returns a structured
first-failure class and one stable ASCII message of at most 256 bytes. Stable
order is logical sequence, logical extras, latency, allocations, heap, storage,
then pages. Each message carries all observed/limit values for its compound
class; logical identity text is exact when valid and safely replaced by
length/content digest when malformed, while an oversized hostile page vector
is represented by length and its exact u64-big-endian content digest. This
fully identifies the first actionable blocker from one candidate without a
diagnostic rerun and without adding a report to measurement failures. At that
boundary, the next observable candidate remained pending the same review and
explicit coordinator authorization; its later run is recorded below.

The same reviewer rejected the first observability implementation because its
256-byte guarantee relied on `debug_assert!` and was false for maximal public
`u64` values combined with an invalid identity. The repaired wire grammar uses
compact unambiguous logical tokens:

- `logical-sequence b=<observed>/<limit> s=<observed>/<limit> c=<observed>/<limit>`;
- `logical-extras n=<observed>/<limit> s=<observed>/<limit>`.

Every ordinary class message is construction-bounded across the full public
measurement domain. The common constructor also enforces the invariant in
release builds: any unexpectedly non-ASCII or overlong formatted diagnostic
becomes `<class> diagnostic_len=<bytes> diagnostic_sha256=sha256:<digest>`.
This is a content identity, not ambiguous truncation. Exact maximal-`u64`,
large invalid-identity, 4,096-value page-vector, and forced constructor-fallback
controls prove every evaluator rejection is ASCII and at most 256 bytes; a
benchmark control publishes the maximal logical diagnostic through the closed
failure schema with an empty report.

Construction-bound remediation evidence: all-feature Clippy passed; index
library 18; build library/reference/resource 39; feature-gated benchmark 18;
reference spec 20; diff check passed. No qualification, production build,
candidate run, or other production-data path ran, and neither retained root
was modified.

Observable-replacement qualification-readiness verdict:
`QUALIFICATION-READY`; no Major or Minor finding and no separately scoped
major issue. The same reviewer confirmed construction-enforced ASCII/256-byte
diagnostics across the full public measurement domain, unchanged predicates,
limits, and first-failure order, and successful maximal-message failure
sealing. Exactly one new observable v2 reuse candidate is authorized. Another
full reference build remains prohibited.

That observable candidate, contract
`09ea27dde9169eb214b6d3a7abc1de7b139c1883fb0112573959a8a56be88887`,
ran exactly once and failed closed on the first actionable class, latency. Its
exact immutable failure root is
`.09ea27dde9169eb214b6d3a7abc1de7b139c1883fb0112573959a8a56be88887.qualification-failed-2390227-0`.
The closed receipt records p50 `7485/5586 ns` and p95 `11984/6100 ns`; because
latency is evaluated before allocations, heap, storage, and pages, no result
for any later class may be inferred. The v1 format, production member, and
accepted limits remain unchanged.

The failed run does not retain adequate environment evidence to compare its
timings directly with Ticket 010. Ticket 010's selected result ran on CPU 0 on
an AMD Ryzen 7 5825U under the `powersave` governor and Linux
`6.17.0-35-generic`; the v2 receipt retains neither an affinity assertion nor
host/governor/kernel evidence. The `5586/6100 ns` limits are arithmetically 25%
above Ticket 010's `4469/4880 ns` headline (with the fractional p50 reduced to
the recorded integer); that is an inference about the limit choice, not proof
that the measurements are comparable. The workloads both use five rounds, 20 warmups,
10,000 cyclic M01–M14 operations, nearest-rank per-round quantiles, and median
round headlines. V2 additionally verifies all 14 cases before every round,
where Ticket 010 performed its exactness preflight before the retained timing
rounds. That difference warms relevant pages and cannot justify claiming that
the v2 failure measures a worse cold path. Any future coordinator-authorized
candidate must pin CPU 0 and durably record affinity plus CPU, governor, and
kernel evidence before interpreting a timing comparison.

There is nevertheless one concrete code regression independent of those
environment limits. Ticket 010's selected decoder handled an unaligned prefix,
copied four decoded bases from a lookup table for every aligned packed byte,
then handled the tail. Production `decode_packed` instead repeated checked
position arithmetic, division, bounds lookup, and table selection for every
base in every roughly 10.1-kilobase M context. Qualification's
`test-read-audit` feature also performed a thread-local counter update for
every timed query even though the harness consumed that counter only after
open.

The bounded remediation ports the selected aligned decoder without changing
the bytes or provider API and prevalidates all input before writing so corrupt
data preserves the caller's destination. Dense-read evidence is now an
explicit constructor result derived from the constructor's mmap ranges; normal
and timed `copy_window` execute the same production path without audit work.
Tests compare the optimized decoder directly with the old scalar reference for
every start across a 1,024-base bounded vector and lengths 1–33, explicitly
cover all four start alignments, short/group/tail shapes, a packed 4,096-byte
page crossing, ambiguity overlap shapes, corrupt/bounds preservation,
concurrent copies, and zero allocation. These are correctness and regression
controls, not performance evidence. At that boundary no performance
improvement was claimed; the later retained qualification below supplies the
acceptance evidence.

Decoder/audit remediation focused evidence: all-feature Clippy passed; index
library 22; build library/reference/resource 40 (24/15/1); feature-gated
benchmark 18; executable reference spec 20; formatting and diff checks passed.
No qualification, candidate, production build, or benchmark workload ran, and
the retained production member and both failure roots were not modified.

The same reviewer then returned `QUALIFICATION-READY` for exactly one
optimized, CPU0-pinned preserved-build reuse candidate. Contract
`edaee0374147b70a5259a4a2f0f4120914232f018531391e9625438fd2db35af`
ran once and passed every unchanged v2 predicate. Its exact retained success
root is
`/home/ian/workspace/data/pangopup-reference-production-011/edaee0374147b70a5259a4a2f0f4120914232f018531391e9625438fd2db35af/`;
launcher stdout and stderr are both empty. The canonical report records:

- latency p50/p95 `1864/2084 ns`, below `5586/6100 ns`;
- zero allocation calls and zero allocation bytes;
- open peak Rust heap `16060` bytes and zero dense bytes read during open;
- benchmark maximum RSS `20938752` bytes and minor/major faults `4738/0`;
- installed bundle `772096272` bytes, including the unchanged `772091760`-byte
  reference member;
- pinned Rust-Zstandard size `656781805` bytes;
- the exact eight dense pages, per-case page-count sum 20, 25 contigs,
  `3088286401` bases, 14 contexts, 680 ignored records, and the unchanged
  sequence/extra-accession digests.

The retained v1 build and both immutable failure roots remain preserved. No
second full reference build occurred. This success qualifies the existing v1
format and provider; it does not change the format, asset bytes, thresholds, or
the separate model/mask/transport scope.

Final gate review then returned one bounded Major finding: the Ticket 011
compiled source-inventory change made the checked 1,000-request SNV regression
fixture non-reproducible. A fresh deterministic generation differed in exactly
10 identity-bearing files and nowhere else. The builder source identity changed
from `a2c05be644b2bc36d10257ee848c5b556b6f9a217e177a6edc4b7220ebdb9133`
to the qualified
`eac8d37ea69d8680d9a834dc4848382a4257f00a668a6c6cdcbc58db71fe81c7`;
the fixture bundle ID consequently changed from
`3126d856f11f1715a0246f3b953a89408e0de7c2fbec825832ac638194463275`
to `cf0ed402d0a6c84241f87d9e77270fd8b4850f6b19a3523670b044749127c598`.

Remediation used the fixture's documented deterministic generator with the
checked `pangolin-precompute` source and a fresh absent output under `/tmp`.
Complete file-set and byte comparison proved that payload, requests, source,
reference, and NOTICE bytes were identical. Removing only
`builder.source_sha256` made the old/new manifests identical; substituting only
the old bundle ID with the new ID made every other drifted file identical. The
checked fixture replaced only:

- `README.md` and `bundle/manifest.json`;
- `expected.jsonl` and `expected/unfiltered.jsonl`;
- `expected/ENSG00000010610.jsonl`, `expected/ENSG00000141499.jsonl`,
  `expected/ENSG00000141510.jsonl`, `expected/ENSG00000169129.jsonl`,
  `expected/ENSG00000175727.jsonl`, and
  `expected/ENSG00000185974.jsonl`.

The full checked/generated trees then compared byte-exactly. Production
reference data and retained qualification evidence were not touched, and no
production build, qualification, or benchmark ran. Exact remediation evidence:

- `cargo run --locked --package pangopup-build --bin
  pangopup-regression-fixture -- tests/fixtures/pangolin-precompute
  /tmp/pangopup-ticket011-regression.7GPFBx/generated` — passed into an absent
  output;
- complete inventory plus byte comparison — exactly the 10 files above
  differed before replacement and the complete trees were byte-exact after;
- `cargo test --locked -p pangopup-build --test snv_regression_fixture` — 1
  passed;
- `cargo test --locked -p pangopup-cli --test snv_regression` — 3 passed,
  including all 1,000 provider expectations and all seven CLI batches;
- `git diff --check` — passed.

The same reviewer rechecked this bounded remediation and the complete diff,
then returned final `ACCEPT`: no Major or Minor findings remained and no major
issue was revealed for a separately scoped future ticket.

Optimized-decoder qualification-readiness verdict: `QUALIFICATION-READY`; no
Major or Minor finding and no separately scoped major issue. The same reviewer
verified upfront corruption safety, exact Ticket 010 prefix/four-base/tail
semantics, unchanged ambiguity/API/format behavior, the normal unaudited query
path, constructor-only audit evidence, exhaustive equivalence/page/ambiguity/
concurrency/zero-allocation controls, and honest documentation. Exactly one
new CPU-0-pinned observable reuse candidate is authorized; another full build
remains prohibited.

First qualification-readiness verdict: `REJECT`; no separately scoped major
issue was revealed. The reviewer reported three Major and two Minor findings:

1. **Major — missing discriminating controls.** Remediated inside Ticket 011
   with generic report/FASTA malformed, duplicate, missing, reordered, and
   resource-limit controls; malformed/trailing gzip identity rejection;
   read-only/symlink inputs; concurrent no-replace; injected rollback; expanded
   header/directory/run/truncation corruption matrix; an actual packed mmap
   page crossing; and full maintenance CLI grammar/redaction specs.
2. **Major — parse before authentication and unbounded hostile gzip work.**
   Remediated by no-follow held descriptors, full byte authentication before
   decompression, post-use descriptor identity checks, and registered decoded
   byte/record/accession/line/run ceilings. Sets/vectors cannot grow beyond the
   profile record ceiling.
3. **Major — insufficient retained qualification evidence.** Remediated with
   opt-in canonical builder heap evidence and a feature-gated harness that
   authenticates the canonical contract, binaries, source inventories,
   source/report, and corpus, then records actual logs and bundle members in
   its output report; measures open heap/dense reads,
   allocations, pages, RSS/faults, installed and pinned Rust-Zstandard sizes;
   records host and mmap/RSS interpretation; and refuses every failed accepted
   threshold through one tested evaluator.
4. **Minor — no-follow race.** Closed by opening source/report with
   `O_NOFOLLOW|O_CLOEXEC`, retaining the descriptors, and checking identity
   before and after use.
5. **Minor — rollback not durably re-synced.** Closed by syncing the parent
   again after removal; an injected first-sync failure proves two sync calls
   and absence of both stage and output.

Initial qualification-readiness re-review verdict: `QUALIFICATION-READY`; no Major or
Minor findings remain and no major separately scoped issue was revealed. The
reviewer reran all-feature Clippy, the 18 index tests, the 36 build/reference/
resource tests, a release all-bin feature build, and `git diff --check`, then
authorized the single identity-bound retained full build without relaxing the
one-run preservation rules.

The retained build then succeeded, but qualification failed immediately on the
exact-corpus projection bug described above. After bounded parser remediation,
the same reviewer returned `PARSER-READY`, found no separately scoped issue,
prohibited another full build, and required the reuse-specific v2 input above.
Because that is a material qualification lifecycle change, the ticket returned
to `proposed` for the same design reviewer before v2 implementation.

The authorized production build under contract
`1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01`
succeeded and remains preserved. The separate qualification harness then
failed before workload execution with `reference benchmark failed: corpus
case`: after authenticating the exact pinned corpus, its consumed-field serde
projections rejected unrelated real-case fields. No timing, Zstandard, or
threshold result was produced, and the old harness identity must not be rerun.

Bounded disposition: remove unknown-field denial only from the three narrow
case projections; serde still requires and types every consumed field and the
outer reader still requires the exact corpus byte length/SHA before projection.
The benchmark binary test now invokes that real authenticated path over the
checked-in corpus, proves 14 M contexts and representative exact fields, and
separately rejects missing `contig` plus wrong `start_1based`/`bases` types.

Replacement-identity analysis is recorded in the artifact. The preserved
build can technically be reused without weakening its source, manifest, or
builder checks. But v1 qualification input does not pre-bind the exact old
bundle/log evidence; those hashes currently enter only the output report. An
honest preferred no-rebuild replacement therefore requires reviewer-approved
v2 input fields for the old contract ID, manifest/bundle/members, build stdout,
heap/resource logs, and old builder identity, together with the repaired new
benchmark/harness identity. No replacement run is authorized under v1.

Historically, the reviewer found the v1 implementation and miniature evidence
qualification-ready before the one retained full build; that build is complete
and must not recur. The optimized v2 preserved-build reuse qualification is now
green. The same reviewer accepted the final diff and retained proof with no
Major or Minor findings and reported no separately scoped major issue. The
already-planned model/checkpoint and GENCODE mask packaging outcome remains the
next frontier item; it was not a defect discovered by this review.

## External Effect Evidence

Coordinator: not applicable. The retained local build is authorized
implementation evidence, not release publication or deployment.

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Exact mini build/reader/corruption controls | focused index/build/benchmark/reference/resource Rust tests | aligned decoder equivalence and focused reader controls pass: index 22, build/reference/resource 40, benchmark 18 |
| Maintenance JSON behavior | `spec/reference.md` | 20 passed on remediated candidate |
| Full source correctness and identity | retained build contract `1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01` | passed: 25 sequences / 3,088,286,401 bases / 14 contexts / exact logical digest |
| Query speed/pages/allocations | retained M01–M14 benchmark | optimized contract `edaee0…35af` passed: p50/p95 `1864/2084 ns`, zero allocations, exact eight pages / sum 20 |
| Open/builder memory and artifact sizes | retained build and optimized v2 report | passed: builder heap 1,201,871; open heap 16,060; dense-open reads 0; member 772,091,760; installed 772,096,272; pinned Zstandard 656,781,805 bytes |
| Documentation/current-future boundary | named docs plus stale-claim scan | retained success recorded; independent review accepted with no findings |
| Repository gates | `make lint`, `make test`, `make spec` | final coordinator rerun passed; executable specs 143 passed |

## Coordinator Final Check

Coordinator: complete. The complete diff and retained artifact inventory were
audited after independent acceptance. The final coordinator gate passed
`make lint`, `make test`, and `make spec` (`143 passed`), followed by
`git diff --check`. The retained production member remains unchanged and no
second full build or additional qualification run occurred. Production
reference is established; model/checkpoint plus GENCODE mask packaging remains
the next unresolved frontier outcome, with no Ticket 012 drafted here.
