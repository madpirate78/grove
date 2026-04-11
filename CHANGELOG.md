# Changelog

All notable changes to Grove are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Grove is pre-1.0 and not yet using semantic versioning; releases are tracked
as dated sections.

## [Unreleased]

### Changed
- **Consolidated polygon and width-shape rendering** into two shared helpers (`render_polygon_mesh`, `render_width_shape`) in `crates/grove-app/src/view/canvas.rs`, parameterised over the world→screen transform closure. Removes ~70 lines of copy-paste between `canvas.rs` (model page) and `drawing_view.rs` (drawing page), and keeps the phantom-tangent mesh-batching fix in a single implementation. Hardscape tool previews updated to use the shared helper as well.

### Fixed
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
