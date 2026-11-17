use crate::project::AniProject;
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageFormat, RgbaImage};
use rayon::prelude::*;
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
    GifInstant,
}

impl VideoExportFormat {
    pub fn ext(&self) -> &'static str {
        match self {
            VideoExportFormat::Mp4 => "mp4",
            VideoExportFormat::Mov => "mov",
            VideoExportFormat::Avi => "avi",
            VideoExportFormat::Mkv => "mkv",
            VideoExportFormat::Webm => "webm",
            VideoExportFormat::GifInstant => "gif",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VideoExportFormat::Mp4 => "MP4 (H.264 - الأكثر انتشاراً)",
            VideoExportFormat::Mov => "MOV (QuickTime)",
            VideoExportFormat::Avi => "AVI (Windows Video)",
            VideoExportFormat::Mkv => "MKV (Matroska)",
            VideoExportFormat::Webm => "WebM (VP9 - شفافية/ويب)",
            VideoExportFormat::GifInstant => "GIF متحرك (فوري - بدون الحاجة لـ FFmpeg!)",
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

    // Composite every layer only once with its opacity. This ensures that
    // overlapping strokes in a layer are not individually dimmed again.
    for layer in project.layers.iter().rev() {
        if !layer.visible || layer.is_guide {
            continue;
        }
        let layer_alpha = if layer.opacity.is_finite() {
            layer.opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if layer_alpha <= 0.0 {
            continue;
        }

        let mut layer_pixmap = Pixmap::new(w, h).expect("Canvas dimensions already checked");
        let fd = project.evaluate_layer_at_frame(layer, frame_idx);

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
                    layer_pixmap.draw_pixmap(
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
            draw_skia_stroke(&mut layer_pixmap, s, Transform::identity(), 1.0);
        }

        for inst in &fd.symbol_instances {
            if let Some(sym) = project.library_symbols.get(&inst.symbol_name) {
                let sym_strokes = project.evaluate_symbol_strokes(sym, frame_idx);
                let ts = Transform::from_translate(-inst.anchor_offset_x, -inst.anchor_offset_y)
                    .post_scale(inst.scale, inst.scale)
                    .post_rotate(inst.rotation_deg)
                    .post_translate(inst.x + inst.anchor_offset_x, inst.y + inst.anchor_offset_y);
                for s in sym_strokes {
                    draw_skia_stroke(&mut layer_pixmap, s, ts, inst.scale);
                }
            }
        }

        pixmap.draw_pixmap(
            0,
            0,
            layer_pixmap.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity: layer_alpha,
                ..tiny_skia::PixmapPaint::default()
            },
            Transform::identity(),
            None,
        );
    }
    pixmap
}

fn draw_skia_stroke(pixmap: &mut Pixmap, s: &crate::project::Stroke, ts: Transform, width_scale: f32) {
    if s.points.is_empty() {
        return;
    }
    let alpha = ((s.color[3] as f32) * s.opacity.clamp(0.0, 1.0)) as u8;
    let mut paint = Paint::default();
    paint.set_color_rgba8(s.color[0], s.color[1], s.color[2], alpha);
    paint.anti_alias = true;

    // Share the G-Pen pressure sampler with the live canvas preview.
    if !s.is_vector && s.brush_preset == crate::project::KritaBrushPreset::GPenInk {
        // Bucket fill must be exported beneath the brush outline, too.
        if let Some(fc) = s.fill_color {
            if s.points.len() >= 3 {
                let mut region = PathBuilder::new();
                region.move_to(s.points[0].x, s.points[0].y);
                for p in &s.points[1..] { region.line_to(p.x, p.y); }
                region.close();
                if let Some(path) = region.finish() {
                    let mut fill_paint = Paint::default();
                    fill_paint.set_color_rgba8(fc[0], fc[1], fc[2],
                        ((fc[3] as f32) * s.opacity.clamp(0.0, 1.0)) as u8);
                    fill_paint.anti_alias = true;
                    pixmap.fill_path(&path, &fill_paint, tiny_skia::FillRule::Winding, ts, None);
                }
            }
        }
        for dab in crate::brush_engine::sample_pressure_dabs(&s.points, s.base_width) {
            let mut pb = PathBuilder::new();
            pb.push_circle(dab.x, dab.y, dab.radius);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, ts, None);
            }
        }
        return;
    }

    if s.points.len() == 1 {
        let mut pb = PathBuilder::new();
        pb.push_circle(
            s.points[0].x,
            s.points[0].y,
            (s.base_width * s.points[0].pressure * 0.5).max(1.0),
        );
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, ts, None);
        }
        return;
    }

    let mut pb = PathBuilder::new();
    pb.move_to(s.points[0].x, s.points[0].y);
    for p in &s.points[1..] {
        pb.line_to(p.x, p.y);
    }
    if let Some(path) = pb.finish() {
        if let Some(fc) = s.fill_color {
            let mut fill_paint = Paint::default();
            fill_paint.set_color_rgba8(fc[0], fc[1], fc[2], ((fc[3] as f32) * s.opacity.clamp(0.0, 1.0)) as u8);
            fill_paint.anti_alias = true;
            pixmap.fill_path(&path, &fill_paint, tiny_skia::FillRule::Winding, ts, None);
        }
        let mut stroke = Stroke::default();
        stroke.width = (s.base_width * width_scale).max(1.0);
        stroke.line_cap = tiny_skia::LineCap::Round;
        stroke.line_join = tiny_skia::LineJoin::Round;
        pixmap.stroke_path(&path, &paint, &stroke, ts, None);
    }
}

