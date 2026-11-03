# Roadmap

Planned work, grouped by theme and roughly in priority order within each group.
Larger ideas are written up in more detail in [`docs/future-features.md`](docs/future-features.md).

**Sizes:** **S** under a day · **M** a few days · **L** a week or more.

## Client-ready plans

The core workflow: start from a sketch or survey scan and produce planting, structural
and site plans that are ready to hand to a client.

- [ ] **PDF scan import** (M). Scanners and surveyors often deliver PDFs, but background
      import only accepts PNG/JPG/WebP/BMP today.
      *Done when:* you can pick a page from a multi-page PDF, rasterise it at a chosen DPI,
      then calibrate and trace over it like any other background image.
- [ ] **Material hatching** (M). Hardscape is drawn as flat colour fills; construction and
      structural plans conventionally hatch materials.
      *Done when:* brick, stone, gravel, timber/decking and concrete each have a hatch
      pattern, selectable per object, rendered in the canvas, drawing view and SVG/PDF export.
- [ ] **Title-block branding** (S). Client submissions should carry the designer's identity.
      *Done when:* a title block can show a logo and practice details (name, address,
      contact), saved as a reusable profile and applied to new drawings.
- [ ] **Plant library management** (M). The plant database is loaded from the bundled
      `data/plants.csv` and can't be changed from inside the app.
      *Done when:* plants can be added and edited in-app, and a designer can import or
      export their own plant list as CSV, with the result persisted between sessions.
- [ ] **DXF import and export** (L). Architects and surveyors exchange site plans as DXF,
      and contractors expect it back.
      *Done when:* DXF lines, arcs, circles, polylines and text import as a locked,
      calibratable reference layer, and drawings export to DXF with layers preserved.

## Site and levels

- [ ] **Contours from spot heights** (M). *Done when:* contour lines at a chosen interval
      are generated from surveyed spot heights and update when the heights change.
- [ ] **More entourage symbols** (S). Sheds, greenhouses, play equipment and bike stores,
      as top-down vector symbols.

## Housekeeping

- [ ] **Continuous integration** (S). A GitHub Actions workflow running `cargo build`,
      `cargo test` and `cargo clippy -D warnings` on Linux, macOS and Windows.
- [ ] **Release binaries** (S). Tagged releases with prebuilt binaries for the three
      desktop platforms.
- [ ] **Help → About dialog** (S). The menu item exists but does nothing yet. It should
      show the version, license and a link to the repository.

## Longer term

- [ ] **Crowd-calibrated cost estimates** (L). Estimate Builder Phase C: regional cost
      multipliers refined by anonymised feedback from landscapers. Needs a small backend.
      See [`docs/future-features.md`](docs/future-features.md#estimate-builder-crowd-calibrated-cost-estimator).
- [ ] **3D export to Blender** (L). Rebuild a plan in 3D from the heights and depths
      already stored on each object.
      See [`docs/future-features.md`](docs/future-features.md#3d-export-to-blender).

## Done

See [`CHANGELOG.md`](CHANGELOG.md) for shipped work, including drawing sheets and
templates, per-viewport plant styles, procedural canopy symbols, the cost estimate
builder, and the egui 0.36 upgrade.
