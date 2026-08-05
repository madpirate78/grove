//! Central canvas: model-space rendering + input, and drawing-sheet mode.

use super::*;

impl GroveApp {
    pub(super) fn show_central_canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let shift = ctx.input(|i| i.modifiers.shift);
        let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);

        // Central canvas
        CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // Drawing mode: render drawing view with its own viewport
                if let WorkspaceMode::Drawing(drawing_id) = self.workspace_mode {
                    // Allocate the full panel for drawing mode
                    let available = ui.available_size();
                    let (response, painter) =
                        ui.allocate_painter(available, egui::Sense::click_and_drag());
                    let rect = response.rect;

                    // Dark desk background
                    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(60, 60, 65));

                    // Auto-fit page on first entry into this drawing
                    if self.drawing_viewport_fitted_for != Some(drawing_id) {
                        if let Some(drawing) = self.project.find_drawing(drawing_id) {
                            let (pw, ph) = drawing.paper_dimensions_mm();
                            self.drawing_viewport.fit_to_rect(
                                glam::Vec2::ZERO,
                                glam::Vec2::new(pw, ph),
                                rect.size(),
                                40.0,
                            );
                            self.drawing_viewport_fitted_for = Some(drawing_id);
                        }
                    }

                    // Right-drag or middle-drag always pans canvas
                    if response.dragged_by(egui::PointerButton::Secondary)
                        || response.dragged_by(egui::PointerButton::Middle)
                    {
                        self.drawing_viewport.pan(response.drag_delta());
                    }

                    // Shift+left-drag: pan viewport model_center if over a viewport
                    if response.dragged_by(egui::PointerButton::Primary) {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            let delta = response.drag_delta();
                            // Lock to a viewport on drag start
                            if self.drawing_interaction.panning_viewport.is_none() {
                                if let Some(hover) = response.hover_pos() {
                                    if let Some(drawing) = self.project.find_drawing(drawing_id) {
                                        if let Some((vp, _)) = crate::view::drawing_view::hit_test_viewport_body(
                                            drawing, &self.drawing_viewport, hover,
                                        ) {
                                            self.drawing_interaction.panning_viewport = Some(vp.id);
                                        }
                                    }
                                }
                            }

                            if let Some(vp_id) = self.drawing_interaction.panning_viewport {
                                // Read scale from viewport before mutating
                                let vp_scale = self.project.find_drawing(drawing_id)
                                    .and_then(|d| d.viewports.iter().find(|v| v.id == vp_id))
                                    .map(|vp| vp.scale);
                                if let Some(scale) = vp_scale {
                                    let zoom = self.drawing_viewport.zoom;
                                    // screen_delta → model delta: divide by (zoom / scale)
                                    let model_dx = delta.x * scale as f32 / zoom;
                                    let model_dy = delta.y * scale as f32 / zoom;
                                    if let Some(d) = self.project.find_drawing_mut(drawing_id) {
                                        if let Some(vp) = d.viewports.iter_mut().find(|v| v.id == vp_id) {
                                            vp.model_center[0] -= model_dx;
                                            vp.model_center[1] -= model_dy;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Clear panning_viewport when drag stops
                    if response.drag_stopped() {
                        self.drawing_interaction.panning_viewport = None;
                    }

                    // Zoom (scroll wheel) — only when pointer is over the central panel
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll_delta.abs() > 0.0 {
                        if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                            if rect.contains(pointer) {
                                let factor = if scroll_delta > 0.0 { 1.05 } else { 1.0 / 1.05 };
                                self.drawing_viewport.zoom_at(pointer, factor);
                            }
                        }
                    }

                    if let Some(drawing) = self.project.find_drawing(drawing_id) {
                        let view_actions = crate::view::drawing_view::draw_drawing_view(
                            ui, &self.project, drawing, &self.drawing_viewport,
                            &response, &mut self.drawing_interaction,
                        );
                        // Process drawing view actions
                        for action in view_actions {
                            match action {
                                crate::view::drawing_view::DrawingViewAction::UpdatedViewport(vp_id, new_vp) => {
                                    if let Some(d) = self.project.find_drawing_mut(drawing_id) {
                                        if let Some(vp) = d.viewports.iter_mut().find(|v| v.id == vp_id) {
                                            *vp = new_vp;
                                        }
                                    }
                                }
                                crate::view::drawing_view::DrawingViewAction::SelectViewport(vp_id) => {
                                    self.drawing_interaction.selected_viewport = vp_id;
                                }
                                crate::view::drawing_view::DrawingViewAction::CreatedAnnotation(ann) => {
                                    if let Some(d) = self.project.find_drawing_mut(drawing_id) {
                                        d.annotations.push(ann);
                                    }
                                }
                                crate::view::drawing_view::DrawingViewAction::UpdatedAnnotation(ann_id, new_ann) => {
                                    if let Some(d) = self.project.find_drawing_mut(drawing_id) {
                                        if let Some(a) = d.annotations.iter_mut().find(|a| a.id == ann_id) {
                                            *a = new_ann;
                                        }
                                    }
                                }
                                crate::view::drawing_view::DrawingViewAction::SelectAnnotation(ann_id) => {
                                    self.drawing_interaction.selected_annotation = ann_id;
                                }
                            }
                        }
                    }

                    let world_pos = response
                        .hover_pos()
                        .map(|p| self.drawing_viewport.screen_to_world(p));
                    self.cursor_world_pos = world_pos;
                    return;
                }

                let canvas_result = self.canvas.show(ui);

                // Render background images (behind entities)
                for img in &self.project.background_images {
                    self.bg_renderer.ensure_texture(&ctx, img);
                    self.bg_renderer
                        .draw(&canvas_result.painter, &self.canvas.viewport, img);
                }

                // Render entities
                let layers = &self.project.layers;
                self.canvas.draw_entities(
                    &canvas_result.painter,
                    &self.project.entities,
                    &self.selection.ids,
                    |layer_id| layers.is_visible(layer_id),
                );

                // Render entourage images (textured quads)
                self.entourage_renderer.draw(
                    &ctx,
                    &canvas_result.painter,
                    &self.canvas.viewport,
                    &self.project.entities,
                    &self.selection.ids,
                    &|layer_id| self.project.layers.is_visible(layer_id),
                );

                // Draw tool preview
                if let Some(ref tool) = self.draw_tool {
                    tool.draw_preview(&canvas_result.painter, &self.canvas.viewport);
                }

                // Draw anchor point marker
                if let Some(anchor) = self.anchor_point {
                    let screen_pos = self.canvas.viewport.world_to_screen(anchor);
                    let size = 10.0;
                    let anchor_color = Color32::from_rgb(255, 80, 50);
                    let stroke = egui::Stroke::new(1.5, anchor_color);
                    // Crosshair
                    canvas_result.painter.line_segment(
                        [egui::pos2(screen_pos.x - size, screen_pos.y),
                         egui::pos2(screen_pos.x + size, screen_pos.y)],
                        stroke,
                    );
                    canvas_result.painter.line_segment(
                        [egui::pos2(screen_pos.x, screen_pos.y - size),
                         egui::pos2(screen_pos.x, screen_pos.y + size)],
                        stroke,
                    );
                    // Circle
                    canvas_result.painter.circle_stroke(screen_pos, 6.0, stroke);
                }

                // Draw select tool preview (box select)
                if self.active_tool_type == ActiveToolType::Select {
                    // Box-select rectangle
                    if let Some(start) = self.box_select_start {
                        if let Some(cursor) = self.cursor_world_pos {
                            let min = glam::Vec2::new(start.x.min(cursor.x), start.y.min(cursor.y));
                            let max = glam::Vec2::new(start.x.max(cursor.x), start.y.max(cursor.y));
                            let s_min = self.canvas.viewport.world_to_screen(min);
                            let s_max = self.canvas.viewport.world_to_screen(max);
                            let rect = egui::Rect::from_min_max(s_min, s_max);
                            canvas_result.painter.rect_filled(
                                rect, 0.0,
                                Color32::from_rgba_unmultiplied(70, 130, 210, 30),
                            );
                            canvas_result.painter.rect_stroke(
                                rect, 0.0,
                                egui::Stroke::new(1.0, Color32::from_rgb(70, 130, 210)),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }

                    // Draw bezier edit handles for selected curve
                    if let Some(sel_id) = self.selection.single() {
                        self.draw_bezier_edit_handles(&canvas_result.painter, sel_id);
                    }
                }

                // --- Handle canvas input ---
                let world_pos = canvas_result
                    .response
                    .hover_pos()
                    .map(|p| self.canvas.viewport.screen_to_world(p));
                self.cursor_world_pos = world_pos;

                let Some(pos) = world_pos else { return };

                // --- Adjustment mode input (calibrate / align / set origin) ---
                if self.calibrate_state.is_some() {
                    let adjust_colour = Color32::from_rgb(255, 200, 50);

                    // Draw preview line/point
                    if let Some(ref cal) = self.calibrate_state {
                        match &cal.mode {
                            AdjustMode::CalibrateScale { first_point, awaiting_distance, .. } => {
                                if let Some(p1) = first_point {
                                    let s1 = self.canvas.viewport.world_to_screen(*p1);
                                    let s2 = if *awaiting_distance {
                                        self.canvas.viewport.world_to_screen(
                                            self.calibrate_second_point.unwrap_or(pos),
                                        )
                                    } else {
                                        self.canvas.viewport.world_to_screen(pos)
                                    };
                                    canvas_result.painter.line_segment(
                                        [s1, s2], egui::Stroke::new(2.0, adjust_colour),
                                    );
                                    canvas_result.painter.circle_filled(s1, 4.0, adjust_colour);
                                    if *awaiting_distance {
                                        canvas_result.painter.circle_filled(s2, 4.0, adjust_colour);
                                    }
                                }
                            }
                            AdjustMode::AlignHorizontal { first_point } => {
                                if let Some(p1) = first_point {
                                    let s1 = self.canvas.viewport.world_to_screen(*p1);
                                    let s2 = self.canvas.viewport.world_to_screen(pos);
                                    canvas_result.painter.line_segment(
                                        [s1, s2], egui::Stroke::new(2.0, adjust_colour),
                                    );
                                    canvas_result.painter.circle_filled(s1, 4.0, adjust_colour);
                                    // Draw a faint horizontal reference
                                    let s_horiz = egui::Pos2::new(s2.x, s1.y);
                                    canvas_result.painter.line_segment(
                                        [s1, s_horiz],
                                        egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 50, 100)),
                                    );
                                }
                            }
                            AdjustMode::SetOrigin => {
                                // Draw crosshair at cursor
                                let sp = self.canvas.viewport.world_to_screen(pos);
                                let len = 12.0;
                                canvas_result.painter.line_segment(
                                    [egui::Pos2::new(sp.x - len, sp.y), egui::Pos2::new(sp.x + len, sp.y)],
                                    egui::Stroke::new(2.0, adjust_colour),
                                );
                                canvas_result.painter.line_segment(
                                    [egui::Pos2::new(sp.x, sp.y - len), egui::Pos2::new(sp.x, sp.y + len)],
                                    egui::Stroke::new(2.0, adjust_colour),
                                );
                            }
                        }
                    }

                    // Handle clicks
                    if canvas_result.response.clicked_by(egui::PointerButton::Primary) {
                        let mut should_suppress = false;
                        if let Some(ref mut cal) = self.calibrate_state {
                            match &mut cal.mode {
                                AdjustMode::CalibrateScale { first_point, awaiting_distance, .. } => {
                                    if !*awaiting_distance {
                                        if first_point.is_none() {
                                            *first_point = Some(pos);
                                            should_suppress = true;
                                        } else {
                                            self.calibrate_second_point = Some(pos);
                                            *awaiting_distance = true;
                                            should_suppress = true;
                                        }
                                    }
                                }
                                AdjustMode::AlignHorizontal { first_point } => {
                                    if first_point.is_none() {
                                        *first_point = Some(pos);
                                        should_suppress = true;
                                    } else {
                                        // Second click: compute angle and apply rotation
                                        let p1 = first_point.unwrap();
                                        let dx = pos.x - p1.x;
                                        let dy = pos.y - p1.y;
                                        let angle = dy.atan2(dx);
                                        let img_id = cal.image_id;
                                        if let Some(img) = self.project.find_background_image_mut(img_id) {
                                            img.rotation_rad -= angle;
                                            self.project.dirty = true;
                                        }
                                        self.calibrate_state = None;
                                        return;
                                    }
                                }
                                AdjustMode::SetOrigin => {
                                    // Move the image so the clicked point becomes (0,0)
                                    let img_id = cal.image_id;
                                    if let Some(img) = self.project.find_background_image_mut(img_id) {
                                        img.position[0] -= pos.x;
                                        img.position[1] -= pos.y;
                                        self.project.dirty = true;
                                    }
                                    self.calibrate_state = None;
                                    return;
                                }
                            }
                        }
                        if should_suppress {
                            return;
                        }
                    }

                    // Escape cancels
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.calibrate_state = None;
                        self.calibrate_second_point = None;
                        return;
                    }

                    // Suppress normal tool input while waiting for clicks
                    let suppress = self.calibrate_state.as_ref().is_some_and(|c| {
                        match &c.mode {
                            AdjustMode::CalibrateScale { awaiting_distance, .. } => !awaiting_distance,
                            AdjustMode::AlignHorizontal { .. } => true,
                            AdjustMode::SetOrigin => true,
                        }
                    });
                    if suppress {
                        return;
                    }
                }

                // --- Bezier point editing (handled before tool_ctx to avoid borrow conflicts) ---
                if self.active_tool_type == ActiveToolType::Select && self.bezier_edit.is_some() {
                    // Ongoing drag — update the point position
                    if canvas_result.response.dragged_by(egui::PointerButton::Primary) {
                        self.apply_bezier_drag(pos);
                        return;
                    }

                    // Drag ended — commit the edit as an undoable command
                    if canvas_result.response.drag_stopped_by(egui::PointerButton::Primary)
                        || canvas_result.response.clicked_by(egui::PointerButton::Primary)
                    {
                        self.apply_bezier_drag(pos);
                        let edit = self.bezier_edit.take().unwrap();
                        if let Some(entity) = self.project.find_entity_mut(edit.entity_id) {
                            let new_geometry = entity.geometry.clone();
                            entity.geometry = edit.original_geometry.clone();
                            let cmd = UpdateGeometryCommand::new(
                                edit.entity_id,
                                edit.original_geometry,
                                new_geometry,
                            );
                            self.history.execute(Box::new(cmd), &mut self.project);
                        }
                        return;
                    }
                    return;
                }

                // --- Smart snap point dwell detection (runs for all tool modes) ---
                if self.snap_config.enabled {
                    let snap_tolerance = self.snap_config.snap_distance / self.canvas.viewport.zoom;
                    let candidates = crate::tools::snap_candidates_from_entities(
                        pos, &self.project.entities, &self.snap_config, &[],
                    );
                    let non_grid_snap = candidates.iter()
                        .filter(|s| !matches!(s.snap_type, grove_core::snap::SnapType::Grid))
                        .filter(|s| s.distance <= snap_tolerance)
                        .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
                        .copied();

                    if let Some(snap) = non_grid_snap {
                        let near_threshold = 1.0; // 1mm
                        if let Some(ref hover) = self.smart_hover {
                            if (snap.position - hover.position).length() < near_threshold {
                                if hover.started.elapsed() >= std::time::Duration::from_millis(1500) {
                                    let already_exists = self.smart_points.iter()
                                        .any(|sp| (sp.position - snap.position).length() < near_threshold);
                                    if !already_exists && self.smart_points.len() < 5 {
                                        self.smart_points.push(SmartPoint { position: snap.position });
                                    }
                                    self.smart_hover = None;
                                }
                            } else {
                                self.smart_hover = Some(SmartPointHover {
                                    position: snap.position,
                                    started: std::time::Instant::now(),
                                });
                            }
                        } else {
                            self.smart_hover = Some(SmartPointHover {
                                position: snap.position,
                                started: std::time::Instant::now(),
                            });
                        }
                    } else {
                        self.smart_hover = None;
                    }

                    if self.smart_hover.is_some() {
                        ctx.request_repaint();
                    }
                }

                // --- Draw extension lines + dwell indicator (always visible) ---
                if !self.smart_points.is_empty() {
                    let clip_rect = canvas_result.painter.clip_rect();
                    let cyan = egui::Color32::from_rgba_unmultiplied(0, 200, 220, 120);
                    let cyan_stroke = egui::Stroke::new(0.8, cyan);

                    for sp in &self.smart_points {
                        let screen = self.canvas.viewport.world_to_screen(sp.position);

                        canvas_result.painter.line_segment(
                            [
                                egui::pos2(clip_rect.left(), screen.y),
                                egui::pos2(clip_rect.right(), screen.y),
                            ],
                            cyan_stroke,
                        );
                        canvas_result.painter.line_segment(
                            [
                                egui::pos2(screen.x, clip_rect.top()),
                                egui::pos2(screen.x, clip_rect.bottom()),
                            ],
                            cyan_stroke,
                        );

                        let size = 5.0;
                        let diamond = vec![
                            egui::pos2(screen.x, screen.y - size),
                            egui::pos2(screen.x + size, screen.y),
                            egui::pos2(screen.x, screen.y + size),
                            egui::pos2(screen.x - size, screen.y),
                        ];
                        let cyan_solid = egui::Color32::from_rgb(0, 200, 220);
                        canvas_result.painter.add(egui::Shape::convex_polygon(
                            diamond,
                            egui::Color32::from_rgba_unmultiplied(0, 200, 220, 80),
                            egui::Stroke::new(1.5, cyan_solid),
                        ));
                    }
                }

                if let Some(ref hover) = self.smart_hover {
                    let elapsed = hover.started.elapsed();
                    if elapsed >= std::time::Duration::from_millis(300) {
                        let progress = (elapsed.as_secs_f32() - 0.3) / 1.2;
                        let progress = progress.clamp(0.0, 1.0);
                        let screen = self.canvas.viewport.world_to_screen(hover.position);
                        let radius = 4.0 + progress * 8.0;
                        let cyan = egui::Color32::from_rgba_unmultiplied(0, 200, 220, (120.0 * progress) as u8);
                        canvas_result.painter.circle_stroke(
                            screen,
                            radius,
                            egui::Stroke::new(1.5, cyan),
                        );
                    }
                }

                let smart_positions: Vec<glam::Vec2> = self.smart_points.iter().map(|sp| sp.position).collect();

                // Borrows entities; smart_points copied out so tools can't alias self
                let tool_ctx = ToolContext {
                    entities: &self.project.entities,
                    snap_config: &self.snap_config,
                    shift_held: shift,
                    active_layer_id: self.project.layers.active_layer_id,
                    viewport_zoom: self.canvas.viewport.zoom,
                    smart_points: &smart_positions,
                };

                // --- Draw tool input ---
                if self.draw_tool.is_some() {
                    // Must split borrow: take tool out, operate, put back
                    let mut tool = self.draw_tool.take().unwrap();

                    tool.on_mouse_move(pos, &tool_ctx);

                    let mut result = ToolResult::Continue;

                    // Double-click finishes (e.g. polyline, bezier)
                    if canvas_result.response.double_clicked_by(egui::PointerButton::Primary) {
                        result = tool.on_double_click(pos, &tool_ctx);
                    } else if canvas_result.response.drag_started_by(egui::PointerButton::Primary) {
                        // Drag start = mouse down (for bezier handle dragging)
                        result = tool.on_mouse_down(pos, &tool_ctx);
                    } else if canvas_result.response.drag_stopped_by(egui::PointerButton::Primary) {
                        // Drag stop = mouse up (finalizes bezier handles)
                        result = tool.on_mouse_up(pos, &tool_ctx);
                    } else if canvas_result.response.clicked_by(egui::PointerButton::Primary) {
                        // Simple click (no drag) = mouse down + immediate up
                        result = tool.on_mouse_down(pos, &tool_ctx);
                        if matches!(result, ToolResult::Continue) {
                            result = tool.on_mouse_up(pos, &tool_ctx);
                        }
                    }

                    // Right-click
                    if matches!(result, ToolResult::Continue)
                        && canvas_result.response.clicked_by(egui::PointerButton::Secondary)
                    {
                        result = tool.on_right_click(pos, &tool_ctx);
                    }

                    // Update snap label
                    self.snap_label = tool
                        .snap_result()
                        .map(|s| format!("Snap: {:?}", s.snap_type));

                    // Text input (for text tool)
                    let typed: Vec<_> = ctx.input(|i| i.events.clone())
                        .into_iter()
                        .filter_map(|e| match e {
                            egui::Event::Text(t) => Some(t),
                            _ => None,
                        })
                        .collect();
                    for t in &typed {
                        tool.on_text_input(t);
                    }

                    // Handle backspace for text tool
                    if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                        tool.on_text_input("\u{8}");
                    }

                    // Keyboard
                    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));

                    if matches!(result, ToolResult::Continue) && enter {
                        result = tool.on_key_enter();
                    }
                    if matches!(result, ToolResult::Continue) && escape {
                        result = tool.on_key_escape();
                    }

                    // Forward generic key presses to tool
                    for key in [egui::Key::C, egui::Key::E, egui::Key::Tab] {
                        if matches!(result, ToolResult::Continue) && ctx.input(|i| i.key_pressed(key)) {
                            result = tool.on_key_pressed(key);
                        }
                    }
                    if matches!(result, ToolResult::Continue)
                        && self.active_tool_type == ActiveToolType::PlaceEntourage
                        && ctx.input(|i| i.key_pressed(egui::Key::R))
                    {
                        result = tool.on_key_pressed(egui::Key::R);
                    }

                    // Put tool back before handling result (which may clear it)
                    self.draw_tool = Some(tool);
                    self.handle_tool_result(result);
                    return;
                }

                // --- Select tool input ---
                if self.active_tool_type == ActiveToolType::Select {
                    self.snap_label = None;
                    let tolerance = 8.0 / self.canvas.viewport.zoom;

                    // --- Click: select/deselect (no drag involved) ---
                    if canvas_result.response.clicked_by(egui::PointerButton::Primary) {
                        // Hit test entities (reverse order = topmost first)
                        let mut hit = None;
                        for entity in tool_ctx.entities.iter().rev() {
                            if entity.hit_test(pos, tolerance) {
                                hit = Some(entity.id);
                                break;
                            }
                        }

                        if let Some(hit_id) = hit {
                            if shift {
                                self.selection.toggle(hit_id);
                            } else {
                                self.selection.set(hit_id);
                            }
                        } else if !shift {
                            self.selection.clear();
                        }
                    }

                    // --- Drag start: begin box select or bezier point drag ---
                    if canvas_result.response.drag_started_by(egui::PointerButton::Primary) {
                        // Check for bezier point drag first
                        let bez_tolerance = 10.0 / self.canvas.viewport.zoom;
                        if let Some((eid, idx, target)) = self.hit_test_bezier_points(pos, bez_tolerance) {
                            let original_geometry = self.project.find_entity(eid)
                                .unwrap().geometry.clone();
                            self.bezier_edit = Some(BezierEditState {
                                entity_id: eid,
                                anchor_index: idx,
                                target,
                                original_geometry,
                            });
                        } else {
                            // Start box selection
                            self.box_select_start = Some(pos);
                        }
                    }

                    // --- Drag stop: finish box select ---
                    if canvas_result.response.drag_stopped_by(egui::PointerButton::Primary) {
                        if let Some(start) = self.box_select_start.take() {
                            let min = glam::Vec2::new(start.x.min(pos.x), start.y.min(pos.y));
                            let max = glam::Vec2::new(start.x.max(pos.x), start.y.max(pos.y));
                            let bb = BoundingBox::new(min, max);
                            if !shift {
                                self.selection.clear();
                            }
                            for entity in &self.project.entities {
                                if let Some(ebb) = entity.bounding_box() {
                                    if bb.contains(ebb.center()) {
                                        self.selection.add(entity.id);
                                    }
                                }
                            }
                        }
                    }

                    // Escape clears selection and smart points
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.selection.clear();
                        self.bezier_edit = None;
                        self.box_select_start = None;
                        self.smart_points.clear();
                        self.smart_hover = None;
                    }

                    // Shift+Right-click: place anchor point (with snapping)
                    if shift && canvas_result.response.clicked_by(egui::PointerButton::Secondary) {
                        let snapped = if self.snap_config.enabled {
                            let smart_positions: Vec<glam::Vec2> = self.smart_points.iter().map(|sp| sp.position).collect();
                            let candidates = crate::tools::snap_candidates_from_entities(
                                pos, &self.project.entities, &self.snap_config, &smart_positions,
                            );
                            let tolerance = self.snap_config.snap_distance / self.canvas.viewport.zoom;
                            grove_core::snap::best_snap(&candidates, tolerance)
                                .map(|s| s.position)
                                .unwrap_or(pos)
                        } else {
                            pos
                        };
                        self.anchor_point = Some(snapped);
                    }

                    // Shift+C: clear anchor point
                    if shift && ctx.input(|i| i.key_pressed(egui::Key::C)) {
                        self.anchor_point = None;
                    }

                    // Space: return to Select mode (works from any tool)
                    if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                        self.switch_tool(ActiveToolType::Select);
                        return;
                    }

                    // Modification shortcuts (Blender-style, when selection is non-empty)
                    if !self.selection.is_empty() && !ctrl {
                        if ctx.input(|i| i.key_pressed(egui::Key::G)) {
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Move);
                            return;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Rotate);
                            return;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Scale);
                            return;
                        }
                        if shift && ctx.input(|i| i.key_pressed(egui::Key::D)) {
                            self.activate_array_tool();
                            return;
                        }
                        // '-' key: activate subtract/cutout tool on selected polygon
                        if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
                            if let Some(sel_id) = self.selection.single() {
                                let is_polygon = self.project.find_entity(sel_id).map(|e| {
                                    matches!(&e.geometry, Geometry::Polygon(_))
                                        || matches!(&e.geometry, Geometry::BezierPath(bp) if bp.closed)
                                }).unwrap_or(false);
                                if is_polygon {
                                    let mut tool = crate::tools::subtract_polygon::SubtractPolygonTool::new(sel_id);
                                    tool.cutout_shape = match self.shape_sub_tool {
                                        ShapeTool::Circle => crate::tools::subtract_polygon::CutoutShape::Circle,
                                        ShapeTool::Rect => crate::tools::subtract_polygon::CutoutShape::Rectangle,
                                        _ => crate::tools::subtract_polygon::CutoutShape::Polygon,
                                    };
                                    self.draw_tool = Some(Box::new(tool));
                                    self.active_tool_type = ActiveToolType::SubtractPolygon;
                                    self.tool_mode = ToolMode::Cutout;
                                    return;
                                }
                            }
                        }
                    }

                    // Ctrl+Arrow: nudge selected entities
                    if ctrl && !self.selection.ids.is_empty() {
                        let nudge = 10.0_f32; // 10 mm per press
                        let delta = if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                            Some(glam::Vec2::new(0.0, -nudge))
                        } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                            Some(glam::Vec2::new(0.0, nudge))
                        } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                            Some(glam::Vec2::new(-nudge, 0.0))
                        } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                            Some(glam::Vec2::new(nudge, 0.0))
                        } else {
                            None
                        };
                        if let Some(d) = delta {
                            let ids: Vec<_> = self.selection.ids.clone();
                            let commands: Vec<Box<dyn grove_core::commands::Command>> = ids
                                .iter()
                                .map(|&id| -> Box<dyn grove_core::commands::Command> {
                                    Box::new(grove_core::commands::MoveEntityCommand::new(id, d))
                                })
                                .collect();
                            let cmd = Box::new(grove_core::commands::CompoundCommand::new(
                                commands,
                                "Nudge entities",
                            ));
                            self.history.execute(cmd, &mut self.project);
                            return;
                        }
                    }

                    // Delete key (X, Delete, or Backspace)
                    if ctx.input(|i| {
                        i.key_pressed(egui::Key::Delete)
                            || i.key_pressed(egui::Key::Backspace)
                            || (!ctrl && i.key_pressed(egui::Key::X))
                    }) {
                        self.delete_selection("Delete entities");
                    }
                }

            });
    }
}
