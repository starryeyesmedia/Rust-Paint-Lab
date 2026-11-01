import unittest

from gui_smoke_helpers import QuietValue


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
