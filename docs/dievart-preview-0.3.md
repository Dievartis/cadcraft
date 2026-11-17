# Dievart preview 0.3

Personal fork preview based on CADCraft 0.20.1; not an upstream release.

## Included

- Dockable Image Manager with raster-specific file filters, search, reload and detach.
- Placement editing, absolute rotation and interior image picking, including rotated images.
- Eight-pixel inner spacing in the Image Manager.
- Loaded images embedded in PDF output, with image and viewport clip outlines.
- Concurrent image decoding budget adapted from @dotCooCoo, upstream PR #3151.
- Previous dockable managers and font-family integration remain on this branch.

## Known limitations

- Image attachment still defaults to one drawing unit wide. DPI-based physical sizing,
  an explicit aspect-ratio lock and additional snap points are pending.
- Brightness, contrast and transparency controls edit stored entity properties; their
  visual effect is not validated for this preview. Do not advertise them as complete.
- Embedded image storage in DWG/DXF and the separate #1565 clipping module are not included.
- This tag contains source code; a Windows executable must be built separately.

## Validation

- Native application cargo check passed before the final spacing change.
- Engine and UI cargo check passed after the final spacing and selection changes.
- Image rotation and invalid-value regression test passed.
- Both decoding-budget tests passed.
- Rotated image interior-picking regression test passed.
- Full workspace CI and manual Windows acceptance of the final snapshot remain pending.

Contribution tracking and pending notifications: [integration notes](dievartis-integration.md).

## Follow-up after the 0.3 tag

The integration branch now adds manual DPI sizing before attachment (default 96 DPI),
with explicit millimetres per drawing unit (1 for mm, 25.4 for inches, 1000 for metres).
Automatic file resolution is enabled by default in the manager (PNG pHYs, JPEG
JFIF/EXIF, BMP pixels-per-metre and TIFF resolution tags). Invalid or absent metadata
uses the manual fallback. Disable "Read resolution from file" for a manual override.
The horizontal density determines scale to preserve pixel aspect ratio; separate X/Y
physical scaling is not applied. The console reports the chosen DPI and its source.
JSON IMAGEATTACH accepts `dpi`, `autoDpi` and `unitMm`
instead of `width`/`scale`; legacy calls keep their existing sizing defaults.
Placement editing has an aspect-ratio lock enabled by default. Image corners, edge
midpoints and center provide END, MID and CEN object snaps, including after rotation.

### Visual adjustments and grips follow-up

Brightness and contrast now transform rendered pixels while preserving source pixels
and alpha; the adjusted raster is shared by canvas, PNG and PDF rendering. Defaults
50/50 are unchanged. A weak cache avoids keeping adjusted buffers after their display
lists are dropped. Manual visual acceptance remains pending.

Selected images expose nine visible grips: proportional corner resizing about the
opposite corner, independent edge resizing from side midpoints, and center translation.
These grips are separate from the END/MID/CEN object-snap points described above.
