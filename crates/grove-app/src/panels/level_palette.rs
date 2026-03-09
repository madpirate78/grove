use egui::Ui;

use crate::panels::widgets::*;
use crate::tools::place_level::{LevelSubType, PlaceLevelTool};

/// Draw the level palette. Returns `true` if the sub-type changed (caller should re-resolve layer).
pub fn draw_level_palette(ui: &mut Ui, tool: &mut PlaceLevelTool) -> bool {
    let mut sub_type_changed = false;

    ui.heading("Levels");
    ui.separator();

    // Sub-type selector
    let prev = tool.sub_type;
    egui::ComboBox::from_label("Type")
        .selected_text(tool.sub_type.label())
        .show_ui(ui, |ui| {
            for st in LevelSubType::ALL {
                ui.selectable_value(&mut tool.sub_type, *st, st.label());
            }
        });
    if tool.sub_type != prev {
        sub_type_changed = true;
    }

    ui.separator();

    // Colour swatch + layer
    let col = tool.sub_type.default_colour();
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            2.0,
            egui::Color32::from_rgb(col[0], col[1], col[2]),
        );
        ui.label(format!("Layer: {}", tool.sub_type.target_layer_name()));
    });

    ui.add_space(4.0);

    // Type-specific properties
    match tool.sub_type {
        LevelSubType::SpotHeight => {
            let sh = &mut tool.spot_height;
            ui.horizontal(|ui| {
                ui.label("Elevation:");
                ui.add(egui::DragValue::new(&mut sh.elevation_mm).suffix(" mm").speed(10.0));
            });
            combo_height_source(ui, &mut sh.source);
            ui.checkbox(&mut sh.label_display, "Show label");
        }
        LevelSubType::Contour => {
            let c = &mut tool.contour;
            ui.horizontal(|ui| {
                ui.label("Elevation:");
                ui.add(egui::DragValue::new(&mut c.elevation_mm).suffix(" mm").speed(10.0));
            });
            combo_contour_type(ui, &mut c.contour_type);
            combo_contour_source(ui, &mut c.source);
            ui.horizontal(|ui| {
                ui.label("Interval:");
                ui.add(egui::DragValue::new(&mut c.interval_mm).suffix(" mm").range(50..=5000));
            });
        }
        LevelSubType::SlopeArrow => {
            let sa = &mut tool.slope_arrow;
            ui.horizontal(|ui| {
                ui.label("Start level:");
                ui.add(egui::DragValue::new(&mut sa.start_level_mm).suffix(" mm").speed(10.0));
            });
            ui.horizontal(|ui| {
                ui.label("End level:");
                ui.add(egui::DragValue::new(&mut sa.end_level_mm).suffix(" mm").speed(10.0));
            });
        }
        LevelSubType::DrainRun => {
            let dr = &mut tool.drain_run;
            combo_drain_type(ui, &mut dr.drain_type);
            ui.horizontal(|ui| {
                ui.label("Pipe dia:");
                ui.add(egui::DragValue::new(&mut dr.pipe_dia_mm).suffix(" mm").range(50..=600));
            });
            ui.horizontal(|ui| {
                ui.label("Invert start:");
                ui.add(egui::DragValue::new(&mut dr.invert_start_mm).suffix(" mm").speed(10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Invert end:");
                ui.add(egui::DragValue::new(&mut dr.invert_end_mm).suffix(" mm").speed(10.0));
            });
            ui.checkbox(&mut dr.manhole, "Manhole");
        }
    }

    sub_type_changed
}

