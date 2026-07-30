# Ticket 034 immutable runtime-release publication evidence

This document freezes the publication procedure, qualified local inputs,
stopped prepublication attempts, and completed immutable public result.

## Closed release identity

- Repository: `genomoncology/pangopup`
- Tag: `runtime-grch38-v1`
- Title: `Pangopup GRCh38 Pangolin runtime v1`
- Target commit: `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`
- Release body: the exact amended Ticket 034 `RELEASE-NOTES.md`
- Required final state: `draft=false`, `immutable=true`

`RELEASE-NOTES.md` is the release body, not an asset. The release has exactly
15 assets: the twelve Ticket 033 runtime assets and three preferred-source
assets. Their total is 859,643,413 bytes.

## Qualified Ticket 033 runtime input

The retained Ticket 033 stage is:

```text
/home/ian/workspace/data/pangopup-runtime-release-033/e6d8497aaf1e3db521360ad969252a2ec6fd14e4
```

It is a mode-`0500`, coordinator-owned directory containing 13 regular,
single-link, mode-`0400` files. Ticket 033 already proved the ten transport
members byte equal to the retained Ticket 030 transport, passed every listed
checksum, and recorded one bounded semantic decode of each frame. Publication
must reauthenticate this closed input; it must not rebuild or recompress it.

| Upload asset | Bytes | SHA-256 |
|---|---:|---|
| `runtime-transport.json` | 3,179 | `415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3` |
| `runtime-profile.json` | 1,366 | `0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c` |
| `model-manifest.json` | 3,823 | `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43` |
| `model-NOTICE` | 648 | `fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f` |
| `model.onnx.zst` | 31,144,867 | `741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568` |
| `reference-manifest.json` | 3,719 | `7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f` |
| `reference-NOTICE` | 793 | `1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499` |
| `reference.pgr.zst` | 656,781,805 | `e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25` |
| `mask-NOTICE` | 978 | `d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb` |
| `domains.pgm.zst` | 3,933,486 | `e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747` |
| `runtime-release-profile.json` | 8,043 | `d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e` |
| `SHA256SUMS` | 934 | `ad5dfff9838b2d17a2d216954b241002b9717dd6215590f28ba3edf0ba2be59f` |

The twelve-file runtime upload payload is 691,883,641 bytes. The thirteenth
local file, pre-amendment `RELEASE-NOTES.md`, is 1,267 bytes with SHA-256
`dba5e20693435d0b94368d7993f18e39e2bd31c4609de10602e4c5e204b5ba1f`.

## Qualified Ticket 034 preferred-source supplement

The coordinator generated one retained supplement at:

```text
/home/ian/workspace/data/pangopup-runtime-source-034
```

The root is a coordinator-owned mode-`0500` directory. Its exact four regular,
single-link, mode-`0400` members are:

| File | Role | Bytes | SHA-256 |
|---|---|---:|---|
| `pangolin-5cf94b8-source.tar.zst` | upload asset: upstream preferred source | 166,859,188 | `c9b457e8cc527dea27f9e491f0ae68278886c6f9b1a07cb16c3b8dd7309f3174` |
| `pangopup-e6d8497-source.tar.zst` | upload asset: converter preferred source | 865,435 | `a7a93f7f0f8b10d5f257a131253030ef85f2cad21f26d152d6ee5db42274c645` |
| `Pangolin-GPL-3.0.txt` | upload asset: standalone upstream license | 35,149 | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |
| `RELEASE-NOTES.md` | exact release body; not uploaded | 1,680 | `63c6ea49f6ddf9c55bac4c6f0290890e43a516a17c12cfd1b3e4649b17561bbd` |

The three preferred-source upload assets total 167,759,772 bytes. Both source
archives were built from exact Git objects, never a worktree:

```text
git archive --format=tar --prefix=Pangolin-5cf94b8/ \
  5cf94b8db938c658391b4305cd7ce33297d44ff7 |
zstd 1.5.5 -19 -T1

git archive --format=tar --prefix=pangopup-e6d8497/ \
  e6d8497aaf1e3db521360ad969252a2ec6fd14e4 |
zstd 1.5.5 -19 -T1
```

