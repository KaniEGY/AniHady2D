# Stick Studio — Session Handoff / سجل استكمال المشروع

## 18 November 2026 — screen-recording QA, smooth audio dragging and invite joins
- Source screen recording: silent MP4, 1920x1080, 60fps, approximately 9m47s. See `VIDEO_QA_2026-11-18.md` for timestamped observations, fixed issues and remaining QA.
- PR #46 on `fix/video-qa-audio-scrub-invites-20261118` captures audio clip from initial pointer press, disables timeline's drag-to-scroll conflict, saves a single Undo state per actual audio move, and implements continuous ruler mouse-held frame scrubbing.
- Unrotated paper now clips artwork/ghost preview at stage bounds; egui rectangular clipping isn't a full solution for rotated paper.
- New `src/online_invite.rs` makes password-free copy/paste `stick://join` invitations with strict address/code/mode validation. A connected host can supply reachable LAN, private overlay or public IPv4/DNS endpoint. DNS resolution occurs in background online worker; optional ZeroTier setup is documented alongside Tailscale.
- **Do not claim the invite links globally discover rooms or cross CGNAT:** there is no deployed internet relay, no TLS in direct Stick TCP and no actual two-PC external test. Password never goes inside the invitation.
- The customer-visible fill settings use Stick-specific wording, with no mention of unrelated products. Internal historic Rust enum naming is left alone for compatibility.
- Require Windows CI test, Clippy, release build and release asset verification before publishing; successful CI is not manual brush/GUI or multi-PC acceptance.

## 18 November 2026 — Work overhaul integrated review
- PRs #40 (vector geometry), #41 (canvas appearance/sizes), #42 (pressure stroke smoothing), #43 (Advanced Timeline), #44 (five new brush presets) were merged in sequence into `integration/work-overhaul-20261118` using preserved source changes and explicitly resolved shared documentation / main.rs differences.
- The build 213 released main was unchanged while staging. After one combined Windows cargo test/Clippy/release build and review, merge the integration PR to main and require the main push release workflow to succeed. Verify `Stick.exe`, `version.txt` and `SHA256SUMS.txt`.
- **No backend relay or internet-wide collaboration replacement shipped in this batch.** Online external networking remains unverified.
- Physical pen, multiple Windows machines, long video/audio files, live UI interaction and visually comparing SVG/PNG exports remain separate manual QA, not claimed as done.
- Later Work/Codex tasks should check current GitHub main head and release first, not reimplement already-merged stage code.

## 18 November 2026 — five new functional drawing presets (PR pending)

- Feature branch `feature/new-real-brush-presets-20261118` from main build 213.
- Appended 5 `KritaBrushPreset` enum variants after the original nine to preserve legacy enum order and maintain .stick schema compatibility. Added actual bilingual UI icons.
- **Fineliner:** fixed geometric width independent of sampled stylus pressure; smoothed centerline shared by egui and raster/video export.
- **Smooth Ink:** cross-track micro-jitter reduction at gentle bends and pressure-aware continuous outline; intentional sharp turns maintained.
- **Tapered Pen:** travel-distance-based smoothly easing tip width at both ends while retaining real pressure through the main stroke.
- **Calligraphy:** fixed-angle 30-degree chisel texture stamp, independent of draw direction, with shared procedural preview/export alpha mask.
- **Soft Sketch Pencil:** distinct lightweight textured/grainy mask and semi-transparent pressure-sensitive stamping.
- Added brush geometry and texture mask tests. No user project files were edited or deleted. Existing 9 presets remain.
- **Pending:** GitHub CI, physical pen/mouse and PNG/video comparison, texture visual QA. Hardness/pressure-curve/spacing sliders remain future work; do not misrepresent them as implemented.

## 18 November 2026 — Advanced Timeline core (PR pending)

