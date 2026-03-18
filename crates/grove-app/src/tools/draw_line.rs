use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{BezierPathData, Entity, Geometry, PolygonData, PolylineData, LineData};
use grove_core::snap::SnapResult;

use super::polygon_drawer::PolygonDrawer;
use super::{Tool, ToolContext, ToolResult};
use crate::view::viewport::ViewTransform;

/// Draw polylines/curves by clicking vertices. Press 'C' to toggle curve mode.
/// Double-click or Enter to finish. Shift constrains to horizontal/vertical.
///
/// In line mode: click adds corner points → straight segments.
/// In curve mode: click-drag adds smooth anchors with symmetric handles → curved segments.
pub struct DrawLineTool {
    drawer: PolygonDrawer,
    layer_id: uuid::Uuid,
}

impl DrawLineTool {
    pub fn new() -> Self {
        Self {
            drawer: PolygonDrawer::new(false), // polyline mode
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn finish_path(&mut self, closed: bool) -> ToolResult {
        if let Some(entity) = self.make_entity(closed) {
            let cmd = Box::new(AddEntityCommand::new(entity));
            self.drawer.clear();
            ToolResult::EmitAndContinue(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn make_entity(&self, closed: bool) -> Option<Entity> {
        if self.drawer.anchors.len() < 2 {
            return None;
        }

        let geom = if self.drawer.has_curves() {
            Geometry::BezierPath(BezierPathData {
                anchors: self.drawer.anchors.clone(),
                closed,
                holes: Vec::new(),
            })
        } else if closed && self.drawer.anchors.len() >= 3 {
            Geometry::Polygon(PolygonData {
                vertices: self.drawer.to_vertices(),
                holes: Vec::new(),
            })
        } else if self.drawer.anchors.len() == 2 {
            Geometry::Line(LineData {
                start: self.drawer.anchors[0].point,
                end: self.drawer.anchors[1].point,
            })
        } else {
            Geometry::Polyline(PolylineData {
                vertices: self.drawer.to_vertices(),
                closed: false,
            })
        };

        let name = if self.drawer.has_curves() {
            "Curve"
        } else if closed {
            "Polygon"
        } else {
            "Line"
        };
        let mut entity = Entity::cad(name, geom);
        entity.layer_id = self.layer_id;
        entity.colour = [220, 220, 225, 255];
        Some(entity)
    }
}

impl Tool for DrawLineTool {
    fn name(&self) -> &str {
        "Line"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        self.drawer.add_anchor(pos, ctx);
        ToolResult::Continue
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.drawer.update_cursor(pos, ctx);
    }

    fn on_mouse_up(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        self.drawer.finalize_handle();
        ToolResult::Continue
    }

    fn on_key_pressed(&mut self, key: egui::Key) -> ToolResult {
        match key {
            egui::Key::C => self.drawer.toggle_curve_mode(),
            egui::Key::E => self.drawer.toggle_edge_trace(),
            egui::Key::Tab => self.drawer.edge_trace_flip(),
            _ => {}
        }
        ToolResult::Continue
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_path(false)
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.drawer.edge_trace_cancel() {
            return ToolResult::Continue;
        }
        if self.drawer.anchors.is_empty() {
            ToolResult::Finished
        } else {
            self.drawer.clear();
            ToolResult::Continue
        }
    }

    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        // on_mouse_down already added the anchor for this click.
        self.drawer.drag_anchor_pos = None;
        self.drawer.dragging_handle = None;

        // Check if the last anchor is near the first — close as polygon
        let tolerance = 8.0 / ctx.viewport_zoom;
        let _ = pos;
        if self.drawer.anchors.len() >= 4 {
            let last = self.drawer.anchors.last().unwrap().point;
            if (last - self.drawer.anchors[0].point).length() <= tolerance {
                self.drawer.anchors.pop();
                return self.finish_path(true);
            }
        }

        self.finish_path(false)
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        self.drawer.drag_anchor_pos = None;
        self.drawer.dragging_handle = None;
        if self.drawer.anchors.len() > 1 {
            self.drawer.anchors.pop();
            ToolResult::Continue
        } else if !self.drawer.anchors.is_empty() {
            self.drawer.clear();
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        self.drawer.draw_preview(
            painter,
            viewport,
            [220, 220, 225, 255],
            false, // no closing line for polyline
            false, // DrawLineTool didn't have self-intersection prevention
        );
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.drawer.snap.as_ref()
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
                [
                    egui::pos2(screen.x - size, screen.y),
                    egui::pos2(screen.x + size, screen.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(screen.x, screen.y - size),
                    egui::pos2(screen.x, screen.y + size),
                ],
                stroke,
            );
        }
        SnapType::Intersection => {
            // X marker
            painter.line_segment(
                [
                    egui::pos2(screen.x - size, screen.y - size),
                    egui::pos2(screen.x + size, screen.y + size),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(screen.x + size, screen.y - size),
                    egui::pos2(screen.x - size, screen.y + size),
                ],
                stroke,
            );
        }
        SnapType::Extension => {
            // Cyan crosshair marker
            let cyan = egui::Color32::from_rgb(0, 200, 220);
            let ext_stroke = egui::Stroke::new(1.5, cyan);
            painter.line_segment(
                [
                    egui::pos2(screen.x - size, screen.y),
                    egui::pos2(screen.x + size, screen.y),
                ],
                ext_stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(screen.x, screen.y - size),
                    egui::pos2(screen.x, screen.y + size),
                ],
                ext_stroke,
            );
            painter.circle_stroke(screen, size * 0.7, ext_stroke);
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
