use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{Entity, Geometry, CircleData};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Draw circles by dragging from center to edge point.
pub struct DrawCircleTool {
    center: Option<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawCircleTool {
    pub fn new() -> Self {
        Self {
            center: None,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn snapped_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(raw, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }
}

impl Tool for DrawCircleTool {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        let snapped = self.snapped_pos(pos, ctx);

        if self.center.is_none() {
            self.center = Some(snapped);
            ToolResult::Continue
        } else {
            let center = self.center.unwrap();
            let radius = (snapped - center).length();

            if radius > 1.0 {
                let mut entity = Entity::cad(
                    "Circle",
                    Geometry::Circle(CircleData { center, radius }),
                );
                entity.layer_id = self.layer_id;
                entity.colour = [220, 220, 225, 255];
                self.center = None;
                ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
            } else {
                ToolResult::Continue
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.snapped_pos(pos, ctx);

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.center.is_some() {
            self.center = None;
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        if let Some(center) = self.center {
            let screen_center = viewport.world_to_screen(center);
            let screen_edge = viewport.world_to_screen(self.current_pos);
            let screen_radius = screen_center.distance(screen_edge);

            painter.circle_stroke(
                screen_center,
                screen_radius,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 220, 225, 140)),
            );

            // Radius line
            painter.line_segment(
                [screen_center, screen_edge],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 150, 158, 100)),
            );

            // Center dot
            painter.circle_filled(screen_center, 3.0, egui::Color32::from_rgb(70, 130, 210));
        }

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