- Feature branch `feature/advanced-timeline-keyframe-edit-20261118`, based on main build 213.
- `src/advanced_timeline.rs`: atomic group move/copy of authored keyframes with layer/frame bounds, locked-layer and collision validation. The operation does not modify audio and the editor wraps successful mutations in existing Undo snapshots.
- Timeline Settings now exposes an **Advanced Timeline — التايم لاين المتقدم** selection alongside OLD and NEW. It renders a distinct timecode ruler/exposure tracks, Ctrl/Shift-click multi-keyframe selection, drag move, Alt-drag duplicate, and +1 frame batch buttons.
- Existing video/image layer cels and sampled waveform audio remain on the same shared timeline, and all three modes read/write the existing project schema. New editor selection state is transient.
- **Limits:** this first core slice does not yet implement non-destructive audio trim, video thumbnails, multi-cell clipboard paste, exposure edge stretching, advanced snap magnets or panel panning. Manual GUI drag acceptance and Windows CI remain outstanding; do not describe it as feature-complete.

## 18 November 2026 — pressure stroke stabilizer/resampler (PR pending)

- Branch `feature/pressure-stroke-quality-20261118` from main build 213.
- Reworked `src/stroke_geometry.rs` G-Pen pressure outline with bounded cross-track jitter reduction on gentle lines, path-distance resampling (24k sample limit), pressure interpolation, short miter limits at sharp corners and consistent round end caps.
- Existing `brush_engine::visit_round_marks` shares this outline in egui preview and tiny-skia raster/video export. No .stick schema change, no alteration to stored points or reported tablet pressure.
- Unit tests cover long fast lines, intentional corners, pressure width, duplicate/invalid points, circular paths and memory budgets.
- **Not accepted yet:** Windows tests/Clippy/release build, real pen hardware, zoom/rotation, translucent anti-alias visual parity and long stroke performance should be checked before merging.

## 18 November 2026 — Canvas Appearance and Canvas Size Mode (PR pending)

- Branch: `feature/canvas-appearance-sizes-20261118` from main build 213 (`fa5136795b6e6f14802ae2afeb661a2aba753dc9`).
- New `src/canvas_appearance.rs`: display-only configurable border color/width, rounded corner mask, optional shadow, workspace color. Settings are user-local AppConfig with serde defaults (old settings still load), never .stick file content or exporter decorations.
- General settings now include a bilingual Canvas Appearance section, live sample and reset button.
- Canvas Size Mode includes named YouTube Thumbnail/HD/Full HD presets, custom ratio lock, and confirmation on shrinking the stage; the existing Undo-backed size action is reused. Original legacy presets remain offered.
- Old artwork is not rescaled or erased when changing canvas bounds. Old .stick schema remains unchanged.
- Requires Windows CI, real GUI inspection, rotation/zoom display checks and actual file preservation/export QA before release; no verification of those physical checks is asserted here.

## 18 November 2026 — adaptive vector interpolation (PR pending)

- Feature branch: `feature/vector-geometry-quality-20261118`, based on main SHA `fa5136795b6e6f14802ae2afeb661a2aba753dc9` (Windows build 213).
- Changed `src/vector_path.rs`: adaptive subdivision (3–24 samples/segment), chord-weighted Hermite tangents, hard-corner preservation, duplicate point filtering, cyclic closed-path interpolation and safer square-cap extension.
- Non-destructive geometry only: the .stick format and stored editable nodes are untouched. The existing preview and raster/video export call this shared path function.
- Added unit tests for straight strokes, corners, closed loops, duplicates, invalid input, adaptive sampling and cap behavior.
- The PR must pass GitHub Windows tests/Clippy/release build before merge. Physical pen, 4K/long-session performance, preview/export image diffs and Windows visual inspection are **not yet validated**. Do not claim a published build based on this branch.

## 18 November 2026 — imported media, audio timeline and bounded fill controls

- Branch: `feature/media-audio-fill-studio-20261118`. This entry records work in progress until the Windows CI run passes and a merge/release is confirmed.
- `MediaItem` gained optional `rotation_deg`, `flip_h` and `flip_v` fields with serde defaults. Imported stills keep aspect ratio, and imported video-frame layers propagate the chosen geometry across keyframes. Stage preview and raster export apply transforms. Source video is still decoded to external image files; the editor does not embed source media within .stick.
- `AudioTrack` gained defaulted `gain` and `muted`. Audio strips are visible in the shared timeline and draggable, with waveform previews, right-dock controls and delayed multi-track playback. Waveform extraction remains sampled (first-minute preview); QA for long MP3s, move/undo, multiple clips and alignment is still needed.
- Bounded bucket fill supports optional precision, positive/negative Dilate/Contract and temporary boundary thickness settings. This is Stick's own vector-contour solver, not Blender's exact raster/vector algorithm. Verify filled regions and undo on actual drawings.
- Texture cache evicts non-current video frames to bound GPU memory. Real hardware performance and imported media duration/file availability must be smoke-tested.
- Before publishing, require `cargo test`, `cargo clippy`, Windows `cargo build --release`, main release `Stick.exe`/version/SHA256 and a physical GUI check where available. Don't claim external media and audio are embedded in cloud backups.


