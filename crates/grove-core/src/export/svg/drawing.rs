//! Drawing-sheet SVG export: paper, viewports, title block, legend.

use super::model::compute_bounds;
use super::primitives::geometry_to_svg;
use super::*;

pub fn export_drawing_svg(
    project: &Project,
    drawing: &Drawing,
    _options: &SvgExportOptions,
) -> Result<String, String> {
    let (paper_w, paper_h) = drawing.paper_dimensions_mm();

    let mut doc = Document::new()
        .set("width", format!("{}mm", paper_w))
        .set("height", format!("{}mm", paper_h))
        .set("viewBox", (0.0, 0.0, paper_w, paper_h))
        .set("xmlns", "http://www.w3.org/2000/svg");

    // Default font for all text elements
    let style = element::Style::new("text { font-family: sans-serif; }");
    doc = doc.add(style);

    // White paper background
    let bg = element::Rectangle::new()
        .set("x", 0)
        .set("y", 0)
        .set("width", paper_w)
        .set("height", paper_h)
        .set("fill", "white");
    doc = doc.add(bg);

    // Sheet border within margins
    let [mt, mr, mb, ml] = drawing.margins;
    let border = element::Rectangle::new()
        .set("x", ml)
        .set("y", mt)
        .set("width", paper_w - ml - mr)
        .set("height", paper_h - mt - mb)
        .set("fill", "none")
        .set("stroke", "black")
        .set("stroke-width", 0.5);
    doc = doc.add(border);

    // Define clip paths for viewports
    let mut defs = element::Definitions::new();
    for vp in &drawing.viewports {
        let clip_id = format!("clip-{}", vp.id);
        let clip_rect = element::Rectangle::new()
            .set("x", vp.sheet_position[0])
            .set("y", vp.sheet_position[1])
            .set("width", vp.sheet_size[0])
            .set("height", vp.sheet_size[1]);
        let clip_path = element::ClipPath::new()
            .set("id", clip_id.as_str())
            .add(clip_rect);
        defs = defs.add(clip_path);
    }
    doc = doc.add(defs);

    // Schedule numbers for Numbered plant markers (keyed to the planting schedule)
    let schedule_numbers = if drawing.viewports.iter().any(|vp| vp.plant_style == PlantStyle::Numbered) {
        crate::garden::schedule::schedule_numbers(&project.entities)
    } else {
        std::collections::HashMap::new()
    };

    // Render each viewport
    for vp in &drawing.viewports {
        let clip_id = format!("clip-{}", vp.id);
        let scale_factor = 1.0 / vp.scale as f32;

        // Viewport center on sheet
        let vp_cx = vp.sheet_position[0] + vp.sheet_size[0] / 2.0;
        let vp_cy = vp.sheet_position[1] + vp.sheet_size[1] / 2.0;

        // Transform: translate to viewport center, scale, then offset by model center
        let transform = format!(
            "translate({}, {}) scale({}) translate({}, {})",
            vp_cx, vp_cy,
            scale_factor,
            -vp.model_center[0], -vp.model_center[1]
        );

        let clip_group = element::Group::new()
            .set("clip-path", format!("url(#{})", clip_id));
        let mut vp_group = element::Group::new()
            .set("transform", transform);

        // Filter and render entities
        let entities: Vec<&Entity> = project.entities.iter()
            .filter(|e| is_layer_visible_in_viewport(e.layer_id, vp, &project.layers))
            .collect();

        // Diagnostic comment: entity count, bounding box, and transform info
        let (ent_min, ent_max) = compute_bounds(&entities);
        let comment = svg::node::Comment::new(format!(
            " Viewport '{}': {} visible entities out of {} total, model_center=({},{}), scale=1:{}, entity_bounds=({},{})..({},{}) ",
            vp.name, entities.len(), project.entities.len(),
            vp.model_center[0], vp.model_center[1], vp.scale,
            ent_min.x, ent_min.y, ent_max.x, ent_max.y,
        ));
        vp_group = vp_group.add(comment);

        for entity in &entities {
            let resolved = resolve_colour(entity, vp.colour_mode);
            // White strokes are invisible on white paper — use black instead
            let resolved = if resolved[0] > 240 && resolved[1] > 240 && resolved[2] > 240 {
                [0, 0, 0, resolved[3]]
            } else {
                resolved
            };
            let stroke_colour = rgba_to_hex(resolved);
            let paper_sw = stroke_weight_for_domain(&entity.domain);
            let sw = paper_sw * vp.scale as f32;
            let schedule_no = match (&entity.domain, vp.plant_style) {
                (DomainData::Plant(pd), PlantStyle::Numbered) => {
                    schedule_numbers.get(&pd.botanical_name).copied()
                }
                _ => None,
            };
            if let Some(elem) = geometry_to_svg(
                &entity.geometry, &entity.domain, &stroke_colour, sw, &entity.name, vp.plant_style, schedule_no,
            ) {
                vp_group = vp_group.add(elem);
            }
        }

        doc = doc.add(clip_group.add(vp_group));

        // Viewport frame (dashed rectangle)
        let frame = element::Rectangle::new()
            .set("x", vp.sheet_position[0])
            .set("y", vp.sheet_position[1])
            .set("width", vp.sheet_size[0])
            .set("height", vp.sheet_size[1])
            .set("fill", "none")
            .set("stroke", "#999")
            .set("stroke-width", 0.25)
            .set("stroke-dasharray", "4,2");
        doc = doc.add(frame);

        // Viewport label
        let label = element::Text::new(format!("{} (1:{}@{})", vp.name, vp.scale, drawing.paper_size.label()))
            .set("x", vp.sheet_position[0] + 2.0)
            .set("y", vp.sheet_position[1] - 1.0)
            .set("font-size", 3.0)
            .set("fill", "#999");
        doc = doc.add(label);
    }

    // Title block
    if drawing.show_title_block {
        let mut tb_data = drawing.title_block.clone();
        // Auto-populate fields from drawing metadata
        if tb_data.drawing_title.is_empty() {
            tb_data.drawing_title = drawing.title.clone();
        }
        if tb_data.sheet_number.is_empty() {
            tb_data.sheet_number = drawing.full_sheet_ref();
        }
        if let Some(rev) = drawing.current_revision() {
            tb_data.revision = rev.label.clone();
        }
        if tb_data.project_name.is_empty() {
            tb_data.project_name = project.metadata.name.clone();
        }
        // Scale from first viewport
        if tb_data.scale.is_empty() || tb_data.scale == "1:100" {
            if let Some(vp) = drawing.viewports.first() {
                tb_data.scale = format!("1:{}@{}", vp.scale, drawing.paper_size.label());
            }
        }

        let tb_group = render_title_block_svg(&tb_data, drawing.paper_size, drawing.orientation);
        doc = doc.add(tb_group);
    }

    // North arrow
    if drawing.show_north_arrow {
        let arrow_x = paper_w - 25.0;
        let arrow_y = mt + 5.0;
        let arrow_group = render_north_arrow_svg(arrow_x, arrow_y, 15.0);
        doc = doc.add(arrow_group);
    }

    // Scale bar
    if drawing.show_scale_bar {
        let scale = drawing.viewports.first().map(|v| v.scale).unwrap_or(100);
        let bar_x = ml + 5.0;
        let bar_y = paper_h - mb - 10.0;
        let bar_group = render_scale_bar_svg(bar_x, bar_y, scale, drawing.paper_size.label());
        doc = doc.add(bar_group);
    }

    // Annotations (paper-space labels with optional leader lines)
    for ann in &drawing.annotations {
        let mut ann_group = element::Group::new()
            .set("class", "annotation");

        // Leader line
        if let (Some(target), Some(vp_id)) = (ann.leader_target_model, ann.viewport_id) {
            if let Some(vp) = drawing.viewports.iter().find(|v| v.id == vp_id) {
                let sheet_target = svg_model_to_sheet(target, vp);
                // Leader line
                let leader = element::Line::new()
                    .set("x1", ann.sheet_position[0])
                    .set("y1", ann.sheet_position[1])
                    .set("x2", sheet_target[0])
                    .set("y2", sheet_target[1])
                    .set("stroke", "#666")
                    .set("stroke-width", 0.25);
                ann_group = ann_group.add(leader);

                // Arrowhead at target
                let dx = sheet_target[0] - ann.sheet_position[0];
                let dy = sheet_target[1] - ann.sheet_position[1];
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.1 {
                    let d = [dx / len, dy / len];
                    let perp = [-d[1], d[0]];
                    let arrow_size = 2.0_f32;
                    let tip = sheet_target;
                    let left = [
                        tip[0] - d[0] * arrow_size + perp[0] * arrow_size * 0.4,
                        tip[1] - d[1] * arrow_size + perp[1] * arrow_size * 0.4,
                    ];
                    let right = [
                        tip[0] - d[0] * arrow_size - perp[0] * arrow_size * 0.4,
                        tip[1] - d[1] * arrow_size - perp[1] * arrow_size * 0.4,
                    ];
                    let arrow = element::Polygon::new()
                        .set("points", format!(
                            "{},{} {},{} {},{}",
                            tip[0], tip[1], left[0], left[1], right[0], right[1]
                        ))
                        .set("fill", "#666");
                    ann_group = ann_group.add(arrow);
                }
            }
        }

        // Bordered background
        if ann.bordered {
            let text_w = ann.text.len() as f32 * ann.font_size * 0.5;
            let text_h = ann.font_size * 1.3;
            let bg = element::Rectangle::new()
                .set("x", ann.sheet_position[0] - 0.5)
                .set("y", ann.sheet_position[1] - ann.font_size * 0.1)
                .set("width", text_w + 1.0)
                .set("height", text_h)
                .set("fill", "white")
                .set("stroke", "#999")
                .set("stroke-width", 0.15);
            ann_group = ann_group.add(bg);
        }

        // Text
        let text = element::Text::new(ann.text.clone())
            .set("x", ann.sheet_position[0])
            .set("y", ann.sheet_position[1] + ann.font_size * 0.8)
            .set("font-size", ann.font_size)
            .set("fill", "#333");
        ann_group = ann_group.add(text);

        doc = doc.add(ann_group);
    }

    // Legend (auto-generated from visible plants)
    if drawing.show_legend {
        let legend_items = collect_legend_items(
            &project.entities, &drawing.viewports, &project.layers,
        );
        if !legend_items.is_empty() {
            let legend_x = ml + 5.0;
            let legend_y = mt + 5.0;
            let legend_group = render_legend_svg(&legend_items, legend_x, legend_y);
            doc = doc.add(legend_group);
        }
    }

    Ok(doc.to_string())
}