Reconstruction matched the exact corresponding `git archive` byte stream.
The upstream archive has 84 entries; the Pangopup archive has 382. The
upstream archive contains the complete tracked tree, all twelve exact
checkpoints, `pangolin/model.py`, packaging metadata, and `LICENSE`. The
Pangopup archive contains the complete tracked target tree, including
`tools/pangolin-model`, `crates/pangopup-build`, `Cargo.lock`, `Makefile`,
README/build instructions, manifests, and model-qualification source.

The twelve checkpoint sizes and hashes, the 3,011-byte
`pangolin/model.py` SHA-256
`4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8`,
and the 35,149-byte license identity above match
`runtime-release-profile.json`. The two named converter paths exist at the
exact Pangopup target commit. The standalone license is byte equal to the
license member in the upstream archive.

The amended notes identify the preferred-source files and state that no raw
Zenodo data, NCBI FASTA/assembly report, GENCODE GTF/SQLite database, or
qualification fixtures are included. The Ticket 033 stage was not modified.

## Exact coordinator preflight

Run from a clean Pangopup checkout after the independently reviewed
publication-ready commit is pushed. These names are fixed; do not substitute a
different target, stage, tag, title, or asset set.

```bash
set -euo pipefail
umask 077

export REPO=genomoncology/pangopup
export TAG=runtime-grch38-v1
export TITLE='Pangopup GRCh38 Pangolin runtime v1'
export TARGET=e6d8497aaf1e3db521360ad969252a2ec6fd14e4
export RUNTIME=/home/ian/workspace/data/pangopup-runtime-release-033/e6d8497aaf1e3db521360ad969252a2ec6fd14e4
export SOURCE=/home/ian/workspace/data/pangopup-runtime-source-034
export TRANSPORT=/home/ian/workspace/data/pangopup-runtime-transport-030/transport
export PRIVATE="$(mktemp -d)"
chmod 0700 "$PRIVATE"
```

Require the configured public remotes, exact Git objects, the target's ancestry
on public `main`, the exact publication-ready commit on local and remote
`main`, and that commit's successful `gate`:

```bash
test "$(git remote get-url origin)" = "git@github.com:genomoncology/pangopup.git"
git cat-file -e "$TARGET^{commit}"
git fetch --force --prune origin main
test "$(git rev-parse HEAD)" = "$PUBLICATION_READY_COMMIT"
test "$(git rev-parse origin/main)" = "$PUBLICATION_READY_COMMIT"
git merge-base --is-ancestor "$TARGET" origin/main
gh api "repos/$REPO/git/commits/$TARGET" --silent
gh run view "$PUBLICATION_READY_RUN" \
  --json headSha,name,status,conclusion,jobs >"$PRIVATE/gate.json"
test "$(jq -r .headSha "$PRIVATE/gate.json")" = "$PUBLICATION_READY_COMMIT"
test "$(jq -r .name "$PRIVATE/gate.json")" = ci
test "$(jq -r .status "$PRIVATE/gate.json")" = completed
test "$(jq -r .conclusion "$PRIVATE/gate.json")" = success
test "$(jq '[.jobs[] | select(
  .name == "gate" and .status == "completed" and .conclusion == "success"
)] | length' "$PRIVATE/gate.json")" -eq 1
```

Require the exact official client, public visibility, immutable-release
setting, and absence of both release and tag. Capture raw authenticated
responses only in the private temporary directory and delete them after
reducing the result to non-secret facts:

```bash
command -v gh
test "$(gh api "repos/$REPO" --jq .visibility)" = public
test "$(gh api "repos/$REPO/immutable-releases" --jq .enabled)" = true
test -z "$(
  gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' |
    grep -Fx "$TAG" || true
)"
test "$(
  gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length
)" -eq 0
```

Reauthenticate the exact closed inventories before opening any upload. The
reviewed expected files below are the authority; no unreviewed extra is
allowed:

