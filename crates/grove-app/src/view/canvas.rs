use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui};
use glam::Vec2;
use uuid::Uuid;

use grove_core::entity::{DomainData, Entity, Geometry};
use grove_core::geometry::{offset_polyline, offset_polyline_sides};

use super::viewport::ViewTransform;
use crate::theme::ThemeColours;

/// The main canvas widget: handles pan, zoom, grid rendering, entity rendering.
pub struct Canvas {
    pub viewport: ViewTransform,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            viewport: ViewTransform::new(),
        }
    }

    /// Draw the canvas and handle input. Returns the canvas rect, painter, and response.
    pub fn show(&mut self, ui: &mut Ui) -> CanvasResponse {
        let available = ui.available_size();
        let (response, painter) =
            ui.allocate_painter(available, Sense::click_and_drag());

        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 0.0, ThemeColours::CANVAS_BG);

        // Handle pan (middle mouse drag or right drag)
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            self.viewport.pan(response.drag_delta());
        }

        // Handle zoom (scroll) — only when pointer is over the canvas rect
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta.abs() > 0.0 {
            if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                if rect.contains(pointer) {
                    let factor = if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
                    self.viewport.zoom_at(pointer, factor);
                }
            }
        }

        // Draw grid
        self.draw_grid(&painter, rect);

        // Draw origin crosshair
        self.draw_origin(&painter, rect);

        CanvasResponse {
            rect,
            response,
            painter,
        }
    }

    /// Render all visible entities onto the canvas.
    pub fn draw_entities(
        &self,
        painter: &egui::Painter,
        entities: &[Entity],
        selected_ids: &[Uuid],
        visible_layer: impl Fn(Uuid) -> bool,
    ) {
        for entity in entities {
            if !visible_layer(entity.layer_id) {
                continue;
            }
            let is_selected = selected_ids.contains(&entity.id);
            self.draw_entity(painter, entity, is_selected);

            // Draw children
            for child in &entity.children {
                let child_selected = selected_ids.contains(&child.id);
                self.draw_entity(painter, child, child_selected);
            }
        }
    }

    fn draw_entity(&self, painter: &egui::Painter, entity: &Entity, selected: bool) {
        let colour = if selected {
            ThemeColours::SELECTION
        } else {
            Color32::from_rgba_premultiplied(
                entity.colour[0],
                entity.colour[1],
                entity.colour[2],
                entity.colour[3],
            )
        };
        let width = if selected { 2.0 } else { 1.5 };
        let stroke = Stroke::new(width, colour);

        // Unified width rendering for any geometry with a centerline
        if let Some(w) = entity.domain.width_mm() {
            if let Some(centerline) = entity.geometry.centerline_points() {
                let fill = Color32::from_rgba_unmultiplied(
                    entity.colour[0],
                    entity.colour[1],
                    entity.colour[2],
                    if selected { 60 } else { 40 },
                );
                let stroke_col = if selected {
                    ThemeColours::SELECTION
                } else {
                    Color32::from_rgba_unmultiplied(
                        entity.colour[0],
                        entity.colour[1],
                        entity.colour[2],
                        200,
                    )
                };
                let width_stroke = Stroke::new(if selected { 2.0 } else { 1.5 }, stroke_col);
                draw_width_shape(painter, &self.viewport, &centerline, w, fill, width_stroke);
                if selected {
                    if let Some(bb) = entity.geometry.bounding_box() {
                        self.draw_selection_handles(painter, bb.min, bb.max);
                    }
                }
                return;
            }
        }

        match &entity.geometry {
            Geometry::None => {}
            Geometry::Point(p) => {
                let screen = self.viewport.world_to_screen(*p);
                painter.circle_filled(screen, 3.0, colour);
            }
            Geometry::Line(l) => {
                let a = self.viewport.world_to_screen(l.start);
                let b = self.viewport.world_to_screen(l.end);
                painter.line_segment([a, b], stroke);
            }
            Geometry::Polyline(pl) => {
                if pl.vertices.len() >= 2 {
                    let points: Vec<Pos2> = pl
                        .vertices
                        .iter()
                        .map(|v| self.viewport.world_to_screen(*v))
                        .collect();
                    for w in points.windows(2) {
                        painter.line_segment([w[0], w[1]], stroke);
                    }
                    if pl.closed && points.len() >= 2 {
                        painter.line_segment(
                            [*points.last().unwrap(), points[0]],
                            stroke,
                        );
                    }
                }
            }
            Geometry::Polygon(pg) => {
                if pg.vertices.len() >= 3 {
                    let fill = if matches!(entity.domain, DomainData::None) {
                        Color32::TRANSPARENT
                    } else {
                        Color32::from_rgba_unmultiplied(
                            entity.colour[0],
                            entity.colour[1],
                            entity.colour[2],
                            30,
                        )
                    };

                    {
                        // Render all triangles as a single mesh so internal edges
                        // don't get individually anti-aliased (which causes artifacts).
                        let triangles = grove_core::geometry::triangulate_polygon_with_holes(
                            &pg.vertices,
                            &pg.holes,
                        );
                        let mut mesh = egui::Mesh::default();
                        for tri in &triangles {
                            let idx = mesh.vertices.len() as u32;
                            for &v in tri {
                                mesh.vertices.push(egui::epaint::Vertex {
                                    pos: self.viewport.world_to_screen(v),
                                    uv: egui::epaint::WHITE_UV,
                                    color: fill,
                                });
                            }
                            mesh.indices.extend_from_slice(&[idx, idx + 1, idx + 2]);
                        }
                        painter.add(egui::Shape::mesh(mesh));
                        // Draw outer boundary
                        let outer_pts: Vec<Pos2> = pg
                            .vertices
                            .iter()
                            .map(|v| self.viewport.world_to_screen(*v))
                            .collect();
                        painter.add(egui::Shape::closed_line(outer_pts, stroke));
                        // Draw hole boundaries
                        for hole in &pg.holes {
                            let hole_pts: Vec<Pos2> = hole
                                .iter()
                                .map(|v| self.viewport.world_to_screen(*v))
                                .collect();
                            painter.add(egui::Shape::closed_line(hole_pts, stroke));
                        }
                    }
                }
            }
            Geometry::Circle(c) => {
                let screen_center = self.viewport.world_to_screen(c.center);
                let screen_radius = c.radius * self.viewport.zoom;

                let is_plant = matches!(&entity.domain, DomainData::Plant(_));

                if is_plant {
                    // Plant rendering: filled circle with category colour
                    let fill = Color32::from_rgba_unmultiplied(
                        entity.colour[0], entity.colour[1], entity.colour[2], 60,
                    );
                    let stroke_col = if selected {
                        ThemeColours::SELECTION
                    } else {
                        Color32::from_rgba_unmultiplied(
                            entity.colour[0], entity.colour[1], entity.colour[2], 200,
                        )
                    };
                    painter.circle_filled(screen_center, screen_radius, fill);
                    painter.circle_stroke(
                        screen_center, screen_radius,
                        Stroke::new(if selected { 2.0 } else { 1.5 }, stroke_col),
                    );

                    // Name label (show if zoom is reasonable)
                    if screen_radius > 8.0 {
                        if let DomainData::Plant(ref pd) = &entity.domain {
                            let label = if !pd.common_name.is_empty() {
                                &pd.common_name
                            } else {
                                &pd.botanical_name
                            };
                            let font_size = (screen_radius * 0.35).clamp(8.0, 14.0);
                            painter.text(
                                screen_center,
                                egui::Align2::CENTER_CENTER,
                                label,
                                egui::FontId::proportional(font_size),
                                Color32::from_rgba_unmultiplied(240, 240, 245, 200),
                            );
                        }
                    }
                } else {
                    // Standard CAD circle rendering
                    if selected {
                        painter.circle_filled(
                            screen_center,
                            screen_radius,
                            Color32::from_rgba_unmultiplied(70, 160, 255, 20),
                        );
                    }
                    painter.circle_stroke(screen_center, screen_radius, stroke);
                    painter.circle_filled(screen_center, 2.0, colour);
                }
            }
            Geometry::Arc(a) => {
                // Approximate arc with line segments
                let n = 32;
                let screen_points: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        let angle = a.start_angle + t * (a.end_angle - a.start_angle);
                        let world = a.center + Vec2::new(angle.cos(), angle.sin()) * a.radius;
                        self.viewport.world_to_screen(world)
                    })
                    .collect();
                for w in screen_points.windows(2) {
                    painter.line_segment([w[0], w[1]], stroke);
                }
            }
            Geometry::Rectangle(r) => {
                let corners = rect_corners(r);
                let screen_corners: Vec<Pos2> = corners
                    .iter()
                    .map(|c| self.viewport.world_to_screen(*c))
                    .collect();

                // Fill
                if selected {
                    painter.add(egui::Shape::convex_polygon(
                        screen_corners.clone(),
                        Color32::from_rgba_unmultiplied(70, 160, 255, 20),
                        stroke,
                    ));
                } else {
                    painter.add(egui::Shape::convex_polygon(
                        screen_corners.clone(),
                        Color32::TRANSPARENT,
                        stroke,
                    ));
                }
            }
            Geometry::Text(t) => {
                let screen = self.viewport.world_to_screen(t.position);
                let font_size = (t.font_size * self.viewport.zoom).max(8.0);
                painter.text(
                    screen,
                    egui::Align2::LEFT_TOP,
                    &t.text,
                    egui::FontId::proportional(font_size),
                    colour,
                );
            }
            Geometry::BezierPath(bp) => {
                let flat = bp.flatten(16);
                if flat.len() >= 2 {
                    let screen_points: Vec<Pos2> = flat
                        .iter()
                        .map(|v| self.viewport.world_to_screen(*v))
                        .collect();
                    for w in screen_points.windows(2) {
                        painter.line_segment([w[0], w[1]], stroke);
                    }
                    if bp.closed && screen_points.len() >= 2 {
                        painter.line_segment(
                            [*screen_points.last().unwrap(), screen_points[0]],
                            stroke,
                        );
                    }
                }
            }
            Geometry::Dimension(d) => {
                match d {
                    grove_core::drawing::dimension::DimensionData::Linear {
                        start, end, offset, ..
                    } => {
                        crate::tools::draw_dimension::draw_dimension_preview(
                            painter,
                            &self.viewport,
                            *start,
                            *end,
                            *offset,
                            colour,
                            stroke,
                        );
                    }
                    _ => {
                        // Angular/Radial dimensions — future
                    }
                }
            }
        }

        // Selection handles
        if selected {
            if let Some(bb) = entity.geometry.bounding_box() {
                self.draw_selection_handles(painter, bb.min, bb.max);
            }
        }
    }

    fn draw_selection_handles(&self, painter: &egui::Painter, min: Vec2, max: Vec2) {
        let handle_size = 4.0;
        let colour = ThemeColours::SELECTION;
        let corners = [
            min,
            Vec2::new(max.x, min.y),
            max,
            Vec2::new(min.x, max.y),
        ];
        for corner in &corners {
            let screen = self.viewport.world_to_screen(*corner);
            let rect = egui::Rect::from_center_size(screen, egui::vec2(handle_size * 2.0, handle_size * 2.0));
            painter.rect_filled(rect, 0.0, colour);
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, clip_rect: Rect) {
        let base_spacing = self.adaptive_grid_spacing();
        let minor_spacing = base_spacing;
        let major_spacing = base_spacing * 5.0;

        let world_min = self.viewport.screen_to_world(clip_rect.min);
        let world_max = self.viewport.screen_to_world(clip_rect.max);

        let min_x = (world_min.x.min(world_max.x) / minor_spacing).floor() as i64;
        let max_x = (world_min.x.max(world_max.x) / minor_spacing).ceil() as i64;
        let min_y = (world_min.y.min(world_max.y) / minor_spacing).floor() as i64;
        let max_y = (world_min.y.max(world_max.y) / minor_spacing).ceil() as i64;

        // Minor grid
        for ix in min_x..=max_x {
            let world_x = ix as f32 * minor_spacing;
            let screen = self.viewport.world_to_screen(Vec2::new(world_x, 0.0));
            if screen.x >= clip_rect.min.x && screen.x <= clip_rect.max.x {
                painter.line_segment(
                    [Pos2::new(screen.x, clip_rect.min.y), Pos2::new(screen.x, clip_rect.max.y)],
                    Stroke::new(1.0, ThemeColours::GRID_MINOR),
                );
            }
        }
        for iy in min_y..=max_y {
            let world_y = iy as f32 * minor_spacing;
            let screen = self.viewport.world_to_screen(Vec2::new(0.0, world_y));
            if screen.y >= clip_rect.min.y && screen.y <= clip_rect.max.y {
                painter.line_segment(
                    [Pos2::new(clip_rect.min.x, screen.y), Pos2::new(clip_rect.max.x, screen.y)],
                    Stroke::new(1.0, ThemeColours::GRID_MINOR),
                );
            }
        }

        // Major grid
        let major_min_x = (world_min.x.min(world_max.x) / major_spacing).floor() as i64;
        let major_max_x = (world_min.x.max(world_max.x) / major_spacing).ceil() as i64;
        let major_min_y = (world_min.y.min(world_max.y) / major_spacing).floor() as i64;
        let major_max_y = (world_min.y.max(world_max.y) / major_spacing).ceil() as i64;

        for ix in major_min_x..=major_max_x {
            let world_x = ix as f32 * major_spacing;
            let screen = self.viewport.world_to_screen(Vec2::new(world_x, 0.0));
            if screen.x >= clip_rect.min.x && screen.x <= clip_rect.max.x {
                painter.line_segment(
                    [Pos2::new(screen.x, clip_rect.min.y), Pos2::new(screen.x, clip_rect.max.y)],
                    Stroke::new(1.0, ThemeColours::GRID_MAJOR),
                );
            }
        }
        for iy in major_min_y..=major_max_y {
            let world_y = iy as f32 * major_spacing;
            let screen = self.viewport.world_to_screen(Vec2::new(0.0, world_y));
            if screen.y >= clip_rect.min.y && screen.y <= clip_rect.max.y {
                painter.line_segment(
                    [Pos2::new(clip_rect.min.x, screen.y), Pos2::new(clip_rect.max.x, screen.y)],
                    Stroke::new(1.0, ThemeColours::GRID_MAJOR),
                );
            }
        }
    }

    fn draw_origin(&self, painter: &egui::Painter, clip_rect: Rect) {
        let origin = self.viewport.world_to_screen(Vec2::ZERO);
        let len = 20.0;

        if clip_rect.contains(origin) {
            painter.line_segment(
                [origin, Pos2::new(origin.x + len, origin.y)],
                Stroke::new(2.0, Color32::from_rgb(200, 60, 60)),
            );
            painter.line_segment(
                [origin, Pos2::new(origin.x, origin.y + len)],
                Stroke::new(2.0, Color32::from_rgb(60, 200, 60)),
            );
        }
    }

    fn adaptive_grid_spacing(&self) -> f32 {
        let target_screen_spacing = 40.0;
        let raw_spacing = target_screen_spacing / self.viewport.zoom;
        let magnitude = 10.0_f32.powf(raw_spacing.log10().floor());
        let normalized = raw_spacing / magnitude;

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

    /// Get the current cursor position in world coordinates, if hovering.
    pub fn cursor_world_pos(&self, response: &egui::Response) -> Option<Vec2> {
        response
            .hover_pos()
            .map(|p| self.viewport.screen_to_world(p))
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from Canvas::show containing what the app needs.
pub struct CanvasResponse {
    pub rect: Rect,
    pub response: egui::Response,
    pub painter: egui::Painter,
}

/// Render a width-based shape from a centerline and width.
/// Uses miter-joined left/right offsets to fill quads per segment — corners join cleanly.
/// Callable from both canvas rendering and tool previews.
pub(crate) fn draw_width_shape(
    painter: &egui::Painter,
    viewport: &ViewTransform,
    centerline: &[Vec2],
    width_mm: f32,
    fill: Color32,
    stroke: Stroke,
) {
    if centerline.len() < 2 {
        return;
    }

    let hw = width_mm / 2.0;

    // Compute miter-joined left/right offset arrays
    let (left, right) = offset_polyline_sides(centerline, hw);

    // Fill each segment as a quad using the miter-joined offsets
    for i in 0..centerline.len() - 1 {
        let quad = vec![
            viewport.world_to_screen(left[i]),
            viewport.world_to_screen(left[i + 1]),
            viewport.world_to_screen(right[i + 1]),
            viewport.world_to_screen(right[i]),
        ];
        painter.add(egui::Shape::convex_polygon(quad, fill, Stroke::NONE));
    }

    // Outline using the full offset polygon for clean edges
    let outline = offset_polyline(centerline, hw);
    let screen_pts: Vec<Pos2> = outline
        .iter()
        .map(|v| viewport.world_to_screen(*v))
        .collect();
    painter.add(egui::Shape::Path(egui::epaint::PathShape {
        points: screen_pts,
        closed: true,
        fill: Color32::TRANSPARENT,
        stroke: egui::epaint::PathStroke::from(stroke),
    }));
}

/// Get the 4 corners of a RectData in world space.
fn rect_corners(r: &grove_core::entity::RectData) -> [Vec2; 4] {
    let half = r.half_size;
    if r.rotation.abs() < f32::EPSILON {
        [
            r.center + Vec2::new(-half.x, -half.y),
            r.center + Vec2::new(half.x, -half.y),
            r.center + Vec2::new(half.x, half.y),
            r.center + Vec2::new(-half.x, half.y),
        ]
    } else {
        let (sin, cos) = r.rotation.sin_cos();
        let local = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ];
        [
            r.center + Vec2::new(local[0].x * cos - local[0].y * sin, local[0].x * sin + local[0].y * cos),
            r.center + Vec2::new(local[1].x * cos - local[1].y * sin, local[1].x * sin + local[1].y * cos),
            r.center + Vec2::new(local[2].x * cos - local[2].y * sin, local[2].x * sin + local[2].y * cos),
            r.center + Vec2::new(local[3].x * cos - local[3].y * sin, local[3].x * sin + local[3].y * cos),
        ]
    }
}
