use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

pub const PROJECT_SCHEMA_VERSION: u32 = 5;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum KritaBrushPreset {
    GPenInk,
    Watercolor,
    ChiselMarker,
    SoftAirbrush,
    SketchPencil,
}

impl Default for KritaBrushPreset {
    fn default() -> Self {
        KritaBrushPreset::GPenInk
    }
}

impl KritaBrushPreset {
    pub fn label(&self) -> &'static str {
        match self {
            KritaBrushPreset::GPenInk => "G-Pen (حبر ثابت بدون ذيل)",
            KritaBrushPreset::Watercolor => "Watercolor (ألوان مائية متصلة)",
            KritaBrushPreset::ChiselMarker => "Marker (ماركر شفاف متراكم)",
            KritaBrushPreset::SoftAirbrush => "Airbrush (بخاخ ناعم)",
            KritaBrushPreset::SketchPencil => "Pencil (قلم رصاص سكتش)",
        }
    }

    pub fn default_opacity(&self) -> f32 {
        match self {
            KritaBrushPreset::GPenInk => 1.0,
            KritaBrushPreset::Watercolor => 0.55,
            KritaBrushPreset::ChiselMarker => 0.42,
            KritaBrushPreset::SoftAirbrush => 0.40,
            KritaBrushPreset::SketchPencil => 0.75,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum AudioSyncMode {
    Normal,
    Stream,
}

impl Default for AudioSyncMode {
    fn default() -> Self {
        AudioSyncMode::Stream
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum TweenMode {
    None,
    Classic,
    Shape,
}

impl Default for TweenMode {
    fn default() -> Self {
        TweenMode::None
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum EasingCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Default for EasingCurve {
    fn default() -> Self {
        EasingCurve::EaseInOut
    }
}

impl EasingCurve {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingCurve::Linear => t,
            EasingCurve::EaseIn => t * t,
            EasingCurve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            EasingCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            EasingCurve::Linear => "Linear (ثابت)",
            EasingCurve::EaseIn => "Ease In (تسارع تدريجي)",
            EasingCurve::EaseOut => "Ease Out (تباطؤ ناعم)",
            EasingCurve::EaseInOut => "Ease In-Out (سينمائي)",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_pressure")]
    pub pressure: f32,
}

fn default_pressure() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Stroke {
    #[serde(default)]
    pub points: Vec<StrokePoint>,
    #[serde(default = "default_black")]
    pub color: [u8; 4],
    #[serde(default)]
    pub fill_color: Option<[u8; 4]>,
    #[serde(default = "default_width")]
    pub base_width: f32,
    #[serde(default)]
    pub is_vector: bool,
    #[serde(default)]
    pub brush_preset: KritaBrushPreset,
    #[serde(default = "default_pressure")]
    pub opacity: f32,
    #[serde(default)]
    pub brush_texture_id: Option<String>,
}

fn default_black() -> [u8; 4] {
    [0, 0, 0, 255]
}

fn default_width() -> f32 {
    6.0
}

impl Stroke {
    pub fn fit_cubic_bezier_spline(&mut self, subdivisions: usize) {
        if self.points.len() < 3 {
            return;
        }
        let mut dense = Vec::with_capacity(self.points.len() * subdivisions);
        let n = self.points.len();
        dense.push(self.points[0].clone());

        for i in 0..(n - 1) {
            let p0 = &self.points[i.saturating_sub(1)];
            let p1 = &self.points[i];
            let p2 = &self.points[(i + 1).min(n - 1)];
            let p3 = &self.points[(i + 2).min(n - 1)];

            for step in 1..=subdivisions {
                let t = step as f32 / subdivisions as f32;
                let t2 = t * t;
                let t3 = t2 * t;

                let x = 0.5
                    * ((2.0 * p1.x)
                        + (-p0.x + p2.x) * t
                        + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                        + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
                let y = 0.5
                    * ((2.0 * p1.y)
                        + (-p0.y + p2.y) * t
                        + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                        + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
                let pressure = p1.pressure + (p2.pressure - p1.pressure) * t;

                dense.push(StrokePoint { x, y, pressure });
            }
        }
        self.points = dense;
    }

    pub fn simplify_vector_nodes(&mut self, tolerance: f32) {
        if self.points.len() <= 3 {
            return;
        }
        self.points = rdp_simplify(&self.points, tolerance);
    }

    pub fn resample_points(&self, target_count: usize) -> Vec<StrokePoint> {
        if self.points.is_empty() || target_count == 0 {
            return Vec::new();
        }
        if self.points.len() == 1 || target_count == 1 {
            return vec![self.points[0].clone(); target_count];
        }
        let mut out = Vec::with_capacity(target_count);
        let max_idx = (self.points.len() - 1) as f32;
        for i in 0..target_count {
            let pos = (i as f32 / (target_count - 1).max(1) as f32) * max_idx;
            let idx0 = pos.floor() as usize;
            let idx1 = (idx0 + 1).min(self.points.len() - 1);
            let frac = pos - idx0 as f32;
            let p0 = &self.points[idx0];
            let p1 = &self.points[idx1];
            out.push(StrokePoint {
                x: p0.x + (p1.x - p0.x) * frac,
                y: p0.y + (p1.y - p0.y) * frac,
                pressure: p0.pressure + (p1.pressure - p0.pressure) * frac,
            });
        }
        out
    }
}

fn rdp_simplify(pts: &[StrokePoint], epsilon: f32) -> Vec<StrokePoint> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut dmax = 0.0f32;
    let mut index = 0;
    let end = pts.len() - 1;
    for i in 1..end {
        let d = perpendicular_distance(&pts[i], &pts[0], &pts[end]);
        if d > dmax {
            index = i;
            dmax = d;
        }
    }
    if dmax > epsilon {
        let mut rec1 = rdp_simplify(&pts[..=index], epsilon);
        let rec2 = rdp_simplify(&pts[index..], epsilon);
        rec1.pop();
        rec1.extend(rec2);
        rec1
    } else {
        vec![pts[0].clone(), pts[end].clone()]
    }
}

fn perpendicular_distance(p: &StrokePoint, a: &StrokePoint, b: &StrokePoint) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = a.x + t * dx;
    let proj_y = a.y + t * dy;
    ((p.x - proj_x).powi(2) + (p.y - proj_y).powi(2)).sqrt()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextItem {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_black")]
    pub color: [u8; 4],
}

fn default_font_size() -> f32 {
    32.0
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MediaType {
    Image,
    VideoFrame,
}

impl Default for MediaType {
    fn default() -> Self {
        MediaType::Image
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaItem {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub media_type: MediaType,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "default_media_dim")]
    pub width: f32,
    #[serde(default = "default_media_dim")]
    pub height: f32,
}

fn default_media_dim() -> f32 {
    300.0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Symbol {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub strokes: Vec<Stroke>,
    #[serde(default)]
    pub nested_keyframes: HashMap<usize, Vec<Stroke>>,
    #[serde(default = "default_one")]
    pub nested_total_frames: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SymbolInstance {
    #[serde(default)]
    pub symbol_name: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "default_pressure")]
    pub scale: f32,
    #[serde(default)]
    pub rotation_deg: f32,
    #[serde(default)]
    pub anchor_offset_x: f32,
    #[serde(default)]
    pub anchor_offset_y: f32,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct FrameData {
    #[serde(default)]
    pub strokes: Vec<Stroke>,
    #[serde(default)]
    pub media_items: Vec<MediaItem>,
    #[serde(default)]
    pub symbol_instances: Vec<SymbolInstance>,
    #[serde(default)]
    pub text_items: Vec<TextItem>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub motion_tween: bool,
    #[serde(default)]
    pub tween_mode: TweenMode,
    #[serde(default)]
    pub easing: EasingCurve,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Layer {
    #[serde(default = "default_layer_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub is_guide: bool,
    /// Whether a real stroke has already earned this layer's one-time credit.
    #[serde(default)]
    pub xp_first_paint_rewarded: bool,
    #[serde(default = "default_pressure")]
    pub opacity: f32,
    #[serde(default)]
    pub keyframes: HashMap<usize, FrameData>,
}

fn default_layer_name() -> String {
    "Layer 1".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioTrack {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default = "default_one")]
    pub start_frame: usize,
    #[serde(default = "default_end_frame")]
    pub end_frame: usize,
    #[serde(default)]
    pub sync_mode: AudioSyncMode,
    #[serde(default)]
    pub waveform_peaks: Vec<f32>,
}

fn default_one() -> usize {
    1
}

fn default_end_frame() -> usize {
    120
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OnionSkinSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_two")]
    pub frames_before: usize,
    #[serde(default = "default_two")]
    pub frames_after: usize,
    #[serde(default = "default_onion_before")]
    pub color_before: [u8; 4],
    #[serde(default = "default_onion_after")]
    pub color_after: [u8; 4],
}

fn default_two() -> usize {
    2
}

fn default_onion_before() -> [u8; 4] {
    [255, 80, 80, 90]
}

fn default_onion_after() -> [u8; 4] {
    [80, 255, 100, 90]
}

impl Default for OnionSkinSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            frames_before: 2,
            frames_after: 2,
            color_before: [255, 80, 80, 90],
            color_after: [80, 255, 100, 90],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AniProject {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default = "default_w")]
    pub width: u32,
    #[serde(default = "default_h")]
    pub height: u32,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_one")]
    pub total_frames: usize,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub audio_tracks: Vec<AudioTrack>,
    #[serde(default)]
    pub library_symbols: HashMap<String, Symbol>,
    #[serde(default)]
    pub onion_skin: OnionSkinSettings,
    /// Saved frame numbers already credited for profile XP.
    #[serde(default)]
    pub xp_credited_frames: Vec<usize>,
}

fn default_schema() -> u32 { PROJECT_SCHEMA_VERSION }
fn default_w() -> u32 { 1920 }
fn default_h() -> u32 { 1080 }
fn default_fps() -> u32 { 24 }

impl Default for AniProject {
    fn default() -> Self {
        let mut keyframes = HashMap::new();
        keyframes.insert(1, FrameData::default());
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            fps: 24,
            total_frames: 1,
            layers: vec![Layer {
                name: "Layer 1".to_string(),
                visible: true,
                locked: false,
                is_guide: false,
                xp_first_paint_rewarded: false,
                opacity: 1.0,
                keyframes,
            }],
            audio_tracks: Vec::new(),
            library_symbols: HashMap::new(),
            onion_skin: OnionSkinSettings::default(),
            xp_credited_frames: Vec::new(),
        }
    }
}

impl AniProject {
    pub fn evaluate_layer_at_frame(&self, layer: &Layer, frame: usize) -> FrameData {
        let prev_k = match layer.keyframes.keys().filter(|&&k| k <= frame).max().copied() {
            Some(k) => k,
            None => return FrameData::default(),
        };
        let prev_fd = match layer.keyframes.get(&prev_k) {
            Some(fd) => fd,
            None => return FrameData::default(),
        };

        let effective_tween = if prev_fd.tween_mode != TweenMode::None {
            prev_fd.tween_mode
        } else if prev_fd.motion_tween {
            TweenMode::Classic
        } else {
            TweenMode::None
        };

        if effective_tween == TweenMode::None || prev_k == frame {
            return prev_fd.clone();
        }

        if let Some(&next_k) = layer.keyframes.keys().filter(|&&k| k > prev_k).min() {
            if frame <= next_k {
                if let Some(next_fd) = layer.keyframes.get(&next_k) {
                    let raw_t = (frame - prev_k) as f32 / (next_k - prev_k).max(1) as f32;
                    let t = prev_fd.easing.apply(raw_t);

                    let mut evaluated = prev_fd.clone();

                    let mut interp_syms = Vec::with_capacity(prev_fd.symbol_instances.len());
                    for (idx, inst) in prev_fd.symbol_instances.iter().enumerate() {
                        if let Some(target) = next_fd.symbol_instances.get(idx) {
                            if target.symbol_name == inst.symbol_name {
                                interp_syms.push(SymbolInstance {
                                    symbol_name: inst.symbol_name.clone(),
                                    x: inst.x + (target.x - inst.x) * t,
                                    y: inst.y + (target.y - inst.y) * t,
                                    scale: inst.scale + (target.scale - inst.scale) * t,
                                    rotation_deg: inst.rotation_deg + (target.rotation_deg - inst.rotation_deg) * t,
                                    anchor_offset_x: inst.anchor_offset_x,
                                    anchor_offset_y: inst.anchor_offset_y,
                                });
                                continue;
                            }
                        }
                        interp_syms.push(inst.clone());
                    }
                    evaluated.symbol_instances = interp_syms;

                    if effective_tween == TweenMode::Shape {
                        let mut morphed_strokes = Vec::with_capacity(prev_fd.strokes.len());
                        for (idx, s0) in prev_fd.strokes.iter().enumerate() {
                            if let Some(s1) = next_fd.strokes.get(idx) {
                                let n = s0.points.len().max(s1.points.len()).clamp(2, 96);
                                let pts0 = s0.resample_points(n);
                                let pts1 = s1.resample_points(n);
                                let mut blended = Vec::with_capacity(n);
                                for (p0, p1) in pts0.iter().zip(pts1.iter()) {
                                    blended.push(StrokePoint {
                                        x: p0.x + (p1.x - p0.x) * t,
                                        y: p0.y + (p1.y - p0.y) * t,
                                        pressure: p0.pressure + (p1.pressure - p0.pressure) * t,
                                    });
                                }
                                let c = [
                                    (s0.color[0] as f32 + (s1.color[0] as f32 - s0.color[0] as f32) * t) as u8,
                                    (s0.color[1] as f32 + (s1.color[1] as f32 - s0.color[1] as f32) * t) as u8,
                                    (s0.color[2] as f32 + (s1.color[2] as f32 - s0.color[2] as f32) * t) as u8,
                                    (s0.color[3] as f32 + (s1.color[3] as f32 - s0.color[3] as f32) * t) as u8,
                                ];
                                morphed_strokes.push(Stroke {
                                    points: blended,
                                    color: c,
                                    fill_color: s0.fill_color.or(s1.fill_color),
                                    base_width: s0.base_width + (s1.base_width - s0.base_width) * t,
                                    is_vector: true,
                                    brush_preset: s0.brush_preset,
                                    opacity: s0.opacity + (s1.opacity - s0.opacity) * t,
                                    brush_texture_id: None,
                                });
                            } else {
                                morphed_strokes.push(s0.clone());
                            }
                        }
                        evaluated.strokes = morphed_strokes;
                    }

                    return evaluated;
                }
            }
        }
        prev_fd.clone()
    }

    pub fn evaluate_symbol_strokes<'a>(&'a self, sym: &'a Symbol, global_frame: usize) -> &'a [Stroke] {
        if sym.nested_keyframes.is_empty() {
            return &sym.strokes;
        }
        let local_f = ((global_frame.saturating_sub(1)) % sym.nested_total_frames.max(1)) + 1;
        if let Some(&k) = sym.nested_keyframes.keys().filter(|&&k| k <= local_f).max() {
            if let Some(st) = sym.nested_keyframes.get(&k) {
                return st;
            }
        }
        &sym.strokes
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        crate::persistence::save_project_safe(self, path)
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        crate::persistence::open_project_safe(path).map(|o| o.project)
    }
}

pub fn parse_svg_to_strokes(svg_path: &Path) -> Result<Vec<Stroke>, String> {
    let content = fs::read_to_string(svg_path).map_err(|e| format!("SVG Read Error: {}", e))?;
    let doc = roxmltree::Document::parse(&content).map_err(|e| format!("SVG Parse Error: {}", e))?;
    let mut result = Vec::new();

    for node in doc.descendants() {
        let tag = node.tag_name().name();
        if tag == "path" {
            if let Some(d) = node.attribute("d") {
                let stroke_col = parse_hex_color(node.attribute("stroke")).unwrap_or([0, 0, 0, 255]);
                let fill_col = parse_hex_color(node.attribute("fill"));
                let width = node
                    .attribute("stroke-width")
                    .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
                    .unwrap_or(4.0);
                let pts = parse_svg_path_d(d);
                if pts.len() >= 2 {
                    result.push(Stroke {
                        points: pts,
                        color: stroke_col,
                        fill_color: fill_col,
                        base_width: width,
                        is_vector: true,
                        brush_preset: KritaBrushPreset::GPenInk,
                        opacity: 1.0,
                        brush_texture_id: None,
                    });
                }
            }
        }
    }

    if result.is_empty() {
        return Err("لم يتم العثور على مسارات فيكتور صالحة داخل ملف SVG.".to_string());
    }
    Ok(result)
}

fn parse_hex_color(attr: Option<&str>) -> Option<[u8; 4]> {
    let s = attr?.trim();
    if s == "none" || s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b, 255]);
        }
    }
    None
}

fn parse_svg_path_d(d: &str) -> Vec<StrokePoint> {
    let mut pts = Vec::new();
    let cleaned = d.replace(',', " ");
    let mut tokens = Vec::new();
    let mut buf = String::new();
    for ch in cleaned.chars() {
        if ch.is_alphabetic() {
            if !buf.trim().is_empty() {
                tokens.push(buf.trim().to_string());
                buf.clear();
            }
            tokens.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !buf.trim().is_empty() {
                tokens.push(buf.trim().to_string());
                buf.clear();
            }
        } else if ch == '-' && !buf.is_empty() && !buf.ends_with('e') && !buf.ends_with('E') {
            tokens.push(buf.trim().to_string());
            buf.clear();
            buf.push(ch);
        } else {
            buf.push(ch);
        }
    }
    if !buf.trim().is_empty() {
        tokens.push(buf.trim().to_string());
    }

    let mut idx = 0;
    let mut cmd = 'M';
    let mut cur_x = 0.0f32;
    let mut cur_y = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;

    while idx < tokens.len() {
        if tokens[idx].len() == 1 && tokens[idx].chars().next().unwrap().is_alphabetic() {
            cmd = tokens[idx].chars().next().unwrap();
            idx += 1;
            if cmd == 'Z' || cmd == 'z' {
                pts.push(StrokePoint { x: start_x, y: start_y, pressure: 1.0 });
                cur_x = start_x;
                cur_y = start_y;
                continue;
            }
        }
        let read_f = |i: &mut usize| -> Option<f32> {
            let v = tokens.get(*i)?.parse::<f32>().ok()?;
            *i += 1;
            Some(v)
        };
        match cmd {
            'M' => {
                if let (Some(x), Some(y)) = (read_f(&mut idx), read_f(&mut idx)) {
                    cur_x = x;
                    cur_y = y;
                    start_x = x;
                    start_y = y;
                    pts.push(StrokePoint { x: cur_x, y: cur_y, pressure: 1.0 });
                    cmd = 'L';
                } else {
                    idx += 1;
                }
            }
            'L' => {
                if let (Some(x), Some(y)) = (read_f(&mut idx), read_f(&mut idx)) {
                    cur_x = x;
                    cur_y = y;
                    pts.push(StrokePoint { x: cur_x, y: cur_y, pressure: 1.0 });
                } else {
                    idx += 1;
                }
            }
            _ => {
                idx += 1;
            }
        }
    }
    pts
}

pub fn import_adobe_animate_project(path: &Path) -> Result<AniProject, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mut xml_content = String::new();

    if ext == "xfl" {
        let parent = path.parent().unwrap_or(Path::new("."));
        let dom_path = parent.join("DOMDocument.xml");
        xml_content = fs::read_to_string(&dom_path)
            .map_err(|_| "لم يتم العثور على DOMDocument.xml بجوار ملف XFL.".to_string())?;
    } else if ext == "fla" {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| "ملف FLA قديم غير مضغوط بصيغة XFL، قم بحفظه كـ XFL من Animate.".to_string())?;
        for i in 0..archive.len() {
            if let Ok(mut entry) = archive.by_index(i) {
                if entry.name().ends_with("DOMDocument.xml") {
                    entry.read_to_string(&mut xml_content).map_err(|e| e.to_string())?;
                    break;
                }
            }
        }
    }

    if xml_content.is_empty() {
        return Err("تعذر قراءة بيانات Adobe Animate.".to_string());
    }

    let doc = roxmltree::Document::parse(&xml_content).map_err(|e| e.to_string())?;
    let mut proj = AniProject::default();
    let root = doc.root_element();
    if let Some(w) = root.attribute("width").and_then(|v| v.parse::<u32>().ok()) {
        proj.width = w;
    }
    if let Some(h) = root.attribute("height").and_then(|v| v.parse::<u32>().ok()) {
        proj.height = h;
    }
    if let Some(fps) = root.attribute("frameRate").and_then(|v| v.parse::<u32>().ok()) {
        proj.fps = fps;
    }

    let mut imported_layers = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "DOMLayer" {
            let l_name = node.attribute("name").unwrap_or("Animate_Layer").to_string();
            let mut kf_map = HashMap::new();
            for frame_node in node.descendants().filter(|n| n.tag_name().name() == "DOMFrame") {
                let idx = frame_node.attribute("index").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0) + 1;
                let mut fd = FrameData::default();
                for edge in frame_node.descendants().filter(|n| n.tag_name().name() == "Edge") {
                    if let Some(edges_str) = edge.attribute("edges") {
                        let pts = parse_animate_edges(edges_str);
                        if pts.len() >= 2 {
                            fd.strokes.push(Stroke {
                                points: pts,
                                color: [0, 0, 0, 255],
                                fill_color: None,
                                base_width: 3.0,
                                is_vector: true,
                                brush_preset: KritaBrushPreset::GPenInk,
                                opacity: 1.0,
                                brush_texture_id: None,
                            });
                        }
                    }
                }
                kf_map.insert(idx, fd);
            }
            if kf_map.is_empty() {
                kf_map.insert(1, FrameData::default());
            }
            imported_layers.push(Layer {
                name: l_name,
                visible: true,
                locked: false,
                is_guide: false,
                xp_first_paint_rewarded: false,
                opacity: 1.0,
                keyframes: kf_map,
            });
        }
    }

    if !imported_layers.is_empty() {
        let max_f = imported_layers.iter().flat_map(|l| l.keyframes.keys().copied()).max().unwrap_or(1);
        proj.layers = imported_layers;
        proj.total_frames = max_f;
    }
    Ok(proj)
}

fn parse_animate_edges(edges: &str) -> Vec<StrokePoint> {
    let mut pts = Vec::new();
    let nums: Vec<f32> = edges
        .split(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    for pair in nums.chunks(2) {
        if pair.len() == 2 {
            pts.push(StrokePoint {
                x: pair[0] / 20.0,
                y: pair[1] / 20.0,
                pressure: 1.0,
            });
        }
    }
    pts
}
