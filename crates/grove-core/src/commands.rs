use std::fmt::Debug;

use crate::drawing::drawing::Drawing;
use crate::entity::{DomainData, Entity, Geometry};
use crate::project::Project;
use uuid::Uuid;

/// A command represents an atomic, undoable mutation to the project.
pub trait Command: Debug + Send {
    fn execute(&mut self, project: &mut Project);
    fn undo(&mut self, project: &mut Project);
    fn description(&self) -> &str;
}

/// Manages the undo/redo stack.
#[derive(Debug)]
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn execute(&mut self, mut cmd: Box<dyn Command>, project: &mut Project) {
        cmd.execute(project);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, project: &mut Project) -> bool {
        if let Some(mut cmd) = self.undo_stack.pop() {
            cmd.undo(project);
            self.redo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, project: &mut Project) -> bool {
        if let Some(mut cmd) = self.redo_stack.pop() {
            cmd.execute(project);
            self.undo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|c| c.description())
    }

    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|c| c.description())
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in commands
// ---------------------------------------------------------------------------

/// Add an entity to the project.
#[derive(Debug)]
pub struct AddEntityCommand {
    entity: Option<Entity>,
    entity_id: Uuid,
}

impl AddEntityCommand {
    pub fn new(entity: Entity) -> Self {
        let id = entity.id;
        Self {
            entity: Some(entity),
            entity_id: id,
        }
    }
}

impl Command for AddEntityCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = self.entity.take() {
            project.add_entity(entity);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.remove_entity(self.entity_id) {
            self.entity = Some(entity);
        }
    }

    fn description(&self) -> &str {
        "Add entity"
    }
}

/// Remove an entity from the project.
#[derive(Debug)]
pub struct RemoveEntityCommand {
    entity_id: Uuid,
    removed: Option<Entity>,
}

impl RemoveEntityCommand {
    pub fn new(entity_id: Uuid) -> Self {
        Self {
            entity_id,
            removed: None,
        }
    }
}

impl Command for RemoveEntityCommand {
    fn execute(&mut self, project: &mut Project) {
        self.removed = project.remove_entity(self.entity_id);
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = self.removed.take() {
            project.add_entity(entity);
        }
    }

    fn description(&self) -> &str {
        "Remove entity"
    }
}

/// Move an entity by a delta.
#[derive(Debug)]
pub struct MoveEntityCommand {
    entity_id: Uuid,
    delta: glam::Vec2,
}

impl MoveEntityCommand {
    pub fn new(entity_id: Uuid, delta: glam::Vec2) -> Self {
        Self { entity_id, delta }
    }
}

impl Command for MoveEntityCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.translate(self.delta);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.translate(-self.delta);
        }
    }

    fn description(&self) -> &str {
        "Move entity"
    }
}

/// Replace an entity's geometry (used for bezier point editing, etc.).
#[derive(Debug)]
pub struct UpdateGeometryCommand {
    entity_id: Uuid,
    old_geometry: Option<Geometry>,
    new_geometry: Option<Geometry>,
}

impl UpdateGeometryCommand {
    pub fn new(entity_id: Uuid, old_geometry: Geometry, new_geometry: Geometry) -> Self {
        Self {
            entity_id,
            old_geometry: Some(old_geometry),
            new_geometry: Some(new_geometry),
        }
    }
}

impl Command for UpdateGeometryCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            if let Some(new_geom) = self.new_geometry.take() {
                self.old_geometry = Some(std::mem::replace(&mut entity.geometry, new_geom));
            }
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            if let Some(old_geom) = self.old_geometry.take() {
                self.new_geometry = Some(std::mem::replace(&mut entity.geometry, old_geom));
            }
        }
    }

    fn description(&self) -> &str {
        "Edit curve points"
    }
}

/// Add multiple entities as a single undoable operation (e.g. plant groups).
#[derive(Debug)]
pub struct AddMultipleEntitiesCommand {
    entities: Option<Vec<Entity>>,
    entity_ids: Vec<Uuid>,
}

