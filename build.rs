use image::{Rgba, RgbaImage};
use std::path::Path;

fn generate_stick_icon(size: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let s = size as f32;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / s;
            let fy = y as f32 / s;

            // تدوير العصا بزاوية مائلة (-35 درجة) لتظهر بشكل جمالي داخل الأيقونة
            let cx = fx - 0.5;
            let cy = fy - 0.5;
            let ang = 0.62_f32;
            let rx = cx * ang.cos() - cy * ang.sin();
            let ry = cx * ang.sin() + cy * ang.cos();

            // أبعاد العصا الصفراء الطويلة وحدودها السوداء
            let half_w = 0.095;
            let half_h = 0.42;
            let border = 0.032;

            if rx.abs() <= half_w + border && ry.abs() <= half_h + border {
                if rx.abs() <= half_w && ry.abs() <= half_h {
                    // جسم العصا الأصفر الذهبي
                    let shade = ((rx + half_w) / (2.0 * half_w) * 35.0) as u8;
                    img.put_pixel(x, y, Rgba([255, 220u8.saturating_sub(shade), 20, 255]));
                } else {
                    // الحدود السوداء الحادة
                    img.put_pixel(x, y, Rgba([15, 15, 18, 255]));
                }
            }
        }
    }
    img
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ico_path = Path::new(&out_dir).join("app_icon.ico");
    let icon_img = generate_stick_icon(256);
    icon_img.save(&ico_path).expect("Failed to generate Stick icon");

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.set("ProductName", "Stick");
        res.set("FileDescription", "Stick 2D Animation & Drawing Studio");
        res.compile().expect("Failed to compile Windows resources");
    }
}
