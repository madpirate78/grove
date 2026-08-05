//! Geometry-to-SVG element conversion shared by model and drawing exports.

use super::plants::plant_circle_svg;
use super::*;

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

#[allow(clippy::too_many_arguments)]
pub(super) fn geometry_to_svg(
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
