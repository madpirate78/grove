use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{Entity, Geometry, RectData};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Draw rectangles by dragging from one corner to the opposite.
/// Press to set first corner, release to set second. Shift constrains to square.
pub struct DrawRectTool {
    first_corner: Option<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawRectTool {
    pub fn new() -> Self {
        Self {
            first_corner: None,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn snapped_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(raw, ctx.entities, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }

    fn effective_second_corner(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let snapped = self.snapped_pos(raw, ctx);
        if ctx.shift_held {
            if let Some(first) = self.first_corner {
                // Constrain to square
                let delta = snapped - first;
                let side = delta.x.abs().max(delta.y.abs());
                return first + Vec2::new(side * delta.x.signum(), side * delta.y.signum());
            }
        }
        snapped
    }
}

impl Tool for DrawRectTool {
    fn name(&self) -> &str {
        "Rectangle"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        let snapped = self.snapped_pos(pos, ctx);

        if self.first_corner.is_none() {
            self.first_corner = Some(snapped);
            ToolResult::Continue
        } else {
            let first = self.first_corner.unwrap();
            let second = self.effective_second_corner(pos, ctx);
            let center = (first + second) * 0.5;
            let half_size = ((second - first) * 0.5).abs();

            if half_size.x > 1.0 && half_size.y > 1.0 {
                let mut entity = Entity::cad(
                    "Rectangle",
                    Geometry::Rectangle(RectData {
                        center,
                        half_size,
                        rotation: 0.0,
                    }),
                );
                entity.layer_id = self.layer_id;
                entity.colour = [220, 220, 225, 255];
                self.first_corner = None;
                ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
            } else {
                ToolResult::Continue
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.effective_second_corner(pos, ctx);

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.first_corner.is_some() {
            self.first_corner = None;
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        if let Some(first) = self.first_corner {
            let a = viewport.world_to_screen(first);
            let b = viewport.world_to_screen(self.current_pos);
            let rect = egui::Rect::from_two_pos(a, b);

            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(220, 220, 225, 20),
            );
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 225, 140)),
                egui::StrokeKind::Outside,
            );

            // Corner dots
            painter.circle_filled(a, 3.0, egui::Color32::from_rgb(70, 130, 210));
        }

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
