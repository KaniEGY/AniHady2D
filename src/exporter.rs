use crate::project::AniProject;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub fn render_frame_pixmap(project: &AniProject, frame_idx: usize, transparent_bg: bool) -> Pixmap {
    let mut pixmap = Pixmap::new(project.width.max(1), project.height.max(1)).unwrap();
    if !transparent_bg {
        pixmap.fill(Color::WHITE);
    }

    for layer in project.layers.iter().rev() {
        if !layer.visible {
            continue;
        }
        let eff_key = layer.keyframes.keys().filter(|&&k| k <= frame_idx).max().copied();
        if let Some(k) = eff_key {
            if let Some(fd) = layer.keyframes.get(&k) {
                for s in &fd.strokes {
                    if s.points.len() < 2 {
                        continue;
                    }
                    let mut pb = PathBuilder::new();
                    pb.move_to(s.points[0].x, s.points[0].y);
                    for p in &s.points[1..] {
                        pb.line_to(p.x, p.y);
                    }
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(s.color[0], s.color[1], s.color[2], s.color[3]);
                        paint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = s.base_width;
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                for inst in &fd.symbol_instances {
                    if let Some(sym) = project.library_symbols.get(&inst.symbol_name) {
                        for s in &sym.strokes {
                            if s.points.len() < 2 {
                                continue;
                            }
                            let mut pb = PathBuilder::new();
                            pb.move_to(s.points[0].x * inst.scale + inst.x, s.points[0].y * inst.scale + inst.y);
                            for p in &s.points[1..] {
                                pb.line_to(p.x * inst.scale + inst.x, p.y * inst.scale + inst.y);
                            }
                            if let Some(path) = pb.finish() {
                                let mut paint = Paint::default();
                                paint.set_color_rgba8(s.color[0], s.color[1], s.color[2], s.color[3]);
                                paint.anti_alias = true;
                                let mut stroke = Stroke::default();
                                stroke.width = s.base_width * inst.scale;
                                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                            }
                        }
                    }
                }
            }
        }
    }
    pixmap
}

pub fn export_transparent_png_sequence(project: &AniProject, folder: &Path) -> Result<usize, String> {
    fs::create_dir_all(folder).map_err(|e| format!("Failed to create folder: {}", e))?;
    let total = project.total_frames.max(1);
    for f in 1..=total {
        let pixmap = render_frame_pixmap(project, f, true);
        let file_path = folder.join(format!("frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| format!("PNG save error: {}", e))?;
    }
    Ok(total)
}

pub fn export_to_mp4(project: &AniProject, output_path: &Path) -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join("anihady_export_frames");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let total = project.total_frames.max(1);
    for f in 1..=total {
        let pixmap = render_frame_pixmap(project, f, false);
        let file_path = temp_dir.join(format!("frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| e.to_string())?;
    }

    let input_pattern = temp_dir.join("frame_%04d.png");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-framerate",
            &project.fps.to_string(),
            "-i",
            input_pattern.to_str().unwrap_or(""),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            output_path.to_str().unwrap_or("output.mp4"),
        ])
        .status()
        .map_err(|_| "FFmpeg not found! Install FFmpeg or use Transparent PNG Sequence export.".to_string())?;

    let _ = fs::remove_dir_all(&temp_dir);
    if status.success() {
        Ok(())
    } else {
        Err("FFmpeg encoding failed.".to_string())
    }
}

pub fn extract_video_frames(video_path: &Path, fps: u32) -> Result<Vec<PathBuf>, String> {
    let out_dir = std::env::temp_dir().join(format!("anihady_vid_{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let pattern = out_dir.join("vid_%04d.png");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            video_path.to_str().unwrap_or(""),
            "-vf",
            &format!("fps={}", fps.max(1)),
            pattern.to_str().unwrap_or(""),
        ])
        .status()
        .map_err(|_| "FFmpeg not found on system.".to_string())?;

    if !status.success() {
        return Err("Failed to extract video frames.".to_string());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&out_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .collect();
    files.sort();
    Ok(files)
}
