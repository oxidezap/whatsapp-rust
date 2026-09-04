"""Regression checks for rejecting incomplete/misparsed benchmark measurements."""
import unittest
import contextlib
import io
import json
import os
import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch

import measure_baseline as measure


OUTPUT = """case                 fastest       │ slowest       │ median        │ mean          │ samples │ iters
╰─ update            1.5 µs        │ 3 µs          │ 2 µs          │ 2.1 µs        │ 20      │ 20
"""


class MeasurementTests(unittest.TestCase):
    def test_parses_time_columns(self):
        result = measure.parse_divan_output(OUTPUT)["case::update"]
        self.assertEqual(result["median_ns"], 2000)
        self.assertEqual(result["iters"], 20)
        self.assertEqual(measure.parse_time_to_ns("1.25 ms"), 1250000)

    def test_counter_rows_do_not_replace_the_parent(self):
        output = OUTPUT.replace("╰─ update", "├─ update")
        output += "│                    123 Kitem/s   │ 456 Kitem/s   │ 333 Kitem/s\n"
        output += OUTPUT.splitlines()[1].replace("update", "next") + "\n"
        self.assertEqual(set(measure.parse_divan_output(output)), {"case::update", "case::next"})

    def test_fastest_in_a_name_is_not_a_table_header(self):
        output = OUTPUT.replace("case", "fastest_group").replace("update", "fastest_lookup")
        self.assertEqual(set(measure.parse_divan_output(output)), {"fastest_group::fastest_lookup"})

    def test_rejects_unknown_unit(self):
        with self.assertRaises(ValueError):
            measure.parse_time_to_ns("10 cycles")

    @patch.object(measure, "binary_digest", return_value="fixed")
    @patch.object(measure, "run_command", return_value="no matching benchmarks")
    def test_empty_filter_result_is_an_error(self, _run, _hash):
        with self.assertRaisesRegex(ValueError, "no benchmark results"):
            measure.run_benchmark_rounds("bench", ["typo"], "2", 3)

    @patch.object(measure, "binary_digest", return_value="fixed")
    @patch.object(measure, "run_command", side_effect=[OUTPUT, OUTPUT.replace("update", "other")])
    def test_missing_benchmark_in_later_round_is_an_error(self, _run, _hash):
        with self.assertRaisesRegex(ValueError, "benchmark set changed"):
            measure.run_benchmark_rounds("bench", [], "2", 2)


    def test_rejects_executable_replacement_during_a_round(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "bench"
            binary.write_bytes(b"baseline")
            def replace_binary(_cmd):
                binary.write_bytes(b"replacement")
                return OUTPUT
            with patch.object(measure, "run_command", side_effect=replace_binary):
                with self.assertRaisesRegex(ValueError, "executable changed"):
                    measure.run_benchmark_rounds(str(binary), [], "2", 1)

    def test_raw_directory_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            raw = Path(directory) / "raw"
            raw.mkdir()
            evidence = raw / "round-1.txt"
            evidence.write_text("baseline evidence")
            with self.assertRaises(FileExistsError):
                measure.run_benchmark_rounds("bench", [], "2", 1, raw)
            self.assertEqual(evidence.read_text(), "baseline evidence")


    def test_checkout_metadata_precedes_raw_output(self):
        original_cwd = os.getcwd()
        with tempfile.TemporaryDirectory() as directory:
            os.chdir(directory)
            try:
                subprocess.run(["git", "init", "-q"], check=True)
                binary = Path("bench")
                binary.write_bytes(b"fixed executable")
                subprocess.run(["git", "add", "bench"], check=True)
                subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                                "commit", "-qm", "fixture"], check=True)
                stdout = io.StringIO()
                argv = ["measure_baseline.py", "--bin", str(binary), "--cpus", "2",
                        "--rounds", "1", "--raw-dir", "raw", "--json"]
                with patch("sys.argv", argv), patch.object(measure, "run_command", return_value=OUTPUT), contextlib.redirect_stdout(stdout):
                    measure.main()
                data = json.loads(stdout.getvalue())
                self.assertEqual(data["metadata"]["git_status"], "")
                self.assertTrue(Path("raw/round-1.txt").exists())
            finally:
                os.chdir(original_cwd)


if __name__ == "__main__":
    unittest.main()
