#!/usr/bin/env bash
set -euo pipefail

# A reader arriving at this repository learns what the software does and never
# learns why it exists or whether they may use it. Three things are missing,
# and this file holds all three.
#
#   1. Why the project exists, and what another party's licence has to do with
#      it. Every claim this repository makes about somebody else's licence or
#      service is a claim about something that can change under it, so each one
#      carries the source it came from and the date that source was read. This
#      file reads that requirement out of the material itself: a unit of prose
#      that cites a source and gives no date is refused, and so is a required
#      subject whose source or date has gone.
#
#   2. What calling this software from other software means for that software's
#      own licence. The explanation describes an arrangement, quotes the text
#      that governs it, and says plainly that it is not legal advice. It rules
#      on nothing itself: a term of art out of the GPL stands here only where
#      the sentence names whoever wrote it and shows the words or the place
#      they came from.
#
#   3. `architecture/README.md` opens with a description of how the system is
#      arranged rather than with the order the work happened in.
#
# What is pinned is the claim, not a form of words. No check asks whether a
# sentence is spelled a particular way; each asks whether the thing the
# sentence is about stands in it, so a copy edit survives and a deletion or a
# reversal does not. Every check reports what it examined and refuses a count
# of zero, and every floor is the number of subjects this file names rather
# than a number typed beside them.
#
# Every refusal below is exercised against a fixture repository before the real
# one is read, in both directions: the honest rewording is accepted and the
# reversal is refused. A check that has never been seen to fail is worth
# nothing, and a check that refuses the sentence it exists to require cannot be
# repaired by the stage that has to make it green.
#
# What this does not prove, stated so that nobody mistakes a green run for more
# than it is.
#
# That the explanation is correct, that a quotation is faithful to the page it
# came from, or that the source still says today what the document records it
# saying on the date beside it -- the date is what lets a later reader go and
# check.
#
# That no individual is named: a person's name has no shape a scan can
# recognise, and that one is left to review.
#
# That no interpretation is expressed in words outside the set below. The set
# stops the ordinary way the slip is written down, not every way, and a reading
# of a score written in plain words reaches the same place unrefused. The
# boundary is held by review; the set catches the habit.
#
# That this repository states no legal conclusion. What is refused is the
# conclusion written in the licence's own vocabulary -- the sentence that says
# an arrangement IS mere aggregation, IS a combined work, IS a separate
# program. A paraphrase reaching the same conclusion in ordinary words carries
# no term of art and passes. No gate reads meaning, and this one does not
# pretend to. What it holds is that the licence's words are handed back to
# whoever wrote them.
#
# That the material is worth reading. Nothing here measures whether an
# explanation explains. The floors below refuse a stub -- a citation with
# nobody named as saying it, an opening that lists the parts instead of
# describing them -- and a document that clears them can still be thin. That
# judgment belongs to whoever reviews what ships.
#
# Nothing here reaches the network, and nothing here writes into the checkout.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

