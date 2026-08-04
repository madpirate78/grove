use std::collections::HashMap;
use egui::Color32;
use uuid::Uuid;

use grove_core::entity::{DomainData, Entity, Geometry};
use grove_core::garden::entourage::EntourageData;
use super::viewport::ViewTransform;

/// Caches GPU textures for entourage entity images.
pub struct EntourageRenderer {
    textures: HashMap<Uuid, egui::TextureHandle>,
}

impl EntourageRenderer {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Ensure a texture exists for an entourage entity. Call before drawing.
    fn ensure_texture(&mut self, ctx: &egui::Context, entity_id: Uuid, data: &EntourageData) {
        if self.textures.contains_key(&entity_id) {
            return;
        }

        // Decode PNG to RGBA
        let Ok(img) = image::load_from_memory(&data.image_data) else {
            log::warn!("Failed to decode entourage image for {}", data.name);
            return;
        };
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());

        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let handle = ctx.load_texture(
            format!("entourage_{}", entity_id),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(entity_id, handle);
    }

    /// Draw all visible entourage entities as textured quads.
    /// Returns the set of entity IDs that were rendered (so canvas can skip them).
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        viewport: &ViewTransform,
        entities: &[Entity],
        selected_ids: &[Uuid],
        visible_layer: &dyn Fn(Uuid) -> bool,
    ) -> Vec<Uuid> {
        let mut rendered = Vec::new();

        for entity in entities {
            if !visible_layer(entity.layer_id) {
                continue;
            }
            let DomainData::Entourage(ref data) = entity.domain else {
                continue;
            };
            let Geometry::Rectangle(ref r) = entity.geometry else {
                continue;
            };

            self.ensure_texture(ctx, entity.id, data);

            let Some(handle) = self.textures.get(&entity.id) else {
                continue;
            };

            let is_selected = selected_ids.contains(&entity.id);

            // Compute corners in screen space
            let half = r.half_size;
            let corners_world = if r.rotation.abs() < f32::EPSILON {
                [
                    r.center + glam::Vec2::new(-half.x, -half.y),
                    r.center + glam::Vec2::new(half.x, -half.y),
                    r.center + glam::Vec2::new(half.x, half.y),
                    r.center + glam::Vec2::new(-half.x, half.y),
                ]
            } else {
                let (sin, cos) = r.rotation.sin_cos();
                let local = [
                    glam::Vec2::new(-half.x, -half.y),
                    glam::Vec2::new(half.x, -half.y),
                    glam::Vec2::new(half.x, half.y),
                    glam::Vec2::new(-half.x, half.y),
                ];
                [
                    r.center + glam::Vec2::new(local[0].x * cos - local[0].y * sin, local[0].x * sin + local[0].y * cos),
                    r.center + glam::Vec2::new(local[1].x * cos - local[1].y * sin, local[1].x * sin + local[1].y * cos),
                    r.center + glam::Vec2::new(local[2].x * cos - local[2].y * sin, local[2].x * sin + local[2].y * cos),
                    r.center + glam::Vec2::new(local[3].x * cos - local[3].y * sin, local[3].x * sin + local[3].y * cos),
                ]
            };

            let corners_screen: [egui::Pos2; 4] = [
                viewport.world_to_screen(corners_world[0]),
                viewport.world_to_screen(corners_world[1]),
                viewport.world_to_screen(corners_world[2]),
                viewport.world_to_screen(corners_world[3]),
            ];

            // Build textured mesh quad
            let uvs = [
                egui::pos2(0.0, 0.0),
                egui::pos2(1.0, 0.0),
                egui::pos2(1.0, 1.0),
                egui::pos2(0.0, 1.0),
            ];

            let tint = Color32::WHITE;
            let mut mesh = egui::Mesh::with_texture(handle.id());
            for i in 0..4 {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: corners_screen[i],
                    uv: uvs[i],
                    color: tint,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));

            // Selection highlight
            if is_selected {
                let sel_stroke = egui::Stroke::new(2.0, crate::theme::ThemeColours::SELECTION);
                painter.add(egui::Shape::closed_line(corners_screen.to_vec(), sel_stroke));
            }

            rendered.push(entity.id);
        }

        rendered
    }

    pub fn clear(&mut self) {
        self.textures.clear();
    }
}
