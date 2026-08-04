//! Global keyboard shortcuts, handled before panels consume key events.

use super::*;

impl GroveApp {
    pub(super) fn handle_global_shortcuts(&mut self, ctx: &Context) {
        let shift = ctx.input(|i| i.modifiers.shift);
        let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);

        // Global keyboard shortcuts (before panels consume keys)
        if ctrl && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
            if shift {
                self.history.redo(&mut self.project);
            } else {
                self.history.undo(&mut self.project);
            }
        }
        if ctrl && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            self.history.redo(&mut self.project);
        }

        // File shortcuts
        if ctrl && !shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            if let Err(e) = self.save() {
                log::error!("Save failed: {e}");
            }
        }
        if ctrl && shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            if let Err(e) = self.save_as() {
                log::error!("Save As failed: {e}");
            }
        }
        if ctrl && ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.open_file();
        }
        if ctrl && ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_project();
        }

        // Clipboard shortcuts — egui_winit converts Ctrl+C/V/X into Event::Copy/Paste/Cut
        // and strips the raw key presses, so we intercept those events directly.
        let mut had_copy = false;
        let mut had_cut = false;
        let mut had_paste = false;
        ctx.input_mut(|i| {
            i.events.retain(|event| match event {
                egui::Event::Copy => { had_copy = true; false }
                egui::Event::Cut => { had_cut = true; false }
                egui::Event::Paste(_) => { had_paste = true; false }
                _ => true,
            });
        });
        if had_copy {
            self.copy_selection_to_clipboard();
        }
        if had_paste {
            self.paste_from_clipboard();
        }
        if had_cut {
            self.copy_selection_to_clipboard();
            let ids: Vec<_> = self.selection.ids.clone();
            if !ids.is_empty() {
                let commands: Vec<Box<dyn grove_core::commands::Command>> = ids
                    .iter()
                    .map(|&id| -> Box<dyn grove_core::commands::Command> {
                        Box::new(grove_core::commands::RemoveEntityCommand::new(id))
                    })
                    .collect();
                let cmd = Box::new(grove_core::commands::CompoundCommand::new(
                    commands,
                    "Cut entities",
                ));
                self.history.execute(cmd, &mut self.project);
            }
            self.selection.clear();
        }
        // Ctrl+D for duplicate (not intercepted by egui_winit, so use key_pressed)
        if ctrl && !shift && ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.duplicate_selection();
        }
    }
}
