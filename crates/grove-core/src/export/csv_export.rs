use std::path::Path;

use crate::entity::Entity;
use crate::garden::schedule::{export_schedule_csv, generate_schedule};

pub fn export_planting_csv(entities: &[Entity], path: &Path) -> Result<(), String> {
    let schedule = generate_schedule(entities);
    let file = std::fs::File::create(path).map_err(|e| format!("Failed to create CSV file: {e}"))?;
    export_schedule_csv(&schedule, file)
}
