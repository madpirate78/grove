use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{Entity, Geometry, TextData};
use grove_core::snap::{self, SnapResult};

use super::{collect_snap_centers, collect_snap_segments, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Text annotation tool. Click to place, then type.
/// The text is entered via a buffer that accumulates keystrokes.
/// Press Enter to finish, Escape to cancel.
pub struct DrawTextTool {
    /// None = waiting for placement click, Some = editing text at position.
    position: Option<Vec2>,
    text_buffer: String,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
    font_size: f32,
}

impl DrawTextTool {
    pub fn new() -> Self {
        Self {
            position: None,
            text_buffer: String::new(),
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
            font_size: 24.0,
        }
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let mut segs = Vec::new();
            let mut centers = Vec::new();
            for entity in ctx.entities {
                collect_snap_segments(&entity.geometry, &mut segs);
                collect_snap_centers(&entity.geometry, &mut centers);
            }
            let mut candidates = snap::snap_points(raw, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(raw, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }

    fn finish_text(&mut self) -> ToolResult {
        if self.text_buffer.is_empty() {
            self.position = None;
            return ToolResult::Continue;
        }

        let pos = self.position.unwrap_or(Vec2::ZERO);
        let entity = {
            let mut e = Entity::cad(
                "Text",
                Geometry::Text(TextData {
                    position: pos,
                    text: self.text_buffer.clone(),
                    font_size: self.font_size,
                    rotation: 0.0,
                }),
            );
            e.layer_id = self.layer_id;
            e.colour = [220, 220, 225, 255];
            e
        };

        self.text_buffer.clear();
        self.position = None;
        ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
    }
}

impl Tool for DrawTextTool {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        if self.position.is_some() {
            // Already editing — place and start new
            let result = self.finish_text();
            return result;
        }
        self.position = Some(self.effective_pos(pos, ctx));
        self.text_buffer.clear();
        ToolResult::Continue
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = pos;
        if self.position.is_none() {
            if ctx.snap_config.enabled {
                let mut segs = Vec::new();
                let mut centers = Vec::new();
                for entity in ctx.entities {
                    collect_snap_segments(&entity.geometry, &mut segs);
                    collect_snap_centers(&entity.geometry, &mut centers);
                }
                let mut candidates = snap::snap_points(pos, &segs, &centers, ctx.snap_config);
                if !ctx.smart_points.is_empty() {
                    candidates.extend(super::extension_snap_candidates(pos, ctx.smart_points));
                }
                let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
                self.snap = snap::best_snap(&candidates, tolerance);
            } else {
                self.snap = None;
            }
        }
    }

    fn on_text_input(&mut self, text: &str) -> bool {
        if self.position.is_some() {
            // Handle backspace
            if text == "\u{8}" {
                self.text_buffer.pop();
            } else if !text.contains('\n') && !text.contains('\r') {
                self.text_buffer.push_str(text);
            }
            true
        } else {
            false
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        if self.position.is_some() {
            self.finish_text()
        } else {
            ToolResult::Continue
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.position.is_some() {
            self.position = None;
            self.text_buffer.clear();
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let accent = egui::Color32::from_rgb(70, 130, 210);

        if let Some(pos) = self.position {
            // Show text being typed with a cursor
            let screen = viewport.world_to_screen(pos);
            let display = format!("{}|", self.text_buffer);
            let font_size = (self.font_size * viewport.zoom).max(8.0);
            painter.text(
                screen,
                egui::Align2::LEFT_TOP,
                &display,
                egui::FontId::proportional(font_size),
                egui::Color32::from_rgb(220, 220, 225),
            );

            // Placement marker
            painter.circle_filled(screen, 3.0, accent);
        } else {
            // Show cursor position
            let screen = viewport.world_to_screen(self.current_pos);
            painter.text(
                screen,
                egui::Align2::LEFT_TOP,
                "T",
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgba_unmultiplied(220, 220, 225, 120),
            );
        }

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
