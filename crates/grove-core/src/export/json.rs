use std::path::Path;

use crate::project::Project;

pub fn export_json(project: &Project) -> Result<String, String> {
    serde_json::to_string_pretty(project).map_err(|e| format!("JSON serialization failed: {e}"))
}

pub fn export_json_to_file(project: &Project, path: &Path) -> Result<(), String> {
    let json = export_json(project)?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Entity, Geometry};
    use glam::Vec2;

    #[test]
    fn test_json_roundtrip() {
        let mut project = Project::new();
        project.add_entity(Entity::cad(
            "circle",
            Geometry::Circle(CircleData {
                center: Vec2::new(10.0, 20.0),
                radius: 50.0,
            }),
        ));

        let json = export_json(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.entity_count(), 1);
    }
}
