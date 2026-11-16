#![windows_subsystem = "windows"]

mod exporter;
mod profile;
mod project;

use arabic_reshaper::arabic_reshape;
use eframe::egui;
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
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use unicode_bidi::BidiInfo;

const FALLBACK_VERSION: &str = "1.5.0";
const GITHUB_REPO: &str = "KaniEGY/AniHady2D";
const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@%D8%A7%D8%A8%D9%84%D9%83%D8%A7%D8%B4-%D8%A7%D9%86%D9%85%D9%8A%D8%B4%D9%86/videos";

#[derive(Clone, Copy, PartialEq)]
enum WorkspaceMode {
    Launcher,
    DrawingOnly,
    AnimationStudio,
}

#[derive(Clone, Copy, PartialEq)]
enum ActiveToolUI {
    SelectNormal,
    SelectLasso,
    VectorPen,
    Brush,
    EraserRed,
    EraserGreen,
    Bucket,
    Eyedropper,
    Text,
    HandCanvas,
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
    sound_effects_enabled: bool,
    tool_select: String,
    #[serde(default = "default_lasso_key")]
    tool_lasso: String,
    tool_pen: String,
    tool_brush: String,
    tool_eraser: String,
    #[serde(default = "default_partial_eraser_key")]
    tool_eraser_partial: String,
    tool_bucket: String,
    tool_eyedropper: String,
    #[serde(default = "default_text_key")]
    tool_text: String,
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

fn default_lasso_key() -> String { "L".to_string() }
fn default_partial_eraser_key() -> String { "T".to_string() }
fn default_text_key() -> String { "N".to_string() }
fn default_hand_key() -> String { "H".to_string() }
fn default_del_frame_key() -> String { "X".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lang: AppLang::Arabic,
            sound_effects_enabled: true,
            tool_select: "V".to_string(),
            tool_lasso: "L".to_string(),
            tool_pen: "P".to_string(),
            tool_brush: "B".to_string(),
            tool_eraser: "E".to_string(),
            tool_eraser_partial: "T".to_string(),
            tool_bucket: "K".to_string(),
            tool_eyedropper: "I".to_string(),
            tool_text: "N".to_string(),
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
            let path = exe.with_file_name("anihady_config.json");
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
            let path = exe.with_file_name("anihady_config.json");
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

fn generate_window_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let nx = (x as f32 / size as f32) * 2.0 - 1.0;
            let ny = (y as f32 / size as f32) * 2.0 - 1.0;
            let dist = (nx.abs().powf(4.0) + ny.abs().powf(4.0)).powf(0.25);
            let idx = ((y * size + x) * 4) as usize;
            if dist <= 0.94 {
                let t = (x + y) as f32 / (2.0 * size as f32);
                rgba[idx] = (25.0 + t * 115.0) as u8;
                rgba[idx + 1] = (50.0 + (1.0 - t) * 140.0) as u8;
                rgba[idx + 2] = (215.0 + t * 40.0) as u8;
                rgba[idx + 3] = 255;
            }
        }
    }
    for y in 16..48 {
        for x in 16..22 {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
        for x in 42..48 {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    for y in 29..35 {
        for x in 16..48 {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx..idx + 4].copy_from_slice(&[110, 235, 255, 255]);
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

fn apply_animate_pro_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_rgb(34, 34, 37);
    style.visuals.window_fill = egui::Color32::from_rgb(40, 40, 44);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(20, 20, 22);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 122, 204);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 48, 52);
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
        ActiveToolUI::VectorPen => egui::Color32::from_rgb(139, 92, 246),
        ActiveToolUI::Brush => egui::Color32::from_rgb(249, 115, 22),
        ActiveToolUI::EraserRed => egui::Color32::from_rgb(239, 68, 68),
        ActiveToolUI::EraserGreen => egui::Color32::from_rgb(34, 197, 94),
        ActiveToolUI::Bucket => egui::Color32::from_rgb(16, 185, 129),
        ActiveToolUI::Eyedropper => egui::Color32::from_rgb(234, 179, 8),
        ActiveToolUI::Text => egui::Color32::from_rgb(236, 72, 153),
        ActiveToolUI::HandCanvas => egui::Color32::from_rgb(168, 85, 247), // أداة اليد باللون البنفسجي
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
        ActiveToolUI::HandCanvas => {
            // أيقونة اليد البنفسجية
            let palm = egui::Rect::from_center_size(c + egui::vec2(0.0, 2.5), egui::vec2(10.0, 9.0));
            painter.rect_filled(palm, 3.0, main_color);
            for dx in [-4.0, -1.3, 1.3, 4.0] {
                painter.line_segment(
                    [c + egui::vec2(dx, -1.0), c + egui::vec2(dx, -7.5)],
                    egui::Stroke::new(2.0, main_color),
                );
            }
            painter.line_segment([c + egui::vec2(-5.0, 2.0), c + egui::vec2(-8.0, -1.0)], egui::Stroke::new(2.0, main_color));
        }
    }
}

