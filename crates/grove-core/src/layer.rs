use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A layer in the project. Colour stored as [u8; 4] RGBA — no UI dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: Uuid,
    pub name: String,
    pub colour: [u8; 4],
    pub visible: bool,
    pub locked: bool,
    pub order: i32,
}

impl Layer {
    pub fn new(name: impl Into<String>, colour: [u8; 4]) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            colour,
            visible: true,
            locked: false,
            order: 0,
        }
    }
}

/// Manages layers for a project. Provides the default GD_* layer set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerManager {
    pub layers: Vec<Layer>,
    pub active_layer_id: Uuid,
}

impl LayerManager {
    pub fn new() -> Self {
        let defaults = Self::default_layers();
        let active = defaults[0].id;
        Self {
            layers: defaults,
            active_layer_id: active,
        }
    }

    /// The standard GD_* layers from the data model spec.
    pub fn default_layers() -> Vec<Layer> {
        let defs: Vec<(&str, [u8; 4], i32)> = vec![
            ("GD_PLANTING", [76, 175, 80, 255], 0),
            ("GD_HARDSCAPE_WALLS", [139, 125, 107, 255], 1),
            ("GD_HARDSCAPE_SURFACES", [196, 184, 168, 255], 2),
            ("GD_HARDSCAPE_STEPS", [160, 147, 125, 255], 3),
            ("GD_HARDSCAPE_STRUCTURES", [160, 82, 45, 255], 4),
            ("GD_HARDSCAPE_WATER", [59, 125, 216, 255], 5),
            ("GD_LEVELS", [229, 57, 53, 255], 6),
            ("GD_DRAINAGE", [27, 94, 32, 255], 7),
            ("GD_BOUNDARY", [158, 158, 158, 255], 8),
            ("GD_BUILDINGS", [117, 117, 117, 255], 9),
            ("GD_SURVEY", [189, 189, 189, 255], 10),
        ];
        defs.into_iter()
            .map(|(name, colour, order)| {
                let mut l = Layer::new(name, colour);
                l.order = order;
                l
            })
            .collect()
    }

    pub fn active_layer(&self) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == self.active_layer_id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn find_by_id(&self, id: Uuid) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn remove_layer(&mut self, id: Uuid) -> Option<Layer> {
        if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
            Some(self.layers.remove(pos))
        } else {
            None
        }
    }

    pub fn set_active(&mut self, id: Uuid) {
        if self.layers.iter().any(|l| l.id == id) {
            self.active_layer_id = id;
        }
    }

    pub fn is_visible(&self, layer_id: Uuid) -> bool {
        self.layers
            .iter()
            .find(|l| l.id == layer_id)
            .map(|l| l.visible)
            .unwrap_or(true)
    }
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layers_created() {
        let lm = LayerManager::new();
        assert_eq!(lm.layers.len(), 11);
        assert!(lm.find_by_name("GD_PLANTING").is_some());
        assert!(lm.find_by_name("GD_DRAINAGE").is_some());
    }

    #[test]
    fn test_add_remove_layer() {
        let mut lm = LayerManager::new();
        let custom = Layer::new("MY_LAYER", [255, 0, 0, 255]);
        let id = custom.id;
        lm.add_layer(custom);
        assert_eq!(lm.layers.len(), 12);
        assert!(lm.find_by_id(id).is_some());
        lm.remove_layer(id);
        assert_eq!(lm.layers.len(), 11);
    }
}
