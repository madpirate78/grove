use std::collections::BTreeMap;

use svg::node::element::{self, Group};

use crate::drawing::layout::is_layer_visible_in_viewport;
use crate::drawing::drawing::DrawingViewport;
use crate::entity::{DomainData, Entity};
use crate::garden::colour::rgba_to_hex;
use crate::layer::LayerManager;

/// A legend entry for a unique plant species.
#[derive(Debug, Clone)]
pub struct LegendItem {
    pub common_name: String,
    pub botanical_name: String,
    pub colour: [u8; 4],
    pub count: usize,
}

/// Collect unique plants visible in a drawing's viewports.
pub fn collect_legend_items(
    entities: &[Entity],
    viewports: &[DrawingViewport],
    layers: &LayerManager,
) -> Vec<LegendItem> {
    // Use BTreeMap for deterministic ordering by botanical name
    let mut items: BTreeMap<String, LegendItem> = BTreeMap::new();

    for entity in entities {
        // Check if this entity is visible in ANY viewport
        let visible = viewports.iter().any(|vp| {
            is_layer_visible_in_viewport(entity.layer_id, vp, layers)
        });
        if !visible {
            continue;
        }

        match &entity.domain {
            DomainData::Plant(plant) => {
                items.entry(plant.botanical_name.clone())
                    .and_modify(|item| item.count += 1)
                    .or_insert(LegendItem {
                        common_name: plant.common_name.clone(),
                        botanical_name: plant.botanical_name.clone(),
                        colour: entity.colour,
                        count: 1,
                    });
            }
            DomainData::PlantGroup(group) => {
                items.entry(group.botanical_name.clone())
                    .and_modify(|item| item.count += group.count)
                    .or_insert(LegendItem {
                        common_name: group.botanical_name.clone(),
                        botanical_name: group.botanical_name.clone(),
                        colour: entity.colour,
                        count: group.count,
                    });
            }
            _ => {}
        }
    }

    items.into_values().collect()
}

/// Render a plant legend as an SVG group at the given position.
pub fn render_legend_svg(items: &[LegendItem], x: f32, y: f32) -> Group {
    let mut group = Group::new().set("id", "legend");

    if items.is_empty() {
        return group;
    }

    let row_height = 6.0_f32;
    let circle_r = 2.0_f32;
    let text_x = x + 8.0;
    let header_size = 5.0_f32;
    let text_size = 3.5_f32;
    let col_qty_x = text_x + 80.0;
    let legend_width = 100.0_f32;

    // Background
    let bg_height = row_height * items.len() as f32 + 12.0;
    let bg = element::Rectangle::new()
        .set("x", x - 2.0)
        .set("y", y - 2.0)
        .set("width", legend_width)
        .set("height", bg_height)
        .set("fill", "white")
        .set("stroke", "black")
        .set("stroke-width", 0.3);
    group = group.add(bg);

    // Header
    let header = element::Text::new("Plant Schedule".to_string())
        .set("x", x)
        .set("y", y + header_size)
        .set("font-size", header_size)
        .set("font-weight", "bold")
        .set("fill", "black");
    group = group.add(header);

    let start_y = y + header_size + 4.0;

    for (i, item) in items.iter().enumerate() {
        let row_y = start_y + i as f32 * row_height;
        let hex = rgba_to_hex(item.colour);

        // Colour circle
        let circle = element::Circle::new()
            .set("cx", x + circle_r + 1.0)
            .set("cy", row_y + circle_r)
            .set("r", circle_r)
            .set("fill", hex.clone())
            .set("stroke", "black")
            .set("stroke-width", 0.2);
        group = group.add(circle);

        // Name (italic botanical + common)
        let name = element::Text::new(format!("{} ({})", item.botanical_name, item.common_name))
            .set("x", text_x)
            .set("y", row_y + text_size)
            .set("font-size", text_size)
            .set("fill", "black");
        group = group.add(name);

        // Quantity
        let qty = element::Text::new(format!("x{}", item.count))
            .set("x", col_qty_x)
            .set("y", row_y + text_size)
            .set("font-size", text_size)
            .set("text-anchor", "end")
            .set("fill", "#666");
        group = group.add(qty);
    }

    group
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Entity, Geometry};
    use crate::garden::plant::{Category, PlantData};
    use crate::layer::LayerManager;
    use glam::Vec2;

    fn make_plant_entity(botanical: &str, common: &str, colour: [u8; 4], layer_id: uuid::Uuid) -> Entity {
        let mut e = Entity::new(
            common,
            Geometry::Circle(CircleData { center: Vec2::ZERO, radius: 500.0 }),
            DomainData::Plant(PlantData {
                plant_id: uuid::Uuid::new_v4(),
                botanical_name: botanical.into(),
                common_name: common.into(),
                category: Category::Perennial,
                height_mm: 600,
                spread_mm: 1000,
                colour_code: String::new(),
                flower_colour: String::new(),
                foliage_colour: String::new(),
                evergreen: false,
                notes: String::new(),
            }),
        );
        e.colour = colour;
        e.layer_id = layer_id;
        e
    }

    #[test]
    fn test_collect_legend_items() {
        let layers = LayerManager::new();
        let layer_id = layers.layers[0].id;
        let vp = DrawingViewport::new("Main", 100);

        let entities = vec![
            make_plant_entity("Rosa rugosa", "Rugosa Rose", [200, 50, 50, 255], layer_id),
            make_plant_entity("Rosa rugosa", "Rugosa Rose", [200, 50, 50, 255], layer_id),
            make_plant_entity("Lavandula angustifolia", "Lavender", [100, 50, 200, 255], layer_id),
        ];

        let items = collect_legend_items(&entities, &[vp], &layers);
        assert_eq!(items.len(), 2);

        let lavender = items.iter().find(|i| i.botanical_name == "Lavandula angustifolia").unwrap();
        assert_eq!(lavender.count, 1);

        let rosa = items.iter().find(|i| i.botanical_name == "Rosa rugosa").unwrap();
        assert_eq!(rosa.count, 2);
    }

    #[test]
    fn test_hidden_layer_excluded_from_legend() {
        let layers = LayerManager::new();
        let layer_id = layers.layers[0].id;
        let mut vp = DrawingViewport::new("Main", 100);
        vp.layer_overrides.visibility.insert(layer_id, false);

        let entities = vec![
            make_plant_entity("Rosa rugosa", "Rose", [200, 50, 50, 255], layer_id),
        ];

        let items = collect_legend_items(&entities, &[vp], &layers);
        assert!(items.is_empty());
    }

    #[test]
    fn test_render_legend_svg() {
        let items = vec![
            LegendItem {
                common_name: "Rose".into(),
                botanical_name: "Rosa rugosa".into(),
                colour: [200, 50, 50, 255],
                count: 3,
            },
        ];
        let group = render_legend_svg(&items, 10.0, 10.0);
        let svg = svg::Document::new().add(group).to_string();
        assert!(svg.contains("Rosa rugosa"));
        assert!(svg.contains("x3"));
        assert!(svg.contains("legend"));
    }
}
