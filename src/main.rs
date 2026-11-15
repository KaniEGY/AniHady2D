mod exporter;
mod project;

use eframe::egui;
use project::{AniProject, AudioTrack, Layer, MediaItem, MediaType, Stroke, StrokePoint, Symbol, SymbolInstance, ToolType};
use rodio::{Decoder, OutputStream, Sink};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn play_sound_effect(file_path: &str) {
    let path = file_path.to_string();
    std::thread::spawn(move || {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                if let Ok(file) = File::open(&path) {
                    if let Ok(source) = Decoder::new(BufReader::new(file)) {
                        sink.append(source);
                        sink.sleep_until_end();
                    }
                }
            }
        }
    });
}

struct AniHadyApp {
    project: AniProject, undo_stack: Vec<AniProject>, current_frame: usize, active_layer_idx: usize,
    active_tool: ToolType, brush_color: [u8; 4], brush_size: f32, pen_pressure_enabled: bool,
    zoom_level: f32, pan_offset: egui::Vec2, is_playing: bool, last_frame_time: f64,
    has_unsaved_changes: bool, show_exit_dialog: bool, show_symbol_dialog: bool, new_symbol_name: String,
    allow_close: bool, current_stroke: Option<Stroke>, selected_stroke_idx: Option<usize>,
    selected_symbol_inst_idx: Option<usize>, texture_cache: HashMap<String, egui::TextureHandle>,
    imported_brushes: Vec<String>, saved_path: Option<PathBuf>, status_message: String,
    audio_sink: Option<(OutputStream, Sink)>,
}

impl Default for AniHadyApp {
    fn default() -> Self {
        play_sound_effect("assets/startup.wav");
        Self {
            project: AniProject::default(), undo_stack: Vec::new(), current_frame: 1, active_layer_idx: 0,
            active_tool: ToolType::VectorPen, brush_color: [0, 0, 0, 255], brush_size: 4.0,
            pen_pressure_enabled: true, zoom_level: 1.0, pan_offset: egui::Vec2::ZERO, is_playing: false,
            last_frame_time: 0.0, has_unsaved_changes: false, show_exit_dialog: false, show_symbol_dialog: false,
            new_symbol_name: "Symbol_1".to_string(), allow_close: false, current_stroke: None,
            selected_stroke_idx: None, selected_symbol_inst_idx: None, texture_cache: HashMap::new(),
            imported_brushes: Vec::new(), saved_path: None, status_message: "جاهز للعمل - AniHady2D".to_string(),
            audio_sink: None,
        }
    }
}

impl AniHadyApp {
    fn save_undo_state(&mut self) {
        if self.undo_stack.len() > 30 { self.undo_stack.remove(0); }
        self.undo_stack.push(self.project.clone());
    }
    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.project = prev;
            self.selected_stroke_idx = None;
            self.selected_symbol_inst_idx = None;
            self.status_message = "تم التراجع (Undo)".to_string();
        }
    }
    fn trigger_save(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new().add_filter("AniHady2D Project", &["ah2d"]).set_file_name(&format!("{}.ah2d", self.project.name)).save_file() {
            if self.project.save_to_file(&path).is_ok() {
                self.saved_path = Some(path);
                self.has_unsaved_changes = false;
                self.status_message = "تم حفظ المشروع!".to_string();
                return true;
            }
        }
        false
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
    fn stop_audio_playback(&mut self) { if let Some((_, sink)) = self.audio_sink.take() { sink.stop(); } }
    fn load_texture_if_needed(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.texture_cache.get(path) { return Some(tex.clone()); }
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let color_image = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
            let handle = ctx.load_texture(path, color_image, egui::TextureOptions::LINEAR);
            self.texture_cache.insert(path.to_string(), handle.clone());
            return Some(handle);
        }
        None
    }
}

