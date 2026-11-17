#![windows_subsystem = "windows"]

mod brushes;
mod brush_engine;
mod changelog;
mod exporter;
mod persistence;
mod profile;
mod project;

use arabic_reshaper::arabic_reshape;
use brushes::{draw_brush_preset_icon, draw_stroke_pro};
use eframe::egui;
use exporter::{ImageExportFormat, VideoExportFormat};
use profile::UserProfile;
use project::{
    AniProject, AudioSyncMode, AudioTrack, EasingCurve, FrameData, KritaBrushPreset, Layer, MediaItem, MediaType,
    Stroke, StrokePoint, Symbol, SymbolInstance, TextItem, TweenMode, PROJECT_SCHEMA_VERSION,
};
use rodio::source::SineWave;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use unicode_bidi::BidiInfo;

const FALLBACK_VERSION: &str = "4.1.0";
const GITHUB_REPO: &str = "KaniEGY/Stick";
const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@%D8%A7%D8%A8%D9%84%D9%83%D8%A7%D8%B4-%D8%A7%D9%86%D9%85%D9%8A%D8%B4%D9%86/videos";

#[derive(Clone, Copy, PartialEq)]
enum WorkspaceMode {
    InstallerSetup,
    Launcher,
    DrawingOnly,
    AnimationStudio,
}

#[derive(Clone, Copy, PartialEq)]
enum ActiveToolUI {
    SelectNormal,
    SelectLasso,
    FreeTransform,
    VectorDirectSelect,
    VectorPen,
    Brush,
    EraserRed,
    EraserGreen,
    Bucket,
    Eyedropper,
    Text,
    ShapeLine,
    HandCanvas,
}

#[derive(Clone, Copy, PartialEq)]
enum GeometricShapeKind {
    StraightLine,
    Circle,
    Square,
    Rectangle,
}

#[derive(Clone, Copy, PartialEq)]
enum SymmetryMode {
    Off,
    Horizontal,
    Vertical,
}

#[derive(Clone, PartialEq)]
enum ExportTaskState {
    Idle,
    Running(String),
    Done(String),
    Failed(String),
}

#[derive(Clone, Copy)]
enum SfxType {
    UiClick,
    NewFrame,
    VectorPenTick,
    BrushScratch,
    EraserRub,
    LassoSelect,
    ZoomStep,
    ExportSuccess,
    ExitFarewell,
    LevelUpFanfare,
}

fn play_sfx_async(enabled: bool, sfx: SfxType, wait_finish: bool) {
    if !enabled {
        return;
    }
    let handle = std::thread::spawn(move || {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                match sfx {
                    SfxType::UiClick => sink.append(SineWave::new(740.0).take_duration(Duration::from_millis(28)).amplify(0.12)),
                    SfxType::NewFrame => {
                        sink.append(SineWave::new(620.0).take_duration(Duration::from_millis(50)).amplify(0.18));
                        sink.append(SineWave::new(920.0).take_duration(Duration::from_millis(55)).amplify(0.18));
                    }
                    SfxType::VectorPenTick => sink.append(SineWave::new(1180.0).take_duration(Duration::from_millis(22)).amplify(0.09)),
                    SfxType::BrushScratch => sink.append(SineWave::new(290.0).take_duration(Duration::from_millis(32)).amplify(0.09)),
                    SfxType::EraserRub => sink.append(SineWave::new(175.0).take_duration(Duration::from_millis(45)).amplify(0.13)),
                    SfxType::LassoSelect => {
                        sink.append(SineWave::new(540.0).take_duration(Duration::from_millis(35)).amplify(0.14));
                        sink.append(SineWave::new(810.0).take_duration(Duration::from_millis(45)).amplify(0.14));
                    }
                    SfxType::ZoomStep => sink.append(SineWave::new(660.0).take_duration(Duration::from_millis(22)).amplify(0.09)),
                    SfxType::ExportSuccess => {
                        sink.append(SineWave::new(523.25).take_duration(Duration::from_millis(110)).amplify(0.20));
                        sink.append(SineWave::new(659.25).take_duration(Duration::from_millis(110)).amplify(0.20));
                        sink.append(SineWave::new(783.99).take_duration(Duration::from_millis(190)).amplify(0.22));
                    }
                    SfxType::ExitFarewell => {
                        sink.append(SineWave::new(520.0).take_duration(Duration::from_millis(130)).amplify(0.18));
                        sink.append(SineWave::new(340.0).take_duration(Duration::from_millis(170)).amplify(0.18));
                    }
                    SfxType::LevelUpFanfare => {
                        sink.append(SineWave::new(440.0).take_duration(Duration::from_millis(90)).amplify(0.22));
                        sink.append(SineWave::new(554.37).take_duration(Duration::from_millis(90)).amplify(0.22));
                        sink.append(SineWave::new(659.25).take_duration(Duration::from_millis(110)).amplify(0.24));
                        sink.append(SineWave::new(880.0).take_duration(Duration::from_millis(300)).amplify(0.28));
                    }
                }
                sink.sleep_until_end();
            }
        }
    });
    if wait_finish {
        let _ = handle.join();
    }
}

fn extract_audio_waveform(path: &str, max_bars: usize) -> Vec<f32> {
    if let Ok(file) = File::open(path) {
        if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
            let samples: Vec<i16> = decoder.take(44100 * 60).collect();
            if samples.is_empty() {
                return vec![0.3; 32];
            }
            let chunk_size = (samples.len() / max_bars.max(1)).max(1);
            let mut peaks = Vec::with_capacity(max_bars);
            for chunk in samples.chunks(chunk_size) {
                let max_val = chunk.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0) as f32;
                peaks.push((max_val / 32768.0).clamp(0.08, 1.0));
            }
            return peaks;
        }
    }
    vec![0.35; 32]
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
enum AppLang {
    English,
    Arabic,
}

fn fix_ar(text: &str) -> String {
    if !text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)) {
        return text.to_string();
    }
    let reshaped = arabic_reshape(text);
    let bidi_info = BidiInfo::new(&reshaped, None);
    if bidi_info.paragraphs.is_empty() {
        return reshaped;
    }
    let para = &bidi_info.paragraphs[0];
    bidi_info.reorder_line(para, para.range.clone()).into_owned()
}

fn tr(lang: AppLang, en: &str, ar: &str) -> String {
    match lang {
        AppLang::English => en.to_string(),
        AppLang::Arabic => fix_ar(ar),
    }
}

fn get_installed_version() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(v) = std::fs::read_to_string(exe.with_file_name("version.txt")) {
            let t = v.trim().trim_start_matches('v').to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    FALLBACK_VERSION.to_string()
}

fn save_installed_version(ver: &str) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::write(exe.with_file_name("version.txt"), ver.trim().trim_start_matches('v'));
    }
}

fn is_running_from_installed_dir() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("stick_installed.marker").exists()))
        .unwrap_or(false)
}

fn create_desktop_shortcut(target_exe: &Path) -> Result<(), String> {
    let ps_cmd = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Desktop = [Environment]::GetFolderPath('Desktop'); \
         $Shortcut = $WshShell.CreateShortcut(\"$Desktop\\Stick.lnk\"); \
         $Shortcut.TargetPath = '{}'; \
         $Shortcut.WorkingDirectory = '{}'; \
         $Shortcut.IconLocation = '{},0'; \
         $Shortcut.Description = 'Stick Studio'; \
         $Shortcut.Save()",
        target_exe.display().to_string().replace('\'', "''"),
        target_exe.parent().unwrap_or(Path::new(".")).display().to_string().replace('\'', "''"),
        target_exe.display().to_string().replace('\'', "''")
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err("Shortcut error".to_string()) }
}

#[derive(Clone, PartialEq)]
enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable { version: String, download_url: String },
    Downloading(String),
    ReadyToRestart,
    Error(String),
}

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    lang: AppLang,
    #[serde(default)]
    last_seen_patch_version: String,
    #[serde(default = "default_true_val")]
    sound_effects_enabled: bool,
    #[serde(default)]
    show_grid: bool,
    #[serde(default = "default_grid_size")]
    grid_size: f32,
    #[serde(default = "default_tl_bg")]
    timeline_bg_color: [u8; 4],
    #[serde(default = "default_tl_cell")]
    timeline_cell_color: [u8; 4],
    #[serde(default = "default_tl_head")]
    timeline_playhead_color: [u8; 4],
    #[serde(default = "default_autosave_mins")]
    autosave_interval_minutes: u64,
    tool_select: String,
    #[serde(default = "default_lasso_key")]
    tool_lasso: String,
    #[serde(default = "default_free_tr_key")]
    tool_free_transform: String,
    #[serde(default = "default_vec_sel_key")]
    tool_vector_select: String,
    tool_pen: String,
    tool_brush: String,
    tool_eraser: String,
    #[serde(default = "default_partial_eraser_key")]
    tool_eraser_partial: String,
    tool_bucket: String,
    tool_eyedropper: String,
    #[serde(default = "default_text_key")]
    tool_text: String,
    #[serde(default = "default_shape_key")]
    tool_shape: String,
    #[serde(default = "default_hand_key")]
    tool_hand: String,
    #[serde(default = "default_del_frame_key")]
    delete_frame_key: String,
    insert_frame: String,
    insert_keyframe: String,
    insert_blank_kf: String,
    convert_symbol: String,
    play_pause: String,
    prev_frame: String,
    next_frame: String,
    brush_smaller: String,
    brush_larger: String,
    reset_canvas: String,
}

fn default_true_val() -> bool { true }
fn default_grid_size() -> f32 { 40.0 }
fn default_autosave_mins() -> u64 { 5 }
fn default_tl_bg() -> [u8; 4] { [28, 29, 34, 255] }
fn default_tl_cell() -> [u8; 4] { [40, 42, 48, 255] }
fn default_tl_head() -> [u8; 4] { [225, 55, 55, 255] }
fn default_lasso_key() -> String { "L".to_string() }
fn default_free_tr_key() -> String { "Q".to_string() }
fn default_vec_sel_key() -> String { "G".to_string() }
fn default_partial_eraser_key() -> String { "T".to_string() }
fn default_text_key() -> String { "N".to_string() }
fn default_shape_key() -> String { "U".to_string() }
fn default_hand_key() -> String { "H".to_string() }
fn default_del_frame_key() -> String { "X".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lang: AppLang::Arabic,
            last_seen_patch_version: String::new(),
            sound_effects_enabled: true,
            show_grid: false,
            grid_size: 40.0,
            timeline_bg_color: default_tl_bg(),
            timeline_cell_color: default_tl_cell(),
            timeline_playhead_color: default_tl_head(),
            autosave_interval_minutes: 5,
            tool_select: "V".to_string(),
            tool_lasso: "L".to_string(),
            tool_free_transform: "Q".to_string(),
            tool_vector_select: "G".to_string(),
            tool_pen: "P".to_string(),
            tool_brush: "B".to_string(),
            tool_eraser: "E".to_string(),
            tool_eraser_partial: "T".to_string(),
            tool_bucket: "K".to_string(),
            tool_eyedropper: "I".to_string(),
            tool_text: "N".to_string(),
            tool_shape: "U".to_string(),
            tool_hand: "H".to_string(),
            delete_frame_key: "X".to_string(),
            insert_frame: "F5".to_string(),
            insert_keyframe: "F6".to_string(),
            insert_blank_kf: "F7".to_string(),
            convert_symbol: "F8".to_string(),
            play_pause: "Enter".to_string(),
            prev_frame: "Comma".to_string(),
            next_frame: "Period".to_string(),
            brush_smaller: "OpenBracket".to_string(),
            brush_larger: "CloseBracket".to_string(),
            reset_canvas: "R".to_string(),
        }
    }
}

impl AppConfig {
    fn load() -> Self {
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(data) = std::fs::read_to_string(exe.with_file_name("stick_config.json")) {
                if let Ok(parsed) = serde_json::from_str(&data) {
                    return parsed;
                }
            }
        }
        Self::default()
    }
    fn save(&self) {
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(exe.with_file_name("stick_config.json"), json);
            }
        }
    }
}

fn is_shortcut_pressed(i: &egui::InputState, key_name: &str) -> bool {
    if key_name.is_empty() { return false; }
    if key_name.eq_ignore_ascii_case("Shift") {
        return i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.alt && i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }));
    }
    if let Some(k) = egui::Key::from_name(key_name) {
        return i.key_pressed(k) && !i.modifiers.ctrl && !i.modifiers.alt;
    }
    false
}

fn generate_stick_window_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let fx = x as f32 - 32.0;
            let fy = y as f32 - 32.0;
            let u = (fx + fy) * 0.7071;
            let v = (-fx + fy) * 0.7071;
            if u.abs() <= 26.0 && v.abs() <= 5.0 {
                if u.abs() >= 24.4 || v.abs() >= 3.4 {
                    rgba[idx..idx + 4].copy_from_slice(&[15, 15, 18, 255]);
                } else {
                    rgba[idx..idx + 4].copy_from_slice(&[255, 215, 25, 255]);
                }
            }
        }
    }
    egui::IconData { rgba, width: size, height: size }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\tahoma.ttf") {
        fonts.font_data.insert("arabic_font".to_owned(), egui::FontData::from_owned(font_data));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "arabic_font".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn apply_stick_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_rgb(30, 31, 36);
    style.visuals.window_fill = egui::Color32::from_rgb(36, 37, 43);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(18, 18, 21);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 122, 204);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(46, 48, 55);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(64, 66, 75);
    style.spacing.item_spacing = egui::vec2(5.0, 5.0);
    style.spacing.button_padding = egui::vec2(7.0, 4.0);
    ctx.set_style(style);
}

