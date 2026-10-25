//! Human-friendly encoded-sRGB entry helpers for the studio color controls.

pub fn parse_hex_rgb(input: &str) -> Option<[u8; 3]> {
    let value = input.trim().strip_prefix('#').unwrap_or(input.trim());
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

pub fn format_hex_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Convert encoded sRGB bytes to HSV with hue in `[0, 1)` and saturation/value
/// in `[0, 1]`. Achromatic input uses hue zero; callers may retain their prior
/// hue separately while editing saturation/value.
pub fn rgb_to_hsv(rgb: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(|channel| f32::from(channel) / 255.0);
    let maximum = r.max(g).max(b);
    let minimum = r.min(g).min(b);
    let delta = maximum - minimum;
    let hue = if delta == 0.0 {
        0.0
    } else if maximum == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if maximum == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let saturation = if maximum == 0.0 { 0.0 } else { delta / maximum };
    [hue, saturation, maximum]
}

/// Convert normalized HSV to encoded sRGB bytes. Inputs are clamped and
/// nonfinite values safely fall back to zero before conversion.
pub fn hsv_to_rgb(hsv: [f32; 3]) -> [u8; 3] {
    let clean = |value: f32| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let hue = clean(hsv[0]).rem_euclid(1.0) * 6.0;
    let saturation = clean(hsv[1]);
    let value = clean(hsv[2]);
    let chroma = value * saturation;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue.floor() as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let offset = value - chroma;
    [r, g, b].map(|channel| ((channel + offset) * 255.0).round() as u8)
}

/// Map a pointer position within a two-dimensional saturation/brightness field
/// to normalized controls. Invalid or zero-sized fields do not mutate color.
pub fn saturation_value_at(bounds: egui::Rect, position: egui::Pos2) -> Option<[f32; 2]> {
    if !bounds.is_positive()
        || !position.x.is_finite()
        || !position.y.is_finite()
        || !bounds.width().is_finite()
        || !bounds.height().is_finite()
    {
        return None;
    }
    Some([
        ((position.x - bounds.left()) / bounds.width()).clamp(0.0, 1.0),
        (1.0 - (position.y - bounds.top()) / bounds.height()).clamp(0.0, 1.0),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color_entry_roundtrips_and_accepts_optional_prefix() {
        assert_eq!(parse_hex_rgb("#12aBc0"), Some([0x12, 0xab, 0xc0]));
        assert_eq!(parse_hex_rgb(" 00FF7f "), Some([0, 255, 127]));
        assert_eq!(format_hex_rgb([0, 255, 127]), "#00FF7F");
    }

    #[test]
    fn malformed_hex_color_is_rejected() {
        for input in ["", "#123", "12345", "#12345678", "#12G456"] {
            assert_eq!(parse_hex_rgb(input), None, "{input}");
        }
    }

    #[test]
    fn hsv_conversion_roundtrips_primary_and_mixed_colors() {
        for color in [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [197, 92, 255],
            [20, 80, 133],
        ] {
            let result = hsv_to_rgb(rgb_to_hsv(color));
            for (expected, actual) in color.into_iter().zip(result) {
                assert!(expected.abs_diff(actual) <= 1, "{color:?} -> {result:?}");
            }
        }
        assert_eq!(rgb_to_hsv([64, 64, 64]), [0.0, 0.0, 64.0 / 255.0]);
        assert_eq!(hsv_to_rgb([0.0, 1.0, 1.0]), [255, 0, 0]);
    }

    #[test]
    fn hsv_conversion_bounds_nonfinite_user_values() {
        assert_eq!(hsv_to_rgb([f32::NAN, f32::INFINITY, f32::NAN]), [0, 0, 0]);
        assert_eq!(hsv_to_rgb([0.0, 0.0, 2.0]), [255, 255, 255]);
    }

    #[test]
    fn saturation_brightness_field_maps_corners_and_rejects_invalid_geometry() {
        let bounds = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(110.0, 70.0));
        assert_eq!(
            saturation_value_at(bounds, egui::pos2(10.0, 20.0)),
            Some([0.0, 1.0])
        );
        assert_eq!(
            saturation_value_at(bounds, egui::pos2(110.0, 70.0)),
            Some([1.0, 0.0])
        );
        assert_eq!(
            saturation_value_at(bounds, egui::pos2(-10.0, 100.0)),
            Some([0.0, 0.0])
        );
        assert_eq!(
            saturation_value_at(egui::Rect::NOTHING, egui::Pos2::ZERO),
            None
        );
        assert_eq!(
            saturation_value_at(bounds, egui::pos2(f32::NAN, 30.0)),
            None
        );
    }
}
