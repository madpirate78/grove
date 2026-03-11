use std::path::PathBuf;

use eframe::Frame;
use egui::{CentralPanel, Color32, Context, SidePanel, TopBottomPanel, Vec2};

use grove_core::commands::{CommandHistory, UpdateGeometryCommand};
use grove_core::entity::Geometry;
use grove_core::garden::database::PlantDatabase;
use grove_core::garden::plant::Plant;
use grove_core::geometry::BoundingBox;
use grove_core::project::Project;
use grove_core::snap::SnapConfig;
use grove_core::storage;

use crate::panels::palette::PaletteState;
use crate::theme::{self, ThemeColours};
use crate::tools::select::{SelectTool, Selection};
use crate::tools::{Tool, ToolContext, ToolResult};
use crate::view::background::BackgroundImageRenderer;
use crate::view::canvas::Canvas;

fn ensure_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        path
    } else {
        path.with_extension(ext)
    }
}

/// What part of a bezier anchor is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BezierDragTarget {
    Anchor,
    HandleIn,
    HandleOut,
}

/// State for editing bezier control points on a selected curve.
#[derive(Debug)]
struct BezierEditState {
    entity_id: uuid::Uuid,
    anchor_index: usize,
    target: BezierDragTarget,
    /// Original geometry saved at drag start for undo.
    original_geometry: Geometry,
}

/// Whether viewing model space or a drawing sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceMode {
    Model,
    Drawing(uuid::Uuid),
}

/// Which tool type is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveToolType {
    Select,
    DrawLine,
    DrawArc,
    DrawRect,
    DrawCircle,
    Text,
    Dimension,
    PlacePlant,
    PlaceGroup,
    PlaceHardscape,
    PlaceLevel,
    Modify,
    LinearArray,
    SubtractPolygon,
}

impl ActiveToolType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::DrawLine => "Line",
            Self::DrawArc => "Arc",
            Self::DrawRect => "Rect",
            Self::DrawCircle => "Circle",
            Self::Text => "Text",
            Self::Dimension => "Dim",
            Self::PlacePlant => "Plant",
            Self::PlaceGroup => "Group",
            Self::PlaceHardscape => "Hardscape",
            Self::PlaceLevel => "Level",
            Self::Modify => "Modify",
            Self::LinearArray => "Array",
            Self::SubtractPolygon => "Subtract",
        }
    }
}

/// High-level toolbar mode (column 1): what kind of entity are we working with?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Select,
    Move,
    Rotate,
    Shape,
    Hardscape,
    Plant,
    Group,
    Level,
    Cutout,
}

/// Drawing sub-tool (column 2): what geometric method are we using?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeTool {
    Line,
    Arc,
    Circle,
    Rect,
    Text,
    Dim,
    Point,
}

impl ShapeTool {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Arc => "Arc",
            Self::Circle => "Circle",
            Self::Rect => "Rect",
            Self::Text => "Text",
            Self::Dim => "Dim",
            Self::Point => "Point",
        }
    }
}

/// Returns the available shape sub-tools for a given mode and sub-type context.
pub fn available_shapes(
    mode: ToolMode,
    hardscape_sub_type: Option<crate::tools::place_hardscape::HardscapeSubType>,
    level_sub_type: Option<crate::tools::place_level::LevelSubType>,
) -> &'static [ShapeTool] {
    use crate::tools::place_hardscape::HardscapeSubType;
    use crate::tools::place_level::LevelSubType;

    match mode {
        ToolMode::Shape => &[ShapeTool::Line, ShapeTool::Arc, ShapeTool::Circle, ShapeTool::Rect, ShapeTool::Text, ShapeTool::Dim],
        ToolMode::Hardscape => {
            match hardscape_sub_type {
                Some(HardscapeSubType::Wall | HardscapeSubType::Path | HardscapeSubType::Step | HardscapeSubType::Edging | HardscapeSubType::Fence) => {
                    &[ShapeTool::Line]
                }
                Some(HardscapeSubType::Pergola | HardscapeSubType::House) => {
                    &[ShapeTool::Rect]
                }
                Some(HardscapeSubType::Patio | HardscapeSubType::RaisedBed | HardscapeSubType::Decking | HardscapeSubType::WaterFeature) => {
                    &[ShapeTool::Line, ShapeTool::Circle, ShapeTool::Rect]
                }
                None => &[ShapeTool::Line, ShapeTool::Circle, ShapeTool::Rect],
            }
        }
        ToolMode::Plant => &[ShapeTool::Point],
        ToolMode::Group => &[ShapeTool::Line],
        ToolMode::Level => {
            match level_sub_type {
                Some(LevelSubType::SpotHeight) => &[ShapeTool::Point],
                Some(LevelSubType::Contour | LevelSubType::SlopeArrow | LevelSubType::DrainRun) => &[ShapeTool::Line],
                None => &[ShapeTool::Point, ShapeTool::Line],
            }
        }
        ToolMode::Cutout => &[ShapeTool::Line, ShapeTool::Circle, ShapeTool::Rect],
        ToolMode::Select | ToolMode::Move | ToolMode::Rotate => &[],
    }
}

/// Which adjustment operation is active on a background image.
enum AdjustMode {
    /// Calibrate scale: draw two points, enter real distance.
    CalibrateScale {
        first_point: Option<glam::Vec2>,
        awaiting_distance: bool,
        distance_input: String,
    },
    /// Align rotation: draw a line that should be horizontal.
    AlignHorizontal {
        first_point: Option<glam::Vec2>,
    },
    /// Set origin: click the point that should be world (0,0).
    SetOrigin,
}

/// State for background image adjustment workflows.
struct CalibrateState {
    image_id: uuid::Uuid,
    mode: AdjustMode,
}

/// The main Grove application state.
pub struct GroveApp {
    pub project: Project,
    pub canvas: Canvas,
    pub history: CommandHistory,
    pub selection: Selection,
    pub snap_config: SnapConfig,

    pub active_tool_type: ActiveToolType,
    pub tool_mode: ToolMode,
    pub shape_sub_tool: ShapeTool,
    pub select_tool: SelectTool,
    pub draw_tool: Option<Box<dyn Tool>>,

    pub cursor_world_pos: Option<glam::Vec2>,
    snap_label: Option<String>,
    first_frame: bool,
    bezier_edit: Option<BezierEditState>,
    /// Box-select drag start position (world coords).
    box_select_start: Option<glam::Vec2>,

    pub plant_db: PlantDatabase,
    pub palette_state: PaletteState,
    pub selected_plant: Option<Plant>,

    // File / persistence
    pub file_path: Option<PathBuf>,
    pub recent_files: Vec<PathBuf>,
    auto_save_elapsed: f32,
    /// Layer being renamed (layer index, buffer)
    layer_rename: Option<(usize, String)>,

    // Background images
    bg_renderer: BackgroundImageRenderer,
    calibrate_state: Option<CalibrateState>,
    /// Second calibration point (stored separately to avoid borrow issues).
    calibrate_second_point: Option<glam::Vec2>,

    /// Anchor point (Blender-style 3D cursor) used as default pivot for Rotate/Scale.
    pub anchor_point: Option<glam::Vec2>,

    /// Internal clipboard for copy/paste.
    clipboard: Vec<grove_core::entity::Entity>,

    /// Current workspace mode: Model or Drawing.
    pub workspace_mode: WorkspaceMode,

    /// Persistent interaction state for drawing mode (viewport drag/resize/create).
    drawing_interaction: crate::view::drawing_view::DrawingInteraction,

    /// Separate view transform for drawing mode (independent zoom/pan from model canvas).
    drawing_viewport: crate::view::viewport::ViewTransform,
    /// The drawing ID we last fit-to-page for (so we auto-fit on first entry).
    drawing_viewport_fitted_for: Option<uuid::Uuid>,
}

