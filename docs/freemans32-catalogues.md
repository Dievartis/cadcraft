# freemans32 catalogue integration

This branch extends the tested Dievartis preview 0.7 (`fa9856113`) with the first
integration set from [freemans32/cadcraft, `perso`](https://github.com/freemans32/cadcraft/tree/perso),
snapshot [`e4964f0ba`](https://github.com/freemans32/cadcraft/commit/e4964f0ba).
It does not merge that fork's complete history or its selection of community PRs.

## Included

- The 89 furniture and appliance recipes, their categories and default dimensions.
- Equipment recipes and built-in models across 13 families: chillers, outdoor and
  indoor units, server-room cooling, pool dehumidifiers and computer racks.
- Parametric placement, repeat placement, rotation and nine insertion anchors
  cycled with Tab. Width/depth/length/height, altitude and technical values remain
  stored with the object; edits regenerate its anonymous block.
- Drawing-owned equipment models, including versioned DXF/DWG XRECORD persistence.
- A Catalogues source in the existing dockable Blocks palette, with Furniture and
  Equipment sources, category navigation, filter, previews and a selected-object
  editor. The existing shared tab close button and 8-point padding are reused.
- Optional tool visibility under Options > Drawing Setup. It defaults to off and
  is saved in interface preferences and profiles. Menus and autocomplete hide
  these tools when off; command execution, drawing geometry and metadata remain
  available so existing objects can still be edited through commands.

Enable **Furniture and equipment catalogues (freemans32)**, then open
**Window > Blocks > Catalogues**. Click a model, choose its insertion point and
its direction (a second click or a typed angle); Enter keeps the previous angle.
Esc ends repeated placement. Select a placed object to edit it in the palette.
Equipment can be saved as a named drawing model. The console also exposes
FURNITUREADD / FURNITUREEDIT / FURNITURELIST and EQUIPMENTADD / EQUIPMENTEDIT /
EQUIPMENTMODELSAVE / EQUIPMENTMODELDELETE / EQUIPMENTSIZEDISPLAY.

## Provenance

Original recipe geometry, parameter types, validation, catalogue, xdata encoding,
placement commands and their tests are by **freemans32**, under the source
repository's MIT OR Apache-2.0 licence. Relevant original commits include:

| Contribution | Original commit |
| --- | --- |
| Furniture catalogue and commands | `978f81711` |
| Initial equipment library and commands | `a411ceb45` |
| Drawing-owned equipment models | `02eaf8eb1` |
| Racks, coolers and dehumidifiers | `d9336322c` |

The imported snapshot contains subsequent fixes to these files. The original
`doc/src/ventilation` recipes live here in `doc/src/catalogue`; their serialized
application names and versions are unchanged. Shared recipe helpers, duct-port
sizes and distribution helpers were extracted with them. The catalogue reads
existing `CADCRAFT_SPACE` outlines for equipment distribution; room creation and
ventilation calculations are not part of this set.

Dievartis integration adapts the presentation to the shared Blocks dock, adds
optional visibility and its persistence, uses the current layout's paper units
when placing in paper space, and restores the complete command snapshot if a
batch placement/edit fails. Preview images use the existing bounded block
renderer and preserve every generated entity. No third-party block assets or
fonts are added. Network/takeoff tests from the source fork are deferred with
those modules; applicable furniture/equipment recipe and command tests accompany
this extraction.

**Contributor notification:** freemans32 is recorded for notification when this
integration is published; no notification has been sent by this change.

## Remaining integration sets

Walls, wall styles, openings, elevations/sections, room creation, structural
objects, HVAC/electrical/plumbing networks, calculations, takeoffs and IFC are
not included. They need the coherent regeneration, connection and property/grip
hooks identified in [the integration audit](freemans32-integration-audit.md).
General block grips remain available here; dedicated equipment connection grips
will accompany the network tools. This is a first integration set, not a release
of all of the source fork's building tools.

## Validation

The following suites passed in this Linux environment:

| Suite | Passed | Existing ignored tests |
| --- | ---: | ---: |
| Drawing model library | 108 | 0 |
| Engine library | 572 | 0 |
| Engine command integration | 1152 | 0 |
| I/O library | 212 | 0 |
| I/O integration | 591 | 24 |
| UI library | 339 | 1 |
| Catalogue UI workflows | 2 | 0 |

Total: 2976 passed; the 25 ignored tests are inherited DWG conformance cases and
an inherited translation-report test. No existing test was disabled.

The catalogue UI workflows also passed with actual headless OpenGL rendering;
furniture and equipment screenshots were inspected. These checks cover optional
visibility/persistence, vertical category/item layout, editor visibility and
starting interactive placement. Command tests cover every furniture model,
parameter editing, nine insertion anchors, model/paper-space units, rollback of
failed batches, and furniture/custom-equipment-model DXF/DWG round trips.

Native application `cargo check -p cadcraft`, Clippy for all targets of the four
changed crates with `-D warnings`, formatting, dependency layering, asset
attribution and the wasm-safe crate check passed. The wasm check retains the
baseline's unused native-renderer/preview-worker warnings. The generated menu
parity checklist remains 306/491 (62%); this counts menu breadth, not behavioural
coverage.

Windows compilation and interactive acceptance remain to be performed on the
user's machine before packaging or release.
