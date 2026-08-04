use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec2;
use svg::node::element::{self, path::Data};
use svg::Document;
use uuid::Uuid;

use crate::drawing::dimension::DimensionData;
use crate::drawing::drawing::{Drawing, PlantStyle};
use crate::drawing::layout::is_layer_visible_in_viewport;
use crate::drawing::legend::{collect_legend_items, render_legend_svg};
use crate::drawing::north_arrow::{render_north_arrow_svg, render_scale_bar_svg};
use crate::drawing::title_block_render::render_title_block_svg;
use crate::entity::{CircleData, DomainData, Entity, Geometry};
use crate::garden::plant::PlantData;
use crate::garden::colour::{resolve_colour, rgba_to_hex};
use crate::geometry::{offset_polyline, offset_polyline_sides};
use crate::project::Project;

pub struct SvgExportOptions {
    pub visible_only: bool,
    pub margin_mm: f32,
    pub stroke_width_mm: f32,
    pub background: Option<[u8; 4]>,
}

impl Default for SvgExportOptions {
    fn default() -> Self {
        Self {
            visible_only: true,
            margin_mm: 50.0,
            stroke_width_mm: 1.0,
            background: None,
        }
    }
}

/// Compute a stroke weight (in mm on paper) based on the entity's domain data.
/// Taller elements get heavier lines, creating visual hierarchy in plan view.
/// House always gets the heaviest weight as the dominant structure.
pub fn stroke_weight_for_domain(domain: &DomainData) -> f32 {
    const MIN_WEIGHT: f32 = 0.18;
    const MAX_WEIGHT: f32 = 0.50;
    const DEFAULT_WEIGHT: f32 = 0.25;
    const MAX_HEIGHT: f32 = 5000.0;
    const HOUSE_WEIGHT: f32 = 0.70;

    if matches!(domain, DomainData::House(_)) {
        return HOUSE_WEIGHT;
    }
    if matches!(domain, DomainData::Entourage(_)) {
        return MIN_WEIGHT;
    }

    match domain.height_mm() {
        None => DEFAULT_WEIGHT,
        Some(h) => {
            let t = (h.max(0) as f32 / MAX_HEIGHT).clamp(0.0, 1.0);
            MIN_WEIGHT + t * (MAX_WEIGHT - MIN_WEIGHT)
        }
    }
}

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

/// Export a single drawing as a print-ready SVG with paper dimensions, viewports, and title block.
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

fn compute_bounds(entities: &[&Entity]) -> (Vec2, Vec2) {
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

/// Render a width-based shape (wall, path, step, edging) as SVG.
/// Matches the drawing view: individual filled quads per segment + stroke-only outline.
fn width_shape_to_svg(
    vertices: &[Vec2],
    width_mm: f32,
    stroke_colour: &str,
    stroke_width: f32,
) -> Box<dyn svg::node::Node> {
    let hw = width_mm / 2.0;
    let mut group = element::Group::new();

    // Filled quads per segment (light fill, no stroke)
    let (left, right) = offset_polyline_sides(vertices, hw);
    for i in 0..vertices.len() - 1 {
        let quad_points = format!(
            "{},{} {},{} {},{} {},{}",
            left[i].x, left[i].y,
            left[i + 1].x, left[i + 1].y,
            right[i + 1].x, right[i + 1].y,
            right[i].x, right[i].y,
        );
        let quad = element::Polygon::new()
            .set("points", quad_points)
            .set("fill", stroke_colour)
            .set("fill-opacity", 0.12)
            .set("stroke", "none");
        group = group.add(quad);
    }

    // Stroke-only outline
    let outline = offset_polyline(vertices, hw);
    let outline_poly = element::Polygon::new()
        .set("points", points_to_string(&outline))
        .set("stroke", stroke_colour)
        .set("stroke-width", stroke_width)
        .set("fill", "none");
    group = group.add(outline_poly);

    Box::new(group)
}

fn plant_label(plant: &PlantData) -> String {
    if !plant.common_name.is_empty() {
        plant.common_name.clone()
    } else {
        plant.botanical_name.clone()
    }
}

fn plant_circle_svg(
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
            group = group.add(plant_text(&plant_label(plant), c, font_size, stroke_colour));
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
            group = group.add(plant_text(&plant_label(plant), c, font_size, stroke_colour));
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
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.18)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width),
            );
            group = group.add(
                element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius * 0.6)
                    .set("fill", "none")
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width * 0.7),
            );
            group = group.add(plant_text(&plant_label(plant), c, font_size, stroke_colour));
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
            group = group.add(plant_text(&key, c, font_size * 1.4, stroke_colour));
        }
    }

    group
}

