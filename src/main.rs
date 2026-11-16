#![windows_subsystem = "windows"]

mod exporter;
mod profile;
mod project;

use arabic_reshaper::arabic_reshape;
use eframe::egui;
use exporter::VideoExportFormat;
use profile::UserProfile;
use project::{
    AniProject, AudioSyncMode, AudioTrack, FrameData, Layer, MediaItem, MediaType, Stroke, StrokePoint, Symbol,
    SymbolInstance, TextItem, ToolType,
};
use rodio::source::{SineWave, TakeDuration};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use unicode_bidi::BidiInfo;

const FALLBACK_VERSION: &str = "2.2.0";
const GITHUB_REPO: &str = "KaniEGY/AniHady2D";
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
                    SfxType::UiClick => {
                        sink.append(SineWave::new(740.0).take_duration(Duration::from_millis(28)).amplify(0.12));
                    }
                    SfxType::NewFrame => {
                        sink.append(SineWave::new(620.0).take_duration(Duration::from_millis(50)).amplify(0.18));
                        sink.append(SineWave::new(920.0).take_duration(Duration::from_millis(55)).amplify(0.18));
                    }
                    SfxType::VectorPenTick => {
                        sink.append(SineWave::new(1180.0).take_duration(Duration::from_millis(22)).amplify(0.09));
                    }
                    SfxType::BrushScratch => {
                        sink.append(SineWave::new(290.0).take_duration(Duration::from_millis(32)).amplify(0.09));
                    }
                    SfxType::EraserRub => {
                        sink.append(SineWave::new(175.0).take_duration(Duration::from_millis(45)).amplify(0.13));
                    }
                    SfxType::LassoSelect => {
                        sink.append(SineWave::new(540.0).take_duration(Duration::from_millis(35)).amplify(0.14));
                        sink.append(SineWave::new(810.0).take_duration(Duration::from_millis(45)).amplify(0.14));
                    }
                    SfxType::ZoomStep => {
                        sink.append(SineWave::new(660.0).take_duration(Duration::from_millis(25)).amplify(0.10));
                    }
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

fn contains_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

fn fix_ar(text: &str) -> String {
    if !contains_arabic(text) {
        return text.to_string();
    }
    let reshaped = arabic_reshape(text);
    let bidi_info = BidiInfo::new(&reshaped, None);
    if bidi_info.paragraphs.is_empty() {
        return reshaped;
    }
    let para = &bidi_info.paragraphs[0];
    let line = para.range.clone();
    bidi_info.reorder_line(para, line).into_owned()
}

fn tr(lang: AppLang, en: &str, ar: &str) -> String {
    match lang {
        AppLang::English => en.to_string(),
        AppLang::Arabic => fix_ar(ar),
    }
}

fn get_installed_version() -> String {
    if let Ok(exe) = std::env::current_exe() {
        let ver_path = exe.with_file_name("version.txt");
        if let Ok(v) = std::fs::read_to_string(ver_path) {
            let trimmed = v.trim().trim_start_matches('v').to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    FALLBACK_VERSION.to_string()
}

fn save_installed_version(ver: &str) {
    if let Ok(exe) = std::env::current_exe() {
        let ver_path = exe.with_file_name("version.txt");
        let _ = std::fs::write(ver_path, ver.trim().trim_start_matches('v'));
    }
}

fn is_running_from_installed_dir() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("stick_installed.marker").exists() {
                return true;
            }
        }
    }
    false
}

