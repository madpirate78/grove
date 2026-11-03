# Roadmap

Planned work, roughly in priority order. Larger ideas are written up in more detail in
[`docs/future-features.md`](docs/future-features.md).

## Near term

- [ ] **Continuous integration.** A GitHub Actions workflow running `cargo build`,
      `cargo test` and `cargo clippy -D warnings` on Linux, macOS and Windows.
- [ ] **Release builds.** Tagged releases with prebuilt binaries for the three desktop platforms.
- [ ] **Help → About dialog.** The menu item exists but does nothing yet. It should show
      the version, license and a link to the repository.
- [ ] **Custom plant lists.** Import and export the plant database from CSV in the app,
      so designers can bring their own nursery lists instead of only the bundled
      `data/plants.csv`.
- [ ] **Larger bundled plant database.** Grow `data/plants.csv` beyond its current ~50
      species, with more trees, hedging, climbers and aquatics.

## Medium term

- [ ] **DXF import.** Bring architects' and surveyors' site plans in as a locked reference
      layer (lines, arcs, circles, polylines, text), reusing the existing scale-calibration flow.
- [ ] **DXF export.** Hand drawings to contractors in the format their CAD tools expect.
- [ ] **Contours from spot heights.** Generate contour lines automatically from surveyed spot heights.
- [ ] **Entourage library expansion.** More top-down vector symbols (sheds, greenhouses,
      play equipment, bike stores), plus user-supplied SVG symbols.

## Longer term

- [ ] **Crowd-calibrated cost estimates.** Estimate Builder Phase C: regional cost
      multipliers refined by anonymised feedback from landscapers. Needs a small backend.
      See [`docs/future-features.md`](docs/future-features.md#estimate-builder-crowd-calibrated-cost-estimator).
- [ ] **3D export to Blender.** Rebuild a plan in 3D from the heights and depths already
      stored on each object. See [`docs/future-features.md`](docs/future-features.md#3d-export-to-blender).

## Done

See [`CHANGELOG.md`](CHANGELOG.md) for shipped work, including drawing sheets and
templates, per-viewport plant styles, procedural canopy symbols, the cost estimate
builder, and the egui 0.36 upgrade.
