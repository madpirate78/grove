use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::geometry::BoundingBox;

/// Types of dimension annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DimensionData {
    Linear {
        start: Vec2,
        end: Vec2,
        offset: f32,
        text_override: Option<String>,
    },
    Angular {
        center: Vec2,
        start_angle: f32,
        end_angle: f32,
        radius: f32,
    },
    Radial {
        center: Vec2,
        radius: f32,
        angle: f32,
    },
}

impl DimensionData {
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        match self {
            DimensionData::Linear { start, end, offset, .. } => {
                let normal = (*end - *start).perp().normalize_or_zero() * *offset;
                let points = [*start, *end, *start + normal, *end + normal];
                BoundingBox::from_points(&points)
            }
            DimensionData::Angular { center, radius, .. } => {
                Some(BoundingBox::new(
                    *center - Vec2::splat(*radius),
                    *center + Vec2::splat(*radius),
                ))
            }
            DimensionData::Radial { center, radius, .. } => {
                Some(BoundingBox::new(
                    *center - Vec2::splat(*radius),
                    *center + Vec2::splat(*radius),
                ))
            }
        }
    }

    /// Calculate the displayed measurement value (in mm).
    pub fn measurement(&self) -> f32 {
        match self {
            DimensionData::Linear { start, end, .. } => (*end - *start).length(),
            DimensionData::Angular { start_angle, end_angle, .. } => {
                (end_angle - start_angle).to_degrees().abs()
            }
            DimensionData::Radial { radius, .. } => *radius,
        }
    }
}
