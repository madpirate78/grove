//! Left contextual palettes: plant, hardscape, level, entourage, annotation.

use super::*;

impl GroveApp {
    pub(super) fn show_left_palettes(&mut self, ui: &mut egui::Ui) {
        // Plant palette (shown when Plant or Group tool active, model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && (self.active_tool_type == ActiveToolType::PlacePlant
                || self.active_tool_type == ActiveToolType::PlaceGroup)
        {
            Panel::left("plant_palette")
                .default_size(260.0)
                .size_range(200.0..=400.0)
                .show(ui, |ui| {
                    if let Some(plant) = crate::panels::palette::draw_palette(
                        ui,
                        &self.plant_db,
                        &mut self.palette_state,
                    ) {
                        self.selected_plant = Some(plant.clone());
                        // Update the active tool with the new plant
                        if let Some(ref mut tool) = self.draw_tool {
                            if let Some(pt) = tool.as_any_mut().downcast_mut::<crate::tools::place_plant::PlacePlantTool>() {
                                pt.set_plant(plant.clone());
                            }
                            if let Some(gt) = tool.as_any_mut().downcast_mut::<crate::tools::place_group::PlaceGroupTool>() {
                                gt.set_plant(plant);
                            }
                        }
                    }
                });
        }

        // Hardscape palette (model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && self.active_tool_type == ActiveToolType::PlaceHardscape
        {
            Panel::left("hardscape_palette")
                .default_size(240.0)
                .size_range(200.0..=360.0)
                .show(ui, |ui| {
                    if let Some(ref mut tool) = self.draw_tool {
                        if let Some(ht) = tool.as_any_mut().downcast_mut::<crate::tools::place_hardscape::PlaceHardscapeTool>() {
                            if crate::panels::hardscape_palette::draw_hardscape_palette(ui, ht) {
                                if let Some(layer) = self.project.layers.find_by_name(ht.target_layer_name()) {
                                    ht.layer_id = layer.id;
                                }
                            }
                        }
                    }
                });
        }

        // Level palette (model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && self.active_tool_type == ActiveToolType::PlaceLevel
        {
            Panel::left("level_palette")
                .default_size(240.0)
                .size_range(200.0..=360.0)
                .show(ui, |ui| {
                    if let Some(ref mut tool) = self.draw_tool {
                        if let Some(lt) = tool.as_any_mut().downcast_mut::<crate::tools::place_level::PlaceLevelTool>() {
                            if crate::panels::level_palette::draw_level_palette(ui, lt) {
                                if let Some(layer) = self.project.layers.find_by_name(lt.target_layer_name()) {
                                    lt.layer_id = layer.id;
                                }
                            }
                        }
                    }
                });
        }

        // Entourage palette (model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && self.active_tool_type == ActiveToolType::PlaceEntourage
        {
            Panel::left("entourage_palette")
                .default_size(240.0)
                .size_range(200.0..=360.0)
                .show(ui, |ui| {
                    if let Some(ref mut tool) = self.draw_tool {
                        if let Some(et) = tool.as_any_mut().downcast_mut::<crate::tools::place_entourage::PlaceEntourageTool>() {
                            crate::panels::entourage_palette::draw_entourage_palette(
                                ui, et, &self.entourage_library, &mut self.entourage_palette_state,
                            );
                        }
                    }
                });
        }

        // Left annotation palette (shown in Drawing mode, replacing the model-space palettes)
        let mut annotation_action: Option<crate::panels::annotation_palette::AnnotationPaletteAction> = None;
        if let WorkspaceMode::Drawing(drawing_id) = self.workspace_mode {
            Panel::left("annotation_palette")
                .default_size(240.0)
                .size_range(200.0..=360.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(drawing) = self.project.find_drawing(drawing_id) {
                            annotation_action = crate::panels::annotation_palette::draw_annotation_palette(ui, drawing);
                        }
                    });
                });
        }

        // Handle annotation palette actions
        if let Some(ann_action) = annotation_action {
            match ann_action {
                crate::panels::annotation_palette::AnnotationPaletteAction::Updated(new_drawing) => {
                    let new_drawing = *new_drawing;
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(old) = self.project.find_drawing(id) {
                            let old_clone = old.clone();
                            let cmd = grove_core::commands::UpdateDrawingCommand::new(id, old_clone, new_drawing);
                            self.history.execute(Box::new(cmd), &mut self.project);
                        }
                    }
                }
                crate::panels::annotation_palette::AnnotationPaletteAction::StartAnnotationMode => {
                    self.drawing_interaction.annotation_mode = true;
                }
                crate::panels::annotation_palette::AnnotationPaletteAction::SetLeader(ann_id) => {
                    self.drawing_interaction.setting_leader = Some(ann_id);
                }
            }
        }
    }
}
