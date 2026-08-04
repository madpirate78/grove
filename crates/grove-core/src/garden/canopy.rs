//! Procedural canopy symbols for the `Canopy` plant rendering style.
//!
//! Real landscape plans draw plants as stylised top-down canopy outlines —
//! scalloped rings for deciduous trees, spiky stars for conifers, soft lobes
//! for shrubs. Outlines are generated deterministically from the species and
//! the plant's position, so the interactive drawing view and the SVG export
//! produce the same symbol without storing any geometry.

use glam::Vec2;

use crate::garden::plant::{Category, PlantData};

/// Canopy symbol archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanopyForm {
    /// Scalloped ring of outward-bulging arcs — deciduous tree.
    Scalloped,
    /// Star of alternating outer/inner points — conifer or ornamental grass.
    Spiky,
    /// Soft irregular blob from layered sinusoids — shrubs and hedges.
    Lobed,
}

/// Pick a canopy archetype from plant attributes (landscape drawing convention).
pub fn canopy_form_for_plant(plant: &PlantData) -> CanopyForm {
    if plant.evergreen || plant.category == Category::Grass {
        CanopyForm::Spiky
    } else if plant.category == Category::Tree {
        CanopyForm::Scalloped
    } else {
        CanopyForm::Lobed
    }
}

/// FNV-1a hash — cheap, stable across platforms and runs.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Species-stable seed: same botanical name → same lobe count and phase.
pub fn species_seed(botanical_name: &str) -> u64 {
    fnv1a(botanical_name.as_bytes())
}

/// Instance rotation derived from the plant's position, so a row of the same
/// species doesn't look rubber-stamped. Deterministic: no stored state, and
/// both renderers derive the identical angle.
pub fn instance_rotation(center: Vec2) -> f32 {
    let bits = [center.x.to_bits().to_le_bytes(), center.y.to_bits().to_le_bytes()].concat();
    let h = fnv1a(&bits);
    (h % 1024) as f32 / 1024.0 * std::f32::consts::TAU
}

/// Generate a closed canopy outline in world space.
///
/// The outline touches `radius` at its widest and never exceeds it, so the
/// symbol respects the plant's spread. Points are ordered around the circle;
/// the loop is closed implicitly (last point connects back to the first).
pub fn canopy_outline(
    center: Vec2,
    radius: f32,
    form: CanopyForm,
    seed: u64,
    rotation: f32,
) -> Vec<Vec2> {
    match form {
        CanopyForm::Scalloped => scalloped_outline(center, radius, seed, rotation),
        CanopyForm::Spiky => spiky_outline(center, radius, seed, rotation),
        CanopyForm::Lobed => lobed_outline(center, radius, seed, rotation),
    }
}

/// Convenience: outline for a plant entity's circle, seeded per species and
/// rotated per instance.
pub fn plant_canopy_outline(plant: &PlantData, center: Vec2, radius: f32) -> Vec<Vec2> {
    canopy_outline(
        center,
        radius,
        canopy_form_for_plant(plant),
        species_seed(&plant.botanical_name),
        instance_rotation(center),
    )
}

fn scalloped_outline(center: Vec2, radius: f32, seed: u64, rotation: f32) -> Vec<Vec2> {
    let lobes = 7 + (seed % 4) as usize; // 7..=10
    let samples_per_lobe = 6;
    let base = radius * 0.86;
    let bulge = radius - base;
    let mut pts = Vec::with_capacity(lobes * samples_per_lobe);
    for lobe in 0..lobes {
        for s in 0..samples_per_lobe {
            let t = s as f32 / samples_per_lobe as f32;
            let theta = rotation
                + (lobe as f32 + t) / lobes as f32 * std::f32::consts::TAU;
            // Cusp at lobe boundaries, full bulge mid-lobe
            let r = base + bulge * (std::f32::consts::PI * t).sin();
            pts.push(center + Vec2::new(theta.cos(), theta.sin()) * r);
        }
    }
    pts
}

