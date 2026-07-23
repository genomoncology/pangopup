# Ticket 007 public-release hygiene and publication evidence

This file pins the reviewed procedure and records the redacted result after
publication. The detailed retained hygiene result lived outside Git under
`$PANGOPUP_PUBLICATION_EVIDENCE` until the immutable release completed.

## Pinned hygiene tool

- Tool: gitleaks 8.30.1
- Archive: `gitleaks_8.30.1_linux_x64.tar.gz`
- Archive SHA-256:
  `551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb`
- Rules: the tool's default pinned rules
- Redaction: full; raw scanner JSON, API responses, logs, and suspected
  secrets are never retained

Install only after checking the downloaded archive:

```bash
archive=gitleaks_8.30.1_linux_x64.tar.gz
curl --fail --location --silent --show-error \
  --output "$archive" \
  "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/$archive"
echo "551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb  $archive" \
  | sha256sum --check --strict
tar --extract --gzip --file "$archive" gitleaks
./gitleaks version
```

## Exact-commit and history scan

After independent code review, local gates, commit, push, and a green Actions
run, set `PUBLICATION_READY_COMMIT` to that exact 40-hex remote `main` commit.
Fail if local `HEAD`, `origin/main`, or the green workflow run names anything
else. Refresh and prune every public branch, tag, and pull-request head before
the scan so closed-but-still-public refs are included and deleted remote refs
cannot survive locally as misleading state. Then scan all refs, not only the
current branch, with full redaction:

```bash
git fetch --force --prune --prune-tags origin \
  '+refs/heads/*:refs/remotes/origin/*' \
  '+refs/tags/*:refs/tags/*' \
  '+refs/pull/*/head:refs/pull/*/head'
test "$(git rev-parse HEAD)" = "$PUBLICATION_READY_COMMIT"
test "$(git rev-parse origin/main)" = "$PUBLICATION_READY_COMMIT"
./gitleaks git --redact=100 --no-banner --exit-code=1 \
  --log-opts=--all .
```

Do not retain scanner JSON or console logs. The durable outside-Git result may
record only the tool/version/archive digest, the command above with non-secret
placeholders, scanned commit, aggregate object counts, exit status, finding
count, and disposition.

## Closed GitHub-hosted-state inventory

Fetch authenticated textual state into a new private temporary directory. Do
not print authentication material. Inventory every category below and retain
only counts and pass/fail:

- repository settings and topics;
- branches and rulesets;
- Actions workflows, runs, logs, and artifacts, including both previously
  observed failed runs, the previously observed zero Actions artifacts, and
  the publication-ready green run;
- issues, pull requests, comments, and discussions;
- wiki refs and pages;
- projects;
- releases and assets;
- Pages;
- deployments and environments;
- webhooks; and
- deploy keys.

GitHub Actions logs are ZIP archives, not trusted directories. Save each log
response as a ZIP without printing it, then run this extraction check for each
archive. It rejects encryption, absolute or parent paths, backslashes, drive
prefixes, symlinks and other non-regular members, duplicate output paths, more
than 10,000 members, any member over 64 MiB, or more than 1 GiB total. It
extracts by copying each bounded regular member rather than calling a general
archive extractor:

```bash
umask 077
export LOG_ZIP=/tmp/<PRIVATE_HOSTED_STATE_COPY>/actions/<RUN_ID>.zip
export LOG_DEST=/tmp/<PRIVATE_HOSTED_STATE_COPY>/actions/<RUN_ID>
mkdir -- "$LOG_DEST"
uv run - <<'PY'
import os
import stat
import zipfile
from pathlib import Path, PurePosixPath

archive = Path(os.environ["LOG_ZIP"])
destination = Path(os.environ["LOG_DEST"])
member_cap = 64 * 1024 * 1024
total_cap = 1024 * 1024 * 1024
total = 0
seen = set()

with zipfile.ZipFile(archive) as source:
    members = source.infolist()
    if len(members) > 10_000:
        raise SystemExit("unsafe Actions log archive: too many members")
    for member in members:
        name = member.filename
        path = PurePosixPath(name)
        mode = (member.external_attr >> 16) & 0xFFFF
        kind = stat.S_IFMT(mode)
        if (
            not name
            or "\\" in name
            or path.is_absolute()
            or any(part in ("", ".", "..") for part in path.parts)
            or (path.parts and ":" in path.parts[0])
            or member.flag_bits & 1
            or (member.is_dir() and kind not in (0, stat.S_IFDIR))
            or (not member.is_dir() and kind not in (0, stat.S_IFREG))
            or member.file_size > member_cap
        ):
            raise SystemExit("unsafe Actions log archive member")
        target = destination.joinpath(*path.parts)
        if target in seen:
            raise SystemExit("unsafe Actions log archive: duplicate member")
        seen.add(target)
        if member.is_dir():
            target.mkdir(parents=True, exist_ok=False)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        copied = 0
        with source.open(member) as reader, target.open("xb") as writer:
            while chunk := reader.read(1024 * 1024):
                copied += len(chunk)
                total += len(chunk)
                if copied > member_cap or total > total_cap:
                    raise SystemExit("unsafe Actions log archive: size cap")
                writer.write(chunk)
        if copied != member.file_size:
            raise SystemExit("unsafe Actions log archive: size changed")
PY
```

