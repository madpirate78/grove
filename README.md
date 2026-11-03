# Grove

A 2D CAD application for garden design, written in Rust with [egui](https://github.com/emilk/egui).

Grove is a general-purpose 2D drafting tool with first-class garden features: a plant
database, planting schedules, legends, plant distribution, hardscape elements, printable
drawing sheets, and cost estimates. It is a native, single-binary 2D tool built for what
garden design actually is: a plan-view task, with no 3D viewport to get in the way.

**Who it's for:** garden and landscape designers, and keen amateurs, who want proper
scaled plans, planting schedules and costings without wrestling a general-purpose
architectural CAD package.

**Built with:** Rust, [egui/eframe](https://github.com/emilk/egui) for the UI,
SQLite ([rusqlite](https://github.com/rusqlite/rusqlite)) for project files,
[glam](https://github.com/bitshifter/glam-rs) for geometry, and
[svg2pdf](https://github.com/typst/svg2pdf)/[resvg](https://github.com/linebender/resvg)
for PDF and image output.

> **Status:** pre-1.0 and under active development. The project file format may still
> change. See the [roadmap](ROADMAP.md) for what's planned next.

## Features

- **Drafting:** lines, arcs, circles, rectangles, polygons (with hole subtraction), text and dimensions. Move/rotate/scale, linear arrays, undo/redo, copy/paste, layers.
- **Planting:** place plants from a bundled plant database (`data/plants.csv`), distribute plants in groups, and generate planting schedules numbered by species.
- **Hardscape:** walls, fences, edging, paths, steps, patios, decking, raised beds, pergolas and ponds, with level markers.
- **Entourage:** top-down people, furniture, vehicles and garden objects to show scale.
- **Background images:** import a site survey or photo and trace over it.
- **Drawing sheets:** model/drawing workspaces with seven templates (master plan, planting plan, construction detail, …), paper sizes, viewports, per-viewport layer overrides and plant styles (simple, canopy, numbered, outline, silhouette), title blocks and revisions.
- **Cost estimates:** a bill of quantities with editable unit rates, plus rate cards you can share as JSON.
- **Export:** SVG, PDF (drawings), planting schedule CSV, cost estimate CSV and JSON.
- **Persistence:** projects are saved as SQLite files.

## Building

Requires Rust **1.95** or newer ([rustup](https://rustup.rs)).

```sh
git clone https://github.com/madpirate78/grove.git
cd grove
cargo run --release -p grove-app
```

File dialogs use the XDG desktop portal on Linux, so a portal backend (for example
`xdg-desktop-portal-gtk` or `-kde`) must be installed. Most desktop distributions ship one.

Run the test suite with:

```sh
cargo test --workspace
```

## Project layout

| Path | Contents |
|------|----------|
| `crates/grove-core` | Pure logic with no UI: entity model, undoable commands, garden domain (schedules, distribution, canopy symbols, estimates), storage, and SVG/PDF/CSV/JSON export |
| `crates/grove-app` | The eframe/egui application: canvas, interactive tools, panels and menus |
| `data/` | Bundled plant database and entourage symbol library |
| `scripts/` | Python helpers for building the entourage library |
| `docs/` | [Architecture](docs/architecture.md), the [domain data model](docs/data-model.md), and [future-feature ideas](docs/future-features.md) |

## Contributing

Issues and pull requests are welcome. Please keep the workspace free of `cargo clippy`
warnings and add tests in `grove-core` for logic changes. Record notable changes in
[`CHANGELOG.md`](CHANGELOG.md).

## License

Grove is released under the [MIT License](LICENSE). The bundled entourage symbols are
original artwork covered by the same license; see
[`data/entourage/LICENSES.md`](data/entourage/LICENSES.md).
