use crate::project::AniProject;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VideoExportFormat {
    Mp4,
    Mov,
    Avi,
    Mkv,
    Webm,
    Gif,
}

impl VideoExportFormat {
    pub fn ext(&self) -> &'static str {
        match self {
            VideoExportFormat::Mp4 => "mp4",
            VideoExportFormat::Mov => "mov",
            VideoExportFormat::Avi => "avi",
            VideoExportFormat::Mkv => "mkv",
            VideoExportFormat::Webm => "webm",
            VideoExportFormat::Gif => "gif",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VideoExportFormat::Mp4 => "MP4 (H.264 - الأكثر انتشاراً)",
            VideoExportFormat::Mov => "MOV (QuickTime)",
            VideoExportFormat::Avi => "AVI (Windows Video)",
            VideoExportFormat::Mkv => "MKV (Matroska)",
            VideoExportFormat::Webm => "WebM (VP9 - شفافية/ويب)",
            VideoExportFormat::Gif => "GIF (صورة متحركة)",
        }
    }
}

/// البحث عن FFmpeg في مجلد البرنامج أو النظام
pub fn find_ffmpeg_executable() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let local_ffmpeg = exe.with_file_name("ffmpeg.exe");
        if local_ffmpeg.exists() {
            return Some(local_ffmpeg);
        }
    }
    if Command::new("ffmpeg").arg("-version").output().is_ok() {
        return Some(PathBuf::from("ffmpeg"));
    }
    let common_paths = [
        r"C:\ffmpeg\bin\ffmpeg.exe",
        r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
        r"C:\Program Files (x86)\ffmpeg\bin\ffmpeg.exe",
    ];
    for p in common_paths {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    None
}

/// تحميل حزمة FFmpeg وفك ضغط ffmpeg.exe تلقائياً داخل مجلد البرنامج
pub fn ensure_ffmpeg_installed(target_dir: &Path) -> Result<PathBuf, String> {
    if let Some(existing) = find_ffmpeg_executable() {
        return Ok(existing);
    }
    let target_exe = target_dir.join("ffmpeg.exe");
    if target_exe.exists() {
        return Ok(target_exe);
    }

    let url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    let resp = ureq::get(url)
        .set("User-Agent", "Stick-Studio-Installer")
        .call()
        .map_err(|e| format!("فشل الاتصال لتحميل FFmpeg: {}", e))?;

    let mut zip_bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut zip_bytes)
        .map_err(|e| format!("خطأ أثناء تحميل FFmpeg: {}", e))?;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("فشل فك ضغط FFmpeg: {}", e))?;

    fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if file.name().ends_with("bin/ffmpeg.exe") || file.name().ends_with("bin\\ffmpeg.exe") {
                let mut out_file = fs::File::create(&target_exe).map_err(|e| format!("فشل حفظ ffmpeg.exe: {}", e))?;
                std::io::copy(&mut file, &mut out_file).map_err(|e| e.to_string())?;
                return Ok(target_exe);
            }
        }
    }

    Err("لم يتم العثور على ffmpeg.exe داخل الحزمة المحملة.".to_string())
}

/// البحث الشامل عن برنامج Blender في كافة الأقراص والمسارات وسجل النظام
pub fn find_blender_executable(custom_path: Option<&str>) -> Option<PathBuf> {
    if let Some(cp) = custom_path {
        let pb = PathBuf::from(cp.trim());
        if pb.exists() && pb.is_file() {
            return Some(pb);
        }
    }
    if Command::new("blender").arg("--version").output().is_ok() {
        return Some(PathBuf::from("blender"));
    }

    // البحث في سجل الويندوز (Registry) عن مسار تثبيت Blender أو ارتباط ملفات .blend
    if let Ok(out) = Command::new("reg")
        .args(["query", r"HKEY_CLASSES_ROOT\blendfile\shell\open\command", "/ve"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for part in stdout.split('"') {
            if part.to_lowercase().ends_with("blender.exe") {
                let pb = PathBuf::from(part);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }

    // البحث في كافة الأقراص C, D, E, F والمجلدات القياسية و Steam
    let drives = ["C:", "D:", "E:", "F:"];
    let sub_dirs = [
        r"Program Files\Blender Foundation",
        r"Program Files (x86)\Blender Foundation",
        r"Program Files (x86)\Steam\steamapps\common\Blender",
        r"SteamLibrary\steamapps\common\Blender",
        r"Blender",
    ];
    for drv in drives {
        for sub in sub_dirs {
            let root = PathBuf::from(format!("{}\\{}", drv, sub));
            if root.join("blender.exe").exists() {
                return Some(root.join("blender.exe"));
            }
            if let Ok(entries) = fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let exe_path = entry.path().join("blender.exe");
                    if exe_path.exists() {
                        return Some(exe_path);
                    }
                }
            }
        }
    }

    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app).join("Programs").join("Blender Foundation");
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                let exe_path = entry.path().join("blender.exe");
                if exe_path.exists() {
                    return Some(exe_path);
                }
            }
        }
    }
    None
}

