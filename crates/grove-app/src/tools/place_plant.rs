use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{CircleData, Entity, Geometry};
use grove_core::garden::colour::category_colour;
use grove_core::garden::plant::{Plant, PlantData};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Place plants by clicking. Shows a ghost circle preview at cursor.
/// Continuously places the currently selected plant from the palette.
pub struct PlacePlantTool {
    plant: Plant,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    pub layer_id: uuid::Uuid,
}

impl PlacePlantTool {
    pub fn new(plant: Plant) -> Self {
        Self {
            plant,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    pub fn set_plant(&mut self, plant: Plant) {
        self.plant = plant;
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(raw, ctx.entities, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }
}

impl Tool for PlacePlantTool {
    fn name(&self) -> &str {
        "Plant"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = if self.layer_id.is_nil() { ctx.active_layer_id } else { self.layer_id };
        let effective = self.effective_pos(pos, ctx);
        let radius = self.plant.radius_mm();

        let colour = category_colour(self.plant.category);
        let mut entity = Entity::new(
            &self.plant.botanical_name,
            Geometry::Circle(CircleData {
                center: effective,
                radius: if radius > 0.0 { radius } else { 200.0 },
            }),
            grove_core::entity::DomainData::Plant(PlantData::from_plant(&self.plant)),
        );
        entity.layer_id = self.layer_id;
        entity.colour = colour;

        ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.effective_pos(pos, ctx);

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config);
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
        let radius = self.plant.radius_mm();
        let r = if radius > 0.0 { radius } else { 200.0 };
        let screen_center = viewport.world_to_screen(self.current_pos);
        let screen_radius = r * viewport.zoom;
        let colour = category_colour(self.plant.category);
        let col = egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 80);
        let stroke_col = egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 180);

        painter.circle_filled(screen_center, screen_radius, col);
        painter.circle_stroke(screen_center, screen_radius, egui::Stroke::new(1.5, stroke_col));

        // Plant name label
        let label = if !self.plant.common_name.is_empty() {
            &self.plant.common_name
        } else {
            &self.plant.botanical_name
        };
        painter.text(
            egui::pos2(screen_center.x, screen_center.y + screen_radius + 4.0),
            egui::Align2::CENTER_TOP,
            label,
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
