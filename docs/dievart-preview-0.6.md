# Dievartis Preview 0.6 preparation

Follow-up to preview 0.5, implementing the fork owner's priorities 5–8.
No Windows binary or public release is produced by this preparation.

## Layer states

Shift-click On/Off, Freeze/Thaw or Lock/Unlock in the Layers palette (including
its dropdown) or the Layer Properties Manager to invert that property separately
for every other layer. The clicked layer remains unchanged; ordinary clicks keep
their existing behavior. One inversion is one undo operation.

The existing engine rule forbidding freezing the current layer remains in force:
that layer stays thawed and a command message reports the exception. Visibility
and locking still invert for the other layers, including the current layer.
The implementation lives in `cmd/layer/invert.rs` and uses the command
`layer.invertothers {name, property: on|frozen|locked}`.

## Drawing preferences

OPTIONS (or PREFERENCES) gains a Drawing Setup tab. It exposes the saved default
template and shortcuts to choose/clear it, edit drawing units, open Page Setup
Manager, customize keyboard shortcuts and save a template. Existing preference
tabs remain available. Units and paper belong to the drawing: save the configured
drawing as a template to reuse millimetres, A3 landscape layouts and title blocks.

The first untitled drawing at desktop startup now uses the persisted template.
A missing/unreadable template reports an error and falls back to a blank drawing;
already-open drawings are untouched. Existing template loading and relative-image
resolution are reused. The preferences surface remains the existing Options
window; it has not been converted into a dockable palette in this change.

## Cursor prompts

Dynamic input is measured before placement. When the prompt does not fit on the
right of the cursor, it is placed to the left. Near the upper edge it moves below
the cursor, with vertical placement constrained to the drawing viewport. A prompt
wider than the entire viewport can still be clipped.

## Palette closing

Every dockable palette group has an always-visible close cross directly after its
tab names, on the same strip. It closes the active tab. The drawing canvas is excluded. Closing uses the existing `ui.dock`
command and hidden-panel restoration, so it does not delete settings or content.
No content row is reserved for the cross. The shared tab-strip extension is used
without replacing the docking renderer or its drag/drop behavior.

## Integration scope

No upstream contribution is copied for these changes. Existing Options, template,
layer and shared docking APIs are reused with their original attribution retained.
The template APIs are credited in [the integration record](dievartis-integration.md).
An existing nested-conditional lint in the image-density reader is simplified
without changing its behavior, so the required Clippy check can run.

Windows acceptance should check mixed layer states, the current-layer exception,
undo, template startup after restarting, cursor prompts near the right edge and
closing/reopening each palette, including a floating palette on another monitor.

## Validation

- Workspace formatting and Clippy across all targets with `-D warnings` passed.
- Two layer-inversion regression tests passed, covering mixed states, preservation
  of the clicked layer, the current-layer freeze guard, undo and invalid inputs.
- Three UI workflow tests passed: palette closing/restoration, opening drawing
  settings and a live command prompt near the right canvas edge.
- Default-template persistence, startup loading and missing-template fallback
  passed in an isolated test process.
- The tooltip placement unit regression passed; six earlier editing/template
  regression tests also passed.
- Headless renders of the preferences, palette close controls and cursor prompt
  were inspected.
- Asset attribution, dependency layering and the web/wasm check passed.

The full `cargo xtask ci` run passed formatting and Clippy, but was interrupted
during compilation of the workspace test executables because the filesystem was
nearly full. Disposable test executables were removed, keeping sources and
dependencies. A complete workspace test run, doctests and the translation-gap
report are not claimed. The focused checks above were run separately.

The tab-strip close follow-up reran the three UI workflow tests, inspected a
headless palette render and passed UI-library Clippy with `-D warnings`. Its close
test also checks alignment with the tab strip before closing and reopening the
palette. Full workspace CI was not repeated for this UI-only refinement.
