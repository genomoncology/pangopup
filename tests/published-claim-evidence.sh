#!/usr/bin/env bash
set -euo pipefail

# Three published claims, each joined here to the thing in the repository that
# makes it true. None of them was read by any gate, so each could go stale
# without a build noticing -- which is how two of them went stale.
#
#   1. The stored version rides on the score item. Ticket 0052 put
#      `data_set_version` on every HTTP score item, and
#      `architecture/compatibility.md` enumerates it there. Two other documents
#      still describe it as a status-route field, so a reader of either
#      concludes a second request is needed to retain the version beside a
#      score.
#
#   2. A gene's score depends on which other same-strand genes were scored
#      beside it. `score_typed` walks the genes overlapping a variant and calls
#      `apply_mask` on shared `gain` and `loss` slices, so the mask one gene
#      applies is still in place when the next same-strand gene is scored. The
#      behaviour is upstream-faithful and the frozen corpus pins it as
#      `P01-same-strand-order`. Nothing a consumer reads says it happens.
#
#   3. A published index size is arithmetic presented as a measurement.
#      `architecture/index.md` states the complete corpus at eleven bytes per
#      locus; the retained build artifact measured a smaller payload. The two
#      reconcile exactly, and the document says nothing about the difference.
#
# Each check has the same shape: find the fact in the evidence the repository
# already keeps, then require the published sentence to agree with it. What is
# pinned is the claim, not a form of words -- a check asks whether the sentence
# names the thing it is about, never whether it is spelled a particular way, so
# a copy edit survives and a deletion or a reversal does not. Where a sentence
# has to be word for word, it is word for word against a field in the evidence
# file rather than against a string typed here, which is the idiom
# `tests/route-disagreement-rate.sh` already uses.
#
# What this does not prove: that the product behaves as the documents say. The
# HTTP contract in `spec/http-service.md` runs a service and asserts that every
# score item carries `data_set_version`; `crates/pangopup-build/tests/` runs the
# frozen corpus; `make spec` runs both. This file holds the published sentences
# to that evidence, and reads nothing but committed text.
#
# Nothing here writes into the checkout. Every fixture stands under a temporary
# directory.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

