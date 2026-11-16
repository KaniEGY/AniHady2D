use crate::project::AniProject;
use image::ImageFormat;
use std::fs;
use std::io::{Cursor, Read};
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ImageExportFormat {
    PngTransparent,
    PngSolid,
    Jpg,
    Webp,
    Bmp,
    Tiff,
}

impl ImageExportFormat {
    pub fn ext(&self) -> &'static str {
        match self {
            ImageExportFormat::PngTransparent | ImageExportFormat::PngSolid => "png",
            ImageExportFormat::Jpg => "jpg",
            ImageExportFormat::Webp => "webp",
            ImageExportFormat::Bmp => "bmp",
            ImageExportFormat::Tiff => "tiff",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ImageExportFormat::PngTransparent => "PNG (خلفية شفافة - Transparent)",
            ImageExportFormat::PngSolid => "PNG (بخلفية المسرح - High Quality)",
            ImageExportFormat::Jpg => "JPG / JPEG (صورة مضغوطة)",
            ImageExportFormat::Webp => "WebP (صيغة الويب الحديثة)",
            ImageExportFormat::Bmp => "BMP (Bitmap غير مضغوط)",
            ImageExportFormat::Tiff => "TIFF (جودة طباعة عالية)",
        }
    }

    pub fn is_transparent(&self) -> bool {
        matches!(self, ImageExportFormat::PngTransparent | ImageExportFormat::Webp)
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

/// تحميل حزمة FFmpeg وفك ضغط ffmpeg.exe تلقائياً (تُستدعى في خيط خلفي حتى لا يتجمد البرنامج)
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
        .set("User-Agent", "Stick-Studio")
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

/// تصدير الرسمة أو الفريم الحالي بأشهر صيغ الصور (PNG, JPG, WebP, BMP, TIFF)
pub fn export_single_image_format(
    project: &AniProject,
    frame_idx: usize,
    out_path: &Path,
    format: ImageExportFormat,
    bg_rgba: [u8; 4],
) -> Result<(), String> {
    let pixmap = render_frame_pixmap(project, frame_idx, format.is_transparent(), bg_rgba);
    let w = pixmap.width();
    let h = pixmap.height();

    // فك ضرب الألفا (Unpremultiply) لاستخراج ألوان RGBA دقيقة
    let mut raw_rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        raw_rgba.push(c.red());
        raw_rgba.push(c.green());
        raw_rgba.push(c.blue());
        raw_rgba.push(c.alpha());
    }

    let img_buf = image::RgbaImage::from_raw(w, h, raw_rgba)
        .ok_or_else(|| "فشل تجهيز بيانات الصورة.".to_string())?;
    let dyn_img = image::DynamicImage::ImageRgba8(img_buf);

    match format {
        ImageExportFormat::PngTransparent | ImageExportFormat::PngSolid => {
            dyn_img.save_with_format(out_path, ImageFormat::Png).map_err(|e| e.to_string())
        }
        ImageExportFormat::Jpg => {
            image::DynamicImage::ImageRgb8(dyn_img.to_rgb8())
                .save_with_format(out_path, ImageFormat::Jpeg)
                .map_err(|e| e.to_string())
        }
        ImageExportFormat::Webp => {
            dyn_img.save_with_format(out_path, ImageFormat::WebP).map_err(|e| e.to_string())
        }
        ImageExportFormat::Bmp => {
            image::DynamicImage::ImageRgb8(dyn_img.to_rgb8())
                .save_with_format(out_path, ImageFormat::Bmp)
                .map_err(|e| e.to_string())
        }
        ImageExportFormat::Tiff => {
            dyn_img.save_with_format(out_path, ImageFormat::Tiff).map_err(|e| e.to_string())
        }
    }
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
