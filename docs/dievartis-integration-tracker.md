# Dievartis fork: integration tracker

This is the central tracking page for contributions integrated into this personal fork.
Last audited: **2026-11-22**. GitHub handles and the 14 adapted community PRs were checked against their public source records.

**Published build:** [Preview 0.7](https://github.com/Dievartis/cadcraft/releases/tag/dievart-preview-0.7), source `bc1bf538012eeb34bb44e06f8fb61259bdc30729`.
[Windows x64 ZIP](https://github.com/Dievartis/cadcraft/releases/download/dievart-preview-0.7/cadcraft-0.7-windows-x64.zip) · [Checksums](https://github.com/Dievartis/cadcraft/releases/download/dievart-preview-0.7/SHA256SUMS.txt).

The published fork build lives on the release tag and the `integration/block-palette-layout` development line. The default branch's source is not the release snapshot. Use the release assets/tag to test the integrated version.

## How to read this page

- **Released / adapted:** selected source code was adapted into a published fork snapshot.
- **Released / inherited:** functionality came through the upstream baseline, rather than a separate fork extraction.
- **Superseded:** kept in history, but replaced in the current release.
- **Audited / candidate:** examined or proposed; not available in the published build.
- Release inclusion does **not** mean the original PR was merged upstream, approved by its author, or endorsed by upstream. Source PRs may still be open. Existing fork integration PRs may also remain open even though their commits are included in a release tag.

## Community adaptations first published in 0.7

| Contribution | Original author | Source | Fork status | Release | Notice |
| --- | --- | --- | --- | --- | --- |
| Correct dash phase for negative hatch-line offsets | [Tangpixel2296](https://github.com/Tangpixel2296) | [#1011](https://github.com/storytold/cadcraft/pull/1011) | Released / adapted | 0.7 | [N07] |
| STRETCH crossing-window preselection | [aldozar](https://github.com/aldozar) | [#999](https://github.com/storytold/cadcraft/pull/999) | Released / adapted | 0.7 | [N07] |
| Shift removal during STRETCH selection | [florumpl](https://github.com/florumpl) | [#3044](https://github.com/storytold/cadcraft/pull/3044) | Released / adapted | 0.7 | [N07] |
| Block insertion unit conversion | [MarekSpodBiedry](https://github.com/MarekSpodBiedry) | [#2006](https://github.com/storytold/cadcraft/pull/2006) | Released / adapted | 0.7 | [N07] |
| Quick layer-state switcher | [Alex-BB44](https://github.com/Alex-BB44) | [#1704](https://github.com/storytold/cadcraft/pull/1704) | Released / adapted | 0.7 | [N07] |
| Optional unselectable locked layers | [Alex-BB44](https://github.com/Alex-BB44) | [#1705](https://github.com/storytold/cadcraft/pull/1705) | Released / adapted | 0.7 | [N07] |
| Move selection to a new layer; purge empty layers | [Alex-BB44](https://github.com/Alex-BB44) | [#1805](https://github.com/storytold/cadcraft/pull/1805) | Released / adapted | 0.7 | [N07] |
| Modifier-aware shortcut recording | [ATARYS-LAB](https://github.com/ATARYS-LAB) | [#3086](https://github.com/storytold/cadcraft/pull/3086) | Released / adapted | 0.7 | [N07] |
| Scrollable long dynamic-input option menus | [barmaninbox](https://github.com/barmaninbox) | [#3046](https://github.com/storytold/cadcraft/pull/3046) | Released / adapted | 0.7 | [N07] |
| Quick hover measurements and Measure dropdown | [wovadi](https://github.com/wovadi) | [#2776](https://github.com/storytold/cadcraft/pull/2776) | Released / adapted | 0.7 | [N07] |
| Known-length scale calibration; Underlay target excluded | [nicholasgower](https://github.com/nicholasgower) | [#1896](https://github.com/storytold/cadcraft/pull/1896) | Released / adapted | 0.7 | [N07] |
| Keep custom hatch definitions through property changes | [midasmatterhorn](https://github.com/midasmatterhorn) | [#1233](https://github.com/storytold/cadcraft/pull/1233) | Released / adapted | 0.7 | [N07] |
| Copy associative hatches with their copied boundaries | [mattlafy](https://github.com/mattlafy) | [#1451](https://github.com/storytold/cadcraft/pull/1451) | Released / adapted | 0.7 | [N07] |
| AR-CONC closed stones and dots | [Tangpixel2296](https://github.com/Tangpixel2296) | [#2020](https://github.com/storytold/cadcraft/pull/2020) | Released / adapted | 0.7 | [N07] |

Exact reviewed commits, integration changes and staged checks: [community integration report][community].

## freemans32 integrations

Source: [freemans32/cadcraft, perso](https://github.com/freemans32/cadcraft/tree/perso), audited snapshot `e4964f0ba`, MIT OR Apache-2.0. The complete fork and its full PR selection were not imported.

| Contribution | Original source commits | Fork adaptation | Status / release | Notice |
| --- | --- | --- | --- | --- |
| Furniture and appliance recipes | `978f81711` | Shared Blocks palette, dimensions, placement, independent category toggles | Released / adapted, 0.7 | [F07] |
| Equipment, custom models, racks/coolers | `a411ceb45`, `02eaf8eb1`, `d9336322c` | Parameter editor, saved models, DXF/DWG persistence, combined family display | Released / adapted, 0.7 | [F07] |
| Walls, styles, junctions and hosted openings | `fbe59962c`, `1b6151e9a`, `76fa82f91`, `a60e616f7`, `77d14aac8` | Optional Architecture tools, preview tiles, shared command/grip paths and rollback | Released / adapted, 0.7 | [F07] |
| Wall grips, 2D elevations, object recognition | `80bd20cee`, `ca76b987b`, `c5e958dc2` | Rotated/reflected STRETCH support; elevations recognise integrated furniture/equipment | Released / adapted, 0.7 | [F07] |

Detailed sources and limits: [catalogues][catalogues] · [walls/openings][walls] · [final palette behavior][palette].

## Retained fork features and upstream convergence

Release numbers below identify documented availability, not necessarily the earliest experimental implementation.

| Feature | Source / attribution | Current status | Documented release | Notice / record |
| --- | --- | --- | --- | --- |
| Dockable Plot Style Manager | Dievartis, [#3142](https://github.com/storytold/cadcraft/pull/3142) | Released / fork work | 0.4 onward | [Earlier integration record][earlier] |
| Dockable text styles and font-family/variant discovery | Dievartis restoration; davidhooey [#2462](https://github.com/storytold/cadcraft/pull/2462) | Released / adapted | 0.4 onward | [N04] |
| Font-folder discovery / SHX parsing | luminatiguy [#2316](https://github.com/storytold/cadcraft/pull/2316), francescoBV [#1944](https://github.com/storytold/cadcraft/pull/1944) | Released / inherited APIs | 0.4 onward | [N04] |
| Image manager, placement/display adjustments and PDF output | Fork integration over upstream raster/PDF APIs; dotCooCoo [#3151](https://github.com/storytold/cadcraft/pull/3151) decoding budget | Released / adapted | 0.4 onward | [N04]; [earlier record][earlier] distinguishes inherited and reference-only work |
| Paper-space units | aldozar [#1085](https://github.com/storytold/cadcraft/pull/1085), Dievartis [#3207](https://github.com/storytold/cadcraft/pull/3207), current upstream encoding | Released; 0.7 migrates legacy fork metadata to official units | 0.4 onward | [N04]; [community report][community] |
| VERTEXEDIT, additive vertex windows and Shift toggles | Fork work requested by Dievartis | Released; separate from STRETCH | 0.5 onward | [Editing report][editing]; reporter outreach [N05] |
| Independent X/Y scaling and selection indicator | Fork work requested by Dievartis | Released | 0.5 onward | [Editing report][editing]; [N05] |
| Templates and persistent defaults | vangel76 [#562](https://github.com/storytold/cadcraft/pull/562), dotCooCoo [#2177](https://github.com/storytold/cadcraft/pull/2177); fork menus/startup integration | Released / adapted | 0.5–0.6 onward | [N05]; [0.6 record][preview06] |
| Shift inversion of other layer states; Drawing Setup; cursor-edge prompt placement | Fork work requested by Dievartis, using existing layer/preferences APIs | Released | 0.6 onward | [0.6 record][preview06] |
| Earlier docked DIMSTYLE manager | Slopapalooza [#1042](https://github.com/storytold/cadcraft/pull/1042), adapted into a dock in the fork | Superseded by the official dialog in 0.7 | Earlier previews | [N04] |
| Official DIMSTYLE dialog | kk376 [#1460](https://github.com/storytold/cadcraft/pull/1460) | Released / inherited upstream baseline | 0.7 | Attribution retained; not represented as a new fork adaptation |
| Common dock-panel close button | nicholasgower [#1867](https://github.com/storytold/cadcraft/pull/1867) | Released / inherited; replaces the fork's separate close implementation | 0.7 | Author also mentioned in [N07] for the separately adapted calibration work |

## Examined but not integrated

These are **not** promises of delivery or claims that the upstream work is incomplete.

| Area | Source / context | Status / boundary |
| --- | --- | --- |
| Rooms, slabs, roofs, structure and full building sections | [freemans32 audit][audit] | Audited only; absent from 0.7 |
| Full HVAC, plumbing and electrical networks/calculations; IFC | [freemans32 audit][audit] | Audited only; absent from 0.7 |
| Dedicated clipping-module integration | [#1565](https://github.com/storytold/cadcraft/pull/1565) | Candidate retained separately; not claimed as integrated |
| Additional PDF underlay chain / calibration | [Community scope][community] | Not integrated; raster-image PDF output is a different, included capability |
| Collaboration server/client | [Community scope][community] | Separate project; not integrated |
| Proposal #1093 | [Earlier integration record][earlier] | Explicitly excluded at the fork owner's request |
| Trailing-brace MTEXT editing report | Fork owner's observation | Not reproduced; no code change or fix claimed |

## Notification log

A posted mention is evidence that a message was posted, **not** proof of delivery, reading or acknowledgment. No acknowledgment is assumed.

| Date | Context | People / role | Evidence |
| --- | --- | --- | --- |
| 2026-11-17 | Preview 0.4 first-publication notice | Slopapalooza, davidhooey, luminatiguy, francescoBV, aldozar, wovadi, dotCooCoo, graficaeusebio-cloud; message distinguishes adapted, inherited and reference-only work | [N04] |
| 2026-11-20 | Preview 0.5 editing/template announcement | Reporters grandixximo, chiruvedadotai, R00118189, Archi-AHG, dsalesarchitect, EP995LAB, scdragos; template API credits to vangel76/dotCooCoo | [N05]; [publication record][preview05] |
| 2026-11-22 | Preview 0.7 first notice for the 14 community adaptations above | Tangpixel2296, aldozar, florumpl, MarekSpodBiedry, Alex-BB44, ATARYS-LAB, barmaninbox, wovadi, nicholasgower, midasmatterhorn, mattlafy | [N07] |
| 2026-11-22 | Preview 0.7 first notice for the selected building-tool extraction | freemans32; furniture/equipment/walls/openings/elevations grouped into one message | [F07] |

The 0.7 notices include aldozar and wovadi for **different, newly adapted contributions**; they do not repeat the 0.4 notice for unchanged work. Other unchanged 0.4 contributors were not mentioned again.

A direct reply to the original freemans32 showcase, upstream issue #3354, was refused with HTTP 403. No successful upstream comment is claimed; the actual notice is the linked mention in this fork.

## Maintenance rules

1. Add an entry when source work is selected: author, PR/commit, intended scope and status. Separate adaptation from inherited APIs and reference-only comparisons.
2. Mark it released only when the exact commit is included in a published tag; add the version, relevant test report and known limits. An open/merged PR label alone is not evidence of usable behavior.
3. Group a contributor's newly published work into one English notice. Reuse the discussion where appropriate; prefer a single fork notice over announcements on many upstream PRs.
4. Record the message URL and date. Do not repeatedly mention an author for unchanged contributions at every release. Recontact them for relevant new adaptations, fixes or technical feedback.
5. When an integration is replaced, mark it superseded and preserve its history and attribution. Never imply that a source author's silence is approval.

[N04]: https://github.com/Dievartis/cadcraft/pull/3#issuecomment-6956902581
[N05]: https://github.com/Dievartis/cadcraft/pull/3#issuecomment-7033174135
[N07]: https://github.com/Dievartis/cadcraft/pull/4#issuecomment-7087060627
[F07]: https://github.com/Dievartis/cadcraft/pull/5#issuecomment-7087065638
[community]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievart-preview-0.7.md
[catalogues]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/freemans32-catalogues.md
[walls]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/freemans32-walls.md
[palette]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/block-palette-layout.md
[earlier]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievartis-integration.md
[editing]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievart-editing-preview.md
[preview06]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievart-preview-0.6.md
[preview05]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/dievart-preview-0.5.md
[audit]: https://github.com/Dievartis/cadcraft/blob/bc1bf538012eeb34bb44e06f8fb61259bdc30729/docs/freemans32-integration-audit.md