fn create_desktop_shortcut(target_exe: &Path) -> Result<(), String> {
    let ps_cmd = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Desktop = [Environment]::GetFolderPath('Desktop'); \
         $Shortcut = $WshShell.CreateShortcut(\"$Desktop\\Stick.lnk\"); \
         $Shortcut.TargetPath = '{}'; \
         $Shortcut.WorkingDirectory = '{}'; \
         $Shortcut.IconLocation = '{},0'; \
         $Shortcut.Description = 'Stick 2D Animation & Drawing Studio'; \
         $Shortcut.Save()",
        target_exe.display().to_string().replace('\'', "''"),
        target_exe.parent().unwrap_or(Path::new(".")).display().to_string().replace('\'', "''"),
        target_exe.display().to_string().replace('\'', "''")
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("فشل إنشاء اختصار سطح المكتب.".to_string())
    }
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
    #[serde(default = "default_true_val")]
    sound_effects_enabled: bool,
    #[serde(default)]
    show_grid: bool,
    #[serde(default = "default_grid_size")]
    grid_size: f32,
    #[serde(default)]
    custom_blender_path: Option<String>,
    #[serde(default = "default_tl_bg")]
    timeline_bg_color: [u8; 4],
    #[serde(default = "default_tl_cell")]
    timeline_cell_color: [u8; 4],
    #[serde(default = "default_tl_head")]
    timeline_playhead_color: [u8; 4],
    tool_select: String,
    #[serde(default = "default_lasso_key")]
    tool_lasso: String,
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
fn default_tl_bg() -> [u8; 4] { [32, 32, 36, 255] }
fn default_tl_cell() -> [u8; 4] { [42, 42, 46, 255] }
fn default_tl_head() -> [u8; 4] { [210, 50, 50, 255] }
fn default_lasso_key() -> String { "L".to_string() }
fn default_vec_sel_key() -> String { "Q".to_string() }
fn default_partial_eraser_key() -> String { "T".to_string() }
fn default_text_key() -> String { "N".to_string() }
fn default_shape_key() -> String { "U".to_string() }
fn default_hand_key() -> String { "H".to_string() }
fn default_del_frame_key() -> String { "X".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lang: AppLang::Arabic,
            sound_effects_enabled: true,
            show_grid: false,
            grid_size: 40.0,
            custom_blender_path: None,
            timeline_bg_color: default_tl_bg(),
            timeline_cell_color: default_tl_cell(),
            timeline_playhead_color: default_tl_head(),
            tool_select: "V".to_string(),
            tool_lasso: "L".to_string(),
            tool_vector_select: "Q".to_string(),
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
            let path = exe.with_file_name("stick_config.json");
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str(&data) {
                    return parsed;
                }
            }
        }
        Self::default()
    }

    fn save(&self) {
        if let Ok(exe) = std::env::current_exe() {
            let path = exe.with_file_name("stick_config.json");
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

fn is_shortcut_pressed(i: &egui::InputState, key_name: &str) -> bool {
    if key_name.is_empty() {
        return false;
    }
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

            let half_len = 26.0;
            let half_thick = 5.0;
            let border = 1.6;

            if u.abs() <= half_len && v.abs() <= half_thick {
                if u.abs() >= (half_len - border) || v.abs() >= (half_thick - border) {
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
    style.visuals.panel_fill = egui::Color32::from_rgb(32, 32, 36);
    style.visuals.window_fill = egui::Color32::from_rgb(38, 38, 43);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(18, 18, 21);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 122, 204);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 48, 53);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(65, 65, 72);
    style.spacing.item_spacing = egui::vec2(5.0, 5.0);
    style.spacing.button_padding = egui::vec2(7.0, 4.0);
    ctx.set_style(style);
}

fn draw_red_heart_icon(painter: &egui::Painter, rect: egui::Rect, hovered: bool) {
    let c = rect.center();
    let red = if hovered {
        egui::Color32::from_rgb(255, 75, 95)
    } else {
        egui::Color32::from_rgb(235, 35, 55)
    };
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
    let base_col = if is_undo {
        egui::Color32::from_rgb(239, 68, 68)
    } else {
        egui::Color32::from_rgb(234, 179, 8)
    };
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
    let accent = get_tool_ui_color(tool);
    let main_color = if active { egui::Color32::WHITE } else { accent };
    let stroke = egui::Stroke::new(2.0, main_color);

    match tool {
        ActiveToolUI::SelectNormal => {
            let pts = vec![
                c + egui::vec2(-5.5, -8.5),
                c + egui::vec2(-5.5, 7.5),
                c + egui::vec2(-1.2, 3.8),
                c + egui::vec2(2.5, 9.5),
                c + egui::vec2(5.2, 8.0),
                c + egui::vec2(1.5, 2.2),
                c + egui::vec2(7.0, 2.2),
            ];
            painter.add(egui::Shape::convex_polygon(pts, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::SelectLasso => {
            painter.circle_stroke(c + egui::vec2(-1.0, -2.0), 6.5, stroke);
            painter.line_segment([c + egui::vec2(3.5, 3.0), c + egui::vec2(7.5, 8.5)], stroke);
        }
        ActiveToolUI::VectorDirectSelect => {
            let pts = vec![
                c + egui::vec2(-4.5, -8.5),
                c + egui::vec2(-4.5, 6.5),
                c + egui::vec2(0.0, 3.0),
                c + egui::vec2(5.5, 3.0),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(6.0, -5.0), 2.5, main_color);
        }
        ActiveToolUI::VectorPen => {
            let pts = vec![
                c + egui::vec2(0.0, -9.5),
                c + egui::vec2(6.5, 1.0),
                c + egui::vec2(3.5, 8.5),
                c + egui::vec2(-3.5, 8.5),
                c + egui::vec2(-6.5, 1.0),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(0.0, -0.5), 2.0, main_color);
            painter.line_segment([c + egui::vec2(0.0, -9.5), c + egui::vec2(0.0, -2.5)], stroke);
        }
        ActiveToolUI::Brush => {
            painter.line_segment([c + egui::vec2(-1.5, 1.5), c + egui::vec2(7.5, -7.5)], egui::Stroke::new(3.2, main_color));
            let tip = vec![
                c + egui::vec2(-2.0, 2.0),
                c + egui::vec2(-7.5, 3.5),
                c + egui::vec2(-8.5, 8.5),
                c + egui::vec2(-3.5, 7.5),
            ];
            painter.add(egui::Shape::convex_polygon(tip, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
            let pts = vec![
                c + egui::vec2(-7.5, 2.0),
                c + egui::vec2(1.0, -6.5),
                c + egui::vec2(7.5, 0.0),
                c + egui::vec2(-1.0, 8.5),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
            let half = vec![
                c + egui::vec2(-7.5, 2.0),
                c + egui::vec2(-3.2, -2.2),
                c + egui::vec2(3.2, 4.2),
                c + egui::vec2(-1.0, 8.5),
            ];
            painter.add(egui::Shape::convex_polygon(half, main_color, egui::Stroke::NONE));
        }
        ActiveToolUI::Bucket => {
            let pts = vec![
                c + egui::vec2(-6.5, 0.0),
                c + egui::vec2(0.0, -6.5),
                c + egui::vec2(6.5, 0.0),
                c + egui::vec2(0.0, 6.5),
            ];
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
            painter.line_segment([c + egui::vec2(-5.0, 2.0), c + egui::vec2(-8.0, -1.0)], egui::Stroke::new(2.0, main_color));
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
        let p1 = c + egui::vec2(angle.cos() * 5.5 * scale, angle.sin() * 5.5 * scale);
        let p2 = c + egui::vec2(angle.cos() * 8.2 * scale, angle.sin() * 8.2 * scale);
        painter.line_segment([p1, p2], egui::Stroke::new(2.0 * scale, color));
    }
}

fn draw_timeline_frame_shape(painter: &egui::Painter, rect: egui::Rect, kind: u8) {
    let c = rect.center();
    match kind {
        1 => {
            painter.circle_filled(c, 4.8, egui::Color32::from_rgb(239, 68, 68));
        }
        2 => {
            let sq = egui::Rect::from_center_size(c, egui::vec2(8.5, 8.5));
            painter.rect_filled(sq, 1.0, egui::Color32::from_rgb(15, 15, 18));
            painter.rect_stroke(sq, 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(160)));
        }
        3 => {
            let r = 5.5;
            let mut pts = Vec::with_capacity(5);
            for i in 0..5 {
                let ang = -std::f32::consts::FRAC_PI_2 + (i as f32) * (std::f32::consts::TAU / 5.0);
                pts.push(c + egui::vec2(ang.cos() * r, ang.sin() * r));
            }
            painter.add(egui::Shape::convex_polygon(
                pts,
                egui::Color32::from_rgb(34, 197, 94),
                egui::Stroke::NONE,
            ));
        }
        _ => {}
    }
}

#[derive(Clone)]
struct HistorySnapshot {
    project: AniProject,
    timeline_length: usize,
    current_frame: usize,
    active_layer_idx: usize,
}

fn point_in_polygon(pt: egui::Pos2, poly: &[egui::Pos2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > pt.y) != (pj.y > pt.y))
            && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / ((pj.y - pi.y) + 1e-6) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn erase_partial_from_strokes(strokes: &mut Vec<Stroke>, center: egui::Pos2, radius: f32) {
    let mut new_strokes = Vec::with_capacity(strokes.len());
    for s in strokes.drain(..) {
        if s.points.len() == 1 {
            let p = &s.points[0];
            if ((p.x - center.x).powi(2) + (p.y - center.y).powi(2)).sqrt() >= radius {
                new_strokes.push(s);
            }
            continue;
        }
        let mut current_seg: Vec<StrokePoint> = Vec::new();
        for p in s.points {
            let dist = ((p.x - center.x).powi(2) + (p.y - center.y).powi(2)).sqrt();
            if dist >= radius {
                current_seg.push(p);
            } else if current_seg.len() >= 2 {
                new_strokes.push(Stroke {
                    points: std::mem::take(&mut current_seg),
                    color: s.color,
                    fill_color: None,
                    base_width: s.base_width,
                    is_vector: s.is_vector,
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
                brush_texture_id: s.brush_texture_id,
            });
        }
    }
    *strokes = new_strokes;
}

fn build_geometric_stroke(
    kind: GeometricShapeKind,
    p0: egui::Pos2,
    p1: egui::Pos2,
    color: [u8; 4],
    width: f32,
) -> Stroke {
    let mut pts = Vec::new();
    match kind {
        GeometricShapeKind::StraightLine => {
            pts.push(StrokePoint { x: p0.x, y: p0.y, pressure: 1.0 });
            pts.push(StrokePoint { x: p1.x, y: p1.y, pressure: 1.0 });
        }
        GeometricShapeKind::Circle => {
            let center = p0.lerp(p1, 0.5);
            let r = (p1 - p0).length() * 0.5;
            let segments = 32;
            for i in 0..=segments {
                let ang = (i as f32 / segments as f32) * std::f32::consts::TAU;
                pts.push(StrokePoint {
                    x: center.x + ang.cos() * r,
                    y: center.y + ang.sin() * r,
                    pressure: 1.0,
                });
            }
        }
        GeometricShapeKind::Square => {
            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let side = dx.abs().max(dy.abs());
            let sx = p0.x + side * dx.signum();
            let sy = p0.y + side * dy.signum();
            let corners = [
                egui::pos2(p0.x, p0.y),
                egui::pos2(sx, p0.y),
                egui::pos2(sx, sy),
                egui::pos2(p0.x, sy),
                egui::pos2(p0.x, p0.y),
            ];
            for c in corners {
                pts.push(StrokePoint { x: c.x, y: c.y, pressure: 1.0 });
            }
        }
        GeometricShapeKind::Rectangle => {
            let corners = [
                egui::pos2(p0.x, p0.y),
                egui::pos2(p1.x, p0.y),
                egui::pos2(p1.x, p1.y),
                egui::pos2(p0.x, p1.y),
                egui::pos2(p0.x, p0.y),
            ];
            for c in corners {
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
        brush_texture_id: None,
    }
}

struct StickApp {
    workspace_mode: WorkspaceMode,
    install_target_dir: String,
    install_status_text: String,
    project: AniProject,
    timeline_length: usize,
    timeline_cell_width: f32,
    undo_stack: Vec<HistorySnapshot>,
    redo_stack: Vec<HistorySnapshot>,
    clipboard_frame: Option<FrameData>,
    current_frame: usize,
    last_scrubbed_frame: usize,
    active_layer_idx: usize,
    selected_audio_idx: Option<usize>,
    active_tool_ui: ActiveToolUI,
    selected_shape_kind: GeometricShapeKind,
    symmetry_mode: SymmetryMode,
    stabilizer_strength: f32,
    shape_drag_start: Option<egui::Pos2>,
    shape_drag_current: Option<egui::Pos2>,
    editing_vector_node: Option<(usize, usize)>,
    lasso_polygon: Vec<egui::Pos2>,
    is_drawing_lasso: bool,
    selected_stroke_indices: Vec<usize>,
    selected_text_idx: Option<usize>,
    selected_symbol_inst_idx: Option<usize>,
    is_resizing_selection: bool,
    brush_color: [u8; 4],
    stage_bg_color: [u8; 4],
    brush_size: f32,
    pen_pressure_enabled: bool,
    pressure_sensitivity: f32,
    zoom_level: f32,
    pan_offset: egui::Vec2,
    canvas_rotation_deg: f32,
    canvas_flip_h: bool,
    canvas_flip_v: bool,
    is_playing: bool,
    last_frame_time: f64,
    last_sfx_time: f64,
    last_xp_tick_time: f64,
    frames_drawn_counter: u64,
    level_up_banner_timer: f64,
    level_up_custom_msg: String,
    has_unsaved_changes: bool,
    show_exit_dialog: bool,
    show_symbol_dialog: bool,
    show_text_dialog: bool,
    show_export_dialog: bool,
    show_timeline_settings: bool,
    show_blender_locate_dialog: bool,
    blender_path_buf: String,
    selected_export_format: VideoExportFormat,
    text_input_buf: String,
    text_font_size: f32,
    pending_text_pos: egui::Pos2,
    show_update_dialog: bool,
    show_settings_dialog: bool,
    show_credits_dialog: bool,
    renaming_layer_idx: Option<usize>,
    rename_layer_buf: String,
    trimming_audio_idx: Option<usize>,
    dragged_layer_idx: Option<usize>,
    dragged_frame: Option<(usize, usize)>,
    settings_tab: usize,
    listening_shortcut_idx: Option<usize>,
    config: AppConfig,
    user_profile: UserProfile,
    avatar_texture: Option<egui::TextureHandle>,
    installed_version: String,
    update_state: Arc<Mutex<UpdateState>>,
    new_symbol_name: String,
    allow_close: bool,
    current_stroke: Option<Stroke>,
    texture_cache: HashMap<String, egui::TextureHandle>,
    imported_brushes: Vec<String>,
    saved_path: Option<PathBuf>,
    status_message: String,
    audio_sink: Option<(OutputStream, Sink)>,
    last_pointer_pos: Option<egui::Pos2>,
    right_tab: usize,
}

impl StickApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        apply_stick_theme(&cc.egui_ctx);
        let mut proj = AniProject::default();
        proj.total_frames = 1;
        let profile = UserProfile::load();
        let mut avatar_tex = None;
        if let Some(ref path) = profile.avatar_path {
            if let Ok(img) = image::open(path) {
                let rgba = img.to_rgba8();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                );
                avatar_tex = Some(cc.egui_ctx.load_texture("user_avatar", color_image, egui::TextureOptions::LINEAR));
            }
        }

        let default_install_path = std::env::var("LOCALAPPDATA")
            .map(|p| format!("{}\\StickStudio", p))
            .unwrap_or_else(|_| "C:\\StickStudio".to_string());

        let initial_mode = if is_running_from_installed_dir() {
            WorkspaceMode::Launcher
        } else {
            WorkspaceMode::InstallerSetup
        };

        let cfg = AppConfig::load();
        let b_buf = cfg.custom_blender_path.clone().unwrap_or_default();

        Self {
            workspace_mode: initial_mode,
            install_target_dir: default_install_path,
            install_status_text: String::new(),
            project: proj,
            timeline_length: 1,
            timeline_cell_width: 28.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard_frame: None,
            current_frame: 1,
            last_scrubbed_frame: 1,
            active_layer_idx: 0,
            selected_audio_idx: None,
            active_tool_ui: ActiveToolUI::Brush,
            selected_shape_kind: GeometricShapeKind::StraightLine,
            symmetry_mode: SymmetryMode::Off,
            stabilizer_strength: 0.35,
            shape_drag_start: None,
            shape_drag_current: None,
            editing_vector_node: None,
            lasso_polygon: Vec::new(),
            is_drawing_lasso: false,
            selected_stroke_indices: Vec::new(),
            selected_text_idx: None,
            selected_symbol_inst_idx: None,
            is_resizing_selection: false,
            brush_color: [0, 0, 0, 255],
            stage_bg_color: [255, 255, 255, 255],
            brush_size: 6.0,
            pen_pressure_enabled: true,
            pressure_sensitivity: 1.0,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
            canvas_rotation_deg: 0.0,
            canvas_flip_h: false,
            canvas_flip_v: false,
            is_playing: false,
            last_frame_time: 0.0,
            last_sfx_time: 0.0,
            last_xp_tick_time: 0.0,
            frames_drawn_counter: 0,
            level_up_banner_timer: 0.0,
            level_up_custom_msg: String::new(),
            has_unsaved_changes: false,
            show_exit_dialog: false,
            show_symbol_dialog: false,
            show_text_dialog: false,
            show_export_dialog: false,
            show_timeline_settings: false,
            show_blender_locate_dialog: false,
            blender_path_buf: b_buf,
            selected_export_format: VideoExportFormat::Mp4,
            text_input_buf: String::new(),
            text_font_size: 36.0,
            pending_text_pos: egui::pos2(200.0, 200.0),
            show_update_dialog: false,
            show_settings_dialog: false,
            show_credits_dialog: false,
            renaming_layer_idx: None,
            rename_layer_buf: String::new(),
            trimming_audio_idx: None,
            dragged_layer_idx: None,
            dragged_frame: None,
            settings_tab: 0,
            listening_shortcut_idx: None,
            config: cfg,
            user_profile: profile,
            avatar_texture: avatar_tex,
            installed_version: get_installed_version(),
            update_state: Arc::new(Mutex::new(UpdateState::Idle)),
            new_symbol_name: "Symbol_1".to_string(),
            allow_close: false,
            current_stroke: None,
            texture_cache: HashMap::new(),
            imported_brushes: Vec::new(),
            saved_path: None,
            status_message: "Stick Ready".to_string(),
            audio_sink: None,
            last_pointer_pos: None,
            right_tab: 0,
        }
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

        // تحميل FFmpeg في الخلفية لعدم تجميد الواجهة
        let dir_clone = target_dir.clone();
        std::thread::spawn(move || {
            let _ = exporter::ensure_ffmpeg_installed(&dir_clone);
        });

        self.play_sfx(SfxType::LevelUpFanfare);
        self.workspace_mode = WorkspaceMode::Launcher;
        self.status_message = "تم التسطيب وإنشاء اختصار Stick على سطح المكتب!".to_string();
    }

    fn play_sfx(&self, sfx: SfxType) {
        play_sfx_async(self.config.sound_effects_enabled, sfx, false);
    }

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
            project: self.project.clone(),
            timeline_length: self.timeline_length,
            current_frame: self.current_frame,
            active_layer_idx: self.active_layer_idx,
        }
    }

    fn restore_snapshot(&mut self, snap: HistorySnapshot) {
        self.project = snap.project;
        self.timeline_length = snap.timeline_length.max(1);
        self.current_frame = snap.current_frame.clamp(1, self.timeline_length);
        self.active_layer_idx = snap.active_layer_idx.min(self.project.layers.len().saturating_sub(1));
        self.selected_stroke_indices.clear();
        self.selected_text_idx = None;
        self.selected_symbol_inst_idx = None;
    }

    fn save_undo_state(&mut self) {
        if self.undo_stack.len() > 40 {
            self.undo_stack.remove(0);
        }
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

    fn get_interpolated_symbols(layer: &Layer, frame: usize) -> Vec<SymbolInstance> {
        let prev_k = match layer.keyframes.keys().filter(|&&k| k <= frame).max().copied() {
            Some(k) => k,
            None => return Vec::new(),
        };
        let prev_fd = match layer.keyframes.get(&prev_k) {
            Some(fd) => fd,
            None => return Vec::new(),
        };
        if !prev_fd.motion_tween || prev_k == frame {
            return prev_fd.symbol_instances.clone();
        }
        if let Some(&next_k) = layer.keyframes.keys().filter(|&&k| k > prev_k).min() {
            if frame <= next_k {
                if let Some(next_fd) = layer.keyframes.get(&next_k) {
                    let t = (frame - prev_k) as f32 / (next_k - prev_k).max(1) as f32;
                    let mut result = Vec::with_capacity(prev_fd.symbol_instances.len());
                    for (idx, inst) in prev_fd.symbol_instances.iter().enumerate() {
                        if let Some(target) = next_fd.symbol_instances.get(idx) {
                            if target.symbol_name == inst.symbol_name {
                                result.push(SymbolInstance {
                                    symbol_name: inst.symbol_name.clone(),
                                    x: inst.x + (target.x - inst.x) * t,
                                    y: inst.y + (target.y - inst.y) * t,
                                    scale: inst.scale + (target.scale - inst.scale) * t,
                                });
                                continue;
                            }
                        }
                        result.push(inst.clone());
                    }
                    return result;
                }
            }
        }
        prev_fd.symbol_instances.clone()
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

    /// إضافة خط مرسوم مع دعم التماثل الأفقي والرأسي في وضع الرسم
    fn commit_stroke_with_symmetry(&mut self, stroke: Stroke, now_time: f64) {
        let stage_w = self.project.width as f32;
        let stage_h = self.project.height as f32;
        let mut strokes_to_add = vec![stroke.clone()];

        match self.symmetry_mode {
            SymmetryMode::Horizontal => {
                let mut mirrored = stroke;
                for p in &mut mirrored.points {
                    p.x = stage_w - p.x;
                }
                strokes_to_add.push(mirrored);
            }
            SymmetryMode::Vertical => {
                let mut mirrored = stroke;
                for p in &mut mirrored.points {
                    p.y = stage_h - p.y;
                }
                strokes_to_add.push(mirrored);
            }
            SymmetryMode::Off => {}
        }

        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            let fd = layer.keyframes.entry(self.current_frame).or_default();
            fd.strokes.extend(strokes_to_add);
            self.has_unsaved_changes = true;
            self.record_frame_drawn_activity(now_time);
        }
    }

    fn action_insert_frame_f5(&mut self) {
        self.save_undo_state();
        self.timeline_length += 1;
        self.project.total_frames = self.timeline_length;
        self.current_frame = self.timeline_length;
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::NewFrame);
        self.status_message = format!("{} -> {}f", tr(self.config.lang, "Frame Extended", "تم تمديد الفريم"), self.timeline_length);
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
        self.status_message = format!("{} ({})", tr(self.config.lang, "Keyframe Copied", "تم نسخ الفريم"), next_f);
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
        self.status_message = format!("{} ({})", tr(self.config.lang, "New Blank Keyframe", "فريم فارغ جديد"), next_f);
    }

    fn copy_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(layer) = self.project.layers.get(layer_idx) {
            let data = Self::get_effective_frame_data(layer, frame_idx).cloned().unwrap_or_default();
            self.clipboard_frame = Some(data);
            self.play_sfx(SfxType::UiClick);
            self.status_message = format!("{} ({})", tr(self.config.lang, "Copied Frame", "تم نسخ الفريم"), frame_idx);
        }
    }

    fn paste_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(clip) = self.clipboard_frame.clone() {
            self.save_undo_state();
            if let Some(layer) = self.project.layers.get_mut(layer_idx) {
                layer.keyframes.insert(frame_idx, clip);
                self.has_unsaved_changes = true;
                self.play_sfx(SfxType::NewFrame);
                self.status_message = format!("{} ({})", tr(self.config.lang, "Pasted to Frame", "تم اللصق في الفريم"), frame_idx);
            }
        }
    }

    fn clear_frame_content(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            layer.keyframes.insert(frame_idx, FrameData::default());
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::EraserRub);
            self.status_message = format!("{} ({})", tr(self.config.lang, "Frame Cleared", "تم تفريغ الفريم"), frame_idx);
        }
    }

    fn remove_frame_completely(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            let mut updated = HashMap::new();
            for (&k, v) in &layer.keyframes {
                if k < frame_idx {
                    updated.insert(k, v.clone());
                } else if k > frame_idx {
                    updated.insert(k - 1, v.clone());
                }
            }
            if updated.is_empty() {
                updated.insert(1, FrameData::default());
            }
            layer.keyframes = updated;
        }

        if self.timeline_length > 1 {
            self.timeline_length -= 1;
            self.project.total_frames = self.timeline_length;
        }
        if self.current_frame > self.timeline_length {
            self.current_frame = self.timeline_length;
        }

        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::EraserRub);
        self.status_message = format!("{} ({})", tr(self.config.lang, "Frame Deleted", "تم حذف الفريم نهائياً"), frame_idx);
    }

    fn scale_selected_items(&mut self, factor: f32) {
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
                if let Some(t_idx) = self.selected_text_idx {
                    if let Some(txt) = frame.text_items.get(t_idx) {
                        min_p = min_p.min(egui::pos2(txt.x, txt.y));
                        max_p = max_p.max(egui::pos2(txt.x + 80.0, txt.y + txt.font_size));
                        has_any = true;
                    }
                }
                if let Some(s_idx) = self.selected_symbol_inst_idx {
                    if let Some(inst) = frame.symbol_instances.get(s_idx) {
                        min_p = min_p.min(egui::pos2(inst.x, inst.y));
                        max_p = max_p.max(egui::pos2(inst.x + 50.0, inst.y + 50.0));
                        has_any = true;
                    }
                }

                if !has_any {
                    return;
                }
                let center = min_p.lerp(max_p, 0.5);

                for &idx in &self.selected_stroke_indices {
                    if let Some(s) = frame.strokes.get_mut(idx) {
                        for p in &mut s.points {
                            p.x = center.x + (p.x - center.x) * factor;
                            p.y = center.y + (p.y - center.y) * factor;
                        }
                    }
                }
                if let Some(t_idx) = self.selected_text_idx {
                    if let Some(txt) = frame.text_items.get_mut(t_idx) {
                        txt.x = center.x + (txt.x - center.x) * factor;
                        txt.y = center.y + (txt.y - center.y) * factor;
                        txt.font_size = (txt.font_size * factor).clamp(8.0, 260.0);
                    }
                }
                if let Some(s_idx) = self.selected_symbol_inst_idx {
                    if let Some(inst) = frame.symbol_instances.get_mut(s_idx) {
                        inst.x = center.x + (inst.x - center.x) * factor;
                        inst.y = center.y + (inst.y - center.y) * factor;
                        inst.scale = (inst.scale * factor).clamp(0.1, 10.0);
                    }
                }
                self.has_unsaved_changes = true;
            }
        }
    }

    fn trigger_audio_stream_scrub(&mut self) {
        if self.is_playing {
            return;
        }
        let fps = self.project.fps.max(1) as f64;
        for track in &self.project.audio_tracks {
            if track.sync_mode == AudioSyncMode::Stream
                && self.current_frame >= track.start_frame
                && self.current_frame <= track.end_frame
            {
                let offset_frames = (self.current_frame - track.start_frame) as f64;
                let seek_secs = offset_frames / fps;
                let path = track.file_path.clone();
                let frame_dur_ms = (1000.0 / fps).clamp(40.0, 120.0) as u64;
                std::thread::spawn(move || {
                    if let Ok((_stream, handle)) = OutputStream::try_default() {
                        if let Ok(sink) = Sink::try_new(&handle) {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                    let skipped = source
                                        .skip_duration(Duration::from_secs_f64(seek_secs))
                                        .take_duration(Duration::from_millis(frame_dur_ms));
                                    sink.append(skipped);
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
            if self.current_frame > track.end_frame {
                return;
            }
            let fps = self.project.fps.max(1) as f64;
            let start_offset = self.current_frame.saturating_sub(track.start_frame) as f64 / fps;
            let max_dur = (track.end_frame.saturating_sub(self.current_frame) + 1) as f64 / fps;
            if let Ok((stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    if let Ok(file) = File::open(&track.file_path) {
                        if let Ok(source) = Decoder::new(BufReader::new(file)) {
                            let clipped = source
                                .skip_duration(Duration::from_secs_f64(start_offset))
                                .take_duration(Duration::from_secs_f64(max_dur.max(0.1)));
                            sink.append(clipped);
                            self.audio_sink = Some((stream, sink));
                        }
                    }
                }
            }
        }
    }

    fn stop_audio_playback(&mut self) {
        if let Some((_, sink)) = self.audio_sink.take() {
            sink.stop();
        }
    }

    fn trigger_save(&mut self) -> bool {
        self.project.total_frames = self.timeline_length;
        if let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Stick Project", &["stick", "ah2d"])
            .set_file_name("project.stick")
            .save_file()
        {
            if path.extension().is_none() {
                path.set_extension("stick");
            }
            match self.project.save_to_file(&path) {
                Ok(_) => {
                    self.saved_path = Some(path);
                    self.has_unsaved_changes = false;
                    self.play_sfx(SfxType::NewFrame);
                    self.status_message = tr(self.config.lang, "Project Saved!", "تم حفظ المشروع بالكامل!");
                    return true;
                }
                Err(e) => {
                    self.status_message = e;
                }
            }
        }
        false
    }

    fn import_image_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        {
            self.save_undo_state();
            let img_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let mut kf = HashMap::new();
            let mut fd = FrameData::default();
            fd.media_items.push(MediaItem {
                path: path.to_string_lossy().to_string(),
                media_type: MediaType::Image,
                x: 100.0,
                y: 50.0,
                width: 400.0,
                height: 300.0,
            });
            kf.insert(self.current_frame, fd);
            self.project.layers.insert(
                0,
                Layer {
                    name: format!("Img_{}", img_name),
                    visible: true,
                    locked: false,
                    keyframes: kf,
                },
            );
            self.active_layer_idx = 0;
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::NewFrame);
            self.status_message = tr(self.config.lang, "Image Layer Added", "تم إضافة طبقة للصورة");
        }
    }

    fn import_video_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Videos", &["mp4", "mov", "avi", "mkv", "webm"])
            .pick_file()
        {
            self.status_message = tr(self.config.lang, "Extracting video frames...", "جاري استخراج فريمات الفيديو...");
            match exporter::extract_video_frames(&path, self.project.fps) {
                Ok(frames) => {
                    self.save_undo_state();
                    let needed_len = self.current_frame + frames.len().saturating_sub(1);
                    if needed_len > self.timeline_length {
                        self.timeline_length = needed_len;
                        self.project.total_frames = needed_len;
                    }
                    let vid_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let mut kf = HashMap::new();
                    for (i, f_path) in frames.iter().enumerate() {
                        let f_idx = self.current_frame + i;
                        let mut fd = FrameData::default();
                        fd.media_items.push(MediaItem {
                            path: f_path.to_string_lossy().to_string(),
                            media_type: MediaType::VideoFrame,
                            x: 0.0,
                            y: 0.0,
                            width: self.project.width as f32,
                            height: self.project.height as f32,
                        });
                        kf.insert(f_idx, fd);
                    }
                    self.project.layers.insert(
                        0,
                        Layer {
                            name: format!("Vid_{}", vid_name),
                            visible: true,
                            locked: false,
                            keyframes: kf,
                        },
                    );
                    self.active_layer_idx = 0;
                    self.has_unsaved_changes = true;
                    self.play_sfx(SfxType::NewFrame);
                    self.status_message = format!("Video Layer: {} frames", frames.len());
                }
                Err(e) => self.status_message = e,
            }
        }
    }

    fn import_blender_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Blender 2D/3D Project", &["blend"])
            .pick_file()
        {
            self.status_message = tr(self.config.lang, "Rendering Blender scene...", "جاري استيراد مشهد بلندر (.blend)...");
            match exporter::import_blender_project_frames(&path, self.config.custom_blender_path.as_deref()) {
                Ok(frames) => {
                    self.save_undo_state();
                    let needed_len = self.current_frame + frames.len().saturating_sub(1);
                    if needed_len > self.timeline_length {
                        self.timeline_length = needed_len;
                        self.project.total_frames = needed_len;
                    }
                    let blend_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let mut kf = HashMap::new();
                    for (i, f_path) in frames.iter().enumerate() {
                        let f_idx = self.current_frame + i;
                        let mut fd = FrameData::default();
                        fd.media_items.push(MediaItem {
                            path: f_path.to_string_lossy().to_string(),
                            media_type: MediaType::VideoFrame,
                            x: 0.0,
                            y: 0.0,
                            width: self.project.width as f32,
                            height: self.project.height as f32,
                        });
                        kf.insert(f_idx, fd);
                    }
                    self.project.layers.insert(
                        0,
                        Layer {
                            name: format!("Blend_{}", blend_name),
                            visible: true,
                            locked: false,
                            keyframes: kf,
                        },
                    );
                    self.active_layer_idx = 0;
                    self.has_unsaved_changes = true;
                    self.play_sfx(SfxType::NewFrame);
                    self.status_message = format!("Blender Layer: {} frames", frames.len());
                }
                Err(e) => {
                    if e == "BLENDER_NOT_FOUND" {
                        self.show_blender_locate_dialog = true;
                        self.status_message = "حدد مكان blender.exe لمرة واحدة".to_string();
                    } else {
                        self.status_message = e;
                    }
                }
            }
        }
    }

    fn import_audio_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio", &["mp3", "wav"])
            .pick_file()
        {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let p_str = path.to_string_lossy().to_string();
            let peaks = extract_audio_waveform(&p_str, 120);
            self.save_undo_state();
            if self.timeline_length < 24 {
                self.timeline_length = 24;
                self.project.total_frames = 24;
            }
            self.project.audio_tracks.push(AudioTrack {
                name: name.clone(),
                file_path: p_str,
                start_frame: self.current_frame,
                end_frame: self.timeline_length,
                sync_mode: AudioSyncMode::Stream,
                waveform_peaks: peaks,
            });
            self.selected_audio_idx = Some(self.project.audio_tracks.len() - 1);
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::NewFrame);
            self.status_message = format!("Audio Layer: {}", name);
        }
    }

    fn check_for_updates(&mut self, ctx: egui::Context) {
        self.show_update_dialog = true;
        *self.update_state.lock().unwrap() = UpdateState::Checking;
        let state_clone = Arc::clone(&self.update_state);
        let current_ver = self.installed_version.clone();

        std::thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases?per_page=1", GITHUB_REPO);
            match ureq::get(&api_url).set("User-Agent", "Stick-Updater").call() {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<serde_json::Value>() {
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
                        } else {
                            *state_clone.lock().unwrap() = UpdateState::UpToDate;
                        }
                    } else {
                        *state_clone.lock().unwrap() = UpdateState::Error("Failed to parse release data.".to_string());
                    }
                }
                Err(e) => {
                    *state_clone.lock().unwrap() = UpdateState::Error(format!("Network error: {}", e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn download_and_apply_update(&mut self, version: String, download_url: String, ctx: egui::Context) {
        *self.update_state.lock().unwrap() = UpdateState::Downloading(format!("Downloading v{}...", version));
        let state_clone = Arc::clone(&self.update_state);

        std::thread::spawn(move || {
            match ureq::get(&download_url).set("User-Agent", "Stick-Updater").call() {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    if response.into_reader().read_to_end(&mut bytes).is_ok() {
                        if let Ok(current_exe) = std::env::current_exe() {
                            if let Some(dir) = current_exe.parent() {
                                let _ = exporter::ensure_ffmpeg_installed(dir);
                            }
                            let new_exe = current_exe.with_file_name("Stick_new.exe");
                            if let Ok(mut file) = File::create(&new_exe) {
                                if file.write_all(&bytes).is_ok() {
                                    save_installed_version(&version);
                                    *state_clone.lock().unwrap() = UpdateState::ReadyToRestart;
                                    ctx.request_repaint();
                                    return;
                                }
                            }
                        }
                    }
                    *state_clone.lock().unwrap() = UpdateState::Error("Failed to save update.".to_string());
                }
                Err(e) => {
                    *state_clone.lock().unwrap() = UpdateState::Error(format!("Download failed: {}", e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn restart_with_new_exe(&mut self) {
        if let Ok(current_exe) = std::env::current_exe() {
            let new_exe = current_exe.with_file_name("Stick_new.exe");
            let ps_script = format!(
                "Start-Sleep -Seconds 2; Move-Item -LiteralPath '{}' -Destination '{}' -Force; Start-Process -FilePath '{}'",
                new_exe.display().to_string().replace('\'', "''"),
                current_exe.display().to_string().replace('\'', "''"),
                current_exe.display().to_string().replace('\'', "''")
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
                .spawn();
            std::process::exit(0);
        }
    }

    fn preload_frame_textures(&mut self, ctx: &egui::Context) {
        let mut paths = Vec::new();
        for layer in &self.project.layers {
            if layer.visible {
                if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame) {
                    for m in &fd.media_items {
                        if !self.texture_cache.contains_key(&m.path) {
                            paths.push(m.path.clone());
                        }
                    }
                }
            }
        }
        for path in paths {
            if let Ok(img) = image::open(&path) {
                let rgba = img.to_rgba8();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                );
                let handle = ctx.load_texture(&path, color_image, egui::TextureOptions::LINEAR);
                self.texture_cache.insert(path, handle);
            }
        }
    }

    /// رسم لوحة خصائص الأدوات المشتركة بين وضع الرسم ووضع الأنيميشن
    fn render_tool_properties_ui(&mut self, ui: &mut egui::Ui, lang: AppLang) {
        match self.active_tool_ui {
            ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso | ActiveToolUI::VectorDirectSelect => {
                ui.strong(tr(lang, "Selection Tool Properties", "خصائص أداة التحديد"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::SelectNormal, tr(lang, "Normal (V)", "عادي (V)")).clicked() {
                        self.active_tool_ui = ActiveToolUI::SelectNormal;
                    }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::SelectLasso, tr(lang, "Lasso (L)", "حرة (L)")).clicked() {
                        self.active_tool_ui = ActiveToolUI::SelectLasso;
                    }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::VectorDirectSelect, tr(lang, "Vector Pts (Q)", "نقاط فيكتور (Q)")).clicked() {
                        self.active_tool_ui = ActiveToolUI::VectorDirectSelect;
                    }
                });
                ui.add_space(6.0);
                ui.label(tr(lang, "Scale Selected Group:", "تكبير وتصغير المجموعة المحددة:"));
                ui.horizontal(|ui| {
                    if ui.button(" -10% ").clicked() {
                        self.save_undo_state();
                        self.scale_selected_items(0.90);
                    }
                    if ui.button(" +10% ").clicked() {
                        self.save_undo_state();
                        self.scale_selected_items(1.10);
                    }
                });
                if let Some(t_idx) = self.selected_text_idx {
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if let Some(txt) = frame.text_items.get_mut(t_idx) {
                                ui.add_space(6.0);
                                ui.separator();
                                ui.strong(tr(lang, "Selected Text Edit", "تعديل النص المحدد"));
                                ui.text_edit_singleline(&mut txt.content);
                                ui.horizontal(|ui| {
                                    ui.label(tr(lang, "Size:", "الحجم:"));
                                    ui.add(egui::DragValue::new(&mut txt.font_size).range(8.0..=260.0));
                                    ui.color_edit_button_srgba_unmultiplied(&mut txt.color);
                                });
                            }
                        }
                    }
                }
            }
            ActiveToolUI::VectorPen => {
                ui.strong(tr(lang, "Vector Pen (P)", "قلم الفيكتور الذكي"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Stroke Width:", "سُمك الخط:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
                ui.small(tr(lang, "Use Vector Select (Q) to edit vector points.", "استخدم أداة نقاط الفيكتور (Q) لتعديل انحناء الخطوط."));
            }
            ActiveToolUI::Brush => {
                ui.strong(tr(lang, "Brush Properties (B)", "خصائص الفرشاة"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Size:", "الحجم:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
                ui.checkbox(&mut self.pen_pressure_enabled, tr(lang, "Pen Pressure Sensitivity", "حساسية ضغط القلم"));
                if self.pen_pressure_enabled {
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Sensitivity:", "درجة الحساسية:"));
                        ui.add(egui::Slider::new(&mut self.pressure_sensitivity, 0.2..=2.0));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Stabilizer:", "مثبت اليد:"));
                    ui.add(egui::Slider::new(&mut self.stabilizer_strength, 0.0..=0.85));
                });
            }
            ActiveToolUI::ShapeLine => {
                ui.strong(tr(lang, "Shapes & Line Tool (U)", "أداة الخطوط والأشكال الهندسية"));
                ui.add_space(4.0);
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::StraightLine, tr(lang, "Straight Line", "خط مستقيم"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Circle, tr(lang, "Circle", "دائرة مضبوطة"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Square, tr(lang, "Square", "مربع"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Rectangle, tr(lang, "Rectangle", "مستطيل"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Line Thickness:", "سُمك الخط:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
            }
            ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
                ui.strong(tr(lang, "Eraser Properties", "خصائص الممحاة"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserRed, tr(lang, "Red (Full E)", "حمراء (كامل E)")).clicked() {
                        self.active_tool_ui = ActiveToolUI::EraserRed;
                    }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserGreen, tr(lang, "Green (Part T)", "خضراء (جزئي T)")).clicked() {
                        self.active_tool_ui = ActiveToolUI::EraserGreen;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Eraser Radius:", "نطاق المسح:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(2.0..=120.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 2.0..=120.0).show_value(false));
            }
            ActiveToolUI::Text => {
                ui.strong(tr(lang, "Text Tool Properties", "خصائص أداة النص"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Default Font Size:", "حجم الخط:"));
                    ui.add(egui::DragValue::new(&mut self.text_font_size).range(10.0..=180.0));
                });
            }
            ActiveToolUI::HandCanvas => {
                ui.strong(tr(lang, "Hand & Canvas Orientation", "أداة اليد وتدوير وعكس الكانفا"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Flip H", "عكس أفقي")).clicked() {
                        self.canvas_flip_h = !self.canvas_flip_h;
                    }
                    if ui.button(tr(lang, "Flip V", "عكس رأسي")).clicked() {
                        self.canvas_flip_v = !self.canvas_flip_v;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Rotation:", "زاوية الدوران:"));
                    ui.add(egui::Slider::new(&mut self.canvas_rotation_deg, -180.0..=180.0));
                });
                if ui.small_button(tr(lang, "Reset Orientation", "إعادة الوضع الأصلي")).clicked() {
                    self.canvas_rotation_deg = 0.0;
                    self.canvas_flip_h = false;
                    self.canvas_flip_v = false;
                }
            }
            ActiveToolUI::Bucket | ActiveToolUI::Eyedropper => {
                ui.strong(tr(lang, "Color & Fill Properties", "خصائص اللون والتعبئة"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            }
        }
    }
}

impl eframe::App for StickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = self.config.lang;
        let now_time = ctx.input(|i| i.time);

        if self.workspace_mode == WorkspaceMode::InstallerSetup {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(75.0);
                    ui.heading(
                        egui::RichText::new("Stick Studio Setup")
                            .size(34.0)
                            .color(egui::Color32::from_rgb(255, 215, 25)),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(fix_ar("مرحباً بك في برنامج Stick! حدد مكان تسطيب البرنامج:"))
                            .size(20.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.add_space(22.0);

                    ui.horizontal(|ui| {
                        let avail = ui.available_width();
                        ui.add_space((avail - 460.0).max(10.0) * 0.5);
                        ui.add(egui::TextEdit::singleline(&mut self.install_target_dir).desired_width(350.0));
                        if ui.button(fix_ar("استعراض...")).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.install_target_dir = folder.join("StickStudio").to_string_lossy().to_string();
                            }
                        }
                    });

                    ui.add_space(24.0);
                    let install_btn = egui::Button::new(
                        egui::RichText::new(fix_ar("تسطيب البرنامج وإنشاء اختصار على سطح المكتب"))
                            .size(18.0)
                            .color(egui::Color32::BLACK),
                    )
                    .min_size(egui::vec2(340.0, 48.0))
                    .fill(egui::Color32::from_rgb(255, 215, 25));

                    if ui.add(install_btn).clicked() {
                        self.perform_full_installation();
                    }

                    ui.add_space(12.0);
                    if ui.small_button(fix_ar("تشغيل مباشر بدون تسطيب")).clicked() {
                        self.workspace_mode = WorkspaceMode::Launcher;
                    }

                    if !self.install_status_text.is_empty() {
                        ui.add_space(12.0);
                        ui.colored_label(egui::Color32::LIGHT_RED, fix_ar(&self.install_status_text));
                    }
                });
            });
            return;
        }

        if self.last_xp_tick_time == 0.0 {
            self.last_xp_tick_time = now_time;
        } else if now_time - self.last_xp_tick_time >= 60.0 {
            let mins = ((now_time - self.last_xp_tick_time) / 60.0).floor() as u64;
            self.last_xp_tick_time += (mins as f64) * 60.0;
            self.user_profile.total_seconds_spent += mins * 60;
            self.grant_xp(mins, now_time);
        }

        if self.current_frame != self.last_scrubbed_frame {
            self.last_scrubbed_frame = self.current_frame;
            self.trigger_audio_stream_scrub();
        }

        // نافذة إشعار ارتقاء المستوى على طراز Solo Leveling بالرسائل المخصصة كل 5 مستويات
        if now_time < self.level_up_banner_timer {
            let (lvl, _, _) = self.user_profile.get_level_progress();
            let rank = self.user_profile.get_hunter_rank();
            egui::Window::new("SYSTEM_ALERT")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(10, 22, 48, 245))
                        .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(56, 189, 248)))
                        .inner_margin(16.0)
                        .rounding(8.0),
                )
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("[ SYSTEM : LEVEL UP ]")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(56, 189, 248)),
                        );
                        ui.heading(
                            egui::RichText::new(format!("LEVEL {}  ({})", lvl, rank))
                                .size(22.0)
                                .color(egui::Color32::from_rgb(250, 204, 21)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(fix_ar(&self.level_up_custom_msg))
                                .size(16.0)
                                .color(egui::Color32::WHITE),
                        );
                    });
                });
            ctx.request_repaint();
        }

        if self.workspace_mode == WorkspaceMode::Launcher {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.heading(
                        egui::RichText::new("Stick Studio")
                            .size(36.0)
                            .color(egui::Color32::from_rgb(255, 215, 25)),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(tr(
                            lang,
                            "Are you here to draw or animate?",
                            "هل أنت داخل ترسم؟",
                        ))
                        .size(24.0)
                        .color(egui::Color32::WHITE),
                    );
                    ui.add_space(30.0);

                    ui.horizontal(|ui| {
                        let avail = ui.available_width();
                        ui.add_space((avail - 440.0).max(10.0) * 0.5);

                        let draw_btn = egui::Button::new(
                            egui::RichText::new(tr(lang, "🎨 Drawing", "🎨 رسم"))
                                .size(22.0)
                                .color(egui::Color32::WHITE),
                        )
                        .min_size(egui::vec2(205.0, 95.0))
                        .fill(egui::Color32::from_rgb(249, 115, 22));

                        if ui.add(draw_btn).clicked() {
                            self.play_sfx(SfxType::NewFrame);
                            self.workspace_mode = WorkspaceMode::DrawingOnly;
                        }

                        ui.add_space(25.0);

                        let anim_btn = egui::Button::new(
                            egui::RichText::new(tr(lang, "🎬 Animation", "🎬 أنيميشن"))
                                .size(22.0)
                                .color(egui::Color32::WHITE),
                        )
                        .min_size(egui::vec2(205.0, 95.0))
                        .fill(egui::Color32::from_rgb(0, 122, 204));

                        if ui.add(anim_btn).clicked() {
                            self.play_sfx(SfxType::NewFrame);
                            self.workspace_mode = WorkspaceMode::AnimationStudio;
                        }
                    });

                    ui.add_space(35.0);
                    ui.horizontal(|ui| {
                        let avail = ui.available_width();
                        ui.add_space((avail - 180.0).max(10.0) * 0.5);
                        if ui.selectable_label(lang == AppLang::Arabic, fix_ar("العربية")).clicked() {
                            self.config.lang = AppLang::Arabic;
                            self.config.save();
                        }
                        ui.separator();
                        if ui.selectable_label(lang == AppLang::English, "English").clicked() {
                            self.config.lang = AppLang::English;
                            self.config.save();
                        }
                    });
                });
            });
            return;
        }

        let mut typing_in_textbox = ctx.wants_keyboard_input();

        let (
            now,
            ctrl_z,
            ctrl_y,
            ctrl_s,
            ctrl_c,
            ctrl_v,
            shift_f5,
            del_pressed,
            space_down,
            ctrl_down,
            alt_down,
            zoom_in_nav,
            zoom_out_nav,
            pan_right_nav,
            pan_left_nav,
            scroll_y,
            close_req,
            pointer_down,
            pointer_pressed,
            pointer_released,
            pointer_pos,
            touch_force,
            sc_select,
            sc_lasso,
            sc_vec_sel,
            sc_pen,
            sc_brush,
            sc_eraser,
            sc_eraser_partial,
            sc_bucket,
            sc_eyedropper,
            sc_text,
            sc_shape,
            sc_hand,
            sc_del_frame,
            sc_f5,
            sc_f6,
            sc_f7,
            sc_f8,
            sc_play,
            sc_prev,
            sc_next,
            sc_smaller,
            sc_larger,
            sc_reset_canvas,
            pressed_key_name,
        ) = ctx.input(|i| {
            let mut captured_key = None;
            let mut stylus_force = None;
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, .. } => {
                        captured_key = Some(key.name().to_string());
                    }
                    egui::Event::Touch { force: Some(f), .. } => {
                        stylus_force = Some(*f);
                    }
                    _ => {}
                }
            }
            if captured_key.is_none() && i.modifiers.shift {
                captured_key = Some("Shift".to_string());
            }

            let z_in = (i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowUp))
                || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(egui::Key::W));
            let z_out = (i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowDown))
                || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(egui::Key::S));
            let p_right = (i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight))
                || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(egui::Key::D));
            let p_left = (i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft))
                || (!i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(egui::Key::A));

            (
                i.time,
                i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                (i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)),
                i.modifiers.ctrl && i.key_pressed(egui::Key::S),
                i.modifiers.ctrl && i.key_pressed(egui::Key::C),
                i.modifiers.ctrl && i.key_pressed(egui::Key::V),
                i.modifiers.shift && i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::Delete),
                i.key_down(egui::Key::Space),
                i.modifiers.ctrl,
                i.modifiers.alt,
                z_in,
                z_out,
                p_right,
                p_left,
                i.raw_scroll_delta.y,
                i.viewport().close_requested(),
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
                stylus_force,
                is_shortcut_pressed(i, &self.config.tool_select),
                is_shortcut_pressed(i, &self.config.tool_lasso),
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

        if let (Some(idx), Some(k_name)) = (self.listening_shortcut_idx, pressed_key_name) {
            match idx {
                0 => self.config.tool_select = k_name,
                1 => self.config.tool_lasso = k_name,
                2 => self.config.tool_vector_select = k_name,
                3 => self.config.tool_pen = k_name,
                4 => self.config.tool_brush = k_name,
                5 => self.config.tool_eraser = k_name,
                6 => self.config.tool_eraser_partial = k_name,
                7 => self.config.tool_bucket = k_name,
                8 => self.config.tool_eyedropper = k_name,
                9 => self.config.tool_text = k_name,
                10 => self.config.tool_shape = k_name,
                11 => self.config.tool_hand = k_name,
                12 => self.config.delete_frame_key = k_name,
                13 => self.config.insert_frame = k_name,
                14 => self.config.insert_keyframe = k_name,
                15 => self.config.insert_blank_kf = k_name,
                16 => self.config.convert_symbol = k_name,
                17 => self.config.play_pause = k_name,
                18 => self.config.prev_frame = k_name,
                19 => self.config.next_frame = k_name,
                20 => self.config.brush_smaller = k_name,
                21 => self.config.brush_larger = k_name,
                22 => self.config.reset_canvas = k_name,
                _ => {}
            }
            self.config.save();
            self.listening_shortcut_idx = None;
            typing_in_textbox = true;
        }

        let any_modal_open = self.show_symbol_dialog
            || self.show_text_dialog
            || self.show_export_dialog
            || self.show_exit_dialog
            || self.show_update_dialog
            || self.show_settings_dialog
            || self.show_timeline_settings
            || self.show_blender_locate_dialog
            || self.show_credits_dialog
            || self.renaming_layer_idx.is_some()
            || self.trimming_audio_idx.is_some();

        if any_modal_open {
            typing_in_textbox = true;
        }

        if !typing_in_textbox {
            if ctrl_z { self.undo(); }
            if ctrl_y { self.redo(); }
            if ctrl_s { self.trigger_save(); }
            if ctrl_c { self.copy_frame(self.active_layer_idx, self.current_frame); }
            if ctrl_v { self.paste_frame(self.active_layer_idx, self.current_frame); }

            if zoom_in_nav {
                self.zoom_level = (self.zoom_level + 0.05).clamp(0.1, 4.0);
                self.play_sfx(SfxType::ZoomStep);
            }
            if zoom_out_nav {
                self.zoom_level = (self.zoom_level - 0.05).clamp(0.1, 4.0);
                self.play_sfx(SfxType::ZoomStep);
            }
            if pan_right_nav {
                self.pan_offset.x += (self.project.width as f32) * 0.05 * self.zoom_level;
                self.play_sfx(SfxType::ZoomStep);
            }
            if pan_left_nav {
                self.pan_offset.x -= (self.project.width as f32) * 0.05 * self.zoom_level;
                self.play_sfx(SfxType::ZoomStep);
            }

            if self.workspace_mode == WorkspaceMode::AnimationStudio {
                if shift_f5 || sc_del_frame {
                    self.remove_frame_completely(self.active_layer_idx, self.current_frame);
                } else if sc_f5 {
                    self.action_insert_frame_f5();
                }
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

            if sc_reset_canvas {
                self.pan_offset = egui::Vec2::ZERO;
                self.zoom_level = 1.0;
                self.canvas_rotation_deg = 0.0;
                self.canvas_flip_h = false;
                self.canvas_flip_v = false;
                self.play_sfx(SfxType::ZoomStep);
                self.status_message = tr(lang, "Canvas Reset (100%)", "تم إعادة ضبط الكانفا");
            }
            if sc_select {
                self.active_tool_ui = ActiveToolUI::SelectNormal;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_lasso {
                self.active_tool_ui = ActiveToolUI::SelectLasso;
                self.play_sfx(SfxType::LassoSelect);
            }
            if sc_vec_sel {
                self.active_tool_ui = ActiveToolUI::VectorDirectSelect;
                self.play_sfx(SfxType::VectorPenTick);
            }
            if sc_pen {
                self.active_tool_ui = ActiveToolUI::VectorPen;
                self.play_sfx(SfxType::VectorPenTick);
            }
            if sc_brush {
                self.active_tool_ui = ActiveToolUI::Brush;
                self.play_sfx(SfxType::BrushScratch);
            }
            if sc_eraser {
                self.active_tool_ui = ActiveToolUI::EraserRed;
                self.play_sfx(SfxType::EraserRub);
            }
            if sc_eraser_partial {
                self.active_tool_ui = ActiveToolUI::EraserGreen;
                self.play_sfx(SfxType::EraserRub);
            }
            if sc_bucket {
                self.active_tool_ui = ActiveToolUI::Bucket;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_eyedropper {
                self.active_tool_ui = ActiveToolUI::Eyedropper;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_text {
                self.active_tool_ui = ActiveToolUI::Text;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_shape {
                self.active_tool_ui = ActiveToolUI::ShapeLine;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_hand {
                self.active_tool_ui = ActiveToolUI::HandCanvas;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_smaller { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if sc_larger { self.brush_size = (self.brush_size + 2.0).min(100.0); }

            if del_pressed {
                if let Some(t_idx) = self.selected_text_idx.take() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if t_idx < frame.text_items.len() {
                                frame.text_items.remove(t_idx);
                                self.has_unsaved_changes = true;
                                self.play_sfx(SfxType::EraserRub);
                            }
                        }
                    }
                } else if let Some(idx) = self.selected_symbol_inst_idx.take() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if idx < frame.symbol_instances.len() {
                                frame.symbol_instances.remove(idx);
                                self.has_unsaved_changes = true;
                                self.play_sfx(SfxType::EraserRub);
                            }
                        }
                    }
                } else if !self.selected_stroke_indices.is_empty() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            let mut to_remove = self.selected_stroke_indices.clone();
                            to_remove.sort_unstable_by(|a, b| b.cmp(a));
                            to_remove.dedup();
                            for idx in to_remove {
                                if idx < frame.strokes.len() {
                                    frame.strokes.remove(idx);
                                }
                            }
                            self.selected_stroke_indices.clear();
                            self.has_unsaved_changes = true;
                            self.play_sfx(SfxType::EraserRub);
                        }
                    }
                } else {
                    self.clear_frame_content(self.active_layer_idx, self.current_frame);
                }
            }
        }

        if self.is_playing && self.workspace_mode == WorkspaceMode::AnimationStudio {
            if now - self.last_frame_time >= 1.0 / (self.project.fps.max(1) as f64) {
                self.current_frame += 1;
                if self.current_frame > self.timeline_length {
                    self.current_frame = 1;
                    self.stop_audio_playback();
                    self.start_audio_playback();
                }
                self.last_frame_time = now;
            }
            ctx.request_repaint();
        }

        // نافذة تحديد مسار Blender يدوياً إذا لم يُكتشف تلقائياً
        if self.show_blender_locate_dialog {
            egui::Window::new(tr(lang, "Locate Blender Executable", "تحديد مسار برنامج Blender"))
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(fix_ar("اختر ملف blender.exe من مجلد تثبيت Blender على جهازك:"));
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.blender_path_buf);
                        if ui.button(fix_ar("استعراض...")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Blender", &["exe"]).pick_file() {
                                self.blender_path_buf = p.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button(fix_ar("حفظ المسار")).clicked() {
                            if !self.blender_path_buf.trim().is_empty() {
                                self.config.custom_blender_path = Some(self.blender_path_buf.trim().to_string());
                                self.config.save();
                            }
                            self.show_blender_locate_dialog = false;
                        }
                        if ui.button(fix_ar("إلغاء")).clicked() {
                            self.show_blender_locate_dialog = false;
                        }
                    });
                });
        }

        // نافذة إعدادات التايم لاين (ألوان التايم لاين)
        if self.show_timeline_settings {
            egui::Window::new(tr(lang, "Timeline Settings", "إعدادات التايم لاين"))
                .collapsible(false)
                .resizable(false)
                .default_width(320.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Timeline Background:", "لون خلفية التايم لاين:"));
                        if ui.color_edit_button_srgba_unmultiplied(&mut self.config.timeline_bg_color).changed() {
                            self.config.save();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Empty Frame Color:", "لون الفريمات الفارغة:"));
                        if ui.color_edit_button_srgba_unmultiplied(&mut self.config.timeline_cell_color).changed() {
                            self.config.save();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Playhead Color:", "لون مؤشر الفريم الحالي:"));
                        if ui.color_edit_button_srgba_unmultiplied(&mut self.config.timeline_playhead_color).changed() {
                            self.config.save();
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Reset Colors", "استعادة الألوان الافتراضية")).clicked() {
                            self.config.timeline_bg_color = default_tl_bg();
                            self.config.timeline_cell_color = default_tl_cell();
                            self.config.timeline_playhead_color = default_tl_head();
                            self.config.save();
                        }
                        if ui.button(tr(lang, "Done", "تم")).clicked() {
                            self.show_timeline_settings = false;
                        }
                    });
                });
        }

        if self.show_export_dialog {
            egui::Window::new(tr(lang, "Export Video / Animation", "تصدير الفيديو بصيغ متعددة"))
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(lang, "Select Video Format:", "اختر صيغة الفيديو:"));
                    ui.add_space(6.0);
                    let formats = [
                        VideoExportFormat::Mp4,
                        VideoExportFormat::Mov,
                        VideoExportFormat::Avi,
                        VideoExportFormat::Mkv,
                        VideoExportFormat::Webm,
                        VideoExportFormat::Gif,
                    ];
                    for fmt in formats {
                        ui.radio_value(&mut self.selected_export_format, fmt, fix_ar(fmt.label()));
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Export Now...", "تصدير الآن...")).clicked() {
                            self.project.total_frames = self.timeline_length;
                            let ext = self.selected_export_format.ext();
                            let default_name = format!("Stick_Export.{}", ext);
                            if let Some(mut path) = rfd::FileDialog::new()
                                .add_filter("Video", &[ext])
                                .set_file_name(&default_name)
                                .save_file()
                            {
                                if path.extension().is_none() {
                                    path.set_extension(ext);
                                }
                                match exporter::export_video_multi_format(
                                    &self.project,
                                    &path,
                                    self.selected_export_format,
                                    self.stage_bg_color,
                                ) {
                                    Ok(_) => {
                                        self.play_sfx(SfxType::ExportSuccess);
                                        self.status_message = format!("Exported .{} Successfully!", ext.to_uppercase());
                                    }
                                    Err(e) => self.status_message = e,
                                }
                            }
                            self.show_export_dialog = false;
                        }
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                            self.show_export_dialog = false;
                        }
                    });
                });
        }

        if self.show_text_dialog {
            egui::Window::new(tr(lang, "Insert Text", "إضافة نص"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(lang, "Enter Text (Arabic or English):", "اكتب النص (عربي أو إنجليزي):"));
                    ui.text_edit_singleline(&mut self.text_input_buf);
                    if !self.text_input_buf.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Arabic Preview:", "معاينة النص:"));
                            ui.heading(fix_ar(&self.text_input_buf));
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Font Size:", "حجم الخط:"));
                        ui.add(egui::DragValue::new(&mut self.text_font_size).range(10.0..=180.0));
                    });
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Add to Canvas", "إضافة للرسم")).clicked() {
                            if !self.text_input_buf.trim().is_empty() {
                                self.save_undo_state();
                                self.ensure_editable_keyframe();
                                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                    if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                        frame.text_items.push(TextItem {
                                            content: self.text_input_buf.clone(),
                                            x: self.pending_text_pos.x,
                                            y: self.pending_text_pos.y,
                                            font_size: self.text_font_size,
                                            color: self.brush_color,
                                        });
                                        self.has_unsaved_changes = true;
                                        self.play_sfx(SfxType::NewFrame);
                                    }
                                }
                            }
                            self.text_input_buf.clear();
                            self.show_text_dialog = false;
                        }
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                            self.show_text_dialog = false;
                        }
                    });
                });
        }

        if let Some(l_idx) = self.renaming_layer_idx {
            egui::Window::new(tr(lang, "Rename Layer", "إعادة تسمية الطبقة"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut self.rename_layer_buf);
                    if !self.rename_layer_buf.is_empty() {
                        ui.label(fix_ar(&self.rename_layer_buf));
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Save", "حفظ")).clicked() {
                            if let Some(layer) = self.project.layers.get_mut(l_idx) {
                                layer.name = fix_ar(&self.rename_layer_buf);
                                self.has_unsaved_changes = true;
                            }
                            self.renaming_layer_idx = None;
                        }
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                            self.renaming_layer_idx = None;
                        }
                    });
                });
        }

        if let Some(a_idx) = self.trimming_audio_idx {
            egui::Window::new(tr(lang, "Trim Audio Track", "تقطيع الصوت على التايم لاين"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(track) = self.project.audio_tracks.get_mut(a_idx) {
                        ui.label(&track.name);
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Start Frame:", "فريم البداية:"));
                            ui.add(egui::DragValue::new(&mut track.start_frame).range(1..=self.timeline_length));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "End Frame:", "فريم النهاية:"));
                            ui.add(egui::DragValue::new(&mut track.end_frame).range(track.start_frame..=self.timeline_length.max(track.start_frame)));
                        });
                    }
                    if ui.button(tr(lang, "Done", "تم")).clicked() {
                        self.trimming_audio_idx = None;
                        self.has_unsaved_changes = true;
                    }
                });
        }

        if self.show_credits_dialog {
            egui::Window::new(tr(lang, "About Stick", "حول برنامج Stick"))
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.heading(egui::RichText::new("Made by Hady").color(egui::Color32::from_rgb(255, 215, 25)));
                        ui.add_space(4.0);
                        ui.label(format!("Stick 2D Animation Studio v{}", self.installed_version));
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(tr(lang, "Official YouTube Channel:", "قناة اليوتيوب الرسمية:"));
                        ui.add_space(4.0);
                        if ui.button(egui::RichText::new("▶ Open YouTube Channel").size(15.0).color(egui::Color32::WHITE))
                            .on_hover_text(YOUTUBE_CHANNEL_URL)
                            .clicked()
                        {
                            ctx.open_url(egui::OpenUrl::new_tab(YOUTUBE_CHANNEL_URL));
                        }
                        ui.add_space(6.0);
                        ui.hyperlink_to("youtube.com/@ابلكاش-انميشن", YOUTUBE_CHANNEL_URL);
                        ui.add_space(12.0);
                        if ui.button(tr(lang, "Close", "إغلاق")).clicked() {
                            self.show_credits_dialog = false;
                        }
                    });
                });
        }

        if self.show_settings_dialog {
            let mut sfx_changed = false;
            egui::Window::new(tr(lang, "Settings & System", "الإعدادات والنظام"))
                .collapsible(false)
                .resizable(false)
                .default_width(495.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.settings_tab == 0, tr(lang, "Shortcuts & Grid", "الاختصارات والمربعات")).clicked() {
                            self.settings_tab = 0;
                        }
                        if ui.selectable_label(self.settings_tab == 1, tr(lang, "[ SYSTEM ] Status", "[ النظام ] حالة اللاعب")).clicked() {
                            self.settings_tab = 1;
                        }
                    });
                    ui.separator();

                    if self.settings_tab == 0 {
                        ui.horizontal(|ui| {
                            ui.strong(tr(lang, "Interface Language:", "لغة الواجهة:"));
                            if ui.selectable_label(self.config.lang == AppLang::English, "English").clicked() {
                                self.config.lang = AppLang::English;
                                self.config.save();
                            }
                            if ui.selectable_label(self.config.lang == AppLang::Arabic, fix_ar("العربية")).clicked() {
                                self.config.lang = AppLang::Arabic;
                                self.config.save();
                            }
                        });
                        ui.add_space(4.0);
                        let sfx_label = tr(
                            lang,
                            "Fun Sound Effects (Tools, Frames, Level Up, Export, Exit)",
                            "المؤثرات الصوتية الترفيهية (الأدوات، الفريمات، ارتقاء المستوى، التصدير)",
                        );
                        if ui.checkbox(&mut self.config.sound_effects_enabled, sfx_label).changed() {
                            self.config.save();
                            sfx_changed = true;
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut self.config.show_grid, tr(lang, "Show Canvas Grid", "إظهار شبكة المربعات")).changed() {
                                self.config.save();
                            }
                            if self.config.show_grid {
                                ui.label(tr(lang, "Grid Size:", "حجم المربع:"));
                                if ui.add(egui::Slider::new(&mut self.config.grid_size, 10.0..=150.0)).changed() {
                                    self.config.save();
                                }
                            }
                        });

                        ui.separator();
                        let items = [
                            (0, tr(lang, "Select Tool (Normal)", "أداة التحديد العادي"), self.config.tool_select.clone()),
                            (1, tr(lang, "Area Select Tool (Lasso)", "أداة تحديد المنطقة الحرة"), self.config.tool_lasso.clone()),
                            (2, tr(lang, "Vector Direct Select", "أداة تحديد نقاط الفيكتور"), self.config.tool_vector_select.clone()),
                            (3, tr(lang, "Vector Pen Tool", "أداة قلم الفيكتور"), self.config.tool_pen.clone()),
                            (4, tr(lang, "Brush Tool", "أداة الفرشاة"), self.config.tool_brush.clone()),
                            (5, tr(lang, "Eraser Red (Full Stroke)", "الممحاة الحمراء (مسح الخط كامل)"), self.config.tool_eraser.clone()),
                            (6, tr(lang, "Eraser Green (Partial)", "الممحاة الخضراء (مسح جزء صغير)"), self.config.tool_eraser_partial.clone()),
                            (7, tr(lang, "Paint Bucket Tool", "أداة دلو التعبئة"), self.config.tool_bucket.clone()),
                            (8, tr(lang, "Eyedropper Tool", "أداة قطارة الألوان"), self.config.tool_eyedropper.clone()),
                            (9, tr(lang, "Text Tool", "أداة الكتابة (النص)"), self.config.tool_text.clone()),
                            (10, tr(lang, "Shape / Line Tool", "أداة الخطوط والأشكال الهندسية"), self.config.tool_shape.clone()),
                            (11, tr(lang, "Hand Tool (Rotate/Flip)", "أداة اليد (تدوير وعكس الكانفا)"), self.config.tool_hand.clone()),
                            (12, tr(lang, "Delete Frame Completely", "حذف الفريم نهائياً"), self.config.delete_frame_key.clone()),
                            (13, tr(lang, "Insert Frame (Extend)", "تمديد الفريم الحالي"), self.config.insert_frame.clone()),
                            (14, tr(lang, "Insert Keyframe (Duplicate)", "إدراج فريم منسوخ"), self.config.insert_keyframe.clone()),
                            (15, tr(lang, "Insert Blank Keyframe", "إدراج فريم فارغ جديد"), self.config.insert_blank_kf.clone()),
                            (16, tr(lang, "Convert to Symbol", "تحويل إلى رمز (Symbol)"), self.config.convert_symbol.clone()),
                            (17, tr(lang, "Play / Pause", "تشغيل / إيقاف"), self.config.play_pause.clone()),
                            (18, tr(lang, "Previous Frame", "الفريم السابق"), self.config.prev_frame.clone()),
                            (19, tr(lang, "Next Frame", "الفريم التالي"), self.config.next_frame.clone()),
                            (20, tr(lang, "Decrease Brush Size", "تصغير حجم الفرشاة"), self.config.brush_smaller.clone()),
                            (21, tr(lang, "Increase Brush Size", "تكبير حجم الفرشاة"), self.config.brush_larger.clone()),
                            (22, tr(lang, "Reset Canvas View", "إرجاع الكانفا للمنتصف"), self.config.reset_canvas.clone()),
                        ];
                        let press_key_txt = tr(lang, "Press any key...", "اضغط أي زر...");
                        egui::ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                            for (idx, label, current_key) in items {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui(egui::vec2(250.0, 22.0), |ui| {
                                        ui.label(label);
                                    });
                                    let is_listening = self.listening_shortcut_idx == Some(idx);
                                    let btn_text = if is_listening { press_key_txt.clone() } else { current_key };
                                    let btn = egui::Button::new(btn_text)
                                        .min_size(egui::vec2(125.0, 22.0))
                                        .fill(if is_listening {
                                            egui::Color32::from_rgb(200, 90, 20)
                                        } else {
                                            egui::Color32::from_rgb(55, 55, 62)
                                        });
                                    if ui.add(btn).clicked() {
                                        self.listening_shortcut_idx = Some(idx);
                                    }
                                });
                            }
                        });
                    } else {
                        let (level, cur_xp, req_xp) = self.user_profile.get_level_progress();
                        let rank = self.user_profile.get_hunter_rank();

                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(10, 18, 36))
                            .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(56, 189, 248)))
                            .inner_margin(14.0)
                            .rounding(6.0)
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        egui::RichText::new("[ SYSTEM : PLAYER STATUS ]")
                                            .size(16.0)
                                            .color(egui::Color32::from_rgb(56, 189, 248)),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    let (av_rect, _) = ui.allocate_exact_size(egui::vec2(76.0, 76.0), egui::Sense::hover());
                                    ui.painter().rect_stroke(av_rect.expand(2.0), 6.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(56, 189, 248)));
                                    if let Some(ref tex) = self.avatar_texture {
                                        ui.painter().image(
                                            tex.id(),
                                            av_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(av_rect, 4.0, egui::Color32::from_rgb(18, 42, 85));
                                        ui.painter().text(
                                            av_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "HUNTER",
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::from_rgb(125, 211, 252),
                                        );
                                    }

                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.colored_label(egui::Color32::from_rgb(125, 211, 252), "NAME:");
                                            if ui.text_edit_singleline(&mut self.user_profile.username).changed() {
                                                self.user_profile.save();
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.colored_label(egui::Color32::from_rgb(125, 211, 252), "JOB:");
                                            ui.colored_label(egui::Color32::WHITE, "Shadow Animator");
                                            ui.separator();
                                            ui.colored_label(egui::Color32::from_rgb(250, 204, 21), format!("RANK: {}", rank));
                                        });
                                        if ui.button(tr(lang, "Change Avatar...", "تغيير صورة الشخصية...")).clicked() {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                                .pick_file()
                                            {
                                                let p_str = path.to_string_lossy().to_string();
                                                if let Ok(img) = image::open(&path) {
                                                    let rgba = img.to_rgba8();
                                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                                        [rgba.width() as usize, rgba.height() as usize],
                                                        rgba.as_raw(),
                                                    );
                                                    self.avatar_texture = Some(ctx.load_texture("user_avatar", color_image, egui::TextureOptions::LINEAR));
                                                    self.user_profile.avatar_path = Some(p_str);
                                                    self.user_profile.save();
                                                }
                                            }
                                        }
                                    });
                                });

                                ui.add_space(10.0);
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading(
                                        egui::RichText::new(format!("LEVEL  {} / 100", level))
                                            .color(egui::Color32::from_rgb(56, 189, 248)),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.colored_label(egui::Color32::from_rgb(125, 211, 252), format!("EXP: {} / {}", cur_xp, req_xp));
                                    });
                                });
                                let ratio = (cur_xp as f32 / req_xp.max(1) as f32).clamp(0.0, 1.0);
                                ui.add(
                                    egui::ProgressBar::new(ratio)
                                        .fill(egui::Color32::from_rgb(14, 165, 233)),
                                );

                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.colored_label(egui::Color32::from_rgb(125, 211, 252), tr(lang, "ACTIVE TIME:", "مدة النشاط:"));
                                    let pt = self.user_profile.format_playtime(lang == AppLang::Arabic);
                                    ui.colored_label(egui::Color32::WHITE, if lang == AppLang::Arabic { fix_ar(&pt) } else { pt });
                                });
                                ui.horizontal(|ui| {
                                    ui.colored_label(egui::Color32::from_rgb(125, 211, 252), tr(lang, "FRAMES FORGED:", "الفريمات المنجزة:"));
                                    ui.colored_label(egui::Color32::WHITE, format!("{}", self.user_profile.total_frames_drawn));
                                });
                            });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if self.settings_tab == 0 && ui.button(tr(lang, "Reset Defaults", "استعادة الافتراضي")).clicked() {
                            let saved_lang = self.config.lang;
                            let saved_sfx = self.config.sound_effects_enabled;
                            self.config = AppConfig::default();
                            self.config.lang = saved_lang;
                            self.config.sound_effects_enabled = saved_sfx;
                            self.config.save();
                            self.listening_shortcut_idx = None;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(tr(lang, "Done", "تم")).clicked() {
                                self.listening_shortcut_idx = None;
                                self.show_settings_dialog = false;
                            }
                        });
                    });
                });
            if sfx_changed && self.config.sound_effects_enabled {
                self.play_sfx(SfxType::NewFrame);
            }
        }

        if self.show_update_dialog {
            let current_st = self.update_state.lock().unwrap().clone();
            egui::Window::new(tr(lang, "Stick Updater", "تحديث البرنامج"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Current Version: v{}", self.installed_version));
                    ui.separator();
                    match current_st {
                        UpdateState::Idle => {}
                        UpdateState::Checking => {
                            ui.horizontal(|ui| { ui.spinner(); ui.label(tr(lang, "Checking GitHub...", "جاري البحث عن تحديثات...")); });
                        }
                        UpdateState::UpToDate => {
                            ui.colored_label(egui::Color32::GREEN, tr(lang, "You are on the latest version!", "لديك أحدث إصدار بالفعل!"));
                            if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_update_dialog = false; }
                        }
                        UpdateState::UpdateAvailable { version, download_url } => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, format!("New Version: v{}", version));
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "Download & Install", "تحميل وتثبيت")).clicked() {
                                    self.download_and_apply_update(version, download_url, ctx.clone());
                                }
                                if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() { self.show_update_dialog = false; }
                            });
                        }
                        UpdateState::Downloading(msg) => {
                            ui.horizontal(|ui| { ui.spinner(); ui.label(msg); });
                        }
                        UpdateState::ReadyToRestart => {
                            ui.colored_label(egui::Color32::GREEN, tr(lang, "Update ready! Restart now?", "اكتمل التحميل! إعادة التشغيل الآن؟"));
                            if ui.button(tr(lang, "Restart & Apply", "إعادة التشغيل والتحديث")).clicked() { self.restart_with_new_exe(); }
                        }
                        UpdateState::Error(err) => {
                            ui.colored_label(egui::Color32::RED, err);
                            if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_update_dialog = false; }
                        }
                    }
                });
        }

        if self.show_symbol_dialog {
            egui::Window::new(tr(lang, "Convert to Symbol", "تحويل إلى رمز"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(lang, "Symbol Name:", "اسم الرمز:"));
                    ui.text_edit_singleline(&mut self.new_symbol_name);
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Create", "إنشاء")).clicked() {
                            self.save_undo_state();
                            self.ensure_editable_keyframe();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    if !frame.strokes.is_empty() {
                                        let sym = Symbol {
                                            name: self.new_symbol_name.clone(),
                                            strokes: frame.strokes.clone(),
                                        };
                                        self.project.library_symbols.insert(self.new_symbol_name.clone(), sym);
                                        frame.strokes.clear();
                                        frame.symbol_instances.push(SymbolInstance {
                                            symbol_name: self.new_symbol_name.clone(),
                                            x: 0.0,
                                            y: 0.0,
                                            scale: 1.0,
                                        });
                                        self.has_unsaved_changes = true;
                                        self.play_sfx(SfxType::NewFrame);
                                    }
                                }
                            }
                            self.show_symbol_dialog = false;
                        }
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() { self.show_symbol_dialog = false; }
                    });
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
            egui::Window::new(tr(lang, "Unsaved Changes", "تغييرات غير محفوظة"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(lang, "Save changes before closing?", "هل تريد حفظ المشروع قبل الإغلاق؟"));
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Save & Exit", "حفظ وخروج")).clicked() && self.trigger_save() {
                            play_sfx_async(self.config.sound_effects_enabled, SfxType::ExitFarewell, true);
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(tr(lang, "Don't Save", "عدم الحفظ")).clicked() {
                            play_sfx_async(self.config.sound_effects_enabled, SfxType::ExitFarewell, true);
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }

        // الشريط العلوي
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let (heart_rect, heart_resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
                if heart_resp.hovered() {
                    ui.painter().rect_filled(heart_rect, 4.0, egui::Color32::from_gray(55));
                }
                draw_red_heart_icon(ui.painter(), heart_rect, heart_resp.hovered());
                if heart_resp.on_hover_text("Made by Hady - Credits").clicked() {
                    self.play_sfx(SfxType::UiClick);
                    self.show_credits_dialog = true;
                }

                ui.colored_label(egui::Color32::from_rgb(255, 215, 25), "Stick");
                ui.separator();

                let mode_btn_lbl = if self.workspace_mode == WorkspaceMode::DrawingOnly {
                    tr(lang, "Switch to Animation", "التبديل للأنيميشن")
                } else {
                    tr(lang, "Switch to Drawing", "التبديل للرسم")
                };
                if ui.button(mode_btn_lbl).clicked() {
                    self.play_sfx(SfxType::UiClick);
                    self.workspace_mode = if self.workspace_mode == WorkspaceMode::DrawingOnly {
                        WorkspaceMode::AnimationStudio
                    } else {
                        WorkspaceMode::DrawingOnly
                    };
                }
                ui.separator();

                ui.menu_button(tr(lang, "File", "ملف"), |ui| {
                    if ui.button(tr(lang, "Save Project (Ctrl+S)", "حفظ المشروع")).clicked() { self.trigger_save(); ui.close_menu(); }
                    if ui.button(tr(lang, "Open Project (.stick / .ah2d)", "فتح مشروع")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Stick Project", &["stick", "ah2d"]).pick_file() {
                            match AniProject::load_from_file(&path) {
                                Ok(proj) => {
                                    self.timeline_length = proj.total_frames.max(1);
                                    self.current_frame = 1;
                                    self.active_layer_idx = 0;
                                    self.selected_stroke_indices.clear();
                                    self.selected_text_idx = None;
                                    self.selected_symbol_inst_idx = None;
                                    self.project = proj;
                                    self.has_unsaved_changes = false;
                                    self.play_sfx(SfxType::NewFrame);
                                    self.status_message = tr(lang, "Project Loaded!", "تم فتح المشروع بنجاح!");
                                }
                                Err(err) => {
                                    self.status_message = err;
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr(lang, "Import Image...", "استيراد صورة...")).clicked() { self.import_image_dialog(); ui.close_menu(); }
                    if self.workspace_mode == WorkspaceMode::DrawingOnly {
                        if ui.button(tr(lang, "Export Transparent PNG...", "حفظ الرسمة كصورة شفافة PNG...")).clicked() {
                            if let Some(mut p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("drawing.png").save_file() {
                                if p.extension().is_none() { p.set_extension("png"); }
                                match exporter::export_single_png(&self.project, self.current_frame, &p, true, self.stage_bg_color) {
                                    Ok(_) => {
                                        self.play_sfx(SfxType::ExportSuccess);
                                        self.status_message = tr(lang, "Saved PNG!", "تم حفظ الصورة الشفافة!");
                                    }
                                    Err(e) => self.status_message = e,
                                }
                            }
                            ui.close_menu();
                        }
                    } else {
                        if ui.button(tr(lang, "Import Video...", "استيراد فيديو...")).clicked() { self.import_video_dialog(); ui.close_menu(); }
                        if ui.button(tr(lang, "Import Blender 2D (.blend)...", "استيراد مشروع بلندر (.blend)...")).clicked() {
                            self.import_blender_dialog();
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Import Audio...", "استيراد صوت...")).clicked() { self.import_audio_dialog(); ui.close_menu(); }
                        ui.separator();
                        if ui.button(tr(lang, "Export Transparent PNGs (Premiere)...", "تصدير صور شفافة (PNG Sequence)...")).clicked() {
                            self.project.total_frames = self.timeline_length;
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                match exporter::export_transparent_png_sequence(&self.project, &folder) {
                                    Ok(n) => {
                                        self.play_sfx(SfxType::ExportSuccess);
                                        self.status_message = format!("Exported {} Transparent PNGs!", n);
                                    }
                                    Err(e) => self.status_message = e,
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Export Video (MP4, MOV, AVI, MKV, GIF)...", "تصدير فيديو بجميع الصيغ...")).clicked() {
                            self.show_export_dialog = true;
                            ui.close_menu();
                        }
                    }
                });

                ui.separator();
                let can_undo = !self.undo_stack.is_empty();
                let (u_rect, u_resp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), egui::Sense::click());
                if u_resp.hovered() && can_undo {
                    ui.painter().rect_filled(u_rect, 4.0, egui::Color32::from_gray(55));
                }
                draw_undo_redo_arrow(ui.painter(), u_rect, true, can_undo);
                if u_resp.on_hover_text("Undo (Ctrl+Z)").clicked() && can_undo {
                    self.undo();
                }

                let can_redo = !self.redo_stack.is_empty();
                let (r_rect, r_resp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), egui::Sense::click());
                if r_resp.hovered() && can_redo {
                    ui.painter().rect_filled(r_rect, 4.0, egui::Color32::from_gray(55));
                }
                draw_undo_redo_arrow(ui.painter(), r_rect, false, can_redo);
                if r_resp.on_hover_text("Redo (Ctrl+Y)").clicked() && can_redo {
                    self.redo();
                }

                ui.separator();
                if ui.button(tr(lang, "+ Image", "+ صورة")).clicked() { self.import_image_dialog(); }
                if self.workspace_mode == WorkspaceMode::AnimationStudio {
                    if ui.button(tr(lang, "+ Video", "+ فيديو")).clicked() { self.import_video_dialog(); }
                    if ui.button("+ .blend").on_hover_text("Import Blender 2D Project").clicked() { self.import_blender_dialog(); }
                    if ui.button(tr(lang, "+ Audio", "+ صوت")).clicked() { self.import_audio_dialog(); }
                }
                ui.separator();
                if ui.button(tr(lang, "Updates", "تحديثات")).clicked() { self.check_for_updates(ctx.clone()); }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (gear_rect, gear_resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                    if gear_resp.hovered() {
                        ui.painter().rect_filled(gear_rect, 4.0, egui::Color32::from_gray(65));
                    }
                    draw_gear_icon(ui.painter(), gear_rect, 1.0);
                    if gear_resp.on_hover_text(tr(lang, "Settings & System", "الإعدادات والنظام")).clicked() {
                        self.play_sfx(SfxType::UiClick);
                        self.show_settings_dialog = true;
                    }

                    ui.separator();
                    if ui.small_button("+5%").clicked() {
                        self.zoom_level = (self.zoom_level + 0.05).clamp(0.1, 4.0);
                        self.play_sfx(SfxType::ZoomStep);
                    }
                    ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                    if ui.small_button("-5%").clicked() {
                        self.zoom_level = (self.zoom_level - 0.05).clamp(0.1, 4.0);
                        self.play_sfx(SfxType::ZoomStep);
                    }
                    if ui.small_button(format!("{} ({})", tr(lang, "Center", "توسيط"), self.config.reset_canvas)).clicked() {
                        self.zoom_level = 1.0;
                        self.pan_offset = egui::Vec2::ZERO;
                        self.canvas_rotation_deg = 0.0;
                        self.canvas_flip_h = false;
                        self.canvas_flip_v = false;
                        self.play_sfx(SfxType::ZoomStep);
                    }
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_GRAY, fix_ar(&self.status_message));
                });
            });
        });

        // شريط الأدوات الأيسر
        egui::SidePanel::left("tools_panel")
            .resizable(true)
            .default_width(64.0)
            .width_range(52.0..=220.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let select_ui = match self.active_tool_ui {
                        ActiveToolUI::SelectLasso => ActiveToolUI::SelectLasso,
                        ActiveToolUI::VectorDirectSelect => ActiveToolUI::VectorDirectSelect,
                        _ => ActiveToolUI::SelectNormal,
                    };
                    let select_label = match select_ui {
                        ActiveToolUI::SelectLasso => tr(lang, "Area Select", "تحديد منطقة"),
                        ActiveToolUI::VectorDirectSelect => tr(lang, "Vector Select", "نقاط فيكتور"),
                        _ => tr(lang, "Select", "تحديد عادي"),
                    };
                    let select_sc = match select_ui {
                        ActiveToolUI::SelectLasso => self.config.tool_lasso.clone(),
                        ActiveToolUI::VectorDirectSelect => self.config.tool_vector_select.clone(),
                        _ => self.config.tool_select.clone(),
                    };

                    let eraser_ui = if self.active_tool_ui == ActiveToolUI::EraserGreen {
                        ActiveToolUI::EraserGreen
                    } else {
                        ActiveToolUI::EraserRed
                    };
                    let eraser_label = if eraser_ui == ActiveToolUI::EraserGreen {
                        tr(lang, "Partial Eraser", "ممحاة جزئية")
                    } else {
                        tr(lang, "Stroke Eraser", "ممحاة كاملة")
                    };
                    let eraser_sc = if eraser_ui == ActiveToolUI::EraserGreen {
                        self.config.tool_eraser_partial.clone()
                    } else {
                        self.config.tool_eraser.clone()
                    };

                    let tools = [
                        (select_ui, select_label, select_sc),
                        (ActiveToolUI::VectorPen, tr(lang, "Vector Pen", "قلم فيكتور"), self.config.tool_pen.clone()),
                        (ActiveToolUI::Brush, tr(lang, "Brush", "فرشاة"), self.config.tool_brush.clone()),
                        (eraser_ui, eraser_label, eraser_sc),
                        (ActiveToolUI::Bucket, tr(lang, "Bucket", "تعبئة"), self.config.tool_bucket.clone()),
                        (ActiveToolUI::Eyedropper, tr(lang, "Picker", "قطارة"), self.config.tool_eyedropper.clone()),
                        (ActiveToolUI::Text, tr(lang, "Text", "نص"), self.config.tool_text.clone()),
                        (ActiveToolUI::ShapeLine, tr(lang, "Shapes / Line", "خطوط وأشكال"), self.config.tool_shape.clone()),
                        (ActiveToolUI::HandCanvas, tr(lang, "Hand / Flip", "اليد / عكس وتدوير"), self.config.tool_hand.clone()),
                    ];
                    let show_labels = ui.available_width() > 95.0;
                    for (tool, name, sc) in tools {
                        let selected = self.active_tool_ui == tool;
                        let accent = get_tool_ui_color(tool);
                        let btn_w = if show_labels { ui.available_width() - 8.0 } else { 42.0 };
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(btn_w, 34.0), egui::Sense::click());
                        let bg = if selected {
                            accent
                        } else if resp.hovered() {
                            egui::Color32::from_rgb(62, 62, 68)
                        } else {
                            egui::Color32::from_rgb(44, 44, 48)
                        };
                        ui.painter().rect_filled(rect, 6.0, bg);
                        let icon_rect = egui::Rect::from_min_size(rect.min, egui::vec2(42.0, 34.0));
                        draw_tool_ui_icon(ui.painter(), icon_rect, tool, selected);
                        if show_labels {
                            ui.painter().text(
                                icon_rect.right_center() + egui::vec2(4.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{} ({})", name, sc),
                                egui::FontId::proportional(13.0),
                                egui::Color32::WHITE,
                            );
                        }
                        if resp.clicked() {
                            self.active_tool_ui = tool;
                            match tool {
                                ActiveToolUI::VectorPen | ActiveToolUI::VectorDirectSelect => self.play_sfx(SfxType::VectorPenTick),
                                ActiveToolUI::Brush => self.play_sfx(SfxType::BrushScratch),
                                ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => self.play_sfx(SfxType::EraserRub),
                                ActiveToolUI::SelectLasso => self.play_sfx(SfxType::LassoSelect),
                                _ => self.play_sfx(SfxType::UiClick),
                            }
                        }
                        if matches!(tool, ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso | ActiveToolUI::VectorDirectSelect) {
                            let lbl_norm = tr(lang, "Normal Select (V)", "تحديد عادي (V)");
                            let lbl_area = tr(lang, "Area / Lasso Select (L)", "تحديد منطقة حرة (L)");
                            let lbl_vec = tr(lang, "Vector Direct Select (Q)", "تحديد نقاط الفيكتور (Q)");
                            resp.context_menu(|ui| {
                                if ui.selectable_label(self.active_tool_ui == ActiveToolUI::SelectNormal, &lbl_norm).clicked() {
                                    self.active_tool_ui = ActiveToolUI::SelectNormal;
                                    self.play_sfx(SfxType::UiClick);
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.active_tool_ui == ActiveToolUI::SelectLasso, &lbl_area).clicked() {
                                    self.active_tool_ui = ActiveToolUI::SelectLasso;
                                    self.play_sfx(SfxType::LassoSelect);
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.active_tool_ui == ActiveToolUI::VectorDirectSelect, &lbl_vec).clicked() {
                                    self.active_tool_ui = ActiveToolUI::VectorDirectSelect;
                                    self.play_sfx(SfxType::VectorPenTick);
                                    ui.close_menu();
                                }
                            });
                        }
                        if matches!(tool, ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen) {
                            let lbl_red = tr(lang, "Red Eraser: Full Stroke (E)", "الممحاة الحمراء: مسح الخط كامل (E)");
                            let lbl_green = tr(lang, "Green Eraser: Partial (T)", "الممحاة الخضراء: مسح جزء صغير (T)");
                            resp.context_menu(|ui| {
                                if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserRed, &lbl_red).clicked() {
                                    self.active_tool_ui = ActiveToolUI::EraserRed;
                                    self.play_sfx(SfxType::EraserRub);
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserGreen, &lbl_green).clicked() {
                                    self.active_tool_ui = ActiveToolUI::EraserGreen;
                                    self.play_sfx(SfxType::EraserRub);
                                    ui.close_menu();
                                }
                            });
                        }
                        if tool == ActiveToolUI::ShapeLine {
                            resp.context_menu(|ui| {
                                if ui.selectable_label(self.selected_shape_kind == GeometricShapeKind::StraightLine, tr(lang, "Straight Line", "خط مستقيم")).clicked() {
                                    self.active_tool_ui = ActiveToolUI::ShapeLine;
                                    self.selected_shape_kind = GeometricShapeKind::StraightLine;
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.selected_shape_kind == GeometricShapeKind::Circle, tr(lang, "Circle", "دائرة مضبوطة")).clicked() {
                                    self.active_tool_ui = ActiveToolUI::ShapeLine;
                                    self.selected_shape_kind = GeometricShapeKind::Circle;
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.selected_shape_kind == GeometricShapeKind::Square, tr(lang, "Square", "مربع")).clicked() {
                                    self.active_tool_ui = ActiveToolUI::ShapeLine;
                                    self.selected_shape_kind = GeometricShapeKind::Square;
                                    ui.close_menu();
                                }
                                if ui.selectable_label(self.selected_shape_kind == GeometricShapeKind::Rectangle, tr(lang, "Rectangle", "مستطيل")).clicked() {
                                    self.active_tool_ui = ActiveToolUI::ShapeLine;
                                    self.selected_shape_kind = GeometricShapeKind::Rectangle;
                                    ui.close_menu();
                                }
                            });
                        }
                        if tool == ActiveToolUI::HandCanvas {
                            resp.context_menu(|ui| {
                                if ui.button(tr(lang, "Flip Horizontal (Left/Right)", "عكس أفقي (يمين / شمال)")).clicked() {
                                    self.canvas_flip_h = !self.canvas_flip_h;
                                    self.play_sfx(SfxType::ZoomStep);
                                    ui.close_menu();
                                }
                                if ui.button(tr(lang, "Flip Vertical (Up/Down)", "عكس رأسي (فوق / تحت)")).clicked() {
                                    self.canvas_flip_v = !self.canvas_flip_v;
                                    self.play_sfx(SfxType::ZoomStep);
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(tr(lang, "Rotate +15°", "تدوير +15 درجة")).clicked() {
                                    self.canvas_rotation_deg = (self.canvas_rotation_deg + 15.0) % 360.0;
                                    self.play_sfx(SfxType::ZoomStep);
                                    ui.close_menu();
                                }
                                if ui.button(tr(lang, "Rotate -15°", "تدوير -15 درجة")).clicked() {
                                    self.canvas_rotation_deg = (self.canvas_rotation_deg - 15.0) % 360.0;
                                    self.play_sfx(SfxType::ZoomStep);
                                    ui.close_menu();
                                }
                                if ui.button(tr(lang, "Reset Orientation", "إعادة الوضع الأصلي")).clicked() {
                                    self.canvas_rotation_deg = 0.0;
                                    self.canvas_flip_h = false;
                                    self.canvas_flip_v = false;
                                    self.play_sfx(SfxType::ZoomStep);
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                    ui.separator();
                    ui.small(tr(lang, "Color", "اللون"));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
                    ui.add_space(4.0);
                    if ui.small_button("+Brush").on_hover_text("Import Brush").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Brush", &["png", "abr"]).pick_file() {
                            self.imported_brushes.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                        }
                    }
                });
            });

        // اللوحة اليمنى: مدعومة بخصائص مخصصة لوضع الرسم ووضع الأنيميشن
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(255.0)
            .width_range(180.0..=480.0)
            .show(ctx, |ui| {
                if self.workspace_mode == WorkspaceMode::DrawingOnly {
                    ui.strong(tr(lang, "🎨 Drawing Studio Tools", "🎨 استوديو الرسم الاحترافي"));
                    ui.separator();

                    // عرض خصائص الأداة المختارة حالياً في وضع الرسم أيضاً
                    self.render_tool_properties_ui(ui, lang);

                    ui.add_space(8.0);
                    ui.separator();
                    ui.strong(tr(lang, "Symmetry Mirroring", "الرسم المتماثل (المرآة)"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Off, tr(lang, "Off", "إيقاف"));
                        ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Horizontal, tr(lang, "Horizontal", "أفقي"));
                        ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Vertical, tr(lang, "Vertical", "رأسي"));
                    });

                    ui.add_space(6.0);
                    ui.strong(tr(lang, "Quick Color Swatches", "ألوان سريعة"));
                    let swatches: [[u8; 4]; 8] = [
                        [0, 0, 0, 255],
                        [255, 255, 255, 255],
                        [239, 68, 68, 255],
                        [249, 115, 22, 255],
                        [250, 204, 21, 255],
                        [34, 197, 94, 255],
                        [56, 189, 248, 255],
                        [168, 85, 247, 255],
                    ];
                    ui.horizontal(|ui| {
                        for col in swatches {
                            let c32 = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 255);
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(r, 4.0, c32);
                            ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                            if resp.clicked() {
                                self.brush_color = col;
                                self.play_sfx(SfxType::UiClick);
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.strong(tr(lang, "Layers", "الطبقات"));
                        if ui.small_button(tr(lang, "+ Layer", "+ طبقة")).clicked() {
                            self.save_undo_state();
                            let count = self.project.layers.len() + 1;
                            let mut kf = HashMap::new();
                            kf.insert(1, Default::default());
                            self.project.layers.push(Layer {
                                name: format!("Layer_{}", count),
                                visible: true,
                                locked: false,
                                keyframes: kf,
                            });
                            self.play_sfx(SfxType::NewFrame);
                        }
                    });
                    let mut del_l = None;
                    let l_len = self.project.layers.len();
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        for (idx, layer) in self.project.layers.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut layer.visible, "");
                                if ui.selectable_label(self.active_layer_idx == idx, &layer.name).clicked() {
                                    self.active_layer_idx = idx;
                                }
                                if l_len > 1 && ui.small_button("X").clicked() {
                                    del_l = Some(idx);
                                }
                            });
                        }
                    });
                    if let Some(idx) = del_l {
                        self.save_undo_state();
                        self.project.layers.remove(idx);
                        if self.active_layer_idx >= self.project.layers.len() {
                            self.active_layer_idx = self.project.layers.len().saturating_sub(1);
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    if ui.button(tr(lang, "💾 Export Transparent PNG", "💾 تصدير الرسمة PNG شفاف")).clicked() {
                        if let Some(mut p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("drawing.png").save_file() {
                            if p.extension().is_none() { p.set_extension("png"); }
                            match exporter::export_single_png(&self.project, self.current_frame, &p, true, self.stage_bg_color) {
                                Ok(_) => {
                                    self.play_sfx(SfxType::ExportSuccess);
                                    self.status_message = tr(lang, "Saved PNG!", "تم حفظ الصورة الشفافة!");
                                }
                                Err(e) => self.status_message = e,
                            }
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.right_tab == 0, tr(lang, "Properties", "الخصائص")).clicked() { self.right_tab = 0; }
                        if ui.selectable_label(self.right_tab == 1, tr(lang, "Library & Audio", "المكتبة والصوت")).clicked() { self.right_tab = 1; }
                    });
                    ui.separator();

                    if self.right_tab == 0 {
                        self.render_tool_properties_ui(ui, lang);

                        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                            if let Some(fd) = layer.keyframes.get_mut(&self.current_frame) {
                                if !fd.symbol_instances.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.checkbox(&mut fd.motion_tween, tr(lang, "Motion Tween (Auto Animate)", "تحريك تلقائي (Motion Tween)"));
                                }
                            }
                        }

                        if let Some(a_idx) = self.selected_audio_idx {
                            if let Some(track) = self.project.audio_tracks.get_mut(a_idx) {
                                ui.add_space(10.0);
                                ui.separator();
                                ui.strong(tr(lang, "Selected Audio Properties", "خصائص الصوت المحدد"));
                                ui.label(&track.name);
                                ui.horizontal(|ui| {
                                    ui.label(tr(lang, "Sync Mode:", "وضع الصوت:"));
                                    ui.selectable_value(&mut track.sync_mode, AudioSyncMode::Stream, "Stream");
                                    ui.selectable_value(&mut track.sync_mode, AudioSyncMode::Normal, "Normal");
                                });
                                ui.horizontal(|ui| {
                                    ui.label(tr(lang, "Range:", "النطاق:"));
                                    ui.add(egui::DragValue::new(&mut track.start_frame).range(1..=self.timeline_length));
                                    ui.label("->");
                                    ui.add(egui::DragValue::new(&mut track.end_frame).range(track.start_frame..=self.timeline_length.max(track.start_frame)));
                                });
                            }
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.strong(tr(lang, "Stage / Document", "إعدادات المسرح"));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("W:");
                            ui.add(egui::DragValue::new(&mut self.project.width).range(320..=3840));
                            ui.label("H:");
                            ui.add(egui::DragValue::new(&mut self.project.height).range(240..=2160));
                        });
                        ui.horizontal(|ui| {
                            if ui.small_button("1080p").clicked() { self.project.width = 1920; self.project.height = 1080; }
                            if ui.small_button("720p").clicked() { self.project.width = 1280; self.project.height = 720; }
                            if ui.small_button("Shorts").clicked() { self.project.width = 1080; self.project.height = 1920; }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Stage Color:", "لون الخلفية:"));
                            ui.color_edit_button_srgba_unmultiplied(&mut self.stage_bg_color);
                        });
                        ui.horizontal(|ui| {
                            ui.label("FPS:");
                            ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=60));
                        });
                    } else {
                        ui.strong(tr(lang, "Symbols Library", "مكتبة الرموز"));
                        let sym_names: Vec<String> = self.project.library_symbols.keys().cloned().collect();
                        if sym_names.is_empty() {
                            ui.small(tr(lang, "Draw & press F8 to create symbol", "ارسم واضغط F8 لإنشاء رمز"));
                        }
                        for s_name in sym_names {
                            ui.horizontal(|ui| {
                                ui.label(&s_name);
                                if ui.small_button(tr(lang, "+ Place", "+ إدراج")).clicked() {
                                    self.save_undo_state();
                                    self.ensure_editable_keyframe();
                                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                        let frame = layer.keyframes.entry(self.current_frame).or_default();
                                        frame.symbol_instances.push(SymbolInstance {
                                            symbol_name: s_name.clone(),
                                            x: 100.0,
                                            y: 100.0,
                                            scale: 1.0,
                                        });
                                        self.has_unsaved_changes = true;
                                        self.play_sfx(SfxType::NewFrame);
                                    }
                                }
                            });
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.strong(tr(lang, "Audio Tracks", "ملفات الصوت"));
                        if ui.button(tr(lang, "+ Import Audio", "+ إضافة صوت")).clicked() {
                            self.import_audio_dialog();
                        }
                        let mut remove_audio_idx = None;
                        for (idx, track) in self.project.audio_tracks.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(self.selected_audio_idx == Some(idx), &track.name).clicked() {
                                    self.selected_audio_idx = Some(idx);
                                }
                                if ui.small_button("X").clicked() {
                                    remove_audio_idx = Some(idx);
                                }
                            });
                        }
                        if let Some(idx) = remove_audio_idx {
                            self.save_undo_state();
                            self.project.audio_tracks.remove(idx);
                            self.selected_audio_idx = None;
                        }
                    }
                }
            });

        // التايم لاين السفلي مع ترس إعدادات الألوان المخصص
        if self.workspace_mode == WorkspaceMode::AnimationStudio {
            let tl_bg = self.config.timeline_bg_color;
            let tl_cell_col = self.config.timeline_cell_color;
            let tl_head_col = self.config.timeline_playhead_color;

            egui::TopBottomPanel::bottom("timeline_panel")
                .resizable(true)
                .default_height(225.0)
                .height_range(110.0..=520.0)
                .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(tl_bg[0], tl_bg[1], tl_bg[2], 255)).inner_margin(6.0))
                .show(ctx, |ui| {
                    let tl_rect = ui.available_rect_before_wrap();
                    let is_over_tl = pointer_pos.map_or(false, |p| tl_rect.contains(p));
                    if ctrl_down && scroll_y != 0.0 && is_over_tl {
                        self.timeline_cell_width = (self.timeline_cell_width + scroll_y * 0.05).clamp(16.0, 64.0);
                    }

                    ui.horizontal(|ui| {
                        if ui.button("|<").clicked() { self.current_frame = 1; }
                        if ui.button("<").clicked() && self.current_frame > 1 { self.current_frame -= 1; }
                        if ui.button(if self.is_playing { tr(lang, "Pause", "إيقاف") } else { tr(lang, "Play", "تشغيل") }).clicked() {
                            self.is_playing = !self.is_playing;
                            if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                        }
                        if ui.button(">").clicked() && self.current_frame < self.timeline_length {
                            self.current_frame += 1;
                        }
                        if ui.button(">|").clicked() { self.current_frame = self.timeline_length; }

                        ui.separator();
                        ui.colored_label(
                            egui::Color32::from_rgba_unmultiplied(tl_head_col[0], tl_head_col[1], tl_head_col[2], 255),
                            format!("{}f", self.current_frame),
                        );
                        ui.label(format!("/ {}f ({:.1}s)", self.timeline_length, self.current_frame as f32 / self.project.fps.max(1) as f32));

                        ui.separator();
                        if ui.button(format!("{} ({})", tr(lang, "+ Frame", "+ تمديد"), self.config.insert_frame)).clicked() {
                            self.action_insert_frame_f5();
                        }
                        if ui.button(format!("{} ({})", tr(lang, "+ Keyframe", "+ فريم منسوخ"), self.config.insert_keyframe)).clicked() {
                            self.action_insert_keyframe_f6(now_time);
                        }
                        if ui.button(format!("{} ({})", tr(lang, "+ Blank KF", "+ فريم فارغ"), self.config.insert_blank_kf)).clicked() {
                            self.action_insert_blank_keyframe_f7(now_time);
                        }

                        ui.separator();
                        ui.checkbox(&mut self.project.onion_skin.enabled, tr(lang, "Onion Skin", "شفافية الحركة"));
                        if self.project.onion_skin.enabled {
                            ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_before).range(0..=10));
                            ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_before);
                            ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_after).range(0..=10));
                            ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_after);
                        }

                        ui.separator();
                        if ui.button(tr(lang, "+ Layer", "+ طبقة")).clicked() {
                            self.save_undo_state();
                            let count = self.project.layers.len() + 1;
                            let mut kf = HashMap::new();
                            kf.insert(1, Default::default());
                            self.project.layers.push(Layer {
                                name: format!("Layer_{}", count),
                                visible: true,
                                locked: false,
                                keyframes: kf,
                            });
                            self.play_sfx(SfxType::NewFrame);
                        }

                        // ترس إعدادات التايم لاين المخصص
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (tl_gear_rect, tl_gear_resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
                            if tl_gear_resp.hovered() {
                                ui.painter().rect_filled(tl_gear_rect, 4.0, egui::Color32::from_gray(65));
                            }
                            draw_gear_icon(ui.painter(), tl_gear_rect, 0.85);
                            if tl_gear_resp.on_hover_text(tr(lang, "Timeline Settings", "إعدادات التايم لاين")).clicked() {
                                self.play_sfx(SfxType::UiClick);
                                self.show_timeline_settings = true;
                            }
                        });
                    });

                    ui.separator();

                    let mut pending_copy: Option<(usize, usize)> = None;
                    let mut pending_paste: Option<(usize, usize)> = None;
                    let mut pending_clear_frame: Option<(usize, usize)> = None;
                    let mut pending_remove_frame: Option<(usize, usize)> = None;
                    let mut pending_dup_layer: Option<usize> = None;
                    let mut delete_layer_idx: Option<usize> = None;
                    let mut swap_layers: Option<(usize, usize)> = None;
                    let mut move_frame_op: Option<((usize, usize), (usize, usize))> = None;
                    let mut remove_audio_idx: Option<usize> = None;

                    let cell_w = self.timeline_cell_width;

                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(150.0, 20.0), |ui| {
                                ui.strong(tr(lang, "Layers", "الطبقات"));
                            });
                            for f in 1..=self.timeline_length {
                                let is_curr = self.current_frame == f;
                                let btn = egui::Button::new(format!("{}", f))
                                    .min_size(egui::vec2(cell_w, 18.0))
                                    .fill(if is_curr {
                                        egui::Color32::from_rgba_unmultiplied(tl_head_col[0], tl_head_col[1], tl_head_col[2], 255)
                                    } else {
                                        egui::Color32::from_rgba_unmultiplied(tl_cell_col[0], tl_cell_col[1], tl_cell_col[2], 255)
                                    });
                                if ui.add(btn).clicked() {
                                    self.current_frame = f;
                                }
                            }
                            if ui.add(egui::Button::new("+").min_size(egui::vec2(24.0, 18.0))).clicked() {
                                self.action_insert_frame_f5();
                            }
                        });

                        let layers_len = self.project.layers.len();
                        let has_clip = self.clipboard_frame.is_some();
                        let lbl_rename = tr(lang, "Rename Layer", "إعادة تسمية الطبقة");
                        let lbl_copy = tr(lang, "Copy Frame (Ctrl+C)", "نسخ الفريم (Copy)");
                        let lbl_paste = tr(lang, "Paste Frame (Ctrl+V)", "لصق الفريم (Paste)");
                        let lbl_clear_frame = tr(lang, "Clear Frame (Make White/Blank)", "مسح المحتوى (Clear - فريم أبيض)");
                        let lbl_remove_frame = format!(
                            "{} ({})",
                            tr(lang, "Delete Frame (Remove & Shift)", "حذف الفريم تماماً وإرجاع الترقيم"),
                            self.config.delete_frame_key
                        );
                        let lbl_dup_layer = tr(lang, "Duplicate Layer", "تكرار الطبقة");
                        let lbl_del_layer = tr(lang, "Delete Layer", "حذف الطبقة");

                        for (l_idx, layer) in self.project.layers.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.allocate_ui(egui::vec2(150.0, 24.0), |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut layer.visible, "");
                                        let is_active_l = self.active_layer_idx == l_idx;
                                        let layer_btn = egui::Button::new(&layer.name)
                                            .selected(is_active_l)
                                            .sense(egui::Sense::click_and_drag());
                                        let layer_resp = ui.add(layer_btn);
                                        if layer_resp.clicked() {
                                            self.active_layer_idx = l_idx;
                                        }
                                        if layer_resp.drag_started() {
                                            self.dragged_layer_idx = Some(l_idx);
                                        }
                                        if pointer_released && layer_resp.hovered() {
                                            if let Some(src_l) = self.dragged_layer_idx.take() {
                                                if src_l != l_idx {
                                                    swap_layers = Some((src_l, l_idx));
                                                }
                                            }
                                        }
                                        layer_resp.context_menu(|ui| {
                                            if ui.button(&lbl_rename).clicked() {
                                                self.renaming_layer_idx = Some(l_idx);
                                                self.rename_layer_buf = layer.name.clone();
                                                ui.close_menu();
                                            }
                                            if ui.button(&lbl_dup_layer).clicked() {
                                                pending_dup_layer = Some(l_idx);
                                                ui.close_menu();
                                            }
                                            if layers_len > 1 && ui.button(&lbl_del_layer).clicked() {
                                                delete_layer_idx = Some(l_idx);
                                                ui.close_menu();
                                            }
                                        });
                                        if layers_len > 1 && ui.small_button("X").clicked() {
                                            delete_layer_idx = Some(l_idx);
                                        }
                                    });
                                });

                                for f in 1..=self.timeline_length {
                                    let is_keyframe = layer.keyframes.contains_key(&f);
                                    let eff_fd = Self::get_effective_frame_data(layer, f);
                                    let has_drawings = eff_fd.map_or(false, |fd| {
                                        !fd.strokes.is_empty()
                                            || !fd.media_items.is_empty()
                                            || !fd.symbol_instances.is_empty()
                                            || !fd.text_items.is_empty()
                                    });

                                    let is_selected = self.active_layer_idx == l_idx && self.current_frame == f;
                                    let shape_kind = if is_keyframe {
                                        if has_drawings { 1 } else { 2 }
                                    } else if has_drawings {
                                        3
                                    } else {
                                        0
                                    };

                                    let fill = if is_selected {
                                        egui::Color32::from_rgb(0, 122, 204)
                                    } else if has_drawings {
                                        egui::Color32::from_rgb(65, 65, 72)
                                    } else {
                                        egui::Color32::from_rgba_unmultiplied(tl_cell_col[0], tl_cell_col[1], tl_cell_col[2], 255)
                                    };

                                    let (cell_rect, cell_resp) = ui.allocate_exact_size(
                                        egui::vec2(cell_w, 22.0),
                                        egui::Sense::click_and_drag(),
                                    );
                                    ui.painter().rect_filled(cell_rect, 3.0, fill);
                                    draw_timeline_frame_shape(ui.painter(), cell_rect, shape_kind);

                                    if cell_resp.clicked() {
                                        self.active_layer_idx = l_idx;
                                        self.current_frame = f;
                                    }
                                    if cell_resp.drag_started() {
                                        self.dragged_frame = Some((l_idx, f));
                                    }
                                    if pointer_released && cell_resp.hovered() {
                                        if let Some(src) = self.dragged_frame.take() {
                                            if src != (l_idx, f) {
                                                move_frame_op = Some((src, (l_idx, f)));
                                            }
                                        }
                                    }

                                    cell_resp.context_menu(|ui| {
                                        if ui.button(&lbl_copy).clicked() {
                                            pending_copy = Some((l_idx, f));
                                            ui.close_menu();
                                        }
                                        if ui.add_enabled(has_clip, egui::Button::new(&lbl_paste)).clicked() {
                                            pending_paste = Some((l_idx, f));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button(&lbl_clear_frame).clicked() {
                                            pending_clear_frame = Some((l_idx, f));
                                            ui.close_menu();
                                        }
                                        if ui.button(&lbl_remove_frame).clicked() {
                                            pending_remove_frame = Some((l_idx, f));
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                        }

                        for (a_idx, track) in self.project.audio_tracks.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.allocate_ui(egui::vec2(150.0, 28.0), |ui| {
                                    let is_sel = self.selected_audio_idx == Some(a_idx);
                                    let lbl_resp = ui.selectable_label(is_sel, format!("🎵 {}", track.name));
                                    if lbl_resp.clicked() {
                                        self.selected_audio_idx = Some(a_idx);
                                    }
                                    lbl_resp.context_menu(|ui| {
                                        if ui.button(tr(lang, "Trim / Cut Range...", "تقطيع نطاق الصوت...")).clicked() {
                                            self.trimming_audio_idx = Some(a_idx);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr(lang, "Delete Audio Track", "حذف تراك الصوت")).clicked() {
                                            remove_audio_idx = Some(a_idx);
                                            ui.close_menu();
                                        }
                                    });
                                });

                                for f in 1..=self.timeline_length {
                                    let active = f >= track.start_frame && f <= track.end_frame;
                                    let (w_rect, w_resp) = ui.allocate_exact_size(
                                        egui::vec2(cell_w, 28.0),
                                        egui::Sense::click(),
                                    );
                                    if active {
                                        ui.painter().rect_filled(w_rect, 2.0, egui::Color32::from_rgb(130, 30, 85));
                                        let peak_idx = (f - track.start_frame) % track.waveform_peaks.len().max(1);
                                        let amp = track.waveform_peaks.get(peak_idx).copied().unwrap_or(0.5);
                                        let bar_h = (w_rect.height() * 0.85 * amp).max(3.0);
                                        let bar_rect = egui::Rect::from_center_size(
                                            w_rect.center(),
                                            egui::vec2((cell_w * 0.65).max(3.0), bar_h),
                                        );
                                        ui.painter().rect_filled(bar_rect, 1.0, egui::Color32::from_rgb(255, 110, 195));
                                    } else {
                                        ui.painter().rect_filled(w_rect, 2.0, egui::Color32::from_rgb(30, 30, 33));
                                    }

                                    if w_resp.clicked() {
                                        self.current_frame = f;
                                        self.selected_audio_idx = Some(a_idx);
                                    }
                                    w_resp.context_menu(|ui| {
                                        if ui.button(tr(lang, "Trim / Cut Audio...", "تقطيع الصوت من هنا...")).clicked() {
                                            self.trimming_audio_idx = Some(a_idx);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr(lang, "Delete Audio", "حذف الصوت")).clicked() {
                                            remove_audio_idx = Some(a_idx);
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                        }
                    });

                    if pointer_released {
                        self.dragged_layer_idx = None;
                        self.dragged_frame = None;
                    }

                    if let Some((src_l, dst_l)) = swap_layers {
                        self.save_undo_state();
                        let moved = self.project.layers.remove(src_l);
                        self.project.layers.insert(dst_l, moved);
                        self.active_layer_idx = dst_l;
                        self.has_unsaved_changes = true;
                    }
                    if let Some(((sl, sf), (dl, df))) = move_frame_op {
                        self.save_undo_state();
                        if let Some(data) = self.project.layers.get_mut(sl).and_then(|l| l.keyframes.remove(&sf)) {
                            if let Some(dst_layer) = self.project.layers.get_mut(dl) {
                                dst_layer.keyframes.insert(df, data);
                                self.active_layer_idx = dl;
                                self.current_frame = df;
                                self.has_unsaved_changes = true;
                            }
                        }
                    }
                    if let Some((l, f)) = pending_copy { self.copy_frame(l, f); }
                    if let Some((l, f)) = pending_paste { self.paste_frame(l, f); }
                    if let Some((l, f)) = pending_clear_frame { self.clear_frame_content(l, f); }
                    if let Some((l, f)) = pending_remove_frame { self.remove_frame_completely(l, f); }
                    if let Some(idx) = remove_audio_idx {
                        self.save_undo_state();
                        self.project.audio_tracks.remove(idx);
                        self.selected_audio_idx = None;
                    }
                    if let Some(l_idx) = pending_dup_layer {
                        self.save_undo_state();
                        if let Some(layer) = self.project.layers.get(l_idx).cloned() {
                            let mut dup = layer;
                            dup.name = format!("{}_copy", dup.name);
                            self.project.layers.insert(l_idx + 1, dup);
                            self.active_layer_idx = l_idx + 1;
                        }
                    }
                    if let Some(idx) = delete_layer_idx {
                        self.save_undo_state();
                        self.project.layers.remove(idx);
                        if self.active_layer_idx >= self.project.layers.len() {
                            self.active_layer_idx = self.project.layers.len().saturating_sub(1);
                        }
                    }
                });
        }

        self.preload_frame_textures(ctx);

        // مساحة العمل والكانفا
        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(canvas_rect);

            let center = canvas_rect.center() + self.pan_offset;
            let scale = self.zoom_level.max(0.1);
            let stage_w = self.project.width as f32;
            let stage_h = self.project.height as f32;
            let stage_size = egui::vec2(stage_w, stage_h) * scale;
            let stage_rect = egui::Rect::from_center_size(center, stage_size);

            // التحقق الدقيق: هل الماوس داخل مربع الرسم الأبيض (stage_rect) ولا توجد أي نوافذ مفتوحة؟
            let is_inside_white_stage = pointer_pos.map_or(false, |p| stage_rect.contains(p) && canvas_rect.contains(p));
            let show_white_dot_cursor = is_inside_white_stage
                && !any_modal_open
                && !space_down
                && !alt_down
                && self.active_tool_ui != ActiveToolUI::HandCanvas;

            if show_white_dot_cursor {
                ctx.set_cursor_icon(egui::CursorIcon::None);
            } else if space_down || self.active_tool_ui == ActiveToolUI::HandCanvas {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            } else {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }

            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));
            if scroll_y != 0.0 && is_over_canvas && !any_modal_open {
                self.zoom_level = (self.zoom_level + scroll_y * 0.001).clamp(0.1, 4.0);
            }

            if alt_down && pointer_down && is_over_canvas && !any_modal_open {
                if let (Some(curr), Some(last)) = (pointer_pos, self.last_pointer_pos) {
                    let dx = curr.x - last.x;
                    self.canvas_rotation_deg = (self.canvas_rotation_deg + dx * 0.45) % 360.0;
                }
            } else if (space_down || self.active_tool_ui == ActiveToolUI::HandCanvas) && pointer_down && !any_modal_open {
                if let (Some(curr), Some(last)) = (pointer_pos, self.last_pointer_pos) {
                    self.pan_offset += curr - last;
                }
            }

            painter.rect_filled(
                stage_rect.translate(egui::vec2(6.0, 6.0)),
                0.0,
                egui::Color32::from_black_alpha(110),
            );
            let bg = self.stage_bg_color;
            painter.rect_filled(
                stage_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]),
            );

            let rot_rad = self.canvas_rotation_deg.to_radians();
            let (sin_r, cos_r) = rot_rad.sin_cos();
            let flip_h = self.canvas_flip_h;
            let flip_v = self.canvas_flip_v;

            let to_screen = |p: egui::Pos2| -> egui::Pos2 {
                let mut nx = p.x - stage_w * 0.5;
                let mut ny = p.y - stage_h * 0.5;
                if flip_h { nx = -nx; }
                if flip_v { ny = -ny; }
                let rx = nx * cos_r - ny * sin_r;
                let ry = nx * sin_r + ny * cos_r;
                egui::pos2(center.x + rx * scale, center.y + ry * scale)
            };

            let to_stage = |sp: egui::Pos2| -> egui::Pos2 {
                let rx = (sp.x - center.x) / scale;
                let ry = (sp.y - center.y) / scale;
                let mut nx = rx * cos_r + ry * sin_r;
                let mut ny = -rx * sin_r + ry * cos_r;
                if flip_h { nx = -nx; }
                if flip_v { ny = -ny; }
                egui::pos2(nx + stage_w * 0.5, ny + stage_h * 0.5)
            };

            if self.config.show_grid {
                let step = self.config.grid_size.max(8.0);
                let grid_col = egui::Color32::from_rgba_unmultiplied(120, 130, 150, 75);
                let mut gx = 0.0;
                while gx <= stage_w {
                    painter.line_segment(
                        [to_screen(egui::pos2(gx, 0.0)), to_screen(egui::pos2(gx, stage_h))],
                        egui::Stroke::new(1.0, grid_col),
                    );
                    gx += step;
                }
                let mut gy = 0.0;
                while gy <= stage_h {
                    painter.line_segment(
                        [to_screen(egui::pos2(0.0, gy)), to_screen(egui::pos2(stage_w, gy))],
                        egui::Stroke::new(1.0, grid_col),
                    );
                    gy += step;
                }
            }

            // رسم خط مرآة التماثل في وضع الرسم
            if self.workspace_mode == WorkspaceMode::DrawingOnly {
                match self.symmetry_mode {
                    SymmetryMode::Horizontal => {
                        painter.line_segment(
                            [to_screen(egui::pos2(stage_w * 0.5, 0.0)), to_screen(egui::pos2(stage_w * 0.5, stage_h))],
                            egui::Stroke::new(1.2, egui::Color32::from_rgba_unmultiplied(249, 115, 22, 140)),
                        );
                    }
                    SymmetryMode::Vertical => {
                        painter.line_segment(
                            [to_screen(egui::pos2(0.0, stage_h * 0.5)), to_screen(egui::pos2(stage_w, stage_h * 0.5))],
                            egui::Stroke::new(1.2, egui::Color32::from_rgba_unmultiplied(249, 115, 22, 140)),
                        );
                    }
                    SymmetryMode::Off => {}
                }
            }

            if !space_down && !alt_down && self.active_tool_ui != ActiveToolUI::HandCanvas && is_over_canvas && !any_modal_open {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |last| (pos - last) / scale);

                    match self.active_tool_ui {
                        ActiveToolUI::Text if pointer_pressed => {
                            self.pending_text_pos = stage_pos;
                            self.show_text_dialog = true;
                        }
                        ActiveToolUI::ShapeLine => {
                            if pointer_pressed {
                                self.shape_drag_start = Some(stage_pos);
                                self.shape_drag_current = Some(stage_pos);
                            } else if pointer_down {
                                self.shape_drag_current = Some(stage_pos);
                            }
                        }
                        ActiveToolUI::VectorDirectSelect => {
                            // أداة تحديد نقاط الفيكتور: تظهر النقاط وتسمح بسحبها وتعديلها
                            if pointer_pressed {
                                self.editing_vector_node = None;
                                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                    if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                        'vec_search: for (s_idx, s) in frame.strokes.iter().enumerate() {
                                            if s.is_vector {
                                                for (p_idx, p) in s.points.iter().enumerate() {
                                                    if ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 15.0 {
                                                        self.editing_vector_node = Some((s_idx, p_idx));
                                                        break 'vec_search;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if self.editing_vector_node.is_some() {
                                    self.save_undo_state();
                                }
                            } else if pointer_down {
                                if let Some((s_idx, p_idx)) = self.editing_vector_node {
                                    self.ensure_editable_keyframe();
                                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                            if let Some(s) = frame.strokes.get_mut(s_idx) {
                                                if let Some(pt) = s.points.get_mut(p_idx) {
                                                    pt.x = stage_pos.x;
                                                    pt.y = stage_pos.y;
                                                    self.has_unsaved_changes = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso => {
                            if pointer_pressed {
                                let mut hit_resize_handle = false;
                                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                    if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                        let mut max_p = egui::pos2(f32::MIN, f32::MIN);
                                        let mut has_sel = false;
                                        for &idx in &self.selected_stroke_indices {
                                            if let Some(s) = frame.strokes.get(idx) {
                                                for p in &s.points {
                                                    max_p = max_p.max(to_screen(egui::pos2(p.x, p.y)));
                                                    has_sel = true;
                                                }
                                            }
                                        }
                                        if let Some(t_idx) = self.selected_text_idx {
                                            if let Some(txt) = frame.text_items.get(t_idx) {
                                                max_p = max_p.max(to_screen(egui::pos2(txt.x + 110.0, txt.y + txt.font_size)));
                                                has_sel = true;
                                            }
                                        }
                                        if has_sel && pos.distance(max_p + egui::vec2(6.0, 6.0)) < 22.0 {
                                            hit_resize_handle = true;
                                        }
                                    }
                                }

                                if hit_resize_handle {
                                    self.save_undo_state();
                                    self.is_resizing_selection = true;
                                } else {
                                    self.is_resizing_selection = false;
                                    if self.active_tool_ui == ActiveToolUI::SelectLasso {
                                        let mut clicked_selected = false;
                                        if !self.selected_stroke_indices.is_empty() {
                                            if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                                if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                                    for &idx in &self.selected_stroke_indices {
                                                        if let Some(s) = frame.strokes.get(idx) {
                                                            if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 28.0) {
                                                                clicked_selected = true;
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if !clicked_selected {
                                            self.selected_stroke_indices.clear();
                                            self.selected_text_idx = None;
                                            self.lasso_polygon.clear();
                                            self.lasso_polygon.push(stage_pos);
                                            self.is_drawing_lasso = true;
                                        }
                                    } else {
                                        self.selected_stroke_indices.clear();
                                        self.selected_symbol_inst_idx = None;
                                        self.selected_text_idx = None;
                                        if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                            if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                                for (i, txt) in frame.text_items.iter().enumerate() {
                                                    let txt_rect = egui::Rect::from_min_size(
                                                        egui::pos2(txt.x - 10.0, txt.y - 10.0),
                                                        egui::vec2((txt.content.len() as f32 * txt.font_size * 0.55).max(80.0), txt.font_size + 20.0),
                                                    );
                                                    if txt_rect.contains(stage_pos) {
                                                        self.selected_text_idx = Some(i);
                                                        break;
                                                    }
                                                }
                                                if self.selected_text_idx.is_none() {
                                                    for (i, inst) in frame.symbol_instances.iter().enumerate() {
                                                        if ((inst.x - stage_pos.x).powi(2) + (inst.y - stage_pos.y).powi(2)).sqrt() < 60.0 {
                                                            self.selected_symbol_inst_idx = Some(i);
                                                            break;
                                                        }
                                                    }
                                                }
                                                if self.selected_text_idx.is_none() && self.selected_symbol_inst_idx.is_none() {
                                                    for (i, s) in frame.strokes.iter().enumerate() {
                                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 20.0) {
                                                            self.selected_stroke_indices.push(i);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if pointer_down && delta.length_sq() > 0.0 {
                                if self.is_resizing_selection || ctx.input(|i| i.modifiers.shift) {
                                    let factor = (1.0 + (delta.x + delta.y) * 0.006).clamp(0.85, 1.15);
                                    self.scale_selected_items(factor);
                                } else if self.is_drawing_lasso {
                                    self.lasso_polygon.push(stage_pos);
                                } else {
                                    self.ensure_editable_keyframe();
                                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                            if let Some(t_idx) = self.selected_text_idx {
                                                if let Some(txt) = frame.text_items.get_mut(t_idx) {
                                                    txt.x += delta.x;
                                                    txt.y += delta.y;
                                                    self.has_unsaved_changes = true;
                                                }
                                            }
                                            if let Some(idx) = self.selected_symbol_inst_idx {
                                                if let Some(inst) = frame.symbol_instances.get_mut(idx) {
                                                    inst.x += delta.x;
                                                    inst.y += delta.y;
                                                    self.has_unsaved_changes = true;
                                                }
                                            }
                                            for &idx in &self.selected_stroke_indices {
                                                if let Some(stroke) = frame.strokes.get_mut(idx) {
                                                    for p in &mut stroke.points {
                                                        p.x += delta.x;
                                                        p.y += delta.y;
                                                    }
                                                    self.has_unsaved_changes = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::Bucket if pointer_pressed => {
                            self.save_undo_state();
                            self.ensure_editable_keyframe();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    for s in frame.strokes.iter_mut().rev() {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 35.0) {
                                            s.fill_color = Some(self.brush_color);
                                            self.has_unsaved_changes = true;
                                            self.play_sfx(SfxType::NewFrame);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen if pointer_down => {
                            if pointer_pressed {
                                self.save_undo_state();
                            }
                            if delta.length() > 0.5 && now - self.last_sfx_time > 0.09 {
                                self.play_sfx(SfxType::EraserRub);
                                self.last_sfx_time = now;
                            }
                            self.ensure_editable_keyframe();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    let r = self.brush_size;
                                    match self.active_tool_ui {
                                        ActiveToolUI::EraserRed => {
                                            frame.strokes.retain(|s| !s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < r));
                                        }
                                        _ => {
                                            erase_partial_from_strokes(&mut frame.strokes, stage_pos, r);
                                        }
                                    }
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        ActiveToolUI::Eyedropper if pointer_pressed => {
                            if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                    for s in &frame.strokes {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 15.0) {
                                            self.brush_color = s.color;
                                            self.play_sfx(SfxType::UiClick);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::VectorPen if pointer_down => {
                            // قلم الفيكتور يرسم بنعومة بدون إظهار النقاط أثناء الرسم
                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    let dist = ((last.x - stage_pos.x).powi(2) + (last.y - stage_pos.y).powi(2)).sqrt();
                                    if dist >= 4.0 {
                                        stroke.points.push(StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure: 1.0 });
                                        if now - self.last_sfx_time > 0.08 {
                                            self.play_sfx(SfxType::VectorPenTick);
                                            self.last_sfx_time = now;
                                        }
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke {
                                    points: vec![StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure: 1.0 }],
                                    color: self.brush_color,
                                    fill_color: None,
                                    base_width: self.brush_size,
                                    is_vector: true,
                                    brush_texture_id: None,
                                });
                            }
                        }
                        ActiveToolUI::Brush if pointer_down => {
                            let speed = delta.length();
                            if speed > 0.8 && now - self.last_sfx_time > 0.08 {
                                self.play_sfx(SfxType::BrushScratch);
                                self.last_sfx_time = now;
                            }
                            let raw_p = if let Some(f) = touch_force {
                                f
                            } else {
                                (1.15 - (speed * 0.025 * self.pressure_sensitivity)).clamp(0.25, 1.35)
                            };
                            let pressure = if self.pen_pressure_enabled { raw_p } else { 1.0 };

                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    let stab = self.stabilizer_strength.clamp(0.0, 0.85);
                                    let smooth_x = last.x * stab + stage_pos.x * (1.0 - stab);
                                    let smooth_y = last.y * stab + stage_pos.y * (1.0 - stab);
                                    let dist = ((last.x - smooth_x).powi(2) + (last.y - smooth_y).powi(2)).sqrt();
                                    if dist >= 1.2 {
                                        stroke.points.push(StrokePoint { x: smooth_x, y: smooth_y, pressure });
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke {
                                    points: vec![StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure }],
                                    color: self.brush_color,
                                    fill_color: None,
                                    base_width: self.brush_size,
                                    is_vector: false,
                                    brush_texture_id: None,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if pointer_released {
                self.is_resizing_selection = false;
                self.editing_vector_node = None;

                if let (Some(p0), Some(p1)) = (self.shape_drag_start.take(), self.shape_drag_current.take()) {
                    if p0.distance(p1) > 3.0 {
                        self.save_undo_state();
                        let shape_stroke = build_geometric_stroke(
                            self.selected_shape_kind,
                            p0,
                            p1,
                            self.brush_color,
                            self.brush_size,
                        );
                        self.commit_stroke_with_symmetry(shape_stroke, now_time);
                        self.play_sfx(SfxType::NewFrame);
                    }
                }

                if self.is_drawing_lasso {
                    self.is_drawing_lasso = false;
                    if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                        if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                            for (i, s) in frame.strokes.iter().enumerate() {
                                if s.points.iter().any(|p| point_in_polygon(egui::pos2(p.x, p.y), &self.lasso_polygon)) {
                                    self.selected_stroke_indices.push(i);
                                }
                            }
                        }
                    }
                    if !self.selected_stroke_indices.is_empty() {
                        self.play_sfx(SfxType::LassoSelect);
                    }
                    self.lasso_polygon.clear();
                }

                if let Some(mut stroke) = self.current_stroke.take() {
                    if stroke.is_vector {
                        // تبسيط نقاط الفيكتور لنقاط تحكم ذكية ثم تنعيمها
                        stroke.simplify_vector_nodes(3.5);
                        stroke.smooth_brush();
                        stroke.simplify_vector_nodes(2.2);
                    } else {
                        stroke.smooth_brush();
                    }
                    self.commit_stroke_with_symmetry(stroke, now_time);
                }
            }

            self.last_pointer_pos = if pointer_down { pointer_pos } else { None };

            if self.project.onion_skin.enabled && !self.is_playing && self.workspace_mode == WorkspaceMode::AnimationStudio {
                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                    for offset in 1..=self.project.onion_skin.frames_before {
                        if self.current_frame > offset {
                            if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame - offset) {
                                for s in &fd.strokes {
                                    draw_stroke_pro(&painter, s, Some(self.project.onion_skin.color_before), scale, 0.0, 0.0, &to_screen);
                                }
                            }
                        }
                    }
                    for offset in 1..=self.project.onion_skin.frames_after {
                        if self.current_frame + offset <= self.timeline_length {
                            if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame + offset) {
                                for s in &fd.strokes {
                                    draw_stroke_pro(&painter, s, Some(self.project.onion_skin.color_after), scale, 0.0, 0.0, &to_screen);
                                }
                            }
                        }
                    }
                }
            }

            for (l_idx, layer) in self.project.layers.iter().enumerate().rev() {
                if layer.visible {
                    if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame) {
                        for m in &fd.media_items {
                            if let Some(tex) = self.texture_cache.get(&m.path) {
                                painter.image(
                                    tex.id(),
                                    egui::Rect::from_min_max(to_screen(egui::pos2(m.x, m.y)), to_screen(egui::pos2(m.x + m.width, m.y + m.height))),
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                        }
                        for s in &fd.strokes {
                            draw_stroke_pro(&painter, s, None, scale, 0.0, 0.0, &to_screen);
                            // إظهار نقاط الفيكتور الذكية فقط عند اختيار أداة Vector Direct Select (Q)
                            if l_idx == self.active_layer_idx && self.active_tool_ui == ActiveToolUI::VectorDirectSelect && s.is_vector {
                                for p in &s.points {
                                    let sp = to_screen(egui::pos2(p.x, p.y));
                                    painter.circle_filled(sp, 4.2, egui::Color32::from_rgb(192, 132, 252));
                                    painter.circle_stroke(sp, 5.0, egui::Stroke::new(1.2, egui::Color32::BLACK));
                                }
                            }
                        }

                        if l_idx == self.active_layer_idx && (!self.selected_stroke_indices.is_empty() || self.selected_text_idx.is_some()) {
                            let mut min_p = egui::pos2(f32::MAX, f32::MAX);
                            let mut max_p = egui::pos2(f32::MIN, f32::MIN);
                            let mut has_box = false;

                            for &s_idx in &self.selected_stroke_indices {
                                if let Some(s) = fd.strokes.get(s_idx) {
                                    for p in &s.points {
                                        let sp = to_screen(egui::pos2(p.x, p.y));
                                        min_p = min_p.min(sp);
                                        max_p = max_p.max(sp);
                                        has_box = true;
                                    }
                                }
                            }
                            if let Some(t_idx) = self.selected_text_idx {
                                if let Some(txt) = fd.text_items.get(t_idx) {
                                    let sp0 = to_screen(egui::pos2(txt.x, txt.y));
                                    let sp1 = to_screen(egui::pos2(txt.x + 110.0, txt.y + txt.font_size));
                                    min_p = min_p.min(sp0);
                                    max_p = max_p.max(sp1);
                                    has_box = true;
                                }
                            }
                            if has_box {
                                let sel_rect = egui::Rect::from_min_max(min_p - egui::vec2(6.0, 6.0), max_p + egui::vec2(6.0, 6.0));
                                painter.rect_stroke(
                                    sel_rect,
                                    2.0,
                                    egui::Stroke::new(1.5, egui::Color32::from_rgb(56, 189, 248)),
                                );
                                let handle_rect = egui::Rect::from_center_size(sel_rect.right_bottom(), egui::vec2(13.0, 13.0));
                                painter.rect_filled(handle_rect, 2.0, egui::Color32::from_rgb(56, 189, 248));
                            }
                        }

                        for txt in &fd.text_items {
                            let shaped = fix_ar(&txt.content);
                            let col = egui::Color32::from_rgba_unmultiplied(txt.color[0], txt.color[1], txt.color[2], txt.color[3]);
                            painter.text(
                                to_screen(egui::pos2(txt.x, txt.y)),
                                egui::Align2::LEFT_TOP,
                                shaped,
                                egui::FontId::proportional(txt.font_size * scale),
                                col,
                            );
                        }
                    }
                    let interp_symbols = Self::get_interpolated_symbols(layer, self.current_frame);
                    for inst in &interp_symbols {
                        if let Some(sym) = self.project.library_symbols.get(&inst.symbol_name) {
                            for s in &sym.strokes {
                                draw_stroke_pro(&painter, s, None, scale * inst.scale, inst.x, inst.y, &to_screen);
                            }
                        }
                    }
                }
            }

            if let Some(stroke) = &self.current_stroke {
                draw_stroke_pro(&painter, stroke, None, scale, 0.0, 0.0, &to_screen);
            }

            if let (Some(p0), Some(p1)) = (self.shape_drag_start, self.shape_drag_current) {
                let preview = build_geometric_stroke(
                    self.selected_shape_kind,
                    p0,
                    p1,
                    self.brush_color,
                    self.brush_size,
                );
                draw_stroke_pro(&painter, &preview, None, scale, 0.0, 0.0, &to_screen);
            }

            if self.is_drawing_lasso && self.lasso_polygon.len() > 1 {
                let screen_pts: Vec<egui::Pos2> = self.lasso_polygon.iter().map(|&p| to_screen(p)).collect();
                painter.add(egui::Shape::closed_line(
                    screen_pts,
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(56, 189, 248)),
                ));
            }

            // رسم النقطة البيضاء بإطار أسود فقط داخل مربع الرسم الأبيض وعند عدم وجود أي نوافذ مفتوحة
            if show_white_dot_cursor {
                if let Some(p) = pointer_pos {
                    painter.circle_filled(p, 3.5, egui::Color32::WHITE);
                    painter.circle_stroke(p, 4.3, egui::Stroke::new(1.2, egui::Color32::BLACK));
                }
            }
        });
    }
}

fn draw_stroke_pro<F: Fn(egui::Pos2) -> egui::Pos2>(
    painter: &egui::Painter,
    stroke: &Stroke,
    override_color: Option<[u8; 4]>,
    scale: f32,
    ox: f32,
    oy: f32,
    to_screen: &F,
) {
    if stroke.points.is_empty() {
        return;
    }

    let c = override_color.unwrap_or(stroke.color);
    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);

    if stroke.points.len() == 1 {
        let p = to_screen(egui::pos2(stroke.points[0].x + ox, stroke.points[0].y + oy));
        let r = (stroke.base_width * stroke.points[0].pressure * scale) * 0.5;
        painter.circle_filled(p, r.max(1.0), color);
        return;
    }

    if let Some(fc) = stroke.fill_color {
        let fill_c = override_color.unwrap_or(fc);
        let pts: Vec<egui::Pos2> = stroke
            .points
            .iter()
            .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            pts,
            egui::Color32::from_rgba_unmultiplied(fill_c[0], fill_c[1], fill_c[2], fill_c[3]),
            egui::Stroke::NONE,
        ));
    }

    let total = stroke.points.len();
    for (idx, w) in stroke.points.windows(2).enumerate() {
        let p0 = to_screen(egui::pos2(w[0].x + ox, w[0].y + oy));
        let p1 = to_screen(egui::pos2(w[1].x + ox, w[1].y + oy));

        let taper = if stroke.is_vector {
            1.0
        } else {
            let progress = idx as f32 / total as f32;
            if progress < 0.12 {
                (progress / 0.12).max(0.25)
            } else if progress > 0.88 {
                ((1.0 - progress) / 0.12).max(0.25)
            } else {
                1.0
            }
        };

        let avg_pressure = ((w[0].pressure + w[1].pressure) * 0.5) * taper;
        let width = (stroke.base_width * avg_pressure * scale).max(1.0);

        painter.circle_filled(p0, width * 0.5, color);
        painter.line_segment([p0, p1], egui::Stroke::new(width, color));
        painter.circle_filled(p1, width * 0.5, color);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Stick")
            .with_inner_size([1366.0, 768.0])
            .with_icon(generate_stick_window_icon()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "Stick",
        options,
        Box::new(|cc| Ok(Box::new(StickApp::new(cc)))),
    )
}
