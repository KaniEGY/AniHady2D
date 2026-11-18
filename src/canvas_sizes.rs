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
    CanvasPreset { name_en: "Instagram Square", name_ar: "إنستجرام مربع", width: 1080, height: 1080 },
    CanvasPreset { name_en: "Instagram Portrait", name_ar: "إنستجرام طولي", width: 1080, height: 1350 },
    CanvasPreset { name_en: "Stories / Reels / TikTok", name_ar: "ستوري / ريلز / تيك توك", width: 1080, height: 1920 },
    CanvasPreset { name_en: "YouTube Video / Thumbnail", name_ar: "يوتيوب فيديو / مصغرة", width: 1920, height: 1080 },
    CanvasPreset { name_en: "YouTube Channel Banner", name_ar: "بانر قناة يوتيوب", width: 2560, height: 1440 },
    CanvasPreset { name_en: "Facebook Link Post", name_ar: "منشور رابط فيسبوك", width: 1200, height: 630 },
    CanvasPreset { name_en: "X / Twitter Header", name_ar: "غلاف إكس", width: 1500, height: 500 },
    CanvasPreset { name_en: "LinkedIn Cover", name_ar: "غلاف لينكدإن", width: 1584, height: 396 },
    CanvasPreset { name_en: "4K Landscape", name_ar: "أفقي 4K", width: 3840, height: 2160 },
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_safe_and_have_unique_dimensions() {
        let mut seen = std::collections::HashSet::new();
        for preset in PRESETS {
            assert!(validate_canvas_size(preset.width, preset.height).is_ok());
            assert!(seen.insert((preset.width, preset.height)));
        }
    }
    #[test]
    fn invalid_and_oversized_canvas_is_rejected() {
        for (w,h) in [(0,1080),(9,500),(8193,500),(4096,8192),(8192,8192)] {
            assert!(validate_canvas_size(w,h).is_err());
        }
        assert!(validate_canvas_size(3840,2160).is_ok());
    }
}