impl eframe::App for AniHadyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) { self.undo(); }
        if ctx.input(|i| i.key_pressed(egui::Key::F8)) { self.show_symbol_dialog = true; }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            if let Some(idx) = self.selected_stroke_idx.take() {
                self.save_undo_state();
                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                    if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                        if idx < frame.strokes.len() { frame.strokes.remove(idx); self.has_unsaved_changes = true; }
                    }
                }
            }
        }
        if self.is_playing {
            let now = ctx.input(|i| i.time);
            if now - self.last_frame_time >= 1.0 / (self.project.fps as f64) {
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
        if self.show_symbol_dialog {
            egui::Window::new("تحويل إلى Symbol (F8)").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label("اسم الـ Symbol:");
                ui.text_edit_singleline(&mut self.new_symbol_name);
                ui.horizontal(|ui| {
                    if ui.button("حفظ في المكتبة").clicked() {
                        self.save_undo_state();
                        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                if !frame.strokes.is_empty() {
                                    let sym = Symbol { name: self.new_symbol_name.clone(), strokes: frame.strokes.clone() };
                                    self.project.library_symbols.insert(self.new_symbol_name.clone(), sym);
                                    frame.strokes.clear();
                                    frame.symbol_instances.push(SymbolInstance { symbol_name: self.new_symbol_name.clone(), x: 0.0, y: 0.0, scale: 1.0 });
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        self.show_symbol_dialog = false;
                    }
                    if ui.button("إلغاء").clicked() { self.show_symbol_dialog = false; }
                });
            });
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.has_unsaved_changes {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_exit_dialog = true;
            } else {
                play_sound_effect("assets/shutdown.wav");
                std::thread::sleep(std::time::Duration::from_millis(350));
            }
        }
        if self.show_exit_dialog {
            egui::Window::new("حفظ المشروع؟").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label("تحب تعمل حفظ للبروجكت قبل ما تقفل البرنامج؟");
                ui.horizontal(|ui| {
                    if ui.button("نعم (حفظ)").clicked() && self.trigger_save() {
                        play_sound_effect("assets/shutdown.wav");
                        std::thread::sleep(std::time::Duration::from_millis(350));
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("لا (خروج على طول)").clicked() {
                        play_sound_effect("assets/shutdown.wav");
                        std::thread::sleep(std::time::Duration::from_millis(350));
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("ملف (File)", |ui| {
                    if ui.button("حفظ المشروع (.ah2d)").clicked() { self.trigger_save(); ui.close_menu(); }
                    if ui.button("فتح مشروع (.ah2d)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("AniHady2D Project", &["ah2d"]).pick_file() {
                            if let Ok(proj) = AniProject::load_from_file(&path) { self.project = proj; self.has_unsaved_changes = false; }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("استيراد صورة").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]).pick_file() {
                            self.save_undo_state();
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                let frame = layer.keyframes.entry(self.current_frame).or_default();
                                frame.media_items.push(MediaItem { path: path.to_string_lossy().to_string(), media_type: MediaType::Image, x: 100.0, y: 50.0, width: 300.0, height: 200.0 });
                                self.has_unsaved_changes = true;
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("استيراد فيديو (MP4/MOV/AVI)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Videos", &["mp4", "mov", "avi", "mkv"]).pick_file() {
                            if let Ok(frames) = exporter::extract_video_frames(&path, self.project.fps) {
                                self.save_undo_state();
                                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                    for (i, f_path) in frames.iter().enumerate() {
                                        let f_idx = self.current_frame + i;
                                        if f_idx <= self.project.total_frames {
                                            let frame = layer.keyframes.entry(f_idx).or_default();
                                            frame.media_items.push(MediaItem { path: f_path.to_string_lossy().to_string(), media_type: MediaType::VideoFrame, x: 0.0, y: 0.0, width: 800.0, height: 450.0 });
                                        }
                                    }
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("استيراد صوت (MP3/WAV)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Audio", &["mp3", "wav"]).pick_file() {
                            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                            self.project.audio_tracks.push(AudioTrack { name, file_path: path.to_string_lossy().to_string(), start_frame: self.current_frame });
                            self.has_unsaved_changes = true;
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("تصدير الفيديو (MP4)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("MP4 Video", &["mp4"]).set_file_name("AniHady2D_Output.mp4").save_file() {
                            match exporter::export_to_mp4(&self.project, &path) {
                                Ok(_) => self.status_message = "تم تصدير MP4 بنجاح!".to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.button("↩ تراجع").clicked() { self.undo(); }
                if ui.button("✨ Symbol (F8)").clicked() { self.show_symbol_dialog = true; }
                ui.separator();
                ui.label(format!("الزوم: {:.0}%", self.zoom_level * 100.0));
                if ui.button("Zoom Default (100%)").clicked() { self.zoom_level = 1.0; self.pan_offset = egui::Vec2::ZERO; }
                ui.separator();
                ui.label(&self.status_message);
            });
        });

        egui::SidePanel::left("tools_panel").exact_width(150.0).show(ctx, |ui| {
            ui.heading("الأدوات");
            ui.separator();
            ui.selectable_value(&mut self.active_tool, ToolType::VectorPen, "✒ قلم فيكتور");
            ui.selectable_value(&mut self.active_tool, ToolType::Brush, "🖌 فرشاة عادية");
            ui.selectable_value(&mut self.active_tool, ToolType::Eraser, "🧽 ممحاة");
            ui.selectable_value(&mut self.active_tool, ToolType::Bucket, "🪣 دلو الألوان");
            ui.selectable_value(&mut self.active_tool, ToolType::Select, "↖ تحديد وتحريك");
            ui.selectable_value(&mut self.active_tool, ToolType::Eyedropper, "💧 نسخ اللون");
            ui.separator();
            if ui.button("+ استيراد فرشاة").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Brush", &["png", "abr"]).pick_file() {
                    self.imported_brushes.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                }
            }
            for b in &self.imported_brushes { ui.label(format!("• {}", b)); }
            ui.separator();
            ui.label("حجم الأداة:");
            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0));
            ui.label("اللون:");
            ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            ui.separator();
            ui.checkbox(&mut self.pen_pressure_enabled, "حساسية القلم");
        });

        egui::SidePanel::right("right_panel").exact_width(200.0).show(ctx, |ui| {
            ui.heading("الطبقات (Layers)");
            if ui.button("+ طبقة جديدة").clicked() {
                self.save_undo_state();
                let count = self.project.layers.len() + 1;
                let mut kf = HashMap::new();
                kf.insert(1, Default::default());
                self.project.layers.push(Layer { name: format!("Layer {}", count), visible: true, locked: false, keyframes: kf });
            }
            ui.separator();
            for (idx, layer) in self.project.layers.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layer.visible, "");
                    if ui.selectable_label(self.active_layer_idx == idx, &layer.name).clicked() { self.active_layer_idx = idx; }
                });
            }
            ui.add_space(15.0);
            ui.separator();
            ui.heading("مكتبة Symbols");
            let sym_names: Vec<String> = self.project.library_symbols.keys().cloned().collect();
            for s_name in sym_names {
                ui.horizontal(|ui| {
                    ui.label(&s_name);
                    if ui.button("+ إدراج").clicked() {
                        self.save_undo_state();
                        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                            let frame = layer.keyframes.entry(self.current_frame).or_default();
                            frame.symbol_instances.push(SymbolInstance { symbol_name: s_name.clone(), x: 50.0, y: 50.0, scale: 1.0 });
                            self.has_unsaved_changes = true;
                        }
                    }
                });
            }
        });

        egui::TopBottomPanel::bottom("timeline_panel").exact_height(180.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.is_playing { "⏸ إيقاف" } else { "▶ تشغيل" }).clicked() {
                    self.is_playing = !self.is_playing;
                    if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                }
                ui.label(format!("الفريم: {} / {}", self.current_frame, self.project.total_frames));
                ui.separator();
                ui.checkbox(&mut self.project.onion_skin.enabled, "Onion Skin");
                if self.project.onion_skin.enabled {
                    ui.label("قبل:");
                    ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_before).range(0..=10));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_before);
                    ui.label("بعد:");
                    ui.add(egui::DragValue::new(&mut self.project.onion_skin.frames_after).range(0..=10));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_after);
                }
            });
            ui.separator();
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for f in 1..=self.project.total_frames {
                        if ui.selectable_label(self.current_frame == f, format!("{}", f)).clicked() { self.current_frame = f; }
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 && response.hovered() {
                self.zoom_level = (self.zoom_level + scroll_delta * 0.001).clamp(-2.0, 3.0);
            }
            if ctx.input(|i| i.key_down(egui::Key::Space)) && response.dragged() {
                self.pan_offset += response.drag_delta();
            }
            let center = response.rect.center() + self.pan_offset;
            let scale = self.zoom_level.abs().max(0.1);
            let stage_size = egui::vec2(self.project.width as f32, self.project.height as f32) * scale;
            let stage_rect = egui::Rect::from_center_size(center, stage_size);
            painter.rect_filled(stage_rect, 0.0, egui::Color32::WHITE);

            let to_stage = |p: egui::Pos2| egui::pos2((p.x - stage_rect.min.x) / scale, (p.y - stage_rect.min.y) / scale);
            let to_screen = |p: egui::Pos2| egui::pos2(stage_rect.min.x + p.x * scale, stage_rect.min.y + p.y * scale);

            if !ctx.input(|i| i.key_down(egui::Key::Space)) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let stage_pos = to_stage(pos);
                    match self.active_tool {
                        ToolType::Select => {
                            if response.clicked() {
                                self.selected_stroke_idx = None;
                                self.selected_symbol_inst_idx = None;
                                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                    if let Some(frame) = layer.keyframes.get(&self.current_frame) {
                                        for (i, inst) in frame.symbol_instances.iter().enumerate() {
                                            if ((inst.x - stage_pos.x).powi(2) + (inst.y - stage_pos.y).powi(2)).sqrt() < 60.0 { self.selected_symbol_inst_idx = Some(i); break; }
                                        }
                                        if self.selected_symbol_inst_idx.is_none() {
                                            for (i, s) in frame.strokes.iter().enumerate() {
                                                if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 20.0) { self.selected_stroke_idx = Some(i); break; }
                                            }
                                        }
                                    }
                                }
                            } else if response.dragged() {
                                let delta = response.drag_delta() / scale;
                                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                    if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                        if let Some(idx) = self.selected_symbol_inst_idx {
                                            if let Some(inst) = frame.symbol_instances.get_mut(idx) { inst.x += delta.x; inst.y += delta.y; self.has_unsaved_changes = true; }
                                        } else if let Some(idx) = self.selected_stroke_idx {
                                            if let Some(stroke) = frame.strokes.get_mut(idx) { for p in &mut stroke.points { p.x += delta.x; p.y += delta.y; } self.has_unsaved_changes = true; }
                                        }
                                    }
                                }
                            }
                        }
                        ToolType::Bucket if response.clicked() => {
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    for s in frame.strokes.iter_mut().rev() {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 35.0) {
                                            s.fill_color = Some(self.brush_color); self.has_unsaved_changes = true; break;
                                        }
                                    }
                                }
                            }
                        }
                        ToolType::Eraser if response.dragged() => {
                            if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                                    let r = self.brush_size;
                                    frame.strokes.retain(|s| !s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < r));
                                    self.has_unsaved_changes = true;
                                }
                            }
                        }
                        ToolType::Eyedropper if response.clicked() => {
                            if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                if let Some(frame) = layer.keyframes.get(&self.current_frame) {
                                    for s in &frame.strokes {
                                        if s.points.iter().any(|p| ((p.x - stage_pos.x).powi(2) + (p.y - stage_pos.y).powi(2)).sqrt() < 15.0) { self.brush_color = s.color; break; }
                                    }
                                }
                            }
                        }
                        ToolType::VectorPen | ToolType::Brush if response.dragged() => {
                            let pressure = if self.pen_pressure_enabled { ctx.input(|i| i.pointer.pressure().unwrap_or(1.0)) } else { 1.0 };
                            let point = StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure };
                            if let Some(stroke) = &mut self.current_stroke { stroke.points.push(point); }
                            else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke { points: vec![point], color: self.brush_color, fill_color: None, base_width: self.brush_size, is_vector: self.active_tool == ToolType::VectorPen, brush_texture_id: None });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if response.drag_stopped() {
                if let Some(mut stroke) = self.current_stroke.take() {
                    if stroke.is_vector { stroke.smooth_vector(); }
                    if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                        layer.keyframes.entry(self.current_frame).or_default().strokes.push(stroke);
                        self.has_unsaved_changes = true;
                    }
                }
            }

            if self.project.onion_skin.enabled && !self.is_playing {
                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                    for offset in 1..=self.project.onion_skin.frames_before {
                        if self.current_frame > offset {
                            if let Some(fd) = layer.keyframes.get(&(self.current_frame - offset)) {
                                for s in &fd.strokes { draw_stroke_scaled(&painter, s, Some(self.project.onion_skin.color_before), scale, 0.0, 0.0, &to_screen); }
                            }
                        }
                    }
                    for offset in 1..=self.project.onion_skin.frames_after {
                        if let Some(fd) = layer.keyframes.get(&(self.current_frame + offset)) {
                            for s in &fd.strokes { draw_stroke_scaled(&painter, s, Some(self.project.onion_skin.color_after), scale, 0.0, 0.0, &to_screen); }
                        }
                    }
                }
            }

            let mut media_to_draw = Vec::new();
            for layer in &self.project.layers {
                if layer.visible {
                    if let Some(fd) = layer.keyframes.get(&self.current_frame) { media_to_draw.extend(fd.media_items.clone()); }
                }
            }
            for m in media_to_draw {
                if let Some(tex) = self.load_texture_if_needed(ctx, &m.path) {
                    painter.image(tex.id(), egui::Rect::from_min_max(to_screen(egui::pos2(m.x, m.y)), to_screen(egui::pos2(m.x + m.width, m.y + m.height))), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
            }

            for layer in &self.project.layers {
                if layer.visible {
                    if let Some(fd) = layer.keyframes.get(&self.current_frame) {
                        for s in &fd.strokes { draw_stroke_scaled(&painter, s, None, scale, 0.0, 0.0, &to_screen); }
                        for inst in &fd.symbol_instances {
                            if let Some(sym) = self.project.library_symbols.get(&inst.symbol_name) {
                                for s in &sym.strokes { draw_stroke_scaled(&painter, s, None, scale * inst.scale, inst.x, inst.y, &to_screen); }
                            }
                        }
                    }
                }
            }
            if let Some(stroke) = &self.current_stroke { draw_stroke_scaled(&painter, stroke, None, scale, 0.0, 0.0, &to_screen); }
        });
    }
}

