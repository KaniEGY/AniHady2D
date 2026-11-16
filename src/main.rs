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

struct AniHadyApp {
    project: AniProject,
    undo_stack: Vec<AniProject>,
    current_frame: usize,
    active_layer_idx: usize,
    active_tool: ToolType,
    brush_color: [u8; 4],
    brush_size: f32,
    pen_pressure_enabled: bool,
    zoom_level: f32,
    pan_offset: egui::Vec2,
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
}

impl AniHadyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        Self {
            project: AniProject::default(),
            undo_stack: Vec::new(),
            current_frame: 1,
            active_layer_idx: 0,
            active_tool: ToolType::Brush,
            brush_color: [0, 0, 0, 255],
            brush_size: 4.0,
            pen_pressure_enabled: true,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
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
            status_message: format!("GPU Ready - AniHady2D v{}", CURRENT_VERSION),
            audio_sink: None,
            last_pointer_pos: None,
        }
    }

    fn check_for_updates(&mut self, ctx: egui::Context) {
        self.show_update_dialog = true;
        {
            let mut st = self.update_state.lock().unwrap();
            *st = UpdateState::Checking;
        }
        let state_clone = Arc::clone(&self.update_state);

        std::thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
            let resp = ureq::get(&api_url)
                .set("User-Agent", "AniHady2D-Updater")
                .call();

            match resp {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<serde_json::Value>() {
                        let tag = json["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
                        let mut exe_url = String::new();

                        if let Some(assets) = json["assets"].as_array() {
                            for asset in assets {
                                if let Some(name) = asset["name"].as_str() {
                                    if name.ends_with(".exe") {
                                        exe_url = asset["browser_download_url"].as_str().unwrap_or("").to_string();
                                        break;
                                    }
                                }
                            }
                        }

                        let mut st = state_clone.lock().unwrap();
                        if !tag.is_empty() && tag != CURRENT_VERSION && !exe_url.is_empty() {
                            *st = UpdateState::UpdateAvailable {
                                version: tag,
                                download_url: exe_url,
                            };
                        } else {
                            *st = UpdateState::UpToDate;
                        }
                    } else {
                        let mut st = state_clone.lock().unwrap();
                        *st = UpdateState::Error("Failed to parse GitHub release data.".to_string());
                    }
                }
                Err(e) => {
                    let mut st = state_clone.lock().unwrap();
                    *st = UpdateState::Error(format!("No releases found or network error: {}", e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn download_and_apply_update(&mut self, download_url: String, ctx: egui::Context) {
        {
            let mut st = self.update_state.lock().unwrap();
            *st = UpdateState::Downloading("Downloading update from GitHub...".to_string());
        }
        let state_clone = Arc::clone(&self.update_state);

        std::thread::spawn(move || {
            let resp = ureq::get(&download_url)
                .set("User-Agent", "AniHady2D-Updater")
                .call();

            match resp {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    if response.into_reader().read_to_end(&mut bytes).is_ok() {
                        if let Ok(current_exe) = std::env::current_exe() {
                            let new_exe = current_exe.with_file_name("AniHady2D_new.exe");
                            if let Ok(mut file) = File::create(&new_exe) {
                                if file.write_all(&bytes).is_ok() {
                                    let mut st = state_clone.lock().unwrap();
                                    *st = UpdateState::ReadyToRestart;
                                    ctx.request_repaint();
                                    return;
                                }
                            }
                        }
                    }
                    let mut st = state_clone.lock().unwrap();
                    *st = UpdateState::Error("Failed to save the downloaded update file.".to_string());
                }
                Err(e) => {
                    let mut st = state_clone.lock().unwrap();
                    *st = UpdateState::Error(format!("Download failed: {}", e));
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
                let _ = std::process::Command::new("cmd")
                    .args(["/C", &bat_path.to_string_lossy()])
                    .spawn();
                std::process::exit(0);
            }
        }
    }

    fn save_undo_state(&mut self) {
        if self.undo_stack.len() > 20 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.project.clone());
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.project = prev;
            self.selected_stroke_idx = None;
            self.selected_symbol_inst_idx = None;
            self.status_message = "Undo done".to_string();
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
                self.status_message = "Project Saved!".to_string();
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
                self.status_message = "Image Imported!".to_string();
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
                        self.status_message = format!("Imported {} video frames!", frames.len());
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
            self.status_message = format!("Audio added: {}", name);
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
        let (ctrl_z, f8_pressed, del_pressed, space_down, scroll_y, close_req, pointer_down, pointer_pressed, pointer_released, pointer_pos) = ctx.input(|i| {
            (
                i.modifiers.ctrl && i.key_pressed(egui::Key::Z),
                i.key_pressed(egui::Key::F8),
                i.key_pressed(egui::Key::Delete),
                i.key_down(egui::Key::Space),
                i.raw_scroll_delta.y,
                i.viewport().close_requested(),
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
            )
        });

        if ctrl_z {
            self.undo();
        }
        if f8_pressed {
            self.show_symbol_dialog = true;
        }
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

        if self.is_playing {
            let now = ctx.input(|i| i.time);
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

        // نافذة التحديث التلقائي (Check for Updates)
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
                            ui.colored_label(egui::Color32::GREEN, "You are using the latest version!");
                            ui.add_space(8.0);
                            if ui.button("Close").clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                        UpdateState::UpdateAvailable { version, download_url } => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, format!("New Update Available: v{}", version));
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("Download & Install Now").clicked() {
                                    self.download_and_apply_update(download_url, ctx.clone());
                                }
                                if ui.button("Later").clicked() {
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
                            ui.colored_label(egui::Color32::GREEN, "Update downloaded! Ready to restart.");
                            ui.add_space(8.0);
                            if ui.button("Restart & Apply Update").clicked() {
                                self.restart_with_new_exe();
                            }
                        }
                        UpdateState::Error(err) => {
                            ui.colored_label(egui::Color32::RED, err);
                            ui.add_space(8.0);
                            if ui.button("Close").clicked() {
                                self.show_update_dialog = false;
