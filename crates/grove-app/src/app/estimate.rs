//! Cost-estimate window: live BOQ display, rate editing, profile import/export.

use grove_core::garden::estimate::UnitCosts;

use super::*;

impl GroveApp {
    /// Load the user's saved rates from the config dir, or seed defaults.
    pub(super) fn load_unit_costs() -> UnitCosts {
        Self::config_dir()
            .map(|d| d.join("unit_costs.json"))
            .filter(|p| p.exists())
            .and_then(|p| UnitCosts::load_from_file(&p).ok())
            .unwrap_or_default()
    }

    fn save_unit_costs(&self) {
        let Some(dir) = Self::config_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = self.unit_costs.save_to_file(&dir.join("unit_costs.json")) {
            log::error!("Failed to save unit costs: {e}");
        }
    }

    pub(super) fn show_estimate_window(&mut self, ctx: &Context) {
        if !self.estimate_window_open {
            return;
        }
        let (changed, action) = crate::panels::estimate_panel::draw_estimate_window(
            ctx,
            &mut self.estimate_window_open,
            &mut self.unit_costs,
            &self.project.entities,
        );
        if changed {
            self.save_unit_costs();
        }

        match action {
            Some(crate::panels::estimate_panel::EstimateAction::ExportCsv) => {
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
            Some(crate::panels::estimate_panel::EstimateAction::ExportProfile) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export Cost Profile")
                    .set_file_name("grove-rates.json")
                    .add_filter("JSON", &["json"])
                    .save_file()
                    .map(|p| ensure_extension(p, "json"))
                {
                    if let Err(e) = self.unit_costs.save_to_file(&path) {
                        log::error!("Profile export failed: {e}");
                    }
                }
            }
            Some(crate::panels::estimate_panel::EstimateAction::ImportProfile) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Import Cost Profile")
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    match UnitCosts::load_from_file(&path) {
                        Ok(costs) => {
                            self.unit_costs = costs;
                            self.save_unit_costs();
                        }
                        Err(e) => log::error!("Profile import failed: {e}"),
                    }
                }
            }
            None => {}
        }
    }
}
