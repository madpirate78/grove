# Grove Data Model

---

This document defines the garden domain types, their properties, and the data conventions Grove uses. It covers three domains: Planting, Hardscape, and Levels. Each object type is specified at a moderate level of detail: enough to drive 2D plan visualisation, schedules and cost estimates, plus basic construction information. In code, each type is a `DomainData` variant (`crates/grove-core/src/entity.rs`) attached to an entity alongside its `Geometry`.

All dimensions are in millimetres. All colours are hex codes. Every object carries a UUID.

---

## 1. Planting

*Establishes the pattern that hardscape and levels objects follow.*

### Plant

*A single plant instance placed in plan view, rendered as a circle sized by Spread.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Botanical Name | String | — | Full botanical name inc. cultivar, e.g. Actaea simplex 'Brunette' |
| Common Name | String | — | Common/trade name |
| Category | Enum | Perennial | Perennial \| Shrub \| Tree \| Grass \| Climber \| Bulb \| Fern \| Hedge \| Aquatic |
| Height MM | Integer | 0 | Mature height in mm |
| Spread MM | Integer | 0 | Mature spread in mm — drives circle diameter in plan |
| Colour Code | Hex String | #4CAF50 | Primary display colour for plan view |
| Flower Colour | String | — | Descriptive flower colour |
| Foliage Colour | String | — | Descriptive foliage colour |
| Evergreen | Boolean | No | Evergreen or deciduous |
| Notes | String | — | Free text: designer, source, planting notes |
| Plant ID | UUID | Auto | Unique identifier for pipeline tracking |

---

## 2. Hardscape

All hardscape objects share a common set of base properties, then add type-specific properties. In Grove the shared properties live in `HardscapeBase` (`crates/grove-core/src/garden/hardscape.rs`), embedded in each type-specific struct.

### 2.0 Common Hardscape Properties

*Inherited by all hardscape objects below.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Object ID | UUID | Auto | Unique identifier for pipeline tracking |
| Material | Enum | Stone | Stone \| Brick \| Timber \| Concrete \| Metal \| Composite \| Gravel \| Resin Bound \| Porcelain \| Render \| Corten \| Glass |
| Material Detail | String | — | Specific product/finish, e.g. 'Yorkstone sawn', 'London stock brick' |
| Colour Code | Hex String | #8B7D6B | Display colour for plan view |
| Finish | String | — | Surface finish: sawn, riven, tumbled, brushed, etc. |
| Notes | String | — | Free text: specification, supplier, construction notes |

### 2.1 Wall

*Drawn as a polyline in plan. Can be freestanding or retaining. Height supports a future 3D export (extrusion).*

| Property | Type | Default | Notes |
|---|---|---|---|
| Height MM | Integer | 450 | Height above adjacent ground level |
| Width MM | Integer | 215 | Wall thickness (single brick = 102, double = 215) |
| Wall Type | Enum | Freestanding | Freestanding \| Retaining \| Seat |
| Capping | String | — | Capping type: twice weathered, flat, bullnose, none |
| Capping Material | String | — | If different from wall material |
| Foundation Depth MM | Integer | 450 | For construction info / costing |
| Jointing | String | Flush | Flush \| Recessed \| Weathered \| Bucket handle |
| Base Level MM | Float | 0 | Level at base of wall (from datum) |

### 2.2 Path

*Drawn as a polyline centreline with width. Rendered as a filled strip in plan.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Width MM | Integer | 900 | Path width |
| Edging | String | — | Reference to Edging object or description |
| Sub Base Depth MM | Integer | 150 | MOT Type 1 or similar |
| Laying Pattern | String | Stretcher | Stretcher \| Herringbone \| Stack \| Random \| Coursed |
| Joint Width MM | Integer | 10 | Joint/gap width |
| Joint Material | String | Mortar | Mortar \| Sand \| Resin \| Open (for permeable) |
| Drainage | Enum | None | None \| Cross fall \| Permeable \| Channel |
| Cross Fall MM/M | Integer | 0 | Fall across path width for drainage |

### 2.3 Patio / Paved Area

*Drawn as a closed shape in plan. Rendered as a filled area.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Sub Base Depth MM | Integer | 150 | MOT Type 1 or similar |
| Laying Pattern | String | Stretcher | Stretcher \| Herringbone \| Stack \| Random \| Coursed |
| Joint Width MM | Integer | 10 | Joint/gap width |
| Joint Material | String | Mortar | Mortar \| Sand \| Resin \| Open |
| Drainage | Enum | Cross fall | None \| Cross fall \| Permeable \| ACO channel |
| Fall Direction | Float (deg) | 0 | Angle of fall in plan (0 = north) |
| Fall Rate MM/M | Integer | 15 | Gradient for drainage |
| Finished Level MM | Float | 0 | Top of paving level from datum |

### 2.4 Step