## 18 November 2026 — timeline layer icons, canvas sizes, smoother vector and help
- Branch: `feature/ui-layer-vector-canvas-online-help-20261118` (base: `5b4c996ccdc169ac751c8c3d805c7c6864323b01`, release `v4.1.0.207`).
- New `src/vector_path.rs` smoothed vector render/export geometry preserves editable nodes and reuses the already-serialized `Stroke.tip_shape` for round/square end caps (default Round). `src/brushes.rs`, `src/exporter.rs`, `src/brush_engine.rs` use consistent stroke geometry; old projects remain readable.
- New `src/canvas_sizes.rs` preset and custom stage size validation caps at 16 MP to limit memory use. Stage resize does not rescale existing art. UI updates dimensions with Undo and keeps draft width/height in sync with project open/undo.
- Bilingual modal guides and conspicuous launcher/online help controls, painted lock/eye icons, timeline label context actions, and Charcoal/Soft Pastel brushes in the existing texture brush pipeline.
- REQUIRE GitHub Windows tests, `cargo clippy`, release build and packaging before merge. Manual GUI/tablet and real two-computer online verification remain pending. Do not claim real-device success based on CI.



## 18 November 2026 — brushes, cursors, timeline and tool audit

- Base `9e1230e602fa7a6ba35259bb5bfee3a220299f8a`, published tag `v4.1.0.203`, development branch `codex/studio-development-audit`.
- DryBrush and Chalk use procedural tips shared by live and raster rendering. Project schema remains 5. Rooms now negotiate protocol 5, and cursor state stays in memory.
- Both timeline layouts share layer-local ripple insertion, cel duplication and undo/redo. Opening, restoring or importing a whole document starts a new undo history.
- Local Windows baseline Cargo run stopped during dependency compilation because MSVC link.exe was unavailable. Existing files also have rustfmt differences. Check the GitHub Windows build for this branch before release. Physical tablet, GPU preview and two-machine room acceptance still require hands-on testing.

> Continuous history, not proof of acceptance. The Git history and Windows workflow results are authoritative. This entry records work started on 17 November 2026.

## Source and branch
- Repository: https://github.com/KaniEGY/Stick
- Original stable main SHA: `41134a99b32b85c235c883a856673aa430ee41ac`
- Feature branch: `feature/stick-art-workspace-fixes-20261117`
- Pull Request: https://github.com/KaniEGY/Stick/pull/19
- Initial code changes started against this exact main SHA; check newest remote SHA before future edits.
- App version in Cargo.toml: `4.1.0` at the beginning of the session. Project schema: `5`. Existing project schema number deliberately unchanged; `Stroke.tip_shape` uses a serde default for old files.

## Source code changed
- `src/main.rs`: bilingual anime-inspired **Art System / نظام الفنان** editor name/avatar panel (XP formula and rank thresholds no longer displayed); mouse-anchored zoom; canvas paper + bitmap rotation, secondary-mouse drag on Hand tool; lasso selection movement; rotation handle on Free Transform; colored lightweight cursors; right-click tool options and new triangle/pentagon geometry; workspace personalization toggles; layer rows and timeline visuals; multiline Arabic text editor preview; app settings saved in user-writable LOCALAPPDATA with executable-adjacent legacy read fallback.
- `src/selection_tools.rs`: lasso intersects crossing stroke segments and not just enclosed vertices; rotation geometry and pointer-anchored zoom functions + tests.
- `src/fill_tools.rs`: larger tolerance for near-closed paths and a regression test. **Not a general raster flood fill engine.**
- `src/project.rs`, `src/brush_engine.rs`, `src/brushes.rs`, `src/exporter.rs`, `src/main.rs`: six `BrushTipShape` types (round, square, triangle, pentagon, tall rectangle, wide rectangle), applied consistently in live preview and export; backward-compatible serde field and roundtrip tests. Text export uses a Windows local font and fontdue.
- `src/persistence/mod.rs`, `src/timeline_ops.rs`: old test constructors updated to initialize the added brush tip field; no project data dropped.
- `src/profile.rs`: profile persisted in per-user writable folder, with old file load fallback.
- `Cargo.toml`: fontdue for text rasterization. `RELEASE_NOTES.md`, `src/changelog/mod.rs`: refreshed bilingual release notes and first-run What's New parsing.

