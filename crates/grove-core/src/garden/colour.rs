use crate::drawing::drawing::ColourMode;
use crate::entity::{DomainData, Entity};
use super::plant::Category;

/// Default colour for each plant category (RGBA).
pub fn category_colour(category: Category) -> [u8; 4] {
    match category {
        Category::Perennial => [76, 175, 80, 255],    // Green
        Category::Shrub => [139, 195, 74, 255],       // Light green
        Category::Tree => [56, 142, 60, 255],         // Dark green
        Category::Grass => [205, 220, 57, 255],       // Lime
        Category::Climber => [156, 39, 176, 255],     // Purple
        Category::Bulb => [255, 193, 7, 255],         // Amber
        Category::Fern => [0, 150, 136, 255],         // Teal
        Category::Hedge => [27, 94, 32, 255],         // Dark green
        Category::Aquatic => [3, 169, 244, 255],      // Light blue
    }
}

/// Parse a hex colour string (e.g. "#4CAF50") to RGBA.
pub fn hex_to_rgba(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b, 255])
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some([r, g, b, a])
    } else {
        None
    }
}

/// Convert RGBA to hex string.
pub fn rgba_to_hex(rgba: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
}

/// Convert HSL to RGBA. h in 0..360, s and l in 0.0..1.0.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 4] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        255,
    ]
}

/// Deterministic colour for a species name based on hash.
pub fn species_colour(botanical_name: &str) -> [u8; 4] {
    // Simple hash to hue
    let mut hash: u32 = 5381;
    for b in botanical_name.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u32);
    }
    let hue = (hash % 360) as f32;
    hsl_to_rgb(hue, 0.65, 0.45)
}

/// Colour for a domain type by category.
pub fn domain_category_colour(domain: &DomainData) -> [u8; 4] {
    match domain {
        DomainData::Plant(p) => category_colour(p.category),
        DomainData::PlantGroup(pg) => category_colour(pg.category),
        DomainData::Wall(_) => [120, 100, 80, 255],      // Brown
        DomainData::Path(_) => [160, 150, 140, 255],      // Warm grey
        DomainData::Patio(_) => [180, 170, 155, 255],     // Sand
        DomainData::Step(_) => [140, 130, 120, 255],      // Stone
        DomainData::Edging(_) => [130, 120, 110, 255],    // Dark stone
        DomainData::RaisedBed(_) => [139, 90, 43, 255],   // Wood
        DomainData::Fence(_) => [110, 80, 50, 255],       // Dark wood
        DomainData::Pergola(_) => [150, 120, 80, 255],    // Light wood
        DomainData::Decking(_) => [160, 130, 90, 255],    // Timber
        DomainData::Surface(s) => s.default_colour(),
        DomainData::WaterFeature(_) => [70, 130, 200, 255], // Water blue
        DomainData::House(_) => [139, 115, 85, 255],        // Brownish
        DomainData::SpotHeight(_) => [100, 100, 100, 255],
        DomainData::Contour(_) => [140, 100, 70, 255],
        DomainData::SlopeArrow(_) => [100, 100, 100, 255],
        DomainData::DrainRun(_) => [80, 120, 160, 255],
        DomainData::Entourage(_) => [120, 120, 120, 255],
        _ => [128, 128, 128, 255],  // Grey for None, TitleBlock, Legend
    }
}

/// Monochrome dark grey.
pub fn monochrome_colour() -> [u8; 4] {
    [40, 40, 40, 255]
}

/// Resolve an entity's colour based on the viewport's colour mode.
pub fn resolve_colour(entity: &Entity, mode: ColourMode) -> [u8; 4] {
    match mode {
        ColourMode::Default => entity.colour,
        ColourMode::ByCategory => domain_category_colour(&entity.domain),
        ColourMode::BySpecies => {
            match &entity.domain {
                DomainData::Plant(p) => species_colour(&p.botanical_name),
                DomainData::PlantGroup(pg) => species_colour(&pg.botanical_name),
                _ => domain_category_colour(&entity.domain),
            }
        }
        ColourMode::Monochrome => monochrome_colour(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_roundtrip() {
        let hex = "#4CAF50";
        let rgba = hex_to_rgba(hex).unwrap();
        assert_eq!(rgba, [76, 175, 80, 255]);
        assert_eq!(rgba_to_hex(rgba), "#4CAF50");
    }

    #[test]
    fn test_category_colours_unique() {
        let colours: Vec<_> = Category::ALL.iter().map(|c| category_colour(*c)).collect();
        // All colours should be different
        for i in 0..colours.len() {
            for j in (i + 1)..colours.len() {
                assert_ne!(colours[i], colours[j], "Categories should have unique colours");
            }
        }
    }

    #[test]
    fn test_hsl_to_rgb_known_values() {
        // Red: HSL(0, 1.0, 0.5)
        let red = hsl_to_rgb(0.0, 1.0, 0.5);
        assert_eq!(red, [255, 0, 0, 255]);

        // Green: HSL(120, 1.0, 0.5)
        let green = hsl_to_rgb(120.0, 1.0, 0.5);
        assert_eq!(green, [0, 255, 0, 255]);

        // Blue: HSL(240, 1.0, 0.5)
        let blue = hsl_to_rgb(240.0, 1.0, 0.5);
        assert_eq!(blue, [0, 0, 255, 255]);
    }

    #[test]
    fn test_species_colour_deterministic() {
        let c1 = species_colour("Rosa rugosa");
        let c2 = species_colour("Rosa rugosa");
        assert_eq!(c1, c2, "Same name should produce same colour");
    }

    #[test]
    fn test_species_colour_different() {
        let c1 = species_colour("Rosa rugosa");
        let c2 = species_colour("Lavandula angustifolia");
        assert_ne!(c1, c2, "Different species should produce different colours");
    }

    #[test]
    fn test_resolve_colour_modes() {
        use crate::entity::*;
        use crate::garden::plant::PlantData;

        let mut entity = Entity::new(
            "test_plant",
            Geometry::Circle(CircleData {
                center: glam::Vec2::ZERO,
                radius: 100.0,
            }),
            DomainData::Plant(PlantData {
                plant_id: uuid::Uuid::new_v4(),
                botanical_name: "Rosa rugosa".into(),
                common_name: "Rugosa Rose".into(),
                category: Category::Shrub,
                height_mm: 1500,
                spread_mm: 1200,
                colour_code: "#FF0000".into(),
                flower_colour: String::new(),
                foliage_colour: String::new(),
                evergreen: false,
                notes: String::new(),
            }),
        );
        entity.colour = [255, 0, 0, 255];

        // Default mode returns entity colour
        assert_eq!(resolve_colour(&entity, ColourMode::Default), [255, 0, 0, 255]);

        // ByCategory returns category colour for Shrub
        assert_eq!(resolve_colour(&entity, ColourMode::ByCategory), category_colour(Category::Shrub));

        // BySpecies returns species-derived colour
        let species_c = resolve_colour(&entity, ColourMode::BySpecies);
        assert_eq!(species_c, species_colour("Rosa rugosa"));

        // Monochrome returns dark grey
        assert_eq!(resolve_colour(&entity, ColourMode::Monochrome), [40, 40, 40, 255]);
    }
}