```bash
test "$(stat -c '%F %a %u' "$RUNTIME")" = \
  "directory 500 $(id -u)"
test "$(stat -c '%F %a %u' "$SOURCE")" = \
  "directory 500 $(id -u)"
find "$RUNTIME" -mindepth 1 -maxdepth 1 -printf '%f\t%y\t%m\t%U\t%n\t%s\n' \
  | LC_ALL=C sort >"$PRIVATE/runtime-inventory.tsv"
find "$SOURCE" -mindepth 1 -maxdepth 1 -printf '%f\t%y\t%m\t%U\t%n\t%s\n' \
  | LC_ALL=C sort >"$PRIVATE/source-inventory.tsv"
test "$(wc -l <"$PRIVATE/runtime-inventory.tsv")" -eq 13
test "$(wc -l <"$PRIVATE/source-inventory.tsv")" -eq 4
test "$(awk -F '\t' -v uid="$(id -u)" \
  '$2!="f" || $3!="400" || $4!=uid || $5!="1"{n++} END{print n+0}' \
  "$PRIVATE/runtime-inventory.tsv")" -eq 0
test "$(awk -F '\t' -v uid="$(id -u)" \
  '$2!="f" || $3!="400" || $4!=uid || $5!="1"{n++} END{print n+0}' \
  "$PRIVATE/source-inventory.tsv")" -eq 0

printf '%s  %s\n' \
  ad5dfff9838b2d17a2d216954b241002b9717dd6215590f28ba3edf0ba2be59f \
  "$RUNTIME/SHA256SUMS" \
  dba5e20693435d0b94368d7993f18e39e2bd31c4609de10602e4c5e204b5ba1f \
  "$RUNTIME/RELEASE-NOTES.md" \
  c9b457e8cc527dea27f9e491f0ae68278886c6f9b1a07cb16c3b8dd7309f3174 \
  "$SOURCE/pangolin-5cf94b8-source.tar.zst" \
  a7a93f7f0f8b10d5f257a131253030ef85f2cad21f26d152d6ee5db42274c645 \
  "$SOURCE/pangopup-e6d8497-source.tar.zst" \
  3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986 \
  "$SOURCE/Pangolin-GPL-3.0.txt" \
  63c6ea49f6ddf9c55bac4c6f0290890e43a516a17c12cfd1b3e4649b17561bbd \
  "$SOURCE/RELEASE-NOTES.md" |
  sha256sum --check --strict
(cd "$RUNTIME" && sha256sum --check --strict SHA256SUMS)

for name in runtime-transport.json runtime-profile.json model-manifest.json \
  model-NOTICE model.onnx.zst reference-manifest.json reference-NOTICE \
  reference.pgr.zst mask-NOTICE domains.pgm.zst; do
  cmp -- "$RUNTIME/$name" "$TRANSPORT/$name"
done
```

Freshly reconstruct both retained archives and compare them with exact Git
objects. Then verify the closed inventories and required members without
extracting them:

```bash
PANGOLIN_REMOTE="$(git -C /home/ian/foss/Pangolin remote get-url origin)"
case "$PANGOLIN_REMOTE" in
  https://github.com/tkzeng/Pangolin.git | git@github.com:tkzeng/Pangolin.git)
    ;;
  *)
    echo "unexpected Pangolin origin" >&2
    exit 1
    ;;
esac
git -C /home/ian/foss/Pangolin cat-file -e \
  5cf94b8db938c658391b4305cd7ce33297d44ff7^{commit}

git -C /home/ian/foss/Pangolin archive --format=tar \
  --prefix=Pangolin-5cf94b8/ \
  5cf94b8db938c658391b4305cd7ce33297d44ff7 \
  >"$PRIVATE/pangolin.expected.tar"
zstd -dc -- "$SOURCE/pangolin-5cf94b8-source.tar.zst" \
  >"$PRIVATE/pangolin.actual.tar"
cmp -- "$PRIVATE/pangolin.expected.tar" "$PRIVATE/pangolin.actual.tar"
test "$(tar -tf "$PRIVATE/pangolin.actual.tar" | wc -l)" -eq 84

git archive --format=tar --prefix=pangopup-e6d8497/ "$TARGET" \
  >"$PRIVATE/pangopup.expected.tar"
zstd -dc -- "$SOURCE/pangopup-e6d8497-source.tar.zst" \
  >"$PRIVATE/pangopup.actual.tar"
cmp -- "$PRIVATE/pangopup.expected.tar" "$PRIVATE/pangopup.actual.tar"
test "$(tar -tf "$PRIVATE/pangopup.actual.tar" | wc -l)" -eq 382

tar -xOf "$PRIVATE/pangolin.actual.tar" Pangolin-5cf94b8/LICENSE |
  cmp - "$SOURCE/Pangolin-GPL-3.0.txt"
tar -xOf "$PRIVATE/pangolin.actual.tar" Pangolin-5cf94b8/pangolin/model.py |
  sha256sum | grep -Fx \
  '4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8  -'
tar -tf "$PRIVATE/pangopup.actual.tar" |
  grep -Fx 'pangopup-e6d8497/tools/pangolin-model/'
tar -tf "$PRIVATE/pangopup.actual.tar" |
  grep -Fx 'pangopup-e6d8497/crates/pangopup-build/'
tar -tf "$PRIVATE/pangopup.actual.tar" |
  grep -Fx 'pangopup-e6d8497/Cargo.lock'
tar -tf "$PRIVATE/pangopup.actual.tar" |
  grep -Fx 'pangopup-e6d8497/Makefile'
```

