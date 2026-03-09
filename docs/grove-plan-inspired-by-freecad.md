Grove — Rust 2D CAD for Garden Design

 Context

 The current Garden Design Workbench is a FreeCAD plugin (Python, ~3,200 lines + 1,780 lines of tests). While
 functional, FreeCAD imposes significant UX overhead: slow startup, confusing 3D viewport for what is fundamentally a 2D
  task, pivy/Coin3D quirks, workbench switching, and decades of accumulated UI decisions that serve general-purpose CAD
 but hinder domain-specific workflows.

 The goal is to build a standalone Rust application — a 2D CAD tool with garden design as its primary focus. Not a pure
 planting planner (too narrow), but a 2D CAD that has first-class garden features: plant database, planting schedules,
 legends, distribution algorithms.

 Stack: egui + WGPU (native, single binary, proven for 2D CAD — MuginCAD validates this approach).

 ---
 What Transfers from the FreeCAD Workbench

 These modules are pure logic with zero FreeCAD dependency and port directly to Rust:

 ┌───────────────────────────────────────────────────────────────────────┬───────┬────────────────────────────┐
 │                                Module                                 │ Lines │      Port complexity       │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ db/database.py — SQLite plant DB, palettes, CSV import/export         │ 414   │ Straightforward (rusqlite) │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ tools/distribute.py — point-in-polygon, grid/hex/Poisson distribution │ 268   │ Direct algorithm port      │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ tools/schedule.py — aggregate counts, CSV export                      │ 125   │ Trivial                    │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ tools/colour.py — category colours, species palette                   │ 65    │ Trivial                    │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ data/plants.csv — 150 plants                                          │ —     │ Ships unchanged            │
 ├───────────────────────────────────────────────────────────────────────┼───────┼────────────────────────────┤
 │ data/palettes.csv — 15 preset palettes                                │ —     │ Ships unchanged            │
 └───────────────────────────────────────────────────────────────────────┴───────┴────────────────────────────┘

 These are replaced entirely (no code to port, just behaviour to replicate):

 ┌─────────────────────────────────────────────────┬───────────────────────────┬────────────────────────┐
 │                     Module                      │       What it does        │    Rust replacement    │
 ├─────────────────────────────────────────────────┼───────────────────────────┼────────────────────────┤
 │ objects/plant_object.py — FreeCAD FeaturePython │ Circle shape + properties │ Plain Rust struct      │
 ├─────────────────────────────────────────────────┼───────────────────────────┼────────────────────────┤
 │ gui/interactive_tools.py — pivy click handlers  │ PlantPlacer/Mover/Remover │ egui canvas Tool trait │
 ├─────────────────────────────────────────────────┼───────────────────────────┼────────────────────────┤
 │ gui/palette_dock.py — PySide6 QDockWidget       │ Palette browser           │ egui side panel        │
 ├─────────────────────────────────────────────────┼───────────────────────────┼────────────────────────┤
 │ tools/layers.py — Draft layer management        │ Layer tree                │ Custom layer model     │
 ├─────────────────────────────────────────────────┼───────────────────────────┼────────────────────────┤
 │ tools/drawing_sheet.py — TechDraw integration   │ Print layout view         │ Native SVG/PDF export  │
 └─────────────────────────────────────────────────┴───────────────────────────┴────────────────────────┘

 ---
 Architecture

 grove/
   Cargo.toml                    # Workspace
   crates/
     grove-core/                 # Pure logic library, no UI
       src/
         lib.rs
         plant.rs                # Plant struct, Category enum
         database.rs             # rusqlite plant DB (port of database.py)
         distribute.rs           # Distribution algorithms (port of distribute.py)
         geometry.rs             # Point, Polygon, point_in_polygon, area, centroid
         schedule.rs             # Aggregation + CSV export
         colour.rs               # Category colours, species palette
         project.rs              # Project document model (placed plants, groups)
         layer.rs                # Layer/group tree structure
         commands.rs             # Undo/redo command pattern
         export/
           mod.rs
           csv.rs                # Schedule CSV
           svg.rs                # SVG plan export
           dxf.rs                # DXF export (via dxf crate)
           pdf.rs                # PDF export (via svg2pdf)
     grove-app/                  # egui application
       src/
         main.rs
         app.rs                  # Top-level eframe::App
         canvas/
           mod.rs                # Canvas widget: pan, zoom, coordinate transforms
           render.rs             # Draw plants, boundaries, grid, background image
           picking.rs            # Hit-testing (point-in-circle, point-in-polygon)
           tools/
             mod.rs              # Tool trait + active tool manager
             select.rs           # Click-select, box-select, shift-multi-select
             place_plant.rs      # Click-to-place from palette
             place_group.rs      # Draw boundary + distribute
             move_tool.rs        # Blender-style G-key grab
             delete.rs           # Click-to-delete
             draw_line.rs        # General 2D: polyline/polygon drawing
             draw_arc.rs         # General 2D: arc/circle drawing
             dimension.rs        # General 2D: linear/angular dimensions
             text.rs             # General 2D: text annotation
         panels/
           mod.rs
           palette.rs            # Plant palette browser
           plant_search.rs       # Search/filter panel
           properties.rs         # Selected object properties inspector
           layers.rs             # Layer tree panel
           schedule.rs           # Schedule preview
         dialogs/
           mod.rs
           group_options.rs      # Spacing + distribution picker
           export.rs             # Export settings
           import_plants.rs      # CSV import
         theme.rs                # Colours, fonts, spacing
   data/
     plants.csv                  # Bundled plant database
     palettes.csv                # Bundled palettes

 ---
 Key Design Decisions

 1. File format: SQLite per project

 Each .grove file is a SQLite database containing placed objects, layers, groups, and a snapshot of referenced plant
 data. Benefits: single-file portability, queryable, trivial undo journal, no custom parser.

 2. Undo/redo: Command pattern

 Each action (place, move, delete, group) produces a Command that knows apply() and undo(). Commands store minimal
 deltas. Stack-based undo/redo.

 3. Coordinate system: millimetres

 Matches the existing workbench convention. ViewTransform handles world↔screen conversion with pan offset + zoom factor.

 4. Canvas rendering: egui Painter

 For MVP, egui's built-in Painter draws circles, polygons, lines, text. Plant counts are typically <1,000 — trivial for
 egui. Custom WGPU shaders can be added later if needed.

 5. 2D CAD primitives (beyond garden features)

 Lines, polylines, arcs, circles, rectangles, text, dimensions. These make it a real 2D CAD, not just a planting diagram
  tool. Useful for drawing paths, walls, fences, paving.

 6. DXF import as first-class feature

 Site plans from architects arrive as DXF. The dxf crate reads DXF entities (lines, arcs, circles, text) and renders
 them as a reference layer. Scale calibration lets the user match DXF units to mm.

 ---
 Phased Roadmap

 Phase 0: Scaffold (1-2 days)

 - Cargo workspace with grove-core + grove-app
 - Dependencies: egui, eframe, rusqlite, uuid, csv, serde, dxf, svg, image, rfd
 - Minimal eframe window that opens with an empty canvas

 Phase 1: Core Library (4-5 days)

 - geometry.rs — Point, Polygon, point_in_polygon, bounding_box, area, centroid
 - distribute.rs — grid, offset_grid, naturalistic (Poisson disk / Bridson's)
 - colour.rs — category colour map, species palette
 - database.rs — full PlantDatabase port (rusqlite, palettes, CSV import/export)
 - schedule.rs — aggregate + CSV export
 - plant.rs — Plant, PlacedPlant, PlantGroup structs
 - Tests ported from the existing Python test suite (77+ tests)

 Phase 2: Canvas + Selection (4-5 days)

 - ViewTransform — pan (middle-drag), zoom (scroll wheel around cursor)
 - Grid background that scales with zoom
 - Render placed plants as filled circles with category-coloured outlines
 - SelectTool — click to select, shift-click multi, drag box-select
 - Selection highlight (dashed outline, resize handles)
 - Properties panel showing selected object data

 Phase 3: Plant Palette + Placement (3-4 days)

 - Side panel: palette dropdown, plant table, search/filter
 - PlacePlantTool — ghost circle at cursor, click to place, Esc to stop
 - Undo/redo for placement
 - Status bar: active tool, plant name, cursor coordinates

 Phase 4: Move, Delete, Groups (3-4 days)

 - MoveTool — G-key grab, plants follow cursor, click confirm, Esc cancel
 - DeleteTool — click-to-delete
 - BoundaryDrawTool — click vertices, rubber-band preview, double-click/Enter to close
 - Group options dialog (spacing, distribution algorithm)
 - Group creation using distribute algorithms

 Phase 5: Layers + Persistence (3-4 days)

 - Layer tree panel (create, rename, reorder, toggle visibility, colour)
 - Assign objects to layers
 - Project save/load as .grove SQLite file
 - Recent projects, auto-save

 Phase 6: General 2D Drawing Tools (5-7 days)

 - DrawLineTool — polyline/polygon drawing
 - DrawArcTool — arc, circle, ellipse
 - DrawRectTool — rectangle
 - TextTool — text annotation with font/size
 - DimensionTool — linear and angular dimensions
 - Snapping: grid snap, endpoint snap, midpoint snap, perpendicular

 Phase 7: DXF Import + Background Images (3-4 days)

 - DXF reader (via dxf crate): lines, arcs, circles, polylines, text
 - Render imported DXF as a locked reference layer
 - Scale calibration: click two points, enter known distance
 - Image import (JPEG/PNG): site survey photos, scanned plans
 - Opacity control for background layers

 Phase 8: Export (3-4 days)

 - CSV schedule export
 - SVG plan export (circles, polygons, lines, text, dimensions)
 - PDF export via svg2pdf
 - DXF export for contractor handover
 - Legend generation (colour-coded species table)

 Phase 9: Print Layout (5-6 days)

 - Separate print layout view
 - Paper size selection (A0–A4), orientation
 - Scale control with standard scales (1:50, 1:100, 1:200)
 - Viewport onto the design canvas
 - Auto-generated legend, title block, north arrow, scale bar
 - PDF export of the layout sheet

 Phase 10: Polish (ongoing)

 - Keyboard shortcuts (configurable)
 - Colour modes (by category, by species, monochrome)
 - Label system with leader lines
 - Copy/paste, array/duplicate tools
 - Performance optimization if needed (spatial index, custom WGPU)

 ---
 Rust Dependencies

 [workspace.dependencies]
 eframe = "0.29"
 egui = "0.29"
 rusqlite = { version = "0.32", features = ["bundled"] }
 uuid = { version = "1", features = ["v4", "serde"] }
 csv = "1"
 serde = { version = "1", features = ["derive"] }
 serde_json = "1"
 rand = "0.8"
 dxf = "0.5"              # DXF read/write
 svg = "0.17"              # SVG generation
 svg2pdf = "0.12"          # PDF export
 image = "0.25"            # Background image loading
 rfd = "0.15"              # Native file dialogs
 directories = "5"         # Platform config paths

 ---
 Risks & Mitigations

 ┌────────────────────────────────────────────┬─────────────────────────────────────────────────────────────────────┐
 │                    Risk                    │                             Mitigation                              │
 ├────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────────┤
 │ General 2D CAD tools (dims, arcs) expand   │ Phase 6 is explicitly later; MVP works without them                 │
 │ scope significantly                        │                                                                     │
 ├────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────────┤
 │ DXF import fidelity (complex drawings)     │ Start with basic entities (lines, arcs, circles); test with real    │
 │                                            │ architect DXFs early                                                │
 ├────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────────┤
 │ egui canvas limitations for complex        │ egui Painter handles circles/lines fine; custom WGPU pass available │
 │ drawings                                   │  as escape hatch                                                    │
 ├────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────────┤
 │ Print layout is complex (multi-viewport,   │ Defer to Phase 9; MVP exports SVG/PDF directly from canvas          │
 │ scale)                                     │                                                                     │
 ├────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────────┤
 │ Competing with LibreCAD/QCAD on general 2D │ Don't try — the differentiator is garden-specific features (plant   │
 │                                            │ DB, schedules, palettes, distribution)                              │
 └────────────────────────────────────────────┴─────────────────────────────────────────────────────────────────────┘

 ---
 Existing Projects to Study

 - MuginCAD (github.com/Hakkology/MuginCAD) — Production Rust 2D CAD with egui, PDF export. Closest architecture
 reference.
 - Graphite — Procedural 2D design tool in Rust
 - CADmium — Browser-based Rust CAD (different approach but good architecture ideas)

 ---
 Summary

 This is a roadmap — the FreeCAD workbench continues as the working tool while Grove is developed. Phases 0–5 produce a
 usable garden planting tool (~20-25 days). Phases 6–9 add the general 2D CAD capabilities (~16-21 days). The pure-logic
  core (database, distribution algorithms, scheduling) ports directly from the existing Python codebase with known-good
 test cases.
