#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
root=$repo/target/production-release-qualification-test
chmod -R u+w "$root" 2>/dev/null || true
rm -rf "$root"
install -d -m 700 "$root/bin"

cat >"$root/bin/pangopup" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

# The model oracles beside this harness carry scoring bytes alone. A real
# release names every accession the shipped gene-name index reaches, so the
# stub adds the naming leaf the checker must take back out. Records only: a
# model oracle carries no source-reference ambiguity to name. The SNV groups
# below no longer come through here. The built executable renders them, so the
# checker sees a release's own leaves on records and on ambiguities.
name_records() {
  python3 "$QUALIFICATION_NAME_RECORDS"
}

command=$1
shift
if [[ "${1:-}" == --help ]]; then
  case "$command" in
    sync) printf '%s\n' 'Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' ;;
    status) printf '%s\n' 'Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]' ;;
    lookup) printf '%s\n' 'Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant <GRCh38-VARIANT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' ;;
    serve) printf '%s\n' 'Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' ;;
    *) exit 2 ;;
  esac
  exit
fi
case "$command" in
  sync)
    if [[ " $* " == *' --offline '* ]]; then
      if [[ " $* " == *' --progress '* ]]; then
        printf '%s\n' \
          'sync: checking snv assets' \
          'sync: reusing installed snv assets' \
          'sync: reusing installed runtime assets' \
          'sync: ready (0 downloaded, 0 resumed)' >&2
      fi
      printf '%s\n' '{"status":"ready","snv":{"status":"reused"},"runtime":{"status":"reused"},"downloaded_bytes":0,"resumed_bytes":0}'
    else
      [[ $HOME == "$QUALIFICATION_EXPECTED_HOME" ]]
      [[ $XDG_DATA_HOME == "$QUALIFICATION_EXPECTED_DATA" ]]
      [[ $XDG_CACHE_HOME == "$QUALIFICATION_EXPECTED_CACHE" ]]
      install -d -m 700 "$XDG_DATA_HOME/pangopup" "$XDG_DATA_HOME/pangopup/bundles"
      case ${QUALIFICATION_SNV_LAYOUT:-safe} in
        safe)
          install -d -m 700 "$QUALIFICATION_EXPECTED_SNV_BUNDLE"
          chmod 555 "${QUALIFICATION_EXPECTED_SNV_BUNDLE%/bundle}" "$QUALIFICATION_EXPECTED_SNV_BUNDLE"
          ;;
        zero) ;;
        multiple)
          install -d -m 700 \
            "$XDG_DATA_HOME/pangopup/bundles/first/bundle" \
            "$XDG_DATA_HOME/pangopup/bundles/second/bundle"
          chmod 555 \
            "$XDG_DATA_HOME/pangopup/bundles/first" \
            "$XDG_DATA_HOME/pangopup/bundles/first/bundle" \
            "$XDG_DATA_HOME/pangopup/bundles/second" \
            "$XDG_DATA_HOME/pangopup/bundles/second/bundle"
          ;;
        symlink)
          install -d -m 555 "$XDG_DATA_HOME/outside-bundle/bundle"
          ln -s "$XDG_DATA_HOME/outside-bundle" "$XDG_DATA_HOME/pangopup/bundles/linked"
          ;;
        unsafe)
          install -d -m 755 "$XDG_DATA_HOME/pangopup/bundles/unsafe"
          install -d -m 555 "$XDG_DATA_HOME/pangopup/bundles/unsafe/bundle"
          ;;
        *) exit 2 ;;
      esac
      if [[ " $* " == *' --progress '* ]]; then
        printf '%s\n' \
          'sync: checking snv assets' \
          'sync: snv payload fresh attempt 1/4 10/10 bytes (10 downloaded, 0 resumed)' \
          'sync: ready (10 downloaded, 0 resumed)' >&2
      fi
      printf '%s\n' '{"status":"ready","snv":{"status":"installed"},"runtime":{"status":"installed"},"downloaded_bytes":10,"resumed_bytes":0}'
    fi
    ;;
  status)
    printf '%s\n' '{"status":"ready","snv":{"status":"ready"},"runtime":{"status":"ready"}}'
    ;;
  lookup)
    if [[ " $* " == *' --model-only '* && " $* " == *' GRCh38:chr12:6801301:G:A '* ]]; then
      printf 'model\n' >>"$QUALIFICATION_LOOKUP_LOG"
      name_records < "$QUALIFICATION_SOURCE/tests/fixtures/executable-release/model-only-snv.jsonl"
      exit
    fi
    if [[ " $* " == *' GRCh38:chr12:6801303:G:GA '* ]]; then
      [[ " $* " != *' --bundle '* ]] || exit 2
      printf 'model\n' >>"$QUALIFICATION_LOOKUP_LOG"
      name_records < "$QUALIFICATION_SOURCE/tests/fixtures/executable-release/m09.jsonl"
      exit
    fi
    group=unfiltered
    bundle=
    variants=()
    while (( $# )); do
      case $1 in
        --bundle) bundle=$2; shift 2 ;;
        --gene) group=$2; shift 2 ;;
        --variant) variants+=("$2"); shift 2 ;;
        --format)
          [[ $2 == jsonl ]] || exit 2
          shift 2
          ;;
        *) exit 2 ;;
      esac
    done
    [[ $bundle == "$QUALIFICATION_EXPECTED_SNV_BUNDLE" ]] || exit 2
    printf 'snv\t%s\n' "$bundle" >>"$QUALIFICATION_LOOKUP_LOG"
    mapfile -t expected_variants < <(awk -F '\t' -v group="$group" 'NR > 1 && $2 == group { print $4 }' "$QUALIFICATION_SOURCE/tests/fixtures/snv-regression/requests.tsv")
    [[ "${variants[*]}" == "${expected_variants[*]}" ]] || exit 2
    render=("$QUALIFICATION_REAL_PANGOPUP" lookup --bundle "$QUALIFICATION_SNV_FIXTURE_BUNDLE" --format jsonl)
    for variant in "${variants[@]}"; do render+=(--variant "$variant"); done
    if [[ $group != unfiltered ]]; then render+=(--gene "$group"); fi
    exec "${render[@]}"
    ;;
  serve)
    exec python3 - "$QUALIFICATION_SOURCE" <<'PY'
