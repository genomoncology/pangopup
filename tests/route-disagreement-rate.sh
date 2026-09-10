#!/usr/bin/env bash
set -euo pipefail

# The published contract states how often the precomputed route and the modeled
# route disagree. That number came from a measurement nothing in the repository
# re-runs on every build: it reads the shipped dataset and runs the production
# model over a named variant set, which is minutes of work and gigabytes of
# assets. A gate cannot repeat it and must not pretend to.
#
# What a gate can hold is the join between the number and its evidence. The
# measurement writes one durable artifact carrying a machine-readable
# `measurement` block, and the two published documents restate what that block
# says. This file refuses any state in which those disagree:
#
#   1. The artifact exists, carries every field the statement needs, and states
#      how to re-run itself.
#   2. Its counts are internally consistent: each published percentage is the
#      arithmetic of the counts beside it, over a denominator larger than zero.
#      A measurement over no records proves nothing and is refused here.
#   3. The named variant set is reachable and large enough to state a rate. Its
#      manifest is a committed file holding exactly the number of variants the
#      artifact declares, so a later reader re-runs the same set instead of a
#      list that lived in a shell; the artifact states the rule the set was
#      drawn by; and the set and both denominators clear their floors. One
#      disagreement in five records is the defect this ticket exists to fix, so
#      a rate published over a handful of records is refused here.
#   4. The two published documents no longer disclaim a rate, carry the
#      artifact's own numbers, link to it, and report value disagreement and
#      position disagreement as two separate numbers -- a sentence about values
#      carrying the value figure, a sentence about positions carrying the
#      position figure. An edit that collapses the pair into one number loses
#      one of those sentences and fails here.
#   5. The artifact's sentence about zero scores appears in the published
#      contract word for word, so how zeros were treated cannot be dropped from
#      the statement while staying in the evidence. Its sentence about the
#      limit of the evidence appears there word for word too.
#
# What this does not prove: that the measurement is correct. This check reads a
# recorded artifact and never re-runs the measurement, so numbers typed by hand
# into that artifact would satisfy it. Nothing short of re-running the
# measurement closes that, which is why the contract has to say so itself. The
# `evidence-limit` sentence is that disclosure, and check 5 holds it in the
# published text rather than leaving it in this comment.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

artifact_relative='planning/artifacts/0059-route-disagreement-rate.md'
compatibility_relative='architecture/compatibility.md'
compatibility_section='## Score values'
score_value_relative='spec/score-value.md'
score_value_section='## The two routes'

# Every field the published statement is made of. A statement missing any one of
# them is a number without the context that makes it readable.
counts=(variant-set-size compared-records value-disagreements \
    position-compared-records position-disagreements)
percents=(value-disagreement-percent position-disagreement-percent)
required=(variant-set variant-set-rule variant-set-manifest "${counts[@]}" \
    "${percents[@]}" zero-score-treatment evidence-limit measured)

# A rate needs enough records to be a rate. The ticket's complaint is that one
# disagreement in five records supports no number at all, so a set or a
# denominator too small to state one is refused here. The position denominator
# carries a lower floor because both routes score most records zero and a zero
# score carries no comparable position, which leaves the position-comparable
# subset a fraction of the value one.
minimum_variants=1000
minimum_compared=1000
minimum_position_compared=100

fail() { printf 'route disagreement rate: %s\n' "$*" >&2; exit 1; }

# The `## <heading>` section of a Markdown file, up to the next `## ` heading.
section() {
    awk -v want="$2" '
        $0 == want { on = 1; next }
        on && /^## / { exit }
        on { print }
    ' "$1"
}

# One `key: value` line out of the artifact's fenced `measurement` block.
field() {
    awk -v key="$2" '
        /^```measurement$/ { on = 1; next }
        on && /^```/ { exit }
        on && index($0, key ": ") == 1 { print substr($0, length(key) + 3); exit }
    ' "$1"
}

# 12345 -> 12,345. The published prose groups large counts; the artifact does
# not, so a count is looked for in both spellings.
grouped() { printf '%s' "$1" | sed -E ':a;s/([0-9]+)([0-9]{3})/\1,\2/;ta'; }

