use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::background_image::BackgroundImage;
use crate::drawing::drawing::Drawing;
use crate::drawing::layout::is_layer_visible_in_viewport;
use crate::drawing::drawing::DrawingViewport;
use crate::entity::Entity;
use crate::layer::LayerManager;

/// Metadata for a Grove project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub author: String,
    pub datum_reference: String,
    pub units: String,
    pub created: String,
    pub modified: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            author: String::new(),
            datum_reference: "Site TBM".into(),
            units: "mm".into(),
            created: String::new(),
            modified: String::new(),
        }
    }
}

/// The project document model: entities + layers + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub entities: Vec<Entity>,
    pub layers: LayerManager,
    pub background_images: Vec<BackgroundImage>,
    #[serde(default)]
    pub drawings: Vec<Drawing>,
    pub dirty: bool,
}

impl Project {
    pub fn new() -> Self {
        Self {
            metadata: ProjectMetadata::default(),
            entities: Vec::new(),
            layers: LayerManager::new(),
            background_images: Vec::new(),
            drawings: Vec::new(),
            dirty: false,
        }
    }

    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
        self.dirty = true;
    }

    pub fn remove_entity(&mut self, id: Uuid) -> Option<Entity> {
        if let Some(pos) = self.entities.iter().position(|e| e.id == id) {
            self.dirty = true;
            Some(self.entities.remove(pos))
        } else {
            // Search children recursively
            for entity in &mut self.entities {
                if let Some(removed) = remove_child_entity(&mut entity.children, id) {
                    self.dirty = true;
                    return Some(removed);
                }
            }
            None
        }
    }

    pub fn find_entity(&self, id: Uuid) -> Option<&Entity> {
        find_entity_recursive(&self.entities, id)
    }

    pub fn find_entity_mut(&mut self, id: Uuid) -> Option<&mut Entity> {
        find_entity_mut_recursive(&mut self.entities, id)
    }

    /// Get all entities on visible layers.
    pub fn visible_entities(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| self.layers.is_visible(e.layer_id))
            .collect()
    }

    /// Total entity count (flat, not counting children).
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn add_background_image(&mut self, img: BackgroundImage) {
        self.background_images.push(img);
        self.dirty = true;
    }

    pub fn remove_background_image(&mut self, id: Uuid) -> Option<BackgroundImage> {
        if let Some(pos) = self.background_images.iter().position(|i| i.id == id) {
            self.dirty = true;
            Some(self.background_images.remove(pos))
        } else {
            None
        }
    }

    pub fn find_background_image(&self, id: Uuid) -> Option<&BackgroundImage> {
        self.background_images.iter().find(|i| i.id == id)
    }

    pub fn find_background_image_mut(&mut self, id: Uuid) -> Option<&mut BackgroundImage> {
        self.background_images.iter_mut().find(|i| i.id == id)
    }

    // --- Drawings ---

    pub fn add_drawing(&mut self, drawing: Drawing) {
        self.drawings.push(drawing);
        self.dirty = true;
    }

    pub fn remove_drawing(&mut self, id: Uuid) -> Option<Drawing> {
        if let Some(pos) = self.drawings.iter().position(|d| d.id == id) {
            self.dirty = true;
            Some(self.drawings.remove(pos))
        } else {
            None
        }
    }

    pub fn find_drawing(&self, id: Uuid) -> Option<&Drawing> {
        self.drawings.iter().find(|d| d.id == id)
    }

    pub fn find_drawing_mut(&mut self, id: Uuid) -> Option<&mut Drawing> {
        self.drawings.iter_mut().find(|d| d.id == id)
    }

    /// Generate the next drawing number (auto-increment).
    pub fn next_drawing_number(&self) -> String {
        let max_num = self.drawings.iter()
            .filter_map(|d| d.number.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{:02}", max_num + 1)
    }

    /// Get entities visible in a specific drawing viewport.
    pub fn entities_for_viewport(&self, viewport: &DrawingViewport) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| is_layer_visible_in_viewport(e.layer_id, viewport, &self.layers))
            .collect()
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

fn find_entity_recursive(entities: &[Entity], id: Uuid) -> Option<&Entity> {
    for e in entities {
        if e.id == id {
            return Some(e);
        }
        if let Some(found) = find_entity_recursive(&e.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_entity_mut_recursive(entities: &mut [Entity], id: Uuid) -> Option<&mut Entity> {
    for e in entities {
        if e.id == id {
            return Some(e);
        }
        if let Some(found) = find_entity_mut_recursive(&mut e.children, id) {
            return Some(found);
        }
    }
    None
}

fn remove_child_entity(children: &mut Vec<Entity>, id: Uuid) -> Option<Entity> {
    if let Some(pos) = children.iter().position(|e| e.id == id) {
        return Some(children.remove(pos));
    }
    for child in children {
        if let Some(removed) = remove_child_entity(&mut child.children, id) {
            return Some(removed);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Geometry};
    use glam::Vec2;

    fn make_circle_entity(name: &str) -> Entity {
        Entity::cad(
            name,
            Geometry::Circle(CircleData {
                center: Vec2::ZERO,
                radius: 100.0,
            }),
        )
    }

    #[test]
    fn test_add_find_remove() {
        let mut proj = Project::new();
        let e = make_circle_entity("test");
        let id = e.id;
        proj.add_entity(e);
        assert_eq!(proj.entity_count(), 1);
        assert!(proj.find_entity(id).is_some());
        proj.remove_entity(id);
        assert_eq!(proj.entity_count(), 0);
    }

    #[test]
    fn test_dirty_flag() {
        let mut proj = Project::new();
        assert!(!proj.dirty);
        proj.add_entity(make_circle_entity("a"));
        assert!(proj.dirty);
    }

    #[test]
    fn test_next_drawing_number() {
        let mut proj = Project::new();
        assert_eq!(proj.next_drawing_number(), "01");
        proj.add_drawing(Drawing::new("01", "First"));
        assert_eq!(proj.next_drawing_number(), "02");
        proj.add_drawing(Drawing::new("05", "Fifth"));
        assert_eq!(proj.next_drawing_number(), "06");
    }

    #[test]
    fn test_entities_for_viewport() {
        let mut proj = Project::new();
        let layer_id = proj.layers.layers[0].id;

        let mut e = make_circle_entity("visible");
        e.layer_id = layer_id;
        proj.add_entity(e);

        let mut vp = crate::drawing::drawing::DrawingViewport::new("Test", 100);
        // No overrides — entity should be visible
        assert_eq!(proj.entities_for_viewport(&vp).len(), 1);

        // Override: hide this layer
        vp.layer_overrides.visibility.insert(layer_id, false);
        assert_eq!(proj.entities_for_viewport(&vp).len(), 0);
    }
}
