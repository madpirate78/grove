use glam::Vec2;
use uuid::Uuid;

use super::{Tool, ToolContext, ToolResult};
use crate::view::viewport::ViewTransform;

/// Selection state: which entities are selected.
#[derive(Debug, Default)]
pub struct Selection {
    pub ids: Vec<Uuid>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    pub fn set(&mut self, id: Uuid) {
        self.ids.clear();
        self.ids.push(id);
    }

    pub fn toggle(&mut self, id: Uuid) {
        if let Some(pos) = self.ids.iter().position(|&i| i == id) {
            self.ids.remove(pos);
        } else {
            self.ids.push(id);
        }
    }

    pub fn add(&mut self, id: Uuid) {
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.ids.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn single(&self) -> Option<Uuid> {
        if self.ids.len() == 1 {
            Some(self.ids[0])
        } else {
            None
        }
    }
}

/// Drag state for box selection.
#[derive(Debug, Clone, Copy)]
enum DragState {
    None,
    Dragging { start: Vec2 },
}

pub struct SelectTool {
    drag: DragState,
    current_pos: Vec2,
    /// IDs hit-tested this frame for click selection.
    hit_id: Option<Uuid>,
}

impl SelectTool {
    pub fn new() -> Self {
        Self {
            drag: DragState::None,
            current_pos: Vec2::ZERO,
            hit_id: None,
        }
    }

    /// After on_mouse_down, call this to get what was selected.
    /// Returns (hit_entity_id, is_box_select_complete, box_min, box_max).
    pub fn take_hit(&mut self) -> Option<Uuid> {
        self.hit_id.take()
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self.drag, DragState::Dragging { .. })
    }

    pub fn drag_rect(&self) -> Option<(Vec2, Vec2)> {
        match self.drag {
            DragState::Dragging { start } => {
                let min = Vec2::new(start.x.min(self.current_pos.x), start.y.min(self.current_pos.y));
                let max = Vec2::new(start.x.max(self.current_pos.x), start.y.max(self.current_pos.y));
                Some((min, max))
            }
            DragState::None => None,
        }
    }
}

impl Tool for SelectTool {
    fn name(&self) -> &str {
        "Select"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        // Hit test against entities (tolerance in world units)
        let tolerance = 8.0 / ctx.viewport_zoom;
        let mut hit = None;
        // Iterate in reverse so topmost entity is hit first
        for entity in ctx.entities.iter().rev() {
            if entity.hit_test(pos, tolerance) {
                hit = Some(entity.id);
                break;
            }
        }

        if let Some(id) = hit {
            self.hit_id = Some(id);
        } else {
            // Start box selection
            self.drag = DragState::Dragging { start: pos };
            self.hit_id = None;
        }

        ToolResult::Continue
    }

    fn on_mouse_move(&mut self, pos: Vec2, _ctx: &ToolContext) {
        self.current_pos = pos;
    }

    fn on_mouse_up(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        if let DragState::Dragging { start } = self.drag {
            // If the drag was very small, treat as a click on empty space (deselect)
            let dist = (pos - start).length();
            if dist < 4.0 / ctx.viewport_zoom {
                // Click on nothing
                self.drag = DragState::None;
                return ToolResult::Continue;
            }
            // Box select will be handled by the app reading drag_rect()
            self.drag = DragState::None;
        }
        ToolResult::Continue
    }

    fn on_key_escape(&mut self) -> ToolResult {
        self.drag = DragState::None;
        // Don't finish — select tool stays active, just clears drag
        ToolResult::Continue
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        // Draw box selection rectangle
        if let Some((min, max)) = self.drag_rect() {
            let screen_min = viewport.world_to_screen(min);
            let screen_max = viewport.world_to_screen(max);
            let rect = egui::Rect::from_min_max(screen_min, screen_max);

            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(70, 130, 210, 30),
            );
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 130, 210)),
                egui::StrokeKind::Outside,
            );
        }
    }
}
