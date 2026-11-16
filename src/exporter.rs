use crate::project::AniProject;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

fn find_executable(name: &str, extra_paths: &[&str]) -> Option<PathBuf> {
    if Command::new(name).arg("-version").output().is_ok() {
        return Some(PathBuf::from(name));
    }
    for p in extra_paths {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn get_ffmpeg_cmd() -> Option<PathBuf> {
    find_executable(
        "ffmpeg",
        &[
            "C:\\ffmpeg\\bin\\ffmpeg.exe",
            "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
            "C:\\ProgramData\\chocolatey\\bin\\ffmpeg.exe",
        ],
    )
}

fn get_blender_cmd() -> Option<PathBuf> {
    find_executable(
        "blender",
        &[
            "C:\\Program Files\\Blender Foundation\\Blender 4.2\\blender.exe",
            "C:\\Program Files\\Blender Foundation\\Blender 4.1\\blender.exe",
            "C:\\Program Files\\Blender Foundation\\Blender 4.0\\blender.exe",
            "C:\\Program Files\\Blender Foundation\\Blender 3.6\\blender.exe",
        ],
    )
}

pub fn render_frame_pixmap(project: &AniProject, frame_idx: usize, transparent_bg: bool) -> Pixmap {
    let w = project.width.max(1);
    let h = project.height.max(1);
    let mut pixmap = Pixmap::new(w, h).unwrap();
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
                for m in &fd.media_items {
                    if let Ok(img) = image::open(&m.path) {
                        let resized = img.resize_exact(m.width.max(1.0) as u32, m.height.max(1.0) as u32, image::imageops::FilterType::Triangle).to_rgba8();
                        if let Some(sub_pix) = tiny_skia::PixmapRef::from_bytes(resized.as_raw(), resized.width(), resized.height()) {
                            pixmap.draw_pixmap(
                                m.x as i32,
                                m.y as i32,
                                sub_pix,
                                &tiny_skia::PixmapPaint::default(),
                                Transform::identity(),
                                None,
                            );
                        }
                    }
                }

                for s in &fd.strokes {
                    if s.points.is_empty() {
                        continue;
                    }
                    let mut pb = PathBuilder::new();
                    pb.move_to(s.points[0].x, s.points[0].y);
                    if s.points.len() == 1 {
                        pb.line_to(s.points[0].x + 0.5, s.points[0].y + 0.5);
                    } else {
                        for p in &s.points[1..] {
                            pb.line_to(p.x, p.y);
                        }
                    }
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(s.color[0], s.color[1], s.color[2], s.color[3]);
                        paint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = s.base_width.max(1.0);
                        stroke.line_cap = tiny_skia::LineCap::Round;
                        stroke.line_join = tiny_skia::LineJoin::Round;
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
                                stroke.width = (s.base_width * inst.scale).max(1.0);
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

/// تصدير الفيديو بأشهر الصيغ (MP4, MOV, MKV, AVI, WEBM, GIF) مع دمج الصوت تلقائياً
pub fn export_video_multi_format(project: &AniProject, output_path: &Path) -> Result<(), String> {
    let ffmpeg = get_ffmpeg_cmd()
        .ok_or_else(|| "FFmpeg not found! Place ffmpeg.exe in C:\\ffmpeg\\bin or system PATH.".to_string())?;

    let temp_dir = std::env::temp_dir().join(format!("stick_export_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let total = project.total_frames.max(1);
    for f in 1..=total {
        let pixmap = render_frame_pixmap(project, f, false);
        let file_path = temp_dir.join(format!("frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| e.to_string())?;
    }

    let input_pattern = temp_dir.join("frame_%04d.png");
    let ext = output_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4")
        .to_lowercase();

    let mut args: Vec<String> = vec![
        "-y".to_string(),
        "-framerate".to_string(),
        project.fps.max(1).to_string(),
        "-i".to_string(),
        input_pattern.to_string_lossy().to_string(),
    ];

    if let Some(track) = project.audio_tracks.first() {
        if Path::new(&track.file_path).exists() && ext != "gif" {
            args.push("-i".to_string());
            args.push(track.file_path.clone());
            args.push("-shortest".to_string());
        }
    }

    match ext.as_str() {
        "webm" => {
            args.extend(["-c:v".into(), "libvpx-vp9".into(), "-pix_fmt".into(), "yuva420p".into()]);
        }
        "avi" => {
            args.extend(["-c:v".into(), "mpeg4".into(), "-q:v".into(), "3".into()]);
        }
        "gif" => {
            args.extend(["-vf".into(), "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".into()]);
        }
        _ => {
            // MP4, MOV, MKV (مع ضمان أبعاد زوجية لـ libx264)
            args.extend([
                "-vf".into(),
                "pad=ceil(iw/2)*2:ceil(ih/2)*2".into(),
                "-c:v".into(),
                "libx264".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
        }
    }

    args.push(output_path.to_string_lossy().to_string());

    let output = Command::new(&ffmpeg)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

    let _ = fs::remove_dir_all(&temp_dir);
    if output.status.success() {
        Ok(())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("Export failed: {}", err_msg.lines().last().unwrap_or("Unknown error")))
    }
}

pub fn extract_video_frames(video_path: &Path, fps: u32) -> Result<Vec<PathBuf>, String> {
    let ffmpeg = get_ffmpeg_cmd()
        .ok_or_else(|| "FFmpeg not found! Place ffmpeg.exe in C:\\ffmpeg\\bin or system PATH.".to_string())?;

    let out_dir = std::env::temp_dir().join(format!("stick_vid_{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let pattern = out_dir.join("vid_%04d.png");
    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-i",
            video_path.to_str().unwrap_or(""),
            "-vf",
            &format!("fps={}", fps.max(1)),
            pattern.to_str().unwrap_or(""),
        ])
        .status()
        .map_err(|e| format!("FFmpeg error: {}", e))?;

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

/// استيراد مشاريع Blender 2D (.blend) وتحويلها لفريمات جاهزة داخل Stick
pub fn import_blender_2d_project(blend_path: &Path) -> Result<Vec<PathBuf>, String> {
    let blender = get_blender_cmd().ok_or_else(|| {
        "Blender not found! Install Blender or add blender.exe to PATH to import .blend files.".to_string()
    })?;

    let out_dir = std::env::temp_dir().join(format!("stick_blend_{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let out_pattern = out_dir.join("blend_####.png");
    let status = Command::new(&blender)
        .args([
            "-b",
            blend_path.to_str().unwrap_or(""),
            "-o",
            out_pattern.to_str().unwrap_or(""),
            "-F",
            "PNG",
            "-a",
        ])
        .status()
        .map_err(|e| format!("Blender execution error: {}", e))?;

    if !status.success() {
        return Err("Blender failed to render 2D frames from .blend file.".to_string());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&out_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("No frames rendered from .blend project.".to_string());
    }
    Ok(files)
}
