use glam::Vec2;
use uuid::Uuid;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{DomainData, Entity, Geometry, RectData};
use grove_core::garden::entourage::{EntourageData, EntourageLibraryItem};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Place entourage images by clicking. Shows a ghost preview at cursor.
/// Each click creates a single entity with `Geometry::Rectangle` and embedded image data.
pub struct PlaceEntourageTool {
    /// Currently selected library item (cloned from the library).
    item: Option<EntourageLibraryItem>,
    pub layer_id: Uuid,
    current_pos: Vec2,
    snap: Option<SnapResult>,
}

impl PlaceEntourageTool {
    pub fn new() -> Self {
        Self {
            item: None,
            layer_id: Uuid::nil(),
            current_pos: Vec2::ZERO,
            snap: None,
        }
    }

    pub fn set_item(&mut self, item: EntourageLibraryItem) {
        self.item = Some(item);
    }

    pub fn current_item(&self) -> Option<&EntourageLibraryItem> {
        self.item.as_ref()
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
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

impl Tool for PlaceEntourageTool {
    fn name(&self) -> &str {
        "Entourage"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let Some(ref item) = self.item else {
            return ToolResult::Continue;
        };

        self.layer_id = if self.layer_id.is_nil() { ctx.active_layer_id } else { self.layer_id };
        let effective = self.effective_pos(pos, ctx);

        let half_w = item.width_mm / 2.0;
        let half_h = item.height_mm / 2.0;

        let mut entity = Entity::new(
            &item.name,
            Geometry::Rectangle(RectData {
                center: effective,
                half_size: Vec2::new(half_w, half_h),
                rotation: 0.0,
            }),
            DomainData::Entourage(EntourageData {
                name: item.name.clone(),
                source_file: item.source_file.clone(),
                width_mm: item.width_mm,
                height_mm: item.height_mm,
                image_data: item.png_data.clone(),
                image_width: item.raster_width,
                image_height: item.raster_height,
            }),
        );
        entity.layer_id = self.layer_id;
        entity.colour = [120, 120, 120, 255];

        ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.effective_pos(pos, ctx);

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        ToolResult::Finished
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let Some(ref item) = self.item else { return };

        let half_w = item.width_mm / 2.0;
        let half_h = item.height_mm / 2.0;

        let top_left = self.current_pos - Vec2::new(half_w, half_h);
        let bottom_right = self.current_pos + Vec2::new(half_w, half_h);

        let screen_min = viewport.world_to_screen(top_left);
        let screen_max = viewport.world_to_screen(bottom_right);
        let rect = egui::Rect::from_min_max(screen_min, screen_max);

        // Semi-transparent preview rectangle
        let fill = egui::Color32::from_rgba_unmultiplied(120, 120, 120, 30);
        let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 120, 120, 100));
        painter.rect(rect, 0.0, fill, stroke, egui::StrokeKind::Outside);

        // Label
        painter.text(
            egui::pos2(rect.center().x, rect.min.y - 4.0),
            egui::Align2::CENTER_BOTTOM,
            &item.name,
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgba_unmultiplied(200, 200, 205, 160),
        );

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