# Refuse the repository rooted at $1. Prints its reason on refusal and its
# counts on acceptance.
examine() {
    local root=$1 artifact=$1/$artifact_relative
    local key value manifest rows document relative heading text sentences

    [[ -f "$artifact" ]] || {
        printf 'no %s: the published rate has no evidence behind it\n' "$artifact_relative" >&2
        return 1
    }

    declare -A measured=()
    for key in "${required[@]}"; do
        value=$(field "$artifact" "$key")
        [[ -n "$value" ]] || {
            printf 'the measurement block in %s carries no %s, so the published statement rests on a field nothing recorded\n' \
                "$artifact_relative" "$key" >&2
            return 1
        }
        measured[$key]=$value
    done

    for key in "${counts[@]}"; do
        [[ ${measured[$key]} =~ ^[0-9]+$ ]] || {
            printf '%s in %s reads %s, which is not a count\n' "$key" "$artifact_relative" "${measured[$key]}" >&2
            return 1
        }
    done
    for key in "${percents[@]}"; do
        [[ ${measured[$key]} =~ ^[0-9]+\.[0-9]{2}$ ]] || {
            printf '%s in %s reads %s, which is not a percentage in hundredths\n' "$key" "$artifact_relative" "${measured[$key]}" >&2
            return 1
        }
    done
    [[ ${measured[measured]} =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || {
        printf 'the measurement date in %s reads %s, not a YYYY-MM-DD date\n' "$artifact_relative" "${measured[measured]}" >&2
        return 1
    }

    grep -q '^## Method$' "$artifact" || {
        printf '%s has no `## Method` section, so the rule its variant set was drawn by is written down nowhere\n' "$artifact_relative" >&2
        return 1
    }

    grep -q '^## Reproducing$' "$artifact" || {
        printf '%s has no `## Reproducing` section, so a later reader cannot get the same number again\n' "$artifact_relative" >&2
        return 1
    }

    # --- the counts hold up on their own ---
    local numerator denominator percent
    local -a pairs=(
        "value-disagreements compared-records value-disagreement-percent"
        "position-disagreements position-compared-records position-disagreement-percent"
    )
    local pair
    for pair in "${pairs[@]}"; do
        read -r numerator denominator percent <<<"$pair"
        (( measured[$denominator] > 0 )) || {
            printf '%s in %s is 0, so %s was measured over nothing and states no rate\n' \
                "$denominator" "$artifact_relative" "$percent" >&2
            return 1
        }
        (( measured[$numerator] <= measured[$denominator] )) || {
            printf '%s (%s) exceeds %s (%s) in %s\n' "$numerator" "${measured[$numerator]}" \
                "$denominator" "${measured[$denominator]}" "$artifact_relative" >&2
            return 1
        }
        awk -v n="${measured[$numerator]}" -v d="${measured[$denominator]}" \
            -v p="${measured[$percent]}" \
            'BEGIN { exact = 100 * n / d; diff = exact - p; if (diff < 0) diff = -diff; exit (diff <= 0.005000001) ? 0 : 1 }' || {
            printf '%s in %s reads %s, but %s of %s is not that percentage\n' \
                "$percent" "$artifact_relative" "${measured[$percent]}" \
                "${measured[$numerator]}" "${measured[$denominator]}" >&2
            return 1
        }
    done

    # --- the named variant set is reachable ---
    manifest=$root/${measured[variant-set-manifest]}
    [[ -f "$manifest" ]] || {
        printf 'the variant set names %s, which is not a file in the repository, so nobody can re-run the set\n' \
            "${measured[variant-set-manifest]}" >&2
        return 1
    }
    rows=$(grep -cvE '^\s*(#|$)' "$manifest" || true)
    (( rows == measured[variant-set-size] )) || {
        printf '%s holds %s variant(s) and %s declares a set of %s\n' \
            "${measured[variant-set-manifest]}" "$rows" "$artifact_relative" "${measured[variant-set-size]}" >&2
        return 1
    }

    # --- the set and its denominators are large enough to state a rate ---
    (( measured[variant-set-size] >= minimum_variants )) || {
        printf 'the variant set holds %s variant(s), fewer than the %s a published rate needs\n' \
            "${measured[variant-set-size]}" "$minimum_variants" >&2
        return 1
    }
    (( measured[compared-records] >= minimum_compared )) || {
        printf 'compared-records is %s, fewer than the %s a published value rate needs\n' \
            "${measured[compared-records]}" "$minimum_compared" >&2
        return 1
    }
    (( measured[position-compared-records] >= minimum_position_compared )) || {
        printf 'position-compared-records is %s, fewer than the %s a published position rate needs\n' \
            "${measured[position-compared-records]}" "$minimum_position_compared" >&2
        return 1
    }

    # --- the published documents say what the artifact measured ---
    for document in \
        "$compatibility_relative|$compatibility_section|full" \
        "$score_value_relative|$score_value_section|rates"; do
        IFS='|' read -r relative heading key <<<"$document"
        [[ -f "$root/$relative" ]] || {
            printf 'no %s to hold the published rate against %s\n' "$relative" "$artifact_relative" >&2
            return 1
        }
        # The published prose is wrapped, so every comparison below reads it as
        # one flat run of words. A reflow is a copy edit and must not fail this.
        text=$(section "$root/$relative" "$heading" | tr '\n' ' ' | tr -s ' ')
        [[ -n "${text// /}" ]] || {
            printf '%s has no `%s` section, so this check read no published statement\n' "$relative" "$heading" >&2
            return 1
        }

        if printf '%s' "$text" | grep -qiE 'no rate of disagreement|claims no rate'; then
            printf '%s still disclaims a rate of disagreement while %s records one\n' "$relative" "$artifact_relative" >&2
            return 1
        fi

        printf '%s' "$text" | grep -qF -- "$artifact_relative" || {
            printf '%s states a rate without linking to %s, so a reader cannot reach the evidence\n' \
                "$relative" "$artifact_relative" >&2
            return 1
        }

        # Value disagreement and position disagreement are two numbers. Each
        # has to stand in a sentence naming what it is a disagreement about.
        sentences=$(printf '%s' "$text" | sed -E 's/\. /.\n/g')
        printf '%s' "$sentences" | grep -F -- "${measured[value-disagreement-percent]}" | grep -qi 'value' || {
            printf '%s carries no sentence reporting %s as the value disagreement %s records\n' \
                "$relative" "${measured[value-disagreement-percent]}" "$artifact_relative" >&2
            return 1
        }
        printf '%s' "$sentences" | grep -F -- "${measured[position-disagreement-percent]}" | grep -qi 'position' || {
            printf '%s carries no sentence reporting %s as the position disagreement %s records\n' \
                "$relative" "${measured[position-disagreement-percent]}" "$artifact_relative" >&2
            return 1
        }

        [[ "$key" == full ]] || continue

        # The contract records the set, its size and the date measured, and
        # states how zero scores were treated in the artifact's own words.
        printf '%s' "$text" | grep -qF -- "${measured[variant-set]}" || {
            printf '%s states a rate without naming the variant set %s\n' "$relative" "${measured[variant-set]}" >&2
            return 1
        }
        printf '%s' "$text" | grep -qF -- "${measured[measured]}" || {
            printf '%s states a rate without the date %s it was measured\n' "$relative" "${measured[measured]}" >&2
            return 1
        }
        printf '%s' "$text" | grep -qE -- "$(grouped "${measured[variant-set-size]}")|${measured[variant-set-size]}" || {
            printf '%s states a rate without the size of the set it was measured over\n' "$relative" >&2
            return 1
        }
        printf '%s' "$text" | grep -qF -- "${measured[zero-score-treatment]}" || {
            printf '%s does not carry how zero scores were treated: %s says "%s"\n' \
                "$relative" "$artifact_relative" "${measured[zero-score-treatment]}" >&2
            return 1
        }
        # No gate re-runs this measurement, so the contract states that limit
        # where the number is read instead of leaving it in the evidence file.
        printf '%s' "$text" | grep -qF -- "${measured[evidence-limit]}" || {
            printf '%s does not carry the limit of its own evidence: %s says "%s"\n' \
                "$relative" "$artifact_relative" "${measured[evidence-limit]}" >&2
            return 1
        }
    done

    printf '%s disagreement on value (%s%%) and %s on position (%s%%) over %s, published and recorded alike\n' \
        "${measured[value-disagreements]}" "${measured[value-disagreement-percent]}" \
        "${measured[position-disagreements]}" "${measured[position-disagreement-percent]}" \
        "${measured[variant-set]}"
}

# --- the check refuses what it exists to refuse ------------------------------
#
# A gate that has never been seen to fail is worth nothing, so every refusal
# above is exercised against a fixture repository before the real one is read.

fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT

cat >"$fixtures/swap.py" <<'PY'
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

# A fixture repository in the state this check exists to require: artifact,
# reachable variant set, and both documents restating what it measured.
plant() {
    local tree=$1
    mkdir -p "$tree/planning/artifacts" "$tree/architecture" "$tree/spec"

    cat >"$tree/$artifact_relative" <<'ARTIFACT'
# Ticket 0059 -- how often the two routes disagree

```measurement
variant-set: fixture-set-v1
variant-set-rule: Every SNV on the fixture contig in dataset order.
variant-set-manifest: planning/artifacts/fixture-set.tsv
variant-set-size: 1000
compared-records: 1200
value-disagreements: 24
value-disagreement-percent: 2.00
position-compared-records: 200
position-disagreements: 3
position-disagreement-percent: 1.50
zero-score-treatment: A record carrying a zero score on either route is left out of the position comparison and kept in the value comparison.
evidence-limit: This rate was measured once against the shipped assets and no gate re-runs it.
measured: 2026-09-10
```

## Method

Every SNV on the fixture contig in dataset order.

## Reproducing

```
bash planning/artifacts/fixture-rerun.sh
```
ARTIFACT

    {
        printf '# a comment\n\n'
        seq 1 1000 | sed -E 's|^|GRCh38:chr1:|; s|$|:A:T|'
    } >"$tree/planning/artifacts/fixture-set.tsv"

    cat >"$tree/$compatibility_relative" <<'COMPAT'
# Compatibility

## Score values

A precomputed score and a modeled score are not interchangeable. Both routes
were run over fixture-set-v1, a set of 1,000 variants, on 2026-09-10. The two
routes report a different value on 2.00 percent of the compared records. They
report a different position on 1.50 percent of the records comparable on
position. A record carrying a zero score on either route is left out of the
position comparison and kept in the value comparison. This rate was measured
once against the shipped assets and no gate re-runs it. The measurement is
[the artifact](../planning/artifacts/0059-route-disagreement-rate.md).

## What a consumer pins

Nothing here.
COMPAT

    cat >"$tree/$score_value_relative" <<'SPEC'
# What a score value is

## The two routes

The two routes disagree on the value for 2.00 percent of the compared records
and on the position for 1.50 percent of the records comparable on position. The
measurement is
[the artifact](../planning/artifacts/0059-route-disagreement-rate.md).

## What a deployment setting moves

Nothing here.
SPEC
}

expect_refusal() {
    local tree=$1 wanted=$2 output status
    set +e
    output=$(examine "$tree" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the check accepted a repository it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so an operator cannot act on it: $output" ;;
    esac
}

# Replace one run of words in a wrapped Markdown file. Whitespace in the pattern
# matches any wrapping, so a fixture edit reads the way the sentence reads.
swap() {
    python3 "$fixtures/swap.py" "$@"
}

# Delete every line matching an extended regular expression.
drop() {
    local target=$1 pattern=$2
    grep -qE -- "$pattern" "$target" || fail "the fixture edit for $target matched nothing"
    sed -i -E "/$pattern/d" "$target"
}

# Rewrite one whole `key: value` line of the artifact's measurement block.
retype() {
    local target=$1 key=$2 value=$3
    grep -q "^$key: " "$target" || fail "the fixture edit for $target matched no $key"
    sed -i -E "s|^$key: .*|$key: $value|" "$target"
}

# Rewrite several `key: value` lines of the measurement block together, for a
# case whose one change has to keep the block's arithmetic consistent.
retype_many() {
    local target=$1
    shift
    while (( $# )); do
        retype "$target" "$1" "$2"
        shift 2
    done
}

# Shrink the fixture's variant set to a handful, manifest and declaration alike.
shrink_set() {
    local target=$1 rows=$2
    printf 'GRCh38:chr1:1:A:T\nGRCh38:chr1:2:A:T\nGRCh38:chr1:3:A:T\nGRCh38:chr1:4:A:T\n' \
        | head -n "$rows" >"$target"
    retype "$artifact_relative" variant-set-size "$rows"
}

# A fixture repository with one thing changed. `$@` after the name is a command
# run inside the tree. A mutation that changes no byte is a broken fixture, not
# a passing check, so the tree is fingerprinted around it.
mutate() {
    local tree="$fixtures/$1" before after
    shift
    plant "$tree"
    before=$(find "$tree" -type f -exec md5sum {} + | sort | md5sum)
    ( cd "$tree" && "$@" )
    after=$(find "$tree" -type f -exec md5sum {} + | sort | md5sum)
    [[ "$before" != "$after" ]] || fail "the mutation for $tree changed nothing, so its case proves nothing"
    printf '%s' "$tree"
}

expect_refusal "$(mutate missing-artifact rm -f "$artifact_relative")" \
    'has no evidence behind it'
expect_refusal "$(mutate missing-field drop "$artifact_relative" '^value-disagreements: ')" \
    'carries no value-disagreements'
expect_refusal "$(mutate empty-field retype "$artifact_relative" measured '')" \
    'carries no measured'
expect_refusal "$(mutate unreadable-count retype "$artifact_relative" compared-records many)" \
    'which is not a count'
expect_refusal "$(mutate unreadable-percent retype "$artifact_relative" value-disagreement-percent '20%')" \
    'not a percentage in hundredths'
expect_refusal "$(mutate unreadable-date retype "$artifact_relative" measured 'last Tuesday')" \
    'not a YYYY-MM-DD date'
expect_refusal "$(mutate no-rerun drop "$artifact_relative" '^## Reproducing$')" \
    'cannot get the same number again'
expect_refusal "$(mutate empty-denominator retype "$artifact_relative" position-compared-records 0)" \
    'was measured over nothing'
expect_refusal "$(mutate impossible-count retype "$artifact_relative" value-disagreements 9000)" \
    'exceeds compared-records'
expect_refusal "$(mutate bad-arithmetic retype "$artifact_relative" value-disagreement-percent 9.00)" \
    'is not that percentage'
expect_refusal "$(mutate unreachable-set rm -f planning/artifacts/fixture-set.tsv)" \
    'not a file in the repository'
expect_refusal "$(mutate wrong-size retype "$artifact_relative" variant-set-size 1001)" \
    'declares a set of 1001'
expect_refusal "$(mutate no-method drop "$artifact_relative" '^## Method$')" \
    'written down nowhere'
expect_refusal "$(mutate handful-of-variants shrink_set planning/artifacts/fixture-set.tsv 4)" \
    'fewer than the 1000 a published rate needs'
expect_refusal "$(mutate handful-of-records retype_many "$artifact_relative" \
    compared-records 500 value-disagreements 10)" \
    'compared-records is 500, fewer than the 1000'
expect_refusal "$(mutate handful-of-positions retype_many "$artifact_relative" \
    position-compared-records 40 position-disagreements 1 position-disagreement-percent 2.50)" \
    'position-compared-records is 40, fewer than the 100'
expect_refusal "$(mutate still-disclaims swap "$compatibility_relative" \
    'A precomputed score and a modeled score are not interchangeable.' \
    'No rate of disagreement is claimed.')" \
    'still disclaims a rate'
expect_refusal "$(mutate unlinked swap "$compatibility_relative" \
    '[the artifact](../planning/artifacts/0059-route-disagreement-rate.md)' 'somewhere')" \
    'cannot reach the evidence'
expect_refusal "$(mutate drifted-number swap "$compatibility_relative" \
    'a different value on 2.00 percent' 'a different value on 1.90 percent')" \
    'no sentence reporting 2.00 as the value disagreement'
expect_refusal "$(mutate collapsed-pair swap "$compatibility_relative" \
    'They report a different position on 1.50 percent of the records comparable on position.' '')" \
    'no sentence reporting 1.50 as the position disagreement'
expect_refusal "$(mutate spec-drifted swap "$score_value_relative" \
    'on the position for 1.50 percent' 'on the position for 2.00 percent')" \
    'no sentence reporting 1.50 as the position disagreement'
expect_refusal "$(mutate unnamed-set swap "$compatibility_relative" \
    'over fixture-set-v1, a set of 1,000 variants' 'over a set of 1,000 variants')" \
    'without naming the variant set'
expect_refusal "$(mutate undated swap "$compatibility_relative" ', on 2026-09-10.' '.')" \
    'without the date'
expect_refusal "$(mutate unsized swap "$compatibility_relative" \
    'a set of 1,000 variants' 'a set of variants')" \
    'without the size of the set'
expect_refusal "$(mutate silent-on-zeros swap "$compatibility_relative" \
    'A record carrying a zero score on either route is left out of the position comparison and kept in the value comparison.' \
    'Zeros went somewhere.')" \
    'does not carry how zero scores were treated'
expect_refusal "$(mutate silent-on-limit swap "$compatibility_relative" \
    'This rate was measured once against the shipped assets and no gate re-runs it.' \
    'The measurement is sound.')" \
    'does not carry the limit of its own evidence'
expect_refusal "$(mutate no-section drop "$compatibility_relative" '^## Score values$')" \
    'read no published statement'

clean="$fixtures/clean"
plant "$clean"
examine "$clean" >/dev/null \
    || fail 'the check refused the state it exists to require, so it can never go green'

# --- the real repository -----------------------------------------------------
examine "$repository" || exit 1