import http.server, json, pathlib, sys
source = pathlib.Path(sys.argv[1])
model = json.loads((source / "tests/fixtures/executable-release/m09.jsonl").read_bytes())
model_only_snv = json.loads((source / "tests/fixtures/executable-release/model-only-snv.jsonl").read_bytes())
automatic_snv = json.loads((source / "tests/fixtures/snv-regression/expected/ENSG00000010610.jsonl").read_text().splitlines()[0])
scoring_identity = "sha256:" + "1" * 64
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def emit(self, value):
        body = json.dumps(value, separators=(",", ":")).encode() + b"\n"
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        values = {
            "/livez": {"status":"live"},
            "/readyz": {"status":"ready"},
            "/v1/status": {"version":"0.5.0","readiness":"ready","scoring_identity":scoring_identity},
        }
        self.emit(values[self.path])
    def do_POST(self):
        body = self.rfile.read(int(self.headers["content-length"]))
        request = json.loads(body)
        if request.get("model_only"):
            result = model_only_snv
        elif request["variants"][0].endswith(":G:A"):
            result = automatic_snv
        else:
            result = model
        result = dict(result)
        result["records"] = [
            dict(record, gene_names={
                "symbol": "CD4",
                "source": "hgnc",
                "hgnc_id": "HGNC:1678",
                "ncbi_gene_id": 920,
                "alias_symbols": ["T4", "Leu-3"],
            })
            for record in result["records"]
        ]
        result["input"] = request["variants"][0]
        result["scoring_identity"] = scoring_identity
        self.emit({"results":[result]})
    def log_message(self, *_): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 18080), Handler).serve_forever()
PY
    ;;
  *) exit 2 ;;
esac
SH
chmod 755 "$root/bin/pangopup"

cat >"$root/bin/name-records.py" <<'NAMERECORDS'
"""Add the naming leaf a real release renders onto every replayed record."""
import re
import sys

LEAF = (
    ',"gene_names":{"symbol":"CD4","source":"hgnc","hgnc_id":"HGNC:1678",'
    '"ncbi_gene_id":920,"alias_symbols":["T4","Leu-3"]}'
)
sys.stdout.write(
    re.sub(r'("stable_gene":"[A-Z0-9]+")', lambda m: m.group(1) + LEAF, sys.stdin.read())
)
NAMERECORDS
export QUALIFICATION_NAME_RECORDS=$root/bin/name-records.py

jq -S -c '(.provenance) as $provenance | .results[0] | .records |= map(. + {stable_gene: (.gene | sub("\\..*$"; ""))}) | . + {provenance:$provenance}' \
  "$repo/tests/fixtures/container-qualification/production-model-oracle.json" \
  >"$root/derived-model-only-snv.json"
jq -S -c . "$repo/tests/fixtures/executable-release/model-only-snv.jsonl" \
  >"$root/checked-model-only-snv.json"
cmp "$root/derived-model-only-snv.json" "$root/checked-model-only-snv.json"

# The stub delegates the seven SNV groups to the built executable and the
# comparison below re-renders them, so nothing past this point runs without it.
# Build before the first use, not after, or a stale or missing build reports
# itself as a failed qualification command.
"$repo/scripts/require-built-commands.sh"
real_cli=$repo/target/debug/pangopup

export QUALIFICATION_SOURCE=$repo
export QUALIFICATION_REAL_PANGOPUP=$real_cli
export QUALIFICATION_SNV_FIXTURE_BUNDLE=$repo/tests/fixtures/snv-regression/bundle
export QUALIFICATION_EXPECTED_HOME=$root/output/home
export QUALIFICATION_EXPECTED_DATA=$root/data
export QUALIFICATION_EXPECTED_CACHE=$root/cache
export QUALIFICATION_EXPECTED_SNV_BUNDLE=$root/data/pangopup/bundles/qualified/bundle
export QUALIFICATION_LOOKUP_LOG=$root/lookups.log
"$repo/scripts/run-production-qualification.sh" \
  "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$root/output"
