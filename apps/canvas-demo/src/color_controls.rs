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
}
