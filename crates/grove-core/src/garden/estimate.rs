//! Offline bill of quantities — Estimate Builder Phase A.
//!
//! Walks the entity list and produces priced line items: plants counted per
//! species, linear hardscape (walls, fences, edging, steps) measured along its
//! centerline, area hardscape (patios, surfaces, decking…) measured from its
//! geometry. Rates come from a `UnitCosts` table seeded with rough installed
//! costs; Phase B will make these user-editable, shareable profiles, which is
//! why the table is string-keyed and serde-ready.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::entity::{DomainData, Entity, Geometry};
use crate::garden::hardscape::WaterFeatureType;
use crate::garden::material::Material;
use crate::garden::plant::Category;
use crate::geometry::polygon_area;

// ---------------------------------------------------------------------------
// Unit cost table
// ---------------------------------------------------------------------------

/// Unit rates used to price the bill of quantities.
///
/// String-keyed so a profile serializes to readable JSON (Phase B: exportable
/// regional rate cards). Missing keys price as zero rather than erroring — a
/// partial profile still produces a useful estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitCosts {
    /// Currency symbol used in exports.
    pub currency: String,
    /// Cost per plant, keyed by `Category::as_str()`.
    pub plant_each: BTreeMap<String, f64>,
    /// Cost per square metre, keyed by hardscape kind.
    pub area_per_m2: BTreeMap<String, f64>,
    /// Cost per linear metre, keyed by hardscape kind.
    pub linear_per_m: BTreeMap<String, f64>,
    /// Cost per item, keyed by kind (point water features etc.).
    pub each: BTreeMap<String, f64>,
    /// Multiplier applied on top of kind rates, keyed by `Material::as_str()`.
    pub material_factor: BTreeMap<String, f64>,
}

impl Default for UnitCosts {
    fn default() -> Self {
        let m = |pairs: &[(&str, f64)]| -> BTreeMap<String, f64> {
            pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
        };
        Self {
            currency: "£".into(),
            // Rough UK installed costs (supply + labour), 2026-ish
            plant_each: m(&[
                ("Tree", 120.0),
                ("Shrub", 25.0),
                ("Perennial", 8.0),
                ("Grass", 7.0),
                ("Climber", 15.0),
                ("Bulb", 1.5),
                ("Fern", 9.0),
                ("Hedge", 12.0),
                ("Aquatic", 10.0),
            ]),
            area_per_m2: m(&[
                ("Patio", 120.0),
                ("Path", 70.0),
                ("Surface", 60.0),
                ("Decking", 140.0),
                ("Raised Bed", 90.0),
                ("Pergola", 180.0),
                ("Water Feature", 150.0),
            ]),
            linear_per_m: m(&[
                ("Wall", 180.0),
                ("Fence", 85.0),
                ("Edging", 25.0),
                ("Steps", 220.0),
            ]),
            each: m(&[("Water Feature (point)", 600.0)]),
            material_factor: m(&[
                ("Stone", 1.3),
                ("Brick", 1.15),
                ("Timber", 1.0),
                ("Concrete", 0.9),
                ("Metal", 1.4),
                ("Composite", 1.2),
                ("Gravel", 0.5),
                ("Resin Bound", 1.25),
                ("Porcelain", 1.5),
                ("Render", 1.0),
                ("Corten", 1.6),
                ("Glass", 1.8),
            ]),
        }
    }
}

impl UnitCosts {
    fn plant_rate(&self, category: Category) -> f64 {
        self.plant_each.get(category.as_str()).copied().unwrap_or(0.0)
    }

    fn area_rate(&self, kind: &str, material: Material) -> f64 {
        self.area_per_m2.get(kind).copied().unwrap_or(0.0) * self.factor(material)
    }

    fn linear_rate(&self, kind: &str, material: Material) -> f64 {
        self.linear_per_m.get(kind).copied().unwrap_or(0.0) * self.factor(material)
    }

    fn each_rate(&self, kind: &str) -> f64 {
        self.each.get(kind).copied().unwrap_or(0.0)
    }

    fn factor(&self, material: Material) -> f64 {
        self.material_factor.get(material.as_str()).copied().unwrap_or(1.0)
    }
}

