//! Shared pressure-aware dab sampling for Stick's existing G-Pen strokes.
//!
//! This is a first, dependency-free drawing-engine increment. It does NOT
//! claim to embed Krita or libmypaint. The same geometry is consumed by the
//! interactive preview and the PNG/video exporter; project schema stays v5.

use crate::project::{BrushTipShape, StrokePoint};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushDab {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

const MAX_DABS_PER_STROKE: usize = 20_000;

/// The same polygon geometry is used for the canvas and exported images.
pub fn dab_polygon(tip: BrushTipShape, dab: BrushDab) -> Vec<(f32, f32)> {
    let r = dab.radius.max(0.25);
    let (w, h) = match tip {
        BrushTipShape::TallRectangle => (r * 0.5, r * 1.5),
        BrushTipShape::WideRectangle => (r * 1.5, r * 0.5),
        _ => (r, r),
    };
    match tip {
        BrushTipShape::Round => Vec::new(),
        BrushTipShape::Square | BrushTipShape::TallRectangle | BrushTipShape::WideRectangle => vec![
            (dab.x-w, dab.y-h), (dab.x+w, dab.y-h),
            (dab.x+w, dab.y+h), (dab.x-w, dab.y+h),
        ],
        BrushTipShape::Triangle | BrushTipShape::Pentagon => {
            let count = if tip == BrushTipShape::Triangle { 3 } else { 5 };
            (0..count).map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 +
                    std::f32::consts::TAU * i as f32 / count as f32;
                (dab.x + r*a.cos(), dab.y + r*a.sin())
            }).collect()
        }
    }
}


fn dab_for(p: &StrokePoint, width: f32) -> BrushDab {
    BrushDab {
        x: p.x,
        y: p.y,
        radius: (0.5 * width * p.pressure.clamp(0.02, 2.0)).max(0.25),
    }
}

/// Samples a polyline into overlapping circular dabs, interpolating pen
/// pressure per segment. Every input point is already in project coordinates;
/// callers apply the view or symbol transform while drawing.
///
/// This keeps the saved Stroke format unchanged for compatibility with all
/// existing Stick v5 projects. A safety cap avoids pathological allocations.
pub fn sample_pressure_dabs(points: &[StrokePoint], width: f32) -> Vec<BrushDab> {
    if points.is_empty() || !width.is_finite() || width <= 0.0 {
        return Vec::new();
    }

    let points: Vec<&StrokePoint> = points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite() && p.pressure.is_finite())
        .collect();
    if points.is_empty() {
        return Vec::new();
    }

    let width = width.clamp(0.5, 1024.0);
    let step = (width * 0.22).clamp(0.75, 6.0);
    let mut out = Vec::new();
    out.push(dab_for(points[0], width));
    let mut until_next = step;

    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy);
        if !len.is_finite() || len <= f32::EPSILON {
            continue;
        }

        let mut dist = until_next;
        while dist <= len {
            if out.len() >= MAX_DABS_PER_STROKE - 1 {
                // Guarantee the final endpoint is represented even for
                // enormous input paths. Keep memory bounded.
                let last = points[points.len() - 1];
                out.push(dab_for(last, width));
                return out;
            }
            let t = (dist / len).clamp(0.0, 1.0);
            let sample = StrokePoint {
                x: a.x + dx * t,
                y: a.y + dy * t,
                pressure: a.pressure + (b.pressure - a.pressure) * t,
            };
            out.push(dab_for(&sample, width));
            dist += step;
        }
        until_next = dist - len;
        if until_next <= 0.0 || !until_next.is_finite() {
            until_next = step;
        }
    }

    let last = points[points.len() - 1];
    if let Some(prev) = out.last() {
        if (prev.x - last.x).hypot(prev.y - last.y) > 0.01 {
            out.push(dab_for(last, width));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32, pressure: f32) -> StrokePoint {
        StrokePoint { x, y, pressure }
    }

    #[test]
    fn six_tip_shapes_have_correct_vertex_counts() {
        let d = BrushDab { x: 10.0, y: 15.0, radius: 5.0 };
        assert_eq!(dab_polygon(BrushTipShape::Round,d).len(),0);
        assert_eq!(dab_polygon(BrushTipShape::Square,d).len(),4);
        assert_eq!(dab_polygon(BrushTipShape::Triangle,d).len(),3);
        assert_eq!(dab_polygon(BrushTipShape::Pentagon,d).len(),5);
        assert_eq!(dab_polygon(BrushTipShape::TallRectangle,d).len(),4);
        assert_eq!(dab_polygon(BrushTipShape::WideRectangle,d).len(),4);
    }

    #[test]
    fn gpen_single_tap_generates_a_visible_dab() {
        let dabs = sample_pressure_dabs(&[point(4.0, 8.0, 0.5)], 10.0);
        assert_eq!(dabs.len(), 1);
        assert!((dabs[0].radius - 2.5).abs() < 0.01);
    }

    #[test]
    fn interpolation_preserves_start_end_and_varies_width() {
        let dabs = sample_pressure_dabs(
            &[point(0.0, 0.0, 0.2), point(40.0, 0.0, 1.0)],
            10.0,
        );
        assert!(dabs.len() > 5);
        assert!((dabs.first().unwrap().x - 0.0).abs() < 0.01);
        assert!((dabs.last().unwrap().x - 40.0).abs() < 0.01);
        assert!(dabs.first().unwrap().radius < dabs.last().unwrap().radius);
        assert!(dabs.iter().all(|d| d.radius.is_finite() && d.radius > 0.0));
    }

    #[test]
    fn invalid_or_empty_input_is_safe() {
        assert!(sample_pressure_dabs(&[], 7.0).is_empty());
        assert!(sample_pressure_dabs(&[point(1.0, 1.0, 1.0)], f32::NAN).is_empty());
        assert!(sample_pressure_dabs(&[point(f32::NAN, 0.0, 1.0)], 7.0).is_empty());
    }

    #[test]
    fn dab_count_stays_bounded_on_long_strokes() {
        let dabs = sample_pressure_dabs(
            &[point(0.0, 0.0, 1.0), point(200_000.0, 0.0, 1.0)],
            6.0,
        );
        assert!(dabs.len() <= MAX_DABS_PER_STROKE);
        assert!((dabs.last().unwrap().x - 200_000.0).abs() < 0.01);
    }
}