For all twelve checkpoint rows in `runtime-release-profile.json`, require the
expected archive member, exact size, and SHA-256. Also perform one fresh
bounded HTTPS fetch of the twelve checkpoints, `pangolin/model.py`, and
`LICENSE` from their literal profile URLs and compare their sizes and hashes.
Never print response headers or URL query strings. Require both converter paths
from the profile at `$TARGET`. Record only `14/14 upstream objects passed` and
`2/2 converter paths passed`:

```bash
PROFILE="$RUNTIME/runtime-release-profile.json"
jq -r '
  (.model_source.checkpoints[] |
    [.url, (.size|tostring), (.sha256|sub("^sha256:";"")),
     ("Pangolin-5cf94b8/pangolin/models/" + .name)]),
  (.model_source.model_py |
    [.url, (.size|tostring), (.sha256|sub("^sha256:";"")),
     ("Pangolin-5cf94b8/" + .path)]),
  (.model_source.license_file |
    [.url, (.size|tostring), (.sha256|sub("^sha256:";"")),
     ("Pangolin-5cf94b8/" + .path)])
  | @tsv
' "$PROFILE" >"$PRIVATE/preferred-source.tsv"
test "$(wc -l <"$PRIVATE/preferred-source.tsv")" -eq 14

i=0
while IFS=$'\t' read -r url size digest member; do
  i=$((i + 1))
  tar -xOf "$PRIVATE/pangolin.actual.tar" "$member" \
    >"$PRIVATE/archive-member-$i"
  test "$(stat -c %s "$PRIVATE/archive-member-$i")" -eq "$size"
  test "$(sha256sum "$PRIVATE/archive-member-$i" | cut -d' ' -f1)" = "$digest"
  curl --fail --location --silent --show-error \
    --connect-timeout 30 --max-time 300 --max-filesize "$size" \
    --output "$PRIVATE/public-source-$i" "$url"
  test "$(stat -c %s "$PRIVATE/public-source-$i")" -eq "$size"
  test "$(sha256sum "$PRIVATE/public-source-$i" | cut -d' ' -f1)" = "$digest"
  cmp -- "$PRIVATE/archive-member-$i" "$PRIVATE/public-source-$i"
done <"$PRIVATE/preferred-source.tsv"
test "$i" -eq 14

test "$(git ls-tree -d --name-only "$TARGET" tools/pangolin-model)" = \
  tools/pangolin-model
test "$(git ls-tree -d --name-only "$TARGET" crates/pangopup-build)" = \
  crates/pangopup-build
```

Immediately before every upload, open the named source, require a regular
single-link mode-`0400` file owned by the coordinator, hash it through the held
descriptor, and revalidate pathname device/inode/type/mode/owner/link
metadata. Keep that descriptor open until the corresponding official `gh
release upload` process has opened the stable private pathname, then repeat the
pathname/hash check after upload. Any change stops publication. The private
mode-`0500` parents prevent untrusted name replacement.

Use this admission check immediately around each upload command below; it
verifies through the retained descriptor and leaves that descriptor open:

```bash
admit_upload_path() {
  local path="$1" size="$2" digest="$3"
  exec {UPLOAD_FD}<"$path"
  local path_state fd_state
  path_state="$(stat -Lc '%d %i %F %a %u %h %s' "$path")"
  fd_state="$(stat -Lc '%d %i %F %a %u %h %s' "/proc/self/fd/$UPLOAD_FD")"
  test "$path_state" = "$fd_state"
  test "$(stat -Lc '%F %a %u %h %s' "/proc/self/fd/$UPLOAD_FD")" = \
    "regular file 400 $(id -u) 1 $size"
  test "$(sha256sum "/proc/self/fd/$UPLOAD_FD" | cut -d' ' -f1)" = "$digest"
}

revalidate_upload_path() {
  local path="$1" digest="$2"
  test "$(stat -Lc '%d %i %F %a %u %h %s' "$path")" = \
    "$(stat -Lc '%d %i %F %a %u %h %s' "/proc/self/fd/$UPLOAD_FD")"
  test "$(sha256sum "$path" | cut -d' ' -f1)" = "$digest"
  exec {UPLOAD_FD}<&-
}
```