Run the pinned scanner against the no-Git temporary copies of all fetched
textual state, including those safely extracted logs:

```bash
./gitleaks dir --redact=100 --no-banner --exit-code=1 \
  /tmp/<PRIVATE_HOSTED_STATE_COPY>
```

Delete the temporary copies and all raw API responses/logs after reducing them
to category counts and pass/fail. A credential, private key, non-public
dataset, customer identifier, or actual dependency on non-public software
blocks visibility. Historical absolute developer paths and a sentence denying
a dependency are benign context and do not justify rewriting history.

## Retained-result shape

Write the redacted result to a new file below
`$PANGOPUP_PUBLICATION_EVIDENCE`; never write a secret or raw scanner/API
output there. It contains only:

- gitleaks version and checked archive digest;
- exact redacted commands with placeholders;
- publication-ready commit;
- aggregate Git object counts;
- hosted-state category counts;
- exit status and finding count for each scan; and
- final disposition.

Phase B must stop unless this retained result reports zero findings and names
the exact pushed commit whose Actions gate passed.

## Hygiene completion evidence

The publication-ready commit was
`2fa6b24b15926dfda5ab3cca1d110cf6acb4d52a`; local `HEAD`, remote `main`, and
GitHub Actions run `30034965591` matched exactly. That run completed
successfully with checkout, pinned tool setup, `make lint`, `make test`, and
`make spec` all green.

The checked gitleaks 8.30.1 archive matched the pinned digest above. The
history scan covered all refreshed branch/tag/pull refs with `--all`: 27
commits, 2 refs, 605 Git objects, and 3,744,570 bytes produced zero findings.

The private hosted-state copy covered repository settings/topics; branch and
ruleset state; one workflow and all eight runs; eight safely extracted log ZIPs
(38 regular members, 515,685 bytes); zero Actions artifacts; issues, pulls,
comments, discussions, wiki, projects, releases/assets, Pages, deployments,
environments, webhooks, and deploy keys. The authenticated repository API
injects a generated `temp_clone_token`; that transport-only credential was
removed by field name before scanning and no value was retained or printed.
The owner made the repository public while the credential lacked the separate
Projects-v2 scope; the resulting public organization search reported no
Pangopup project and was added to the scan. The final 62-file, 803,144-byte
hosted-content scan produced zero findings.

The redacted retained result is outside Git at
`data/pangopup/publication/evidence/audit-2fa6b24.md` relative to the workspace.
No raw API response, scanner report, Actions ZIP, log, suspected secret, or
credential was committed.

## External publication evidence

The owner made `genomoncology/pangopup` public before coordinator Phase B; the
coordinator observed `visibility=public` and did not toggle it. No tag or
release existed at that checkpoint.

The first immutable-release PUT incorrectly supplied an `enabled` body and was
rejected with HTTP 422 without changing state. The corrected official empty
PUT succeeded; the required GET returned `enabled=true` and
`enforced_by_owner=false`.

Draft release `358895554` was created with tag `snv-grch38-v1`, title
`Pangopup GRCh38 SNV scores v1`, target
`851f57d6ffb75a2c099a3d1263b1e94b60aad0e8`, and the exact reviewed notes.
Each expected asset used one reviewed uploader invocation and no retry:

| Asset | Attempts | Bytes | GitHub SHA-256 |
|---|---:|---:|---|
| `transport.json` | 1 | 1,266 | `f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a` |
| `bundle-manifest.json` | 1 | 3,589 | `c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3` |
| `NOTICE` | 1 | 1,709 | `9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| `payload.pgi.zst.part0000` | 1 | 1,000,000,000 | `07c1f9a2e33e1a5bd929500eefd00b84764c82d56e3f573c35d380419e4ed42a` |
| `payload.pgi.zst.part0001` | 1 | 931,687,706 | `87580144fd828676d7adb269059cf2b425b342fe5ccee442888e0b93994adc74` |
| `proof-receipt.json` | 1 | 2,194 | `9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475` |
| `release-profile.json` | 1 | 2,821 | `63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6` |
| `SHA256SUMS` | 1 | 595 | `54c29666c74bb35701d14f10d7d2b2ba3dcadc116a111429274da8aa975dce2e` |

After every invocation, the complete draft inventory was fetched. Every asset
immediately reported `state=uploaded` with its non-null reviewed digest. The
exact closed inventory, target, title, and body were rechecked before the draft
was published once.

The completed release reports `draft=false`, `immutable=true`, the exact tag,
target, title, body, and eight assets. The public tag resolves to the intended
Ticket 006 commit. Unauthenticated bounded reads succeeded for the repository,
release page, `release-profile.json`, `transport.json`,
`bundle-manifest.json`, `NOTICE`, `proof-receipt.json`, and `SHA256SUMS`; all
six downloaded small files matched their exact sizes and SHA-256 identities.
Neither payload part was downloaded. Each local large part was read exactly
once as its successful upload request body, with no retry.

Public release:
https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v1