# A rejection here means the shipped renderer and the checker disagree about
# what a scored record carries. The oracles beside the checker are deliberately
# independent of the renderer and never move, so the checker is the file that
# has to learn about a changed field. Say so, because a maintainer reading a
# byte mismatch has no other way to know which of the two descriptions moved.
require_checker_accepts() {
  local label=$1 output=$2
  shift 2
  if ! "$repo/scripts/check-production-qualification.py" "$output" "$repo" "$@" \
    >"$root/$label.out" 2>"$root/$label.err"; then
    cat "$root/$label.err" >&2
    printf 'scripts/check-production-qualification.py describes what a scored record carries. A field added to, removed from or renamed in what the tool prints must be reflected there.\n' >&2
    return 1
  fi
  grep -Fxq 'production qualification passed' "$root/$label.out"
}
require_checker_accepts check "$root/output"
[[ $(grep -Fc $'snv\t' "$root/lookups.log") == 7 ]]
[[ $(grep -Fxc $'snv\t'"$QUALIFICATION_EXPECTED_SNV_BUNDLE" "$root/lookups.log") == 7 ]]
[[ $(grep -Fxc 'model' "$root/lookups.log") == 2 ]]

# What the checker compared must be what the shipped renderer printed.
#
# The stub above replays committed oracles. A record this harness writes by
# hand proves nothing about a release: a field added to, removed from or
# renamed in what the tool prints never reaches the checker, which is how
# v0.5.0's naming leaf passed every gate and would have failed qualification.
# The seven SNV groups need no published asset. The built executable scores
# them against the repository SNV fixture bundle and prints the release's own
# bytes, so the qualification output must be exactly those bytes.
rendered=$root/rendered
install -d -m 700 "$rendered"
groups=(
  ENSG00000010610
  ENSG00000141499
  ENSG00000141510
  ENSG00000169129
  ENSG00000175727
  ENSG00000185974
  unfiltered
)
for group in "${groups[@]}"; do
  mapfile -t rendered_variants < <(awk -F '\t' -v group="$group" 'NR > 1 && $2 == group { print $4 }' \
    "$repo/tests/fixtures/snv-regression/requests.tsv")
  render_command=("$real_cli" lookup --bundle "$repo/tests/fixtures/snv-regression/bundle" --format jsonl)
  for variant in "${rendered_variants[@]}"; do render_command+=(--variant "$variant"); done
  if [[ $group != unfiltered ]]; then render_command+=(--gene "$group"); fi
  "${render_command[@]}" >"$rendered/$group.jsonl"
  if ! cmp -s "$root/output/snv-$group.jsonl" "$rendered/$group.jsonl"; then
    printf 'the qualification output for %s is not what the shipped renderer printed, so scripts/check-production-qualification.py compared a record this harness wrote by hand\n' "$group" >&2
    exit 1
  fi
done

# The renderer names three kinds of record: a precomputed score, a model score
# and a source-reference ambiguity. The checker takes the naming leaf back out
# of each. Require the compared output to carry a named ambiguity, so the
# removal rule is proved against every kind the renderer names rather than
# against the one kind a hand-written record happens to carry.
python3 - "$root/output/snv-unfiltered.jsonl" <<'NAMEDAMBIGUITY'
import json
import pathlib
import sys

ambiguities = [
    ambiguity
    for line in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
    for ambiguity in json.loads(line)["source_reference_ambiguities"]
]
assert ambiguities, "the unfiltered group carried no source-reference ambiguity"
assert any("gene_names" in ambiguity for ambiguity in ambiguities), (
    "no source-reference ambiguity the checker compared carries a naming leaf, so "
    "scripts/check-production-qualification.py never removes one from that kind of record"
)
NAMEDAMBIGUITY

# The model route and the HTTP score item replay oracles, because no repository
# fixture reproduces the published model's scores. Their naming leaf is not
# excused by that: it is a rendering, and the shipped renderer prints it for
# this accession on a route repository fixtures do reach. Pin the replayed leaf
# to the one the executable just printed, so the two agree by evidence.
rendered_leaf=$(
  "$real_cli" lookup --bundle "$repo/tests/fixtures/snv-regression/bundle" --format jsonl \
    --variant GRCh38:chr12:6801301:G:A --gene ENSG00000010610 \
    | python3 -c 'import json, sys; print(json.dumps(json.loads(sys.stdin.readline())["records"][0]["gene_names"], separators=(",", ":")))'
)
for replayed in model-M09.jsonl model-only-SNV.jsonl http-snv.txt http-model.txt http-model-only.txt; do
  if ! grep -Fq "\"gene_names\":$rendered_leaf" "$root/output/$replayed"; then
    printf 'the naming leaf replayed into %s is not the leaf the shipped renderer prints for ENSG00000010610\n' "$replayed" >&2
    exit 1
  fi
done

# The two model oracles are the published model's answers, so the harness has to
# keep replaying their scores. The record's shape is a different matter. The
# built executable renders a model-route record from committed fixtures alone --
# the miniature model kernel, the route reference bundle and the route mask --
# on both routes the release qualifies: the fallback a precomputed miss takes,
# and the forced route. Nothing about those two records is comparable to the
# oracles except their shape, and their shape is exactly what the checker
# describes, because it compares the release's bytes against an oracle it can
# never move. Require the renderer and each oracle to carry the same keys, in
# the same order, with the same leaf types, so a field added to, removed from or
# renamed in a model-route record fails here rather than at qualification time.
model_shape_cache=$root/model-shape-cache
install -d -m 700 "$model_shape_cache"
# The model route writes a result cache. Point it inside this run and clear the
# four variables that could redirect it, exactly as the runner does, so the
# render reaches the committed fixtures and nothing an installed profile owns.
model_render=(
  env -u PANGOPUP_DATA_DIR -u PANGOPUP_CACHE_DIR -u PANGOPUP_MODEL_CACHE \
    -u PANGOPUP_MODEL_CACHE_MAX_ENTRIES "XDG_CACHE_HOME=$model_shape_cache"
  "$real_cli" lookup
  --variant GRCh38:chr1:5051:A:AC
  --reference-bundle "$repo/tests/fixtures/reference-route-test/bundle"
  --mask "$repo/tests/fixtures/route-mask/domains.pgm"
  --model-bundle "$repo/tests/fixtures/pangolin-model-kernel-mini/bundle"
  --format jsonl
)
"${model_render[@]}" --bundle "$repo/tests/fixtures/snv-regression/bundle" \
  >"$rendered/model-fallback.jsonl"
