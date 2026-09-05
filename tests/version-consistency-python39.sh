#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

python3 - "$repository/scripts/check-version-consistency.py" <<'PY'
import builtins
import contextlib
import io
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
    namespace = runpy.run_path(checker)
    main = namespace["main"]
    checker_globals = main.__globals__
    original_read = checker_globals["read"]
    cargo = original_read("Cargo.toml")
    lock = original_read("Cargo.lock")
    candidate = namespace["workspace_version"]()

    main()

    def replace_once(text, old, new):
        if text.count(old) != 1:
            raise AssertionError(f"expected one occurrence of {old!r}")
        return text.replace(old, new, 1)

    def expect_rejected(label, overrides):
        def read_with_mutation(path):
            if path in overrides:
                return overrides[path]
            return original_read(path)

        checker_globals["read"] = read_with_mutation
        try:
            with contextlib.redirect_stderr(io.StringIO()):
                main()
        except SystemExit as error:
            if error.code != 1:
                raise AssertionError(f"{label} exited with {error.code!r}") from error
        else:
            raise AssertionError(f"checker accepted {label}")
        finally:
            checker_globals["read"] = original_read

    expect_rejected(
        "a changed workspace version",
        {
            "Cargo.toml": replace_once(
                cargo, f'version = "{candidate}"', 'version = "9.9.9"'
            )
        },
    )
    expect_rejected(
        "a changed PangoPup lockfile package version",
        {
            "Cargo.lock": replace_once(
                lock,
                f'name = "pangopup-assets"\nversion = "{candidate}"',
                'name = "pangopup-assets"\nversion = "9.9.9"',
            )
        },
    )
finally:
    builtins.__import__ = original_import
PY
