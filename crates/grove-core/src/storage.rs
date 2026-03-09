//! Save and load Grove projects as `.grove` SQLite files.
//!
//! Schema:
//! - `metadata` — key/value pairs for project metadata
//! - `layers` — one row per layer (JSON blob)
//! - `entities` — one row per entity (JSON blob)

use rusqlite::{params, Connection};
use std::path::Path;

use crate::background_image::BackgroundImage;
use crate::drawing::drawing::Drawing;
use crate::entity::Entity;
use crate::layer::{Layer, LayerManager};
use crate::project::{Project, ProjectMetadata};

/// Current schema version. Bump when the schema changes.
const SCHEMA_VERSION: i32 = 3;

/// Save a project to a `.grove` SQLite file.
pub fn save_project(project: &Project, path: &Path) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|e| format!("Cannot open file: {e}"))?;

    // Create tables (idempotent)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS grove_meta (key TEXT PRIMARY KEY, value TEXT);
         CREATE TABLE IF NOT EXISTS layers (id TEXT PRIMARY KEY, data TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS entities (id TEXT PRIMARY KEY, data TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS background_images (id TEXT PRIMARY KEY, metadata TEXT NOT NULL, image_blob BLOB NOT NULL);
         CREATE TABLE IF NOT EXISTS drawings (id TEXT PRIMARY KEY, data TEXT NOT NULL);",
    )
    .map_err(|e| format!("Schema creation failed: {e}"))?;

    // Use a transaction for atomicity
    let tx = conn.unchecked_transaction().map_err(|e| format!("Transaction start failed: {e}"))?;

    // Clear existing data
    tx.execute_batch("DELETE FROM grove_meta; DELETE FROM layers; DELETE FROM entities; DELETE FROM background_images; DELETE FROM drawings;")
        .map_err(|e| format!("Clear failed: {e}"))?;

    // Write schema version
    tx.execute(
        "INSERT INTO grove_meta (key, value) VALUES (?1, ?2)",
        params!["schema_version", SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| format!("Write schema version failed: {e}"))?;

    // Write metadata
    let meta_json =
        serde_json::to_string(&project.metadata).map_err(|e| format!("Serialize metadata: {e}"))?;
    tx.execute(
        "INSERT INTO grove_meta (key, value) VALUES (?1, ?2)",
        params!["project_metadata", meta_json],
    )
    .map_err(|e| format!("Write metadata failed: {e}"))?;

    // Write active layer id
    tx.execute(
        "INSERT INTO grove_meta (key, value) VALUES (?1, ?2)",
        params!["active_layer_id", project.layers.active_layer_id.to_string()],
    )
    .map_err(|e| format!("Write active layer failed: {e}"))?;

    // Write layers
    for layer in &project.layers.layers {
        let json = serde_json::to_string(layer).map_err(|e| format!("Serialize layer: {e}"))?;
        tx.execute(
            "INSERT INTO layers (id, data) VALUES (?1, ?2)",
            params![layer.id.to_string(), json],
        )
        .map_err(|e| format!("Write layer failed: {e}"))?;
    }

    // Write entities
    for entity in &project.entities {
        let json = serde_json::to_string(entity).map_err(|e| format!("Serialize entity: {e}"))?;
        tx.execute(
            "INSERT INTO entities (id, data) VALUES (?1, ?2)",
            params![entity.id.to_string(), json],
        )
        .map_err(|e| format!("Write entity failed: {e}"))?;
    }

    // Write drawings
    for drawing in &project.drawings {
        let json = serde_json::to_string(drawing).map_err(|e| format!("Serialize drawing: {e}"))?;
        tx.execute(
            "INSERT INTO drawings (id, data) VALUES (?1, ?2)",
            params![drawing.id.to_string(), json],
        )
        .map_err(|e| format!("Write drawing failed: {e}"))?;
    }

    // Write background images
    for img in &project.background_images {
        let meta_json =
            serde_json::to_string(img).map_err(|e| format!("Serialize bg image: {e}"))?;
        tx.execute(
            "INSERT INTO background_images (id, metadata, image_blob) VALUES (?1, ?2, ?3)",
            params![img.id.to_string(), meta_json, img.image_data],
        )
        .map_err(|e| format!("Write bg image failed: {e}"))?;
    }

    tx.commit().map_err(|e| format!("Commit failed: {e}"))?;

    log::info!("Saved project to {}", path.display());
    Ok(())
}

/// Load a project from a `.grove` SQLite file.
pub fn load_project(path: &Path) -> Result<Project, String> {
    let conn = Connection::open(path).map_err(|e| format!("Cannot open file: {e}"))?;

    // Check schema version
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM grove_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(v) = &version {
        let v: i32 = v.parse().unwrap_or(0);
        if v > SCHEMA_VERSION {
            return Err(format!(
                "File was created with a newer version (schema {v}, this app supports {SCHEMA_VERSION})"
            ));
        }
    }

    // Read metadata
    let metadata: ProjectMetadata = conn
        .query_row(
            "SELECT value FROM grove_meta WHERE key = 'project_metadata'",
            [],
            |row| {
                let json: String = row.get(0)?;
                Ok(json)
            },
        )
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    // Read active layer id
    let active_layer_id: uuid::Uuid = conn
        .query_row(
            "SELECT value FROM grove_meta WHERE key = 'active_layer_id'",
            [],
            |row| {
                let s: String = row.get(0)?;
                Ok(s)
            },
        )
        .ok()
        .and_then(|s| uuid::Uuid::parse_str(&s).ok())
        .unwrap_or(uuid::Uuid::nil());

    // Read layers
    let mut layers: Vec<Layer> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT data FROM layers")
        .map_err(|e| format!("Query layers: {e}"))?;
    let layer_iter = stmt
        .query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        })
        .map_err(|e| format!("Read layers: {e}"))?;
    for json_result in layer_iter {
        let json = json_result.map_err(|e| format!("Read layer row: {e}"))?;
        let layer: Layer =
            serde_json::from_str(&json).map_err(|e| format!("Deserialize layer: {e}"))?;
        layers.push(layer);
    }

    // If no layers were stored, create defaults
    if layers.is_empty() {
        layers = LayerManager::default_layers();
    }

    let layer_manager = LayerManager {
        active_layer_id: if layers.iter().any(|l| l.id == active_layer_id) {
            active_layer_id
        } else {
            layers[0].id
        },
        layers,
    };

    // Read entities
    let mut entities: Vec<Entity> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT data FROM entities")
        .map_err(|e| format!("Query entities: {e}"))?;
    let entity_iter = stmt
        .query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        })
        .map_err(|e| format!("Read entities: {e}"))?;
    for json_result in entity_iter {
        let json = json_result.map_err(|e| format!("Read entity row: {e}"))?;
        let entity: Entity =
            serde_json::from_str(&json).map_err(|e| format!("Deserialize entity: {e}"))?;
        entities.push(entity);
    }

    // Read background images (gracefully handle missing table for v1 files)
    let mut background_images: Vec<BackgroundImage> = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT metadata, image_blob FROM background_images") {
        let bg_iter = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((json, blob))
            })
            .map_err(|e| format!("Read bg images: {e}"))?;
        for row_result in bg_iter {
            let (json, blob) = row_result.map_err(|e| format!("Read bg image row: {e}"))?;
            let mut img: BackgroundImage =
                serde_json::from_str(&json).map_err(|e| format!("Deserialize bg image: {e}"))?;
            img.image_data = blob;
            background_images.push(img);
        }
    }

    // Read drawings (gracefully handle missing table for older files)
    let mut drawings: Vec<Drawing> = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT data FROM drawings") {
        let drawing_iter = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| format!("Read drawings: {e}"))?;
        for json_result in drawing_iter {
            let json = json_result.map_err(|e| format!("Read drawing row: {e}"))?;
            let drawing: Drawing =
                serde_json::from_str(&json).map_err(|e| format!("Deserialize drawing: {e}"))?;
            drawings.push(drawing);
        }
    }

    log::info!(
        "Loaded project from {} ({} layers, {} entities, {} bg images, {} drawings)",
        path.display(),
        layer_manager.layers.len(),
        entities.len(),
        background_images.len(),
        drawings.len(),
    );

    Ok(Project {
        metadata,
        entities,
        layers: layer_manager,
        background_images,
        drawings,
        dirty: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Geometry};
    use glam::Vec2;

    #[test]
    fn test_save_load_roundtrip() {
        let mut project = Project::new();
        project.metadata.name = "Test Garden".into();
        project.metadata.author = "Tester".into();

        let mut e = Entity::cad(
            "circle1",
            Geometry::Circle(CircleData {
                center: Vec2::new(100.0, 200.0),
                radius: 50.0,
            }),
        );
        e.layer_id = project.layers.layers[0].id;
        e.colour = [255, 0, 0, 255];
        project.add_entity(e);

        let dir = std::env::temp_dir();
        let path = dir.join("test_grove_roundtrip.grove");

        // Save
        save_project(&project, &path).unwrap();

        // Load
        let loaded = load_project(&path).unwrap();

        assert_eq!(loaded.metadata.name, "Test Garden");
        assert_eq!(loaded.metadata.author, "Tester");
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.entities[0].name, "circle1");
        assert_eq!(loaded.entities[0].colour, [255, 0, 0, 255]);
        assert_eq!(loaded.layers.layers.len(), project.layers.layers.len());
        assert!(!loaded.dirty);

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_background_image_roundtrip() {
        use crate::background_image::{BackgroundImage, ColourMode};

        let mut project = Project::new();

        // Create a tiny 2x2 PNG
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        let pixels: [u8; 16] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        image::ImageEncoder::write_image(
            encoder,
            &pixels,
            2,
            2,
            image::ColorType::Rgba8.into(),
        )
        .unwrap();

        let mut img = BackgroundImage::from_bytes("survey.png".into(), buf).unwrap();
        img.opacity = 0.5;
        img.colour_mode = ColourMode::Desaturated;
        img.position = [100.0, 200.0];
        img.rotation_rad = 0.25;
        let img_id = img.id;
        project.add_background_image(img);

        let dir = std::env::temp_dir();
        let path = dir.join("test_grove_bg_roundtrip.grove");

        save_project(&project, &path).unwrap();
        let loaded = load_project(&path).unwrap();

        assert_eq!(loaded.background_images.len(), 1);
        let loaded_img = &loaded.background_images[0];
        assert_eq!(loaded_img.id, img_id);
        assert_eq!(loaded_img.name, "survey.png");
        assert_eq!(loaded_img.width_px, 2);
        assert_eq!(loaded_img.height_px, 2);
        assert!((loaded_img.opacity - 0.5).abs() < 0.01);
        assert_eq!(loaded_img.colour_mode, ColourMode::Desaturated);
        assert!((loaded_img.position[0] - 100.0).abs() < 0.01);
        assert!((loaded_img.rotation_rad - 0.25).abs() < 0.01);
        // Verify image data survived
        assert!(!loaded_img.image_data.is_empty());
        let (decoded, w, h) = loaded_img.decode_to_rgba().unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(decoded.len(), 16);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_drawing_roundtrip() {
        let mut project = Project::new();
        let mut drawing = Drawing::new("01", "Planting Plan");
        drawing.viewports.push(
            crate::drawing::drawing::DrawingViewport::new("Main", 100),
        );
        drawing.add_revision("First draft", "AB");
        let drawing_id = drawing.id;
        project.add_drawing(drawing);

        let dir = std::env::temp_dir();
        let path = dir.join("test_grove_drawing_roundtrip.grove");

        save_project(&project, &path).unwrap();
        let loaded = load_project(&path).unwrap();

        assert_eq!(loaded.drawings.len(), 1);
        let d = &loaded.drawings[0];
        assert_eq!(d.id, drawing_id);
        assert_eq!(d.title, "Planting Plan");
        assert_eq!(d.viewports.len(), 1);
        assert_eq!(d.revisions.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
