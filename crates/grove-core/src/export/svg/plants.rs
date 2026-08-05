//! Plant symbol rendering: discs, canopy outlines, fitted labels.

use super::*;

fn plant_label(plant: &PlantData) -> String {
    if !plant.common_name.is_empty() {
        plant.common_name.clone()
    } else {
        plant.botanical_name.clone()
    }
}

pub(super) fn plant_circle_svg(
    c: &CircleData,
    plant: &PlantData,
    stroke_colour: &str,
    stroke_width: f32,
    plant_style: PlantStyle,
    schedule_no: Option<usize>,
) -> element::Group {
    // Font size in model-space mm; at 1:100 scale, 200mm → 2mm on paper
    let font_size = (c.radius * 0.6).max(200.0);
    let mut group = element::Group::new();

    match plant_style {
        PlantStyle::Simple => {
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.24)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
            if let Some(t) = plant_text(&plant_label(plant), c, font_size, stroke_colour) {
                group = group.add(t);
            }
        }
        PlantStyle::Outline => {
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", "none")
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
            if let Some(t) = plant_text(&plant_label(plant), c, font_size, stroke_colour) {
                group = group.add(t);
            }
        }
        PlantStyle::Silhouette => {
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.9)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
        }
        PlantStyle::Canopy => {
            // Procedural canopy symbol: species-seeded outline, position-seeded rotation
            let outline = crate::garden::canopy::plant_canopy_outline(plant, c.center, c.radius);
            let d = crate::garden::canopy::outline_to_svg_path(&outline);
            group = group.add(
                element::Path::new()
                    .set("d", d)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.15)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
            // Planting-position marker at the stem
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", (c.radius * 0.06).max(30.0))
                    .set("fill", stroke_colour),
            );
            if let Some(t) = plant_text(&plant_label(plant), c, font_size, stroke_colour) {
                group = group.add(t);
            }
        }
        PlantStyle::Numbered => {
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.2)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
            let key = schedule_no
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string());
            if let Some(t) = plant_text(&key, c, font_size * 1.4, stroke_colour) {
                group = group.add(t);
            }
        }
    }

    group
}

/// Centred plant label, shrunk to fit within the plant circle. Returns `None`
/// when fitting would need an illegibly small font — an oversized label would
/// overflow onto neighbouring plants.
fn plant_text(text: &str, c: &CircleData, font_size: f32, stroke_colour: &str) -> Option<element::Text> {
    let max_width = c.radius * 1.8;
    // No text metrics at SVG-generation time: estimate proportional glyph
    // width as 0.55em, the usual average for sans-serif faces.
    let est_width = font_size * 0.55 * text.chars().count().max(1) as f32;
    let font_size = if est_width > max_width {
        font_size * max_width / est_width
    } else {
        font_size
    };
    // 140 model-mm ≈ 1.4mm on paper at 1:100 — smaller than that is noise
    if font_size < 140.0 {
        return None;
    }
    Some(
        element::Text::new(text.to_string())
            .set("x", c.center.x)
            .set("y", c.center.y)
            .set("text-anchor", "middle")
            .set("dominant-baseline", "central")
            .set("font-size", font_size)
            .set("fill", stroke_colour),
    )
}