fn plant_text(text: &str, c: &CircleData, font_size: f32, stroke_colour: &str) -> element::Text {
    element::Text::new(text.to_string())
        .set("x", c.center.x)
        .set("y", c.center.y)
        .set("text-anchor", "middle")
        .set("dominant-baseline", "central")
        .set("font-size", font_size)
        .set("fill", stroke_colour)
}

#[allow(clippy::too_many_arguments)]
fn geometry_to_svg(
    geometry: &Geometry,
    domain: &DomainData,
    stroke_colour: &str,
    stroke_width: f32,
    name: &str,
    plant_style: PlantStyle,
    schedule_no: Option<usize>,
) -> Option<Box<dyn svg::Node>> {
    match geometry {
        Geometry::None => None,

        Geometry::Point(p) => {
            let c = element::Circle::new()
                .set("cx", p.x)
                .set("cy", p.y)
                .set("r", 1.5)
                .set("fill", stroke_colour);
            Some(Box::new(c))
        }

        Geometry::Line(l) => {
            // Width-aware rendering for walls, paths, steps, edging
            if let Some(w) = domain.width_mm() {
                let verts = vec![l.start, l.end];
                return Some(width_shape_to_svg(&verts, w, stroke_colour, stroke_width));
            }

            let line = element::Line::new()
                .set("x1", l.start.x)
                .set("y1", l.start.y)
                .set("x2", l.end.x)
                .set("y2", l.end.y)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", "none");
            Some(Box::new(line))
        }

        Geometry::Polyline(pl) => {
            // Width-aware rendering for walls, paths, steps, edging
            if let Some(w) = domain.width_mm() {
                if pl.vertices.len() >= 2 {
                    return Some(width_shape_to_svg(&pl.vertices, w, stroke_colour, stroke_width));
                }
            }

            let points_str = points_to_string(&pl.vertices);
            if pl.closed {
                let poly = element::Polygon::new()
                    .set("points", points_str)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", "none");
                Some(Box::new(poly))
            } else {
                let poly = element::Polyline::new()
                    .set("points", points_str)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", "none");
                Some(Box::new(poly))
            }
        }

        Geometry::Polygon(pg) => {
            if pg.holes.is_empty() {
                let points_str = points_to_string(&pg.vertices);
                let poly = element::Polygon::new()
                    .set("points", points_str)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.12);
                Some(Box::new(poly))
            } else {
                // Use <path> with fill-rule="evenodd" for polygons with holes
                let mut data = Data::new();
                // Outer ring
                for (i, v) in pg.vertices.iter().enumerate() {
                    if i == 0 {
                        data = data.move_to((v.x, v.y));
                    } else {
                        data = data.line_to((v.x, v.y));
                    }
                }
                data = data.close();
                // Hole rings
                for hole in &pg.holes {
                    for (i, v) in hole.iter().enumerate() {
                        if i == 0 {
                            data = data.move_to((v.x, v.y));
                        } else {
                            data = data.line_to((v.x, v.y));
                        }
                    }
                    data = data.close();
                }
                let path = element::Path::new()
                    .set("d", data)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", stroke_colour)
                    .set("fill-opacity", 0.12)
                    .set("fill-rule", "evenodd");
                Some(Box::new(path))
            }
        }

        Geometry::Circle(c) => {
            // Check if this is a plant (domain-aware rendering)
            if let DomainData::Plant(ref plant) = domain {
                Some(Box::new(plant_circle_svg(c, plant, stroke_colour, stroke_width, plant_style, schedule_no)))
            } else {
                let circle = element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", "none");
                Some(Box::new(circle))
            }
        }

        Geometry::Arc(a) => {
            let start_x = a.center.x + a.radius * a.start_angle.cos();
            let start_y = a.center.y + a.radius * a.start_angle.sin();
            let end_x = a.center.x + a.radius * a.end_angle.cos();
            let end_y = a.center.y + a.radius * a.end_angle.sin();

            let mut sweep = a.end_angle - a.start_angle;
            if sweep < 0.0 {
                sweep += std::f32::consts::TAU;
            }
            let large_arc = if sweep > std::f32::consts::PI { 1 } else { 0 };
            let sweep_flag = 1; // positive angle direction

            let data = Data::new()
                .move_to((start_x, start_y))
                .elliptical_arc_to((a.radius, a.radius, 0, large_arc, sweep_flag, end_x, end_y));

            let path = element::Path::new()
                .set("d", data)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", "none");
            Some(Box::new(path))
        }

        Geometry::Rectangle(r) => {
            let x = r.center.x - r.half_size.x;
            let y = r.center.y - r.half_size.y;
            let w = r.half_size.x * 2.0;
            let h = r.half_size.y * 2.0;

            // Entourage: embed image as base64 <image> element
            if let DomainData::Entourage(ref ent) = domain {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&ent.image_data);
                let href = format!("data:image/png;base64,{}", b64);
                let mut img = element::Image::new()
                    .set("x", x)
                    .set("y", y)
                    .set("width", w)
                    .set("height", h)
                    .set("href", href);
                if r.rotation.abs() > f32::EPSILON {
                    let deg = r.rotation.to_degrees();
                    img = img.set(
                        "transform",
                        format!("rotate({} {} {})", deg, r.center.x, r.center.y),
                    );
                }
                return Some(Box::new(img));
            }

            let mut rect = element::Rectangle::new()
                .set("x", x)
                .set("y", y)
                .set("width", w)
                .set("height", h)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", "none");

            if r.rotation.abs() > f32::EPSILON {
                let deg = r.rotation.to_degrees();
                rect = rect.set(
                    "transform",
                    format!("rotate({} {} {})", deg, r.center.x, r.center.y),
                );
            }
            Some(Box::new(rect))
        }

        Geometry::Text(t) => {
            let mut text_elem = element::Text::new(t.text.clone())
                .set("x", t.position.x)
                .set("y", t.position.y)
                .set("font-size", t.font_size)
                .set("fill", stroke_colour);

            if t.rotation.abs() > f32::EPSILON {
                let deg = t.rotation.to_degrees();
                text_elem = text_elem.set(
                    "transform",
                    format!("rotate({} {} {})", deg, t.position.x, t.position.y),
                );
            }
            Some(Box::new(text_elem))
        }

        Geometry::BezierPath(bp) => {
            // Width-aware rendering: flatten curve and use offset polygon (matches drawing view)
            if let Some(w) = domain.width_mm() {
                let flat = bp.flatten(16);
                if flat.len() >= 2 {
                    return Some(width_shape_to_svg(&flat, w, stroke_colour, stroke_width));
                }
            }

            let segments = bp.to_bezier_segments();
            if segments.is_empty() {
                return None;
            }

            let mut data = Data::new().move_to((segments[0].p0.x, segments[0].p0.y));
            for seg in &segments {
                data = data.cubic_curve_to((
                    seg.p1.x, seg.p1.y,
                    seg.p2.x, seg.p2.y,
                    seg.p3.x, seg.p3.y,
                ));
            }
            if bp.closed {
                data = data.close();
            }

            let path = element::Path::new()
                .set("d", data)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", "none");
            Some(Box::new(path))
        }

        Geometry::Dimension(dim) => {
            Some(Box::new(dimension_to_svg(dim, stroke_colour, stroke_width, name)))
        }
    }
}

