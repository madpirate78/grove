# Future Feature Ideas

## Estimate Builder (Crowd-Calibrated Cost Estimator)

**Status:** Brainstorming (2026-03-23)

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

- **Phase A — Offline bill of quantities:** Enumerate entities, apply default unit costs, produce a simple table/export. No server needed, ships as part of Grove.
- **Phase B — Export/import cost profiles:** Let users tweak unit costs and share profiles (JSON files) so landscapers in different regions can publish their own rate cards.
- **Phase C — Online feedback service:** Server component that collects actuals vs estimates and computes regional adjustments.

---

## Per-Drawing Plant Rendering Styles

**Status:** Brainstorming (2026-04-18)

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

- **Phase A** — Add `PlantStyle` enum, wire to drawing/viewport, implement `Simple` and `Outline` (trivial). Drawing properties UI.
- **Phase B** — Procedural `Canopy` (scalloped edge generator) and `Silhouette`.
- **Phase C** — `Numbered` style with a planting-schedule numbering scheme.
- **Phase D** — SVG library for canopy symbols (hook into the existing entourage pipeline).
