use std::path::Path;

use crate::entity::Entity;
use crate::garden::schedule::{export_schedule_csv, generate_schedule};

pub fn export_planting_csv(entities: &[Entity], path: &Path) -> Result<(), String> {
    let schedule = generate_schedule(entities);
    let file = std::fs::File::create(path).map_err(|e| format!("Failed to create CSV file: {e}"))?;
    export_schedule_csv(&schedule, file)
}

/// Export a priced bill of quantities using the given unit-cost profile.
pub fn export_estimate_csv(
    entities: &[Entity],
    costs: &crate::garden::estimate::UnitCosts,
    path: &Path,
) -> Result<(), String> {
    use crate::garden::estimate::{export_boq_csv, generate_boq};
    let boq = generate_boq(entities, costs);
    let file = std::fs::File::create(path).map_err(|e| format!("Failed to create CSV file: {e}"))?;
    export_boq_csv(&boq, costs, file)
}
