use image::{ImageBuffer, Rgba};
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ico_path = Path::new(&out_dir).join("app_icon.ico");
    generate_pro_icon(&ico_path);

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.compile().unwrap();
    }
}

fn generate_pro_icon(path: &Path) {
    let size = 256u32;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(size, size);

    for y in 0..size {
        for x in 0..size {
            let nx = (x as f32 / size as f32) * 2.0 - 1.0;
            let ny = (y as f32 / size as f32) * 2.0 - 1.0;
            // حواف دائرية ناعمة (Squircle) مثل أيقونات Adobe
            let dist = (nx.abs().powf(4.0) + ny.abs().powf(4.0)).powf(0.25);

            if dist <= 0.94 {
                let t = (x + y) as f32 / (2.0 * size as f32);
                // تدرج لوني من الأزرق الملكي إلى البنفسجي النيون
                let r = (25.0 + t * 115.0) as u8;
                let g = (40.0 + (1.0 - t) * 140.0) as u8;
                let b = (210.0 + t * 45.0) as u8;

                // إطار داخلي مضيء
                if dist > 0.87 {
                    img.put_pixel(x, y, Rgba([120, 210, 255, 255]));
                } else {
                    img.put_pixel(x, y, Rgba([r, g, b, 255]));
                }
            } else {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    let white = Rgba([255, 255, 255, 255]);
    let accent = Rgba([100, 230, 255, 255]);

    // رسم حرف A هندسي احترافي + رمز حركة
    let mut draw_rect = |x0: u32, y0: u32, w: u32, h: u32, col: Rgba<u8>| {
        for py in y0..(y0 + h).min(size) {
            for px in x0..(x0 + w).min(size) {
                img.put_pixel(px, py, col);
            }
        }
    };

    // حرف A (أو H مدمج بشكل فني)
    draw_rect(64, 64, 24, 128, white);
    draw_rect(168, 64, 24, 128, white);
    draw_rect(64, 116, 128, 22, accent);
    // لمسة قمة A
    draw_rect(88, 64, 80, 20, white);

    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(img);
    dyn_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.push(0); // 256px
    ico.push(0); // 256px
    ico.push(0);
    ico.push(0);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(png_bytes.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&png_bytes);

    fs::write(path, ico).unwrap();
}
