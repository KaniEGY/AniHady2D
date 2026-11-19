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
            KritaBrushPreset::DryBrush => egui::Color32::from_rgb(225, 156, 102),
            KritaBrushPreset::Chalk => egui::Color32::from_rgb(196, 222, 226),
            KritaBrushPreset::Charcoal => egui::Color32::from_rgb(149, 165, 178),
            KritaBrushPreset::SoftPastel => egui::Color32::from_rgb(255, 171, 208),
            KritaBrushPreset::Fineliner => egui::Color32::from_rgb(131, 230, 250),
            KritaBrushPreset::SmoothInk => egui::Color32::from_rgb(79, 197, 241),
            KritaBrushPreset::TaperedPen => egui::Color32::from_rgb(240, 186, 101),
            KritaBrushPreset::Calligraphy => egui::Color32::from_rgb(232, 144, 226),
            KritaBrushPreset::SoftSketchPencil => egui::Color32::from_rgb(187, 200, 211),
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
        KritaBrushPreset::DryBrush => {
            for i in 0..5 {
                let x = i as f32 * 2.5 - 5.0;
                painter.line_segment([c + egui::vec2(x-2.0, 7.0 - (i%2) as f32 * 3.0),
                    c + egui::vec2(x+2.0,-7.0)], egui::Stroke::new(1.6,col));
            }
        }
        KritaBrushPreset::Chalk => {
            painter.line_segment([c + egui::vec2(-4.0,5.0), c + egui::vec2(5.0,-6.0)],
                egui::Stroke::new(5.5,col));
            for (x,y) in [(-7.0,7.0),(-3.0,8.0)] {
                painter.circle_filled(c + egui::vec2(x,y),1.1,col);
            }
        }
        KritaBrushPreset::Charcoal => {
            for i in 0..7 {
                let d=i as f32 - 3.0;
                painter.line_segment([c+egui::vec2(d-4.0,8.0),c+egui::vec2(d+4.0,-8.0)],
                    egui::Stroke::new(if i%2==0 {1.6}else{2.5},col));
            }
        }
        KritaBrushPreset::SoftPastel => {
            painter.circle_filled(c,7.0,col);
            for i in 0..5 {
                let x = i as f32 * 3.0 - 6.0;
                painter.circle_filled(c+egui::vec2(x,5.0-(i%2) as f32*3.0),1.5,col);
            }
        }
        KritaBrushPreset::Fineliner => {
            painter.line_segment([c+egui::vec2(-7.,7.),c+egui::vec2(7.,-7.)],
                egui::Stroke::new(1.7,col));
            painter.circle_filled(c+egui::vec2(7.,-7.),1.8,col);
        }
        KritaBrushPreset::SmoothInk => {
            let pts=(0..20).map(|i| {
                let t=i as f32/19.;
                c+egui::vec2(-8.+t*16.,(t*std::f32::consts::TAU).sin()*4.)
            }).collect();
            painter.add(egui::Shape::line(pts,egui::Stroke::new(2.6,col)));
        }
        KritaBrushPreset::TaperedPen => {
            painter.add(egui::Shape::convex_polygon(vec![
                c+egui::vec2(-8.,8.),c+egui::vec2(-3.,-2.),
                c+egui::vec2(0.,-7.),c+egui::vec2(2.,-2.),
                c+egui::vec2(8.,-8.),c+egui::vec2(4.,4.)
            ],col,egui::Stroke::NONE));
        }
        KritaBrushPreset::Calligraphy => {
            painter.add(egui::Shape::convex_polygon(vec![
                c+egui::vec2(-8.,4.),c+egui::vec2(3.,-8.),
                c+egui::vec2(7.,-4.),c+egui::vec2(-4.,8.)
            ],col,egui::Stroke::NONE));
        }
        KritaBrushPreset::SoftSketchPencil => {
            for k in 0..3 {
                painter.line_segment([
                    c+egui::vec2(-8.+k as f32,7.),
                    c+egui::vec2(7.,-8.+k as f32)
                ],egui::Stroke::new(1.0,col));
            }
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
        let rings:Vec<Vec<egui::Pos2>>=crate::fill_tools::split_fill_rings(&stroke.points)
            .into_iter()
            .map(|ring|ring.iter()
                .map(|p|to_screen(egui::pos2(p.x+ox,p.y+oy))).collect())
            .collect();
        // Region fills may be concave and may contain nested closed contours.
        // Earcut receives those inner rings as holes so they stay independently
        // paintable instead of inheriting the outer Bucket color.
        if let Some(mesh) = crate::fill_render::solid_polygon_mesh_rings(
            &rings,
            egui::Color32::from_rgba_unmultiplied(
                fill_c[0], fill_c[1], fill_c[2],
                (fill_c[3] as f32 * combined_opacity) as u8,
            ),
        ) {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    // Optional fills are rendered above. The brush body must not create
    // an artificial halo when its own combined opacity is zero.
    if eff_alpha == 0 { return; }

    if let Some(texture) = crate::texture_brush::TextureBrush::for_stroke(stroke) {
        texture.preview(painter, stroke, [c[0],c[1],c[2],eff_alpha], |x,y|
            to_screen(egui::pos2(x+ox,y+oy)));
        return;
    }

    // Polygonal tips share their sampled geometry with PNG/video exports.
    if !stroke.is_vector && stroke.tip_shape != BrushTipShape::Round {
        crate::brush_engine::visit_pressure_dabs(&stroke.points, stroke.base_width, |dab,_| {
            let verts: Vec<egui::Pos2> = crate::brush_engine::dab_polygon(stroke.tip_shape, dab)
                .into_iter()
                .map(|(x,y)| to_screen(egui::pos2(x + ox, y + oy))).collect();
            if verts.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(verts, color, egui::Stroke::NONE));
            }
        });
        return;
    }

    use crate::brush_engine::{visit_round_marks, RoundMark};
    let screen = |x,y| to_screen(egui::pos2(x+ox,y+oy));
    let mut screen_pts: Option<Vec<egui::Pos2>> = None;
    visit_round_marks(stroke, |mark, flow| {
        let ink = egui::Color32::from_rgba_unmultiplied(c[0],c[1],c[2], (eff_alpha as f32 * flow) as u8);
        let width = match mark {
            RoundMark::Dot(dab) => {
                painter.circle_filled(screen(dab.x,dab.y), dab.radius*scale, ink);
                return;
            }
            RoundMark::Outline(outline) => {
                let path: Vec<_> = outline.into_iter().map(|(x,y)| screen(x,y)).collect();
                if let Some(mesh) = crate::fill_render::solid_polygon_mesh(&path, ink) {
                    painter.add(egui::Shape::mesh(mesh));
                    return;
                }
                stroke.base_width
            }
            RoundMark::Line { width } => width,
        };
        let project_points = || {
            if stroke.is_vector || stroke.brush_preset == KritaBrushPreset::Fineliner {
                let mut path = crate::vector_path::smoothed_path(&stroke.points);
                if stroke.tip_shape == BrushTipShape::Square {
                    crate::vector_path::extend_square_caps(&mut path, width);
                }
                path.into_iter().map(|(x,y)| screen(x,y)).collect()
            } else if stroke.brush_preset == KritaBrushPreset::SmoothInk {
                crate::brush_engine::smooth_ink_points(&stroke.points, width)
                    .into_iter().map(|p| screen(p.x,p.y)).collect()
            } else {
                crate::brush_engine::smooth_freehand_points(&stroke.points, width)
                    .into_iter().map(|p| screen(p.x,p.y)).collect()
            }
        };
        let points = if flow == 1.0 {
            screen_pts.take().unwrap_or_else(project_points)
        } else {
            screen_pts.get_or_insert_with(project_points).clone()
        };
        painter.add(egui::Shape::line(points, egui::Stroke::new(width * scale, ink)));
        // egui's polyline body does not guarantee circular end caps.
        // Apply explicit caps to every round-tip line brush (including G-Pen),
        // not only vector/Fineliner strokes. Polygon/pressure-outline brushes
        // return above and keep their own geometry.
        if stroke.tip_shape == BrushTipShape::Round {
            if let (Some(first), Some(last)) = (stroke.points.first(), stroke.points.last()) {
                let radius = (width * scale * 0.5).max(0.25);
                painter.circle_filled(screen(first.x,first.y),radius,ink);
                painter.circle_filled(screen(last.x,last.y),radius,ink);
            }
        }
    });
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
    fn ordinary_gpen_round_tip_remains_round_cap_eligible() {
        let s = stroke(&[(0.0,0.0),(20.0,0.0)]);
        assert_eq!(s.brush_preset, KritaBrushPreset::GPenInk);
        assert_eq!(s.tip_shape, BrushTipShape::Round);
        let mut line_marks = 0;
        crate::brush_engine::visit_round_marks(&s, |mark, _| {
            if matches!(mark, crate::brush_engine::RoundMark::Line { .. }) {
                line_marks += 1;
            }
        });
        assert_eq!(line_marks, 1);
    }

    #[test]
    fn airbrush_alpha_follows_opacity_without_phantom_glow() {
        let mut airbrush = stroke(&[(0.0,0.0), (20.0,0.0)]);
        airbrush.brush_preset = KritaBrushPreset::SoftAirbrush;
        let mut alpha = Vec::new();
        crate::brush_engine::visit_round_marks(&airbrush, |_, flow| alpha.push((flow * 100.0) as u8));
        assert_eq!(alpha, [25,50,100]);
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