fn draw_stroke_scaled<F: Fn(egui::Pos2) -> egui::Pos2>(painter: &egui::Painter, stroke: &Stroke, override_color: Option<[u8; 4]>, scale: f32, ox: f32, oy: f32, to_screen: &F) {
    if stroke.points.len() < 2 { return; }
    if let Some(fc) = stroke.fill_color {
        let fill_c = override_color.unwrap_or(fc);
        let pts: Vec<egui::Pos2> = stroke.points.iter().map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy))).collect();
        painter.add(egui::Shape::convex_polygon(pts, egui::Color32::from_rgba_unmultiplied(fill_c[0], fill_c[1], fill_c[2], fill_c[3]), egui::Stroke::NONE));
    }
    let c = override_color.unwrap_or(stroke.color);
    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    for w in stroke.points.windows(2) {
        painter.line_segment([to_screen(egui::pos2(w[0].x + ox, w[0].y + oy)), to_screen(egui::pos2(w[1].x + ox, w[1].y + oy))], egui::Stroke::new(stroke.base_width * w[1].pressure * scale, color));
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native("AniHady2D", eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_title("AniHady2D").with_inner_size([1280.0, 720.0]), ..Default::default() }, Box::new(|_cc| Ok(Box::new(AniHadyApp::default()))))
}
