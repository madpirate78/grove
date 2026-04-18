use egui::Ui;

use grove_core::garden::entourage::{EntourageCategory, EntourageLibrary};
use crate::tools::place_entourage::PlaceEntourageTool;

/// Persistent state for the entourage palette.
#[derive(Debug)]
pub struct EntouragePaletteState {
    pub active_category: EntourageCategory,
}

impl Default for EntouragePaletteState {
    fn default() -> Self {
        Self {
            active_category: EntourageCategory::People,
        }
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
                // Auto-select first item in new category
                if let Some(first) = library.items_for_category(*cat).first() {
                    tool.set_item((*first).clone());
                    changed = true;
                }
            }
        }
    });

    ui.separator();

    // Items in current category
    let items = library.items_for_category(state.active_category);
    let current_name = tool.current_item().map(|i| i.source_file.clone());

    egui::ScrollArea::vertical().show(ui, |ui| {
        for item in &items {
            let selected = current_name.as_deref() == Some(&item.source_file);

            let response = ui.horizontal(|ui| {
                // Colour swatch / mini icon
                let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    2.0,
                    egui::Color32::from_rgb(120, 120, 120),
                );

                // Size info
                let size_label = format!(
                    "{} ({:.0}x{:.0}mm)",
                    item.name,
                    item.width_mm,
                    item.height_mm,
                );
                ui.selectable_label(selected, &size_label)
            });

            if response.inner.clicked() && !selected {
                tool.set_item((*item).clone());
                changed = true;
            }
        }
    });

    ui.separator();
    ui.add_space(4.0);
    ui.colored_label(
        egui::Color32::from_rgb(120, 120, 125),
        "Add custom SVG/PNG to\ndata/entourage/{category}/",
    );

    changed
}
