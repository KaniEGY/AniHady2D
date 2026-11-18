# Stick Studio — Performance Review (19 Nov 2026)

This review targets interactive latency in the drawing canvas and animation timeline after the recent feature expansion.

## Hot paths reviewed

### 1. Sculpt pointer loop
Previously every pointer move could rebuild the point vector of every nearby stroke, and filled strokes were skipped entirely. That made long sculpt sessions increasingly expensive while also preventing Bucket fills from deforming.

Changes:
- Fast bounding-box rejection before point-level sculpt work.
- Local densification runs only when a nearby segment is still too sparse.
- Filled contours are sculptable.
- Closed vector/fill seams are re-closed after deformation.
- Point-growth limits remain bounded.

### 2. Frame-by-frame audio scrub
Previously every frame step spawned a new OS thread, opened a new audio output stream, and played a very short snippet. Rapid frame stepping could leave multiple overlapping snippets/threads and make speech hard to understand.

Changes:
- One reusable scrub OutputStream.
- Previous scrub sinks are stopped before the next frame preview.
- Lip-sync preview uses a tested 220–340 ms context window with short pre-roll.
- Full playback still uses its separate synchronized sinks.

### 3. Canvas popup / cursor routing
The custom drawing cursor used only the canvas rectangle, so floating windows or menus physically over the canvas could inherit a hidden/crosshair cursor and allow background tool interaction.

Changes:
- Canvas input now requires the CentralPanel to be the topmost egui layer at the pointer.
- Floating windows, popup menus and feedback/settings restore the normal OS cursor.
- Feedback dialog is included in the modal interaction guard.

### 4. Canvas quick menus
Shape/color radial menus and Alt/Ctrl adjustment feedback are isolated in `canvas_quick_menu.rs` instead of expanding the main drawing state machine.

### 5. Vector Pen smoothing
Freehand vector input gets a bounded two-pass control-node smoothing step before node reduction. Hard ~90°+ corners remain anchored while high-frequency tablet/mouse zig-zag is reduced. Render-time smoothing still preserves editable control points.

## Guardrails kept
- No project-schema bump.
- No unbounded frame/media texture cache.
- Existing Undo snapshots remain operation-level rather than pointer-sample-level.
- Sculpt and vector point counts keep hard caps.
- Timeline relocation still validates collisions/locks/bounds atomically before committing.

## Manual QA still useful
- Very large projects with thousands of filled contours.
- Real tablet input at high event rates.
- Multiple simultaneous long audio tracks during frame stepping.
- Different Windows DPI/scaling values for radial menus and custom cursor feedback.
