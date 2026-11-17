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
