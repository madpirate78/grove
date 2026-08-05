//! Clipboard and selection edit operations: copy, cut, paste, duplicate, delete.

use super::*;

impl GroveApp {
    /// Delete the current selection as one undoable compound command.
    pub(super) fn delete_selection(&mut self, label: &str) {
        let ids: Vec<_> = self.selection.ids.clone();
        if !ids.is_empty() {
            let commands: Vec<Box<dyn grove_core::commands::Command>> = ids
                .iter()
                .map(|&id| -> Box<dyn grove_core::commands::Command> {
                    Box::new(grove_core::commands::RemoveEntityCommand::new(id))
                })
                .collect();
            let cmd = Box::new(grove_core::commands::CompoundCommand::new(commands, label));
            self.history.execute(cmd, &mut self.project);
        }
        self.selection.clear();
    }

    pub(super) fn cut_selection(&mut self) {
        self.copy_selection_to_clipboard();
        self.delete_selection("Cut entities");
    }

    /// Recursively assign new UUIDs to an entity and all its children.
    pub(super) fn assign_new_ids(entity: &mut grove_core::entity::Entity) {
        entity.id = uuid::Uuid::new_v4();
        for child in &mut entity.children {
            Self::assign_new_ids(child);
        }
    }

    pub(super) fn copy_selection_to_clipboard(&mut self) {
        self.clipboard = self
            .selection
            .ids
            .iter()
            .filter_map(|id| self.project.entities.iter().find(|e| e.id == *id).cloned())
            .collect();
    }

    pub(super) fn paste_from_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let offset = glam::Vec2::new(50.0, 50.0);
        let mut new_entities = Vec::new();
        for ent in &self.clipboard {
            let mut clone = ent.clone();
            Self::assign_new_ids(&mut clone);
            clone.locked = false;
            clone.geometry.translate(offset);
            new_entities.push(clone);
        }
        let new_ids: Vec<uuid::Uuid> = new_entities.iter().map(|e| e.id).collect();
        let cmd = Box::new(grove_core::commands::AddMultipleEntitiesCommand::new(
            new_entities,
        ));
        self.history.execute(cmd, &mut self.project);
        self.selection.clear();
        for id in new_ids {
            self.selection.add(id);
        }
    }

    pub(super) fn duplicate_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let offset = glam::Vec2::new(50.0, 50.0);
        let selected: Vec<grove_core::entity::Entity> = self
            .selection
            .ids
            .iter()
            .filter_map(|id| self.project.entities.iter().find(|e| e.id == *id).cloned())
            .collect();
        let mut new_entities = Vec::new();
        for ent in selected {
            let mut clone = ent;
            Self::assign_new_ids(&mut clone);
            clone.locked = false;
            clone.geometry.translate(offset);
            new_entities.push(clone);
        }
        let new_ids: Vec<uuid::Uuid> = new_entities.iter().map(|e| e.id).collect();
        let cmd = Box::new(grove_core::commands::AddMultipleEntitiesCommand::new(
            new_entities,
        ));
        self.history.execute(cmd, &mut self.project);
        self.selection.clear();
        for id in new_ids {
            self.selection.add(id);
        }
    }
}
