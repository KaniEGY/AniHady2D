#![windows_subsystem = "windows"]

mod brushes;
mod brush_engine;
mod stroke_geometry;
mod sculpt_tools;
mod stylus;
mod updater;
mod changelog;
mod exporter;
mod persistence;
mod onion_skin;
mod fill_tools;
mod fill_render;
mod eraser_tools;
mod selection_tools;
mod layer_ops;
mod timeline_ops;
mod timeline_styles;
mod profile;
mod project;
mod collab;
mod cloud_sync;
mod project_sync;
mod remote_access;
mod transform_handles;

use arabic_reshaper::arabic_reshape;
use brushes::{draw_brush_preset_icon, draw_stroke_pro};
use stylus::TouchPressureTracker;
use eframe::egui;
use exporter::{ImageExportFormat, VideoExportFormat};
use profile::UserProfile;
use collab::{OnlineMode, OnlineEvent, OnlineSession, RoomListing, Participant};
use project::{
    AniProject, AudioSyncMode, AudioTrack, EasingCurve, FrameData, KritaBrushPreset, Layer, MediaItem, MediaType,
    BrushTipShape, Stroke, StrokePoint, Symbol, SymbolInstance, TextItem, TweenMode, PROJECT_SCHEMA_VERSION,
};
// Game UI sounds are generated as samples without shipping external audio assets.
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
const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@%D8%A7%D8%A8%D9%84%D9%83%D8%A7%D8%B4-%D8%A7%D9%86%D9%85%D9%8A%D8%B4%D9%86";
const INSTAGRAM_URL: &str = "https://www.instagram.com/ablikash?exln=NmFtNWZidnBvb3k1";
const SUPPORT_URL: &str = "https://ipn.eg/S/imhady/instapay/0tZZ7c";

#[derive(Clone, Copy, PartialEq)]
enum WorkspaceMode {
    InstallerSetup,
    Launcher,
    DrawingOnly,
    AnimationStudio,
    OnlineLobby,
}

#[derive(Clone, Copy, PartialEq)]
enum OnlinePage {
    ChooseMode,
    ChooseAction,
    CreateRoom,
    JoinRoom,
}

#[derive(Clone, Copy, PartialEq)]
enum ActiveToolUI {
    SelectNormal,
    SelectLasso,
    FreeTransform,
    VectorDirectSelect,
    Sculpt,
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
    Triangle,
    Pentagon,
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
    LevelUpFanfare(u8),
    ToolSelect(u8),
}

// Original synthesised game-style UI sounds. No external assets or audio downloads.
fn game_ui_samples(sfx: SfxType) -> Vec<f32> {
    let duration = match sfx {
        SfxType::UiClick => 0.062,
        SfxType::NewFrame => 0.095,
        SfxType::VectorPenTick => 0.042,
        SfxType::BrushScratch => 0.044,
        SfxType::EraserRub => 0.062,
        SfxType::LassoSelect => 0.115,
        SfxType::ZoomStep => 0.040,
        SfxType::ExportSuccess => 0.340,
        SfxType::ExitFarewell => 0.190,
        SfxType::LevelUpFanfare(_) => 0.520,
        SfxType::ToolSelect(_) => 0.090,
    };
    let rate = 44_100.0f32;
    let count = (duration * rate) as usize;
    let mut output = Vec::with_capacity(count);
    let mut seed = 0x9e37_79b9u32;
    for i in 0..count {
        let t = i as f32 / rate;
        let progress = (t / duration).clamp(0.0,1.0);
        // Pitch slides make more expressive 'pop', 'blip' and 'reward' sounds
        // than the old flat sine-wave beeps.
        let (f0,f1,loud,noise) = match sfx {
            SfxType::UiClick => (770.,520.,0.14,0.06),
            SfxType::NewFrame => (510.,970.,0.18,0.03),
            SfxType::VectorPenTick => (1160.,740.,0.085,0.045),
            SfxType::BrushScratch => (580.,310.,0.06,0.16),
            SfxType::EraserRub => (360.,190.,0.075,0.20),
            SfxType::LassoSelect => (580.,1120.,0.14,0.07),
            SfxType::ZoomStep => (680.,815.,0.09,0.02),
            SfxType::ExportSuccess => (560.,950.,0.17,0.025),
            SfxType::ExitFarewell => (530.,230.,0.11,0.03),
            SfxType::LevelUpFanfare(variation) => {
                let band = 420. + (variation/10) as f32*24.;
                (band,band*1.8,0.18,0.03)
            }
            SfxType::ToolSelect(index) => {
                let base = 535. + (index as f32)*26.;
                (base,base*1.21,0.095,0.05)
            }
        };
        let f = f0+(f1-f0)*progress;
        // Chirp phase integral, with a short upper-octave sparkle.
        let phase = std::f32::consts::TAU*(f0*t + (f1-f0)*t*t/(2.0*duration));
        let mut tone = phase.sin()*0.73 + (2.02*phase).sin()*0.20;
        if matches!(sfx, SfxType::ExportSuccess | SfxType::LevelUpFanfare(_)) {
            let note = (progress*4.0).floor() as usize;
            let arpeggio = [1.0,1.25,1.5,2.0][note.min(3)];
            tone = (phase*arpeggio).sin()*0.8 + (phase*arpeggio*2.0).sin()*0.18;
        }
        seed ^= seed<<13;
        seed ^= seed>>17;
        seed ^= seed<<5;
        let grit = ((seed&65535) as f32/32767.5-1.0)*noise;
        let attack = (t/0.007).min(1.0);
        let release = (1.0-progress).powf(2.4);
        output.push(((tone+grit)*loud*attack*release).clamp(-0.38,0.38));
    }
    output
}

fn play_sfx_async(enabled: bool, sfx: SfxType, wait_finish: bool) {
    if !enabled { return; }
    let handle = std::thread::spawn(move || {
        if let Ok((_stream,stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                sink.append(rodio::buffer::SamplesBuffer::new(1, 44_100, game_ui_samples(sfx)));
                sink.sleep_until_end();
            }
        }
    });
    if wait_finish { let _ = handle.join(); }
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
    // Shape each line independently: a multiline TextItem must not lose
    // right-to-left ordering after the first paragraph.
    text.split('\n').map(|line| {
        let reshaped = arabic_reshape(line);
        let bidi_info = BidiInfo::new(&reshaped, None);
        if let Some(para) = bidi_info.paragraphs.first() {
            bidi_info.reorder_line(para, para.range.clone()).into_owned()
        } else {
            reshaped
        }
    }).collect::<Vec<_>>().join("\n")
}

fn tr(lang: AppLang, en: &str, ar: &str) -> String {
    match lang {
        AppLang::English => en.to_string(),
        AppLang::Arabic => fix_ar(ar),
    }
}

// Arabic text is manually split into short, independent lines before shaping.
// fix_ar() reorders each *entire* logical line, so letting egui wrap one
// long pre-shaped paragraph would visually scramble the sentence order.
const SUPPORT_MESSAGE_LINES: [(&str, &str); 7] = [
    ("Yes, I'm broke, poor, and barely have enough to eat...",
     "أنا صحيح كحيان وفقير وجربوع ومش لاقي آكل،"),
    ("But you absolutely don't have to send me money!",
     "ولكن إنت مش لازم تبعت فلوس خالص!"),
    ("As I said, I'll keep working on the program",
     "زي ما بقولك، تطوير البرنامج هيفضل شغال"),
    ("from time to time as long as people enjoy it.",
     "من فترة للتانية طول ما الناس عاجبها البرنامج."),
    ("This support section is entirely optional.",
     "وجزء الدعم ده اختياري تمامًا،"),
    ("It's only for anyone who likes the program",
     "وللي عجبه البرنامج وشايف إني أستحق مكافأة،"),
    ("and thinks I deserve a little reward. That's all!",
     "ممكن يدعمني لو حابب. وبس!"),
];

fn support_message_lines(lang: AppLang) -> Vec<String> {
    SUPPORT_MESSAGE_LINES.iter().map(|(en, ar)| tr(lang, en, ar)).collect()
}

// Notification strings are shaped once in the notice window itself.
// Using tr() here would reshape Arabic twice and scramble its word order.
fn tr_notice(lang: AppLang, en: &str, ar: &str) -> String {
    match lang {
        AppLang::English => en.to_string(),
        AppLang::Arabic => ar.to_string(),
    }
}


const TOOLBAR_ITEMS: [ActiveToolUI; 14] = [
    ActiveToolUI::SelectNormal,
    ActiveToolUI::SelectLasso,
    ActiveToolUI::FreeTransform,
    ActiveToolUI::VectorDirectSelect,
    ActiveToolUI::Sculpt,
    ActiveToolUI::VectorPen,
    ActiveToolUI::Brush,
    ActiveToolUI::EraserRed,
    ActiveToolUI::EraserGreen,
    ActiveToolUI::Bucket,
    ActiveToolUI::Eyedropper,
    ActiveToolUI::Text,
    ActiveToolUI::ShapeLine,
    ActiveToolUI::HandCanvas,
];

fn tool_display_name(tool: ActiveToolUI, lang: AppLang) -> String {
    let (en, ar) = match tool {
        ActiveToolUI::SelectNormal => ("Selection", "تحديد"),
        ActiveToolUI::SelectLasso => ("Lasso", "تحديد حر"),
        ActiveToolUI::FreeTransform => ("Transform", "تحويل"),
        ActiveToolUI::VectorDirectSelect => ("Vector Nodes", "نقاط الفيكتور"),
        ActiveToolUI::Sculpt => ("Sculpt / Finger", "نحت / صباع"),
        ActiveToolUI::VectorPen => ("Ink Pen", "قلم حبر"),
        ActiveToolUI::Brush => ("Brush", "فرشاة"),
        ActiveToolUI::EraserRed => ("Eraser", "ممحاة"),
        ActiveToolUI::EraserGreen => ("Partial Eraser", "ممحاة جزئية"),
        ActiveToolUI::Bucket => ("Fill", "تعبئة"),
        ActiveToolUI::Eyedropper => ("Eyedropper", "قطارة الألوان"),
        ActiveToolUI::Text => ("Text", "نص"),
        ActiveToolUI::ShapeLine => ("Shapes", "أشكال"),
        ActiveToolUI::HandCanvas => ("Pan Canvas", "تحريك اللوحة"),
    };
    tr(lang, en, ar)
}

fn tool_shortcut<'a>(tool: ActiveToolUI, config: &'a AppConfig) -> &'a str {
    match tool {
        ActiveToolUI::SelectNormal => &config.tool_select,
        ActiveToolUI::SelectLasso => &config.tool_lasso,
        ActiveToolUI::FreeTransform => &config.tool_free_transform,
        ActiveToolUI::VectorDirectSelect => &config.tool_vector_select,
        ActiveToolUI::Sculpt => &config.tool_sculpt,
        ActiveToolUI::VectorPen => &config.tool_pen,
        ActiveToolUI::Brush => &config.tool_brush,
        ActiveToolUI::EraserRed => &config.tool_eraser,
        ActiveToolUI::EraserGreen => &config.tool_eraser_partial,
        ActiveToolUI::Bucket => &config.tool_bucket,
        ActiveToolUI::Eyedropper => &config.tool_eyedropper,
        ActiveToolUI::Text => &config.tool_text,
        ActiveToolUI::ShapeLine => &config.tool_shape,
        ActiveToolUI::HandCanvas => &config.tool_hand,
    }
}

const BASIC_COLORS: [([u8; 3], &str); 12] = [
    ([0, 0, 0], "Black"),
    ([255, 255, 255], "White"),
    ([239, 68, 68], "Red"),
    ([249, 115, 22], "Orange"),
    ([234, 179, 8], "Yellow"),
    ([34, 197, 94], "Green"),
    ([20, 184, 166], "Teal"),
    ([56, 189, 248], "Sky"),
    ([37, 99, 235], "Blue"),
    ([139, 92, 246], "Violet"),
    ([236, 72, 153], "Pink"),
    ([120, 113, 108], "Gray"),
];

fn get_installed_version() -> String {
    // The GitHub Release is a standalone Stick.exe download. A sidecar
    // version.txt may be missing or may belong to a different EXE.
    // The version baked into the executable is the only reliable authority.
    updater::embedded_build_version().to_string()
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
    UpdateAvailable(updater::ReleaseInfo),
    Downloading { received: u64, total: u64 },
    ReadyToRestart { version: String, staged_exe: PathBuf },
    Error(String),
}

#[derive(Clone)]
enum RollbackState {
    Idle,
    Checking,
    Available(Vec<updater::ArchivedRelease>),
    Error(String),
}

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    lang: AppLang,
    #[serde(default)]
    last_seen_patch_version: String,
    #[serde(default)]
    recent_projects: Vec<String>,
    #[serde(default = "default_true_val")]
    sound_effects_enabled: bool,
    #[serde(default)]
    show_grid: bool,
    #[serde(default)]
    perspective_mode: u8,
    #[serde(default = "default_perspective_spacing")]
    perspective_spacing: f32,
    #[serde(default = "default_perspective_width")]
    perspective_line_width: f32,
    #[serde(default = "default_perspective_color")]
    perspective_color: [u8; 4],
    #[serde(default = "default_perspective_opacity")]
    perspective_opacity: f32,
    #[serde(default = "default_vanishing_x")]
    vanishing_x: f32,
    #[serde(default = "default_vanishing_y")]
    vanishing_y: f32,
    #[serde(default = "default_left_vp_x")]
    vanishing_left_x: f32,
    #[serde(default = "default_right_vp_x")]
    vanishing_right_x: f32,
    #[serde(default = "default_two_point_y")]
    vanishing_2point_y: f32,
    #[serde(default = "default_timeline_height")]
    timeline_dock_height: f32,
    #[serde(default)]
    timeline_style: u8,
    #[serde(default = "default_new_timeline_cell_width")]
    timeline_new_cell_width: f32,
    #[serde(default = "default_new_timeline_row_height")]
    timeline_new_row_height: f32,
    #[serde(default = "default_true_val")]
    timeline_show_timecodes: bool,
    #[serde(default = "default_true_val")]
    show_top_bar: bool,
    #[serde(default = "default_true_val")]
    show_left_dock: bool,
    #[serde(default = "default_true_val")]
    show_right_dock: bool,
    #[serde(default = "default_right_split")]
    right_panel_split: f32,
    #[serde(default = "default_true_val")]
    show_timeline_dock: bool,
    #[serde(default = "default_grid_size")]
    grid_size: f32,
    #[serde(default = "default_tl_bg")]
    timeline_bg_color: [u8; 4],
    #[serde(default = "default_tl_cell")]
    timeline_cell_color: [u8; 4],
    #[serde(default = "default_tl_head")]
    timeline_playhead_color: [u8; 4],
    #[serde(default = "default_ui_panel_color")]
    ui_panel_color: [u8; 4],
    #[serde(default = "default_ui_accent_color")]
    ui_accent_color: [u8; 4],
    #[serde(default = "default_autosave_mins")]
    autosave_interval_minutes: u64,
    #[serde(default = "default_cursor_scale")]
    brush_cursor_scale: f32,
    #[serde(default = "default_cursor_scale")]
    tool_icon_scale: f32,
    #[serde(default)]
    icon_cursor_ring: bool,
    #[serde(default = "default_google_client_id")]
    cloud_google_client_id: String,
    #[serde(default)]
    cloud_email: String,
    #[serde(default)]
    cloud_auto_backup: bool,
    tool_select: String,
    #[serde(default = "default_lasso_key")]
    tool_lasso: String,
    #[serde(default = "default_free_tr_key")]
    tool_free_transform: String,
    #[serde(default = "default_vec_sel_key")]
    tool_vector_select: String,
    tool_pen: String,
    #[serde(default = "default_sculpt_key")]
    tool_sculpt: String,
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
fn default_perspective_spacing() -> f32 { 40.0 }
fn default_perspective_width() -> f32 { 1.0 }
fn default_perspective_color() -> [u8;4] { [58, 154, 227, 255] }
fn default_perspective_opacity() -> f32 { 0.32 }
fn default_vanishing_x() -> f32 { 0.5 }
fn default_vanishing_y() -> f32 { 0.30 }
fn default_left_vp_x() -> f32 { 0.14 }
fn default_right_vp_x() -> f32 { 0.86 }
fn default_two_point_y() -> f32 { 0.36 }
fn default_timeline_height() -> f32 { 310.0 }
fn default_new_timeline_row_height() -> f32 { 50.0 }
fn default_new_timeline_cell_width() -> f32 { 64.0 }
fn default_right_split() -> f32 { 0.45 }
fn default_grid_size() -> f32 { 40.0 }
fn default_autosave_mins() -> u64 { 5 }
fn default_cursor_scale() -> f32 { 1.0 }
fn default_google_client_id() -> String {
    option_env!("STICK_GOOGLE_CLIENT_ID").unwrap_or("").to_string()
}
fn default_tl_bg() -> [u8; 4] { [28, 29, 34, 255] }
fn default_tl_cell() -> [u8; 4] { [40, 42, 48, 255] }
fn default_tl_head() -> [u8; 4] { [225, 55, 55, 255] }
fn default_ui_panel_color() -> [u8; 4] { [30, 31, 36, 255] }
fn default_ui_accent_color() -> [u8; 4] { [0, 122, 204, 255] }
fn default_lasso_key() -> String { "L".to_string() }
fn default_free_tr_key() -> String { "Q".to_string() }
fn default_vec_sel_key() -> String { "G".to_string() }
fn default_sculpt_key() -> String { "J".to_string() }
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
            recent_projects: Vec::new(),
            sound_effects_enabled: true,
            show_grid: false,
            perspective_mode: 0,
            perspective_spacing: default_perspective_spacing(),
            perspective_line_width: default_perspective_width(),
            perspective_color: default_perspective_color(),
            perspective_opacity: default_perspective_opacity(),
            vanishing_x: default_vanishing_x(),
            vanishing_y: default_vanishing_y(),
            vanishing_left_x: default_left_vp_x(),
            vanishing_right_x: default_right_vp_x(),
            vanishing_2point_y: default_two_point_y(),
            timeline_dock_height: default_timeline_height(),
            timeline_style: 0,
            timeline_new_cell_width: default_new_timeline_cell_width(),
            timeline_new_row_height: default_new_timeline_row_height(),
            timeline_show_timecodes: true,
            show_top_bar: true,
            show_left_dock: true,
            show_right_dock: true,
            right_panel_split: default_right_split(),
            show_timeline_dock: true,
            grid_size: 40.0,
            timeline_bg_color: default_tl_bg(),
            timeline_cell_color: default_tl_cell(),
            timeline_playhead_color: default_tl_head(),
            ui_panel_color: default_ui_panel_color(),
            ui_accent_color: default_ui_accent_color(),
            autosave_interval_minutes: 5,
            brush_cursor_scale: 1.0,
            tool_icon_scale: 1.0,
            icon_cursor_ring: false,
            cloud_google_client_id: default_google_client_id(),
            cloud_email: String::new(),
            cloud_auto_backup: false,
            tool_select: "V".to_string(),
            tool_lasso: "L".to_string(),
            tool_free_transform: "Q".to_string(),
            tool_vector_select: "G".to_string(),
            tool_pen: "P".to_string(),
            tool_sculpt: default_sculpt_key(),
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

/// User-specific settings live in the user's writable profile rather than in
/// Program Files. The executable-adjacent location is a legacy read fallback.
fn local_setting_paths(filename: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(base).join("StickStudio").join(filename));
    }
    if let Ok(exe) = std::env::current_exe() {
        paths.push(exe.with_file_name(filename));
    }
    paths
}

impl AppConfig {
    fn load() -> Self {
        for path in local_setting_paths("stick_config.json") {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(mut parsed) = serde_json::from_str::<Self>(&data) {
                    // Perspective overlays are tools, not saved project content.
                    // Never silently re-enable guides on the next launch.
                    parsed.perspective_mode = 0;
                    // Existing installations may persist an empty OAuth ID,
                    // overriding a newly configured public Desktop client ID
                    // embedded in updated official releases.
                    if parsed.cloud_google_client_id.trim().is_empty() {
                        parsed.cloud_google_client_id = default_google_client_id();
                    }
                    return parsed;
                }
            }
        }
        Self::default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            for path in local_setting_paths("stick_config.json") {
                if let Some(parent) = path.parent() {
                    if std::fs::create_dir_all(parent).is_err() { continue; }
                }
                if std::fs::write(&path, &json).is_ok() { break; }
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

/// Decode a locally selected avatar using the supported image codecs.
/// Render a small circle-cropped thumbnail; original project media is untouched.
fn load_artist_avatar(ctx: &egui::Context, path: Option<&str>) -> Option<egui::TextureHandle> {
    let path = path?;
    let reader = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let decoded = reader.decode().ok()?;
    let mut rgba = decoded.resize_to_fill(128, 128, image::imageops::FilterType::Lanczos3).to_rgba8();
    for y in 0..128 {
        for x in 0..128 {
            let dx = x as f32 - 63.5;
            let dy = y as f32 - 63.5;
            if dx * dx + dy * dy > 63.0 * 63.0 {
                rgba.get_pixel_mut(x, y).0[3] = 0;
            }
        }
    }
    let color_image = egui::ColorImage::from_rgba_unmultiplied([128, 128], rgba.as_raw());
    Some(ctx.load_texture("stick_artist_avatar", color_image, egui::TextureOptions::LINEAR))
}

fn apply_user_theme(ctx: &egui::Context, config: &AppConfig) {
    let mut style = (*ctx.style()).clone();
    let c = config.ui_panel_color;
    let a = config.ui_accent_color;
    style.visuals.panel_fill = egui::Color32::from_rgb(c[0], c[1], c[2]);
    style.visuals.window_fill = egui::Color32::from_rgb(
        c[0].saturating_add(7), c[1].saturating_add(7), c[2].saturating_add(7)
    );
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(a[0], a[1], a[2]);
    ctx.set_style(style);
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
        ActiveToolUI::Sculpt => egui::Color32::from_rgb(250, 136, 185),
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

// Cohesive two-tone vector icons adapted to the colorful toolbar reference.
// They stay crisp at high-DPI scaling and at the smaller mouse-cursor size.
fn draw_tool_ui_icon(painter: &egui::Painter, rect: egui::Rect, tool: ActiveToolUI, active: bool) {
    let c = rect.center();
    let primary = get_tool_ui_color(tool);
    let pale = egui::Color32::from_rgb(234, 245, 255);
    let deep = egui::Color32::from_rgb(39, 43, 62);
    let line = egui::Stroke::new(2.2, primary);
    let icon_scale=(rect.width()/26.0).clamp(0.6,2.0);
    let p = |x:f32,y:f32| c+egui::vec2(x*icon_scale,y*icon_scale);
    match tool {
        ActiveToolUI::SelectNormal => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(-7.,-10.),p(-5.,9.),p(-0.5,3.0),p(5.,2.)],
                primary,egui::Stroke::new(0.8,pale)));
            painter.line_segment([p(0.,3.),p(5.,10.)],egui::Stroke::new(3.3,primary));
            painter.line_segment([p(-5.5,-7.),p(-4.5,4.)],egui::Stroke::new(1.2,pale));
        }
        ActiveToolUI::SelectLasso => {
            painter.circle_stroke(p(-2.5,-3.5),8.0,egui::Stroke::new(3.0,primary));
            painter.circle_stroke(p(-2.5,-3.5),5.5,egui::Stroke::new(0.9,pale));
            painter.line_segment([p(3.5,2.),p(10.,10.)],egui::Stroke::new(3.4,primary));
            painter.circle_filled(p(10.,10.),2.1,pale);
        }
        ActiveToolUI::FreeTransform => {
            painter.rect_stroke(egui::Rect::from_center_size(c,egui::vec2(14.,14.)),0.,
                egui::Stroke::new(1.6,primary));
            for (x,y) in [(-8.,-8.),(8.,-8.),(-8.,8.),(8.,8.)] {
                let r=egui::Rect::from_center_size(p(x,y),egui::vec2(5.,5.));
                painter.rect_filled(r,0.,primary);
                painter.rect_stroke(r,0.,egui::Stroke::new(0.8,pale));
            }
            painter.circle_stroke(p(0.,-11.),2.1,egui::Stroke::new(1.7,primary));
        }
        ActiveToolUI::VectorDirectSelect => {
            let verts=[p(-8.,-9.),p(-5.,9.),p(8.,0.)];
            painter.line_segment([verts[0],verts[1]],line);
            painter.line_segment([verts[1],verts[2]],line);
            painter.line_segment([verts[0],verts[2]],line);
            for (i,point) in verts.iter().enumerate() {
                painter.circle_filled(*point,if i==1{3.4}else{3.0},primary);
                painter.circle_filled(*point,1.0,pale);
            }
            painter.line_segment([p(-5.,9.),p(9.,-9.)],egui::Stroke::new(1.0,primary));
        }
        ActiveToolUI::Sculpt => {
            painter.circle_stroke(p(-3.,-2.),8.0,egui::Stroke::new(2.0,primary));
            painter.line_segment([p(-8.,7.),p(7.,-8.)],egui::Stroke::new(3.4,primary));
            painter.circle_filled(p(7.,-8.),3.0,pale);
        }
        ActiveToolUI::VectorPen => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.,-11.),p(8.,0.),p(3.5,9.),p(-3.5,9.),p(-8.,0.)],
                primary,egui::Stroke::new(1.0,deep)));
            painter.line_segment([p(0.,-9.),p(0.,-1.)],egui::Stroke::new(1.5,pale));
            painter.circle_filled(p(0.,0.5),2.4,pale);
            painter.rect_filled(egui::Rect::from_min_max(p(-5.,10.),p(5.,12.)),0.,deep);
        }
        ActiveToolUI::Brush => {
            painter.line_segment([p(-3.,4.),p(8.,-9.)],egui::Stroke::new(5.,primary));
            painter.line_segment([p(1.,-1.),p(7.,-8.)],egui::Stroke::new(1.4,pale));
            painter.add(egui::Shape::convex_polygon(
                vec![p(-4.,3.),p(-10.,7.),p(-10.,10.),p(-3.,9.),p(-1.,6.)],
                egui::Color32::from_rgb(217,72,56),egui::Stroke::NONE));
            painter.circle_filled(p(-8.,8.),1.4,primary);
        }
        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
            let top = if tool==ActiveToolUI::EraserRed {
                egui::Color32::from_rgb(244,88,112)
            } else { egui::Color32::from_rgb(44,207,117) };
            painter.add(egui::Shape::convex_polygon(
                vec![p(-9.,3.),p(1.,-10.),p(9.,-3.),p(-1.,10.)],
                top,egui::Stroke::new(0.7,pale)));
            painter.add(egui::Shape::convex_polygon(
                vec![p(-9.,3.),p(-1.,10.),p(4.,3.),p(-4.,-4.)],
                egui::Color32::from_rgb(190,197,211),egui::Stroke::NONE));
            painter.line_segment([p(-8.,11.),p(9.,11.)],egui::Stroke::new(1.6,primary));
        }
        ActiveToolUI::Bucket => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(-8.,0.),p(-1.,-8.),p(8.,-1.),p(1.,7.)],
                primary,egui::Stroke::new(1.0,deep)));
            painter.line_segment([p(-5.,-1.),p(2.,-5.)],egui::Stroke::new(1.5,pale));
            painter.line_segment([p(-3.,-10.),p(3.,-10.)],egui::Stroke::new(2.3,primary));
            painter.line_segment([p(-3.,-10.),p(-3.,-5.)],egui::Stroke::new(2.3,primary));
            painter.add(egui::Shape::convex_polygon(
                vec![p(7.,3.),p(10.,8.),p(6.,10.),p(4.,8.)],
                egui::Color32::from_rgb(37,227,183),egui::Stroke::NONE));
        }
        ActiveToolUI::Eyedropper => {
            painter.line_segment([p(-7.,7.),p(4.,-4.)],egui::Stroke::new(4.0,primary));
            painter.circle_filled(p(6.,-6.),4.0,primary);
            painter.circle_filled(p(7.,-7.),1.5,pale);
            painter.add(egui::Shape::convex_polygon(
                vec![p(-8.,6.),p(-10.,10.),p(-4.,8.)],
                egui::Color32::from_rgb(246,215,68),egui::Stroke::NONE));
        }
        ActiveToolUI::Text => {
            painter.line_segment([p(-9.,-9.),p(9.,-9.)],egui::Stroke::new(3.4,primary));
            painter.line_segment([p(0.,-9.),p(0.,10.)],egui::Stroke::new(3.4,primary));
            painter.line_segment([p(-5.,10.),p(5.,10.)],egui::Stroke::new(3.4,primary));
            painter.circle_filled(p(-8.,-9.),1.5,pale);
        }
        ActiveToolUI::ShapeLine => {
            let sq=egui::Rect::from_min_max(p(-10.,-10.),p(1.,1.));
            painter.rect_stroke(sq,1.0,egui::Stroke::new(3.0,primary));
            painter.circle_stroke(p(5.,5.),6.0,egui::Stroke::new(3.0,primary));
            painter.circle_filled(p(5.,5.),2.5,egui::Color32::from_rgb(46,207,188));
        }
        ActiveToolUI::HandCanvas => {
            let hand = egui::Color32::from_rgb(172,102,253);
            painter.add(egui::Shape::convex_polygon(
                vec![p(-8.,1.),p(7.,1.),p(7.,9.),p(-4.,9.),p(-9.,4.)],
                hand,egui::Stroke::new(0.9,deep)));
            for (x,y) in [(-5.,-8.),(-1.,-10.),(3.,-9.),(7.,-6.)] {
                painter.line_segment([p(x,y),p(x,3.)],egui::Stroke::new(3.1,hand));
            }
            painter.line_segment([p(-7.,4.),p(-4.,7.)],egui::Stroke::new(1.2,pale));
        }
    }
    if active {
        painter.circle_filled(rect.right_top()+egui::vec2(-2.5,2.5),1.5,pale);
    }
}

