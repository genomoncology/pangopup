#!/usr/bin/env bash
set -euo pipefail

# A changelog is the one published file in this repository that nothing
# executes. Every other published claim is joined to evidence a gate reads --
# `tests/published-claim-evidence.sh` holds three of them -- and a list of
# releases joined to nothing goes stale the first time a release is cut without
# it, with every gate green.
#
# Two ways it goes stale, and this file holds both.
#
#   1. A release is prepared and the changelog is not written. `Cargo.toml`
#      states the version this tree would publish. The newest section has to
#      name that version, so bumping the version without describing it is red.
#
#   2. A release is tagged and the changelog never catches up. Every tag this
#      repository made for the software is read out of git, and each one has to
#      have a section carrying its version and the date the tag was made.
#
# The version this tree would release is not tagged yet, so its section carries
# the word `unreleased` where a released section carries a date. That is held in
# both directions: a section for an untagged version that shows a date is
# refused, and a section for a tagged version that says `unreleased` is refused.
# Without the first half, a changelog could publish a release date for a release
# nobody made.
#
# `snv-grch38-v1` and `runtime-grch38-v1` tag the separately versioned scoring
# asset releases, not the software. `architecture/delivery.md` describes them as
# their own immutable releases with their own notices, and `NOTICE` names the
# runtime one as separately versioned. They carry no software version and get no
# section, so the tag scan reads only tags spelled `v<major>.<minor>.<patch>`
# and reports the others as set aside rather than silently dropping them.
#
# Every check here reports what it examined and refuses a count of zero. A scan
# that read no section, no tag or no version would otherwise pass by finding
# nothing, which is the failure this file exists to stop.
#
# Sections 2 and 3 prove the refusals rather than asserting them. The real tree
# is read first, so that a refusal below is attributable to the mutation that
# caused it and not to something already wrong.
#
# What this does not prove: that an entry is true. No gate reads meaning. What
# holds the entries is that each one was written from a diff, a tag or a file in
# this repository, and the record for ticket 0119 names what was read for each.
#
# Nothing here writes into the checkout. Every fixture stands under a temporary
# directory.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

changelog_relative='CHANGELOG.md'
readme_relative='README.md'
manifest_relative='Cargo.toml'

# Measured on 2026-09-11: `git tag` carries seven tags, five of them software
# releases spelled `v<major>.<minor>.<patch>` -- v0.1.0, v0.2.0, v0.3.0, v0.4.0
# and v0.4.1 -- and two asset releases. The floors are those counts. A clone
# that fetched no tags, or a scan that stopped recognising the shape, reads
# fewer and is refused rather than passing on what is left.
release_tag_floor=5
asset_tag_floor=2

# The word a section carries in place of a date when no tag names its version.
unreleased='unreleased'

fail() { printf 'changelog release coverage: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The version `Cargo.toml` states for the workspace. Read by the exact shape it
# is written in, so a key that is renamed or moved is not found, and a version
# that is not found is refused by name below rather than skipped.
manifest_version() {
    sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)"$/\1/p' "$1" | head -1
}

# Every `## <version> - <when>` heading of the changelog, in the order they
# stand, one `<version> <when>` line each.
sections() {
    sed -nE 's/^## ([0-9]+\.[0-9]+\.[0-9]+) - (.+)$/\1 \2/p' "$1"
}

# Whether $1 is a later version than $2. Equal versions are not later, which is
# what makes a repeated section an ordering refusal rather than a pass.
later_version() {
    local top
    [[ "$1" != "$2" ]] || return 1
    top=$(printf '%s\n%s\n' "$1" "$2" | sort -rV | head -1)
    [[ "$top" == "$1" ]]
}

