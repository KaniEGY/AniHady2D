use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum ToolType {
    Select,
    VectorPen,
    Brush,
    Eraser,
    Bucket,
    Eyedropper,
    Text,
    ShapeLine,
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
    pub brush_texture_id: Option<String>,
}

fn default_black() -> [u8; 4] {
    [0, 0, 0, 255]
}

fn default_width() -> f32 {
    5.0
}

impl Stroke {
    /// تنعيم خطوط الفرشاة العادية بتقنية Chaikin
    pub fn smooth_brush(&mut self) {
        if self.points.len() < 3 {
            return;
        }
        for _ in 0..2 {
            let mut next_pts = Vec::with_capacity(self.points.len() * 2);
            next_pts.push(self.points[0].clone());
            for w in self.points.windows(2) {
                let p0 = &w[0];
                let p1 = &w[1];
                next_pts.push(StrokePoint {
                    x: p0.x * 0.75 + p1.x * 0.25,
                    y: p0.y * 0.75 + p1.y * 0.25,
                    pressure: p0.pressure * 0.75 + p1.pressure * 0.25,
                });
                next_pts.push(StrokePoint {
                    x: p0.x * 0.25 + p1.x * 0.75,
                    y: p0.y * 0.25 + p1.y * 0.75,
                    pressure: p0.pressure * 0.25 + p1.pressure * 0.75,
                });
            }
            next_pts.push(self.points.last().unwrap().clone());
            self.points = next_pts;
        }
    }

    /// تبسيط نقاط الفيكتور (Douglas-Peucker) لترك نقاط تحكم ذكية وقليلة فقط
    pub fn simplify_vector_nodes(&mut self, tolerance: f32) {
        if self.points.len() <= 3 {
            return;
        }
        self.points = rdp_simplify(&self.points, tolerance);
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
    pub motion_tween: bool,
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
}

fn default_w() -> u32 { 1280 }
fn default_h() -> u32 { 720 }
fn default_fps() -> u32 { 24 }

/// هياكل التوافق العكسي (Legacy V1) لقراءة أقدم ملفات .ah2d التي كانت تخزن الفريمات كـ Vec<FrameData>
#[derive(Deserialize)]
struct LegacyLayerV1 {
    #[serde(default = "default_layer_name")]
    name: String,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    frames: Vec<FrameData>,
}

#[derive(Deserialize)]
struct LegacyProjectV1 {
    #[serde(default = "default_w")]
    width: u32,
    #[serde(default = "default_h")]
    height: u32,
    #[serde(default = "default_fps")]
    fps: u32,
    #[serde(default)]
    layers: Vec<LegacyLayerV1>,
}

impl Default for AniProject {
    fn default() -> Self {
        let mut keyframes = HashMap::new();
        keyframes.insert(1, FrameData::default());
        Self {
            width: 1280,
            height: 720,
            fps: 24,
            total_frames: 1,
            layers: vec![Layer {
                name: "Layer 1".to_string(),
                visible: true,
                locked: false,
                keyframes,
            }],
            audio_tracks: Vec::new(),
            library_symbols: HashMap::new(),
            onion_skin: OnionSkinSettings::default(),
        }
    }
}

impl AniProject {
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_vec(self).map_err(|e| format!("Serialize error: {}", e))?;
        let compressed = zstd::encode_all(&json[..], 3).map_err(|e| format!("Compress error: {}", e))?;
        fs::write(path, compressed).map_err(|e| format!("Write error: {}", e))
    }

    /// محوّل المشاريع الشامل: يقرأ أي مشروع من أي إصدار سابق أو حالي بدون فشل
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let raw = fs::read(path).map_err(|e| format!("Read error: {}", e))?;
        let decompressed = zstd::decode_all(&raw[..]).unwrap_or_else(|_| raw.clone());

        // 1. محاولة القراءة بالهيكل الحالي (JSON أو MessagePack)
        let mut loaded: Option<AniProject> = serde_json::from_slice::<AniProject>(&decompressed)
            .ok()
            .or_else(|| rmp_serde::from_slice::<AniProject>(&decompressed).ok())
            .or_else(|| serde_json::from_slice::<AniProject>(&raw).ok())
            .or_else(|| rmp_serde::from_slice::<AniProject>(&raw).ok());

        // 2. إذا كان المشروع قديماً جداً (V1 حيث كانت الفريمات Vec<FrameData>)، يتم تحويله تلقائياً
        if loaded.is_none() {
            let legacy_opt: Option<LegacyProjectV1> = serde_json::from_slice::<LegacyProjectV1>(&decompressed)
                .ok()
                .or_else(|| rmp_serde::from_slice::<LegacyProjectV1>(&decompressed).ok())
                .or_else(|| serde_json::from_slice::<LegacyProjectV1>(&raw).ok())
                .or_else(|| rmp_serde::from_slice::<LegacyProjectV1>(&raw).ok());

            if let Some(leg) = legacy_opt {
                let mut converted_layers = Vec::new();
                let mut max_len = 1usize;
                for l in leg.layers {
                    let mut kf_map = HashMap::new();
                    for (idx, fd) in l.frames.into_iter().enumerate() {
                        kf_map.insert(idx + 1, fd);
                    }
                    if kf_map.is_empty() {
                        kf_map.insert(1, FrameData::default());
                    }
                    max_len = max_len.max(kf_map.len());
                    converted_layers.push(Layer {
                        name: l.name,
                        visible: l.visible,
                        locked: l.locked,
                        keyframes: kf_map,
                    });
                }
                loaded = Some(AniProject {
                    width: leg.width,
                    height: leg.height,
                    fps: leg.fps,
                    total_frames: max_len,
                    layers: converted_layers,
                    audio_tracks: Vec::new(),
                    library_symbols: HashMap::new(),
                    onion_skin: OnionSkinSettings::default(),
                });
            }
        }

        match loaded {
            Some(mut proj) => {
                if proj.layers.is_empty() {
                    let mut kf = HashMap::new();
                    kf.insert(1, FrameData::default());
                    proj.layers.push(Layer {
                        name: "Layer 1".to_string(),
                        visible: true,
                        locked: false,
                        keyframes: kf,
                    });
                }
                let max_kf = proj
                    .layers
                    .iter()
                    .flat_map(|l| l.keyframes.keys().copied())
                    .max()
                    .unwrap_or(1);
                proj.total_frames = proj.total_frames.max(max_kf).max(1);
                Ok(proj)
            }
            None => Err("تعذر قراءة ملف المشروع، تأكد من أن الملف غير تالف.".to_string()),
        }
    }
}
