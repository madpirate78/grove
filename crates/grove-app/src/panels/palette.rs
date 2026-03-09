use egui::Ui;
use grove_core::garden::plant::{Category, Plant};
use grove_core::garden::colour::category_colour;
use grove_core::garden::database::PlantDatabase;

/// State for the plant palette panel.
pub struct PaletteState {
    pub search_query: String,
    pub selected_category: Option<Category>,
    pub selected_plant_index: Option<usize>,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_category: None,
            selected_plant_index: None,
        }
    }
}

/// Draw the plant palette panel. Returns the currently selected plant (if any changed).
pub fn draw_palette(
    ui: &mut Ui,
    db: &PlantDatabase,
    state: &mut PaletteState,
) -> Option<Plant> {
    let mut newly_selected: Option<Plant> = None;

    ui.heading("Plant Palette");
    ui.separator();

    // Search bar
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);
    });

    ui.add_space(4.0);

    // Category filter tabs
    ui.horizontal_wrapped(|ui| {
        // "All" button
        let all_selected = state.selected_category.is_none();
        if ui.selectable_label(all_selected, "All").clicked() {
            state.selected_category = None;
        }
        for cat in Category::ALL {
            let is_selected = state.selected_category == Some(*cat);
            let colour = category_colour(*cat);
            let label = egui::RichText::new(cat.as_str())
                .color(egui::Color32::from_rgb(colour[0], colour[1], colour[2]));
            if ui.selectable_label(is_selected, label).clicked() {
                state.selected_category = Some(*cat);
            }
        }
    });

    ui.separator();

    // Filter plants
    let query = state.search_query.to_lowercase();
    let filtered: Vec<(usize, &Plant)> = db
        .plants
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if let Some(cat) = state.selected_category {
                if p.category != cat {
                    return false;
                }
            }
            if !query.is_empty() {
                let matches = p.botanical_name.to_lowercase().contains(&query)
                    || p.common_name.to_lowercase().contains(&query);
                if !matches {
                    return false;
                }
            }
            true
        })
        .collect();

    ui.label(format!("{} plants", filtered.len()));
    ui.add_space(4.0);

    // Plant list
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for (db_index, plant) in &filtered {
                let is_selected = state.selected_plant_index == Some(*db_index);
                let colour = category_colour(plant.category);
                let col = egui::Color32::from_rgb(colour[0], colour[1], colour[2]);

                let response = ui.horizontal(|ui| {
                    // Colour swatch
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, col);

                    // Plant name
                    let display = if !plant.common_name.is_empty() {
                        format!("{} ({})", plant.common_name, plant.botanical_name)
                    } else {
                        plant.botanical_name.clone()
                    };
                    let text = if is_selected {
                        egui::RichText::new(display).strong()
                    } else {
                        egui::RichText::new(display)
                    };
                    ui.selectable_label(is_selected, text)
                });

                if response.inner.clicked() {
                    state.selected_plant_index = Some(*db_index);
                    newly_selected = Some((*plant).clone());
                }

                // Show details for selected plant
                if is_selected {
                    ui.indent("plant_details", |ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(160, 160, 170),
                            format!("Category: {}", plant.category.as_str()),
                        );
                        if plant.height_mm > 0 {
                            ui.colored_label(
                                egui::Color32::from_rgb(160, 160, 170),
                                format!("Height: {}mm, Spread: {}mm", plant.height_mm, plant.spread_mm),
                            );
                        }
                        if !plant.flower_colour.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(160, 160, 170),
                                format!("Flower: {}", plant.flower_colour),
                            );
                        }
                        if !plant.notes.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(140, 140, 150),
                                &plant.notes,
                            );
                        }
                    });
                }
            }
        });

    newly_selected
}