# Refuse the changelog at $1 against the version at $2 and the released tags
# listed in the file at $3, one `<version> <date>` line each. Prints its counts
# on acceptance and its reason on refusal, and exits either way, so every call
# site is the real check or a subshell that expects the refusal.
examine() {
    local changelog=$1 version=$2 tags=$3
    local -a section_version=() section_when=() tag_version=() tag_date=()
    local this when index found sections_read=0 tags_read=0

    [[ -f "$changelog" ]] \
        || fail "$changelog is not a readable file, so no release list was read at all"
    [[ -n "$version" ]] \
        || fail "no version was read out of $manifest_relative, so nothing says which version this tree would release"

    while read -r this when; do
        section_version+=("$this")
        section_when+=("$when")
        sections_read=$((sections_read + 1))
    done < <(sections "$changelog")
    (( sections_read > 0 )) \
        || fail "read 0 version section(s) out of $changelog, so this check examined nothing"

    while read -r this when; do
        [[ -n "$this" ]] || continue
        tag_version+=("$this")
        tag_date+=("$when")
        tags_read=$((tags_read + 1))
    done <"$tags"
    (( tags_read > 0 )) \
        || fail "read 0 released tag(s), so this check examined nothing and would agree with any changelog"

    # The newest section names the version this tree would publish. A version
    # bumped without an entry is the ordinary way a release ships undescribed.
    [[ "${section_version[0]}" == "$version" ]] \
        || fail "the newest section of $changelog names ${section_version[0]}, and $manifest_relative states $version, so no section describes $version"

    # Newest first, stated in the file itself and relied on by the check above.
    for (( index = 1; index < sections_read; index++ )); do
        if later_version "${section_version[index]}" "${section_version[index - 1]}"; then
            fail "the section for ${section_version[index]} stands below the section for ${section_version[index - 1]} in $changelog, and the list is newest first"
        fi
        [[ "${section_version[index]}" != "${section_version[index - 1]}" ]] \
            || fail "$changelog carries two sections for ${section_version[index]}"
    done

    # Every released tag has its section, carrying the date the tag was made.
    for (( index = 0; index < tags_read; index++ )); do
        found=
        for (( this = 0; this < sections_read; this++ )); do
            if [[ "${section_version[this]}" == "${tag_version[index]}" ]]; then
                found=${section_when[this]}
                break
            fi
        done
        [[ -n "$found" ]] \
            || fail "no section of $changelog names ${tag_version[index]}, which this repository released"
        [[ "$found" != "$unreleased" ]] \
            || fail "the section for ${tag_version[index]} says $unreleased, and this repository tagged that version on ${tag_date[index]}"
        [[ "$found" == "${tag_date[index]}" ]] \
            || fail "the section for ${tag_version[index]} gives $found, and the tag for that version was made on ${tag_date[index]}"
    done

    # The other direction: a section naming no released tag is the version this
    # tree would publish, and it says so instead of publishing a release date
    # for a release nobody made.
    for (( index = 0; index < sections_read; index++ )); do
        found=
        for (( this = 0; this < tags_read; this++ )); do
            if [[ "${tag_version[this]}" == "${section_version[index]}" ]]; then
                found=yes
                break
            fi
        done
        if [[ -n "$found" ]]; then
            continue
        fi
        [[ "${section_version[index]}" == "$version" ]] \
            || fail "the section for ${section_version[index]} in $changelog names a version no tag released and $manifest_relative does not state"
        [[ "${section_when[index]}" == "$unreleased" ]] \
            || fail "the section for ${section_version[index]} gives ${section_when[index]}, and no tag names that version, so it must say $unreleased"
    done

    printf '%d section(s) cover %d released tag(s), newest %s\n' \
        "$sections_read" "$tags_read" "$version"
}

# --- 1. the real tree -------------------------------------------------------

changelog="$repository/$changelog_relative"
manifest="$repository/$manifest_relative"
readme="$repository/$readme_relative"

for file in "$changelog" "$manifest" "$readme"; do
    [[ -f "$file" ]] || fail "missing $file"
done

version=$(manifest_version "$manifest")
[[ -n "$version" ]] \
    || fail "could not read the workspace version out of $manifest_relative, so this check has nothing to hold the newest section to"

