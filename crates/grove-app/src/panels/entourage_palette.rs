use std::collections::HashMap;

use egui::Ui;

use grove_core::garden::entourage::{EntourageCategory, EntourageLibrary, EntourageLibraryItem};
use crate::tools::place_entourage::PlaceEntourageTool;

const THUMB_PX: f32 = 48.0;

/// Persistent state for the entourage palette.
///
/// Holds a cache of GPU textures for library items, keyed by `source_file`
/// (stable across app runs since library items come from files on disk).
pub struct EntouragePaletteState {
    pub active_category: EntourageCategory,
    thumbnails: HashMap<String, egui::TextureHandle>,
}

impl Default for EntouragePaletteState {
    fn default() -> Self {
        Self {
            active_category: EntourageCategory::People,
            thumbnails: HashMap::new(),
        }
    }
}

impl EntouragePaletteState {
    fn thumbnail_for(&mut self, ctx: &egui::Context, item: &EntourageLibraryItem) -> Option<&egui::TextureHandle> {
        let key = item.source_file.clone();
        if !self.thumbnails.contains_key(&key) {
            if item.raster_rgba.is_empty() || item.raster_width == 0 || item.raster_height == 0 {
                return None;
            }
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [item.raster_width as usize, item.raster_height as usize],
                &item.raster_rgba,
            );
            let handle = ctx.load_texture(
                format!("entourage_thumb_{}", item.source_file),
                image,
                egui::TextureOptions::LINEAR,
            );
            self.thumbnails.insert(key.clone(), handle);
        }
        self.thumbnails.get(&key)
    }
}

/// Draw the entourage palette. Returns `true` if the selected item changed.
pub fn draw_entourage_palette(
    ui: &mut Ui,
    tool: &mut PlaceEntourageTool,
    library: &EntourageLibrary,
    state: &mut EntouragePaletteState,
) -> bool {
    let mut changed = false;

    ui.heading("Entourage");
    ui.separator();

    if library.items.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(180, 140, 100),
            "No entourage files found.\nAdd SVG/PNG files to\ndata/entourage/{category}/",
        );
        return false;
    }

    // Category tabs
    ui.horizontal_wrapped(|ui| {
        for cat in EntourageCategory::ALL {
            let count = library.items_for_category(*cat).len();
            let label = format!("{} ({})", cat.label(), count);
            let selected = *cat == state.active_category;
            if ui.selectable_label(selected, &label).clicked() && !selected {
                state.active_category = *cat;
                if let Some(first) = library.items_for_category(*cat).first() {
                    tool.set_item((*first).clone());
                    changed = true;
                }
            }
        }
    });

    ui.separator();

    let items = library.items_for_category(state.active_category);
    let current_name = tool.current_item().map(|i| i.source_file.clone());

    let ctx = ui.ctx().clone();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for item in &items {
            let selected = current_name.as_deref() == Some(&item.source_file);
            let texture = state.thumbnail_for(&ctx, item).cloned();

            let response = ui
                .horizontal(|ui| {
                    // Thumbnail (falls back to grey swatch if texture unavailable).
                    match texture {
                        Some(handle) => {
                            // Preserve aspect ratio inside a THUMB_PX square.
                            let src_w = item.raster_width.max(1) as f32;
                            let src_h = item.raster_height.max(1) as f32;
                            let scale = (THUMB_PX / src_w).min(THUMB_PX / src_h);
                            let size = egui::vec2(src_w * scale, src_h * scale);
                            // Allocate a fixed square so rows line up.
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(THUMB_PX, THUMB_PX), egui::Sense::hover());
                            let image_rect = egui::Rect::from_center_size(rect.center(), size);
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                egui::Color32::from_rgb(28, 28, 30),
                            );
                            ui.painter().image(
                                handle.id(),
                                image_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(THUMB_PX, THUMB_PX), egui::Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                egui::Color32::from_rgb(120, 120, 120),
                            );
                        }
                    }

                    // Two-line info block to the right of the thumbnail. We draw the
                    // name as a colored label (not selectable_label) because the row's
                    // outer horizontal captures the click via interact(Sense::click()).
                    ui.vertical(|ui| {
                        let name_colour = if selected {
                            egui::Color32::from_rgb(110, 180, 255)
                        } else {
                            egui::Color32::from_rgb(220, 220, 225)
                        };
                        ui.colored_label(name_colour, &item.name);
                        ui.small(format!("{:.0} × {:.0} mm", item.width_mm, item.height_mm));
                    })
                    .response
                })
                .response;

            if response.interact(egui::Sense::click()).clicked() && !selected {
                tool.set_item((*item).clone());
                changed = true;
            }
        }
    });

    ui.separator();
    ui.add_space(4.0);
    ui.colored_label(
        egui::Color32::from_rgb(120, 120, 125),
        "Press R / Shift+R to rotate before placing.\nAdd custom SVG/PNG to\ndata/entourage/{category}/",
    );

    changed
}
