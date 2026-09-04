"""Regression checks for rejecting incomplete/misparsed benchmark measurements."""
import unittest
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

    def test_rejects_unknown_unit(self):
        with self.assertRaises(ValueError):
            measure.parse_time_to_ns("10 cycles")

    @patch.object(measure, "run_command", return_value="no matching benchmarks")
    def test_empty_filter_result_is_an_error(self, _run):
        with self.assertRaisesRegex(ValueError, "no benchmark results"):
            measure.run_benchmark_rounds("bench", ["typo"], "2", 3)

    @patch.object(measure, "run_command", side_effect=[OUTPUT, OUTPUT.replace("update", "other")])
    def test_missing_benchmark_in_later_round_is_an_error(self, _run):
        with self.assertRaisesRegex(ValueError, "benchmark set changed"):
            measure.run_benchmark_rounds("bench", [], "2", 2)


if __name__ == "__main__":
    unittest.main()
