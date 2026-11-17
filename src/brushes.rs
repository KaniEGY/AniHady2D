use crate::project::{KritaBrushPreset, Stroke};
use eframe::egui;

/// رسم أيقونة مخصصة لكل فرشة في لوحة الخصائص
pub fn draw_brush_preset_icon(painter: &egui::Painter, rect: egui::Rect, preset: KritaBrushPreset, active: bool) {
    let c = rect.center();
    let col = if active {
        egui::Color32::WHITE
    } else {
        match preset {
            KritaBrushPreset::GPenInk => egui::Color32::from_rgb(56, 189, 248),
            KritaBrushPreset::Watercolor => egui::Color32::from_rgb(34, 197, 94),
            KritaBrushPreset::ChiselMarker => egui::Color32::from_rgb(249, 115, 22),
            KritaBrushPreset::SoftAirbrush => egui::Color32::from_rgb(192, 132, 252),
            KritaBrushPreset::SketchPencil => egui::Color32::from_rgb(250, 204, 21),
        }
    };

    match preset {
        KritaBrushPreset::GPenInk => {
            // سن قلم حبر معدني
            let pts = vec![
                c + egui::vec2(0.0, -9.0),
                c + egui::vec2(6.0, 2.0),
                c + egui::vec2(3.0, 8.0),
                c + egui::vec2(-3.0, 8.0),
                c + egui::vec2(-6.0, 2.0),
            ];
            painter.add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
        }
        KritaBrushPreset::Watercolor => {
            // قطرة ألوان مائية ناعمة
            painter.circle_filled(c + egui::vec2(0.0, 2.5), 6.0, col);
            let top = vec![
                c + egui::vec2(0.0, -8.5),
                c + egui::vec2(5.5, 1.5),
                c + egui::vec2(-5.5, 1.5),
            ];
            painter.add(egui::Shape::convex_polygon(top, col, egui::Stroke::NONE));
        }
        KritaBrushPreset::ChiselMarker => {
            // سن ماركر مشطوف عريض
            let pts = vec![
                c + egui::vec2(-6.0, 7.0),
                c + egui::vec2(2.0, -7.0),
                c + egui::vec2(7.0, -3.0),
                c + egui::vec2(-1.0, 9.0),
            ];
            painter.add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
        }
        KritaBrushPreset::SoftAirbrush => {
            // بخاخ رذاذ ناعم
            painter.circle_stroke(c, 7.0, egui::Stroke::new(1.5, col));
            painter.circle_filled(c, 3.0, col);
            painter.circle_filled(c + egui::vec2(-4.0, -3.0), 1.2, col);
            painter.circle_filled(c + egui::vec2(4.0, 3.0), 1.2, col);
        }
        KritaBrushPreset::SketchPencil => {
            // قلم رصاص خشبي
            let body = vec![
                c + egui::vec2(-6.0, 4.0),
                c + egui::vec2(4.0, -6.0),
                c + egui::vec2(7.0, -3.0),
                c + egui::vec2(-3.0, 7.0),
            ];
            painter.add(egui::Shape::convex_polygon(body, col, egui::Stroke::NONE));
            let tip = vec![
                c + egui::vec2(-6.0, 4.0),
                c + egui::vec2(-3.0, 7.0),
                c + egui::vec2(-8.0, 8.0),
            ];
            painter.add(egui::Shape::convex_polygon(tip, col, egui::Stroke::NONE));
        }
    }
}

