import unittest

from scale_slider import (
    fraction_for_scale,
    pointer_for_fraction,
    secant_fraction,
    track_width_in_pixels,
)


class ScaleSliderTests(unittest.TestCase):
    def test_scale_to_fraction_covers_range(self):
        self.assertEqual(fraction_for_scale(0.8), 0.0)
        self.assertAlmostEqual(fraction_for_scale(1.0), 0.25)
        self.assertAlmostEqual(fraction_for_scale(1.5), 0.875)
        self.assertEqual(fraction_for_scale(1.6), 1.0)

    def test_pointer_uses_handle_center_track_and_pixels_per_point(self):
        slider = (10.0, 20.0, 100.0, 44.0, 1.5)
        self.assertEqual(pointer_for_fraction(slider, 0.0), (34.8, 63.0))
        self.assertAlmostEqual(pointer_for_fraction(slider, 1.0)[0], 145.2)
        self.assertAlmostEqual(pointer_for_fraction(slider, 0.25)[0], 62.4)
        self.assertEqual(pointer_for_fraction(slider, 0.25)[1], 63.0)
        self.assertAlmostEqual(track_width_in_pixels(slider), 110.4)

    def test_secant_corrects_hosted_pointer_offset(self):
        # A nominal 100% pointer position persisted as 102%; measured
        # feedback moves the next real pointer click toward the exact value.
        self.assertAlmostEqual(
            secant_fraction((0.0, 0.8), (0.25, 1.02), 1.0),
            0.22727272727272727,
        )

    def test_secant_bisects_when_the_estimate_is_within_pointer_resolution(self):
        self.assertEqual(
            secant_fraction((0.2, 0.99), (0.25, 1.02), 1.0, 0.02),
            0.225,
        )

    def test_feedback_corrects_fractional_scale_pointer_offset(self):
        target = 1.0
        fraction = fraction_for_scale(target)
        lower = (0.0, 0.8)
        upper = (1.0, 1.6)
        for _ in range(4):
            # Model a hosted X server whose coordinate conversion shifts a
            # pointer click by 2.5% of the slider track.
            value = 0.8 + 0.8 * (fraction + 0.025)
            if abs(value - target) < 0.005:
                break
            if value < target:
                lower = (fraction, value)
            else:
                upper = (fraction, value)
            fraction = secant_fraction(lower, upper, target, 0.75 / 110.4)
        self.assertAlmostEqual(value, target, delta=0.005)

    def test_invalid_geometry_or_bracket_is_rejected(self):
        with self.assertRaises(ValueError):
            pointer_for_fraction((0, 0, 20, 44, 1), 0.5)
        with self.assertRaises(ValueError):
            secant_fraction((0.5, 1.2), (0.4, 1.3), 1.25)


if __name__ == "__main__":
    unittest.main()
