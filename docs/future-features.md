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
