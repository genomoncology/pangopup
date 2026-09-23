#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
python3 - "$repo/scripts/check-production-qualification.py" <<'PY'
import copy
import hashlib
import importlib.util
import json
import pathlib
import sys
import tempfile

path = pathlib.Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("qualification", path)
assert spec and spec.loader
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
repo = path.parent.parent
assert checker.qualified_v2_profile_id(repo) == (
    "sha256:ce91b332d04a776f12a4603e60cf40894dc002360c13b3541e03d3b748cd3983"
)

version = "0.3.0"
profile = "sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c"
policy = "sequential:1/1"
identity = "sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"
data_set = "sha256:4685370f5c7b0ce762eaa123ff93ae33c209b0006bcf41a492ab17258bbe0ac5"
status = {
    "version": version,
    "runtime_profile_id": profile,
    "model": {"effective_cpu_policy": policy},
    "scoring_identity": identity,
    "data_set_version": data_set,
}
assert checker.require_recomputed_identities(status, version, profile) == (identity, data_set)

def rejects(label, mutate, expected):
    changed = copy.deepcopy(status)
    mutate(changed)
    try:
        checker.require_recomputed_identities(changed, version, profile)
    except SystemExit as error:
        assert str(error) == expected, (label, error)
    else:
        raise AssertionError("accepted " + label)

rejects("software", lambda value: value.__setitem__("version", "0.3.1"),
        "HTTP status software version mismatch")
rejects("profile", lambda value: value.__setitem__("runtime_profile_id", "sha256:" + "7" * 64),
        "HTTP status data-set version does not match canonical preimage")
rejects("policy", lambda value: value["model"].__setitem__("effective_cpu_policy", "sequential:1/2"),
        "HTTP status scoring identity does not match canonical preimage")
rejects("scoring digest", lambda value: value.__setitem__("scoring_identity", "sha256:" + "8" * 64),
        "HTTP status scoring identity does not match canonical preimage")
rejects("data-set digest", lambda value: value.__setitem__("data_set_version", "sha256:" + "9" * 64),
        "HTTP status data-set version does not match canonical preimage")
rejects("missing policy", lambda value: value["model"].pop("effective_cpu_policy"),
        "HTTP status effective CPU policy is invalid")

def digest(schema, **inputs):
    preimage = json.dumps({"schema": schema, **inputs}, sort_keys=True, separators=(",", ":")).encode()
    return "sha256:" + hashlib.sha256(preimage).hexdigest()

def coherent_profile_change(value):
    value["runtime_profile_id"] = "sha256:" + "7" * 64
    value["data_set_version"] = digest(
        "pangopup.scoring-data-set-version.v1",
        software_version=version, runtime_profile_id=value["runtime_profile_id"])
    value["scoring_identity"] = digest(
        "pangopup.active-scoring-identity.v1",
        software_version=version, runtime_profile_id=value["runtime_profile_id"],
        effective_cpu_policy=policy)

def coherent_policy_change(value):
    value["model"]["effective_cpu_policy"] = "sequential:1/2"
    value["scoring_identity"] = digest(
        "pangopup.active-scoring-identity.v1",
        software_version=version, runtime_profile_id=profile,
        effective_cpu_policy=value["model"]["effective_cpu_policy"])

rejects("coherent profile", coherent_profile_change,
        "HTTP status runtime profile id does not match qualified profile")
rejects("coherent policy", coherent_policy_change,
        "HTTP status effective CPU policy does not match qualification command")

with tempfile.TemporaryDirectory() as directory:
    test_root = pathlib.Path(directory)
    authority = test_root / "release-profiles/runtime-release-profile-v2.json"
    authority.parent.mkdir()
    original = (repo / "release-profiles/runtime-release-profile-v2.json").read_bytes()
    authority.write_bytes(original[:-1] + b" ")
    try:
        checker.qualified_v2_profile_id(test_root)
    except SystemExit as error:
        assert str(error) == "qualified v2 runtime release profile identity mismatch", error
    else:
        raise AssertionError("accepted altered release authority")
print("production qualification identities: canonical preimages and nine mutations passed")
PY
