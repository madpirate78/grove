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

/// Map botanical name → 1-based row number in the planting schedule.
/// Numbers follow the schedule/CSV row order (category, then botanical name),
/// so a numbered plant marker on a drawing keys into the exported schedule.
pub fn schedule_numbers(entities: &[Entity]) -> HashMap<String, usize> {
    generate_schedule(entities)
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.botanical_name.clone(), i + 1))
        .collect()
}

fn collect_plants(entity: &Entity, counts: &mut HashMap<String, ScheduleEntry>) {
    match &entity.domain {
        DomainData::Plant(data) => {
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
        // A group stands for `count` plants of one species; the group data
        // carries no common name or dimensions, so those stay empty unless a
        // sibling `Plant` of the same species fills them in first.
        DomainData::PlantGroup(group) => {
            counts
                .entry(group.botanical_name.clone())
                .and_modify(|e| e.quantity += group.count)
                .or_insert_with(|| ScheduleEntry {
                    botanical_name: group.botanical_name.clone(),
                    common_name: String::new(),
                    category: group.category,
                    quantity: group.count,
                    height_mm: 0,
                    spread_mm: 0,
                    colour_code: String::new(),
                    evergreen: false,
                });
        }
        _ => {}
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
        "#",
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

    for (i, entry) in entries.iter().enumerate() {
        wtr.write_record([
            &(i + 1).to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Entity, Geometry};
    use crate::garden::plant::PlantData;
    use glam::Vec2;

    fn plant_entity(botanical: &str, category: Category) -> Entity {
        let data = PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: botanical.into(),
            common_name: botanical.into(),
            category,
            height_mm: 1000,
            spread_mm: 800,
            colour_code: "#4CAF50".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen: false,
            notes: String::new(),
        };
        Entity::new(
            botanical,
            Geometry::Circle(CircleData { center: Vec2::ZERO, radius: 400.0 }),
            DomainData::Plant(data),
        )
    }

    #[test]
    fn test_plant_group_counts_in_schedule() {
        use crate::garden::plant::{DistributionMethod, PlantGroupData};

        let group = Entity::new(
            "Lavender drift",
            Geometry::Circle(CircleData { center: Vec2::ZERO, radius: 2000.0 }),
            DomainData::PlantGroup(PlantGroupData {
                plant_id: uuid::Uuid::new_v4(),
                botanical_name: "Lavandula angustifolia".into(),
                category: Category::Shrub,
                spacing_mm: 400.0,
                distribution: DistributionMethod::Grid,
                count: 25,
            }),
        );
        let entities = vec![
            plant_entity("Lavandula angustifolia", Category::Shrub),
            group,
        ];

        let schedule = generate_schedule(&entities);
        assert_eq!(schedule.len(), 1);
        // 1 individual plant + 25 in the group; individual supplies the details
        assert_eq!(schedule[0].quantity, 26);
        assert_eq!(schedule[0].common_name, "Lavandula angustifolia");
    }

    #[test]
    fn test_schedule_numbers_follow_schedule_order() {
        // Insertion order deliberately scrambled; numbering must follow
        // schedule order (category, then botanical name), not insertion.
        let entities = vec![
            plant_entity("Zebra grass", Category::Grass),
            plant_entity("Rosa rugosa", Category::Shrub),
            plant_entity("Acer palmatum", Category::Tree),
            plant_entity("Rosa rugosa", Category::Shrub),
        ];

        let schedule = generate_schedule(&entities);
        let numbers = schedule_numbers(&entities);

        assert_eq!(numbers.len(), 3);
        for (i, entry) in schedule.iter().enumerate() {
            assert_eq!(numbers[&entry.botanical_name], i + 1);
        }
    }

    #[test]
    fn test_schedule_csv_has_row_numbers() {
        let entities = vec![
            plant_entity("Rosa rugosa", Category::Shrub),
            plant_entity("Acer palmatum", Category::Tree),
        ];
        let schedule = generate_schedule(&entities);
        let mut buf = Vec::new();
        export_schedule_csv(&schedule, &mut buf).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let mut lines = csv.lines();
        assert!(lines.next().unwrap().starts_with("#,Botanical Name"));
        assert!(lines.next().unwrap().starts_with("1,"));
        assert!(lines.next().unwrap().starts_with("2,"));
    }
}