impl AddMultipleEntitiesCommand {
    pub fn new(entities: Vec<Entity>) -> Self {
        let ids = entities.iter().map(|e| e.id).collect();
        Self {
            entities: Some(entities),
            entity_ids: ids,
        }
    }
}

impl Command for AddMultipleEntitiesCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entities) = self.entities.take() {
            for entity in entities {
                project.add_entity(entity);
            }
        }
    }

    fn undo(&mut self, project: &mut Project) {
        let mut removed = Vec::new();
        // Remove in reverse order to maintain consistency
        for &id in self.entity_ids.iter().rev() {
            if let Some(entity) = project.remove_entity(id) {
                removed.push(entity);
            }
        }
        removed.reverse();
        self.entities = Some(removed);
    }

    fn description(&self) -> &str {
        "Place plant group"
    }
}

/// Rotate an entity around a center point.
#[derive(Debug)]
pub struct RotateEntityCommand {
    entity_id: Uuid,
    center: glam::Vec2,
    angle: f32,
}

impl RotateEntityCommand {
    pub fn new(entity_id: Uuid, center: glam::Vec2, angle: f32) -> Self {
        Self { entity_id, center, angle }
    }
}

impl Command for RotateEntityCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.rotate(self.center, self.angle);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.rotate(self.center, -self.angle);
        }
    }

    fn description(&self) -> &str {
        "Rotate entity"
    }
}

/// Scale an entity from a center point.
#[derive(Debug)]
pub struct ScaleEntityCommand {
    entity_id: Uuid,
    center: glam::Vec2,
    factor: f32,
}

impl ScaleEntityCommand {
    pub fn new(entity_id: Uuid, center: glam::Vec2, factor: f32) -> Self {
        Self { entity_id, center, factor }
    }
}

impl Command for ScaleEntityCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.scale(self.center, self.factor);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            entity.geometry.scale(self.center, 1.0 / self.factor);
        }
    }

    fn description(&self) -> &str {
        "Scale entity"
    }
}

/// Wraps multiple commands into a single undo/redo step.
#[derive(Debug)]
pub struct CompoundCommand {
    commands: Vec<Box<dyn Command>>,
    description: String,
}

impl CompoundCommand {
    pub fn new(commands: Vec<Box<dyn Command>>, description: impl Into<String>) -> Self {
        Self {
            commands,
            description: description.into(),
        }
    }
}

impl Command for CompoundCommand {
    fn execute(&mut self, project: &mut Project) {
        for cmd in &mut self.commands {
            cmd.execute(project);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(project);
        }
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Update an entity's domain data (for editable properties panel).
#[derive(Debug)]
pub struct UpdateDomainDataCommand {
    entity_id: Uuid,
    old_domain: Option<DomainData>,
    new_domain: Option<DomainData>,
}

impl UpdateDomainDataCommand {
    pub fn new(entity_id: Uuid, old_domain: DomainData, new_domain: DomainData) -> Self {
        Self {
            entity_id,
            old_domain: Some(old_domain),
            new_domain: Some(new_domain),
        }
    }
}

impl Command for UpdateDomainDataCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            if let Some(new_dom) = self.new_domain.take() {
                self.old_domain = Some(std::mem::replace(&mut entity.domain, new_dom));
            }
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(entity) = project.find_entity_mut(self.entity_id) {
            if let Some(old_dom) = self.old_domain.take() {
                self.new_domain = Some(std::mem::replace(&mut entity.domain, old_dom));
            }
        }
    }

    fn description(&self) -> &str {
        "Update properties"
    }
}

// ---------------------------------------------------------------------------
// Drawing commands
// ---------------------------------------------------------------------------

/// Add a drawing to the project.
#[derive(Debug)]
pub struct AddDrawingCommand {
    drawing: Option<Drawing>,
    drawing_id: Uuid,
}

