use egui::{Context, RichText};
use grove_core::entity::Entity;
use grove_core::garden::estimate::{boq_total, generate_boq, BoqUnit, UnitCosts};

use crate::theme::ThemeColours;

/// Actions the estimate window asks the app to perform (file dialogs live
/// app-side, matching the other panels).
pub enum EstimateAction {
    ExportCsv,
    ExportProfile,
    ImportProfile,
}

/// Draw the floating cost-estimate window: live bill of quantities on top,
/// editable unit rates below. Returns `(rates_changed, action)`.
pub fn draw_estimate_window(
    ctx: &Context,
    open: &mut bool,
    costs: &mut UnitCosts,
    entities: &[Entity],
) -> (bool, Option<EstimateAction>) {
    let mut changed = false;
    let mut action = None;

    egui::Window::new("Cost Estimate")
        .open(open)
        .default_width(430.0)
        .default_height(520.0)
        .resizable(true)
        .show(ctx, |ui| {
            let boq = generate_boq(entities, costs);
            let total = boq_total(&boq);

            ui.horizontal(|ui| {
                ui.heading(format!("Total: {}{:.2}", costs.currency, total));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(ThemeColours::TEXT_DIM, "rough guide, not a quote");
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // --- Bill of quantities ---
                if boq.is_empty() {
                    ui.colored_label(ThemeColours::TEXT_DIM, "Nothing priceable in the project yet.");
                } else {
                    let mut current_group = "";
                    egui::Grid::new("boq_grid")
                        .num_columns(4)
                        .striped(true)
                        .min_col_width(60.0)
                        .show(ui, |ui| {
                            for line in &boq {
                                if line.group != current_group {
                                    current_group = line.group;
                                    ui.label(RichText::new(current_group).strong());
                                    ui.label("");
                                    ui.label("");
                                    ui.label("");
                                    ui.end_row();
                                }
                                ui.label(&line.description);
                                let qty = match line.unit {
                                    BoqUnit::Each => format!("{}", line.quantity as i64),
                                    _ => format!("{:.1} {}", line.quantity, line.unit.label()),
                                };
                                ui.label(qty);
                                ui.label(format!("@ {}{:.2}", costs.currency, line.unit_cost));
                                ui.label(format!("{}{:.2}", costs.currency, line.total));
                                ui.end_row();
                            }
                        });
                }

                ui.add_space(8.0);

                // --- Editable unit rates ---
                ui.collapsing("Unit Rates", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Currency:");
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut costs.currency).desired_width(40.0))
                            .changed();
                    });
                    changed |= rate_grid(ui, "Per plant", &mut costs.plant_each);
                    changed |= rate_grid(ui, "Per m²", &mut costs.area_per_m2);
                    changed |= rate_grid(ui, "Per linear metre", &mut costs.linear_per_m);
                    changed |= rate_grid(ui, "Per item", &mut costs.each);
                    changed |= rate_grid(ui, "Material multipliers", &mut costs.material_factor);
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Export CSV…").clicked() {
                    action = Some(EstimateAction::ExportCsv);
                }
                if ui.button("Export Profile…")
                    .on_hover_text("Save these rates as a shareable JSON rate card")
                    .clicked()
                {
                    action = Some(EstimateAction::ExportProfile);
                }
                if ui.button("Import Profile…")
                    .on_hover_text("Replace these rates with a JSON rate card")
                    .clicked()
                {
                    action = Some(EstimateAction::ImportProfile);
                }
                if ui.button("Reset Defaults").clicked() {
                    *costs = UnitCosts::default();
                    changed = true;
                }
            });
        });

    (changed, action)
}

/// Grid of editable rates for one map. Keys are fixed; values are editable.
fn rate_grid(
    ui: &mut egui::Ui,
    title: &str,
    rates: &mut std::collections::BTreeMap<String, f64>,
) -> bool {
    let mut changed = false;
    ui.add_space(4.0);
    ui.label(RichText::new(title).strong());
    egui::Grid::new(title)
        .num_columns(2)
        .min_col_width(120.0)
        .show(ui, |ui| {
            for (key, value) in rates.iter_mut() {
                ui.label(key.as_str());
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .speed(0.5)
                            .range(0.0..=1_000_000.0)
                            .max_decimals(2),
                    )
                    .changed();
                ui.end_row();
            }
        });
    changed
}
