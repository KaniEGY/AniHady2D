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
    pub pressure: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Stroke {
    pub points: Vec<StrokePoint>,
    pub color: [u8; 4],
    pub fill_color: Option<[u8; 4]>,
    pub base_width: f32,
    pub is_vector: bool,
    pub brush_texture_id: Option<String>,
}

impl Stroke {
    pub fn smooth_vector(&mut self) {
        if self.points.len() < 3 {
            return;
        }
        let mut smoothed = Vec::with_capacity(self.points.len());
        smoothed.push(self.points[0].clone());
        for i in 1..self.points.len() - 1 {
            let p0 = &self.points[i - 1];
            let p1 = &self.points[i];
            let p2 = &self.points[i + 1];
            smoothed.push(StrokePoint {
                x: p0.x * 0.25 + p1.x * 0.5 + p2.x * 0.25,
                y: p0.y * 0.25 + p1.y * 0.5 + p2.y * 0.25,
                pressure: p1.pressure,
            });
        }
        smoothed.push(self.points.last().unwrap().clone());
        self.points = smoothed;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextItem {
    pub content: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [u8; 4],
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MediaType {
    Image,
    VideoFrame,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaItem {
    pub path: String,
    pub media_type: MediaType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub strokes: Vec<Stroke>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SymbolInstance {
    pub symbol_name: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct FrameData {
    pub strokes: Vec<Stroke>,
    pub media_items: Vec<MediaItem>,
    pub symbol_instances: Vec<SymbolInstance>,
    #[serde(default)]
    pub text_items: Vec<TextItem>,
    #[serde(default)]
    pub motion_tween: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub keyframes: HashMap<usize, FrameData>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioTrack {
    pub name: String,
    pub file_path: String,
    pub start_frame: usize,
    #[serde(default = "default_end_frame")]
    pub end_frame: usize,
    #[serde(default)]
    pub sync_mode: AudioSyncMode,
    #[serde(default)]
    pub waveform_peaks: Vec<f32>,
}

fn default_end_frame() -> usize {
    120
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OnionSkinSettings {
    pub enabled: bool,
    pub frames_before: usize,
    pub frames_after: usize,
    pub color_before: [u8; 4],
    pub color_after: [u8; 4],
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
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub total_frames: usize,
    pub layers: Vec<Layer>,
    pub audio_tracks: Vec<AudioTrack>,
    pub library_symbols: HashMap<String, Symbol>,
    pub onion_skin: OnionSkinSettings,
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

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let raw = fs::read(path).map_err(|e| format!("Read error: {}", e))?;
        if let Ok(decompressed) = zstd::decode_all(&raw[..]) {
            if let Ok(proj) = serde_json::from_slice::<AniProject>(&decompressed) {
                return Ok(proj);
            }
            if let Ok(proj) = rmp_serde::from_slice::<AniProject>(&decompressed) {
                return Ok(proj);
            }
        }
        serde_json::from_slice::<AniProject>(&raw).map_err(|e| format!("Invalid project file: {}", e))
    }
}
