# Stick Studio — 13-tool engineering audit (17 Nov 2026)

**Scope:** Existing 13-item drawing toolbar, live canvas, region fills, exact hit geometry and save/export parity. This does not substitute for using a physical pen tablet or Windows GUI acceptance testing.

## Open-source references

- AniCraft `crates/engine/src/cmds_paint.rs`: bounded raster coverage, gap-thickened walls, connected-component fill, growth under ink before contour tracing. Stick implements analogous concepts independently in its existing data model.
- AniCraft `src/erasertool.rs` and `crates/engine/src/cmds_erase.rs`: gesture-based erase with precise swept geometry instead of per-point-only selection.
- AniCraft `src/pathtools.rs` and `crates/engine/src/selection.rs`: screen-space hit tolerance and explicit geometry editing.
- Krita offers separate fill reference/gap/selection behavior. No GPL code or assets copied from Krita. No AniCraft source imported.

## Source audit table

| Tool | Code reviewed / change | Automated checks | Further Windows UI QA |
| --- | --- | --- | --- |
| Standard selection | **Fixed** bounding-box false-positive hits; exact stroke-segment and fill-region detection | `select_click_rejects_diagonal_line_bounding_box_false_positive` | Clicking stroke under rotated/scaled canvas |
| Lasso | Reviewed edge-crossing geometry; retained existing selection semantics | Existing lasso boundary/crossing tests | Multi-stroke lasso + move |
| Free Transform | Reviewed four resize handles / rotate hitboxes from previous update; unchanged project representation | Existing transform and zoom tests | Mouse and pen dragging each handle |
| Vector nodes | Reviewed vertex hit radius and drag paths | Existing nearest-node tests | Dragging dense vector nodes |
| Vector ink pen | Reviewed stroke commit and bezier fitting paths | Shape and geometry tests | Stylus precision and zero-length strokes |
| Brush presets | **Fixed** pointer release truncation following stabilization; renderer now tessellates filled concave shapes | Existing pressure dab/brush-tip tests; new polygon tests | Rapid scribble, loop closure, export parity |
| Full eraser | **Fixed** lines that crossed cursor between two far-away vertices; now detects segment intersection and sweeps drag path | `complete_eraser_hits_long_segment_without_nearby_vertices` | Erase with fast mouse motion |
| Partial eraser | **Fixed** segment-circle clipping into two fragments (instead of point filtering), improved quick-drag coverage | `partial_eraser_splits_line_at_disc_boundaries` | Dotted/stylus lines and precise corners |
| Bucket | **Fixed** concave fill rendering, white hairlines, clicks near ink and local crossing loops; **added** adjustable small-gap closure and reference visible layers | Half-circle, concave-notch mesh, white-halo, near-wall, doodle lobe, gap cases | User's actual red circles/black loops; fill exported image and save/reload |
| Eyedropper | **Fixed** picking along entire line and interior of fills; samples frontmost visible, non-guide layer | `eyedropper_uses_segments_and_opaque_fill_colors` | Pick overlapping layer colors and alpha |
| Text | Reviewed edit and selectable text bounding region | Existing Arabic multiline and text bounds tests | Arabic shaping after save/export |
| Shapes/lines | **Fixed** collapsed square if dragged perfectly vertically; validate line/rectangle/circle/triangle/pentagon | `geometric_tools_generate_finite_closed_shapes`, `squares_have_real_area_even_for_purely_vertical_drag` | Fast drags, mouse release outside canvas |
| Hand/canvas | Reviewed zoom-anchored pan, rotate and flip mapping | Existing cursor-anchored zoom test | Cursor location after multiple flips and 180° rotation |

## Bucket design details

- Rasterize visible linework into bounded wall occupancy (maximum grid 2 million cells).
- Ignore fill-only contours as walls when scanning. Use a configurable gap closure radius.
- When clicking on a thick outline, search only nearby transparent components and reject every region connected to the canvas edge.
- Grow the bounded selected region *into its existing wall cells only* to cover anti-aliasing seams beneath linework without crossing dividers.
- Trace region to polygon and tessellate its real (often concave) shape via Earcut. The export engine already uses path filling, so the live preview should now match.
- Allow painting on a new layer from visible other-layer contours via opt-in `Reference all visible layers`; default remains original active-layer mode.
- Repeated bucket clicks place the new filled region above older fill-only elements but underneath linework, so changing the fill color is visible.

## Open limitations

- A different source layer/transform can require a broader per-layer compositing model, which this targeted fix does not replace.
- The fill geometry uses a single outer contour; multi-hole, self-overlapping artistic regions need further native face decomposition and advanced fill rules.
- Erasing directly inside a standalone fill region removes the entire fill element; truly painting transparent partial holes needs multi-contour fill support.
- This is not an imported Krita raster paint engine, and not every AniCraft feature belongs in Stick.
- **Before claiming a fully verified update:** Windows CI tests and release build must pass at latest HEAD; manually verify drawing and bucket against the user screenshot, test all 13 toolbar tools, save/reopen old project, PNG/video export, Undo and Online edits on 2 PCs. Never publish a failing build.
