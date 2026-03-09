use std::collections::HashMap;

use crate::entity::{DomainData, Entity};
use crate::garden::plant::Category;

/// A single row in the planting schedule.
#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub botanical_name: String,
    pub common_name: String,
    pub category: Category,
    pub quantity: usize,
    pub height_mm: i32,
    pub spread_mm: i32,
    pub colour_code: String,
    pub evergreen: bool,
}

/// Generate a planting schedule from project entities.
pub fn generate_schedule(entities: &[Entity]) -> Vec<ScheduleEntry> {
    let mut counts: HashMap<String, ScheduleEntry> = HashMap::new();

    for entity in entities {
        collect_plants(entity, &mut counts);
    }

    let mut entries: Vec<_> = counts.into_values().collect();
    entries.sort_by(|a, b| {
        a.category
            .as_str()
            .cmp(b.category.as_str())
            .then(a.botanical_name.cmp(&b.botanical_name))
    });
    entries
}

fn collect_plants(entity: &Entity, counts: &mut HashMap<String, ScheduleEntry>) {
    if let DomainData::Plant(ref data) = entity.domain {
        counts
            .entry(data.botanical_name.clone())
            .and_modify(|e| e.quantity += 1)
            .or_insert_with(|| ScheduleEntry {
                botanical_name: data.botanical_name.clone(),
                common_name: data.common_name.clone(),
                category: data.category,
                quantity: 1,
                height_mm: data.height_mm,
                spread_mm: data.spread_mm,
                colour_code: data.colour_code.clone(),
                evergreen: data.evergreen,
            });
    }
    for child in &entity.children {
        collect_plants(child, counts);
    }
}

/// Export schedule to CSV.
pub fn export_schedule_csv<W: std::io::Write>(
    entries: &[ScheduleEntry],
    writer: W,
) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record([
        "Botanical Name",
        "Common Name",
        "Category",
        "Quantity",
        "Height (mm)",
        "Spread (mm)",
        "Colour",
        "Evergreen",
    ])
    .map_err(|e| format!("{e}"))?;

    for entry in entries {
        wtr.write_record([
            &entry.botanical_name,
            &entry.common_name,
            entry.category.as_str(),
            &entry.quantity.to_string(),
            &entry.height_mm.to_string(),
            &entry.spread_mm.to_string(),
            &entry.colour_code,
            &if entry.evergreen { "Yes".to_string() } else { "No".to_string() },
        ])
        .map_err(|e| format!("{e}"))?;
    }

    wtr.flush().map_err(|e| format!("{e}"))?;
    Ok(())
}
