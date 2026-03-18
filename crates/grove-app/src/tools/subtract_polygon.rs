use glam::Vec2;
use uuid::Uuid;

use grove_core::commands::BooleanSubtractCommand;
use grove_core::snap::SnapResult;

use super::polygon_drawer::PolygonDrawer;
use super::{Tool, ToolContext, ToolResult};
use super::{collect_snap_centers, collect_snap_segments, ortho_constrain};
use crate::view::viewport::ViewTransform;

/// Shape mode for the cutout tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoutShape {
    /// Freeform polygon drawn with PolygonDrawer.
    Polygon,
    /// Circle: click center, click edge.
    Circle,
    /// Rectangle: click corner, click opposite corner.
    Rectangle,
}

/// Tool for cutting holes in polygon entities using boolean difference.
/// Supports polygon, circle, and rectangle cutout shapes.
/// Handles partial overlaps (notches from boundary) as well as full interior cutouts.
pub struct SubtractPolygonTool {
    target_entity_id: Uuid,
    pub cutout_shape: CutoutShape,
    drawer: PolygonDrawer,
    // Circle mode state
    circle_center: Option<Vec2>,
    // Rectangle mode state
    rect_first_corner: Option<Vec2>,
    // Shared cursor position
    current_pos: Vec2,
    snap: Option<SnapResult>,
}

impl SubtractPolygonTool {
    pub fn new(target_entity_id: Uuid) -> Self {
        Self {
            target_entity_id,
            cutout_shape: CutoutShape::Polygon,
            drawer: PolygonDrawer::new(true), // closing mode for polygon holes
            circle_center: None,
            rect_first_corner: None,
            current_pos: Vec2::ZERO,
            snap: None,
        }
    }