fn draw_red_heart_icon(painter: &egui::Painter, rect: egui::Rect, hovered: bool) {
    let c = rect.center();
    let red = if hovered { egui::Color32::from_rgb(255, 75, 95) } else { egui::Color32::from_rgb(235, 35, 55) };
    let mut pts = Vec::with_capacity(40);
    for i in 0..40 {
        let t = (i as f32 / 40.0) * std::f32::consts::TAU;
        let x = 16.0 * t.sin().powi(3);
        let y = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());
        pts.push(c + egui::vec2(x * 0.45, y * 0.45));
    }
    painter.add(egui::Shape::convex_polygon(pts, red, egui::Stroke::NONE));
}

fn draw_undo_redo_arrow(painter: &egui::Painter, rect: egui::Rect, is_undo: bool, enabled: bool) {
    let c = rect.center();
    let base_col = if is_undo { egui::Color32::from_rgb(239, 68, 68) } else { egui::Color32::from_rgb(234, 179, 8) };
    let col = if enabled { base_col } else { egui::Color32::from_gray(95) };
    let stroke = egui::Stroke::new(2.3, col);
    if is_undo {
        painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(6.0, 0.0)], stroke);
        painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(-1.0, -5.0)], stroke);
        painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(-1.0, 5.0)], stroke);
    } else {
        painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(6.0, 0.0)], stroke);
        painter.line_segment([c + egui::vec2(6.0, 0.0), c + egui::vec2(1.0, -5.0)], stroke);
        painter.line_segment([c + egui::vec2(6.0, 0.0), c + egui::vec2(1.0, 5.0)], stroke);
    }
}

fn get_tool_ui_color(tool: ActiveToolUI) -> egui::Color32 {
    match tool {
        ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso => egui::Color32::from_rgb(56, 189, 248),
        ActiveToolUI::FreeTransform => egui::Color32::from_rgb(245, 158, 11),
        ActiveToolUI::VectorDirectSelect => egui::Color32::from_rgb(192, 132, 252),
        ActiveToolUI::VectorPen => egui::Color32::from_rgb(139, 92, 246),
        ActiveToolUI::Brush => egui::Color32::from_rgb(249, 115, 22),
        ActiveToolUI::EraserRed => egui::Color32::from_rgb(239, 68, 68),
        ActiveToolUI::EraserGreen => egui::Color32::from_rgb(34, 197, 94),
        ActiveToolUI::Bucket => egui::Color32::from_rgb(16, 185, 129),
        ActiveToolUI::Eyedropper => egui::Color32::from_rgb(234, 179, 8),
        ActiveToolUI::Text => egui::Color32::from_rgb(236, 72, 153),
        ActiveToolUI::ShapeLine => egui::Color32::from_rgb(20, 184, 166),
        ActiveToolUI::HandCanvas => egui::Color32::from_rgb(168, 85, 247),
    }
}

