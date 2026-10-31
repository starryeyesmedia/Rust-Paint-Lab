"""Pointer geometry and feedback correction for the UI-scale smoke test."""

SCALE_MIN = 0.8
SCALE_MAX = 1.6
HANDLE_INSET_RATIO = 0.30


def fraction_for_scale(scale):
    if not SCALE_MIN <= scale <= SCALE_MAX:
        raise ValueError(f"scale outside supported range: {scale}")
    return (scale - SCALE_MIN) / (SCALE_MAX - SCALE_MIN)


def pointer_for_fraction(slider, fraction):
    if not 0.0 <= fraction <= 1.0:
        raise ValueError(f"slider fraction outside [0, 1]: {fraction}")
    left, top, width, height, pixels_per_point = slider
    inset = height * HANDLE_INSET_RATIO
    track_width = width - 2.0 * inset
    if track_width <= 0.0 or pixels_per_point <= 0.0:
        raise ValueError(f"invalid slider geometry: {slider}")
    x = left + inset + track_width * fraction
    y = top + height / 2.0
    return x * pixels_per_point, y * pixels_per_point


def track_width_in_pixels(slider):
    _, _, width, height, pixels_per_point = slider
    return (width - 2.0 * height * HANDLE_INSET_RATIO) * pixels_per_point


def secant_fraction(lower, upper, target, minimum_fraction_step=0.0):
    """Interpolate a slider fraction from measured (fraction, value) bounds."""
    low_fraction, low_value = lower
    high_fraction, high_value = upper
    if not low_fraction < high_fraction or not low_value < high_value:
        raise ValueError(f"invalid monotonic slider bracket: {lower}, {upper}")
    if not low_value <= target <= high_value:
        raise ValueError(f"target outside measured slider bracket: {target}")
    candidate = low_fraction + (
        (target - low_value) * (high_fraction - low_fraction)
        / (high_value - low_value)
    )
    if (candidate - low_fraction < minimum_fraction_step
            or high_fraction - candidate < minimum_fraction_step):
        candidate = (low_fraction + high_fraction) / 2.0
    return candidate
