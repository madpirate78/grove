use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{Entity, Geometry, PolylineData, LineData};
use grove_core::snap::{self, SnapResult};

use super::{collect_snap_centers, collect_snap_segments, ortho_constrain, Tool, ToolContext, ToolResult};
use crate::view::viewport::ViewTransform;

/// Draw polylines by clicking vertices. Double-click or Enter to finish.
/// Shift constrains to horizontal/vertical from the last point.
pub struct DrawLineTool {
    vertices: Vec<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawLineTool {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        // Ortho constraint first (shift)
        let constrained = if ctx.shift_held && !self.vertices.is_empty() {
            ortho_constrain(*self.vertices.last().unwrap(), raw)
        } else {
            raw
        };

        // Then snap
        if ctx.snap_config.enabled {
            let (segs, centers) = self.snap_data(ctx);
            let candidates = snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }

        constrained
    }

    fn snap_data(&self, ctx: &ToolContext) -> (Vec<(Vec2, Vec2)>, Vec<snap::CenterPoint>) {
        let mut segs = Vec::new();
        let mut centers = Vec::new();
        for entity in ctx.entities {
            collect_snap_segments(&entity.geometry, &mut segs);
            collect_snap_centers(&entity.geometry, &mut centers);
        }
        // Segments from the polyline being drawn
        for w in self.vertices.windows(2) {
            segs.push((w[0], w[1]));
        }
        (segs, centers)
    }

    fn finish_polyline(&mut self) -> ToolResult {
        if let Some(entity) = self.make_entity() {
            let cmd = Box::new(AddEntityCommand::new(entity));
            self.vertices.clear();
            ToolResult::EmitAndContinue(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn make_entity(&self) -> Option<Entity> {
        if self.vertices.len() < 2 {
            return None;
        }
        let geom = if self.vertices.len() == 2 {
            Geometry::Line(LineData {
                start: self.vertices[0],
                end: self.vertices[1],
            })
        } else {
            Geometry::Polyline(PolylineData {
                vertices: self.vertices.clone(),
                closed: false,
            })
        };
        let mut entity = Entity::cad("Line", geom);
        entity.layer_id = self.layer_id;
        entity.colour = [220, 220, 225, 255];
        Some(entity)
    }
}

impl Tool for DrawLineTool {
    fn name(&self) -> &str {
        "Line"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        let effective = self.effective_pos(pos, ctx);
        self.vertices.push(effective);
        ToolResult::Continue
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        let effective = self.effective_pos(pos, ctx);
        self.current_pos = effective;

        // Update snap indicator
        if ctx.snap_config.enabled {
            let constrained = if ctx.shift_held && !self.vertices.is_empty() {
                ortho_constrain(*self.vertices.last().unwrap(), pos)
            } else {
                pos
            };
            let (segments, centers) = self.snap_data(ctx);
            let candidates = snap::snap_points(constrained, &segments, &centers, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_polyline()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.vertices.is_empty() {
            ToolResult::Finished
        } else {
            self.vertices.clear();
            ToolResult::Continue
        }
    }

    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        // Add final point and finish
        let effective = self.effective_pos(pos, ctx);
        self.vertices.push(effective);
        self.finish_polyline()
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        // Right-click undoes last vertex, or cancels if none left
        if self.vertices.len() > 1 {
            self.vertices.pop();
            ToolResult::Continue
        } else if !self.vertices.is_empty() {
            self.vertices.clear();
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        if self.vertices.is_empty() {
            return;
        }

        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 220, 225));
        let preview_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 225, 140));

        // Draw confirmed segments
        for w in self.vertices.windows(2) {
            let a = viewport.world_to_screen(w[0]);
            let b = viewport.world_to_screen(w[1]);
            painter.line_segment([a, b], stroke);
        }

        // Draw preview segment to cursor
        let last = viewport.world_to_screen(*self.vertices.last().unwrap());
        let cursor = viewport.world_to_screen(self.current_pos);
        painter.line_segment([last, cursor], preview_stroke);

        // Draw vertex dots
        for v in &self.vertices {
            let screen = viewport.world_to_screen(*v);
            painter.circle_filled(screen, 3.0, egui::Color32::from_rgb(70, 130, 210));
        }

        // Draw snap indicator
        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}

/// Draw a snap indicator at the given snap result.
pub fn draw_snap_indicator(painter: &egui::Painter, viewport: &ViewTransform, snap: &SnapResult) {
    use grove_core::snap::SnapType;

    let screen = viewport.world_to_screen(snap.position);
    let size = 5.0;
    let colour = egui::Color32::from_rgb(255, 200, 50);
    let stroke = egui::Stroke::new(1.5, colour);

    match snap.snap_type {
        SnapType::Endpoint => {
            // Square marker
            let rect = egui::Rect::from_center_size(screen, egui::vec2(size * 2.0, size * 2.0));
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        }
        SnapType::Midpoint => {
            // Triangle marker
            let points = vec![
                egui::pos2(screen.x, screen.y - size),
                egui::pos2(screen.x - size, screen.y + size),
                egui::pos2(screen.x + size, screen.y + size),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                egui::Color32::TRANSPARENT,
                stroke,
            ));
        }
        SnapType::Center => {
            // Circle marker
            painter.circle_stroke(screen, size, stroke);
        }
        SnapType::Grid => {
            // Crosshair marker
            painter.line_segment(
                [egui::pos2(screen.x - size, screen.y), egui::pos2(screen.x + size, screen.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(screen.x, screen.y - size), egui::pos2(screen.x, screen.y + size)],
                stroke,
            );
        }
        SnapType::Intersection => {
            // X marker
            painter.line_segment(
                [egui::pos2(screen.x - size, screen.y - size), egui::pos2(screen.x + size, screen.y + size)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(screen.x + size, screen.y - size), egui::pos2(screen.x - size, screen.y + size)],
                stroke,
            );
        }
        SnapType::Perpendicular | SnapType::Nearest => {
            // Diamond marker
            let points = vec![
                egui::pos2(screen.x, screen.y - size),
                egui::pos2(screen.x + size, screen.y),
                egui::pos2(screen.x, screen.y + size),
                egui::pos2(screen.x - size, screen.y),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                egui::Color32::TRANSPARENT,
                stroke,
            ));
        }
    }
}
