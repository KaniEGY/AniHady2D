use std::fs::File;
use std::io::Write;
use std::path::Path;

fn generate_stick_icon_ico() -> Vec<u8> {
    let size = 256u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            // إحداثيات مائلة بزاوية 45 درجة لرسم عصا طويلة قطرية
            let fx = x as f32 - 128.0;
            let fy = y as f32 - 128.0;
            let u = (fx + fy) * std::f32::consts::FRAC_1_SQRT_2; // الطول على امتداد العصا
            let v = (-fx + fy) * std::f32::consts::FRAC_1_SQRT_2; // السُمك العرضي للعصا

            let half_len = 104.0;
            let half_thick = 18.0;
            let border = 5.0;

            if u.abs() <= half_len && v.abs() <= half_thick {
                if u.abs() >= (half_len - border) || v.abs() >= (half_thick - border) {
                    // حدود سوداء حادة
                    rgba[idx..idx + 4].copy_from_slice(&[18, 18, 22, 255]);
                } else {
                    // جسم العصا باللون الأصفر الذهبي الحيوي
                    let shade = if v < 0.0 { 255 } else { 235 };
                    rgba[idx..idx + 4].copy_from_slice(&[shade, (shade as f32 * 0.84) as u8, 20, 255]);
                }
            }
        }
    }

    // تحويل RGBA إلى صيغة ICO القياسية لويندوز (BGRA مقلوب رأسياً)
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    ico.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
    let bmp_size = 40 + (size * size * 4);
    ico.extend_from_slice(&bmp_size.to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());

    ico.extend_from_slice(&40u32.to_le_bytes());
    ico.extend_from_slice(&(size as i32).to_le_bytes());
    ico.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&0u32.to_le_bytes());
    ico.extend_from_slice(&(size * size * 4).to_le_bytes());
    ico.extend_from_slice(&[0u8; 16]);

    for y in (0..size).rev() {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            ico.push(rgba[idx + 2]);
            ico.push(rgba[idx + 1]);
            ico.push(rgba[idx]);
            ico.push(rgba[idx + 3]);
        }
    }
    ico
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let ico_path = Path::new(&out_dir).join("stick_icon.ico");
        let mut f = File::create(&ico_path).unwrap();
        f.write_all(&generate_stick_icon_ico()).unwrap();

        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.set("ProductName", "Stick");
        res.set("FileDescription", "Stick 2D Animation & Drawing Studio");
        let _ = res.compile();
    }
}
