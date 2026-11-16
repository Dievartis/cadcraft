# DIEVARTIS integration record

This branch integrates paper-space unit corrections into the dockable-manager preview.
It is a development integration, not a published release.

## Sources and attribution

| Contribution | Original source | Integrated commits / adaptation |
| --- | --- | --- |
| Preview baseline with dockable managers | [Dievartis fork](https://github.com/Dievartis/cadcraft/tree/d94ad4f792eeeda3d33e327ac8b39fc2529d02df) | `d94ad4f792eeeda3d33e327ac8b39fc2529d02df` |
| Dimension Style Manager | @Slopapalooza [#1042](https://github.com/storytold/cadcraft/pull/1042); dockable integration [fork #2](https://github.com/Dievartis/cadcraft/pull/2) | Existing implementation retained; Dievartis adapts its content to a dockable panel. Original manager authorship is not claimed. |
| Plot Style Manager | @Dievartis [#3142](https://github.com/storytold/cadcraft/pull/3142); integration context [fork #1](https://github.com/Dievartis/cadcraft/pull/1) | Included in baseline; original contributions retain their authorship. |
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