## Required QA before publication
1. Check latest PR workflow: https://github.com/KaniEGY/Stick/actions . A successful run must include **Run Compatibility & Unit Tests**, **Build Release EXE**, **Package Clean Folder**, and **Upload Artifact**.
2. Earlier feature commits had successful Windows tests/build. The first brush-shape commit failed due to two omitted test constructors and was corrected in subsequent commits. **Do not treat old successes as validation of newest HEAD.**
3. Run a manual Windows GUI smoke test against the actual packaged EXE when possible: select/drag/resize/rotate strokes, lasso crossing, fill near-closed loop, open legacy project, save/reopen, multi-line Arabic editor and export, try multiple avatar formats, customize/restart panels, rotate/flip bitmap and paper, compare exported colors and frame sequences.
4. Ensure latest PR is green before merging. Release and updater files are generated from merged `main`, not from a stand-alone patch EXE. Verify main push workflow and GitHub Releases page after merge.

## Known remaining gaps
- Bucket fills recognized closed stroke contours; **raster flood fill across disconnected raster strokes is not implemented**.
- Full Krita-grade raster brush engine and AniCraft adapter are exploratory; **no GPL Krita code was copied** into Stick. Any such import requires deliberate GPL licensing decisions.
- Text preview on rotated/flipped canvas may not yet transform glyph orientation identically to bitmap media; independent render/export matching needs testing.
- Anti-cheat: XP formulas hidden in the UI/release notes, but a fully local open-source executable cannot keep award mechanics secret or cheat-proof without a trusted service.
- Full Windows interactive QA was not available in this connector session; do not claim GUI acceptance solely from Cargo CI.

## Architecture rules
- Drawing Mode and Animation Mode share one document model. No rebuilding from scratch.
- Every additional stored field needs backwards-compatibility tests, and future schema changes need migrations.
- Release only a complete build after validation; never ship a binary-only hotfix as if it were a new version.
- Update this handoff with new head SHA, CI URLs, test outcomes and outstanding work at each follow-up.


## 17 November 2026 — Online collaboration preview PR #20
- Base main SHA: de373bd262ba2179e322f681775fa4a4ac0d08ee
- Branch: feature/online-lan-rooms-20261117-v2
- PR: https://github.com/KaniEGY/Stick/pull/20
- Added src/collab.rs and integrated online lobby/room header in src/main.rs. Standalone onboarding in STICK_ONLINE_GUIDE.md.
- LAN discovery via UDP 47553, host TCP 47554, optional password using per-connection nonce challenge, capacity 2–12 participants including host, join by LAN discovery or direct IPv4.
- Host-authoritative newly committed stroke broadcasting and initial project snapshot; local unsaved projects are backed up before an incoming snapshot. Absolute media/audio filesystem paths are stripped from outbound project snapshots.
- **Not yet fully collaborative animation editing:** layer/timeline/move/undo/text/media operations remain unsynchronized, and full host resync is explicit. No global Internet room directory and no end-to-end transport encryption; no CGNAT NAT traversal.
- Initial PR test workflow (c511c8d) failed because MutexGuard PoisonError was passed to io::Error::other; corrected in 3aca0ff. Subsequent Windows unit tests completed successfully, while release-build and newest-HEAD workflows still require checking. Windows two-PC GUI QA has NOT been performed.
- Strict release criteria: latest HEAD Windows CI must finish green; test local and second-PC join/packet behavior before claiming the feature is stable. Keep normal Drawing/Animation and save formats unchanged. Do not call an early successful build proof of manual online acceptance.
- Next task: verify the latest head workflow, then review unsynced structure operations, secure transport and optional Internet discovery/relay architecture.