fn draw_tool_ui_icon(painter: &egui::Painter, rect: egui::Rect, tool: ActiveToolUI, active: bool) {
    let c = rect.center();
    let main_color = if active { egui::Color32::WHITE } else { get_tool_ui_color(tool) };
    let stroke = egui::Stroke::new(2.0, main_color);
    match tool {
        ActiveToolUI::SelectNormal => {
            let pts = vec![
                c + egui::vec2(-5.5, -8.5), c + egui::vec2(-5.5, 7.5), c + egui::vec2(-1.2, 3.8),
                c + egui::vec2(2.5, 9.5), c + egui::vec2(5.2, 8.0), c + egui::vec2(1.5, 2.2), c + egui::vec2(7.0, 2.2),
            ];
            painter.add(egui::Shape::convex_polygon(pts, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::SelectLasso => {
            painter.circle_stroke(c + egui::vec2(-1.0, -2.0), 6.5, stroke);
            painter.line_segment([c + egui::vec2(3.5, 3.0), c + egui::vec2(7.5, 8.5)], stroke);
        }
        ActiveToolUI::FreeTransform => {
            let sq = egui::Rect::from_center_size(c, egui::vec2(13.0, 13.0));
            painter.rect_stroke(sq, 1.0, stroke);
            painter.circle_filled(c, 2.2, main_color);
        }
        ActiveToolUI::VectorDirectSelect => {
            let pts = vec![c + egui::vec2(-4.5, -8.5), c + egui::vec2(-4.5, 6.5), c + egui::vec2(0.0, 3.0), c + egui::vec2(5.5, 3.0)];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(6.0, -5.0), 2.5, main_color);
        }
        ActiveToolUI::VectorPen => {
            let pts = vec![c + egui::vec2(0.0, -9.5), c + egui::vec2(6.5, 1.0), c + egui::vec2(3.5, 8.5), c + egui::vec2(-3.5, 8.5), c + egui::vec2(-6.5, 1.0)];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(0.0, -0.5), 2.0, main_color);
        }
        ActiveToolUI::Brush => {
            painter.line_segment([c + egui::vec2(-1.5, 1.5), c + egui::vec2(7.5, -7.5)], egui::Stroke::new(3.2, main_color));
            let tip = vec![c + egui::vec2(-2.0, 2.0), c + egui::vec2(-7.5, 3.5), c + egui::vec2(-8.5, 8.5), c + egui::vec2(-3.5, 7.5)];
            painter.add(egui::Shape::convex_polygon(tip, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
            let pts = vec![c + egui::vec2(-7.5, 2.0), c + egui::vec2(1.0, -6.5), c + egui::vec2(7.5, 0.0), c + egui::vec2(-1.0, 8.5)];
            painter.add(egui::Shape::closed_line(pts, stroke));
            let half = vec![c + egui::vec2(-7.5, 2.0), c + egui::vec2(-3.2, -2.2), c + egui::vec2(3.2, 4.2), c + egui::vec2(-1.0, 8.5)];
            painter.add(egui::Shape::convex_polygon(half, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::Bucket => {
            let pts = vec![c + egui::vec2(-6.5, 0.0), c + egui::vec2(0.0, -6.5), c + egui::vec2(6.5, 0.0), c + egui::vec2(0.0, 6.5)];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(7.5, 4.5), 2.6, main_color);
        }
        ActiveToolUI::Eyedropper => {
            painter.line_segment([c + egui::vec2(-7.0, 7.0), c + egui::vec2(2.5, -2.5)], egui::Stroke::new(2.4, main_color));
            painter.circle_filled(c + egui::vec2(5.0, -5.0), 3.6, main_color);
        }
        ActiveToolUI::Text => {
            painter.line_segment([c + egui::vec2(-7.0, -7.5), c + egui::vec2(7.0, -7.5)], egui::Stroke::new(2.5, main_color));
            painter.line_segment([c + egui::vec2(0.0, -7.5), c + egui::vec2(0.0, 8.0)], egui::Stroke::new(2.5, main_color));
        }
        ActiveToolUI::ShapeLine => {
            painter.rect_stroke(egui::Rect::from_min_size(c + egui::vec2(-7.5, -7.5), egui::vec2(8.0, 8.0)), 1.0, stroke);
            painter.circle_stroke(c + egui::vec2(4.0, 4.0), 4.5, stroke);
        }
        ActiveToolUI::HandCanvas => {
            let palm = egui::Rect::from_center_size(c + egui::vec2(0.0, 2.5), egui::vec2(10.0, 9.0));
            painter.rect_filled(palm, 3.0, main_color);
            for dx in [-4.0, -1.3, 1.3, 4.0] {
                painter.line_segment([c + egui::vec2(dx, -1.0), c + egui::vec2(dx, -7.5)], egui::Stroke::new(2.0, main_color));
            }
        }
    }
}

fn draw_gear_icon(painter: &egui::Painter, rect: egui::Rect, scale: f32) {
    let c = rect.center();
    let color = egui::Color32::from_gray(220);
    painter.circle_stroke(c, 5.5 * scale, egui::Stroke::new(1.8 * scale, color));
    painter.circle_filled(c, 2.0 * scale, color);
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::FRAC_PI_4;
        painter.line_segment(
            [c + egui::vec2(angle.cos() * 5.5 * scale, angle.sin() * 5.5 * scale), c + egui::vec2(angle.cos() * 8.2 * scale, angle.sin() * 8.2 * scale)],
            egui::Stroke::new(2.0 * scale, color),
        );
    }
}

fn draw_anicraft_timeline_cell(painter: &egui::Painter, rect: egui::Rect, is_keyframe: bool, has_content: bool, tween_mode: TweenMode) {
    let c = rect.center();
    match tween_mode {
        TweenMode::Classic => {
            painter.rect_filled(rect.shrink(1.0), 2.0, egui::Color32::from_rgba_unmultiplied(124, 58, 237, 150));
            painter.line_segment([egui::pos2(rect.left() + 2.0, c.y), egui::pos2(rect.right() - 2.0, c.y)], egui::Stroke::new(1.5, egui::Color32::from_rgb(221, 214, 254)));
        }
        TweenMode::Shape => {
            painter.rect_filled(rect.shrink(1.0), 2.0, egui::Color32::from_rgba_unmultiplied(5, 150, 105, 150));
            painter.line_segment([egui::pos2(rect.left() + 2.0, c.y), egui::pos2(rect.right() - 2.0, c.y)], egui::Stroke::new(1.5, egui::Color32::from_rgb(167, 243, 208)));
        }
        TweenMode::None => {}
    }
    if is_keyframe {
        if has_content {
            painter.circle_filled(c, 4.5, egui::Color32::from_rgb(15, 15, 20));
            painter.circle_stroke(c, 4.8, egui::Stroke::new(1.1, egui::Color32::WHITE));
        } else {
            painter.circle_stroke(c, 4.4, egui::Stroke::new(1.3, egui::Color32::from_gray(225)));
        }
    } else if has_content && tween_mode == TweenMode::None {
        painter.line_segment([egui::pos2(rect.left() + 2.0, c.y), egui::pos2(rect.right() - 2.0, c.y)], egui::Stroke::new(1.0, egui::Color32::from_gray(130)));
    }
}

#[derive(Clone)]
struct HistorySnapshot {
    project: Arc<AniProject>,
    timeline_length: usize,
    current_frame: usize,
    active_layer_idx: usize,
}

fn erase_partial_from_strokes(strokes: &mut Vec<Stroke>, center: egui::Pos2, radius: f32) {
    let mut new_strokes = Vec::with_capacity(strokes.len());
    for s in strokes.drain(..) {
        if s.points.len() == 1 {
            if ((s.points[0].x - center.x).powi(2) + (s.points[0].y - center.y).powi(2)).sqrt() >= radius {
                new_strokes.push(s);
            }
            continue;
        }
        let mut current_seg: Vec<StrokePoint> = Vec::new();
        for p in s.points {
            if ((p.x - center.x).powi(2) + (p.y - center.y).powi(2)).sqrt() >= radius {
                current_seg.push(p);
            } else if current_seg.len() >= 2 {
                new_strokes.push(Stroke {
                    points: std::mem::take(&mut current_seg),
                    color: s.color,
                    fill_color: None,
                    base_width: s.base_width,
                    is_vector: s.is_vector,
                    brush_preset: s.brush_preset,
                    opacity: s.opacity,
                    brush_texture_id: s.brush_texture_id.clone(),
                });
            } else {
                current_seg.clear();
            }
        }
        if current_seg.len() >= 2 {
            new_strokes.push(Stroke {
                points: current_seg,
                color: s.color,
                fill_color: None,
                base_width: s.base_width,
                is_vector: s.is_vector,
                brush_preset: s.brush_preset,
                opacity: s.opacity,
                brush_texture_id: s.brush_texture_id,
            });
        }
    }
    *strokes = new_strokes;
}

fn build_geometric_stroke(kind: GeometricShapeKind, p0: egui::Pos2, p1: egui::Pos2, color: [u8; 4], width: f32) -> Stroke {
    let mut pts = Vec::new();
    match kind {
        GeometricShapeKind::StraightLine => {
            pts.push(StrokePoint { x: p0.x, y: p0.y, pressure: 1.0 });
            pts.push(StrokePoint { x: p1.x, y: p1.y, pressure: 1.0 });
        }
        GeometricShapeKind::Circle => {
            let center = p0.lerp(p1, 0.5);
            let r = (p1 - p0).length() * 0.5;
            for i in 0..=36 {
                let ang = (i as f32 / 36.0) * std::f32::consts::TAU;
                pts.push(StrokePoint { x: center.x + ang.cos() * r, y: center.y + ang.sin() * r, pressure: 1.0 });
            }
        }
        GeometricShapeKind::Square => {
            let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
            let side = dx.abs().max(dy.abs());
            let (sx, sy) = (p0.x + side * dx.signum(), p0.y + side * dy.signum());
            for c in [egui::pos2(p0.x, p0.y), egui::pos2(sx, p0.y), egui::pos2(sx, sy), egui::pos2(p0.x, sy), egui::pos2(p0.x, p0.y)] {
                pts.push(StrokePoint { x: c.x, y: c.y, pressure: 1.0 });
            }
        }
        GeometricShapeKind::Rectangle => {
            for c in [egui::pos2(p0.x, p0.y), egui::pos2(p1.x, p0.y), egui::pos2(p1.x, p1.y), egui::pos2(p0.x, p1.y), egui::pos2(p0.x, p0.y)] {
                pts.push(StrokePoint { x: c.x, y: c.y, pressure: 1.0 });
            }
        }
    }
    Stroke {
        points: pts,
        color,
        fill_color: None,
        base_width: width,
        is_vector: true,
        brush_preset: KritaBrushPreset::GPenInk,
        opacity: 1.0,
        brush_texture_id: None,
    }
}

struct StickApp {
    workspace_mode: WorkspaceMode,
    install_target_dir: String,
    install_status_text: String,
    project: AniProject,
    current_project_path: Option<PathBuf>,
    last_autosave_time: f64,
    is_fullscreen: bool,
    show_project_notice: bool,
    project_notice_title: String,
    project_notice_body: String,
    show_recovery_dialog: bool,
    show_project_details: bool,
    timeline_length: usize,
    timeline_cell_width: f32,
    loop_enabled: bool,
    loop_start_frame: usize,
    loop_end_frame: usize,
    undo_stack: Vec<HistorySnapshot>,
    redo_stack: Vec<HistorySnapshot>,
    clipboard_frame: Option<FrameData>,
    current_frame: usize,
    last_scrubbed_frame: usize,
    active_layer_idx: usize,
    editing_symbol_name: Option<String>,
    selected_audio_idx: Option<usize>,
    active_tool_ui: ActiveToolUI,
    active_krita_brush: KritaBrushPreset,
    brush_opacity: f32,
    selected_shape_kind: GeometricShapeKind,
    symmetry_mode: SymmetryMode,
    stabilizer_strength: f32,
    stabilized_cursor: Option<egui::Pos2>,
    shape_drag_start: Option<egui::Pos2>,
    shape_drag_current: Option<egui::Pos2>,
    selected_stroke_indices: Vec<usize>,
    custom_anchor_offset: egui::Vec2,
    brush_color: [u8; 4],
    stage_bg_color: [u8; 4],
    brush_size: f32,
    zoom_level: f32,
    pan_offset: egui::Vec2,
    canvas_rotation_deg: f32,
    canvas_flip_h: bool,
    canvas_flip_v: bool,
    is_playing: bool,
    last_frame_time: f64,
    last_xp_tick_time: f64,
    frames_drawn_counter: u64,
    level_up_banner_timer: f64,
    level_up_custom_msg: String,
    has_unsaved_changes: bool,
    show_exit_dialog: bool,
    show_symbol_dialog: bool,
    show_text_dialog: bool,
    show_export_studio: bool,
    export_studio_tab: usize,
    show_command_palette: bool,
    selected_export_format: VideoExportFormat,
    selected_image_format: ImageExportFormat,
    export_task_state: Arc<Mutex<ExportTaskState>>,
    text_input_buf: String,
    text_font_size: f32,
    pending_text_pos: egui::Pos2,
    show_update_dialog: bool,
    show_patch_notes_dialog: bool,
    show_settings_dialog: bool,
    show_credits_dialog: bool,
    settings_tab: usize,
    listening_shortcut_idx: Option<usize>,
    config: AppConfig,
    user_profile: UserProfile,
    installed_version: String,
    update_state: Arc<Mutex<UpdateState>>,
    new_symbol_name: String,
    allow_close: bool,
    current_stroke: Option<Stroke>,
    texture_cache: HashMap<String, egui::TextureHandle>,
    status_message: String,
    audio_sink: Option<(OutputStream, Sink)>,
    last_pointer_pos: Option<egui::Pos2>,
}

impl StickApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        apply_stick_theme(&cc.egui_ctx);
        let mut proj = AniProject::default();
        proj.total_frames = 1;
        let profile = UserProfile::load();
        let default_install_path = std::env::var("LOCALAPPDATA").map(|p| format!("{}\\StickStudio", p)).unwrap_or_else(|_| "C:\\StickStudio".to_string());
        let initial_mode = if is_running_from_installed_dir() { WorkspaceMode::Launcher } else { WorkspaceMode::InstallerSetup };

        Self {
            workspace_mode: initial_mode,
            install_target_dir: default_install_path,
            install_status_text: String::new(),
            project: proj,
            current_project_path: None,
            last_autosave_time: 0.0,
            is_fullscreen: false,
            show_project_notice: false,
            project_notice_title: String::new(),
            project_notice_body: String::new(),
            show_recovery_dialog: false,
            show_project_details: false,
            timeline_length: 1,
            timeline_cell_width: 24.0,
            loop_enabled: false,
            loop_start_frame: 1,
            loop_end_frame: 12,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard_frame: None,
            current_frame: 1,
            last_scrubbed_frame: 1,
            active_layer_idx: 0,
            editing_symbol_name: None,
            selected_audio_idx: None,
            active_tool_ui: ActiveToolUI::Brush,
            active_krita_brush: KritaBrushPreset::GPenInk,
            brush_opacity: 1.0,
            selected_shape_kind: GeometricShapeKind::StraightLine,
            symmetry_mode: SymmetryMode::Off,
            stabilizer_strength: 0.30,
            stabilized_cursor: None,
            shape_drag_start: None,
            shape_drag_current: None,
            selected_stroke_indices: Vec::new(),
            custom_anchor_offset: egui::Vec2::ZERO,
            brush_color: [0, 0, 0, 255],
            stage_bg_color: [255, 255, 255, 255],
            brush_size: 7.0,
            zoom_level: 0.75,
            pan_offset: egui::Vec2::ZERO,
            canvas_rotation_deg: 0.0,
            canvas_flip_h: false,
            canvas_flip_v: false,
            is_playing: false,
            last_frame_time: 0.0,
            last_xp_tick_time: 0.0,
            frames_drawn_counter: 0,
            level_up_banner_timer: 0.0,
            level_up_custom_msg: String::new(),
            has_unsaved_changes: false,
            show_exit_dialog: false,
            show_symbol_dialog: false,
            show_text_dialog: false,
            show_export_studio: false,
            export_studio_tab: 0,
            show_command_palette: false,
            selected_export_format: VideoExportFormat::GifInstant,
            selected_image_format: ImageExportFormat::PngTransparent,
            export_task_state: Arc::new(Mutex::new(ExportTaskState::Idle)),
            text_input_buf: String::new(),
            text_font_size: 36.0,
            pending_text_pos: egui::pos2(200.0, 200.0),
            show_update_dialog: false,
            show_patch_notes_dialog: AppConfig::load().last_seen_patch_version != FALLBACK_VERSION,
            show_settings_dialog: false,
            show_credits_dialog: false,
            settings_tab: 0,
            listening_shortcut_idx: None,
            config: AppConfig::load(),
            user_profile: profile,
            installed_version: get_installed_version(),
            update_state: Arc::new(Mutex::new(UpdateState::Idle)),
            new_symbol_name: "Symbol_1".to_string(),
            allow_close: false,
            current_stroke: None,
            texture_cache: HashMap::new(),
            status_message: "Stick Studio Ready".to_string(),
            audio_sink: None,
            last_pointer_pos: None,
        }
    }

    fn is_active_layer_locked(&self) -> bool {
        self.project.layers.get(self.active_layer_idx).map(|l| l.locked).unwrap_or(false)
    }

    fn perform_full_installation(&mut self) {
        let target_dir = PathBuf::from(self.install_target_dir.trim());
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            self.install_status_text = format!("خطأ في إنشاء المجلد: {}", e);
            return;
        }
        let target_exe = target_dir.join("Stick.exe");
        if let Ok(curr_exe) = std::env::current_exe() {
            if curr_exe != target_exe {
                if let Err(e) = std::fs::copy(&curr_exe, &target_exe) {
                    self.install_status_text = format!("خطأ في نسخ البرنامج: {}", e);
                    return;
                }
            }
        }
        let _ = std::fs::write(target_dir.join("stick_installed.marker"), "installed");
        let _ = std::fs::write(target_dir.join("version.txt"), FALLBACK_VERSION);
        let _ = create_desktop_shortcut(&target_exe);
        self.play_sfx(SfxType::LevelUpFanfare);
        self.workspace_mode = WorkspaceMode::Launcher;
        self.status_message = "تم التسطيب وإنشاء اختصار Stick على سطح المكتب!".to_string();
    }

    fn start_background_video_export(&mut self, output_path: PathBuf, ctx: egui::Context) {
        self.project.total_frames = self.timeline_length;
        let proj_clone = self.project.clone();
        let fmt = self.selected_export_format;
        let bg = self.stage_bg_color;
        let sfx_ok = self.config.sound_effects_enabled;
        let state_arc = Arc::clone(&self.export_task_state);
        *state_arc.lock().unwrap() = ExportTaskState::Running("جاري تصدير الفريمات بالتوازي فائق السرعة...".to_string());

        std::thread::spawn(move || {
            let res = exporter::export_video_multi_format(&proj_clone, &output_path, fmt, bg);
            let mut st = state_arc.lock().unwrap();
            match res {
                Ok(_) => {
                    play_sfx_async(sfx_ok, SfxType::ExportSuccess, false);
                    *st = ExportTaskState::Done(format!("تم التصدير بنجاح بصيغة .{}!", fmt.ext().to_uppercase()));
                }
                Err(err) => *st = ExportTaskState::Failed(err),
            }
            ctx.request_repaint();
        });
    }

    fn play_sfx(&self, sfx: SfxType) { play_sfx_async(self.config.sound_effects_enabled, sfx, false); }

    fn grant_xp(&mut self, amount: u64, now_time: f64) {
        if self.user_profile.add_xp_check_levelup(amount) {
            let (new_lvl, _, _) = self.user_profile.get_level_progress();
            self.level_up_custom_msg = UserProfile::get_level_up_system_message(new_lvl).to_string();
            self.level_up_banner_timer = now_time + 5.5;
            self.play_sfx(SfxType::LevelUpFanfare);
        }
    }

    fn record_frame_drawn_activity(&mut self, now_time: f64) {
        self.user_profile.total_frames_drawn += 1;
        self.frames_drawn_counter += 1;
        if self.frames_drawn_counter >= 20 {
            self.frames_drawn_counter = 0;
            self.grant_xp(35, now_time);
        } else {
            self.user_profile.save();
        }
    }

    fn take_snapshot(&self) -> HistorySnapshot {
        HistorySnapshot {
            project: Arc::new(self.project.clone()),
            timeline_length: self.timeline_length,
            current_frame: self.current_frame,
            active_layer_idx: self.active_layer_idx,
        }
    }

    fn restore_snapshot(&mut self, snap: HistorySnapshot) {
        self.project = (*snap.project).clone();
        self.timeline_length = snap.timeline_length.max(1);
        self.current_frame = snap.current_frame.clamp(1, self.timeline_length);
        self.active_layer_idx = snap.active_layer_idx.min(self.project.layers.len().saturating_sub(1));
        self.selected_stroke_indices.clear();
    }

    fn save_undo_state(&mut self) {
        if self.undo_stack.len() > 60 { self.undo_stack.remove(0); }
        self.undo_stack.push(self.take_snapshot());
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.take_snapshot());
            self.restore_snapshot(prev);
            self.play_sfx(SfxType::UiClick);
            self.status_message = tr(self.config.lang, "Undo", "تراجع");
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.take_snapshot());
            self.restore_snapshot(next);
            self.play_sfx(SfxType::UiClick);
            self.status_message = tr(self.config.lang, "Redo", "إعادة");
        }
    }

    fn get_effective_keyframe_index(layer: &Layer, frame: usize) -> Option<usize> {
        layer.keyframes.keys().filter(|&&k| k <= frame).max().copied()
    }

    fn get_effective_frame_data<'a>(layer: &'a Layer, frame: usize) -> Option<&'a FrameData> {
        Self::get_effective_keyframe_index(layer, frame).and_then(|k| layer.keyframes.get(&k))
    }

    fn ensure_editable_keyframe(&mut self) {
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if !layer.keyframes.contains_key(&self.current_frame) {
                let inherited = Self::get_effective_keyframe_index(layer, self.current_frame)
                    .and_then(|k| layer.keyframes.get(&k).cloned())
                    .unwrap_or_default();
                layer.keyframes.insert(self.current_frame, inherited);
            }
        }
    }

    fn commit_stroke_with_symmetry(&mut self, stroke: Stroke, now_time: f64) {
        if let Some(ref sym_name) = self.editing_symbol_name.clone() {
            if let Some(sym) = self.project.library_symbols.get_mut(sym_name) {
                let kf_strokes = sym.nested_keyframes.entry(self.current_frame).or_insert_with(|| sym.strokes.clone());
                kf_strokes.push(stroke.clone());
                if self.current_frame == 1 { sym.strokes = kf_strokes.clone(); }
                self.has_unsaved_changes = true;
                self.record_frame_drawn_activity(now_time);
                return;
            }
        }
        let (stage_w, stage_h) = (self.project.width as f32, self.project.height as f32);
        let mut strokes_to_add = vec![stroke.clone()];
        match self.symmetry_mode {
            SymmetryMode::Horizontal => {
                let mut m = stroke;
                for p in &mut m.points { p.x = stage_w - p.x; }
                strokes_to_add.push(m);
            }
            SymmetryMode::Vertical => {
                let mut m = stroke;
                for p in &mut m.points { p.y = stage_h - p.y; }
                strokes_to_add.push(m);
            }
            SymmetryMode::Off => {}
        }
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            layer.keyframes.entry(self.current_frame).or_default().strokes.extend(strokes_to_add);
            self.has_unsaved_changes = true;
            self.record_frame_drawn_activity(now_time);
        }
    }

    fn transform_selected_items(&mut self, scale_factor: f32, delta_angle_deg: f32, flip_h: bool, flip_v: bool) {
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                let mut min_p = egui::pos2(f32::MAX, f32::MAX);
                let mut max_p = egui::pos2(f32::MIN, f32::MIN);
                let mut has_any = false;
                for &idx in &self.selected_stroke_indices {
                    if let Some(s) = frame.strokes.get(idx) {
                        for p in &s.points {
                            min_p = min_p.min(egui::pos2(p.x, p.y));
                            max_p = max_p.max(egui::pos2(p.x, p.y));
                            has_any = true;
                        }
                    }
                }
                if !has_any { return; }
                let pivot = min_p.lerp(max_p, 0.5) + self.custom_anchor_offset;
                let (sin_a, cos_a) = delta_angle_deg.to_radians().sin_cos();
                for &idx in &self.selected_stroke_indices {
                    if let Some(s) = frame.strokes.get_mut(idx) {
                        for p in &mut s.points {
                            let mut dx = (p.x - pivot.x) * scale_factor;
                            let mut dy = (p.y - pivot.y) * scale_factor;
                            if flip_h { dx = -dx; }
                            if flip_v { dy = -dy; }
                            p.x = pivot.x + (dx * cos_a - dy * sin_a);
                            p.y = pivot.y + (dx * sin_a + dy * cos_a);
                        }
                    }
                }
                self.has_unsaved_changes = true;
            }
        }
    }

    fn action_insert_frame_f5(&mut self) {
        self.save_undo_state();
        self.timeline_length += 1;
        self.project.total_frames = self.timeline_length;
        self.current_frame = self.timeline_length;
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::NewFrame);
    }

    fn action_insert_keyframe_f6(&mut self, now_time: f64) {
        self.save_undo_state();
        let next_f = self.current_frame + 1;
        if next_f > self.timeline_length {
            self.timeline_length = next_f;
            self.project.total_frames = self.timeline_length;
        }
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            let copied = Self::get_effective_keyframe_index(layer, self.current_frame)
                .and_then(|k| layer.keyframes.get(&k).cloned())
                .unwrap_or_default();
            layer.keyframes.insert(next_f, copied);
        }
        self.current_frame = next_f;
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::NewFrame);
        self.record_frame_drawn_activity(now_time);
    }

    fn action_insert_blank_keyframe_f7(&mut self, now_time: f64) {
        self.save_undo_state();
        let next_f = self.current_frame + 1;
        if next_f > self.timeline_length {
            self.timeline_length = next_f;
            self.project.total_frames = self.timeline_length;
        }
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            layer.keyframes.insert(next_f, FrameData::default());
        }
        self.current_frame = next_f;
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::NewFrame);
        self.record_frame_drawn_activity(now_time);
    }

    fn copy_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(layer) = self.project.layers.get(layer_idx) {
            self.clipboard_frame = Some(Self::get_effective_frame_data(layer, frame_idx).cloned().unwrap_or_default());
            self.play_sfx(SfxType::UiClick);
        }
    }

    fn paste_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(clip) = self.clipboard_frame.clone() {
            self.save_undo_state();
            if let Some(layer) = self.project.layers.get_mut(layer_idx) {
                layer.keyframes.insert(frame_idx, clip);
                self.has_unsaved_changes = true;
                self.play_sfx(SfxType::NewFrame);
            }
        }
    }

    fn remove_frame_completely(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            let mut updated = HashMap::new();
            for (&k, v) in &layer.keyframes {
                if k < frame_idx { updated.insert(k, v.clone()); }
                else if k > frame_idx { updated.insert(k - 1, v.clone()); }
            }
            if updated.is_empty() { updated.insert(1, FrameData::default()); }
            layer.keyframes = updated;
        }
        if self.timeline_length > 1 {
            self.timeline_length -= 1;
            self.project.total_frames = self.timeline_length;
        }
        if self.current_frame > self.timeline_length { self.current_frame = self.timeline_length; }
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::EraserRub);
    }

    fn trigger_audio_stream_scrub(&mut self) {
        if self.is_playing { return; }
        let fps = self.project.fps.max(1) as f64;
        for track in &self.project.audio_tracks {
            if track.sync_mode == AudioSyncMode::Stream && self.current_frame >= track.start_frame && self.current_frame <= track.end_frame {
                let seek_secs = (self.current_frame - track.start_frame) as f64 / fps;
                let path = track.file_path.clone();
                let frame_dur_ms = (1000.0 / fps).clamp(40.0, 120.0) as u64;
                std::thread::spawn(move || {
                    if let Ok((_s, h)) = OutputStream::try_default() {
                        if let Ok(sink) = Sink::try_new(&h) {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(src) = Decoder::new(BufReader::new(file)) {
                                    sink.append(src.skip_duration(Duration::from_secs_f64(seek_secs)).take_duration(Duration::from_millis(frame_dur_ms)));
                                    sink.sleep_until_end();
                                }
                            }
                        }
                    }
                });
                break;
            }
        }
    }

    fn start_audio_playback(&mut self) {
        if let Some(track) = self.project.audio_tracks.first() {
            if self.current_frame > track.end_frame { return; }
            let fps = self.project.fps.max(1) as f64;
            let start_offset = self.current_frame.saturating_sub(track.start_frame) as f64 / fps;
            let max_dur = (track.end_frame.saturating_sub(self.current_frame) + 1) as f64 / fps;
            if let Ok((stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    if let Ok(file) = File::open(&track.file_path) {
                        if let Ok(src) = Decoder::new(BufReader::new(file)) {
                            sink.append(src.skip_duration(Duration::from_secs_f64(start_offset)).take_duration(Duration::from_secs_f64(max_dur.max(0.1))));
                            self.audio_sink = Some((stream, sink));
                        }
                    }
                }
            }
        }
    }

    fn stop_audio_playback(&mut self) {
        if let Some((_, sink)) = self.audio_sink.take() { sink.stop(); }
    }

    fn trigger_save(&mut self) -> bool {
        if let Some(existing_path) = self.current_project_path.clone() {
            self.project.total_frames = self.timeline_length;
            match persistence::save_project_safe(&self.project, &existing_path) {
                Ok(_) => {
                    self.has_unsaved_changes = false;
                    self.play_sfx(SfxType::NewFrame);
                    self.status_message = tr(self.config.lang, "Saved atomically!", "تم الحفظ الذري الآمن!");
                    return true;
                }
                Err(err) => {
                    self.project_notice_title = "فشل حفظ المشروع".to_string();
                    self.project_notice_body = err;
                    self.show_project_notice = true;
                    return false;
                }
            }
        }
        self.trigger_save_as()
    }

    fn trigger_save_as(&mut self) -> bool {
        self.project.total_frames = self.timeline_length;
        if let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Stick Project", &["stick", "ah2d"])
            .set_file_name("project.stick")
            .save_file()
        {
            if path.extension().is_none() {
                path.set_extension("stick");
            }
            match persistence::save_project_safe(&self.project, &path) {
                Ok(_) => {
                    self.current_project_path = Some(path);
                    self.has_unsaved_changes = false;
                    self.play_sfx(SfxType::NewFrame);
                    self.status_message = tr(self.config.lang, "Project Saved!", "تم حفظ المشروع بالكامل!");
                    return true;
                }
                Err(err) => {
                    self.project_notice_title = "فشل حفظ المشروع".to_string();
                    self.project_notice_body = err;
                    self.show_project_notice = true;
                }
            }
        }
        false
    }

    fn trigger_open_project(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Stick", &["stick", "ah2d"]).pick_file() {
            match persistence::open_project_safe(&p) {
                Ok(outcome) => {
                    self.save_undo_state();
                    self.timeline_length = outcome.project.total_frames.max(1);
                    self.current_frame = 1;
                    self.active_layer_idx = 0;
                    self.project = outcome.project;
                    self.has_unsaved_changes = false;
                    self.play_sfx(SfxType::NewFrame);

                    if outcome.report.was_migrated {
                        // لا نكتب فوق الملف القديم تلقائياً؛ ننشئ نسخة محدثة مستقلة أو نترك المستخدم يختار مكان الحفظ
                        let migrated_path = p.with_file_name(format!(
                            "{}_v{}.stick",
                            p.file_stem().and_then(|s| s.to_str()).unwrap_or("project"),
                            PROJECT_SCHEMA_VERSION
                        ));
                        self.current_project_path = Some(migrated_path);
                        self.has_unsaved_changes = true;
                        let mut msg = format!(
                            "تم إنشاء هذا المشروع بإصدار قديم من Stick ({}).\nتم فتح نسخة متوافقة مع الإصدار الحالي (Schema v{}) مع الاحتفاظ بالملف الأصلي سليماً دون تعديل.",
                            outcome.report.source_format_label, outcome.report.new_schema
                        );
                        if let Some(bak) = outcome.legacy_backup_path {
                            msg.push_str(&format!("\n\nتم حفظ نسخة احتياطية إضافية في:\n{}", bak.display()));
                        }
                        if !outcome.missing_assets.is_empty() {
                            msg.push_str(&format!("\n\nتنبيه: يوجد {} ملفات صور/صوت خارجية غير موجودة في مسارها القديم، لكن الرسومات والفريمات سليمة 100%.", outcome.missing_assets.len()));
                        }
                        self.project_notice_title = "تمت ترقية المشروع القديم بأمان".to_string();
                        self.project_notice_body = msg;
                        self.show_project_notice = true;
                    } else {
                        self.current_project_path = Some(p);
                        if !outcome.missing_assets.is_empty() {
                            self.project_notice_title = "تنبيه: ملفات وسائط مفقودة".to_string();
                            self.project_notice_body = format!(
                                "تم فتح المشروع بنجاح والرسومات سليمة، ولكن تعذر العثور على {} ملفات صور/صوت خارجية:\n{}",
                                outcome.missing_assets.len(),
                                outcome.missing_assets.join("\n")
                            );
                            self.show_project_notice = true;
                        }
                    }
                }
                Err(err_msg) => {
                    self.project_notice_title = "تعذر فتح ملف المشروع (مشروعك الحالي سليم)".to_string();
                    self.project_notice_body = err_msg;
                    self.show_project_notice = true;
                }
            }
        }
    }

    fn import_image_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]).pick_file() {
            self.save_undo_state();
            let img_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let mut kf = HashMap::new();
            let mut fd = FrameData::default();
            fd.media_items.push(MediaItem { path: path.to_string_lossy().to_string(), media_type: MediaType::Image, x: 100.0, y: 50.0, width: 400.0, height: 300.0 });
            kf.insert(self.current_frame, fd);
            self.project.layers.insert(0, Layer { name: format!("Img_{}", img_name), visible: true, locked: false, is_guide: false, opacity: 1.0, keyframes: kf });
            self.active_layer_idx = 0;
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::NewFrame);
        }
    }

    fn import_svg_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).pick_file() {
            if let Ok(strokes) = project::parse_svg_to_strokes(&path) {
                self.save_undo_state();
                self.ensure_editable_keyframe();
                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                    layer.keyframes.entry(self.current_frame).or_default().strokes.extend(strokes);
                    self.has_unsaved_changes = true;
                    self.play_sfx(SfxType::NewFrame);
                }
            }
        }
    }

    fn import_animate_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Adobe Animate", &["xfl", "fla"]).pick_file() {
            if let Ok(proj) = project::import_adobe_animate_project(&path) {
                self.save_undo_state();
                self.timeline_length = proj.total_frames.max(1);
                self.current_frame = 1;
                self.active_layer_idx = 0;
                self.project = proj;
                self.has_unsaved_changes = true;
                self.play_sfx(SfxType::NewFrame);
            }
        }
    }

    fn import_video_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Videos", &["mp4", "mov", "avi", "mkv", "webm"]).pick_file() {
            if let Ok(frames) = exporter::extract_video_frames(&path, self.project.fps) {
                self.save_undo_state();
                let needed_len = self.current_frame + frames.len().saturating_sub(1);
                if needed_len > self.timeline_length {
                    self.timeline_length = needed_len;
                    self.project.total_frames = needed_len;
                }
                let mut kf = HashMap::new();
                for (i, f_path) in frames.iter().enumerate() {
                    let mut fd = FrameData::default();
                    fd.media_items.push(MediaItem { path: f_path.to_string_lossy().to_string(), media_type: MediaType::VideoFrame, x: 0.0, y: 0.0, width: self.project.width as f32, height: self.project.height as f32 });
                    kf.insert(self.current_frame + i, fd);
                }
                self.project.layers.insert(0, Layer { name: "Video_Layer".to_string(), visible: true, locked: false, is_guide: false, opacity: 1.0, keyframes: kf });
                self.active_layer_idx = 0;
                self.has_unsaved_changes = true;
                self.play_sfx(SfxType::NewFrame);
            }
        }
    }

    fn import_audio_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Audio", &["mp3", "wav"]).pick_file() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let p_str = path.to_string_lossy().to_string();
            let peaks = extract_audio_waveform(&p_str, 120);
            self.save_undo_state();
            if self.timeline_length < 24 { self.timeline_length = 24; self.project.total_frames = 24; }
            self.project.audio_tracks.push(AudioTrack { name, file_path: p_str, start_frame: self.current_frame, end_frame: self.timeline_length, sync_mode: AudioSyncMode::Stream, waveform_peaks: peaks });
            self.selected_audio_idx = Some(self.project.audio_tracks.len() - 1);
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::NewFrame);
        }
    }

    fn check_for_updates(&mut self, ctx: egui::Context) {
        self.show_update_dialog = true;
        *self.update_state.lock().unwrap() = UpdateState::Checking;
        let state_clone = Arc::clone(&self.update_state);
        let current_ver = self.installed_version.clone();
        std::thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases?per_page=1", GITHUB_REPO);
            if let Ok(resp) = ureq::get(&api_url).set("User-Agent", "Stick-Updater").call() {
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    if let Some(latest) = json.as_array().and_then(|arr| arr.first()) {
                        let tag = latest["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
                        let mut exe_url = String::new();
                        if let Some(assets) = latest["assets"].as_array() {
                            for asset in assets {
                                if asset["name"].as_str().unwrap_or("").ends_with(".exe") {
                                    exe_url = asset["browser_download_url"].as_str().unwrap_or("").to_string();
                                    break;
                                }
                            }
                        }
                        let mut st = state_clone.lock().unwrap();
                        if !tag.is_empty() && tag != current_ver && !exe_url.is_empty() {
                            *st = UpdateState::UpdateAvailable { version: tag, download_url: exe_url };
                        } else {
                            *st = UpdateState::UpToDate;
                        }
                    }
                }
            }
            ctx.request_repaint();
        });
    }

    fn download_and_apply_update(&mut self, version: String, download_url: String, ctx: egui::Context) {
        *self.update_state.lock().unwrap() = UpdateState::Downloading(format!("Downloading v{}...", version));
        let state_clone = Arc::clone(&self.update_state);
        std::thread::spawn(move || {
            if let Ok(resp) = ureq::get(&download_url).set("User-Agent", "Stick-Updater").call() {
                let mut bytes = Vec::new();
                if resp.into_reader().read_to_end(&mut bytes).is_ok() {
                    if let Ok(curr) = std::env::current_exe() {
                        if let Ok(mut file) = File::create(curr.with_file_name("Stick_new.exe")) {
                            if file.write_all(&bytes).is_ok() {
                                save_installed_version(&version);
                                *state_clone.lock().unwrap() = UpdateState::ReadyToRestart;
                                ctx.request_repaint();
                                return;
                            }
                        }
                    }
                }
            }
            *state_clone.lock().unwrap() = UpdateState::Error("Download failed.".to_string());
            ctx.request_repaint();
        });
    }

    fn restart_with_new_exe(&mut self) {
        if let Ok(curr) = std::env::current_exe() {
            let new_exe = curr.with_file_name("Stick_new.exe");
            let ps = format!("Start-Sleep -Seconds 2; Move-Item -LiteralPath '{}' -Destination '{}' -Force; Start-Process -FilePath '{}'", new_exe.display(), curr.display(), curr.display());
            let _ = std::process::Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]).spawn();
            std::process::exit(0);
        }
    }

    fn preload_frame_textures(&mut self, ctx: &egui::Context) {
        let mut paths = Vec::new();
        for layer in &self.project.layers {
            if layer.visible {
                let fd = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                for m in &fd.media_items {
                    if !self.texture_cache.contains_key(&m.path) { paths.push(m.path.clone()); }
                }
            }
        }
        for path in paths {
            if let Ok(img) = image::open(&path) {
                let rgba = img.to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                self.texture_cache.insert(path.clone(), ctx.load_texture(&path, ci, egui::TextureOptions::LINEAR));
            }
        }
    }

    fn render_tool_properties_ui(&mut self, ui: &mut egui::Ui, lang: AppLang) {
        match self.active_tool_ui {
            ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso | ActiveToolUI::FreeTransform | ActiveToolUI::VectorDirectSelect => {
                ui.strong(tr(lang, "Transform & Selection", "خصائص التحديد والتحويل الحر"));
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::SelectNormal, tr(lang, "Select (V)", "تحديد (V)")).clicked() { self.active_tool_ui = ActiveToolUI::SelectNormal; }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::FreeTransform, tr(lang, "Transform (Q)", "تحويل حر (Q)")).clicked() { self.active_tool_ui = ActiveToolUI::FreeTransform; }
                });
                ui.horizontal(|ui| {
                    if ui.button(" -10% ").clicked() { self.save_undo_state(); self.transform_selected_items(0.90, 0.0, false, false); }
                    if ui.button(" +10% ").clicked() { self.save_undo_state(); self.transform_selected_items(1.10, 0.0, false, false); }
                    if ui.button(" ↻ 15° ").clicked() { self.save_undo_state(); self.transform_selected_items(1.0, 15.0, false, false); }
                    if ui.button(" ↺ 15° ").clicked() { self.save_undo_state(); self.transform_selected_items(1.0, -15.0, false, false); }
                });
            }
            ActiveToolUI::VectorPen => {
                ui.strong(tr(lang, "Vector Ink Pen (P)", "قلم الفيكتور الثابت (بدون ذيل)"));
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Stroke Width:", "سُمك الخط:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
            }
            ActiveToolUI::Brush => {
                ui.strong(tr(lang, "Brushes (Icon Presets)", "اختر الفرشة بالأيقونة"));
                ui.add_space(4.0);
                let presets = [
                    (KritaBrushPreset::GPenInk, "G-Pen"),
                    (KritaBrushPreset::Watercolor, "Watercolor"),
                    (KritaBrushPreset::ChiselMarker, "Marker"),
                    (KritaBrushPreset::SoftAirbrush, "Airbrush"),
                    (KritaBrushPreset::SketchPencil, "Pencil"),
                ];
                ui.horizontal_wrapped(|ui| {
                    for (preset, short_name) in presets {
                        let is_sel = self.active_krita_brush == preset;
                        let (btn_rect, resp) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
                        let bg = if is_sel { egui::Color32::from_rgb(0, 122, 204) } else if resp.hovered() { egui::Color32::from_rgb(62, 64, 72) } else { egui::Color32::from_rgb(44, 46, 52) };
                        ui.painter().rect_filled(btn_rect, 6.0, bg);
                        let icon_area = egui::Rect::from_min_size(btn_rect.min, egui::vec2(44.0, 28.0));
                        draw_brush_preset_icon(ui.painter(), icon_area, preset, is_sel);
                        ui.painter().text(egui::pos2(btn_rect.center().x, btn_rect.bottom() - 7.0), egui::Align2::CENTER_CENTER, short_name, egui::FontId::proportional(9.5), egui::Color32::WHITE);
                        if resp.on_hover_text(fix_ar(preset.label())).clicked() {
                            self.active_krita_brush = preset;
                            self.brush_opacity = preset.default_opacity();
                            self.play_sfx(SfxType::UiClick);
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Size:", "الحجم:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=120.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=120.0).show_value(false));
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Opacity:", "الشفافية:"));
                    ui.add(egui::Slider::new(&mut self.brush_opacity, 0.05..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Stabilizer:", "مثبت اليد:"));
                    ui.add(egui::Slider::new(&mut self.stabilizer_strength, 0.0..=0.75));
                });
            }
            ActiveToolUI::ShapeLine => {
                ui.strong(tr(lang, "Shapes & Line Tool (U)", "أداة الخطوط والأشكال"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::StraightLine, tr(lang, "Straight Line", "خط مستقيم"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Circle, tr(lang, "Circle", "دائرة"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Square, tr(lang, "Square", "مربع"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Rectangle, tr(lang, "Rectangle", "مستطيل"));
            }
            ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
                ui.strong(tr(lang, "Eraser Properties", "خصائص الممحاة"));
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserRed, tr(lang, "Red (Full E)", "حمراء (كامل E)")).clicked() { self.active_tool_ui = ActiveToolUI::EraserRed; }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserGreen, tr(lang, "Green (Part T)", "خضراء (جزئي T)")).clicked() { self.active_tool_ui = ActiveToolUI::EraserGreen; }
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 2.0..=120.0));
            }
            ActiveToolUI::Text => {
                ui.strong(tr(lang, "Text Properties", "خصائص النص"));
                ui.add(egui::DragValue::new(&mut self.text_font_size).range(10.0..=180.0));
            }
            ActiveToolUI::HandCanvas => {
                ui.strong(tr(lang, "Canvas Orientation", "تدوير وعكس الكانفا"));
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Flip H", "عكس أفقي")).clicked() { self.canvas_flip_h = !self.canvas_flip_h; }
                    if ui.button(tr(lang, "Flip V", "عكس رأسي")).clicked() { self.canvas_flip_v = !self.canvas_flip_v; }
                });
                ui.add(egui::Slider::new(&mut self.canvas_rotation_deg, -180.0..=180.0));
            }
            ActiveToolUI::Bucket | ActiveToolUI::Eyedropper => {
                ui.strong(tr(lang, "Fill & Picker (Solid Color)", "لون التعبئة (Solid)"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            }
        }
    }
}