fail() { printf 'published claim evidence: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The `## <heading>` section of a Markdown file, up to the next `## ` heading,
# read as one flat run of words. The published prose is hand-wrapped, so every
# comparison below reads it unwrapped: a reflow is a copy edit and must not
# fail anything here.
section() {
    awk -v want="$2" '
        $0 == want { on = 1; next }
        on && /^## / { exit }
        on { print }
    ' "$1" | tr '\n' ' ' | tr -s ' '
}

# The whole of a Markdown file, read the same way.
flatten() { tr '\n' ' ' <"$1" | tr -s ' '; }

# One sentence per line. A full stop followed by a space ends one.
sentences() { sed -E 's/\. /.\n/g'; }

# 12345 -> 12,345. The published prose groups large counts; a table cell in the
# build artifact groups them too, and arithmetic here does not.
grouped() { printf '%s' "$1" | sed -E ':a;s/([0-9]+)([0-9]{3})/\1,\2/;ta'; }

# 12,345 -> 12345.
ungrouped() { printf '%s' "$1" | tr -d ','; }

# =============================================================================
# 1. the stored version rides on the score item
# =============================================================================
#
# `architecture/compatibility.md` is the one place the full response shape is
# enumerated, so it is read first: if the enumeration stops carrying
# `data_set_version` on the score item, the fact this check is about is gone
# and the check says so rather than holding two documents to a claim the
# repository no longer makes.
#
# Then each of the two documents has to carry a sentence that names
# `data_set_version` and names the score item. That is the claim: a reader of
# either document learns the value is already on the item in front of them.
# Nothing is required about how the sentence is worded, where it stands in the
# section, or what else it says -- only that the two things stand in one
# sentence together, and that the sentence is not a denial. A sentence saying
# the item does not carry it satisfies every word test and reverses the claim,
# so the small set of shapes the shell has for saying "not" is refused.
#
# The scan for a negation runs over a span rather than the whole sentence: from
# the start of the sentence through the later of the two terms the claim joins.
# A negative word standing there stands between the reader and the claim, and
# one standing after the claim has been made is about something else. That is
# what lets "Every returned score item carries `data_set_version`, so reaching
# it takes no second request" state the claim while "No returned score item
# carries `data_set_version`" does not, without the rule having to guess which
# `no` is which.
#
# Every word in the set is matched on word boundaries. Without them `nor`
# matches "ignore", "minor" and "honor", and `is not` matches "this notation",
# so ordinary prose stating the claim was refused as a denial.
#
# What the set still does not catch is a hedge: "rarely depends" and "carries
# it only when a deployment opts in" weaken the claim without denying it, and
# `rarely` cannot be refused because the dependence this file is about really
# is rare and the published sentence may say so.

inventory_relative='architecture/compatibility.md'
stored_version_documents=(
    "architecture/service.md|## Active scoring identity"
    "README.md|## HTTP service"
)

# A sentence that denies rather than states. Each is a way the shell of the
# sentence can be negative while both terms still stand in it. Matched on word
# boundaries, over the claim span rather than the whole sentence.
negations='(\<no\>|\<not\>|\<never\>|\<neither\>|\<nor\>|\<none\>|\<cannot\>|\<without\>|\<lack(s|ed|ing)?\>|\<omit(s|ted|ting)?\>|\<absent\>|\<missing\>|\<independent\>|\<unaffected\>|\<exclude(s|d)?\>|n'"'"'t\>)'

# One sentence per line in, one claim span per line out: the run from the start
# of the sentence through the later of the two terms the claim joins. Both
# terms are lowercase extended regular expressions. A sentence carrying
# neither is passed through whole, so nothing is silently exempted.
claim_span() {
    awk -v first="$1" -v second="$2" '
        {
            low = tolower($0)
            end = 0
            if (match(low, first)) end = RSTART + RLENGTH - 1
            if (match(low, second) && RSTART + RLENGTH - 1 > end) end = RSTART + RLENGTH - 1
            print (end > 0 ? substr($0, 1, end) : $0)
        }
    '
}

# The lines of $1 whose claim span carries no negation, printed whole.
affirmative_only() {
    local first=$2 second=$3 line span
    while IFS= read -r line; do
        [[ -n "$line" ]] || continue
        span=$(printf '%s\n' "$line" | claim_span "$first" "$second")
        printf '%s' "$span" | grep -Eqi -- "$negations" && continue
        printf '%s\n' "$line"
    done <<<"$1"
}

# Does the flat text at $1 carry a sentence naming both the score item and
# `data_set_version`, without denying it? Prints the sentence; exits 1 with the
# reason on standard error.
states_item_carries_version() {
    local text=$1 relative=$2 candidates affirmative
    candidates=$(printf '%s' "$text" | sentences | grep -F 'data_set_version' | grep -Ei 'score item' || true)
    [[ -n "$candidates" ]] || {
        printf '%s carries no sentence naming both a score item and `data_set_version`, so a reader of it still concludes the value reaches them only through the status route and that retaining it beside a score costs a second request\n' \
            "$relative" >&2
        return 1
    }
    affirmative=$(affirmative_only "$candidates" 'score items?' 'data_set_version' || true)
    [[ -n "$affirmative" ]] || {
        printf '%s names a score item and `data_set_version` in the same sentence only to deny it: %s\n' \
            "$relative" "$(printf '%s\n' "$candidates" | head -n 1)" >&2
        return 1
    }
    printf '%s\n' "$affirmative" | head -n 1
}

check_stored_version() {
    local root=$1 entry relative heading text

    [[ -f "$root/$inventory_relative" ]] || {
        printf 'no %s, so nothing enumerates the response shape the other documents describe\n' \
            "$inventory_relative" >&2
        return 1
    }
    states_item_carries_version "$(flatten "$root/$inventory_relative")" "$inventory_relative" >/dev/null || {
        printf '%s no longer enumerates `data_set_version` on the score item, and it is the one place the full response shape is enumerated, so the claim the other documents would restate is not the claim this repository makes\n' \
            "$inventory_relative" >&2
        return 1
    }

    for entry in "${stored_version_documents[@]}"; do
        IFS='|' read -r relative heading <<<"$entry"
        [[ -f "$root/$relative" ]] || {
            printf 'no %s to hold against the enumerated response shape\n' "$relative" >&2
            return 1
        }
        text=$(section "$root/$relative" "$heading")
        [[ -n "${text// /}" ]] || {
            printf '%s has no `%s` section, so this check read no published statement out of it\n' \
                "$relative" "$heading" >&2
            return 1
        }
        states_item_carries_version "$text" "$relative:$heading" >/dev/null || return 1
    done

    printf '%s and %s state that the score item carries `data_set_version`, as %s enumerates\n' \
        "${stored_version_documents[0]%%|*}" "${stored_version_documents[1]%%|*}" "$inventory_relative"
}

# =============================================================================
# 2. a gene's score depends on the same-strand genes scored beside it
# =============================================================================
#
# The evidence is the frozen upstream corpus. `P01-same-strand-order` is a
# postprocess case carrying two same-strand genes over one variant, and its
# expected answers are the behaviour: unmasked, the two genes agree; masked,
# they do not, because the mask the first gene applied to the shared arrays is
# still in place when the second is scored. The corpus declares the case in its
# manifest, the shipped builder names it, and
# `crates/pangopup-build/tests/compatibility.rs` mutates it to prove the
# comparison would notice a change.
#
# What the fixture proves on its own is narrower than the sentence: it proves
# the corpus pins a masked same-strand overlap where the two genes' answers
# differ. That the difference is carry-over rather than the two masks alone is
# what the product's own tests prove by mutating this case, which is why their
# hold on it is checked here too. This file never asserts the mechanism itself.
#
# Then `architecture/compatibility.md` -- the published contract -- has to
# carry a sentence saying a score can depend on the other same-strand genes
# over the same variant, and has to name the case, so a reader who wants to see
# the behaviour can find where it is pinned.
#
# The contract also has to name the one record of the measured set where the
# dependence changes an answer a consumer would read, and that record is read
# out of a committed file rather than taken on the document's word.
#
# What is committed is `planning/artifacts/0059-route-disagreement-records.tsv`,
# one row per compared gene record of the 2,615-variant set, and the identity
# that picks the record out of it is this: the row's two routes render a
# different value, and its variant carries more than one gene row. A variant
# with one gene has no other same-strand gene to depend on, so a value
# disagreement there is about something else. Exactly one row in the file
# satisfies both, and the check requires exactly one, so a later set that
# produced a second turns this red rather than leaving "the one record"
# standing as a stale singular.
#
# What is not committed is the cumulative-against-isolated pair of numbers the
# ticket quotes. Those came from a patched build that computed an isolated
# per-gene answer beside the shipped one, and that build is in no committed
# file. The committed row says what the two shipped routes render, which is
# what a consumer meets, so that is what the published sentence may state and
# what this reads.

corpus_case='P01-same-strand-order'
corpus_coverage='postprocess.same_strand_order'
corpus_manifest='tests/fixtures/pangolin-compat-v1/manifest.json'
corpus_cases='tests/fixtures/pangolin-compat-v1/cases.jsonl'
corpus_mutation_tests='crates/pangopup-build/tests/compatibility.rs'
contract_relative='architecture/compatibility.md'
measured_records='planning/artifacts/0059-route-disagreement-records.tsv'

# Read the record the contract names out of the measurement file and report
# what it says, or exit 1 with the reason the file does not bear it out.
read_named_record() {
    python3 - "$1" "$2" <<'PY'
import re
import sys

records_path, contract_text = sys.argv[1], sys.argv[2]

try:
    rows = [line.rstrip("\n").split("\t")
            for line in open(records_path, encoding="utf-8") if line.strip()]
except OSError as error:
    sys.exit("the measurement records cannot be read: %s" % error)
if len(rows) < 2:
    sys.exit("%s holds no records, so nothing says which one the dependence "
             "changes an answer at" % records_path)

header, rows = rows[0], rows[1:]
try:
    iv, ig = header.index("variant"), header.index("stable_gene")
    bg, bl = header.index("bundle_gain"), header.index("bundle_loss")
    mg, ml = header.index("model_gain"), header.index("model_loss")
except ValueError as missing:
    sys.exit("%s does not carry the columns this reads (%s)" % (records_path, missing))

genes_at = {}
for row in rows:
    genes_at[row[iv]] = genes_at.get(row[iv], 0) + 1

# The one record where the two routes render a different value at a variant
# carrying more than one gene. A variant with a single gene has no other
# same-strand gene for an answer to depend on.
changed = [row for row in rows
           if genes_at[row[iv]] > 1 and (row[bg] != row[mg] or row[bl] != row[ml])]
if len(changed) != 1:
    sys.exit("%d record(s) in %s render a different value at a variant carrying "
             "more than one gene, and the contract states there is one; the "
             "measurement set and the published sentence no longer agree about "
             "how many records the dependence changes" % (len(changed), records_path))
record = changed[0]
variant, gene = record[iv], record[ig]

# The contract names it when one sentence carries the gene id and the variant.
# An assembly prefix on the variant is the document's to keep or drop.
locus = variant.split(":", 1)[1] if variant.count(":") > 3 else variant
named = None
for sentence in re.split(r"(?<=\.)\s", contract_text):
    if gene in sentence and (variant in sentence or locus in sentence):
        named = sentence
        break
if named is None:
    sys.exit("no sentence in the contract names both %s and %s, so a reader is "
             "told the dependence can change an answer and not which answer it "
             "changed" % (locus, gene))

print("%s %s bundle_gain=%s@%s model_gain=%s genes_at_variant=%d"
      % (locus, gene, record[bg], record[header.index("bundle_gain_position")],
         record[mg], genes_at[variant]))
PY
}

# Read the frozen case and report what it pins, as `key=value` words. Exits 2
# with the reason on standard error when the corpus cannot support the
# sentence.
read_corpus_case() {
    python3 - "$1" "$2" "$3" "$4" <<'PY'
import json
import sys

manifest_path, cases_path, case_id, coverage_tag = sys.argv[1:5]

try:
    manifest = json.load(open(manifest_path, encoding="utf-8"))
except (OSError, ValueError) as error:
    sys.exit("the corpus manifest cannot be read: %s" % error)

if case_id not in manifest.get("case_ids", []):
    sys.exit("the corpus manifest does not list %s among its case ids, so the "
             "behaviour the contract points at is not in the frozen corpus" % case_id)
if coverage_tag not in manifest.get("coverage", []):
    sys.exit("the corpus manifest does not declare %s among what it covers" % coverage_tag)

case = None
try:
    for line in open(cases_path, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        record = json.loads(line)
        if record.get("id") == case_id:
            case = record
            break
except (OSError, ValueError) as error:
    sys.exit("the corpus cases cannot be read: %s" % error)

if case is None:
    sys.exit("%s is not a case in %s, so the contract would point at nothing"
             % (case_id, cases_path))
if coverage_tag not in case.get("coverage", []):
    sys.exit("%s does not carry the %s coverage tag" % (case_id, coverage_tag))

genes = case.get("genes", [])
if len(genes) < 2:
    sys.exit("%s carries %d gene(s), and a claim about one gene's score depending "
             "on another needs at least two over the same variant" % (case_id, len(genes)))

expected = case.get("expected", {})
unmasked = expected.get("unmasked", [])
masked = expected.get("masked", [])
if len(unmasked) < 2 or len(masked) < 2:
    sys.exit("%s does not carry an unmasked and a masked answer for each of its "
             "genes, so it pins no comparison" % case_id)


def answers(entries):
    return [(entry.get("gain_bits"), entry.get("gain_position"),
             entry.get("loss_bits"), entry.get("loss_position")) for entry in entries]


unmasked_answers = answers(unmasked)
masked_answers = answers(masked)

if len(set(unmasked_answers)) != 1:
    sys.exit("the genes of %s already disagree before masking, so a masked "
             "disagreement says nothing about what masking carried over" % case_id)
if len(set(masked_answers)) == 1:
    sys.exit("the genes of %s agree after masking, so the case pins no "
             "gene-to-gene difference for the contract to be about" % case_id)

print("genes=%d unmasked=%d masked=%d" % (len(genes), len(unmasked), len(masked)))
PY
}

check_same_strand() {
    local root=$1 pinned text candidates affirmative mutations named

    [[ -f "$root/$corpus_manifest" ]] || {
        printf 'no %s, so the frozen corpus the contract points at is not here\n' "$corpus_manifest" >&2
        return 1
    }
    [[ -f "$root/$corpus_cases" ]] || {
        printf 'no %s, so the frozen corpus the contract points at is not here\n' "$corpus_cases" >&2
        return 1
    }
    pinned=$(read_corpus_case "$root/$corpus_manifest" "$root/$corpus_cases" "$corpus_case" "$corpus_coverage") || {
        printf '%s\n' "$pinned" >&2
        return 1
    }

    # The corpus comparison has to still read the case, or the case is a record
    # nothing exercises.
    [[ -f "$root/$corpus_mutation_tests" ]] || {
        printf 'no %s, so nothing proves the corpus comparison would notice %s changing\n' \
            "$corpus_mutation_tests" "$corpus_case" >&2
        return 1
    }
    mutations=$(grep -Fc -- "$corpus_case" "$root/$corpus_mutation_tests" || true)
    (( mutations > 0 )) || {
        printf '%s no longer names %s, so nothing proves the frozen comparison would notice that case changing\n' \
            "$corpus_mutation_tests" "$corpus_case" >&2
        return 1
    }

    [[ -f "$root/$contract_relative" ]] || {
        printf 'no %s to carry the published statement\n' "$contract_relative" >&2
        return 1
    }
    text=$(flatten "$root/$contract_relative")

    # The claim: a score can depend on the other same-strand genes over the
    # same variant. One sentence has to name the strand relation, the
    # dependence, and the thing that depends.
    candidates=$(printf '%s' "$text" | sentences \
        | grep -Ei 'same[- ]strand' | grep -Ei 'depend' | grep -Ei 'score|gain|loss' || true)
    [[ -n "$candidates" ]] || {
        printf '%s carries no sentence saying that a score can depend on the other same-strand genes overlapping the same variant, so nothing a consumer reads says the answer for one gene is not computed on its own\n' \
            "$contract_relative" >&2
        return 1
    }
    affirmative=$(affirmative_only "$candidates" 'same[- ]strand' '(score|gain|loss)' || true)
    [[ -n "$affirmative" ]] || {
        printf '%s names the dependence only to deny it: %s\n' \
            "$contract_relative" "$(printf '%s\n' "$candidates" | head -n 1)" >&2
        return 1
    }

    printf '%s' "$text" | grep -Fq -- "$corpus_case" || {
        printf '%s states the dependence without naming %s, so a reader cannot reach the frozen case that pins the behaviour\n' \
            "$contract_relative" "$corpus_case" >&2
        return 1
    }

    [[ -f "$root/$measured_records" ]] || {
        printf 'no %s, so the record the contract names has no measurement to be read out of\n' \
            "$measured_records" >&2
        return 1
    }
    named=$(read_named_record "$root/$measured_records" "$text") || {
        printf '%s\n' "$named" >&2
        return 1
    }

    printf '%s states the same-strand dependence, names %s, which the corpus pins (%s), and names the one measured record it changes (%s)\n' \
        "$contract_relative" "$corpus_case" "$pinned" "$named"
}

# =============================================================================
# 3. the published index size is arithmetic, and says so
# =============================================================================
#
# Three committed numbers reconcile exactly, and this is the check that they
# do:
#
#   gene loci          L   the retained build artifact's measured count
#   REF=N loci         E   the loci the format holds in an exception section
#   bytes per locus    W   the fixed-width format's own width
#   published size     P   what architecture/index.md states for the corpus
#   measured payload   M   what the retained build artifact measured
#
#   P = L x W          the published figure is a product, not a measurement
#   M = (L - E) x W    the payload holds every locus but the exceptions
#   P - M = E x W      and the whole difference is those exception loci
#
# Nothing here re-measures anything: every number is read out of a file the
# repository already carries, and the three identities are what make the
# reading self-checking. A count that drifts in one file and not the other
# fails, which is the staleness the ticket is about.
#
# Then the section that states P has to say it is derived, name what it is
# derived from, carry the measured payload beside it, and account for the
# exception loci. Those four are checked as claims: a word from the small set
# the language has for "this is arithmetic", and the three numbers themselves.
# No sentence is pinned.
#
# Scope. A rule over every byte figure in `architecture/` would be wider than
# the thing it covers: that folder publishes two dozen of them, most of them
# entropy results whose derivation is the subject of the section they stand in.
# The narrow census is the one section that states the corpus size, where a
# second unmeasured figure is the way this defect comes back.

index_relative='architecture/index.md'
index_section='## Selected fixed-width v1'
build_artifact_relative='planning/artifacts/003-full-index-build.md'

# The grouped number in the `| <label> | <number> |` row of a Markdown table.
# Backticks around the label are code markup rather than part of the name, so
# `| `REF=N` loci | 30 |` and `| REF=N loci | 30 |` read the same.
table_cell() {
    awk -v want="$2" -F'|' '
        {
            label = $2
            gsub(/`/, "", label)
            gsub(/^[ \t]+|[ \t]+$/, "", label)
            if (label != want) next
            value = $3
            gsub(/[ \t,]/, "", value)
            if (value ~ /^[0-9]+$/) { print value; exit }
        }
    ' "$1"
}

# Every byte figure in a run of text, ungrouped, one per line. A figure is a
# grouped number standing immediately before `bytes` or before a `-byte`
# compound, which is how this repository writes a size and how it writes
# nothing else.
byte_figures() {
    grep -oE '[0-9][0-9,]*(-byte|[[:space:]]+bytes)' <<<"$1" \
        | grep -oE '^[0-9][0-9,]*' | tr -d ','
}

# The `<grouped number>-byte <label>` figure in a run of text.
labelled_byte_figure() {
    grep -oE "[0-9][0-9,]*-byte $2" <<<"$1" | head -n 1 | grep -oE '^[0-9][0-9,]*' | tr -d ','
}

# Words the language has for saying a figure is arithmetic rather than a
# reading. A section stating a product has to use one of them.
derivation='(derived|derivation|computed|computes|arithmetic|product of|multiplied|multiplying|times the)'

check_index_size() {
    local root=$1 artifact=$1/$build_artifact_relative
    local loci exceptions width published measured text expected_payload difference

    [[ -f "$artifact" ]] || {
        printf 'no %s, so the published corpus size has no measurement to reconcile against\n' \
            "$build_artifact_relative" >&2
        return 1
    }
    [[ -f "$root/$index_relative" ]] || {
        printf 'no %s to carry the published corpus size\n' "$index_relative" >&2
        return 1
    }

    loci=$(table_cell "$artifact" 'Gene loci')
    exceptions=$(table_cell "$artifact" 'REF=N loci')
    measured=$(labelled_byte_figure "$(flatten "$artifact")" 'payload')

    [[ "$loci" =~ ^[0-9]+$ ]] && (( loci > 0 )) || {
        printf '%s records no `Gene loci` count, so nothing says what the published corpus size is a product of\n' \
            "$build_artifact_relative" >&2
        return 1
    }
    [[ "$exceptions" =~ ^[0-9]+$ ]] || {
        printf '%s records no `REF=N loci` count, so nothing accounts for the loci held outside the fixed-width payload\n' \
            "$build_artifact_relative" >&2
        return 1
    }
    [[ "$measured" =~ ^[0-9]+$ ]] && (( measured > 0 )) || {
        printf '%s records no measured payload size, so the published figure has nothing to be reconciled against\n' \
            "$build_artifact_relative" >&2
        return 1
    }

    text=$(section "$root/$index_relative" "$index_section")
    [[ -n "${text// /}" ]] || {
        printf '%s has no `%s` section, so this check read no published size\n' \
            "$index_relative" "$index_section" >&2
        return 1
    }

    width=$(grep -oE '[0-9]+ bytes per locus' <<<"$text" | head -n 1 | grep -oE '^[0-9]+')
    [[ "$width" =~ ^[0-9]+$ ]] && (( width > 0 )) || {
        printf '%s does not state how many bytes the format spends per locus, so its corpus size cannot be checked against anything\n' \
            "$index_section" >&2
        return 1
    }
    # The corpus size is the largest byte figure the section states: the format
    # spends a width per locus and the corpus holds every locus, so nothing
    # else in a section about this format can be bigger.
    local figures
    figures=$(byte_figures "$text" | sort -n -u)
    [[ -n "$figures" ]] || {
        printf '%s carries no byte figure at all, so this check read no published size\n' "$index_section" >&2
        return 1
    }
    published=$(printf '%s\n' "$figures" | tail -n 1)
    (( published > width )) || {
        printf '%s states no corpus size larger than the %s bytes it spends per locus, so it publishes no size for the corpus\n' \
            "$index_section" "$width" >&2
        return 1
    }

    # --- the three identities ---
    (( published == loci * width )) || {
        printf '%s states %s bytes for the complete corpus, and %s loci at %s bytes each is %s; the published figure is not the product it is presented as\n' \
            "$index_relative" "$(grouped "$published")" "$(grouped "$loci")" "$width" \
            "$(grouped $((loci * width)))" >&2
        return 1
    }
    expected_payload=$(( (loci - exceptions) * width ))
    (( measured == expected_payload )) || {
        printf '%s measured a %s-byte payload, and %s loci less %s held in the exception section, at %s bytes each, is %s; the two no longer reconcile\n' \
            "$build_artifact_relative" "$(grouped "$measured")" "$(grouped "$loci")" \
            "$exceptions" "$width" "$(grouped "$expected_payload")" >&2
        return 1
    }
    difference=$(( published - measured ))
    (( difference == exceptions * width )) || {
        printf 'the published corpus size and the measured payload differ by %s bytes, and the %s loci held outside the payload account for %s; the difference is not the exception section\n' \
            "$(grouped "$difference")" "$exceptions" "$(grouped $((exceptions * width)))" >&2
        return 1
    }

    # --- the section says what the figure is ---
    printf '%s' "$text" | grep -Eqi -- "$derivation" || {
        printf '%s states %s bytes for the complete corpus and presents it as a measurement; it is %s loci multiplied by %s, and the section says nothing that tells a reader so\n' \
            "$index_section" "$(grouped "$published")" "$(grouped "$loci")" "$width" >&2
        return 1
    }
    printf '%s' "$text" | grep -Eq "(^|[^0-9,])$(grouped "$loci")([^0-9]|$)" || {
        printf '%s states a derived corpus size without the %s loci it is derived from, so a reader cannot check the arithmetic\n' \
            "$index_section" "$(grouped "$loci")" >&2
        return 1
    }
    printf '%s' "$text" | grep -Eq "(^|[^0-9,])$(grouped "$measured")([^0-9]|$)" || {
        printf '%s states a derived corpus size without the measured %s-byte payload beside it, so a reader checking the figure against a file they downloaded meets only the arithmetic\n' \
            "$index_section" "$(grouped "$measured")" >&2
        return 1
    }
    printf '%s' "$text" | grep -Eq "(^|[^0-9,])$(grouped "$exceptions")([^0-9]|$)" || {
        printf '%s states the payload size without accounting for the %s loci held in the exception section rather than in it\n' \
            "$index_section" "$exceptions" >&2
        return 1
    }

    # --- the narrow census: no second unmeasured figure in that section ---
    local figure unaccounted=
    for figure in $figures; do
        (( figure == published || figure == measured || figure == width )) && continue
        grep -Fq -- "$(grouped "$figure")" "$artifact" && continue
        unaccounted+=" $(grouped "$figure")"
    done
    [[ -z "$unaccounted" ]] || {
        printf '%s publishes byte figure(s)%s that %s does not record and this section does not reconcile, so a reader cannot tell which of its sizes were measured\n' \
            "$index_section" "$unaccounted" "$build_artifact_relative" >&2
        return 1
    }

    printf '%s bytes published, %s bytes measured, %s loci at %s bytes each, %s held outside the payload; all three reconcile and the document says the figure is derived\n' \
        "$(grouped "$published")" "$(grouped "$measured")" "$(grouped "$loci")" \
        "$width" "$exceptions"
}

# =============================================================================
# the checks refuse what they exist to refuse
# =============================================================================
#
# A check that has never been seen to fail is worth nothing, so every refusal
# above is exercised against a fixture repository before the real one is read.
# The accepted half is exercised first: a check that refused its own clean
# fixture would make the repair impossible to write.

cat >"$work/swap.py" <<'PY'
"""Replace one run of words in a wrapped Markdown file, or fail loudly."""
import re
import sys

path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path, encoding="utf-8").read()
pattern = r"\s+".join(re.escape(word) for word in old.split())
replaced, count = re.subn(pattern, new.replace("\\", "\\\\"), text, count=1)
if count == 0:
    sys.exit("swap matched nothing in %s: %s" % (path, old))
open(path, "w", encoding="utf-8").write(replaced)
PY

swap() { python3 "$work/swap.py" "$@"; }

# A fixture repository in the state all three checks exist to require.
plant() {
    local tree=$1
    mkdir -p "$tree/architecture" "$tree/planning/artifacts" \
        "$tree/tests/fixtures/pangolin-compat-v1" "$tree/crates/pangopup-build/tests"

    cat >"$tree/$inventory_relative" <<'COMPAT'
# Compatibility

## v0.5.0 response-shape inventory

- Every score item: adds `data_set_version`.

## Score values

Every score item carries `data_set_version` beside `scoring_identity`. Reaching
it takes no second request.

A gene's score can depend on which other same-strand genes overlap the same
variant, because the scorer masks shared arrays in order. The frozen corpus
pins that behaviour as P01-same-strand-order. It changes one record of the
measured set, chr9:100:G:T in GENE_TWO, where the precomputed route reports
0.09 at -4 and the model reports 0.00.
COMPAT

    cat >"$tree/$measured_records" <<'RECORDS'
variant	stable_gene	bundle_gain	bundle_gain_position	bundle_loss	bundle_loss_position	model_gain	model_gain_position	model_loss	model_loss_position
GRCh38:chr9:100:G:T	GENE_ONE	0.00	-50	0.00	-50	0.00	5	0.00	-50
GRCh38:chr9:100:G:T	GENE_TWO	0.09	-4	0.00	-50	0.00	5	0.00	-50
GRCh38:chr1:200:C:G	GENE_THREE	0.00	-50	0.00	-50	0.01	-2	0.00	-50
GRCh38:chr2:300:T:A	GENE_FOUR	0.00	-50	0.00	-50	0.00	-50	0.00	-50
GRCh38:chr2:300:T:A	GENE_FIVE	0.00	-50	0.00	-50	0.00	-50	0.00	-50
RECORDS

    cat >"$tree/architecture/service.md" <<'SERVICE'
# Service Boundary

## Active scoring identity

The status route and every returned score item expose the same full SHA-256
value. Every returned score item also carries `data_set_version`.

## Something else

Nothing here.
SERVICE

    cat >"$tree/README.md" <<'README'
# PangoPup

## HTTP service

Status and score items share the `scoring_identity`. Every score item carries
`data_set_version` too, so retaining it beside a score takes no second request.

## Docker

Nothing here.
README

    cat >"$tree/$corpus_manifest" <<'MANIFEST'
{"schema":"pangopup-compat-v1","coverage":["postprocess.same_strand_order"],"case_ids":["P01-same-strand-order"]}
MANIFEST

    cat >"$tree/$corpus_cases" <<'CASES'
{"id":"P01-same-strand-order","coverage":["postprocess.same_strand_order"],"genes":[{"id":"GENE_A","boundaries":[99]},{"id":"GENE_B","boundaries":[101]}],"expected":{"unmasked":[{"gene":"GENE_A","gain_bits":"3f4ccccd","gain_position":-1,"loss_bits":"bf19999a","loss_position":-1},{"gene":"GENE_B","gain_bits":"3f4ccccd","gain_position":-1,"loss_bits":"bf19999a","loss_position":-1}],"masked":[{"gene":"GENE_A","gain_bits":"3f333333","gain_position":1,"loss_bits":"bf19999a","loss_position":-1},{"gene":"GENE_B","gain_bits":"3e99999a","gain_position":2,"loss_bits":"00000000","loss_position":-2}]}}
CASES

    cat >"$tree/$corpus_mutation_tests" <<'TESTS'
// Mutating the frozen case proves the comparison would notice it changing.
line_mutation(cases, "P01-same-strand-order", |line| line);
TESTS

    # 1,000,000 loci, 30 of them held in the exception section, at 11 bytes
    # each: 11,000,000 published and 10,999,670 measured, 330 bytes apart.
    cat >"$tree/$build_artifact_relative" <<'ARTIFACT'
# Full index build

The writer lifecycle guarantees that the synced final index coexists before
return with the 10,999,670-byte payload scratch.

| Measure | Value |
| --- | --- |
| Gene loci | 1,000,000 |
| REF=N loci | 30 |
ARTIFACT

    cat >"$tree/$index_relative" <<'INDEX'
# Precomputed SNV Index

## Selected fixed-width v1

Three 28-bit score records plus a three-bit reference fit in 87 bits, or 11
bytes per locus. Over the complete corpus that is a derived 11,000,000 bytes,
the product of 1,000,000 loci and that width. The builder measured the payload
at 10,999,670 bytes, because 30 `N` loci are held in an exception section
rather than in the fixed-width payload.

## Something else

Nothing here.
INDEX
}

# A fixture repository with one thing changed. A mutation that changes no byte
# is a broken fixture, not a passing check, so the tree is fingerprinted around
# it.
mutate() {
    local tree="$work/$1" before after
    shift
    plant "$tree"
    before=$(find "$tree" -type f -exec md5sum {} + | sort | md5sum)
    ( cd "$tree" && "$@" )
    after=$(find "$tree" -type f -exec md5sum {} + | sort | md5sum)
    [[ "$before" != "$after" ]] || fail "the mutation for $tree changed nothing, so its case proves nothing"
    printf '%s' "$tree"
}

expect_refusal() {
    local check=$1 tree=$2 wanted=$3 output status
    set +e
    output=$("$check" "$tree" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "$check accepted a repository it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so an operator cannot act on it: $output" ;;
    esac
}

drop_line() {
    local target=$1 pattern=$2
    grep -qE -- "$pattern" "$target" || fail "the fixture edit for $target matched nothing"
    sed -i -E "/$pattern/d" "$target"
}

clean="$work/clean"
plant "$clean"
for check in check_stored_version check_same_strand check_index_size; do
    "$check" "$clean" >/dev/null \
        || fail "$check refused the state it exists to require, so it can never go green"
done

# --- 1. the stored version --------------------------------------------------
expect_refusal check_stored_version "$(mutate sv-status-only swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'The status route also publishes `data_set_version`.')" \
    'carries no sentence naming both a score item and `data_set_version`'
expect_refusal check_stored_version "$(mutate sv-readme-status-only swap README.md \
    'Every score item carries `data_set_version` too, so retaining it beside a score takes no second request.' \
    'Store `data_set_version` as the data-set version when a system has one version field.')" \
    'carries no sentence naming both a score item and `data_set_version`'
expect_refusal check_stored_version "$(mutate sv-reversed swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'A returned score item does not carry `data_set_version`.')" \
    'only to deny it'
expect_refusal check_stored_version "$(mutate sv-no-section drop_line architecture/service.md \
    '^## Active scoring identity$')" \
    'has no `## Active scoring identity` section'
expect_refusal check_stored_version "$(mutate sv-inventory-dropped sed -i \
    '/score item/d' "$inventory_relative")" \
    'no longer enumerates `data_set_version` on the score item'

expect_refusal check_stored_version "$(mutate sv-no-item swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'No returned score item carries `data_set_version`.')" \
    'only to deny it'
expect_refusal check_stored_version "$(mutate sv-route-not-item swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'The status route, not the score item, publishes `data_set_version`.')" \
    'only to deny it'
expect_refusal check_stored_version "$(mutate sv-absent swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    '`data_set_version` is absent from every returned score item.')" \
    'only to deny it'

# An ordinary word is not a denial. `nor` stands inside "ignore", "minor" and
# "honor", and `is not` inside "this notation", so without word boundaries each
# of these stated the claim and was refused for stating it.
for ordinary in \
    'Every returned score item also carries `data_set_version`, so a consumer can ignore the status route.' \
    'Every returned score item also carries `data_set_version`, a minor addition this notation spells out.' \
    'Every returned score item also carries `data_set_version` to honor the inventory.'
do
    ordinary_tree=$(mutate "sv-ordinary-$(printf '%s' "$ordinary" | cksum | cut -d' ' -f1)" \
        swap architecture/service.md \
        'Every returned score item also carries `data_set_version`.' "$ordinary")
    check_stored_version "$ordinary_tree" >/dev/null \
        || fail "the stored-version check read an ordinary word as a denial: $ordinary"
done

# The claim span. A negation after the claim has been made is about something
# else, and the sentence the repair is most likely to be written as says so.
after_tree=$(mutate sv-negation-after-claim swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'Every returned score item also carries `data_set_version`, so retaining it beside a score takes no second request and does not cost one.')
check_stored_version "$after_tree" >/dev/null \
    || fail 'the stored-version check read a negation standing after the claim as a denial of it'

# A copy edit is not a regression. The sentence is rewritten from end to end
# and the check still passes, which is what keeps it from pinning a wording.
edited=$(mutate sv-copy-edited swap architecture/service.md \
    'Every returned score item also carries `data_set_version`.' \
    'Alongside that identity, `data_set_version` rides on each returned score item as well.')
check_stored_version "$edited" >/dev/null \
    || fail 'the stored-version check refused a rewritten sentence that still states the claim, so it pins a form of words rather than the claim'

# --- 2. the same-strand dependence -----------------------------------------
expect_refusal check_same_strand "$(mutate ss-unstated swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant, because the scorer masks shared arrays in order." \
    'Each gene is scored over the variant.')" \
    'carries no sentence saying that a score can depend'
expect_refusal check_same_strand "$(mutate ss-reversed swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant," \
    "A gene's score does not depend on which other same-strand genes overlap the same variant,")" \
    'only to deny it'
expect_refusal check_same_strand "$(mutate ss-uncited swap "$inventory_relative" \
    'The frozen corpus pins that behaviour as P01-same-strand-order.' \
    'The frozen corpus pins that behaviour.')" \
    'without naming P01-same-strand-order'
expect_refusal check_same_strand "$(mutate ss-case-gone rm -f "$corpus_cases")" \
    'the frozen corpus the contract points at is not here'
expect_refusal check_same_strand "$(mutate ss-unlisted sed -i \
    's/"P01-same-strand-order"\]/"P02-other"]/' "$corpus_manifest")" \
    'does not list P01-same-strand-order among its case ids'
expect_refusal check_same_strand "$(mutate ss-one-gene sed -i \
    's/,{"id":"GENE_B","boundaries":\[101\]}//' "$corpus_cases")" \
    'and a claim about one gene'
expect_refusal check_same_strand "$(mutate ss-masked-agree sed -i \
    's/"gain_bits":"3e99999a","gain_position":2,"loss_bits":"00000000","loss_position":-2/"gain_bits":"3f333333","gain_position":1,"loss_bits":"bf19999a","loss_position":-1/' \
    "$corpus_cases")" \
    'agree after masking'
expect_refusal check_same_strand "$(mutate ss-unmasked-differ sed -i \
    's/{"gene":"GENE_B","gain_bits":"3f4ccccd","gain_position":-1,"loss_bits":"bf19999a","loss_position":-1}/{"gene":"GENE_B","gain_bits":"3e4ccccd","gain_position":-3,"loss_bits":"be4ccccd","loss_position":-3}/' \
    "$corpus_cases")" \
    'already disagree before masking'
expect_refusal check_same_strand "$(mutate ss-unmutated sed -i \
    's/P01-same-strand-order/P02-other/' "$corpus_mutation_tests")" \
    'no longer names P01-same-strand-order'

expect_refusal check_same_strand "$(mutate ss-record-unnamed swap "$inventory_relative" \
    'It changes one record of the measured set, chr9:100:G:T in GENE_TWO, where the precomputed route reports 0.09 at -4 and the model reports 0.00.' \
    'It changes one record of the measured set.')" \
    'names both chr9:100:G:T and GENE_TWO'
expect_refusal check_same_strand "$(mutate ss-record-wrong-gene swap "$inventory_relative" \
    'chr9:100:G:T in GENE_TWO' 'chr9:100:G:T in GENE_NINE')" \
    'names both chr9:100:G:T and GENE_TWO'
expect_refusal check_same_strand "$(mutate ss-record-wrong-locus swap "$inventory_relative" \
    'chr9:100:G:T in GENE_TWO' 'chr9:900:G:T in GENE_TWO')" \
    'names both chr9:100:G:T and GENE_TWO'
expect_refusal check_same_strand "$(mutate ss-record-not-alone sed -i \
    's/^GRCh38:chr2:300:T:A\tGENE_FIVE\t0.00\t-50\t0.00\t-50\t0.00/GRCh38:chr2:300:T:A\tGENE_FIVE\t0.00\t-50\t0.00\t-50\t0.04/' \
    "$measured_records")" \
    'and the contract states there is one'
expect_refusal check_same_strand "$(mutate ss-records-gone rm -f "$measured_records")" \
    'has no measurement to be read out of'

expect_refusal check_same_strand "$(mutate ss-independent swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant, because the scorer masks shared arrays in order." \
    "A gene's score is independent of which other same-strand genes overlap the same variant.")" \
    'only to deny it'
expect_refusal check_same_strand "$(mutate ss-no-effect swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant, because the scorer masks shared arrays in order." \
    "Which other same-strand genes overlap a variant has no effect on the gain and loss a gene depends on.")" \
    'only to deny it'
expect_refusal check_same_strand "$(mutate ss-unaffected swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant, because the scorer masks shared arrays in order." \
    "A gene's score is unaffected by which other same-strand genes it depends beside.")" \
    'only to deny it'

