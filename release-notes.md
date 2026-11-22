A personal-fork preview of CADCraft, combining the community baseline update with selected parametric libraries from freemans32. This is not an official upstream CADCraft release. The application's inherited Cargo version is 0.21.0; the fork preview identifier is 0.7.

### Trade libraries

Open **Window > Blocks > Trade Libraries**. Use **Library settings** in the Blocks palette to enable Furniture/Equipment and Architecture. These settings are saved in preferences and profiles. Disabling tools preserves existing drawing geometry and metadata.

- Furniture: 89 furniture/appliance recipes, parametric dimensions and placement.
- Equipment: 13 families, including chillers, indoor/outdoor units, server-room cooling, dehumidifiers and computer racks; custom drawing-owned models are retained in DXF/DWG.
- Architecture: parametric walls and styles, hosted doors/windows/openings, wall junctions, curved walls, grips and 2D wall elevations.
- Source tabs remain at the top. **Left-hand categories toggle independently**: for example, Wall units, Wall racks and Floor racks can be displayed together. Clicking an active category again disables only that category.
- The explorer supports Filter, icon sizes, Details and List. Architecture uses generated wall/opening preview tiles by default; clicking a tile starts placement with that recipe.
- Selected-object dimension/property editors remain below the gallery. Current Drawing and Libraries keep all insertion options in a compact bottom footer.
- The **?** button directly shows the three instruction/credit texts on hover or click.

### Community baseline and retained fork features

- Upstream baseline at `a70b9b02e4fb7f2c3b50902d33e8daf96d904053`, including the official DIMSTYLE dialog and shared dock-tab close controls.
- Paper-unit compatibility with older fork drawings and the official page-setup encoding.
- STRETCH preselection and Shift removal, block insertion unit conversion, layer-state tools, shortcut recording and long dynamic-input menus.
- Hover measurements, known-length scale calibration, custom hatch persistence, copied hatch associations and AR-CONC improvements.
- Existing image management and PDF image output, font import/families, plot styles, templates, VERTEXEDIT, independent X/Y scaling, selection indicator, Shift-click layer inversion and cursor-edge positioning remain included.

### Credits and provenance

The original furniture, equipment, wall, opening and elevation recipes and applicable tests are from [freemans32/cadcraft](https://github.com/freemans32/cadcraft/tree/perso), audited at `e4964f0ba`, under MIT OR Apache-2.0. The integration adapts their presentation and shared command paths; it does not import that fork's complete building/network toolset or claim new authorship of its recipes.

Community adaptations retain attribution to Tangpixel2296, aldozar, florumpl, MarekSpodBiedry, Alex-BB44, ATARYS-LAB, barmaninbox, wovadi, nicholasgower, midasmatterhorn and mattlafy, alongside the original upstream and retained fork contributors. Exact PRs, source commits and adaptation details are recorded below:

- [Community baseline and contribution table](https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievart-preview-0.7.md)
- [Furniture/equipment provenance](https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/freemans32-catalogues.md)
- [Wall/opening provenance](https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/freemans32-walls.md)
- [Final palette behavior](https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/block-palette-layout.md)

### Validation and limits

The final UI change passed 339 UI unit tests, 18 classic Blocks palette tests and 3 trade-library workflows: **360 passed**, with one previously ignored report test. Workflows cover combined categories, filters, compact wrapping, tile-driven wall/opening placement, property editors and direct help. Headless GPU captures and accessibility queries were inspected. Clippy for all UI targets with warnings denied, formatting, dependency layers, asset attribution, WASM and parity checks passed. Earlier baseline and wall/IO integration matrices are documented in the linked reports; these are separate staged runs, not a claim of complete workspace CI on the release commit.

- Architecture is 2D parametric plan geometry with height data and separate 2D elevation snapshots; regenerate elevations after plan changes.
- Sections, rooms, slabs, roofs, full HVAC/electrical/plumbing networks, engineering calculations and IFC from the source fork are not included.
- PDF underlay calibration and collaboration are not included. Existing raster images can be calibrated and exported to PDF.
- Wall lining returns and hot-grip stretch-copy retain the limitations documented in the wall report.
- A reported MTEXT editing failure involving a trailing brace could not be reproduced; no fix is claimed.
- Additional fallback fonts require the optional, pinned `craft-fonts` build input. Bundling that pack must be verified for the supplied executable; installed Windows fonts alone do not establish that it was embedded.

Source commit: `bc1bf538012eeb34bb44e06f8fb61259bdc30729`.

### Windows archive

Download **cadcraft-0.7-windows-x64.zip**, extract it and run `cadcraft.exe`.

The fork owner compiled and supplied the Windows x64 executable for this release. ZIP integrity and the PE x64 architecture were verified. The supplied executable is preserved byte-for-byte; the distributed archive additionally includes licences, notices, source credits and integration reports.

- Archive: 28,605,143 bytes.
- Executable: 79,985,664 bytes.
- Archive SHA-256: `43eb0df7793ca750517135a480e675fb313585960caa3199820c929597d7df89`.
- Executable SHA-256: `cf55a81812e3668d152a7c560f24f4c238b567bc6a1fbcc998bb80362f374d0a`.

`SHA256SUMS.txt` is attached. This is an owner-built binary, not a reproducible-build or signing attestation. The optional additional fallback-font pack was not independently verified in this binary; system-font support remains available.
