# DIEVARTIS integration record

This branch integrates paper-space unit corrections into the dockable-manager preview.
It also restores the dockable Text Style Manager and font import from the saved preview.
It is a development integration, not a published release.

## Sources and attribution

| Contribution | Original source | Integrated commits / adaptation |
| --- | --- | --- |
| Preview baseline with dockable managers | [Dievartis fork](https://github.com/Dievartis/cadcraft/tree/d94ad4f792eeeda3d33e327ac8b39fc2529d02df) | `d94ad4f792eeeda3d33e327ac8b39fc2529d02df` |
| Dimension Style Manager | @Slopapalooza [#1042](https://github.com/storytold/cadcraft/pull/1042); dockable integration [fork #2](https://github.com/Dievartis/cadcraft/pull/2) | Existing implementation retained; Dievartis adapts its content to a dockable panel. Original manager authorship is not claimed. |
| Plot Style Manager | @Dievartis [#3142](https://github.com/storytold/cadcraft/pull/3142); integration context [fork #1](https://github.com/Dievartis/cadcraft/pull/1) | Included in baseline; original contributions retain their authorship. |
| Recovered Text Style Manager and font import | [Saved preview](https://github.com/Dievartis/cadcraft/commit/0cf789bea2029957abb546f2edfaae2bac75cd4d), recorded by Git as @Dievartis; development history supplied by the fork owner | The dockable UI was developed in the fork, then compared with the classic dialog in #2462. This restoration recovers `textstyles.rs`, its docking hooks and the saved MTEXT family choice. |
| Classic Text Style dialog used for comparison | @davidhooey [#2462](https://github.com/storytold/cadcraft/pull/2462) | The fork owner identifies this contribution as the source compared with the dockable UI. The retained V8 step-1 discussion schedules font-family and variant integration as the next step; the recovered snapshot still uses a flat font list. This branch now adapts family/variant discovery from head `dbb28a2884326f7ff16725224fb744830fed0ec0` into the fork's dockable panel, with bounded naming-table reads and cached discovery. The classic dialog container is not imported. |
| Shared font-folder discovery APIs | @luminatiguy [#2316](https://github.com/storytold/cadcraft/pull/2316), landed in [#2803](https://github.com/storytold/cadcraft/pull/2803) | Existing font-folder registration and redraw-generation APIs, reused by this restoration. |
| SHX font parsing | @francescoBV [#1944](https://github.com/storytold/cadcraft/pull/1944), landed in [#2751](https://github.com/storytold/cadcraft/pull/2751) | Existing SHX reader, reused for validation and rendering. |
| Paper-space units | @aldozar [#1085](https://github.com/storytold/cadcraft/pull/1085), extended by @Dievartis [#3207](https://github.com/storytold/cadcraft/pull/3207) | `acf90b3a562eb31f46b11fd2043de742169352a4`, `bc5f4d3a8e14de19176b0be55699e2f10324f471` |

This is a record of the integration above, not an exhaustive inventory of inherited
CADCraft contributions. Existing copyright notices and MIT / Apache-2.0 licenses
remain applicable. No new dependency or asset is added. Authors have not been
contacted as part of this integration.

## Scope

Keep the managers from the preview and add the unit changes in their existing
page-setup, paper-rendering and DXF modules. Store PAPERUNITS and optional PSVIEW
in one CADCRAFT extended-data group after the normal LAYOUT fields. Read unit
metadata from the complete record, including older pre-AcDbLayout placement.

## Acceptance checks

- Selecting A3 landscape yields a 420 by 297 paper-unit sheet with both metric and imperial MEASUREMENT.
- Explicit inches remain 25.4 mm per paper unit in a metric drawing.
- Explicit and inherited unit choices survive a DXF round trip, with and without a saved view.
- Saved paper views survive the same round trip; one CADCRAFT group follows normal layout fields.
- Manually verify the existing drawing, viewport scale and both dockable managers in the Windows build before release.

Changing units on an existing layout does not rescale its existing geometry.
The original drawing and its external image files are needed to validate the user's full workflow.

## Restored text styles

Bare STYLE / ST opens the Text Style Manager alongside DIMSTYLE and the Plot Style Manager.
Programmatic STYLE calls still reach the engine without opening a panel. Creation, edits,
renaming, deletion and current-style selection use the existing engine commands.

Desktop font import accepts TTF, OTF and SHX files, up to 32 MiB, and copies them into
CADCraft/fonts under the platform's application-data directory. Duplicate file names get
numbered suffixes; existing font files are retained. The stored font name is resolved through
the registered managed directory, including after application startup. Import is disabled in
the web build. Imported font files are user data and are not bundled or committed here.

The browser and preview remain UI code. No font, render or engine crate is changed by this
restoration, and no new dependency is added. All additions are in English.

### Recovered development history

The fork owner supplied an excerpt identifying PR #2462 by @davidhooey. It records a
successful V8 step-1 build and correction of an `Option` iteration warning in
`textstyles.rs`. The restored code already uses `if let Some` for that branch.

The excerpt explicitly describes family / variant selection from #2462 as a next step.
The current restored snapshot supports font-file import and selection from a flat list;
it does not yet reproduce #2462's family / variant selectors. A later V8 revision or
separate integration is needed to recover that behavior. Do not claim those selectors
are already restored, or substitute unrelated PR #1083 for the identified source.

## Font families and variants

The dockable panel groups outline fonts by their naming-table family and offers a
separate Font style selector. It preserves an existing file name until the user
changes a selection and applies properties. Imported fonts remain in the managed
folder; missing names, SHX and the built-in stroke font retain their existing
values. Discovery is cached and invalidated by font registration or folder rescans.
The engine change only exposes its existing used-style query to the panel.

Source: @davidhooey, upstream PR #2462. The naming-table index and regression
fixture are adapted from that contribution; docking, import and property editing
remain the fork's implementation. No font binary or dependency is added.

Validation for this adaptation covers the fonts unit suite, dockable-manager UI
tests, font-import tests, DXF round trips, undo, a cold collection-name index,
and a headless-rendered panel inspection. Formatting, Clippy for fonts/engine/UI
(all targets), asset attribution, dependency layering and the wasm check pass.
Full workspace CI and manual Windows acceptance remain required before release.

## Image loading memory budget

Adapted from @dotCooCoo's upstream PR #3151:
https://github.com/storytold/cadcraft/pull/3151

Concurrent loaders reserve the largest supported decoded image size before starting
another file. Near the limit they wait for ongoing decodes; failures release their
reservation. The upstream policy permits at most one decoded image beyond the limit;
the existing retention pass excludes excess images. This bounds decoded pixel buffers,
not total process memory, decoder scratch space or compressed source buffers.
The reservation matches the current 128-million-pixel decoder cap (512 million RGBA bytes).
The change stays in the image loader and preserves the fork's image-editing commands.

### Contributors to notify

| Contributor | Contribution used | Notification status |
| --- | --- | --- |
| @dotCooCoo | #3151: concurrent image decoding budget and regression tests | Pending; no notification sent |

PR #1565 remains a separate clipping-module candidate. PR #1093 is excluded from
this integration at the fork owner's request.