pub fn export_drawing_svg_to_file(
    project: &Project,
    drawing: &Drawing,
    options: &SvgExportOptions,
    path: &Path,
) -> Result<(), String> {
    let svg_string = export_drawing_svg(project, drawing, options)?;
    std::fs::write(path, svg_string).map_err(|e| format!("Failed to write SVG: {e}"))
}

/// Export all drawings to a directory, one SVG file per drawing.
/// Files are named `{number}-{title}.svg`.
pub fn export_all_drawings_svg(
    project: &Project,
    options: &SvgExportOptions,
    dir: &Path,
) -> Result<usize, String> {
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", dir.display()));
    }
    let mut count = 0;
    for drawing in &project.drawings {
        let filename = format!(
            "{}-{}.svg",
            drawing.full_sheet_ref(),
            drawing.title.replace(' ', "_").replace('/', "-"),
        );
        let path = dir.join(filename);
        export_drawing_svg_to_file(project, drawing, options, &path)?;
        count += 1;
    }
    Ok(count)
}

/// Convert model-space point to sheet coords through a viewport (for SVG export).
fn svg_model_to_sheet(model_pt: [f32; 2], vp: &crate::drawing::drawing::DrawingViewport) -> [f32; 2] {
    let vp_cx = vp.sheet_position[0] + vp.sheet_size[0] / 2.0;
    let vp_cy = vp.sheet_position[1] + vp.sheet_size[1] / 2.0;
    [
        vp_cx + (model_pt[0] - vp.model_center[0]) / vp.scale as f32,
        vp_cy + (model_pt[1] - vp.model_center[1]) / vp.scale as f32,
    ]
}