fn dimension_to_svg(
    dim: &DimensionData,
    stroke_colour: &str,
    stroke_width: f32,
    _name: &str,
) -> element::Group {
    let mut group = element::Group::new();

    match dim {
        DimensionData::Linear { start, end, offset, text_override } => {
            let dir = (*end - *start).normalize_or_zero();
            let normal = Vec2::new(-dir.y, dir.x) * *offset;
            let ext_start = *start + normal;
            let ext_end = *end + normal;

            // Extension lines
            let ext1 = element::Line::new()
                .set("x1", start.x).set("y1", start.y)
                .set("x2", ext_start.x).set("y2", ext_start.y)
                .set("stroke", stroke_colour).set("stroke-width", stroke_width * 0.5);
            let ext2 = element::Line::new()
                .set("x1", end.x).set("y1", end.y)
                .set("x2", ext_end.x).set("y2", ext_end.y)
                .set("stroke", stroke_colour).set("stroke-width", stroke_width * 0.5);

            // Dimension line
            let dim_line = element::Line::new()
                .set("x1", ext_start.x).set("y1", ext_start.y)
                .set("x2", ext_end.x).set("y2", ext_end.y)
                .set("stroke", stroke_colour).set("stroke-width", stroke_width);

            // Label
            let mid = (ext_start + ext_end) * 0.5;
            let measurement = (*end - *start).length();
            let label = text_override.clone().unwrap_or_else(|| format!("{:.0}", measurement));
            let text = element::Text::new(label)
                .set("x", mid.x)
                .set("y", mid.y - 2.0)
                .set("text-anchor", "middle")
                .set("font-size", stroke_width * 8.0)
                .set("fill", stroke_colour);

            group = group.add(ext1).add(ext2).add(dim_line).add(text);
        }
        DimensionData::Angular { center, start_angle, end_angle, radius } => {
            let start_x = center.x + radius * start_angle.cos();
            let start_y = center.y + radius * start_angle.sin();
            let end_x = center.x + radius * end_angle.cos();
            let end_y = center.y + radius * end_angle.sin();

            let mut sweep = end_angle - start_angle;
            if sweep < 0.0 { sweep += std::f32::consts::TAU; }
            let large_arc = if sweep > std::f32::consts::PI { 1 } else { 0 };

            let data = Data::new()
                .move_to((start_x, start_y))
                .elliptical_arc_to((*radius, *radius, 0, large_arc, 1, end_x, end_y));
            let path = element::Path::new()
                .set("d", data)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", "none");

            let mid_angle = start_angle + sweep * 0.5;
            let mid_x = center.x + radius * mid_angle.cos();
            let mid_y = center.y + radius * mid_angle.sin();
            let label = format!("{:.1}°", sweep.to_degrees());
            let text = element::Text::new(label)
                .set("x", mid_x).set("y", mid_y)
                .set("text-anchor", "middle")
                .set("font-size", stroke_width * 8.0)
                .set("fill", stroke_colour);

            group = group.add(path).add(text);
        }
        DimensionData::Radial { center, radius, angle } => {
            let end_x = center.x + radius * angle.cos();
            let end_y = center.y + radius * angle.sin();

            let line = element::Line::new()
                .set("x1", center.x).set("y1", center.y)
                .set("x2", end_x).set("y2", end_y)
                .set("stroke", stroke_colour).set("stroke-width", stroke_width);

            let mid_x = (center.x + end_x) * 0.5;
            let mid_y = (center.y + end_y) * 0.5;
            let label = format!("R{:.0}", radius);
            let text = element::Text::new(label)
                .set("x", mid_x).set("y", mid_y - 2.0)
                .set("text-anchor", "middle")
                .set("font-size", stroke_width * 8.0)
                .set("fill", stroke_colour);

            group = group.add(line).add(text);
        }
    }

    group
}

