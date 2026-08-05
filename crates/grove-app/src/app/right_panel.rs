//! Right side panel: entity properties, layers, background images, drawing manager.

use super::*;

impl GroveApp {
    pub(super) fn show_right_panel(&mut self, ui: &mut egui::Ui) {
        // Right panel
        let mut props_change: Option<(uuid::Uuid, grove_core::entity::DomainData)> = None;
        let mut drawing_action: Option<crate::panels::drawing_properties::DrawingPropertiesAction> = None;
        let mut drawing_mgr_action: Option<crate::panels::drawing_manager::DrawingManagerAction> = None;
        Panel::right("properties_panel")
            .default_size(220.0)
            .size_range(180.0..=400.0)
            .show(ui, |ui| {
                if let WorkspaceMode::Drawing(drawing_id) = self.workspace_mode {
                    // Drawing mode: show drawing properties
                    if let Some(drawing) = self.project.find_drawing(drawing_id) {
                        drawing_action = crate::panels::drawing_properties::draw_drawing_properties(
                            ui, drawing, &self.project.layers, &self.project.entities,
                        );
                    } else {
                        ui.label("Drawing not found.");
                        if ui.button("Back to Model").clicked() {
                            self.workspace_mode = WorkspaceMode::Model;
                        }
                    }
                } else {
                // Model mode: show entity properties + layers + drawing manager
                if self.selection.ids.len() > 1 {
                    crate::panels::properties::draw_multi_selection(ui, self.selection.ids.len());
                } else if self.selection.single().is_some() {
                    props_change = crate::panels::properties::draw_properties_panel(
                        ui, &self.project, &self.selection,
                    );
                } else {
                    crate::panels::properties::draw_no_selection(ui);
                }

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.heading("Layers");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+").on_hover_text("Add layer").clicked() {
                            let n = self.project.layers.layers.len();
                            let mut layer = grove_core::layer::Layer::new(
                                format!("Layer {}", n + 1),
                                [200, 200, 200, 255],
                            );
                            layer.order = n as i32;
                            self.project.layers.add_layer(layer);
                            self.project.dirty = true;
                        }
                    });
                });
                ui.separator();

                let active_id = self.project.layers.active_layer_id;
                let layer_count = self.project.layers.layers.len();
                let mut delete_idx: Option<usize> = None;
                let mut move_up_idx: Option<usize> = None;
                let mut move_down_idx: Option<usize> = None;

                for i in 0..layer_count {
                    let layer_id = self.project.layers.layers[i].id;
                    let layer_colour = self.project.layers.layers[i].colour;
                    let layer_visible = self.project.layers.layers[i].visible;
                    let layer_name = self.project.layers.layers[i].name.clone();
                    let is_active = layer_id == active_id;

                    ui.horizontal(|ui| {
                        let colour = Color32::from_rgba_premultiplied(
                            layer_colour[0],
                            layer_colour[1],
                            layer_colour[2],
                            layer_colour[3],
                        );
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, colour);

                        let vis_label = if layer_visible { "V" } else { "." };
                        if ui.small_button(vis_label).clicked() {
                            self.project.layers.layers[i].visible = !layer_visible;
                        }

                        // Rename mode
                        if let Some((rename_idx, ref mut buf)) = self.layer_rename {
                            if rename_idx == i {
                                let response = ui.text_edit_singleline(buf);
                                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let new_name = buf.clone();
                                    self.project.layers.layers[i].name = new_name;
                                    self.project.dirty = true;
                                    self.layer_rename = None;
                                }
                                return;
                            }
                        }

                        let label = if is_active {
                            egui::RichText::new(&layer_name).color(ThemeColours::TEXT_PRIMARY)
                        } else {
                            egui::RichText::new(&layer_name).color(ThemeColours::TEXT_SECONDARY)
                        };
                        let response = ui.selectable_label(is_active, label);
                        if response.clicked() {
                            self.project.layers.set_active(layer_id);
                        }
                        if response.double_clicked() {
                            self.layer_rename = Some((i, layer_name.clone()));
                        }
                        response.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                self.layer_rename = Some((i, layer_name.clone()));
                                ui.close();
                            }
                            if i > 0 && ui.button("Move Up").clicked() {
                                move_up_idx = Some(i);
                                ui.close();
                            }
                            if i < layer_count - 1 && ui.button("Move Down").clicked() {
                                move_down_idx = Some(i);
                                ui.close();
                            }
                            if layer_count > 1 {
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    delete_idx = Some(i);
                                    ui.close();
                                }
                            }
                        });
                    });
                }

                // Apply deferred mutations
                if let Some(i) = move_up_idx {
                    self.project.layers.layers.swap(i, i - 1);
                    self.project.dirty = true;
                }
                if let Some(i) = move_down_idx {
                    self.project.layers.layers.swap(i, i + 1);
                    self.project.dirty = true;
                }
                if let Some(i) = delete_idx {
                    let removed_id = self.project.layers.layers[i].id;
                    self.project.layers.layers.remove(i);
                    if self.project.layers.active_layer_id == removed_id {
                        self.project.layers.active_layer_id = self.project.layers.layers[0].id;
                    }
                    self.project.dirty = true;
                }

                // Background Images section
                if !self.project.background_images.is_empty() {
                    ui.add_space(16.0);
                    ui.heading("Background");
                    ui.separator();

                    let mut remove_id: Option<uuid::Uuid> = None;
                    let mut start_adjust: Option<(uuid::Uuid, AdjustMode)> = None;

                    for img in &mut self.project.background_images {
                        ui.label(&img.name);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut img.visible, "Visible");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Opacity:");
                            ui.add(egui::Slider::new(&mut img.opacity, 0.0..=1.0).max_decimals(2));
                        });

                        let mode_label = match &img.colour_mode {
                            grove_core::background_image::ColourMode::Normal => "Normal",
                            grove_core::background_image::ColourMode::Inverted => "Inverted",
                            grove_core::background_image::ColourMode::Desaturated => "Desaturated",
                            grove_core::background_image::ColourMode::Tinted(_) => "Tinted",
                        };
                        egui::ComboBox::from_id_salt(format!("bg_colour_{}", img.id))
                            .selected_text(mode_label)
                            .show_ui(ui, |ui| {
                                let is_normal = matches!(img.colour_mode, grove_core::background_image::ColourMode::Normal);
                                if ui.selectable_label(is_normal, "Normal").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Normal;
                                }
                                let is_inv = matches!(img.colour_mode, grove_core::background_image::ColourMode::Inverted);
                                if ui.selectable_label(is_inv, "Inverted").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Inverted;
                                }
                                let is_desat = matches!(img.colour_mode, grove_core::background_image::ColourMode::Desaturated);
                                if ui.selectable_label(is_desat, "Desaturated").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Desaturated;
                                }
                                let is_tinted = matches!(img.colour_mode, grove_core::background_image::ColourMode::Tinted(_));
                                if ui.selectable_label(is_tinted, "Tinted").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Tinted([180, 160, 120]);
                                }
                            });

                        if img.rotation_rad.abs() > 1e-6 {
                            ui.colored_label(
                                ThemeColours::TEXT_DIM,
                                format!("Rotation: {:.1}\u{00B0}", img.rotation_rad.to_degrees()),
                            );
                        }

                        ui.horizontal(|ui| {
                            if ui.button("Calibrate").clicked() {
                                start_adjust = Some((img.id, AdjustMode::CalibrateScale {
                                    first_point: None,
                                    awaiting_distance: false,
                                    distance_input: String::new(),
                                }));
                            }
                            if ui.button("Align").on_hover_text("Draw a line that should be horizontal").clicked() {
                                start_adjust = Some((img.id, AdjustMode::AlignHorizontal {
                                    first_point: None,
                                }));
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Flip H").on_hover_text("Flip horizontally").clicked() {
                                img.flip_h = !img.flip_h;
                            }
                            if ui.button("Flip V").on_hover_text("Flip vertically").clicked() {
                                img.flip_v = !img.flip_v;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Set Origin").on_hover_text("Click the point that should be (0,0)").clicked() {
                                start_adjust = Some((img.id, AdjustMode::SetOrigin));
                            }
                            if ui.button("Remove").clicked() {
                                remove_id = Some(img.id);
                            }
                        });
                        ui.separator();
                    }

                    // Adjustment mode status & distance input
                    if let Some(ref mut cal) = self.calibrate_state {
                        match &mut cal.mode {
                            AdjustMode::CalibrateScale { first_point, awaiting_distance, distance_input } => {
                                if *awaiting_distance {
                                    ui.add_space(4.0);
                                    ui.colored_label(ThemeColours::ACCENT, "Enter real distance (mm):");
                                    let response = ui.text_edit_singleline(distance_input);
                                    if response.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        if let Ok(real_mm) = distance_input.trim().parse::<f32>() {
                                            if real_mm > 0.0 {
                                                let img_id = cal.image_id;
                                                let p1 = first_point.unwrap();
                                                let p2 = self.calibrate_second_point.unwrap();
                                                if let Some(img) =
                                                    self.project.find_background_image_mut(img_id)
                                                {
                                                    let world_dist = ((p2.x - p1.x).powi(2)
                                                        + (p2.y - p1.y).powi(2))
                                                    .sqrt();
                                                    let pixel_dist = world_dist * img.pixels_per_mm;
                                                    img.pixels_per_mm = pixel_dist / real_mm;
                                                    self.project.dirty = true;
                                                }
                                            }
                                        }
                                        self.calibrate_state = None;
                                        self.calibrate_second_point = None;
                                    }
                                } else {
                                    let msg = if first_point.is_none() {
                                        "Click first calibration point"
                                    } else {
                                        "Click second calibration point"
                                    };
                                    ui.colored_label(ThemeColours::ACCENT, msg);
                                    if ui.button("Cancel").clicked() {
                                        self.calibrate_state = None;
                                    }
                                }
                            }
                            AdjustMode::AlignHorizontal { first_point } => {
                                let msg = if first_point.is_none() {
                                    "Click first point of horizontal line"
                                } else {
                                    "Click second point of horizontal line"
                                };
                                ui.colored_label(ThemeColours::ACCENT, msg);
                                if ui.button("Cancel").clicked() {
                                    self.calibrate_state = None;
                                }
                            }
                            AdjustMode::SetOrigin => {
                                ui.colored_label(ThemeColours::ACCENT, "Click the point that should be (0,0)");
                                if ui.button("Cancel").clicked() {
                                    self.calibrate_state = None;
                                }
                            }
                        }
                    }

                    if let Some((id, mode)) = start_adjust {
                        self.calibrate_state = Some(CalibrateState {
                            image_id: id,
                            mode,
                        });
                        self.calibrate_second_point = None;
                    }
                    if let Some(id) = remove_id {
                        self.project.remove_background_image(id);
                        self.bg_renderer.remove(id);
                        if self
                            .calibrate_state
                            .as_ref()
                            .is_some_and(|c| c.image_id == id)
                        {
                            self.calibrate_state = None;
                        }
                    }
                }

                // Drawing Manager section
                ui.add_space(16.0);
                drawing_mgr_action = crate::panels::drawing_manager::draw_drawing_manager(
                    ui, &self.project,
                );
                } // close else (model mode)
            });

        // Handle drawing manager actions
        if let Some(mgr_action) = drawing_mgr_action {
            match mgr_action {
                crate::panels::drawing_manager::DrawingManagerAction::OpenDrawing(id) => {
                    self.workspace_mode = WorkspaceMode::Drawing(id);
                }
                crate::panels::drawing_manager::DrawingManagerAction::CreateFromTemplate(template) => {
                    let number = self.project.next_drawing_number();
                    let drawing = grove_core::drawing::templates::create_from_template(
                        template, &number, &self.project.layers,
                    );
                    let id = drawing.id;
                    let cmd = grove_core::commands::AddDrawingCommand::new(drawing);
                    self.history.execute(Box::new(cmd), &mut self.project);
                    self.workspace_mode = WorkspaceMode::Drawing(id);
                }
                crate::panels::drawing_manager::DrawingManagerAction::RemoveDrawing(id) => {
                    let cmd = grove_core::commands::RemoveDrawingCommand::new(id);
                    self.history.execute(Box::new(cmd), &mut self.project);
                    if self.workspace_mode == WorkspaceMode::Drawing(id) {
                        self.workspace_mode = WorkspaceMode::Model;
                    }
                }
            }
        }

        // Handle drawing properties actions
        if let Some(dp_action) = drawing_action {
            match dp_action {
                crate::panels::drawing_properties::DrawingPropertiesAction::Updated(new_drawing) => {
                    let new_drawing = *new_drawing;
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(old) = self.project.find_drawing(id) {
                            let old_clone = old.clone();
                            let cmd = grove_core::commands::UpdateDrawingCommand::new(id, old_clone, new_drawing);
                            self.history.execute(Box::new(cmd), &mut self.project);
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::ExportSvg => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(id) {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export Drawing SVG")
                                .set_file_name(format!("{}-{}.svg", drawing.full_sheet_ref(), drawing.title))
                                .add_filter("SVG", &["svg"])
                                .save_file()
                                .map(|p| ensure_extension(p, "svg"))
                            {
                                let options = grove_core::export::svg::SvgExportOptions::default();
                                if let Err(e) = grove_core::export::svg::export_drawing_svg_to_file(
                                    &self.project, drawing, &options, &path,
                                ) {
                                    log::error!("Drawing SVG export failed: {e}");
                                }
                            }
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::ExportPdf => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(id) {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export Drawing PDF")
                                .set_file_name(format!("{}-{}.pdf", drawing.full_sheet_ref(), drawing.title))
                                .add_filter("PDF", &["pdf"])
                                .save_file()
                                .map(|p| ensure_extension(p, "pdf"))
                            {
                                if let Err(e) = grove_core::export::pdf::export_drawing_pdf(
                                    &self.project, drawing, &path,
                                ) {
                                    log::error!("Drawing PDF export failed: {e}");
                                }
                            }
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::BackToModel => {
                    self.workspace_mode = WorkspaceMode::Model;
                }
            }
        }

        // Apply property edits from the properties panel
        if let Some((id, new_domain)) = props_change {
            if let Some(entity) = self.project.find_entity(id) {
                let old_domain = entity.domain.clone();
                let cmd = grove_core::commands::UpdateDomainDataCommand::new(id, old_domain, new_domain);
                self.history.execute(Box::new(cmd), &mut self.project);
            }
        }
    }
}