## 17 November 2026 — Automatic publication, rollback and Layers dock
- User explicitly requests publication of EVERY tested update through GitHub Releases and Stick updater. Do not hold working builds or replace full versions with repair-only files. Preserve historical releases for rollback and never publish a known failing build.
- Online beta PR #20 merged to main at commit 5afb33b2b2ece1e5ccb44e100dde2395608198fb, following latest-HEAD passing Windows CI. Confirm main publication run separately.
- Next branch: feature/release-rollback-layers-20261117
- src/updater.rs: query GitHub Releases, sort by published_at, verify official asset URLs, SHA-256 digest and sizes, offer up to 5 older builds. Reuse existing safe staged installer. Regression tests added.
- src/main.rs: settings Previous Versions tab with immediate predecessor, 5 older releases, dates, confirmation and unsaved work guard; improved resizable right Layers dock with search, sampled actual thumbnail preview, per-layer metadata, double-click rename and guide action.
- RELEASE_NOTES.md bilingual update. No GPL Krita source code imported.
- QA: latest PR Windows cargo test and release build must be green before merge; Windows manual rollback, export, reopening older projects and live layer dock validation remain unverified. Online beta is NOT secure/encrypted Internet collaboration, only limited LAN/direct-IP stroke synchronization.

## 17 Nov 2026 — New online/bucket/perspective UI sprint
- Branch: feature/online-bucket-perspective-ui-20261117, based on main babfb56cc54f3da2c4dc09cf728ab9ef1873fcc7.
- Fixed rollback duplicated immediate prior release; parse 6 candidates, previous first + 5 older in list.
- Added project-independent non-destructive perspective guide overlays, configurable stage-aware square, lines and one-point perspective; timeline height config with manual slider and drag-memory; adjustable persistent properties/layers split and collapsible Colors/Symmetry sections.
- Rebuilt bucket fallback with bounded wall mask, flood-region trace to vector fill for split freehand contours; tests for closed vs open.
- Online protocol v2: local room code 8 digits, room roster with colored names and self-reported local levels, hosts currently open project, directed UDP broadcast. No worldwide rendezvous, automatic Google OAuth or universal concurrent operation syncing.
- New original colorful vector icons for 13 tools and game UI synthesized sounds. No third-party asset icons copied.
- DO NOT claim manual two-PC or cloud QA complete until tested, and DO NOT publish a failing Windows build. User explicitly authorized auto-publishing every passing version on main.


## 17 November 2026 — LAN document sync / bucket divider follow-up
- Starting main SHA: `41f2e98507fa3aefa3194a8ae167a8848095cdc3`.
- Branch: `fix/online-project-sync-bucket-20261117`; PR: https://github.com/KaniEGY/Stick/pull/25.
- Added `src/project_sync.rs`: granular JSON-structured edit operations for project fields, layers, frame/keyframe data, stroke insertion/removal, text, transform and settings, with merge unit tests. No saved project schema change.
- `src/collab.rs`: LAN protocol v3; guest sends edits, host validates/serializes and relays them, plus baseline management and fallback resync for rejected edits; 20k frames supported. Absolute filesystem paths are sanitized. Loopback coverage for structural changes added.
- `src/main.rs`: sends all project changes after UI update; receives remote edits; stops duplicate legacy immediate stroke messages; host role explained clearly. Bucket now tries region flood first to respect interior line boundaries.
- `src/fill_tools.rs`: regression test for circle divided by diameter and single-half fill.
- Existing AniCraft-inspired timeline controls remain in place. `src/timeline_ops.rs` now preserves exposure artwork when deleting the first frame, and the UI marks text/symbol frame content. No AniCraft code imported or animation engine replaced in this sprint. Plan targeted integration only after stable collaboration and acceptance QA.
- QA: PR Windows `cargo test` and `cargo build --release` via Actions; at time of writing, newest SHA result **not yet confirmed**. Two-PC GUI acceptance **not performed**. Do not claim this as proven in production until verified.
- Known limitations: simultaneous edits to identical array positions can conflict; external media bytes not transferred; no encryption/global Internet room discovery; no Windows executable generated locally (GitHub Actions builds it).
- Next: check latest PR Windows CI, fix any errors, merge only if green, verify main release, and test two-PC erasing/layer/frame/bucket behavior and existing project save/open.


