use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec2;
use svg::node::element::{self, path::Data};
use svg::Document;
use uuid::Uuid;

use crate::drawing::dimension::DimensionData;
use crate::drawing::drawing::Drawing;
use crate::drawing::layout::is_layer_visible_in_viewport;
use crate::drawing::legend::{collect_legend_items, render_legend_svg};
use crate::drawing::north_arrow::{render_north_arrow_svg, render_scale_bar_svg};
use crate::drawing::title_block_render::render_title_block_svg;
use crate::entity::{DomainData, Entity, Geometry};
use crate::garden::colour::rgba_to_hex;
use crate::geometry::offset_polyline;
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

    for (_layer_id, (layer_name, layer_entities)) in &layer_groups {
        let mut group = element::Group::new().set("id", layer_name.as_str());
        for entity in layer_entities {
            let stroke_colour = rgba_to_hex(entity.colour);
            let sw = options.stroke_width_mm;
            if let Some(elem) = geometry_to_svg(&entity.geometry, &entity.domain, &stroke_colour, sw, &entity.name) {
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
    options: &SvgExportOptions,
) -> Result<String, String> {
    let (paper_w, paper_h) = drawing.paper_dimensions_mm();

    let mut doc = Document::new()
        .set("width", format!("{}mm", paper_w))
        .set("height", format!("{}mm", paper_h))
        .set("viewBox", (0.0, 0.0, paper_w, paper_h))
        .set("xmlns", "http://www.w3.org/2000/svg");

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

        let mut vp_group = element::Group::new()
            .set("clip-path", format!("url(#{})", clip_id))
            .set("transform", transform);

        // Filter and render entities
        let entities: Vec<&Entity> = project.entities.iter()
            .filter(|e| is_layer_visible_in_viewport(e.layer_id, vp, &project.layers))
            .collect();

        for entity in &entities {
            let stroke_colour = rgba_to_hex(entity.colour);
            let sw = options.stroke_width_mm * vp.scale as f32;
            if let Some(elem) = geometry_to_svg(
                &entity.geometry, &entity.domain, &stroke_colour, sw, &entity.name,
            ) {
                vp_group = vp_group.add(elem);
            }
        }

        doc = doc.add(vp_group);

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

fn geometry_to_svg(
    geometry: &Geometry,
    domain: &DomainData,
    stroke_colour: &str,
    stroke_width: f32,
    name: &str,
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
                let outline = offset_polyline(&verts, w / 2.0);
                let fill = format!(
                    "rgba({},{},{},0.12)",
                    parse_hex_r(stroke_colour),
                    parse_hex_g(stroke_colour),
                    parse_hex_b(stroke_colour)
                );
                let poly = element::Polygon::new()
                    .set("points", points_to_string(&outline))
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width)
                    .set("fill", fill);
                return Some(Box::new(poly));
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
                    let outline = offset_polyline(&pl.vertices, w / 2.0);
                    let fill = format!(
                        "rgba({},{},{},0.12)",
                        parse_hex_r(stroke_colour),
                        parse_hex_g(stroke_colour),
                        parse_hex_b(stroke_colour)
                    );
                    let poly = element::Polygon::new()
                        .set("points", points_to_string(&outline))
                        .set("stroke", stroke_colour)
                        .set("stroke-width", stroke_width)
                        .set("fill", fill);
                    return Some(Box::new(poly));
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
            let points_str = points_to_string(&pg.vertices);
            // Semi-transparent fill like canvas rendering
            let fill_colour = format!(
                "rgba({},{},{},0.12)",
                parse_hex_r(stroke_colour),
                parse_hex_g(stroke_colour),
                parse_hex_b(stroke_colour)
            );
            let poly = element::Polygon::new()
                .set("points", points_str)
                .set("stroke", stroke_colour)
                .set("stroke-width", stroke_width)
                .set("fill", fill_colour);
            Some(Box::new(poly))
        }

        Geometry::Circle(c) => {
            // Check if this is a plant (domain-aware rendering)
            if let DomainData::Plant(ref plant) = domain {
                let label = &plant.common_name;
                let fill_colour = format!(
                    "rgba({},{},{},0.24)",
                    parse_hex_r(stroke_colour),
                    parse_hex_g(stroke_colour),
                    parse_hex_b(stroke_colour)
                );
                let circle = element::Circle::new()
                    .set("cx", c.center.x)
                    .set("cy", c.center.y)
                    .set("r", c.radius)
                    .set("fill", fill_colour)
                    .set("stroke", stroke_colour)
                    .set("stroke-width", stroke_width);

                let font_size = (c.radius * 0.6).max(8.0);
                let text = element::Text::new(label.clone())
                    .set("x", c.center.x)
                    .set("y", c.center.y)
                    .set("text-anchor", "middle")
                    .set("dominant-baseline", "central")
                    .set("font-size", font_size)
                    .set("fill", stroke_colour);

                let group = element::Group::new().add(circle).add(text);
                Some(Box::new(group))
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

fn parse_hex_r(hex: &str) -> u8 {
    u8::from_str_radix(&hex[1..3], 16).unwrap_or(128)
}

fn parse_hex_g(hex: &str) -> u8 {
    u8::from_str_radix(&hex[3..5], 16).unwrap_or(128)
}

fn parse_hex_b(hex: &str) -> u8 {
    u8::from_str_radix(&hex[5..7], 16).unwrap_or(128)
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
        assert!(svg.contains("rgba(76,175,80,0.24)"), "Plant circle should have fill");
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
        assert!(svg.contains("rgba("), "Wall polygon should have rgba fill");
        // Should NOT contain a plain <polyline> for this entity
        assert!(!svg.contains("<polyline"), "Wall should not render as <polyline>");
    }
}