/// مصيّر الفرش الاحترافي:
/// - يرسم الألوان المائية والماركر والبخاخ كمسار واحد متصل (PathShape) بدون دوائر متداخلة أو نقاط متقطعة
/// - يزيل الذيل المدبب من G-Pen وقلم الفيكتور
pub fn draw_stroke_pro<F: Fn(egui::Pos2) -> egui::Pos2>(
    painter: &egui::Painter,
    stroke: &Stroke,
    override_color: Option<[u8; 4]>,
    layer_opacity: f32,
    scale: f32,
    ox: f32,
    oy: f32,
    to_screen: &F,
) {
    if stroke.points.is_empty() {
        return;
    }

    let c = override_color.unwrap_or(stroke.color);
    let combined_opacity = stroke.opacity.clamp(0.05, 1.0) * layer_opacity.clamp(0.05, 1.0);
    let eff_alpha = ((c[3] as f32) * combined_opacity) as u8;
    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], eff_alpha);

    // Drawing engine step 1: identical pressure-aware G-Pen geometry on
    // screen and in exports. Existing project files and other brushes remain unchanged.
    if !stroke.is_vector && stroke.brush_preset == KritaBrushPreset::GPenInk {
        for dab in crate::brush_engine::sample_pressure_dabs(&stroke.points, stroke.base_width) {
            let p = to_screen(egui::pos2(dab.x + ox, dab.y + oy));
            painter.circle_filled(p, (dab.radius * scale.abs()).max(0.25), color);
        }
        return;
    }

    if stroke.points.len() == 1 {
        let p = to_screen(egui::pos2(stroke.points[0].x + ox, stroke.points[0].y + oy));
        let r = (stroke.base_width * stroke.points[0].pressure * scale) * 0.5;
        painter.circle_filled(p, r.max(1.0), color);
        return;
    }

    if let Some(fc) = stroke.fill_color {
        let fill_c = override_color.unwrap_or(fc);
        let pts: Vec<egui::Pos2> = stroke
            .points
            .iter()
            .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            pts,
            egui::Color32::from_rgba_unmultiplied(
                fill_c[0],
                fill_c[1],
                fill_c[2],
                (fill_c[3] as f32 * layer_opacity) as u8,
            ),
            egui::Stroke::NONE,
        ));
    }

    let screen_pts: Vec<egui::Pos2> = stroke
        .points
        .iter()
        .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
        .collect();

    if !stroke.is_vector {
        match stroke.brush_preset {
            KritaBrushPreset::Watercolor => {
                // خط ألوان مائية متصل بحافة مائية ناعمة بدون نقاط متقطعة
                let w = (stroke.base_width * scale).max(2.0);
                let halo_alpha = (eff_alpha / 3).max(12);
                let halo_col = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], halo_alpha);
                painter.add(egui::Shape::line(screen_pts.clone(), egui::Stroke::new(w * 1.25, halo_col)));
                painter.add(egui::Shape::line(screen_pts, egui::Stroke::new(w * 0.90, color)));
                return;
            }
            KritaBrushPreset::ChiselMarker => {
                // ماركر شفاف متصل يغمق تلقائياً عند رسم خط جديد فوق خط سابق
                let w = (stroke.base_width * scale).max(2.0);
                painter.add(egui::Shape::line(screen_pts, egui::Stroke::new(w, color)));
                return;
            }
            KritaBrushPreset::SoftAirbrush => {
                let w = (stroke.base_width * scale).max(2.0);
                let outer = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (eff_alpha / 4).max(8));
                let mid = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (eff_alpha / 2).max(14));
                painter.add(egui::Shape::line(screen_pts.clone(), egui::Stroke::new(w * 1.55, outer)));
                painter.add(egui::Shape::line(screen_pts.clone(), egui::Stroke::new(w * 1.05, mid)));
                painter.add(egui::Shape::line(screen_pts, egui::Stroke::new(w * 0.65, color)));
                return;
            }
            KritaBrushPreset::GPenInk | KritaBrushPreset::SketchPencil => {}
        }
    }

    // G-Pen و Vector Ink Pen: خط ثابت السُمك ناعم بدون ذيل في البداية أو النهاية
    let w = (stroke.base_width * scale).max(1.0);
    painter.add(egui::Shape::line(screen_pts, egui::Stroke::new(w, color)));
    if let Some(first) = stroke.points.first() {
        painter.circle_filled(to_screen(egui::pos2(first.x + ox, first.y + oy)), w * 0.5, color);
    }
    if let Some(last) = stroke.points.last() {
        painter.circle_filled(to_screen(egui::pos2(last.x + ox, last.y + oy)), w * 0.5, color);
    }
}
