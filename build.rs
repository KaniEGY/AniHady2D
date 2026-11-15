use image::{Rgba, RgbaImage};
use std::f32::consts::PI;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn generate_wav(path: &Path, freqs: &[f32], duration_sec: f32) {
    let sample_rate = 44100u32;
    let num_samples = (sample_rate as f32 * duration_sec) as u32;
    let mut data = Vec::with_capacity((num_samples * 2) as usize);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration_sec;
        let freq_idx = ((progress * freqs.len() as f32) as usize).min(freqs.len() - 1);
        let freq = freqs[freq_idx];
        let envelope = (1.0 - progress).powf(1.5);
        let sample = ((2.0 * PI * freq * t).sin() * envelope * 16000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
    }
    let data_size = data.len() as u32;
    let file_size = 36 + data_size;
    let mut file = File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&file_size.to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();
    file.write_all(&data).unwrap();
}

fn main() {
    fs::create_dir_all("assets").unwrap();
    let ico_path = Path::new("assets/icon.ico");
    if !ico_path.exists() {
        let mut img = RgbaImage::from_pixel(64, 64, Rgba([18, 22, 33, 255]));
        let accent = Rgba([0, 210, 255, 255]);
        for y in 16..48 { for x in 15..22 { img.put_pixel(x, y, accent); } }
        for y in 16..48 {
            for x in 29..35 { img.put_pixel(x, y, accent); }
            for x in 44..50 { img.put_pixel(x, y, accent); }
        }
        for y in 29..35 { for x in 29..50 { img.put_pixel(x, y, accent); } }
        img.save(ico_path).unwrap();
    }
    let startup_wav = Path::new("assets/startup.wav");
    if !startup_wav.exists() { generate_wav(startup_wav, &[440.0, 554.37, 659.25, 880.0], 0.45); }
    let shutdown_wav = Path::new("assets/shutdown.wav");
    if !shutdown_wav.exists() { generate_wav(shutdown_wav, &[659.25, 554.37, 440.0], 0.35); }
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
}