    /// Reset drawing state when switching cutout shape.
    pub fn reset_drawing_state(&mut self) {
        self.drawer.clear();
        self.circle_center = None;
        self.rect_first_corner = None;
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let anchor = match self.cutout_shape {
            CutoutShape::Circle => self.circle_center,
            CutoutShape::Rectangle => self.rect_first_corner,
            CutoutShape::Polygon => return self.drawer.effective_pos(raw, ctx),
        };
        let constrained = if ctx.shift_held && anchor.is_some() {
            ortho_constrain(anchor.unwrap(), raw)
        } else {
            raw
        };
        if ctx.snap_config.enabled {
            let mut segs = Vec::new();
            let mut centers = Vec::new();
            for entity in ctx.entities {
                collect_snap_segments(&entity.geometry, &mut segs);
                collect_snap_centers(&entity.geometry, &mut centers);
            }
            let mut candidates = grove_core::snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = grove_core::snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        constrained
    }

    fn finish_cutout_polygon(&mut self) -> ToolResult {
        if self.drawer.anchors.len() < 3 {
            return ToolResult::Continue;
        }
        let cutout_vertices = if self.drawer.has_curves() {
            self.drawer.to_bezier_path_closed().flatten(32)
        } else {
            self.drawer.to_vertices()
        };
        let cmd = Box::new(BooleanSubtractCommand::new(self.target_entity_id, cutout_vertices));
        self.drawer.clear();
        ToolResult::Emit(cmd)
    }

    fn finish_cutout_circle(&mut self) -> ToolResult {
        let center = match self.circle_center {
            Some(c) => c,
            None => return ToolResult::Continue,
        };
        let radius = (self.current_pos - center).length();
        if radius < 1.0 {
            return ToolResult::Continue;
        }
        let cutout_vertices = grove_core::boolean_ops::circle_to_polygon(center, radius, 48);
        let cmd = Box::new(BooleanSubtractCommand::new(self.target_entity_id, cutout_vertices));
        self.circle_center = None;
        ToolResult::Emit(cmd)
    }

    fn finish_cutout_rect(&mut self) -> ToolResult {
        let first = match self.rect_first_corner {
            Some(f) => f,
            None => return ToolResult::Continue,
        };
        let second = self.current_pos;
        let center = (first + second) * 0.5;
        let half_size = ((second - first) * 0.5).abs();
        if half_size.x < 1.0 || half_size.y < 1.0 {
            return ToolResult::Continue;
        }
        let cutout_vertices = grove_core::boolean_ops::rect_to_polygon(center, half_size);
        let cmd = Box::new(BooleanSubtractCommand::new(self.target_entity_id, cutout_vertices));
        self.rect_first_corner = None;
        ToolResult::Emit(cmd)
    }
}

impl Tool for SubtractPolygonTool {
    fn name(&self) -> &str {
        "Subtract"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any_ref(&self) -> &dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        match self.cutout_shape {
            CutoutShape::Polygon => {
                self.drawer.add_anchor(pos, ctx);
                ToolResult::Continue
            }
            CutoutShape::Circle => {
                let effective = self.effective_pos(pos, ctx);
                if self.circle_center.is_none() {
                    self.circle_center = Some(effective);
                    ToolResult::Continue
                } else {
                    self.current_pos = effective;
                    self.finish_cutout_circle()
                }
            }
            CutoutShape::Rectangle => {
                let effective = self.effective_pos(pos, ctx);
                if self.rect_first_corner.is_none() {
                    self.rect_first_corner = Some(effective);
                    ToolResult::Continue
                } else {
                    self.current_pos = effective;
                    self.finish_cutout_rect()
                }
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        match self.cutout_shape {
            CutoutShape::Polygon => {
                self.drawer.update_cursor(pos, ctx);
                self.current_pos = self.drawer.current_pos;
            }
            CutoutShape::Circle | CutoutShape::Rectangle => {
                let effective = self.effective_pos(pos, ctx);
                self.current_pos = effective;

                if ctx.snap_config.enabled {
                    let constrained = if ctx.shift_held {
                        let anchor = match self.cutout_shape {
                            CutoutShape::Circle => self.circle_center,
                            CutoutShape::Rectangle => self.rect_first_corner,
                            _ => None,
                        };
                        if let Some(a) = anchor { ortho_constrain(a, pos) } else { pos }
                    } else {
                        pos
                    };
                    let mut segs = Vec::new();
                    let mut centers = Vec::new();
                    for entity in ctx.entities {
                        collect_snap_segments(&entity.geometry, &mut segs);
                        collect_snap_centers(&entity.geometry, &mut centers);
                    }
                    let mut candidates = grove_core::snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
                    if !ctx.smart_points.is_empty() {
                        candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
                    }
                    let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
                    self.snap = grove_core::snap::best_snap(&candidates, tolerance);
                } else {
                    self.snap = None;
                }
            }
        }
    }

    fn on_mouse_up(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.cutout_shape == CutoutShape::Polygon {
            self.drawer.finalize_handle();
        }
        ToolResult::Continue
    }

    fn on_key_pressed(&mut self, key: egui::Key) -> ToolResult {
        if self.cutout_shape == CutoutShape::Polygon {
            match key {
                egui::Key::C => self.drawer.toggle_curve_mode(),
                egui::Key::E => self.drawer.toggle_edge_trace(),
                egui::Key::Tab => self.drawer.edge_trace_flip(),
                _ => {}
            }
        }
        ToolResult::Continue
    }

    fn on_key_enter(&mut self) -> ToolResult {
        match self.cutout_shape {
            CutoutShape::Polygon => self.finish_cutout_polygon(),
            CutoutShape::Circle => self.finish_cutout_circle(),
            CutoutShape::Rectangle => self.finish_cutout_rect(),
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.cutout_shape == CutoutShape::Polygon && self.drawer.edge_trace_cancel() {
            return ToolResult::Continue;
        }
        match self.cutout_shape {
            CutoutShape::Polygon => {
                if self.drawer.anchors.is_empty() {
                    ToolResult::Finished
                } else {
                    self.drawer.clear();
                    ToolResult::Continue
                }
            }
            CutoutShape::Circle => {
                if self.circle_center.is_none() {
                    ToolResult::Finished
                } else {
                    self.circle_center = None;
                    ToolResult::Continue
                }
            }
            CutoutShape::Rectangle => {
                if self.rect_first_corner.is_none() {
                    ToolResult::Finished
                } else {
                    self.rect_first_corner = None;
                    ToolResult::Continue
                }
            }
        }
    }

    fn on_double_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.cutout_shape != CutoutShape::Polygon {
            return ToolResult::Continue;
        }
        self.drawer.drag_anchor_pos = None;
        self.drawer.dragging_handle = None;

        // Remove duplicate trailing anchor
        if self.drawer.anchors.len() >= 2 {
            let last = self.drawer.anchors[self.drawer.anchors.len() - 1].point;
            let prev = self.drawer.anchors[self.drawer.anchors.len() - 2].point;
            if (last - prev).length() < 1.0 {
                self.drawer.anchors.pop();
            }
        }

        self.finish_cutout_polygon()
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        match self.cutout_shape {
            CutoutShape::Polygon => {
                self.drawer.drag_anchor_pos = None;
                self.drawer.dragging_handle = None;
                if self.drawer.anchors.len() > 1 {
                    self.drawer.undo_last();
                    ToolResult::Continue
                } else if !self.drawer.anchors.is_empty() {
                    self.drawer.clear();
                    ToolResult::Continue
                } else {
                    ToolResult::Finished
                }
            }
            CutoutShape::Circle => {
                if self.circle_center.is_some() {
                    self.circle_center = None;
                    ToolResult::Continue
                } else {
                    ToolResult::Finished
                }
            }
            CutoutShape::Rectangle => {
                if self.rect_first_corner.is_some() {
                    self.rect_first_corner = None;
                    ToolResult::Continue
                } else {
                    ToolResult::Finished
                }
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let col = [220u8, 100, 60, 255];
        let label_colour = egui::Color32::from_rgba_unmultiplied(220, 100, 60, 200);

        match self.cutout_shape {
            CutoutShape::Polygon => {
                self.drawer.draw_preview(painter, viewport, col, true, true);
            }
            CutoutShape::Circle => {
                let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 180));
                let fill = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 30);

                if let Some(center) = self.circle_center {
                    let screen_center = viewport.world_to_screen(center);
                    let screen_edge = viewport.world_to_screen(self.current_pos);
                    let screen_radius = screen_center.distance(screen_edge);

                    painter.circle_filled(screen_center, screen_radius, fill);
                    painter.circle_stroke(screen_center, screen_radius, stroke);

                    // Radius line
                    painter.line_segment(
                        [screen_center, screen_edge],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 80)),
                    );
                    painter.circle_filled(screen_center, 3.0, egui::Color32::from_rgb(col[0], col[1], col[2]));
                }
            }
            CutoutShape::Rectangle => {
                let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 180));
                let fill = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 30);