copy_edited=$(mutate ss-copy-edited swap "$inventory_relative" \
    "A gene's score can depend on which other same-strand genes overlap the same variant, because the scorer masks shared arrays in order." \
    'Which same-strand genes overlap a variant is something the gain and loss a gene reports can depend on, since the masks are applied to shared arrays in turn.')
record_edited=$(mutate ss-record-copy-edited swap "$inventory_relative" \
    'It changes one record of the measured set, chr9:100:G:T in GENE_TWO, where the precomputed route reports 0.09 at -4 and the model reports 0.00.' \
    'Of the whole measured set only GENE_TWO at chr9:100:G:T reads differently, 0.09 at -4 from the published dataset against 0.00 from the model.')
check_same_strand "$record_edited" >/dev/null \
    || fail 'the same-strand check refused a rewritten sentence that still names the measured record, so it pins a form of words rather than the record'

check_same_strand "$copy_edited" >/dev/null \
    || fail 'the same-strand check refused a rewritten sentence that still states the claim, so it pins a form of words rather than the claim'

# --- 3. the index size ------------------------------------------------------
expect_refusal check_index_size "$(mutate ix-presented-as-measured swap "$index_relative" \
    'that is a derived 11,000,000 bytes, the product of 1,000,000 loci and that width' \
    'that is 11,000,000 bytes')" \
    'presents it as a measurement'
