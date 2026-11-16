#![windows_subsystem = "windows"]

mod exporter;
mod project;

use eframe::egui;
use project::{AniProject, AudioTrack, FrameData, Layer, MediaItem, MediaType, Stroke, StrokePoint, Symbol, SymbolInstance, ToolType};
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const FALLBACK_VERSION: &str = "1.1.25";
const GITHUB_REPO: &str = "KaniEGY/AniHady2D";
const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@%D8%A7%D8%A8%D9%84%D9%83%D8%A7%D8%B4-%D8%A7%D9%86%D9%85%D9%8A%D8%B4%D9%86/videos";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
enum AppLang {
    English,
    Arabic,
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

    fn tr<'a>(&self, en: &'a str, ar: &'a str) -> &'a str {
        match self.config.lang {
            AppLang::English => en,
            AppLang::Arabic => ar,
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
            self.status_message = self.tr("Undo", "تراجع").to_string();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.take_snapshot());
            self.restore_snapshot(next);
            self.status_message = self.tr("Redo", "إعادة").to_string();
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

    /// F5: تمديد مدة ظهور الفريم
    fn action_insert_frame_f5(&mut self) {
        self.save_undo_state();
        self.timeline_length += 1;
        self.project.total_frames = self.timeline_length;
        self.current_frame = self.timeline_length;
        self.has_unsaved_changes = true;
        self.status_message = format!("{} -> {}f", self.tr("Frame Extended", "تم تمديد الفريم"), self.timeline_length);
    }

    /// F6: إدراج Keyframe مع نسخ محتوى الفريم الحالي إلى فريم جديد بدون حذف القديم
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
        self.status_message = format!("{} ({})", self.tr("Keyframe Copied", "تم نسخ الفريم"), next_f);
    }

