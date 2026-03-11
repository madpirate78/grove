use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::material::Material;

// ---------------------------------------------------------------------------
// Common hardscape base (embedded in each type)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardscapeBase {
    pub object_id: Uuid,
    pub material: Material,
    pub material_detail: String,
    pub colour_code: String,
    pub finish: String,
    pub notes: String,
}

impl Default for HardscapeBase {
    fn default() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            material: Material::Stone,
            material_detail: String::new(),
            colour_code: "#8B7D6B".into(),
            finish: String::new(),
            notes: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Wall
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallType {
    Freestanding,
    Retaining,
    Seat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallData {
    pub base: HardscapeBase,
    pub height_mm: i32,
    pub width_mm: i32,
    pub wall_type: WallType,
    pub capping: String,
    pub capping_material: String,
    pub foundation_depth_mm: i32,
    pub jointing: String,
    pub base_level_mm: f64,
}

impl Default for WallData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#8B7D6B".into(),
                ..Default::default()
            },
            height_mm: 450,
            width_mm: 215,
            wall_type: WallType::Freestanding,
            capping: String::new(),
            capping_material: String::new(),
            foundation_depth_mm: 450,
            jointing: "Flush".into(),
            base_level_mm: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrainageType {
    None,
    CrossFall,
    Permeable,
    Channel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathData {
    pub base: HardscapeBase,
    pub width_mm: i32,
    pub edging: String,
    pub sub_base_depth_mm: i32,
    pub laying_pattern: String,
    pub joint_width_mm: i32,
    pub joint_material: String,
    pub drainage: DrainageType,
    pub cross_fall_mm_per_m: i32,
}

impl Default for PathData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#C4B8A8".into(),
                ..Default::default()
            },
            width_mm: 900,
            edging: String::new(),
            sub_base_depth_mm: 150,
            laying_pattern: "Stretcher".into(),
            joint_width_mm: 10,
            joint_material: "Mortar".into(),
            drainage: DrainageType::None,
            cross_fall_mm_per_m: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Patio
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatioDrainage {
    None,
    CrossFall,
    Permeable,
    AcoChannel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatioData {
    pub base: HardscapeBase,
    pub sub_base_depth_mm: i32,
    pub laying_pattern: String,
    pub joint_width_mm: i32,
    pub joint_material: String,
    pub drainage: PatioDrainage,
    pub fall_direction_deg: f64,
    pub fall_rate_mm_per_m: i32,
    pub finished_level_mm: f64,
}

impl Default for PatioData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#C4B8A8".into(),
                ..Default::default()
            },
            sub_base_depth_mm: 150,
            laying_pattern: "Stretcher".into(),
            joint_width_mm: 10,
            joint_material: "Mortar".into(),
            drainage: PatioDrainage::CrossFall,
            fall_direction_deg: 0.0,
            fall_rate_mm_per_m: 15,
            finished_level_mm: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Step
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandrailSide {
    Left,
    Right,
    Both,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepData {
    pub base: HardscapeBase,
    pub riser_height_mm: i32,
    pub tread_depth_mm: i32,
    pub width_mm: i32,
    pub nosing_mm: i32,
    pub riser_material: String,
    pub num_risers: i32,
    pub base_level_mm: f64,
    pub handrail: bool,
    pub handrail_side: HandrailSide,
}

impl Default for StepData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#A0937D".into(),
                ..Default::default()
            },
            riser_height_mm: 150,
            tread_depth_mm: 350,
            width_mm: 900,
            nosing_mm: 25,
            riser_material: String::new(),
            num_risers: 1,
            base_level_mm: 0.0,
            handrail: false,
            handrail_side: HandrailSide::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Edging
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgingType {
    Flush,
    Raised,
    MowingStrip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgingData {
    pub base: HardscapeBase,
    pub height_mm: i32,
    pub width_mm: i32,
    pub depth_below_mm: i32,
    pub edging_type: EdgingType,
    pub profile: String,
}

impl Default for EdgingData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#6B6158".into(),
                ..Default::default()
            },
            height_mm: 150,
            width_mm: 50,
            depth_below_mm: 100,
            edging_type: EdgingType::Flush,
            profile: "Flat".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Raised Bed
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaisedBedData {
    pub base: HardscapeBase,
    pub height_mm: i32,
    pub wall_width_mm: i32,
    pub capping: String,
    pub soil_depth_mm: i32,
    pub drainage: bool,
    pub liner: bool,
    pub base_level_mm: f64,
}

impl Default for RaisedBedData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#8B7D6B".into(),
                ..Default::default()
            },
            height_mm: 450,
            wall_width_mm: 100,
            capping: String::new(),
            soil_depth_mm: 400,
            drainage: true,
            liner: false,
            base_level_mm: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Fence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FenceType {
    CloseBoard,
    HitAndMiss,
    Trellis,
    PostAndRail,
    Picket,
    Woven,
    MetalRailing,
    Slatted,
    Acoustic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostSetting {
    Concrete,
    PostSpike,
    BoltDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FenceData {
    pub base: HardscapeBase,
    pub height_mm: i32,
    pub fence_type: FenceType,
    pub post_spacing_mm: i32,
    pub post_size: String,
    pub post_material: String,
    pub post_setting: PostSetting,
    pub gravel_board: bool,
    pub gravel_board_height_mm: i32,
    pub gate: bool,
}

impl Default for FenceData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#A0522D".into(),
                material: Material::Timber,
                ..Default::default()
            },
            height_mm: 1800,
            fence_type: FenceType::CloseBoard,
            post_spacing_mm: 1800,
            post_size: "100x100".into(),
            post_material: String::new(),
            post_setting: PostSetting::Concrete,
            gravel_board: true,
            gravel_board_height_mm: 150,
            gate: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Pergola
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureType {
    Pergola,
    Arbour,
    Gazebo,
    ShadeSail,
    Carport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoofType {
    Open,
    Polycarbonate,
    Glass,
    Fabric,
    GreenRoof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostFixing {
    BoltDown,
    SetInConcrete,
    PostAnchor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PergolaData {
    pub base: HardscapeBase,
    pub height_mm: i32,
    pub post_size: String,
    pub beam_size: String,
    pub post_spacing_mm: i32,
    pub rafter_spacing_mm: i32,
    pub rafter_overhang_mm: i32,
    pub structure_type: StructureType,
    pub roof: RoofType,
    pub post_fixing: PostFixing,
}

impl Default for PergolaData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#A0522D".into(),
                material: Material::Timber,
                ..Default::default()
            },
            height_mm: 2400,
            post_size: "100x100".into(),
            beam_size: "150x50".into(),
            post_spacing_mm: 2000,
            rafter_spacing_mm: 400,
            rafter_overhang_mm: 200,
            structure_type: StructureType::Pergola,
            roof: RoofType::Open,
            post_fixing: PostFixing::BoltDown,
        }
    }
}

