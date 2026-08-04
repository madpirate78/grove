//! Top menu bar: File / Edit / View / Draw / Garden / Export / Help + workspace toggle.

use super::*;

impl GroveApp {
    pub(super) fn show_menu_bar(&mut self, ctx: &Context) {
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project  Ctrl+N").clicked() {
                        self.new_project();
                        ui.close_menu();
                    }
                    if ui.button("Open…  Ctrl+O").clicked() {
                        ui.close_menu();
                        self.open_file();
                    }
                    ui.separator();
                    if ui.button("Save  Ctrl+S").clicked() {
                        ui.close_menu();
                        if let Err(e) = self.save() {
                            log::error!("Save failed: {e}");
                        }
                    }
                    if ui.button("Save As…  Ctrl+Shift+S").clicked() {
                        ui.close_menu();
                        if let Err(e) = self.save_as() {
                            log::error!("Save As failed: {e}");
                        }
                    }
                    if !self.recent_files.is_empty() {
                        ui.separator();
                        ui.menu_button("Recent Projects", |ui| {
                            let recents = self.recent_files.clone();
                            for path in &recents {
                                let label = path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| path.display().to_string());
                                if ui.button(&label).on_hover_text(path.display().to_string()).clicked() {
                                    let p = path.clone();
                                    ui.close_menu();
                                    self.open_path(p);
                                }
                            }
                        });
                    }
                    ui.separator();
                    if ui.button("Import Background Image…").clicked() {
                        ui.close_menu();
                        let dialog = rfd::FileDialog::new()
                            .set_title("Import Background Image")
                            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]);
                        if let Some(path) = dialog.pick_file() {
                            match std::fs::read(&path) {
                                Ok(data) => {
                                    let name = path.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "image".into());
                                    match grove_core::background_image::BackgroundImage::from_bytes(name, data) {
                                        Ok(img) => {
                                            self.project.add_background_image(img);
                                        }
                                        Err(e) => log::error!("Failed to import image: {e}"),
                                    }
                                }
                                Err(e) => log::error!("Failed to read file: {e}"),
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let undo_label = match self.history.undo_description() {
                        Some(desc) => format!("Undo: {desc}"),
                        None => "Undo".to_string(),
                    };
                    if ui
                        .add_enabled(self.history.can_undo(), egui::Button::new(undo_label))
                        .clicked()
                    {
                        self.history.undo(&mut self.project);
                        ui.close_menu();
                    }
                    let redo_label = match self.history.redo_description() {
                        Some(desc) => format!("Redo: {desc}"),
                        None => "Redo".to_string(),
                    };
                    if ui
                        .add_enabled(self.history.can_redo(), egui::Button::new(redo_label))
                        .clicked()
                    {
                        self.history.redo(&mut self.project);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Select All").clicked() {
                        for e in &self.project.entities {
                            self.selection.add(e.id);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let has_selection = !self.selection.is_empty();
                    let has_clipboard = !self.clipboard.is_empty();
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Copy  Ctrl+C"))
                        .clicked()
                    {
                        self.copy_selection_to_clipboard();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_clipboard, egui::Button::new("Paste  Ctrl+V"))
                        .clicked()
                    {
                        self.paste_from_clipboard();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Cut  Ctrl+X"))
                        .clicked()
                    {
                        self.copy_selection_to_clipboard();
                        let ids: Vec<_> = self.selection.ids.clone();
                        if !ids.is_empty() {
                            let commands: Vec<Box<dyn grove_core::commands::Command>> = ids
                                .iter()
                                .map(|&id| -> Box<dyn grove_core::commands::Command> {
                                    Box::new(grove_core::commands::RemoveEntityCommand::new(id))
                                })
                                .collect();
                            let cmd = Box::new(grove_core::commands::CompoundCommand::new(
                                commands,
                                "Cut entities",
                            ));
                            self.history.execute(cmd, &mut self.project);
                        }
                        self.selection.clear();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Duplicate  Ctrl+D"))
                        .clicked()
                    {
                        self.duplicate_selection();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom to Fit").clicked() {
                        let mut combined: Option<BoundingBox> = None;
                        for e in &self.project.entities {
                            if let Some(bb) = e.bounding_box() {
                                combined = Some(match combined {
                                    Some(c) => c.union(&bb),
                                    None => bb,
                                });
                            }
                        }
                        if let Some(bb) = combined {
                            let expanded = bb.expand(200.0);
                            self.canvas.viewport.fit_to_rect(
                                expanded.min,
                                expanded.max,
                                egui::vec2(800.0, 600.0),
                                40.0,
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Reset View").clicked() {
                        self.canvas.viewport.offset = glam::Vec2::new(400.0, 300.0);
                        self.canvas.viewport.zoom = 0.5;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.snap_config.enabled, "Snapping");
                    ui.checkbox(&mut self.snap_config.grid_enabled, "Grid Snap");
                    ui.checkbox(&mut self.snap_config.endpoint_enabled, "Endpoint Snap");
                    ui.checkbox(&mut self.snap_config.midpoint_enabled, "Midpoint Snap");
                });
                ui.menu_button("Draw", |ui| {
                    if ui.button("Line").clicked() {
                        self.switch_tool(ActiveToolType::DrawLine);
                        ui.close_menu();
                    }
                    if ui.button("Rectangle").clicked() {
                        self.switch_tool(ActiveToolType::DrawRect);
                        ui.close_menu();
                    }
                    if ui.button("Arc").clicked() {
                        self.switch_tool(ActiveToolType::DrawArc);
                        ui.close_menu();
                    }
                    if ui.button("Circle").clicked() {
                        self.switch_tool(ActiveToolType::DrawCircle);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Text").clicked() {
                        self.switch_tool(ActiveToolType::Text);
                        ui.close_menu();
                    }
                    if ui.button("Dimension").clicked() {
                        self.switch_tool(ActiveToolType::Dimension);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Garden", |ui| {
                    if ui.button("Place Plant").clicked() {
                        self.switch_tool(ActiveToolType::PlacePlant);
                        ui.close_menu();
                    }
                    if ui.button("Place Group").clicked() {
                        self.switch_tool(ActiveToolType::PlaceGroup);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Place Hardscape").clicked() {
                        self.switch_tool(ActiveToolType::PlaceHardscape);
                        ui.close_menu();
                    }
                    if ui.button("Place Level").clicked() {
                        self.switch_tool(ActiveToolType::PlaceLevel);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Cost Estimate…").clicked() {
                        self.estimate_window_open = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.colored_label(
                        ThemeColours::TEXT_DIM,
                        format!("Database: {} plants", self.plant_db.plant_count()),
                    );
                });
                ui.menu_button("Export", |ui| {
                    if ui.button("Export SVG...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export SVG")
                            .add_filter("SVG", &["svg"])
                            .save_file()
                            .map(|p| ensure_extension(p, "svg"))
                        {
                            let options = grove_core::export::svg::SvgExportOptions::default();
                            if let Err(e) = grove_core::export::svg::export_svg_to_file(
                                &self.project, &options, &path,
                            ) {
                                log::error!("SVG export failed: {e}");
                            }
                        }
                    }
                    if ui.button("Export Planting Schedule CSV...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export Planting Schedule")
                            .add_filter("CSV", &["csv"])
                            .save_file()
                            .map(|p| ensure_extension(p, "csv"))
                        {
                            if let Err(e) = grove_core::export::csv_export::export_planting_csv(
                                &self.project.entities, &path,
                            ) {
                                log::error!("CSV export failed: {e}");
                            }
                        }
                    }
                    if ui.button("Export Cost Estimate CSV...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export Cost Estimate")
                            .add_filter("CSV", &["csv"])
                            .save_file()
                            .map(|p| ensure_extension(p, "csv"))
                        {
                            if let Err(e) = grove_core::export::csv_export::export_estimate_csv(
                                &self.project.entities, &self.unit_costs, &path,
                            ) {
                                log::error!("Estimate export failed: {e}");
                            }
                        }
                    }
                    // Drawing SVG export (available when a drawing is open)
                    if let WorkspaceMode::Drawing(drawing_id) = &self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(*drawing_id) {
                            let label = format!("Export Drawing SVG ({})...", drawing.title);
                            if ui.button(label).clicked() {
                                ui.close_menu();
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
                            // Drawing PDF export
                            let pdf_label = format!("Export Drawing PDF ({})...", drawing.title);
                            if ui.button(pdf_label).clicked() {
                                ui.close_menu();
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
                    // Export All Drawings
                    if !self.project.drawings.is_empty() {
                        let label = format!("Export All Drawings ({})...", self.project.drawings.len());
                        if ui.button(label).clicked() {
                            ui.close_menu();
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Export All Drawings — Choose Folder")
                                .pick_folder()
                            {
                                let options = grove_core::export::svg::SvgExportOptions::default();
                                match grove_core::export::svg::export_all_drawings_svg(
                                    &self.project, &options, &dir,
                                ) {
                                    Ok(count) => log::info!("Exported {count} drawings to {}", dir.display()),
                                    Err(e) => log::error!("Batch drawing export failed: {e}"),
                                }
                            }
                        }
                    }
                    if ui.button("Export JSON...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export JSON")
                            .add_filter("JSON", &["json"])
                            .save_file()
                            .map(|p| ensure_extension(p, "json"))
                        {
                            if let Err(e) = grove_core::export::json::export_json_to_file(
                                &self.project, &path,
                            ) {
                                log::error!("JSON export failed: {e}");
                            }
                        }
                    }
                });
                // Workspace mode toggle
                ui.separator();
                let in_model = self.workspace_mode == WorkspaceMode::Model;
                if ui.selectable_label(in_model, "Model").clicked() {
                    self.workspace_mode = WorkspaceMode::Model;
                }
                let in_drawing = matches!(self.workspace_mode, WorkspaceMode::Drawing(_));
                if ui.selectable_label(in_drawing, "Drawing").on_hover_text("Switch to drawing mode").clicked() {
                    // If no drawing is open, open the first one (or stay in model)
                    if let Some(d) = self.project.drawings.first() {
                        self.workspace_mode = WorkspaceMode::Drawing(d.id);
                    }
                }

                ui.menu_button("Help", |ui| {
                    if ui.button("About Grove").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });
    }
}