"${model_render[@]}" --model-only >"$rendered/model-only.jsonl"
python3 - \
  "$rendered/model-fallback.jsonl" "$repo/tests/fixtures/executable-release/m09.jsonl" \
    M09-insertion-short-plus \
  "$rendered/model-only.jsonl" "$repo/tests/fixtures/executable-release/model-only-snv.jsonl" \
    'model-only SNV' \
  <<'MODELSHAPE'
import json
import pathlib
import sys


def shape(value):
    """The key structure and leaf types, with list contents left opaque.

    Two model-route records scored from different bundles share no value: the
    accession, both scores, every position and every provenance identity
    differ. What they must share is the key set, the key order and the type of
    each leaf, because the checker compares a release's bytes against an oracle
    byte for byte.
    """
    if isinstance(value, dict):
        return [[key, shape(item)] for key, item in value.items()]
    if isinstance(value, list):
        return "list"
    if isinstance(value, bool):
        return "bool"
    return type(value).__name__


def model_shape(path, drop_names):
    value = json.loads(pathlib.Path(path).read_text(encoding="utf-8").splitlines()[0])
    assert value["provenance"]["kind"] == "model", f"{path} is not a model-route record"
    records = value["records"]
    assert len(records) == 1, f"{path} carries {len(records)} records, not one"
    record = dict(records[0])
    if drop_names:
        # scripts/check-production-qualification.py removes the naming leaf
        # before it compares, because the oracles carry scoring bytes alone.
        record.pop("gene_names", None)
    return [shape(value), shape(record)]


arguments = sys.argv[1:]
assert len(arguments) % 3 == 0 and arguments, "expected rendered/oracle/label triples"
for index in range(0, len(arguments), 3):
    rendered_path, oracle_path, label = arguments[index : index + 3]
    rendered = model_shape(rendered_path, drop_names=True)
    oracle = model_shape(oracle_path, drop_names=False)
    assert rendered == oracle, (
        f"the shipped renderer and the {label} oracle disagree about what a "
        "model-route record carries, so scripts/check-production-qualification.py "
        "describes a record the tool no longer prints\n"
        f"  renderer: {json.dumps(rendered)}\n"
        f"  oracle:   {json.dumps(oracle)}"
    )
MODELSHAPE

# A field added to what the tool prints for a scored record must fail the
# checker, on each surface the checker compares with its own implementation:
# the precomputed route and the model route byte for byte, the HTTP score item
# through a decoded comparison. The oracles never move, so the checker is what
# must change, and the harness must say that.
cp -a "$root/output" "$root/drift-add-output"
sed -i 's/"stable_gene":"\([A-Z0-9]*\)"/"stable_gene":"\1","drift_probe":true/' \
  "$root/drift-add-output/snv-ENSG00000010610.jsonl"
if require_checker_accepts drift-add "$root/drift-add-output" 2>"$root/drift-add.guidance"; then
  printf 'checker accepted a precomputed record carrying a field the oracles do not\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/drift-add.err"
grep -Fq 'scripts/check-production-qualification.py' "$root/drift-add.guidance"

cp -a "$root/output" "$root/drift-add-model-output"
sed -i 's/"stable_gene":"\([A-Z0-9]*\)"/"stable_gene":"\1","drift_probe":true/' \
  "$root/drift-add-model-output/model-M09.jsonl"
if require_checker_accepts drift-add-model "$root/drift-add-model-output" \
  2>"$root/drift-add-model.guidance"; then
  printf 'checker accepted a model record carrying a field the oracles do not\n' >&2
  exit 1
fi
grep -Fxq 'model oracle mismatch: M09-insertion-short-plus' "$root/drift-add-model.err"
grep -Fq 'scripts/check-production-qualification.py' "$root/drift-add-model.guidance"

cp -a "$root/output" "$root/drift-remove-output"
sed -i 's/,"loss_position":-\?[0-9]\+//g' "$root/drift-remove-output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/drift-remove-output" "$repo" \
  >"$root/drift-remove.out" 2>"$root/drift-remove.err"; then
  printf 'checker accepted a precomputed record missing a field the oracles carry\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/drift-remove.err"

reuse_output=$root/reuse-output
QUALIFICATION_EXPECTED_HOME=$reuse_output/home \
  "$repo/scripts/run-production-qualification.sh" \
    "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$reuse_output" \
    --reuse-installed
require_checker_accepts reuse-check "$reuse_output" --reuse-installed

