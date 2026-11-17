//! Safe shape-contour filling in the vector-backed Stick canvas.
//!
//! This does not claim to be a raster flood-fill engine. Fully arbitrary
//! painted pixel boundaries need a raster layer and should be implemented
//! separately so old projects remain compatible and the app stays light.
use crate::project::{BrushTipShape, KritaBrushPreset, Stroke, StrokePoint};

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

/// Trace near-touching stroke endpoints to recover closed contours drawn with
/// several pen strokes. This remains vector-contour fill, not pixel flood fill.
/// Click location must lie inside the recovered polygon.
pub fn linked_closed_contour(strokes: &[Stroke], x: f32, y: f32) -> Option<Vec<StrokePoint>> {
    if strokes.is_empty() || strokes.len() > 128 { return None; }
    let mut smallest: Option<(f32, Vec<StrokePoint>)> = None;

    fn dist_sq(a: &StrokePoint, b: &StrokePoint) -> f32 {
        (a.x-b.x)*(a.x-b.x)+(a.y-b.y)*(a.y-b.y)
    }

    let valid = |s: &Stroke| s.points.len() >= 2 && s.points.len() <= 8_192
        && s.points.iter().all(|p| p.x.is_finite() && p.y.is_finite())
        && s.fill_color.is_none();

    for (start_index, start) in strokes.iter().enumerate() {
        if !valid(start) { continue; }
        for start_reversed in [false,true] {
            let mut assembled = start.points.clone();
            if start_reversed { assembled.reverse(); }
            let origin = assembled[0].clone();
            let mut used = vec![false; strokes.len()];
            used[start_index] = true;
            let mut count = 1usize;
            let mut width = start.base_width.abs();

            // A small cap prevents pathological branching or memory growth.
            while count <= 20 && assembled.len() <= 8192 {
                let end = assembled.last().unwrap().clone();
                let tolerance = (width * 2.5).clamp(16.0, 30.0);
                if count >= 2 && dist_sq(&end, &origin) <= tolerance*tolerance {
                    let mut closed = assembled.clone();
                    closed.push(origin.clone());
                    let trial = Stroke {
                        points: closed.clone(),
                        color: [0,0,0,0],
                        fill_color: None,
                        base_width: 1.0,
                        is_vector: true,
                        brush_preset: KritaBrushPreset::GPenInk,
                        opacity: 1.0,
                        brush_texture_id: None,
                        tip_shape: BrushTipShape::Round,
                    };
                    if contour_contains(&trial, x, y) {
                        let area: f32 = closed.windows(2)
                            .map(|pair| pair[0].x*pair[1].y-pair[1].x*pair[0].y)
                            .sum::<f32>().abs() * 0.5;
                        if area > 25.0 && smallest.as_ref().is_none_or(|(old,_)| area < *old) {
                            smallest = Some((area,closed));
                        }
                    }
                    break;
                }
                let mut best: Option<(usize,bool,f32)> = None;
                for (idx, stroke) in strokes.iter().enumerate() {
                    if used[idx] || !valid(stroke) { continue; }
                    let local_tolerance = tolerance.max(
                        (stroke.base_width.abs() * 2.5).clamp(16.0,30.0));
                    let first_d = dist_sq(&end,&stroke.points[0]);
                    let last_d = dist_sq(&end,stroke.points.last().unwrap());
                    for (reversed,d) in [(false,first_d),(true,last_d)] {
                        if d <= local_tolerance * local_tolerance &&
                            best.is_none_or(|(_,_,prev)| d < prev) {
                            best = Some((idx,reversed,d));
                        }
                    }
                }
                let Some((next_index,reversed,_)) = best else { break; };
                used[next_index] = true;
                count += 1;
                let next = &strokes[next_index];
                width = width.max(next.base_width.abs());
                if reversed {
                    assembled.extend(next.points.iter().rev().skip(1).cloned());
                } else {
                    assembled.extend(next.points.iter().skip(1).cloned());
                }
            }
        }
    }
    smallest.map(|(_,p)| p)
}

pub fn can_fill_contour(strokes: &[Stroke], x: f32, y: f32) -> bool {
    find_topmost_shape(strokes, x, y).is_some() ||
        linked_closed_contour(strokes, x, y).is_some()
}

pub fn apply_contour_fill(strokes: &mut Vec<Stroke>, x: f32, y: f32, color: [u8; 4]) -> bool {
    if apply_shape_fill(strokes, x, y, color) { return true; }
    if let Some(points) = linked_closed_contour(strokes, x, y) {
        // Insert underneath linework so the original contours stay visible.
        strokes.insert(0, Stroke {
            points,
            color: [0,0,0,0],
            fill_color: Some(color),
            base_width: 1.0,
            is_vector: true,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0,
            brush_texture_id: None,
            tip_shape: BrushTipShape::Round,
        });
        return true;
    }
    false
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
            brush_texture_id: None, tip_shape: crate::project::BrushTipShape::Round,
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
    fn bucket_joins_multiple_touching_brush_strokes_around_a_region() {
        let mut segments = vec![
            shape(&[(10.0,10.0),(60.0,10.0)]),
            shape(&[(60.0,10.0),(60.0,60.0)]),
            shape(&[(60.0,60.0),(10.0,60.0)]),
            shape(&[(10.0,60.0),(10.0,10.0)]),
        ];
        assert!(can_fill_contour(&segments, 35.0,35.0));
        assert!(apply_contour_fill(&mut segments,35.0,35.0,[200,20,40,255]));
        assert_eq!(segments.len(),5);
        assert_eq!(segments[0].fill_color,Some([200,20,40,255]));
        assert_eq!(segments[0].color[3],0);
    }

    #[test]
    fn disconnected_open_strokes_do_not_create_a_fill() {
        let mut segments = vec![
            shape(&[(10.0,10.0),(60.0,10.0)]),
            shape(&[(60.0,10.0),(60.0,60.0)]),
            shape(&[(60.0,60.0),(10.0,60.0)]),
        ];
        assert!(!can_fill_contour(&segments,35.0,35.0));
        assert!(!apply_contour_fill(&mut segments,35.0,35.0,[200,20,40,255]));
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