fn points_to_string(points: &[Vec2]) -> String {
    points
        .iter()
        .map(|p| format!("{},{}", p.x, p.y))
        .collect::<Vec<_>>()
        .join(" ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::*;
    use crate::garden::plant::{Category, PlantData};
    use glam::Vec2;

    fn make_project() -> Project {
        Project::new()
    }

    #[test]
    fn test_empty_project() {
        let project = make_project();
        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("viewBox"));
        // Default fallback: 0,0 to 1000,1000 + margins
        assert!(svg.contains("1100mm")); // 1000 + 2*50
    }

    #[test]
    fn test_line_to_svg() {
        let mut project = make_project();
        let mut e = Entity::cad(
            "line1",
            Geometry::Line(LineData {
                start: Vec2::new(10.0, 20.0),
                end: Vec2::new(100.0, 200.0),
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<line"));
        assert!(svg.contains("x1=\"10\""));
        assert!(svg.contains("y1=\"20\""));
        assert!(svg.contains("x2=\"100\""));
        assert!(svg.contains("y2=\"200\""));
    }

    #[test]
    fn test_circle_to_svg() {
        let mut project = make_project();
        let mut e = Entity::cad(
            "circle1",
            Geometry::Circle(CircleData {
                center: Vec2::new(50.0, 60.0),
                radius: 30.0,
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<circle"));
        assert!(svg.contains("cx=\"50\""));
        assert!(svg.contains("cy=\"60\""));
        assert!(svg.contains("r=\"30\""));
        assert!(svg.contains("fill=\"none\""));
    }

    #[test]
    fn test_plant_circle_has_fill_and_label() {
        let mut project = make_project();
        let plant_data = PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: "Rosa rugosa".into(),
            common_name: "Rugosa Rose".into(),
            category: Category::Shrub,
            height_mm: 1500,
            spread_mm: 1200,
            colour_code: "#4CAF50".into(),
            flower_colour: "Pink".into(),
            foliage_colour: "Green".into(),
            evergreen: false,
            notes: String::new(),
        };
        let mut e = Entity::new(
            "plant1",
            Geometry::Circle(CircleData {
                center: Vec2::new(100.0, 100.0),
                radius: 600.0,
            }),
            DomainData::Plant(plant_data),
        );
        e.colour = [76, 175, 80, 255];
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("fill-opacity=\"0.24\""), "Plant circle should have fill opacity");
        assert!(svg.contains("Rugosa Rose"), "Plant should have label");
    }

    #[test]
    fn test_rectangle_to_svg() {
        let mut project = make_project();
        let mut e = Entity::cad(
            "rect1",
            Geometry::Rectangle(RectData {
                center: Vec2::new(50.0, 50.0),
                half_size: Vec2::new(25.0, 15.0),
                rotation: 0.0,
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<rect"));
        assert!(svg.contains("width=\"50\""));
        assert!(svg.contains("height=\"30\""));
    }

    #[test]
    fn test_arc_to_svg() {
        let mut project = make_project();
        let mut e = Entity::cad(
            "arc1",
            Geometry::Arc(ArcData {
                center: Vec2::new(100.0, 100.0),
                radius: 50.0,
                start_angle: 0.0,
                end_angle: std::f32::consts::FRAC_PI_2,
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.contains("A")); // Arc command
    }

    #[test]
    fn test_bezier_to_svg() {
        let mut project = make_project();
        let bp = BezierPathData {
            anchors: vec![
                BezierAnchor {
                    point: Vec2::new(0.0, 0.0),
                    handle_in: None,
                    handle_out: Some(Vec2::new(50.0, 0.0)),
                },
                BezierAnchor {
                    point: Vec2::new(100.0, 100.0),
                    handle_in: Some(Vec2::new(50.0, 100.0)),
                    handle_out: None,
                },
            ],
            closed: false,
            holes: Vec::new(),
        };
        let mut e = Entity::cad("bezier1", Geometry::BezierPath(bp));
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.contains("C")); // Cubic bezier command
    }

    #[test]
    fn test_text_to_svg() {
        let mut project = make_project();
        let mut e = Entity::cad(
            "text1",
            Geometry::Text(TextData {
                position: Vec2::new(10.0, 20.0),
                text: "Hello Garden".into(),
                font_size: 14.0,
                rotation: 0.0,
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<text"));
        assert!(svg.contains("Hello Garden"));
    }

    #[test]
    fn test_hidden_layer_excluded() {
        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;

        let mut e = Entity::cad(
            "hidden_line",
            Geometry::Line(LineData {
                start: Vec2::ZERO,
                end: Vec2::new(100.0, 0.0),
            }),
        );
        e.layer_id = layer_id;
        project.add_entity(e);

        // Hide the layer
        project.layers.layers[0].visible = false;

        let svg = export_svg(&project, &SvgExportOptions { visible_only: true, ..Default::default() }).unwrap();
        assert!(!svg.contains("<line"), "Hidden layer entities should be excluded");
    }

    #[test]
    fn test_layer_grouping() {
        let mut project = make_project();
        let layer0_id = project.layers.layers[0].id;
        let layer1_id = project.layers.layers[1].id;
        let layer0_name = project.layers.layers[0].name.clone();
        let layer1_name = project.layers.layers[1].name.clone();

        let mut e1 = Entity::cad(
            "line_a",
            Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0) }),
        );
        e1.layer_id = layer0_id;
        let mut e2 = Entity::cad(
            "line_b",
            Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(0.0, 10.0) }),
        );
        e2.layer_id = layer1_id;
        project.add_entity(e1);
        project.add_entity(e2);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains(&format!("id=\"{}\"", layer0_name)));
        assert!(svg.contains(&format!("id=\"{}\"", layer1_name)));
    }

    #[test]
    fn test_drawing_svg_paper_dimensions() {
        let project = make_project();
        let drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("841mm"), "Should have A1 landscape width");
        assert!(svg.contains("594mm"), "Should have A1 landscape height");
    }

    #[test]
    fn test_drawing_svg_has_title_block() {
        let mut project = make_project();
        project.metadata.name = "Test Garden".into();
        let drawing = crate::drawing::drawing::Drawing::new("01", "Master Plan");
        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("title-block"), "Should have title block group");
        assert!(svg.contains("Test Garden"), "Should auto-populate project name");
        assert!(svg.contains("Master Plan"), "Should auto-populate drawing title");
    }

    #[test]
    fn test_drawing_svg_viewport_clipping() {
        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;
        let mut e = Entity::cad(
            "line1",
            Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
        );
        e.layer_id = layer_id;
        project.add_entity(e);

        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        drawing.viewports.push(
            crate::drawing::drawing::DrawingViewport::new("Main", 100),
        );

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("clip-path"), "Should have clip path on viewport group");
        assert!(svg.contains("<clipPath"), "Should define clip path");
        assert!(svg.contains("<line"), "Should render the entity");
    }

    #[test]
    fn test_drawing_svg_layer_override_hides_entity() {
        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;
        let mut e = Entity::cad(
            "hidden_line",
            Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
        );
        e.layer_id = layer_id;
        project.add_entity(e);

        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        drawing.show_title_block = false;
        let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp.layer_overrides.visibility.insert(layer_id, false);
        drawing.viewports.push(vp);

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(!svg.contains("x1=\"0\""), "Entity on hidden layer should be excluded from viewport");
    }

    #[test]
    fn test_drawing_svg_numbered_plant_style() {
        use crate::drawing::drawing::PlantStyle;
        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;
        for (name, category, x) in [
            ("Rosa rugosa", Category::Shrub, 0.0_f32),
            ("Acer palmatum", Category::Tree, 2000.0),
        ] {
            let plant_data = PlantData {
                plant_id: uuid::Uuid::new_v4(),
                botanical_name: name.into(),
                common_name: name.into(),
                category,
                height_mm: 1000,
                spread_mm: 800,
                colour_code: "#4CAF50".into(),
                flower_colour: String::new(),
                foliage_colour: String::new(),
                evergreen: false,
                notes: String::new(),
            };
            let mut e = Entity::new(
                name,
                Geometry::Circle(CircleData { center: Vec2::new(x, 0.0), radius: 400.0 }),
                DomainData::Plant(plant_data),
            );
            e.layer_id = layer_id;
            project.add_entity(e);
        }

        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Planting");
        let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp.plant_style = PlantStyle::Numbered;
        drawing.viewports.push(vp);

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        // Schedule order: Rosa rugosa (Shrub) = 1, Acer palmatum (Tree) = 2
        assert!(svg.contains(">\n1\n</text>") || svg.contains(">1</text>"),
            "Numbered style should render schedule number 1");
        assert!(svg.contains(">\n2\n</text>") || svg.contains(">2</text>"),
            "Numbered style should render schedule number 2");
    }

    #[test]
    fn test_drawing_svg_monochrome() {
        use crate::drawing::drawing::ColourMode;

        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;
        let mut e = Entity::cad(
            "line1",
            Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
        );
        e.colour = [255, 0, 0, 255];
        e.layer_id = layer_id;
        project.add_entity(e);

        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp.colour_mode = ColourMode::Monochrome;
        drawing.viewports.push(vp);
        drawing.show_title_block = false;

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        // Monochrome should use dark grey (#282828) not red (#FF0000)
        assert!(svg.contains("#282828"), "Monochrome mode should produce dark grey stroke");
        assert!(!svg.contains("#FF0000"), "Monochrome mode should not use original red colour");
    }

    #[test]
    fn test_drawing_svg_by_species() {
        use crate::drawing::drawing::ColourMode;

        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;
        let plant_data = PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: "Rosa rugosa".into(),
            common_name: "Rugosa Rose".into(),
            category: Category::Shrub,
            height_mm: 1500,
            spread_mm: 1200,
            colour_code: "#FF0000".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen: false,
            notes: String::new(),
        };
        let mut e = Entity::new(
            "plant1",
            Geometry::Circle(CircleData {
                center: Vec2::new(0.0, 0.0),
                radius: 600.0,
            }),
            DomainData::Plant(plant_data),
        );
        e.colour = [255, 0, 0, 255];
        e.layer_id = layer_id;
        project.add_entity(e);

        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp.colour_mode = ColourMode::BySpecies;
        drawing.viewports.push(vp);
        drawing.show_title_block = false;
        drawing.show_legend = false;

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        // Should not use original red (legend disabled to isolate viewport rendering)
        assert!(!svg.contains("#FF0000"), "BySpecies mode should not use original colour");
        // Should contain a species-derived colour (deterministic)
        let species_c = crate::garden::colour::species_colour("Rosa rugosa");
        let species_hex = rgba_to_hex(species_c);
        assert!(svg.contains(&species_hex), "BySpecies mode should use species-derived colour");
    }

    #[test]
    fn test_drawing_svg_annotation_rendered() {
        let project = make_project();
        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        drawing.show_title_block = false;
        drawing.annotations.push(
            crate::drawing::drawing::DrawingAnnotation::new("Test Label", [100.0, 50.0])
        );

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("Test Label"), "Annotation text should appear in SVG");
        assert!(svg.contains("annotation"), "Annotation group class should appear");
    }

    #[test]
    fn test_drawing_svg_annotation_leader() {
        let mut project = make_project();
        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
        drawing.show_title_block = false;
        let vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        let vp_id = vp.id;
        drawing.viewports.push(vp);

        let mut ann = crate::drawing::drawing::DrawingAnnotation::new("With Leader", [50.0, 30.0]);
        ann.leader_target_model = Some([1000.0, 2000.0]);
        ann.viewport_id = Some(vp_id);
        drawing.annotations.push(ann);

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<line"), "Leader line should be in SVG");
        assert!(svg.contains("<polygon"), "Leader arrowhead should be in SVG");
        assert!(svg.contains("With Leader"), "Annotation text should be in SVG");
    }

    #[test]
    fn test_drawing_svg_entities_at_garden_coords() {
        // Simulate a real garden: entities centered around (5000, 4000) mm
        // with model_center set to the design centroid
        use crate::garden::hardscape::WallData;

        let mut project = make_project();
        let layer_id = project.layers.layers[0].id;

        // Garden boundary wall: 10m x 8m rectangle offset from origin
        let mut wall = Entity::cad(
            "boundary",
            Geometry::Polyline(PolylineData {
                vertices: vec![
                    Vec2::new(1000.0, 1000.0),
                    Vec2::new(9000.0, 1000.0),
                    Vec2::new(9000.0, 7000.0),
                    Vec2::new(1000.0, 7000.0),
                ],
                closed: true,
            }),
        );
        wall.domain = DomainData::Wall(WallData {
            width_mm: 215,
            ..Default::default()
        });
        wall.layer_id = layer_id;
        project.add_entity(wall);

        // Line across the garden
        let mut path_line = Entity::cad(
            "path_line",
            Geometry::Line(LineData {
                start: Vec2::new(3000.0, 1000.0),
                end: Vec2::new(3000.0, 7000.0),
            }),
        );
        path_line.layer_id = layer_id;
        path_line.colour = [100, 100, 100, 255];
        project.add_entity(path_line);

        // Test 1: model_center at design centroid, scale 1:100
        let mut drawing = crate::drawing::drawing::Drawing::new("01", "Garden Plan");
        drawing.paper_size = crate::drawing::layout::PaperSize::A3;
        drawing.show_title_block = false;
        let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp.model_center = [5000.0, 4000.0]; // centroid of the garden
        drawing.viewports.push(vp);

        let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
        assert!(svg.contains("<polygon"), "Wall should render as polygon in viewport");
        assert!(svg.contains("<line"), "Line entity should render in viewport");
        assert!(svg.contains("model_center=(5000,4000)"), "Diagnostic should show correct model_center");
        assert!(svg.contains("2 visible entities"), "Both entities should be visible");

        // Test 2: model_center at origin (default) — entities at 5000mm offset should
        // still be in the viewport at scale 1:100 (5000/100 = 50mm from center, viewport is 200mm from center)
        let mut drawing2 = crate::drawing::drawing::Drawing::new("02", "Default Center");
        drawing2.paper_size = crate::drawing::layout::PaperSize::A3;
        drawing2.show_title_block = false;
        let mut vp2 = crate::drawing::drawing::DrawingViewport::new("Main", 100);
        vp2.model_center = [0.0, 0.0]; // default
        drawing2.viewports.push(vp2);

        let svg2 = export_drawing_svg(&project, &drawing2, &SvgExportOptions::default()).unwrap();
        assert!(svg2.contains("<polygon"), "Wall should still render with default model_center at 1:100");

        // Test 3: at scale 1:30 with model_center at origin, entities at (5000,4000)
        // would be at (5000/30=167, 4000/30=133) mm from viewport center
        // Viewport is 200mm from center horizontally, 150mm vertically — should fit
        let mut drawing3 = crate::drawing::drawing::Drawing::new("03", "Scale 30");
        drawing3.paper_size = crate::drawing::layout::PaperSize::A3;
        drawing3.show_title_block = false;
        let mut vp3 = crate::drawing::drawing::DrawingViewport::new("Main", 30);
        vp3.model_center = [5000.0, 4000.0]; // centered on design
        drawing3.viewports.push(vp3);

        let svg3 = export_drawing_svg(&project, &drawing3, &SvgExportOptions::default()).unwrap();
        assert!(svg3.contains("scale=1:30"), "Diagnostic should show scale 1:30");
        assert!(svg3.contains("<polygon"), "Wall should render at 1:30 with centered viewport");
    }

    #[test]
    fn test_wall_polyline_renders_as_polygon_with_fill() {
        use crate::garden::hardscape::WallData;

        let mut project = make_project();
        let mut e = Entity::cad(
            "wall1",
            Geometry::Polyline(PolylineData {
                vertices: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1000.0, 0.0),
                    Vec2::new(1000.0, 1000.0),
                ],
                closed: false,
            }),
        );
        e.domain = DomainData::Wall(WallData {
            width_mm: 215,
            ..Default::default()
        });
        e.layer_id = project.layers.layers[0].id;
        project.add_entity(e);

        let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
        // Wall should render as a filled polygon, not a plain polyline
        assert!(svg.contains("<polygon"), "Wall should render as <polygon>");
        assert!(svg.contains("fill-opacity=\"0.12\""), "Wall polygon should have fill opacity");
        // Should NOT contain a plain <polyline> for this entity
        assert!(!svg.contains("<polyline"), "Wall should not render as <polyline>");
    }

    #[test]
    fn test_stroke_weight_range() {
        // No height → default 0.25
        let none_weight = stroke_weight_for_domain(&DomainData::None);
        assert!((none_weight - 0.25).abs() < 0.01, "None domain should get default weight");

        // Height 0 → min 0.18
        let ground_weight = stroke_weight_for_domain(&DomainData::Edging(
            crate::garden::hardscape::EdgingData { height_mm: 0, ..Default::default() },
        ));
        assert!((ground_weight - 0.18).abs() < 0.01, "Height 0 should get min weight");

        // Height 5000 → max 0.50
        let tree_weight = stroke_weight_for_domain(&DomainData::Plant(
            crate::garden::plant::PlantData {
                plant_id: uuid::Uuid::new_v4(),
                botanical_name: "Quercus robur".into(),
                common_name: "Oak".into(),
                category: crate::garden::plant::Category::Tree,
                height_mm: 5000,
                spread_mm: 4000,
                colour_code: "#4CAF50".into(),
                flower_colour: String::new(),
                foliage_colour: String::new(),
                evergreen: false,
                notes: String::new(),
            },
        ));
        assert!((tree_weight - 0.50).abs() < 0.01, "Height 5000 should get max weight");
    }

    #[test]
    fn test_stroke_weight_house_override() {
        let house = DomainData::House(crate::garden::hardscape::HouseData::default());
        let weight = stroke_weight_for_domain(&house);
        assert!((weight - 0.70).abs() < 0.01, "House should always get 0.70mm weight");

        // Even with a different height, house still gets fixed weight
        let mut house_data = crate::garden::hardscape::HouseData::default();
        house_data.height_mm = 100;
        let weight2 = stroke_weight_for_domain(&DomainData::House(house_data));
        assert!((weight2 - 0.70).abs() < 0.01, "House weight should be fixed regardless of height");
    }
}
