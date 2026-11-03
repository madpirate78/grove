# Changelog

All notable changes to Grove are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Grove is pre-1.0 and not yet using semantic versioning; releases are tracked
as dated sections.

## [Unreleased]

### Added
- **Open-source release prep** — Grove is now MIT licensed (`LICENSE`), with licence/repository metadata shared across both crates via `[workspace.package]` a project `README.md`, and a `ROADMAP.md` of planned work. The entourage `LICENSES.md` was stale: it credited third-party symbols that had already been removed and left out every symbol actually shipped. `scripts/fetch_entourage.py` now lists any on-disk symbol without a manifest entry as original MIT artwork, and always attributes from the full manifest (previously `--only <category>` dropped other categories' attributions).

### Removed
- **Legacy import tooling and docs from the project's origins** — a 3D-model → SVG conversion route in the entourage scripts and its dead manifest entries; the unused `dxf` workspace dependency; and the original porting plan, whose still-relevant design decisions now live in `docs/architecture.md`. The domain spec was renamed to `docs/data-model.md` and rewritten as Grove's own data model; its never-built Blender export pipeline moved to `docs/future-features.md`.
- **Cost estimate window + shareable rate cards (Estimate Builder Phase B)** — Garden → "Cost Estimate…" opens a floating window with the live priced bill of quantities and editable unit rates (per-plant, per-m², per-linear-metre, per-item, material multipliers, currency). Rates persist across sessions in the app config dir and drive the CSV export. Profiles export/import as JSON rate cards so landscapers can share regional pricing.
- **Cost estimate export (Estimate Builder Phase A)** — `garden::estimate` generates an offline bill of quantities: plants counted per species (matching the planting schedule), walls/fences/edging/steps priced per linear metre of centerline, patios/surfaces/decking/raised beds/pergolas/ponds per square metre (holes subtracted), width-based paths as length × width. Default UK unit rates with per-material multipliers in a serde-ready `UnitCosts` table (ground-work for Phase B shareable rate cards). New Export → "Export Cost Estimate CSV…" menu item.
- **Procedural canopy symbols** for the `Canopy` plant style — `garden::canopy` generates deterministic outlines seeded by species (scalloped for deciduous trees, spiky for evergreens and grasses, lobed for shrubs) and rotated per instance from the plant's position, so plans don't look rubber-stamped. Rendered as filled vector paths in both the drawing view (shared polygon-mesh helper) and the drawing SVG export, replacing the concentric-ring placeholder.
- **Per-viewport plant rendering styles** — `PlantStyle` (Simple / Canopy / Numbered / Outline / Silhouette) on `DrawingViewport`, selectable in the drawing properties panel and honoured by both the drawing view and drawing SVG export. `Numbered` renders the plant's row number in the planting schedule (deterministic order: category, then botanical name), and the CSV planting schedule gained a matching `#` column so markers key into it.

### Changed
- **Upgraded egui/eframe 0.31 → 0.36** (five releases). Migration highlights: `eframe::App::update(ctx, …)` became `App::ui(ui, …)` with panels built on the root `Ui`; `SidePanel`/`TopBottomPanel` unified into `egui::Panel` (`default_size`/`exact_size`/`size_range` replace the width/height-specific builders); `egui::menu::bar` became `egui::MenuBar`; `ui.close_menu()` became `ui.close()`; `Context::set_style` became `set_global_style`. The Ctrl+C/V/X event-interception workaround in `shortcuts.rs` is still required (egui-winit 0.36 converts those keys to `Event::Copy/Cut/Paste` exactly as 0.31 did), and the `stroke_closed_loop` miter-spike workaround is retained.
- **Split `export/svg.rs` (1,674 lines) into focused child modules** under `export/svg/` — `model` (whole-project export), `drawing` (paper sheets, viewports, title block, legend), `plants` (plant symbols and fitted labels), `primitives` (shared geometry-to-SVG conversion), and `tests`. Code moved verbatim, same pattern as the `app.rs` split; the public API (`export::svg::*`) is unchanged via re-exports.
- **Deduplicated cut/delete command building** — the `RemoveEntityCommand` + `CompoundCommand` block was copy-pasted in four places (keyboard shortcuts, Edit menu, toolbar, canvas Delete key). Now a single `delete_selection`/`cut_selection` pair in `app/edit_ops.rs`, so delete semantics live in one spot.
- **Split `app.rs` (3,110 lines) into focused child modules** under `src/app/` — shortcuts, file_ops, edit_ops, menu_bar, status_bar, toolbar, palettes, right_panel, canvas_panel, bezier_edit — with `update()` reduced to a short orchestrator. Code moved verbatim; modules are children of the `app` module so private-field access is unchanged.
- **Folded `grove-widgets` into `grove-app`** (`src/widgets.rs`); the crate was 127 lines with half of it unused. Workspace is now two crates: `grove-core` and `grove-app`.
- **Fixed all 57 clippy warnings** — removed dead code (unused `Tool::name`, `SelectTool`, dead enum variants/fields), replaced unwrap-after-`is_some` patterns with `if let`/`match`, boxed the large `Drawing` payload in panel action enums, and wired the previously-dead entourage texture-cache purge into New/Open project (fixes a small leak).
- **Consolidated polygon and width-shape rendering** into two shared helpers (`render_polygon_mesh`, `render_width_shape`) in `crates/grove-app/src/view/canvas.rs`, parameterised over the world→screen transform closure. Removes ~70 lines of copy-paste between `canvas.rs` (model page) and `drawing_view.rs` (drawing page), and keeps the phantom-tangent mesh-batching fix in a single implementation. Hardscape tool previews updated to use the shared helper as well.

### Fixed
- **Legend, planting schedule, and cost estimate now count plants identically** — the drawing legend counted `PlantGroup.count` but never looked inside child entities, while the schedule and bill of quantities recursed into children but ignored `PlantGroup` entirely, so the same drawing could show different quantities in its legend and its schedule. All three now walk the entity tree the same way: every `Plant` entity counts as one and every `PlantGroup` contributes its `count`, with an invariant test pinning legend totals to schedule totals.
- **Plant labels overflowing their symbols** — labels now shrink to fit within the plant circle and are dropped entirely when they'd be illegibly small, instead of rendering at a fixed minimum font that overflowed onto neighbouring plants. Applied consistently in the model canvas, drawing view (all plant styles incl. schedule numbers), and SVG export (estimated glyph metrics, since no text measurement exists at generation time).
- **Miter spikes at sharp polygon and width-shape corners** — worked around egui 0.31's [emilk/egui#1226](https://github.com/emilk/egui/issues/1226), in which `add_line_loop` hardcodes the sharp-corner cut-off to `false`, causing closed stroked paths to shoot miter offsets toward infinity at acute corners. New `stroke_closed_loop` helper in `canvas.rs` strokes closed loops as open paths with the first vertex duplicated at the end, routing through egui's `add_open_points` tessellator which applies the cut-off correctly. Fixes visible artifacts on polygon boundaries (outer and holes) and on wall/path/edging outlines.

## [2026-03-18 — edge trace & hardscape polish] — 2e0491e

### Added
- Edge trace feature for the polygon drawing tool — `visible_edges()` in `grove-core` extracts snapable boundaries from existing geometry (natural boundaries for shapes, left/right offset sides for width entities), enabling snap-to-edge while drawing polygons.
- Hardscape palette improvements and assorted tool refinements.

## [2026-03-11 — polygon tools & boolean ops] — 6f25d2e

### Added
- Polygon drawing tools.
- Boolean operations on polygons.

### Fixed
- **Phantom tangents inside filled polygons** — polygon fills now render as a single batched `egui::Mesh` rather than many small `convex_polygon` calls, preventing egui from anti-aliasing the internal triangulation edges. (This fix is distinct from the closed-path miter spike fixed in [Unreleased].)

## [2026-03-11 — drawings, viewport modes, SVG export] — fc6d234

### Added
- Drawing annotations (labels with leader lines).
- Viewport colour modes (default / by category / by species / monochrome) for print-oriented rendering.
- Enhanced SVG export.

## [2026-03-09 — initial commit] — 6ced1bb

### Added
- Initial Grove 2D CAD garden design application — three-crate workspace (`grove-core`, `grove-widgets`, `grove-app`), entity system with `Geometry` + `DomainData`, tool/command split with undo, egui canvas with pan/zoom/grid, persistence via rusqlite, base drawing tools, and the model/drawing workspace modes.
