#![windows_subsystem = "windows"]

mod exporter;
mod project;

use eframe::egui;
use project::{AniProject, AudioTrack, FrameData, Layer, MediaItem, MediaType, Stroke, StrokePoint, Symbol, SymbolInstance, ToolType};
use rodio::source::{SineWave, TakeDuration};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FALLBACK_VERSION: &str = "1.1.35";
const GITHUB_REPO: &str = "KaniEGY/AniHady2D";
const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@%D8%A7%D8%A8%D9%84%D9%83%D8%A7%D8%B4-%D8%A7%D9%86%D9%85%D9%8A%D8%B4%D9%86/videos";

#[derive(Clone, Copy)]
enum SfxType {
    NewFrame,
    BrushScratch,
    EraserRub,
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
                    SfxType::NewFrame => {
                        sink.append(SineWave::new(620.0).take_duration(Duration::from_millis(55)).amplify(0.18));
                        sink.append(SineWave::new(880.0).take_duration(Duration::from_millis(55)).amplify(0.18));
                    }
                    SfxType::BrushScratch => {
                        sink.append(SineWave::new(320.0).take_duration(Duration::from_millis(30)).amplify(0.08));
                    }
                    SfxType::EraserRub => {
                        sink.append(SineWave::new(190.0).take_duration(Duration::from_millis(45)).amplify(0.12));
                    }
                    SfxType::ExportSuccess => {
                        sink.append(SineWave::new(523.25).take_duration(Duration::from_millis(120)).amplify(0.20));
                        sink.append(SineWave::new(659.25).take_duration(Duration::from_millis(120)).amplify(0.20));
                        sink.append(SineWave::new(783.99).take_duration(Duration::from_millis(200)).amplify(0.22));
                    }
                    SfxType::ExitFarewell => {
                        sink.append(SineWave::new(520.0).take_duration(Duration::from_millis(140)).amplify(0.18));
                        sink.append(SineWave::new(340.0).take_duration(Duration::from_millis(180)).amplify(0.18));
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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
enum AppLang {
    English,
    Arabic,
}

fn tr<'a>(lang: AppLang, en: &'a str, ar: &'a str) -> &'a str {
    match lang {
        AppLang::English => en,
        AppLang::Arabic => ar,
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
    tool_pen: String,
    tool_brush: String,
    tool_eraser: String,
    tool_bucket: String,
    tool_eyedropper: String,
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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lang: AppLang::English,
            sound_effects_enabled: true,
            tool_select: "V".to_string(),
            tool_pen: "P".to_string(),
            tool_brush: "B".to_string(),
            tool_eraser: "E".to_string(),
            tool_bucket: "K".to_string(),
            tool_eyedropper: "I".to_string(),
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

fn get_tool_accent_color(tool: ToolType) -> egui::Color32 {
    match tool {
        ToolType::Select => egui::Color32::from_rgb(56, 189, 248),
        ToolType::VectorPen => egui::Color32::from_rgb(168, 85, 247),
        ToolType::Brush => egui::Color32::from_rgb(249, 115, 22),
        ToolType::Eraser => egui::Color32::from_rgb(244, 63, 94),
        ToolType::Bucket => egui::Color32::from_rgb(34, 197, 94),
        ToolType::Eyedropper => egui::Color32::from_rgb(234, 179, 8),
    }
}

fn draw_vector_tool_icon(painter: &egui::Painter, rect: egui::Rect, tool: ToolType, active: bool) {
    let c = rect.center();
    let accent = get_tool_accent_color(tool);
    let main_color = if active { egui::Color32::WHITE } else { accent };
    let stroke = egui::Stroke::new(2.0, main_color);

    match tool {
        ToolType::Select => {
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
        ToolType::VectorPen => {
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
            painter.line_segment([c + egui::vec2(-4.5, 5.5), c + egui::vec2(4.5, 5.5)], stroke);
        }
        ToolType::Brush => {
            painter.line_segment(
                [c + egui::vec2(-1.5, 1.5), c + egui::vec2(7.5, -7.5)],
                egui::Stroke::new(3.2, main_color),
            );
            painter.line_segment(
                [c + egui::vec2(-3.0, 3.0), c + egui::vec2(-0.5, 0.5)],
                egui::Stroke::new(4.0, egui::Color32::from_gray(220)),
            );
            let tip = vec![
                c + egui::vec2(-2.0, 2.0),
                c + egui::vec2(-7.5, 3.5),
                c + egui::vec2(-8.5, 8.5),
                c + egui::vec2(-3.5, 7.5),
            ];
            painter.add(egui::Shape::convex_polygon(tip, main_color, egui::Stroke::NONE));
        }
        ToolType::Eraser => {
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
        ToolType::Bucket => {
            let pts = vec![
                c + egui::vec2(-6.5, 0.0),
                c + egui::vec2(0.0, -6.5),
                c + egui::vec2(6.5, 0.0),
                c + egui::vec2(0.0, 6.5),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.circle_filled(c + egui::vec2(7.5, 4.5), 2.6, main_color);
            painter.line_segment([c + egui::vec2(7.5, 1.5), c + egui::vec2(7.5, 4.5)], stroke);
        }
        ToolType::Eyedropper => {
            painter.line_segment([c + egui::vec2(-7.0, 7.0), c + egui::vec2(2.5, -2.5)], egui::Stroke::new(2.4, main_color));
            painter.circle_filled(c + egui::vec2(5.0, -5.0), 3.6, main_color);
            painter.line_segment([c + egui::vec2(1.0, -4.5), c + egui::vec2(4.5, -1.0)], egui::Stroke::new(2.2, egui::Color32::WHITE));
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

#[derive(Clone)]
struct HistorySnapshot {
    project: AniProject,
    timeline_length: usize,
    current_frame: usize,
    active_layer_idx: usize,
}

struct AniHadyApp {
    project: AniProject,
    timeline_length: usize,
    undo_stack: Vec<HistorySnapshot>,
    redo_stack: Vec<HistorySnapshot>,
    clipboard_frame: Option<FrameData>,
    current_frame: usize,
    active_layer_idx: usize,
    active_tool: ToolType,
    brush_color: [u8; 4],
    stage_bg_color: [u8; 4],
    brush_size: f32,
    pen_pressure_enabled: bool,
    pressure_sensitivity: f32,
    zoom_level: f32,
    pan_offset: egui::Vec2,
    is_playing: bool,
    last_frame_time: f64,
    last_sfx_time: f64,
    has_unsaved_changes: bool,
    show_exit_dialog: bool,
    show_symbol_dialog: bool,
    show_update_dialog: bool,
    show_settings_dialog: bool,
    show_credits_dialog: bool,
    listening_shortcut_idx: Option<usize>,
    config: AppConfig,
    installed_version: String,
    update_state: Arc<Mutex<UpdateState>>,
    new_symbol_name: String,
    allow_close: bool,
    current_stroke: Option<Stroke>,
    selected_stroke_idx: Option<usize>,
    selected_symbol_inst_idx: Option<usize>,
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
        Self {
            project: proj,
            timeline_length: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard_frame: None,
            current_frame: 1,
            active_layer_idx: 0,
            active_tool: ToolType::Brush,
            brush_color: [0, 0, 0, 255],
            stage_bg_color: [255, 255, 255, 255],
            brush_size: 6.0,
            pen_pressure_enabled: true,
            pressure_sensitivity: 1.0,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
            is_playing: false,
            last_frame_time: 0.0,
            last_sfx_time: 0.0,
            has_unsaved_changes: false,
            show_exit_dialog: false,
            show_symbol_dialog: false,
            show_update_dialog: false,
            show_settings_dialog: false,
            show_credits_dialog: false,
            listening_shortcut_idx: None,
            config: AppConfig::load(),
            installed_version: get_installed_version(),
            update_state: Arc::new(Mutex::new(UpdateState::Idle)),
            new_symbol_name: "Symbol_1".to_string(),
            allow_close: false,
            current_stroke: None,
            selected_stroke_idx: None,
            selected_symbol_inst_idx: None,
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
        self.selected_stroke_idx = None;
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
            self.status_message = tr(self.config.lang, "Undo", "تراجع").to_string();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.take_snapshot());
            self.restore_snapshot(next);
            self.status_message = tr(self.config.lang, "Redo", "إعادة").to_string();
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
        self.status_message = format!("{} ({})", tr(self.config.lang, "New Blank Keyframe", "فريم فارغ جديد"), next_f);
    }

    fn copy_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(layer) = self.project.layers.get(layer_idx) {
            let data = Self::get_effective_frame_data(layer, frame_idx).cloned().unwrap_or_default();
            self.clipboard_frame = Some(data);
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

    fn delete_or_clear_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            layer.keyframes.insert(frame_idx, FrameData::default());
            self.has_unsaved_changes = true;
            self.play_sfx(SfxType::EraserRub);
            self.status_message = format!("{} ({})", tr(self.config.lang, "Cleared Frame", "تم مسح الفريم"), frame_idx);
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

    fn trigger_save(&mut self) -> bool {
        self.project.total_frames = self.timeline_length;
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("AniHady2D Project", &["ah2d"])
            .set_file_name("project.ah2d")
            .save_file()
        {
            if self.project.save_to_file(&path).is_ok() {
                self.saved_path = Some(path);
                self.has_unsaved_changes = false;
                self.status_message = tr(self.config.lang, "Saved!", "تم الحفظ!").to_string();
                return true;
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
            self.ensure_editable_keyframe();
            if let Some(layer) = self.project.layers.get_mut(self.active_layer_