fn draw_gear_icon(painter: &egui::Painter, rect: egui::Rect, scale: f32) {
    let c = rect.center();
    painter.rect_filled(rect.shrink(1.0), 6.0, egui::Color32::from_rgb(115, 75, 19));
    painter.rect_stroke(rect.shrink(1.0), 6.0,
        egui::Stroke::new(1.4, egui::Color32::from_rgb(255, 194, 73)));
    let color = egui::Color32::from_rgb(255, 232, 165);
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
            let sx_dir = if dx < 0.0 { -1.0 } else { 1.0 };
            let sy_dir = if dy < 0.0 { -1.0 } else { 1.0 };
            let (sx, sy) = (p0.x + side * sx_dir, p0.y + side * sy_dir);
            for c in [egui::pos2(p0.x, p0.y), egui::pos2(sx, p0.y), egui::pos2(sx, sy), egui::pos2(p0.x, sy), egui::pos2(p0.x, p0.y)] {
                pts.push(StrokePoint { x: c.x, y: c.y, pressure: 1.0 });
            }
        }
        GeometricShapeKind::Rectangle => {
            for c in [egui::pos2(p0.x, p0.y), egui::pos2(p1.x, p0.y), egui::pos2(p1.x, p1.y), egui::pos2(p0.x, p1.y), egui::pos2(p0.x, p0.y)] {
                pts.push(StrokePoint { x: c.x, y: c.y, pressure: 1.0 });
            }
        }
        GeometricShapeKind::Triangle | GeometricShapeKind::Pentagon => {
            let count = if kind == GeometricShapeKind::Triangle { 3 } else { 5 };
            let center = p0.lerp(p1, 0.5);
            let rx = ((p1.x - p0.x) * 0.5).abs();
            let ry = ((p1.y - p0.y) * 0.5).abs();
            for i in 0..=count {
                let theta = -std::f32::consts::FRAC_PI_2
                    + (i as f32 / count as f32) * std::f32::consts::TAU;
                pts.push(StrokePoint {
                    x: center.x + theta.cos() * rx,
                    y: center.y + theta.sin() * ry,
                    pressure: 1.0,
                });
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
        brush_texture_id: None, tip_shape: BrushTipShape::Round,
    }
}

struct StickApp {
    workspace_mode: WorkspaceMode,
    online_page: OnlinePage,
    online_enabled: bool,
    online_mode: OnlineMode,
    online_room_name: String,
    online_password: String,
    online_capacity: usize,
    online_address: String,
    online_code_input: String,
    online_participants: Vec<Participant>,
    online_rooms: Vec<RoomListing>,
    online_discovery: Option<std::sync::mpsc::Receiver<collab::RoomSearchResult>>,
    online_remote_info: Option<remote_access::TailnetInfo>,
    online_session: Option<OnlineSession>,
    online_message: String,
    online_members: usize,
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
    show_timeline_settings: bool,
    show_feedback_dialog: bool,
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
    shape_fill_enabled: bool,
    pending_delete_layer: Option<usize>,
    layer_rename_index: Option<usize>,
    layer_rename_buffer: String,
    layer_filter: String,
    layer_clipboard: Option<Layer>,
    symmetry_mode: SymmetryMode,
    stabilizer_strength: f32,
    stabilized_cursor: Option<egui::Pos2>,
    shape_drag_start: Option<egui::Pos2>,
    shape_drag_current: Option<egui::Pos2>,
    selected_stroke_indices: Vec<usize>,
    selected_vector_node: Option<(usize, usize)>,
    selected_vector_nodes: Vec<(usize,usize)>,
    vector_marquee_start: Option<egui::Pos2>,
    vector_marquee_current: Option<egui::Pos2>,
    vector_axis_drag: Option<char>,
    vector_tolerance: f32,
    sculpt_radius: f32,
    sculpt_strength: f32,
    transform_drag_anchor: Option<egui::Pos2>,
    transform_rotate_pivot: Option<egui::Pos2>,
    transform_axis: Option<char>,
    lasso_dragging_selection: bool,
    selected_text_item_index: Option<usize>,
    selection_rect_start: Option<egui::Pos2>,
    selection_rect_current: Option<egui::Pos2>,
    lasso_points: Vec<egui::Pos2>,
    custom_anchor_offset: egui::Vec2,
    brush_color: [u8; 4],
    stage_bg_color: [u8; 4],
    brush_size: f32,
    bucket_gap_close: f32,
    bucket_reference_all_layers: bool,
    brush_tip_shape: BrushTipShape,
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
    cloud_job: Option<std::sync::mpsc::Receiver<cloud_sync::Event>>,
    profile_sync_pending: bool,
    last_profile_cloud_sync_time: f64,
    cloud_message: String,
    cloud_files: Vec<cloud_sync::CloudFile>,
    cloud_restore_confirmation: Option<cloud_sync::CloudFile>,
    show_shortcuts_dialog: bool,
    show_credits_dialog: bool,
    show_support_dialog: bool,
    settings_tab: usize,
    listening_shortcut_idx: Option<usize>,
    config: AppConfig,
    user_profile: UserProfile,
    artist_avatar: Option<egui::TextureHandle>,
    installed_version: String,
    update_state: Arc<Mutex<UpdateState>>,
    rollback_state: Arc<Mutex<RollbackState>>,
    rollback_choice: Option<updater::ReleaseInfo>,
    new_symbol_name: String,
    allow_close: bool,
    current_stroke: Option<Stroke>,
    stylus_tracker: TouchPressureTracker,
    texture_cache: HashMap<String, egui::TextureHandle>,
    status_message: String,
    audio_sink: Option<(OutputStream, Sink)>,
    last_pointer_pos: Option<egui::Pos2>,
    last_secondary_pointer_pos: Option<egui::Pos2>,
}

