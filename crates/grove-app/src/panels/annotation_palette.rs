use egui::Ui;
use grove_core::drawing::drawing::Drawing;

/// Actions returned by the annotation palette for the app to handle.
pub enum AnnotationPaletteAction {
    Updated(Drawing),
    StartAnnotationMode,
    SetLeader(uuid::Uuid),
}

/// Draw the annotation palette panel (shown on the left in Drawing mode).
/// Returns an action if the user modified annotations or triggered a mode.
pub fn draw_annotation_palette(
    ui: &mut Ui,
    drawing: &Drawing,
) -> Option<AnnotationPaletteAction> {
    let mut action = None;
    let mut drawing = drawing.clone();
    let mut changed = false;

    ui.heading("Annotations");
    ui.separator();

    if ui.button("Add Label").on_hover_text("Click on the paper to place a label").clicked() {
        action = Some(AnnotationPaletteAction::StartAnnotationMode);
    }

    ui.add_space(4.0);

    let mut remove_ann: Option<usize> = None;
    for (i, ann) in drawing.annotations.iter_mut().enumerate() {
        ui.push_id(format!("ann_{}", i), |ui| {
            ui.horizontal(|ui| {
                ui.label("Text:");
                if ui.text_edit_singleline(&mut ann.text).changed() {
                    changed = true;
                }
                if ui.small_button("X").on_hover_text("Remove annotation").clicked() {
                    remove_ann = Some(i);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Size:");
                if ui.add(egui::DragValue::new(&mut ann.font_size).speed(0.1).range(1.0..=20.0).suffix(" mm")).changed() {
                    changed = true;
                }
                if ui.checkbox(&mut ann.bordered, "Border").changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Pos:");
                if ui.add(egui::DragValue::new(&mut ann.sheet_position[0]).speed(1.0).suffix(" mm")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut ann.sheet_position[1]).speed(1.0).suffix(" mm")).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                if ann.leader_target_model.is_some() {
                    if ui.small_button("Clear Leader").clicked() {
                        ann.leader_target_model = None;
                        ann.viewport_id = None;
                        changed = true;
                    }
                } else {
                    if ui.small_button("Set Leader").clicked() {
                        action = Some(AnnotationPaletteAction::SetLeader(ann.id));
                    }
                }
            });
        });
        ui.separator();
    }

    if let Some(idx) = remove_ann {
        drawing.annotations.remove(idx);
        changed = true;
    }

    if changed && action.is_none() {
        action = Some(AnnotationPaletteAction::Updated(drawing));
    }

    action
}