## Draft-first publication commands

Create exactly one draft with the exact amended body:

```bash
jq -n \
  --arg tag_name "$TAG" \
  --arg target_commitish "$TARGET" \
  --arg name "$TITLE" \
  --rawfile body "$SOURCE/RELEASE-NOTES.md" \
  '{
    tag_name: $tag_name,
    target_commitish: $target_commitish,
    name: $name,
    body: $body,
    draft: true,
    prerelease: false,
    generate_release_notes: false
  }' >"$PRIVATE/create-release.json"
jq -rj .body "$PRIVATE/create-release.json" |
  cmp - "$SOURCE/RELEASE-NOTES.md"

gh api --method POST "repos/$REPO/releases" \
  --input "$PRIVATE/create-release.json" \
  --jq .id >"$PRIVATE/release-id"

export RELEASE_ID="$(cat "$PRIVATE/release-id")"
test -n "$RELEASE_ID"
```

After creation and after every upload, query the draft by exact release ID and
require the exact tag, target, title, body, `draft=true`, `immutable=false`,
and no unexpected asset. Compare every present asset's
`name,size,digest,state` with the reviewed table in this document. The expected
count starts at zero and increases by exactly one. Every state must be
`uploaded` and every digest must be non-null `sha256:<reviewed hash>`:

```bash
cat >"$PRIVATE/upload-plan.tsv" <<'EOF'
runtime-transport.json	3179	sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3	RUNTIME	uploaded
runtime-profile.json	1366	sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c	RUNTIME	uploaded
model-manifest.json	3823	sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43	RUNTIME	uploaded
model-NOTICE	648	sha256:fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f	RUNTIME	uploaded
model.onnx.zst	31144867	sha256:741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568	RUNTIME	uploaded
reference-manifest.json	3719	sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f	RUNTIME	uploaded
reference-NOTICE	793	sha256:1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499	RUNTIME	uploaded
reference.pgr.zst	656781805	sha256:e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25	RUNTIME	uploaded
mask-NOTICE	978	sha256:d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb	RUNTIME	uploaded
domains.pgm.zst	3933486	sha256:e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747	RUNTIME	uploaded
runtime-release-profile.json	8043	sha256:d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e	RUNTIME	uploaded
SHA256SUMS	934	sha256:ad5dfff9838b2d17a2d216954b241002b9717dd6215590f28ba3edf0ba2be59f	RUNTIME	uploaded
pangolin-5cf94b8-source.tar.zst	166859188	sha256:c9b457e8cc527dea27f9e491f0ae68278886c6f9b1a07cb16c3b8dd7309f3174	SOURCE	uploaded
pangopup-e6d8497-source.tar.zst	865435	sha256:a7a93f7f0f8b10d5f257a131253030ef85f2cad21f26d152d6ee5db42274c645	SOURCE	uploaded
Pangolin-GPL-3.0.txt	35149	sha256:3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986	SOURCE	uploaded
EOF

assert_tag_absent() {
  test "$(
    gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length
  )" -eq 0
}

check_draft() {
  local expected_count="$1"
  assert_tag_absent
  gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/release.json"
  test "$(jq -r .id "$PRIVATE/release.json")" = "$RELEASE_ID"
  test "$(jq -r .tag_name "$PRIVATE/release.json")" = "$TAG"
  test "$(jq -r .target_commitish "$PRIVATE/release.json")" = "$TARGET"
  test "$(jq -r .name "$PRIVATE/release.json")" = "$TITLE"
  test "$(jq -r .draft "$PRIVATE/release.json")" = true
  test "$(jq -r .prerelease "$PRIVATE/release.json")" = false
  test "$(jq -r .immutable "$PRIVATE/release.json")" = false
  jq -rj .body "$PRIVATE/release.json" | cmp - "$SOURCE/RELEASE-NOTES.md"
  gh api --paginate "repos/$REPO/releases/$RELEASE_ID/assets" \
    --jq '.[] | [.name,.size,.digest,.state] | @tsv' |
    LC_ALL=C sort >"$PRIVATE/remote-assets.tsv"
  awk -F '\t' -v count="$expected_count" \
    'NR <= count {print $1 "\t" $2 "\t" $3 "\t" $5}' \
    "$PRIVATE/upload-plan.tsv" |
    LC_ALL=C sort >"$PRIVATE/expected-assets.tsv"
  cmp -- "$PRIVATE/expected-assets.tsv" "$PRIVATE/remote-assets.tsv"
}

check_draft 0
```

