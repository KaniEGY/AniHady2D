# Rust Drawing Applications Review — 2026-11-19

This document records the source review used to guide Stick improvements.
The goal is clean-room architectural learning: understand proven behaviors,
then implement Stick-native solutions that fit our egui/eframe project model.

## License rule used for this review

- **Permissive references** may inform implementation directly when their
  license obligations are respected: Graphite (Apache-2.0), JFPainter (MIT),
  rustcreate (MIT/Apache-2.0), TraceDraw (MIT/Apache-2.0), KiPaint
  (Apache-2.0, mostly Graphite-derived at this stage).
- **GPL references are behavior/architecture references only** for Stick:
  Rnote, rusty-painter and Umber. No GPL implementation code is copied into
  Stick by this review.

## 1. Rnote — flxzt/rnote — GPL-3.0

Reviewed:
- `crates/rnote-ui/src/canvas/input.rs`
- `crates/rnote-engine/src/pens/pensconfig/brushconfig.rs`
- rendering/composition paths

Useful findings:
- Process historical pointer samples, not only the final cursor position of a UI frame.
- Keep stylus and mouse sessions distinct so emulated mouse motion cannot steal a pen stroke.
- Apply each historical pressure sample to its own point.
- Backlog processing should have a policy/limit instead of allowing unlimited delayed work.
- Textured brush randomness is seeded so saved/replayed strokes are deterministic.

Stick decision:
- Adopt the behavioral idea of bounded intra-frame pointer backlog processing.
- Keep Stick's own pressure/event implementation and project format.

## 2. Graphite — GraphiteEditor/Graphite — Apache-2.0

Reviewed:
- `editor/src/messages/tool/tool_messages/brush_tool.rs`
- `node-graph/libraries/brush-types/src/lib.rs`
- `node-graph/libraries/brush-types/src/cache.rs`
- `node-graph/nodes/brush/src/basic_brush/render.rs`
- `node-graph/nodes/brush/src/basic_brush/kernel.rs`

Useful findings:
- Brush strokes store position, pressure, tilt, twist and time as sample channels.
- Tiny nearby samples can be merged to avoid redundant work.
- Active-stroke rendering keeps pending state and extends only the new prefix.
- Brush render caches are bounded by view/age.
- Expensive hardness kernels are precomputed and cached.

Stick decision:
- Keep Stick's simpler stroke schema for compatibility.
- Adopt bounded sampling and preserve room for future timed/tilt channels.
- Do not replace Stick's renderer with Graphite's node/GPU pipeline in a maintenance update.

## 3. JFPainter — JFLibrary/JFPainter — MIT

Reviewed:
- `license.txt`
- `src/tools/brush.rs`
- `src/tools/brushes/mod.rs`
- `src/tools/brushes/custom.rs`
- `src/tools/brushes/presets.rs`

Useful findings:
- Every move sample received in a frame is passed through the brush engine.
- Every sample can carry its own pressure.
- Dab spacing remainder is carried across input segments.
- A stroke has a dirty region and one history transaction from press to release.
- Custom tips have bounded dynamics and validated import ranges.

Stick decision:
- Adopt all-move-samples-per-frame behavior with a hard safety cap.
- Keep Stick's existing one-stroke/one-undo behavior and procedural brush presets.

## 4. rusty-painter — Vectra777/rusty-painter — GPL-3.0

Reviewed:
- `src/brush_engine/stroke.rs`
- `src/app/view/render/mod.rs`
- tablet input paths
- `docs/src/dev/performance.md`

Useful findings:
- Stroke state tracks dirty tiles and drains only newly changed tiles per frame.
- Layers below the active paint layer can be cached during a stroke.
- Every batched tablet sample is consumed instead of dropping samples.
- Lift noise/near-zero final pressure is explicitly filtered.
- Performance tests measure brush lag at a real pen-like sample cadence.

Stick decision:
- Adopt lift-safety and bounded input backlog behavior independently.
- Tiled compositor/dirty-tile storage is a larger architectural migration and is
  deferred until Stick can change its raster/project model without breaking files.

## 5. Umber — Spillebulle/Umber — GPL-3.0

Reviewed:
- `crates/umber-core/src/input.rs`
- `crates/umber-core/src/stroke.rs`
- tile/render architecture

Useful findings:
- Pressure source is resolved per stroke.
- Once a real pressure sample proves the current contact is a pen, a later
  missing pressure reading is treated as lift/zero, not as a full-pressure mouse.
- The pen/mouse decision resets for each new stroke so a later mouse still works.
- Delayed timed dabs have a hard per-sample cap to prevent a stalled frame from
  creating thousands of catch-up stamps.
- Stroke coverage is separated from final opacity to avoid overlap darkening.

Stick decision:
- Adopt the per-contact pressure latch and zero-on-lift behavior independently.
- Bound queued pointer samples per frame.
- Keep Stick's existing brush compositing model for this release.

## 6. rustcreate — Juanestban/rustcreate — MIT / Apache-2.0

Reviewed:
- `crates/core/src/brush.rs`
- `crates/app/src/canvas.rs`
- `crates/render/src/lib.rs`
- `docs/adr/0004-pen-input-windows-ink.md`
- `docs/adr/0005-tiled-rgba8-canvas.md`

