# Grove Architecture

A short orientation for contributors. For the domain types themselves see
[`data-model.md`](data-model.md).

## Crates

- **`grove-core`**: pure logic with no UI dependency. It holds the entity model, undoable
  commands, the garden domain (schedules, distribution, canopy symbols, estimates),
  drawing sheets, storage and export. Almost all tests live here.
- **`grove-app`**: the eframe/egui application. It holds the canvas, interactive tools,
  panels and menus. It follows MVVM: the model lives in core, and the view-model and view
  live here.

`grove-app/src/app.rs` holds the application state and a short `ui()` orchestrator.
Each responsibility (shortcuts, file operations, menus, panels, canvas, …) is a child
module under `src/app/`. They're children rather than siblings so that Rust's module
privacy lets them reach `GroveApp`'s private fields. New `impl GroveApp` blocks belong
there.

## Key design decisions

1. **One entity type, two enums.** An `Entity` (`grove-core/src/entity.rs`) pairs a
   `Geometry` (the CAD shape: line, arc, polygon, …) with a `DomainData` (what it means:
   plant, wall, path, spot height, …). Generic CAD operations work on geometry; garden
   features work on domain data.
2. **Tools vs commands.** A `Tool` (`grove-app/src/tools`) is the interactive UI protocol:
   clicks, previews, keyboard handling. A `Command` (`grove-core/src/commands.rs`) is an
   atomic, undoable mutation with `apply`/`undo`. Tools produce commands; only commands
   mutate the project, so undo/redo comes for free.
3. **Millimetres everywhere.** World coordinates are millimetres: `f64` in entity data,
   `f32` in geometry and rendering. `ViewTransform` (`grove-app/src/view/viewport.rs`)
   converts world ↔ screen using a pan offset and zoom factor.
4. **SQLite project files.** A `.grove` file is a SQLite database (`grove-core/src/storage.rs`)
   with a schema version. That gives single-file portability without a custom parser, and
   older files keep loading as the schema grows.
5. **Immediate-mode rendering with the egui `Painter`.** Typical plans have well under a
   few thousand entities, which egui draws comfortably.
6. **A general 2D CAD with garden features, not a planting-only tool.** Lines, arcs,
   dimensions and text make it usable for whole site plans. The garden domain (plant
   database, schedules, distribution, estimates) is what sets it apart.