                if let Some(first) = self.rect_first_corner {
                    let a = viewport.world_to_screen(first);
                    let b = viewport.world_to_screen(self.current_pos);
                    let rect = egui::Rect::from_two_pos(a, b);
                    painter.rect_filled(rect, 0.0, fill);
                    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
                    painter.circle_filled(a, 3.0, egui::Color32::from_rgb(col[0], col[1], col[2]));
                }
            }
        }

        // Label near cursor
        let cursor_screen = viewport.world_to_screen(self.current_pos);
        let shape_label = match self.cutout_shape {
            CutoutShape::Polygon => "Cutout",
            CutoutShape::Circle => "Cutout Circle",
            CutoutShape::Rectangle => "Cutout Rect",
        };
        painter.text(
            egui::pos2(cursor_screen.x + 16.0, cursor_screen.y + 4.0),
            egui::Align2::LEFT_TOP,
            shape_label,
            egui::FontId::proportional(10.0),
            label_colour,
        );

        // Snap indicator
        let snap_ref = if self.cutout_shape == CutoutShape::Polygon {
            self.drawer.snap.as_ref()
        } else {
            self.snap.as_ref()
        };
        if let Some(snap) = snap_ref {
            super::draw_line::draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        if self.cutout_shape == CutoutShape::Polygon {
            self.drawer.snap.as_ref()
        } else {
            self.snap.as_ref()
        }
    }
}
