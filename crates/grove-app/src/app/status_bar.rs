//! Bottom status bar: tool, cursor position, snap, zoom, layer, hints.

use super::*;

impl GroveApp {
    pub(super) fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let shift = ctx.input(|i| i.modifiers.shift);

        Panel::bottom("status_bar")
            .exact_size(24.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 0.0, ThemeColours::STATUS_BAR);
                    ui.spacing_mut().item_spacing.x = 16.0;

                    ui.colored_label(
                        ThemeColours::TEXT_SECONDARY,
                        format!("Tool: {}", self.active_tool_type.label()),
                    );

                    if let Some(pos) = self.cursor_world_pos {
                        ui.colored_label(
                            ThemeColours::TEXT_SECONDARY,
                            format!("x:{:.0} y:{:.0}", pos.x, pos.y),
                        );
                    }

                    if let Some(ref label) = self.snap_label {
                        ui.colored_label(ThemeColours::SNAP_INDICATOR, label);
                    }

                    ui.colored_label(
                        ThemeColours::TEXT_DIM,
                        format!("Zoom: {:.0}%", self.canvas.viewport.zoom * 100.0),
                    );

                    if let Some(layer) = self.project.layers.active_layer() {
                        ui.colored_label(
                            ThemeColours::TEXT_DIM,
                            format!("Layer: {}", layer.name),
                        );
                    }

                    ui.colored_label(
                        ThemeColours::TEXT_DIM,
                        format!("Entities: {}", self.project.entity_count()),
                    );

                    if !self.selection.is_empty() {
                        ui.colored_label(
                            ThemeColours::ACCENT,
                            format!("Selected: {}", self.selection.ids.len()),
                        );
                    }

                    if let Some(ref cal) = self.calibrate_state {
                        let label = match &cal.mode {
                            AdjustMode::CalibrateScale { .. } => "CALIBRATE SCALE",
                            AdjustMode::AlignHorizontal { .. } => "ALIGN ROTATION",
                            AdjustMode::SetOrigin => "SET ORIGIN",
                        };
                        ui.colored_label(ThemeColours::ACCENT, label);
                    }

                    if self.snap_config.enabled {
                        ui.colored_label(ThemeColours::TEXT_DIM, "SNAP");
                    }

                    if shift && self.draw_tool.is_some() {
                        ui.colored_label(ThemeColours::ACCENT, "ORTHO");
                    }

                    if let Some(anchor) = self.anchor_point {
                        ui.colored_label(
                            ThemeColours::ACCENT,
                            format!("Anchor: x:{:.0} y:{:.0}", anchor.x, anchor.y),
                        );
                    }

                    // Tool-specific status hints
                    if let Some(ref tool) = self.draw_tool {
                        if let Some(modify) = tool.as_any_ref().downcast_ref::<crate::tools::modify::ModifyTool>() {
                            ui.colored_label(ThemeColours::ACCENT, modify.status_hint());
                        }
                        if let Some(array) = tool.as_any_ref().downcast_ref::<crate::tools::array::LinearArrayTool>() {
                            ui.colored_label(ThemeColours::ACCENT, array.status_hint());
                        }
                    }

                    // Curve mode hint for drawing tools
                    match self.active_tool_type {
                        ActiveToolType::DrawLine
                        | ActiveToolType::PlaceHardscape
                        | ActiveToolType::SubtractPolygon => {
                            if let Some(ref tool) = self.draw_tool {
                                // Check if the drawer is in curve mode by checking the tool name
                                // and inspecting the polygon_drawer state via downcast
                                let curve_hint = if self.active_tool_type == ActiveToolType::PlaceHardscape {
                                    tool.as_any_ref()
                                        .downcast_ref::<crate::tools::place_hardscape::PlaceHardscapeTool>()
                                        .and_then(|ht| {
                                            // Only show curve hint when in line/polygon drawing sub-mode
                                            match ht.shape_override {
                                                Some(crate::tools::place_hardscape::ShapeOverride::Circle)
                                                | Some(crate::tools::place_hardscape::ShapeOverride::Rectangle) => None,
                                                _ => Some("'C' curve mode"),
                                            }
                                        })
                                } else if self.active_tool_type == ActiveToolType::SubtractPolygon {
                                    let cutout_shape = self.draw_tool.as_ref().and_then(|t| {
                                        t.as_any_ref()
                                            .downcast_ref::<crate::tools::subtract_polygon::SubtractPolygonTool>()
                                            .map(|st| st.cutout_shape)
                                    });
                                    match cutout_shape {
                                        Some(crate::tools::subtract_polygon::CutoutShape::Circle) => Some("Click center, click edge"),
                                        Some(crate::tools::subtract_polygon::CutoutShape::Rectangle) => Some("Click corner, click opposite"),
                                        _ => Some("Draw cutout  'C' curve  Dbl-click finish"),
                                    }
                                } else {
                                    Some("'C' curve mode")
                                };
                                if let Some(hint) = curve_hint {
                                    ui.colored_label(ThemeColours::TEXT_DIM, hint);
                                }
                            }
                        }
                        _ => {}
                    }

                    // Hint for polygon selection: press '-' to cut
                    if self.active_tool_type == ActiveToolType::Select {
                        if let Some(sel_id) = self.selection.single() {
                            let is_polygon = self.project.find_entity(sel_id).map(|e| {
                                matches!(&e.geometry, Geometry::Polygon(_))
                                    || matches!(&e.geometry, Geometry::BezierPath(bp) if bp.closed)
                            }).unwrap_or(false);
                            if is_polygon {
                                ui.colored_label(ThemeColours::TEXT_DIM, "'-' cutout hole");
                            }
                        }
                    }
                });
            });
    }
}
