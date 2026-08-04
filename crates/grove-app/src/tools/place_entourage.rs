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
/// Press R to cycle the placement rotation in 45° steps.
pub struct PlaceEntourageTool {
    /// Currently selected library item (cloned from the library).
    item: Option<EntourageLibraryItem>,
    pub layer_id: Uuid,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    /// Rotation applied to newly placed items, in radians.
    rotation_rad: f32,
}

impl PlaceEntourageTool {
    pub fn new() -> Self {
        Self {
            item: None,
            layer_id: Uuid::nil(),
            current_pos: Vec2::ZERO,
            snap: None,
            rotation_rad: 0.0,
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

    fn rotated_corners(&self, center: Vec2, half: Vec2) -> [Vec2; 4] {
        let local = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ];
        if self.rotation_rad.abs() < f32::EPSILON {
            return local.map(|p| center + p);
        }
        let (sin, cos) = self.rotation_rad.sin_cos();
        local.map(|p| center + Vec2::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos))
    }
}

impl Tool for PlaceEntourageTool {
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
                rotation: self.rotation_rad,
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

    fn on_key_pressed(&mut self, key: egui::Key) -> ToolResult {
        if key == egui::Key::R {
            self.rotation_rad += std::f32::consts::FRAC_PI_4; // 45°
            if self.rotation_rad >= std::f32::consts::TAU {
                self.rotation_rad -= std::f32::consts::TAU;
            }
            return ToolResult::Continue;
        }
        ToolResult::Continue
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let Some(ref item) = self.item else { return };

        let half_w = item.width_mm / 2.0;
        let half_h = item.height_mm / 2.0;
        let half = Vec2::new(half_w, half_h);

        let corners_world = self.rotated_corners(self.current_pos, half);
        let corners_screen: Vec<egui::Pos2> = corners_world
            .iter()
            .map(|p| viewport.world_to_screen(*p))
            .collect();

        let fill = egui::Color32::from_rgba_unmultiplied(120, 120, 120, 30);
        let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 120, 120, 160));

        let mut mesh = egui::Mesh::default();
        for (i, p) in corners_screen.iter().enumerate() {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *p,
                uv: egui::epaint::WHITE_UV,
                color: fill,
            });
            let _ = i;
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        // Work around egui's closed-path miter spike at sharp corners.
        crate::view::canvas::stroke_closed_loop(painter, corners_screen.clone(), stroke);

        // Label + rotation indicator above the preview
        let top_screen = viewport.world_to_screen(self.current_pos - Vec2::new(0.0, half_h));
        let rotation_deg = self.rotation_rad.to_degrees().round() as i32;
        let label = if rotation_deg == 0 {
            item.name.clone()
        } else {
            format!("{} · {}°", item.name, ((rotation_deg % 360) + 360) % 360)
        };
        painter.text(
            egui::pos2(top_screen.x, top_screen.y - 4.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgba_unmultiplied(200, 200, 205, 180),
        );

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
