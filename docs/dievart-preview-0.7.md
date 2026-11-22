# Dievartis Preview 0.7 integration

Development branch: `integration/preview-0.7-community-update`. This is an integration
candidate, not a published release. Windows acceptance remains required.

## Official baseline

Merge the official `storytold/cadcraft` main at
`a70b9b02e4fb7f2c3b50902d33e8daf96d904053`, retaining the fork's image manager,
font-family picker and font import, plot-style panel, drawing templates, vertex
selection, independent XY scaling, selection indicator, layer inversion and
cursor-prompt positioning. The workspace version is inherited from upstream;
it does not identify a Dievartis preview on its own.

DIMSTYLE now uses the official dialog (#1460), rather than a second docked editor.
An older saved workspace's DIMSTYLE pane is closed and its dialog opened without
discarding the positions of other panes. The legacy panel identity remains readable.
Text and plot styles and images remain dockable. Header close buttons use the
official common docking implementation (#1867), replacing the fork's extra-tab
buttons. The drawing canvas remains protected.

## Paper-unit compatibility

The official `PageSetup.paper_mm: Option<bool>` and `paperUnits: "mm"|"inches"`
command spelling are authoritative. `None` follows the drawing's MEASUREMENT.
Legacy `pagesetup {paperMm: bool}` calls remain accepted; a simultaneous official
`paperUnits` choice takes precedence. One-off plot paper changes do not silently
rescale existing layout geometry.

The reader recognizes the preview-era `PAPERUNITS` xdata codes (0 inherited,
1 millimetres, 2 inches, 3 legacy explicit millimetres), including metadata before
AcDbLayout and separate CADCRAFT xdata groups. Official `PSUNITS` metadata wins
when both are present. Saving uses the official encoding. Saved paper views are
retained. Selecting the same paper again preserves an explicit unit choice, as
in current upstream; an ANSI family does not force inches in a metric drawing.

## Community contributions adapted

Original code remains under CADCraft's MIT OR Apache-2.0 licence. These are
adaptations of the contributions below, not new authorship claims. Titles and
commit history in the original PRs record additional contributors.

| Source PR | Original contributor | Reviewed head |
|---|---|---|
| [#1011](https://github.com/storytold/cadcraft/pull/1011) | @Tangpixel2296 | `5285a546f33cf8d48b85f5503ab0432ad53f5239` |
| [#999](https://github.com/storytold/cadcraft/pull/999) | @aldozar | `7f2d6cc245e03306a8083a065904d1ade91df486` |
| [#3044](https://github.com/storytold/cadcraft/pull/3044) | @florumpl | `638e7e1149d0766eec815dcb72b958e6163e7f55` |
| [#2006](https://github.com/storytold/cadcraft/pull/2006) | @MarekSpodBiedry | `04eced295a813b81c513c3f3068879b379b44c58` |
| [#1704](https://github.com/storytold/cadcraft/pull/1704) | @Alex-BB44 | `7768018b3161c535b45842e6c4081ab94bad3d5c` |
| [#1705](https://github.com/storytold/cadcraft/pull/1705) | @Alex-BB44 | `510ad49938245db23b52cf985f0d5b8846cf4658` |
| [#1805](https://github.com/storytold/cadcraft/pull/1805) | @Alex-BB44 | `155eb342b4bdddf7865fc297567ded4e24fa1240` |
| [#3086](https://github.com/storytold/cadcraft/pull/3086) | @ATARYS-LAB | `db4e7e2421598233fc2a1697229450ddc0f7524a` |
| [#3046](https://github.com/storytold/cadcraft/pull/3046) | @barmaninbox | `52993fb878a0e60030d53f54ddd8b96ba6bcb362` |
| [#2776](https://github.com/storytold/cadcraft/pull/2776) | @wovadi | `7f8cbc768724d0cbb5226a29107e837c5dc40fb2` |
| [#1896](https://github.com/storytold/cadcraft/pull/1896) | @nicholasgower | `10ea780553a9ee760ea760b083b98fe002dfe11e` |
| [#1233](https://github.com/storytold/cadcraft/pull/1233) | @midasmatterhorn | `1cae514a0da2ef4ac8358284ea43491980124cfe` |
| [#1451](https://github.com/storytold/cadcraft/pull/1451) | @mattlafy | `df5c6c3a350255b2e4ca5b89dffa3ca014ee5746` |
| [#2020](https://github.com/storytold/cadcraft/pull/2020) | @Tangpixel2296 | `deb8cda68c051a2fdbd520f03ad0ce38be03783a` |

- #999 and #3044: retain the windows that formed a preselection for STRETCH;
  Shift removes objects during its selection phase. VERTEXEDIT's independent
  Shift toggle is retained. Window and group selection still respect viewport-
  frozen layers. Stale windows are not applied after drawing edits or undo.
- #2006: block insertions convert source INSUNITS to target INSUNITS on top of
  requested scales. Unitless/unknown units and xrefs keep their existing policy.
- #1704, #1705 and #1805: quick layer-state switcher, optional unselectable locked
  layers (snaps remain available), moving selected objects to a new layer and
  purging empty layers. The switcher restores visibility only; full restoration
  retains upstream's property masks and saved current-layer rules. Existing
  Shift-click inversion and current-layer freeze guards remain.
- #3086 and #3046: ignore modifier-only shortcut recording and scroll long
  dynamic-input keyword menus, preserving the fork's cursor-edge placement.
- #2776 and #1896: quick hover measurements and scale calibration using a known
  distance. Calibrate Selection can include raster images; the separate Underlay
  target remains unavailable and reports that limitation. No PDF underlay support
  is claimed by this integration.
- #1011, #1233, #1451 and #2020: custom hatch definitions remain attached to the hatch,
  copied boundaries are remapped to copied hatches, and the original AR-CONC
  design uses closed stones and dots. Negative line offsets keep the correct dash
  phase (#1011), a prerequisite of the closed pattern. The newer bounded renderer and work caps
  remain in force when dense swatches zoom in.

The PDF underlay chain and collaboration server/client are separate projects;
they are not part of this candidate. freemans32's personal building tools are
being audited separately; see `freemans32-integration-audit.md`.

The source table is also the notification queue for publication. No new
contributor notification is claimed for this unaccepted development candidate.

## Packaging fonts

Keep the official optional font input when packaging the next executable:
`CRAFT_FONTS_DIR` must point to a checkout of `storytold/craft-fonts` at the
revision already pinned by the release workflow (`b076417b5880bd4154bd22f00695b1d28c6bea83`).
The existing Windows packaging script copies its font licences. A plain local
build without that environment variable does not embed this extra fallback pack;
using installed system fonts is a separate capability, not a replacement for it.
The code and tests must continue working without this optional build input.

## Acceptance

Verify existing DXF/DWG drawings and templates, A3 landscape units, old preview
paper settings after reopen/save, image placement/DPI/brightness/contrast/PDF
output, font import, layer inversion plus undo, STRETCH preselection and Shift,
VERTEXEDIT Shift toggles, XY scaling, official DIMSTYLE and palette close/reopen.
Verify the Measure dropdown, long prompt options and shortcut recording.

## Validation

The final focused test matrix passed 3,587 tests with no failures. Twenty-five
pre-existing ignored tests remain ignored (24 I/O conformance cases, including
documented native DWG limitations, and one UI translation-report test); no test
was newly ignored or disabled by this integration.

| Suite | Passed |
|---|---:|
| Engine unit / integration | 572 / 1,138 |
| Fork image, paper-unit, block-unit, template, XY/vertex and layer-inversion regressions | 48 |
| I/O unit / integration / preview-format migration | 212 / 591 / 8 |
| UI unit / integration / retained-preview workflows | 339 / 230 / 4 |
| Rendering unit / integration | 143 / 68 |
| Document / font unit tests | 97 / 136 |
| Default-template profile and startup | 1 |

Workspace Clippy across all targets with `-D warnings`, dependency layering,
asset attribution (28 files) and the wasm check passed. Native application and
test-target compilation checks also passed. An additional complete engine
integration run passed after the negative hatch-offset adaptation.
Formatting passed and the generated menu-breadth checklist was regenerated with
`cargo xtask parity`. The two padded Polish button-label whitespace diagnostics
come from the unchanged upstream translation strings; their significant spaces
are preserved.

The image panel was rendered headlessly with the GL backend and inspected. Its
AccessKit-backed close-button query, tab-strip alignment and close/reopen workflow
passed. The retained-preview UI suite also checks default settings, cursor-edge
prompts and migration of a legacy DIMSTYLE pane while keeping image/text panes.

The inherited strict Chinese menu-completeness test requires translations for
new visible command labels. Fourteen missing entries were supplied instead of
weakening or disabling that check; this is a small exception to the current
English-only feature-change guideline. Other missing translations retain the
normal English fallback.

The full workspace `cargo xtask ci` aggregate and regression controls with the
adaptations reverted were not run. Tests here use the optional font input unset;
they do not establish the contents of a packaged fallback-font bundle. Windows
compilation and acceptance with the owner's real drawings are still required.
