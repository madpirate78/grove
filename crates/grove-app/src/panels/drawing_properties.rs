use egui::Ui;
use grove_core::drawing::drawing::{Drawing, DrawingViewport, RevisionStatus};
use grove_core::drawing::layout::{Orientation, PaperSize};
use grove_core::layer::LayerManager;

pub enum DrawingPropertiesAction {
    Updated(Drawing),
    ExportSvg,
    ExportPdf,
    BackToModel,
}

pub fn draw_drawing_properties(
    ui: &mut Ui,
    drawing: &Drawing,
    layers: &LayerManager,
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

            // Position on sheet
            ui.collapsing("Position & Size", |ui| {
                ui.horizontal(|ui| {
                    ui.label("X:");
                    if ui.add(egui::DragValue::new(&mut vp.sheet_position[0]).speed(1.0).suffix(" mm")).changed() {
                        changed = true;
                    }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut vp.sheet_position[1]).speed(1.0).suffix(" mm")).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("W:");
                    if ui.add(egui::DragValue::new(&mut vp.sheet_size[0]).speed(1.0).suffix(" mm").range(10.0..=2000.0)).changed() {
                        changed = true;
                    }
                    ui.label("H:");
                    if ui.add(egui::DragValue::new(&mut vp.sheet_size[1]).speed(1.0).suffix(" mm").range(10.0..=2000.0)).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Center X:");
                    if ui.add(egui::DragValue::new(&mut vp.model_center[0]).speed(10.0).suffix(" mm")).changed() {
                        changed = true;
                    }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut vp.model_center[1]).speed(10.0).suffix(" mm")).changed() {
                        changed = true;
                    }
                });
                if ui.button("Fit to margins").on_hover_text("Resize viewport to fill printable area").clicked() {
                    let (pw, ph) = paper_dims;
                    let [mt, mr, mb, ml] = margins_copy;
                    vp.sheet_position = [ml, mt];
                    vp.sheet_size = [pw - ml - mr, ph - mt - mb];
                    changed = true;
                }
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
                        if override_vis.is_some() {
                            if ui.small_button("reset").clicked() {
                                vp.layer_overrides.visibility.remove(&layer.id);
                                changed = true;
                            }
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
        action = Some(DrawingPropertiesAction::Updated(drawing));
    }

    action
}
