# Ticket 045 Apple Silicon retest

This is a compact record of the Mac retest supplied to the PangoPup project.
The raw evidence was produced and retained on the test Mac; it was not opened
independently from this Linux checkout.

## Qualified build

- Commit: `e5d7d1ae788ac9e1c6c194c60b832ba8837b238f`.
- Host: Apple M5 Max, macOS 26.4 build 25E246, `arm64`.
- Runtime: Docker Desktop 4.73.0 with Docker Engine 29.4.3 and a native
  Linux/ARM64 server.
- Image: `linux/arm64`, 23,380,620 bytes, with the exact commit in its revision
  label.

## Ticket 043–045 results

- Focused `-h` and `--help` passed for all runtime leaves and asset namespaces
  without an asset mount, network request, or bound service port.
- The documented read-only-data `status` command returned ready twice. A
  fingerprint over every durable file's path, size, and modification time was
  unchanged before and after both observations.
- A `sync --progress` run was deliberately interrupted after visible progress.
  The safe partial reached 263,438,336 bytes. The identical command resumed
  from exactly that offset and completed.
- The completed invocation reported 2,360,124,034 downloaded bytes and
  263,438,336 resumed bytes. Forty-eight sampled aggregate progress counters
  were monotonic and their final totals matched the final JSON.
- `--quiet`, network-disabled `--offline --progress`, the conflicting
  `--progress --quiet` error, and byte-compatible final JSON all behaved as
  specified.
- Network-disabled SNV lookup retained precomputed provenance. A non-SNV model
  call, HTTP live/ready/status, SNV scoring, cached model scoring, graceful
  stop, and persistent named volumes all passed.

The test directly proved cross-process/container resume. It did **not**
deliberately induce an automatic within-process transient retry; that policy
remains covered by repository tests rather than this Mac exercise.

## Retained limitation

ONNX Runtime 1.24.2 emitted `Unknown CPU vendor. cpuinfo_vendor value: 0` on
Apple Silicon for help, status, sync, precomputed lookup, and service startup.
The warning did not change observed results, but polluted stderr even on paths
that did not need model inference.

## Raw evidence

The complete report and command evidence remain on the Mac at:

`/Users/ian/Desktop/pangopup-mac-evidence-ticket045`

No repository files, image publication, or volume deletion occurred during the
retest. All four pre-existing and retest data/cache volumes were retained.
