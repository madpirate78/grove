use std::collections::HashMap;

use egui::Color32;
use uuid::Uuid;

use grove_core::background_image::{apply_colour_mode, BackgroundImage, ColourMode};

use super::viewport::ViewTransform;

struct CachedTexture {
    handle: egui::TextureHandle,
    colour_mode: ColourMode,
}

/// Manages GPU textures for background images and draws them on the canvas.
pub struct BackgroundImageRenderer {
    textures: HashMap<Uuid, CachedTexture>,
}

impl BackgroundImageRenderer {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Ensure a texture is uploaded for the given background image.
    /// Re-uploads if the colour mode has changed.
    pub fn ensure_texture(&mut self, ctx: &egui::Context, img: &BackgroundImage) {
        let needs_update = match self.textures.get(&img.id) {
            Some(cached) => cached.colour_mode != img.colour_mode,
            None => true,
        };

        if !needs_update {
            return;
        }

        let Ok((mut pixels, w, h)) = img.decode_to_rgba() else {
            log::warn!("Failed to decode background image {}", img.name);
            return;
        };

        apply_colour_mode(&mut pixels, &img.colour_mode);

        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
        let handle = ctx.load_texture(
            format!("bg_{}", img.id),
            image,
            egui::TextureOptions::LINEAR,
        );

        self.textures.insert(
            img.id,
            CachedTexture {
                handle,
                colour_mode: img.colour_mode.clone(),
            },
        );
    }

    /// Draw a single background image onto the canvas.
    pub fn draw(
        &self,
        painter: &egui::Painter,
        viewport: &ViewTransform,
        img: &BackgroundImage,
    ) {
        if !img.visible {
            return;
        }

        let Some(cached) = self.textures.get(&img.id) else {
            return;
        };

        let alpha = (img.opacity * 255.0).clamp(0.0, 255.0) as u8;
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

        let pos = glam::Vec2::new(img.position[0], img.position[1]);
        let size = img.size_mm();
        let w = size[0];
        let h = size[1];

        let has_transform = img.rotation_rad.abs() > 1e-6 || img.flip_h || img.flip_v;

        if !has_transform {
            // Fast path: axis-aligned, no flip
            let bottom_right = pos + glam::Vec2::new(w, h);
            let screen_min = viewport.world_to_screen(pos);
            let screen_max = viewport.world_to_screen(bottom_right);
            let rect = egui::Rect::from_min_max(screen_min, screen_max);
            painter.image(
                cached.handle.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        } else {
            // Transformed: build a mesh with rotated corners and flipped UVs
            let (sin, cos) = img.rotation_rad.sin_cos();
            let rotate = |dx: f32, dy: f32| -> glam::Vec2 {
                glam::Vec2::new(dx * cos - dy * sin, dx * sin + dy * cos)
            };

            let corners_world = [
                pos,                          // top-left
                pos + rotate(w, 0.0),         // top-right
                pos + rotate(w, h),           // bottom-right
                pos + rotate(0.0, h),         // bottom-left
            ];

            let corners_screen: [egui::Pos2; 4] = [
                viewport.world_to_screen(corners_world[0]),
                viewport.world_to_screen(corners_world[1]),
                viewport.world_to_screen(corners_world[2]),
                viewport.world_to_screen(corners_world[3]),
            ];

            // UV coordinates with flip support
            let (u0, u1) = if img.flip_h { (1.0, 0.0) } else { (0.0, 1.0) };
            let (v0, v1) = if img.flip_v { (1.0, 0.0) } else { (0.0, 1.0) };
            let uvs = [
                egui::pos2(u0, v0), // top-left
                egui::pos2(u1, v0), // top-right
                egui::pos2(u1, v1), // bottom-right
                egui::pos2(u0, v1), // bottom-left
            ];

            let mut mesh = egui::Mesh::with_texture(cached.handle.id());
            for i in 0..4 {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: corners_screen[i],
                    uv: uvs[i],
                    color: tint,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

            painter.add(egui::Shape::mesh(mesh));
        }
    }

    /// Remove a specific texture.
    pub fn remove(&mut self, id: Uuid) {
        self.textures.remove(&id);
    }

    /// Remove all textures.
    pub fn clear(&mut self) {
        self.textures.clear();
    }
}

impl Default for BackgroundImageRenderer {
    fn default() -> Self {
        Self::new()
    }
}