// ---------------------------------------------------------------------------
// Bill of quantities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoqUnit {
    Each,
    SquareMetre,
    LinearMetre,
}

impl BoqUnit {
    pub fn label(&self) -> &'static str {
        match self {
            BoqUnit::Each => "ea",
            BoqUnit::SquareMetre => "m2",
            BoqUnit::LinearMetre => "m",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoqLine {
    pub group: &'static str,
    pub description: String,
    pub unit: BoqUnit,
    pub quantity: f64,
    pub unit_cost: f64,
    pub total: f64,
}

/// Generate an aggregated, priced bill of quantities from project entities.
///
/// Plant quantities are counted the same way as the planting schedule
/// (per-`Plant` entity, recursing into children) so the two always agree.
pub fn generate_boq(entities: &[Entity], costs: &UnitCosts) -> Vec<BoqLine> {
    // (group, description, unit) → (quantity, unit_cost)
    let mut agg: BTreeMap<(&'static str, String, BoqUnit), (f64, f64)> = BTreeMap::new();
    for entity in entities {
        collect_entity(entity, costs, &mut agg);
    }

    let mut lines: Vec<BoqLine> = agg
        .into_iter()
        .map(|((group, description, unit), (quantity, unit_cost))| BoqLine {
            group,
            description,
            unit,
            quantity,
            unit_cost,
            total: quantity * unit_cost,
        })
        .collect();

    // Plants first, then hardscape — the order clients expect on an estimate
    let group_order = |g: &str| match g {
        "Plants" => 0,
        "Hardscape" => 1,
        _ => 2,
    };
    lines.sort_by(|a, b| {
        group_order(a.group)
            .cmp(&group_order(b.group))
            .then_with(|| a.description.cmp(&b.description))
    });
    lines
}

/// Sum of all line totals.
pub fn boq_total(lines: &[BoqLine]) -> f64 {
    lines.iter().map(|l| l.total).sum()
}

fn add(
    agg: &mut BTreeMap<(&'static str, String, BoqUnit), (f64, f64)>,
    group: &'static str,
    description: String,
    unit: BoqUnit,
    quantity: f64,
    unit_cost: f64,
) {
    if quantity <= 0.0 {
        return;
    }
    let entry = agg.entry((group, description, unit)).or_insert((0.0, unit_cost));
    entry.0 += quantity;
}

fn collect_entity(
    entity: &Entity,
    costs: &UnitCosts,
    agg: &mut BTreeMap<(&'static str, String, BoqUnit), (f64, f64)>,
) {
    let geom = &entity.geometry;
    match &entity.domain {
        DomainData::Plant(p) => {
            let display = if p.common_name.is_empty() {
                p.botanical_name.clone()
            } else {
                format!("{} ({})", p.botanical_name, p.common_name)
            };
            add(agg, "Plants", display, BoqUnit::Each, 1.0, costs.plant_rate(p.category));
        }
        DomainData::Wall(w) => {
            let mat = w.base.material;
            add(agg, "Hardscape", format!("Wall — {}", mat.as_str()),
                BoqUnit::LinearMetre, length_m(geom), costs.linear_rate("Wall", mat));
        }
        DomainData::Fence(f) => {
            let mat = f.base.material;
            add(agg, "Hardscape", format!("Fence — {}", mat.as_str()),
                BoqUnit::LinearMetre, length_m(geom), costs.linear_rate("Fence", mat));
        }
        DomainData::Edging(e) => {
            let mat = e.base.material;
            add(agg, "Hardscape", format!("Edging — {}", mat.as_str()),
                BoqUnit::LinearMetre, length_m(geom), costs.linear_rate("Edging", mat));
        }
        DomainData::Step(s) => {
            let mat = s.base.material;
            add(agg, "Hardscape", format!("Steps — {}", mat.as_str()),
                BoqUnit::LinearMetre, length_m(geom), costs.linear_rate("Steps", mat));
        }
        DomainData::Path(p) => {
            let mat = p.base.material;
            let area = length_m(geom) * p.width_mm as f64 / 1000.0;
            add(agg, "Hardscape", format!("Path — {}", mat.as_str()),
                BoqUnit::SquareMetre, area, costs.area_rate("Path", mat));
        }
        DomainData::Surface(s) => {
            let mat = s.base.material;
            // Width surfaces are drawn as a centerline; area ones as a polygon
            let area = match s.width_mm {
                Some(w) => length_m(geom) * w as f64 / 1000.0,
                None => area_m2(geom),
            };
            add(agg, "Hardscape", format!("Surface — {}", mat.as_str()),
                BoqUnit::SquareMetre, area, costs.area_rate("Surface", mat));
        }
        DomainData::Patio(p) => {
            let mat = p.base.material;
            add(agg, "Hardscape", format!("Patio — {}", mat.as_str()),
                BoqUnit::SquareMetre, area_m2(geom), costs.area_rate("Patio", mat));
        }
        DomainData::Decking(d) => {
            let mat = d.base.material;
            add(agg, "Hardscape", format!("Decking — {}", mat.as_str()),
                BoqUnit::SquareMetre, area_m2(geom), costs.area_rate("Decking", mat));
        }
        DomainData::RaisedBed(r) => {
            let mat = r.base.material;
            add(agg, "Hardscape", format!("Raised Bed — {}", mat.as_str()),
                BoqUnit::SquareMetre, area_m2(geom), costs.area_rate("Raised Bed", mat));
        }
        DomainData::Pergola(p) => {
            let mat = p.base.material;
            add(agg, "Hardscape", format!("Pergola — {}", mat.as_str()),
                BoqUnit::SquareMetre, area_m2(geom), costs.area_rate("Pergola", mat));
        }
        DomainData::WaterFeature(w) => {
            let mat = w.base.material;
            match w.feature_type {
                WaterFeatureType::Pond | WaterFeatureType::Rill | WaterFeatureType::Stream => {
                    add(agg, "Hardscape", format!("Water Feature — {:?}", w.feature_type),
                        BoqUnit::SquareMetre, area_m2(geom), costs.area_rate("Water Feature", mat));
                }
                _ => {
                    add(agg, "Hardscape", format!("Water Feature — {:?}", w.feature_type),
                        BoqUnit::Each, 1.0, costs.each_rate("Water Feature (point)"));
                }
            }
        }
        // Existing structures, annotations, levels, and presentation entities
        // carry no install cost
        _ => {}
    }

    for child in &entity.children {
        collect_entity(child, costs, agg);
    }
}

// ---------------------------------------------------------------------------
// Geometry measurement (model mm → metres)
// ---------------------------------------------------------------------------

fn length_m(geometry: &Geometry) -> f64 {
    let Some(pts) = geometry.centerline_points() else {
        return 0.0;
    };
    let mm: f32 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    mm as f64 / 1000.0
}

fn area_m2(geometry: &Geometry) -> f64 {
    let mm2 = match geometry {
        Geometry::Circle(c) => std::f32::consts::PI * c.radius * c.radius,
        Geometry::Rectangle(r) => 4.0 * r.half_size.x * r.half_size.y,
        Geometry::Polygon(pg) => {
            let outer = polygon_area(&pg.vertices).abs();
            let holes: f32 = pg.holes.iter().map(|h| polygon_area(h).abs()).sum();
            (outer - holes).max(0.0)
        }
        Geometry::Polyline(pl) if pl.closed => polygon_area(&pl.vertices).abs(),
        Geometry::BezierPath(bp) if bp.closed => {
            polygon_area(&bp.flatten(16)).abs()
        }
        _ => 0.0,
    };
    mm2 as f64 / 1_000_000.0
}

// ---------------------------------------------------------------------------
// CSV export
// ---------------------------------------------------------------------------

pub fn export_boq_csv<W: std::io::Write>(
    lines: &[BoqLine],
    costs: &UnitCosts,
    writer: W,
) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(writer);
    let cur = &costs.currency;
    wtr.write_record([
        "Group",
        "Item",
        "Quantity",
        "Unit",
        &format!("Rate ({cur})"),
        &format!("Total ({cur})"),
    ])
    .map_err(|e| format!("{e}"))?;

    for line in lines {
        let qty = match line.unit {
            BoqUnit::Each => format!("{}", line.quantity as i64),
            _ => format!("{:.1}", line.quantity),
        };
        wtr.write_record([
            line.group,
            &line.description,
            &qty,
            line.unit.label(),
            &format!("{:.2}", line.unit_cost),
            &format!("{:.2}", line.total),
        ])
        .map_err(|e| format!("{e}"))?;
    }

    wtr.write_record(["", "", "", "", "Total", &format!("{:.2}", boq_total(lines))])
        .map_err(|e| format!("{e}"))?;
    wtr.flush().map_err(|e| format!("{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, LineData, PolygonData};
    use crate::garden::hardscape::{PatioData, WallData};
    use crate::garden::plant::PlantData;
    use glam::Vec2;

    fn plant_entity(botanical: &str, category: Category) -> Entity {
        let data = PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: botanical.into(),
            common_name: String::new(),
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
    fn test_plants_aggregate_by_species() {
        let entities = vec![
            plant_entity("Rosa rugosa", Category::Shrub),
            plant_entity("Rosa rugosa", Category::Shrub),
            plant_entity("Acer palmatum", Category::Tree),
        ];
        let costs = UnitCosts::default();
        let boq = generate_boq(&entities, &costs);
        assert_eq!(boq.len(), 2);
        let rosa = boq.iter().find(|l| l.description.contains("Rosa")).unwrap();
        assert_eq!(rosa.quantity, 2.0);
        assert_eq!(rosa.unit_cost, 25.0);
        assert_eq!(rosa.total, 50.0);
    }

    #[test]
    fn test_wall_priced_per_metre_with_material_factor() {
        let mut wall = WallData::default();
        wall.base.material = Material::Brick;
        let e = Entity::new(
            "wall",
            Geometry::Line(LineData {
                start: Vec2::ZERO,
                end: Vec2::new(4000.0, 0.0), // 4 m
            }),
            DomainData::Wall(wall),
        );
        let costs = UnitCosts::default();
        let boq = generate_boq(&[e], &costs);
        assert_eq!(boq.len(), 1);
        let line = &boq[0];
        assert_eq!(line.unit, BoqUnit::LinearMetre);
        assert!((line.quantity - 4.0).abs() < 1e-6);
        // 180 £/m × 1.15 brick factor × 4 m
        assert!((line.total - 180.0 * 1.15 * 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_patio_area_subtracts_holes() {
        let mut patio = PatioData::default();
        patio.base.material = Material::Stone;
        // 5 m × 4 m patio with a 1 m × 1 m planting hole = 19 m²
        let e = Entity::new(
            "patio",
            Geometry::Polygon(PolygonData {
                vertices: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(5000.0, 0.0),
                    Vec2::new(5000.0, 4000.0),
                    Vec2::new(0.0, 4000.0),
                ],
                holes: vec![vec![
                    Vec2::new(1000.0, 1000.0),
                    Vec2::new(2000.0, 1000.0),
                    Vec2::new(2000.0, 2000.0),
                    Vec2::new(1000.0, 2000.0),
                ]],
            }),
            DomainData::Patio(patio),
        );
        let boq = generate_boq(&[e], &UnitCosts::default());
        assert!((boq[0].quantity - 19.0).abs() < 1e-4);
    }

    #[test]
    fn test_csv_has_total_row() {
        let entities = vec![plant_entity("Rosa rugosa", Category::Shrub)];
        let boq = generate_boq(&entities, &UnitCosts::default());
        let mut buf = Vec::new();
        export_boq_csv(&boq, &UnitCosts::default(), &mut buf).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        assert!(csv.starts_with("Group,Item,Quantity,Unit"));
        assert!(csv.contains("Rosa rugosa"));
        assert!(csv.lines().last().unwrap().contains("Total,25.00"));
    }

    #[test]
    fn test_unit_costs_json_roundtrip() {
        // Phase B will ship rate cards as JSON files — keep the format stable
        let costs = UnitCosts::default();
        let json = serde_json::to_string_pretty(&costs).unwrap();
        let back: UnitCosts = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plant_each, costs.plant_each);
        assert_eq!(back.material_factor, costs.material_factor);
    }
}
