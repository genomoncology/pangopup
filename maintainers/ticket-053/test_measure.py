#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///

import importlib.util
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

MODULE_PATH = Path(__file__).with_name("measure.py")
SPEC = importlib.util.spec_from_file_location("ticket053_measure", MODULE_PATH)
assert SPEC and SPEC.loader
measure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(measure)


class ParserTests(unittest.TestCase):
    def test_proc_quantities(self):
        self.assertEqual(
            measure.parse_quantity_lines("Rss: 123 kB\nPss: 45 kB\n", ("Rss", "Pss")),
            {"Rss": 123, "Pss": 45},
        )

    def test_proc_quantities_reject_missing_and_malformed(self):
        with self.assertRaises(measure.ContractError):
            measure.parse_quantity_lines("Rss: nope kB\n", ("Rss",))
        with self.assertRaises(measure.ContractError):
            measure.parse_quantity_lines("Rss: 1 kB\n", ("Rss", "Pss"))

    def test_proc_stat_handles_spaces_and_parentheses(self):
        fields = ["S"] + ["0"] * 20
        fields[7] = "17"
        fields[9] = "3"
        self.assertEqual(
            measure.parse_proc_stat("42 (a tricky) name) " + " ".join(fields)),
            {"minor_faults": 17, "major_faults": 3},
        )
        with self.assertRaises(measure.ContractError):
            measure.parse_proc_stat("truncated")

    def test_gnu_time(self):
        text = "noise\nPANGOPUP_ELAPSED=0.12\nPANGOPUP_MAX_RSS_KB=99\nPANGOPUP_MINOR_FAULTS=7\nPANGOPUP_MAJOR_FAULTS=1\n"
        self.assertEqual(
            measure.parse_gnu_time(text),
            {
                "elapsed_ms": 120.0,
                "max_rss_kb": 99,
                "minor_faults": 7,
                "major_faults": 1,
            },
        )
        with self.assertRaises(measure.ContractError):
            measure.parse_gnu_time("PANGOPUP_ELAPSED=1\n")

    def test_fault_counters_only_compare_same_process(self):
        measure.check_fault_progression(
            [
                {"pid": 1, "minor_faults": 10, "major_faults": 2},
                {"pid": 2, "minor_faults": 1, "major_faults": 0},
                {"pid": 1, "minor_faults": 11, "major_faults": 2},
            ]
        )
        with self.assertRaises(measure.ContractError):
            measure.check_fault_progression(
                [
                    {"pid": 1, "minor_faults": 10, "major_faults": 2},
                    {"pid": 1, "minor_faults": 9, "major_faults": 2},
                ]
            )

    def test_aggregate_and_empty_rejection(self):
        self.assertEqual(
            measure.aggregate([{"x": 1}, {"x": 9}, {"x": 4}], "x"),
            {"median": 4, "maximum": 9},
        )
        with self.assertRaises(measure.ContractError):
            measure.aggregate([], "x")

    def test_dirty_source_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            repo = Path(root)
            (repo / "untracked").write_text("dirty")
            import subprocess

            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            with self.assertRaises(measure.ContractError):
                measure.validate_source(repo, None)


class FakeChild:
    def __init__(self, timeout_once=False):
        self.pid = 123
        self.stdout = io.BytesIO()
        self._pangopup_stderr = io.BytesIO(b"diagnostic")
        self.signals = []
        self.killed = False
        self.waits = 0
        self.timeout_once = timeout_once
        self.exited = False

    def poll(self):
        return 0 if self.exited else None

    def send_signal(self, value):
        self.signals.append(value)

    def wait(self, timeout):
        self.waits += 1
        if self.timeout_once and self.waits == 1:
            raise subprocess.TimeoutExpired("fake", timeout)
        self.exited = True
        return -9 if self.killed else 0

    def kill(self):
        self.killed = True