impl AddDrawingCommand {
    pub fn new(drawing: Drawing) -> Self {
        let id = drawing.id;
        Self {
            drawing: Some(drawing),
            drawing_id: id,
        }
    }
}

impl Command for AddDrawingCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(drawing) = self.drawing.take() {
            project.add_drawing(drawing);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(drawing) = project.remove_drawing(self.drawing_id) {
            self.drawing = Some(drawing);
        }
    }

    fn description(&self) -> &str {
        "Add drawing"
    }
}

/// Remove a drawing from the project.
#[derive(Debug)]
pub struct RemoveDrawingCommand {
    drawing_id: Uuid,
    removed: Option<Drawing>,
}

impl RemoveDrawingCommand {
    pub fn new(drawing_id: Uuid) -> Self {
        Self {
            drawing_id,
            removed: None,
        }
    }
}

impl Command for RemoveDrawingCommand {
    fn execute(&mut self, project: &mut Project) {
        self.removed = project.remove_drawing(self.drawing_id);
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(drawing) = self.removed.take() {
            project.add_drawing(drawing);
        }
    }

    fn description(&self) -> &str {
        "Remove drawing"
    }
}

/// Update a drawing (stores old/new snapshots).
#[derive(Debug)]
pub struct UpdateDrawingCommand {
    drawing_id: Uuid,
    old_drawing: Option<Drawing>,
    new_drawing: Option<Drawing>,
}

impl UpdateDrawingCommand {
    pub fn new(drawing_id: Uuid, old_drawing: Drawing, new_drawing: Drawing) -> Self {
        Self {
            drawing_id,
            old_drawing: Some(old_drawing),
            new_drawing: Some(new_drawing),
        }
    }
}

impl Command for UpdateDrawingCommand {
    fn execute(&mut self, project: &mut Project) {
        if let Some(new_drawing) = self.new_drawing.take() {
            if let Some(existing) = project.find_drawing_mut(self.drawing_id) {
                self.old_drawing = Some(std::mem::replace(existing, new_drawing));
            }
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(old_drawing) = self.old_drawing.take() {
            if let Some(existing) = project.find_drawing_mut(self.drawing_id) {
                self.new_drawing = Some(std::mem::replace(existing, old_drawing));
            }
        }
    }

    fn description(&self) -> &str {
        "Update drawing"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleData, Geometry};
    use glam::Vec2;

    fn make_entity() -> Entity {
        Entity::cad(
            "test",
            Geometry::Circle(CircleData {
                center: Vec2::ZERO,
                radius: 50.0,
            }),
        )
    }

    #[test]
    fn test_add_undo_redo() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let e = make_entity();
        let id = e.id;

        history.execute(Box::new(AddEntityCommand::new(e)), &mut project);
        assert_eq!(project.entity_count(), 1);

        history.undo(&mut project);
        assert_eq!(project.entity_count(), 0);

        history.redo(&mut project);
        assert_eq!(project.entity_count(), 1);
        assert!(project.find_entity(id).is_some());
    }

    #[test]
    fn test_move_undo_roundtrip() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let e = make_entity();
        let id = e.id;
        let original_center = if let Geometry::Circle(ref c) = e.geometry {
            c.center
        } else {
            panic!();
        };

        history.execute(Box::new(AddEntityCommand::new(e)), &mut project);
        let delta = Vec2::new(100.0, 50.0);
        history.execute(Box::new(MoveEntityCommand::new(id, delta)), &mut project);

        let moved = &project.find_entity(id).unwrap().geometry;
        if let Geometry::Circle(c) = moved {
            assert!((c.center.x - (original_center.x + 100.0)).abs() < 0.01);
        }

