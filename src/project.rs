use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum ToolType { VectorPen, Brush, Eraser, Bucket, Select, Eyedropper }

#[derive(Serialize, Deserialize, Clone)]
pub struct StrokePoint { pub x: f32, pub y: f32, pub pressure: f32 }

#[derive(Serialize, Deserialize, Clone)]
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
        if !self.is_vector || self.points.len() < 3 { return; }
        let mut smoothed = Vec::with_capacity(self.points.len() * 2);
        smoothed.push(self.points[0].clone());
        for w in self.points.windows(2) {
            let (p0, p1) = (&w[0], &w[1]);
            smoothed.push(StrokePoint { x: 0.75 * p0.x + 0.25 * p1.x, y: 0.75 * p0.y + 0.25 * p1.y, pressure: 0.75 * p0.pressure + 0.25 * p1.pressure });
            smoothed.push(StrokePoint { x: 0.25 * p0.x + 0.75 * p1.x, y: 0.25 * p0.y + 0.75 * p1.y, pressure: 0.25 * p0.pressure + 0.75 * p1.pressure });
        }
        smoothed.push(self.points.last().unwrap().clone());
        self.points = smoothed;
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Symbol { pub name: String, pub strokes: Vec<Stroke> }

#[derive(Serialize, Deserialize, Clone)]
pub struct SymbolInstance { pub symbol_name: String, pub x: f32, pub y: f32, pub scale: f32 }

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum MediaType { Image, VideoFrame }

#[derive(Serialize, Deserialize, Clone)]
pub struct MediaItem { pub path: String, pub media_type: MediaType, pub x: f32, pub y: f32, pub width: f32, pub height: f32 }

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FrameData { pub strokes: Vec<Stroke>, pub symbol_instances: Vec<SymbolInstance>, pub media_items: Vec<MediaItem> }

#[derive(Serialize, Deserialize, Clone)]
pub struct Layer { pub name: String, pub visible: bool, pub locked: bool, pub keyframes: HashMap<usize, FrameData> }

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioTrack { pub name: String, pub file_path: String, pub start_frame: usize }

#[derive(Serialize, Deserialize, Clone)]
pub struct OnionSkinSettings { pub enabled: bool, pub frames_before: usize, pub frames_after: usize, pub color_before: [u8; 4], pub color_after: [u8; 4] }

#[derive(Serialize, Deserialize, Clone)]
pub struct AniProject {
    pub name: String, pub width: u32, pub height: u32, pub fps: u32, pub total_frames: usize,
    pub layers: Vec<Layer>, pub audio_tracks: Vec<AudioTrack>, pub library_symbols: HashMap<String, Symbol>, pub onion_skin: OnionSkinSettings,
}

impl Default for AniProject {
    fn default() -> Self {
        let mut initial_keyframes = HashMap::new();
        initial_keyframes.insert(1, FrameData::default());
        Self {
            name: "مشروع_جديد".to_string(), width: 800, height: 450, fps: 24, total_frames: 120,
            layers: vec![Layer { name: "Layer 1".to_string(), visible: true, locked: false, keyframes: initial_keyframes }],
            audio_tracks: Vec::new(), library_symbols: HashMap::new(),
            onion_skin: OnionSkinSettings { enabled: true, frames_before: 2, frames_after: 2, color_before: [255, 80, 80, 90], color_after: [80, 255, 80, 90] },
        }
    }
}

impl AniProject {
    pub fn save_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let binary_data = rmp_serde::to_vec(self)?;
        let compressed_data = zstd::encode_all(&binary_data[..], 3)?;
        let mut file = File::create(path)?;
        file.write_all(&compressed_data)?;
        Ok(())
    }
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)?;
        let binary_data = zstd::decode_all(&compressed_data[..])?;
        Ok(rmp_serde::from_slice(&binary_data)?)
    }
}