for layout in zero multiple symlink unsafe; do
  bad=$root/bad-$layout
  if QUALIFICATION_SNV_LAYOUT=$layout \
    QUALIFICATION_EXPECTED_HOME=$bad/output/home \
    QUALIFICATION_EXPECTED_DATA=$bad/data \
    QUALIFICATION_EXPECTED_CACHE=$bad/cache \
    QUALIFICATION_EXPECTED_SNV_BUNDLE=$bad/data/pangopup/bundles/qualified/bundle \
    QUALIFICATION_LOOKUP_LOG=$bad/lookups.log \
    "$repo/scripts/run-production-qualification.sh" \
      "$root/bin/pangopup" "$repo" "$bad/data" "$bad/cache" "$bad/output" \
      >"$root/$layout.out" 2>"$root/$layout.err"; then
    printf 'runner accepted %s installed SNV layout\n' "$layout" >&2
    exit 1
  fi
done
grep -Fxq 'expected exactly one installed SNV bundle, found 0' "$root/zero.err"
grep -Fxq 'expected exactly one installed SNV bundle, found 2' "$root/multiple.err"
grep -Fxq 'installed SNV bundle is unsafe' "$root/symlink.err"
grep -Fxq 'installed SNV bundle is unsafe' "$root/unsafe.err"

if "$repo/scripts/run-production-qualification.sh" \
  "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$root/second-output" \
  >"$root/fresh.out" 2>"$root/fresh.err"; then
  printf 'runner accepted existing XDG directories\n' >&2
  exit 1
fi
grep -Fxq 'qualification directories must be absent' "$root/fresh.err"

cp -a "$root/output" "$root/reused-online-output"
sed -i 's/"installed"/"reused"/g' "$root/reused-online-output/sync-online.json"
if "$repo/scripts/check-production-qualification.py" "$root/reused-online-output" "$repo" >"$root/reused.out" 2>"$root/reused.err"; then
  printf 'checker accepted reused first online sync\n' >&2
  exit 1
fi
grep -Fxq 'unexpected snv state: sync-online.json' "$root/reused.err"

cp -a "$root/output" "$root/format-output"
sed -i '1s/,/, /' "$root/format-output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$repo" >"$root/format.out" 2>"$root/format.err"; then
  printf 'checker accepted formatting drift\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/format.err"

cp -a "$root/output" "$root/progress-output"
sed -i 's/(10 downloaded, 0 resumed)/(9 downloaded, 0 resumed)/' \
  "$root/progress-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-output" "$repo" >"$root/progress.out" 2>"$root/progress.err"; then
  printf 'checker accepted progress/final mismatch\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress totals do not match final JSON' "$root/progress.err"

cp -a "$root/output" "$root/progress-decrease-output"
sed -i '0,/(10 downloaded, 0 resumed)/s//(11 downloaded, 0 resumed)/' \
  "$root/progress-decrease-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-decrease-output" "$repo" >"$root/progress-decrease.out" 2>"$root/progress-decrease.err"; then
  printf 'checker accepted transfer counters above completion\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress counters decreased' "$root/progress-decrease.err"

cp -a "$root/output" "$root/progress-duplicate-output"
tail -1 "$root/progress-duplicate-output/sync-online.progress" \
  >>"$root/progress-duplicate-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-duplicate-output" "$repo" >"$root/progress-duplicate.out" 2>"$root/progress-duplicate.err"; then
  printf 'checker accepted duplicate completion records\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress lacks transfer or completion evidence' \
  "$root/progress-duplicate.err"

cp -a "$root/output" "$root/model-only-output"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' \
  "$root/model-only-output/model-only-SNV.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/model-only-output" "$repo" >"$root/model-only.out" 2>"$root/model-only.err"; then
  printf 'checker accepted changed model-only result\n' >&2
  exit 1
fi
grep -Fxq 'model-only SNV oracle mismatch' "$root/model-only.err"

cp -a "$root/output" "$root/unnamed-output"
sed -i 's/,"gene_names":{[^}]*}//g' "$root/unnamed-output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/unnamed-output" "$repo" >"$root/unnamed.out" 2>"$root/unnamed.err"; then
  printf 'checker accepted a release that named no gene\n' >&2
  exit 1
fi
grep -Fxq 'the release named no gene in snv-ENSG00000010610.jsonl' "$root/unnamed.err"

cp -a "$root/output" "$root/unnamed-model-output"
sed -i 's/,"gene_names":{[^}]*}//g' "$root/unnamed-model-output/model-M09.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/unnamed-model-output" "$repo" >"$root/unnamed-model.out" 2>"$root/unnamed-model.err"; then
  printf 'checker accepted a model route that named no gene\n' >&2
  exit 1
fi
grep -Fxq 'the release named no gene in M09-insertion-short-plus' "$root/unnamed-model.err"

cp -a "$root/output" "$root/unnamed-http-output"
python3 - "$root/unnamed-http-output/http-snv.txt" <<'STRIPNAMES'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
head, separator, body = path.read_bytes().partition(b"\r\n\r\n")
assert separator
value = json.loads(body)
for record in value["results"][0]["records"]:
    record.pop("gene_names", None)
body = json.dumps(value, separators=(",", ":")).encode() + b"\n"
head = b"\r\n".join(
    line
    for line in head.split(b"\r\n")
    if not line.lower().startswith(b"content-length:")
)
head += b"\r\ncontent-length: " + str(len(body)).encode()
path.write_bytes(head + separator + body)
STRIPNAMES
if "$repo/scripts/check-production-qualification.py" "$root/unnamed-http-output" "$repo" >"$root/unnamed-http.out" 2>"$root/unnamed-http.err"; then
  printf 'checker accepted an HTTP response that named no gene\n' >&2
  exit 1