*Drawn as a polyline (nosing line) with tread depth. Multiple steps form a flight.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Riser Height MM | Integer | 150 | Height of riser (building regs: 100–220) |
| Tread Depth MM | Integer | 350 | Depth of tread (min 280 recommended) |
| Width MM | Integer | 900 | Width of step |
| Nosing MM | Integer | 25 | Nosing projection beyond riser |
| Riser Material | String | — | If different from tread material |
| Num Risers | Integer | 1 | Number of risers in a flight |
| Base Level MM | Float | 0 | Level at bottom of first riser |
| Handrail | Boolean | No | Whether handrail required |
| Handrail Side | Enum | Both | Left \| Right \| Both \| None |

### 2.5 Edging

*Drawn as a polyline. Defines the boundary between surfaces.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Height MM | Integer | 150 | Height of edging above ground |
| Width MM | Integer | 50 | Width/thickness of edging |
| Depth Below MM | Integer | 100 | Depth set into ground |
| Edging Type | Enum | Flush | Flush \| Raised \| Mowing strip |
| Profile | String | Flat | Flat \| Bullnose \| Half round \| Rope top |

### 2.6 Raised Bed

*Drawn as a closed shape in plan. Contains planting. Height supports a future 3D export (extrusion).*

| Property | Type | Default | Notes |
|---|---|---|---|
| Height MM | Integer | 450 | Wall height above ground |
| Wall Width MM | Integer | 100 | Wall thickness (timber: 50, sleeper: 200, brick: 102/215) |
| Capping | String | — | Capping type if applicable |
| Soil Depth MM | Integer | 400 | Internal soil depth |
| Drainage | Boolean | Yes | Whether drainage holes/layer included |
| Liner | Boolean | No | Whether lined (timber beds) |
| Base Level MM | Float | 0 | Ground level at base |

### 2.7 Fence

*Drawn as a polyline in plan. Height supports a future 3D export (extrusion).*

| Property | Type | Default | Notes |
|---|---|---|---|
| Height MM | Integer | 1800 | Height of fence |
| Fence Type | Enum | Close board | Close board \| Hit and miss \| Trellis \| Post and rail \| Picket \| Woven \| Metal railing \| Slatted \| Acoustic |
| Post Spacing MM | Integer | 1800 | Centre to centre post spacing |
| Post Size MM | String | 100x100 | Post cross section |
| Post Material | String | — | If different from fence material |
| Post Setting | Enum | Concrete | Concrete \| Post spike \| Bolt down |
| Gravel Board | Boolean | Yes | Whether gravel board at base |
| Gravel Board H MM | Integer | 150 | Gravel board height |
| Gate | Boolean | No | Whether section includes a gate |

### 2.8 Pergola / Structure

*Drawn as a rectangular footprint or polyline of posts in plan.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Height MM | Integer | 2400 | Height to underside of beam |
| Post Size MM | String | 100x100 | Post cross section |
| Beam Size MM | String | 150x50 | Beam/rafter cross section |
| Post Spacing MM | Integer | 2000 | Centre to centre |
| Rafter Spacing MM | Integer | 400 | Centre to centre |
| Rafter Overhang MM | Integer | 200 | Overhang beyond outer beam |
| Structure Type | Enum | Pergola | Pergola \| Arbour \| Gazebo \| Shade sail \| Carport |
| Roof | Enum | Open | Open \| Polycarbonate \| Glass \| Fabric \| Green roof |
| Post Fixing | Enum | Bolt down | Bolt down \| Set in concrete \| Post anchor |

### 2.9 Decking

*Drawn as a closed shape in plan. Rendered as a filled area with board direction.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Board Width MM | Integer | 140 | Board width |
| Board Gap MM | Integer | 6 | Gap between boards |
| Board Direction | Float (deg) | 0 | Board run direction in plan (0 = north) |
| Deck Height MM | Integer | 300 | Height above ground to top of board |
| Joist Spacing MM | Integer | 400 | Joist centres |
| Substructure | Enum | Timber | Timber \| Aluminium \| Adjustable pedestal |
| Balustrade | Boolean | No | Whether balustrade required |
| Balustrade Height MM | Integer | 1100 | If applicable |
| Balustrade Type | String | — | Glass \| Metal \| Timber \| Wire |

### 2.10 Water Feature

*Drawn as a closed shape or point in plan. Type would determine the 3D representation in a future 3D export.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Feature Type | Enum | Pond | Pond \| Rill \| Fountain \| Wall spout \| Bowl \| Trough \| Stream |
| Depth MM | Integer | 600 | Water depth / feature depth |
| Liner | String | Butyl | Butyl \| EPDM \| Fibreglass \| Concrete \| Preformed |
| Pump | Boolean | Yes | Whether pumped circulation |
| Pump LPH | Integer | 0 | Pump flow rate (litres per hour) |
| Lighting | Boolean | No | Underwater or feature lighting |
| Water Level MM | Float | 0 | Top water level from datum |
| Edge Treatment | String | — | Coping, pebble beach, marginal shelf, etc. |
| Filtration | Enum | None | None \| Mechanical \| Biological \| UV \| Combined |