Useful findings:
- Adaptive motion filtering removes slow jitter while allowing fast motion through.
- A pulled-string stabilizer and StreamLine can be combined.
- Stroke finish catches the filtered path up to the real pointer endpoint.
- Raster layers use lazy tiles and history snapshots only touched tiles.
- Changed tiles alone are uploaded to the GPU.

Stick decision:
- Stick already has adaptive stabilizing and pointer-up catch-up; retain it.
- Make catch-up pressure-aware so pen lift cannot synthesize a full-width tail.
- Tiled storage is deferred as a schema/render architecture project.

## 7. TraceDraw — Project-Special/tracedraw — MIT / Apache-2.0

Reviewed:
- `docs/architecture.md`
- `apps/tracedraw/src/canvas.rs`
- `apps/tracedraw/src/brushes.rs`
- `crates/render/src/image_cache.rs`

Useful findings:
- UI, document model and renderer are strictly separated.
- Vector hit tests use the visibly flattened path rather than only raw control chords.
- A whole continuous brush gesture remains one undo operation.
- Render results and decoded images use bounded caches.
- Time deltas used by held tools are clamped before they affect geometry.

Stick decision:
- Stick already adopted visible-curve vector hit testing in Build 293.
- Keep one undo snapshot per gesture.
- Consider a bounded media decode/render cache as a later performance task.

## 8. KiPaint — mel0k1/KiPaint — Apache-2.0

Reviewed:
- README/current project status
- input preprocessor
- brush/cache code inherited from Graphite

Useful findings:
- Centralized pointer/key preprocessing prevents impossible key-up/down states.
- Pointer interaction time is normalized relative to pointer-down.
- Current functional brush/render core is primarily the Graphite base; many
  painting-specific KiPaint features are still roadmap items.

Stick decision:
- Use the input-preprocessor concept as a design direction, but avoid a wholesale
  event-system rewrite in this compatibility-focused update.

## Changes selected for the immediate Stick update

1. **Bounded intra-frame pointer backlog**
   - Consume real `PointerMoved` events in order during Brush and Vector Pen strokes.
   - Ignore hover history before the primary press.
   - Deduplicate almost-identical points.
   - Cap each UI frame at 96 samples while preserving the path's first and last sample.

2. **Pressure-capable contact latch**
   - Once a contact reports real pressure, a missing/zero pressure sample during that
     same contact is treated as pen lift, not a mouse at 100% pressure.
   - The latch resets for the next contact, so mouse drawing remains full pressure.

3. **Pressure-aware stroke finish**
   - Do not synthesize a pointer-up endpoint when the pressure-capable contact ended
     at zero. This prevents an artificial thick tail/blob after stylus lift.

4. **Regression tests**
   - Hover events before press cannot enter the stroke.
   - Intra-frame pointer samples preserve order.
   - Backlog is bounded and still preserves endpoints.
   - Invalid/duplicate samples are discarded.
   - Missing pressure after a real pen sample becomes zero.
   - A later mouse contact returns to normal full pressure.

## Deferred architectural work

The strongest long-term performance theme across rusty-painter, Umber and
rustcreate is tiled raster storage + touched-tile undo + changed-tile GPU upload.
That can materially improve very large canvases, but it should be a dedicated
migration with project compatibility tests rather than being mixed into an
input-quality hotfix.


## Brush correction pass — video review (2026-11-19)

The uploaded brush test video exposed a concrete rendering defect: **Marker**
showed each rectangular texture stamp as a separate block instead of reading as
one marker stroke. The other textured media also showed too much stamp cadence
at large sizes.

Additional source comparison for this pass:

- **Rnote** models Marker separately from Textured and Solid styles. Its default
  marker uses a smooth style and constant pressure rather than a repeated
  pressure-sized texture nib. Stick now follows that behavioral split: Marker is
  rendered as one continuous translucent stroke, not a stack of rectangular
  texture quads.
- **rustcreate** exposes brush spacing as a fraction of diameter; its reference
  presets use dense values such as 0.08 ink, 0.10 airbrush and 0.15 pencil.
  Stick's old textured path was effectively around 0.24 diameter. Textured
  Stick media now use denser per-preset spacing roughly in the 0.09–0.14
  diameter range while retaining the existing hard 20k-dab safety budget.
- **JFPainter** processes all input samples through a brush engine that owns
  width/hardness/spacing and accumulates stroke coverage before commit. Stick
  keeps its simpler project format, but now uses the same principle that
  spacing belongs to the brush family rather than one universal constant.
- **Graphite** bounds sample spacing from brush footprint and keeps an
  incremental stroke walk. Stick keeps its current renderer but smooths the
  travel direction used by grain stamps so pencil/chalk/dry media do not rotate
  abruptly from one noisy sample to the next.

Implementation selected for Stick:
1. Marker leaves the texture-stamp path and becomes a single continuous,
   constant-width translucent line.
2. Watercolor, Airbrush, Sketch, Graphite, Dry, Chalk, Charcoal, Pastel,
   Calligraphy and Soft Sketch receive denser per-preset dab spacing.
3. Direction-bearing grain brushes get short direction memory to reduce blocky
   stamp rotation while preserving texture variation.
4. Vector Pen is intentionally unchanged.
5. The changes remain preview/export shared so the canvas and PNG/video output
   continue to agree.