fn draw_gear_icon(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    let color = egui::Color32::from_gray(220);
    painter.circle_stroke(c, 5.5, egui::Stroke::new(1.8, color));
    painter.circle_filled(c, 2.0, color);
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::FRAC_PI_4;
        let p1 = c + egui::vec2(angle.cos() * 5.5, angle.sin() * 5.5);
        let p2 = c + egui::vec2(angle.cos() * 8.2, angle.sin() * 8.2);
        painter.line_segment([p1, p2], egui::Stroke::new(2.0, color));
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

struct AniHadyApp {
    workspace_mode: WorkspaceMode,
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
    lasso_polygon: Vec<egui::Pos2>,
    is_drawing_lasso: bool,
    selected_stroke_indices: Vec<usize>,
    selected_symbol_inst_idx: Option<usize>,
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
    has_unsaved_changes: bool,
    show_exit_dialog: bool,
    show_symbol_dialog: bool,
    show_text_dialog: bool,
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

impl AniHadyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        apply_animate_pro_theme(&cc.egui_ctx);
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
        Self {
            workspace_mode: WorkspaceMode::Launcher,
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
            lasso_polygon: Vec::new(),
            is_drawing_lasso: false,
            selected_stroke_indices: Vec::new(),
            selected_symbol_inst_idx: None,
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
            has_unsaved_changes: false,
            show_exit_dialog: false,
            show_symbol_dialog: false,
            show_text_dialog: false,
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
            config: AppConfig::load(),
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
            status_message: "Ready".to_string(),
            audio_sink: None,
            last_pointer_pos: None,
            right_tab: 0,
        }
    }

    fn play_sfx(&self, sfx: SfxType) {
        play_sfx_async(self.config.sound_effects_enabled, sfx, false);
    }

    fn record_frame_drawn_activity(&mut self) {
        self.user_profile.total_frames_drawn += 1;
        self.frames_drawn_counter += 1;
        if self.frames_drawn_counter >= 20 {
            self.frames_drawn_counter = 0;
            self.user_profile.add_xp_silent(35);
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

    fn action_insert_frame_f5(&mut self) {
        self.save_undo_state();
        self.timeline_length += 1;
        self.project.total_frames = self.timeline_length;
        self.current_frame = self.timeline_length;
        self.has_unsaved_changes = true;
        self.play_sfx(SfxType::NewFrame);
        self.status_message = format!("{} -> {}f", tr(self.config.lang, "Frame Extended", "تم تمديد الفريم"), self.timeline_length);
    }

    fn action_insert_keyframe_f6(&mut self) {
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
        self.record_frame_drawn_activity();
        self.status_message = format!("{} ({})", tr(self.config.lang, "Keyframe Copied", "تم نسخ الفريم"), next_f);
    }

    fn action_insert_blank_keyframe_f7(&mut self) {
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
        self.record_frame_drawn_activity();
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

    /// Clear: تفريغ محتوى الفريم وجعله أبيض/فارغ مع بقاء مكانه
    fn clear_frame_content(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            layer.keyframes.insert(frame_idx, FrameData::default());
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::EraserRub);
            self.status_message = format!("{} ({})", tr(self.config.lang, "Frame Cleared", "تم تفريغ الفريم"), frame_idx);
        }
    }

    /// Delete (X): إزالة الفريم نهائياً من التايم لاين وتزحيف الفريمات اللي بعده وإرجاع الترقيم الصحيح
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

        // تقليص طول التايم لاين وتحديث الترقيم إذا كان الفريم المحذوف في نهاية التايم لاين أو يقلص الطول الكلي
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

    fn scale_selected_strokes(&mut self, factor: f32) {
        if self.selected_stroke_indices.is_empty() {
            return;
        }
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                let mut min_p = egui::pos2(f32::MAX, f32::MAX);
                let mut max_p = egui::pos2(f32::MIN, f32::MIN);
                for &idx in &self.selected_stroke_indices {
                    if let Some(s) = frame.strokes.get(idx) {
                        for p in &s.points {
                            min_p = min_p.min(egui::pos2(p.x, p.y));
                            max_p = max_p.max(egui::pos2(p.x, p.y));
                        }
                    }
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
            .add_filter("AniHady2D Project", &["ah2d"])
            .set_file_name("project.ah2d")
            .save_file()
        {
            if path.extension().is_none() {
                path.set_extension("ah2d");
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
            .add_filter("Videos", &["mp4", "mov", "avi", "mkv"])
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
            match ureq::get(&api_url).set("User-Agent", "AniHady2D-Updater").call() {
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
            match ureq::get(&download_url).set("User-Agent", "AniHady2D-Updater").call() {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    if response.into_reader().read_to_end(&mut bytes).is_ok() {
                        if let Ok(current_exe) = std::env::current_exe() {
                            let new_exe = current_exe.with_file_name("AniHady2D_new.exe");
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
            let new_exe = current_exe.with_file_name("AniHady2D_new.exe");
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
}

impl eframe::App for AniHadyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = self.config.lang;
        let now_time = ctx.input(|i| i.time);

        if self.last_xp_tick_time == 0.0 {
            self.last_xp_tick_time = now_time;
        } else if now_time - self.last_xp_tick_time >= 60.0 {
            let mins = ((now_time - self.last_xp_tick_time) / 60.0).floor() as u64;
            self.last_xp_tick_time += (mins as f64) * 60.0;
            self.user_profile.total_seconds_spent += mins * 60;
            self.user_profile.add_xp_silent(mins);
        }

        if self.current_frame != self.last_scrubbed_frame {
            self.last_scrubbed_frame = self.current_frame;
            self.trigger_audio_stream_scrub();
        }

        if self.workspace_mode == WorkspaceMode::Launcher {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.heading(
                        egui::RichText::new("AniHady2D Studio")
                            .size(32.0)
                            .color(egui::Color32::from_rgb(80, 180, 255)),
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
            sc_pen,
            sc_brush,
            sc_eraser,
            sc_eraser_partial,
            sc_bucket,
            sc_eyedropper,
            sc_text,
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
                is_shortcut_pressed(i, &self.config.tool_pen),
                is_shortcut_pressed(i, &self.config.tool_brush),
                is_shortcut_pressed(i, &self.config.tool_eraser),
                is_shortcut_pressed(i, &self.config.tool_eraser_partial),
                is_shortcut_pressed(i, &self.config.tool_bucket),
                is_shortcut_pressed(i, &self.config.tool_eyedropper),
                is_shortcut_pressed(i, &self.config.tool_text),
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
                2 => self.config.tool_pen = k_name,
                3 => self.config.tool_brush = k_name,
                4 => self.config.tool_eraser = k_name,
                5 => self.config.tool_eraser_partial = k_name,
                6 => self.config.tool_bucket = k_name,
                7 => self.config.tool_eyedropper = k_name,
                8 => self.config.tool_text = k_name,
                9 => self.config.tool_hand = k_name,
                10 => self.config.delete_frame_key = k_name,
                11 => self.config.insert_frame = k_name,
                12 => self.config.insert_keyframe = k_name,
                13 => self.config.insert_blank_kf = k_name,
                14 => self.config.convert_symbol = k_name,
                15 => self.config.play_pause = k_name,
                16 => self.config.prev_frame = k_name,
                17 => self.config.next_frame = k_name,
                18 => self.config.brush_smaller = k_name,
                19 => self.config.brush_larger = k_name,
                20 => self.config.reset_canvas = k_name,
                _ => {}
            }
            self.config.save();
            self.listening_shortcut_idx = None;
            typing_in_textbox = true;
        }

        if self.show_symbol_dialog
            || self.show_text_dialog
            || self.show_exit_dialog
            || self.show_update_dialog
            || self.show_settings_dialog
            || self.show_credits_dialog
            || self.renaming_layer_idx.is_some()
            || self.trimming_audio_idx.is_some()
        {
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
                if sc_f6 { self.action_insert_keyframe_f6(); }
                if sc_f7 { self.action_insert_blank_keyframe_f7(); }
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
            if sc_hand {
                self.active_tool_ui = ActiveToolUI::HandCanvas;
                self.play_sfx(SfxType::UiClick);
            }
            if sc_smaller { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if sc_larger { self.brush_size = (self.brush_size + 2.0).min(100.0); }

            if del_pressed {
                if let Some(idx) = self.selected_symbol_inst_idx.take() {
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

        if space_down || self.active_tool_ui == ActiveToolUI::HandCanvas {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
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
                            ui.label(tr(lang, "Preview:", "معاينة:"));
                            ui.strong(fix_ar(&self.text_input_buf));
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
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Save", "حفظ")).clicked() {
                            if let Some(layer) = self.project.layers.get_mut(l_idx) {
                                layer.name = self.rename_layer_buf.clone();
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
            egui::Window::new(tr(lang, "About AniHady2D", "حول برنامج AniHady2D"))
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.heading(egui::RichText::new("Made by Hady").color(egui::Color32::from_rgb(255, 80, 100)));
                        ui.add_space(4.0);
                        ui.label(format!("AniHady2D Animation Studio v{}", self.installed_version));
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
                .default_width(480.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.settings_tab == 0, tr(lang, "Shortcuts & General", "الاختصارات واللغة")).clicked() {
                            self.settings_tab = 0;
                        }
                        if ui.selectable_label(self.settings_tab == 1, tr(lang, "System & Level", "النظام والمستوى")).clicked() {
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
                            "Fun Sound Effects (All Tools, Frames, Zoom, Export, Exit)",
                            "المؤثرات الصوتية الترفيهية (لكل أداة، الفريمات، الزوم، التصدير، الإغلاق)",
                        );
                        if ui.checkbox(&mut self.config.sound_effects_enabled, sfx_label).changed() {
                            self.config.save();
                            sfx_changed = true;
                        }
                        ui.separator();
                        ui.strong(tr(lang, "Navigation & Zoom Guide:", "شرح اختصارات الزوم وتحريك الكانفا:"));
                        ui.label(tr(lang, "• Ctrl + Up Arrow or W : Zoom In (+5%)", "• Ctrl + سهم فوق أو حرف W : تكبير الكانفا بنسبة 5%"));
                        ui.label(tr(lang, "• Ctrl + Down Arrow or S : Zoom Out (-5%)", "• Ctrl + سهم تحت أو حرف S : تصغير الكانفا بنسبة 5%"));
                        ui.label(tr(lang, "• Ctrl + Right / Left or D / A : Pan Right / Left (5%)", "• Ctrl + سهم يمين / شمال أو D / A : تحريك الكانفا يمين وشمال 5%"));
                        ui.separator();

                        let items = [
                            (0, tr(lang, "Select Tool (Normal)", "أداة التحديد العادي"), self.config.tool_select.clone()),
                            (1, tr(lang, "Area Select Tool (Lasso)", "أداة تحديد المنطقة الحرة"), self.config.tool_lasso.clone()),
                            (2, tr(lang, "Vector Pen Tool", "أداة قلم الفيكتور"), self.config.tool_pen.clone()),
                            (3, tr(lang, "Brush Tool", "أداة الفرشاة"), self.config.tool_brush.clone()),
                            (4, tr(lang, "Eraser Red (Full Stroke)", "الممحاة الحمراء (مسح الخط كامل)"), self.config.tool_eraser.clone()),
                            (5, tr(lang, "Eraser Green (Partial)", "الممحاة الخضراء (مسح جزء صغير)"), self.config.tool_eraser_partial.clone()),
                            (6, tr(lang, "Paint Bucket Tool", "أداة دلو التعبئة"), self.config.tool_bucket.clone()),
                            (7, tr(lang, "Eyedropper Tool", "أداة قطارة الألوان"), self.config.tool_eyedropper.clone()),
                            (8, tr(lang, "Text Tool", "أداة الكتابة (النص)"), self.config.tool_text.clone()),
                            (9, tr(lang, "Hand Tool (Rotate/Flip)", "أداة اليد (تدوير وعكس الكانفا)"), self.config.tool_hand.clone()),
                            (10, tr(lang, "Delete Frame Completely", "حذف الفريم نهائياً"), self.config.delete_frame_key.clone()),
                            (11, tr(lang, "Insert Frame (Extend)", "تمديد الفريم الحالي"), self.config.insert_frame.clone()),
                            (12, tr(lang, "Insert Keyframe (Duplicate)", "إدراج فريم منسوخ"), self.config.insert_keyframe.clone()),
                            (13, tr(lang, "Insert Blank Keyframe", "إدراج فريم فارغ جديد"), self.config.insert_blank_kf.clone()),
                            (14, tr(lang, "Convert to Symbol", "تحويل إلى رمز (Symbol)"), self.config.convert_symbol.clone()),
                            (15, tr(lang, "Play / Pause", "تشغيل / إيقاف"), self.config.play_pause.clone()),
                            (16, tr(lang, "Previous Frame", "الفريم السابق"), self.config.prev_frame.clone()),
                            (17, tr(lang, "Next Frame", "الفريم التالي"), self.config.next_frame.clone()),
                            (18, tr(lang, "Decrease Brush Size", "تصغير حجم الفرشاة"), self.config.brush_smaller.clone()),
                            (19, tr(lang, "Increase Brush Size", "تكبير حجم الفرشاة"), self.config.brush_larger.clone()),
                            (20, tr(lang, "Reset Canvas View", "إرجاع الكانفا للمنتصف"), self.config.reset_canvas.clone()),
                        ];
                        let press_key_txt = tr(lang, "Press any key...", "اضغط أي زر...");
                        egui::ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                            for (idx, label, current_key) in items {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui(egui::vec2(245.0, 22.0), |ui| {
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
                        ui.horizontal(|ui| {
                            let (av_rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                            if let Some(ref tex) = self.avatar_texture {
                                ui.painter().image(
                                    tex.id(),
                                    av_rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().rect_filled(av_rect, 10.0, egui::Color32::from_rgb(0, 122, 204));
                                ui.painter().text(
                                    av_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "ARTIST",
                                    egui::FontId::proportional(14.0),
                                    egui::Color32::WHITE,
                                );
                            }

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(tr(lang, "Character Name:", "اسم الشخصية:"));
                                    if ui.text_edit_singleline(&mut self.user_profile.username).changed() {
                                        self.user_profile.save();
                                    }
                                });
                                if !self.user_profile.username.is_empty() {
                                    ui.small(fix_ar(&self.user_profile.username));
                                }
                                if ui.button(tr(lang, "Import Avatar Image...", "استيراد صورة شخصية...")).clicked() {
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
                        ui.heading(format!("{} {}", tr(lang, "Level", "المستوى"), level));
                        let ratio = (cur_xp as f32 / req_xp.max(1) as f32).clamp(0.0, 1.0);
                        ui.add(
                            egui::ProgressBar::new(ratio)
                                .text(format!("{} / {} XP", cur_xp, req_xp))
                                .fill(egui::Color32::from_rgb(34, 197, 94)),
                        );

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.strong(tr(lang, "Total Time Spent:", "إجمالي وقت الاستخدام:"));
                            let pt = self.user_profile.format_playtime(lang == AppLang::Arabic);
                            ui.label(if lang == AppLang::Arabic { fix_ar(&pt) } else { pt });
                        });
                        ui.horizontal(|ui| {
                            ui.strong(tr(lang, "Total Frames Created:", "إجمالي الفريمات المرسومة:"));
                            ui.label(format!("{}", self.user_profile.total_frames_drawn));
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
            egui::Window::new(tr(lang, "AniHady2D Updater", "تحديث البرنامج"))
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

                ui.colored_label(egui::Color32::from_rgb(80, 175, 255), "AniHady2D");
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
                    if ui.button(tr(lang, "Open Project (.ah2d)", "فتح مشروع")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("AniHady2D", &["ah2d"]).pick_file() {
                            if let Ok(proj) = AniProject::load_from_file(&path) {
                                self.timeline_length = proj.total_frames.max(1);
                                self.current_frame = 1;
                                self.active_layer_idx = 0;
                                self.project = proj;
                                self.has_unsaved_changes = false;
                                self.play_sfx(SfxType::NewFrame);
                                self.status_message = tr(lang, "Project Loaded!", "تم فتح المشروع بنجاح!");
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr(lang, "Import Image...", "استيراد صورة...")).clicked() { self.import_image_dialog(); ui.close_menu(); }
                    if self.workspace_mode == WorkspaceMode::AnimationStudio {
                        if ui.button(tr(lang, "Import Video...", "استيراد فيديو...")).clicked() { self.import_video_dialog(); ui.close_menu(); }
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
                        if ui.button(tr(lang, "Export Video (MP4)...", "تصدير فيديو (MP4)...")).clicked() {
                            self.project.total_frames = self.timeline_length;
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("MP4 Video", &["mp4"])
                                .set_file_name("AniHady2D_Export.mp4")
                                .save_file()
                            {
                                match exporter::export_to_mp4(&self.project, &path) {
                                    Ok(_) => {
                                        self.play_sfx(SfxType::ExportSuccess);
                                        self.status_message = tr(lang, "Exported MP4!", "تم تصدير الفيديو!");
                                    }
                                    Err(e) => self.status_message = e,
                                }
                            }
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
                    if ui.button(tr(lang, "+ Audio", "+ صوت")).clicked() { self.import_audio_dialog(); }
                }
                ui.separator();
                if ui.button(tr(lang, "Updates", "تحديثات")).clicked() { self.check_for_updates(ctx.clone()); }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (gear_rect, gear_resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                    if gear_resp.hovered() {
                        ui.painter().rect_filled(gear_rect, 4.0, egui::Color32::from_gray(65));
                    }
                    draw_gear_icon(ui.painter(), gear_rect);
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
                    ui.colored_label(egui::Color32::LIGHT_GRAY, &self.status_message);
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
                    let select_ui = if self.active_tool_ui == ActiveToolUI::SelectLasso {
                        ActiveToolUI::SelectLasso
                    } else {
                        ActiveToolUI::SelectNormal
                    };
                    let select_label = if select_ui == ActiveToolUI::SelectLasso {
                        tr(lang, "Area Select", "تحديد منطقة")
                    } else {
                        tr(lang, "Select", "تحديد عادي")
                    };
                    let select_sc = if select_ui == ActiveToolUI::SelectLasso {
                        self.config.tool_lasso.clone()
                    } else {
                        self.config.tool_select.clone()
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
                        (ActiveToolUI::HandCanvas, tr(lang, "Hand / Flip", "اليد / عكس وتدوير"), self.config.tool_hand.clone()),
                    ];
                    let show_labels = ui.available_width() > 95.0;
                    for (tool, name, sc) in tools {
                        let selected = self.active_tool_ui == tool;
                        let accent = get_tool_ui_color(tool);
                        let btn_w = if show_labels { ui.available_width() - 8.0 } else { 42.0 };
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(btn_w, 36.0), egui::Sense::click());
                        let bg = if selected {
                            accent
                        } else if resp.hovered() {
                            egui::Color32::from_rgb(62, 62, 68)
                        } else {
                            egui::Color32::from_rgb(44, 44, 48)
                        };
                        ui.painter().rect_filled(rect, 6.0, bg);
                        let icon_rect = egui::Rect::from_min_size(rect.min, egui::vec2(42.0, 36.0));
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
                                ActiveToolUI::VectorPen => self.play_sfx(SfxType::VectorPenTick),
                                ActiveToolUI::Brush => self.play_sfx(SfxType::BrushScratch),
                                ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => self.play_sfx(SfxType::EraserRub),
                                ActiveToolUI::SelectLasso => self.play_sfx(SfxType::LassoSelect),
                                _ => self.play_sfx(SfxType::UiClick),
                            }
                        }
                        if matches!(tool, ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso) {
                            let lbl_norm = tr(lang, "Normal Select (V)", "تحديد عادي (V)");
                            let lbl_area = tr(lang, "Area / Lasso Select (L)", "تحديد منطقة حرة (L)");
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

        // اللوحة اليمنى
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(245.0)
            .width_range(170.0..=480.0)
            .show(ctx, |ui| {
                if self.workspace_mode == WorkspaceMode::DrawingOnly {
                    ui.strong(tr(lang, "Brush & Zoom", "الفرشاة والزوم"));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Size:", "الحجم:"));
                        ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                    });
                    ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
                    ui.checkbox(&mut self.pen_pressure_enabled, tr(lang, "Pen Pressure", "حساسية الضغط"));

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Zoom:", "الزوم:"));
                        ui.add(egui::Slider::new(&mut self.zoom_level, 0.1..=4.0).show_value(true));
                    });

                    ui.add_space(10.0);
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
                    ui.add_space(4.0);
                    let mut del_l = None;
                    let l_len = self.project.layers.len();
                    egui::ScrollArea::vertical().show(ui, |ui| {
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
                } else {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.right_tab == 0, tr(lang, "Properties", "الخصائص")).clicked() { self.right_tab = 0; }
                        if ui.selectable_label(self.right_tab == 1, tr(lang, "Library & Audio", "المكتبة والصوت")).clicked() { self.right_tab = 1; }
                    });
                    ui.separator();

                    if self.right_tab == 0 {
                        ui.strong(tr(lang, "Tool Settings", "إعدادات الأداة"));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Size:", "الحجم:"));
                            ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                        });
                        ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));

                        ui.add_space(6.0);
                        ui.checkbox(&mut self.pen_pressure_enabled, tr(lang, "Pen Pressure Sensitivity", "حساسية ضغط القلم"));
                        if self.pen_pressure_enabled {
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, "Sensitivity:", "درجة الحساسية:"));
                                ui.add(egui::Slider::new(&mut self.pressure_sensitivity, 0.2..=2.0));
                            });
                        }

                        // إعدادات أداة اليد (عكس وتدوير الكانفا)
                        if self.active_tool_ui == ActiveToolUI::HandCanvas {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.strong(tr(lang, "Canvas Transform (Hand)", "تدوير وعكس الكانفا"));
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "Flip H", "عكس أفقي")).clicked() {
                                    self.canvas_flip_h = !self.canvas_flip_h;
                                }
                                if ui.button(tr(lang, "Flip V", "عكس رأسي")).clicked() {
                                    self.canvas_flip_v = !self.canvas_flip_v;
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, "Angle:", "الزاوية:"));
                                ui.add(egui::Slider::new(&mut self.canvas_rotation_deg, -180.0..=180.0));
                            });
                            if ui.small_button(tr(lang, "Reset Transform", "إعادة الوضع الأصلي")).clicked() {
                                self.canvas_rotation_deg = 0.0;
                                self.canvas_flip_h = false;
                                self.canvas_flip_v = false;
                            }
                        }

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

        // التايم لاين الاحترافي
        if self.workspace_mode == WorkspaceMode::AnimationStudio {
            egui::TopBottomPanel::bottom("timeline_panel")
                .resizable(true)
                .default_height(225.0)
                .height_range(110.0..=520.0)
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
                        ui.colored_label(egui::Color32::from_rgb(255, 95, 95), format!("{}f", self.current_frame));
                        ui.label(format!("/ {}f ({:.1}s)", self.timeline_length, self.current_frame as f32 / self.project.fps.max(1) as f32));

                        ui.separator();
                        if ui.button(format!("{} ({})", tr(lang, "+ Frame", "+ تمديد"), self.config.insert_frame)).clicked() {
                            self.action_insert_frame_f5();
                        }
                        if ui.button(format!("{} ({})", tr(lang, "+ Keyframe", "+ فريم منسوخ"), self.config.insert_keyframe)).clicked() {
                            self.action_insert_keyframe_f6();
                        }
                        if ui.button(format!("{} ({})", tr(lang, "+ Blank KF", "+ فريم فارغ"), self.config.insert_blank_kf)).clicked() {
                            self.action_insert_blank_keyframe_f7();
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
                                    .fill(if is_curr { egui::Color32::from_rgb(210, 50, 50) } else { egui::Color32::from_rgb(45, 45, 48) });
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
                                        egui::Color32::from_rgb(42, 42, 46)
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

                                    // كليك يمين على الفريم: Copy / Paste / Clear (تبيض الفريم) / Delete (حذف الفريم تماماً)
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

        // تحميل الصور مسبقاً قبل الرسم لتجنب أي تعارض في استعارة self
        self.preload_frame_textures(ctx);

        // مساحة العمل والكانفا
        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(canvas_rect);
            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));

            if scroll_y != 0.0 && is_over_canvas {
                self.zoom_level = (self.zoom_level + scroll_y * 0.001).clamp(0.1, 4.0);
            }

            // التحريك بأداة اليد البنفسجية أو بالضغط على المسطرة
            let is_hand_panning = (space_down || self.active_tool_ui == ActiveToolUI::HandCanvas) && pointer_down;
            if is_hand_panning {
                if let (Some(curr), Some(last)) = (pointer_pos, self.last_pointer_pos) {
                    self.pan_offset += curr - last;
                }
            }

            let center = canvas_rect.center() + self.pan_offset;
            let scale = self.zoom_level.max(0.1);
            let stage_w = self.project.width as f32;
            let stage_h = self.project.height as f32;
            let stage_size = egui::vec2(stage_w, stage_h) * scale;
            let stage_rect = egui::Rect::from_center_size(center, stage_size);

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

            // تحويل الإحداثيات مع دعم التدوير والعكس الأفقي والرأسي من أداة اليد
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

            let modal_open = self.show_update_dialog
                || self.show_settings_dialog
                || self.show_symbol_dialog
                || self.show_text_dialog
                || self.show_exit_dialog
                || self.show_credits_dialog
                || self.renaming_layer_idx.is_some()
                || self.trimming_audio_idx.is_some();

            if !space_down && self.active_tool_ui != ActiveToolUI::HandCanvas && is_over_canvas && !modal_open {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |last| (pos - last) / scale);

                    match self.active_tool_ui {
                        ActiveToolUI::Text if pointer_pressed => {
                            self.pending_text_pos = stage_pos;
                            self.show_text_dialog = true;
                        }
                        ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso => {
                            if !self.selected_stroke_indices.is_empty() && pointer_down && ctx.input(|i| i.modifiers.shift) {
                                let factor = (1.0 + delta.x * 0.005).clamp(0.85, 1.15);
                                self.scale_selected_strokes(factor);
                            } else if self.active_tool_ui == ActiveToolUI::SelectLasso {
                                if pointer_pressed {
                                    let mut clicked_selected = false;
                                    if !self.selected_stroke_indices.is_empty() {
                                        if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                            if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                                for &idx in &self.selected_stroke_indices {
                                                    if let Some(s) = frame.strokes.get(idx) {
                                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 25.0) {
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
                                        self.lasso_polygon.clear();
                                        self.lasso_polygon.push(stage_pos);
                                        self.is_drawing_lasso = true;
                                    }
                                } else if pointer_down {
                                    if self.is_drawing_lasso {
                                        self.lasso_polygon.push(stage_pos);
                                    } else if !self.selected_stroke_indices.is_empty() && delta.length_sq() > 0.0 {
                                        self.ensure_editable_keyframe();
                                        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
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
                            } else {
                                if pointer_pressed {
                                    self.selected_stroke_indices.clear();
                                    self.selected_symbol_inst_idx = None;
                                    if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                        if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                            for (i, inst) in frame.symbol_instances.iter().enumerate() {
                                                if ((inst.x - stage_pos.x).powi(2) + (inst.y - stage_pos.y).powi(2)).sqrt() < 60.0 {
                                                    self.selected_symbol_inst_idx = Some(i);
                                                    break;
                                                }
                                            }
                                            if self.selected_symbol_inst_idx.is_none() {
                                                for (i, s) in frame.strokes.iter().enumerate() {
                                                    if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 20.0) {
                                                        self.selected_stroke_indices.push(i);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if pointer_down && delta.length_sq() > 0.0 {
                                    self.ensure_editable_keyframe();
                                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                            if let Some(idx) = self.selected_symbol_inst_idx {
                                                if let Some(inst) = frame.symbol_instances.get_mut(idx) {
                                                    inst.x += delta.x;
                                                    inst.y += delta.y;
                                                    self.has_unsaved_changes = true;
                                                }
                                            } else {
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
                        ActiveToolUI::VectorPen | ActiveToolUI::Brush if pointer_down => {
                            let speed = delta.length();
                            if speed > 0.8 && now - self.last_sfx_time > 0.08 {
                                if self.active_tool_ui == ActiveToolUI::VectorPen {
                                    self.play_sfx(SfxType::VectorPenTick);
                                } else {
                                    self.play_sfx(SfxType::BrushScratch);
                                }
                                self.last_sfx_time = now;
                            }
                            let raw_p = if let Some(f) = touch_force {
                                f
                            } else {
                                (1.15 - (speed * 0.025 * self.pressure_sensitivity)).clamp(0.2, 1.4)
                            };
                            let pressure = if self.pen_pressure_enabled { raw_p } else { 1.0 };

                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    let dist = ((last.x - stage_pos.x).powi(2) + (last.y - stage_pos.y).powi(2)).sqrt();
                                    if dist >= 1.5 {
                                        stroke.points.push(StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure });
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke {
                                    points: vec![StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure }],
                                    color: self.brush_color,
                                    fill_color: None,
                                    base_width: self.brush_size,
                                    is_vector: self.active_tool_ui == ActiveToolUI::VectorPen,
                                    brush_texture_id: None,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if pointer_released {
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
                        stroke.smooth_vector();
                    }
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        layer.keyframes.entry(self.current_frame).or_default().strokes.push(stroke);
                        self.has_unsaved_changes = true;
                        self.record_frame_drawn_activity();
                    }
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
                        for (s_idx, s) in fd.strokes.iter().enumerate() {
                            draw_stroke_pro(&painter, s, None, scale, 0.0, 0.0, &to_screen);
                            if l_idx == self.active_layer_idx && self.selected_stroke_indices.contains(&s_idx) && !s.points.is_empty() {
                                let mut min_p = egui::pos2(f32::MAX, f32::MAX);
                                let mut max_p = egui::pos2(f32::MIN, f32::MIN);
                                for p in &s.points {
                                    let sp = to_screen(egui::pos2(p.x, p.y));
                                    min_p = min_p.min(sp);
                                    max_p = max_p.max(sp);
                                }
                                let sel_rect = egui::Rect::from_min_max(min_p - egui::vec2(5.0, 5.0), max_p + egui::vec2(5.0, 5.0));
                                painter.rect_stroke(
                                    sel_rect,
                                    2.0,
                                    egui::Stroke::new(1.5, egui::Color32::from_rgb(56, 189, 248)),
                                );
                                let handle_rect = egui::Rect::from_center_size(sel_rect.right_bottom(), egui::vec2(10.0, 10.0));
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

            if self.is_drawing_lasso && self.lasso_polygon.len() > 1 {
                let screen_pts: Vec<egui::Pos2> = self.lasso_polygon.iter().map(|&p| to_screen(p)).collect();
                painter.add(egui::Shape::closed_line(
                    screen_pts,
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(56, 189, 248)),
                ));
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
            if progress < 0.08 {
                (progress / 0.08).max(0.35)
            } else if progress > 0.92 {
                ((1.0 - progress) / 0.08).max(0.35)
            } else {
                1.0
            }
        };

        let avg_pressure = ((w[0].pressure + w[1].pressure) * 0.5) * taper;
        let width = (stroke.base_width * avg_pressure * scale).max(1.0);

        painter.line_segment([p0, p1], egui::Stroke::new(width, color));
        painter.circle_filled(p1, width * 0.5, color);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AniHady2D")
            .with_inner_size([1366.0, 768.0])
            .with_icon(generate_window_icon()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "AniHady2D",
        options,
        Box::new(|cc| Ok(Box::new(AniHadyApp::new(cc)))),
    )
}
