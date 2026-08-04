use egui::Ui;
use grove_core::drawing::drawing::{ColourMode, Drawing, DrawingViewport, PlantStyle, RevisionStatus};
use grove_core::drawing::layout::{Orientation, PaperSize};
use grove_core::entity::Entity;
use grove_core::layer::LayerManager;

pub enum DrawingPropertiesAction {
    Updated(Box<Drawing>),
    ExportSvg,
    ExportPdf,
    BackToModel,
}

pub fn draw_drawing_properties(
    ui: &mut Ui,
    drawing: &Drawing,
    layers: &LayerManager,
    entities: &[Entity],
) -> Option<DrawingPropertiesAction> {
    let mut action = None;
    let mut drawing = drawing.clone();
    let mut changed = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
    ui.heading("Drawing Properties");
    ui.separator();

    // Title
    ui.horizontal(|ui| {
        ui.label("Title:");
        if ui.text_edit_singleline(&mut drawing.title).changed() {
            changed = true;
        }
    });

    // Number
    ui.horizontal(|ui| {
        ui.label("Number:");
        if ui.text_edit_singleline(&mut drawing.number).changed() {
            changed = true;
        }
    });

    // Paper size
    ui.horizontal(|ui| {
        ui.label("Paper:");
        let paper_label = match drawing.paper_size {
            PaperSize::A4 => "A4",
            PaperSize::A3 => "A3",
            PaperSize::A2 => "A2",
            PaperSize::A1 => "A1",
            PaperSize::A0 => "A0",
        };
        egui::ComboBox::from_id_salt("paper_size")
            .selected_text(paper_label)
            .show_ui(ui, |ui| {
                for (size, label) in [
                    (PaperSize::A4, "A4"),
                    (PaperSize::A3, "A3"),
                    (PaperSize::A2, "A2"),
                    (PaperSize::A1, "A1"),
                    (PaperSize::A0, "A0"),
                ] {
                    if ui.selectable_label(drawing.paper_size == size, label).clicked() {
                        drawing.paper_size = size;
                        changed = true;
                    }
                }
            });
    });

    // Orientation
    ui.horizontal(|ui| {
        ui.label("Orient:");
        if ui.selectable_label(drawing.orientation == Orientation::Landscape, "Landscape").clicked() {
            drawing.orientation = Orientation::Landscape;
            changed = true;
        }
        if ui.selectable_label(drawing.orientation == Orientation::Portrait, "Portrait").clicked() {
            drawing.orientation = Orientation::Portrait;
            changed = true;
        }
    });

    ui.separator();

    // Toggles
    if ui.checkbox(&mut drawing.show_title_block, "Title Block").changed() { changed = true; }
    if ui.checkbox(&mut drawing.show_north_arrow, "North Arrow").changed() { changed = true; }
    if ui.checkbox(&mut drawing.show_scale_bar, "Scale Bar").changed() { changed = true; }
    if ui.checkbox(&mut drawing.show_legend, "Legend").changed() { changed = true; }

    ui.separator();

    // Title block fields
    if drawing.show_title_block {
        ui.collapsing("Title Block Fields", |ui| {
            ui.horizontal(|ui| {
                ui.label("Project:");
                if ui.text_edit_singleline(&mut drawing.title_block.project_name).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Drawn by:");
                if ui.text_edit_singleline(&mut drawing.title_block.drawn_by).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Date:");
                if ui.text_edit_singleline(&mut drawing.title_block.date).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Client:");
                if ui.text_edit_singleline(&mut drawing.title_block.client).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Notes:");
                if ui.text_edit_singleline(&mut drawing.title_block.notes).changed() {
                    changed = true;
                }
            });
        });
        ui.separator();
    }

    // Viewports
    ui.heading("Viewports");
    let mut remove_vp: Option<usize> = None;
    let paper_dims = drawing.paper_dimensions_mm();
    let margins_copy = drawing.margins;
    for (i, vp) in drawing.viewports.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.strong(&vp.name);
                ui.label(format!("1:{}@{}", vp.scale, drawing.paper_size.label()));
                if ui.small_button("X").on_hover_text("Remove viewport").clicked() {
                    remove_vp = Some(i);
                }
            });

            // Name
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut vp.name).changed() {
                    changed = true;
                }
            });

            // Scale
            let mut scale_val = vp.scale as i32;
            if ui.add(egui::Slider::new(&mut scale_val, 1..=500).text("Scale 1:")).changed() {
                vp.scale = scale_val.max(1) as u32;
                changed = true;
            }

            // Utility buttons (position/size adjustable by mouse on canvas)
            if ui.button("Fit to margins").on_hover_text("Resize viewport to fill printable area").clicked() {
                let (pw, ph) = paper_dims;
                let [mt, mr, mb, ml] = margins_copy;
                vp.sheet_position = [ml, mt];
                vp.sheet_size = [pw - ml - mr, ph - mt - mb];
                changed = true;
            }
            ui.horizontal(|ui| {
                if ui.button("Center on design").on_hover_text("Set model center to centroid of all entities").clicked() {
                    if let Some((center, _)) = compute_entity_bounds(entities) {
                        vp.model_center = center;
                        changed = true;
                    }
                }
                if ui.button("Fit design").on_hover_text("Center and scale to show all entities").clicked() {
                    if let Some((center, extent)) = compute_entity_bounds(entities) {
                        vp.model_center = center;
                        let margin_factor = 1.1;
                        let scale_x = if vp.sheet_size[0] > 0.0 { extent[0] * margin_factor / vp.sheet_size[0] } else { 100.0 };
                        let scale_y = if vp.sheet_size[1] > 0.0 { extent[1] * margin_factor / vp.sheet_size[1] } else { 100.0 };
                        let scale = scale_x.max(scale_y).max(1.0);
                        vp.scale = nice_scale(scale);
                        changed = true;
                    }
                }
            });

            // Colour mode
            ui.horizontal(|ui| {
                ui.label("Colour:");
                egui::ComboBox::from_id_salt(format!("colour_mode_{}", i))
                    .selected_text(vp.colour_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in ColourMode::ALL {
                            if ui.selectable_label(vp.colour_mode == *mode, mode.label()).clicked() {
                                vp.colour_mode = *mode;
                                changed = true;
                            }
                        }
                    });
            });

            // Plant style
            ui.horizontal(|ui| {
                ui.label("Plants:");
                egui::ComboBox::from_id_salt(format!("plant_style_{}", i))
                    .selected_text(vp.plant_style.label())
                    .show_ui(ui, |ui| {
                        for style in PlantStyle::ALL {
                            if ui.selectable_label(vp.plant_style == *style, style.label()).clicked() {
                                vp.plant_style = *style;
                                changed = true;
                            }
                        }
                    });
            });

            // Layer overrides
            ui.collapsing("Layer Overrides", |ui| {
                if ui.button("Reset all").on_hover_text("Clear all overrides").clicked() {
                    vp.layer_overrides.visibility.clear();
                    changed = true;
                }
                for layer in &layers.layers {
                    let model_vis = layer.visible;
                    let override_vis = vp.layer_overrides.visibility.get(&layer.id).copied();
                    let effective = override_vis.unwrap_or(model_vis);
                    let mut checked = effective;

                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut checked, &layer.name).changed() {
                            if checked == model_vis {
                                vp.layer_overrides.visibility.remove(&layer.id);
                            } else {
                                vp.layer_overrides.visibility.insert(layer.id, checked);
                            }
                            changed = true;
                        }
                        if override_vis.is_some() && ui.small_button("reset").clicked() {
                            vp.layer_overrides.visibility.remove(&layer.id);
                            changed = true;
                        }
                    });
                }
            });
        });
        ui.separator();
    }

    if let Some(idx) = remove_vp {
        drawing.viewports.remove(idx);
        changed = true;
    }

    if ui.button("Add Viewport").clicked() {
        let (pw, ph) = drawing.paper_dimensions_mm();
        let [mt, mr, mb, ml] = drawing.margins;
        let mut vp = DrawingViewport::new(
            format!("View {}", drawing.viewports.len() + 1),
            100,
        );
        // Default: fill printable area
        vp.sheet_position = [ml, mt];
        vp.sheet_size = [pw - ml - mr, ph - mt - mb];
        drawing.viewports.push(vp);
        changed = true;
    }

    ui.add_space(8.0);
    ui.separator();

    // Revisions
    ui.heading("Revisions");
    ui.horizontal(|ui| {
        ui.label("Status:");
        if ui.selectable_label(drawing.revision_status == RevisionStatus::Preliminary, "Prelim").clicked() {
            drawing.revision_status = RevisionStatus::Preliminary;
            changed = true;
        }
        if ui.selectable_label(drawing.revision_status == RevisionStatus::Issued, "Issued").clicked() {
            drawing.revision_status = RevisionStatus::Issued;
            changed = true;
        }
    });

    for rev in &drawing.revisions {
        ui.horizontal(|ui| {
            ui.strong(&rev.label);
            ui.label(&rev.description);
            if !rev.by.is_empty() {
                ui.label(format!("({})", rev.by));
            }
        });
    }

    if ui.button("Add Revision").clicked() {
        drawing.add_revision("", "");
        changed = true;
    }

    ui.add_space(16.0);

    if ui.button("Export Drawing SVG...").clicked() {
        action = Some(DrawingPropertiesAction::ExportSvg);
    }
    if ui.button("Export Drawing PDF...").clicked() {
        action = Some(DrawingPropertiesAction::ExportPdf);
    }

    ui.add_space(8.0);
    if ui.button("Back to Model").clicked() {
        action = Some(DrawingPropertiesAction::BackToModel);
    }

    }); // end ScrollArea

    if changed && action.is_none() {
        action = Some(DrawingPropertiesAction::Updated(Box::new(drawing)));
    }

    action
}

/// Compute bounding box center and extent of all entities.
/// Returns (center [x,y], extent [width, height]) or None if no entities have geometry.
fn compute_entity_bounds(entities: &[Entity]) -> Option<([f32; 2], [f32; 2])> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut found = false;

    for entity in entities {
        if let Some(bb) = entity.bounding_box() {
            found = true;
            min_x = min_x.min(bb.min.x);
            min_y = min_y.min(bb.min.y);
            max_x = max_x.max(bb.max.x);
            max_y = max_y.max(bb.max.y);
        }
    }

    if !found {
        return None;
    }

    let center = [(min_x + max_x) / 2.0, (min_y + max_y) / 2.0];
    let extent = [max_x - min_x, max_y - min_y];
    Some((center, extent))
}

/// Round a raw scale factor to a nice drawing scale value.
fn nice_scale(raw: f32) -> u32 {
    const SCALES: &[u32] = &[1, 2, 5, 10, 20, 25, 50, 75, 100, 150, 200, 250, 500];
    let raw_u = raw.ceil() as u32;
    for &s in SCALES {
        if s >= raw_u {
            return s;
        }
    }
    // If larger than all nice scales, round up to nearest 50
    raw_u.div_ceil(50) * 50
}