Upload in this exact executable order. There is no `--clobber`, replacement,
retry, or parallel upload. Every reviewed row maps explicitly to one retained
root, remains held and authenticated across exactly one official `gh` upload,
is revalidated afterward, and advances the closed draft by exactly one asset:

```bash
n=0
while IFS=$'\t' read -r name size digest root_name state; do
  test "$state" = uploaded
  case "$root_name" in
    RUNTIME) root="$RUNTIME" ;;
    SOURCE) root="$SOURCE" ;;
    *) echo "unexpected upload root" >&2; exit 1 ;;
  esac
  path="$root/$name"
  admit_upload_path "$path" "$size" "${digest#sha256:}"
  gh release upload "$TAG" "$path" --repo "$REPO"
  revalidate_upload_path "$path" "${digest#sha256:}"
  n=$((n + 1))
  check_draft "$n"
done <"$PRIVATE/upload-plan.tsv"
test "$n" -eq 15
```

A failed upload or mismatch stops the operation. It is not retried in this
run.

## Final draft check and one-way publication

Immediately before publication, fetch the release and all assets again.
Require the exact release ID, tag, target, title, body, `draft=true`,
`prerelease=false`, `immutable=false`, and closed 15-asset inventory totaling
859,643,413 bytes. Require each exact reviewed size and GitHub-reported digest.
Also reauthenticate both complete retained inputs and all fifteen upload
pathnames one last time.

```bash
check_draft 15
test "$(awk -F '\t' '{total += $2} END {print total}' \
  "$PRIVATE/upload-plan.tsv")" -eq 859643413

while IFS=$'\t' read -r name size digest root_name state; do
  test "$state" = uploaded
  case "$root_name" in
    RUNTIME) root="$RUNTIME" ;;
    SOURCE) root="$SOURCE" ;;
    *) echo "unexpected upload root" >&2; exit 1 ;;
  esac
  path="$root/$name"
  admit_upload_path "$path" "$size" "${digest#sha256:}"
  revalidate_upload_path "$path" "${digest#sha256:}"
done <"$PRIVATE/upload-plan.tsv"
assert_tag_absent
check_draft 15
```

Publish exactly once:

```bash
gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" \
  -F draft=false \
  --silent
```

Then require the one-way completed state:

```bash
test "$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .draft)" = false
test "$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .immutable)" = true
test "$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .tag_name)" = "$TAG"
test "$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .target_commitish)" = \
  "$TARGET"
gh api "repos/$REPO/git/ref/tags/$TAG" >"$PRIVATE/published-tag.json"
test "$(jq -r .ref "$PRIVATE/published-tag.json")" = "refs/tags/$TAG"
test "$(jq -r .object.type "$PRIVATE/published-tag.json")" = commit
test "$(jq -r .object.sha "$PRIVATE/published-tag.json")" = "$TARGET"
```

A completed mutable release is failure, not permission to edit or replace it.
After publication, never delete, edit, replace, or retry the release, tag, or
any asset.

## Prepublication rollback

Rollback is permitted only while the exact operation-owned release is still a
draft and no tag exists. First re-fetch it by exact ID and require its exact
tag, target, title, body, and `draft=true`. An unexpected tag stops rollback;
the operation never deletes any tag. Then delete only that exact draft release
ID:

```bash
assert_tag_absent
gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/rollback-release.json"
test "$(jq -r .id "$PRIVATE/rollback-release.json")" = "$RELEASE_ID"
test "$(jq -r .tag_name "$PRIVATE/rollback-release.json")" = "$TAG"
test "$(jq -r .target_commitish "$PRIVATE/rollback-release.json")" = "$TARGET"
test "$(jq -r .name "$PRIVATE/rollback-release.json")" = "$TITLE"
test "$(jq -r .draft "$PRIVATE/rollback-release.json")" = true
jq -rj .body "$PRIVATE/rollback-release.json" |
  cmp - "$SOURCE/RELEASE-NOTES.md"
gh api --method DELETE "repos/$REPO/releases/$RELEASE_ID" --silent
assert_tag_absent
test -z "$(
  gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' |
    grep -Fx "$TAG" || true
)"
```

Record the stopped operation and do not recreate or retry it in the same run.

## Bounded unauthenticated public verification

After successful immutable publication, use a clean temporary directory and
unauthenticated HTTPS. Require public repository and release metadata, exact
tag/target/title/body, `draft=false`, `immutable=true`, and the complete
15-asset names/sizes/digests.

```bash
export PUBLIC=https://github.com/genomoncology/pangopup/releases/download/runtime-grch38-v1
export API=https://api.github.com/repos/genomoncology/pangopup
export PUBLIC_CHECK="$(mktemp -d)"
chmod 0700 "$PUBLIC_CHECK"
curl --fail --silent --show-error \
  --connect-timeout 30 --max-time 120 --max-filesize 1048576 \
  "$API/releases/tags/runtime-grch38-v1" >"$PUBLIC_CHECK/release.json"
curl --fail --silent --show-error \
  --connect-timeout 30 --max-time 120 --max-filesize 65536 \
  "$API/git/ref/tags/runtime-grch38-v1" >"$PUBLIC_CHECK/tag.json"
test "$(jq -r .target_commitish "$PUBLIC_CHECK/release.json")" = "$TARGET"
test "$(jq -r .name "$PUBLIC_CHECK/release.json")" = "$TITLE"
test "$(jq -r .draft "$PUBLIC_CHECK/release.json")" = false
test "$(jq -r .immutable "$PUBLIC_CHECK/release.json")" = true
jq -rj .body "$PUBLIC_CHECK/release.json" |
  cmp - "$SOURCE/RELEASE-NOTES.md"
jq -r '.assets[] | [.name,.size,.digest,.state] | @tsv' \
  "$PUBLIC_CHECK/release.json" | LC_ALL=C sort \
  >"$PUBLIC_CHECK/public-assets.tsv"
awk -F '\t' '{print $1 "\t" $2 "\t" $3 "\t" $5}' \
  "$PRIVATE/upload-plan.tsv" | LC_ALL=C sort \
  >"$PUBLIC_CHECK/expected-assets.tsv"
cmp -- "$PUBLIC_CHECK/expected-assets.tsv" \
  "$PUBLIC_CHECK/public-assets.tsv"
test "$(jq -r .ref "$PUBLIC_CHECK/tag.json")" = \
  refs/tags/runtime-grch38-v1
test "$(jq -r .object.type "$PUBLIC_CHECK/tag.json")" = commit
test "$(jq -r .object.sha "$PUBLIC_CHECK/tag.json")" = "$TARGET"
```

Download and compare all small runtime files:

```text
runtime-transport.json
runtime-profile.json
model-manifest.json
model-NOTICE
reference-manifest.json
reference-NOTICE
mask-NOTICE
runtime-release-profile.json
SHA256SUMS
Pangolin-GPL-3.0.txt
```

```bash
for name in runtime-transport.json runtime-profile.json model-manifest.json \
  model-NOTICE reference-manifest.json reference-NOTICE mask-NOTICE \
  runtime-release-profile.json SHA256SUMS; do
  size="$(awk -F '\t' -v name="$name" '$1 == name {print $2}' \
    "$PRIVATE/upload-plan.tsv")"
  test -n "$size"
  curl --fail --location --silent --show-error \
    --connect-timeout 30 --max-time 120 --max-filesize "$size" \
    --output "$PUBLIC_CHECK/$name" "$PUBLIC/$name"
  cmp -- "$PUBLIC_CHECK/$name" "$RUNTIME/$name"
done
curl --fail --location --silent --show-error \
  --connect-timeout 30 --max-time 120 --max-filesize 35149 \
  --output "$PUBLIC_CHECK/Pangolin-GPL-3.0.txt" \
  "$PUBLIC/Pangolin-GPL-3.0.txt"
cmp -- "$PUBLIC_CHECK/Pangolin-GPL-3.0.txt" \
  "$SOURCE/Pangolin-GPL-3.0.txt"
```