fail() { printf 'repository sourcing: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

motivation_relative='architecture/motivation.md'
architecture_relative='architecture/README.md'

# The whole of a Markdown file as one flat run of words. The published prose is
# hand-wrapped, so every comparison below reads it unwrapped: a reflow is a
# copy edit and must not fail anything here.
flatten() { tr '\n' ' ' <"$1" | tr -s ' '; }

# One sentence per line. A full stop followed by a space ends one.
sentences() { sed -E 's/\. /.\n/g'; }

# A set of patterns as one alternation. Each set below is scanned in a single
# pass: the same refusal for the same reason, without a process a term.
alternation() { local joined; joined=$(printf '%s|' "$@"); printf '(%s)' "${joined%|}"; }

# An ISO date. It is the shape a capture date is written in, and the only one a
# later reader can order against the day they are reading.
date_pattern='[0-9]{4}-[0-9]{2}-[0-9]{2}'

# A reference to something outside this repository: a link, a DOI, or a PubMed
# identifier. These are the three ways this material cites another party.
source_pattern='(https?://|[Dd][Oo][Ii]\>|PMID\>)'

# Words that hand a claim back to whoever made it. A sentence carrying one of
# these reports what somebody else wrote. A sentence carrying none of them is
# this repository speaking in its own voice, which is the difference the ticket
# turns on: quote the text, describe the engineering, rule on neither.
attributions='\<(states?|stated|says?|said|writes?|wrote|reads?|describes?|defines?|answers?|reports?|published|according to)\>'

# Where the reported words, or the place they came from, stand: a quotation, a
# numbered section of a licence, or a link. An attributing verb with none of
# these beside it is a gesture at a source rather than a source.
attribution_anchor='("|section [0-9]|https?://)'

# A date that no reader can act on. The capture date exists so a later reader
# can go and check; a date that never happened, or one that has not happened
# yet, defeats that. The year is the granularity, so the gate does not change
# its mind on the day a date passes.
this_year=$(date +%Y)
implausible_dates() {
    awk -v maxyear="$this_year" '
        {
            s = $0
            while (match(s, /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/)) {
                d = substr(s, RSTART, RLENGTH)
                s = substr(s, RSTART + RLENGTH)
                y = substr(d, 1, 4) + 0; m = substr(d, 6, 2) + 0; n = substr(d, 9, 2) + 0
                if (y < 1970 || y > maxyear || m < 1 || m > 12 || n < 1 || n > 31) { print d }
            }
        }
    '
}

# ---------------------------------------------------------------------------
# units of prose
# ---------------------------------------------------------------------------
#
# A unit is what a date has to stand in with the source it dates. A blank line
# ends one and a Markdown list marker starts one, so a bulleted list of sources
# is as many units as it has bullets rather than one unit that a single date
# anywhere in it would satisfy. Wrapping is collapsed, so a source and the date
# beside it on two wrapped lines of one sentence stay together.

units() {
    awk '
        function flush() { if (unit != "") { print unit; unit = "" } }
        /^[[:space:]]*$/ { flush(); next }
        /^[[:space:]]*([-*+]|[0-9]+\.)[[:space:]]/ { flush() }
        { sub(/^[[:space:]]+/, ""); unit = (unit == "" ? $0 : unit " " $0) }
        END { flush() }
    ' "$1"
}

# ---------------------------------------------------------------------------
# negation
# ---------------------------------------------------------------------------
#
# One check below turns on whether a sentence denies something. Every word is
# matched on word boundaries: without them `nor` matches "ignore", "minor" and
# "honor" and `not` matches "notation", and ordinary prose stating the claim is
# read as a denial of it. The scan runs over the claim span -- the start of the
# sentence through the end of the term the claim is about -- because a negation
# standing before the term reverses the claim and one standing after it is
# about something else. "This is legal advice, no exceptions" is not a denial,
# and a rule reading the whole sentence would call it one.

negations="(\\<no\\>|\\<not\\>|\\<never\\>|\\<neither\\>|\\<nor\\>|\\<none\\>|\\<nothing\\>|\\<cannot\\>|\\<without\\>|n'[t]\\>)"

# One sentence per line in, one claim span per line out: the run from the start
# of the line through the end of the first match of the term. A line carrying
# no match of the term is dropped, because there is no span in it to read.
claim_span() {
    awk -v term="$1" '
        {
            low = tolower($0)
            if (match(low, term)) { print substr($0, 1, RSTART + RLENGTH - 1) }
        }
    '
}

# ===========================================================================
# 1. every external claim carries its source and the date that source was read
# ===========================================================================
#
# Each required subject is a claim this repository makes about another party.
# The subject's terms, the source it came from, and a date all stand in one
# unit: a date in a different paragraph from the link it dates tells a reader
# nothing about which claim it covers.
#
# The floor on sourced units is the number of subjects named here, computed
# from the list rather than typed beside it, so adding a subject raises the
# floor and a list that lost its entries cannot pass by requiring nothing.

subjects=(
    'the upstream predictor licence|PolyForm Strict License 1\.0\.0|CC BY.?NC 4\.0|github\.com/Illumina/SpliceAI'
    'the public lookup service|handful of queries|spliceailookup\.broadinstitute\.org'
    'the upstream model paper|10\.1186/s13059-022-02664-4'
)

check_sources() {
    local tree=$1
    local file="$tree/$motivation_relative"

    if [[ ! -f "$file" ]]; then
        printf '%s does not exist, so a reader who opens README.md and follows one link learns nothing about why this project exists\n' \
            "$motivation_relative" >&2
        return 1
    fi

    (( ${#subjects[@]} > 0 )) || fail 'no subject is required, so this check examines nothing'

    local all flat sourced=0 examined=0 unit
    all=$(units "$file")
    flat=$(flatten "$file")
    if [[ -z "$all" ]]; then
        printf '%s holds no prose, so nothing in it was examined for a source or a date\n' \
            "$motivation_relative" >&2
        return 1
    fi

    # --- the census: a unit that cites a source states the date it was read,
    # --- and hands the words it reports back to whoever wrote them ---
    local undated= unattributed=
    while IFS= read -r unit; do
        [[ -n "$unit" ]] || continue
        examined=$((examined + 1))
        if ! grep -Eq -- "$source_pattern" <<<"$unit"; then continue; fi
        sourced=$((sourced + 1))
        if ! grep -Eqi -- "$attributions" <<<"$unit"; then
            unattributed+=$'\n  '$(printf '%.110s' "$unit")
        fi
        if grep -Eq -- "$date_pattern" <<<"$unit"; then continue; fi
        undated+=$'\n  '$(printf '%.110s' "$unit")
    done <<<"$all"

    if (( examined == 0 )); then
        printf '%s was split into no units at all, so this check read nothing\n' \
            "$motivation_relative" >&2
        return 1
    fi

    if [[ -n "$undated" ]]; then
        printf '%s cites a source without the date it was read, so a later reader cannot tell whether it still says this:%s\n' \
            "$motivation_relative" "$undated" >&2
        return 1
    fi

    # A bare link beside a quoted string is a citation. The ticket asks for the
    # claim to be carried as a quotation attributed to the party that made it,
    # so the unit has to name somebody as saying it.
    if [[ -n "$unattributed" ]]; then
        printf '%s cites a source and never says who said it, so the claim stands as this project asserting it rather than as that party words:%s\n' \
            "$motivation_relative" "$unattributed" >&2
        return 1
    fi

    local bad_dates
    bad_dates=$(implausible_dates <<<"$flat")
    if [[ -n "$bad_dates" ]]; then
        printf '%s dates a capture %s, which is not a day anybody could have read a page on, so the date buys a later reader nothing\n' \
            "$motivation_relative" "$(head -n 1 <<<"$bad_dates")" >&2
        return 1
    fi

    # --- each required subject: its matter, its source and its date, one unit ---
    local spec name candidates pattern i
    local -a field
    for spec in "${subjects[@]}"; do
        IFS='|' read -r -a field <<<"$spec"
        name=${field[0]}
        (( ${#field[@]} > 1 )) || fail "the subject '$name' names nothing to look for, so it requires nothing"
        candidates=$all
        for (( i = 1; i < ${#field[@]}; i++ )); do
            pattern=${field[i]}
            candidates=$(grep -E -- "$pattern" <<<"$candidates" || true)
            if [[ -z "$candidates" ]]; then
                printf '%s carries no single unit stating %s together with the matter it turns on: nothing in it matches %s\n' \
                    "$motivation_relative" "$name" "$pattern" >&2
                return 1
            fi
        done
        if ! grep -Eq -- "$date_pattern" <<<"$candidates"; then
            printf '%s states %s without the date that source was read beside it, so a later reader cannot check whether it still holds\n' \
                "$motivation_relative" "$name" >&2
            return 1
        fi
    done

    # --- the floor: as many sourced units as there are parties spoken about ---
    if (( sourced < ${#subjects[@]} )); then
        printf '%s cites %d source(s) while this repository makes claims about %d other parties, so at least one claim stands as this project asserting it\n' \
            "$motivation_relative" "$sourced" "${#subjects[@]}" >&2
        return 1
    fi

    # --- the licence this repository is under ---
    if ! grep -Fq -- 'GPL-3.0-only' <<<"$flat"; then
        printf '%s explains what another party permits and never says which licence this repository is itself under\n' \
            "$motivation_relative" >&2
        return 1
    fi

    # --- and one that is not the upstream predictor's any more ---
    if grep -Fq -- 'GPLv3' <<<"$flat"; then
        printf '%s repeats a third-party page describing the upstream predictor as GPLv3, which that predictor LICENSE file no longer says; state what the LICENSE file says today\n' \
            "$motivation_relative" >&2
        return 1
    fi

    printf '%d units read in %s, %d of them citing a source and every one of those dated, %d required subjects each sourced and dated in one unit\n' \
        "$examined" "$motivation_relative" "$sourced" "${#subjects[@]}"
}

# ===========================================================================
# 2. the explanation is an explanation, and says so
# ===========================================================================
#
# Four things. The document says what calling this software from other software
# means for that software's own licence: one sentence naming both a licence and
# the other software. It says plainly that it is not legal advice
# -- and says it as a denial, because a sentence naming legal advice without
# denying it is the reverse of the claim and satisfies every word test. It
# asserts no legal conclusion. And it does not slip from why a precomputed index
# exists into what a score means, which is the boundary this repository holds
# everywhere else.
#
# That sentence is found by joining two things rather than by matching a phrase:
# a word for the other software, and a licence. Either alone is everywhere in a
# document like this -- the sentence stating which licence this repository is
# under carries one, and the sentence about being called carries the other --
# and only the sentence the reader came for carries both. Three rewordings that
# share no vocabulary are accepted below.
#
# A consumer of this software is described generically, so no ticket key, no
# decision reference, no branch and no commit identifier stands in it. A name
# is left to review; it has no shape a scan can recognise.

callers='(other software|another (program|piece)|\<caller\>|\<call(s|ed|ing)\>|\<consumer\>|separate program|mere aggregation|combined work|\<link(s|ed|ing)?\>)'

legal_conclusions=(
    '\<courts?\>' '\<lawful\>' '\<unlawful\>' '\<illegal\>'
    '\<liable\>' '\<liabilit(y|ies)\>' '\<infringe(s|d|ment)?\>'
)

# The set above catches a sentence about what the law decides. It does not
# catch the sentence this material is most likely to get wrong, because that
# sentence uses no courtroom word at all. "This is mere aggregation." "That is
# a combined work under one licence." "A caller is a separate program." Each is
# a term of art out of the GPL and the guidance around it, and each says how
# the licence applies to an arrangement. Saying so is the legal conclusion the
# ticket forbids, whatever tone it is written in.
#
# So a term of art may stand here only where the sentence hands it back to
# whoever wrote it: an attributing verb, and the words themselves or the place
# they came from. "The LICENSE file states: '...aggregate...'" is the licence
# speaking. "Running the executable is mere aggregation" is this repository
# ruling, and is refused.
#
# The limit, stated plainly: this refuses the conclusion that uses the term of
# art. A paraphrase that reaches the same conclusion in ordinary words -- "the
# calling program keeps its own licence" -- carries no term of art and passes.
# No gate reads meaning. What a gate can hold is that this repository does not
# put the licence's own vocabulary in its own mouth, and that is what this one
# holds. The rest is review.
terms_of_art=(
    'mere aggregation' 'combined work' 'separate programs?'
    'derivative works?' 'works? based on' '\<copyleft\>'
)

interpretations=(
    '\<pathogenic(ity)?\>' '\<benign\>' '\<deleterious\>' '\<clinical(ly)?\>'
    '\<diagnos[a-z]*\>' '\<ACMG\>' 'variant classification' '\<evidence (for|of)\>'
)

identifiers=(
    '\<ticket [0-9]' '\<ADR [0-9]' 'refs/heads/' '\<origin/'
)

check_explanation() {
    local tree=$1
    local file="$tree/$motivation_relative"

    if [[ ! -f "$file" ]]; then
        printf '%s does not exist, so a reader learns nothing about what calling this software from other software means for that software own licence\n' \
            "$motivation_relative" >&2
        return 1
    fi

    (( ${#legal_conclusions[@]} > 0 )) || fail 'the legal-conclusion set is empty, so this check refuses nothing'
    (( ${#terms_of_art[@]} > 0 )) || fail 'the term-of-art set is empty, so this check refuses nothing'
    (( ${#interpretations[@]} > 0 )) || fail 'the interpretation set is empty, so this check refuses nothing'
    (( ${#identifiers[@]} > 0 )) || fail 'the identifier set is empty, so this check refuses nothing'

    local flat candidates spans
    flat=$(flatten "$file")

    # --- it says what the arrangement means for the caller's own licence ---
    # Read with `< <(...)` rather than piped into the quiet grep: under
    # `pipefail` a quiet grep closes the pipe on its first match, the stage
    # above dies of SIGPIPE, and the match just found is reported as no match.
    if ! grep -Eqi -- '\<licen[cs]e\>' < <(sentences <<<"$flat" | grep -Ei -- "$callers"); then
        printf '%s never says in one sentence what calling this software from other software means for that software own licence\n' \
            "$motivation_relative" >&2
        return 1
    fi

    # --- it says plainly that it is not legal advice, everywhere it says it ---
    #
    # This repository's own sentences only. A quotation from another party and
    # a Markdown link label are somebody else's words, so both are lifted out
    # before the census: a source that happens to use the phrase is not this
    # repository claiming to give legal advice, and refusing it would push the
    # code stage into paraphrasing a quotation it was told to reproduce.
    #
    # Every remaining sentence that names legal advice denies it -- not one of
    # them, all of them. One honest denial beside a paragraph that offers
    # advice for a commercial deployment is the document the reader must not
    # get, and a rule satisfied by any single denial ships it.
    local own undenied
    own=$(sed -E 's/"[^"]*"//g; s/\[[^]]*\]//g' <<<"$flat")
    candidates=$(sentences <<<"$own" | grep -i -- 'legal advice' || true)
    if [[ -z "$candidates" ]]; then
        printf '%s explains what another party licence permits and never says in its own words that the explanation is not legal advice\n' \
            "$motivation_relative" >&2
        return 1
    fi
    spans=$(claim_span 'legal advice' <<<"$candidates")
    [[ -n "$spans" ]] || fail 'the claim span for legal advice came back empty, so the denial test read nothing'
    undenied=$(grep -Evi -- "$negations" <<<"$spans" || true)
    if [[ -n "$undenied" ]]; then
        printf '%s names legal advice only to claim it, rather than to say the explanation is not legal advice: %s\n' \
            "$motivation_relative" "$(head -n 1 <<<"$undenied")" >&2
        return 1
    fi

    # Each set is scanned as one alternation rather than one grep a term: the
    # same refusal, named the same way, at a fraction of the process count.
    local hit
    hit=$(grep -Eoi -- "$(alternation "${legal_conclusions[@]}")" <<<"$flat" || true)
    if [[ -n "$hit" ]]; then
        printf '%s says what the law decides rather than what the arrangement is: it uses %s, and this repository asserts no legal conclusion\n' \
            "$motivation_relative" "$(head -n 1 <<<"$hit")" >&2
        return 1
    fi

    # --- a term of art belongs to whoever wrote it ---
    local art asserted= art_pattern
    art_pattern=$(alternation "${terms_of_art[@]}")
    while IFS= read -r art; do
        [[ -n "$art" ]] || continue
        if grep -Eqi -- "$attributions" <<<"$art"; then
            if grep -Eqi -- "$attribution_anchor" <<<"$art"; then continue; fi
        fi
        asserted+=$'\n  '$(printf '%.110s' "$art")
    done < <(sentences <<<"$flat" | grep -Ei -- "$art_pattern" || true)
    if [[ -n "$asserted" ]]; then
        printf '%s rules on how a licence applies rather than describing the arrangement and quoting the text: a term of art stands in this repository own voice, with nobody named as saying it and no quotation, section or link beside it:%s\n' \
            "$motivation_relative" "$asserted" >&2
        return 1
    fi

    hit=$(grep -Eoi -- "$(alternation "${interpretations[@]}")" <<<"$flat" || true)
    if [[ -n "$hit" ]]; then
        printf '%s uses %s; this repository states what the software computes and where the number came from, and interpretation is outside what it speaks to\n' \
            "$motivation_relative" "$(head -n 1 <<<"$hit")" >&2
        return 1
    fi

    hit=$(grep -Eoi -- "$(alternation "${identifiers[@]}")" <<<"$flat" || true)
    if [[ -n "$hit" ]]; then
        printf '%s names %s, so a consumer of this software is not described generically\n' \
            "$motivation_relative" "$(head -n 1 <<<"$hit")" >&2
        return 1
    fi

    # A word that is a commit identifier and not a number: seven or more
    # characters, every one of them a hexadecimal digit, with at least one
    # letter and at least one digit among them. Both halves are what keep an
    # ordinary word and a published identifier out of it -- `defaced` carries
    # no digit and the PubMed identifier `35449021` carries no letter.
    hit=$(awk '
        {
            n = split($0, word, /[^0-9A-Za-z]+/)
            for (i = 1; i <= n; i++) {
                w = tolower(word[i])
                if (length(w) >= 7 && w ~ /^[0-9a-f]+$/ && w ~ /[0-9]/ && w ~ /[a-f]/) {
                    print word[i]
                    exit
                }
            }
        }
    ' <<<"$flat")
    if [[ -n "$hit" ]]; then
        printf '%s names %s, which reads as a commit identifier, so a consumer of this software is not described generically\n' \
            "$motivation_relative" "$hit" >&2
        return 1
    fi

    printf '%s denies being legal advice in every sentence that names it, asserts none of %d legal conclusions, attributes each of %d terms of art it uses, uses none of %d interpretation terms, and carries no identifier of %d named shapes or of a commit\n' \
        "$motivation_relative" "${#legal_conclusions[@]}" "${#terms_of_art[@]}" \
        "${#interpretations[@]}" "${#identifiers[@]}"
}

# ===========================================================================
# 3. the architecture folder opens with how the system is arranged
# ===========================================================================
#
# The opening is everything between the title and the first `## ` heading. It
# is what somebody opening the folder reads first, so it describes the pieces
# and how they fit, and the order the work happened in stands below a heading
# of its own or not at all.
#
# The markers are the vocabulary of a build history and of nothing else: a
# ticket key, an accepted-decision reference, one decision superseding another,
# a run that was ineligible, a thing that is shipped, a thing that remains
# future. None of the six describes an arrangement. A word like "retained" is
# deliberately outside the set, because a document can honestly say what the
# system retains.
#
# The terms are the parts the product's own explanation already names: an
# answer comes from the precomputed lookup, the model, or the cache, and it is
# reached through the CLI or the service.

history_markers=(
    '\<ticket [0-9]' '\<ADR [0-9]' '\<supersed(e|es|ed|ing)\>'
    '\<ineligible\>' '\<shipped\>' '\<remains? future\>'
)

arrangement_terms=( '\<lookup\>' '\<model\>' '\<cache\>' '\<CLI\>' '\<service\>' )

# "Lookup, model, cache, CLI, service." names every part and describes no
# arrangement. The ticket asks for plain sentences saying how the system is
# arranged, so the opening has room for a clause about each part rather than
# just the word for it. Six words a part is the floor, computed from the list
# above so that adding a part raises it. Naming five parts in five words is a
# list; this refuses a list.
opening_words_per_part=6

check_architecture_opening() {
    local tree=$1
    local file="$tree/$architecture_relative"

    if [[ ! -f "$file" ]]; then
        printf '%s does not exist, so the architecture folder opens with nothing\n' \
            "$architecture_relative" >&2
        return 1
    fi

    (( ${#history_markers[@]} > 0 )) || fail 'the build-history marker set is empty, so this check refuses nothing'
    (( ${#arrangement_terms[@]} > 0 )) || fail 'the arrangement term set is empty, so this check requires nothing'

    local opening words floor
    opening=$(awk 'NR == 1 { next } /^## / { exit } { print }' "$file" | tr '\n' ' ' | tr -s ' ')
    if [[ -z "${opening// /}" ]]; then
        printf '%s has no opening: the first `## ` heading follows the title directly, so a reader meets a list of links and no description\n' \
            "$architecture_relative" >&2
        return 1
    fi

    words=$(wc -w <<<"$opening" | tr -d '[:space:]')
    floor=$(( opening_words_per_part * ${#arrangement_terms[@]} ))
    if (( words < floor )); then
        printf '%s opens with %d words for %d parts of the system, which is a list of their names and not a description of how they are arranged; %d words is the floor\n' \
            "$architecture_relative" "$words" "${#arrangement_terms[@]}" "$floor" >&2
        return 1
    fi

    local pattern hit
    hit=$(grep -Eoi -- "$(alternation "${history_markers[@]}")" <<<"$opening" || true)
    if [[ -n "$hit" ]]; then
        printf '%s opens with the order the work happened in rather than with how the system is arranged: %s stands above the first heading\n' \
            "$architecture_relative" "$(head -n 1 <<<"$hit")" >&2
        return 1
    fi

    local missing=
    for pattern in "${arrangement_terms[@]}"; do
        grep -Eqi -- "$pattern" <<<"$opening" || missing+=" $pattern"
    done
    if [[ -n "$missing" ]]; then
        printf '%s opens without naming part(s) of the system it describes:%s\n' \
            "$architecture_relative" "$missing" >&2
        return 1
    fi

    printf '%s opens with %s words naming all %d parts of the system and none of %d build-history markers\n' \
        "$architecture_relative" "$(wc -w <<<"$opening" | tr -d '[:space:]')" \
        "${#arrangement_terms[@]}" "${#history_markers[@]}"
}

# ===========================================================================
# the checks refuse what they exist to refuse, and accept what they require
# ===========================================================================

checks=(check_sources check_explanation check_architecture_opening)

plant() {
    local tree=$1
    mkdir -p "$tree/architecture"

    cat >"$tree/$motivation_relative" <<'WHY'
# Why this project exists

A splice predictor a reader has already heard of cannot be called from software
that is sold.

- The upstream predictor LICENSE file states: "SpliceAI source code is provided
  under the PolyForm Strict License 1.0.0. SpliceAI models are provided under
  CC BY NC 4.0 license for academic and non-commercial use." Read at
  https://github.com/Illumina/SpliceAI on 2026-08-28.

- The public lookup service states: "This service supports no more than a
  handful of queries per-user per-minute." Read at
  https://spliceailookup.broadinstitute.org on 2026-08-28.

- The predictor this project runs is Pangolin, published as Zeng T and Li YI,
  Genome Biology 2022, DOI 10.1186/s13059-022-02664-4. Read on 2026-08-28.

Pangolin is under the GNU General Public License, and this repository is
GPL-3.0-only because it inherits from it.

Software that calls this one over its HTTP boundary, or runs its executable and
reads the JSON it writes, is not linked against anything here, and this
repository states no conclusion about that program own licence. The LICENSE file
in this repository says: "Inclusion of a covered work in an aggregate does not
cause this License to apply to the other parts of the aggregate." This is a
description of an arrangement and not legal advice.
WHY

    cat >"$tree/$architecture_relative" <<'ARCH'
# Pangopup Architecture

Pangopup answers a variant query from a memory-mapped precomputed lookup index
first. A miss falls through to the Pangolin model, and every model answer is
saved in a SQLite cache beside it. The CLI and the HTTP service are the two ways in.

## Build history

Ticket 012 selected the mask layout, and ADR 0013 shipped it, superseding
ADR 0011. The first batching run was ineligible. Process-manager packaging
remains future.
ARCH
}

# Replace one run of words in a wrapped Markdown file, or fail loudly. The run
# is matched across whatever whitespace the wrapping put inside it, so a fixture
# edit names the sentence as a reader sees it rather than as it is wrapped.
swap() {
    SWAP_OLD=$2 SWAP_NEW=$3 perl -0777 -i -pe '
        my $old = $ENV{SWAP_OLD};
        my $pat = join("\\s+", map { quotemeta } split /\s+/, $old);
        die "swap matched nothing: $old\n" unless s/$pat/$ENV{SWAP_NEW}/s;
    ' "$1"
}

append() { printf '\n%s\n' "$2" >>"$1"; }

# A fixture repository with one thing changed. A mutation that changes no byte
# is a broken fixture, not a passing check, so the tree is fingerprinted around
# it.
mutate() {
    local tree="$work/$1" before after
    shift
    rm -rf "$tree"
    plant "$tree"
    before=$( { cat "$tree/$motivation_relative" "$tree/$architecture_relative" 2>/dev/null || true; } | md5sum )
    ( cd "$tree" && "$@" )
    after=$( { cat "$tree/$motivation_relative" "$tree/$architecture_relative" 2>/dev/null || true; } | md5sum )
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
        *) fail "the refusal does not name '$wanted', so an operator cannot act on it: $output" ;;
    esac
}

expect_acceptance() {
    local check=$1 tree=$2 why=$3
    "$check" "$tree" >/dev/null \
        || fail "$check refused a repository that states what it requires -- $why -- so it pins a form of words rather than the claim"
}

# The accepted half first: a check that refused its own clean fixture would make
# the repair impossible to write.
clean="$work/clean"
plant "$clean"
for check in "${checks[@]}"; do
    "$check" "$clean" >/dev/null \
        || fail "$check refused the state it exists to require, so it can never go green"
done

# --- 1. sources and dates ---------------------------------------------------

expect_refusal check_sources "$(mutate src-date-dropped \
    sed -i 's| on 2026-08-28\.| .|' "$motivation_relative")" \
    'cites a source without the date it was read'

expect_refusal check_sources "$(mutate src-one-date-dropped \
    sed -i 's|https://spliceailookup.broadinstitute.org on 2026-08-28\.|https://spliceailookup.broadinstitute.org.|' "$motivation_relative")" \
    'cites a source without the date it was read'

expect_refusal check_sources "$(mutate src-licence-source-dropped \
    sed -i 's|https://github.com/Illumina/SpliceAI|the upstream repository|' "$motivation_relative")" \
    'the upstream predictor licence'

expect_refusal check_sources "$(mutate src-service-quote-dropped \
    sed -i 's|handful of queries|a few queries|' "$motivation_relative")" \
    'the public lookup service'

expect_refusal check_sources "$(mutate src-doi-dropped \
    sed -i 's|10\.1186/s13059-022-02664-4|a 2022 paper|' "$motivation_relative")" \
    'the upstream model paper'

expect_refusal check_sources "$(mutate src-undated-addition \
    append "$motivation_relative" '- Another page at https://example.invalid/terms says something else.')" \
    'cites a source without the date it was read'

expect_refusal check_sources "$(mutate src-stale-licence-line \
    append "$motivation_relative" 'A third-party page still describes the upstream predictor as GPLv3.')" \
    'GPLv3'

expect_refusal check_sources "$(mutate src-own-licence-dropped \
    sed -i 's|GPL-3\.0-only|the same licence|' "$motivation_relative")" \
    'never says which licence this repository is itself under'

expect_refusal check_sources "$(mutate src-gone rm "$motivation_relative")" \
    'does not exist'

# A copy edit, and a reflow with it: the same source and the same date, a
# different sentence around them and a different wrap.
expect_acceptance check_sources "$(mutate src-copy-edited swap "$motivation_relative" \
    'The predictor this project runs is Pangolin, published as Zeng T and Li YI, Genome Biology 2022, DOI 10.1186/s13059-022-02664-4. Read on 2026-08-28.' \
    'The predictor this project actually runs is Pangolin. It was published by Zeng T and Li YI in Genome Biology in 2022 under DOI 10.1186/s13059-022-02664-4, and that record was read on 2026-08-28.')" \
    'the same source and the same date in one unit, worded differently'

# A citation with nobody named as saying it. The quoted words and the link are
# there; what is gone is the party the words belong to, which is what turns a
# quotation into this project asserting the thing itself.
expect_refusal check_sources "$(mutate src-unattributed swap "$motivation_relative" \
    'The public lookup service states: "This service supports no more than a handful of queries per-user per-minute." Read at' \
    '"This service supports no more than a handful of queries per-user per-minute", https://spliceailookup.broadinstitute.org, 2026-08-28, and also at')" \
    'never says who said it'

# A capture date nobody could have read a page on. The date exists so a later
# reader can go and check, and one that has not happened yet buys nothing.
expect_refusal check_sources "$(mutate src-future-date \
    sed -i 's|2026-08-28|2099-08-28|g' "$motivation_relative")" \
    'not a day anybody could have read a page on'

expect_refusal check_sources "$(mutate src-impossible-date \
    sed -i 's|2026-08-28|2026-99-28|g' "$motivation_relative")" \
    'not a day anybody could have read a page on'

# A date that moved out of the unit its source stands in satisfies a rule that
# reads the whole file and not this one.
expect_refusal check_sources "$(mutate src-date-in-another-unit swap "$motivation_relative" \
    'Genome Biology 2022, DOI 10.1186/s13059-022-02664-4. Read on 2026-08-28.' \
    'Genome Biology 2022, DOI 10.1186/s13059-022-02664-4.')" \
    'cites a source without the date it was read'

# --- 2. the explanation -----------------------------------------------------

# The four ordinary ways of writing the denial. Without word boundaries `nor`
# stands inside "ignores" and `not` inside "notation", and these were read as
# saying the opposite of what they say.
for denial in \
    'None of this is legal advice.' \
    'Nothing here is legal advice, and a reader who ignores it loses nothing.' \
    'This notation describes an arrangement; it does not constitute legal advice.' \
    'It is no substitute for legal advice.'
do
    expect_acceptance check_explanation "$(mutate "exp-denial-$(printf '%s' "$denial" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'This is a description of an arrangement and not legal advice.' "$denial")" \
        "the denial written as: $denial"
done

# The four ordinary ways of reversing it. Each names legal advice and none of
# them denies it. The last one is why the scan reads the claim span rather than
# the whole sentence.
for claim in \
    'This is legal advice.' \
    'Treat the paragraphs above as legal advice from the maintainers.' \
    'What follows is legal advice for a commercial deployment.' \
    'This is legal advice, no exceptions.'
do
    expect_refusal check_explanation "$(mutate "exp-claim-$(printf '%s' "$claim" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'This is a description of an arrangement and not legal advice.' "$claim")" \
        'only to claim it'
done

expect_refusal check_explanation "$(mutate exp-boundary-gone swap "$motivation_relative" \
    'Software that calls this one over its HTTP boundary, or runs its executable and reads the JSON it writes, is not linked against anything here, and this repository states no conclusion about that program own licence.' \
    'Other software is welcome to use this one.')" \
    'never says in one sentence what calling this software from other software means'

# The boundary written the other ordinary ways. Each reaches the caller's own
# licence, each shares no vocabulary with the others, and none of them says how
# the licence applies: two point the reader at the text that does, and the third
# states only what this repository declines to answer.
for boundary in \
    'A consumer runs this executable and reads its JSON, or calls the HTTP service; which licence that consumer own software is under is answered from the licence text and not here.' \
    'The GPL-3.0-only licence text in this repository LICENSE file is where a program that links or calls this one goes to read what applies to it.' \
    'This repository names no licence for a caller, because it is not the caller licence that this repository is in a position to state.'
do
    expect_acceptance check_explanation "$(mutate "exp-boundary-$(printf '%s' "$boundary" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'Software that calls this one over its HTTP boundary, or runs its executable and reads the JSON it writes, is not linked against anything here, and this repository states no conclusion about that program own licence.' \
        "$boundary")" \
        "the boundary written as: $boundary"
done

# The sentence the ticket forbids, in the forms it is actually written in. Each
# uses a term of art out of the GPL, each says that an arrangement IS one, and
# none of them names anybody as saying so. This repository states no legal
# conclusion of its own, and a rule that only watched for courtroom words would
# let every one of these through.
for ruling in \
    'Running the executable and reading the JSON it writes is mere aggregation, and the reading program keeps its own licence.' \
    'A program that links this project crates into its own binary is a combined work under one licence.' \
    'Software that calls this one over its HTTP boundary is a separate program with its own licence.' \
    'Nothing a caller builds on the JSON this repository emits is a derivative work of it, whatever licence that caller is under.'
do
    expect_refusal check_explanation "$(mutate "exp-ruling-$(printf '%s' "$ruling" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'Software that calls this one over its HTTP boundary, or runs its executable and reads the JSON it writes, is not linked against anything here, and this repository states no conclusion about that program own licence.' \
        "$ruling")" \
        'rules on how a licence applies'
done

# The same term of art, handed back to the party whose words it is. The verb and
# the quotation or the section are what make the difference. The fixture takes
# its quotation from this repository own LICENSE file, so the rule is proved
# against text checked into the tree beside it.
for quoted in \
    'The LICENSE file in this repository defines a covered work as "either the unmodified Program or a work based on the Program", and a caller that links none of it holds none of it.' \
    'On what a combined work is, the GPL-3.0-only licence text at section 5 is what a caller reads, and this repository quotes it rather than summarising it.'
do
    expect_acceptance check_explanation "$(mutate "exp-attributed-$(printf '%s' "$quoted" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'Software that calls this one over its HTTP boundary, or runs its executable and reads the JSON it writes, is not linked against anything here, and this repository states no conclusion about that program own licence.' \
        "$quoted")" \
        "a term of art attributed to whoever wrote it: $quoted"
done

# Two denials where one is reversed. A rule satisfied by any single denial ships
# a document that denies giving legal advice and then gives some.
for mixed in \
    'None of this is legal advice. For a commercial deployment, what follows is legal advice.' \
    'What follows is legal advice for a commercial deployment. None of this is legal advice.'
do
    expect_refusal check_explanation "$(mutate "exp-mixed-$(printf '%s' "$mixed" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'This is a description of an arrangement and not legal advice.' "$mixed")" \
        'only to claim it'
done

# The phrase inside somebody else's words. This material is built out of
# quotations, and a source that uses the phrase is not this repository claiming
# to give legal advice.
for borrowed in \
    'A third party writes: "nothing on this page is legal advice". This is a description of an arrangement and not legal advice.' \
    'See [what counts as legal advice](https://example.invalid/x), read on 2026-08-28. This is a description of an arrangement and not legal advice.'
do
    expect_acceptance check_explanation "$(mutate "exp-borrowed-$(printf '%s' "$borrowed" | cksum | cut -d' ' -f1)" \
        swap "$motivation_relative" \
        'This is a description of an arrangement and not legal advice.' "$borrowed")" \
        "the phrase standing inside somebody else words: $borrowed"
done

expect_refusal check_explanation "$(mutate exp-advice-gone swap "$motivation_relative" \
    'This is a description of an arrangement and not legal advice.' \
    'This is a description of an arrangement.')" \
    'never says in its own words that the explanation is not legal advice'

expect_refusal check_explanation "$(mutate exp-legal-conclusion swap "$motivation_relative" \
    'Software that calls this one over its HTTP boundary, or runs its executable and reads the JSON it writes, is not linked against anything here, and this repository states no conclusion about that program own licence.' \
    'A court would find that software calling this one over its HTTP boundary is a separate program with its own licence.')" \
    'says what the law decides rather than what the arrangement is'

expect_refusal check_explanation "$(mutate exp-interpretation swap "$motivation_relative" \
    'A splice predictor a reader has already heard of cannot be called from software that is sold.' \
    'A high gain score is evidence for a pathogenic splicing effect.')" \
    'interpretation is outside what it speaks to'

expect_refusal check_explanation "$(mutate exp-ticket-key swap "$motivation_relative" \
    'A splice predictor a reader has already heard of cannot be called from software that is sold.' \
    'Ticket 0077 asked for this material.')" \
    'not described generically'

expect_refusal check_explanation "$(mutate exp-commit-id swap "$motivation_relative" \
    'A splice predictor a reader has already heard of cannot be called from software that is sold.' \
    'The software that consumes this one pins the release at 02f6399ab1c.')" \
    'reads as a commit identifier'

# The near miss the commit-identifier scan has to let through. A PubMed
# identifier is eight characters every one of which is a hexadecimal digit.
expect_acceptance check_explanation "$(mutate exp-pmid swap "$motivation_relative" \
    'A splice predictor a reader has already heard of cannot be called from software that is sold.' \
    'The upstream model paper is PMID 35449021, and a splice predictor a reader has already heard of cannot be called from software that is sold.')" \
    'a PubMed identifier carries no letter and is not a commit identifier'

# An honest motivation sentence, naming a licence constraint and a score
# without saying what the score means.
expect_acceptance check_explanation "$(mutate exp-honest-motivation swap "$motivation_relative" \
    'A splice predictor a reader has already heard of cannot be called from software that is sold.' \
    'The licence the better-known predictor is offered under is why this repository publishes precomputed scores of its own.')" \
    'a motivation stated without interpreting a score'

# --- 3. the architecture opening --------------------------------------------

for marker in \
    'Ticket 012 chose the shape of the mask.' \
    'ADR 0013 fixes the mask boundary.' \
    'It supersedes the earlier arrangement.' \
    'The first batching run was ineligible.' \
    'Both routes are shipped.' \
    'Process-manager packaging remains future.'
do
    expect_refusal check_architecture_opening "$(mutate "arch-$(printf '%s' "$marker" | cksum | cut -d' ' -f1)" \
        swap "$architecture_relative" \
        'The CLI and the HTTP service are the two ways in.' \
        "The CLI and the HTTP service are the two ways in. $marker")" \
        'opens with the order the work happened in'
done

expect_refusal check_architecture_opening "$(mutate arch-no-service swap "$architecture_relative" \
    'The CLI and the HTTP service are the two ways in.' \
    'The CLI is the way in.')" \
    'without naming part(s) of the system'

# Every part named, and nothing said about any of them. This is the opening a
# gate that counted names alone would call a description.
expect_refusal check_architecture_opening "$(mutate arch-bare-list swap "$architecture_relative" \
    'Pangopup answers a variant query from a memory-mapped precomputed lookup index first. A miss falls through to the Pangolin model, and every model answer is saved in a SQLite cache beside it. The CLI and the HTTP service are the two ways in.' \
    'Lookup, model, cache, CLI, service.')" \
    'a list of their names and not a description'

expect_refusal check_architecture_opening "$(mutate arch-no-opening swap "$architecture_relative" \
    'Pangopup answers a variant query from a memory-mapped precomputed lookup index first. A miss falls through to the Pangolin model, and every model answer is saved in a SQLite cache beside it. The CLI and the HTTP service are the two ways in.' \
    '')" \
    'has no opening'

# A rewrite of the opening that still names every part.
expect_acceptance check_architecture_opening "$(mutate arch-rewritten swap "$architecture_relative" \
    'Pangopup answers a variant query from a memory-mapped precomputed lookup index first. A miss falls through to the Pangolin model, and every model answer is saved in a SQLite cache beside it. The CLI and the HTTP service are the two ways in.' \
    'Queries are answered by a lookup in a memory-mapped index of precomputed Pangolin scores. Anything that index does not hold goes to the model, whose answers land in a SQLite cache. Both kinds of answer reach a caller through the same CLI, and through the HTTP service that wraps it.')" \
    'every part named in different words'

# A history marker standing below the first heading is where it belongs, and the
# opening check must not reach down for it.
expect_acceptance check_architecture_opening "$clean" \
    'the build history stands under a heading of its own, below the opening'

# ===========================================================================
# the real repository
# ===========================================================================

status=0
for check in "${checks[@]}"; do
    if result=$("$check" "$repository" 2>&1); then
        printf 'repository sourcing: %s\n' "$result"
    else
        printf '%s\n' "$result" >&2
        status=1
    fi
done
exit "$status"