## 17 Nov 2026 — Internet friends + transform handles
- Branch: `feature/tailnet-friends-and-transform-handles-20261117`, starting main SHA `e30433183824d7e1428665b160dd91eee7b133e5`.
- `src/remote_access.rs`: optional Tailscale CLI status JSON parsing, 100.64/10 host address discovery and bounded active peer enumeration with unit tests. Tailscale is separately installed; no API keys, accounts or firewall modifications in Stick.
- `src/collab.rs`: send existing UDP discovery request as unicast to visible Tailscale peers (up to 64), keep LAN broadcast and direct IP fallback. Better TCP refused/timeout help text.
- `src/main.rs`: show Tailscale readiness in Online lobby, copy friend-host IP, join by 100.x address, show discovered friend rooms, and explain that LAN 192.168.x is not public.
- `src/transform_handles.rs` + `src/main.rs`: all four corner resize handles with opposite anchors, constant-screen-pixel hit areas (22px radius), larger 21px rotation target and more visible handles. Unit tests added for all corners and zoom.
- `STICK_ONLINE_GUIDE.md`: complete Tailscale guide and troubleshooting, with manual two-PC QA checklist. This does not implement global room code discovery or built-in VPN/relay services.
- User wants every successfully tested version published by default through GitHub Releases and Check for Updates. DO NOT publish only if user expressly says not to; keep earlier release rollback choices and don't publish failed CI.
- CI and runtime result: to be filled from actual Windows workflow results. Windows two-PC cross-ISP GUI test is not performed here.


## 17 Nov 2026 — Tools audit, fill seams and eraser precision
- Branch: `fix/tools-bucket-eraser-selection-20261117`, based on `main` SHA `8b5c1368679ae13400ab3999937a55bd46c097a2`.
- Scope: inspect all 13 drawing toolbar tools, compare to AniCraft (engine paint, eraser, path tools and selection), independent implementation. No Krita GPL source copied.
- New `src/fill_render.rs` + `earcutr` dependency: proper concave region triangulation for live canvas, aligning with tiny-skia export path. `src/fill_tools.rs`: grow bounded flooded region under the ink to eliminate white gaps, support clicks near wall, retain divider isolation, tests for self-intersecting doodle loops, small gaps and outline underpaint.
- `src/eraser_tools.rs`: full-stroke hit against segment geometry; partial eraser analytically clips paths against eraser disc instead of only testing vertices. Main UI interpolates fast eraser drags; tests cover long line, clipping, dab and untouched strokes.
- `src/selection_tools.rs`: exact line/filled polygon picking and precise hit test (no bbox false positives); eyedropper now searches topmost visible layer. `src/main.rs`: bucket gap slider + optional all-visible-layer reference, fill-only ordering for recolor, smoothed brush pointer-up endpoint and square drag fix.
- `TOOLS_AUDIT_2026_11_17.md`: table covering all 13 tool paths, automation and Windows GUI follow-up, plus explicit known boundaries.
- User's standing release preference: publish every tested update by default, unless explicitly told not to. Always keep old releases and rollback; no known-failing build on main.
- QA: latest PR Windows cargo test/release build plus manual painting/old-project/save/export/2 PC still need verification before stating complete acceptance.


