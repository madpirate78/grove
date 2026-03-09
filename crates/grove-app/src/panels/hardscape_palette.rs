use egui::Ui;

use crate::panels::widgets::*;
use crate::tools::place_hardscape::{HardscapeSubType, PlaceHardscapeTool};

/// Draw the hardscape palette. Returns `true` if the sub-type changed (caller should re-resolve layer).
pub fn draw_hardscape_palette(ui: &mut Ui, tool: &mut PlaceHardscapeTool) -> bool {
    let mut sub_type_changed = false;

    ui.heading("Hardscape");
    ui.separator();

    // Sub-type selector
    let prev = tool.sub_type;
    egui::ComboBox::from_label("Type")
        .selected_text(tool.sub_type.label())
        .show_ui(ui, |ui| {
            for st in HardscapeSubType::ALL {
                ui.selectable_value(&mut tool.sub_type, *st, st.label());
            }
        });
    if tool.sub_type != prev {
        sub_type_changed = true;
    }

    ui.separator();

    // Show colour swatch
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
        HardscapeSubType::Wall => {
            let w = &mut tool.wall;
            combo_wall_type(ui, &mut w.wall_type);
            drag_mm(ui, "Height", &mut w.height_mm);
            drag_mm(ui, "Width", &mut w.width_mm);
            combo_material(ui, &mut w.base.material);
        }
        HardscapeSubType::Path => {
            let p = &mut tool.path;
            drag_mm(ui, "Width", &mut p.width_mm);
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut p.laying_pattern);
            });
            combo_drainage(ui, &mut p.drainage);
            combo_material(ui, &mut p.base.material);
        }
        HardscapeSubType::Patio => {
            let p = &mut tool.patio;
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut p.laying_pattern);
            });
            combo_patio_drainage(ui, &mut p.drainage);
            drag_mm(ui, "Fall (mm/m)", &mut p.fall_rate_mm_per_m);
            combo_material(ui, &mut p.base.material);
        }
        HardscapeSubType::Step => {
            let s = &mut tool.step;
            drag_mm(ui, "Riser H", &mut s.riser_height_mm);
            drag_mm(ui, "Tread D", &mut s.tread_depth_mm);
            drag_mm(ui, "Width", &mut s.width_mm);
            ui.add(egui::DragValue::new(&mut s.num_risers).prefix("Risers: ").range(1..=20));
            combo_material(ui, &mut s.base.material);
        }
        HardscapeSubType::Edging => {
            let e = &mut tool.edging;
            combo_edging_type(ui, &mut e.edging_type);
            drag_mm(ui, "Height", &mut e.height_mm);
            drag_mm(ui, "Width", &mut e.width_mm);
            combo_material(ui, &mut e.base.material);
        }
        HardscapeSubType::RaisedBed => {
            let r = &mut tool.raised_bed;
            drag_mm(ui, "Height", &mut r.height_mm);
            drag_mm(ui, "Wall W", &mut r.wall_width_mm);
            drag_mm(ui, "Soil D", &mut r.soil_depth_mm);
            ui.checkbox(&mut r.drainage, "Drainage");
            combo_material(ui, &mut r.base.material);
        }
        HardscapeSubType::Fence => {
            let f = &mut tool.fence;
            combo_fence_type(ui, &mut f.fence_type);
            drag_mm(ui, "Height", &mut f.height_mm);
            drag_mm(ui, "Post Spacing", &mut f.post_spacing_mm);
            combo_post_setting(ui, &mut f.post_setting);
            ui.checkbox(&mut f.gravel_board, "Gravel board");
        }
        HardscapeSubType::Pergola => {
            let p = &mut tool.pergola;
            combo_structure_type(ui, &mut p.structure_type);
            drag_mm(ui, "Height", &mut p.height_mm);
            combo_roof_type(ui, &mut p.roof);
            combo_post_fixing(ui, &mut p.post_fixing);
            combo_material(ui, &mut p.base.material);
        }
        HardscapeSubType::Decking => {
            let d = &mut tool.decking;
            drag_mm(ui, "Board W", &mut d.board_width_mm);
            drag_mm(ui, "Gap", &mut d.board_gap_mm);
            drag_mm(ui, "Deck H", &mut d.deck_height_mm);
            combo_substructure(ui, &mut d.substructure);
            ui.checkbox(&mut d.balustrade, "Balustrade");
            combo_material(ui, &mut d.base.material);
        }
        HardscapeSubType::WaterFeature => {
            let wf = &mut tool.water_feature;
            combo_water_feature_type(ui, &mut wf.feature_type);
            drag_mm(ui, "Depth", &mut wf.depth_mm);
            ui.checkbox(&mut wf.pump, "Pump");
            ui.checkbox(&mut wf.lighting, "Lighting");
            combo_filtration(ui, &mut wf.filtration);
        }
    }

    sub_type_changed
}

