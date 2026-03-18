use egui::Ui;

use grove_core::garden::hardscape::SurfaceLabel;

use crate::panels::widgets::*;
use crate::tools::place_hardscape::{
    HardscapeCategory, PlaceHardscapeTool, StructureKind, WallKind,
};

/// Draw the hardscape palette. Returns `true` if the category/kind changed (caller should re-resolve layer).
pub fn draw_hardscape_palette(ui: &mut Ui, tool: &mut PlaceHardscapeTool) -> bool {
    let mut changed = false;

    ui.heading("Hardscape");
    ui.separator();

    // Category selector
    let prev_cat = tool.category;
    egui::ComboBox::from_label("Category")
        .selected_text(tool.category.label())
        .show_ui(ui, |ui| {
            for cat in HardscapeCategory::ALL {
                ui.selectable_value(&mut tool.category, *cat, cat.label());
            }
        });
    if tool.category != prev_cat {
        changed = true;
    }

    ui.separator();

    // Show colour swatch and layer
    let col = match tool.category {
        HardscapeCategory::Surfaces => tool.surface.default_colour(),
        _ => tool.category.default_colour(),
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            2.0,
            egui::Color32::from_rgb(col[0], col[1], col[2]),
        );
        ui.label(format!("Layer: {}", tool.category.target_layer_name()));
    });

    ui.add_space(4.0);

    // Category-specific sub-selector and properties
    match tool.category {
        HardscapeCategory::Walls => {
            // Wall / Edging radio
            let prev_kind = tool.wall_kind;
            ui.horizontal(|ui| {
                for kind in WallKind::ALL {
                    ui.selectable_value(&mut tool.wall_kind, *kind, kind.label());
                }
            });
            if tool.wall_kind != prev_kind {
                changed = true;
            }

            ui.add_space(4.0);

            match tool.wall_kind {
                WallKind::Wall => {
                    let w = &mut tool.wall;
                    combo_wall_type(ui, &mut w.wall_type);
                    drag_mm(ui, "Height", &mut w.height_mm);
                    drag_mm(ui, "Width", &mut w.width_mm);
                    combo_material(ui, &mut w.base.material);
                }
                WallKind::Edging => {
                    let e = &mut tool.edging;
                    combo_edging_type(ui, &mut e.edging_type);
                    drag_mm(ui, "Height", &mut e.height_mm);
                    drag_mm(ui, "Width", &mut e.width_mm);
                    combo_material(ui, &mut e.base.material);
                }
            }
        }
        HardscapeCategory::Surfaces => {
            // Surface label selector
            combo_surface_label(ui, &mut tool.surface.label);

            // Width toggle for path mode
            let has_width = tool.surface.width_mm.is_some();
            let mut path_mode = has_width;
            ui.checkbox(&mut path_mode, "Regular width (path mode)");
            if path_mode != has_width {
                if path_mode {
                    tool.surface.width_mm = Some(900);
                } else {
                    tool.surface.width_mm = None;
                }
                changed = true;
            }

            if let Some(ref mut w) = tool.surface.width_mm {
                drag_mm(ui, "Width", w);
            }

            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut tool.surface.laying_pattern);
            });
            combo_drainage(ui, &mut tool.surface.drainage);
            combo_material(ui, &mut tool.surface.base.material);

            if matches!(tool.surface.label, SurfaceLabel::Decking) {
                drag_mm(ui, "Board W", &mut tool.surface.board_width_mm);
                drag_mm(ui, "Board Gap", &mut tool.surface.board_gap_mm);
            }
        }
        HardscapeCategory::Steps => {
            let s = &mut tool.step;
            drag_mm(ui, "Riser H", &mut s.riser_height_mm);
            drag_mm(ui, "Tread D", &mut s.tread_depth_mm);
            drag_mm(ui, "Width", &mut s.width_mm);
            ui.add(egui::DragValue::new(&mut s.num_risers).prefix("Risers: ").range(1..=20));
            combo_material(ui, &mut s.base.material);
        }
        HardscapeCategory::Structures => {
            let prev_kind = tool.structure_kind;
            ui.horizontal(|ui| {
                for kind in StructureKind::ALL {
                    ui.selectable_value(&mut tool.structure_kind, *kind, kind.label());
                }
            });
            if tool.structure_kind != prev_kind {
                changed = true;
            }

            ui.add_space(4.0);

            match tool.structure_kind {
                StructureKind::Fence => {
                    let f = &mut tool.fence;
                    combo_fence_type(ui, &mut f.fence_type);
                    drag_mm(ui, "Height", &mut f.height_mm);
                    drag_mm(ui, "Post Spacing", &mut f.post_spacing_mm);
                    combo_post_setting(ui, &mut f.post_setting);
                    ui.checkbox(&mut f.gravel_board, "Gravel board");
                }
                StructureKind::Pergola => {
                    let p = &mut tool.pergola;
                    combo_structure_type(ui, &mut p.structure_type);
                    drag_mm(ui, "Height", &mut p.height_mm);
                    combo_roof_type(ui, &mut p.roof);
                    combo_post_fixing(ui, &mut p.post_fixing);
                    combo_material(ui, &mut p.base.material);
                }
            }
        }
        HardscapeCategory::Water => {
            let wf = &mut tool.water_feature;
            combo_water_feature_type(ui, &mut wf.feature_type);
            drag_mm(ui, "Depth", &mut wf.depth_mm);
            ui.checkbox(&mut wf.pump, "Pump");
            ui.checkbox(&mut wf.lighting, "Lighting");
            combo_filtration(ui, &mut wf.filtration);
        }
        HardscapeCategory::Buildings => {
            let h = &mut tool.house;
            drag_mm(ui, "Wall Thickness", &mut h.wall_thickness_mm);
            drag_mm(ui, "Height", &mut h.height_mm);
            combo_material(ui, &mut h.base.material);
        }
    }

    changed
}
