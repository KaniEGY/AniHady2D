# Stick Studio — Session Handoff / سجل استكمال المشروع

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
- Existing OniCraft-inspired timeline controls remain in place; no AniCraft code imported or animation engine replaced in this sprint. Plan targeted integration only after stable collaboration and acceptance QA.
- QA: PR Windows `cargo test` and `cargo build --release` via Actions; at time of writing, newest SHA result **not yet confirmed**. Two-PC GUI acceptance **not performed**. Do not claim this as proven in production until verified.
- Known limitations: simultaneous edits to identical array positions can conflict; external media bytes not transferred; no encryption/global Internet room discovery; no Windows executable generated locally (GitHub Actions builds it).
- Next: check latest PR Windows CI, fix any errors, merge only if green, verify main release, and test two-PC erasing/layer/frame/bucket behavior and existing project save/open.