impl GroveApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        let mut plant_db = PlantDatabase::new();

        // Try to load plants from CSV (look in data/ relative to executable, then current dir)
        let csv_paths = [
            "data/plants.csv",
            "../data/plants.csv",
            "../../data/plants.csv",
        ];
        for path in &csv_paths {
            if let Ok(file) = std::fs::File::open(path) {
                let reader = std::io::BufReader::new(file);
                match plant_db.load_from_csv(reader) {
                    Ok(count) => {
                        log::info!("Loaded {count} plants from {path}");
                        break;
                    }
                    Err(e) => log::warn!("Failed to load {path}: {e}"),
                }
            }
        }

        let recent_files = Self::load_recent_files();

        Self {
            project: Project::new(),
            canvas: Canvas::new(),
            history: CommandHistory::new(),
            selection: Selection::default(),
            snap_config: SnapConfig::default(),
            active_tool_type: ActiveToolType::Select,
            tool_mode: ToolMode::Select,
            shape_sub_tool: ShapeTool::Line,
            select_tool: SelectTool::new(),
            draw_tool: None,
            cursor_world_pos: None,
            snap_label: None,
            first_frame: true,
            bezier_edit: None,
            box_select_start: None,
            plant_db,
            palette_state: PaletteState::default(),
            selected_plant: None,
            file_path: None,
            recent_files,
            auto_save_elapsed: 0.0,
            layer_rename: None,
            bg_renderer: BackgroundImageRenderer::new(),
            calibrate_state: None,
            calibrate_second_point: None,
            anchor_point: Some(glam::Vec2::ZERO),
            clipboard: Vec::new(),
            workspace_mode: WorkspaceMode::Model,
            drawing_interaction: crate::view::drawing_view::DrawingInteraction::default(),
            drawing_viewport: crate::view::viewport::ViewTransform::with_zoom_limits(0.5, 10.0),
            drawing_viewport_fitted_for: None,
        }
    }

    fn switch_tool(&mut self, tool_type: ActiveToolType) {
        self.active_tool_type = tool_type;
        self.bezier_edit = None;
        self.box_select_start = None;
        // Sync tool_mode and shape_sub_tool from active_tool_type
        self.tool_mode = match tool_type {
            ActiveToolType::Select => ToolMode::Select,
            ActiveToolType::Modify => ToolMode::Move, // updated by activate_modify_tool
            ActiveToolType::DrawLine | ActiveToolType::DrawArc | ActiveToolType::DrawCircle
            | ActiveToolType::DrawRect | ActiveToolType::Text | ActiveToolType::Dimension => ToolMode::Shape,
            ActiveToolType::PlacePlant => ToolMode::Plant,
            ActiveToolType::PlaceGroup => ToolMode::Group,
            ActiveToolType::PlaceHardscape => ToolMode::Hardscape,
            ActiveToolType::PlaceLevel => ToolMode::Level,
            ActiveToolType::SubtractPolygon => ToolMode::Cutout,
            ActiveToolType::LinearArray => self.tool_mode, // keep current
        };
        // Sync shape_sub_tool for Shape mode
        if let Some(shape) = match tool_type {
            ActiveToolType::DrawLine => Some(ShapeTool::Line),
            ActiveToolType::DrawArc => Some(ShapeTool::Arc),
            ActiveToolType::DrawCircle => Some(ShapeTool::Circle),
            ActiveToolType::DrawRect => Some(ShapeTool::Rect),
            ActiveToolType::Text => Some(ShapeTool::Text),
            ActiveToolType::Dimension => Some(ShapeTool::Dim),
            _ => None,
        } {
            self.shape_sub_tool = shape;
        }
        self.draw_tool = match tool_type {
            ActiveToolType::DrawLine => {
                Some(Box::new(crate::tools::draw_line::DrawLineTool::new()))
            }
            ActiveToolType::DrawRect => {
                Some(Box::new(crate::tools::draw_rect::DrawRectTool::new()))
            }
            ActiveToolType::DrawCircle => {
                Some(Box::new(crate::tools::draw_circle::DrawCircleTool::new()))
            }
            ActiveToolType::DrawArc => {
                Some(Box::new(crate::tools::draw_arc::DrawArcTool::new()))
            }
            ActiveToolType::Text => {
                Some(Box::new(crate::tools::draw_text::DrawTextTool::new()))
            }
            ActiveToolType::Dimension => {
                Some(Box::new(crate::tools::draw_dimension::DrawDimensionTool::new()))
            }
            ActiveToolType::PlacePlant => {
                let plant = self.selected_plant.clone().unwrap_or_else(|| {
                    self.plant_db.plants.first().cloned().unwrap_or_else(|| {
                        grove_core::garden::plant::Plant::new("Unknown", grove_core::garden::plant::Category::Perennial)
                    })
                });
                let mut tool = crate::tools::place_plant::PlacePlantTool::new(plant);
                if let Some(layer) = self.project.layers.find_by_name("GD_PLANTING") {
                    tool.layer_id = layer.id;
                }
                Some(Box::new(tool))
            }
            ActiveToolType::PlaceGroup => {
                // Check if selected entities contain plants — extract species proportions
                let mut species_map: std::collections::BTreeMap<String, (grove_core::garden::plant::PlantData, [u8; 4], f32, usize)> =
                    std::collections::BTreeMap::new();
                for &id in &self.selection.ids {
                    if let Some(entity) = self.project.find_entity(id) {
                        if let grove_core::entity::DomainData::Plant(ref pd) = entity.domain {
                            let entry = species_map.entry(pd.botanical_name.clone()).or_insert_with(|| {
                                (pd.clone(), entity.colour, pd.radius_mm(), 0)
                            });
                            entry.3 += 1;
                        }
                    }
                }

                if species_map.len() >= 2 {
                    // Multi-species mode
                    let mix: Vec<crate::tools::place_group::SpeciesEntry> = species_map
                        .into_values()
                        .map(|(plant_data, colour, radius_mm, count)| {
                            crate::tools::place_group::SpeciesEntry {
                                plant_data,
                                colour,
                                radius_mm,
                                proportion: count as f32,
                            }
                        })
                        .collect();
                    let mut tool = crate::tools::place_group::PlaceGroupTool::new_from_mix(mix);
                    if let Some(layer) = self.project.layers.find_by_name("GD_PLANTING") {
                        tool.layer_id = layer.id;
                    }
                    Some(Box::new(tool))
                } else {
                    // Single-species mode (existing behaviour)
                    let plant = self.selected_plant.clone().unwrap_or_else(|| {
                        self.plant_db.plants.first().cloned().unwrap_or_else(|| {
                            grove_core::garden::plant::Plant::new("Unknown", grove_core::garden::plant::Category::Perennial)
                        })
                    });
                    let mut tool = crate::tools::place_group::PlaceGroupTool::new(plant);
                    if let Some(layer) = self.project.layers.find_by_name("GD_PLANTING") {
                        tool.layer_id = layer.id;
                    }
                    Some(Box::new(tool))
                }
            }
            ActiveToolType::PlaceHardscape => {
                let mut tool = crate::tools::place_hardscape::PlaceHardscapeTool::new();
                if let Some(layer) = self.project.layers.find_by_name(tool.target_layer_name()) {
                    tool.layer_id = layer.id;
                }
                Some(Box::new(tool))
            }
            ActiveToolType::PlaceLevel => {
                let mut tool = crate::tools::place_level::PlaceLevelTool::new();
                if let Some(layer) = self.project.layers.find_by_name(tool.target_layer_name()) {
                    tool.layer_id = layer.id;
                }
                Some(Box::new(tool))
            }
            _ => None,
        };
        self.snap_label = None;
    }

    // -- File operations --

    fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "Grove").map(|d| d.config_dir().to_path_buf())
    }

    fn load_recent_files() -> Vec<PathBuf> {
        let Some(dir) = Self::config_dir() else {
            return Vec::new();
        };
        let path = dir.join("recent.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
            .unwrap_or_default()
    }

    fn save_recent_files(&self) {
        let Some(dir) = Self::config_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let json = serde_json::to_string_pretty(&self.recent_files).unwrap_or_default();
        let _ = std::fs::write(dir.join("recent.json"), json);
    }

    fn add_to_recent(&mut self, path: &PathBuf) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(10);
        self.save_recent_files();
    }

    fn new_project(&mut self) {
        self.project = Project::new();
        self.history.clear();
        self.selection.clear();
        self.file_path = None;
        self.draw_tool = None;
        self.active_tool_type = ActiveToolType::Select;
        self.bg_renderer.clear();
        self.calibrate_state = None;
    }

    fn save(&mut self) -> Result<(), String> {
        if let Some(ref path) = self.file_path.clone() {
            storage::save_project(&self.project, path)?;
            self.project.dirty = false;
            Ok(())
        } else {
            self.save_as()
        }
    }

    fn save_as(&mut self) -> Result<(), String> {
        let dialog = rfd::FileDialog::new()
            .set_title("Save Grove Project")
            .add_filter("Grove Project", &["grove"])
            .set_file_name("untitled.grove");
        if let Some(path) = dialog.save_file() {
            storage::save_project(&self.project, &path)?;
            self.project.dirty = false;
            self.add_to_recent(&path);
            self.file_path = Some(path);
            Ok(())
        } else {
            Ok(()) // User cancelled
        }
    }

    fn open_file(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_title("Open Grove Project")
            .add_filter("Grove Project", &["grove"]);
        if let Some(path) = dialog.pick_file() {
            self.open_path(path);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        match storage::load_project(&path) {
            Ok(project) => {
                self.project = project;
                self.history.clear();
                self.selection.clear();
                self.draw_tool = None;
                self.active_tool_type = ActiveToolType::Select;
                self.bg_renderer.clear();
                self.calibrate_state = None;
                self.add_to_recent(&path);
                self.file_path = Some(path);
                log::info!("Project opened successfully");
            }
            Err(e) => {
                log::error!("Failed to open project: {e}");
            }
        }
    }

    fn update_title(&self, ctx: &Context) {
        let name = self.file_path.as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.project.metadata.name.clone());
        let dirty = if self.project.dirty { " *" } else { "" };
        let title = format!("Grove — {name}{dirty}");
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Hit-test bezier anchors and handles for the selected entity.
    /// Returns (anchor_index, drag_target) if hit, plus the entity id.
    fn hit_test_bezier_points(
        &self,
        pos: glam::Vec2,
        tolerance: f32,
    ) -> Option<(uuid::Uuid, usize, BezierDragTarget)> {
        let sel_id = self.selection.single()?;
        let entity = self.project.find_entity(sel_id)?;
        let anchors = match &entity.geometry {
            Geometry::BezierPath(bp) => &bp.anchors,
            _ => return None,
        };

        // Check handles first (they're smaller targets, prioritize them)
        for (i, anchor) in anchors.iter().enumerate() {
            if let Some(h) = anchor.handle_in {
                if (pos - h).length() <= tolerance {
                    return Some((sel_id, i, BezierDragTarget::HandleIn));
                }
            }
            if let Some(h) = anchor.handle_out {
                if (pos - h).length() <= tolerance {
                    return Some((sel_id, i, BezierDragTarget::HandleOut));
                }
            }
        }
        // Then check anchor points
        for (i, anchor) in anchors.iter().enumerate() {
            if (pos - anchor.point).length() <= tolerance {
                return Some((sel_id, i, BezierDragTarget::Anchor));
            }
        }
        None
    }

    /// Draw bezier edit handles (anchors + tangent lines) for a selected bezier entity.
    fn draw_bezier_edit_handles(&self, painter: &egui::Painter, entity_id: uuid::Uuid) {
        let Some(entity) = self.project.find_entity(entity_id) else { return };
        let anchors = match &entity.geometry {
            Geometry::BezierPath(bp) => &bp.anchors,
            _ => return,
        };

        let accent = Color32::from_rgb(70, 130, 210);
        let handle_colour = Color32::from_rgb(200, 140, 60);
        let handle_stroke = egui::Stroke::new(1.0, handle_colour);

        for anchor in anchors {
            let screen = self.canvas.viewport.world_to_screen(anchor.point);

            // Draw tangent lines and handle dots
            if let Some(h_in) = anchor.handle_in {
                let h_screen = self.canvas.viewport.world_to_screen(h_in);
                painter.line_segment([h_screen, screen], handle_stroke);
                painter.circle_filled(h_screen, 3.5, handle_colour);
            }
            if let Some(h_out) = anchor.handle_out {
                let h_screen = self.canvas.viewport.world_to_screen(h_out);
                painter.line_segment([screen, h_screen], handle_stroke);
                painter.circle_filled(h_screen, 3.5, handle_colour);
            }

            // Anchor point (drawn on top)
            painter.circle_filled(screen, 5.0, accent);
            painter.circle_filled(screen, 3.0, Color32::from_rgb(30, 30, 36));
        }
    }

    /// Apply a drag delta to the bezier point being edited.
    fn apply_bezier_drag(&mut self, pos: glam::Vec2) {
        let Some(ref edit) = self.bezier_edit else { return };
        let entity_id = edit.entity_id;
        let idx = edit.anchor_index;
        let target = edit.target;

        let Some(entity) = self.project.find_entity_mut(entity_id) else { return };
        let anchors = match &mut entity.geometry {
            Geometry::BezierPath(bp) => &mut bp.anchors,
            _ => return,
        };
        let Some(anchor) = anchors.get_mut(idx) else { return };

        match target {
            BezierDragTarget::Anchor => {
                let delta = pos - anchor.point;
                anchor.point = pos;
                // Move handles with the anchor
                if let Some(ref mut h) = anchor.handle_in {
                    *h += delta;
                }
                if let Some(ref mut h) = anchor.handle_out {
                    *h += delta;
                }
            }
            BezierDragTarget::HandleOut => {
                anchor.handle_out = Some(pos);
                // Mirror the other handle for smooth continuity
                anchor.handle_in = Some(anchor.point * 2.0 - pos);
            }
            BezierDragTarget::HandleIn => {
                anchor.handle_in = Some(pos);
                // Mirror the other handle for smooth continuity
                anchor.handle_out = Some(anchor.point * 2.0 - pos);
            }
        }
    }

    fn handle_tool_result(&mut self, result: ToolResult) {
        let preserve_selection = matches!(
            self.active_tool_type,
            ActiveToolType::Modify | ActiveToolType::LinearArray | ActiveToolType::SubtractPolygon
        );
        match result {
            ToolResult::Continue => {}
            ToolResult::Emit(cmd) => {
                self.history.execute(cmd, &mut self.project);
                if preserve_selection {
                    // Preserve selection (modify chaining, or keep target after cutout)
                } else {
                    // Auto-select the last entity and return to Select tool
                    if let Some(last) = self.project.entities.last() {
                        self.selection.set(last.id);
                    }
                }
                self.switch_tool(ActiveToolType::Select);
            }
            ToolResult::EmitAndContinue(cmd) => {
                self.history.execute(cmd, &mut self.project);
            }
            ToolResult::Finished => {
                self.switch_tool(ActiveToolType::Select);
            }
        }
    }

    fn activate_modify_tool(&mut self, mode: crate::tools::modify::ModifyMode) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.ids.clone();
        // For Rotate/Scale, use anchor point as pre-set pivot if available
        let tool = if mode != crate::tools::modify::ModifyMode::Move {
            if let Some(anchor) = self.anchor_point {
                crate::tools::modify::ModifyTool::new_with_base(mode, ids, &self.project.entities, anchor)
            } else {
                crate::tools::modify::ModifyTool::new(mode, ids, &self.project.entities)
            }
        } else {
            crate::tools::modify::ModifyTool::new(mode, ids, &self.project.entities)
        };
        self.draw_tool = Some(Box::new(tool));
        self.active_tool_type = ActiveToolType::Modify;
        self.tool_mode = match mode {
            crate::tools::modify::ModifyMode::Move => ToolMode::Move,
            crate::tools::modify::ModifyMode::Rotate => ToolMode::Rotate,
            _ => ToolMode::Move,
        };
    }

    /// Check whether a shape sub-tool should appear active in column 2.
    fn is_shape_active(
        &self,
        shape: ShapeTool,
        cutout_shape: Option<crate::tools::subtract_polygon::CutoutShape>,
        hardscape_override: Option<crate::tools::place_hardscape::ShapeOverride>,
    ) -> bool {
        match self.tool_mode {
            ToolMode::Shape => self.shape_sub_tool == shape,
            ToolMode::Hardscape => {
                use crate::tools::place_hardscape::ShapeOverride;
                match shape {
                    ShapeTool::Line => matches!(hardscape_override, None | Some(ShapeOverride::Line)),
                    ShapeTool::Circle => hardscape_override == Some(ShapeOverride::Circle),
                    ShapeTool::Rect => hardscape_override == Some(ShapeOverride::Rectangle),
                    _ => false,
                }
            }
            ToolMode::Cutout => {
                use crate::tools::subtract_polygon::CutoutShape;
                match shape {
                    ShapeTool::Line => cutout_shape == Some(CutoutShape::Polygon),
                    ShapeTool::Circle => cutout_shape == Some(CutoutShape::Circle),
                    ShapeTool::Rect => cutout_shape == Some(CutoutShape::Rectangle),
                    _ => false,
                }
            }
            // Single-option modes: the one available shape is always active
            ToolMode::Plant | ToolMode::Group | ToolMode::Level => true,
            _ => false,
        }
    }

    /// Handle a mode button click from column 1.
    fn apply_mode_change(&mut self, new_mode: ToolMode) {
        self.tool_mode = new_mode;
        match new_mode {
            ToolMode::Select => {
                self.switch_tool(ActiveToolType::Select);
            }
            ToolMode::Move => {
                self.activate_modify_tool(crate::tools::modify::ModifyMode::Move);
            }
            ToolMode::Rotate => {
                self.activate_modify_tool(crate::tools::modify::ModifyMode::Rotate);
            }
            ToolMode::Shape => {
                // Activate the current shape sub-tool (or default to Line)
                self.apply_shape_change(self.shape_sub_tool);
            }
            ToolMode::Hardscape => {
                self.switch_tool(ActiveToolType::PlaceHardscape);
            }
            ToolMode::Plant => {
                self.shape_sub_tool = ShapeTool::Point;
                self.switch_tool(ActiveToolType::PlacePlant);
            }
            ToolMode::Group => {
                self.shape_sub_tool = ShapeTool::Line;
                self.switch_tool(ActiveToolType::PlaceGroup);
            }
            ToolMode::Level => {
                self.switch_tool(ActiveToolType::PlaceLevel);
            }
            ToolMode::Cutout => {
                // Cutout requires a polygon to be selected
                let can_subtract = self.selection.single().map(|id| {
                    self.project.find_entity(id).map(|e| {
                        matches!(&e.geometry, Geometry::Polygon(_))
                            || matches!(&e.geometry, Geometry::BezierPath(bp) if bp.closed)
                    }).unwrap_or(false)
                }).unwrap_or(false);
                if can_subtract {
                    if let Some(sel_id) = self.selection.single() {
                        let mut tool = crate::tools::subtract_polygon::SubtractPolygonTool::new(sel_id);
                        // Map current shape_sub_tool to CutoutShape
                        tool.cutout_shape = match self.shape_sub_tool {
                            ShapeTool::Circle => crate::tools::subtract_polygon::CutoutShape::Circle,
                            ShapeTool::Rect => crate::tools::subtract_polygon::CutoutShape::Rectangle,
                            _ => crate::tools::subtract_polygon::CutoutShape::Polygon,
                        };
                        self.draw_tool = Some(Box::new(tool));
                        self.active_tool_type = ActiveToolType::SubtractPolygon;
                    }
                }
            }
        }
    }

    /// Handle a shape sub-tool click from column 2.
    fn apply_shape_change(&mut self, shape: ShapeTool) {
        self.shape_sub_tool = shape;
        match self.tool_mode {
            ToolMode::Shape => {
                let tool_type = match shape {
                    ShapeTool::Line => ActiveToolType::DrawLine,
                    ShapeTool::Arc => ActiveToolType::DrawArc,
                    ShapeTool::Circle => ActiveToolType::DrawCircle,
                    ShapeTool::Rect => ActiveToolType::DrawRect,
                    ShapeTool::Text => ActiveToolType::Text,
                    ShapeTool::Dim => ActiveToolType::Dimension,
                    ShapeTool::Point => ActiveToolType::DrawLine, // fallback
                };
                self.switch_tool(tool_type);
            }
            ToolMode::Hardscape => {
                use crate::tools::place_hardscape::ShapeOverride;
                let ovr = match shape {
                    ShapeTool::Circle => Some(ShapeOverride::Circle),
                    ShapeTool::Rect => Some(ShapeOverride::Rectangle),
                    _ => None, // Line = default polygon drawer
                };
                if let Some(ref mut tool) = self.draw_tool {
                    if let Some(ht) = tool.as_any_mut().downcast_mut::<crate::tools::place_hardscape::PlaceHardscapeTool>() {
                        ht.shape_override = ovr;
                        ht.reset_drawing_state();
                    }
                }
            }
            ToolMode::Cutout => {
                use crate::tools::subtract_polygon::CutoutShape;
                let cs = match shape {
                    ShapeTool::Circle => CutoutShape::Circle,
                    ShapeTool::Rect => CutoutShape::Rectangle,
                    _ => CutoutShape::Polygon,
                };
                if let Some(ref mut tool) = self.draw_tool {
                    if let Some(st) = tool.as_any_mut().downcast_mut::<crate::tools::subtract_polygon::SubtractPolygonTool>() {
                        st.cutout_shape = cs;
                        st.reset_drawing_state();
                    }
                }
            }
            // Single-option modes: clicking the shape doesn't change anything
            _ => {}
        }
    }

    fn activate_array_tool(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.ids.clone();
        let tool = crate::tools::array::LinearArrayTool::new(ids, &self.project.entities);
        self.draw_tool = Some(Box::new(tool));
        self.active_tool_type = ActiveToolType::LinearArray;
    }

    /// Recursively assign new UUIDs to an entity and all its children.
    fn assign_new_ids(entity: &mut grove_core::entity::Entity) {
        entity.id = uuid::Uuid::new_v4();
        for child in &mut entity.children {
            Self::assign_new_ids(child);
        }
    }

    fn copy_selection_to_clipboard(&mut self) {
        self.clipboard = self
            .selection
            .ids
            .iter()
            .filter_map(|id| self.project.entities.iter().find(|e| e.id == *id).cloned())
            .collect();
    }

    fn paste_from_clipboard(&mut self) {
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

    fn duplicate_selection(&mut self) {
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

impl eframe::App for GroveApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if self.first_frame {
            theme::apply_theme(ctx);
            self.canvas.viewport.offset = glam::Vec2::new(400.0, 300.0);
            self.canvas.viewport.zoom = 0.5;
            self.first_frame = false;
        }

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

        // Auto-save every 60 seconds if dirty and file exists
        if self.project.dirty && self.file_path.is_some() {
            self.auto_save_elapsed += ctx.input(|i| i.stable_dt);
            if self.auto_save_elapsed >= 60.0 {
                self.auto_save_elapsed = 0.0;
                if let Err(e) = self.save() {
                    log::error!("Auto-save failed: {e}");
                }
            }
        } else {
            self.auto_save_elapsed = 0.0;
        }

        // Update window title
        self.update_title(ctx);

        // Menu bar
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project  Ctrl+N").clicked() {
                        self.new_project();
                        ui.close_menu();
                    }
                    if ui.button("Open…  Ctrl+O").clicked() {
                        ui.close_menu();
                        self.open_file();
                    }
                    ui.separator();
                    if ui.button("Save  Ctrl+S").clicked() {
                        ui.close_menu();
                        if let Err(e) = self.save() {
                            log::error!("Save failed: {e}");
                        }
                    }
                    if ui.button("Save As…  Ctrl+Shift+S").clicked() {
                        ui.close_menu();
                        if let Err(e) = self.save_as() {
                            log::error!("Save As failed: {e}");
                        }
                    }
                    if !self.recent_files.is_empty() {
                        ui.separator();
                        ui.menu_button("Recent Projects", |ui| {
                            let recents = self.recent_files.clone();
                            for path in &recents {
                                let label = path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| path.display().to_string());
                                if ui.button(&label).on_hover_text(path.display().to_string()).clicked() {
                                    let p = path.clone();
                                    ui.close_menu();
                                    self.open_path(p);
                                }
                            }
                        });
                    }
                    ui.separator();
                    if ui.button("Import Background Image…").clicked() {
                        ui.close_menu();
                        let dialog = rfd::FileDialog::new()
                            .set_title("Import Background Image")
                            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]);
                        if let Some(path) = dialog.pick_file() {
                            match std::fs::read(&path) {
                                Ok(data) => {
                                    let name = path.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "image".into());
                                    match grove_core::background_image::BackgroundImage::from_bytes(name, data) {
                                        Ok(img) => {
                                            self.project.add_background_image(img);
                                        }
                                        Err(e) => log::error!("Failed to import image: {e}"),
                                    }
                                }
                                Err(e) => log::error!("Failed to read file: {e}"),
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let undo_label = match self.history.undo_description() {
                        Some(desc) => format!("Undo: {desc}"),
                        None => "Undo".to_string(),
                    };
                    if ui
                        .add_enabled(self.history.can_undo(), egui::Button::new(undo_label))
                        .clicked()
                    {
                        self.history.undo(&mut self.project);
                        ui.close_menu();
                    }
                    let redo_label = match self.history.redo_description() {
                        Some(desc) => format!("Redo: {desc}"),
                        None => "Redo".to_string(),
                    };
                    if ui
                        .add_enabled(self.history.can_redo(), egui::Button::new(redo_label))
                        .clicked()
                    {
                        self.history.redo(&mut self.project);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Select All").clicked() {
                        for e in &self.project.entities {
                            self.selection.add(e.id);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let has_selection = !self.selection.is_empty();
                    let has_clipboard = !self.clipboard.is_empty();
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Copy  Ctrl+C"))
                        .clicked()
                    {
                        self.copy_selection_to_clipboard();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_clipboard, egui::Button::new("Paste  Ctrl+V"))
                        .clicked()
                    {
                        self.paste_from_clipboard();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Cut  Ctrl+X"))
                        .clicked()
                    {
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
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Duplicate  Ctrl+D"))
                        .clicked()
                    {
                        self.duplicate_selection();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom to Fit").clicked() {
                        let mut combined: Option<BoundingBox> = None;
                        for e in &self.project.entities {
                            if let Some(bb) = e.bounding_box() {
                                combined = Some(match combined {
                                    Some(c) => c.union(&bb),
                                    None => bb,
                                });
                            }
                        }
                        if let Some(bb) = combined {
                            let expanded = bb.expand(200.0);
                            self.canvas.viewport.fit_to_rect(
                                expanded.min,
                                expanded.max,
                                egui::vec2(800.0, 600.0),
                                40.0,
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Reset View").clicked() {
                        self.canvas.viewport.offset = glam::Vec2::new(400.0, 300.0);
                        self.canvas.viewport.zoom = 0.5;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.snap_config.enabled, "Snapping");
                    ui.checkbox(&mut self.snap_config.grid_enabled, "Grid Snap");
                    ui.checkbox(&mut self.snap_config.endpoint_enabled, "Endpoint Snap");
                    ui.checkbox(&mut self.snap_config.midpoint_enabled, "Midpoint Snap");
                });
                ui.menu_button("Draw", |ui| {
                    if ui.button("Line").clicked() {
                        self.switch_tool(ActiveToolType::DrawLine);
                        ui.close_menu();
                    }
                    if ui.button("Rectangle").clicked() {
                        self.switch_tool(ActiveToolType::DrawRect);
                        ui.close_menu();
                    }
                    if ui.button("Arc").clicked() {
                        self.switch_tool(ActiveToolType::DrawArc);
                        ui.close_menu();
                    }
                    if ui.button("Circle").clicked() {
                        self.switch_tool(ActiveToolType::DrawCircle);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Text").clicked() {
                        self.switch_tool(ActiveToolType::Text);
                        ui.close_menu();
                    }
                    if ui.button("Dimension").clicked() {
                        self.switch_tool(ActiveToolType::Dimension);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Garden", |ui| {
                    if ui.button("Place Plant").clicked() {
                        self.switch_tool(ActiveToolType::PlacePlant);
                        ui.close_menu();
                    }
                    if ui.button("Place Group").clicked() {
                        self.switch_tool(ActiveToolType::PlaceGroup);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Place Hardscape").clicked() {
                        self.switch_tool(ActiveToolType::PlaceHardscape);
                        ui.close_menu();
                    }
                    if ui.button("Place Level").clicked() {
                        self.switch_tool(ActiveToolType::PlaceLevel);
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.colored_label(
                        ThemeColours::TEXT_DIM,
                        format!("Database: {} plants", self.plant_db.plant_count()),
                    );
                });
                ui.menu_button("Export", |ui| {
                    if ui.button("Export SVG...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export SVG")
                            .add_filter("SVG", &["svg"])
                            .save_file()
                            .map(|p| ensure_extension(p, "svg"))
                        {
                            let options = grove_core::export::svg::SvgExportOptions::default();
                            if let Err(e) = grove_core::export::svg::export_svg_to_file(
                                &self.project, &options, &path,
                            ) {
                                log::error!("SVG export failed: {e}");
                            }
                        }
                    }
                    if ui.button("Export Planting Schedule CSV...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export Planting Schedule")
                            .add_filter("CSV", &["csv"])
                            .save_file()
                            .map(|p| ensure_extension(p, "csv"))
                        {
                            if let Err(e) = grove_core::export::csv_export::export_planting_csv(
                                &self.project.entities, &path,
                            ) {
                                log::error!("CSV export failed: {e}");
                            }
                        }
                    }
                    // Drawing SVG export (available when a drawing is open)
                    if let WorkspaceMode::Drawing(drawing_id) = &self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(*drawing_id) {
                            let label = format!("Export Drawing SVG ({})...", drawing.title);
                            if ui.button(label).clicked() {
                                ui.close_menu();
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Export Drawing SVG")
                                    .set_file_name(format!("{}-{}.svg", drawing.full_sheet_ref(), drawing.title))
                                    .add_filter("SVG", &["svg"])
                                    .save_file()
                                    .map(|p| ensure_extension(p, "svg"))
                                {
                                    let options = grove_core::export::svg::SvgExportOptions::default();
                                    if let Err(e) = grove_core::export::svg::export_drawing_svg_to_file(
                                        &self.project, drawing, &options, &path,
                                    ) {
                                        log::error!("Drawing SVG export failed: {e}");
                                    }
                                }
                            }
                            // Drawing PDF export
                            let pdf_label = format!("Export Drawing PDF ({})...", drawing.title);
                            if ui.button(pdf_label).clicked() {
                                ui.close_menu();
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Export Drawing PDF")
                                    .set_file_name(format!("{}-{}.pdf", drawing.full_sheet_ref(), drawing.title))
                                    .add_filter("PDF", &["pdf"])
                                    .save_file()
                                    .map(|p| ensure_extension(p, "pdf"))
                                {
                                    if let Err(e) = grove_core::export::pdf::export_drawing_pdf(
                                        &self.project, drawing, &path,
                                    ) {
                                        log::error!("Drawing PDF export failed: {e}");
                                    }
                                }
                            }
                        }
                    }
                    // Export All Drawings
                    if !self.project.drawings.is_empty() {
                        let label = format!("Export All Drawings ({})...", self.project.drawings.len());
                        if ui.button(label).clicked() {
                            ui.close_menu();
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Export All Drawings — Choose Folder")
                                .pick_folder()
                            {
                                let options = grove_core::export::svg::SvgExportOptions::default();
                                match grove_core::export::svg::export_all_drawings_svg(
                                    &self.project, &options, &dir,
                                ) {
                                    Ok(count) => log::info!("Exported {count} drawings to {}", dir.display()),
                                    Err(e) => log::error!("Batch drawing export failed: {e}"),
                                }
                            }
                        }
                    }
                    if ui.button("Export JSON...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export JSON")
                            .add_filter("JSON", &["json"])
                            .save_file()
                            .map(|p| ensure_extension(p, "json"))
                        {
                            if let Err(e) = grove_core::export::json::export_json_to_file(
                                &self.project, &path,
                            ) {
                                log::error!("JSON export failed: {e}");
                            }
                        }
                    }
                });
                // Workspace mode toggle
                ui.separator();
                let in_model = self.workspace_mode == WorkspaceMode::Model;
                if ui.selectable_label(in_model, "Model").clicked() {
                    self.workspace_mode = WorkspaceMode::Model;
                }
                let in_drawing = matches!(self.workspace_mode, WorkspaceMode::Drawing(_));
                if ui.selectable_label(in_drawing, "Drawing").on_hover_text("Switch to drawing mode").clicked() {
                    // If no drawing is open, open the first one (or stay in model)
                    if let Some(d) = self.project.drawings.first() {
                        self.workspace_mode = WorkspaceMode::Drawing(d.id);
                    }
                }

                ui.menu_button("Help", |ui| {
                    if ui.button("About Grove").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });

        // Status bar
        TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
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

        // Left toolbar (hidden in Drawing mode) — two-column layout
        if matches!(self.workspace_mode, WorkspaceMode::Model) {
        SidePanel::left("toolbar")
            .exact_width(130.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Gather context for column 2 before entering closure
                let hardscape_sub_type = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::place_hardscape::PlaceHardscapeTool>()
                        .map(|ht| ht.sub_type)
                });
                let level_sub_type = self.draw_tool.as_ref().and_then(|t| {
                    t.as_any_ref()
                        .downcast_ref::<crate::tools::place_level::PlaceLevelTool>()
                        .map(|lt| lt.sub_type)
                });
                let shapes = available_shapes(self.tool_mode, hardscape_sub_type, level_sub_type);

                let blue = Color32::from_rgb(60, 90, 140);
                let green = Color32::from_rgb(50, 120, 60);
                let red = Color32::from_rgb(150, 50, 50);

                // Current mode colour for column 2 buttons
                let mode_colour = match self.tool_mode {
                    ToolMode::Select | ToolMode::Move | ToolMode::Rotate | ToolMode::Shape => Some(blue),
                    ToolMode::Hardscape | ToolMode::Plant | ToolMode::Group | ToolMode::Level => Some(green),
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

                grove_widgets::toolbar::toolbar_two_column(ui,
                    // ─── Column 1: Mode buttons ───
                    |ui| {
                        // Select
                        if grove_widgets::toolbar::tool_button(ui, "Select", self.tool_mode == ToolMode::Select).clicked() {
                            mode_action = Some(ToolMode::Select);
                        }
                        // Move
                        if grove_widgets::toolbar::tool_button(ui, "Move", self.tool_mode == ToolMode::Move).clicked() {
                            mode_action = Some(ToolMode::Move);
                        }
                        // Rotate
                        if grove_widgets::toolbar::tool_button(ui, "Rotate", self.tool_mode == ToolMode::Rotate).clicked() {
                            mode_action = Some(ToolMode::Rotate);
                        }
                        // Delete (action button, not a mode)
                        if grove_widgets::toolbar::tool_button(ui, "Delete", false).clicked() {
                            delete_action = true;
                        }

                        grove_widgets::toolbar::tool_separator(ui);

                        // Shape
                        if grove_widgets::toolbar::tool_button(ui, "Shape", self.tool_mode == ToolMode::Shape).clicked() {
                            mode_action = Some(ToolMode::Shape);
                        }
                        // Hardscape
                        if grove_widgets::toolbar::tool_button_colored(ui, "Hardsc.", self.tool_mode == ToolMode::Hardscape, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Hardscape);
                        }
                        // Plant
                        if grove_widgets::toolbar::tool_button_colored(ui, "Plant", self.tool_mode == ToolMode::Plant, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Plant);
                        }
                        // Group
                        if grove_widgets::toolbar::tool_button_colored(ui, "Group", self.tool_mode == ToolMode::Group, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Group);
                        }
                        // Level
                        if grove_widgets::toolbar::tool_button_colored(ui, "Level", self.tool_mode == ToolMode::Level, Some(green)).clicked() {
                            mode_action = Some(ToolMode::Level);
                        }

                        grove_widgets::toolbar::tool_separator(ui);

                        // Cutout
                        if grove_widgets::toolbar::tool_button_colored(ui, "Cutout", self.tool_mode == ToolMode::Cutout, Some(red)).clicked() {
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
                            if grove_widgets::toolbar::tool_button_colored(
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
                            "Delete entities",
                        ));
                        self.history.execute(cmd, &mut self.project);
                    }
                    self.selection.clear();
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

        // Plant palette (shown when Plant or Group tool active, model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && (self.active_tool_type == ActiveToolType::PlacePlant
                || self.active_tool_type == ActiveToolType::PlaceGroup)
        {
            SidePanel::left("plant_palette")
                .default_width(260.0)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    if let Some(plant) = crate::panels::palette::draw_palette(
                        ui,
                        &self.plant_db,
                        &mut self.palette_state,
                    ) {
                        self.selected_plant = Some(plant.clone());
                        // Update the active tool with the new plant
                        if let Some(ref mut tool) = self.draw_tool {
                            if let Some(pt) = tool.as_any_mut().downcast_mut::<crate::tools::place_plant::PlacePlantTool>() {
                                pt.set_plant(plant.clone());
                            }
                            if let Some(gt) = tool.as_any_mut().downcast_mut::<crate::tools::place_group::PlaceGroupTool>() {
                                gt.set_plant(plant);
                            }
                        }
                    }
                });
        }

        // Hardscape palette (model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && self.active_tool_type == ActiveToolType::PlaceHardscape
        {
            SidePanel::left("hardscape_palette")
                .default_width(240.0)
                .width_range(200.0..=360.0)
                .show(ctx, |ui| {
                    if let Some(ref mut tool) = self.draw_tool {
                        if let Some(ht) = tool.as_any_mut().downcast_mut::<crate::tools::place_hardscape::PlaceHardscapeTool>() {
                            if crate::panels::hardscape_palette::draw_hardscape_palette(ui, ht) {
                                if let Some(layer) = self.project.layers.find_by_name(ht.target_layer_name()) {
                                    ht.layer_id = layer.id;
                                }
                            }
                        }
                    }
                });
        }

        // Level palette (model mode only)
        if matches!(self.workspace_mode, WorkspaceMode::Model)
            && self.active_tool_type == ActiveToolType::PlaceLevel
        {
            SidePanel::left("level_palette")
                .default_width(240.0)
                .width_range(200.0..=360.0)
                .show(ctx, |ui| {
                    if let Some(ref mut tool) = self.draw_tool {
                        if let Some(lt) = tool.as_any_mut().downcast_mut::<crate::tools::place_level::PlaceLevelTool>() {
                            if crate::panels::level_palette::draw_level_palette(ui, lt) {
                                if let Some(layer) = self.project.layers.find_by_name(lt.target_layer_name()) {
                                    lt.layer_id = layer.id;
                                }
                            }
                        }
                    }
                });
        }

        // Left annotation palette (shown in Drawing mode, replacing the model-space palettes)
        let mut annotation_action: Option<crate::panels::annotation_palette::AnnotationPaletteAction> = None;
        if let WorkspaceMode::Drawing(drawing_id) = self.workspace_mode {
            SidePanel::left("annotation_palette")
                .default_width(240.0)
                .width_range(200.0..=360.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(drawing) = self.project.find_drawing(drawing_id) {
                            annotation_action = crate::panels::annotation_palette::draw_annotation_palette(ui, drawing);
                        }
                    });
                });
        }

        // Right panel
        let mut props_change: Option<(uuid::Uuid, grove_core::entity::DomainData)> = None;
        let mut drawing_action: Option<crate::panels::drawing_properties::DrawingPropertiesAction> = None;
        let mut drawing_mgr_action: Option<crate::panels::drawing_manager::DrawingManagerAction> = None;
        SidePanel::right("properties_panel")
            .default_width(220.0)
            .width_range(180.0..=400.0)
            .show(ctx, |ui| {
                if let WorkspaceMode::Drawing(drawing_id) = self.workspace_mode {
                    // Drawing mode: show drawing properties
                    if let Some(drawing) = self.project.find_drawing(drawing_id) {
                        drawing_action = crate::panels::drawing_properties::draw_drawing_properties(
                            ui, drawing, &self.project.layers, &self.project.entities,
                        );
                    } else {
                        ui.label("Drawing not found.");
                        if ui.button("Back to Model").clicked() {
                            self.workspace_mode = WorkspaceMode::Model;
                        }
                    }
                } else {
                // Model mode: show entity properties + layers + drawing manager
                if self.selection.ids.len() > 1 {
                    crate::panels::properties::draw_multi_selection(ui, self.selection.ids.len());
                } else if self.selection.single().is_some() {
                    props_change = crate::panels::properties::draw_properties_panel(
                        ui, &self.project, &self.selection,
                    );
                } else {
                    crate::panels::properties::draw_no_selection(ui);
                }

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.heading("Layers");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+").on_hover_text("Add layer").clicked() {
                            let n = self.project.layers.layers.len();
                            let mut layer = grove_core::layer::Layer::new(
                                format!("Layer {}", n + 1),
                                [200, 200, 200, 255],
                            );
                            layer.order = n as i32;
                            self.project.layers.add_layer(layer);
                            self.project.dirty = true;
                        }
                    });
                });
                ui.separator();

                let active_id = self.project.layers.active_layer_id;
                let layer_count = self.project.layers.layers.len();
                let mut delete_idx: Option<usize> = None;
                let mut move_up_idx: Option<usize> = None;
                let mut move_down_idx: Option<usize> = None;

                for i in 0..layer_count {
                    let layer_id = self.project.layers.layers[i].id;
                    let layer_colour = self.project.layers.layers[i].colour;
                    let layer_visible = self.project.layers.layers[i].visible;
                    let layer_name = self.project.layers.layers[i].name.clone();
                    let is_active = layer_id == active_id;

                    ui.horizontal(|ui| {
                        let colour = Color32::from_rgba_premultiplied(
                            layer_colour[0],
                            layer_colour[1],
                            layer_colour[2],
                            layer_colour[3],
                        );
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, colour);

                        let vis_label = if layer_visible { "V" } else { "." };
                        if ui.small_button(vis_label).clicked() {
                            self.project.layers.layers[i].visible = !layer_visible;
                        }

                        // Rename mode
                        if let Some((rename_idx, ref mut buf)) = self.layer_rename {
                            if rename_idx == i {
                                let response = ui.text_edit_singleline(buf);
                                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let new_name = buf.clone();
                                    self.project.layers.layers[i].name = new_name;
                                    self.project.dirty = true;
                                    self.layer_rename = None;
                                }
                                return;
                            }
                        }

                        let label = if is_active {
                            egui::RichText::new(&layer_name).color(ThemeColours::TEXT_PRIMARY)
                        } else {
                            egui::RichText::new(&layer_name).color(ThemeColours::TEXT_SECONDARY)
                        };
                        let response = ui.selectable_label(is_active, label);
                        if response.clicked() {
                            self.project.layers.set_active(layer_id);
                        }
                        if response.double_clicked() {
                            self.layer_rename = Some((i, layer_name.clone()));
                        }
                        response.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                self.layer_rename = Some((i, layer_name.clone()));
                                ui.close_menu();
                            }
                            if i > 0 {
                                if ui.button("Move Up").clicked() {
                                    move_up_idx = Some(i);
                                    ui.close_menu();
                                }
                            }
                            if i < layer_count - 1 {
                                if ui.button("Move Down").clicked() {
                                    move_down_idx = Some(i);
                                    ui.close_menu();
                                }
                            }
                            if layer_count > 1 {
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    delete_idx = Some(i);
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                }

                // Apply deferred mutations
                if let Some(i) = move_up_idx {
                    self.project.layers.layers.swap(i, i - 1);
                    self.project.dirty = true;
                }
                if let Some(i) = move_down_idx {
                    self.project.layers.layers.swap(i, i + 1);
                    self.project.dirty = true;
                }
                if let Some(i) = delete_idx {
                    let removed_id = self.project.layers.layers[i].id;
                    self.project.layers.layers.remove(i);
                    if self.project.layers.active_layer_id == removed_id {
                        self.project.layers.active_layer_id = self.project.layers.layers[0].id;
                    }
                    self.project.dirty = true;
                }

                // Background Images section
                if !self.project.background_images.is_empty() {
                    ui.add_space(16.0);
                    ui.heading("Background");
                    ui.separator();

                    let mut remove_id: Option<uuid::Uuid> = None;
                    let mut start_adjust: Option<(uuid::Uuid, AdjustMode)> = None;

                    for img in &mut self.project.background_images {
                        ui.label(&img.name);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut img.visible, "Visible");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Opacity:");
                            ui.add(egui::Slider::new(&mut img.opacity, 0.0..=1.0).max_decimals(2));
                        });

                        let mode_label = match &img.colour_mode {
                            grove_core::background_image::ColourMode::Normal => "Normal",
                            grove_core::background_image::ColourMode::Inverted => "Inverted",
                            grove_core::background_image::ColourMode::Desaturated => "Desaturated",
                            grove_core::background_image::ColourMode::Tinted(_) => "Tinted",
                        };
                        egui::ComboBox::from_id_salt(format!("bg_colour_{}", img.id))
                            .selected_text(mode_label)
                            .show_ui(ui, |ui| {
                                let is_normal = matches!(img.colour_mode, grove_core::background_image::ColourMode::Normal);
                                if ui.selectable_label(is_normal, "Normal").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Normal;
                                }
                                let is_inv = matches!(img.colour_mode, grove_core::background_image::ColourMode::Inverted);
                                if ui.selectable_label(is_inv, "Inverted").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Inverted;
                                }
                                let is_desat = matches!(img.colour_mode, grove_core::background_image::ColourMode::Desaturated);
                                if ui.selectable_label(is_desat, "Desaturated").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Desaturated;
                                }
                                let is_tinted = matches!(img.colour_mode, grove_core::background_image::ColourMode::Tinted(_));
                                if ui.selectable_label(is_tinted, "Tinted").clicked() {
                                    img.colour_mode = grove_core::background_image::ColourMode::Tinted([180, 160, 120]);
                                }
                            });

                        if img.rotation_rad.abs() > 1e-6 {
                            ui.colored_label(
                                ThemeColours::TEXT_DIM,
                                format!("Rotation: {:.1}\u{00B0}", img.rotation_rad.to_degrees()),
                            );
                        }

                        ui.horizontal(|ui| {
                            if ui.button("Calibrate").clicked() {
                                start_adjust = Some((img.id, AdjustMode::CalibrateScale {
                                    first_point: None,
                                    awaiting_distance: false,
                                    distance_input: String::new(),
                                }));
                            }
                            if ui.button("Align").on_hover_text("Draw a line that should be horizontal").clicked() {
                                start_adjust = Some((img.id, AdjustMode::AlignHorizontal {
                                    first_point: None,
                                }));
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Flip H").on_hover_text("Flip horizontally").clicked() {
                                img.flip_h = !img.flip_h;
                            }
                            if ui.button("Flip V").on_hover_text("Flip vertically").clicked() {
                                img.flip_v = !img.flip_v;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Set Origin").on_hover_text("Click the point that should be (0,0)").clicked() {
                                start_adjust = Some((img.id, AdjustMode::SetOrigin));
                            }
                            if ui.button("Remove").clicked() {
                                remove_id = Some(img.id);
                            }
                        });
                        ui.separator();
                    }

                    // Adjustment mode status & distance input
                    if let Some(ref mut cal) = self.calibrate_state {
                        match &mut cal.mode {
                            AdjustMode::CalibrateScale { first_point, awaiting_distance, distance_input } => {
                                if *awaiting_distance {
                                    ui.add_space(4.0);
                                    ui.colored_label(ThemeColours::ACCENT, "Enter real distance (mm):");
                                    let response = ui.text_edit_singleline(distance_input);
                                    if response.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        if let Ok(real_mm) = distance_input.trim().parse::<f32>() {
                                            if real_mm > 0.0 {
                                                let img_id = cal.image_id;
                                                let p1 = first_point.unwrap();
                                                let p2 = self.calibrate_second_point.unwrap();
                                                if let Some(img) =
                                                    self.project.find_background_image_mut(img_id)
                                                {
                                                    let world_dist = ((p2.x - p1.x).powi(2)
                                                        + (p2.y - p1.y).powi(2))
                                                    .sqrt();
                                                    let pixel_dist = world_dist * img.pixels_per_mm;
                                                    img.pixels_per_mm = pixel_dist / real_mm;
                                                    self.project.dirty = true;
                                                }
                                            }
                                        }
                                        self.calibrate_state = None;
                                        self.calibrate_second_point = None;
                                    }
                                } else {
                                    let msg = if first_point.is_none() {
                                        "Click first calibration point"
                                    } else {
                                        "Click second calibration point"
                                    };
                                    ui.colored_label(ThemeColours::ACCENT, msg);
                                    if ui.button("Cancel").clicked() {
                                        self.calibrate_state = None;
                                    }
                                }
                            }
                            AdjustMode::AlignHorizontal { first_point } => {
                                let msg = if first_point.is_none() {
                                    "Click first point of horizontal line"
                                } else {
                                    "Click second point of horizontal line"
                                };
                                ui.colored_label(ThemeColours::ACCENT, msg);
                                if ui.button("Cancel").clicked() {
                                    self.calibrate_state = None;
                                }
                            }
                            AdjustMode::SetOrigin => {
                                ui.colored_label(ThemeColours::ACCENT, "Click the point that should be (0,0)");
                                if ui.button("Cancel").clicked() {
                                    self.calibrate_state = None;
                                }
                            }
                        }
                    }

                    if let Some((id, mode)) = start_adjust {
                        self.calibrate_state = Some(CalibrateState {
                            image_id: id,
                            mode,
                        });
                        self.calibrate_second_point = None;
                    }
                    if let Some(id) = remove_id {
                        self.project.remove_background_image(id);
                        self.bg_renderer.remove(id);
                        if self
                            .calibrate_state
                            .as_ref()
                            .map_or(false, |c| c.image_id == id)
                        {
                            self.calibrate_state = None;
                        }
                    }
                }

                // Drawing Manager section
                ui.add_space(16.0);
                drawing_mgr_action = crate::panels::drawing_manager::draw_drawing_manager(
                    ui, &self.project,
                );
                } // close else (model mode)
            });

        // Handle drawing manager actions
        if let Some(mgr_action) = drawing_mgr_action {
            match mgr_action {
                crate::panels::drawing_manager::DrawingManagerAction::OpenDrawing(id) => {
                    self.workspace_mode = WorkspaceMode::Drawing(id);
                }
                crate::panels::drawing_manager::DrawingManagerAction::CreateFromTemplate(template) => {
                    let number = self.project.next_drawing_number();
                    let drawing = grove_core::drawing::templates::create_from_template(
                        template, &number, &self.project.layers,
                    );
                    let id = drawing.id;
                    let cmd = grove_core::commands::AddDrawingCommand::new(drawing);
                    self.history.execute(Box::new(cmd), &mut self.project);
                    self.workspace_mode = WorkspaceMode::Drawing(id);
                }
                crate::panels::drawing_manager::DrawingManagerAction::RemoveDrawing(id) => {
                    let cmd = grove_core::commands::RemoveDrawingCommand::new(id);
                    self.history.execute(Box::new(cmd), &mut self.project);
                    if self.workspace_mode == WorkspaceMode::Drawing(id) {
                        self.workspace_mode = WorkspaceMode::Model;
                    }
                }
            }
        }

        // Handle drawing properties actions
        if let Some(dp_action) = drawing_action {
            match dp_action {
                crate::panels::drawing_properties::DrawingPropertiesAction::Updated(new_drawing) => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(old) = self.project.find_drawing(id) {
                            let old_clone = old.clone();
                            let cmd = grove_core::commands::UpdateDrawingCommand::new(id, old_clone, new_drawing);
                            self.history.execute(Box::new(cmd), &mut self.project);
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::ExportSvg => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(id) {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export Drawing SVG")
                                .set_file_name(format!("{}-{}.svg", drawing.full_sheet_ref(), drawing.title))
                                .add_filter("SVG", &["svg"])
                                .save_file()
                                .map(|p| ensure_extension(p, "svg"))
                            {
                                let options = grove_core::export::svg::SvgExportOptions::default();
                                if let Err(e) = grove_core::export::svg::export_drawing_svg_to_file(
                                    &self.project, drawing, &options, &path,
                                ) {
                                    log::error!("Drawing SVG export failed: {e}");
                                }
                            }
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::ExportPdf => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(drawing) = self.project.find_drawing(id) {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export Drawing PDF")
                                .set_file_name(format!("{}-{}.pdf", drawing.full_sheet_ref(), drawing.title))
                                .add_filter("PDF", &["pdf"])
                                .save_file()
                                .map(|p| ensure_extension(p, "pdf"))
                            {
                                if let Err(e) = grove_core::export::pdf::export_drawing_pdf(
                                    &self.project, drawing, &path,
                                ) {
                                    log::error!("Drawing PDF export failed: {e}");
                                }
                            }
                        }
                    }
                }
                crate::panels::drawing_properties::DrawingPropertiesAction::BackToModel => {
                    self.workspace_mode = WorkspaceMode::Model;
                }
            }
        }

        // Handle annotation palette actions
        if let Some(ann_action) = annotation_action {
            match ann_action {
                crate::panels::annotation_palette::AnnotationPaletteAction::Updated(new_drawing) => {
                    if let WorkspaceMode::Drawing(id) = self.workspace_mode {
                        if let Some(old) = self.project.find_drawing(id) {
                            let old_clone = old.clone();
                            let cmd = grove_core::commands::UpdateDrawingCommand::new(id, old_clone, new_drawing);
                            self.history.execute(Box::new(cmd), &mut self.project);
                        }
                    }
                }
                crate::panels::annotation_palette::AnnotationPaletteAction::StartAnnotationMode => {
                    self.drawing_interaction.annotation_mode = true;
                }
                crate::panels::annotation_palette::AnnotationPaletteAction::SetLeader(ann_id) => {
                    self.drawing_interaction.setting_leader = Some(ann_id);
                }
            }
        }

        // Apply property edits from the properties panel
        if let Some((id, new_domain)) = props_change {
            if let Some(entity) = self.project.find_entity(id) {
                let old_domain = entity.domain.clone();
                let cmd = grove_core::commands::UpdateDomainDataCommand::new(id, old_domain, new_domain);
                self.history.execute(Box::new(cmd), &mut self.project);
            }
        }

        // Central canvas
        CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
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
                                let factor = if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
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
                                crate::view::drawing_view::DrawingViewAction::CreatedViewport(new_vp) => {
                                    if let Some(d) = self.project.find_drawing_mut(drawing_id) {
                                        d.viewports.push(new_vp);
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
                    self.bg_renderer.ensure_texture(ctx, img);
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
                    let suppress = self.calibrate_state.as_ref().map_or(false, |c| {
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

                // Copy selected IDs to avoid borrow conflict with selection mutations
                let selected_ids_copy: Vec<uuid::Uuid> = self.selection.ids.clone();

                // Build tool context from copies (avoids borrow issues)
                let tool_ctx = ToolContext {
                    entities: &self.project.entities,
                    snap_config: &self.snap_config,
                    shift_held: shift,
                    ctrl_held: ctrl,
                    active_layer_id: self.project.layers.active_layer_id,
                    viewport_zoom: self.canvas.viewport.zoom,
                    selected_ids: &selected_ids_copy,
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
                    if matches!(result, ToolResult::Continue) {
                        if ctx.input(|i| i.key_pressed(egui::Key::C)) {
                            result = tool.on_key_pressed(egui::Key::C);
                        }
                    }

                    // Put tool back before handling result (which may clear it)
                    self.draw_tool = Some(tool);
                    drop(tool_ctx);
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

                    // Escape clears selection
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.selection.clear();
                        self.bezier_edit = None;
                        self.box_select_start = None;
                    }

                    // Shift+Right-click: place anchor point (with snapping)
                    if shift && canvas_result.response.clicked_by(egui::PointerButton::Secondary) {
                        let snapped = if self.snap_config.enabled {
                            let candidates = crate::tools::snap_candidates_from_entities(
                                pos, &self.project.entities, &self.snap_config,
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

                    // Modification shortcuts (Blender-style, when selection is non-empty)
                    if !self.selection.is_empty() && !ctrl {
                        if ctx.input(|i| i.key_pressed(egui::Key::G)) {
                            drop(tool_ctx);
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Move);
                            return;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                            drop(tool_ctx);
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Rotate);
                            return;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                            drop(tool_ctx);
                            self.activate_modify_tool(crate::tools::modify::ModifyMode::Scale);
                            return;
                        }
                        if shift && ctx.input(|i| i.key_pressed(egui::Key::D)) {
                            drop(tool_ctx);
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
                                    drop(tool_ctx);
                                    let tool = crate::tools::subtract_polygon::SubtractPolygonTool::new(sel_id);
                                    self.draw_tool = Some(Box::new(tool));
                                    self.active_tool_type = ActiveToolType::SubtractPolygon;
                                    self.tool_mode = ToolMode::Cutout;
                                    return;
                                }
                            }
                        }
                    }

                    // Delete key (X, Delete, or Backspace)
                    if ctx.input(|i| {
                        i.key_pressed(egui::Key::Delete)
                            || i.key_pressed(egui::Key::Backspace)
                            || (!ctrl && i.key_pressed(egui::Key::X))
                    }) {
                        let ids: Vec<_> = self.selection.ids.clone();
                        drop(tool_ctx);
                        if !ids.is_empty() {
                            let commands: Vec<Box<dyn grove_core::commands::Command>> = ids
                                .iter()
                                .map(|&id| -> Box<dyn grove_core::commands::Command> {
                                    Box::new(grove_core::commands::RemoveEntityCommand::new(id))
                                })
                                .collect();
                            let cmd = Box::new(grove_core::commands::CompoundCommand::new(
                                commands,
                                "Delete entities",
                            ));
                            self.history.execute(cmd, &mut self.project);
                        }
                        self.selection.clear();
                    }
                }
            });
    }
}
