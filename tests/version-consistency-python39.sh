#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

python3 - "$repository/scripts/check-version-consistency.py" <<'PY'
import builtins
import runpy
import sys

checker = sys.argv[1]
original_import = builtins.__import__


def import_without_tomllib(name, globals=None, locals=None, fromlist=(), level=0):
    if name == "tomllib" or name.startswith("tomllib."):
        raise ModuleNotFoundError("No module named 'tomllib'", name="tomllib")
    return original_import(name, globals, locals, fromlist, level)


builtins.__import__ = import_without_tomllib
try:
    runpy.run_path(checker, run_name="__main__")
finally:
    builtins.__import__ = original_import
PY
