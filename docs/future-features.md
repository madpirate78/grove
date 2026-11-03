# Future Feature Ideas

## Estimate Builder (Crowd-Calibrated Cost Estimator)

**Status:** Phases A and B shipped (2026-08-04); Phase C (online feedback service) not started. Proposed 2026-03-23.

### Concept

A cost estimation system that calculates project costs from the entities in a design (plants, hardscape, structures, etc.), refined over time using anonymised feedback from professional landscapers, segmented by location.

### How it would work

**Local estimation (in Grove):**
- Walk the entity list — plants, paving, fencing, beds, etc.
- Apply unit costs (per plant, per m², per linear metre) to generate a bill of quantities
- Purely offline, no server dependency

**Crowd feedback loop (needs a backend):**
- Professionals register/verify as landscapers
- After a project, they report "actual cost was X% higher/lower than estimate"
- Feedback tagged with location (postcode/region) and project date
- Aggregated adjustments become regional cost multipliers

**Location-based adjustment:**
- Labour rates, material availability, and soil conditions vary by region
- Model: `base_cost × regional_multiplier` per category (plants, labour, materials, hardscape)
- Multipliers update as feedback accumulates

### Open Questions

- **Scope:** Part of Grove or a separate service Grove talks to? Feedback aggregation wants a server component.
- **Cold start:** Before enough feedback, need seed data (published landscaping cost guides, trade association rate cards).
- **Privacy/trust:** Professionals sharing real project costs is sensitive. Anonymisation and aggregation (never exposing individual quotes) is important.
- **Granularity:** Region-level multipliers are simple. Costs also vary by project scale, access difficulty, soil type — how deep initially?
- **Data model:** The entity system already has domain data (plants with species, hardscape with materials). Cost metadata could attach to `DomainData` variants, making the bill of quantities a natural query over the project.

### Suggested Phases

- **Phase A — Offline bill of quantities:** DONE (2026-08-04). `garden::estimate` walks entities and produces a priced BOQ: plants counted per species (same recursion as the planting schedule, so quantities always agree), linear hardscape (walls/fences/edging/steps) measured along centerlines, area hardscape (patios/surfaces/decking/beds/pergolas/ponds) from geometry, with hole subtraction. `UnitCosts` is string-keyed + serde so Phase B rate cards are a JSON file away; seeded with rough UK installed rates and per-material multipliers. Exported via Export → Cost Estimate CSV.
- **Phase B — Export/import cost profiles:** DONE (2026-08-04). Garden → Cost Estimate… opens a floating window with the live priced BOQ and an editable Unit Rates section (all rate maps + currency + material multipliers). Edits persist to the app config dir (`unit_costs.json`) and apply to every export. Profiles import/export as JSON rate cards via the window's buttons; importing replaces the active rates wholesale, and partial profiles price missing keys as zero.
- **Phase C — Online feedback service:** Server component that collects actuals vs estimates and computes regional adjustments.

---

## Per-Drawing Plant Rendering Styles

**Status:** Shipped (2026-08-04), all phases. Proposed 2026-04-18; kept here as a design record.

### Concept

Today plants render identically in every drawing view: a filled circle with the common/botanical-name label. Real landscape plans show plants differently depending on the drawing's purpose:

- **Master plan** — stylised canopy symbols (scalloped circles, flower rosettes) to communicate character at a glance.
- **Planting plan** — numbered circles that key into a schedule table.
- **Construction detail** — outline-only circles so hardscape reads clearly over the top.
- **Sketch/concept** — loose, hand-drawn-looking blobs.

A plant should know its species (`DomainData::Plant`) and leave the *rendering* to the drawing it appears in.

### Proposed design

Add a `plant_style: PlantStyle` field on `Drawing` (or on `DrawingViewport` for finer control — per-viewport is what `colour_mode` does already). Enum variants:

```rust
pub enum PlantStyle {
    Simple,      // current: filled disk + label
    Canopy,      // stylised top-down canopy symbol
    Numbered,    // circle with schedule number
    Outline,     // stroke only, no fill
    Silhouette,  // solid opaque disk
}
```

Drawing view renderer branches on `plant_style` in the block at `crates/grove-app/src/view/drawing_view.rs:540` (where `DomainData::Plant` is currently handled). Model view keeps the simple style — this is a drawing-specific presentation concern.

Drawing properties panel gains a dropdown for style, alongside the existing `colour_mode` picker.

### Open questions

- Storage scope: per-drawing (simpler) or per-viewport (matches `colour_mode` precedent)? Per-viewport is more flexible.
- Canopy symbols: procedural (drawn from Rust code) or loaded from SVG library (like entourage)? Procedural is simpler for a first cut; SVG library unlocks user-extensibility.
- Numbered style needs a schedule-number source: auto-assigned by species, or a manual `number` field on each plant entity.
- SVG export must mirror whatever the drawing view shows.

### Suggested phases

- **Phase A** — DONE (2026-08-04): `PlantStyle` enum on `DrawingViewport`, `Simple`/`Outline`, drawing properties UI.
- **Phase B** — DONE (2026-08-04): `Canopy` (concentric-ring hint, not yet scalloped) and `Silhouette`.
- **Phase C** — DONE (2026-08-04): `Numbered` style renders the plant's 1-based row number in the planting schedule (`garden::schedule::schedule_numbers`, keyed by botanical name in schedule order: category, then botanical name). The CSV export now has a matching `#` column.
- **Phase D** — DONE (2026-08-04) as *procedural* symbols rather than an SVG asset library: `garden::canopy` generates species-seeded outlines (Scalloped for deciduous trees, Spiky for evergreens/grasses, Lobed for shrubs) with position-seeded rotation, rendered as vector paths in both the drawing view and SVG export. The entourage-pipeline route was rejected because, at the time, entourage symbols embedded rasterized PNGs, which is wrong for print-oriented plan output. Entourage has since moved to pure vector, so a user-extensible SVG symbol library is now a realistic follow-up.

---

## 3D Export to Blender

**Status:** Idea (carried over from the original data-model spec; not started)

### Concept

Export a design to Blender for 3D visualisation. Grove's domain properties already carry the vertical information needed (wall/fence heights, riser heights, deck heights, water depths, spot heights), so the plan can be rebuilt in 3D without redrawing.

### How it would work

Two parallel streams, read by a Blender import add-on:
- **Geometry** — per-object outlines/centrelines (OBJ, or a dedicated JSON geometry block).
- **Data** — a JSON file grouping objects by type (`plants`, `hardscape`, `levels`), each record carrying its UUID, position/geometry reference and domain properties. UUIDs let a re-export update an existing Blender scene instead of rebuilding it.

Per-type reconstruction in Blender:
- **Plants** — billboard or 3D asset scaled by height and spread, coloured by colour code; geometry nodes to scatter within group boundaries.
- **Walls / fences / raised beds** — extrude the polyline to height and width; fence panel geometry chosen by fence type.
- **Paths / patios / decking** — thin slabs from the outline; paving or board pattern from laying pattern / board direction.
- **Steps** — one box per riser, stacked from the base level.
- **Pergolas** — posts and beams generated from footprint and spacing properties.
- **Water features** — cut down to depth, water material on the surface.
- **Terrain** — Delaunay-triangulate spot heights into a ground mesh and drape other objects onto it.

Layers (`GD_*`) map to Blender collections. Materials come from a library keyed to Material + Finish (stone sawn/riven/tumbled, brick stock/engineering, timber, concrete, metal, gravel, porcelain, render, water, grass, soil, mulch).