fn spiky_outline(center: Vec2, radius: f32, seed: u64, rotation: f32) -> Vec<Vec2> {
    let spikes = 9 + (seed % 5) as usize; // 9..=13
    let inner = radius * 0.62;
    let mut pts = Vec::with_capacity(spikes * 2);
    for i in 0..spikes * 2 {
        let theta = rotation + i as f32 / (spikes * 2) as f32 * std::f32::consts::TAU;
        let r = if i % 2 == 0 { radius } else { inner };
        pts.push(center + Vec2::new(theta.cos(), theta.sin()) * r);
    }
    pts
}

fn lobed_outline(center: Vec2, radius: f32, seed: u64, rotation: f32) -> Vec<Vec2> {
    let k1 = 3.0 + (seed % 3) as f32; // 3..=5 major lobes
    let k2 = 7.0 + ((seed >> 8) % 4) as f32; // finer ripple
    let p1 = ((seed >> 16) % 628) as f32 / 100.0;
    let p2 = ((seed >> 24) % 628) as f32 / 100.0;
    let samples = 48;
    // Max of the modulation below is 0.85 + 0.10 + 0.05 = 1.0 → never exceeds radius
    let mut pts = Vec::with_capacity(samples);
    for i in 0..samples {
        let theta = rotation + i as f32 / samples as f32 * std::f32::consts::TAU;
        let m = 0.85 + 0.10 * (k1 * theta + p1).sin().abs() + 0.05 * (k2 * theta + p2).sin();
        pts.push(center + Vec2::new(theta.cos(), theta.sin()) * (radius * m));
    }
    pts
}

/// Render an outline as an SVG path `d` attribute (closed).
pub fn outline_to_svg_path(pts: &[Vec2]) -> String {
    use std::fmt::Write;
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(d, "{}{:.1} {:.1} ", cmd, p.x, p.y);
    }
    d.push('Z');
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant(botanical: &str, category: Category, evergreen: bool) -> PlantData {
        PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: botanical.into(),
            common_name: botanical.into(),
            category,
            height_mm: 1000,
            spread_mm: 800,
            colour_code: "#4CAF50".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen,
            notes: String::new(),
        }
    }

    #[test]
    fn test_form_selection() {
        assert_eq!(canopy_form_for_plant(&plant("Pinus", Category::Tree, true)), CanopyForm::Spiky);
        assert_eq!(canopy_form_for_plant(&plant("Stipa", Category::Grass, false)), CanopyForm::Spiky);
        assert_eq!(canopy_form_for_plant(&plant("Acer", Category::Tree, false)), CanopyForm::Scalloped);
        assert_eq!(canopy_form_for_plant(&plant("Rosa", Category::Shrub, false)), CanopyForm::Lobed);
    }

    #[test]
    fn test_outline_stays_within_radius() {
        let center = Vec2::new(1000.0, -500.0);
        let radius = 600.0;
        for form in [CanopyForm::Scalloped, CanopyForm::Spiky, CanopyForm::Lobed] {
            let pts = canopy_outline(center, radius, form, 12345, 0.7);
            assert!(pts.len() >= 18, "{form:?} should have enough points");
            for p in &pts {
                let d = (*p - center).length();
                assert!(d <= radius * 1.001, "{form:?} point exceeds radius: {d} > {radius}");
                assert!(d >= radius * 0.5, "{form:?} point implausibly close to center: {d}");
            }
        }
    }

    #[test]
    fn test_deterministic_per_species_and_position() {
        let p = plant("Acer palmatum", Category::Tree, false);
        let a = plant_canopy_outline(&p, Vec2::new(10.0, 20.0), 500.0);
        let b = plant_canopy_outline(&p, Vec2::new(10.0, 20.0), 500.0);
        assert_eq!(a, b, "same species + position must produce identical outline");

        let c = plant_canopy_outline(&p, Vec2::new(900.0, 20.0), 500.0);
        assert_ne!(a[0] - Vec2::new(10.0, 20.0), c[0] - Vec2::new(900.0, 20.0),
            "different positions should rotate the symbol differently");
    }

    #[test]
    fn test_svg_path_closed() {
        let pts = canopy_outline(Vec2::ZERO, 100.0, CanopyForm::Scalloped, 1, 0.0);
        let d = outline_to_svg_path(&pts);
        assert!(d.starts_with('M'));
        assert!(d.ends_with('Z'));
        assert!(d.matches('L').count() >= pts.len() - 1);
    }
}
