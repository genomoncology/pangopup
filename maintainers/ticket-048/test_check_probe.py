#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Mutation tests for Ticket 048's cheap source checker."""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "maintainers/ticket-048/check_probe.py"
FILES = (
    ".dockerignore",
    "Dockerfile",
    "crates/pangopup-model/Cargo.toml",
    "maintainers/ticket-048/Dockerfile",
    "maintainers/ticket-048/expected-cpuinfo.patch",
    "maintainers/ticket-048/run-mac-probe.sh",
)


class ProbeCheckerTests(unittest.TestCase):
    def fixture(self) -> Path:
        temp = Path(tempfile.mkdtemp(prefix="pangopup-ticket048-check-"))
        self.addCleanup(shutil.rmtree, temp)
        for relative in FILES:
            destination = temp / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        return temp

    def run_checker(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(CHECKER), "--root", str(root)],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_reviewed_recipe_passes(self) -> None:
        result = self.run_checker(self.fixture())
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_extra_patch_change_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/expected-cpuinfo.patch"
        path.write_text(path.read_text() + "-extra\n+change\n", encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_missing_link_proof_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/Dockerfile"
        path.write_text(path.read_text().replace("grep -F 'cpuinfo_initialize'", "grep -F 'not-the-symbol'"), encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_production_dependency_leak_fails(self) -> None:
        root = self.fixture()
        path = root / "crates/pangopup-model/Cargo.toml"
        path.write_text(path.read_text().replace("=2.0.0-rc.12", "=2.0.0-rc.13"), encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_stderr_suppression_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/Dockerfile"
        path.write_text(path.read_text() + "\n# 2>/dev/null\n", encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_unauthenticated_cpuinfo_source_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/Dockerfile"
        path.write_text(
            path.read_text().replace("FETCHCONTENT_SOURCE_DIR_PYTORCH_CPUINFO=/opt/cpuinfo", "onnxruntime_SOME_OTHER_DEFINE=ON"),
            encoding="utf-8",
        )
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_missing_third_party_notices_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/Dockerfile"
        path.write_text(path.read_text().replace("/opt/onnxruntime/ThirdPartyNotices.txt", "/missing/notices"), encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_runner_without_live_https_authentication_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/run-mac-probe.sh"
        path.write_text(path.read_text().replace("https://github.com/genomoncology/pangopup.git", "local-origin"), encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_runner_without_library_identity_comparison_fails(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/run-mac-probe.sh"
        path.write_text(path.read_text().replace("invalid-identical-cpuinfo-libraries", "ignored-identical-libraries"), encoding="utf-8")
        self.assertNotEqual(self.run_checker(root).returncode, 0)

    def test_runner_cannot_restore_bsd_padded_wc_parsing(self) -> None:
        root = self.fixture()
        path = root / "maintainers/ticket-048/run-mac-probe.sh"
        path.write_text(
            path.read_text().replace(
                "awk 'END { exit NR == 1 ? 0 : 1 }' \"${evidence_dir}/live-remote-ref.txt\"",
                "test \"$(wc -l < \"${evidence_dir}/live-remote-ref.txt\")\" = 1",
            ),
            encoding="utf-8",
        )
        self.assertNotEqual(self.run_checker(root).returncode, 0)


if __name__ == "__main__":
    unittest.main()
