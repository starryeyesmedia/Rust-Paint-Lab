import tempfile
import unittest
from pathlib import Path

from app_log_index import ApplicationLogIndex


class ApplicationLogIndexTests(unittest.TestCase):
    def test_incremental_append_counts_and_stable_values(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "application.log"
            index = ApplicationLogIndex()
            path.write_bytes(b"UI_TEST_STATE: path 4 ")
            self.assertEqual(index.refresh(path), [])
            self.assertEqual(index.count("UI_TEST_STATE"), 0)

            with path.open("ab") as log:
                log.write(b"true\nUI_TEST_OTHER: ready\n")
            index.refresh(path)
            self.assertEqual(index.count("UI_TEST_STATE"), 1)
            self.assertEqual(index.value("UI_TEST_STATE"), "path 4 true")
            self.assertIsNone(index.stable_value("UI_TEST_STATE"))
            self.assertEqual(index.count_prefix("UI_TEST_STATE: path 4"), 1)
            self.assertEqual(index.count_text("path 4"), 1)

            with path.open("ab") as log:
                log.write(b"UI_TEST_STATE: path 4 true path 4\n")
            index.refresh(path)
            self.assertEqual(index.count("UI_TEST_STATE"), 2)
            self.assertIsNone(index.stable_value("UI_TEST_STATE"))
            self.assertEqual(index.count_prefix("UI_TEST_STATE: path 4"), 2)
            self.assertEqual(index.count_text("path 4"), 3)

            with path.open("ab") as log:
                log.write(b"UI_TEST_STATE: path 4 true path 4\n")
            index.refresh(path)
            self.assertEqual(index.count("UI_TEST_STATE"), 3)
            self.assertEqual(index.stable_value("UI_TEST_STATE"), "path 4 true path 4")
            self.assertEqual(index.count_prefix("UI_TEST_STATE: path 4"), 3)
            self.assertEqual(index.count_text("path 4"), 5)

    def test_truncation_resets_old_markers(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "application.log"
            index = ApplicationLogIndex()
            path.write_text("UI_TEST_STATE: old\n", encoding="utf-8")
            index.refresh(path)
            self.assertEqual(index.value("UI_TEST_STATE"), "old")

            path.write_text("UI_TEST_STATE: n\n", encoding="utf-8")
            index.refresh(path)
            self.assertEqual(index.count("UI_TEST_STATE"), 1)
            self.assertEqual(index.value("UI_TEST_STATE"), "n")
            self.assertIsNone(index.stable_value("UI_TEST_STATE"))


if __name__ == "__main__":
    unittest.main()
