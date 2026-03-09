use crate::garden::plant::{Category, Plant};
use uuid::Uuid;

/// In-memory plant database. Will be backed by rusqlite in Phase 4.
#[derive(Debug, Default)]
pub struct PlantDatabase {
    pub plants: Vec<Plant>,
}

impl PlantDatabase {
    pub fn new() -> Self {
        Self { plants: Vec::new() }
    }

    pub fn add_plant(&mut self, plant: Plant) {
        self.plants.push(plant);
    }

    pub fn find_by_id(&self, id: Uuid) -> Option<&Plant> {
        self.plants.iter().find(|p| p.id == id)
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&Plant> {
        let query = name.to_lowercase();
        self.plants
            .iter()
            .filter(|p| {
                p.botanical_name.to_lowercase().contains(&query)
                    || p.common_name.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn filter_by_category(&self, category: Category) -> Vec<&Plant> {
        self.plants.iter().filter(|p| p.category == category).collect()
    }

    /// Load plants from CSV data (reader).
    pub fn load_from_csv<R: std::io::Read>(&mut self, reader: R) -> Result<usize, String> {
        let mut rdr = csv::Reader::from_reader(reader);
        let mut count = 0;

        for result in rdr.records() {
            let record = result.map_err(|e| format!("CSV error: {e}"))?;

            // Expected columns: botanical_name, common_name, category, height_mm, spread_mm,
            //                   colour_code, flower_colour, foliage_colour, evergreen, notes
            if record.len() < 6 {
                continue;
            }

            let botanical_name = record.get(0).unwrap_or("").to_string();
            let common_name = record.get(1).unwrap_or("").to_string();
            let category = record
                .get(2)
                .and_then(Category::from_str)
                .unwrap_or(Category::Perennial);
            let height_mm = record
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let spread_mm = record
                .get(4)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let colour_code = record.get(5).unwrap_or("#4CAF50").to_string();
            let flower_colour = record.get(6).unwrap_or("").to_string();
            let foliage_colour = record.get(7).unwrap_or("").to_string();
            let evergreen = record
                .get(8)
                .map(|s| s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes"))
                .unwrap_or(false);
            let notes = record.get(9).unwrap_or("").to_string();

            let mut plant = Plant::new(botanical_name, category);
            plant.common_name = common_name;
            plant.height_mm = height_mm;
            plant.spread_mm = spread_mm;
            plant.colour_code = colour_code;
            plant.flower_colour = flower_colour;
            plant.foliage_colour = foliage_colour;
            plant.evergreen = evergreen;
            plant.notes = notes;

            self.plants.push(plant);
            count += 1;
        }

        Ok(count)
    }

    /// Export plants to CSV.
    pub fn export_to_csv<W: std::io::Write>(&self, writer: W) -> Result<(), String> {
        let mut wtr = csv::Writer::from_writer(writer);
        wtr.write_record([
            "botanical_name",
            "common_name",
            "category",
            "height_mm",
            "spread_mm",
            "colour_code",
            "flower_colour",
            "foliage_colour",
            "evergreen",
            "notes",
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;

        for plant in &self.plants {
            wtr.write_record([
                &plant.botanical_name,
                &plant.common_name,
                plant.category.as_str(),
                &plant.height_mm.to_string(),
                &plant.spread_mm.to_string(),
                &plant.colour_code,
                &plant.flower_colour,
                &plant.foliage_colour,
                &if plant.evergreen { "Yes".to_string() } else { "No".to_string() },
                &plant.notes,
            ])
            .map_err(|e| format!("CSV write error: {e}"))?;
        }

        wtr.flush().map_err(|e| format!("CSV flush error: {e}"))?;
        Ok(())
    }

    pub fn plant_count(&self) -> usize {
        self.plants.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_roundtrip() {
        let mut db = PlantDatabase::new();
        let mut p = Plant::new("Lavandula angustifolia", Category::Shrub);
        p.common_name = "English Lavender".into();
        p.height_mm = 600;
        p.spread_mm = 600;
        db.add_plant(p);

        let mut buf = Vec::new();
        db.export_to_csv(&mut buf).unwrap();

        let mut db2 = PlantDatabase::new();
        let count = db2.load_from_csv(buf.as_slice()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(db2.plants[0].botanical_name, "Lavandula angustifolia");
        assert_eq!(db2.plants[0].category, Category::Shrub);
    }

    #[test]
    fn test_search() {
        let mut db = PlantDatabase::new();
        db.add_plant(Plant::new("Lavandula angustifolia", Category::Shrub));
        db.add_plant(Plant::new("Rosa rugosa", Category::Shrub));
        db.add_plant(Plant::new("Hosta sieboldiana", Category::Perennial));

        assert_eq!(db.find_by_name("lav").len(), 1);
        assert_eq!(db.find_by_name("rosa").len(), 1);
        assert_eq!(db.filter_by_category(Category::Shrub).len(), 2);
    }
}