// ---------------------------------------------------------------------------
// Decking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Substructure {
    Timber,
    Aluminium,
    AdjustablePedestal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckingData {
    pub base: HardscapeBase,
    pub board_width_mm: i32,
    pub board_gap_mm: i32,
    pub board_direction_deg: f64,
    pub deck_height_mm: i32,
    pub joist_spacing_mm: i32,
    pub substructure: Substructure,
    pub balustrade: bool,
    pub balustrade_height_mm: i32,
    pub balustrade_type: String,
}

impl Default for DeckingData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#C19A6B".into(),
                material: Material::Composite,
                ..Default::default()
            },
            board_width_mm: 140,
            board_gap_mm: 6,
            board_direction_deg: 0.0,
            deck_height_mm: 300,
            joist_spacing_mm: 400,
            substructure: Substructure::Timber,
            balustrade: false,
            balustrade_height_mm: 1100,
            balustrade_type: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// House
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseData {
    pub base: HardscapeBase,
    pub wall_thickness_mm: i32,
    pub height_mm: i32,
}

impl Default for HouseData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#8B7355".into(),
                ..Default::default()
            },
            wall_thickness_mm: 300,
            height_mm: 2700,
        }
    }
}

// ---------------------------------------------------------------------------
// Water Feature
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterFeatureType {
    Pond,
    Rill,
    Fountain,
    WallSpout,
    Bowl,
    Trough,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filtration {
    None,
    Mechanical,
    Biological,
    Uv,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterFeatureData {
    pub base: HardscapeBase,
    pub feature_type: WaterFeatureType,
    pub depth_mm: i32,
    pub liner: String,
    pub pump: bool,
    pub pump_lph: i32,
    pub lighting: bool,
    pub water_level_mm: f64,
    pub edge_treatment: String,
    pub filtration: Filtration,
}

impl Default for WaterFeatureData {
    fn default() -> Self {
        Self {
            base: HardscapeBase {
                colour_code: "#3B7DD8".into(),
                ..Default::default()
            },
            feature_type: WaterFeatureType::Pond,
            depth_mm: 600,
            liner: "Butyl".into(),
            pump: true,
            pump_lph: 0,
            lighting: false,
            water_level_mm: 0.0,
            edge_treatment: String::new(),
            filtration: Filtration::None,
        }
    }
}
