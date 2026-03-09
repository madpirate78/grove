use serde::{Deserialize, Serialize};

/// Material types from the data model spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Material {
    Stone,
    Brick,
    Timber,
    Concrete,
    Metal,
    Composite,
    Gravel,
    ResinBound,
    Porcelain,
    Render,
    Corten,
    Glass,
}

impl Material {
    pub const ALL: &'static [Material] = &[
        Material::Stone,
        Material::Brick,
        Material::Timber,
        Material::Concrete,
        Material::Metal,
        Material::Composite,
        Material::Gravel,
        Material::ResinBound,
        Material::Porcelain,
        Material::Render,
        Material::Corten,
        Material::Glass,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Material::Stone => "Stone",
            Material::Brick => "Brick",
            Material::Timber => "Timber",
            Material::Concrete => "Concrete",
            Material::Metal => "Metal",
            Material::Composite => "Composite",
            Material::Gravel => "Gravel",
            Material::ResinBound => "Resin Bound",
            Material::Porcelain => "Porcelain",
            Material::Render => "Render",
            Material::Corten => "Corten",
            Material::Glass => "Glass",
        }
    }
}
