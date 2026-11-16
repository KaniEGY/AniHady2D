mod exporter;
mod project;

use eframe::egui;
use project::{AniProject, AudioTrack, Layer, MediaItem, MediaType, Stroke, StrokePoint, Symbol, SymbolInstance, ToolType};
use rodio::{Decoder, OutputStream, Sink};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "KaniEGY/AniHady2D";

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
    style.visuals.panel_fill = egui::Color32::from_rgb(38, 38, 40);
    style.visuals.window_fill = egui::Color32::from_rgb(44, 44, 47);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(24, 24, 26);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 120, 215);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(52, 52, 56);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(68, 68, 74);
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    ctx.set_style(style);
}

struct AniHadyApp {
    project: AniProject,
    undo_stack: Vec<AniProject>,
    current_frame: usize,
    active_layer_idx: usize,
    active_tool: ToolType,
    brush_color: [u8; 4],
    stage_bg_color: [u8; 4],
    brush_size: f32,
    pen_pressure_enabled: bool,
    zoom_level: f32,
    pan_offset: egui::Vec2,
    last_space_tap_time: f64,
    is_playing: bool,
    last_frame_time: f64,
    has_unsaved_changes: bool,
    show_exit_dialog: bool,
    show_symbol_dialog: bool,
    show_update_dialog: bool,
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
    right_tab: usize, // 0: Properties, 1: Library & Audio
}

