use super::plant::Category;

/// Default colour for each plant category (RGBA).
pub fn category_colour(category: Category) -> [u8; 4] {
    match category {
        Category::Perennial => [76, 175, 80, 255],    // Green
        Category::Shrub => [139, 195, 74, 255],       // Light green
        Category::Tree => [56, 142, 60, 255],         // Dark green
        Category::Grass => [205, 220, 57, 255],       // Lime
        Category::Climber => [156, 39, 176, 255],     // Purple
        Category::Bulb => [255, 193, 7, 255],         // Amber
        Category::Fern => [0, 150, 136, 255],         // Teal
        Category::Hedge => [27, 94, 32, 255],         // Dark green
        Category::Aquatic => [3, 169, 244, 255],      // Light blue
    }
}

/// Parse a hex colour string (e.g. "#4CAF50") to RGBA.
pub fn hex_to_rgba(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b, 255])
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some([r, g, b, a])
    } else {
        None
    }
}

/// Convert RGBA to hex string.
pub fn rgba_to_hex(rgba: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_roundtrip() {
        let hex = "#4CAF50";
        let rgba = hex_to_rgba(hex).unwrap();
        assert_eq!(rgba, [76, 175, 80, 255]);
        assert_eq!(rgba_to_hex(rgba), "#4CAF50");
    }

    #[test]
    fn test_category_colours_unique() {
        let colours: Vec<_> = Category::ALL.iter().map(|c| category_colour(*c)).collect();
        // All colours should be different
        for i in 0..colours.len() {
            for j in (i + 1)..colours.len() {
                assert_ne!(colours[i], colours[j], "Categories should have unique colours");
            }
        }
    }
}