# A clone with no tags reads no release, finds nothing missing, and agrees with
# itself. That is refused by the floor below rather than run.
tags="$work/released-tags"
aside="$work/asset-tags"
: >"$tags"
: >"$aside"
while IFS= read -r tag; do
    [[ -n "$tag" ]] || continue
    if [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        printf '%s %s\n' "${tag#v}" \
            "$(git -C "$repository" log -1 --format=%cd --date=format:%Y-%m-%d "$tag")" >>"$tags"
    else
        printf '%s\n' "$tag" >>"$aside"
    fi
done < <(git -C "$repository" tag | sort -V)

released=$(wc -l <"$tags" | tr -d '[:space:]')
set_aside=$(wc -l <"$aside" | tr -d '[:space:]')
(( released >= release_tag_floor )) \
    || fail "read $released software release tag(s) against a floor of $release_tag_floor, so the tags this changelog must cover are not all here. Fetch the full history and its tags."
(( set_aside >= asset_tag_floor )) \
    || fail "read $set_aside asset release tag(s) against a floor of $asset_tag_floor, so the scan that tells a software release from an asset release is reading a tree it does not recognise"

counted=$(examine "$changelog" "$version" "$tags")

# The changelog is reachable from the guide a consumer arrives at.
grep -Fq -- "($changelog_relative)" "$readme" \
    || fail "$readme_relative carries no link to $changelog_relative, so a consumer deciding whether to upgrade is not pointed at the list"

# --- 2. a changelog that stopped describing the tree is refused -------------

refuses() {
    local named=$1 changelog=$2 version=$3 tags=$4 what=$5 report
    if report=$(examine "$changelog" "$version" "$tags" 2>&1); then
        fail "$what is admitted: $report"
    fi
    grep -Fq -- "$named" <<<"$report" \
        || fail "$what was refused, but the refusal does not name $named: $report"
}

# The version moved and the changelog did not. The refusal names the version it
# could not find.
refuses "$version" "$changelog" '0.5.1' "$tags" 'a version with no section'

# A tag the changelog does not cover.
extra="$work/tags-with-an-extra-release"
cp "$tags" "$extra"
printf '0.9.9 2026-09-09\n' >>"$extra"
refuses '0.9.9' "$changelog" "$version" "$extra" 'a released tag no section names'

# The same hole made from the other side: the section for a released version is
# gone.
covered=$(head -1 "$tags" | cut -d' ' -f1)
mutant="$work/section-removed.md"
sed "/^## $covered - /d" "$changelog" >"$mutant"
refuses "$covered" "$mutant" "$version" "$tags" 'a released version whose section was removed'

# A released section that claims not to be released.
mutant="$work/released-section-says-unreleased.md"
sed -E "s/^## $covered - .*$/## $covered - $unreleased/" "$changelog" >"$mutant"
refuses "$covered" "$mutant" "$version" "$tags" 'a released section marked unreleased'

# The direction that publishes a date nobody made: the unreleased section
# carrying one.
mutant="$work/unreleased-section-dated.md"
sed -E "s/^## $version - $unreleased\$/## $version - 2026-09-11/" "$changelog" >"$mutant"
refuses "$version" "$mutant" "$version" "$tags" 'an untagged version carrying a release date'

# A section whose date is not the date the tag was made.
mutant="$work/wrong-date.md"
sed -E "s/^## $covered - .*\$/## $covered - 2020-01-01/" "$changelog" >"$mutant"
refuses "$covered" "$mutant" "$version" "$tags" 'a section dated away from its tag'

# Oldest first.
mutant="$work/reordered.md"
awk -v newest="## $version - " '
    index($0, newest) == 1 { held = $0; next }
    { print }
    END { if (held != "") { print held } }
' "$changelog" >"$mutant"
refuses "$version" "$mutant" "$version" "$tags" 'a list that is not newest first'

# --- 3. a check that examined nothing is refused ----------------------------

mutant="$work/no-sections.md"
sed -E '/^## [0-9]+\.[0-9]+\.[0-9]+ - /d' "$changelog" >"$mutant"
refuses 'read 0 version section(s)' "$mutant" "$version" "$tags" 'a changelog with no version section'

mutant_tags="$work/no-tags"
: >"$mutant_tags"
refuses 'read 0 released tag(s)' "$changelog" "$version" "$mutant_tags" 'a tag list that names no release'

printf 'changelog release coverage: %s, and %s covers %d asset release tag(s) with no section\n' \
    "$counted" "$changelog_relative" "$set_aside"
