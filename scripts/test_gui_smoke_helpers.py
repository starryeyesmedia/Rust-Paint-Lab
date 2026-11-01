import unittest

from gui_smoke_helpers import QuietValue, marker_value


class MarkerValueTests(unittest.TestCase):
    def test_marker_value_strips_separator_whitespace(self):
        self.assertEqual(
            marker_value(["UI_TEST_NEXT_HOVER: page=1 hovered=false"],
                         "UI_TEST_NEXT_HOVER:"),
            "page=1 hovered=false",
        )

    def test_marker_value_uses_the_latest_matching_line(self):
        self.assertEqual(
            marker_value(["UI_TEST_PAGE: page=1", "other", "UI_TEST_PAGE: page=2"],
                         "UI_TEST_PAGE:"),
            "page=2",
        )

    def test_marker_value_returns_none_when_prefix_is_absent(self):
        self.assertIsNone(marker_value(["other: value"], "UI_TEST_PAGE:"))


class QuietValueTests(unittest.TestCase):
    def test_value_must_remain_unchanged_for_the_full_quiet_period(self):
        observer = QuietValue(0.25)
        self.assertIsNone(observer.observe((10, 20), 1.0))
        self.assertIsNone(observer.observe((10, 20), 1.2))
        self.assertEqual(observer.observe((10, 20), 1.25), (10, 20))

    def test_geometry_change_restarts_the_quiet_period(self):
        observer = QuietValue(0.25)
        self.assertIsNone(observer.observe((10, 20), 1.0))
        self.assertIsNone(observer.observe((10, 21), 1.2))
        self.assertIsNone(observer.observe((10, 21), 1.4))
        self.assertEqual(observer.observe((10, 21), 1.45), (10, 21))

    def test_missing_observation_resets_prior_stability(self):
        observer = QuietValue(0.25)
        self.assertIsNone(observer.observe((10, 20), 1.0))
        self.assertIsNone(observer.observe((10, 20), 1.2))
        self.assertIsNone(observer.observe(None, 1.21))
        self.assertIsNone(observer.observe((10, 20), 1.22))
        self.assertEqual(observer.observe((10, 20), 1.47), (10, 20))

    def test_nonpositive_quiet_period_is_rejected(self):
        with self.assertRaises(ValueError):
            QuietValue(0)


if __name__ == "__main__":
    unittest.main()
