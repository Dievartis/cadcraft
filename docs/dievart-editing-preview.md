# Editing and template preview

Development follow-up to Dievart preview 0.4. No release binary is published by this change.

## 1. Selected vertex editing

Select lines or polylines, then Modify > Edit Selected Vertices (`VERTEXEDIT`).
All eligible nodes are shown as square handles. Click two opposite corners of the
vertex window; selected nodes use the hot-grip color. Shift-click toggles individual
nodes, and additional two-click windows add more nodes. Press Enter to confirm,
then pick a displacement base point and destination. Only the chosen nodes move. The
rectangle is previewed between the first and second click. Escape cancels without
changing geometry; the final operation is undoable.

This implementation uses additive two-click rectangles rather than press-and-drag.
It supports LINE, LWPOLYLINE and POLYLINE3D. It does
not replace STRETCH or import upstream PRs #999 and #2783; those remain separate
fix candidates. The restricted node-edit workflow was requested by the fork owner.

## 2. Independent XY scaling

Select objects, then Modify > Scale X/Y (`SCALEXY`). Pick the fixed base point,
enter X factor, then Y factor. For example, X=1 and Y=0.5 halves the height.
Positive factors from 0.000001 to 1000000 are supported. Circles become ellipses.

Supported geometry: lines, zero-width straight polylines, 3D polylines (XY only),
points, circles, arcs, ellipses, splines, images and SOLID/TRACE faces. Arcs become
elliptical arcs and existing ellipses retain their transformed shape. Other geometry, including
text, dimensions and block references, is rejected before
any selected object is changed. Negative/mirrored scales and a live deformation
preview are not implemented. The original uniform SCALE command is unchanged.

## 3. Drawing templates

File > Templates offers Save as Template, Choose Default Template and Clear Default
Template. File > New from Template remains available. Prepare millimetre units,
two A3 landscape layouts, layers/styles and title blocks in a drawing, save it as
DWT, and choose it as the default. The default QNEWTEMPLATE path is saved in the
existing application preferences and restored on application startup.
Linked image paths are anchored to the template folder and their rasters are loaded
when the untitled drawing is created, so a title-block logo is not lost on creation.

Save as Template is a normal Save As and changes the current drawing's file path.
A new drawing made from a template remains untitled. Moving/deleting a linked
template will cause the next NEW to report an error. Importing a layout into an
existing drawing through LAYOUT Template is not implemented by this change.

The engine implementation is inherited from @vangel76's upstream PR #562 and the
existing layout-save support from @dotCooCoo's #2177. This change adds UI access
and default-template persistence; it does not claim original template authorship.

## 4. Selection indicator

A small LED next to the four-square drawing switcher is green when the active
drawing's selection is empty and red when it is nonempty, including offscreen
objects. It has a textual tooltip and no object counter or destructive click action.

## Acceptance

Verified during implementation:

- Engine/UI compilation and library Clippy with `-D warnings`.
- Engine regression coverage: restricted vertex movement and undo; the interactive
  vertex-edit workflow; circle-to-ellipse scaling followed by scaling the ellipse again;
  loading a template's relative title-block image into a new untitled drawing.
- An isolated UI preference round-trip test for the default template.
- Headless-rendered file tabs inspected in both green/empty and red/selected LED states.
- Formatting and diff whitespace checks.

Manual Windows acceptance is required for command prompts, panel layout, template
file dialogs and restart persistence. Full workspace CI is not claimed for this
development snapshot.
