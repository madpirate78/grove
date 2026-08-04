//! File operations: new/save/open, recent-files list, window title, auto-save.

use super::*;

impl GroveApp {
    pub(super) fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "Grove").map(|d| d.config_dir().to_path_buf())
    }

    pub(super) fn load_recent_files() -> Vec<PathBuf> {
        let Some(dir) = Self::config_dir() else {
            return Vec::new();
        };
        let path = dir.join("recent.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
            .unwrap_or_default()
    }

    pub(super) fn save_recent_files(&self) {
        let Some(dir) = Self::config_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let json = serde_json::to_string_pretty(&self.recent_files).unwrap_or_default();
        let _ = std::fs::write(dir.join("recent.json"), json);
    }

    pub(super) fn add_to_recent(&mut self, path: &PathBuf) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(10);
        self.save_recent_files();
    }

    pub(super) fn new_project(&mut self) {
        self.project = Project::new();
        self.history.clear();
        self.selection.clear();
        self.file_path = None;
        self.draw_tool = None;
        self.active_tool_type = ActiveToolType::Select;
        self.bg_renderer.clear();
        self.calibrate_state = None;
    }

    pub(super) fn save(&mut self) -> Result<(), String> {
        if let Some(ref path) = self.file_path.clone() {
            storage::save_project(&self.project, path)?;
            self.project.dirty = false;
            Ok(())
        } else {
            self.save_as()
        }
    }

    pub(super) fn save_as(&mut self) -> Result<(), String> {
        let dialog = rfd::FileDialog::new()
            .set_title("Save Grove Project")
            .add_filter("Grove Project", &["grove"])
            .set_file_name("untitled.grove");
        if let Some(path) = dialog.save_file() {
            storage::save_project(&self.project, &path)?;
            self.project.dirty = false;
            self.add_to_recent(&path);
            self.file_path = Some(path);
            Ok(())
        } else {
            Ok(()) // User cancelled
        }
    }

    pub(super) fn open_file(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_title("Open Grove Project")
            .add_filter("Grove Project", &["grove"]);
        if let Some(path) = dialog.pick_file() {
            self.open_path(path);
        }
    }

    pub(super) fn open_path(&mut self, path: PathBuf) {
        match storage::load_project(&path) {
            Ok(project) => {
                self.project = project;
                self.history.clear();
                self.selection.clear();
                self.draw_tool = None;
                self.active_tool_type = ActiveToolType::Select;
                self.bg_renderer.clear();
                self.calibrate_state = None;
                self.add_to_recent(&path);
                self.file_path = Some(path);
                log::info!("Project opened successfully");
            }
            Err(e) => {
                log::error!("Failed to open project: {e}");
            }
        }
    }

    pub(super) fn update_title(&self, ctx: &Context) {
        let name = self.file_path.as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.project.metadata.name.clone());
        let dirty = if self.project.dirty { " *" } else { "" };
        let title = format!("Grove — {name}{dirty}");
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Auto-save every 60 seconds if dirty and a file path exists.
    pub(super) fn tick_autosave(&mut self, ctx: &Context) {
        if self.project.dirty && self.file_path.is_some() {
            self.auto_save_elapsed += ctx.input(|i| i.stable_dt);
            if self.auto_save_elapsed >= 60.0 {
                self.auto_save_elapsed = 0.0;
                if let Err(e) = self.save() {
                    log::error!("Auto-save failed: {e}");
                }
            }
        } else {
            self.auto_save_elapsed = 0.0;
        }
    }
}
