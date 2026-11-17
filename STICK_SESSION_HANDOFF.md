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
