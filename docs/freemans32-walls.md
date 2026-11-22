# freemans32 walls and openings integration

This integration extends the catalogue branch at `03aae164f` with the wall and
opening tools from [freemans32/cadcraft, perso](https://github.com/freemans32/cadcraft/tree/perso),
snapshot `e4964f0ba`. It preserves the previous catalogue work and its resizable
gallery. The source licences are MIT OR Apache-2.0; no new assets or dependencies
are added.

## Use

Enable **Architectural walls and openings (freemans32)** under **Options > Drawing
Setup**, then open **Window > Blocks > Catalogues > Architecture**. The furniture
and equipment switch is independent. Architecture menus and autocomplete entries
are hidden when the architecture switch is off; decoding, drawing geometry,
parameter preservation, commands and automatic wall cleanup remain available.
The preference is kept in both interface preferences and profiles.

Draw in Model space, including Model space entered through a layout viewport.
Geometry commands explain this requirement when invoked in paper space. Style
management remains available there. The current generators and opening hit tests
operate on model geometry; silently placing a paper-space opening on a model wall
would be incorrect.

- Select a wall style and use **Draw Wall**. Pick successive points; Tab cycles
  justification. The command line also exposes style, height, arc and closing
  options. Height and component thicknesses are in millimetres; the wall keeps
  its drawing-unit conversion.
- Use **Wall Styles** to create, copy, rename or edit styles. The original style
  dialog edits the component layers, thicknesses, functions, hatch patterns,
  scales, angles, colours and junction priorities. Apply regenerates intact walls
  using that style; manual edits are preserved and counted.
- Choose an opening type and style, then **Place Opening**, or use **Corner
  Window**. Doors, double doors, windows and empty bays are independent objects;
  their hosts are cut and recut as objects move, copy or disappear.
- Select a wall or opening to edit it in the Architecture palette. Wall vertex
  and segment grips regenerate the path. Crossing-window STRETCH changes the
  included path vertices and rejoins neighbouring walls.
- **Wall Elevation** makes a separate 2D snapshot with openings at their sill and
  height, dimensions, and nearby furniture/equipment from the integrated
  catalogue. It does not follow later plan changes automatically.

Console/JSON commands: WALLADD, WALLEDIT, WALLSTYLEEDIT, WALLCLEANUP,
WALLELEVATION, OPENINGADD, OPENINGEDIT, OPENINGCONVERT and CORNERWINDOW.

## Provenance and adaptations

Original wall/opening recipes, standard styles, xdata encoding, junction engine,
wall grips, commands, style dialog and applicable regression tests are by
**freemans32**. Their per-instance `CADCRAFT_WALL` / `WALL1` and
`CADCRAFT_OPENING` / `OPENING1` formats are unchanged. Drawing-owned styles use
its versioned `CADCRAFT_WALLSTYLES` XRECORD in DXF and DWG.

| Original contribution | Source commit |
| --- | --- |
| Wall styles and parametric walls | `fbe59962c` |
| Wall/style persistence | `1b6151e9a` |
| L/T/X junctions | `76fa82f91` |
| Hosted openings | `a60e616f7` |
| Additional styles and opening symbols | `77d14aac8` |
| Automatic cleanup after changes | `adeb3a0ce` |
| Curved walls | `b447383d3`, `97c272f1c` |
| Elevations and opening conversion | `ca76b987b` |
| Wall grips | `80bd20cee` |
| Cached wall inspection | `d1e0b355f` |
| Objects in elevations | `c5e958dc2` |

The imported files retain later fixes present at the snapshot. Shared opening
xdata/geometry helpers reuse the existing catalogue module. Integration adds a
separate Architecture preference and source in the shared Blocks dock, its
selected-object editor and the original classic style dialog. The post-command
hook is restricted to the document actually edited, so switching/opening a
drawing does not trigger cleanup based on a different drawing's snapshot.

The STRETCH adapter preserves rotated/reflected reference placement while editing
local path vertices. Failed architectural command batches, including JSON and
interactive STRETCH, restore the original drawing snapshot. Ordinary drawing,
selection, image grips, templates and existing managers retain their own paths.

**Contributor notification:** freemans32 remains recorded for notification when
published. This integration does not send a notification or publish a release.

## Scope and remaining work

This is 2D parametric plan geometry with height data and 2D wall elevations.
Building sections (SECTIONADD), rooms, slabs, roofs, ceilings, structural objects,
networks/calculations and IFC are separate integration sets. Elevations currently
recognise furniture and equipment, not the source fork's unintegrated terminals,
air handling units or electrical devices.

Four original integration tests that require those unintegrated modules are
reserved with them: room-derived slab/ceiling outlines, complete building
sections, roof sections, and elevations including electrical devices. Applicable
wall/opening tests are imported, with a separate equipment/furniture elevation
regression for the supported families. No existing test is disabled.

As in the source, non-unit-scaled or manually modified walls are preserved;
explicit WALLEDIT `force` regenerates their parametric representation. Hot-grip
stretch-copy is outside the current wall adapter; ordinary COPY preserves wall
parameters. Component `returnsAtOpenings` is preserved metadata; lining returns
are not generated. Junction/xdata budgets remain those of the source generator.

## Validation

The following Linux checks passed:

| Suite | Passed | Existing ignored tests |
| --- | ---: | ---: |
| Drawing model library | 150 | 0 |
| Engine library | 572 | 0 |
| Engine command integration | 1208 | 0 |
| I/O library | 212 | 0 |
| I/O integration | 596 | 24 |
| UI library | 339 | 1 |
| Catalogue/architecture UI workflows | 3 | 0 |

Total: 3080 passed, with 25 inherited conformance/report tests ignored. The 56
wall command tests include original applicable cases and added regressions for
supported elevation objects, reflected/rotated STRETCH, rollback after a failed
multi-wall stretch and the model-space requirement. The I/O cases cover walls,
styles, openings, junctions and cuts in both DXF and DWG. The optional architecture
UI, selected-wall/opening editors and original style dialog were exercised with
headless OpenGL rendering and their screenshots inspected.

Clippy for all targets of the four changed crates with `-D warnings`, dependency
layering, asset attribution, the wasm-safe crate check, native application check
and menu-parity generation passed. Menu breadth remains 306/491 (62%), which is
not a behavioural-completeness metric. The wasm build retains baseline warnings
for native-only preview/render worker fields.

Windows compilation and interactive acceptance are required before packaging or
release.