impl StickApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        apply_stick_theme(&cc.egui_ctx);
        let mut proj = AniProject::default();
        proj.total_frames = 1;
        let profile = UserProfile::load();
        let artist_avatar = load_artist_avatar(&cc.egui_ctx, profile.avatar_path.as_deref());
        let default_install_path = std::env::var("LOCALAPPDATA").map(|p| format!("{}\\StickStudio", p)).unwrap_or_else(|_| "C:\\StickStudio".to_string());
        let initial_mode = if is_running_from_installed_dir() { WorkspaceMode::Launcher } else { WorkspaceMode::InstallerSetup };

        Self {
            workspace_mode: initial_mode,
            online_page: OnlinePage::ChooseMode,
            online_enabled: false,
            online_mode: OnlineMode::Drawing,
            online_room_name: "My Studio".to_owned(),
            online_password: String::new(),
            online_capacity: 4,
            online_address: String::new(),
            online_code_input: String::new(),
            online_participants: Vec::new(),
            online_rooms: Vec::new(),
            online_discovery: None,
            online_remote_info: None,
            online_session: None,
            online_message: String::new(),
            online_members: 1,
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
            timeline_cell_width: 30.0,
            show_timeline_settings: false,
            show_feedback_dialog: false,
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
            shape_fill_enabled: false,
            pending_delete_layer: None,
            layer_rename_index: None,
            layer_rename_buffer: String::new(),
            layer_filter: String::new(),
            layer_clipboard: None,
            symmetry_mode: SymmetryMode::Off,
            stabilizer_strength: 0.30,
            stabilized_cursor: None,
            shape_drag_start: None,
            shape_drag_current: None,
            selected_stroke_indices: Vec::new(),
            selected_vector_node: None,
            selected_vector_nodes: Vec::new(),
            vector_marquee_start: None,
            vector_marquee_current: None,
            vector_axis_drag: None,
            vector_tolerance: 2.7,
            sculpt_radius: 64.0,
            sculpt_strength: 0.65,
            transform_drag_anchor: None,
            transform_rotate_pivot: None,
            transform_axis: None,
            lasso_dragging_selection: false,
            selected_text_item_index: None,
            selection_rect_start: None,
            selection_rect_current: None,
            lasso_points: Vec::new(),
            custom_anchor_offset: egui::Vec2::ZERO,
            brush_color: [0, 0, 0, 255],
            stage_bg_color: [255, 255, 255, 255],
            brush_size: 7.0,
            bucket_gap_close: 3.0,
            bucket_reference_all_layers: false,
            brush_tip_shape: BrushTipShape::Round,
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
            show_patch_notes_dialog: AppConfig::load().last_seen_patch_version != get_installed_version(),
            show_settings_dialog: false,
            cloud_job: None,
            profile_sync_pending: false,
            last_profile_cloud_sync_time: 0.0,
            cloud_message: String::new(),
            cloud_files: Vec::new(),
            cloud_restore_confirmation: None,
            show_shortcuts_dialog: false,
            show_credits_dialog: false,
            show_support_dialog: false,
            settings_tab: 0,
            listening_shortcut_idx: None,
            config: AppConfig::load(),
            user_profile: profile,
            artist_avatar,
            installed_version: get_installed_version(),
            update_state: Arc::new(Mutex::new(UpdateState::Idle)),
            rollback_state: Arc::new(Mutex::new(RollbackState::Idle)),
            rollback_choice: None,
            new_symbol_name: "Symbol_1".to_string(),
            allow_close: false,
            current_stroke: None,
            stylus_tracker: TouchPressureTracker::default(),
            texture_cache: HashMap::new(),
            status_message: "Stick Studio Ready".to_string(),
            audio_sink: None,
            last_pointer_pos: None,
            last_secondary_pointer_pos: None,
        }
    }

    fn refresh_online_rooms(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(collab::discover_rooms_with_remote()); });
        self.online_discovery = Some(rx);
    }

    fn poll_online_events(&mut self, ctx: &egui::Context) {
        let discovered = self.online_discovery.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = discovered {
            self.online_rooms = result.rooms;
            self.online_remote_info = Some(result.tailnet);
            self.online_discovery = None;
        }
        let events = self.online_session.as_ref().map(|s| s.events()).unwrap_or_default();
        for event in events {
            match event {
                OnlineEvent::Connected(project) => {
                    if self.has_unsaved_changes {
                        if let Err(err) = persistence::atomic_save::write_recovery_snapshot(&self.project, "online_before_join") {
                            self.online_message = format!("Cannot back up local work: {}", err);
                            self.online_session = None;
                            continue;
                        }
                    }
                    self.save_undo_state();
                    self.timeline_length = project.total_frames.max(1);
                    self.current_frame = 1;
                    self.active_layer_idx = 0;
                    self.project = project;
                    if let Some(session) = &self.online_session { session.set_baseline(&self.project); }
                    self.current_project_path = None;
                    self.has_unsaved_changes = true;
                    self.online_message.clear();
                    self.workspace_mode = match self.online_mode {
                        OnlineMode::Drawing => WorkspaceMode::DrawingOnly,
                        OnlineMode::Animation => WorkspaceMode::AnimationStudio,
                    };
                }
                OnlineEvent::Strokes { layer, frame, strokes } => {
                    if collab::apply_strokes(&mut self.project, layer, frame, strokes) {
                        self.timeline_length = self.timeline_length.max(self.project.total_frames);
                        self.has_unsaved_changes = true;
                        // Undo snapshots from before a remote operation could erase another user's edits.
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                    }
                }
                OnlineEvent::Edits(edits) => {
                    let result = self.online_session.as_ref()
                        .map(|session| session.ingest_remote_edits(&mut self.project, &edits));
                    match result {
                        Some(Ok(())) => {
                            self.timeline_length = self.timeline_length.max(self.project.total_frames);
                            self.active_layer_idx = self.active_layer_idx.min(self.project.layers.len().saturating_sub(1));
                            self.undo_stack.clear();
                            self.redo_stack.clear();
                            self.selected_stroke_indices.clear();
                            self.has_unsaved_changes = true;
                        }
                        Some(Err(err)) => {
                            self.online_message = format!("Online edit conflict: {err}. Ask the host to sync the project.");
                            self.status_message = self.online_message.clone();
                        }
                        None => {}
                    }
                }
                OnlineEvent::Snapshot(project) => {
                    if self.has_unsaved_changes {
                        if let Err(err) = persistence::atomic_save::write_recovery_snapshot(&self.project, "online_before_resync") {
                            self.online_message = format!("Cannot back up local work: {}", err);
                            continue;
                        }
                    }
                    self.project = project;
                    if let Some(session) = &self.online_session { session.set_baseline(&self.project); }
                    self.timeline_length = self.project.total_frames.max(1);
                    self.current_frame = self.current_frame.min(self.timeline_length);
                    self.active_layer_idx = self.active_layer_idx.min(self.project.layers.len().saturating_sub(1));
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.selected_stroke_indices.clear();
                    self.has_unsaved_changes = true;
                }
                OnlineEvent::Members(n) => self.online_members = n,
                OnlineEvent::Roster(roster) => {
                    self.online_members = roster.len();
                    self.online_participants = roster;
                }
                OnlineEvent::HostHandoff { address, promote, title, room_code, limit, project } => {
                    let password = self.online_session.as_ref()
                        .map(|s| s.password.clone()).unwrap_or_default();
                    self.leave_online_session();
                    let nickname = self.user_profile.username.trim().to_owned();
                    let nickname = if nickname.is_empty() { "Artist".into() } else { nickname };
                    let level = self.user_profile.get_level_progress().0;
                    self.online_room_name = title.clone();
                    self.online_code_input = room_code.clone();
                    self.online_password = password.clone();
                    let result = if promote {
                        if let Some(snapshot) = project {
                            let session = OnlineSession::host_room_with_code(
                                title, password, limit, self.online_mode,
                                snapshot.clone(), nickname, level, Some(room_code));
                            session.map(|new_session| {
                                // Transfer takes the previous host's canonical snapshot,
                                // not a guessed copy of the guest's unsent edits.
                                if self.has_unsaved_changes {
                                    let _ = persistence::atomic_save::write_recovery_snapshot(
                                        &self.project, "online_before_host_transfer");
                                }
                                self.project = snapshot;
                                self.timeline_length = self.project.total_frames.max(1);
                                self.active_layer_idx = self.active_layer_idx.min(self.project.layers.len().saturating_sub(1));
                                self.undo_stack.clear();
                                self.redo_stack.clear();
                                self.online_members = 1;
                                self.online_participants = vec![Participant {
                                    id: 0, name: self.user_profile.username.clone(),
                                    level, color: [64, 185, 250],
                                }];
                                self.online_session = Some(new_session);
                                self.has_unsaved_changes = true;
                                self.online_message = tr(self.config.lang,
                                    "You are the new room host. Other members are reconnecting.",
                                    "إنت دلوقتي الهوست الجديد، وباقي الأعضاء بيعيدوا الاتصال.").to_owned();
                            })
                        } else {
                            Err("Host transfer did not include the room project".to_owned())
                        }
                    } else {
                        OnlineSession::rejoin_after_handoff(address, password,
                            nickname, level, self.online_mode).map(|mut new_session| {
                                new_session.title = title;
                                new_session.room_code = room_code;
                                self.online_session = Some(new_session);
                                self.online_message = tr(self.config.lang,
                                    "Reconnecting to the new host...", "جاري الاتصال بالهوست الجديد...").into();
                        })
                    };
                    if let Err(err) = result {
                        self.online_message = format!("Host transfer failed: {err}");
                        self.status_message = self.online_message.clone();
                        self.has_unsaved_changes = true;
                    }
                }
                OnlineEvent::Notice(message) => self.online_message = message,
                OnlineEvent::Disconnected(message) => {
                    self.online_message = message;
                    self.online_session = None;
                    // Keep the local project and any unsaved edits intact after disconnect.
                    self.has_unsaved_changes = true;
                    self.status_message = self.online_message.clone();
                }
            }
        }
        if self.online_session.is_some() || self.online_discovery.is_some() {
            ctx.request_repaint_after(Duration::from_millis(160));
        }
    }

    fn leave_online_session(&mut self) {
        if let Some(session) = self.online_session.take() {
            if session.is_host {
                // Flush this editor's pending local changes before choosing the
                // authoritative document snapshot that the successor inherits.
                if let Err(err) = session.sync_local_changes(&self.project) {
                    self.online_message = format!("Could not flush edits before handoff: {err}");
                }
                match session.handoff_before_leave() {
                    Ok(true) => self.online_message =
                        "Room ownership offered to the earliest connected guest.".into(),
                    Ok(false) => {}
                    Err(err) => self.online_message = format!("Host transfer could not finish: {err}"),
                }
            }
            session.close();
        }
    }

    fn start_cloud_backup(&mut self) {
        if self.cloud_job.is_some(){return;}
        let Some(account)=self.user_profile.cloud_account_id.clone() else{return};
        let client=self.config.cloud_google_client_id.clone();
        let profile=cloud_sync::CloudProfile::from_user(&self.user_profile);
        let mut paths:Vec<PathBuf>=self.config.recent_projects.iter()
            .map(PathBuf::from).collect();
        if let Some(path)=&self.current_project_path {paths.push(path.clone());}
        paths.sort();paths.dedup();
        self.cloud_job=Some(cloud_sync::spawn_job(move||{
            cloud_sync::backup(&client,&account,profile,paths)
                .map(cloud_sync::Event::BackedUp)
        }));
        self.profile_sync_pending = false;
        self.cloud_message="Uploading Google Drive backups...".to_owned();
    }

    fn maybe_sync_artist_profile(&mut self, now_time: f64) {
        if !self.profile_sync_pending || !self.config.cloud_auto_backup
            || self.cloud_job.is_some() { return; }
        let Some(account) = self.user_profile.cloud_account_id.clone() else { return; };
        if self.last_profile_cloud_sync_time > 0.0
            && now_time - self.last_profile_cloud_sync_time < 90.0 { return; }
        let client = self.config.cloud_google_client_id.clone();
        let profile = cloud_sync::CloudProfile::from_user(&self.user_profile);
        self.cloud_job = Some(cloud_sync::spawn_job(move || {
            cloud_sync::backup_profile(&client, &account, profile)
                .map(cloud_sync::Event::BackedUp)
        }));
        self.last_profile_cloud_sync_time = now_time;
        self.profile_sync_pending = false;
    }

    fn poll_cloud_job(&mut self, ctx: &egui::Context) {
        let event=self.cloud_job.as_ref().and_then(|rx|rx.try_recv().ok());
        if let Some(event)=event {
            self.cloud_job=None;
            match event {
                cloud_sync::Event::SignedIn(identity)=>{
                    self.user_profile.cloud_account_id=Some(identity.account_id);
                    self.user_profile.save();
                    self.config.cloud_email=identity.email;
                    self.config.save();
                    self.profile_sync_pending = true;
                    self.last_profile_cloud_sync_time = 0.0;
                    self.cloud_message="Google sign-in successful. You can back up your projects.".into();
                }
                cloud_sync::Event::BackedUp(n)=>{
                    self.cloud_message=format!("Google Drive: backed up {n} items (profile + saved projects).");
                }
                cloud_sync::Event::Listed(files)=>{
                    let count=files.len();
                    self.cloud_files=files;
                    self.cloud_message=format!("{count} Google Drive backup item(s) found.");
                }
                cloud_sync::Event::RestoredProject(path)=>{
                    self.open_project_at_path(path.clone());
                    self.cloud_message=format!("Restored backup as a new local copy: {}",path.display());
                }
                cloud_sync::Event::RestoredProfile(profile)=>{
                    profile.apply(&mut self.user_profile);
                    self.artist_avatar = load_artist_avatar(ctx, self.user_profile.avatar_path.as_deref());
                    self.cloud_message="Cloud profile restored. Artist name, avatar and progress have been updated.".into();
                }
                cloud_sync::Event::Error(err)=>self.cloud_message=err,
            }
        }
    }

    fn render_online_lobby(&mut self, ctx: &egui::Context) {
        let lang = self.config.lang;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(36.0);
                ui.heading(egui::RichText::new(tr(lang, "STICK ONLINE", "ستيك أونلاين"))
                    .size(33.0).color(egui::Color32::from_rgb(79, 195, 247)));
                ui.add_space(10.0);
                ui.label(tr(lang, "Your computer hosts the room. No dedicated Stick server.",
                    "جهاز صاحب الغرفة هو الهوست، بدون سيرفر مخصص للبرنامج."));
                ui.add_space(12.0);
                if ui.checkbox(&mut self.online_enabled,
                    tr(lang, "Enable online networking for this session",
                        "تشغيل اتصال الأونلاين للجلسة دي فقط")).changed() {
                    if !self.online_enabled {
                        self.leave_online_session();
                        self.online_discovery=None;
                        self.online_remote_info=None;
                        self.online_rooms.clear();
                    }
                }
                if !self.online_enabled {
                    ui.colored_label(egui::Color32::from_rgb(238,192,110),
                        tr(lang, "Offline mode. No room discovery or host connection runs until you enable online.",
                            "وضع أوفلاين: لا بحث عن غرف ولا اتصال بسيرفر قبل ما تشغّل الأونلاين بنفسك."));
                    if ui.button(tr(lang, "Back to start", "الرجوع للرئيسية")).clicked(){
                        self.workspace_mode=WorkspaceMode::Launcher;
                    }
                    return;
                }
                ui.add_space(18.0);
                match self.online_page {
                    OnlinePage::ChooseMode => {
                        ui.heading(tr(lang, "What do you want to do together?", "عايزين تعملوا إيه سوا؟"));
                        ui.add_space(15.0);
                        if ui.add_sized([320.0, 64.0], egui::Button::new(tr(lang, "Draw Together", "نرسم سوا"))
                            .fill(egui::Color32::from_rgb(245, 116, 26))).clicked() {
                            self.online_mode = OnlineMode::Drawing;
                            self.online_page = OnlinePage::ChooseAction;
                        }
                        ui.add_space(14.0);
                        if ui.add_sized([320.0, 64.0], egui::Button::new(tr(lang, "Animate Together", "أنيميشن سوا"))
                            .fill(egui::Color32::from_rgb(0, 125, 205))).clicked() {
                            self.online_mode = OnlineMode::Animation;
                            self.online_page = OnlinePage::ChooseAction;
                        }
                    }
                    OnlinePage::ChooseAction => {
                        ui.heading(tr(lang, "Create or Join a Room", "إنشاء غرفة أو الانضمام لغرفة"));
                        ui.add_space(18.0);
                        if ui.add_sized([300.0, 58.0], egui::Button::new(tr(lang, "Create Room", "إنشاء غرفة"))).clicked() {
                            self.online_page = OnlinePage::CreateRoom;
                            self.online_message.clear();
                        }
                        ui.add_space(12.0);
                        if ui.add_sized([300.0, 58.0], egui::Button::new(tr(lang, "Join Room", "انضمام لغرفة"))).clicked() {
                            self.online_page = OnlinePage::JoinRoom;
                            self.online_message.clear();
                            self.refresh_online_rooms();
                        }
                        ui.add_space(25.0);
                        ui.label(egui::RichText::new(tr(lang,
                            "Your PC hosts the room. For friends on other Internet connections, use Tailscale on both computers (no router port-forwarding).",
                            "جهازك هو سيرفر الغرفة. عشان تلعب مع أصحابك على نت مختلف، شغّل Tailscale على الجهازين من غير فتح بورت في الراوتر."))
                            .color(egui::Color32::from_rgb(238, 192, 110)));
                        if let Some(info) = &self.online_remote_info {
                            if info.connected {
                                if let Some(ip) = info.address {
                                    ui.colored_label(egui::Color32::LIGHT_GREEN,
                                        format!("Tailscale: {}:{}", ip, collab::TCP_PORT));
                                }
                            } else {
                                ui.label(tr(lang, "Tailscale is not connected.", "Tailscale مش متصل حاليًا."));
                            }
                        }
                        ui.hyperlink_to(tr(lang, "Get Tailscale for playing with friends", "تنزيل Tailscale للعب مع أصحابك"),
                            "https://tailscale.com/download");
                    }
                    OnlinePage::CreateRoom => {
                        ui.heading(tr(lang, "Host a room", "إنشاء غرفة جديدة"));
                        ui.add_space(12.0);
                        ui.label(tr(lang,
                            "Your PC becomes the host/server. Keep Stick open; friends can connect over LAN or via Tailscale from other networks.",
                            "جهازك هيبقى الهوست وسيرفر الغرفة. خليك فاتح Stick، وأصحابك يقدروا يدخلوا من نفس النت أو من أي نت تاني باستخدام Tailscale."));
                        ui.add_space(8.0);
                        if ui.small_button(tr(lang, "Refresh Tailscale status", "تحديث حالة Tailscale")).clicked() {
                            self.refresh_online_rooms();
                        }
                        if let Some(info) = &self.online_remote_info {
                            if let Some(ip) = info.address {
                                ui.colored_label(egui::Color32::LIGHT_GREEN,
                                    format!("Friends address: {}:{}", ip, collab::TCP_PORT));
                            }
                        }
                        ui.add_space(16.0);
                        egui::Grid::new("host_room_fields").min_col_width(140.0).spacing([12.0, 12.0]).show(ui, |ui| {
                            ui.label(tr(lang, "Room name", "اسم الغرفة"));
                            ui.add(egui::TextEdit::singleline(&mut self.online_room_name).desired_width(250.0));
                            ui.end_row();
                            ui.label(tr(lang, "Password (optional)", "كلمة السر (اختياري)"));
                            ui.add(egui::TextEdit::singleline(&mut self.online_password).password(true).desired_width(250.0));
                            ui.end_row();
                            ui.label(tr(lang, "Max players", "العدد الأقصى"));
                            ui.add(egui::Slider::new(&mut self.online_capacity, 2..=12).integer());
                            ui.end_row();
                        });
                        ui.add_space(18.0);
                        if ui.add_sized([220.0, 44.0], egui::Button::new(tr(lang, "Create and Start", "إنشاء وبدء الغرفة"))
                            .fill(egui::Color32::from_rgb(18, 160, 113))).clicked() {
                            // Hosting the current project keeps its artwork, layers and timeline.
                            // Never replace a user's existing document with a blank room.
                            let mut project = self.project.clone();
                            project.total_frames = self.timeline_length.max(1);
                            let nickname = if self.user_profile.username.trim().is_empty() {
                                "Artist".to_owned()
                            } else { self.user_profile.username.trim().to_owned() };
                            let level = self.user_profile.get_level_progress().0;
                            match OnlineSession::host_room(self.online_room_name.trim().to_owned(),
                                    self.online_password.clone(), self.online_capacity, self.online_mode,
                                    project, nickname.clone(), level) {
                                Ok(session) => {
                                    self.online_participants = vec![Participant {
                                        id: 0, name: nickname, level, color: [64,185,250],
                                    }];
                                    self.online_code_input = session.room_code.clone();
                                    self.online_session = Some(session);
                                    self.online_members = 1;
                                    self.online_message.clear();
                                    self.workspace_mode = match self.online_mode {
                                        OnlineMode::Drawing => WorkspaceMode::DrawingOnly,
                                        OnlineMode::Animation => WorkspaceMode::AnimationStudio,
                                    };
                                }
                                Err(err) => self.online_message = err,
                            }
                        }
                    }
                    OnlinePage::JoinRoom => {
                        ui.heading(tr(lang,"Join a room","الانضمام لغرفة"));
                        ui.small(tr(lang,
                            "Rooms from your LAN or authorized Tailscale friends appear here.",
                            "هنا بتظهر غرف نفس الشبكة أو أصحابك المسموح لهم على Tailscale."));
                        ui.add_space(10.0);
                        ui.horizontal(|ui|{
                            if ui.button(tr(lang,"Refresh","تحديث الغرف")).clicked(){
                                self.refresh_online_rooms();
                            }
                            if self.online_discovery.is_some(){ui.spinner();}
                            if let Some(info)=&self.online_remote_info {
                                if info.connected {
                                    ui.colored_label(egui::Color32::LIGHT_GREEN,
                                        format!("Tailscale • {} {}",info.peers.len(),
                                            tr(lang,"friends","جهاز متاح")));
                                }
                            }
                        });
                        ui.separator();
                        if self.online_rooms.is_empty() {
                            ui.label(tr(lang,
                                "No rooms found. You can still join directly using your friend's Tailscale IP.",
                                "مفيش غرف ظاهرة. تقدر تدخل مباشرة بعنوان Tailscale بتاع صاحبك."));
                        } else {
                            let rooms=self.online_rooms.clone();
                            egui::ScrollArea::vertical().max_height(245.0).show(ui,|ui|{
                                for room in rooms {
                                    ui.group(|ui|{
                                        ui.horizontal(|ui|{
                                            ui.vertical(|ui|{
                                                ui.strong(&room.title);
                                                ui.small(format!("{}  •  {}/{}  •  #{}",
                                                    if room.mode==OnlineMode::Drawing{
                                                        tr(lang,"Drawing","رسم")
                                                    } else {tr(lang,"Animation","أنيميشن")},
                                                    room.members,room.limit,room.room_code));
                                            });
                                            if room.locked {ui.label("🔒");}
                                            if ui.add_enabled(self.online_session.is_none(),
                                                egui::Button::new(
                                                    if room.locked {tr(lang,"Select","اختيار")}
                                                    else {tr(lang,"Join","دخول")})).clicked(){
                                                self.online_address=room.address.clone();
                                                self.online_code_input=room.room_code.clone();
                                                self.online_room_name=room.title.clone();
                                                self.online_mode=room.mode;
                                                self.online_password.clear();
                                                if room.locked {
                                                    self.online_message=tr(lang,
                                                        "This room is locked. Enter its password below.",
                                                        "الغرفة دي بكلمة سر. اكتبها في خانة الدخول المباشر تحت.").to_owned();
                                                } else {
                                                    let nickname=self.user_profile.username.trim().to_owned();
                                                    let level=self.user_profile.get_level_progress().0;
                                                    match OnlineSession::join_room(
                                                        room.address.clone(),String::new(),
                                                        if nickname.is_empty(){"Artist".into()}else{nickname},
                                                        level,room.mode){
                                                        Ok(mut session)=>{
                                                            session.title=room.title.clone();
                                                            session.room_code=room.room_code.clone();
                                                            self.online_session=Some(session);
                                                            self.online_message=tr(lang,
                                                                "Connecting to room...","جاري الاتصال بالغرفة...").to_owned();
                                                        }
                                                        Err(error)=>self.online_message=error,
                                                    }
                                                }
                                            }
                                        });
                                    });
                                }
                            });
                        }
                        ui.add_space(12.0);
                        ui.separator();
                        ui.strong(tr(lang,"Join directly","الدخول المباشر"));
                        ui.small(tr(lang,
                            "Enter the host's 100.x.x.x:47554 (Tailscale) or LAN IP. A room code alone isn't a global invite.",
                            "حط عنوان الهوست 100.x.x.x:47554 من Tailscale، أو عنوانه على نفس الشبكة."));
                        ui.add(egui::TextEdit::singleline(&mut self.online_address)
                            .desired_width(325.0).hint_text("100.74.20.10:47554"));
                        ui.add(egui::TextEdit::singleline(&mut self.online_password).password(true)
                            .desired_width(325.0).hint_text(tr(lang,
                                "Password (only if required)","كلمة السر (لو موجودة)")));
                        let connecting=self.online_session.is_some();
                        if ui.add_enabled(!connecting,
                            egui::Button::new(tr(lang,"Connect to host","اتصل بالغرفة"))).clicked(){
                            let nickname=self.user_profile.username.trim().to_owned();
                            let level=self.user_profile.get_level_progress().0;
                            match OnlineSession::join_room(self.online_address.trim().to_owned(),
                                self.online_password.clone(),
                                if nickname.is_empty(){"Artist".into()}else{nickname},
                                level,self.online_mode) {
                                Ok(mut session)=>{
                                    session.title=self.online_room_name.clone();
                                    session.room_code=self.online_code_input.clone();
                                    self.online_session=Some(session);
                                    self.online_message=tr(lang,"Connecting to host...",
                                        "جاري الاتصال بجهاز الهوست...").to_owned();
                                }
                                Err(err)=>self.online_message=err,
                            }
                        }
                        if connecting{ui.spinner();}
                        ui.collapsing(tr(lang,"Room code / troubleshooting",
                            "كود الغرفة / حل مشاكل الاتصال"),|ui|{
                            ui.label(tr(lang,
                                "Eight-digit codes search only rooms discovered above. For other networks share the host's Tailscale IP.",
                                "الكود بيدور فقط في الغرف الظاهرة فوق. لو على نت مختلف استخدم عنوان Tailscale بتاع الهوست."));
                            ui.horizontal(|ui|{
                                ui.add(egui::TextEdit::singleline(&mut self.online_code_input)
                                    .char_limit(8).desired_width(135.0).hint_text("12345678"));
                                if ui.button(tr(lang,"Find room","بحث")).clicked(){
                                    if let Some(room)=collab::find_room_by_code(
                                        &self.online_rooms,&self.online_code_input){
                                        self.online_address=room.address.clone();
                                        self.online_room_name=room.title.clone();
                                        self.online_mode=room.mode;
                                        self.online_message.clear();
                                    } else {
                                        self.online_message=tr(lang,
                                            "Code not found. Refresh rooms or enter the host's Tailscale IP.",
                                            "الكود مش ظاهر. حدّث الغرف أو استخدم عنوان Tailscale الخاص بالهوست.").into();
                                    }
                                }
                            });
                            ui.hyperlink_to("Tailscale", "https://tailscale.com/download");
                            ui.small(tr(lang,
                                "The host must keep Stick open; both devices must be permitted to reach each other. TCP 47554 must be allowed in the host firewall. Do not disable the firewall.",
                                "لازم صاحب الغرفة يسيب Stick شغال، والأجهزة تكون مسموح لها تتواصل. اسمح لـStick ببورت TCP 47554 في جدار حماية الهوست، من غير تعطيل الحماية."));
                        });

                    }
                }
                if !self.online_message.is_empty() {
                    ui.add_space(14.0);
                    ui.colored_label(egui::Color32::from_rgb(255, 170, 80), self.online_message.as_str());
                }
                ui.add_space(20.0);
                ui.label(egui::RichText::new(tr(lang,
                    "LAN and Tailscale friend rooms can appear automatically. Remote friends can join via the host's Tailscale 100.x IP without port forwarding; all peers must have access to that device. Stick's LAN mode is not encrypted by itself.",
                    "غرف الشبكة المحلية وأصحابك على Tailscale ممكن تظهر تلقائيًا. لو غرفة صاحبك مش ظاهرة، استخدم عنوان Tailscale 100.x الخاص بالهوست من غير فتح بورت في الراوتر. لازم الأجهزة يكون مسموح لها بالوصول لبعض؛ اتصال LAN نفسه غير مشفر."))
                    .small().color(egui::Color32::GRAY));
                ui.add_space(20.0);
                if ui.button(tr(lang, "Back", "رجوع")).clicked() {
                    match self.online_page {
                        OnlinePage::ChooseMode => {
                            self.workspace_mode = WorkspaceMode::Launcher;
                            self.online_enabled = false;
                        },
                        OnlinePage::ChooseAction => self.online_page = OnlinePage::ChooseMode,
                        OnlinePage::CreateRoom | OnlinePage::JoinRoom => {
                            self.leave_online_session();
                            self.online_page = OnlinePage::ChooseAction;
                        }
                    }
                    self.online_message.clear();
                }
            });
        });
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
        // The installer and the self-updater must report the SAME exact
        // compiled build number, including the GitHub Actions run suffix.
        if let Err(e) = std::fs::write(
            target_dir.join("version.txt"), updater::embedded_build_version()
        ) {
            self.install_status_text = format!("Couldn't record the installed version: {e}");
            return;
        }
        if let Err(e) = std::fs::write(target_dir.join("stick_installed.marker"), "installed") {
            self.install_status_text = format!("Couldn't finish installation: {e}");
            return;
        }
        let _ = create_desktop_shortcut(&target_exe);
        self.play_sfx(SfxType::LevelUpFanfare(0));
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
        if amount > 0 { self.profile_sync_pending = true; }
        if self.user_profile.add_xp_check_levelup(amount) {
            let (level, _, _) = self.user_profile.get_level_progress();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
            let seed = nanos ^ self.user_profile.xp.rotate_left(17) ^ (level as u64 * 31337);
            self.level_up_custom_msg = profile::level_up_message(
                (seed % 1000) as usize, self.config.lang == AppLang::Arabic
            );
            self.level_up_banner_timer = now_time + 10.0;
            let sound = profile::level_up_sound_variant(level, self.user_profile.xp, seed);
            self.play_sfx(SfxType::LevelUpFanfare(sound));
        }
    }

    fn record_frame_drawn_activity(&mut self, now_time: f64) {
        let mut award = 0u64;
        // Count a unique frame only once within the same project, even if
        // artists redraw it many times. Persist the marker in the project.
        if !self.project.xp_credited_frames.contains(&self.current_frame) {
            self.project.xp_credited_frames.push(self.current_frame);
            self.user_profile.total_frames_drawn += 1;
            if self.user_profile.total_frames_drawn % 20 == 0 { award += 50; }
            self.has_unsaved_changes = true;
        }
        // Count a layer only when it receives a genuine stroke for the first
        // time. Duplicates and imported layers are not awarded automatically.
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if !layer.xp_first_paint_rewarded {
                layer.xp_first_paint_rewarded = true;
                self.user_profile.total_painted_layers += 1;
                if self.user_profile.total_painted_layers % 5 == 0 { award += 20; }
                self.has_unsaved_changes = true;
            }
        }
        if award > 0 { self.grant_xp(award, now_time); }
        else { self.user_profile.save(); }
        self.profile_sync_pending = true;
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
        self.selected_vector_node = None;
        self.transform_drag_anchor = None;
        self.selected_text_item_index = None;
        self.selection_rect_start = None;
        self.selection_rect_current = None;
        self.lasso_points.clear();
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
            // Document-level sync at the end of update also covers new strokes.
        }
    }

    fn selected_canvas_bounds(&self) -> Option<egui::Rect> {
        let layer = self.project.layers.get(self.active_layer_idx)?;
        let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
        let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
        let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut found = false;
        for &index in &self.selected_stroke_indices {
            if let Some(stroke) = frame.strokes.get(index) {
                for p in &stroke.points {
                    min.x = min.x.min(p.x);
                    min.y = min.y.min(p.y);
                    max.x = max.x.max(p.x);
                    max.y = max.y.max(p.y);
                    found = true;
                }
            }
        }
        if let Some(index) = self.selected_text_item_index {
            if let Some(text) = frame.text_items.get(index) {
                let bounds = selection_tools::text_bounds(text);
                min = min.min(bounds.min);
                max = max.max(bounds.max);
                found = true;
            }
        }
        if found { Some(egui::Rect::from_min_max(min, max).expand(2.0)) }
        else { None }
    }

    fn resize_selection_from_corner(&mut self, anchor: egui::Pos2, factor: f32) {
        if self.is_active_layer_locked() || !factor.is_finite() { return; }
        let factor = factor.clamp(0.25, 4.0);
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                for &index in &self.selected_stroke_indices {
                    if let Some(stroke) = frame.strokes.get_mut(index) {
                        selection_tools::scale_points_around(stroke, anchor, factor);
                    }
                }
                if let Some(index) = self.selected_text_item_index {
                    if let Some(text) = frame.text_items.get_mut(index) {
                        text.x = anchor.x + (text.x - anchor.x) * factor;
                        text.y = anchor.y + (text.y - anchor.y) * factor;
                        text.font_size = (text.font_size * factor).clamp(4.0, 512.0);
                    }
                }
                self.has_unsaved_changes = true;
            }
        }
    }

    fn move_selected_items(&mut self, delta: egui::Vec2) {
        if self.is_active_layer_locked() || delta.length_sq() <= 0.0 { return; }
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                for &index in &self.selected_stroke_indices {
                    if let Some(stroke) = frame.strokes.get_mut(index) {
                        for p in &mut stroke.points { p.x += delta.x; p.y += delta.y; }
                    }
                }
                if let Some(idx) = self.selected_text_item_index {
                    if let Some(text) = frame.text_items.get_mut(idx) {
                        text.x += delta.x; text.y += delta.y;
                    }
                }
                self.has_unsaved_changes = true;
            }
        }
    }

    fn rotate_selection_around(&mut self, pivot: egui::Pos2, radians: f32) {
        if self.is_active_layer_locked() || !radians.is_finite() || radians.abs() < 0.00001 {
            return;
        }
        self.ensure_editable_keyframe();
        if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
            if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                for &index in &self.selected_stroke_indices {
                    if let Some(stroke) = frame.strokes.get_mut(index) {
                        selection_tools::rotate_points_around(stroke, pivot, radians);
                    }
                }
                if let Some(index) = self.selected_text_item_index {
                    if let Some(text) = frame.text_items.get_mut(index) {
                        let (sn, cs) = radians.sin_cos();
                        let (dx, dy) = (text.x - pivot.x, text.y - pivot.y);
                        text.x = pivot.x + dx * cs - dy * sn;
                        text.y = pivot.y + dx * sn + dy * cs;
                    }
                }
                self.has_unsaved_changes = true;
            }
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

    fn remember_recent_project(&mut self, path: &Path) {
        let full = path.to_string_lossy().to_string();
        self.config.recent_projects.retain(|x| x != &full);
        self.config.recent_projects.insert(0, full);
        self.config.recent_projects.truncate(8);
        self.config.save();
    }

    fn trigger_save(&mut self) -> bool {
        if let Some(existing_path) = self.current_project_path.clone() {
            self.project.total_frames = self.timeline_length;
            match persistence::save_project_safe(&self.project, &existing_path) {
                Ok(_) => {
                    self.has_unsaved_changes = false;
                    self.remember_recent_project(&existing_path);
                    if self.config.cloud_auto_backup { self.start_cloud_backup(); }
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
                    self.remember_recent_project(&path);
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
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Stick", &["stick", "ah2d"]).pick_file() {
            self.open_project_at_path(p);
        }
    }

    fn open_project_at_path(&mut self, p: PathBuf) {
        match persistence::open_project_safe(&p) {
                Ok(outcome) => {
                    self.remember_recent_project(&p);
                    self.save_undo_state();
                    self.timeline_length = outcome.project.total_frames.max(1);
                    self.current_frame = 1;
                    self.active_layer_idx = 0;
                    self.project = outcome.project;
                    self.has_unsaved_changes = false;
                    self.play_sfx(SfxType::NewFrame);

                    if outcome.report.was_migrated {
                        // لا نعيّن مسار حفظ آلياً: قد يوجد بالفعل مشروع مُحوّل بالاسم المتوقع.
                        // Ctrl+S سيفتح Save As لحماية الملف الأصلي وأي نسخة مُحوّلة سابقة.
                        self.current_project_path = None;
                        self.has_unsaved_changes = true;
                        let mut msg = format!(
                            "تم إنشاء هذا المشروع بإصدار قديم من Stick ({}).\nتم فتح نسخة متوافقة مع الإصدار الحالي (Schema v{}) مع الاحتفاظ بالملف الأصلي سليماً دون تعديل.\nعند الضغط على حفظ سيُطلب منك اختيار اسم ومسار للنسخة الجديدة (Save As) حتى لا نستبدل مشروعاً آخر.",
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

    fn import_image_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]).pick_file() {
            self.save_undo_state();
            let img_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let mut kf = HashMap::new();
            let mut fd = FrameData::default();
            fd.media_items.push(MediaItem { path: path.to_string_lossy().to_string(), media_type: MediaType::Image, x: 100.0, y: 50.0, width: 400.0, height: 300.0 });
            kf.insert(self.current_frame, fd);
            self.project.layers.insert(0, Layer { name: format!("Img_{}", img_name), visible: true, locked: false, is_guide: false, xp_first_paint_rewarded: false, opacity: 1.0, keyframes: kf });
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
                self.project.layers.insert(0, Layer { name: "Video_Layer".to_string(), visible: true, locked: false, is_guide: false, xp_first_paint_rewarded: false, opacity: 1.0, keyframes: kf });
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
        let state = Arc::clone(&self.update_state);
        let installed = self.installed_version.clone();
        std::thread::spawn(move || {
            let next = match updater::check_online(&installed) {
                Ok(Some(release)) => UpdateState::UpdateAvailable(release),
                Ok(None) => UpdateState::UpToDate,
                Err(error) => UpdateState::Error(error),
            };
            *state.lock().unwrap() = next;
            ctx.request_repaint();
        });
    }

    fn refresh_release_history(&mut self, ctx: egui::Context) {
        *self.rollback_state.lock().unwrap() = RollbackState::Checking;
        self.rollback_choice = None;
        let state = Arc::clone(&self.rollback_state);
        let installed = self.installed_version.clone();
        std::thread::spawn(move || {
            let result = match updater::fetch_rollback_history(&installed) {
                Ok(releases) => RollbackState::Available(releases),
                Err(error) => RollbackState::Error(error),
            };
            *state.lock().unwrap() = result;
            ctx.request_repaint();
        });
    }

    fn download_and_apply_update(&mut self, info: updater::ReleaseInfo, ctx: egui::Context) {
        if self.has_unsaved_changes || self.current_stroke.is_some() {
            self.project_notice_title = tr_notice(self.config.lang, "Save your work first", "احفظ شغلك الأول");
            self.project_notice_body = tr_notice(
                self.config.lang,
                "Save your open project (Ctrl+S), then click Install again. We won't restart and lose unsaved work.",
                "احفظ المشروع المفتوح (Ctrl+S)، وبعد كده اضغط تثبيت تاني. البرنامج مش هيقفل ويضيّع شغلك.",
            );
            self.show_project_notice = true;
            return;
        }
        *self.update_state.lock().unwrap() = UpdateState::Downloading {
            received: 0,
            total: info.size,
        };
        let state = Arc::clone(&self.update_state);
        std::thread::spawn(move || {
            let next = match std::env::current_exe() {
                Ok(current) => {
                    let progress_state = Arc::clone(&state);
                    let progress_ctx = ctx.clone();
                    match updater::download_verified_with_progress(&info, &current, move |received, total| {
                        if let Ok(mut guard) = progress_state.lock() {
                            *guard = UpdateState::Downloading { received, total };
                        }
                        progress_ctx.request_repaint();
                    }) {
                        Ok(staged_exe) => UpdateState::ReadyToRestart {
                            version: info.version.clone(), staged_exe,
                        },
                        Err(error) => UpdateState::Error(error),
                    }
                },
                Err(error) => UpdateState::Error(format!("Couldn't find Stick.exe: {error}")),
            };
            *state.lock().unwrap() = next;
            ctx.request_repaint();
        });
    }

    fn restart_with_new_exe(&mut self, version: String, staged_exe: PathBuf) {
        if self.has_unsaved_changes || self.current_stroke.is_some() {
            self.project_notice_title = tr_notice(self.config.lang, "Save before restarting", "احفظ قبل إعادة التشغيل");
            self.project_notice_body = tr_notice(
                self.config.lang,
                "You have unsaved changes. Save your project, then restart the update.",
                "عندك تعديلات لسه متحفظتش. احفظ المشروع وبعدها كمّل التحديث.",
            );
            self.show_project_notice = true;
            return;
        }
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|current| updater::spawn_replacer(&current, &staged_exe, &version));
        match result {
            Ok(()) => std::process::exit(0),
            Err(error) => *self.update_state.lock().unwrap() = UpdateState::Error(error),
        }
    }

    fn render_update_dialog(&mut self, ctx: &egui::Context, lang: AppLang) {
        if self.show_update_dialog {
            let update_snapshot = self.update_state.lock().unwrap().clone();
            egui::Window::new(tr(lang, "Check for Updates", "تحديثات البرنامج"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Stick v{}", self.installed_version));
                    ui.separator();
                    match update_snapshot {
                        UpdateState::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr(lang, "Checking GitHub for updates...", "بندور على تحديثات جديدة..."));
                            });
                        }
                        UpdateState::UpToDate => {
                            ui.label(tr(lang, "No updates available right now. You're up to date!", "مفيش تحديث دلوقتي، إنت على آخر إصدار!"));
                            if ui.button(tr(lang, "Close", "قفل")).clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                        UpdateState::UpdateAvailable(info) => {
                            ui.label(tr(lang, "A new Stick version is ready.", "في تحديث جديد لبرنامج Stick."));
                            ui.label(format!("v{} → v{}", self.installed_version, info.version));
                            if ui.button(tr(lang, "Download and install update", "نزّل وثبّت التحديث")).clicked() {
                                self.download_and_apply_update(info, ctx.clone());
                            }
                            if ui.button(tr(lang, "Not now", "مش دلوقتي")).clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                        UpdateState::Downloading { received, total } => {
                            ui.label(tr(lang, "Downloading update...", "جاري تحميل التحديث..."));
                            let fraction = if total == 0 { 0.0 } else {
                                (received as f32 / total as f32).clamp(0.0, 1.0)
                            };
                            ui.add(egui::ProgressBar::new(fraction)
                                .desired_width(310.0).show_percentage());
                            ui.label(format!("{:.1} / {:.1} MB  •  {}%",
                                received as f64 / 1_048_576.0,
                                total as f64 / 1_048_576.0,
                                (fraction * 100.0).round() as u32));
                            if received == total {
                                ui.small(tr(lang, "Checking SHA-256 and preparing installation...",
                                    "جاري التحقق من سلامة الملف وتجهيز التثبيت..."));
                            }
                        }
                        UpdateState::ReadyToRestart { version, staged_exe } => {
                            if self.has_unsaved_changes || self.current_stroke.is_some() {
                                ui.label(tr(lang, "Update downloaded. Save your project before restarting.", "التحديث اتنزّل. احفظ مشروعك قبل ما البرنامج يعيد التشغيل."));
                                if ui.button(tr(lang, "Save and restart", "احفظ واعمل إعادة تشغيل")).clicked() {
                                    if self.trigger_save() {
                                        self.restart_with_new_exe(version, staged_exe);
                                    }
                                }
                            } else {
                                ui.label(tr(lang, "Update checked. Restarting Stick to finish installation...", "اتأكدنا إن التحديث سليم. Stick هيعيد التشغيل عشان يكمّل التثبيت..."));
                                self.restart_with_new_exe(version, staged_exe);
                            }
                        }
                        UpdateState::Error(error) => {
                            ui.colored_label(egui::Color32::LIGHT_RED, tr(lang, "Update failed. Your current Stick version is safe.", "التحديث فشل، بس نسختك الحالية سليمة."));
                            ui.label(error);
                            ui.hyperlink_to(
                                tr(lang, "Download the latest Stick manually", "نزّل آخر إصدار من GitHub يدويًا"),
                                "https://github.com/KaniEGY/Stick/releases/latest",
                            );
                            if ui.button(tr(lang, "Try again", "جرّب تاني")).clicked() {
                                self.check_for_updates(ctx.clone());
                            }
                            if ui.button(tr(lang, "Close", "قفل")).clicked() {
                                self.show_update_dialog = false;
                            }
                        }
                        UpdateState::Idle => {}
                    }
                });
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

    fn render_feedback_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_feedback_dialog { return; }
        let lang = self.config.lang;
        let mut open = true;
        let mut close_clicked = false;
        egui::Window::new(tr(lang, "Contact & feedback", "تواصل مع المطوّر"))
            .open(&mut open)
            .resizable(false).collapsible(false)
            .default_width(425.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(tr(lang, "Bug reports and feature ideas", "بلغ عن مشكلة أو اقترح ميزة"));
                ui.label(tr(lang,
                    "Choose a contact method. These links open in your browser.",
                    "اختار الطريقة المناسبة؛ الروابط دي هتفتح في المتصفح."));
                ui.add_space(12.0);
                ui.strong(tr(lang, "Instagram — message the developer", "إنستجرام — ابعت للمطوّر"));
                ui.hyperlink_to("Instagram @ablikash",
                    "https://www.instagram.com/ablikash?exln=NmFtNWZidnBvb3k1");
                ui.add_space(14.0);
                ui.strong(tr(lang, "GitHub — report a bug or suggest a feature",
                    "GitHub — افتح بلاغ أو اقترح ميزة"));
                ui.hyperlink_to(tr(lang, "Open GitHub Issues", "افتح صفحة الاقتراحات والمشاكل"),
                    "https://github.com/KaniEGY/Stick/issues/new/choose");
                ui.add_space(16.0);
                if ui.button(tr(lang, "Close", "إغلاق")).clicked() { close_clicked = true; }
            });
        self.show_feedback_dialog = open && !close_clicked;
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
            ActiveToolUI::Sculpt => {
                ui.strong(tr(lang,"Sculpt / Finger (J)","النحت / الصباع (J)"));
                ui.label(tr(lang,"Drag nearby linework to reshape it with soft falloff.",
                    "اسحب جزء من الخط لتحريكه بنعومة من غير تحريك الرسمة كلها."));
                ui.add(egui::Slider::new(&mut self.sculpt_radius,8.0..=360.0)
                    .text(tr(lang,"Radius","حجم التأثير")));
                ui.add(egui::Slider::new(&mut self.sculpt_strength,0.05..=1.0)
                    .text(tr(lang,"Strength","قوة التأثير")));
            }
            ActiveToolUI::VectorPen => {
                ui.strong(tr(lang, "Vector Ink Pen (P)", "قلم الفيكتور الثابت (بدون ذيل)"));
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Stroke Width:", "سُمك الخط:"));
                    ui.add(egui::DragValue::new(&mut self.brush_size).range(1.0..=100.0));
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=100.0).show_value(false));
                ui.add(egui::Slider::new(&mut self.brush_opacity, 0.0..=1.0)
                    .text(tr(lang, "Opacity", "الشفافية")));
                ui.add(egui::Slider::new(&mut self.vector_tolerance,0.4..=8.0)
                    .text(tr(lang,"Node reduction","تقليل النقاط")));
                ui.small(tr(lang,"Right-click the Vector Pen for advanced options.",
                    "كلك يمين على قلم الفيكتور هتلاقي إعدادات متقدمة."));
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
                ui.separator();
                ui.strong(tr(lang, "Brush Tip Shape", "شكل الفرشة"));
                ui.horizontal_wrapped(|ui| {
                    for (shape, en, ar) in [
                        (BrushTipShape::Round, "● Circle", "● دائري"),
                        (BrushTipShape::Square, "■ Square", "■ مربع"),
                        (BrushTipShape::Triangle, "▲ Triangle", "▲ مثلث"),
                        (BrushTipShape::Pentagon, "⬟ Pentagon", "⬟ خماسي"),
                        (BrushTipShape::TallRectangle, "▮ Tall", "▮ مستطيل طويل"),
                        (BrushTipShape::WideRectangle, "▬ Wide", "▬ مستطيل عريض"),
                    ] {
                        ui.selectable_value(&mut self.brush_tip_shape, shape, tr(lang,en,ar));
                    }
                });
            }
            ActiveToolUI::ShapeLine => {
                ui.strong(tr(lang, "Shapes & Line Tool (U)", "أداة الخطوط والأشكال"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::StraightLine, tr(lang, "Straight Line", "خط مستقيم"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Circle, tr(lang, "Circle", "دائرة"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Square, tr(lang, "Square", "مربع"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Rectangle, tr(lang, "Rectangle", "مستطيل"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Triangle, tr(lang, "Triangle", "مثلث"));
                ui.radio_value(&mut self.selected_shape_kind, GeometricShapeKind::Pentagon, tr(lang, "Pentagon", "خماسي"));
                ui.separator();
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=120.0)
                    .text(tr(lang, "Outline Size", "سُمك حدود الشكل")));
                ui.add(egui::Slider::new(&mut self.brush_opacity, 0.0..=1.0)
                    .text(tr(lang, "Opacity", "شفافية الشكل")));
                ui.checkbox(&mut self.shape_fill_enabled, tr(lang, "Fill shape", "لوّن جوّه الشكل"));
            }
            ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
                ui.strong(tr(lang, "Eraser Properties", "خصائص الممحاة"));
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserRed, tr(lang, "Red (Full E)", "حمراء (كامل E)")).clicked() { self.active_tool_ui = ActiveToolUI::EraserRed; }
                    if ui.selectable_label(self.active_tool_ui == ActiveToolUI::EraserGreen, tr(lang, "Green (Part T)", "خضراء (جزئي T)")).clicked() { self.active_tool_ui = ActiveToolUI::EraserGreen; }
                });
                ui.add(egui::Slider::new(&mut self.brush_size, 2.0..=120.0)
                    .text(tr(lang, "Eraser Size", "حجم الممحاة")));
            }
            ActiveToolUI::Text => {
                ui.strong(tr(lang, "Text Properties", "خصائص النص"));
                ui.add(egui::DragValue::new(&mut self.text_font_size).range(10.0..=180.0));
                ui.small(tr(lang, "To move text, switch to Selection (V), click the text, then drag.",
                    "عشان تحرك النص: اختار تحديد (V)، دوس على النص واسحبه."));
                ui.add(egui::Slider::new(&mut self.brush_opacity, 0.0..=1.0)
                    .text(tr(lang, "Text Opacity", "شفافية النص")));
            }
            ActiveToolUI::HandCanvas => {
                ui.strong(tr(lang, "Canvas Orientation", "تدوير وعكس الكانفا"));
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Flip H", "عكس أفقي")).clicked() { self.canvas_flip_h = !self.canvas_flip_h; }
                    if ui.button(tr(lang, "Flip V", "عكس رأسي")).clicked() { self.canvas_flip_v = !self.canvas_flip_v; }
                });
                ui.add(egui::Slider::new(&mut self.canvas_rotation_deg, -180.0..=180.0));
            }
            ActiveToolUI::Bucket => {
                ui.strong(tr(lang, "Paint Bucket • Bounded Regions", "الدلو • تلوين المساحات المقفولة"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
                ui.add(egui::Slider::new(&mut self.bucket_gap_close, 0.0..=12.0)
                    .text(tr(lang, "Close small gaps", "قفل الفجوات الصغيرة")));
                ui.checkbox(&mut self.bucket_reference_all_layers,
                    tr(lang, "Reference all visible layers", "استخدم حدود كل الليرات الظاهرة"));
                ui.small(tr(lang,
                    "Fill follows the visible enclosed area and grows slightly under outlines. Lower gap tolerance if adjacent regions merge.",
                    "الدلو بيلون المنطقة المقفولة ويمد اللون تحت الخط عشان ميعملش حواف بيضا. قلل قفل الفجوات لو منطقتين اتصلوا ببعض."));
            }
            ActiveToolUI::Eyedropper => {
                ui.strong(tr(lang, "Eyedropper • Pick Fill / Line", "القطّارة • لون التعبئة أو الخط"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
                ui.small(tr(lang,
                    "Click any visible colored stroke or filled region on the active layer.",
                    "اضغط على خط أو مساحة متلونة في اللير الحالي لاختيار لونها."));
            }
        }
    }
}

impl eframe::App for StickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = self.config.lang;
        self.poll_online_events(ctx);
        self.poll_cloud_job(ctx);
        let now_time = ctx.input(|i| i.time);
        self.maybe_sync_artist_profile(now_time);
        if self.profile_sync_pending { ctx.request_repaint_after(Duration::from_secs(4)); }
        apply_user_theme(ctx, &self.config);
        // The shortcut guide is discoverable even when the toolbar is hidden.
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_shortcuts_dialog = !self.show_shortcuts_dialog;
        }

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
            self.grant_xp(mins.saturating_mul(5), now_time);
        }

        if self.current_frame != self.last_scrubbed_frame {
            self.last_scrubbed_frame = self.current_frame;
            self.trigger_audio_stream_scrub();
        }

        if now_time < self.level_up_banner_timer {
            let (lvl, _, _) = self.user_profile.get_level_progress();
            let rank = self.user_profile.get_hunter_rank();
            egui::Window::new("HUNTER_SYSTEM_LEVEL_UP")
                .title_bar(false).collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -22.0])
                .min_width(360.0)
                .frame(egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgb(14, 20, 33))
                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(79, 159, 255)))
                    .rounding(10.0).inner_margin(22.0))
                .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(tr(lang, "✦ LEVEL UP ✦", "✦ مستوى جديد ✦"))
                        .size(18.0).strong().color(egui::Color32::from_rgb(88, 205, 255)));
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new(format!("LEVEL {} — {}-Rank", lvl, rank))
                        .size(26.0).color(egui::Color32::from_rgb(250, 204, 21)));
                    ui.add_space(6.0);
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
                    ui.add_space(16.0);
                    if ui.add(egui::Button::new(egui::RichText::new(tr(lang, "ONLINE", "أونلاين")).size(22.0))
                        .min_size(egui::vec2(440.0, 70.0)).fill(egui::Color32::from_rgb(24, 154, 127))).clicked() {
                        self.online_page = OnlinePage::ChooseMode;
                        self.online_message.clear();
                        self.workspace_mode = WorkspaceMode::OnlineLobby;
                        // Merely viewing the lobby must not probe the network.
                        // Networking starts only after explicit opt-in.
                    }
                });
                ui.add_space(16.0);
                if ui.button(tr(lang, "Check for Updates", "تحديث البرنامج")).clicked() {
                    self.check_for_updates(ctx.clone());
                }
                if ui.link(tr(lang, "Report a bug or suggest a feature",
                    "بلّغ عن مشكلة أو اقترح ميزة")).clicked() {
                    self.show_feedback_dialog = true;
                }
                if !self.config.recent_projects.is_empty() {
                    ui.add_space(22.0);
                    ui.separator();
                    ui.heading(tr(lang, "Recent Projects", "مشاريعك الأخيرة"));
                    for recent in self.config.recent_projects.clone().iter().take(8) {
                        let filename = std::path::Path::new(recent)
                            .file_name().and_then(|f| f.to_str())
                            .unwrap_or(recent.as_str()).to_string();
                        if ui.button(format!("📁 {}", filename))
                            .on_hover_text(recent.clone()).clicked() {
                            self.open_project_at_path(PathBuf::from(recent));
                        }
                    }
                    if ui.small_button(tr(lang, "Clear Recent List", "امسح قائمة المشاريع الأخيرة")).clicked() {
                        self.config.recent_projects.clear();
                        self.config.save();
                    }
                }
            });
            // Show version notes on the first run after an update, even
            // before the user chooses Drawing or Animation workspace.
            let mut launcher_ack = false;
            changelog::render_whats_new_modal(
                ctx, lang, &self.installed_version,
                &mut self.show_patch_notes_dialog, &mut launcher_ack,
            );
            if launcher_ack {
                self.config.last_seen_patch_version = self.installed_version.clone();
                self.config.save();
            }
            // Show the updater before mode selection too, so users do not
            // need to start a drawing project just to install a new build.
            self.render_update_dialog(ctx, lang);
            self.render_feedback_dialog(ctx);
            return;
        }

        if self.workspace_mode == WorkspaceMode::OnlineLobby {
            self.render_online_lobby(ctx);
            return;
        }

        let mut patch_acknowledged = false;
        changelog::render_whats_new_modal(
            ctx,
            lang,
            &self.installed_version,
            &mut self.show_patch_notes_dialog,
            &mut patch_acknowledged,
        );
        if patch_acknowledged {
            self.config.last_seen_patch_version = self.installed_version.clone();
            self.config.save();
            self.play_sfx(SfxType::NewFrame);
        }

        let mut typing_in_textbox = ctx.wants_keyboard_input();
        let (
            now, ctrl_z, ctrl_y, ctrl_s, ctrl_shift_s, ctrl_c, ctrl_v, ctrl_k, shift_f5, del_pressed, f10_pressed,
            space_down, ctrl_down, alt_down, zoom_in_nav, zoom_out_nav, pan_right_nav, pan_left_nav,
            scroll_y, close_req, pointer_down, pointer_pressed, pointer_released, pointer_pos,
            sc_select, sc_lasso, sc_free_tr, sc_vec_sel, sc_sculpt, sc_pen, sc_brush, sc_eraser, sc_eraser_partial,
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
                is_shortcut_pressed(i, &self.config.tool_sculpt),
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

        // Best-effort tablet pressure delivered by egui Touch events.
        // Regular mouse input and unsupported tablets safely use 1.0.
        let drawing_pressure = ctx.input(|i| self.stylus_tracker.update(&i.events, pointer_pos, pointer_down));

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
                4 => self.config.tool_sculpt = k_name,
                5 => self.config.tool_pen = k_name,
                6 => self.config.tool_brush = k_name,
                7 => self.config.tool_eraser = k_name,
                8 => self.config.tool_eraser_partial = k_name,
                9 => self.config.tool_bucket = k_name,
                10 => self.config.tool_eyedropper = k_name,
                11 => self.config.tool_text = k_name,
                12 => self.config.tool_shape = k_name,
                13 => self.config.tool_hand = k_name,
                _ => {}
            }
            self.config.save();
            self.listening_shortcut_idx = None;
            typing_in_textbox = true;
        }

        let any_modal_open = self.show_symbol_dialog || self.show_text_dialog || self.show_export_studio
            || self.show_command_palette || self.show_exit_dialog || self.show_update_dialog
            || self.show_settings_dialog || self.show_credits_dialog
            || self.show_support_dialog || self.layer_rename_index.is_some()
            || self.show_timeline_settings || self.show_project_notice
            || self.show_recovery_dialog || self.show_project_details;
        if any_modal_open { typing_in_textbox = true; }

        let old_selected_tool = self.active_tool_ui;
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
            if sc_sculpt { self.active_tool_ui = ActiveToolUI::Sculpt; }
            if sc_pen { self.active_tool_ui = ActiveToolUI::VectorPen; }
            if sc_brush { self.active_tool_ui = ActiveToolUI::Brush; }
            if sc_eraser { self.active_tool_ui = ActiveToolUI::EraserRed; }
            if sc_eraser_partial { self.active_tool_ui = ActiveToolUI::EraserGreen; }
            if sc_bucket { self.active_tool_ui = ActiveToolUI::Bucket; }
            if sc_eyedropper { self.active_tool_ui = ActiveToolUI::Eyedropper; }
            if sc_text { self.active_tool_ui = ActiveToolUI::Text; }
            if sc_shape { self.active_tool_ui = ActiveToolUI::ShapeLine; }
            if sc_hand { self.active_tool_ui = ActiveToolUI::HandCanvas; }
            if self.active_tool_ui != old_selected_tool {
                self.play_sfx(SfxType::ToolSelect(self.active_tool_ui as u8));
            }
            if sc_smaller { self.brush_size = (self.brush_size - 2.0).max(1.0); }
            if sc_larger { self.brush_size = (self.brush_size + 2.0).min(120.0); }
            if del_pressed && (!self.selected_stroke_indices.is_empty() || self.selected_text_item_index.is_some())
                && !self.is_active_layer_locked()
            {
                self.save_undo_state();
                self.ensure_editable_keyframe();
                if let Some(layer) = self.project.layers.get_mut(self.active_layer_idx) {
                    if let Some(frame) = layer.keyframes.get_mut(&self.current_frame) {
                        let mut to_rem = self.selected_stroke_indices.clone();
                        to_rem.sort_unstable_by(|a, b| b.cmp(a));
                        to_rem.dedup();
                        for idx in to_rem { if idx < frame.strokes.len() { frame.strokes.remove(idx); } }
                        if let Some(index) = self.selected_text_item_index.take() {
                            if index < frame.text_items.len() { frame.text_items.remove(index); }
                        }
                        self.selected_stroke_indices.clear();
                        self.has_unsaved_changes = true;
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
            egui::Window::new(tr(lang, "Insert Text", "إضافة نص")).collapsible(false).resizable(true)
                .default_width(440.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(tr(lang, "Type here. The preview shows correctly joined Arabic letters.",
                    "اكتب هنا؛ المعاينة بتعرض الحروف العربية متصلة بشكل صحيح."));
                ui.add(egui::TextEdit::multiline(&mut self.text_input_buf)
                    .desired_width(f32::INFINITY).desired_rows(4));
                if !self.text_input_buf.is_empty() {
                    ui.label(tr(lang, "Live text preview", "معاينة النص"));
                    ui.label(egui::RichText::new(fix_ar(&self.text_input_buf))
                        .size(self.text_font_size.clamp(14.0, 48.0))
                        .color(egui::Color32::from_rgb(236, 241, 255)));
                }
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

        if self.show_shortcuts_dialog {
            egui::Window::new(tr(lang, "Keyboard Shortcuts (F1)", "شرح الاختصارات (F1)"))
                .default_width(390.0)
                .resizable(true)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(tr(lang, "The tool names and keys below match your current settings.", "الأسماء والاختصارات دي مطابقة لإعداداتك الحالية."));
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                        for tool in TOOLBAR_ITEMS {
                            ui.horizontal(|ui| {
                                ui.label(tool_display_name(tool, lang));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.monospace(tool_shortcut(tool, &self.config));
                                });
                            });
                        }
                        ui.separator();
                        for (en, ar, key) in [
                            ("Save", "حفظ", "Ctrl+S"),
                            ("Save As", "حفظ باسم", "Ctrl+Shift+S"),
                            ("Undo", "تراجع", "Ctrl+Z"),
                            ("Redo", "إعادة", "Ctrl+Y"),
                            ("Command Palette", "لوحة الأوامر", "Ctrl+K"),
                            ("Fullscreen", "ملء الشاشة", "F10"),
                            ("Insert Frame", "إضافة فريم", self.config.insert_frame.as_str()),
                            ("Keyframe", "فريم مفتاحي", self.config.insert_keyframe.as_str()),
                            ("Blank Keyframe", "فريم مفتاحي فارغ", self.config.insert_blank_kf.as_str()),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, en, ar));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.monospace(key);
                                });
                            });
                        }
                        ui.separator();
                        ui.label(tr(lang, "Brush colors: right panel, under Colors. Onion Skin: Animation workspace, above the Timeline.", "الألوان: في اللوحة اليمنى تحت عنوان الألوان. عرض الفريمات المجاورة: في وضع الأنيميشن فوق التايم لاين."));
                    });
                    ui.separator();
                    if ui.button(tr(lang, "Close (F1)", "إغلاق (F1)")).clicked() {
                        self.show_shortcuts_dialog = false;
                    }
                });
        }

        if self.show_settings_dialog {
            egui::Window::new(tr(lang, "Stick Preferences", "إعدادات Stick"))
                .collapsible(false).resizable(true).default_width(570.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.settings_tab, 0, tr(lang, "General", "عام"));
                    ui.selectable_value(&mut self.settings_tab, 1, tr(lang, "Art System", "نظام الفنان"));
                    ui.selectable_value(&mut self.settings_tab, 2, tr(lang, "Shortcuts", "اختصارات"));
                    ui.selectable_value(&mut self.settings_tab, 3, tr(lang, "Colors & Timeline", "ألوان وتايم لاين"));
                    ui.selectable_value(&mut self.settings_tab, 4, tr(lang, "Personalization", "التخصيص"));
                    ui.selectable_value(&mut self.settings_tab, 5, tr(lang, "Previous Versions", "النسخة الاحتياطية"));
                    ui.selectable_value(&mut self.settings_tab, 6, tr(lang, "Perspective", "المنظور"));
                    ui.selectable_value(&mut self.settings_tab, 7, tr(lang, "Sign in", "تسجيل دخول"));
                });
                ui.separator();
                match self.settings_tab {
                    0 => {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.config.lang == AppLang::English, "English").clicked() {
                                self.config.lang = AppLang::English; self.config.save();
                            }
                            if ui.selectable_label(self.config.lang == AppLang::Arabic, fix_ar("العربي المصري")).clicked() {
                                self.config.lang = AppLang::Arabic; self.config.save();
                            }
                        });
                        if ui.checkbox(&mut self.config.sound_effects_enabled,
                            tr(lang, "Enable Tool & Level-Up Sound Effects", "شغّل أو اقفل أصوات الأدوات والمستويات")).changed() {
                            self.config.save();
                        }
                        if ui.checkbox(&mut self.config.show_grid,
                            tr(lang, "Show Grid", "إظهار شبكة الرسم")).changed() {
                            self.config.save();
                        }
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Auto-Save Every (minutes)", "الحفظ التلقائي كل كام دقيقة"));
                            if ui.add(egui::DragValue::new(&mut self.config.autosave_interval_minutes)
                                .range(1..=60)).changed() { self.config.save(); }
                        });
                    }
                    1 => {
                        let (lvl, cxp, rxp) = self.user_profile.get_level_progress();
                        let rank = self.user_profile.get_hunter_rank();
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(17, 23, 40))
                            .rounding(12.0)
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if let Some(avatar) = &self.artist_avatar {
                                        ui.image((avatar.id(), egui::vec2(84.0, 84.0)));
                                    } else {
                                        let (rect, _) = ui.allocate_exact_size(egui::vec2(84.0,84.0), egui::Sense::hover());
                                        ui.painter().circle_filled(rect.center(), 41.0,
                                            egui::Color32::from_rgb(59, 75, 112));
                                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "✦",
                                            egui::FontId::proportional(37.0), egui::Color32::from_rgb(139, 204, 255));
                                    }
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(tr(lang, "ARTIST PROFILE", "ملف الفنان"))
                                            .color(egui::Color32::from_rgb(119, 210, 255)).size(12.0));
                                        ui.label(egui::RichText::new(format!("LEVEL {}  •  {}-Rank", lvl, rank))
                                            .color(egui::Color32::from_rgb(255, 218, 102)).size(20.0).strong());
                                        ui.label(egui::RichText::new(tr(lang, "Create. Practice. Evolve.", "ارسم. اتدرّب. اتطوّر."))
                                            .color(egui::Color32::LIGHT_GRAY));
                                    });
                                });
                                ui.add_space(10.0);
                                ui.label(tr(lang, "Artist name", "اسم الفنان"));
                                if ui.add(egui::TextEdit::singleline(&mut self.user_profile.username)
                                    .desired_width(f32::INFINITY).char_limit(48)).changed() {
                                    self.user_profile.save();
                                    self.profile_sync_pending = true;
                                    self.last_profile_cloud_sync_time = 0.0;
                                }
                                ui.horizontal(|ui| {
                                    if ui.button(tr(lang, "Choose profile picture", "اختار صورة شخصية")).clicked() {
                                        let formats = ["png", "jpg", "jpeg", "webp", "gif", "bmp",
                                            "tiff", "tif", "tga", "ico", "pnm", "qoi", "dds", "hdr", "exr"];
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter("Supported images", &formats).pick_file() {
                                            if let Some(path_string) = path.to_str() {
                                                if let Some(avatar) = load_artist_avatar(ctx, Some(path_string)) {
                                                    self.artist_avatar = Some(avatar);
                                                    self.user_profile.avatar_path = Some(path_string.to_owned());
                                                    self.user_profile.save();
                                                    self.profile_sync_pending = true;
                                                    self.last_profile_cloud_sync_time = 0.0;
                                                } else {
                                                    self.status_message = tr(lang, "Can't decode this image format.",
                                                        "تعذر فتح الصورة. جرّب PNG أو JPG أو WebP.");
                                                }
                                            }
                                        }
                                    }
                                    if self.artist_avatar.is_some()
                                        && ui.small_button(tr(lang, "Remove", "إزالة")).clicked() {
                                        self.artist_avatar = None;
                                        self.user_profile.avatar_path = None;
                                        self.user_profile.save();
                                        self.profile_sync_pending = true;
                                        self.last_profile_cloud_sync_time = 0.0;
                                    }
                                });
                                ui.add_space(12.0);
                                ui.add(egui::ProgressBar::new((cxp as f32 / rxp.max(1) as f32)
                                    .clamp(0.0, 1.0)).text(format!("{} / {} XP", cxp, rxp)));
                                ui.label(tr(lang, "Every masterpiece begins with one frame.",
                                    "كل لوحة عظيمة بتبدأ بخط واحد."));
                            });
                    }
                    2 => {
                        ui.label(tr(lang,
                            "Select Rebind, then press your new key. The change saves automatically.",
                            "اضغط تغيير جنب الأداة، وبعدها دوس الزرار الجديد. الاختصار بيتحفظ تلقائي."));
                        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                            for (index, tool) in TOOLBAR_ITEMS.iter().copied().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(tool_display_name(tool, lang));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let is_listening = self.listening_shortcut_idx == Some(index);
                                        let caption = if is_listening {
                                            tr(lang, "Press a key...", "دوس أي زرار...")
                                        } else {
                                            format!("{}  ✎", tool_shortcut(tool, &self.config))
                                        };
                                        if ui.button(caption).clicked() {
                                            self.listening_shortcut_idx = Some(index);
                                        }
                                    });
                                });
                            }
                        });
                        if ui.button(tr(lang, "Reset Keyboard Shortcuts", "رجّع اختصارات الكيبورد الأصلية")).clicked() {
                            let d = AppConfig::default();
                            self.config.tool_select = d.tool_select;
                            self.config.tool_lasso = d.tool_lasso;
                            self.config.tool_free_transform = d.tool_free_transform;
                            self.config.tool_vector_select = d.tool_vector_select;
                            self.config.tool_pen = d.tool_pen;
                            self.config.tool_brush = d.tool_brush;
                            self.config.tool_eraser = d.tool_eraser;
                            self.config.tool_eraser_partial = d.tool_eraser_partial;
                            self.config.tool_bucket = d.tool_bucket;
                            self.config.tool_eyedropper = d.tool_eyedropper;
                            self.config.tool_text = d.tool_text;
                            self.config.tool_shape = d.tool_shape;
                            self.config.tool_hand = d.tool_hand;
                            self.listening_shortcut_idx = None;
                            self.config.save();
                        }
                    }
                    3 => {
                        ui.label(tr(lang, "Customize the interface", "خصّص شكل البرنامج"));
                        let mut colors_changed = false;
                        for (en, ar, color) in [
                            ("Panel background", "لون اللوحات", &mut self.config.ui_panel_color),
                            ("Selected accent", "لون التحديد", &mut self.config.ui_accent_color),
                            ("Timeline background", "لون خلفية التايم لاين", &mut self.config.timeline_bg_color),
                            ("Timeline cells", "لون خلايا الفريمات", &mut self.config.timeline_cell_color),
                            ("Playhead", "لون مؤشر الفريم الحالي", &mut self.config.timeline_playhead_color),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, en, ar));
                                if ui.color_edit_button_srgba_unmultiplied(color).changed() {
                                    colors_changed = true;
                                }
                            });
                        }
                        if colors_changed { self.config.save(); }
                        ui.separator();
                        if ui.button(tr(lang, "Restore Default Colors", "رجّع كل الألوان للأصل")).clicked() {
                            self.config.ui_panel_color = default_ui_panel_color();
                            self.config.ui_accent_color = default_ui_accent_color();
                            self.config.timeline_bg_color = default_tl_bg();
                            self.config.timeline_cell_color = default_tl_cell();
                            self.config.timeline_playhead_color = default_tl_head();
                            self.config.save();
                        }
                        ui.small(tr(lang, "You can also resize the Timeline by dragging its top edge.",
                            "تقدر تكبّر أو تصغّر التايم لاين بسحب حافته من فوق."));
                    }
                    4 => {
                        ui.heading(tr(lang, "Customize your workspace", "خصص مساحة شغلك"));
                        ui.small(tr(lang, "Hide panels to maximize canvas space. Your choices are saved.",
                            "اخفي اللي مش محتاجه عشان تكبر مساحة الرسم. الاختيارات بتتحفظ."));
                        let mut changed = false;
                        ui.separator();
                        ui.strong(tr(lang,"Tool cursor personalization","تخصيص مؤشر أدوات الرسم"));
                        changed |= ui.add(egui::Slider::new(&mut self.config.brush_cursor_scale,0.4..=2.5)
                            .text(tr(lang,"Brush cursor size","حجم مؤشر الفرشة"))).changed();
                        changed |= ui.add(egui::Slider::new(&mut self.config.tool_icon_scale,0.6..=2.0)
                            .text(tr(lang,"Tool icon under cursor","حجم أيقونة الأداة جنب الماوس"))).changed();
                        changed |= ui.checkbox(&mut self.config.icon_cursor_ring,
                            tr(lang,"Show circle around tool cursor icon","إظهار الدائرة حول أيقونة الأداة")).changed();
                        ui.separator();
                        changed |= ui.checkbox(&mut self.config.show_left_dock,
                            tr(lang, "Left tools panel", "قائمة الأدوات على الشمال")).changed();
                        changed |= ui.checkbox(&mut self.config.show_right_dock,
                            tr(lang, "Right properties and layers panel", "الخصائص والطبقات على اليمين")).changed();
                        changed |= ui.checkbox(&mut self.config.show_timeline_dock,
                            tr(lang, "Timeline in animation mode", "التايم لاين في وضع الأنيميشن")).changed();
                        changed |= ui.checkbox(&mut self.config.show_top_bar,
                            tr(lang, "Top toolbar", "الشريط العلوي")).changed();
                        if ui.button(tr(lang, "Restore standard workspace", "رجّع شكل البرنامج الافتراضي")).clicked() {
                            self.config.show_left_dock = true;
                            self.config.show_right_dock = true;
                            self.config.show_top_bar = true;
                            self.config.show_timeline_dock = true;
                            changed = true;
                        }
                        if changed { self.config.save(); }
                        ui.small(tr(lang, "When the top bar is hidden, use the floating gear to reopen settings.",
                            "لو خفيت الشريط العلوي، تقدر تفتح الإعدادات من الترس العائم."));
                    }
                    5 => {
                        ui.heading(tr(lang, "Restore a previous Stick version", "الرجوع لإصدار سابق من Stick"));
                        ui.label(tr(lang,
                            "Official GitHub releases only. The old version is downloaded, hash-checked, and installed when Stick restarts.",
                            "الإصدارات الرسمية فقط من GitHub. هننزّل النسخة القديمة ونتحقق منها قبل استبدال البرنامج."));
                        ui.separator();
                        let snapshot = self.rollback_state.lock().unwrap().clone();
                        if matches!(snapshot, RollbackState::Idle) {
                            self.refresh_release_history(ctx.clone());
                        } else {
                            if ui.button(tr(lang, "Refresh release list", "تحديث قائمة الإصدارات")).clicked() {
                                self.refresh_release_history(ctx.clone());
                            }
                            match snapshot {
                                RollbackState::Checking => {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(tr(lang, "Loading the release history...", "بنحمّل سجل الإصدارات من GitHub..."));
                                    });
                                }
                                RollbackState::Available(releases) => {
                                    if releases.is_empty() {
                                        ui.label(tr(lang, "No verified older releases available.",
                                            "مفيش إصدارات أقدم متاحة ومتحقق منها."));
                                    } else {
                                        ui.add_space(8.0);
                                        ui.label(egui::RichText::new(tr(lang, "Back to previous version", "الرجوع للنسخة السابقة")).strong());
                                        let previous = &releases[0];
                                        ui.horizontal(|ui| {
                                            ui.label(format!("Stick v{}   •   {}", previous.info.version,
                                                previous.published_at.get(0..10).unwrap_or("?")));
                                            if ui.button(tr(lang, "Restore previous", "ارجع للنسخة السابقة")).clicked() {
                                                self.rollback_choice = Some(previous.info.clone());
                                            }
                                        });
                                        ui.add_space(10.0);
                                        ui.label(egui::RichText::new(tr(lang,
                                            "Five versions before the previous release", "آخر 5 نسخ قبل النسخة السابقة")).strong());
                                        egui::ScrollArea::vertical().max_height(205.0).show(ui, |ui| {
                                            for (index, release) in releases.iter().skip(1).enumerate() {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("{}. v{}", index + 1, release.info.version));
                                                    ui.label(release.published_at.get(0..10).unwrap_or("?"));
                                                    if ui.small_button(tr(lang, "Select", "اختيار")).clicked() {
                                                        self.rollback_choice = Some(release.info.clone());
                                                    }
                                                });
                                            }
                                        });
                                    }
                                }
                                RollbackState::Error(error) => {
                                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                                    ui.hyperlink_to(tr(lang, "Open GitHub release history", "افتح إصدارات GitHub"),
                                        "https://github.com/KaniEGY/Stick/releases");
                                }
                                RollbackState::Idle => {}
                            }
                        }
                        if let Some(release) = self.rollback_choice.clone() {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.colored_label(egui::Color32::from_rgb(246, 190, 100),
                                tr(lang, "Older Stick builds may not support projects saved by newer versions. Back up your projects first.",
                                    "تحذير: النسخ القديمة ممكن متفتحش المشاريع المحفوظة بإصدارات أحدث. احفظ نسخة احتياطية من مشاريعك الأول."));
                            ui.label(format!("v{}  →  v{}", self.installed_version, release.version));
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "Confirm download and restore", "تأكيد تنزيل واسترجاع الإصدار")).clicked() {
                                    self.rollback_choice = None;
                                    self.show_settings_dialog = false;
                                    self.show_update_dialog = true;
                                    self.download_and_apply_update(release, ctx.clone());
                                }
                                if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                                    self.rollback_choice = None;
                                }
                            });
                        }
                    }
                    6 => {
                        ui.heading(tr(lang, "Drawing guides / Perspective", "أدوات المنظور وخطوط الإرشاد"));
                        ui.small(tr(lang,
                            "These are non-destructive on-screen guides. They follow canvas rotation and zoom and do not become part of exported artwork.",
                            "الخطوط دي إرشادية على الشاشة فقط، بتتحرك مع الكانفا ومش بتدخل في ملف الرسم أو التصدير."));
                        ui.horizontal_wrapped(|ui| {
                            ui.selectable_value(&mut self.config.perspective_mode, 0, tr(lang, "Off", "إيقاف"));
                            ui.selectable_value(&mut self.config.perspective_mode, 1, tr(lang, "Square grid", "مربعات"));
                            ui.selectable_value(&mut self.config.perspective_mode, 2, tr(lang, "Parallel lines", "خطوط متساوية"));
                            ui.selectable_value(&mut self.config.perspective_mode, 3, tr(lang, "1-point perspective", "منظور بنقطة تلاشي"));
                            ui.selectable_value(&mut self.config.perspective_mode, 4, tr(lang, "2-point perspective", "منظور بنقطتين تلاشي"));
                        });
                        ui.separator();
                        let mut changed = false;
                        changed |= ui.add(egui::Slider::new(&mut self.config.perspective_spacing, 8.0..=300.0)
                            .text(tr(lang, "Grid size / line spacing", "حجم المربعات / مسافة الخطوط"))).changed();
                        changed |= ui.add(egui::Slider::new(&mut self.config.perspective_line_width, 0.5..=5.0)
                            .text(tr(lang, "Line thickness", "سمك الخط"))).changed();
                        changed |= ui.add(egui::Slider::new(&mut self.config.perspective_opacity, 0.02..=1.0)
                            .text(tr(lang, "Opacity", "الشفافية"))).changed();
                        ui.horizontal(|ui| {
                            ui.label(tr(lang, "Guide color", "لون الخطوط"));
                            changed |= ui.color_edit_button_srgba_unmultiplied(&mut self.config.perspective_color).changed();
                        });
                        if self.config.perspective_mode == 3 {
                            changed |= ui.add(egui::Slider::new(&mut self.config.vanishing_x, 0.05..=0.95)
                                .text(tr(lang, "Vanishing X", "نقطة التلاشي أفقي"))).changed();
                            changed |= ui.add(egui::Slider::new(&mut self.config.vanishing_y, 0.05..=0.85)
                                .text(tr(lang, "Vanishing Y", "نقطة التلاشي رأسي"))).changed();
                        } else if self.config.perspective_mode == 4 {
                            changed |= ui.add(egui::Slider::new(&mut self.config.vanishing_left_x, 0.02..=0.47)
                                .text(tr(lang, "Left vanishing point X", "نقطة التلاشي الأولى أفقي"))).changed();
                            changed |= ui.add(egui::Slider::new(&mut self.config.vanishing_right_x, 0.53..=0.98)
                                .text(tr(lang, "Right vanishing point X", "نقطة التلاشي الثانية أفقي"))).changed();
                            changed |= ui.add(egui::Slider::new(&mut self.config.vanishing_2point_y, 0.07..=0.93)
                                .text(tr(lang, "Horizon height", "ارتفاع خط الأفق"))).changed();
                            ui.small(tr(lang,
                                "Adjust both points independently. The guides are never exported.",
                                "حرّك كل نقطة تلاشي بشكل مستقل. الخطوط إرشادية ومش بتظهر في التصدير."));
                        }
                        if ui.button(tr(lang, "Reset perspective settings", "رجّع إعدادات المنظور الأصلية")).clicked() {
                            self.config.perspective_spacing = default_perspective_spacing();
                            self.config.perspective_line_width = default_perspective_width();
                            self.config.perspective_color = default_perspective_color();
                            self.config.perspective_opacity = default_perspective_opacity();
                            self.config.vanishing_x = default_vanishing_x();
                            self.config.vanishing_y = default_vanishing_y();
                            self.config.vanishing_left_x = default_left_vp_x();
                            self.config.vanishing_right_x = default_right_vp_x();
                            self.config.vanishing_2point_y = default_two_point_y();
                            self.config.perspective_mode = 0;
                            changed = true;
                        }
                        if changed { self.config.save(); }
                    }
                    7 => {
                        ui.heading(tr(lang,"Google account & Drive backup",
                            "حساب جوجل والحفظ السحابي"));
                        ui.label(tr(lang,
                            "Connect through Google's official browser login. Stick never asks for your Google password.",
                            "سجّل دخولك من متصفح جوجل الرسمي. Stick مش بيطلب كلمة المرور."));
                        let oauth_ready = cloud_sync::oauth_client_is_configured(
                            self.config.cloud_google_client_id.trim());
                        if !oauth_ready {
                            // Short lines keep legacy Arabic visual shaping from
                            // being corrupted by line wrapping in the settings tab.
                            ui.colored_label(egui::Color32::from_rgb(255,194,88),
                                tr(lang, "Google sign-in is not configured for this build.",
                                    "تسجيل الدخول بجوجل لسه مش متفعّل في النسخة دي."));
                            ui.small(tr(lang,
                                "Developer: enable Drive API and configure Desktop OAuth in Google Cloud.",
                                "المطوّر لازم يفعّل Google Drive API ويجهّز OAuth Desktop."));
                            ui.small(tr(lang,
                                "Then set STICK_GOOGLE_CLIENT_ID in GitHub Actions and publish a new build.",
                                "وبعدين يضيف STICK_GOOGLE_CLIENT_ID في GitHub وينشر نسخة جديدة."));
                        }
                        ui.collapsing(tr(lang,"Developer OAuth setup","إعداد المطوّر لجوجل"),|ui|{
                            if ui.add(egui::TextEdit::singleline(&mut self.config.cloud_google_client_id)
                                .desired_width(380.0)
                                .hint_text("...apps.googleusercontent.com")).changed(){
                                self.config.save();
                            }
                            ui.hyperlink_to(tr(lang,"Google Cloud credentials","Google Cloud OAuth"),
                                "https://console.cloud.google.com/apis/credentials");
                            ui.small(tr(lang,"Enable Google Drive API and create Desktop OAuth client. No client secret is needed in Stick.",
                                "فعّل Google Drive API واعمل OAuth Client من نوع Desktop. مش مطلوب تحفظ Client Secret في Stick."));
                        });
                        ui.add_space(8.0);
                        if let Some(id)=self.user_profile.cloud_account_id.clone(){
                            ui.colored_label(egui::Color32::LIGHT_GREEN,
                                format!("{}: {}",tr(lang,"Linked account","الحساب المرتبط"),
                                    if self.config.cloud_email.is_empty(){"Google"}else{self.config.cloud_email.as_str()}));
                            ui.small(tr(lang,"Backups are stored in a private Google Drive app-data folder. Only saved project files are uploaded.",
                                "النسخ محفوظة بشكل خاص في مساحة التطبيق على Google Drive، والمشاريع اللي اتحفظت محليًا بس هي اللي بتترفع."));
                            if ui.checkbox(&mut self.config.cloud_auto_backup,
                                tr(lang,"Automatically back up projects and artist progress",
                                    "احفظ المشاريع ومستوى الفنان تلقائيًا على جوجل درايف")).changed(){
                                self.config.save();
                                if self.config.cloud_auto_backup {
                                    self.profile_sync_pending = true;
                                    self.last_profile_cloud_sync_time = 0.0;
                                }
                            }
                            ui.horizontal_wrapped(|ui|{
                                if ui.add_enabled(self.cloud_job.is_none(),
                                    egui::Button::new(tr(lang,"Back up level and saved projects",
                                        "احفظ المستوى والمشاريع المحفوظة سحابيًا"))).clicked(){
                                    self.start_cloud_backup();
                                }
                                if ui.add_enabled(self.cloud_job.is_none(),
                                    egui::Button::new(tr(lang,"Browse cloud backups","عرض النسخ السحابية"))).clicked(){
                                    let client=self.config.cloud_google_client_id.clone();
                                    let account=id.clone();
                                    self.cloud_job=Some(cloud_sync::spawn_job(move||{
                                        cloud_sync::list_cloud(&client,&account)
                                            .map(cloud_sync::Event::Listed)
                                    }));
                                    self.cloud_message=tr(lang,"Loading Google Drive backups...","جاري قراءة النسخ...").to_owned();
                                }
                            });
                            ui.add_space(8.0);
                            if ui.add_enabled(self.cloud_job.is_none(),
                                egui::Button::new(tr(lang,"Sign out of Google","تسجيل الخروج من جوجل"))).clicked(){
                                match cloud_sync::sign_out(&id){
                                    Ok(())=>{
                                        self.user_profile.cloud_account_id=None;
                                        self.user_profile.save();
                                        self.config.cloud_email.clear();
                                        self.config.save();
                                        self.cloud_files.clear();
                                        self.profile_sync_pending = false;
                                        self.cloud_message=tr(lang,"Google account disconnected on this computer.",
                                            "تم فصل حساب جوجل عن الجهاز. النسخ الموجودة في Drive لم تُحذف.").to_owned();
                                    }
                                    Err(err)=>self.cloud_message=err
                                }
                            }
                        } else if ui.add_enabled(self.cloud_job.is_none() && oauth_ready,
                            egui::Button::new(tr(lang,"Sign in with Google","تسجيل الدخول باستخدام جوجل")))
                                .on_hover_text(tr(lang,
                                    "Requires a valid Google Desktop OAuth Client ID; no passwords are collected.",
                                    "لازم Google OAuth Client ID سليم من نوع Desktop؛ مش بنطلب كلمة المرور."))
                                .clicked(){
                            let client=self.config.cloud_google_client_id.trim().to_string();
                            self.cloud_job=Some(cloud_sync::spawn_job(move||{
                                cloud_sync::sign_in(&client).map(cloud_sync::Event::SignedIn)
                            }));
                            self.cloud_message=tr(lang,"Complete Google sign-in in your browser...",
                                "كمّل تسجيل الدخول في متصفح جوجل...").to_string();
                        }
                        if self.cloud_job.is_some(){ui.spinner();}
                        if !self.cloud_message.is_empty(){
                            ui.label(self.cloud_message.as_str());
                        }
                        if !self.cloud_files.is_empty(){
                            ui.separator();
                            ui.strong(tr(lang,"Available backups","النسخ السحابية المتاحة"));
                            egui::ScrollArea::vertical().max_height(215.0).show(ui,|ui|{
                                for file in self.cloud_files.clone().into_iter().take(100){
                                    ui.horizontal(|ui|{
                                        ui.label(format!("{} ({})",file.name,file.modified));
                                        if ui.add_enabled(self.cloud_job.is_none(),
                                            egui::Button::new(tr(lang,"Restore copy","استعادة نسخة"))).clicked(){
                                            self.cloud_restore_confirmation=Some(file);
                                        }
                                    });
                                }
                            });
                        }
                        if let Some(file)=self.cloud_restore_confirmation.clone(){
                            ui.separator();
                            ui.colored_label(egui::Color32::from_rgb(255,204,105),
                                tr(lang,"Restore from Google Drive? The original local project will NOT be overwritten.",
                                    "تستعيد النسخة دي؟ ملف المشروع المحلي الأصلي مش هيتم استبداله."));
                            ui.small(file.name.as_str());
                            ui.horizontal(|ui|{
                                if ui.button(tr(lang,"Confirm restore","تأكيد الاستعادة")).clicked(){
                                    if let Some(id)=self.user_profile.cloud_account_id.clone(){
                                        let client=self.config.cloud_google_client_id.clone();
                                        self.cloud_job=Some(cloud_sync::spawn_job(move||{
                                            cloud_sync::restore(&client,&id,file)
                                        }));
                                        self.cloud_message=tr(lang,"Restoring cloud copy...",
                                            "جاري استعادة النسخة السحابية...").to_owned();
                                    }
                                    self.cloud_restore_confirmation=None;
                                }
                                if ui.button(tr(lang,"Cancel","إلغاء")).clicked(){
                                    self.cloud_restore_confirmation=None;
                                }
                            });
                        }
                    }
                    _ => {}
                }
                ui.add_space(8.0);
                ui.separator();
                if ui.button(tr(lang, "Done", "تمام")).clicked() {
                    self.listening_shortcut_idx = None;
                    self.show_settings_dialog = false;
                }
            });
        }

        if self.show_credits_dialog {
            egui::Window::new(tr(lang, "About Stick's Creator", "عن صاحب Stick"))
                .collapsible(false).resizable(false).default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.heading(tr(lang, "Stick Studio — made by Hady", "Stick Studio — من تطوير هادي"));
                ui.separator();
                ui.hyperlink_to(tr(lang, "YouTube Channel", "قناة يوتيوب"), YOUTUBE_CHANNEL_URL);
                ui.hyperlink_to(tr(lang, "Instagram", "إنستجرام"), INSTAGRAM_URL);
                if ui.button(tr(lang, "Close", "إغلاق")).clicked() { self.show_credits_dialog = false; }
            });
        }
        // Escape is always an emergency exit for the support dialog.
        if self.show_support_dialog && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_support_dialog = false;
        }
        if self.show_support_dialog {
            let mut open = true;
            let mut close_clicked = false;
            // A bounded, fixed dialog avoids egui's unconstrained RTL child
            // layout stretching the popup to the full viewport height.
            egui::Window::new(tr(lang, "Support Stick Studio", "ادعم Stick Studio"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .fixed_size(egui::vec2(525.0, 305.0))
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("♥").size(23.0)
                            .color(egui::Color32::from_rgb(255, 207, 64)));
                        ui.heading(tr(lang, "Support is completely optional",
                            "الدعم اختياري تمامًا"));
                    });
                    ui.separator();
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(34, 37, 49))
                        .rounding(7.0)
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_source("stick_support_text_scroll")
                                .max_height(168.0)
                                .show(ui, |ui| {
                                    for (index, line) in support_message_lines(lang).into_iter().enumerate() {
                                        let rich = egui::RichText::new(line)
                                            .size(15.0)
                                            .color(egui::Color32::from_rgb(238, 237, 245));
                                        if lang == AppLang::Arabic {
                                            // Explicit dimensions prevent each RTL row from
                                            // consuming all remaining window vertical space.
                                            ui.allocate_ui_with_layout(
                                                egui::vec2(475.0, 22.0),
                                                egui::Layout::right_to_left(egui::Align::Min),
                                                |ui| { ui.add(egui::Label::new(rich)); },
                                            );
                                        } else {
                                            ui.add(egui::Label::new(rich));
                                        }
                                        if index == 3 { ui.add_space(5.0); }
                                    }
                                });
                        });
                    ui.add_space(5.0);
                    ui.label("🙂  🥳");
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.hyperlink_to(
                            tr(lang, "Optional InstaPay reward", "رابط المكافأة على إنستا باي"),
                            SUPPORT_URL,
                        );
                        if ui.button(tr(lang, "Close", "إغلاق")).clicked() {
                            close_clicked = true;
                        }
                    });
                });
            self.show_support_dialog = open && !close_clicked;
        }

        self.render_update_dialog(ctx, lang);
        self.render_feedback_dialog(ctx);

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
                // Never block window teardown on an audio device/worker.
                // The old wait_finish=true could keep Windows waiting forever.
                self.leave_online_session();
                self.allow_close = true;
            }
        }

        if self.show_exit_dialog {
            egui::Window::new("Exit").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr(lang,"Save & Exit","احفظ واخرج")).clicked() && self.trigger_save() {
                        self.show_exit_dialog=false;
                        self.allow_close = true;
                        self.leave_online_session();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(tr(lang,"Exit without saving","اخرج من غير حفظ")).clicked() {
                        self.show_exit_dialog=false;
                        self.allow_close = true;
                        self.leave_online_session();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(tr(lang,"Cancel","إلغاء")).clicked(){self.show_exit_dialog=false;}
                });
            });
        }

        // Online status appears above the editor independently of customizable toolbars.
        if let Some(session) = self.online_session.as_ref() {
            let title = session.title.clone();
            let address = session.address.clone();
            let code = session.room_code.clone();
            let host = session.is_host;
            let mut leave = false;
            let mut resync = false;
            egui::TopBottomPanel::top("online_room_status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(60, 210, 160), tr(lang, "ONLINE", "أونلاين"));
                    ui.label(format!("{}  •  {} online", title, self.online_members));
                    if host {
                        ui.colored_label(egui::Color32::from_rgb(245, 193, 82),
                            tr(lang, "HOST: Your PC is the room server", "هوست: جهازك هو سيرفر الغرفة"));
                        ui.label(format!("LAN {}", address));
                        if let Some(ip) = self.online_remote_info.as_ref().and_then(|info| info.address) {
                            let friends_address = format!("{}:{}", ip, collab::TCP_PORT);
                            if ui.button(format!("Friends {} ⧉", friends_address))
                                .on_hover_text(tr(lang, "Copy Tailscale address for friends", "انسخ عنوان Tailscale لأصحابك")).clicked() {
                                ui.output_mut(|out| out.copied_text = friends_address);
                            }
                        }
                        if ui.button(format!("Room #{}  ⧉", code)).on_hover_text(
                            tr(lang, "Copy room code", "نسخ كود الغرفة")).clicked() {
                            ui.output_mut(|out| out.copied_text = code.clone());
                        }
                        ui.menu_button(tr(lang, "Host controls", "إدارة الغرفة"), |ui| {
                            ui.set_min_width(300.0);
                            ui.label(egui::RichText::new(tr(lang,
                                "Host permissions • participants and tool colors",
                                "صلاحيات الهوست • الأعضاء وألوان الأدوات")).strong());
                            ui.separator();
                            let palette: [[u8; 3]; 8] = [
                                [64,185,250], [247,115,87], [159,125,248], [47,207,147],
                                [240,186,54], [229,98,184], [76,200,210], [248,102,135],
                            ];
                            for participant in self.online_participants.clone() {
                                ui.horizontal(|ui| {
                                    let color = egui::Color32::from_rgb(
                                        participant.color[0], participant.color[1], participant.color[2]);
                                    ui.colored_label(color, format!("● {}  Lv.{}",
                                        participant.name, participant.level));
                                    if participant.id == 0 {
                                        ui.small(tr(lang, "HOST", "الهوست"));
                                    } else if ui.small_button(tr(lang, "Kick", "طرد")).clicked() {
                                        if let Err(err) = session.kick_member(participant.id) {
                                            self.online_message = err;
                                        }
                                        ui.close_menu();
                                    }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    ui.small(tr(lang, "Tool color:", "لون الأدوات:"));
                                    for rgb in palette {
                                        let swatch = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                        let (rect, click) = ui.allocate_exact_size(
                                            egui::vec2(19.0, 19.0), egui::Sense::click());
                                        ui.painter().rect_filled(rect, 3.0, swatch);
                                        if participant.color == rgb {
                                            ui.painter().rect_stroke(rect.expand(1.0), 3.0,
                                                egui::Stroke::new(2.0, egui::Color32::WHITE));
                                        }
                                        if click.clicked() {
                                            if let Err(err) = session.change_member_color(participant.id, rgb) {
                                                self.online_message = err;
                                            }
                                        }
                                    }
                                });
                                ui.separator();
                            }
                            ui.small(tr(lang, "The oldest joined guest becomes host when you leave normally. A sudden PC crash cannot guarantee a handoff.",
                                "أول ضيف دخل الغرفة يستلم الهوست لما تخرج طبيعي. لو الجهاز فصل فجأة، النقل مش مضمون."));
                        });
                        if ui.small_button(tr(lang, "Sync full project", "مزامنة المشروع بالكامل")).clicked() { resync = true; }
                    }
                    if ui.small_button(tr(lang, "Leave Room", "مغادرة الغرفة")).clicked() { leave = true; }
                });
            });
            if resync {
                if let Some(session) = self.online_session.as_ref() {
                    session.resync_from_host(self.project.clone());
                }
            }
            if leave {
                self.leave_online_session();
                self.online_participants.clear();
                self.online_enabled = false;
                self.online_discovery = None;
                self.online_remote_info = None;
                self.online_rooms.clear();
                self.workspace_mode = WorkspaceMode::Launcher;
            }
        }
        // الشريط العلوي
        if self.config.show_top_bar {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let (h_rect, h_resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
                draw_red_heart_icon(ui.painter(), h_rect, h_resp.hovered());
                if h_resp.clicked() { self.show_credits_dialog = true; }
                ui.colored_label(egui::Color32::from_rgb(255, 215, 25), "Stick");
                ui.separator();

                let m_lbl = if self.workspace_mode == WorkspaceMode::DrawingOnly { tr(lang, "Switch to Animation", "التبديل للأنيميشن") } else { tr(lang, "Switch to Drawing", "التبديل للرسم") };
                if ui.add_enabled(self.online_session.is_none(), egui::Button::new(m_lbl)).clicked() {
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
                if ui.button(tr(lang, "? Shortcuts (F1)", "؟ الاختصارات (F1)")).clicked() {
                    self.show_shortcuts_dialog = true;
                }
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
                if ui.button(tr(lang, "Check for Updates", "تحديث البرنامج")).clicked() { self.check_for_updates(ctx.clone()); }
                if ui.link(egui::RichText::new(
                    tr(lang, "Feedback", "ابلغ عن مشكلة"))
                    .color(egui::Color32::from_rgb(80, 190, 255))).clicked() {
                    self.show_feedback_dialog = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (gr, gresp) = ui.allocate_exact_size(egui::vec2(34.0, 27.0), egui::Sense::click());
                    draw_gear_icon(ui.painter(), gr, 1.18);
                    let gresp = gresp.on_hover_text(tr(lang, "Settings • Google sign-in, appearance and shortcuts",
                        "الإعدادات • تسجيل جوجل والمظهر والاختصارات"));
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

        }
        if !self.config.show_top_bar {
            egui::Area::new(egui::Id::new("restore_workspace_settings"))
                .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if ui.button("⚙").on_hover_text(tr(lang, "Workspace settings", "إعدادات مساحة العمل"))
                        .clicked() {
                        self.show_settings_dialog = true;
                        self.settings_tab = 4;
                    }
                });
        }
        // شريط أدوات ثنائي اللغة: اسم دائم بجوار كل أيقونة واختصار فعلي من الإعدادات.
        if self.config.show_left_dock {
        egui::SidePanel::left("tools_panel").resizable(true).default_width(210.0).min_width(180.0).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.strong(tr(lang, "Drawing Tools", "أدوات الرسم"));
                ui.add_space(5.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for tool in TOOLBAR_ITEMS {
                        let selected = self.active_tool_ui == tool;
                        let name = tool_display_name(tool, lang);
                        let key = tool_shortcut(tool, &self.config);
                        let width = ui.available_width().max(170.0);
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(width, 34.0), egui::Sense::click()
                        );
                        let bg = if selected {
                            egui::Color32::from_rgb(34, 56, 70)
                        } else if response.hovered() {
                            egui::Color32::from_rgb(62, 62, 68)
                        } else {
                            egui::Color32::from_rgb(44, 44, 48)
                        };
                        ui.painter().rect_filled(rect, 6.0, bg);
                        if selected {
                            ui.painter().rect_stroke(rect.shrink(0.7),6.0,
                                egui::Stroke::new(1.6,get_tool_ui_color(tool)));
                        }
                        if self.online_session.is_some() {
                            if let Some(member) = self.online_participants.iter().find(|p|
                                p.name == self.user_profile.username) {
                                let rgb = member.color;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_max(
                                        rect.right_top()+egui::vec2(-4.0,4.0),
                                        rect.right_bottom()+egui::vec2(-2.0,-4.0)),
                                    2.0,egui::Color32::from_rgb(rgb[0],rgb[1],rgb[2]));
                            }
                        }
                        let icon_rect = egui::Rect::from_min_size(
                            rect.left_top() + egui::vec2(5.0, 2.0), egui::vec2(30.0, 30.0)
                        );
                        draw_tool_ui_icon(ui.painter(), icon_rect, tool, selected);
                        ui.painter().text(
                            rect.left_center() + egui::vec2(43.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name.clone(),
                            egui::FontId::proportional(14.0),
                            egui::Color32::WHITE,
                        );
                        let shortcut_hint = format!("{}: {}", tr(lang, "Shortcut", "الاختصار"), key);
                        let response = response.on_hover_text(format!("{} — {}", name, shortcut_hint));
                        response.context_menu(|ui| {
                            ui.strong(tr(lang, "Quick tool options", "اختيارات الأداة السريعة"));
                            ui.separator();
                            match tool {
                                ActiveToolUI::Sculpt => {
                                    ui.add(egui::Slider::new(&mut self.sculpt_radius,8.0..=360.0)
                                        .text(tr(lang,"Radius","حجم الصباع")));
                                    ui.add(egui::Slider::new(&mut self.sculpt_strength,0.05..=1.0)
                                        .text(tr(lang,"Strength","القوة")));
                                }
                                ActiveToolUI::VectorDirectSelect | ActiveToolUI::VectorPen => {
                                    ui.label(tr(lang,"Editable Vector Nodes","نقاط الفيكتور القابلة للتعديل"));
                                    ui.add(egui::Slider::new(&mut self.vector_tolerance,0.4..=8.0)
                                        .text(tr(lang,"Simplify amount","تقليل النقط")));
                                    if ui.button(tr(lang,"Simplify selected strokes","قلل نقاط الخطوط المحددة")).clicked(){
                                        self.save_undo_state();
                                        self.ensure_editable_keyframe();
                                        if let Some(layer)=self.project.layers.get_mut(self.active_layer_idx){
                                            if let Some(frame)=layer.keyframes.get_mut(&self.current_frame){
                                                for (idx,stroke) in frame.strokes.iter_mut().enumerate(){
                                                    if stroke.is_vector && (self.selected_stroke_indices.is_empty()
                                                        || self.selected_stroke_indices.contains(&idx)){
                                                        stroke.simplify_vector_nodes(self.vector_tolerance);
                                                        self.has_unsaved_changes=true;
                                                    }
                                                }
                                            }
                                        }
                                        self.selected_vector_nodes.clear();
                                        self.selected_vector_node=None;
                                        ui.close_menu();
                                    }
                                }
                                ActiveToolUI::ShapeLine => {
                                    for (kind, en, ar) in [
                                        (GeometricShapeKind::StraightLine, "Line", "خط"),
                                        (GeometricShapeKind::Circle, "Circle", "دائرة"),
                                        (GeometricShapeKind::Square, "Square", "مربع"),
                                        (GeometricShapeKind::Rectangle, "Rectangle", "مستطيل"),
                                        (GeometricShapeKind::Triangle, "Triangle", "مثلث"),
                                        (GeometricShapeKind::Pentagon, "Pentagon", "خماسي"),
                                    ] {
                                        if ui.selectable_label(self.selected_shape_kind == kind, tr(lang, en, ar)).clicked() {
                                            self.selected_shape_kind = kind;
                                            self.active_tool_ui = ActiveToolUI::ShapeLine;
                                            ui.close_menu();
                                        }
                                    }
                                }
                                ActiveToolUI::Brush => {
                                    for (preset, label) in [
                                        (KritaBrushPreset::GPenInk, "G-Pen"),
                                        (KritaBrushPreset::Watercolor, "Watercolor"),
                                        (KritaBrushPreset::ChiselMarker, "Marker"),
                                        (KritaBrushPreset::SoftAirbrush, "Airbrush"),
                                        (KritaBrushPreset::SketchPencil, "Pencil"),
                                    ] {
                                        if ui.selectable_label(self.active_krita_brush == preset, label).clicked() {
                                            self.active_krita_brush = preset;
                                            self.brush_opacity = preset.default_opacity();
                                            self.active_tool_ui = ActiveToolUI::Brush;
                                            ui.close_menu();
                                        }
                                    }
                                }
                                ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen => {
                                    ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=120.0)
                                        .text(tr(lang, "Size", "الحجم")));
                                }
                                ActiveToolUI::HandCanvas => {
                                    if ui.button(tr(lang, "Reset canvas rotation", "تصفير زاوية اللوحة")).clicked() {
                                        self.canvas_rotation_deg = 0.0;
                                        self.canvas_flip_h = false;
                                        self.canvas_flip_v = false;
                                        ui.close_menu();
                                    }
                                }
                                _ => {
                                    if ui.button(tr(lang, "Open tool properties", "افتح خصائص الأداة")).clicked() {
                                        self.active_tool_ui = tool;
                                        self.config.show_right_dock = true;
                                        self.config.save();
                                        ui.close_menu();
                                    }
                                }
                            }
                        });
                        if response.clicked() {
                            self.active_tool_ui = tool;
                            self.play_sfx(SfxType::ToolSelect(tool as u8));
                        }
                        ui.add_space(3.0);
                    }
                });
                ui.separator();
                if ui.small_button(tr(lang, "? Keyboard Shortcuts (F1)", "؟ شرح الاختصارات (F1)")).clicked() {
                    self.show_shortcuts_dialog = true;
                }
                ui.label(tr(lang, "Current color", "اللون الحالي"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            });
        });

        }
        // Split dock: tools keep their own scrolling upper section; layers
        // remain in a dedicated lower section, like a painting editor.
        let mut layer_menu_action: Option<(usize, layer_ops::LayerAction)> = None;
        let mut copy_layer_index: Option<usize> = None;
        let mut paste_layer_index: Option<usize> = None;
        let mut rename_layer_index: Option<usize> = None;
        if self.config.show_right_dock {
        egui::SidePanel::right("right_panel").resizable(true).default_width(376.0)
            .min_width(285.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(tr(lang, "Tool Properties", "خصائص الأداة"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Slider::new(&mut self.config.right_panel_split, 0.18..=0.85)
                        .show_value(false).text(tr(lang, "Split", "تقسيم"))).changed() {
                        self.config.save();
                    }
                });
            });
            let panel_height = ui.available_height();
            let tool_height = ((panel_height - 44.0).max(180.0)
                * self.config.right_panel_split.clamp(0.18, 0.85)).max(110.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), tool_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical().id_salt("tool_properties_scroll")
                        .auto_shrink([false, false]).show(ui, |ui| {
                            self.render_tool_properties_ui(ui, lang);
            ui.add_space(8.0);
            ui.separator();
            egui::CollapsingHeader::new(tr(lang, "Colors", "الألوان"))
                .default_open(true).show(ui, |ui| {
            ui.label(tr(lang, "Click a swatch or use the custom color picker:", "اختار لون جاهز أو افتح منتقي الألوان:"));
            ui.horizontal_wrapped(|ui| {
                for (rgb, en_name) in BASIC_COLORS {
                    let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    let selected = self.brush_color[0..3] == rgb;
                    let button = egui::Button::new(" ")
                        .fill(color)
                        .stroke(if selected {
                            egui::Stroke::new(2.0, egui::Color32::YELLOW)
                        } else {
                            egui::Stroke::new(1.0, egui::Color32::GRAY)
                        })
                        .min_size(egui::vec2(27.0, 27.0));
                    if ui.add(button).on_hover_text(en_name).clicked() {
                        self.brush_color = [rgb[0], rgb[1], rgb[2], 255];
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr(lang, "Custom color:", "لون مخصص:"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
            });
            });
            ui.add_space(8.0);
            ui.separator();
            egui::CollapsingHeader::new(tr(lang, "Symmetry Mirroring", "الرسم المتماثل"))
                .default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Off, "Off");
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Horizontal, "H");
                ui.selectable_value(&mut self.symmetry_mode, SymmetryMode::Vertical, "V");
            });
            });
            ui.add_space(6.0);
                        }); // independent tool-props scroll
                }); // adjustable upper dock
            // A genuine draggable splitter between Properties and the Layers dock.
            let (handle_rect, handle) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 13.0), egui::Sense::click_and_drag());
            ui.painter().rect_filled(handle_rect, 3.0,
                if handle.hovered() || handle.dragged() {
                    egui::Color32::from_rgb(57, 96, 119)
                } else { egui::Color32::from_rgb(47, 50, 59) });
            for dx in [-11.0,0.0,11.0] {
                ui.painter().circle_filled(handle_rect.center()+egui::vec2(dx,0.0),
                    1.8,egui::Color32::from_gray(186));
            }
            if handle.dragged() {
                self.config.right_panel_split = (self.config.right_panel_split
                    + handle.drag_delta().y / panel_height.max(250.0)).clamp(0.18, 0.85);
                self.config.save();
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr(lang, "LAYERS", "الطبقات")).size(16.0)
                    .strong().color(egui::Color32::from_rgb(115, 208, 244)));
                ui.label(format!("({})", self.project.layers.len()));
            });
            ui.add(egui::TextEdit::singleline(&mut self.layer_filter)
                .hint_text(tr(lang, "Search layers...", "ابحث عن لير..."))
                .desired_width(f32::INFINITY));
            egui::ScrollArea::vertical().id_salt("layer_dock_scroll")
                .auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.small_button(tr(lang, "+ New Layer", "+ لير جديد")).clicked() {
                    self.save_undo_state();
                    let mut kf = HashMap::new();
                    kf.insert(1, Default::default());
                    self.project.layers.insert(0, Layer { name: format!("Layer_{}", self.project.layers.len() + 1), visible: true, locked: false, is_guide: false, xp_first_paint_rewarded: false, opacity: 1.0, keyframes: kf });
                    self.active_layer_idx = 0;
                    self.has_unsaved_changes = true;
                }
                if ui.small_button(tr(lang, "Duplicate", "نسخ اللير"))
                    .on_hover_text(tr(lang, "Duplicate selected layer", "اعمل نسخة من اللير")).clicked() {
                    layer_menu_action = Some((self.active_layer_idx, layer_ops::LayerAction::Duplicate));
                }
                if ui.add_enabled(self.active_layer_idx > 0, egui::Button::new("↑"))
                    .on_hover_text(tr(lang, "Move Layer Up", "حرّك اللير لفوق")).clicked() {
                    layer_menu_action = Some((self.active_layer_idx, layer_ops::LayerAction::MoveUp));
                }
                if ui.add_enabled(self.active_layer_idx + 1 < self.project.layers.len(), egui::Button::new("↓"))
                    .on_hover_text(tr(lang, "Move Layer Down", "حرّك اللير لتحت")).clicked() {
                    layer_menu_action = Some((self.active_layer_idx, layer_ops::LayerAction::MoveDown));
                }
            });
            if let Some(al) = self.project.layers.get_mut(self.active_layer_idx) {
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Layer Opacity", "شفافية اللير"));
                    if ui.add(egui::Slider::new(&mut al.opacity, 0.0..=1.0)).changed() {
                        self.has_unsaved_changes = true;
                    }
                });
            }
            let can_delete_layers = self.project.layers.len() > 1;
            let dims = (self.project.width.max(1) as f32, self.project.height.max(1) as f32);
            let current_frame = self.current_frame;
            let filter = self.layer_filter.trim().to_lowercase();
            for (idx, layer) in self.project.layers.iter_mut().enumerate() {
                if !filter.is_empty() && !layer.name.to_lowercase().contains(&filter) { continue; }
                let selected_row = self.active_layer_idx == idx;
                egui::Frame::none()
                    .fill(if selected_row { egui::Color32::from_rgb(34, 65, 90) }
                        else { egui::Color32::from_rgb(37, 39, 46) })
                    .stroke(egui::Stroke::new(if selected_row { 1.4 } else { 0.5 },
                        if selected_row { egui::Color32::from_rgb(78, 183, 235) }
                        else { egui::Color32::from_gray(67) }))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(6.0, 7.0))
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let has_art = layer.keyframes.values().any(|frame|
                        !frame.strokes.is_empty() || !frame.text_items.is_empty() || !frame.media_items.is_empty());
                    // Live thumbnail based on the layer's stored drawing strokes.
                    let (thumb, _) = ui.allocate_exact_size(egui::vec2(48.0, 38.0), egui::Sense::hover());
                    ui.painter().rect_filled(thumb, 3.0, egui::Color32::from_gray(82));
                    let inner = thumb.shrink(3.0);
                    ui.painter().rect_filled(inner, 2.0, egui::Color32::from_gray(236));
                    if let Some((_, data)) = layer.keyframes.iter()
                        .filter(|(f,_)| **f <= current_frame).max_by_key(|(f,_)| **f) {
                        for stroke in data.strokes.iter().take(70) {
                            let color = egui::Color32::from_rgb(stroke.color[0], stroke.color[1], stroke.color[2]);
                            let project_to_thumb = |p: &StrokePoint| egui::pos2(
                                inner.left() + (p.x / dims.0).clamp(0.0,1.0)*inner.width(),
                                inner.top() + (p.y / dims.1).clamp(0.0,1.0)*inner.height());
                            if stroke.points.len() == 1 {
                                let p = &stroke.points[0];
                                if p.x.is_finite() && p.y.is_finite() {
                                    ui.painter().circle_filled(project_to_thumb(p), 1.4, color);
                                }
                            } else {
                                for pair in stroke.points.windows(2).step_by((stroke.points.len()/28).max(1)) {
                                    if pair.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                                        ui.painter().line_segment([project_to_thumb(&pair[0]),
                                            project_to_thumb(&pair[1])], egui::Stroke::new(1.1, color));
                                    }
                                }
                            }
                        }
                        if !data.media_items.is_empty() || !data.text_items.is_empty() {
                            ui.painter().text(inner.center(), egui::Align2::CENTER_CENTER,
                                "▧", egui::FontId::proportional(13.0),
                                egui::Color32::from_rgb(40, 110, 161));
                        }
                    }
                    if !has_art {
                        ui.painter().circle_stroke(inner.center(), 5.0,
                            egui::Stroke::new(1.0, egui::Color32::from_gray(150)));
                    }
                    if ui.checkbox(&mut layer.visible, "")
                        .on_hover_text(tr(lang, "Show or hide layer", "أظهر أو اخفي اللير")).changed() {
                        self.has_unsaved_changes = true;
                    }
                    if ui.small_button(if layer.locked { "🔒" } else { "🔓" })
                        .on_hover_text(tr(lang, "Lock or unlock layer", "اقفل أو افتح قفل اللير")).clicked() {
                        layer.locked = !layer.locked;
                        self.has_unsaved_changes = true;
                    }
                    let layer_response = ui.vertical(|ui| {
                        let response = ui.selectable_label(selected_row,
                            egui::RichText::new(&layer.name).strong().size(14.0));
                        let stroke_count: usize = layer.keyframes.values().map(|f| f.strokes.len()).sum();
                        ui.label(egui::RichText::new(format!("{} {} • {} {}{}",
                            layer.keyframes.len(), tr(lang, "frames", "فريم"),
                            stroke_count, tr(lang, "strokes", "خط"),
                            if layer.is_guide { " • GUIDE" } else { "" }))
                            .small().color(egui::Color32::from_gray(160)));
                        response
                    }).inner;
                    if layer_response.clicked() {
                        self.active_layer_idx = idx;
                        self.selected_stroke_indices.clear();
                    }
                    if layer_response.double_clicked() { rename_layer_index = Some(idx); }
                    layer_response.context_menu(|ui| {
                        ui.strong(&layer.name);
                        ui.separator();
                        if ui.button(tr(lang, "Rename", "غيّر الاسم")).clicked() {
                            rename_layer_index = Some(idx); ui.close_menu();
                        }
                        if ui.button(tr(lang, "Copy Layer", "انسخ اللير")).clicked() {
                            copy_layer_index = Some(idx); ui.close_menu();
                        }
                        if ui.add_enabled(self.layer_clipboard.is_some(),
                            egui::Button::new(tr(lang, "Paste Layer", "الصق اللير"))).clicked() {
                            paste_layer_index = Some(idx); ui.close_menu();
                        }
                        if ui.button(tr(lang,
                            if layer.is_guide { "Disable Guide Layer" } else { "Make Guide Layer (not exported)" },
                            if layer.is_guide { "إلغاء وضع الإرشاد" } else { "لير إرشادي (مش بيتصدر)" })).clicked() {
                            layer.is_guide = !layer.is_guide;
                            self.has_unsaved_changes = true;
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Duplicate", "دبلكيت اللير")).clicked() {
                            layer_menu_action = Some((idx, layer_ops::LayerAction::Duplicate));
                            ui.close_menu();
                        }
                        if ui.add_enabled(can_delete_layers && !layer.locked,
                            egui::Button::new(tr(lang, "Delete Layer", "احذف اللير"))).clicked() {
                            self.pending_delete_layer = Some(idx); ui.close_menu();
                        }
                    });
                    if ui.add_enabled(can_delete_layers && !layer.locked,
                        egui::Button::new("×"))
                        .on_hover_text(tr(lang, "Delete Layer", "احذف اللير")).clicked() {
                        self.pending_delete_layer = Some(idx);
                    }
                });
                });
                ui.add_space(2.0);
            }
                // Current room roster lives under Layers in the right dock.
                if self.online_session.is_some() {
                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(egui::RichText::new(tr(lang, "ROOM MEMBERS", "الناس اللي في الغرفة"))
                        .strong().color(egui::Color32::from_rgb(103, 208, 239)));
                    ui.small(tr(lang, "Colors are unique per participant. Levels are self-reported by each PC.",
                        "كل مشارك له لون مميز؛ المستويات بيبلغ بيها الجهاز ومش موثّقة بسيرفر."));
                    for participant in &self.online_participants {
                        ui.horizontal(|ui| {
                            let c = participant.color;
                            let color = egui::Color32::from_rgb(c[0],c[1],c[2]);
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(13.0,13.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(),6.0,color);
                            ui.colored_label(color, &participant.name);
                            ui.label(format!("LVL {}",participant.level));
                        });
                    }
                    ui.add_space(6.0);
                }
                }); // scrollable layer dock
        });
        }
        if let Some(idx) = copy_layer_index {
            self.layer_clipboard = self.project.layers.get(idx).cloned();
        }
        if let Some(idx) = paste_layer_index {
            if let Some(mut cloned) = self.layer_clipboard.clone() {
                self.save_undo_state();
                cloned.name = format!("{} Copy", cloned.name);
                cloned.xp_first_paint_rewarded = false;
                cloned.locked = false;
                self.project.layers.insert(idx.min(self.project.layers.len()), cloned);
                self.active_layer_idx = idx.min(self.project.layers.len() - 1);
                self.selected_stroke_indices.clear();
                self.has_unsaved_changes = true;
            }
        }
        if let Some(idx) = rename_layer_index {
            if let Some(layer) = self.project.layers.get(idx) {
                self.layer_rename_buffer = layer.name.clone();
                self.layer_rename_index = Some(idx);
            }
        }
        if let Some(idx) = self.layer_rename_index {
            egui::Window::new(tr(lang, "Rename Layer", "غيّر اسم اللير"))
                .collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.layer_rename_buffer)
                        .desired_width(300.0));
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                            self.layer_rename_index = None;
                        }
                        if ui.button(tr(lang, "Rename", "غيّر الاسم")).clicked() {
                            let new_name = self.layer_rename_buffer.trim().to_owned();
                            if !new_name.is_empty() && new_name.chars().count() <= 80 {
                                self.save_undo_state();
                                if let Some(layer) = self.project.layers.get_mut(idx) {
                                    layer.name = new_name;
                                    self.has_unsaved_changes = true;
                                }
                                self.layer_rename_index = None;
                            }
                        }
                    });
                });
        }
        if let Some(index) = self.pending_delete_layer {
            let layer_name = self.project.layers.get(index).map(|l| l.name.clone()).unwrap_or_default();
            egui::Window::new(tr(lang, "Delete Layer?", "عايز تحذف اللير؟"))
                .collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("{}: {}", tr(lang, "Layer", "اللير"), layer_name));
                    ui.label(tr(lang, "This removes the whole layer. You can undo it.",
                        "ده هيمسح اللير كله. وتقدر ترجع بالتراجع."));
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Cancel", "إلغاء")).clicked() {
                            self.pending_delete_layer = None;
                        }
                        if ui.button(tr(lang, "Delete Layer", "احذف اللير")).clicked() {
                            layer_menu_action = Some((index, layer_ops::LayerAction::Delete));
                            self.pending_delete_layer = None;
                        }
                    });
                });
        }
        if let Some((index, action)) = layer_menu_action {
            self.save_undo_state();
            match layer_ops::apply_layer_action(&mut self.project, index, action) {
                Ok(new_index) => {
                    self.active_layer_idx = new_index;
                    self.selected_stroke_indices.clear();
                    self.has_unsaved_changes = true;
                }
                Err(reason) => {
                    self.undo_stack.pop();
                    self.status_message = reason.to_owned();
                }
            }
        }

        let mut frame_menu_action: Option<(usize, usize, timeline_ops::FrameAction)> = None;
        let mut timeline_visibility_toggle: Option<usize> = None;
        let mut timeline_lock_toggle: Option<usize> = None;
        // شريط التايم لاين السفلي في وضع الأنيميشن
        if self.workspace_mode == WorkspaceMode::AnimationStudio && self.config.show_timeline_dock {
            // Explicit height and an interior drag grip prevent the timeline from
            // snapping back when the window is redrawn or reopened.
            let available = (ctx.screen_rect().height() - 160.0).max(180.0);
            egui::TopBottomPanel::bottom("timeline_panel").resizable(false)
                .exact_height(self.config.timeline_dock_height.clamp(140.0, available))
                .show(ctx, |ui| {
                let (handle_rect, handle) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 13.0), egui::Sense::drag());
                ui.painter().rect_filled(handle_rect, 0.0,
                    if handle.hovered() || handle.dragged() {
                        egui::Color32::from_rgb(56, 91, 120)
                    } else { egui::Color32::from_rgb(50, 52, 61) });
                for dx in [-19.0,-9.0,1.0,11.0] {
                    ui.painter().line_segment(
                        [handle_rect.center()+egui::vec2(dx,-2.0),
                         handle_rect.center()+egui::vec2(dx+7.0,-2.0)],
                        egui::Stroke::new(2.0, egui::Color32::from_gray(163)));
                }
                if handle.dragged() {
                    let delta = ui.input(|i| i.pointer.delta().y);
                    if delta.abs() > 0.1 {
                        self.config.timeline_dock_height =
                            (self.config.timeline_dock_height - delta).clamp(140.0,available);
                        self.config.save();
                    }
                }
                let bg = self.config.timeline_bg_color;
                ui.painter().rect_filled(ui.max_rect(), 0.0,
                    egui::Color32::from_rgb(bg[0],bg[1],bg[2]));
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(tr(lang, "● ANIMATION TIMELINE", "● الخط الزمني للأنيميشن"))
                        .size(16.0).strong().color(egui::Color32::from_rgb(107, 210, 252)));
                    if ui.button("⚙").on_hover_text(
                        tr(lang, "Timeline Settings", "إعدادات التايم لاين")
                    ).clicked() {
                        self.show_timeline_settings = true;
                    }
                    ui.separator();
                    if ui.button("⏮").on_hover_text(tr(lang, "First frame", "أول فريم")).clicked() { self.current_frame = 1; }
                    if ui.button("◀").on_hover_text(tr(lang, "Previous frame", "الفريم السابق")).clicked() && self.current_frame > 1 { self.current_frame -= 1; }
                    let play_button = egui::Button::new(if self.is_playing { "Ⅱ  PAUSE" } else { "▶  PLAY" })
                        .fill(if self.is_playing { egui::Color32::from_rgb(163, 59, 62) }
                            else { egui::Color32::from_rgb(21, 133, 111) });
                    if ui.add(play_button).clicked() {
                        self.is_playing = !self.is_playing;
                        if self.is_playing { self.start_audio_playback(); } else { self.stop_audio_playback(); }
                    }
                    if ui.button("▶").on_hover_text(tr(lang, "Next frame", "الفريم الجاي")).clicked() && self.current_frame < self.timeline_length { self.current_frame += 1; }
                    ui.toggle_value(&mut self.loop_enabled, tr(lang, "🔁 Loop Range", "🔁 تكرار جزء"));
                    ui.label(egui::RichText::new(format!("Frame {} / {}", self.current_frame, self.timeline_length))
                        .color(egui::Color32::from_rgb(255, 214, 110)).strong());
                    if ui.button(tr(lang, "+ Frame (F5)", "+ فريم (F5)")).clicked() { self.action_insert_frame_f5(); }
                    if ui.button(tr(lang, "◆ Keyframe (F6)", "◆ كي فريم (F6)")).clicked() { self.action_insert_keyframe_f6(now_time); }
                    if ui.button(tr(lang, "◇ Blank (F7)", "◇ فاضي (F7)")).clicked() { self.action_insert_blank_keyframe_f7(now_time); }
                    if ui.checkbox(&mut self.project.onion_skin.enabled, tr(lang, "Onion Skin", "عرض الفريمات المجاورة"))
                        .on_hover_text(tr(lang, "Show faded previous and next frames to help animate.", "إظهار الفريمات السابقة والتالية بشفافية لتسهيل التحريك."))
                        .changed()
                    {
                        self.has_unsaved_changes = true;
                    }
                });
                if self.loop_enabled {
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Repeat from", "كرر من فريم"));
                        ui.add(egui::DragValue::new(&mut self.loop_start_frame).range(1..=self.timeline_length.max(1)));
                        ui.label(tr(lang, "to", "لحد"));
                        ui.add(egui::DragValue::new(&mut self.loop_end_frame).range(1..=self.timeline_length.max(1)));
                        if ui.small_button(tr(lang, "Start = current", "البداية = الحالي")).clicked() {
                            self.loop_start_frame = self.current_frame;
                        }
                        if ui.small_button(tr(lang, "End = current", "النهاية = الحالي")).clicked() {
                            self.loop_end_frame = self.current_frame;
                        }
                    });
                    if self.loop_start_frame > self.loop_end_frame {
                        self.loop_end_frame = self.loop_start_frame;
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Timeline zoom", "تكبير التايم لاين"));
                    if self.config.timeline_style == 1 {
                        if ui.add(egui::Slider::new(&mut self.config.timeline_new_cell_width,
                            28.0..=110.0).show_value(false)).changed() { self.config.save(); }
                    } else {
                        ui.add(egui::Slider::new(&mut self.timeline_cell_width, 16.0..=84.0)
                            .show_value(false));
                    }
                    if ui.add(egui::Slider::new(&mut self.config.timeline_dock_height, 140.0..=850.0)
                        .text(tr(lang, "Panel height", "ارتفاع التايم لاين"))).changed() {
                        self.config.save();
                    }
                    if ui.small_button(tr(lang, "+24 frames", "زوّد 24 فريم")).clicked() {
                        self.save_undo_state();
                        self.timeline_length = self.timeline_length.saturating_add(24).min(20_000);
                        self.project.total_frames = self.timeline_length;
                        self.has_unsaved_changes = true;
                    }
                });
                let modern = self.config.timeline_style == 1;
                let cell_w = if modern {
                    self.config.timeline_new_cell_width.clamp(28.0, 110.0)
                } else { self.timeline_cell_width };
                let label_w = if modern { 218.0 } else { 198.0 };
                let header_h = if modern { 39.0 } else { 30.0 };
                let row_h = if modern {
                    self.config.timeline_new_row_height.clamp(36.0, 76.0)
                } else { 34.0 };
                let cols = self.timeline_length.max(30);
                egui::ScrollArea::both().id_salt("timeline_frame_scroller").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(label_w, header_h), |ui| {
                            ui.strong(egui::RichText::new(
                                if modern { tr(lang, "TRACKS / EXPOSURES", "الطبقات / الفريمات") }
                                else { tr(lang, "LAYERS  /  FRAMES", "الطبقات  /  الفريمات") })
                                .color(egui::Color32::from_rgb(117, 196, 241)));
                        });
                        for f in 1..=cols {
                            let is_c = self.current_frame == f;
                            let (r, resp) = ui.allocate_exact_size(
                                egui::vec2(cell_w, header_h), egui::Sense::click_and_drag());
                            if modern {
                                timeline_styles::draw_header(ui.painter(), r, f, self.project.fps,
                                    is_c, self.loop_enabled
                                        && f >= self.loop_start_frame && f <= self.loop_end_frame,
                                    self.config.timeline_show_timecodes);
                            } else {
                                if self.loop_enabled && f >= self.loop_start_frame && f <= self.loop_end_frame {
                                    ui.painter().rect_filled(r.shrink(1.0), 2.0, egui::Color32::from_rgb(57, 103, 83));
                                }
                                if is_c {
                                    let h = self.config.timeline_playhead_color;
                                    ui.painter().rect_filled(r, 3.0,
                                        egui::Color32::from_rgb(h[0], h[1], h[2]));
                                }
                                if is_c || f == 1 || f % 5 == 0 {
                                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{}", f),
                                        egui::FontId::proportional(11.0), egui::Color32::WHITE);
                                }
                            }
                            if resp.clicked() || (pointer_down && resp.hovered()) {
                                if f > self.timeline_length { self.timeline_length = f; self.project.total_frames = f; }
                                self.current_frame = f;
                            }
                        }
                    });
                    for (l_idx, layer) in self.project.layers.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(label_w, row_h), |ui| {
                                ui.horizontal(|ui| {
                                    if ui.small_button(if layer.visible { "◉" } else { "○" })
                                        .on_hover_text(tr(lang, "Toggle visibility", "إظهار أو إخفاء اللير"))
                                        .clicked() { timeline_visibility_toggle = Some(l_idx); }
                                    if ui.small_button(if layer.locked { "▣" } else { "▢" })
                                        .on_hover_text(tr(lang, "Toggle layer lock", "قفل أو فتح اللير"))
                                        .clicked() { timeline_lock_toggle = Some(l_idx); }
                                    if ui.selectable_label(self.active_layer_idx == l_idx,
                                        egui::RichText::new(&layer.name).strong().size(12.5)
                                            .color(if self.active_layer_idx == l_idx {
                                                egui::Color32::from_rgb(113, 209, 255)
                                            } else { egui::Color32::WHITE }))
                                        .on_hover_text(&layer.name).clicked() {
                                        self.active_layer_idx = l_idx;
                                    }
                                });
                            });
                            for f in 1..=cols {
                                let is_b = f > self.timeline_length;
                                let is_kf = !is_b && layer.keyframes.contains_key(&f);
                                let eff = if is_b { None } else { Self::get_effective_frame_data(layer, f) };
                                let has_d = eff.map_or(false, |fd| !fd.strokes.is_empty()
                                    || !fd.media_items.is_empty() || !fd.text_items.is_empty()
                                    || !fd.symbol_instances.is_empty());
                                let tw = eff.map(|fd| fd.tween_mode).unwrap_or(TweenMode::None);
                                let (cr, cresp) = ui.allocate_exact_size(egui::vec2(cell_w, row_h), egui::Sense::click());
                                let cell = self.config.timeline_cell_color;
                                let accent = self.config.ui_accent_color;
                                let bg = if self.active_layer_idx == l_idx && self.current_frame == f {
                                    egui::Color32::from_rgb(accent[0], accent[1], accent[2])
                                } else if has_d {
                                    egui::Color32::from_rgb(
                                        cell[0].saturating_add(30),
                                        cell[1].saturating_add(30),
                                        cell[2].saturating_add(30))
                                } else {
                                    egui::Color32::from_rgb(cell[0], cell[1], cell[2])
                                };
                                if modern {
                                    // Continuous exposure strips and diamonds replace the
                                    // boxed legacy frame grid. Editing actions below stay shared.
                                    timeline_styles::draw_cell(ui.painter(), cr, is_kf,
                                        has_d && !is_b, tw,
                                        self.active_layer_idx == l_idx,
                                        layer.locked || is_b,
                                        self.current_frame == f);
                                } else {
                                ui.painter().rect_filled(cr.shrink2(egui::vec2(1.0, 2.0)), 4.0, bg);
                                if self.current_frame == f {
                                    ui.painter().line_segment(
                                        [cr.left_top() + egui::vec2(0.0, 1.0),
                                         cr.left_bottom() - egui::vec2(0.0, 1.0)],
                                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 202, 65))
                                    );
                                }
                                if !is_b { draw_anicraft_timeline_cell(ui.painter(), cr, is_kf, has_d, tw); }
                                }
                                if cresp.clicked() {
                                    if is_b { self.timeline_length = f; self.project.total_frames = f; }
                                    self.active_layer_idx = l_idx;
                                    self.current_frame = f;
                                }
                                let is_locked = layer.locked;
                                let can_delete_global = self.timeline_length > 1
                                    && self.project.layers.iter().all(|l| !l.locked);
                                cresp.context_menu(|ui| {
                                    ui.strong(format!("{} — {}", layer.name, f));
                                    ui.separator();
                                    if ui.button(tr(lang, "Copy Frame", "انسخ الفريم")).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::Copy));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(!is_locked && self.clipboard_frame.is_some(),
                                        egui::Button::new(tr(lang, "Paste Frame", "الصق الفريم"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::Paste));
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.add_enabled(!is_locked, egui::Button::new(
                                        tr(lang, "Create Keyframe", "اعمل كي فريم"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::Keyframe));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(!is_locked, egui::Button::new(
                                        tr(lang, "Blank Keyframe", "كي فريم فاضي"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::BlankKeyframe));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(!is_locked, egui::Button::new(
                                        tr(lang, "Clear Frame", "امسح محتوى الفريم"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::Clear));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(!is_locked && layer.keyframes.contains_key(&f),
                                        egui::Button::new(tr(lang, "Delete Keyframe", "احذف الكي فريم"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::DeleteKeyframe));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(can_delete_global, egui::Button::new(
                                        tr(lang, "Delete Time (all layers)", "احذف زمن الفريم من كل الليرز"))).clicked() {
                                        frame_menu_action = Some((l_idx, f, timeline_ops::FrameAction::DeleteFrame));
                                        ui.close_menu();
                                    }
                                    if !can_delete_global {
                                        ui.small(tr(lang, "Unlock layers before deleting time.", "افتح قفل الليرز عشان تحذف الزمن."));
                                    }
                                });
                            }
                        });
                    }
                });
            });
            // The grip and the height slider both modify the same persisted value.
        }
        if let Some(idx) = timeline_visibility_toggle {
            self.save_undo_state();
            if let Some(layer) = self.project.layers.get_mut(idx) {
                layer.visible = !layer.visible;
                self.has_unsaved_changes = true;
            }
        }
        if let Some(idx) = timeline_lock_toggle {
            self.save_undo_state();
            if let Some(layer) = self.project.layers.get_mut(idx) {
                layer.locked = !layer.locked;
                self.has_unsaved_changes = true;
            }
        }

        if self.show_timeline_settings && self.workspace_mode == WorkspaceMode::AnimationStudio {
            let mut open = self.show_timeline_settings;
            egui::Window::new(tr(lang, "⚙ Timeline Preferences", "⚙ إعدادات التايم لاين"))
                .open(&mut open).resizable(true).default_width(430.0)
                .show(ctx, |ui| {
                    ui.heading(tr(lang, "Timeline style", "شكل التايم لاين"));
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        changed |= ui.selectable_value(&mut self.config.timeline_style, 0,
                            "OLD Timeline").changed();
                        changed |= ui.selectable_value(&mut self.config.timeline_style, 1,
                            "NEW Timeline").changed();
                    });
                    ui.small(if self.config.timeline_style == 0 {
                        tr(lang, "Legacy Stick frame grid: unchanged.",
                            "OLD: نفس شبكة الفريمات الحالية بدون أي تغيير.")
                    } else {
                        tr(lang,
                            "NEW: exposure-sheet tracks, keyframe diamonds and a time ruler. Both layouts edit the same project.",
                            "NEW: صفوف أوضح، كي فريمات ملوّنة ومسطرة توقيت. الشكلين بيعدلوا نفس المشروع.")
                    });
                    if self.config.timeline_style == 1 {
                        changed |= ui.add(egui::Slider::new(
                            &mut self.config.timeline_new_row_height, 36.0..=76.0)
                            .text(tr(lang, "Track row height", "ارتفاع صف اللير"))).changed();
                        changed |= ui.checkbox(&mut self.config.timeline_show_timecodes,
                            tr(lang, "Timecode labels", "عرض التوقيت بدل رقم الفريم")).changed();
                    }
                    if changed { self.config.save(); }
                    ui.separator();
                    ui.heading(tr(lang, "Onion Skin", "الأونين سكين"));
                    if ui.checkbox(&mut self.project.onion_skin.enabled,
                        tr(lang, "Enable Onion Skin", "تشغيل عرض الفريمات المجاورة")).changed() {
                        self.has_unsaved_changes = true;
                    }
                    ui.separator();
                    ui.strong(tr(lang, "Previous frames", "الفريمات اللي ورا"));
                    if ui.add(egui::Slider::new(&mut self.project.onion_skin.frames_before, 0..=12)
                        .text(tr(lang, "Count", "العدد"))).changed() { self.has_unsaved_changes = true; }
                    let mut prev_alpha = self.project.onion_skin.color_before[3] as f32 / 255.0;
                    if ui.add(egui::Slider::new(&mut prev_alpha, 0.0..=1.0)
                        .text(tr(lang, "Opacity", "الشفافية"))).changed() {
                        self.project.onion_skin.color_before[3] = (prev_alpha * 255.0).round() as u8;
                        self.has_unsaved_changes = true;
                    }
                    ui.label(tr(lang, "Ghost tint", "لون الفريمات القديمة"));
                    if ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_before).changed() {
                        self.has_unsaved_changes = true;
                    }
                    ui.separator();
                    ui.strong(tr(lang, "Next frames", "الفريمات اللي قدام"));
                    if ui.add(egui::Slider::new(&mut self.project.onion_skin.frames_after, 0..=12)
                        .text(tr(lang, "Count", "العدد"))).changed() { self.has_unsaved_changes = true; }
                    let mut next_alpha = self.project.onion_skin.color_after[3] as f32 / 255.0;
                    if ui.add(egui::Slider::new(&mut next_alpha, 0.0..=1.0)
                        .text(tr(lang, "Opacity", "الشفافية"))).changed() {
                        self.project.onion_skin.color_after[3] = (next_alpha * 255.0).round() as u8;
                        self.has_unsaved_changes = true;
                    }
                    ui.label(tr(lang, "Ghost tint", "لون الفريمات الجاية"));
                    if ui.color_edit_button_srgba_unmultiplied(&mut self.project.onion_skin.color_after).changed() {
                        self.has_unsaved_changes = true;
                    }
                    ui.separator();
                    ui.label(tr(lang, "Timeline cell width", "عرض الفريم في التايم لاين"));
                    if self.config.timeline_style == 1 {
                        if ui.add(egui::Slider::new(&mut self.config.timeline_new_cell_width,
                            28.0..=110.0)).changed() { self.config.save(); }
                    } else {
                        ui.add(egui::Slider::new(&mut self.timeline_cell_width, 20.0..=90.0));
                    }
                    ui.small(tr(lang, "Drag the Timeline's top border to resize it vertically.",
                        "اسحب الحافة اللي فوق للتايم لاين عشان تكبره أو تصغّره."));
                });
            self.show_timeline_settings = open;
        }

        if let Some((layer_index, frame_index, action)) = frame_menu_action {
            match action {
                timeline_ops::FrameAction::Copy => self.copy_frame(layer_index, frame_index),
                timeline_ops::FrameAction::Paste => {
                    if self.project.layers.get(layer_index).is_some_and(|layer| !layer.locked) {
                        self.paste_frame(layer_index, frame_index);
                    }
                }
                edit => {
                    // Save a single undo snapshot; only one frame command
                    // executes per context-menu selection.
                    self.save_undo_state();
                    match timeline_ops::apply_frame_edit(&mut self.project, layer_index, frame_index, edit) {
                        Ok(()) => {
                            self.timeline_length = self.project.total_frames;
                            self.current_frame = frame_index.min(self.timeline_length);
                            self.active_layer_idx = layer_index.min(self.project.layers.len().saturating_sub(1));
                            self.loop_end_frame = self.loop_end_frame.min(self.timeline_length).max(1);
                            self.loop_start_frame = self.loop_start_frame.min(self.loop_end_frame).max(1);
                            self.has_unsaved_changes = true;
                        }
                        Err(reason) => {
                            self.undo_stack.pop();
                            self.status_message = reason.to_string();
                        }
                    }
                }
            }
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
            let custom_brush_cursor = matches!(self.active_tool_ui,
                ActiveToolUI::Brush | ActiveToolUI::VectorPen | ActiveToolUI::Sculpt
                    | ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen);
            let show_dot = is_inside_stage && !any_modal_open && !space_down && !alt_down && custom_brush_cursor;
            let show_tool_icon = is_inside_stage && !any_modal_open && !space_down
                && !alt_down && !custom_brush_cursor && self.active_tool_ui != ActiveToolUI::HandCanvas;
            let tool_cursor = if show_dot || show_tool_icon {
                egui::CursorIcon::None
            } else if space_down || self.active_tool_ui == ActiveToolUI::HandCanvas {
                if pointer_down || ctx.input(|i| i.pointer.secondary_down()) {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                }
            } else {
                match self.active_tool_ui {
                    ActiveToolUI::Text => egui::CursorIcon::Text,
                    ActiveToolUI::Bucket | ActiveToolUI::ShapeLine
                        | ActiveToolUI::VectorDirectSelect | ActiveToolUI::FreeTransform
                        | ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso
                        | ActiveToolUI::Eyedropper => egui::CursorIcon::Crosshair,
                    _ => egui::CursorIcon::Default,
                }
            };
            ctx.set_cursor_icon(tool_cursor);

            let is_over_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));
            if scroll_y != 0.0 && is_over_canvas && !any_modal_open {
                let old_zoom = self.zoom_level;
                let new_zoom = (old_zoom * (1.0 + scroll_y * 0.0018).clamp(0.82, 1.22)).clamp(0.1, 6.0);
                if let Some(pointer) = pointer_pos {
                    self.pan_offset = selection_tools::zoom_pan_around_pointer(
                        self.pan_offset, canvas_rect.center(), pointer, old_zoom, new_zoom,
                    );
                }
                self.zoom_level = new_zoom;
            }
            if (space_down || self.active_tool_ui == ActiveToolUI::HandCanvas) && pointer_down && !any_modal_open {
                if let (Some(c), Some(l)) = (pointer_pos, self.last_pointer_pos) { self.pan_offset += c - l; }
            }
            // With the hand tool, left drag pans; right drag rotates the entire stage view.
            let secondary_down = ctx.input(|i| i.pointer.secondary_down());
            if self.active_tool_ui == ActiveToolUI::HandCanvas && secondary_down && is_over_canvas && !any_modal_open {
                if let (Some(current), Some(previous)) = (pointer_pos, self.last_secondary_pointer_pos) {
                    let angle = self.canvas_rotation_deg + (current.x - previous.x) * 0.45;
                    self.canvas_rotation_deg = (angle + 180.0).rem_euclid(360.0) - 180.0;
                }
                self.last_secondary_pointer_pos = pointer_pos;
            } else {
                self.last_secondary_pointer_pos = None;
            }

            let bg = self.stage_bg_color;
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
            // Rotate the paper along with the artwork, rather than rotating
            // only the strokes over an unrotated white rectangle.
            painter.add(egui::Shape::convex_polygon(
                vec![to_screen(egui::pos2(0.0, 0.0)),
                     to_screen(egui::pos2(stage_w, 0.0)),
                     to_screen(egui::pos2(stage_w, stage_h)),
                     to_screen(egui::pos2(0.0, stage_h))],
                egui::Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]),
                egui::Stroke::NONE,
            ));

            // Non-destructive drawing guides; composed in stage coordinates and transformed
            // with the actual paper. They do not affect project strokes or exports.
            if self.config.perspective_mode > 0 {
                let step = self.config.perspective_spacing.clamp(8.0, 300.0);
                let lw = self.config.perspective_line_width.clamp(0.5, 5.0);
                let gc = self.config.perspective_color;
                let alpha = ((gc[3] as f32) * self.config.perspective_opacity.clamp(0.02, 1.0))
                    .round() as u8;
                let paint = egui::Stroke::new(lw,
                    egui::Color32::from_rgba_unmultiplied(gc[0], gc[1], gc[2], alpha));
                match self.config.perspective_mode {
                    1 => {
                        for n in 1..=((stage_w / step).ceil() as usize).min(500) {
                            let x = n as f32 * step;
                            if x < stage_w {
                                painter.line_segment([to_screen(egui::pos2(x, 0.0)),
                                    to_screen(egui::pos2(x, stage_h))], paint);
                            }
                        }
                        for n in 1..=((stage_h / step).ceil() as usize).min(500) {
                            let y = n as f32 * step;
                            if y < stage_h {
                                painter.line_segment([to_screen(egui::pos2(0.0, y)),
                                    to_screen(egui::pos2(stage_w, y))], paint);
                            }
                        }
                    }
                    2 => {
                        for n in 1..=((stage_h / step).ceil() as usize).min(500) {
                            let y = n as f32 * step;
                            if y < stage_h {
                                painter.line_segment([to_screen(egui::pos2(0.0, y)),
                                    to_screen(egui::pos2(stage_w, y))], paint);
                            }
                        }
                    }
                    3 => {
                        let vp = egui::pos2(stage_w * self.config.vanishing_x.clamp(0.05,0.95),
                            stage_h * self.config.vanishing_y.clamp(0.05,0.85));
                        let rays = ((stage_w / step).ceil() as usize).clamp(4, 48);
                        for n in 0..=rays {
                            let x = stage_w * n as f32 / rays as f32;
                            painter.line_segment([to_screen(vp), to_screen(egui::pos2(x, stage_h))], paint);
                        }
                        let lines = ((stage_h / step).ceil() as usize).clamp(2, 36);
                        for n in 1..=lines {
                            let fraction = n as f32 / (lines + 1) as f32;
                            let depth = fraction * fraction;
                            let y = vp.y + (stage_h - vp.y) * depth;
                            let left = vp.x * (1.0 - depth);
                            let right = vp.x + (stage_w - vp.x) * depth;
                            painter.line_segment([to_screen(egui::pos2(left, y)),
                                to_screen(egui::pos2(right, y))], paint);
                        }
                        painter.circle_stroke(to_screen(vp), 4.0, paint);
                    }
                    4 => {
                        // Two independently controlled vanishing points on a
                        // shared horizon, with fan rays clipped to the paper.
                        // Guides use the same stage-to-screen transform as the
                        // artwork and are intentionally excluded from exports.
                        let horizon = stage_h * self.config.vanishing_2point_y.clamp(0.07, 0.93);
                        let left = egui::pos2(stage_w * self.config.vanishing_left_x.clamp(0.02,0.47), horizon);
                        let right = egui::pos2(stage_w * self.config.vanishing_right_x.clamp(0.53,0.98), horizon);
                        let rays = ((stage_w / step).ceil() as usize).clamp(6,48);
                        for n in 0..=rays {
                            let x = stage_w * (n as f32) / (rays as f32);
                            for vp in [left, right] {
                                painter.line_segment([to_screen(vp), to_screen(egui::pos2(x,stage_h))], paint);
                                // A smaller fan above the horizon helps draw
                                // tall objects using the same two directions.
                                if n % 2 == 0 {
                                    painter.line_segment([to_screen(vp),
                                        to_screen(egui::pos2(x,0.0))], paint);
                                }
                            }
                        }
                        painter.line_segment([to_screen(egui::pos2(0.0,horizon)),
                            to_screen(egui::pos2(stage_w,horizon))], paint);
                        for vp in [left,right] {
                            painter.circle_filled(to_screen(vp), 4.0, paint.color);
                            painter.circle_stroke(to_screen(vp), 8.0, paint);
                        }
                    }
                    _ => {}
                }
            }

            if !space_down && !alt_down && self.active_tool_ui != ActiveToolUI::HandCanvas && is_over_canvas && !any_modal_open && !self.is_active_layer_locked() {
                if let Some(pos) = pointer_pos {
                    let stage_pos = to_stage(pos);
                    let delta = self.last_pointer_pos.map_or(egui::Vec2::ZERO, |l| to_stage(pos) - to_stage(l));
                    match self.active_tool_ui {
                        ActiveToolUI::Text if pointer_pressed => { self.pending_text_pos = stage_pos; self.show_text_dialog = true; }
                        ActiveToolUI::ShapeLine => {
                            if pointer_pressed { self.shape_drag_start = Some(stage_pos); self.shape_drag_current = Some(stage_pos); }
                            else if pointer_down { self.shape_drag_current = Some(stage_pos); }
                        }
                        ActiveToolUI::Bucket if pointer_pressed => {
                            // Existing exact vector-contour fill takes priority. If the
                            // region is drawn using many brush strokes or intersections,
                            // use a bounded raster wall mask + traced vector region.
                            // Optionally fill an active color layer using the combined
                            // visible outlines, as in modern 2D painting workflows.
                            let reference_strokes: Vec<Stroke> = if self.bucket_reference_all_layers {
                                self.project.layers.iter()
                                    .filter(|layer| layer.visible && !layer.is_guide)
                                    .flat_map(|layer|
                                        self.project.evaluate_layer_at_frame(layer, self.current_frame).strokes)
                                    .collect()
                            } else {
                                self.project.layers.get(self.active_layer_idx)
                                    .map(|layer|
                                        self.project.evaluate_layer_at_frame(layer, self.current_frame).strokes)
                                    .unwrap_or_default()
                            };
                            let fallback = fill_tools::flood_fill_polygon(
                                &reference_strokes, (stage_pos.x, stage_pos.y),
                                (self.project.width as f32, self.project.height as f32),
                                self.bucket_gap_close);
                            let can_fill = !self.bucket_reference_all_layers
                                && fallback.is_none()
                                && fill_tools::can_fill_contour(
                                    &reference_strokes, stage_pos.x, stage_pos.y);
                            if can_fill || fallback.is_some() {
                                self.save_undo_state();
                                self.ensure_editable_keyframe();
                                if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                    if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                                        if fallback.map(|points| {
                                            // Re-filling an existing region must visibly
                                            // override the previous fill while staying below
                                            // every outline. Previously insert(0) hid new
                                            // colors beneath older fill-only elements.
                                            let insert_at = f.strokes.iter().rposition(|s|
                                                s.color[3] == 0 && s.fill_color.is_some())
                                                .map(|i| i + 1).unwrap_or(0);
                                            f.strokes.insert(insert_at, fill_tools::polygon_fill_stroke(
                                                points, self.brush_color));
                                            true
                                        }).unwrap_or(false)
                                            || (can_fill && fill_tools::apply_contour_fill(
                                                &mut f.strokes, stage_pos.x, stage_pos.y, self.brush_color)) {
                                            self.has_unsaved_changes = true;
                                        }
                                    }
                                }
                            } else {
                                self.status_message = tr(lang,
                                    "Fill requires a closed region. Close small gaps between lines and retry.",
                                    "التعبئة محتاجة مساحة مقفولة؛ اقفل الفجوات الصغيرة بين الخطوط وجرّب تاني.");
                            }
                        }
                        ActiveToolUI::EraserRed | ActiveToolUI::EraserGreen if pointer_down => {
                            if pointer_pressed { self.save_undo_state(); }
                            self.ensure_editable_keyframe();
                            if let Some(l) = self.project.layers.get_mut(self.active_layer_idx) {
                                if let Some(f) = l.keyframes.get_mut(&self.current_frame) {
                                    let r = self.brush_size.max(2.0);
                                    let previous = if pointer_pressed {
                                        stage_pos
                                    } else {
                                        self.last_pointer_pos.map(&to_stage).unwrap_or(stage_pos)
                                    };
                                    // Interpolate along quick mouse/tablet drags. The old
                                    // point-only eraser left gaps between input frames.
                                    let distance = previous.distance(stage_pos);
                                    let steps = (distance / (r * 0.5).max(2.0))
                                        .ceil().clamp(1.0, 80.0) as usize;
                                    let mut changed = false;
                                    for step in 0..=steps {
                                        let sample = previous.lerp(stage_pos, step as f32 / steps as f32);
                                        changed |= if self.active_tool_ui == ActiveToolUI::EraserRed {
                                            eraser_tools::erase_whole(
                                                &mut f.strokes, (sample.x, sample.y), r)
                                        } else {
                                            eraser_tools::erase_partial(
                                                &mut f.strokes, (sample.x, sample.y), r)
                                        };
                                    }
                                    if changed { self.has_unsaved_changes = true; }
                                }
                            }
                        }
                        ActiveToolUI::Eyedropper if pointer_pressed => {
                            // Visible topmost layer must take precedence.
                            let picked = self.project.layers.iter().rev()
                                .filter(|layer| layer.visible && !layer.is_guide)
                                .find_map(|layer| {
                                    let evaluated = self.project.evaluate_layer_at_frame(
                                        layer, self.current_frame);
                                    selection_tools::pick_color(
                                        &evaluated.strokes, stage_pos, (5.0 / scale).max(1.0))
                                });
                            if let Some(color) = picked {
                                self.brush_color = color;
                            }
                        }
                        ActiveToolUI::Sculpt if pointer_down => {
                            if pointer_pressed { self.save_undo_state(); }
                            if !pointer_pressed && !self.is_active_layer_locked() {
                                if let Some(previous)=self.last_pointer_pos {
                                    let previous=to_stage(previous);
                                    let movement=stage_pos-previous;
                                    if movement.length_sq()>0.00001 {
                                        self.ensure_editable_keyframe();
                                        if let Some(layer)=self.project.layers.get_mut(self.active_layer_idx){
                                            if let Some(frame)=layer.keyframes.get_mut(&self.current_frame){
                                                if sculpt_tools::sculpt(
                                                    &mut frame.strokes,
                                                    (previous.x,previous.y),
                                                    (movement.x,movement.y),
                                                    self.sculpt_radius,self.sculpt_strength){
                                                    self.has_unsaved_changes=true;
                                                }
                                            }
                                        }
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
                                self.current_stroke = Some(Stroke { points: vec![StrokePoint { x: stage_pos.x, y: stage_pos.y, pressure: 1.0 }], color: self.brush_color, fill_color: None, base_width: self.brush_size, is_vector: true, brush_preset: KritaBrushPreset::GPenInk, opacity: 1.0, brush_texture_id: None, tip_shape: BrushTipShape::Round });
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
                                        stroke.points.push(StrokePoint { x: virt.x, y: virt.y, pressure: drawing_pressure });
                                    }
                                }
                            } else {
                                self.save_undo_state();
                                self.current_stroke = Some(Stroke { points: vec![StrokePoint { x: virt.x, y: virt.y, pressure: drawing_pressure }], color: self.brush_color, fill_color: None, base_width: self.brush_size, is_vector: false, brush_preset: self.active_krita_brush, opacity: self.brush_opacity, brush_texture_id: None, tip_shape: self.brush_tip_shape });
                            }
                        }
                        ActiveToolUI::VectorDirectSelect => {
                            if pointer_pressed {
                                let vector_pivot=self.project.layers.get(self.active_layer_idx)
                                    .and_then(|layer| {
                                        let frame=self.project.evaluate_layer_at_frame(layer,self.current_frame);
                                        let pts:Vec<_>=self.selected_vector_nodes.iter().filter_map(|&(si,pi)|
                                            frame.strokes.get(si)?.points.get(pi)).collect();
                                        (!pts.is_empty()).then(|| {
                                            let x=pts.iter().map(|p|p.x).sum::<f32>()/pts.len() as f32;
                                            let y=pts.iter().map(|p|p.y).sum::<f32>()/pts.len() as f32;
                                            egui::pos2(x,y)
                                        })
                                    });
                                let axis=vector_pivot.and_then(|pivot|
                                    transform_handles::translation_axis(
                                        egui::Rect::from_center_size(pivot,egui::vec2(1.,1.)),
                                        stage_pos,scale));
                                if let Some(axis)=axis {
                                    self.vector_axis_drag=Some(axis);
                                    self.save_undo_state();
                                } else {
                                self.vector_axis_drag=None;
                                let picked=self.project.layers.get(self.active_layer_idx)
                                    .and_then(|layer| {
                                        let frame=self.project.evaluate_layer_at_frame(
                                            layer,self.current_frame);
                                        selection_tools::nearest_vector_node(
                                            &frame.strokes,stage_pos,(13.0/scale).max(2.0))
                                    });
                                let extend=ctx.input(|i|i.modifiers.shift);
                                if let Some(node)=picked {
                                    if extend {
                                        if let Some(i)=self.selected_vector_nodes.iter().position(|&p|p==node){
                                            self.selected_vector_nodes.remove(i);
                                        }else{self.selected_vector_nodes.push(node);}
                                    }else if !self.selected_vector_nodes.contains(&node) {
                                        self.selected_vector_nodes=vec![node];
                                    }
                                    self.selected_vector_node=Some(node);
                                    self.vector_marquee_start=None;
                                    self.vector_marquee_current=None;
                                    self.save_undo_state();
                                } else {
                                    if !extend {self.selected_vector_nodes.clear();}
                                    self.selected_vector_node=None;
                                    self.vector_marquee_start=Some(stage_pos);
                                    self.vector_marquee_current=Some(stage_pos);
                                }
                                } // regular vertex selection, not the axis gizmo
                            } else if pointer_down && self.vector_marquee_start.is_some() {
                                self.vector_marquee_current=Some(stage_pos);
                            } else if pointer_down && delta.length_sq()>0.0
                                && !self.selected_vector_nodes.is_empty()
                                && !self.is_active_layer_locked() {
                                let nodes=self.selected_vector_nodes.clone();
                                let constrained=match self.vector_axis_drag {
                                    Some('x')=>egui::vec2(delta.x,0.),
                                    Some('y')=>egui::vec2(0.,delta.y),
                                    _=>delta,
                                };
                                self.ensure_editable_keyframe();
                                if let Some(layer)=self.project.layers.get_mut(self.active_layer_idx){
                                    if let Some(frame)=layer.keyframes.get_mut(&self.current_frame){
                                        for (si,pi) in nodes {
                                            if let Some(stroke)=frame.strokes.get_mut(si){
                                                let len=stroke.points.len();
                                                if let Some(point)=stroke.points.get_mut(pi){
                                                    point.x+=constrained.x;
                                                    point.y+=constrained.y;
                                                    self.has_unsaved_changes=true;
                                                }
                                                // A closed contour must stay closed when
                                                // dragging either coincident endpoint.
                                                if len>2 && (pi==0||pi==len-1){
                                                    let first=stroke.points[0].clone();
                                                    let last=stroke.points[len-1].clone();
                                                    if (first.x-last.x).hypot(first.y-last.y)<25.0 {
                                                        if pi==0 {stroke.points[len-1]=first;}
                                                        else {stroke.points[0]=last;}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ActiveToolUI::SelectNormal | ActiveToolUI::FreeTransform => {
                            if pointer_pressed {
                                // A dedicated handle above the selection provides free rotation.
                                let bounds = self.selected_canvas_bounds();
                                let rotate_pivot = if self.active_tool_ui == ActiveToolUI::FreeTransform {
                                    bounds.and_then(|rect| transform_handles::rotation_pivot(rect, stage_pos, scale))
                                } else { None };
                                let resize_anchor = if self.active_tool_ui == ActiveToolUI::FreeTransform {
                                    bounds.and_then(|rect| transform_handles::resize_anchor(rect, stage_pos, scale))
                                } else { None };
                                let move_axis = bounds.and_then(|rect|
                                    transform_handles::translation_axis(rect, stage_pos, scale));
                                let reuse_selection = bounds.is_some_and(|rect| rect.contains(stage_pos))
                                    && (!self.selected_stroke_indices.is_empty() || self.selected_text_item_index.is_some());
                                if let Some(axis)=move_axis {
                                    self.save_undo_state();
                                    self.transform_axis=Some(axis);
                                    self.transform_drag_anchor=None;
                                    self.transform_rotate_pivot=None;
                                } else if let Some(pivot) = rotate_pivot {
                                    self.save_undo_state();
                                    self.transform_rotate_pivot = Some(pivot);
                                    self.transform_drag_anchor = None;
                                } else if let Some(anchor) = resize_anchor {
                                    self.save_undo_state();
                                    self.transform_drag_anchor = Some(anchor);
                                    self.transform_rotate_pivot = None;
                                } else if reuse_selection {
                                    self.transform_axis=None;
                                    self.save_undo_state();
                                    self.transform_drag_anchor = None;
                                    self.transform_rotate_pivot = None;
                                } else {
                                self.transform_axis=None;
                                self.transform_drag_anchor = None;
                                self.transform_rotate_pivot = None;
                                self.selected_stroke_indices.clear();
                                self.selected_text_item_index = None;
                                self.selection_rect_start = None;
                                self.selection_rect_current = None;
                                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                                    let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                                    self.selected_text_item_index = frame.text_items.iter()
                                        .enumerate().rev()
                                        .find(|(_, text)| selection_tools::text_bounds(text).contains(stage_pos))
                                        .map(|(index, _)| index);
                                    if self.selected_text_item_index.is_none() {
                                        if let Some((index, _)) = frame.strokes.iter().enumerate().rev()
                                            .find(|(_, stroke)| selection_tools::stroke_hit_test(
                                                stroke, stage_pos, (6.0 / scale).max(1.0))) {
                                            self.selected_stroke_indices.push(index);
                                        }
                                    }
                                }
                                if self.selected_text_item_index.is_none() && self.selected_stroke_indices.is_empty() {
                                    self.selection_rect_start = Some(stage_pos);
                                    self.selection_rect_current = Some(stage_pos);
                                } else {
                                    self.save_undo_state();
                                }
                                } // finished regular selection; otherwise grabbed transform handle
                            } else if pointer_down {
                                if let Some(axis)=self.transform_axis {
                                    let movement=if axis=='x'{
                                        egui::vec2(delta.x,0.0)
                                    }else{egui::vec2(0.0,delta.y)};
                                    self.move_selected_items(movement);
                                } else if let Some(pivot) = self.transform_rotate_pivot {
                                    if let Some(previous) = self.last_pointer_pos {
                                        let prev = to_stage(previous) - pivot;
                                        let curr = stage_pos - pivot;
                                        let radians = curr.y.atan2(curr.x) - prev.y.atan2(prev.x);
                                        self.rotate_selection_around(pivot, radians);
                                    }
                                } else if let Some(anchor) = self.transform_drag_anchor {
                                    if let Some(previous) = self.last_pointer_pos {
                                        let previous_stage = to_stage(previous);
                                        let old_length = previous_stage.distance(anchor).max(1.0);
                                        let new_length = stage_pos.distance(anchor).max(1.0);
                                        self.resize_selection_from_corner(anchor,
                                            (new_length / old_length).clamp(0.4, 2.5));
                                    }
                                } else if self.selection_rect_start.is_some() {
                                    self.selection_rect_current = Some(stage_pos);
                                } else if delta.length_sq() > 0.0 {
                                    self.move_selected_items(delta);
                                }
                            }
                        }
                        ActiveToolUI::SelectLasso => {
                            if pointer_pressed {
                                let existing = self.selected_canvas_bounds().is_some_and(
                                    |bounds| bounds.contains(stage_pos));
                                self.lasso_dragging_selection = existing &&
                                    (!self.selected_stroke_indices.is_empty() || self.selected_text_item_index.is_some());
                                self.lasso_points.clear();
                                if self.lasso_dragging_selection {
                                    self.save_undo_state();
                                } else {
                                    self.selected_stroke_indices.clear();
                                    self.selected_text_item_index = None;
                                    self.lasso_points.push(stage_pos);
                                }
                            } else if pointer_down && self.lasso_dragging_selection {
                                self.move_selected_items(delta);
                            } else if pointer_down && self.lasso_points.last()
                                .is_none_or(|&prev| prev.distance(stage_pos) >= 2.5) {
                                if self.lasso_points.len() < 4_096 {
                                    self.lasso_points.push(stage_pos);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            if pointer_released {
                self.vector_axis_drag=None;
                if let (Some(start),Some(end))=(
                    self.vector_marquee_start.take(),self.vector_marquee_current.take()){
                    let rect=egui::Rect::from_two_pos(start,end);
                    if rect.width()>3.0 || rect.height()>3.0 {
                        let extend=ctx.input(|i|i.modifiers.shift);
                        if !extend {self.selected_vector_nodes.clear();}
                        if let Some(layer)=self.project.layers.get(self.active_layer_idx){
                            let frame=self.project.evaluate_layer_at_frame(layer,self.current_frame);
                            for (si,stroke) in frame.strokes.iter().enumerate(){
                                if !stroke.is_vector {continue;}
                                for (pi,p) in stroke.points.iter().enumerate(){
                                    let key=(si,pi);
                                    if rect.contains(egui::pos2(p.x,p.y))
                                        && !self.selected_vector_nodes.contains(&key)
                                        && self.selected_vector_nodes.len()<4_000 {
                                        self.selected_vector_nodes.push(key);
                                    }
                                }
                            }
                        }
                    }
                }
                self.transform_drag_anchor = None;
                self.transform_rotate_pivot = None;
                self.transform_axis = None;
                self.lasso_dragging_selection = false;
                if let (Some(start), Some(end)) = (
                    self.selection_rect_start.take(), self.selection_rect_current.take()
                ) {
                    let rect = egui::Rect::from_two_pos(start, end);
                    if rect.width() > 3.0 || rect.height() > 3.0 {
                        if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                            let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                            self.selected_stroke_indices = frame.strokes.iter().enumerate()
                                .filter_map(|(idx, s)| selection_tools::stroke_intersects_marquee(s, rect).then_some(idx))
                                .collect();
                            self.selected_text_item_index = frame.text_items.iter().position(
                                |text| selection_tools::text_bounds(text).intersects(rect)
                            );
                        }
                    }
                }
                if self.lasso_points.len() >= 3 {
                    if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                        let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                        self.selected_stroke_indices = frame.strokes.iter().enumerate()
                            .filter_map(|(idx, s)| selection_tools::stroke_inside_lasso(s, &self.lasso_points).then_some(idx))
                            .collect();
                        self.selected_text_item_index = frame.text_items.iter().position(
                            |txt| selection_tools::point_in_polygon(selection_tools::text_bounds(txt).center(), &self.lasso_points)
                        );
                    }
                }
                self.lasso_points.clear();
                self.stabilized_cursor = None;
                if let (Some(p0), Some(p1)) = (self.shape_drag_start.take(), self.shape_drag_current.take()) {
                    if p0.distance(p1) > 3.0 {
                        self.save_undo_state();
                        let mut shape = build_geometric_stroke(self.selected_shape_kind, p0, p1, self.brush_color, self.brush_size);
                        shape.opacity = self.brush_opacity;
                        if self.shape_fill_enabled && self.selected_shape_kind != GeometricShapeKind::StraightLine {
                            shape.fill_color = Some(self.brush_color);
                        }
                        self.commit_stroke_with_symmetry(shape, now_time);
                    }
                }
                if let Some(mut stroke) = self.current_stroke.take() {
                    // Keep the actual pointer-up endpoint so stabilized brush
                    // strokes don't visibly stop short or leave a tiny loop gap.
                    if !stroke.is_vector {
                        if let Some(pointer) = pointer_pos.filter(|p| canvas_rect.contains(*p)) {
                            let last_stage = to_stage(pointer);
                            if stroke.points.last().is_some_and(|p|
                                (p.x-last_stage.x).hypot(p.y-last_stage.y)>0.75) {
                                let pressure = stroke.points.last()
                                    .map(|p| p.pressure).unwrap_or(1.0);
                                stroke.points.push(StrokePoint {
                                    x: last_stage.x, y: last_stage.y, pressure
                                });
                            }
                        }
                    }
                    if stroke.is_vector {
                        // Keep control points editable. Eightfold Catmull-Rom
                        // expansion was causing thousands of artificial nodes.
                        stroke.simplify_vector_nodes(self.vector_tolerance);
                    } else {
                        // Freehand already has samples + smoothing stabilizer.
                        // Adding four Catmull-Rom subdivisions overshoots sharp
                        // turns and makes visibly broken/looped brush segments.
                        // Keep the true pointer trajectory.
                    }
                    self.commit_stroke_with_symmetry(stroke, now_time);
                }
            }

            self.last_pointer_pos = if pointer_down { pointer_pos } else { None };

            // Real Onion Skin preview: faint past/future drawings behind
            // the current frame. Only active in Animation workspace. No
            // changes to saved data and never exported as actual artwork.
            if self.workspace_mode == WorkspaceMode::AnimationStudio
                && self.project.onion_skin.enabled
            {
                let settings = &self.project.onion_skin;
                for ghost in onion_skin::ghost_frames(
                    self.current_frame,
                    self.timeline_length,
                    settings.frames_before,
                    settings.frames_after,
                ) {
                    let base_tint = match ghost.side {
                        onion_skin::GhostSide::Previous => settings.color_before,
                        onion_skin::GhostSide::Next => settings.color_after,
                    };
                    let tint = onion_skin::ghost_color(base_tint, ghost.distance);
                    for layer in self.project.layers.iter().rev() {
                        if !layer.visible || layer.is_guide { continue; }
                        let layer_opacity = layer.opacity.clamp(0.0, 1.0);
                        if layer_opacity <= 0.0 { continue; }
                        let ghost_data = self.project.evaluate_layer_at_frame(layer, ghost.frame);
                        for stroke in &ghost_data.strokes {
                            draw_stroke_pro(
                                &painter, stroke, Some(tint), layer_opacity,
                                scale, 0.0, 0.0, &to_screen,
                            );
                        }
                        for instance in &ghost_data.symbol_instances {
                            if let Some(symbol) = self.project.library_symbols.get(&instance.symbol_name) {
                                for stroke in self.project.evaluate_symbol_strokes(symbol, ghost.frame) {
                                    // Symbols are previewed at their existing position.
                                    // More complex transforms will be unified with the
                                    // shared compositor in a later milestone.
                                    draw_stroke_pro(
                                        &painter, stroke, Some(tint), layer_opacity,
                                        scale, instance.x, instance.y, &to_screen,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            for layer in self.project.layers.iter().rev() {
                if layer.visible {
                    let l_op = layer.opacity.clamp(0.05, 1.0);
                    let fd = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                    for m in &fd.media_items {
                        if let Some(tex) = self.texture_cache.get(&m.path) {
                            let mut mesh = egui::Mesh::with_texture(tex.id());
                            let tint = egui::Color32::from_white_alpha((255.0 * l_op) as u8);
                            for (pt, uv) in [
                                (egui::pos2(m.x, m.y), egui::pos2(0.0, 0.0)),
                                (egui::pos2(m.x + m.width, m.y), egui::pos2(1.0, 0.0)),
                                (egui::pos2(m.x + m.width, m.y + m.height), egui::pos2(1.0, 1.0)),
                                (egui::pos2(m.x, m.y + m.height), egui::pos2(0.0, 1.0)),
                            ] {
                                mesh.vertices.push(egui::epaint::Vertex { pos: to_screen(pt), uv, color: tint });
                            }
                            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                            painter.add(egui::Shape::mesh(mesh));
                        }
                    }
                    for s in &fd.strokes { draw_stroke_pro(&painter, s, None, l_op, scale, 0.0, 0.0, &to_screen); }
                    for txt in &fd.text_items {
                        let tint = egui::Color32::from_rgba_unmultiplied(txt.color[0], txt.color[1],
                            txt.color[2], (txt.color[3] as f32 * l_op) as u8);
                        let galley = painter.layout_no_wrap(
                            fix_ar(&txt.content),
                            egui::FontId::proportional(txt.font_size * scale),
                            tint,
                        );
                        let text_shape = egui::epaint::TextShape::new(
                            to_screen(egui::pos2(txt.x, txt.y)), galley, tint,
                        ).with_angle(self.canvas_rotation_deg.to_radians());
                        painter.add(egui::Shape::Text(text_shape));
                    }
                }
            }

            if let (Some(start), Some(end)) = (self.selection_rect_start, self.selection_rect_current) {
                let a = to_screen(start);
                let b = to_screen(end);
                painter.rect_stroke(
                    egui::Rect::from_two_pos(a, b), 0.0,
                    egui::Stroke::new(1.4, egui::Color32::from_rgb(45, 170, 255)),
                );
            }
            if self.lasso_points.len() >= 2 {
                let path: Vec<_> = self.lasso_points.iter().copied().map(&to_screen).collect();
                painter.add(egui::Shape::line(path, egui::Stroke::new(
                    1.4, egui::Color32::from_rgb(45, 170, 255)
                )));
            }
            if let Some(idx) = self.selected_text_item_index {
                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                    let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                    if let Some(txt) = frame.text_items.get(idx) {
                        let bounds = selection_tools::text_bounds(txt);
                        painter.rect_stroke(
                            egui::Rect::from_two_pos(to_screen(bounds.min), to_screen(bounds.max)),
                            1.5, egui::Stroke::new(1.4, egui::Color32::from_rgb(45, 170, 255))
                        );
                    }
                }
            }
            // Persistent visual selection and a draggable scale handle.
            if matches!(self.active_tool_ui, ActiveToolUI::SelectNormal | ActiveToolUI::SelectLasso | ActiveToolUI::FreeTransform) {
                if let Some(bounds) = self.selected_canvas_bounds() {
                    let screen_bounds = egui::Rect::from_two_pos(
                        to_screen(bounds.min), to_screen(bounds.max)
                    );
                    painter.rect_stroke(screen_bounds, 0.0, egui::Stroke::new(
                        1.6, egui::Color32::from_rgb(54, 194, 255)
                    ));
                    // Two axis arrows remain visible after selecting artwork.
                    // Drag red for X only, cyan for Y only; corner squares and
                    // rotation remain available through Free Transform.
                    let origin=to_screen(bounds.center());
                    for (end,col,label) in [
                        (bounds.center()+egui::vec2(52.0/scale,0.0),
                            egui::Color32::from_rgb(244,78,86),"X"),
                        (bounds.center()+egui::vec2(0.0,-52.0/scale),
                            egui::Color32::from_rgb(60,192,244),"Y")
                    ] {
                        let tip=to_screen(end);
                        painter.line_segment([origin,tip],egui::Stroke::new(3.0,col));
                        painter.circle_filled(tip,10.0,col);
                        painter.text(tip,egui::Align2::CENTER_CENTER,label,
                            egui::FontId::proportional(11.0),egui::Color32::BLACK);
                    }
                    if self.active_tool_ui == ActiveToolUI::FreeTransform {
                        let top = to_screen(bounds.center_top());
                        let handle = to_screen(bounds.center_top()
                            + egui::vec2(0.0, -transform_handles::ROTATE_OFFSET_PX / scale));
                        painter.line_segment([top, handle], egui::Stroke::new(1.5, egui::Color32::from_rgb(54, 194, 255)));
                        painter.circle_filled(handle, 12.5, egui::Color32::from_rgb(19, 31, 42));
                        painter.circle_stroke(handle, 12.5, egui::Stroke::new(2.0, egui::Color32::from_rgb(54, 194, 255)));
                        painter.text(handle, egui::Align2::CENTER_CENTER, "↻",
                            egui::FontId::proportional(16.0), egui::Color32::WHITE);
                        for corner in [screen_bounds.left_top(), screen_bounds.right_top(),
                                       screen_bounds.left_bottom(), screen_bounds.right_bottom()] {
                            let handle = egui::Rect::from_center_size(corner,
                                egui::vec2(transform_handles::RESIZE_VISUAL_PX, transform_handles::RESIZE_VISUAL_PX));
                            painter.rect_filled(handle, 1.0, egui::Color32::WHITE);
                            painter.rect_stroke(handle, 1.0, egui::Stroke::new(
                                1.5, egui::Color32::from_rgb(54, 194, 255)
                            ));
                        }
                    }
                }
            }
            if let (Some(a),Some(b))=(self.vector_marquee_start,self.vector_marquee_current){
                painter.rect_stroke(egui::Rect::from_two_pos(to_screen(a),to_screen(b)),0.0,
                    egui::Stroke::new(1.4,egui::Color32::from_rgb(255,169,75)));
            }
            if self.active_tool_ui == ActiveToolUI::VectorDirectSelect
                && !self.selected_vector_nodes.is_empty() {
                if let Some(layer)=self.project.layers.get(self.active_layer_idx){
                    let frame=self.project.evaluate_layer_at_frame(layer,self.current_frame);
                    let pts:Vec<_>=self.selected_vector_nodes.iter().filter_map(|&(si,pi)|
                        frame.strokes.get(si)?.points.get(pi)).collect();
                    if !pts.is_empty(){
                        let center=egui::pos2(
                            pts.iter().map(|p|p.x).sum::<f32>()/pts.len() as f32,
                            pts.iter().map(|p|p.y).sum::<f32>()/pts.len() as f32);
                        for (end,col,label) in [
                            (center+egui::vec2(52./scale,0.),egui::Color32::from_rgb(244,78,86),"X"),
                            (center+egui::vec2(0.,-52./scale),egui::Color32::from_rgb(60,192,244),"Y")
                        ]{
                            painter.line_segment([to_screen(center),to_screen(end)],
                                egui::Stroke::new(2.7,col));
                            painter.circle_filled(to_screen(end),9.,col);
                            painter.text(to_screen(end),egui::Align2::CENTER_CENTER,label,
                                egui::FontId::proportional(11.0),egui::Color32::BLACK);
                        }
                    }
                }
            }
            if self.active_tool_ui == ActiveToolUI::VectorDirectSelect {
                if let Some(layer) = self.project.layers.get(self.active_layer_idx) {
                    if layer.visible {
                        let frame = self.project.evaluate_layer_at_frame(layer, self.current_frame);
                        let mut nodes_drawn = 0usize;
                        for (stroke_index, stroke) in frame.strokes.iter().enumerate() {
                            if !stroke.is_vector { continue; }
                            for (node_index, node) in stroke.points.iter().enumerate() {
                                if nodes_drawn >= 1_500 { break; }
                                let p = to_screen(egui::pos2(node.x, node.y));
                                if canvas_rect.contains(p) {
                                    let selected = self.selected_vector_nodes.contains(&(stroke_index,node_index));
                                    let color = if selected {
                                        egui::Color32::from_rgb(255, 202, 58)
                                    } else {
                                        egui::Color32::from_rgb(80, 198, 255)
                                    };
                                    painter.rect_filled(
                                        egui::Rect::from_center_size(p, egui::vec2(
                                            if selected { 11.0 } else { 7.0 },
                                            if selected { 11.0 } else { 7.0 }
                                        )), 1.0, color
                                    );
                                    nodes_drawn += 1;
                                }
                            }
                            if nodes_drawn >= 1_500 { break; }
                        }
                    }
                }
            }
            if let Some(stroke) = &self.current_stroke { draw_stroke_pro(&painter, stroke, None, 1.0, scale, 0.0, 0.0, &to_screen); }
            if let (Some(p0), Some(p1)) = (self.shape_drag_start, self.shape_drag_current) {
                let mut shape = build_geometric_stroke(self.selected_shape_kind, p0, p1, self.brush_color, self.brush_size);
                shape.opacity = self.brush_opacity;
                if self.shape_fill_enabled && self.selected_shape_kind != GeometricShapeKind::StraightLine {
                    shape.fill_color = Some(self.brush_color);
                }
                draw_stroke_pro(&painter, &shape, None, 1.0, scale, 0.0, 0.0, &to_screen);
            }
            if show_dot {
                if let Some(p) = pointer_pos {
                    let radius = ((if self.active_tool_ui == ActiveToolUI::Sculpt {
                        self.sculpt_radius * scale
                    } else { self.brush_size * self.zoom_level * 0.5 })
                        * self.config.brush_cursor_scale.clamp(0.4,2.5)).clamp(2.0,500.0);
                    if self.active_tool_ui == ActiveToolUI::Sculpt {
                        painter.circle_stroke(p,radius,egui::Stroke::new(1.8,
                            egui::Color32::from_rgb(247,141,197)));
                        painter.circle_filled(p,2.5,egui::Color32::from_rgb(247,141,197));
                    } else if self.active_tool_ui == ActiveToolUI::EraserRed
                        || self.active_tool_ui == ActiveToolUI::EraserGreen {
                        let color = if self.active_tool_ui == ActiveToolUI::EraserRed {
                            egui::Color32::from_rgb(255, 96, 113)
                        } else {
                            egui::Color32::from_rgb(45, 221, 145)
                        };
                        painter.circle_stroke(p, radius, egui::Stroke::new(2.0, color));
                        painter.circle_filled(p, 2.5, color);
                    } else {
                        match self.active_krita_brush {
                            KritaBrushPreset::ChiselMarker if self.active_tool_ui == ActiveToolUI::Brush => {
                                painter.rect_stroke(egui::Rect::from_center_size(p,
                                    egui::vec2(radius * 2.0, radius * 1.2)), 0.0,
                                    egui::Stroke::new(1.8, egui::Color32::from_rgb(255, 165, 67)));
                            }
                            KritaBrushPreset::SoftAirbrush if self.active_tool_ui == ActiveToolUI::Brush => {
                                painter.circle_filled(p, radius,
                                    egui::Color32::from_rgba_unmultiplied(182, 134, 255, 40));
                                painter.circle_stroke(p, radius,
                                    egui::Stroke::new(1.4, egui::Color32::from_rgb(195, 144, 255)));
                            }
                            KritaBrushPreset::Watercolor if self.active_tool_ui == ActiveToolUI::Brush => {
                                painter.circle_stroke(p, radius,
                                    egui::Stroke::new(1.5, egui::Color32::from_rgb(70, 230, 198)));
                                painter.circle_filled(p, 2.5, egui::Color32::from_rgb(70, 230, 198));
                            }
                            KritaBrushPreset::SketchPencil if self.active_tool_ui == ActiveToolUI::Brush => {
                                painter.circle_stroke(p, radius,
                                    egui::Stroke::new(1.2, egui::Color32::from_rgb(240, 202, 75)));
                                painter.line_segment([p + egui::vec2(-5.0, -5.0),
                                    p + egui::vec2(5.0, 5.0)],
                                    egui::Stroke::new(1.2, egui::Color32::from_rgb(240, 202, 75)));
                            }
                            _ => {
                                painter.circle_stroke(p, radius + 1.0,
                                    egui::Stroke::new(2.0, egui::Color32::WHITE));
                                painter.circle_stroke(p, radius,
                                    egui::Stroke::new(1.3, egui::Color32::BLACK));
                                painter.circle_filled(p, 1.7, egui::Color32::BLACK);
                            }
                        }
                    }
                }
            } else if show_tool_icon {
                if let Some(p) = pointer_pos {
                    let icon_size=(26.0*self.config.tool_icon_scale.clamp(0.6,2.0)).max(16.0);
                    // Eyedropper used to render its nib ~30px BELOW the real
                    // mouse hotspot while hiding the system cursor. Align the
                    // nib (icon local -10,+10) exactly with pointer_pos.
                    let icon_center = if self.active_tool_ui == ActiveToolUI::Eyedropper {
                        let s=icon_size/26.0;
                        p + egui::vec2(10.0*s,-10.0*s)
                    } else {
                        p + egui::vec2(8.0+icon_size*0.5,7.0+icon_size*0.5)
                    };
                    let rect = egui::Rect::from_center_size(icon_center,
                        egui::vec2(icon_size, icon_size));
                    let tool_color = self.online_participants.iter()
                        .find(|participant| self.online_session.is_some()
                            && participant.name == self.user_profile.username)
                        .map(|p| egui::Color32::from_rgb(p.color[0],p.color[1],p.color[2]))
                        .unwrap_or_else(|| get_tool_ui_color(self.active_tool_ui));
                    if self.config.icon_cursor_ring {
                        painter.circle_filled(rect.center(), icon_size*0.54,
                            egui::Color32::from_rgba_unmultiplied(24, 27, 36, 180));
                        painter.circle_stroke(rect.center(), icon_size*0.54,
                            egui::Stroke::new(1.2, tool_color));
                    }
                    draw_tool_ui_icon(&painter, rect, self.active_tool_ui, false);
                }
            }
        });

        // Relay all document mutations after each UI frame, including erasing,
        // layer operations, frame/keyframe changes, fills, text and transform.
        if let Some(session) = &self.online_session {
            if let Err(err) = session.sync_local_changes(&self.project) {
                self.online_message = format!("Online sync: {err}");
                self.status_message = self.online_message.clone();
            }
        }

        // Positioned relative to the full window, not just remaining menu
        // space: the optional-support heart stays in the TOP CENTER.
        if !self.show_support_dialog && !self.show_settings_dialog {
            egui::Area::new(egui::Id::new("center_gold_support_heart"))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 5.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let button = egui::Button::new(
                        egui::RichText::new("♥").size(21.0).strong()
                            .color(egui::Color32::from_rgb(255, 206, 53))
                    ).fill(egui::Color32::from_rgb(50, 44, 29));
                    if ui.add(button).on_hover_text(tr(lang,
                        "Support Stick (optional)", "دعم البرنامج (اختياري)")).clicked() {
                        self.show_support_dialog = true;
                    }
                });
        }
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

#[cfg(test)]
mod tool_discovery_tests {
    use super::*;

    #[test]
    fn arabic_text_keeps_multiple_lines_without_dropping_content() {
        let joined = fix_ar("مرحبا يا فنان\nمرحبا يا مبدع");
        assert_eq!(joined.split('\n').count(), 2);
        assert!(joined.chars().any(|c| ('\u{FB50}'..='\u{FEFF}').contains(&c)));
    }

    #[test]
    fn support_copy_keeps_arabic_lines_short_and_separate() {
        assert_eq!(support_message_lines(AppLang::Arabic).len(), 7);
        for (en, ar) in SUPPORT_MESSAGE_LINES {
            assert!(!en.is_empty() && !ar.is_empty());
            assert!(!ar.contains('\n'));
            // Prevent the previous one-paragraph RTL wrapping regression.
            assert!(ar.chars().count() <= 48, "Arabic copy line too long: {ar}");
        }
        let rendered = support_message_lines(AppLang::Arabic);
        assert!(rendered.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn updater_notice_keeps_arabic_unshaped_until_one_render_pass() {
        let message = tr_notice(
            AppLang::Arabic,
            "Save your work first",
            "احفظ شغلك الأول",
        );
        assert_eq!(message, "احفظ شغلك الأول");
        assert!(!message.is_empty());
    }

    #[test]
    fn every_toolbar_tool_has_a_localized_name_and_shortcut() {
        let config = AppConfig::default();
        for tool in TOOLBAR_ITEMS {
            let name_en = tool_display_name(tool, AppLang::English);
            let name_ar = tool_display_name(tool, AppLang::Arabic);
            assert!(!name_en.trim().is_empty());
            assert!(!name_ar.trim().is_empty());
            assert!(!tool_shortcut(tool, &config).trim().is_empty());
        }
    }

    #[test]
    fn shortcuts_follow_user_configuration() {
        let mut config = AppConfig::default();
        config.tool_brush = "J".to_string();
        assert_eq!(tool_shortcut(ActiveToolUI::Brush, &config), "J");
    }

    #[test]
    fn geometric_tools_generate_finite_closed_shapes() {
        let from = egui::pos2(20.0, 25.0);
        let to = egui::pos2(130.0, 145.0);
        for kind in [
            GeometricShapeKind::Circle, GeometricShapeKind::Square,
            GeometricShapeKind::Rectangle, GeometricShapeKind::Triangle,
            GeometricShapeKind::Pentagon
        ] {
            let shape = build_geometric_stroke(kind,from,to,[220,10,10,255],5.0);
            assert!(shape.points.len()>=4);
            let first=&shape.points[0];
            let last=shape.points.last().unwrap();
            assert!((first.x-last.x).abs() < 0.1 && (first.y-last.y).abs() < 0.1);
            assert!(shape.points.iter().all(|p|p.x.is_finite()&&p.y.is_finite()));
            assert!(fill_tools::contour_contains(&shape,75.0,83.0));
        }
    }

    #[test]
    fn squares_have_real_area_even_for_purely_vertical_drag() {
        let square=build_geometric_stroke(
            GeometricShapeKind::Square,
            egui::pos2(50.0,20.0),egui::pos2(50.0,80.0),[0,0,0,255],3.0);
        let xs: Vec<_>=square.points.iter().map(|p|p.x).collect();
        assert!(xs.iter().copied().fold(f32::NEG_INFINITY,f32::max)
            -xs.iter().copied().fold(f32::INFINITY,f32::min)>50.0);
    }

    #[test]
    fn palette_contains_unique_opaque_colors() {
        let mut seen = std::collections::HashSet::new();
        for (rgb, _) in BASIC_COLORS {
            assert!(seen.insert(rgb));
        }
        assert_eq!(seen.len(), 12);
    }
}
