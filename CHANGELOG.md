# Changelog

All notable changes to Grove are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Grove is pre-1.0 and not yet using semantic versioning; releases are tracked
as dated sections.

## [Unreleased]

### Added
- **Procedural canopy symbols** for the `Canopy` plant style — `garden::canopy` generates deterministic outlines seeded by species (scalloped for deciduous trees, spiky for evergreens and grasses, lobed for shrubs) and rotated per instance from the plant's position, so plans don't look rubber-stamped. Rendered as filled vector paths in both the drawing view (shared polygon-mesh helper) and the drawing SVG export, replacing the concentric-ring placeholder.
- **Per-viewport plant rendering styles** — `PlantStyle` (Simple / Canopy / Numbered / Outline / Silhouette) on `DrawingViewport`, selectable in the drawing properties panel and honoured by both the drawing view and drawing SVG export. `Numbered` renders the plant's row number in the planting schedule (deterministic order: category, then botanical name), and the CSV planting schedule gained a matching `#` column so markers key into it.

### Changed
- **Split `app.rs` (3,110 lines) into focused child modules** under `src/app/` — shortcuts, file_ops, edit_ops, menu_bar, status_bar, toolbar, palettes, right_panel, canvas_panel, bezier_edit — with `update()` reduced to a short orchestrator. Code moved verbatim; modules are children of the `app` module so private-field access is unchanged.
- **Folded `grove-widgets` into `grove-app`** (`src/widgets.rs`); the crate was 127 lines with half of it unused. Workspace is now two crates: `grove-core` and `grove-app`.
- **Fixed all 57 clippy warnings** — removed dead code (unused `Tool::name`, `SelectTool`, dead enum variants/fields), replaced unwrap-after-`is_some` patterns with `if let`/`match`, boxed the large `Drawing` payload in panel action enums, and wired the previously-dead entourage texture-cache purge into New/Open project (fixes a small leak).
- **Consolidated polygon and width-shape rendering** into two shared helpers (`render_polygon_mesh`, `render_width_shape`) in `crates/grove-app/src/view/canvas.rs`, parameterised over the world→screen transform closure. Removes ~70 lines of copy-paste between `canvas.rs` (model page) and `drawing_view.rs` (drawing page), and keeps the phantom-tangent mesh-batching fix in a single implementation. Hardscape tool previews updated to use the shared helper as well.

### Fixed
- **Plant labels overflowing their symbols** — labels now shrink to fit within the plant circle and are dropped entirely when they'd be illegibly small, instead of rendering at a fixed minimum font that overflowed onto neighbouring plants. Applied consistently in the model canvas, drawing view (all plant styles incl. schedule numbers), and SVG export (estimated glyph metrics, since no text measurement exists at generation time).
- **Miter spikes at sharp polygon and width-shape corners** — worked around egui 0.31's [emilk/egui#1226](https://github.com/emilk/egui/issues/1226), in which `add_line_loop` hardcodes the sharp-corner cut-off to `false`, causing closed stroked paths to shoot miter offsets toward infinity at acute corners. New `stroke_closed_loop` helper in `canvas.rs` strokes closed loops as open paths with the first vertex duplicated at the end, routing through egui's `add_open_points` tessellator which applies the cut-off correctly. Fixes visible artifacts on polygon boundaries (outer and holes) and on wall/path/edging outlines.

## [2026-04 — edge trace & hardscape polish] — 2e0491e

### Added
- Edge trace feature for the polygon drawing tool — `visible_edges()` in `grove-core` extracts snapable boundaries from existing geometry (natural boundaries for shapes, left/right offset sides for width entities), enabling snap-to-edge while drawing polygons.
- Hardscape palette improvements and assorted tool refinements.

## [2026-03 — polygon tools & boolean ops] — 6f25d2e

### Added
- Polygon drawing tools.
- Boolean operations on polygons.

### Fixed
- **Phantom tangents inside filled polygons** — polygon fills now render as a single batched `egui::Mesh` rather than many small `convex_polygon` calls, preventing egui from anti-aliasing the internal triangulation edges. (This fix is distinct from the closed-path miter spike fixed in [Unreleased].)

## [2026-02 — drawings, viewport modes, SVG export] — fc6d234

### Added
- Drawing annotations (labels with leader lines).
- Viewport colour modes (default / by category / by species / monochrome) for print-oriented rendering.
- Enhanced SVG export.

## [2026-01 — initial commit] — 6ced1bb

### Added
- Initial Grove 2D CAD garden design application — three-crate workspace (`grove-core`, `grove-widgets`, `grove-app`), entity system with `Geometry` + `DomainData`, tool/command split with undo, egui canvas with pan/zoom/grid, persistence via rusqlite, base drawing tools, and the model/drawing workspace modes.
