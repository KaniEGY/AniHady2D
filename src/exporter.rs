use crate::project::AniProject;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub fn export_to_mp4(project: &AniProject, output_path: &Path) -> Result<(), String> {
    let (width, height) = (project.width, project.height);
    let mut args = vec![
        "-y".to_string(), "-f".to_string(), "rawvideo".to_string(), "-vcodec".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "rgba".to_string(), "-s".to_string(), format!("{}x{}", width, height),
        "-r".to_string(), project.fps.to_string(), "-i".to_string(), "-".to_string(),
    ];
    if let Some(audio) = project.audio_tracks.first() {
        args.extend(["-i".to_string(), audio.file_path.clone(), "-shortest".to_string()]);
    }
    args.extend([
        "-c:v".to_string(), "libx264".to_string(), "-pix_fmt".to_string(), "yuv420p".to_string(),
        "-preset".to_string(), "fast".to_string(), "-crf".to_string(), "18".to_string(),
        output_path.to_str().unwrap_or("output.mp4").to_string(),
    ]);
    let mut ffmpeg = Command::new("ffmpeg").args(&args).stdin(Stdio::piped()).spawn().map_err(|e| format!("تأكد من FFmpeg: {}", e))?;
    let mut stdin = ffmpeg.stdin.take().ok_or("فشل فتح FFmpeg")?;
    for frame_idx in 1..=project.total_frames {
        let mut pixmap = Pixmap::new(width, height).ok_or("فشل إنشاء مساحة الرندر")?;
        pixmap.fill(Color::WHITE);
        for layer in &project.layers {
            if !layer.visible { continue; }
            if let Some(frame_data) = layer.keyframes.get(&frame_idx) {
                for stroke in &frame_data.strokes { render_skia_stroke(&mut pixmap, stroke, 0.0, 0.0, 1.0); }
                for inst in &frame_data.symbol_instances {
                    if let Some(sym) = project.library_symbols.get(&inst.symbol_name) {
                        for stroke in &sym.strokes { render_skia_stroke(&mut pixmap, stroke, inst.x, inst.y, inst.scale); }
                    }
                }
            }
        }
        stdin.write_all(pixmap.data()).map_err(|e| format!("خطأ في الفريم: {}", e))?;
    }
    drop(stdin);
    ffmpeg.wait().map_err(|e| format!("خطأ في التصدير: {}", e))?;
    Ok(())
}

fn render_skia_stroke(pixmap: &mut Pixmap, stroke: &crate::project::Stroke, ox: f32, oy: f32, scale: f32) {
    if stroke.points.len() < 2 { return; }
    if let Some(fc) = stroke.fill_color {
        let mut pb = PathBuilder::new();
        pb.move_to(stroke.points[0].x * scale + ox, stroke.points[0].y * scale + oy);
        for p in &stroke.points[1..] { pb.line_to(p.x * scale + ox, p.y * scale + oy); }
        pb.close();
        if let Some(path) = pb.finish() {
            let mut fill_paint = Paint::default();
            fill_paint.set_color_rgba8(fc[0], fc[1], fc[2], fc[3]);
            pixmap.fill_path(&path, &fill_paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    let mut paint = Paint::default();
    paint.set_color_rgba8(stroke.color[0], stroke.color[1], stroke.color[2], stroke.color[3]);
    paint.anti_alias = true;
    for window in stroke.points.windows(2) {
        let mut pb = PathBuilder::new();
        pb.move_to(window[0].x * scale + ox, window[0].y * scale + oy);
        pb.line_to(window[1].x * scale + ox, window[1].y * scale + oy);
        if let Some(path) = pb.finish() {
            let mut skia_stroke = Stroke::default();
            skia_stroke.width = stroke.base_width * window[1].pressure * scale;
            pixmap.stroke_path(&path, &paint, &skia_stroke, Transform::identity(), None);
        }
    }
}

pub fn extract_video_frames(video_path: &Path, fps: u32) -> Result<Vec<PathBuf>, String> {
    let temp_dir = std::env::temp_dir().join("AniHady2D_ImportedVideo");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    let pattern = temp_dir.join("frame_%04d.png");
    let status = Command::new("ffmpeg").args(["-i", video_path.to_str().unwrap_or(""), "-vf", &format!("fps={}", fps), pattern.to_str().unwrap_or("")]).status().map_err(|e| format!("فشل FFmpeg: {}", e))?;
    if !status.success() { return Err("فشل استخراج الفيديو".to_string()); }
    let mut frames = Vec::new();
    if let Ok(entries) = fs::read_dir(&temp_dir) { for entry in entries.flatten() { frames.push(entry.path()); } }
    frames.sort();
    Ok(frames)
}
