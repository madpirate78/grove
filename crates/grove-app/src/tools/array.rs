use glam::Vec2;
use uuid::Uuid;

use grove_core::commands::AddMultipleEntitiesCommand;
use grove_core::entity::Entity;

use super::{Tool, ToolContext, ToolResult};
use super::modify::draw_geometry_outline;
use crate::view::viewport::ViewTransform;

enum ArrayPhase {
    PickStart,
    PickEnd { start: Vec2 },
    Preview { start: Vec2, direction: Vec2 },
}

pub struct LinearArrayTool {
    original_entities: Vec<Entity>,
    count: usize,
    spacing_mm: f32,
    phase: ArrayPhase,
    current_pos: Vec2,
    input: String,
}

impl LinearArrayTool {
    pub fn new(target_ids: Vec<Uuid>, entities: &[Entity]) -> Self {
        let original_entities: Vec<_> = target_ids
            .iter()
            .filter_map(|id| entities.iter().find(|e| e.id == *id).cloned())
            .collect();

        Self {
            original_entities,
            count: 3,
            spacing_mm: 500.0,
            phase: ArrayPhase::PickStart,
            current_pos: Vec2::ZERO,
            input: String::new(),
        }
    }

    fn build_array_entities(&self, direction: Vec2) -> Vec<Entity> {
        let dir_norm = direction.normalize_or_zero();
        let mut new_entities = Vec::new();

        for i in 1..self.count {
            let offset = dir_norm * self.spacing_mm * i as f32;
            for orig in &self.original_entities {
                let mut clone = orig.clone();
                clone.id = Uuid::new_v4();
                clone.geometry.translate(offset);
                new_entities.push(clone);
            }
        }

        new_entities
    }

    pub fn status_hint(&self) -> &'static str {
        match &self.phase {
            ArrayPhase::PickStart => "Array: Click start point",
            ArrayPhase::PickEnd { .. } => "Array: Click end point (defines direction)",
            ArrayPhase::Preview { .. } => "Array: +/- count, type spacing, Enter to confirm",
        }
    }
}

impl Tool for LinearArrayTool {
    fn name(&self) -> &str {
        "Linear Array"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any_ref(&self) -> &dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        match &self.phase {
            ArrayPhase::PickStart => {
                self.phase = ArrayPhase::PickEnd { start: pos };
                ToolResult::Continue
            }
            ArrayPhase::PickEnd { start } => {
                let direction = pos - *start;
                if direction.length() < 1.0 {
                    return ToolResult::Continue;
                }
                // Set spacing from the click distance
                self.spacing_mm = direction.length();
                self.phase = ArrayPhase::Preview {
                    start: *start,
                    direction,
                };
                ToolResult::Continue
            }
            ArrayPhase::Preview { direction, .. } => {
                // Click confirms
                let entities = self.build_array_entities(*direction);
                if entities.is_empty() {
                    return ToolResult::Finished;
                }
                let cmd = Box::new(AddMultipleEntitiesCommand::new(entities));
                ToolResult::Emit(cmd)
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, _ctx: &ToolContext) {
        self.current_pos = pos;
    }

    fn on_key_enter(&mut self) -> ToolResult {
        if let ArrayPhase::Preview { direction, .. } = &self.phase {
            // Parse spacing from input if present
            if !self.input.is_empty() {
                if let Ok(spacing) = self.input.parse::<f32>() {
                    self.spacing_mm = spacing;
                    self.input.clear();
                }
            }
            let entities = self.build_array_entities(*direction);
            if entities.is_empty() {
                return ToolResult::Finished;
            }
            let cmd = Box::new(AddMultipleEntitiesCommand::new(entities));
            ToolResult::Emit(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        ToolResult::Finished
    }

    fn on_text_input(&mut self, text: &str) -> bool {
        if let ArrayPhase::Preview { .. } = &self.phase {
            if text == "\u{8}" {
                self.input.pop();
                return true;
            }
            // + increases count, - decreases
            if text == "+" || text == "=" {
                self.count = (self.count + 1).min(50);
                return true;
            }
            if text == "-" && self.input.is_empty() {
                self.count = self.count.saturating_sub(1).max(2);
                return true;
            }
            for c in text.chars() {
                if c.is_ascii_digit() || c == '.' {
                    self.input.push(c);
                }
            }
            true
        } else {
            false
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let preview_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(100, 200, 255, 160));
        let guide_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 200, 50, 100));

        match &self.phase {
            ArrayPhase::PickStart => {}
            ArrayPhase::PickEnd { start } => {
                // Show direction line preview
                let a = viewport.world_to_screen(*start);
                let b = viewport.world_to_screen(self.current_pos);
                painter.line_segment([a, b], guide_stroke);

                // Show marker at start
                painter.circle_filled(a, 4.0, egui::Color32::from_rgb(255, 200, 50));
            }
            ArrayPhase::Preview { start, direction } => {
                let dir_norm = direction.normalize_or_zero();

                // Draw direction line
                let a = viewport.world_to_screen(*start);
                let end_pt = *start + dir_norm * self.spacing_mm * self.count as f32;
                let b = viewport.world_to_screen(end_pt);
                painter.line_segment([a, b], guide_stroke);

                // Draw ghost copies
                for i in 1..self.count {
                    let offset = dir_norm * self.spacing_mm * i as f32;
                    for orig in &self.original_entities {
                        let mut preview_geom = orig.geometry.clone();
                        preview_geom.translate(offset);
                        draw_geometry_outline(painter, viewport, &preview_geom, preview_stroke);
                    }
                }

                // Show info text
                let info = if self.input.is_empty() {
                    format!(
                        "Count: {} | Spacing: {:.0}mm | +/- to adjust",
                        self.count, self.spacing_mm
                    )
                } else {
                    format!(
                        "Count: {} | Spacing: {}mm (typing)",
                        self.count, self.input
                    )
                };
                let text_pos = viewport.world_to_screen(self.current_pos);
                painter.text(
                    egui::pos2(text_pos.x + 15.0, text_pos.y - 15.0),
                    egui::Align2::LEFT_BOTTOM,
                    info,
                    egui::FontId::proportional(14.0),
                    egui::Color32::from_rgb(255, 200, 50),
                );
            }
        }
    }
}
