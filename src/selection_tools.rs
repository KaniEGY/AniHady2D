//! Lightweight stage-space hit testing for marquee/lasso and text selection.
//! No extra image buffers; works with existing .stick vector project data.
use crate::project::{Stroke, TextItem};
use eframe::egui;

pub fn text_bounds(text: &TextItem) -> egui::Rect {
    let lines: Vec<&str> = text.content.lines().collect();
    let max_chars = lines.iter().map(|line| line.chars().count()).max().unwrap_or(1).max(1);
    let height = (lines.len().max(1) as f32 * text.font_size * 1.32).max(12.0);
    let width = (max_chars as f32 * text.font_size * 0.60).max(14.0);
    egui::Rect::from_min_size(egui::pos2(text.x, text.y), egui::vec2(width, height))
}

pub fn stroke_intersects_marquee(stroke: &Stroke, rect: egui::Rect) -> bool {
    if stroke.points.is_empty() { return false; }
    let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
    let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in &stroke.points {
        if !p.x.is_finite() || !p.y.is_finite() { return false; }
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    egui::Rect::from_min_max(min, max).expand(stroke.base_width.max(1.0) * 0.5).intersects(rect)
}

pub fn point_in_polygon(point: egui::Pos2, polygon: &[egui::Pos2]) -> bool {
    if polygon.len() < 3 { return false; }
    let mut inside = false;
    let mut prev = polygon[polygon.len() - 1];
    for &p in polygon {
        if (p.y > point.y) != (prev.y > point.y) {
            let intersection = (prev.x - p.x) * (point.y - p.y) / (prev.y - p.y) + p.x;
            if point.x < intersection { inside = !inside; }
        }
        prev = p;
    }
    inside
}

pub fn stroke_inside_lasso(stroke: &Stroke, polygon: &[egui::Pos2]) -> bool {
    stroke.points.iter().any(|p| point_in_polygon(egui::pos2(p.x, p.y), polygon))
}

/// Return the nearest editable vertex on any vector stroke within a
/// stage-space radius; brush raster paths are intentionally not node-edited.
pub fn nearest_vector_node(
    strokes: &[Stroke],
    cursor: egui::Pos2,
    radius: f32,
) -> Option<(usize, usize)> {
    let mut nearest = None;
    let mut best = radius.max(0.0).powi(2);
    for (si, stroke) in strokes.iter().enumerate() {
        if !stroke.is_vector { continue; }
        for (pi, point) in stroke.points.iter().enumerate() {
            let distance = (point.x - cursor.x).powi(2) + (point.y - cursor.y).powi(2);
            if distance.is_finite() && distance <= best {
                nearest = Some((si, pi));
                best = distance;
            }
        }
    }
    nearest
}

/// Geometry-only scaling to keep interactive transform independent of Zoom.
pub fn scale_points_around(stroke: &mut Stroke, anchor: egui::Pos2, factor: f32) {
    if !factor.is_finite() || factor <= 0.0 { return; }
    for point in &mut stroke.points {
        point.x = anchor.x + (point.x - anchor.x) * factor;
        point.y = anchor.y + (point.y - anchor.y) * factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{KritaBrushPreset, StrokePoint};
    fn stroke(x1: f32, y1: f32, x2: f32, y2: f32) -> Stroke {
        Stroke {
            points: vec![
                StrokePoint { x: x1, y: y1, pressure: 1.0 },
                StrokePoint { x: x2, y: y2, pressure: 1.0 }
            ],
            color: [0,0,0,255], fill_color: None,
            base_width: 2.0, is_vector: true,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0, brush_texture_id: None,
        }
    }
    #[test]
    fn marquee_selects_crossing_strokes_with_outside_endpoints() {
        let rect = egui::Rect::from_min_max(egui::pos2(5.0,5.0), egui::pos2(20.0,20.0));
        assert!(stroke_intersects_marquee(&stroke(-10.0, 10.0, 50.0, 10.0), rect));
        assert!(!stroke_intersects_marquee(&stroke(40.0, 40.0, 60.0, 60.0), rect));
    }
    #[test]
    fn lasso_point_containment_distinguishes_inside_outside() {
        let poly = [egui::pos2(0.0,0.0), egui::pos2(30.0,0.0), egui::pos2(15.0,30.0)];
        assert!(point_in_polygon(egui::pos2(15.0,10.0), &poly));
        assert!(!point_in_polygon(egui::pos2(40.0,40.0), &poly));
        assert!(stroke_inside_lasso(&stroke(15.0,10.0,40.0,40.0), &poly));
    }
    #[test]
    fn vector_node_hit_test_selects_nearest_actual_vertex() {
        let vector = stroke(10.0, 10.0, 80.0, 80.0);
        let mut raster = stroke(11.0, 11.0, 70.0, 70.0);
        raster.is_vector = false;
        assert_eq!(nearest_vector_node(&[vector, raster],
            egui::pos2(12.0, 12.0), 7.0), Some((0, 0)));
        assert_eq!(nearest_vector_node(&[stroke(10.0, 10.0, 80.0, 80.0)],
            egui::pos2(40.0, 40.0), 7.0), None);
    }
    #[test]
    fn transform_scale_applies_to_all_vertices_without_pixel_drifts() {
        let mut shape = stroke(10.0, 20.0, 20.0, 40.0);
        scale_points_around(&mut shape, egui::pos2(10.0, 20.0), 2.0);
        assert_eq!((shape.points[0].x, shape.points[0].y), (10.0, 20.0));
        assert_eq!((shape.points[1].x, shape.points[1].y), (30.0, 60.0));
    }

    #[test]
    fn text_hit_bounds_track_content_size() {
        let text = TextItem {
            content: "Hello".into(), x: 10.0, y: 20.0,
            font_size: 30.0, color: [0,0,0,255],
        };
        let b = text_bounds(&text);
        assert!(b.contains(egui::pos2(15.0,25.0)));
        assert!(b.width() >= 85.0);
    }
}