impl eframe::App for StickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = self.config.lang;
        let now_time = ctx.input(|i| i.time);

        // الحفظ التلقائي الدوري (Auto-Save Recovery) كل 5 دقائق عند وجود تعديلات
        let autosave_secs = (self.config.autosave_interval_minutes.max(1) * 60) as f64;
        if self.last_autosave_time == 0.0 {
            self.last_autosave_time = now_time;
        } else if self.has_unsaved_changes && (now_time - self.last_autosave_time >= autosave_secs) {
            self.last_autosave_time = now_time;
            let label = self.current_project_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("unsaved_project");
            let _ = persistence::atomic_save::write_recovery_snapshot(&self.project, label);
        }

        if self.workspace_mode == WorkspaceMode::InstallerSetup {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(75.0);
                    ui.heading(egui::RichText::new("Stick Studio Setup").size(34.0).color(egui::Color32::from_rgb(255, 215, 25)));
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(fix_ar("مرحباً بك في برنامج Stick! حدد مكان تسطيب البرنامج:")).size(20.0).color(egui::Color32::WHITE));
                    ui.add_space(22.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 460.0).max(10.0) * 0.5);
                        ui.add(egui::TextEdit::singleline(&mut self.install_target_dir).desired_width(350.0));
                        if ui.button(fix_ar("استعراض...")).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.install_target_dir = folder.join("StickStudio").to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.add_space(24.0);
                    if ui.add(egui::Button::new(egui::RichText::new(fix_ar("تسطيب البرنامج وإنشاء اختصار على سطح المكتب")).size(18.0).color(egui::Color32::BLACK)).min_size(egui::vec2(340.0, 48.0)).fill(egui::Color32::from_rgb(255, 215, 25))).clicked() {
                        self.perform_full_installation();
                    }
                    ui.add_space(12.0);
                    if ui.small_button(fix_ar("تشغيل مباشر بدون تسطيب")).clicked() { self.workspace_mode = WorkspaceMode::Launcher; }
                });
            });
            return;
        }

        if self.last_xp_tick_time == 0.0 { self.last_xp_tick_time = now_time; }
        else if now_time - self.last_xp_tick_time >= 60.0 {
            let mins = ((now_time - self.last_xp_tick_time) / 60.0).floor() as u64;
            self.last_xp_tick_time += (mins as f64) * 60.0;
            self.user_profile.total_seconds_spent += mins * 60;
            self.grant_xp(mins, now_time);
        }

        if self.current_frame != self.last_scrubbed_frame {
            self.last_scrubbed_frame = self.current_frame;
            self.trigger_audio_stream_scrub();
        }

        if now_time < self.level_up_banner_timer {
            let (lvl, _, _) = self.user_profile.get_level_progress();
            let rank = self.user_profile.get_hunter_rank();
            egui::Window::new("SYSTEM_ALERT").title_bar(false).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 48.0]).show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(egui::RichText::new(format!("LEVEL {} ({})", lvl, rank)).size(22.0).color(egui::Color32::from_rgb(250, 204, 21)));
                    ui.label(egui::RichText::new(fix_ar(&self.level_up_custom_msg)).size(16.0).color(egui::Color32::WHITE));
                });
            });
            ctx.request_repaint();
        }

        if self.workspace_mode == WorkspaceMode::Launcher {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.heading(egui::RichText::new("Stick Studio").size(36.0).color(egui::Color32::from_rgb(255, 215, 25)));
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(tr(lang, "Are you here to draw or animate?", "هل أنت داخل ترسم؟")).size(24.0).color(egui::Color32::WHITE));
                    ui.add_space(30.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 440.0).max(10.0) * 0.5);
                        if ui.add(egui::Button::new(egui::RichText::new(tr(lang, "🎨 Drawing", "🎨 رسم")).size(22.0).color(egui::Color32::WHITE)).min_size(egui::vec2(205.0, 95.0)).fill(egui::Color32::from_rgb(249, 115, 22))).clicked() {
                            self.play_sfx(SfxType::NewFrame);
                            self.workspace_mode = WorkspaceMode::DrawingOnly;
                        }
                        ui.add_space(25.0);
                        if ui.add(egui::Button::new(egui::RichText::new(tr(lang, "🎬 Animation", "🎬 أنيميشن")).size(22.0).color(egui::Color32::WHITE)).min_size(egui::vec2(205.0, 95.0)).fill(egui::Color32::from_rgb(0, 122, 204))).clicked() {
                            self.play_sfx(SfxType::NewFrame);
                            self.workspace_mode = WorkspaceMode::AnimationStudio;
                        }
                    });
                });
            });
            return;
        }

        let mut patch_acknowledged = false;
        changelog::render_whats_new_modal(
            ctx,
            lang,
            &mut self.show_patch_notes_dialog,
            &mut patch_acknowledged,
        );
        if patch_acknowledged {
            self.config.last_seen_patch_version = FALLBACK_VERSION.to_string();
            self.config.save();
            self.play_sfx(SfxType::NewFrame);
        }

        let mut typing_in_textbox = ctx.wants_keyboard_input();
        let (
            now, ctrl_z, ctrl_y, ctrl_s, ctrl_shift_s, ctrl_c, ctrl_v, ctrl_k, shift_f5, del_pressed, f10_pressed,
            space_down, ctrl_down, alt_down, zoom_in_nav, zoom_out_nav, pan_right_nav, pan_left_nav,
            scroll_y, close_req, pointer_down, pointer_pressed, pointer_released, pointer_pos,
            sc_select, sc_lasso, sc_free_tr, sc_vec_sel, sc_pen, sc_brush, sc_eraser, sc_eraser_partial,
            sc_bucket, sc_eyedropper, sc_text, sc_shape, sc_hand, sc_del_frame, sc_f5, sc_f6, sc_f7, sc_f8,
            sc_play, sc_prev, sc_next, sc_smaller, sc_larger, sc_reset_canvas, pressed_key_name,
        ) = ctx.input(|i| {
            let mut captured_key = None;
            for event in &i.events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    captured_key = Some(key.name().to_string());
                }
            }
            if captured_key.is_none() && i.modifiers.shift { captured_key = Some("Shift".to_string()); }
            let z_in = (i.modifiers.ctrl && i.key_down(egui::Key::ArrowUp)) || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_down(egui::Key::W));
            let z_out = (i.modifiers.ctrl && i.key_down(egui::Key::ArrowDown)) || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_down(egui::Key::S));
            let p_right = (i.modifiers.ctrl && i.key_down(egui::Key::ArrowRight)) || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_down(egui::Key::D));
            let p_left = (i.modifiers.ctrl && i.key_down(egui::Key::ArrowLeft)) || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_down(egui::Key::A));
            (
                i.time,
                i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                (i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)),
                i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.ctrl && i.key_pressed(egui::Key::C),
                i.modifiers.ctrl && i.key_pressed(egui::Key::V),
                i.modifiers.ctrl && i.key_pressed(egui::Key::K),
                i.modifiers.shift && i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::F10),
                i.key_down(egui::Key::Space),
                i.modifiers.ctrl,
                i.modifiers.alt,
                z_in, z_out, p_right, p_left,
                i.raw_scroll_delta.y,
                i.viewport().close_requested(),
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
                is_shortcut_pressed(i, &self.config.tool_select),
                is_shortcut_pressed(i, &self.config.tool_lasso),
                is_shortcut_pressed(i, &self.config.tool_free_transform),
                is_shortcut_pressed(i, &self.config.tool_vector_select),
                is_shortcut_pressed(i, &self.config.tool_pen),
                is_shortcut_pressed(i, &self.config.tool_brush),
                is_shortcut_pressed(i, &self.config.tool_eraser),
                is_shortcut_pressed(i, &self.config.tool_eraser_partial),
                is_shortcut_pressed(i, &self.config.tool_bucket),
                is_shortcut_pressed(i, &self.config.tool_eyedropper),
                is_shortcut_pressed(i, &self.config.tool_text),
                is_shortcut_pressed(i, &self.config.tool_shape),
                is_shortcut_pressed(i, &self.config.tool_hand),
                is_shortcut_pressed(i, &self.config.delete_frame_key),
                is_shortcut_pressed(i, &self.config.insert_frame),
                is_shortcut_pressed(i, &self.config.insert_keyframe),
                is_shortcut_pressed(i, &self.config.insert_blank_kf),
                is_shortcut_pressed(i, &self.config.convert_symbol),
                is_shortcut_pressed(i, &self.config.play_pause),
                is_shortcut_pressed(i, &self.config.prev_frame),
                is_shortcut_pressed(i, &self.config.next_frame),
                is_shortcut_pressed(i, &self.config.brush_smaller),
                is_shortcut_pressed(i, &self.config.brush_larger),
                is_shortcut_pressed(i, &self.config.reset_canvas),
                captured_key,
            )
        });

        // اختصار F10: التبديل لوضع ملء الشاشة الحقيقي بدون ظهور Taskbar والرجوع للوضع الطبيعي عند الضغط عليه مرة ثانية
        if f10_pressed {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        }

        if ctrl_k { self.show_command_palette = !self.show_command_palette; }

        if let (Some(idx), Some(k_name)) = (self.listening_shortcut_idx, pressed_key_name) {
            match idx {
                0 => self.config.tool_select = k_name,
                1 => self.config.tool_lasso = k_name,
                2 => self.config.tool_free_transform = k_name,
                3 => self.config.tool_vector_select = k_name,
                4 => self.config.tool_pen = k_name,
                5 => self.config.tool_brush = k_name,
                6 => self.config.tool_eraser = k_name,
                7 => self.config.tool_eraser_partial = k_name,
                8 => self.config.tool_bucket = k_name,
                9 => self.config.tool_eyedropper = k_name,
                10 => self.config.tool_text = k_name,
                11 => self.config.tool_shape = k_name,
                12 => self.config.tool_hand = k_name,
                _ => {}
            }
            self.config.save();
            self.listening_shortcut_idx = None;
            typing_in_textbox = true;
        }

        let any_modal_open = self.show_symbol_dialog || self.show_text_dialog || self.show_export_studio
            || self.show_command_palette || self.show_exit_dialog || self.show_update_dialog
            || self.show_settings_dialog || self.show_credits_dialog || self.show_project_notice
            || self.show_recovery_dialog || self.show_project_details;
        if any_modal_open { typing_in_textbox = true; }

        if !typing_in_textbox {
            if ctrl_z { self.undo(); }
            if ctrl_y { self.redo(); }
            if ctrl_shift_s { self.trigger_save_as(); }
            else if ctrl_s { self.trigger_save(); }
            if ctrl_c { self.copy_frame(self.active_layer_idx, self.current_frame); }
            if ctrl_v { self.paste_frame(self.active_layer_idx, self.current_frame); }
            if zoom_in_nav { self.zoom_level = (self.zoom_level * 1.045).clamp(0.1, 6.0); ctx.request_repaint(); }
            if zoom_out_nav { self.zoom_level = (self.zoom_level / 1.045).clamp(0.1, 6.0); ctx.request_repaint(); }
            if pan_right_nav { self.pan_offset.x += 18.0; ctx.request_repaint(); }
            if pan_left_nav { self.pan_offset.x -= 18.0; ctx.request_repaint(); }

            if self.workspace_mode == WorkspaceMode::AnimationStudio {
                if shift_f5 || sc_del_frame { self.remove_frame_completely(self.active_layer_idx, self.current_frame); }
                else if sc_f5 { self.action_insert_frame_f5(); }
                if sc_f6 { self.action_insert_keyframe_f6(now_time); }
                if sc_f7 { self.action_insert_blank_keyframe_f7(now_time); }
                if sc_f8 { self.show_symbol_dialog = true; }
                if sc_play {
                    self.is_playing = !self.is_playing;
                    if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                }
                if sc_prev && self.current_frame > 1 { self.current_frame -= 1; }
                if sc_next && self.current_frame < self.timeline_length { self.current_frame += 1; }
            }

            if sc_reset_canvas { self.pan_offset = egui::Vec2::ZERO; self.zoom_level = 0.75; self.canvas_rotation_deg = 0.0; self.canvas_flip_h = false; self.canvas_flip_v = false; }
            if sc_select { self.active_tool_ui = ActiveToolUI::SelectNormal; }
            if sc_lasso { self.active_tool_ui = ActiveToolUI::SelectLasso; }
            if sc_free_tr { self.active_tool_ui = ActiveToolUI::FreeTransform; }
            if sc_vec_sel { self.active_tool_ui = ActiveToolUI::VectorDirectSelect; }
            if sc_pen { self.active_tool_ui = ActiveToolUI::VectorPen; }
            if sc_brush { self.active_tool_ui = ActiveToolUI::Brush; }
            if sc_eraser { self.active_tool_ui = ActiveToolUI::EraserRed; }
            if sc_eraser_partial { self.active_tool_ui = ActiveToolUI::EraserGreen; }
            if sc_bucket { self.active_tool_ui = ActiveToolUI::Bucket; }
            if sc_eyedropper { self.active_tool_ui = ActiveToolUI::Eyedropper; }
            if sc_text { self.active_tool_ui = ActiveToolUI::Text; }
            if sc_shape { self.active_tool_ui = ActiveToolUI::ShapeLine; }
            if sc_hand { self.active_tool_ui = ActiveToolUI::HandCanvas; }
            if sc_smaller { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if sc_larger { self.brush_size = (self.brush_size + 2.0).min(120.0); }
            if del_pressed && !self.selected_stroke_indices.is_empty() {
                self.save_undo_state();
                self.ensure_editable_keyframe();
                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                    if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                        let mut to_rem = self.selected_stroke_indices.clone();
                        to_rem.sort_unstable_by(|a, b| b.cmp(a));
                        to_rem.dedup();
                        for idx in to_rem { if idx < frame.strokes.len() { frame.strokes.remove(idx); } }
                        self.selected_stroke_indices.clear();
                    }
                }
            }
        }

        if self.is_playing && self.workspace_mode == WorkspaceMode::AnimationStudio {
            if now - self.last_frame_time >= 1.0 / (self.project.fps.max(1) as f64) {
                self.current_frame += 1;
                let max_b = if self.loop_enabled { self.loop_end_frame.clamp(1, self.timeline_length) } else { self.timeline_length };
                let min_b = if self.loop_enabled { self.loop_start_frame.clamp(1, max_b) } else { 1 };
                if self.current_frame > max_b { self.current_frame = min_b; self.stop_audio_playback(); self.start_audio_playback(); }
                self.last_frame_time = now;
            }
            ctx.request_repaint();
        }

        // نافذة تنبيهات وتقارير توافق المشاريع والأخطاء
        if self.show_project_notice {
            egui::Window::new(fix_ar(&self.project_notice_title))
                .collapsible(false)
                .resizable(false)
                .default_width(470.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(fix_ar(&self.project_notice_body));
                    ui.add_space(10.0);
                    if ui.button(tr(lang, "OK", "حسناً")).clicked() {
                        self.show_project_notice = false;
                    }
                });
        }

        // نافذة استرجاع النسخ الاحتياطية (File -> Recover Project)
        if self.show_recovery_dialog {
            let snapshots = persistence::atomic_save::list_recovery_snapshots();
            egui::Window::new(tr(lang, "Recover Project (Auto-Saves)", "استعادة مشروع من النسخ الاحتياطية"))
                .collapsible(false)
                .resizable(false)
                .default_width(500.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(
                        lang,
                        "Select a recovery snapshot to open safely without overwriting your original file:",
                        "اختر نسخة استرجاع لفتحها بأمان دون استبدال ملف المشروع الأصلي:",
                    ));
                    ui.separator();
                    if snapshots.is_empty() {
                        ui.label(tr(lang, "No recovery snapshots found yet.", "لا توجد نسخ استرجاع تلقائية محفوظة حالياً."));
                    } else {
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for entry in &snapshots {
                                ui.horizontal(|ui| {
                                    ui.label(format!("📄 {} ({} KB)", entry.file_name, (entry.size_bytes / 1024).max(1)));
                                    if ui.button(tr(lang, "Restore", "استعادة")).clicked() {
                                        if let Ok(outcome) = persistence::open_project_safe(&entry.path) {
                                            self.save_undo_state();
                                            self.timeline_length = outcome.project.total_frames.max(1);
                                            self.current_frame = 1;
                                            self.active_layer_idx = 0;
                                            self.project = outcome.project;
                                            // نمسح المسار النشط حتى يطلب Save As ولا يكتب فوق الأصل
                                            self.current_project_path = None;
                                            self.has_unsaved_changes = true;
                                            self.show_recovery_dialog = false;
                                            self.play_sfx(SfxType::NewFrame);
                                        }
                                    }
                                });
                            }
                        });
                    }
                    ui.separator();
                    if ui.button(tr(lang, "Close", "إغلاق")).clicked() {
                        self.show_recovery_dialog = false;
                    }
                });
        }

        // نافذة تفاصيل إصدار المشروع (Project Version Details)
        if self.show_project_details {
            let inv = persistence::migration::ProjectInventory::from_project(&self.project);
            egui::Window::new(tr(lang, "Project Version & Integrity Details", "تفاصيل إصدار المشروع وسلامة البيانات"))
                .collapsible(false)
                .resizable(false)
                .default_width(430.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("App Version: v{}", self.installed_version));
                    ui.label(format!("Project Schema Version: v{}", self.project.schema_version));
                    ui.label(format!("Canvas: {}x{} @ {} FPS", self.project.width, self.project.height, self.project.fps));
                    ui.separator();
                    ui.label(format!("Layers: {} | Keyframes: {}", inv.layer_count, inv.keyframe_count));
                    ui.label(format!("Strokes: {} | Total Points: {}", inv.stroke_count, inv.total_points));
                    ui.label(format!("Audio Tracks: {} | Symbols: {}", inv.audio_count, inv.symbol_count));
                    if let Some(ref p) = self.current_project_path {
                        ui.label(format!("Active Path: {}", p.display()));
                    } else {
                        ui.label("Active Path: (Unsaved / New Copy)");
                    }
                    ui.separator();
                    if ui.button(tr(lang, "Close", "إغلاق")).clicked() {
                        self.show_project_details = false;
                    }
                });
        }

        if self.show_export_studio {
            let task_st = self.export_task_state.lock().unwrap().clone();
            egui::Window::new(tr(lang, "Stick Export Studio", "استوديو التصدير الاحترافي")).collapsible(false).resizable(false).default_width(460.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.export_studio_tab, 0, tr(lang, "🎬 Video & GIF", "🎬 فيديو و GIF"));
                    ui.selectable_value(&mut self.export_studio_tab, 1, tr(lang, "🖼 Images", "🖼 صور"));
                    ui.selectable_value(&mut self.export_studio_tab, 2, tr(lang, "🎮 Sprite Sheet", "🎮 ألعاب"));
                    ui.selectable_value(&mut self.export_studio_tab, 3, tr(lang, "🌐 Lottie Web", "🌐 ويب Lottie"));
                });
                ui.separator();
                match task_st {
                    ExportTaskState::Running(msg) => { ui.horizontal(|ui| { ui.spinner(); ui.label(fix_ar(&msg)); }); ctx.request_repaint(); }
                    ExportTaskState::Done(msg) => {
                        ui.colored_label(egui::Color32::GREEN, fix_ar(&msg));
                        if ui.button(tr(lang, "Done", "تم")).clicked() { *self.export_task_state.lock().unwrap() = ExportTaskState::Idle; self.show_export_studio = false; }
                    }
                    ExportTaskState::Failed(err) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, fix_ar(&err));
                        if ui.button(tr(lang, "Close", "إغلاق")).clicked() { *self.export_task_state.lock().unwrap() = ExportTaskState::Idle; self.show_export_studio = false; }
                    }
                    ExportTaskState::Idle => {
                        if self.export_studio_tab == 0 {
                            for fmt in [VideoExportFormat::GifInstant, VideoExportFormat::Mp4, VideoExportFormat::Mov, VideoExportFormat::Avi, VideoExportFormat::Mkv, VideoExportFormat::Webm] {
                                ui.radio_value(&mut self.selected_export_format, fmt, fix_ar(fmt.label()));
                            }
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "🚀 Export Now...", "🚀 تصدير الآن...")).clicked() {
                                    let ext = self.selected_export_format.ext();
                                    if let Some(mut p) = rfd::FileDialog::new().add_filter("Anim", &[ext]).set_file_name(&format!("Stick.{}", ext)).save_file() {
                                        if p.extension().is_none() { p.set_extension(ext); }
                                        self.start_background_video_export(p, ctx.clone());
                                    }
                                }
                                if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() { self.show_export_studio = false; }
                            });
                        } else if self.export_studio_tab == 1 {
                            for fmt in [ImageExportFormat::PngTransparent, ImageExportFormat::PngSolid, ImageExportFormat::Jpg, ImageExportFormat::Webp, ImageExportFormat::Bmp, ImageExportFormat::Tiff] {
                                ui.radio_value(&mut self.selected_image_format, fmt, fix_ar(fmt.label()));
                            }
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "💾 Save Image...", "💾 حفظ كصورة...")).clicked() {
                                    let ext = self.selected_image_format.ext();
                                    if let Some(mut p) = rfd::FileDialog::new().add_filter("Img", &[ext]).set_file_name(&format!("Stick.{}", ext)).save_file() {
                                        if p.extension().is_none() { p.set_extension(ext); }
                                        if exporter::export_single_image_format(&self.project, self.current_frame, &p, self.selected_image_format, self.stage_bg_color).is_ok() {
                                            self.play_sfx(SfxType::ExportSuccess);
                                            *self.export_task_state.lock().unwrap() = ExportTaskState::Done("تم حفظ الصورة!".to_string());
                                        }
                                    }
                                }
                                if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_export_studio = false; }
                            });
                        } else if self.export_studio_tab == 2 {
                            if ui.button(tr(lang, "🎮 Export Sprite Sheet...", "🎮 تصدير Sprite Sheet...")).clicked() {
                                self.project.total_frames = self.timeline_length;
                                if let Some(mut p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("SpriteSheet.png").save_file() {
                                    if p.extension().is_none() { p.set_extension("png"); }
                                    if exporter::export_sprite_sheet(&self.project, &p).is_ok() {
                                        *self.export_task_state.lock().unwrap() = ExportTaskState::Done("تم تصدير Sprite Sheet!".to_string());
                                    }
                                }
                            }
                            if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_export_studio = false; }
                        } else {
                            if ui.button(tr(lang, "🌐 Export Lottie JSON...", "🌐 تصدير Lottie JSON...")).clicked() {
                                self.project.total_frames = self.timeline_length;
                                if let Some(mut p) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("Lottie.json").save_file() {
                                    if p.extension().is_none() { p.set_extension("json"); }
                                    if exporter::export_lottie_json(&self.project, &p).is_ok() {
                                        *self.export_task_state.lock().unwrap() = ExportTaskState::Done("تم تصدير Lottie JSON!".to_string());
                                    }
                                }
                            }
                            if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_export_studio = false; }
                        }
                    }
                }
            });
        }

        if self.show_text_dialog {
            egui::Window::new(tr(lang, "Insert Text", "إضافة نص")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.text_input_buf);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Add", "إضافة")).clicked() {
                        if !self.text_input_buf.trim().is_empty() {
                            self.save_undo_state();
                            self.ensure_editable_keyframe();
                            if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                l.keyframes.entry(self.current_frame).or_default().text_items.push(TextItem { content: self.text_input_buf.clone(), x: self.pending_text_pos.x, y: self.pending_text_pos.y, font_size: self.text_font_size, color: self.brush_color });
                                self.has_unsaved_changes = true;
                            }
                        }
                        self.text_input_buf.clear();
                        self.show_text_dialog = false;
                    }
                    if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() { self.show_text_dialog = false; }
                });
            });
        }

        if self.show_settings_dialog {
            egui::Window::new(tr(lang, "Settings & System", "الإعدادات والنظام")).collapsible(false).resizable(false).default_width(460.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings_tab, 0, tr(lang, "General & Auto-Save", "عام والحفظ التلقائي"));
                    ui.selectable_value(&mut self.settings_tab, 1, "[ SYSTEM ] Status");
                });
                ui.separator();
                if self.settings_tab == 0 {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.config.lang == AppLang::English, "English").clicked() { self.config.lang = AppLang::English; self.config.save(); }
                        if ui.selectable_label(self.config.lang == AppLang::Arabic, fix_ar("العربية")).clicked() { self.config.lang = AppLang::Arabic; self.config.save(); }
                    });
                    if ui.checkbox(&mut self.config.sound_effects_enabled, tr(lang, "Sound Effects", "المؤثرات الصوتية")).changed() { self.config.save(); }
                    if ui.checkbox(&mut self.config.show_grid, tr(lang, "Show Grid", "إظهار المربعات")).changed() { self.config.save(); }
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Auto-Save Interval (mins):", "فترة الحفظ التلقائي (دقائق):"));
                        if ui.add(egui::DragValue::new(&mut self.config.autosave_interval_minutes).range(1..=60)).changed() {
                            self.config.save();
                        }
                    });
                } else {
                    let (lvl, cxp, rxp) = self.user_profile.get_level_progress();
                    ui.heading(format!("LEVEL {} / 100  ({})", lvl, self.user_profile.get_hunter_rank()));
                    ui.add(egui::ProgressBar::new((cxp as f32 / rxp.max(1) as f32).clamp(0.0, 1.0)));
                }
                if ui.button(tr(lang, "Done", "تم")).clicked() { self.show_settings_dialog = false; }
            });
        }

        if self.show_credits_dialog {
            egui::Window::new("Stick").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.heading("Made by Hady");
                ui.hyperlink_to("YouTube Channel", YOUTUBE_CHANNEL_URL);
                if ui.button("Close").clicked() { self.show_credits_dialog = false; }
            });
        }

        if self.show_update_dialog {
            let st = self.update_state.lock().unwrap().clone();
            egui::Window::new("Updater").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                match st {
                    UpdateState::Checking => { ui.spinner(); }
                    UpdateState::UpToDate => { ui.label("Up to date!"); if ui.button("Close").clicked() { self.show_update_dialog = false; } }
                    UpdateState::UpdateAvailable { version, download_url } => {
                        if ui.button(format!("Install v{}", version)).clicked() { self.download_and_apply_update(version, download_url, ctx.clone()); }
                    }
                    UpdateState::Downloading(m) => { ui.label(m); }
                    UpdateState::ReadyToRestart => { if ui.button("Restart").clicked() { self.restart_with_new_exe(); } }
                    UpdateState::Error(e) => { ui.label(e); if ui.button("Close").clicked() { self.show_update_dialog = false; } }
                    UpdateState::Idle => {}
                }
            });
        }

        if self.show_symbol_dialog {
            egui::Window::new("Symbol").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.new_symbol_name);
                if ui.button("Create").clicked() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                            let mut nested = HashMap::new();
                            nested.insert(1, f.strokes.clone());
                            self.project.library_symbols.insert(self.new_symbol_name.clone(), Symbol { name: self.new_symbol_name.clone(), strokes: f.strokes.clone(), nested_keyframes: nested, nested_total_frames: 1 });
                            f.strokes.clear();
                            f.symbol_instances.push(SymbolInstance { symbol_name: self.new_symbol_name.clone(), x: 0.0, y: 0.0, scale: 1.0, rotation_deg: 0.0, anchor_offset_x: 0.0, anchor_offset_y: 0.0 });
                            self.has_unsaved_changes = true;
                        }
                    }
                    self.show_symbol_dialog = false;
                }
            });
        }

        if close_req && !self.allow_close {
            if self.has_unsaved_changes {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_exit_dialog = true;
            } else {
                play_sfx_async(self.config.sound_effects_enabled, SfxType::ExitFarewell, true);
            }
        }

        if self.show_exit_dialog {
            egui::Window::new("Exit").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Save & Exit").clicked() && self.trigger_save() { self.allow_close = true; ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                    if ui.button("Exit").clicked() { self.allow_close = true; ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
            });
        }

        // الشريط العلوي
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let (h_rect, h_resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
                draw_red_heart_icon(ui.painter(), h_rect, h_resp.hovered());
                if h_resp.clicked() { self.show_credits_dialog = true; }
                ui.colored_label(egui::Color32::from_rgb(255, 215, 25), "Stick");
                ui.separator();
                let m_lbl = if self.workspace_mode == WorkspaceMode::DrawingOnly { tr(lang, "Switch to Animation", "التبديل للأنيميشن") } else { tr(lang, "Switch to Drawing", "التبديل للرسم") };
                if ui.button(m_lbl).clicked() {
                    self.workspace_mode = if self.workspace_mode == WorkspaceMode::DrawingOnly { WorkspaceMode::AnimationStudio } else { WorkspaceMode::DrawingOnly };
                }
                ui.separator();
                ui.menu_button(tr(lang, "File", "ملف"), |ui| {
                    if ui.button(tr(lang, "Save (Ctrl+S)", "حفظ (Ctrl+S)")).clicked() { self.trigger_save(); ui.close_menu(); }
                    if ui.button(tr(lang, "Save As... (Ctrl+Shift+S)", "حفظ باسم...")).clicked() { self.trigger_save_as(); ui.close_menu(); }
                    if ui.button(tr(lang, "Open (.stick / .ah2d)", "فتح مشروع")).clicked() { self.trigger_open_project(); ui.close_menu(); }
                    if ui.button(tr(lang, "Recover Project...", "استعادة نسخة احتياطية...")).clicked() { self.show_recovery_dialog = true; ui.close_menu(); }
                    if ui.button(tr(lang, "Project Details", "تفاصيل إصدار المشروع")).clicked() { self.show_project_details = true; ui.close_menu(); }
                    ui.separator();
                    if ui.button(tr(lang, "Import Image...", "استيراد صورة")).clicked() { self.import_image_dialog(); ui.close_menu(); }
                    if ui.button(tr(lang, "Import SVG...", "استيراد SVG")).clicked() { self.import_svg_dialog(); ui.close_menu(); }
                    if ui.button(tr(lang, "Import Adobe Animate...", "استيراد Animate")).clicked() { self.import_animate_dialog(); ui.close_menu(); }
                    if ui.button(tr(lang, "🚀 Export Studio...", "🚀 استوديو التصدير")).clicked() { self.show_export_studio = true; ui.close_menu(); }
                });
                let can_u = !self.undo_stack.is_empty();
                let (ur, uresp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), egui::Sense::click());
                draw_undo_redo_arrow(ui.painter(), ur, true, can_u);
                if uresp.clicked() && can_u { self.undo(); }

                let can_r = !self.redo_stack.is_empty();
                let (rr, rresp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), egui::Sense::click());
                draw_undo_redo_arrow(ui.painter(), rr, false, can_r);
                if rresp.clicked() && can_r { self.redo(); }

                ui.separator();
                if ui.button(tr(lang, "+ Image", "+ صورة")).clicked() { self.import_image_dialog(); }
                if ui.button("+ SVG").clicked() { self.import_svg_dialog(); }
                if self.workspace_mode == WorkspaceMode::AnimationStudio {
                    if ui.button(tr(lang, "+ Video", "+ فيديو")).clicked() { self.import_video_dialog(); }
                    if ui.button(tr(lang, "+ Audio", "+ صوت")).clicked() { self.import_audio_dialog(); }
                }
                if ui.button(egui::RichText::new(tr(lang, "Export Studio", "استوديو التصدير")).color(egui::Color32::from_rgb(56, 189, 248))).clicked() {
                    self.show_export_studio = true;
                }
                if ui.button(tr(lang, "Updates", "تحديثات")).clicked() { self.check_for_updates(ctx.clone()); }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (gr, gresp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                    draw_gear_icon(ui.painter(), gr, 1.0);
                    if gresp.clicked() { self.show_settings_dialog = true; }
                    if ui.small_button("F10 Fullscreen").clicked() {
                        self.is_fullscreen = !self.is_fullscreen;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
                    }
                    if ui.small_button("+15%").clicked() { self.zoom_level = (self.zoom_level * 1.15).clamp(0.1, 6.0); }
                    ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                    if ui.small_button("-15%").clicked() { self.zoom_level = (self.zoom_level / 1.15).clamp(0.1, 6.0); }
                    if ui.small_button("Center (R)").clicked() { self.zoom_level = 0.75; self.pan_offset = egui::Vec2::ZERO; }
                });
            });
        });

        // شريط الأدوات الأيسر
        egui::SidePanel::left("tools_panel").resizable(true).default_width(60.0).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                let tools = [
                    (ActiveToolUI::SelectNormal, "Select (V)"),
                    (ActiveToolUI::FreeTransform, "Transform (Q)"),
                    (ActiveToolUI::VectorPen, "Ink Pen (P)"),
                    (ActiveToolUI::Brush, "Brushes (B)"),
                    (ActiveToolUI::EraserRed, "Eraser (E)"),
                    (ActiveToolUI::EraserGreen, "Part Eraser (T)"),
                    (ActiveToolUI::Bucket, "Bucket Fill (K)"),
                    (ActiveToolUI::Eyedropper, "Picker (I)"),
                    (ActiveToolUI::Text, "Text (N)"),
                    (ActiveToolUI::ShapeLine, "Shapes (U)"),
                    (ActiveToolUI::HandCanvas, "Hand (H)"),
                ];
                for (tool, tip) in tools {
                    let sel = self.active_tool_ui == tool;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 30.0), egui::Sense::click());
                    let bg = if sel { get_tool_ui_color(tool) } else if resp.hovered() { egui::Color32::from_rgb(62, 62, 68) } else { egui::Color32::from_rgb(44, 44, 48) };
                    ui.painter().rect_filled(rect, 6.0, bg);
                    draw_tool_ui_icon(ui.painter(), rect, tool, sel);
                    if resp.on_hover_text(tip).clicked() { self.active_tool_ui = tool; }
                }
                ui.separator();
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            });
        });

        // اللوحة اليمنى
        egui::SidePanel::right("right_panel").resizable(true).default_width(270.0).show(ctx, |ui| {
            self.render_tool_properties_ui(ui, lang);
            ui.add_space(8.0);
            ui.separator();
            ui.strong(tr(lang, "Symmetry Mirroring", "الرسم المتماثل"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Off, "Off");
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Horizontal, "H");
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Vertical, "V");
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.strong(tr(lang, "Layers", "الطبقات"));
                if ui.small_button("+ Layer").clicked() {
                    self.save_undo_state();
                    let mut kf = HashMap::new();
                    kf.insert(1, Default::default());
                    self.project.layers.insert(0, Layer { name: format!("Layer_{}", self.project.layers.len() + 1), visible: true, locked: false, is_guide: false, opacity: 1.0, keyframes: kf });
                    self.active_layer_idx = 0;
                    self.has_unsaved_changes = true;
                }
                if self.project.layers.len() > 1 && self.active_layer_idx + 1 < self.project.layers.len() {
                    if ui.small_button("Merge ↓").clicked() {
                        self.save_undo_state();
                        let top = self.project.layers.remove(self.active_layer_idx);
                        if let Some(below) = self.project.layers.get_mut(self.active_layer_idx) {
                            if let Some(tfd) = top.keyframes.get(&1) { below.keyframes.entry(1).or_default().strokes.extend(tfd.strokes.clone()); }
                        }
                        self.has_unsaved_changes = true;
                    }
                }
            });
            if let Some(al) = self.project.layers.get_mut(self.active_layer_idx) {
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    ui.add(egui::Slider::new(&mut al.opacity, 0.05..=1.0));
                });
            }
            for (idx, layer) in self.project.layers.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layer.visible, "");
                    if ui.small_button(if layer.locked { "🔒" } else { "🔓" }).clicked() { layer.locked = !layer.locked; }
                    if ui.selectable_label(self.active_layer_idx == idx, &layer.name).clicked() { self.active_layer_idx = idx; }
                });
            }
        });

        // شريط التايم لاين السفلي في وضع الأنيميشن
        if self.workspace_mode == WorkspaceMode::AnimationStudio {
            egui::TopBottomPanel::bottom("timeline_panel").resizable(true).default_height(200.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("|<").clicked() { self.current_frame = 1; }
                    if ui.button("<").clicked() && self.current_frame > 1 { self.current_frame -= 1; }
                    if ui.button(if self.is_playing { "Pause" } else { "Play" }).clicked() {
                        self.is_playing = !self.is_playing;
                        if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                    }
                    if ui.button(">").clicked() && self.current_frame < self.timeline_length { self.current_frame += 1; }
                    ui.toggle_value(&mut self.loop_enabled, "🔁 Loop");
                    ui.label(format!("{}f / {}f", self.current_frame, self.timeline_length));
                    if ui.button("Frame (F5)").clicked() { self.action_insert_frame_f5(); }
                    if ui.button("Keyframe (F6)").clicked() { self.action_insert_keyframe_f6(now_time); }
                    if ui.button("Blank (F7)").clicked() { self.action_insert_blank_keyframe_f7(now_time); }
                    ui.checkbox(&mut self.project.onion_skin.enabled, "Onion Skin");
                });
                ui.separator();
                let cell_w = self.timeline_cell_width;
                let cols = self.timeline_length.max(30);
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(140.0, 20.0), |ui| { ui.strong("Timeline"); });
                        for f in 1..=cols {
                            let is_c = self.current_frame == f;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(cell_w, 20.0), egui::Sense::click_and_drag());
                            if is_c { ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(225, 55, 55)); }
                            if is_c || f == 1 || f % 5 == 0 {
                                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{}", f), egui::FontId::proportional(11.0), egui::Color32::WHITE);
                            }
                            if resp.clicked() || (pointer_down && resp.hovered()) {
                                if f > self.timeline_length { self.timeline_length = f; self.project.total_frames = f; }
                                self.current_frame = f;
                            }
                        }
                    });
                    for (l_idx, layer) in self.project.layers.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(140.0, 22.0), |ui| { ui.label(&layer.name); });
                            for f in 1..=cols {
                                let is_b = f > self.timeline_length;
                                let is_kf = !is_b && layer.keyframes.contains_key(&f);
                                let eff = if is_b { None } else { Self::get_effective_frame_data(layer, f) };
                                let has_d = eff.map_or(false, |fd| !fd.strokes.is_empty() || !fd.media_items.is_empty());
                                let tw = eff.map(|fd| fd.tween_mode).unwrap_or(TweenMode::None);
                                let (cr, cresp) = ui.allocate_exact_size(egui::vec2(cell_w, 22.0), egui::Sense::click());
                                let bg = if self.active_layer_idx == l_idx && self.current_frame == f { egui::Color32::from_rgb(0, 122, 204) } else if has_d { egui::Color32::from_rgb(65, 68, 76) } else { egui::Color32::from_rgb(40, 42, 48) };
                                ui.painter().rect_filled(cr, 2.0, bg);
                                if !is_b { draw_anicraft_timeline_cell(ui.painter(), cr, is_kf, has_d, tw); }
                                if cresp.clicked() {
                                    if is_b { self.timeline_length = f; self.project.total_frames = f; }
                                    self.active_layer_idx = l_idx;
                                    self.current_frame = f;
                                }
                            }
                        });
                    }
                });
            });
        }

        self.preload_frame_textures(ctx);

        // مساحة العمل والكانفا
        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(canvas_rect);
            let center = canvas_rect.center() + self.pan_offset;
            let scale = self.zoom_level.max(0.1);
            let (stage_w, stage_h) = (self.project.width as f32, self.project.height as f32);
            let stage_rect = egui::Rect::from_center_size(center, egui::vec2(stage_w, stage_h) * scale);

            let is_inside_stage = pointer_pos.map_or(false, |p| stage_rect.contains(p) && canvas_rect.contains(p));
            let show_dot = is_inside_stage && !any_modal_open && !space_down && !alt_down && self.active_tool_ui != ActiveToolUI::HandCanvas;
            ctx.set_cursor_icon(if show_dot { egui::CursorIcon::None } else if space_down || self.active_tool_ui == ActiveToolUI::HandCanvas { egui::CursorIcon::Grab } else { egui::CursorIcon::Default });

            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));
            if scroll_y != 0.0 && is_over_canvas && !any_modal_open {
                self.zoom_level = (self.zoom_level * (1.0 + scroll_y * 0.0022).clamp(0.75, 1.35)).clamp(0.1, 6.0);
            }
            if (space_down || self.active_tool_ui == ActiveToolUI::HandCanvas) && pointer_down && !any_modal_open {
                if let (Some(c), Some(l)) = (pointer_pos, self.last_pointer_pos) { self.pan_offset += c - l; }
            }

            let bg = self.stage_bg_color;
            painter.rect_filled(stage_rect, 0.0, egui::Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]));

            let (sin_r, cos_r) = self.canvas_rotation_deg.to_radians().sin_cos();
            let (fh, fv) = (self.canvas_flip_h, self.canvas_flip_v);
            let to_screen = |p: egui::Pos2| -> egui::Pos2 {
                let mut nx = p.x - stage_w * 0.5;
                let mut ny = p.y - stage_h * 0.5;
                if fh { nx = -nx; }
                if fv { ny = -ny; }
                egui::pos2(center.x + (nx * cos_r - ny * sin_r) * scale, center.y + (nx * sin_r + ny * cos_r) * scale)
            };
            let to_stage = |sp: egui::Pos2| -> egui::Pos2 {
                let (rx, ry) = ((sp.x - center.x) / scale, (sp.y - center.y) / scale);
                let mut nx = rx * cos_r + ry * sin_r;
                let mut ny = -rx * sin_r + ry * cos_r;
                if fh { nx = -nx; }
                if fv { ny = -ny; }
                egui::pos2(nx + stage_w * 0.5, ny + stage_h * 0.5)
            };

            if !space_down && !alt_down && self.active_tool_ui != ActiveToolUI::HandCanvas && is_over_canvas && !any_modal_open && !self.is_active_layer_locked() {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |l| (pos - l) / scale);
                    match self.active_tool_ui {
                        ActiveToolUI::Text if pointer_pressed => { self.pending_text_pos = stage_pos; self.show_text_dialog = true; }
                        ActiveToolUI::ShapeLine => {
                            if pointer_pressed { self.shape_drag_start = Some(stage_pos); self.shape_drag_current = Some(stage_pos); }
                            else if pointer_down { self.shape_drag_current = Some(stage_pos); }
                        }
                        ActiveToolUI::Bucket if pointer_pressed => {
                            self.save_undo_state();
                            self.ensure_editable_keyframe();
                            if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                                    for s in f.strokes.iter_mut().rev() {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 35.0) {
                                            s.fill_color = Some(self.brush_color);
                                            self.has_unsaved_changes = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen if pointer_down => {
                            if pointer_pressed { self.save_undo_state(); }
                            self.ensure_editable_keyframe();
                            if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                                    let r = self.brush_size;
                                    if self.active_tool_ui == ActiveToolUI::EraserRed {
                                        f.strokes.retain(|s| !s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < r));
                                    } else {
                                        erase_partial_from_strokes(&mut f.strokes, stage_pos, r);
                                    }
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        ActiveToolUI::Eyedropper if pointer_pressed => {
                            if let Some(l) = self.project.layers.get(self.active_layer_idx) {
                                let f = self.project.evaluate_layer_at_frame(l, self.current_frame);
                                for s in &f.strokes {
                                    if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 15.0) {
                                        self.brush_color = s.color;
                                        break;
                                    }
                                }
                            }
                        }
                        ActiveToolUI::VectorPen if pointer_down => {
                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    if ((last.x - stage_pos.x).powi(2) + (last.y - stage_pos.y).powi(2)).sqrt() >= 4.0 {
                                        stroke.points.push(StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure: 1.0 });
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke { points: vec![StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure: 1.0 }], color: self.brush_color, fill_color: None, base_width: self.brush_size, is_vector: true, brush_preset: KritaBrushPreset::GPenInk, opacity: 1.0, brush_texture_id: None });
                            }
                        }
                        ActiveToolUI::Brush if pointer_down => {
                            let stab = self.stabilizer_strength.clamp(0.0, 0.70);
                            let virt = if let Some(pv) = self.stabilized_cursor {
                                egui::pos2(pv.x * stab + stage_pos.x * (1.0 - stab), pv.y * stab + stage_pos.y * (1.0 - stab))
                            } else { stage_pos };
                            self.stabilized_cursor = Some(virt);
                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    if ((last.x - virt.x).powi(2) + (last.y - virt.y).powi(2)).sqrt() >= 1.5 {
                                        stroke.points.push(StrokePoint { x: virt.x, y: virt.y, pressure: 1.0 });
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke { points: vec![StrokePoint { x: virt.x, y: virt.y, pressure: 1.0 }], color: self.brush_color, fill_color: None, base_width: self.brush_size, is_vector: false, brush_preset: self.active_krita_brush, opacity: self.brush_opacity, brush_texture_id: None });
                            }
                        }
                        ActiveToolUI::SelectNormal | ActiveToolUI::FreeTransform => {
                            if pointer_pressed {
                                self.selected_stroke_indices.clear();
                                if let Some(l) = self.project.layers.get(self.active_layer_idx) {
                                    let f = self.project.evaluate_layer_at_frame(l, self.current_frame);
                                    for (i, s) in f.strokes.iter().enumerate() {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 20.0) {
                                            self.selected_stroke_indices.push(i);
                                            break;
                                        }
                                    }
                                }
                            } else if pointer_down && delta.length_sq() > 0.0 {
                                self.ensure_editable_keyframe();
                                if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                    if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                                        for &idx in &self.selected_stroke_indices {
                                            if let Some(s) = f.strokes.get_mut(idx) {
                                                for p in &mut s.points { p.x += delta.x; p.y += delta.y; }
                                            }
                                        }
                                        self.has_unsaved_changes = true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            if pointer_released {
                self.stabilized_cursor = None;
                if let (Some(p0), Some(p1)) = (self.shape_drag_start.take(), self.shape_drag_current.take()) {
                    if p0.distance(p1) > 3.0 {
                        self.save_undo_state();
                        self.commit_stroke_with_symmetry(build_geometric_stroke(self.selected_shape_kind, p0, p1, self.brush_color, self.brush_size), now_time);
                    }
                }
                if let Some(mut stroke) = self.current_stroke.take() {
                    if stroke.is_vector {
                        stroke.simplify_vector_nodes(2.2);
                        stroke.fit_cubic_bezier_spline(8);
                    } else {
                        stroke.fit_cubic_bezier_spline(4);
                    }
                    self.commit_stroke_with_symmetry(stroke, now_time);
                }
            }

            self.last_pointer_pos = if pointer_down { pointer_pos } else { None };

            for layer in self.project.layers.iter().rev() {
                if layer.visible {
                    let l_op = layer.opacity.clamp(0.05, 1.0);
                    let fd = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                    for m in &fd.media_items {
                        if let Some(tex) = self.texture_cache.get(&m.path) {
                            painter.image(tex.id(), egui::Rect::from_min_max(to_screen(egui::pos2(m.x, m.y)), to_screen(egui::pos2(m.x + m.width, m.y + m.height))), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::from_white_alpha((255.0 * l_op) as u8));
                        }
                    }
                    for s in &fd.strokes { draw_stroke_pro(&painter, s, None, l_op, scale, 0.0, 0.0, &to_screen); }
                    for txt in &fd.text_items {
                        painter.text(to_screen(egui::pos2(txt.x, txt.y)), egui::Align2::LEFT_TOP, fix_ar(&txt.content), egui::FontId::proportional(txt.font_size * scale), egui::Color32::from_rgba_unmultiplied(txt.color[0], txt.color[1], txt.color[2], (txt.color[3] as f32 * l_op) as u8));
                    }
                }
            }

            if let Some(stroke) = &self.current_stroke { draw_stroke_pro(&painter, stroke, None, 1.0, scale, 0.0, 0.0, &to_screen); }
            if let (Some(p0), Some(p1)) = (self.shape_drag_start, self.shape_drag_current) {
                draw_stroke_pro(&painter, &build_geometric_stroke(self.selected_shape_kind, p0, p1, self.brush_color, self.brush_size), None, 1.0, scale, 0.0, 0.0, &to_screen);
            }
            if show_dot {
                if let Some(p) = pointer_pos {
                    painter.circle_filled(p, 3.5, egui::Color32::WHITE);
                    painter.circle_stroke(p, 4.3, egui::Stroke::new(1.2, egui::Color32::BLACK));
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Stick").with_inner_size([1366.0, 768.0]).with_icon(generate_stick_window_icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("Stick", options, Box::new(|cc| Ok(Box::new(StickApp::new(cc)))))
}
