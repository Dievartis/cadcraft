# freemans32 building-tools integration audit

Source: [freemans32/cadcraft, perso](https://github.com/freemans32/cadcraft/tree/perso),
snapshot `e4964f0ba` inspected on 2026-11-22. A local source reference
`integration/freemans32-audit` preserves the inspected snapshot. This is a source
and dependency assessment; these personal building tools are not yet integrated
into the Dievartis preview, and were not built or manually tested here.

## Finding

This is an integrated set of application features rather than independently
loadable plug-ins. The selected wall, ventilation, building-command, palette and
HVAC paths alone add about 47,000 lines across 97 files relative to their common
upstream ancestor, including tests. That figure is not the complete fork diff.
The fork also contains community PRs; importing all of `perso` would import those
decisions together with its personal features and overlap our retained work.

## Data model

Walls and equipment are generated anonymous block references with application
xdata describing their parameters. Other CAD software can display the block
geometry without the generator. Editing a parameter, regenerating the geometry,
special grips, property controls and commands require the matching application
code. This is distinct from implementing the generic dynamic-block action graph.

The visible plan is 2D; height and altitude are model parameters. Wall elevations
and building sections produce separate 2D block snapshots. They do not follow
subsequent plan changes automatically. The IFC exporter constructs solids from
the parameters and documents incomplete wall voids, junctions, roof coverage and
external-viewer validation.

## Integration sets

| Set | Main source paths | Dependencies and adaptation |
|---|---|---|
| Furniture catalogue | `doc/src/ventilation/furniture.rs`, `furniture_catalog.rs`, corresponding engine commands and furniture palette | Shared `HvacPiece`, units, validation, labels, xdata and anonymous-block generation. Catalogue geometry uses fractional shapes, not external proprietary block files. Keep catalogue and insertion UI separate. |
| Equipment | `doc/src/ventilation/equipment.rs`, `engine/src/cmd/equipment.rs` | Same drawing helpers; built-in models and drawing-owned models; model persistence hooks in Drawing and I/O must accompany commands. Property editors, regeneration and pick/grip paths must be retained. |
| Walls and openings | `doc/src/wall.rs`, `wall/*`, `wall_library.rs`, opening modules, `engine/src/cmd/arch.rs`, wall-style dialog | Styles, component hatches, units, openings, joining/cleanup after edits and special grips. Engine post-command regeneration hooks are part of behaviour, not optional UI. |
| Elevations and sections | `wall/elevation.rs`, `wall/section.rs` | Walls/openings plus recognition of equipment, terminals, furniture, slabs, structure, ceilings, roofs, ducts and pipes. Their imports make a walls-only extraction insufficient for the full current implementation. |
| HVAC/electrical/plumbing | `doc/src/ventilation/*`, `crates/hvac`, engine commands and palettes | Calculation code, network graphs, labels and reports. Keep as later, separately validated integration sets. Standards checks require documented scope and do not establish regulatory compliance by themselves. |
| Building UI | `ui-egui/src/palettes/{building,electricity,furniture,hvachelp,objects,plumbing,regulation,thumbs}.rs` | Current source puts many tools in Tool Sets. Adapt presentation to the shared docking system and optional discipline controls rather than transplanting the whole palette. |

## Proposed first extraction

Start with shared unit/xdata/block helpers and furniture/equipment. Preserve their
original limits and parameter formats, then add the engine commands and a gallery
hosted by the existing block/palette infrastructure. Compare behaviour against
the source fork using synthetic drawings before adding walls.

The existing official Blocks palette is now part of the upstream baseline. It
lists drawing blocks and drawing-file libraries; the procedural furniture and
equipment catalogue is a different source. A common host may expose separate
sources without pretending generated equipment are ordinary editable definitions.

## Optional discipline controls

Keep document decoding, anonymous geometry display and parameter preservation
available regardless of which tools are visible. A user may open an architectural
drawing while using a general drafting workspace. Disable/hide discipline menus,
tool groups and catalogue tabs through preferences; do not drop document data or
make the file unreadable when a discipline is off. This is runtime tool visibility,
not a claim that Rust modules are dynamically unloaded from the executable.

Add discoverable activation and distinct general-drafting defaults. Regeneration
of existing objects must remain available or report an explicit limitation. Do
not distribute a setting that silently freezes stale parametric geometry.

## Provenance and review checkpoints

Personal source commits include furniture catalogue `978f81711`, initial equipment
`a411ceb45`, drawing-owned equipment models `02eaf8eb1`, computer racks
`d9336322c`, wall/opening elevation `ca76b987b`, sections `18361f061`, wall grips
`80bd20cee` and elevation equipment `c5e958dc2`. Preserve original authorship and
MIT/Apache notices in each extraction and record adapted paths and commits.

Before implementation, enumerate the exact common-helper closure for the chosen
set, compare document fields and command hooks against the updated baseline,
check save/reopen with disabled tool groups, and verify move/copy/scale/undo,
grips, units, PDF and wasm. Review IFC separately from the 2D generators.