pub fn render_frame_pixmap(project: &AniProject, frame_idx: usize, transparent_bg: bool, bg_rgba: [u8; 4]) -> Pixmap {
    let w = project.width.max(1);
    let h = project.height.max(1);
    let mut pixmap = Pixmap::new(w, h).unwrap();

    if !transparent_bg {
        pixmap.fill(Color::from_rgba8(bg_rgba[0], bg_rgba[1], bg_rgba[2], 255));
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
                        let resized = img
                            .resize_exact(
                                m.width.max(1.0) as u32,
                                m.height.max(1.0) as u32,
                                image::imageops::FilterType::Triangle,
                            )
                            .to_rgba8();
                        if let Some(mut sub_pix) = Pixmap::new(resized.width(), resized.height()) {
                            sub_pix.data_mut().copy_from_slice(resized.as_raw());
                            pixmap.draw_pixmap(
                                m.x as i32,
                                m.y as i32,
                                sub_pix.as_ref(),
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
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(s.color[0], s.color[1], s.color[2], s.color[3]);
                    paint.anti_alias = true;

                    if s.points.len() == 1 {
                        let mut pb = PathBuilder::new();
                        pb.push_circle(
                            s.points[0].x,
                            s.points[0].y,
                            (s.base_width * s.points[0].pressure * 0.5).max(1.0),
                        );
                        if let Some(path) = pb.finish() {
                            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                        }
                        continue;
                    }

                    let mut pb = PathBuilder::new();
                    pb.move_to(s.points[0].x, s.points[0].y);
                    for p in &s.points[1..] {
                        pb.line_to(p.x, p.y);
                    }
                    if let Some(path) = pb.finish() {
                        if let Some(fc) = s.fill_color {
                            let mut fill_paint = Paint::default();
                            fill_paint.set_color_rgba8(fc[0], fc[1], fc[2], fc[3]);
                            fill_paint.anti_alias = true;
                            pixmap.fill_path(&path, &fill_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                        }
                        let mut stroke = Stroke::default();
                        stroke.width = s.base_width;
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
                                stroke.width = s.base_width * inst.scale;
                                stroke.line_cap = tiny_skia::LineCap::Round;
                                stroke.line_join = tiny_skia::LineJoin::Round;
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
        let pixmap = render_frame_pixmap(project, f, true, [255, 255, 255, 255]);
        let file_path = folder.join(format!("stick_frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| format!("PNG save error: {}", e))?;
    }
    Ok(total)
}

pub fn export_single_png(project: &AniProject, frame_idx: usize, out_path: &Path, transparent: bool, bg_rgba: [u8; 4]) -> Result<(), String> {
    let pixmap = render_frame_pixmap(project, frame_idx, transparent, bg_rgba);
    pixmap.save_png(out_path).map_err(|e| format!("PNG save error: {}", e))
}

pub fn export_video_multi_format(
    project: &AniProject,
    output_path: &Path,
    format: VideoExportFormat,
    bg_rgba: [u8; 4],
) -> Result<(), String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let ffmpeg_bin = ensure_ffmpeg_installed(&exe_dir)?;

    let temp_dir = std::env::temp_dir().join(format!("stick_export_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let total = project.total_frames.max(1);
    let is_transparent = format == VideoExportFormat::Webm;
    for f in 1..=total {
        let pixmap = render_frame_pixmap(project, f, is_transparent, bg_rgba);
        let file_path = temp_dir.join(format!("frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| e.to_string())?;
    }

    let input_pattern = temp_dir.join("frame_%04d.png");
    let fps_str = project.fps.max(1).to_string();
    let in_str = input_pattern.to_str().unwrap_or("");
    let out_str = output_path.to_str().unwrap_or("output.mp4");

    let mut cmd = Command::new(&ffmpeg_bin);
    cmd.args(["-y", "-framerate", &fps_str, "-i", in_str]);

    match format {
        VideoExportFormat::Mp4 => {
            cmd.args([
                "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                "-c:v", "libx264",
                "-preset", "fast",
                "-crf", "18",
                "-pix_fmt", "yuv420p",
                out_str,
            ]);
        }
        VideoExportFormat::Mov => {
            cmd.args([
                "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-f", "mov",
                out_str,
            ]);
        }
        VideoExportFormat::Avi => {
            cmd.args(["-c:v", "mpeg4", "-q:v", "3", out_str]);
        }
        VideoExportFormat::Mkv => {
            cmd.args([
                "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                out_str,
            ]);
        }
        VideoExportFormat::Webm => {
            cmd.args(["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", out_str]);
        }
        VideoExportFormat::Gif => {
            cmd.args(["-vf", "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse", out_str]);
        }
    }

    let output = cmd.output().map_err(|e| format!("FFmpeg launch error: {}", e))?;
    let _ = fs::remove_dir_all(&temp_dir);

    if output.status.success() {
        Ok(())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("فشل التصدير: {}", err_msg.lines().last().unwrap_or("Unknown error")))
    }
}

pub fn extract_video_frames(video_path: &Path, fps: u32) -> Result<Vec<PathBuf>, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let ffmpeg_bin = ensure_ffmpeg_installed(&exe_dir)?;

    let out_dir = std::env::temp_dir().join(format!("stick_vid_{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let pattern = out_dir.join("vid_%04d.png");
    let status = Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-i",
            video_path.to_str().unwrap_or(""),
            "-vf",
            &format!("fps={}", fps.max(1)),
            pattern.to_str().unwrap_or(""),
        ])
        .status()
        .map_err(|_| "Failed to execute FFmpeg.".to_string())?;

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

pub fn import_blender_project_frames(blend_path: &Path, custom_blender: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let blender_bin = find_blender_executable(custom_blender)
        .ok_or_else(|| "BLENDER_NOT_FOUND".to_string())?;

    let out_dir = std::env::temp_dir().join(format!("stick_blend_{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let out_pattern = out_dir.join("blend_####.png");
    let status = Command::new(&blender_bin)
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
        .map_err(|e| format!("Blender error: {}", e))?;

    if !status.success() {
        return Err("فشل استخراج المشهد من ملف Blender.".to_string());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&out_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("لم يتم العثور على فريمات مرئية داخل ملف .blend".to_string());
    }
    Ok(files)
}