expect_refusal check_index_size "$(mutate ix-no-measured-payload swap "$index_relative" \
    'The builder measured the payload at 10,999,670 bytes, because 30 `N` loci are held in an exception section rather than in the fixed-width payload.' \
    'The exception section is small.')" \
    'without the measured 10,999,670-byte payload beside it'
expect_refusal check_index_size "$(mutate ix-no-exception-account swap "$index_relative" \
    'because 30 `N` loci are held in an exception section rather than in the fixed-width payload' \
    'for reasons the builder records')" \
    'without accounting for the'
expect_refusal check_index_size "$(mutate ix-no-source swap "$index_relative" \
    'the product of 1,000,000 loci and that width' 'a derived figure')" \
    'without the 1,000,000 loci it is derived from'
expect_refusal check_index_size "$(mutate ix-product-drifted swap "$index_relative" \
    'a derived 11,000,000 bytes' 'a derived 11,000,001 bytes')" \
    'is not the product it is presented as'
expect_refusal check_index_size "$(mutate ix-payload-drifted sed -i \
    's/10,999,670-byte payload/10,999,600-byte payload/' "$build_artifact_relative")" \
    'the two no longer reconcile'
expect_refusal check_index_size "$(mutate ix-exceptions-dropped drop_line \
    "$build_artifact_relative" '^\| REF=N loci \|')" \
    'records no `REF=N loci` count'
