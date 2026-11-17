use crate::project::{BrushTipShape, KritaBrushPreset, Stroke};
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

/// Fast preview-only visibility check. Export is untouched. We must keep
/// a conservative margin for wide brush circles and anti-aliased edges.
/// Any invalid coordinate fails open to avoid hiding user artwork.
fn may_touch_clip<F: Fn(egui::Pos2) -> egui::Pos2>(
    stroke: &Stroke,
    ox: f32,
    oy: f32,
    scale: f32,
    clip: egui::Rect,
    to_screen: &F,
) -> bool {
    if stroke.points.is_empty() { return false; }
    if !scale.is_finite() || !clip.is_finite() { return true; }

    let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
    let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in &stroke.points {
        if !p.x.is_finite() || !p.y.is_finite() { return true; }
        let q = to_screen(egui::pos2(p.x + ox, p.y + oy));
        if !q.x.is_finite() || !q.y.is_finite() { return true; }
        min.x = min.x.min(q.x);
        min.y = min.y.min(q.y);
        max.x = max.x.max(q.x);
        max.y = max.y.max(q.y);
    }
    let margin = (stroke.base_width.abs() * scale.abs() * 2.5 + 4.0).max(4.0);
    if !margin.is_finite() { return true; }
    let bounds = egui::Rect::from_min_max(
        min - egui::vec2(margin, margin),
        max + egui::vec2(margin, margin),
    );
    clip.intersects(bounds)
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

    // Large offscreen strokes used to allocate and paint thousands of dabs
    // on every UI frame. Cull conservatively before sampling/painting.
    if !may_touch_clip(stroke, ox, oy, scale, painter.clip_rect(), to_screen) {
        return;
    }

    let c = override_color.unwrap_or(stroke.color);
    let combined_opacity = stroke.opacity.clamp(0.0, 1.0) * layer_opacity.clamp(0.0, 1.0);
    let eff_alpha = ((c[3] as f32) * combined_opacity) as u8;
    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], eff_alpha);

    // Draw any Bucket fill before the early-return G-Pen dab renderer.
    // The old order silently discarded fill colors on hand-drawn loops.
    if let Some(fc) = stroke.fill_color {
        let fill_c = override_color.unwrap_or(fc);
        let pts: Vec<egui::Pos2> = stroke
            .points
            .iter()
            .map(|p| to_screen(egui::pos2(p.x + ox, p.y + oy)))
            .collect();
        // Region fills are often concave. Convex-only tessellation used
        // to produce white seams and incorrect triangles in the live canvas.
        if let Some(mesh) = crate::fill_render::solid_polygon_mesh(
            &pts,
            egui::Color32::from_rgba_unmultiplied(
                fill_c[0], fill_c[1], fill_c[2],
                (fill_c[3] as f32 * combined_opacity) as u8,
            ),
        ) {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    // Polygonal tips share their sampled geometry with PNG/video exports.
    if !stroke.is_vector && stroke.tip_shape != BrushTipShape::Round {
        for dab in crate::brush_engine::sample_pressure_dabs(&stroke.points, stroke.base_width) {
            let verts: Vec<egui::Pos2> = crate::brush_engine::dab_polygon(stroke.tip_shape, dab)
                .into_iter()
                .map(|(x,y)| to_screen(egui::pos2(x + ox, y + oy))).collect();
            if verts.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(verts, color, egui::Stroke::NONE));
            }
        }
        return;
    }

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

#[cfg(test)]
mod viewport_tests {
    use super::*;
    use crate::project::StrokePoint;

    fn stroke(points: &[(f32, f32)]) -> Stroke {
        Stroke {
            points: points.iter().map(|&(x,y)| StrokePoint { x, y, pressure: 1.0 }).collect(),
            color: [0,0,0,255],
            fill_color: None,
            base_width: 8.0,
            is_vector: false,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0,
            brush_texture_id: None,
            tip_shape: BrushTipShape::Round,
        }
    }

    #[test]
    fn skips_completely_offscreen_strokes() {
        let clip = egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(100.0,100.0));
        let far = stroke(&[(1000.0,1000.0),(1200.0,1200.0)]);
        assert!(!may_touch_clip(&far,0.0,0.0,1.0,clip,&|p|p));
    }

    #[test]
    fn keeps_line_crossing_viewport_even_with_endpoints_outside() {
        let clip = egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(100.0,100.0));
        let crossing = stroke(&[(-200.0,50.0),(300.0,50.0)]);
        assert!(may_touch_clip(&crossing,0.0,0.0,1.0,clip,&|p|p));
    }

    #[test]
    fn wide_stroke_near_edge_is_not_lost() {
        let clip = egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(100.0,100.0));
        let near = stroke(&[(104.0,50.0)]);
        assert!(may_touch_clip(&near,0.0,0.0,1.0,clip,&|p|p));
    }
}