fi
grep -Fxq 'HTTP SNV named no gene' "$root/unnamed-http.err"

cp -a "$root/output" "$root/http-output"
sed -i 's/"version":"0.5.0"/"version":"9.9.9"/' "$root/http-output/http-status.txt"
if "$repo/scripts/check-production-qualification.py" "$root/http-output" "$repo" >"$root/http.out" 2>"$root/http.err"; then
  printf 'checker accepted changed HTTP status version\n' >&2
  exit 1
fi
grep -Fxq 'HTTP status response mismatch' "$root/http.err"

cp -a "$root/output" "$root/http-model-only-output"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' \
  "$root/http-model-only-output/http-model-only.txt"
if "$repo/scripts/check-production-qualification.py" "$root/http-model-only-output" "$repo" >"$root/http-model-only.out" 2>"$root/http-model-only.err"; then
  printf 'checker accepted changed HTTP model-only SNV\n' >&2
  exit 1
fi
grep -Fxq 'HTTP model-only SNV response mismatch' "$root/http-model-only.err"

expect_http_contract_rejected() {
  local label=$1 file=$2 mutation=$3 expected=$4
  local changed="$root/http-contract-$label"
  cp -a "$root/output" "$changed"
  python3 - "$changed/$file" "$mutation" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
mutation = sys.argv[2]
head, separator, body = path.read_bytes().partition(b"\r\n\r\n")
assert separator
value = json.loads(body)
item = value.get("results", [{}])[0]
if mutation == "missing-input":
    del item["input"]
elif mutation == "input-type":
    item["input"] = 7
elif mutation == "input-value":
    item["input"] = "GRCh38:chr1:1:A:C"
elif mutation == "missing-identity":
    del item["scoring_identity"]
elif mutation == "identity-type":
    item["scoring_identity"] = 7
elif mutation == "identity-malformed":
    item["scoring_identity"] = "sha256:" + "A" * 64
elif mutation == "identity-mismatch":
    item["scoring_identity"] = "sha256:" + "2" * 64
elif mutation == "status-identity-mismatch":
    value["scoring_identity"] = "sha256:" + "2" * 64
elif mutation == "extra-item-property":
    item["transport_extra"] = True
elif mutation == "record-extra-property":
    item["records"][0]["drift_probe"] = True
elif mutation == "missing-score-property":
    del item["status"]
elif mutation == "wrong-score-type":
    item["position"] = "6801301"
elif mutation == "wrong-score-value":
    item["position"] += 1
elif mutation == "integer-as-float":
    item["position"] = float(item["position"])
elif mutation == "boolean-as-integer":
    item["provenance"]["masked"] = 1
else:
    raise AssertionError(mutation)
path.write_bytes(head + separator + json.dumps(value, separators=(",", ":")).encode() + b"\n")
PY
  if "$repo/scripts/check-production-qualification.py" "$changed" "$repo" \
    >"$root/http-contract-$label.out" 2>"$root/http-contract-$label.err"; then
    printf 'checker accepted HTTP contract mutation: %s\n' "$label" >&2
    exit 1
  fi
  grep -Fxq "$expected" "$root/http-contract-$label.err"
}

expect_http_contract_rejected missing-input http-snv.txt missing-input \
  'HTTP SNV item shape mismatch'
expect_http_contract_rejected input-type http-snv.txt input-type \
  'HTTP SNV input mismatch'
expect_http_contract_rejected input-value http-snv.txt input-value \
  'HTTP SNV input mismatch'
expect_http_contract_rejected missing-identity http-model.txt missing-identity \
  'HTTP model item shape mismatch'
expect_http_contract_rejected identity-type http-model.txt identity-type \
  'HTTP model scoring identity is invalid'
expect_http_contract_rejected identity-malformed http-model.txt identity-malformed \
  'HTTP model scoring identity is invalid'
expect_http_contract_rejected cross-item-identity http-model-only.txt identity-mismatch \
  'HTTP model-only SNV scoring identity mismatch'
expect_http_contract_rejected status-identity http-status.txt status-identity-mismatch \
  'HTTP SNV scoring identity mismatch'
expect_http_contract_rejected extra-item-property http-model-only.txt extra-item-property \
  'HTTP model-only SNV item shape mismatch'
# A field added to the scored record inside the item, not to the item envelope.
# `json_equal` is the checker's third and only decoded comparison, so an added
# field has to be rejected there too or a change to what the tool prints
# reaches the release through the HTTP surface alone.
expect_http_contract_rejected record-extra-property http-snv.txt record-extra-property \
  'HTTP SNV response mismatch'
expect_http_contract_rejected missing-score-property http-snv.txt missing-score-property \
  'HTTP SNV item shape mismatch'
expect_http_contract_rejected wrong-score-type http-snv.txt wrong-score-type \
  'HTTP SNV response mismatch'
expect_http_contract_rejected wrong-score-value http-snv.txt wrong-score-value \
  'HTTP SNV response mismatch'
expect_http_contract_rejected integer-as-float http-snv.txt integer-as-float \
  'HTTP SNV response mismatch'
expect_http_contract_rejected boolean-as-integer http-model.txt boolean-as-integer \
  'HTTP model response mismatch'