## 17 Nov 2026 — online opt-in, brush continuity, vector points and sculpt
- Branch `fix/online-vector-brush-sculpt-customization-20261117`, starting at `61f26cbe0c0c0390c08d8e85b900b2458092776c`.
- `src/main.rs`: opt-in network use in lobby (offline by default), no automatic Tailscale discovery on entering lobby, truthful diagnostic for zero permitted Tailscale devices; nonblocking exit dialog; 14-tool toolbar with new Sculpt, configurable vector decimation, selected points via Shift/box, axis translate gizmos, cursor personalization persisted with serde-compatible defaults.
- `src/stroke_geometry.rs`/`src/brushes.rs`/`src/exporter.rs`: connected pressure-aware G-Pen outline instead of translucent overlapping dabs; removed destructive 4x freehand and 8x vector interpolation at commit. PNG export uses same shape and safe long-line fallback.
- `src/sculpt_tools.rs`: local radial falloff deformation with bounded point insertion; tests; `src/transform_handles.rs`: axis arrow hitboxes and tests.
- Source references: AniCraft `src/pathtools.rs` for knot and sub-selection workflow; `src/erasertool.rs` for gesture-based edits; Blender 4.x user screenshots for point select and axis arrows. No third-party code copied.
- Protocol remains host-based and 8-digit room codes are not global. Tailscale zero peers means no shared/authorized machines visible. Access must be configured outside Stick.
- CI on latest HEAD and real Windows graphics/multi-PC acceptance must be validated. Full Bézier handles and Blender grease-pencil equivalence are not claimed.
- User preference: publish complete changes without waiting for green GitHub Action, unless explicitly told not to publish. Maintain historical rollback releases.


## 17 Nov 2026 — OLD/NEW Timeline and community feedback
- Branch: `feat/old-new-timeline-feedback-20261117`; based on main `2416d3c515fdd121cb669af2422991ce9eac29da`.
- User supplied `AniHady2D_0.1.1_Full_Setup.zip`. Inspected safely without running EXE: exactly `AniHady2D-Setup.exe`, `README_AR.txt`, and `SHA256SUMS.txt`. README states the installed app contains layers/timeline and AV tools and has its own internal README. Archive contains NO source and no screenshot of the old timeline. Do not claim exact visual cloning.
- `src/timeline_styles.rs`: optional exposure-sheet rendering with frame/timecode ruler, held strips, keyframe diamonds, playhead and tween colors, tests.
- `src/main.rs`: persisted config `timeline_style` (default 0=OLD), `timeline_new_cell_width` (default 64), `timeline_new_row_height` (default 50), `timeline_show_timecodes` (default true). OLD preserves its prior width/row/layout. NEW shares frame/project operations; controls exposed in timeline gear. `show_feedback_dialog`: main launcher feedback link opens a closable popup with exact user-supplied Instagram URL and GitHub Issues.
- Validated source archive only. Windows unit tests / UI visual testing still needed before making runtime claims.


## 17 Nov 2026 — Google account backup, two-point perspective, picker and join UI
- Branch: `feat/drive-sync-2point-picker-online-20261117`, based on main `c871afecb18a991d6e1113d37adabf5194c73d63`. PR #30.
- `src/cloud_sync.rs`: Desktop OAuth PKCE via loopback browser; scope `openid email drive.appdata`, refresh in Windows Credential Manager using keyring; Drive appDataFolder backup/list/download. CloudProfile is strictly sanitized (no tokens/local paths), project files backed up from saved recent list only. Restore creates separate local copies and needs explicit user confirmation. Optional async auto-backup after local Save. OAuth Desktop client must be configured using GitHub Actions variable `STICK_GOOGLE_CLIENT_ID`; no credentials are auto-provisioned.
- `src/main.rs`: added Settings → تسجيل دخول tab with auth, backup/list/restore/signout, clear messages and optional auto-backup. Perspective mode reset to Off in AppConfig::load every launch; new two-point guide with two adjustable horizontal vanishing points and adjustable horizon; fixed eyedropper cursor drawing hotspot at native position; joined-room UI condensed to list/cards + direct IP/password and expandable code and troubleshooting.
- `src/collab.rs`: existing localhost TCP host↔guest handshake test was verified; a duplicate same-port test was removed because parallel runs conflicted. This does not prove Tailscale on different networks or traverse CGNAT on its own.
- `.github/workflows/build.yml`: public `STICK_GOOGLE_CLIENT_ID` variable available to Rust compile. Set this variable after provisioning Google Cloud Desktop OAuth and Drive API/consent screen (see `GOOGLE_DRIVE_SETUP.md`).
- Note: appDataFolder backup is invisible in standard My Drive. External media assets referenced by project file paths are not included; disclose limitation.
- Need Windows CI and manual Google OAuth / cross-ISP connectivity test before claiming production integration fully works. Publish user-approved updates by default, preserve old releases; user specifically permits merge before green action, but flag untested paths truthfully.
