# Block palette layout

The block palette keeps Current Drawing, Libraries and Trade Libraries available. Library settings in the palette enable furniture/equipment and architecture; these switches continue to use the existing options command and saved preferences. Drawing Setup no longer duplicates the switches. Disabling a module does not remove existing drawing objects.

Furniture retains collapsible categories and equipment retains family grouping, with compact plan-preview rows by default. Both share the classic library's Filter and View controls, including icon sizes, Details and List. The question-mark menu holds placement instructions, selection help and source credits. Selected-object dimension editors remain available.

Current Drawing and Libraries retain scale X/Y/Z, rotation, explode and repeat placement. The compact footer reserves its measured content height at the bottom; the gallery uses the remaining height without a fixed maximum. Resizing and wrapped controls trigger remeasurement.

Recipes remain derived from freemans32/cadcraft under MIT OR Apache-2.0, as recorded in freemans32-catalogues.md and freemans32-walls.md. This change affects presentation only and does not replace recipe geometry or drawing data.

The French label “Bibliothèques métier” is included at the operator's explicit request; other new interface strings use English translation keys.

Validation: 339 UI unit tests, 18 classic block-palette tests and 3 trade-library workflows pass (one existing ignored unit test). The workflows exercise activation from the palette, preferences/profiles, resizing, compact rows, filter input, icon layout, placement and selected-object editors. GPU captures were checked with their accessibility queries. Formatting, Clippy for all UI targets with warnings denied, dependency layers, asset attribution, WASM and parity checks pass. Other UI integration binaries were not rerun for this presentation change; no Windows executable was built in the Linux workspace.
