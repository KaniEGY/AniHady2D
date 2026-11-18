//! Canvas dimension presets, shared by drawing and animation workspaces.
//! Resizing changes the stage bounds; it does not destructively scale artwork.
pub const MAX_DIMENSION: u32 = 8_192;
pub const MAX_PIXELS: u64 = 16_777_216;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanvasPreset {
    pub name_en: &'static str,
    pub name_ar: &'static str,
    pub width: u32,
    pub height: u32,
}

pub const PRESETS: &[CanvasPreset] = &[
    CanvasPreset { name_en: "YouTube Video", name_ar: "يوتيوب فيديو", width: 1920, height: 1080 },
    CanvasPreset { name_en: "YouTube Thumbnail", name_ar: "صورة مصغرة يوتيوب", width: 1280, height: 720 },
    CanvasPreset { name_en: "Instagram Square", name_ar: "إنستجرام مربع", width: 1080, height: 1080 },
    CanvasPreset { name_en: "Instagram Portrait", name_ar: "إنستجرام طولي", width: 1080, height: 1350 },
    CanvasPreset { name_en: "TikTok / Reels", name_ar: "تيك توك / ريلز", width: 1080, height: 1920 },
    CanvasPreset { name_en: "Facebook Post", name_ar: "بوست فيسبوك", width: 1200, height: 630 },
    CanvasPreset { name_en: "4K", name_ar: "4K", width: 3840, height: 2160 },
    CanvasPreset { name_en: "HD", name_ar: "HD", width: 1280, height: 720 },
    CanvasPreset { name_en: "Full HD", name_ar: "Full HD", width: 1920, height: 1080 },
    // Keep legacy sizes available to avoid removing previous editor workflows.
    CanvasPreset { name_en: "YouTube Channel Banner", name_ar: "بانر قناة يوتيوب", width: 2560, height: 1440 },
    CanvasPreset { name_en: "X / Twitter Header", name_ar: "غلاف إكس", width: 1500, height: 500 },
    CanvasPreset { name_en: "LinkedIn Cover", name_ar: "غلاف لينكدإن", width: 1584, height: 396 },
];

pub fn validate_canvas_size(width: u32, height: u32) -> Result<(), &'static str> {
    if !(16..=MAX_DIMENSION).contains(&width) || !(16..=MAX_DIMENSION).contains(&height) {
        return Err("Width and height must each be between 16 and 8192 pixels.");
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("Canvas exceeds the 16-megapixel memory safety limit.");
    }
    Ok(())
}

/// Preserve the user's draft aspect ratio while editing one dimension.
/// The caller still validates total pixel memory with validate_canvas_size.
pub fn paired_dimension(edited: u32, width_was_edited: bool, ratio: f32) -> u32 {
    let ratio = if ratio.is_finite() && ratio > 0.001 && ratio < 1_000.0 {
        ratio
    } else { 16.0 / 9.0 };
    let result = if width_was_edited {
        edited as f32 / ratio
    } else {
        edited as f32 * ratio
    };
    result.round().clamp(16.0, MAX_DIMENSION as f32) as u32
}

/// Size changes can hide artwork that falls outside the new stage bounds,
/// even though the original artwork is never deleted.
pub fn may_hide_artwork(old_width: u32, old_height: u32, width: u32, height: u32) -> bool {
    width < old_width || height < old_height
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_safe_and_have_unique_names() {
        let mut seen = std::collections::HashSet::new();
        for preset in PRESETS {
            assert!(validate_canvas_size(preset.width, preset.height).is_ok());
            assert!(seen.insert(preset.name_en));
        }
    }
    #[test]
    fn aspect_lock_and_shrink_detection() {
        assert_eq!(paired_dimension(1920, true, 16.0/9.0), 1080);
        assert_eq!(paired_dimension(1080, false, 16.0/9.0), 1920);
        assert!(!may_hide_artwork(1280,720,1920,1080));
        assert!(may_hide_artwork(1920,1080,1280,720));
        assert!(may_hide_artwork(1920,1080,1920,1000));
    }
    #[test]
    fn invalid_and_oversized_canvas_is_rejected() {
        for (w,h) in [(0,1080),(9,500),(8193,500),(4096,8192),(8192,8192)] {
            assert!(validate_canvas_size(w,h).is_err());
        }
        assert!(validate_canvas_size(3840,2160).is_ok());
    }
}