class ServiceLifecycleTests(unittest.TestCase):
    def test_silent_startup_has_bounded_deadline(self):
        read_fd, write_fd = os.pipe()
        child = FakeChild()
        child.stdout = os.fdopen(read_fd, "rb", buffering=0)
        try:
            with self.assertRaisesRegex(measure.ContractError, "before deadline"):
                measure.read_startup_line(child, timeout=0.01)
        finally:
            child.stdout.close()
            os.close(write_fd)

    def test_malformed_listening_line_is_rejected(self):
        for line in (b"not-json", b"[]", b'{"event":"wrong","address":"x"}'):
            child = FakeChild()
            with (
                self.subTest(line=line),
                mock.patch.object(measure.subprocess, "Popen", return_value=child),
                mock.patch.object(measure, "read_startup_line", return_value=line),
            ):
                with self.assertRaises(measure.ContractError):
                    measure.start_service(Path("binary"), Path("data"), Path("cache"))
            self.assertEqual(child.signals, [measure.signal.SIGTERM])
            self.assertTrue(child.exited)

    def test_post_start_failure_still_stops_child(self):
        child = FakeChild()
        with mock.patch.object(
            measure, "start_service", return_value=(child, "127.0.0.1:1")
        ):
            with self.assertRaisesRegex(RuntimeError, "after start"):
                with measure.service_session(
                    Path("binary"), Path("data"), Path("cache")
                ):
                    raise RuntimeError("after start")
        self.assertEqual(child.signals, [measure.signal.SIGTERM])
        self.assertTrue(child.exited)

    def test_sigterm_timeout_escalates_to_kill_and_wait(self):
        child = FakeChild(timeout_once=True)
        measure.stop_service(child, require_success=False)
        self.assertEqual(child.signals, [measure.signal.SIGTERM])
        self.assertTrue(child.killed)
        self.assertEqual(child.waits, 2)
        self.assertTrue(child.exited)


class RetainedCacheTests(unittest.TestCase):
    @staticmethod
    def write(path, content):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return "sha256:" + measure.sha256(path)

    def test_manifest_members_are_authenticated_and_totals_derived(self):
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            snv_profile = "s"
            runtime_profile = "r"
            snv = root / "profiles/s/transport"
            runtime = root / "profiles/r/transport"
            bundle_digest = self.write(snv / "bundle-manifest.json", b"bundle")
            notice_digest = self.write(snv / "NOTICE", b"notice")
            part_digest = self.write(snv / "part", b"payload")
            snv_manifest = {
                "schema": "pangopup.snv-transport.v1",
                "bundle": {
                    "bundle_id": measure.EXPECTED["snv_bundle_id"],
                    "manifest": {
                        "path": "bundle-manifest.json",
                        "size": 6,
                        "sha256": bundle_digest,
                    },
                    "notice": {"path": "NOTICE", "size": 6, "sha256": notice_digest},
                },
                "payload": {
                    "parts": [{"path": "part", "size": 7, "sha256": part_digest}]
                },
            }
            (snv / "transport.json").write_text(json.dumps(snv_manifest))
            member_digest = self.write(runtime / "model.zst", b"model")
            runtime_manifest = {
                "schema": "pangopup.runtime-transport.v1",
                "runtime_profile_id": measure.EXPECTED["runtime_profile_id"],
                "members": [
                    {
                        "name": "model.zst",
                        "stored_bytes": 5,
                        "stored_sha256": member_digest,
                    }
                ],
            }
            (runtime / "runtime-transport.json").write_text(
                json.dumps(runtime_manifest)
            )
            expected = {
                "snv_profile": snv_profile,
                "snv_transport_sha256": measure.sha256(snv / "transport.json"),
                "runtime_profile": runtime_profile,
                "runtime_transport_sha256": measure.sha256(
                    runtime / "runtime-transport.json"
                ),
            }
            with mock.patch.dict(measure.EXPECTED_CACHE, expected, clear=True):
                result = measure.validate_cache(root)
                self.assertEqual(
                    result["download_bytes"]["snv"],
                    (snv / "transport.json").stat().st_size + 19,
                )
                self.assertEqual(
                    result["download_bytes"]["runtime"],
                    (runtime / "runtime-transport.json").stat().st_size + 5,
                )
                (runtime / "model.zst").write_bytes(b"wrong")
                with self.assertRaises(measure.ContractError):
                    measure.validate_cache(root)


if __name__ == "__main__":
    unittest.main()