impl AniHadyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        apply_animate_pro_theme(&cc.egui_ctx);
        Self {
            project: AniProject::default(),
            undo_stack: Vec::new(),
            current_frame: 1,
            active_layer_idx: 0,
            active_tool: ToolType::Brush,
            brush_color: [0, 0, 0, 255],
            stage_bg_color: [255, 255, 255, 255],
            brush_size: 4.0,
            pen_pressure_enabled: true,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
            last_space_tap_time: -10.0,
            is_playing: false,
            last_frame_time: 0.0,
            has_unsaved_changes: false,
            show_exit_dialog: false,
            show_symbol_dialog: false,
            show_update_dialog: false,
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

    fn insert_keyframe_f6(&mut self) {
        self.save_undo_state();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            let prev_data = if self.current_frame > 1 {
                layer.keyframes.get(&(self.current_frame - 1)).cloned().unwrap_or_default()
            } else {
                Default::default()
            };
            layer.keyframes.insert(self.current_frame, prev_data);
            self.has_unsaved_changes = true;
            self.status_message = format!("Keyframe inserted at Frame {} (F6)", self.current_frame);
        }
    }

    fn check_for_updates(&mut self, ctx: egui::Context) {
        self.show_update_dialog = true;
        *self.update_state.lock().unwrap() = UpdateState::Checking;
        let state_clone = Arc::clone(&self.update_state);

        std::thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
            match ureq::get(&api_url).set("User-Agent", "AniHady2D-Updater").call() {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<serde_json::Value>() {
                        let tag = json["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
                        let mut exe_url = String::new();
                        if let Some(assets) = json["assets"].as_array() {
                            for asset in assets {
                                if asset["name"].as_str().unwrap_or("").ends_with(".exe") {
                                    exe_url = asset["browser_download_url"].as_str().unwrap_or("").to_string();
                                    break;
                                }
                            }
                        }
                        let mut st = state_clone.lock().unwrap();
                        if !tag.is_empty() && tag != CURRENT_VERSION && !exe_url.is_empty() {
                            *st = UpdateState::UpdateAvailable { version: tag, download_url: exe_url };
                        } else {
                            *st = UpdateState::UpToDate;
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

    fn download_and_apply_update(&mut self, download_url: String, ctx: egui::Context) {
        *self.update_state.lock().unwrap() = UpdateState::Downloading("Downloading update...".to_string());
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
            let bat_path = current_exe.with_file_name("update_anihady.bat");
            let bat_content = format!(
                "@echo off\r\ntimeout /t 2 /nobreak > NUL\r\nmove /y \"{}\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
                new_exe.display(),
                current_exe.display(),
                current_exe.display()
            );
            if std::fs::write(&bat_path, bat_content).is_ok() {
                let _ = std::process::Command::new("cmd").args(["/C", &bat_path.to_string_lossy()]).spawn();
                std::process::exit(0);
            }
        }
    }

    fn save_undo_state(&mut self) {
        if self.undo_stack.len() > 25 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.project.clone());
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.project = prev;
            self.selected_stroke_idx = None;
            self.selected_symbol_inst_idx = None;
            self.status_message = "Undo".to_string();
        }
    }

    fn trigger_save(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("AniHady2D Project", &["ah2d"])
            .set_file_name("project.ah2d")
            .save_file()
        {
            if self.project.save_to_file(&path).is_ok() {
                self.saved_path = Some(path);
                self.has_unsaved_changes = false;
                self.status_message = "Saved!".to_string();
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
                self.status_message = "Image Imported".to_string();
            }
        }
    }

    fn import_video_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Videos", &["mp4", "mov", "avi", "mkv"])
            .pick_file()
        {
            self.status_message = "Extracting video frames...".to_string();
            match exporter::extract_video_frames(&path, self.project.fps) {
                Ok(frames) => {
                    self.save_undo_state();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        for (i, f_path) in frames.iter().enumerate() {
                            let f_idx = self.current_frame + i;
                            if f_idx <= self.project.total_frames {
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
            ctrl_s,
            f5_pressed,
            f6_pressed,
            f8_pressed,
            enter_pressed,
            comma_pressed,
            period_pressed,
            del_pressed,
            space_down,
            space_pressed,
            key_v,
            key_p,
            key_b,
            key_e,
            key_k,
            key_i,
            bracket_left,
            bracket_right,
            scroll_y,
            close_req,
            pointer_down,
            pointer_pressed,
            pointer_released,
            pointer_pos,
        ) = ctx.input(|i| {
            (
                i.time,
                i.modifiers.ctrl && i.key_pressed(egui::Key::Z),
                i.modifiers.ctrl && i.key_pressed(egui::Key::S),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::F6),
                i.key_pressed(egui::Key::F8),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Comma),
                i.key_pressed(egui::Key::Period),
                i.key_pressed(egui::Key::Delete),
                i.key_down(egui::Key::Space),
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::V),
                i.key_pressed(egui::Key::P),
                i.key_pressed(egui::Key::B),
                i.key_pressed(egui::Key::E),
                i.key_pressed(egui::Key::K),
                i.key_pressed(egui::Key::I),
                i.key_pressed(egui::Key::OpenBracket),
                i.key_pressed(egui::Key::CloseBracket),
                i.raw_scroll_delta.y,
                i.viewport().close_requested(),
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
            )
        });

        if self.show_symbol_dialog || self.show_exit_dialog || self.show_update_dialog {
            typing_in_textbox = true;
        }

        // الاختصارات الاحترافية (Adobe Animate Shortcuts)
        if !typing_in_textbox {
            if ctrl_z {
                self.undo();
            }
            if ctrl_s {
                self.trigger_save();
            }
            if f5_pressed {
                self.project.total_frames += 10;
                self.status_message = format!("Extended timeline to {} frames (F5)", self.project.total_frames);
            }
            if f6_pressed {
                self.insert_keyframe_f6();
            }
            if f8_pressed {
                self.show_symbol_dialog = true;
            }
            if enter_pressed {
                self.is_playing = !self.is_playing;
                if self.is_playing {
                    self.start_audio_playback();
                } else {
                    self.stop_audio_playback();
                }
            }
            if comma_pressed && self.current_frame > 1 {
                self.current_frame -= 1;
            }
            if period_pressed && self.current_frame < self.project.total_frames {
                self.current_frame += 1;
            }
            // دبل مسطرة (Double Space) لإرجاع الكانفا لمنتصف الشاشة والزوم 100%
            if space_pressed {
                if now - self.last_space_tap_time < 0.35 {
                    self.pan_offset = egui::Vec2::ZERO;
                    self.zoom_level = 1.0;
                    self.status_message = "Canvas Reset (100%)".to_string();
                    self.last_space_tap_time = -10.0;
                } else {
                    self.last_space_tap_time = now;
                }
            }
            // اختصارات الأدوات
            if key_v { self.active_tool = ToolType::Select; }
            if key_p { self.active_tool = ToolType::VectorPen; }
            if key_b { self.active_tool = ToolType::Brush; }
            if key_e { self.active_tool = ToolType::Eraser; }
            if key_k { self.active_tool = ToolType::Bucket; }
            if key_i { self.active_tool = ToolType::Eyedropper; }
            if bracket_left { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if bracket_right { self.brush_size = (self.brush_size + 2.0).min(100.0); }

            if del_pressed {
                if let Some(idx) = self.selected_stroke_idx.take() {
                    self.save_undo_state();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                            if idx < frame.strokes.len() {
                                frame.strokes.remove(idx);
                                self.has_unsaved_changes = true;
                            }
                        }
                    }
                }
            }
        }

        if space_down {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }

        if self.is_playing {
            if now - self.last_frame_time >= 1.0 / (self.project.fps.max(1) as f64) {
                self.current_frame += 1;
                if self.current_frame > self.project.total_frames {
                    self.current_frame = 1;
                    self.stop_audio_playback();
                    self.start_audio_playback();
                }
                self.last_frame_time = now;
            }
            ctx.request_repaint();
        }

        // نافذة التحديثات
        if self.show_update_dialog {
            let current_st = self.update_state.lock().unwrap().clone();
            egui::Window::new("AniHady2D Updater")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Current Version: v{}", CURRENT_VERSION));
                    ui.separator();
                    match current_st {
                        UpdateState::Idle => {}
                        UpdateState::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Checking GitHub for updates...");
                            });
                        }
                        UpdateState::UpToDate => {
                            ui.colored_label(egui::Color32::GREEN, "You are on the latest version!");
                            if ui.button("Close").clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                        UpdateState::UpdateAvailable { version, download_url } => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, format!("New Version Available: v{}", version));
                            ui.horizontal(|ui| {
                                if ui.button("Download & Install").clicked() {
                                    self.download_and_apply_update(download_url, ctx.clone());
                                }
                                if ui.button("Cancel").clicked() {
                                    self.show_update_dialog = false;
                                }
                            });
                        }
                        UpdateState::Downloading(msg) => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(msg);
                            });
                        }
                        UpdateState::ReadyToRestart => {
                            ui.colored_label(egui::Color32::GREEN, "Update ready! Restart now?");
                            if ui.button("Restart & Apply").clicked() {
                                self.restart_with_new_exe();
                            }
                        }
                        UpdateState::Error(err) => {
                            ui.colored_label(egui::Color32::RED, err);
                            if ui.button("Close").clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                    }
                });
        }

        if self.show_symbol_dialog {
            egui::Window::new("Convert to Symbol (F8)")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Symbol Name:");
                    ui.text_edit_singleline(&mut self.new_symbol_name);
                    ui.horizontal(|ui| {
                        if ui.button("Create Symbol").clicked() {
                            self.save_undo_state();
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
                        if ui.button("Cancel").clicked() {
                            self.show_symbol_dialog = false;
                        }
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
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Save changes to project before closing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save & Exit").clicked() && self.trigger_save() {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Don't Save").clicked() {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }

        // الشريط العلوي الاحترافي (Adobe Animate Top Header)
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.colored_label(egui::Color32::from_rgb(80, 170, 255), "◆ AniHady2D");
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("Save Project (Ctrl+S)").clicked() {
                        self.trigger_save();
                        ui.close_menu();
                    }
                    if ui.button("Open Project (.ah2d)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("AniHady2D", &["ah2d"]).pick_file() {
                            if let Ok(proj) = AniProject::load_from_file(&path) {
                                self.project = proj;
                                self.has_unsaved_changes = false;
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Import Image...").clicked() {
                        self.import_image_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Import Video...").clicked() {
                        self.import_video_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Import Audio...").clicked() {
                        self.import_audio_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Video (MP4)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("MP4 Video", &["mp4"])
                            .set_file_name("AniHady2D_Export.mp4")
                            .save_file()
                        {
                            match exporter::export_to_mp4(&self.project, &path) {
                                Ok(_) => self.status_message = "Exported MP4!".to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.button("Reset Canvas View (Double Space)").clicked() {
                        self.pan_offset = egui::Vec2::ZERO;
                        self.zoom_level = 1.0;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Insert", |ui| {
                    if ui.button("Keyframe (F6)").clicked() {
                        self.insert_keyframe_f6();
                        ui.close_menu();
                    }
                    if ui.button("Convert to Symbol (F8)").clicked() {
                        self.show_symbol_dialog = true;
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.button("🖼 Image").clicked() { self.import_image_dialog(); }
                if ui.button("🎬 Video").clicked() { self.import_video_dialog(); }
                if ui.button("🎵 Audio").clicked() { self.import_audio_dialog(); }
                ui.separator();
                if ui.button("🔄 Updates").clicked() { self.check_for_updates(ctx.clone()); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Zoom: {:.0}%", self.zoom_level * 100.0));
                    if ui.small_button("100%").clicked() {
                        self.zoom_level = 1.0;
                        self.pan_offset = egui::Vec2::ZERO;
                    }
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_GRAY, &self.status_message);
                });
            });
        });

        // شريط الأدوات الجانبي الأيسر المدمج (Adobe Animate Classic Toolbar)
        egui::SidePanel::left("tools_panel").exact_width(64.0).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                let tools = [
                    (ToolType::Select, "↖", "Select (V)"),
                    (ToolType::VectorPen, "✒", "Vector Pen (P)"),
                    (ToolType::Brush, "🖌", "Brush (B)"),
                    (ToolType::Eraser, "⌫", "Eraser (E)"),
                    (ToolType::Bucket, "🪣", "Paint Bucket (K)"),
                    (ToolType::Eyedropper, "💧", "Eyedropper (I)"),
                ];
                for (tool, icon, tip) in tools {
                    let selected = self.active_tool == tool;
                    let btn = egui::Button::new(egui::RichText::new(icon).size(18.0))
                        .min_size(egui::vec2(44.0, 38.0))
                        .fill(if selected { egui::Color32::from_rgb(0, 120, 215) } else { egui::Color32::from_rgb(48, 48, 52) });
                    if ui.add(btn).on_hover_text(tip).clicked() {
                        self.active_tool = tool;
                    }
                }
                ui.separator();
                ui.label("Color");
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
                ui.add_space(4.0);
                if ui.button("+Brush").on_hover_text("Import Custom Brush").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Brush", &["png", "abr"]).pick_file() {
                        self.imported_brushes.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                    }
                }
            });
        });

        // لوحة الخصائص والمكتبة اليمنى (Properties & Library Tabs مثل Adobe Animate)
        egui::SidePanel::right("right_panel").exact_width(235.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.right_tab == 0, "Properties").clicked() {
                    self.right_tab = 0;
                }
                if ui.selectable_label(self.right_tab == 1, "Library & Audio").clicked() {
                    self.right_tab = 1;
                }
            });
            ui.separator();

            if self.right_tab == 0 {
                // تبويب الخصائص (Properties)
                ui.strong("Tool Settings");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Size ([ / ]):");
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
                ui.checkbox(&mut self.pen_pressure_enabled, "Pen Pressure Sensitivity");

                ui.add_space(12.0);
                ui.separator();
                ui.strong("Document / Stage");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(&mut self.project.width).range(320..=3840));
                    ui.label("Height:");
                    ui.add(egui::DragValue::new(&mut self.project.height).range(240..=2160));
                });
                ui.horizontal(|ui| {
                    if ui.small_button("1080p").clicked() {
                        self.project.width = 1920;
                        self.project.height = 1080;
                    }
                    if ui.small_button("720p").clicked() {
                        self.project.width = 1280;
                        self.project.height = 720;
                    }
                    if ui.small_button("Shorts").clicked() {
                        self.project.width = 1080;
                        self.project.height = 1920;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Stage Color:");
                    ui.color_edit_button_srgba_unmultiplied(&mut self.stage_bg_color);
                });
                ui.horizontal(|ui| {
                    ui.label("FPS:");
                    ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=60));
                });

                ui.add_space(12.0);
                ui.separator();
                ui.strong("Shortcuts Guide");
                ui.small("• Space + Drag: Pan Canvas");
                ui.small("• Double Space: Reset View");
                ui.small("• Enter: Play / Pause");
                ui.small("• , / . : Prev / Next Frame");
                ui.small("• F6: Insert Keyframe");
                ui.small("• F8: Convert to Symbol");
                ui.small("• V / P / B / E / K: Tools");
            } else {
                // تبويب المكتبة والصوت (Library & Audio)
                ui.strong("Symbols Library");
                let sym_names: Vec<String> = self.project.library_symbols.keys().cloned().collect();
                if sym_names.is_empty() {
                    ui.small("Select/Draw & press F8 to create symbol");
                }
                for s_name in sym_names {
                    ui.horizontal(|ui| {
                        ui.label(format!("❖ {}", s_name));
                        if ui.small_button("+ Place").clicked() {
                            self.save_undo_state();
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
                ui.strong("Audio Tracks");
                if ui.button("+ Import Audio (MP3/WAV)").clicked() {
                    self.import_audio_dialog();
                }
                let mut remove_audio_idx = None;
                for (idx, track) in self.project.audio_tracks.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("🎵 {}", track.name));
                        if ui.small_button("X").clicked() {
                            remove_audio_idx = Some(idx);
                        }
                    });
                }
                if let Some(idx) = remove_audio_idx {
                    self.project.audio_tracks.remove(idx);
                }
            }
        });

        // التايم لاين السفلي (Adobe Animate Timeline)
        egui::TopBottomPanel::bottom("timeline_panel").exact_height(215.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("|<").on_hover_text("First Frame").clicked() { self.current_frame = 1; }
                if ui.button("<").on_hover_text("Prev Frame (,)").clicked() && self.current_frame > 1 { self.current_frame -= 1; }
                if ui.button(if self.is_playing { "⏸" } else { "▶" }).on_hover_text("Play/Pause (Enter)").clicked() {
                    self.is_playing = !self.is_playing;
                    if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                }
                if ui.button(">").on_hover_text("Next Frame (.)").clicked() && self.current_frame < self.project.total_frames {
                    self.current_frame += 1;
                }
                if ui.button(">|").on_hover_text("Last Frame").clicked() { self.current_frame = self.project.total_frames; }

                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("{}f", self.current_frame));
                ui.label(format!("/ {} ({:.1}s)", self.project.total_frames, self.current_frame as f32 / self.project.fps.max(1) as f32));

                ui.separator();
                ui.checkbox(&mut self.project.onion_skin.enabled, "Onion Skin");
                if self.project.onion_skin.enabled {
                    ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_before).range(0..=10));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_before);
                    ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_after).range(0..=10));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_after);
                }

                ui.separator();
                if ui.button("+ Layer").clicked() {
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
                if ui.button("+ Keyframe (F6)").clicked() {
                    self.insert_keyframe_f6();
                }
                if ui.button("Clear Frame").clicked() {
                    self.save_undo_state();
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        layer.keyframes.remove(&self.current_frame);
                        self.has_unsaved_changes = true;
                    }
                }
            });

            ui.separator();

            egui::ScrollArea::both().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.allocate_ui(egui::vec2(145.0, 20.0), |ui| {
                        ui.strong("Layers");
                    });
                    for f in 1..=self.project.total_frames {
                        let is_curr = self.current_frame == f;
                        let btn = egui::Button::new(format!("{}", f))
                            .min_size(egui::vec2(24.0, 18.0))
                            .fill(if is_curr { egui::Color32::from_rgb(210, 50, 50) } else { egui::Color32::from_rgb(45, 45, 48) });
                        if ui.add(btn).clicked() {
                            self.current_frame = f;
                        }
                    }
                });

                let mut delete_layer_idx = None;
                let layers_len = self.project.layers.len();
                for (l_idx, layer) in self.project.layers.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(145.0, 24.0), |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut layer.visible, "");
                                if ui.selectable_label(self.active_layer_idx == l_idx, &layer.name).clicked() {
                                    self.active_layer_idx = l_idx;
                                }
                                if layers_len > 1 && ui.small_button("X").clicked() {
                                    delete_layer_idx = Some(l_idx);
                                }
                            });
                        });

                        for f in 1..=self.project.total_frames {
                            let has_content = layer.keyframes.get(&f).map_or(false, |fd| {
                                !fd.strokes.is_empty() || !fd.media_items.is_empty() || !fd.symbol_instances.is_empty()
                            });
                            let is_selected = self.active_layer_idx == l_idx && self.current_frame == f;
                            let symbol = if has_content { "●" } else { "○" };
                            let fill = if is_selected {
                                egui::Color32::from_rgb(0, 120, 215)
                            } else if has_content {
                                egui::Color32::from_rgb(75, 75, 82)
                            } else {
                                egui::Color32::from_rgb(34, 34, 37)
                            };
                            let cell = egui::Button::new(symbol)
                                .min_size(egui::vec2(24.0, 22.0))
                                .fill(fill);
                            if ui.add(cell).clicked() {
                                self.active_layer_idx = l_idx;
                                self.current_frame = f;
                            }
                        }
                    });
                }

                if let Some(idx) = delete_layer_idx {
                    self.project.layers.remove(idx);
                    if self.active_layer_idx >= self.project.layers.len() {
                        self.active_layer_idx = self.project.layers.len().saturating_sub(1);
                    }
                }

                for track in &self.project.audio_tracks {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(145.0, 22.0), |ui| {
                            ui.colored_label(egui::Color32::from_rgb(100, 210, 255), format!("🎵 {}", track.name));
                        });
                        for f in 1..=self.project.total_frames {
                            let active = f >= track.start_frame;
                            let fill = if active {
                                egui::Color32::from_rgb(25, 115, 135)
                            } else {
                                egui::Color32::from_rgb(30, 30, 33)
                            };
                            let cell = egui::Button::new(if active { "~" } else { " " })
                                .min_size(egui::vec2(24.0, 20.0))
                                .fill(fill);
                            if ui.add(cell).clicked() {
                                self.current_frame = f;
                            }
                        }
                    });
                }
            });
        });

        // مساحة العمل والكانفا (Stage)
        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(canvas_rect);
            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));

            if scroll_y != 0.0 && is_over_canvas {
                self.zoom_level = (self.zoom_level + scroll_y * 0.001).clamp(0.1, 4.0);
            }

            // تحريك الكانفا عند الضغط المطول على المسطرة (Space + Drag)
            if space_down && pointer_down {
                if let (Some(curr), Some(last)) = (pointer_pos, self.last_pointer_pos) {
                    self.pan_offset += curr - last;
                }
            }

            let center = canvas_rect.center() + self.pan_offset;
            let scale = self.zoom_level.max(0.1);
            let stage_size = egui::vec2(self.project.width as f32, self.project.height as f32) * scale;
            let stage_rect = egui::Rect::from_center_size(center, stage_size);

            // ظل خفيف للمسرح مثل Adobe Animate
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

            if !space_down && is_over_canvas && !self.show_update_dialog {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |last| (pos - last) / scale);

                    match self.active_tool {
                        ToolType::Select => {
                            if pointer_pressed {
                                self.selected_stroke_idx = None;
                                self.selected_symbol_inst_idx = None;
                                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                    if let Some(frame) = layer.keyframes.get(&self.current_frame) {
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
                                if let Some(frame) = layer.keyframes.get(&self.current_frame) {
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
                            let pressure = if self.pen_pressure_enabled {
                                (1.0 - (speed * 0.015)).clamp(0.35, 1.0)
                            } else {
                                1.0
                            };

                            if let Some(stroke) = &mut self.current_stroke {
                                if let Some(last) = stroke.points.last() {
                                    let dist = ((last.x - stage_pos.x).powi(2) + (last.y - stage_pos.y).powi(2)).sqrt();
                                    if dist >= 2.0 {
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
                            if let Some(fd) = layer.keyframes.get(&(self.current_frame - offset)) {
                                for s in &fd.strokes {
                                    draw_stroke_fast(&painter, s, Some(self.project.onion_skin.color_before), scale, 0.0, 0.0, &to_screen);
                                }
                            }
                        }
                    }
                    for offset in 1..=self.project.onion_skin.frames_after {
                        if let Some(fd) = layer.keyframes.get(&(self.current_frame + offset)) {
                            for s in &fd.strokes {
                                draw_stroke_fast(&painter, s, Some(self.project.onion_skin.color_after), scale, 0.0, 0.0, &to_screen);
                            }
                        }
                    }
                }
            }

            let mut media_to_draw = Vec::new();
            for layer in &self.project.layers {
                if layer.visible {
                    if let Some(fd) = layer.keyframes.get(&self.current_frame) {
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
                    if let Some(fd) = layer.keyframes.get(&self.current_frame) {
                        for s in &fd.strokes {
                            draw_stroke_fast(&painter, s, None, scale, 0.0, 0.0, &to_screen);
                        }
                        for inst in &fd.symbol_instances {
                            if let Some(sym) = self.project.library_symbols.get(&inst.symbol_name) {
                                for s in &sym.strokes {
                                    draw_stroke_fast(&painter, s, None, scale * inst.scale, inst.x, inst.y, &to_screen);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(stroke) = &self.current_stroke {
                draw_stroke_fast(&painter, stroke, None, scale, 0.0, 0.0, &to_screen);
            }
        });
    }
}

fn draw_stroke_fast<F: Fn(egui::Pos2) -> egui::Pos2>(
    painter: &egui::Painter,
    stroke: &Stroke,
    override_color: Option<[u8; 4]>,
    scale: f32,
    ox: f32,
    oy: f32,
    to_screen: &F,
) {
    if stroke.points.len() < 2 {
        return;
    }
    let pts: Vec<egui::Pos2> = stroke
        .points
        .iter()
        .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
        .collect();

    if let Some(fc) = stroke.fill_color {
        let fill_c = override_color.unwrap_or(fc);
        painter.add(egui::Shape::convex_polygon(
            pts.clone(),
            egui::Color32::from_rgba_unmultiplied(fill_c[0], fill_c[1], fill_c[2], fill_c[3]),
            egui::Stroke::NONE,
        ));
    }

    let c = override_color.unwrap_or(stroke.color);
    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    painter.add(egui::Shape::line(pts, egui::Stroke::new(stroke.base_width * scale, color)));
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AniHady2D")
            .with_inner_size([1366.0, 768.0]),
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