cp -a "$root/output" "$root/truncated-output"
sed -i '$d' "$root/truncated-output/snv-unfiltered.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/truncated-output" "$repo" >"$root/truncated.out" 2>"$root/truncated.err"; then
  printf 'checker accepted truncated output\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: unfiltered' "$root/truncated.err"

sed -i '0,/"gain_score":"0.00"/s//"gain_score":"0.99"/' "$root/output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/output" "$repo" >"$root/tamper.out" 2>"$root/tamper.err"; then
  printf 'checker accepted changed score\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/tamper.err"

fixture=$root/substituted-source/tests/fixtures
install -d -m 700 "$fixture/snv-regression/expected" "$fixture/executable-release"
cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
cp "$repo/tests/fixtures/snv-regression/expected/"*.jsonl "$fixture/snv-regression/expected/"
cp "$repo/tests/fixtures/executable-release/m09.jsonl" "$fixture/executable-release/m09.jsonl"
cp "$repo/tests/fixtures/executable-release/model-only-snv.jsonl" \
  "$fixture/executable-release/model-only-snv.jsonl"
cp "$fixture/snv-regression/expected/ENSG00000169129.jsonl" "$fixture/snv-regression/expected/unfiltered.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/substitution.out" 2>"$root/substitution.err"; then
  printf 'checker accepted substituted oracle\n' >&2
  exit 1
fi
grep -Fxq 'expected oracle identity mismatch: unfiltered' "$root/substitution.err"

cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
sed -i '$d' "$fixture/snv-regression/requests.tsv"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/request.out" 2>"$root/request.err"; then
  printf 'checker accepted truncated requests\n' >&2
  exit 1
fi
grep -Fxq 'request fixture identity mismatch' "$root/request.err"

cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
cp "$repo/tests/fixtures/snv-regression/expected/unfiltered.jsonl" "$fixture/snv-regression/expected/unfiltered.jsonl"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' "$fixture/executable-release/m09.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/model-substitution.out" 2>"$root/model-substitution.err"; then
  printf 'checker accepted substituted model oracle\n' >&2
  exit 1
fi
grep -Fxq 'model oracle identity mismatch: M09-insertion-short-plus' "$root/model-substitution.err"

awk '
  $0 == "<!-- BEGIN TICKET 038 COORDINATOR SCRIPT -->" { inside=1; next }
  $0 == "<!-- END TICKET 038 COORDINATOR SCRIPT -->" { inside=0; found=1; next }
  inside && $0 != "```bash" && $0 != "```" { print }
  END { if (!found) exit 1 }
' "$repo/planning/artifacts/038-public-linux-release.md" >"$root/coordinator-runbook.sh"
bash -n "$root/coordinator-runbook.sh"

awk '
  $0 == "<!-- BEGIN TICKET 050 COORDINATOR SCRIPT -->" { inside=1; next }
  $0 == "<!-- END TICKET 050 COORDINATOR SCRIPT -->" { inside=0; found=1; next }
  inside && $0 != "```bash" && $0 != "```" { print }
  END { if (!found) exit 1 }
' "$repo/planning/artifacts/050-public-linux-release.md" >"$root/ticket050-runbook.sh"
bash -n "$root/ticket050-runbook.sh"
grep -Fq 'readonly TAG=v0.2.0' "$root/ticket050-runbook.sh"
grep -Fq '"$SOURCE_TREE/scripts/qualify-linux-release.sh" "$PRIVATE/release" "$VERSION" "$COMMIT"' "$root/ticket050-runbook.sh"
[[ $(grep -Fc 'run-production-qualification.sh' "$root/ticket050-runbook.sh") == 3 ]]
[[ $(grep -Fc -- '--reuse-installed' "$root/ticket050-runbook.sh") == 2 ]]
grep -Fq 'gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" -F draft=false' "$root/ticket050-runbook.sh"
grep -Fq 'PUBLISHED=1' "$root/ticket050-runbook.sh"
grep -Fq 'https://raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh' "$root/ticket050-runbook.sh"
grep -Fq 'repos/$REPO/actions/workflows/package-linux.yml' "$root/ticket050-runbook.sh"
grep -Fq 'ARTIFACT_ID=$(jq -r .artifacts[0].id "$PRIVATE/artifacts.json")' "$root/ticket050-runbook.sh"
grep -Fq 'exec {UPLOAD_FD}<"$source_path"' "$root/ticket050-runbook.sh"
grep -Fq 'sha256sum "/proc/self/fd/$UPLOAD_FD"' "$root/ticket050-runbook.sh"
grep -Fq 'https://github.com/$REPO/releases/download/$TAG/$name' "$root/ticket050-runbook.sh"
[[ $(grep -Fc 'curl -q -fsSL' "$root/ticket050-runbook.sh") == 5 ]]
! grep -Fq 'curl -fsSL' "$root/ticket050-runbook.sh"
! grep -Eq 'GH_TOKEN=|GITHUB_TOKEN=|Authorization:' "$repo/planning/artifacts/050-public-linux-release.md"