expect_refusal check_index_size "$(mutate ix-loci-dropped drop_line \
    "$build_artifact_relative" '^\| Gene loci \|')" \
    'records no `Gene loci` count'
expect_refusal check_index_size "$(mutate ix-no-width swap "$index_relative" \
    'or 11 bytes per locus' 'or eleven bytes each')" \
    'does not state how many bytes the format spends per locus'
expect_refusal check_index_size "$(mutate ix-second-unmeasured swap "$index_relative" \
    'rather than in the fixed-width payload.' \
    'rather than in the fixed-width payload. The directories add 4,096 bytes.')" \
    'that planning/artifacts/003-full-index-build.md does not record'
expect_refusal check_index_size "$(mutate ix-no-section drop_line "$index_relative" \
    '^## Selected fixed-width v1$')" \
    'has no `## Selected fixed-width v1` section'

index_edited=$(mutate ix-copy-edited swap "$index_relative" \
    'Over the complete corpus that is a derived 11,000,000 bytes, the product of 1,000,000 loci and that width.' \
    'Across the whole corpus that comes to 11,000,000 bytes, computed as 1,000,000 loci times that width rather than read off a file.')
check_index_size "$index_edited" >/dev/null \
    || fail 'the index-size check refused a rewritten sentence that still states the claim, so it pins a form of words rather than the claim'

# =============================================================================
# the real repository
# =============================================================================

status=0
for check in check_stored_version check_same_strand check_index_size; do
    if result=$("$check" "$repository" 2>&1); then
        printf 'published claim evidence: %s\n' "$result"
    else
        printf '%s\n' "$result" >&2
        status=1
    fi
done
exit "$status"
