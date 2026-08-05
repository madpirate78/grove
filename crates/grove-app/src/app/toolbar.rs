//! Left two-column tool palette (mode buttons + shape sub-tools).

use super::*;

impl GroveApp {
    pub(super) fn show_toolbar(&mut self, ctx: &Context) {
        // Left toolbar (hidden in Drawing mode) — two-column layout
        if matches!(self.workspace_mode, WorkspaceMode::Model) {
        SidePanel::left("toolbar")
            .exact_width(130.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Gather context for column 2 before entering closure
                let hardscape_tool_ref = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::place_hardscape::PlaceHardscapeTool>()
                });
                let level_sub_type = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::place_level::PlaceLevelTool>()
                        .map(|lt| lt.sub_type)
                });
                let shapes = available_shapes(self.tool_mode, hardscape_tool_ref, level_sub_type);

                let blue = Color32::from_rgb(60, 90, 140);
                let green = Color32::from_rgb(50, 120, 60);
                let red = Color32::from_rgb(150, 50, 50);

                // Current mode colour for column 2 buttons
                let mode_colour = match self.tool_mode {
                    ToolMode::Select | ToolMode::Move | ToolMode::Rotate | ToolMode::Shape => Some(blue),
                    ToolMode::Hardscape | ToolMode::Plant | ToolMode::Group | ToolMode::Level | ToolMode::Entourage => Some(green),
                    ToolMode::Cutout => Some(red),
                };

                // Determine which cutout shape is active (for column 2 highlighting)
                let current_cutout_shape = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::subtract_polygon::SubtractPolygonTool>()
                        .map(|st| st.cutout_shape)
                });

                // Determine current hardscape shape override (for column 2 highlighting)
                let current_hardscape_override = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::place_hardscape::PlaceHardscapeTool>()
                        .and_then(|ht| ht.shape_override)
                });

                // Track click actions to apply after the UI closure
                let mut mode_action: Option<ToolMode> = None;
                let mut delete_action = false;
                let mut shape_action: Option<ShapeTool> = None;
                let has_sel = !self.selection.is_empty();

                crate::widgets::toolbar_two_column(ui,
                    // ─── Column 1: Mode buttons ───
                    |ui| {
                        // Select
                        if crate::widgets::tool_button(ui, "Select", self.tool_mode == ToolMode::Select).clicked() {
                            mode_action = Some(ToolMode::Select);
                        }
                        // Move
                        if crate::widgets::tool_button_enabled(ui, "Move", self.tool_mode == ToolMode::Move, None, has_sel).clicked() {
                            mode_action = Some(ToolMode::Move);
                        }
                        // Rotate
                        if crate::widgets::tool_button_enabled(ui, "Rotate", self.tool_mode == ToolMode::Rotate, None, has_sel).clicked() {
                            mode_action = Some(ToolMode::Rotate);
                        }
                        // Delete (action button, not a mode)
                        if crate::widgets::tool_button_enabled(ui, "Delete", false, None, has_sel).clicked() {
                            delete_action = true;
                        }

                        crate::widgets::tool_separator(ui);

                        // Shape
                        if crate::widgets::tool_button(ui, "Shape", self.tool_mode == ToolMode::Shape).clicked() {
                            mode_action = Some(ToolMode::Shape);
                        }
                        // Hardscape
                        if crate::widgets::tool_button_colored(ui, "Hardsc.", self.tool_mode == ToolMode::Hardscape, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Hardscape);
                        }
                        // Plant
                        if crate::widgets::tool_button_colored(ui, "Plant", self.tool_mode == ToolMode::Plant, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Plant);
                        }
                        // Group
                        if crate::widgets::tool_button_colored(ui, "Group", self.tool_mode == ToolMode::Group, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Group);
                        }
                        // Level
                        if crate::widgets::tool_button_colored(ui, "Level", self.tool_mode == ToolMode::Level, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Level);
                        }
                        // Entourage
                        if crate::widgets::tool_button_colored(ui, "Entour.", self.tool_mode == ToolMode::Entourage, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Entourage);
                        }

                        crate::widgets::tool_separator(ui);

                        // Cutout
                        if crate::widgets::tool_button_enabled(ui, "Cutout", self.tool_mode == ToolMode::Cutout, Some(red), has_sel).clicked() {
                            mode_action = Some(ToolMode::Cutout);
                        }
                    },
                    // ─── Column 2: Shape sub-tools ───
                    |ui| {
                        if shapes.is_empty() {
                            ui.add_space(4.0);
                            ui.colored_label(Color32::from_rgb(80, 80, 85), "—");
                            return;
                        }

                        for &shape in shapes {
                            let is_active = self.is_shape_active(shape, current_cutout_shape, current_hardscape_override);
                            if crate::widgets::tool_button_colored(
                                ui, shape.label(), is_active, mode_colour,
                            ).clicked() {
                                shape_action = Some(shape);
                            }
                        }
                    },
                );

                // ─── Apply actions (outside the closure to satisfy borrow checker) ───

                // Delete action
                if delete_action {
                    self.delete_selection("Delete entities");
                }

                // Mode change
                if let Some(new_mode) = mode_action {
                    self.apply_mode_change(new_mode);
                }

                // Shape sub-tool change
                if let Some(shape) = shape_action {
                    self.apply_shape_change(shape);
                }
            });
        }
    }
}