assert_source_authority() {
  local runbook=$1
  grep -Fxq 'readonly CHECKOUT=$PWD' "$runbook" || return 1
  grep -Fxq 'test -z "$(git -C "$CHECKOUT" replace -l)"' "$runbook" || return 1
  grep -Fxq 'GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" cat-file -e "$COMMIT^{commit}"' "$runbook" || return 1
  grep -Fxq 'GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" archive --format=tar "$COMMIT" \' "$runbook" || return 1
  [[ $(grep -Fc 'GIT_NO_REPLACE_OBJECTS=1' "$runbook") == 2 ]] || return 1
  grep -Fxq 'readonly SOURCE_TREE=$PRIVATE/source' "$runbook" || return 1
  grep -Fxq 'readonly BODY=$SOURCE_TREE/planning/artifacts/050-release-notes.md' "$runbook" || return 1
  grep -Fq '"$SOURCE_TREE/scripts/qualify-linux-release.sh"' "$runbook" || return 1
  [[ $(grep -Fc '"$SOURCE_TREE/scripts/check-production-qualification.py"' "$runbook") == 2 ]] || return 1
  ! grep -Eq '^scripts/(qualify-linux-release\.sh|check-production-qualification\.py)' "$runbook" || return 1
}
assert_source_authority "$root/ticket050-runbook.sh"

cp "$root/ticket050-runbook.sh" "$root/mutable-source-runbook.sh"
sed -i 's|readonly SOURCE_TREE=\$PRIVATE/source|readonly SOURCE_TREE=$CHECKOUT|' \
  "$root/mutable-source-runbook.sh"
if assert_source_authority "$root/mutable-source-runbook.sh"; then
  printf 'source authority accepted mutable checkout\n' >&2
  exit 1
fi

cp "$root/ticket050-runbook.sh" "$root/replace-enabled-runbook.sh"
sed -i 's/GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" archive/git -C "$CHECKOUT" archive/' \
  "$root/replace-enabled-runbook.sh"
if assert_source_authority "$root/replace-enabled-runbook.sh"; then
  printf 'source authority accepted replacement-enabled archive\n' >&2
  exit 1
fi

assert_ownership_contract() {
  local runbook=$1
  grep -Fxq 'readonly HOST_UID=$(id -u)' "$runbook" || return 1
  grep -Fxq 'readonly HOST_GID=$(id -g)' "$runbook" || return 1
  [[ $(grep -Fxc -- '  --user "$HOST_UID:$HOST_GID" \' "$runbook") == 1 ]] || return 1
  [[ $(grep -Fxc -- '  --env "HOST_UID=$HOST_UID" --env "HOST_GID=$HOST_GID" \' "$runbook") == 1 ]] || return 1
  [[ $(grep -Fc '/usr/bin/setpriv --reuid="$HOST_UID" --regid="$HOST_GID" --clear-groups' "$runbook") == 4 ]] || return 1
  [[ $(grep -Fc 'env -i HOME=/qualification/home XDG_DATA_HOME=/qualification/data XDG_CACHE_HOME=/qualification/cache PATH=/usr/bin:/bin' "$runbook") == 4 ]] || return 1
  grep -Fxq '    chown "$HOST_UID:$HOST_GID" /qualification/install /qualification/home /qualification/post' "$runbook" || return 1
  ! grep -Eq 'chown[[:space:]].*/qualification/(data|cache)|chown[[:space:]]+-R' "$runbook" || return 1
}
assert_ownership_contract "$root/coordinator-runbook.sh"

cp "$root/coordinator-runbook.sh" "$root/missing-user-runbook.sh"
sed -i '0,/--user "\$HOST_UID:\$HOST_GID"/s///' "$root/missing-user-runbook.sh"
if assert_ownership_contract "$root/missing-user-runbook.sh"; then
  printf 'ownership contract accepted missing prepublication user mapping\n' >&2
  exit 1
fi

cp "$root/coordinator-runbook.sh" "$root/drifted-setpriv-runbook.sh"
sed -i '0,/--clear-groups/s//--keep-groups/' "$root/drifted-setpriv-runbook.sh"
if assert_ownership_contract "$root/drifted-setpriv-runbook.sh"; then
  printf 'ownership contract accepted drifted privilege drop\n' >&2
  exit 1
fi

cp "$root/coordinator-runbook.sh" "$root/unsafe-chown-runbook.sh"
printf '\nchown -R "$HOST_UID:$HOST_GID" /qualification/data /qualification/cache\n' >>"$root/unsafe-chown-runbook.sh"
if assert_ownership_contract "$root/unsafe-chown-runbook.sh"; then
  printf 'ownership contract accepted qualified-data chown\n' >&2
  exit 1
fi

image=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
[[ "$(grep -Fc "$image" "$repo/planning/artifacts/038-public-linux-release.md")" == 4 ]]
grep -Fq 'build runner: GitHub-hosted Ubuntu 24.04' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'admitted maximum imported GLIBC version: `2.39`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'package run `30648307402` failed while linking' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30651619497` passed the full gate' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30652858960` then redundantly reran the full' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30653836700` built the release executables' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'exactly one corrected dispatch is permitted' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'State: **COMPLETE — immutable public release `v0.1.0` targets reviewed commit' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Target `ci` run ID/URL: `30657770808`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Workflow run ID/URL: `30657987617`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Draft/release ID: `363278563`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'checksum-verifying tagged installer are shipped' "$repo/AGENTS.md"
! grep -Fq 'public executable publication remains a separate ticket' "$repo/AGENTS.md"
grep -Fq 'passes that exact bundle path explicitly to each of the seven ordered' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'M09 model request deliberately has no `--bundle`' "$repo/planning/artifacts/038-public-linux-release.md"

printf 'production release qualification tests passed\n'