fn pixmap_to_rgba_image(pixmap: &Pixmap) -> RgbaImage {
    let w = pixmap.width();
    let h = pixmap.height();
    let mut raw_rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        raw_rgba.push(c.red());
        raw_rgba.push(c.green());
        raw_rgba.push(c.blue());
        raw_rgba.push(c.alpha());
    }
    RgbaImage::from_raw(w, h, raw_rgba).unwrap_or_else(|| RgbaImage::new(w, h))
}

pub fn export_single_image_format(
    project: &AniProject,
    frame_idx: usize,
    out_path: &Path,
    format: ImageExportFormat,
    bg_rgba: [u8; 4],
) -> Result<(), String> {
    let pixmap = render_frame_pixmap(project, frame_idx, format.is_transparent(), bg_rgba);
    let img_buf = pixmap_to_rgba_image(&pixmap);
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
    let frames: Vec<usize> = (1..=total).collect();

    frames.par_iter().try_for_each(|&f| {
        let pixmap = render_frame_pixmap(project, f, true, [255, 255, 255, 255]);
        let file_path = folder.join(format!("stick_frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| format!("PNG save error: {}", e))
    })?;

    Ok(total)
}

pub fn export_sprite_sheet(project: &AniProject, out_path: &Path) -> Result<usize, String> {
    let total = project.total_frames.max(1);
    let fw = project.width.max(1);
    let fh = project.height.max(1);

    let frames: Vec<RgbaImage> = (1..=total)
        .into_par_iter()
        .map(|f| {
            let pix = render_frame_pixmap(project, f, true, [255, 255, 255, 255]);
            pixmap_to_rgba_image(&pix)
        })
        .collect();

    let mut sheet = RgbaImage::new(fw * (total as u32), fh);
    for (idx, frame_img) in frames.iter().enumerate() {
        image::imageops::overlay(&mut sheet, frame_img, (idx as i64) * (fw as i64), 0);
    }
    sheet.save_with_format(out_path, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(total)
}

pub fn export_lottie_json(project: &AniProject, out_path: &Path) -> Result<(), String> {
    let total = project.total_frames.max(1);
    let fps = project.fps.max(1);
    let mut layers_json = Vec::new();

    for (l_idx, layer) in project.layers.iter().enumerate() {
        if !layer.visible || layer.is_guide {
            continue;
        }
        let fd = project.evaluate_layer_at_frame(layer, 1);
        let mut shapes = Vec::new();
        for s in &fd.strokes {
            if s.points.len() < 2 {
                continue;
            }
            let vertices: Vec<[f32; 2]> = s.points.iter().map(|p| [p.x, p.y]).collect();
            let in_tangents: Vec<[f32; 2]> = vec![[0.0, 0.0]; vertices.len()];
            let out_tangents: Vec<[f32; 2]> = vec![[0.0, 0.0]; vertices.len()];
            shapes.push(serde_json::json!({
                "ty": "gr",
                "it": [
                    {
                        "ty": "sh",
                        "ks": {
                            "a": 0,
                            "k": {
                                "i": in_tangents,
                                "o": out_tangents,
                                "v": vertices,
                                "c": false
                            }
                        }
                    },
                    {
                        "ty": "st",
                        "c": { "a": 0, "k": [s.color[0] as f32 / 255.0, s.color[1] as f32 / 255.0, s.color[2] as f32 / 255.0, 1.0] },
                        "o": { "a": 0, "k": 100 },
                        "w": { "a": 0, "k": s.base_width },
                        "lc": 2,
                        "lj": 2
                    }
                ]
            }));
        }

        layers_json.push(serde_json::json!({
            "ddd": 0,
            "ind": l_idx + 1,
            "ty": 4,
            "nm": layer.name,
            "sr": 1,
            "ks": {
                "o": { "a": 0, "k": 100 },
                "r": { "a": 0, "k": 0 },
                "p": { "a": 0, "k": [0, 0, 0] },
                "a": { "a": 0, "k": [0, 0, 0] },
                "s": { "a": 0, "k": [100, 100, 100] }
            },
            "shapes": shapes,
            "ip": 0,
            "op": total,
            "st": 0
        }));
    }

    let lottie = serde_json::json!({
        "v": "5.7.4",
        "fr": fps,
        "ip": 0,
        "op": total,
        "w": project.width,
        "h": project.height,
        "nm": "Stick_Animation",
        "ddd": 0,
        "assets": [],
        "layers": layers_json
    });

    let serialized = serde_json::to_string_pretty(&lottie).map_err(|e| e.to_string())?;
    fs::write(out_path, serialized).map_err(|e| e.to_string())
}

pub fn export_instant_gif(project: &AniProject, out_path: &Path, bg_rgba: [u8; 4]) -> Result<(), String> {
    let total = project.total_frames.max(1);
    let fps = project.fps.max(1);
    let delay_ms = (1000 / fps).max(15);

    let frames_rgba: Vec<RgbaImage> = (1..=total)
        .into_par_iter()
        .map(|f| {
            let pix = render_frame_pixmap(project, f, false, bg_rgba);
            pixmap_to_rgba_image(&pix)
        })
        .collect();

    let file = fs::File::create(out_path).map_err(|e| e.to_string())?;
    let mut encoder = GifEncoder::new_with_speed(file, 10);
    encoder.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;

    for img in frames_rgba {
        let frame = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(delay_ms, 1));
        encoder.encode_frame(frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn export_video_multi_format(
    project: &AniProject,
    output_path: &Path,
    format: VideoExportFormat,
    bg_rgba: [u8; 4],
) -> Result<(), String> {
    if format == VideoExportFormat::GifInstant {
        return export_instant_gif(project, output_path, bg_rgba);
    }

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

    let indices: Vec<usize> = (1..=total).collect();
    indices.par_iter().try_for_each(|&f| {
        let pixmap = render_frame_pixmap(project, f, is_transparent, bg_rgba);
        let file_path = temp_dir.join(format!("frame_{:04}.png", f));
        pixmap.save_png(&file_path).map_err(|e| e.to_string())
    })?;

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
                "-preset", "ultrafast",
                "-crf", "20",
                "-pix_fmt", "yuv420p",
                out_str,
            ]);
        }
        VideoExportFormat::Mov => {
            cmd.args([
                "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                "-c:v", "libx264",
                "-preset", "ultrafast",
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
                "-preset", "ultrafast",
                "-pix_fmt", "yuv420p",
                out_str,
            ]);
        }
        VideoExportFormat::Webm => {
            cmd.args(["-c:v", "libvpx-vp9", "-deadline", "realtime", "-pix_fmt", "yuva420p", out_str]);
        }
        VideoExportFormat::GifInstant => {}
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

#[cfg(test)]
mod layer_opacity_tests {
    use super::*;
    use crate::project::{KritaBrushPreset, Stroke as ProjectStroke, StrokePoint};

    fn project_with_solid_dab(opacity: f32) -> AniProject {
        let mut project = AniProject::default();
        project.width = 48;
        project.height = 48;
        project.layers[0].opacity = opacity;
        project.layers[0]
            .keyframes
            .get_mut(&1)
            .unwrap()
            .strokes
            .push(ProjectStroke {
                points: vec![StrokePoint { x: 24.0, y: 24.0, pressure: 1.0 }],
                color: [240, 30, 20, 255],
                fill_color: None,
                base_width: 16.0,
                is_vector: false,
                brush_preset: KritaBrushPreset::GPenInk,
                opacity: 1.0,
                brush_texture_id: None,
            });
        project
    }

    fn center_alpha(project: &AniProject) -> u8 {
        render_frame_pixmap(project, 1, true, [0, 0, 0, 0])
            .pixel(24, 24)
            .expect("center pixel")
            .alpha()
    }

    #[test]
    fn layer_opacity_changes_export_alpha() {
        assert_eq!(center_alpha(&project_with_solid_dab(0.0)), 0);
        assert_eq!(center_alpha(&project_with_solid_dab(1.0)), 255);
        let half = center_alpha(&project_with_solid_dab(0.5));
        assert!((125..=130).contains(&half), "expected about 128, got {half}");
    }

    #[test]
    fn overlapping_strokes_do_not_square_layer_opacity() {
        let mut project = project_with_solid_dab(0.5);
        let duplicate = project.layers[0].keyframes[&1].strokes[0].clone();
        project.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(duplicate);
        let alpha = center_alpha(&project);
        assert!((125..=130).contains(&alpha), "unexpected accumulated layer alpha: {alpha}");
    }

    #[test]
    fn hidden_and_guide_layers_do_not_export() {
        let mut project = project_with_solid_dab(1.0);
        project.layers[0].visible = false;
        assert_eq!(center_alpha(&project), 0);
        project.layers[0].visible = true;
        project.layers[0].is_guide = true;
        assert_eq!(center_alpha(&project), 0);
    }
}
