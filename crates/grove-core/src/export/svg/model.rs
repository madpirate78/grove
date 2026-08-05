//! Model-space SVG export: the whole project on one sheet.

use super::primitives::geometry_to_svg;
use super::*;

pub fn export_svg(project: &Project, options: &SvgExportOptions) -> Result<String, String> {
    let entities: Vec<&Entity> = if options.visible_only {
        project.visible_entities()
    } else {
        project.entities.iter().collect()
    };

    // Compute global bounding box
    let (min, max) = compute_bounds(&entities);
    let margin = options.margin_mm;
    let vb_x = min.x - margin;
    let vb_y = min.y - margin;
    let vb_w = (max.x - min.x) + 2.0 * margin;
    let vb_h = (max.y - min.y) + 2.0 * margin;

    let mut doc = Document::new()
        .set("viewBox", (vb_x, vb_y, vb_w, vb_h))
        .set("width", format!("{}mm", vb_w))
        .set("height", format!("{}mm", vb_h))
        .set("xmlns", "http://www.w3.org/2000/svg");

    // Default font for all text elements
    let style = element::Style::new("text { font-family: sans-serif; }");
    doc = doc.add(style);

    // Optional background
    if let Some(bg) = options.background {
        let rect = element::Rectangle::new()
            .set("x", vb_x)
            .set("y", vb_y)
            .set("width", vb_w)
            .set("height", vb_h)
            .set("fill", rgba_to_hex(bg));
        doc = doc.add(rect);
    }

    // Group entities by layer
    let mut layer_groups: BTreeMap<Uuid, (String, Vec<&Entity>)> = BTreeMap::new();
    for entity in &entities {
        let layer_name = project
            .layers
            .find_by_id(entity.layer_id)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "default".to_string());
        layer_groups
            .entry(entity.layer_id)
            .or_insert_with(|| (layer_name, Vec::new()))
            .1
            .push(entity);
    }

    for (layer_name, layer_entities) in layer_groups.values() {
        let mut group = element::Group::new().set("id", layer_name.as_str());
        for entity in layer_entities {
            // White strokes are invisible on white paper — use black instead
            let colour = if entity.colour[0] > 240 && entity.colour[1] > 240 && entity.colour[2] > 240 {
                [0, 0, 0, entity.colour[3]]
            } else {
                entity.colour
            };
            let stroke_colour = rgba_to_hex(colour);
            let sw = stroke_weight_for_domain(&entity.domain);
            if let Some(elem) = geometry_to_svg(&entity.geometry, &entity.domain, &stroke_colour, sw, &entity.name, PlantStyle::Simple, None) {
                group = group.add(elem);
            }
        }
        doc = doc.add(group);
    }

    Ok(doc.to_string())
}

pub fn export_svg_to_file(
    project: &Project,
    options: &SvgExportOptions,
    path: &Path,
) -> Result<(), String> {
    let svg_string = export_svg(project, options)?;
    std::fs::write(path, svg_string).map_err(|e| format!("Failed to write SVG: {e}"))
}

pub(super) fn compute_bounds(entities: &[&Entity]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut found = false;

    for entity in entities {
        if let Some(bb) = entity.bounding_box() {
            found = true;
            min = min.min(bb.min);
            max = max.max(bb.max);
        }
    }

    if !found {
        (Vec2::ZERO, Vec2::new(1000.0, 1000.0))
    } else {
        (min, max)
    }
}