        history.undo(&mut project);
        let restored = &project.find_entity(id).unwrap().geometry;
        if let Geometry::Circle(c) = restored {
            assert!((c.center.x - original_center.x).abs() < 0.01);
        }
    }

    #[test]
    fn test_rotate_undo_roundtrip() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let e = Entity::cad(
            "test",
            Geometry::Point(Vec2::new(10.0, 0.0)),
        );
        let id = e.id;
        history.execute(Box::new(AddEntityCommand::new(e)), &mut project);

        let angle = std::f32::consts::FRAC_PI_2;
        history.execute(
            Box::new(RotateEntityCommand::new(id, Vec2::ZERO, angle)),
            &mut project,
        );

        if let Geometry::Point(p) = &project.find_entity(id).unwrap().geometry {
            assert!((p.x).abs() < 0.01);
            assert!((p.y - 10.0).abs() < 0.01);
        }

        history.undo(&mut project);
        if let Geometry::Point(p) = &project.find_entity(id).unwrap().geometry {
            assert!((p.x - 10.0).abs() < 0.01);
            assert!((p.y).abs() < 0.01);
        }
    }

    #[test]
    fn test_compound_command_undo() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let e1 = make_entity();
        let e2 = make_entity();
        let id1 = e1.id;
        let id2 = e2.id;
        history.execute(Box::new(AddEntityCommand::new(e1)), &mut project);
        history.execute(Box::new(AddEntityCommand::new(e2)), &mut project);

        let delta = Vec2::new(50.0, 0.0);
        let compound = CompoundCommand::new(
            vec![
                Box::new(MoveEntityCommand::new(id1, delta)),
                Box::new(MoveEntityCommand::new(id2, delta)),
            ],
            "Move multiple",
        );
        history.execute(Box::new(compound), &mut project);

        // Both moved
        if let Geometry::Circle(c) = &project.find_entity(id1).unwrap().geometry {
            assert!((c.center.x - 50.0).abs() < 0.01);
        }
        if let Geometry::Circle(c) = &project.find_entity(id2).unwrap().geometry {
            assert!((c.center.x - 50.0).abs() < 0.01);
        }

        // Single undo reverts both
        history.undo(&mut project);
        if let Geometry::Circle(c) = &project.find_entity(id1).unwrap().geometry {
            assert!((c.center.x).abs() < 0.01);
        }
        if let Geometry::Circle(c) = &project.find_entity(id2).unwrap().geometry {
            assert!((c.center.x).abs() < 0.01);
        }
    }

    #[test]
    fn test_redo_cleared_on_new_command() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        history.execute(Box::new(AddEntityCommand::new(make_entity())), &mut project);
        history.undo(&mut project);
        assert!(history.can_redo());

        history.execute(Box::new(AddEntityCommand::new(make_entity())), &mut project);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_add_drawing_undo_redo() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let d = Drawing::new("01", "Test Drawing");
        let id = d.id;
        history.execute(Box::new(AddDrawingCommand::new(d)), &mut project);
        assert_eq!(project.drawings.len(), 1);

        history.undo(&mut project);
        assert_eq!(project.drawings.len(), 0);

        history.redo(&mut project);
        assert_eq!(project.drawings.len(), 1);
        assert!(project.find_drawing(id).is_some());
    }

    #[test]
    fn test_remove_drawing_undo() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let d = Drawing::new("01", "Test");
        let id = d.id;
        project.add_drawing(d);

        history.execute(Box::new(RemoveDrawingCommand::new(id)), &mut project);
        assert_eq!(project.drawings.len(), 0);

        history.undo(&mut project);
        assert_eq!(project.drawings.len(), 1);
    }

    #[test]
    fn test_update_drawing_undo() {
        let mut project = Project::new();
        let mut history = CommandHistory::new();

        let d = Drawing::new("01", "Original");
        let id = d.id;
        project.add_drawing(d.clone());

        let mut updated = d;
        updated.title = "Updated".into();
        history.execute(
            Box::new(UpdateDrawingCommand::new(id, project.find_drawing(id).unwrap().clone(), updated)),
            &mut project,
        );
        assert_eq!(project.find_drawing(id).unwrap().title, "Updated");

        history.undo(&mut project);
        assert_eq!(project.find_drawing(id).unwrap().title, "Original");
    }
}
