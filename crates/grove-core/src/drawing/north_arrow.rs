use svg::node::element::{self, path::Data, Group};

/// Render a north arrow SVG group at the given position (mm on sheet).
/// `size` is the overall height of the arrow in mm.
pub fn render_north_arrow_svg(x: f32, y: f32, size: f32) -> Group {
    let mut group = Group::new().set("id", "north-arrow");

    let width = size * 0.3;

    // Arrow body — filled triangle (right half dark, left half outline)
    let tip = (x, y);
    let left = (x - width, y + size);
    let right = (x + width, y + size);
    let mid_bottom = (x, y + size);

    // Right half (filled black)
    let right_data = Data::new()
        .move_to(tip)
        .line_to(right)
        .line_to(mid_bottom)
        .close();
    let right_half = element::Path::new()
        .set("d", right_data)
        .set("fill", "black")
        .set("stroke", "black")
        .set("stroke-width", 0.3);
    group = group.add(right_half);

    // Left half (outline only)
    let left_data = Data::new()
        .move_to(tip)
        .line_to(left)
        .line_to(mid_bottom)
        .close();
    let left_half = element::Path::new()
        .set("d", left_data)
        .set("fill", "white")
        .set("stroke", "black")
        .set("stroke-width", 0.3);
    group = group.add(left_half);

    // "N" label above the arrow
    let n_label = element::Text::new("N".to_string())
        .set("x", x)
        .set("y", y - 2.0)
        .set("text-anchor", "middle")
        .set("font-size", size * 0.5)
        .set("font-weight", "bold")
        .set("fill", "black");
    group = group.add(n_label);

    group
}

/// Render a scale bar SVG group at the given position.
/// `scale` is the denominator (e.g. 100 for 1:100).
/// The bar shows a real-world distance that fits neatly on the sheet.
pub fn render_scale_bar_svg(x: f32, y: f32, scale: u32, paper_label: &str) -> Group {
    let mut group = Group::new().set("id", "scale-bar");

    // Pick a nice round real-world distance that produces a 40–80mm bar
    let target_bar_mm = 60.0_f32;
    let target_real = target_bar_mm * scale as f32; // real-world mm at this scale

    // Round to a nice number
    let real_distance = nice_round_number(target_real);
    let bar_length_mm = real_distance / scale as f32;

    let segments = 5;
    let seg_width = bar_length_mm / segments as f32;
    let bar_height = 2.5_f32;

    // Alternating black/white segments
    for i in 0..segments {
        let sx = x + i as f32 * seg_width;
        let fill = if i % 2 == 0 { "black" } else { "white" };
        let rect = element::Rectangle::new()
            .set("x", sx)
            .set("y", y)
            .set("width", seg_width)
            .set("height", bar_height)
            .set("fill", fill)
            .set("stroke", "black")
            .set("stroke-width", 0.3);
        group = group.add(rect);
    }

    // Start label (0)
    let zero = element::Text::new("0".to_string())
        .set("x", x)
        .set("y", y + bar_height + 4.0)
        .set("text-anchor", "middle")
        .set("font-size", 3.0)
        .set("fill", "black");
    group = group.add(zero);

    // End label (distance with units)
    let label = format_distance(real_distance);
    let end_label = element::Text::new(label)
        .set("x", x + bar_length_mm)
        .set("y", y + bar_height + 4.0)
        .set("text-anchor", "middle")
        .set("font-size", 3.0)
        .set("fill", "black");
    group = group.add(end_label);

    // Scale ratio
    let ratio = element::Text::new(format!("1:{}@{}", scale, paper_label))
        .set("x", x + bar_length_mm / 2.0)
        .set("y", y - 1.5)
        .set("text-anchor", "middle")
        .set("font-size", 3.0)
        .set("fill", "black");
    group = group.add(ratio);

    group
}

/// Pick a nice round number close to `target`.
fn nice_round_number(target: f32) -> f32 {
    if target <= 0.0 {
        return 1.0;
    }
    let magnitude = 10.0_f32.powf(target.log10().floor());
    let normalized = target / magnitude;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    nice * magnitude
}

/// Format a distance in mm as a human-readable string.
fn format_distance(mm: f32) -> String {
    if mm >= 1000.0 {
        let m = mm / 1000.0;
        if (m - m.round()).abs() < 0.01 {
            format!("{}m", m.round() as i32)
        } else {
            format!("{:.1}m", m)
        }
    } else {
        format!("{}mm", mm.round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_round_number() {
        assert_eq!(nice_round_number(3200.0), 2000.0);
        assert_eq!(nice_round_number(7500.0), 10000.0);
        assert_eq!(nice_round_number(12000.0), 10000.0);
        assert_eq!(nice_round_number(180.0), 200.0);
        assert_eq!(nice_round_number(450.0), 500.0);
    }

    #[test]
    fn test_format_distance() {
        assert_eq!(format_distance(1000.0), "1m");
        assert_eq!(format_distance(5000.0), "5m");
        assert_eq!(format_distance(10000.0), "10m");
        assert_eq!(format_distance(500.0), "500mm");
        assert_eq!(format_distance(2500.0), "2.5m");
    }

    #[test]
    fn test_north_arrow_svg() {
        let group = render_north_arrow_svg(100.0, 50.0, 15.0);
        let svg = svg::Document::new().add(group).to_string();
        assert!(svg.contains("north-arrow"));
        assert!(svg.contains("north-arrow"));
        // The svg crate may encode Text content in various ways
        assert!(svg.contains("N"));
    }

    #[test]
    fn test_scale_bar_svg() {
        let group = render_scale_bar_svg(50.0, 100.0, 100, "A1");
        let svg = svg::Document::new().add(group).to_string();
        assert!(svg.contains("scale-bar"));
        assert!(svg.contains("1:100@A1"));
    }
}
