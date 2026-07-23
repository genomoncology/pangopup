# Ticket 007 public-release hygiene and publication evidence

This file pins the reviewable procedure before any repository visibility,
immutable-release setting, tag, release, or asset changes. It deliberately
contains empty completion headings rather than claims about a future commit or
external result. The retained hygiene result lives outside Git under
`$PANGOPUP_PUBLICATION_EVIDENCE` until publication is complete.

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

Pending coordinator execution after the publication-ready commit is pushed and
its remote gate is green.

## External publication evidence

Pending coordinator-only Phase B.