For each source archive, request only bytes `0-63` through its public
`releases/download/runtime-grch38-v1/<NAME>` URL and compare those 64 bytes
with the retained archive header. Do not publicly re-download
`model.onnx.zst`, `reference.pgr.zst`, `domains.pgm.zst`, or either complete
source archive. Their reauthenticated local identities plus GitHub's
server-side SHA-256 digests are the proof.

```bash
for name in pangolin-5cf94b8-source.tar.zst \
  pangopup-e6d8497-source.tar.zst; do
  head -c 64 "$SOURCE/$name" >"$PUBLIC_CHECK/$name.expected-header"
  status="$(
    curl --fail --location --silent --show-error --range 0-63 \
      --max-filesize 64 --connect-timeout 30 --max-time 120 \
      --output "$PUBLIC_CHECK/$name.header" \
      --write-out '%{http_code}' "$PUBLIC/$name"
  )"
  test "$status" = 206
  test "$(stat -c %s "$PUBLIC_CHECK/$name.header")" -eq 64
  cmp -- "$PUBLIC_CHECK/$name.expected-header" \
    "$PUBLIC_CHECK/$name.header"
done
```

## Publication result

The publication-ready commit was
`e553e9efcd2959d7a59bc483af99668761ff4d72`; GitHub Actions `ci` run
`30586447723` completed successfully with job `gate` successful before the
coordinator performed the public effect.

### Stopped first draft attempt

The coordinator's first publication attempt created private draft release
`362748317` with the exact reviewed tag, target, and title. The tag remained
absent and the draft had zero assets. The immediate `check_draft 0` body check
stopped the operation because the API body was 1,679 bytes while the retained
notes were 1,680 bytes: Bash command substitution in the former
`-f body="$(cat ...)"` command removed the terminal line feed.

The coordinator reauthenticated that exact empty owned draft, deleted only
release ID `362748317`, and confirmed both release and tag absent. No asset was
opened or uploaded; no tag was created; no release was published; and the
operation was not retried. The replacement command builds a private JSON
request with `jq --rawfile`, proves its decoded body byte equal to the retained
notes, and passes it to official `gh api --input`.

### Stopped second draft attempt

The coordinator's second attempt created private draft release `362753207`
with the exact reviewed fields, an absent tag, and zero assets. The reviewed
Bash upload loop was mistakenly invoked from Zsh, where assigning the loop's
`path` variable also changes the shell's special `PATH` array. Command
resolution failed before the first upload. The coordinator reauthenticated and
deleted only that exact empty draft, confirmed release and tag absent, and did
not retry that attempt. No asset was opened or uploaded and nothing was
published. The successful operation used explicit `/bin/bash`.

### Successful immutable publication

The coordinator created release ID `362753898` from the byte-exact reviewed
request and uploaded each of the 15 reviewed assets exactly once. After every
upload, the complete remote draft prefix matched the expected
name/size/digest/state records. The final closed inventory totals 859,643,413
bytes and matches the 15 identities recorded above.

The release was published once and now reports `draft=false` and
`immutable=true`. `refs/tags/runtime-grch38-v1` is a direct commit ref to
`e6d8497aaf1e3db521360ad969252a2ec6fd14e4`.

Bounded unauthenticated verification passed:

- public release and tag metadata;
- all 15 remote names, sizes, states, and GitHub SHA-256 digests;
- byte-exact reads of all ten small runtime/license assets; and
- final-HTTP-206, exactly 64-byte Range probes for both source archives.

The three runtime frames and complete source archives were not downloaded a
second time. No raw NCBI, GENCODE, or Zenodo input was published, and no
runtime asset was rebuilt, recompressed, replaced, or retried.

Public release:
https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v1

Record only bounded, non-secret facts:

- publication-ready commit and Actions run;
- redacted preflight pass counts;
- exact release ID and generated tag ref identity;
- one attempt per asset;
- closed remote name/size/digest inventory after each upload;
- final `draft` and `immutable` state;
- bounded unauthenticated read results; and
- public release URL.

No raw authenticated API response, credential, environment dump, source
archive extraction, or unbounded download belongs in Git.
