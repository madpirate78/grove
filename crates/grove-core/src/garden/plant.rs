use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Plant categories matching the data model spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Perennial,
    Shrub,
    Tree,
    Grass,
    Climber,
    Bulb,
    Fern,
    Hedge,
    Aquatic,
}

impl Category {
    pub const ALL: &'static [Category] = &[
        Category::Perennial,
        Category::Shrub,
        Category::Tree,
        Category::Grass,
        Category::Climber,
        Category::Bulb,
        Category::Fern,
        Category::Hedge,
        Category::Aquatic,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Perennial => "Perennial",
            Category::Shrub => "Shrub",
            Category::Tree => "Tree",
            Category::Grass => "Grass",
            Category::Climber => "Climber",
            Category::Bulb => "Bulb",
            Category::Fern => "Fern",
            Category::Hedge => "Hedge",
            Category::Aquatic => "Aquatic",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "perennial" => Some(Category::Perennial),
            "shrub" => Some(Category::Shrub),
            "tree" => Some(Category::Tree),
            "grass" => Some(Category::Grass),
            "climber" => Some(Category::Climber),
            "bulb" => Some(Category::Bulb),
            "fern" => Some(Category::Fern),
            "hedge" => Some(Category::Hedge),
            "aquatic" => Some(Category::Aquatic),
            _ => None,
        }
    }
}

/// A plant in the database (template, not placed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plant {
    pub id: Uuid,
    pub botanical_name: String,
    pub common_name: String,
    pub category: Category,
    pub height_mm: i32,
    pub spread_mm: i32,
    pub colour_code: String,
    pub flower_colour: String,
    pub foliage_colour: String,
    pub evergreen: bool,
    pub notes: String,
}

impl Plant {
    pub fn new(botanical_name: impl Into<String>, category: Category) -> Self {
        Self {
            id: Uuid::new_v4(),
            botanical_name: botanical_name.into(),
            common_name: String::new(),
            category,
            height_mm: 0,
            spread_mm: 0,
            colour_code: "#4CAF50".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen: false,
            notes: String::new(),
        }
    }

    /// Radius in mm (half of spread).
    pub fn radius_mm(&self) -> f32 {
        self.spread_mm as f32 / 2.0
    }
}

/// Domain data for a placed plant entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantData {
    pub plant_id: Uuid,
    pub botanical_name: String,
    pub common_name: String,
    pub category: Category,
    pub height_mm: i32,
    pub spread_mm: i32,
    pub colour_code: String,
    pub flower_colour: String,
    pub foliage_colour: String,
    pub evergreen: bool,
    pub notes: String,
}

impl PlantData {
    pub fn from_plant(plant: &Plant) -> Self {
        Self {
            plant_id: plant.id,
            botanical_name: plant.botanical_name.clone(),
            common_name: plant.common_name.clone(),
            category: plant.category,
            height_mm: plant.height_mm,
            spread_mm: plant.spread_mm,
            colour_code: plant.colour_code.clone(),
            flower_colour: plant.flower_colour.clone(),
            foliage_colour: plant.foliage_colour.clone(),
            evergreen: plant.evergreen,
            notes: plant.notes.clone(),
        }
    }

    pub fn radius_mm(&self) -> f32 {
        self.spread_mm as f32 / 2.0
    }
}

/// Distribution method for plant groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistributionMethod {
    Grid,
    OffsetGrid,
    PoissonDisk,
}

/// Domain data for a plant group entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantGroupData {
    pub plant_id: Uuid,
    pub botanical_name: String,
    pub category: Category,
    pub spacing_mm: f32,
    pub distribution: DistributionMethod,
    pub count: usize,
}
