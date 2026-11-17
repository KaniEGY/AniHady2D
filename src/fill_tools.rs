//! Safe shape-contour filling in the vector-backed Stick canvas.
//!
//! This does not claim to be a raster flood-fill engine. Fully arbitrary
//! painted pixel boundaries need a raster layer and should be implemented
//! separately so old projects remain compatible and the app stays light.
use crate::project::{Stroke, StrokePoint};

fn distance_sq_to_segment(x: f32, y: f32, a: &StrokePoint, b: &StrokePoint) -> f32 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let denom = vx * vx + vy * vy;
    let t = if denom > 0.0001 {
        (((x - a.x) * vx + (y - a.y) * vy) / denom).clamp(0.0, 1.0)
    } else { 0.0 };
    let dx = x - (a.x + t * vx);
    let dy = y - (a.y + t * vy);
    dx * dx + dy * dy
}

pub fn contour_contains(stroke: &Stroke, x: f32, y: f32) -> bool {
    // Support both closed vector contours and clearly closed freehand
    // brush paths. Open brush lines must never fill.
    let points = &stroke.points;
    if points.len() < 4 || !x.is_finite() || !y.is_finite() {
        return false;
    }
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return false;
    }
    let first = &points[0];
    let last = &points[points.len() - 1];
    let close_dx = first.x - last.x;
    let close_dy = first.y - last.y;
    // Painter strokes often finish near, but not precisely on, their start.
/// Permit gaps proportionate to the pen width without treating every open
/// line as a fillable polygon.
    let close_tolerance = (stroke.base_width.abs() * 2.5).clamp(10.0, 30.0);
    if close_dx * close_dx + close_dy * close_dy > close_tolerance * close_tolerance {
        return false;
    }
    // A user clicking directly on the border should still fill the shape.
    if points.windows(2).any(|w|
        distance_sq_to_segment(x, y, &w[0], &w[1]) <= 9.0
    ) {
        return true;
    }
    // Standard even/odd polygon containment for rectangles, circles,
    // and closed vector paths (including concave outlines).
    let mut inside = false;
    let mut prev = last;
    for point in points {
        let crosses = (point.y > y) != (prev.y > y);
        if crosses {
            let crossing_x = (prev.x - point.x) * (y - point.y) /
                (prev.y - point.y) + point.x;
            if x < crossing_x {
                inside = !inside;
            }
        }
        prev = point;
    }
    inside
}

pub fn find_topmost_shape(strokes: &[Stroke], x: f32, y: f32) -> Option<usize> {
    strokes.iter().enumerate().rev().find_map(|(idx, s)| {
        contour_contains(s, x, y).then_some(idx)
    })
}

pub fn apply_shape_fill(strokes: &mut [Stroke], x: f32, y: f32, color: [u8; 4]) -> bool {
    if let Some(index) = find_topmost_shape(strokes, x, y) {
        strokes[index].fill_color = Some(color);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::KritaBrushPreset;

    fn shape(points: &[(f32, f32)]) -> Stroke {
        Stroke {
            points: points.iter().map(|&(x,y)| StrokePoint { x, y, pressure: 1.0 }).collect(),
            color: [0, 0, 0, 255],
            fill_color: None,
            base_width: 3.0,
            is_vector: true,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0,
            brush_texture_id: None,
        }
    }

    #[test]
    fn bucket_fills_closed_rectangle_only_at_clicked_interior() {
        let mut shapes = vec![shape(&[(10.0,10.0),(50.0,10.0),(50.0,50.0),(10.0,50.0),(10.0,10.0)])];
        let color = [10, 110, 210, 255];
        assert!(!apply_shape_fill(&mut shapes, 75.0, 75.0, color));
        assert!(apply_shape_fill(&mut shapes, 30.0, 30.0, color));
        assert_eq!(shapes[0].fill_color, Some(color));
    }

    #[test]
    fn bucket_does_not_fill_open_line_or_random_surrounding_area() {
        let mut shapes = vec![shape(&[(10.0,10.0),(50.0,10.0),(50.0,50.0),(10.0,50.0)])];
        assert!(!apply_shape_fill(&mut shapes, 30.0, 30.0, [255,0,0,255]));
        assert!(shapes[0].fill_color.is_none());
    }

    #[test]
    fn bucket_handles_closed_freehand_brush_contours() {
        let mut ring = shape(&[(10.0, 10.0), (50.0, 10.0), (50.0, 50.0),
            (10.0, 50.0), (13.0, 12.0)]);
        ring.is_vector = false;
        let mut strokes = vec![ring];
        assert!(apply_shape_fill(&mut strokes, 30.0, 30.0, [20, 80, 130, 255]));
        assert_eq!(strokes[0].fill_color, Some([20, 80, 130, 255]));
    }
    #[test]
    fn bucket_rejects_open_freehand_paths() {
        let mut ring = shape(&[(10.0, 10.0), (50.0, 10.0), (50.0, 50.0),
            (10.0, 50.0)]);
        ring.is_vector = false;
        assert!(!contour_contains(&ring, 30.0, 30.0));
    }

    #[test]
    fn bucket_chooses_topmost_overlapping_closed_shape() {
        let first = shape(&[(0.0,0.0),(50.0,0.0),(50.0,50.0),(0.0,50.0),(0.0,0.0)]);
        let top = shape(&[(10.0,10.0),(40.0,10.0),(40.0,40.0),(10.0,40.0),(10.0,10.0)]);
        let mut shapes = vec![first, top];
        assert!(apply_shape_fill(&mut shapes, 25.0, 25.0, [50,60,70,255]));
        assert!(shapes[0].fill_color.is_none());
        assert_eq!(shapes[1].fill_color, Some([50,60,70,255]));
    }
}