    /// F7: إنشاء Blank Keyframe جديد تماماً بعد الفريم الحالي بدون حذف الفريم الحالي أبداً!
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
        self.status_message = format!("{} ({})", self.tr("New Blank Keyframe", "فريم فارغ جديد"), next_f);
    }

    fn copy_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(layer) = self.project.layers.get(layer_idx) {
            let data = Self::get_effective_frame_data(layer, frame_idx).cloned().unwrap_or_default();
            self.clipboard_frame = Some(data);
            self.status_message = format!("{} ({})", self.tr("Copied Frame", "تم نسخ الفريم"), frame_idx);
        }
    }

    fn paste_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        if let Some(clip) = self.clipboard_frame.clone() {
            self.save_undo_state();
            if let Some(layer) = self.project.layers.get_mut(layer_idx) {
                layer.keyframes.insert(frame_idx, clip);
                self.has_unsaved_changes = true;
                self.status_message = format!("{} ({})", self.tr("Pasted to Frame", "تم اللصق في الفريم"), frame_idx);
            }
        }
    }

    fn delete_or_clear_frame(&mut self, layer_idx: usize, frame_idx: usize) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(layer_idx) {
            layer.keyframes.insert(frame_idx, FrameData::default());
            self.has_unsaved_changes = true;
            self.status_message = format!("{} ({})", self.tr("Cleared Frame", "تم مسح الفريم"), frame_idx);
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
                self.status_message = self.tr("Saved!", "تم الحفظ!").to_string();
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
            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                let frame = layer.keyframes.entry(self.current_frame).or_default();
                frame.media_items.push(MediaItem {
                    path: path.to_string_lossy().to_string(),
                    media_type: MediaType::Image,
                    x: 100.0,
                    y: 50.0,
                    width: 400.0,
                    height: 300.0,
                });
                self.has_unsaved_changes = true;
                self.status_message = self.tr("Image Imported", "تم استيراد الصورة").to_string();
            }
        }
    }

    fn import_video_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Videos", &["mp4", "mov", "avi", "mkv"])
            .pick_file()
        {
            self.status_message = self.tr("Extracting video frames...", "جاري استخراج فريمات الفيديو...").to_string();
            match exporter::extract_video_frames(&path, self.project.fps) {
                Ok(frames) => {
                    self.save_undo_state();
                    let needed_len = self.current_frame + frames.len().saturating_sub(1);
                    if needed_len > self.timeline_length {
                        self.timeline_length = needed_len;
                        self.project.total_frames = needed_len;
                    }
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        for (i, f_path) in frames.iter().enumerate() {
                            let f_idx = self.current_frame + i;
                            let frame = layer.keyframes.entry(f_idx).or_default();
                            frame.media_items.push(MediaItem {
                                path: f_path.to_string_lossy().to_string(),
                                media_type: MediaType::VideoFrame,
                                x: 0.0,
                                y: 0.0,
                                width: self.project.width as f32,
                                height: self.project.height as f32,
                            });
                        }
                        self.has_unsaved_changes = true;
                        self.status_message = format!("Imported {} frames", frames.len());
                    }
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
            self.save_undo_state();
            self.project.audio_tracks.push(AudioTrack {
                name: name.clone(),
                file_path: path.to_string_lossy().to_string(),
                start_frame: self.current_frame,
            });
            self.has_unsaved_changes = true;
            self.status_message = format!("Audio: {}", name);
        }
    }

    fn start_audio_playback(&mut self) {
        if let Some(track) = self.project.audio_tracks.first() {
            if let Ok((stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    if let Ok(file) = File::open(&track.file_path) {
                        if let Ok(source) = Decoder::new(BufReader::new(file)) {
                            sink.append(source);
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

    fn load_texture_if_needed(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.texture_cache.get(path) {
            return Some(tex.clone());
        }
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            );
            let handle = ctx.load_texture(path, color_image, egui::TextureOptions::LINEAR);
            self.texture_cache.insert(path.to_string(), handle.clone());
            return Some(handle);
        }
        None
    }
}

impl eframe::App for AniHadyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            scroll_y,
            close_req,
            pointer_down,
            pointer_pressed,
            pointer_released,
            pointer_pos,
            touch_force,
            sc_select,
            sc_pen,
            sc_brush,
            sc_eraser,
            sc_bucket,
            sc_eyedropper,
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
                i.raw_scroll_delta.y,
                i.viewport().close_requested(),
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
                stylus_force,
                is_shortcut_pressed(i, &self.config.tool_select),
                is_shortcut_pressed(i, &self.config.tool_pen),
                is_shortcut_pressed(i, &self.config.tool_brush),
                is_shortcut_pressed(i, &self.config.tool_eraser),
                is_shortcut_pressed(i, &self.config.tool_bucket),
                is_shortcut_pressed(i, &self.config.tool_eyedropper),
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
                1 => self.config.tool_pen = k_name,
                2 => self.config.tool_brush = k_name,
                3 => self.config.tool_eraser = k_name,
                4 => self.config.tool_bucket = k_name,
                5 => self.config.tool_eyedropper = k_name,
                6 => self.config.insert_frame = k_name,
                7 => self.config.insert_keyframe = k_name,
                8 => self.config.insert_blank_kf = k_name,
                9 => self.config.convert_symbol = k_name,
                10 => self.config.play_pause = k_name,
                11 => self.config.prev_frame = k_name,
                12 => self.config.next_frame = k_name,
                13 => self.config.brush_smaller = k_name,
                14 => self.config.brush_larger = k_name,
                15 => self.config.reset_canvas = k_name,
                _ => {}
            }
            self.config.save();
            self.listening_shortcut_idx = None;
            typing_in_textbox = true;
        }

        if self.show_symbol_dialog || self.show_exit_dialog || self.show_update_dialog || self.show_settings_dialog || self.show_credits_dialog {
            typing_in_textbox = true;
        }

        if !typing_in_textbox {
            if ctrl_z { self.undo(); }
            if ctrl_y { self.redo(); }
            if ctrl_s { self.trigger_save(); }
            if ctrl_c { self.copy_frame(self.active_layer_idx, self.current_frame); }
            if ctrl_v { self.paste_frame(self.active_layer_idx, self.current_frame); }

            if shift_f5 && self.timeline_length > 1 {
                self.save_undo_state();
                self.timeline_length -= 1;
                self.project.total_frames = self.timeline_length;
                if self.current_frame > self.timeline_length {
                    self.current_frame = self.timeline_length;
                }
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
            if sc_reset_canvas {
                self.pan_offset = egui::Vec2::ZERO;
                self.zoom_level = 1.0;
                self.status_message = self.tr("Canvas Centered (100%)", "تم توسيط مساحة الرسم (100%)").to_string();
            }
            if sc_select { self.active_tool = ToolType::Select; }
            if sc_pen { self.active_tool = ToolType::VectorPen; }
            if sc_brush { self.active_tool = ToolType::Brush; }
            if sc_eraser { self.active_tool = ToolType::Eraser; }
            if sc_bucket { self.active_tool = ToolType::Bucket; }
            if sc_eyedropper { self.active_tool = ToolType::Eyedropper; }
            if sc_smaller { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if sc_larger { self.brush_size = (self.brush_size + 2.0).min(100.0); }

            // زر Delete يحذف العنصر المحدد أو يمسح الفريم الحالي
            if del_pressed {
                if let Some(idx) = self.selected_symbol_inst_idx.take() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if idx < frame.symbol_instances.len() {
                                frame.symbol_instances.remove(idx);
                                self.has_unsaved_changes = true;
                            }
                        }
                    }
                } else if let Some(idx) = self.selected_stroke_idx.take() {
                    self.save_undo_state();
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if idx < frame.strokes.len() {
                                frame.strokes.remove(idx);
                                self.has_unsaved_changes = true;
                            }
                        }
                    }
                } else {
                    self.delete_or_clear_frame(self.active_layer_idx, self.current_frame);
                }
            }
        }

        if space_down {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }

        if self.is_playing {
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

        if self.show_credits_dialog {
            egui::Window::new(self.tr("About AniHady2D", "حول برنامج AniHady2D"))
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
                        ui.label(self.tr("Official YouTube Channel:", "قناة اليوتيوب الرسمية:"));
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
                        if ui.button(self.tr("Close", "إغلاق")).clicked() {
                            self.show_credits_dialog = false;
                        }
                    });
                });
        }

        // نافذة الإعدادات (اختيار اللغة عربي/إنجليزي + تخصيص كل الاختصارات)
        if self.show_settings_dialog {
            egui::Window::new(self.tr("Settings & Shortcuts", "الإعدادات والاختصارات"))
                .collapsible(false)
                .resizable(false)
                .default_width(450.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    // قائمة اختيار اللغة (عربي / English)
                    ui.horizontal(|ui| {
                        ui.strong(self.tr("Interface Language:", "لغة الواجهة:"));
                        if ui.selectable_label(self.config.lang == AppLang::English, "English").clicked() {
                            self.config.lang = AppLang::English;
                            self.config.save();
                        }
                        if ui.selectable_label(self.config.lang == AppLang::Arabic, "العربية (Arabic)").clicked() {
                            self.config.lang = AppLang::Arabic;
                            self.config.save();
                        }
                    });
                    ui.separator();
                    ui.label(self.tr(
                        "Click any shortcut button, then press your desired key:",
                        "اضغط على أي اختصار ثم اضغط الزر المطلوب من لوحة المفاتيح:",
                    ));
                    ui.add_space(4.0);

                    let items = [
                        (0, self.tr("Select Tool", "أداة التحديد والتحريك"), &self.config.tool_select),
                        (1, self.tr("Vector Pen Tool", "أداة قلم الفيكتور"), &self.config.tool_pen),
                        (2, self.tr("Brush Tool", "أداة الفرشاة"), &self.config.tool_brush),
                        (3, self.tr("Eraser Tool", "أداة الممحاة"), &self.config.tool_eraser),
                        (4, self.tr("Paint Bucket Tool", "أداة دلو التعبئة"), &self.config.tool_bucket),
                        (5, self.tr("Eyedropper Tool", "أداة قطارة الألوان"), &self.config.tool_eyedropper),
                        (6, self.tr("Insert Frame (Extend)", "تمديد الفريم الحالي"), &self.config.insert_frame),
                        (7, self.tr("Insert Keyframe (Duplicate)", "إدراج فريم منسوخ"), &self.config.insert_keyframe),
                        (8, self.tr("Insert Blank Keyframe", "إدراج فريم فارغ جديد"), &self.config.insert_blank_kf),
                        (9, self.tr("Convert to Symbol", "تحويل إلى رمز (Symbol)"), &self.config.convert_symbol),
                        (10, self.tr("Play / Pause", "تشغيل / إيقاف"), &self.config.play_pause),
                        (11, self.tr("Previous Frame", "الفريم السابق"), &self.config.prev_frame),
                        (12, self.tr("Next Frame", "الفريم التالي"), &self.config.next_frame),
                        (13, self.tr("Decrease Brush Size", "تصغير حجم الفرشاة"), &self.config.brush_smaller),
                        (14, self.tr("Increase Brush Size", "تكبير حجم الفرشاة"), &self.config.brush_larger),
                        (15, self.tr("Reset Canvas View", "إرجاع الكانفا للمنتصف"), &self.config.reset_canvas),
                    ];
                    egui::ScrollArea::vertical().max_height(310.0).show(ui, |ui| {
                        for (idx, label, current_key) in items {
                            ui.horizontal(|ui| {
                                ui.allocate_ui(egui::vec2(230.0, 22.0), |ui| {
                                    ui.label(label);
                                });
                                let is_listening = self.listening_shortcut_idx == Some(idx);
                                let btn_text = if is_listening {
                                    self.tr("Press any key...", "اضغط أي زر...").to_string()
                                } else {
                                    current_key.clone()
                                };
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
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(self.tr("Reset Defaults", "استعادة الافتراضي")).clicked() {
                            let saved_lang = self.config.lang;
                            self.config = AppConfig::default();
                            self.config.lang = saved_lang;
                            self.config.save();
                            self.listening_shortcut_idx = None;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(self.tr("Done", "تم")).clicked() {
                                self.listening_shortcut_idx = None;
                                self.show_settings_dialog = false;
                            }
                        });
                    });
                });
        }

        if self.show_update_dialog {
            let current_st = self.update_state.lock().unwrap().clone();
            egui::Window::new(self.tr("AniHady2D Updater", "تحديث البرنامج"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Current Version: v{}", self.installed_version));
                    ui.separator();
                    match current_st {
                        UpdateState::Idle => {}
                        UpdateState::Checking => {
                            ui.horizontal(|ui| { ui.spinner(); ui.label(self.tr("Checking GitHub...", "جاري البحث عن تحديثات...")); });
                        }
                        UpdateState::UpToDate => {
                            ui.colored_label(egui::Color32::GREEN, self.tr("You are on the latest version!", "لديك أحدث إصدار بالفعل!"));
                            if ui.button(self.tr("Close", "إغلاق")).clicked() { self.show_update_dialog = false; }
                        }
                        UpdateState::UpdateAvailable { version, download_url } => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, format!("New Version: v{}", version));
                            ui.horizontal(|ui| {
                                if ui.button(self.tr("Download & Install", "تحميل وتثبيت")).clicked() {
                                    self.download_and_apply_update(version, download_url, ctx.clone());
                                }
                                if ui.button(self.tr("Cancel", "إلغاء")).clicked() { self.show_update_dialog = false; }
                            });
                        }
                        UpdateState::Downloading(msg) => {
                            ui.horizontal(|ui| { ui.spinner(); ui.label(msg); });
                        }
                        UpdateState::ReadyToRestart => {
                            ui.colored_label(egui::Color32::GREEN, self.tr("Update ready! Restart now?", "اكتمل التحميل! إعادة التشغيل الآن؟"));
                            if ui.button(self.tr("Restart & Apply", "إعادة التشغيل والتحديث")).clicked() { self.restart_with_new_exe(); }
                        }
                        UpdateState::Error(err) => {
                            ui.colored_label(egui::Color32::RED, err);
                            if ui.button(self.tr("Close", "إغلاق")).clicked() { self.show_update_dialog = false; }
                        }
                    }
                });
        }

        if self.show_symbol_dialog {
            egui::Window::new(self.tr("Convert to Symbol", "تحويل إلى رمز"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(self.tr("Symbol Name:", "اسم الرمز:"));
                    ui.text_edit_singleline(&mut self.new_symbol_name);
                    ui.horizontal(|ui| {
                        if ui.button(self.tr("Create", "إنشاء")).clicked() {
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
                                    }
                                }
                            }
                            self.show_symbol_dialog = false;
                        }
                        if ui.button(self.tr("Cancel", "إلغاء")).clicked() { self.show_symbol_dialog = false; }
                    });
                });
        }

        if close_req && !self.allow_close {
            if self.has_unsaved_changes {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_exit_dialog = true;
            }
        }

        if self.show_exit_dialog {
            egui::Window::new(self.tr("Unsaved Changes", "تغييرات غير محفوظة"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(self.tr("Save changes before closing?", "هل تريد حفظ المشروع قبل الإغلاق؟"));
                    ui.horizontal(|ui| {
                        if ui.button(self.tr("Save & Exit", "حفظ وخروج")).clicked() && self.trigger_save() {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(self.tr("Don't Save", "عدم الحفظ")).clicked() {
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
                    self.show_credits_dialog = true;
                }

                ui.colored_label(egui::Color32::from_rgb(80, 175, 255), "AniHady2D");
                ui.separator();
                ui.menu_button(self.tr("File", "ملف"), |ui| {
                    if ui.button(self.tr("Save Project (Ctrl+S)", "حفظ المشروع")).clicked() { self.trigger_save(); ui.close_menu(); }
                    if ui.button(self.tr("Open Project (.ah2d)", "فتح مشروع")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("AniHady2D", &["ah2d"]).pick_file() {
                            if let Ok(proj) = AniProject::load_from_file(&path) {
                                self.timeline_length = proj.total_frames.max(1);
                                self.project = proj;
                                self.has_unsaved_changes = false;
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(self.tr("Import Image...", "استيراد صورة...")).clicked() { self.import_image_dialog(); ui.close_menu(); }
                    if ui.button(self.tr("Import Video...", "استيراد فيديو...")).clicked() { self.import_video_dialog(); ui.close_menu(); }
                    if ui.button(self.tr("Import Audio...", "استيراد صوت...")).clicked() { self.import_audio_dialog(); ui.close_menu(); }
                    ui.separator();
                    if ui.button(self.tr("Export Video (MP4)...", "تصدير فيديو (MP4)...")).clicked() {
                        self.project.total_frames = self.timeline_length;
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("MP4 Video", &["mp4"])
                            .set_file_name("AniHady2D_Export.mp4")
                            .save_file()
                        {
                            match exporter::export_to_mp4(&self.project, &path) {
                                Ok(_) => self.status_message = self.tr("Exported MP4!", "تم تصدير الفيديو!").to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button(self.tr("Edit", "تحرير"), |ui| {
                    if ui.add_enabled(!self.undo_stack.is_empty(), egui::Button::new(self.tr("Undo (Ctrl+Z)", "تراجع (Ctrl+Z)"))).clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(!self.redo_stack.is_empty(), egui::Button::new(self.tr("Redo (Ctrl+Y)", "إعادة (Ctrl+Y)"))).clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(self.tr("Copy Frame (Ctrl+C)", "نسخ الفريم (Ctrl+C)")).clicked() {
                        self.copy_frame(self.active_layer_idx, self.current_frame);
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.clipboard_frame.is_some(), egui::Button::new(self.tr("Paste Frame (Ctrl+V)", "لصق الفريم (Ctrl+V)"))).clicked() {
                        self.paste_frame(self.active_layer_idx, self.current_frame);
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.add_enabled(!self.undo_stack.is_empty(), egui::Button::new("↶")).on_hover_text("Undo (Ctrl+Z)").clicked() {
                    self.undo();
                }
                if ui.add_enabled(!self.redo_stack.is_empty(), egui::Button::new("↷")).on_hover_text("Redo (Ctrl+Y)").clicked() {
                    self.redo();
                }
                ui.separator();
                if ui.button(self.tr("+ Image", "+ صورة")).clicked() { self.import_image_dialog(); }
                if ui.button(self.tr("+ Video", "+ فيديو")).clicked() { self.import_video_dialog(); }
                if ui.button(self.tr("+ Audio", "+ صوت")).clicked() { self.import_audio_dialog(); }
                ui.separator();
                if ui.button(self.tr("Updates", "تحديثات")).clicked() { self.check_for_updates(ctx.clone()); }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (gear_rect, gear_resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                    if gear_resp.hovered() {
                        ui.painter().rect_filled(gear_rect, 4.0, egui::Color32::from_gray(65));
                    }
                    draw_gear_icon(ui.painter(), gear_rect);
                    if gear_resp.on_hover_text(self.tr("Settings & Language", "الإعدادات واللغة")).clicked() {
                        self.show_settings_dialog = true;
                    }

                    ui.separator();
                    ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                    if ui.small_button(format!("{} ({})", self.tr("Center", "توسيط"), self.config.reset_canvas)).clicked() {
                        self.zoom_level = 1.0;
                        self.pan_offset = egui::Vec2::ZERO;
                    }
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_GRAY, &self.status_message);
                });
            });
        });

        // شريط الأدوات الأيسر (قابل للتوسيع والتضييق بحرية)
        egui::SidePanel::left("tools_panel")
            .resizable(true)
            .default_width(64.0)
            .width_range(52.0..=220.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let tools = [
                        (ToolType::Select, self.tr("Select", "تحديد"), &self.config.tool_select),
                        (ToolType::VectorPen, self.tr("Vector Pen", "قلم فيكتور"), &self.config.tool_pen),
                        (ToolType::Brush, self.tr("Brush", "فرشاة"), &self.config.tool_brush),
                        (ToolType::Eraser, self.tr("Eraser", "ممحاة"), &self.config.tool_eraser),
                        (ToolType::Bucket, self.tr("Bucket", "تعبئة"), &self.config.tool_bucket),
                        (ToolType::Eyedropper, self.tr("Picker", "قطارة"), &self.config.tool_eyedropper),
                    ];
                    let show_labels = ui.available_width() > 95.0;
                    for (tool, name, sc) in tools {
                        let selected = self.active_tool == tool;
                        let accent = get_tool_accent_color(tool);
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
                        draw_vector_tool_icon(ui.painter(), icon_rect, tool, selected);
                        if show_labels {
                            ui.painter().text(
                                icon_rect.right_center() + egui::vec2(4.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{} ({})", name, sc),
                                egui::FontId::proportional(13.0),
                                egui::Color32::WHITE,
                            );
                        }
                        if resp.on_hover_text(format!("{} ({})", name, sc)).clicked() {
                            self.active_tool = tool;
                        }
                    }
                    ui.separator();
                    ui.small(self.tr("Color", "اللون"));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
                    ui.add_space(4.0);
                    if ui.small_button("+Brush").on_hover_text("Import Brush").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Brush", &["png", "abr"]).pick_file() {
                            self.imported_brushes.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                        }
                    }
                });
            });

        // لوحة الخصائص والمكتبة اليمنى (قابلة للتوسيع والتضييق بحرية)
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(240.0)
            .width_range(160.0..=480.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.right_tab == 0, self.tr("Properties", "الخصائص")).clicked() { self.right_tab = 0; }
                    if ui.selectable_label(self.right_tab == 1, self.tr("Library & Audio", "المكتبة والصوت")).clicked() { self.right_tab = 1; }
                });
                ui.separator();

                if self.right_tab == 0 {
                    ui.strong(self.tr("Tool Settings", "إعدادات الأداة"));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(self.tr("Size:", "الحجم:"));
                        ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                    });
                    ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));

                    ui.add_space(6.0);
                    ui.checkbox(&mut self.pen_pressure_enabled, self.tr("Pen Pressure Sensitivity", "حساسية ضغط القلم"));
                    if self.pen_pressure_enabled {
                        ui.horizontal(|ui| {
                            ui.label(self.tr("Sensitivity:", "درجة الحساسية:"));
                            ui.add(egui::Slider::new(&mut self.pressure_sensitivity, 0.2..=2.0));
                        });
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.strong(self.tr("Stage / Document", "إعدادات المسرح"));
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
                        ui.label(self.tr("Stage Color:", "لون الخلفية:"));
                        ui.color_edit_button_srgba_unmultiplied(&mut self.stage_bg_color);
                    });
                    ui.horizontal(|ui| {
                        ui.label("FPS:");
                        ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=60));
                    });
                } else {
                    ui.strong(self.tr("Symbols Library", "مكتبة الرموز"));
                    let sym_names: Vec<String> = self.project.library_symbols.keys().cloned().collect();
                    if sym_names.is_empty() {
                        ui.small(self.tr("Draw & press F8 to create symbol", "ارسم واضغط F8 لإنشاء رمز"));
                    }
                    for s_name in sym_names {
                        ui.horizontal(|ui| {
                            ui.label(&s_name);
                            if ui.small_button(self.tr("+ Place", "+ إدراج")).clicked() {
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
                                }
                            }
                        });
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.strong(self.tr("Audio Tracks", "ملفات الصوت"));
                    if ui.button(self.tr("+ Import Audio", "+ إضافة صوت")).clicked() {
                        self.import_audio_dialog();
                    }
                    let mut remove_audio_idx = None;
                    for (idx, track) in self.project.audio_tracks.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(&track.name);
                            if ui.small_button("X").clicked() {
                                remove_audio_idx = Some(idx);
                            }
                        });
                    }
                    if let Some(idx) = remove_audio_idx {
                        self.save_undo_state();
                        self.project.audio_tracks.remove(idx);
                    }
                }
            });

        // التايم لاين السفلي (قابل للتوسيع والتضييق بحرية + قائمة كليك يمين احترافية)
        egui::TopBottomPanel::bottom("timeline_panel")
            .resizable(true)
            .default_height(215.0)
            .height_range(100.0..=500.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("|<").clicked() { self.current_frame = 1; }
                    if ui.button("<").clicked() && self.current_frame > 1 { self.current_frame -= 1; }
                    if ui.button(if self.is_playing { self.tr("Pause", "إيقاف") } else { self.tr("Play", "تشغيل") }).clicked() {
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
                    if ui.button(format!("{} ({})", self.tr("+ Frame", "+ تمديد"), self.config.insert_frame)).clicked() {
                        self.action_insert_frame_f5();
                    }
                    if ui.button(format!("{} ({})", self.tr("+ Keyframe", "+ فريم منسوخ"), self.config.insert_keyframe)).clicked() {
                        self.action_insert_keyframe_f6();
                    }
                    if ui.button(format!("{} ({})", self.tr("+ Blank KF", "+ فريم فارغ"), self.config.insert_blank_kf)).clicked() {
                        self.action_insert_blank_keyframe_f7();
                    }

                    ui.separator();
                    ui.checkbox(&mut self.project.onion_skin.enabled, self.tr("Onion Skin", "شفافية الحركة"));
                    if self.project.onion_skin.enabled {
                        ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_before).range(0..=10));
                        ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_before);
                        ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_after).range(0..=10));
                        ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_after);
                    }

                    ui.separator();
                    if ui.button(self.tr("+ Layer", "+ طبقة")).clicked() {
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
                    }
                });

                ui.separator();

                let mut pending_copy: Option<(usize, usize)> = None;
                let mut pending_paste: Option<(usize, usize)> = None;
                let mut pending_delete_frame: Option<(usize, usize)> = None;
                let mut pending_dup_layer: Option<usize> = None;
                let mut delete_layer_idx: Option<usize> = None;

                egui::ScrollArea::both().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(145.0, 20.0), |ui| {
                            ui.strong(self.tr("Layers", "الطبقات"));
                        });
                        for f in 1..=self.timeline_length {
                            let is_curr = self.current_frame == f;
                            let btn = egui::Button::new(format!("{}", f))
                                .min_size(egui::vec2(26.0, 18.0))
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
                    let lbl_copy = self.tr("Copy Frame (Ctrl+C)", "نسخ الفريم (Copy)").to_string();
                    let lbl_paste = self.tr("Paste Frame (Ctrl+V)", "لصق الفريم (Paste)").to_string();
                    let lbl_del_frame = self.tr("Delete / Clear Frame (Del)", "حذف محتوى الفريم (Delete)").to_string();
                    let lbl_dup_layer = self.tr("Duplicate Layer", "تكرار الطبقة").to_string();
                    let lbl_del_layer = self.tr("Delete Layer", "حذف الطبقة").to_string();

                    for (l_idx, layer) in self.project.layers.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(145.0, 24.0), |ui| {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut layer.visible, "");
                                    let layer_resp = ui.selectable_label(self.active_layer_idx == l_idx, &layer.name);
                                    if layer_resp.clicked() {
                                        self.active_layer_idx = l_idx;
                                    }
                                    // كليك يمين على الطبقة (Layer Context Menu)
                                    layer_resp.context_menu(|ui| {
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
                                    !fd.strokes.is_empty() || !fd.media_items.is_empty() || !fd.symbol_instances.is_empty()
                                });

                                let is_selected = self.active_layer_idx == l_idx && self.current_frame == f;
                                let symbol = if is_keyframe {
                                    if has_drawings { "●" } else { "○" }
                                } else if has_drawings {
                                    "─"
                                } else {
                                    " "
                                };

                                let fill = if is_selected {
                                    egui::Color32::from_rgb(0, 122, 204)
                                } else if has_drawings {
                                    egui::Color32::from_rgb(72, 72, 78)
                                } else {
                                    egui::Color32::from_rgb(42, 42, 46)
                                };

                                let cell = egui::Button::new(symbol)
                                    .min_size(egui::vec2(26.0, 22.0))
                                    .fill(fill);
                                let cell_resp = ui.add(cell);
                                if cell_resp.clicked() {
                                    self.active_layer_idx = l_idx;
                                    self.current_frame = f;
                                }
                                // كليك يمين على أي فريم (Copy / Paste / Delete)
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
                                    if ui.button(&lbl_del_frame).clicked() {
                                        pending_delete_frame = Some((l_idx, f));
                                        ui.close_menu();
                                    }
                                });
                            }
                        });
                    }

                    for track in &self.project.audio_tracks {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(145.0, 22.0), |ui| {
                                ui.colored_label(egui::Color32::from_rgb(100, 210, 255), &track.name);
                            });
                            for f in 1..=self.timeline_length {
                                let active = f >= track.start_frame;
                                let fill = if active {
                                    egui::Color32::from_rgb(25, 115, 135)
                                } else {
                                    egui::Color32::from_rgb(30, 30, 33)
                                };
                                let cell = egui::Button::new(if active { "~" } else { " " })
                                    .min_size(egui::vec2(26.0, 20.0))
                                    .fill(fill);
                                if ui.add(cell).clicked() {
                                    self.current_frame = f;
                                }
                            }
                        });
                    }
                });

                if let Some((l, f)) = pending_copy {
                    self.copy_frame(l, f);
                }
                if let Some((l, f)) = pending_paste {
                    self.paste_frame(l, f);
                }
                if let Some((l, f)) = pending_delete_frame {
                    self.delete_or_clear_frame(l, f);
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

        // مساحة العمل والكانفا
        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(canvas_rect);
            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));

            if scroll_y != 0.0 && is_over_canvas {
                self.zoom_level = (self.zoom_level + scroll_y * 0.001).clamp(0.1, 4.0);
            }

            if space_down && pointer_down {
                if let (Some(curr), Some(last)) = (pointer_pos, self.last_pointer_pos) {
                    self.pan_offset += curr - last;
                }
            }

            let center = canvas_rect.center() + self.pan_offset;
            let scale = self.zoom_level.max(0.1);
            let stage_size = egui::vec2(self.project.width as f32, self.project.height as f32) * scale;
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

            let to_stage = |p: egui::Pos2| egui::pos2((p.x - stage_rect.min.x) / scale, (p.y - stage_rect.min.y) / scale);
            let to_screen = |p: egui::Pos2| egui::pos2(stage_rect.min.x + p.x * scale, stage_rect.min.y + p.y * scale);

            let modal_open = self.show_update_dialog
                || self.show_settings_dialog
                || self.show_symbol_dialog
                || self.show_exit_dialog
                || self.show_credits_dialog;

            if !space_down && is_over_canvas && !modal_open {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |last| (pos - last) / scale);

                    match self.active_tool {
                        ToolType::Select => {
                            if pointer_pressed {
                                self.selected_stroke_idx = None;
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
                                                    self.selected_stroke_idx = Some(i);
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
                                        } else if let Some(idx) = self.selected_stroke_idx {
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
                        ToolType::Bucket if pointer_pressed => {
                            self.save_undo_state();
                            self.ensure_editable_keyframe();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    for s in frame.strokes.iter_mut().rev() {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 35.0) {
                                            s.fill_color = Some(self.brush_color);
                                            self.has_unsaved_changes = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ToolType::Eraser if pointer_down => {
                            if pointer_pressed {
                                self.save_undo_state();
                            }
                            self.ensure_editable_keyframe();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    let r = self.brush_size;
                                    frame.strokes.retain(|s| !s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < r));
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        ToolType::Eyedropper if pointer_pressed => {
                            if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                if let Some(frame) = Self::get_effective_frame_data(layer, self.current_frame) {
                                    for s in &frame.strokes {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 15.0) {
                                            self.brush_color = s.color;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ToolType::VectorPen | ToolType::Brush if pointer_down => {
                            let speed = delta.length();
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
                                    is_vector: self.active_tool == ToolType::VectorPen,
                                    brush_texture_id: None,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if pointer_released {
                if let Some(mut stroke) = self.current_stroke.take() {
                    if stroke.is_vector {
                        stroke.smooth_vector();
                    }
                    self.ensure_editable_keyframe();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        layer.keyframes.entry(self.current_frame).or_default().strokes.push(stroke);
                        self.has_unsaved_changes = true;
                    }
                }
            }

            self.last_pointer_pos = if pointer_down { pointer_pos } else { None };

            if self.project.onion_skin.enabled && !self.is_playing {
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

            let mut media_to_draw = Vec::new();
            for layer in &self.project.layers {
                if layer.visible {
                    if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame) {
                        media_to_draw.extend(fd.media_items.clone());
                    }
                }
            }
            for m in media_to_draw {
                if let Some(tex) = self.load_texture_if_needed(ctx, &m.path) {
                    painter.image(
                        tex.id(),
                        egui::Rect::from_min_max(to_screen(egui::pos2(m.x, m.y)), to_screen(egui::pos2(m.x + m.width, m.y + m.height))),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }

            for layer in &self.project.layers {
                if layer.visible {
                    if let Some(fd) = Self::get_effective_frame_data(layer, self.current_frame) {
                        for s in &fd.strokes {
                            draw_stroke_pro(&painter, s, None, scale, 0.0, 0.0, &to_screen);
                        }
                        for inst in &fd.symbol_instances {
                            if let Some(sym) = self.project.library_symbols.get(&inst.symbol_name) {
                                for s in &sym.strokes {
                                    draw_stroke_pro(&painter, s, None, scale * inst.scale, inst.x, inst.y, &to_screen);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(stroke) = &self.current_stroke {
                draw_stroke_pro(&painter, stroke, None, scale, 0.0, 0.0, &to_screen);
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

    if stroke.is_vector {
        let pts: Vec<egui::Pos2> = stroke
            .points
            .iter()
            .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
            .collect();
        painter.add(egui::Shape::line(pts, egui::Stroke::new(stroke.base_width * scale, color)));
    } else {
        let total = stroke.points.len();
        for (idx, w) in stroke.points.windows(2).enumerate() {
            let p0 = to_screen(egui::pos2(w[0].x + ox, w[0].y + oy));
            let p1 = to_screen(egui::pos2(w[1].x + ox, w[1].y + oy));

            let progress = idx as f32 / total as f32;
            let taper = if progress < 0.08 {
                (progress / 0.08).max(0.35)
            } else if progress > 0.92 {
                ((1.0 - progress) / 0.08).max(0.35)
            } else {
                1.0
            };

            let avg_pressure = ((w[0].pressure + w[1].pressure) * 0.5) * taper;
            let width = (stroke.base_width * avg_pressure * scale).max(1.0);

            painter.line_segment([p0, p1], egui::Stroke::new(width, color));
            painter.circle_filled(p1, width * 0.5, color);
        }
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
