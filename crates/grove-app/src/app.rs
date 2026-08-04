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
use crate::tools::select::Selection;
use crate::tools::{Tool, ToolContext, ToolResult};
use crate::view::background::BackgroundImageRenderer;
use crate::view::canvas::Canvas;

mod bezier_edit;
mod canvas_panel;
mod edit_ops;
mod estimate;
mod file_ops;
mod menu_bar;
mod palettes;
mod right_panel;
mod shortcuts;
mod status_bar;
mod toolbar;

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
    PlaceEntourage,
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
            Self::PlaceEntourage => "Entourage",
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
    Entourage,
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

/// Returns the available shape sub-tools for a given mode and hardscape/level context.
pub fn available_shapes(
    mode: ToolMode,
    hardscape_tool: Option<&crate::tools::place_hardscape::PlaceHardscapeTool>,
    level_sub_type: Option<crate::tools::place_level::LevelSubType>,
) -> &'static [ShapeTool] {
    use crate::tools::place_hardscape::{HardscapeCategory, StructureKind};
    use crate::tools::place_level::LevelSubType;

    match mode {
        ToolMode::Shape => &[ShapeTool::Line, ShapeTool::Arc, ShapeTool::Circle, ShapeTool::Rect, ShapeTool::Text, ShapeTool::Dim],
        ToolMode::Hardscape => {
            match hardscape_tool.map(|ht| (ht.category, ht.structure_kind, ht.surface.width_mm.is_some())) {
                Some((HardscapeCategory::Walls | HardscapeCategory::Steps, _, _)) => {
                    &[ShapeTool::Line]
                }
                Some((HardscapeCategory::Structures, StructureKind::Fence, _)) => {
                    &[ShapeTool::Line]
                }
                Some((HardscapeCategory::Structures, StructureKind::Pergola, _)) => {
                    &[ShapeTool::Rect]
                }
                Some((HardscapeCategory::Surfaces, _, true)) => {
                    // Path mode (has width) — polyline only
                    &[ShapeTool::Line]
                }
                Some((HardscapeCategory::Surfaces | HardscapeCategory::Water | HardscapeCategory::Buildings, _, _)) => {
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
        ToolMode::Entourage => &[ShapeTool::Point],
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
/// An activated smart snap point that projects extension lines.
struct SmartPoint {
    position: glam::Vec2,
}

/// Tracks the current hover over a snap point for dwell detection.
struct SmartPointHover {
    position: glam::Vec2,
    started: std::time::Instant,
}

pub struct GroveApp {
    pub project: Project,
    pub canvas: Canvas,
    pub history: CommandHistory,
    pub selection: Selection,
    pub snap_config: SnapConfig,

    pub active_tool_type: ActiveToolType,
    pub tool_mode: ToolMode,
    pub shape_sub_tool: ShapeTool,
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

    /// Activated smart snap points that project H/V extension lines.
    smart_points: Vec<SmartPoint>,
    /// Current dwell hover tracking for smart point activation.
    smart_hover: Option<SmartPointHover>,

    /// Current workspace mode: Model or Drawing.
    pub workspace_mode: WorkspaceMode,

    /// Persistent interaction state for drawing mode (viewport drag/resize/create).
    drawing_interaction: crate::view::drawing_view::DrawingInteraction,

    /// Separate view transform for drawing mode (independent zoom/pan from model canvas).
    drawing_viewport: crate::view::viewport::ViewTransform,
    /// The drawing ID we last fit-to-page for (so we auto-fit on first entry).
    drawing_viewport_fitted_for: Option<uuid::Uuid>,

    // Cost estimate
    unit_costs: grove_core::garden::estimate::UnitCosts,
    estimate_window_open: bool,

    // Entourage
    entourage_library: grove_core::garden::entourage::EntourageLibrary,
    entourage_palette_state: crate::panels::entourage_palette::EntouragePaletteState,
    entourage_renderer: crate::view::entourage_renderer::EntourageRenderer,
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

        // Load entourage library from data/entourage/
        let entourage_library = {
            let search_paths = [
                "data/entourage",
                "../data/entourage",
                "../../data/entourage",
            ];
            let mut lib = grove_core::garden::entourage::EntourageLibrary::new();
            for path in &search_paths {
                let p = std::path::Path::new(path);
                if p.is_dir() {
                    lib = grove_core::garden::entourage::EntourageLibrary::load_from_directory(p);
                    break;
                }
            }
            lib
        };

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
            smart_points: Vec::new(),
            smart_hover: None,
            workspace_mode: WorkspaceMode::Model,
            drawing_interaction: crate::view::drawing_view::DrawingInteraction::default(),
            drawing_viewport: crate::view::viewport::ViewTransform::with_zoom_limits(0.5, 10.0),
            drawing_viewport_fitted_for: None,
            unit_costs: Self::load_unit_costs(),
            estimate_window_open: false,
            entourage_library,
            entourage_palette_state: crate::panels::entourage_palette::EntouragePaletteState::default(),
            entourage_renderer: crate::view::entourage_renderer::EntourageRenderer::new(),
        }
    }

    fn switch_tool(&mut self, tool_type: ActiveToolType) {
        self.active_tool_type = tool_type;
        self.bezier_edit = None;
        self.box_select_start = None;
        self.smart_points.clear();
        self.smart_hover = None;
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
            ActiveToolType::PlaceEntourage => ToolMode::Entourage,
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
            ActiveToolType::PlaceEntourage => {
                let mut tool = crate::tools::place_entourage::PlaceEntourageTool::new();
                if let Some(layer) = self.project.layers.find_by_name("GD_ENTOURAGE") {
                    tool.layer_id = layer.id;
                }
                // Auto-select first item from current category
                let cat = self.entourage_palette_state.active_category;
                if let Some(first) = self.entourage_library.items_for_category(cat).first() {
                    tool.set_item((*first).clone());
                }
                Some(Box::new(tool))
            }
            _ => None,
        };
        self.snap_label = None;
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
                    ShapeTool::Line => hardscape_override.is_none(),
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
            ToolMode::Plant | ToolMode::Group | ToolMode::Level | ToolMode::Entourage => true,
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
            ToolMode::Entourage => {
                self.shape_sub_tool = ShapeTool::Point;
                self.switch_tool(ActiveToolType::PlaceEntourage);
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
}

impl eframe::App for GroveApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if self.first_frame {
            theme::apply_theme(ctx);
            self.canvas.viewport.offset = glam::Vec2::new(400.0, 300.0);
            self.canvas.viewport.zoom = 0.5;
            self.first_frame = false;
        }

        // Must run before panels, which consume key events
        self.handle_global_shortcuts(ctx);
        self.tick_autosave(ctx);
        self.update_title(ctx);

        // Panel order matters in egui: top/bottom, then sides, central last
        self.show_menu_bar(ctx);
        self.show_status_bar(ctx);
        self.show_toolbar(ctx);
        self.show_left_palettes(ctx);
        self.show_right_panel(ctx);
        self.show_central_canvas(ctx);

        // Floating windows go after all panels
        self.show_estimate_window(ctx);
    }
}
