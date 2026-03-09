use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Spot Height
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeightSource {
    Survey,
    Design,
    Interpolated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotHeightData {
    pub object_id: Uuid,
    pub elevation_mm: f64,
    pub source: HeightSource,
    pub label_display: bool,
    pub datum_reference: String,
    pub accuracy_mm: i32,
}

impl Default for SpotHeightData {
    fn default() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            elevation_mm: 0.0,
            source: HeightSource::Survey,
            label_display: true,
            datum_reference: "Site TBM".into(),
            accuracy_mm: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Contour
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContourType {
    Major,
    Minor,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContourSource {
    Survey,
    Generated,
    Design,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContourData {
    pub object_id: Uuid,
    pub elevation_mm: f64,
    pub contour_type: ContourType,
    pub source: ContourSource,
    pub interval_mm: i32,
}

impl Default for ContourData {
    fn default() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            elevation_mm: 0.0,
            contour_type: ContourType::Minor,
            source: ContourSource::Generated,
            interval_mm: 250,
        }
    }
}

// ---------------------------------------------------------------------------
// Slope Arrow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlopeArrowData {
    pub object_id: Uuid,
    pub start_level_mm: f64,
    pub end_level_mm: f64,
}

impl SlopeArrowData {
    /// Calculate gradient as rise/run. Returns None if length is zero.
    pub fn gradient(&self, length_mm: f64) -> Option<f64> {
        if length_mm.abs() < f64::EPSILON {
            return None;
        }
        let fall = self.start_level_mm - self.end_level_mm;
        Some(fall / length_mm)
    }

    /// Format as "1:X" ratio.
    pub fn gradient_ratio(&self, length_mm: f64) -> Option<String> {
        self.gradient(length_mm).map(|g| {
            if g.abs() < f64::EPSILON {
                "Level".to_string()
            } else {
                format!("1:{:.0}", (1.0 / g.abs()))
            }
        })
    }
}

impl Default for SlopeArrowData {
    fn default() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            start_level_mm: 0.0,
            end_level_mm: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Drain Run
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrainType {
    Foul,
    Surface,
    Combined,
    LandDrain,
    FrenchDrain,
    Soakaway,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainRunData {
    pub object_id: Uuid,
    pub invert_start_mm: f64,
    pub invert_end_mm: f64,
    pub pipe_dia_mm: i32,
    pub drain_type: DrainType,
    pub manhole: bool,
}

impl DrainRunData {
    pub fn gradient(&self, length_mm: f64) -> Option<f64> {
        if length_mm.abs() < f64::EPSILON {
            return None;
        }
        let fall = self.invert_start_mm - self.invert_end_mm;
        Some(fall / length_mm)
    }
}

impl Default for DrainRunData {
    fn default() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            invert_start_mm: 0.0,
            invert_end_mm: 0.0,
            pipe_dia_mm: 110,
            drain_type: DrainType::Surface,
            manhole: false,
        }
    }
}