---

## 3. Levels

Levels objects define the vertical dimension of the site. They are driven by survey data (spot heights) and design decisions (finished levels). They sit on the dedicated `GD_LEVELS` layer.

### 3.1 Spot Height

*A point marker with an elevation value. The primary input from survey data.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Elevation MM | Float | 0 | Height from datum (typically OS datum or site datum) |
| Source | Enum | Survey | Survey \| Design \| Interpolated |
| Label Display | Boolean | Yes | Whether to show elevation text on plan |
| Datum Reference | String | Site TBM | Which datum this height relates to |
| Accuracy MM | Integer | 10 | Accuracy of measurement |
| Object ID | UUID | Auto | Unique identifier |

### 3.2 Contour Line

*A polyline at a constant elevation. Can be generated from spot heights or drawn manually.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Elevation MM | Float | 0 | Height of this contour |
| Contour Type | Enum | Minor | Major \| Minor \| Index |
| Source | Enum | Generated | Survey \| Generated \| Design |
| Interval MM | Integer | 250 | Contour interval (for generated sets) |
| Object ID | UUID | Auto | Unique identifier |

### 3.3 Slope Arrow

*A line with start and end elevations indicating fall direction. Used on paved areas and drainage runs.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Start Level MM | Float | 0 | Elevation at arrow tail |
| End Level MM | Float | 0 | Elevation at arrow head |
| Gradient | String (auto) | — | Auto-calculated as 1:x or mm/m |
| Length MM | Float (auto) | — | Auto-calculated from geometry |
| Object ID | UUID | Auto | Unique identifier |

### 3.4 Drain Run

*A polyline representing a drainage pipe or channel with invert levels.*

| Property | Type | Default | Notes |
|---|---|---|---|
| Invert Start MM | Float | 0 | Invert level at start |
| Invert End MM | Float | 0 | Invert level at end |
| Pipe Dia MM | Integer | 110 | Pipe diameter |
| Drain Type | Enum | Foul | Foul \| Surface \| Combined \| Land drain \| French drain \| Soakaway |
| Gradient | String (auto) | — | Auto-calculated |
| Manhole | Boolean | No | Whether this run connects to a manhole/IC |
| Object ID | UUID | Auto | Unique identifier |

---

## 4. Conventions and Layer Structure

### 4.1 Layer Naming

New objects are placed on these default layers:

- **GD_PLANTING** — All plant objects
- **GD_HARDSCAPE_WALLS** — Walls, raised beds
- **GD_HARDSCAPE_SURFACES** — Paths, patios, decking
- **GD_HARDSCAPE_STEPS** — Steps and ramps
- **GD_HARDSCAPE_STRUCTURES** — Pergolas, arbours, fences
- **GD_HARDSCAPE_WATER** — Water features
- **GD_HARDSCAPE_EDGING** — Edging
- **GD_LEVELS** — Spot heights, contours, slope arrows
- **GD_DRAINAGE** — Drain runs, manholes
- **GD_BOUNDARY** — Site boundary (existing survey)
- **GD_BUILDINGS** — Existing buildings (from survey)
- **GD_SURVEY** — Original survey data, base image
- **GD_ENTOURAGE** — People, furniture, vehicles and other scale references

### 4.2 Coordinate System

Grove uses millimetres (`f64` in entity data, `f32` in geometry and rendering). The origin (0,0) should be set to a meaningful point — typically the corner of the house nearest the garden. Z = 0 should correspond to the site datum. All elevations in properties are relative to this datum.

### 4.3 Colour Conventions for Plan View

Each object type should have a default plan colour that can be overridden per instance. Suggested defaults:

- **Plants:** By category or colour code property
- **Walls:** #8B7D6B (warm stone)
- **Paths/Patios:** #C4B8A8 (pale stone)
- **Steps:** #A0937D (mid stone)
- **Edging:** #6B6158 (dark stone)
- **Raised beds:** #8B7D6B (as walls)
- **Fences:** #A0522D (timber brown)
- **Pergolas:** #A0522D (timber brown)
- **Decking:** #C19A6B (golden timber)
- **Water:** #3B7DD8 (blue)
- **Spot heights:** #E53935 (red)
- **Contours:** #B71C1C (dark red)
- **Drain runs:** #1B5E20 (dark green dashed)

### 4.4 ID and Versioning

Every object gets a UUID (Object ID or Plant ID) generated on creation. This ID persists across saves and exports, so external tools can match objects between exports. If an object is duplicated, the copy gets a new UUID.
