# Dievartis Preview 0.9 development integrations

- Building and trade libraries: freemans32/cadcraft, branch `perso`, through `db6b941efa`. The trial branch retains its Blocks palette as the host for network tools.
- Dynamic block authoring: codyvogt, storytold/cadcraft PRs [#1827](https://github.com/storytold/cadcraft/pull/1827), [#2169](https://github.com/storytold/cadcraft/pull/2169), and [#2184](https://github.com/storytold/cadcraft/pull/2184). The stacked patch comes from `0bbb7f50` and was adapted to the fork's existing command routes, Block Editor, and lookup evaluator.
- The insulation geometry and hatch pattern editor are original CADCraft fork changes. The insulation presets are native to CADCraft's renderer; another CAD application may need its own corresponding shape to render them.
