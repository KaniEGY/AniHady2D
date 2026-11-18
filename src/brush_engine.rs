//! Pressure-aware stroke geometry shared by the live canvas and raster exports.

use crate::project::{BrushTipShape, KritaBrushPreset, Stroke, StrokePoint};

/// Each layer is painted once so a translucent wash can build up between
/// strokes without darkening at input event boundaries.
pub enum RoundMark {
    Dot(BrushDab),
    Outline(Vec<(f32, f32)>),
    Line { width: f32 },
}

pub fn visit_round_marks(stroke: &Stroke, mut paint: impl FnMut(RoundMark, f32)) {
    let Some(first) = stroke.points.first() else {
        return;
    };
    let layers: &[(f32, f32)] = match (stroke.is_vector, stroke.brush_preset) {
        (false, KritaBrushPreset::Watercolor) => &[(1.25, 1.0 / 3.0), (0.90, 1.0)],
        (false, KritaBrushPreset::SoftAirbrush) => &[(1.55, 0.25), (1.05, 0.5), (0.65, 1.0)],
        _ => &[(1.0, 1.0)],
    };
    let pressure = if stroke.is_vector {
        1.0
    } else {
        first.pressure.clamp(0.02, 2.0)
    };
    let varying = !stroke.is_vector
        && stroke
            .points
            .iter()
            .any(|p| (p.pressure - pressure).abs() > 0.001);
    for &(size, flow) in layers {
        let width = stroke.base_width * size;
        if stroke.points.len() == 1 {
            paint(RoundMark::Dot(dab_for(first, width)), flow);
        } else {
            if varying || (!stroke.is_vector && stroke.brush_preset == KritaBrushPreset::GPenInk) {
                let outline = crate::stroke_geometry::pressure_outline(&stroke.points, width);
                if !outline.is_empty() {
                    paint(RoundMark::Outline(outline), flow);
                    continue;
                }
            }
            paint(
                RoundMark::Line {
                    width: (width * pressure).max(0.5),
                },
                flow,
            );
        }
    }
}

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
        BrushTipShape::Square | BrushTipShape::TallRectangle | BrushTipShape::WideRectangle => {
            vec![
                (dab.x - w, dab.y - h),
                (dab.x + w, dab.y - h),
                (dab.x + w, dab.y + h),
                (dab.x - w, dab.y + h),
            ]
        }
        BrushTipShape::Triangle | BrushTipShape::Pentagon => {
            let count = if tip == BrushTipShape::Triangle { 3 } else { 5 };
            (0..count)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2
                        + std::f32::consts::TAU * i as f32 / count as f32;
                    (dab.x + r * a.cos(), dab.y + r * a.sin())
                })
                .collect()
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

/// Visit dabs at distances measured along the whole path, independent of input
/// event density. The budget stretches spacing across extreme paths so their
/// entire trajectory is represented.
pub fn visit_pressure_dabs(
    points: &[StrokePoint],
    width: f32,
    mut paint: impl FnMut(BrushDab, [f32; 2]),
) {
    if !width.is_finite() || width <= 0.0 {
        return;
    }
    let valid = points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite() && p.pressure.is_finite());
    let mut path = valid.clone();
    let Some(first) = path.next() else { return };
    let width = width.clamp(0.5, 1024.0);

    // Use f64 for travel distances: a very large coordinate must still
    // advance when adding one spacing interval.
    let length = |a: &StrokePoint, b: &StrokePoint| {
        (f64::from(b.x) - f64::from(a.x)).hypot(f64::from(b.y) - f64::from(a.y))
    };
    let total: f64 = valid.zip(path.clone()).map(|(a, b)| length(a, b)).sum();
    let minimum_spacing = total / (MAX_DABS_PER_STROKE - 2) as f64;
    let spacing = |d: BrushDab| (f64::from(d.radius) * 0.48).max(0.2).max(minimum_spacing);
    let mut previous = first;
    let mut last_dab = dab_for(first, width);
    let mut direction = [1.0, 0.0];
    let mut started = false;
    let mut remaining = spacing(last_dab);
    let mut count = 0;
    for next in path {
        let len = length(previous, next);
        if len > 0.0 {
            let dx = f64::from(next.x) - f64::from(previous.x);
            let dy = f64::from(next.y) - f64::from(previous.y);
            direction = [(dx / len) as f32, (dy / len) as f32];
            if !started {
                paint(last_dab, direction);
                started = true;
                count += 1;
            }
            while remaining <= len && count < MAX_DABS_PER_STROKE - 1 {
                let t = remaining / len;
                let sample = StrokePoint {
                    x: (f64::from(previous.x) + dx * t) as f32,
                    y: (f64::from(previous.y) + dy * t) as f32,
                    pressure: previous.pressure + (next.pressure - previous.pressure) * t as f32,
                };
                last_dab = dab_for(&sample, width);
                paint(last_dab, direction);
                count += 1;
                remaining += spacing(last_dab);
            }
            remaining -= len;
        }
        previous = next;
    }
    if !started || (last_dab.x - previous.x).hypot(last_dab.y - previous.y) > 0.01 {
        paint(dab_for(previous, width), direction);
    }
}

#[cfg(test)]
fn sample_pressure_dabs(points: &[StrokePoint], width: f32) -> Vec<BrushDab> {
    let mut dabs = Vec::new();
    visit_pressure_dabs(points, width, |dab, _| dabs.push(dab));
    dabs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32, pressure: f32) -> StrokePoint {
        StrokePoint { x, y, pressure }
    }

    #[test]
    fn six_tip_shapes_have_correct_vertex_counts() {
        let d = BrushDab {
            x: 10.0,
            y: 15.0,
            radius: 5.0,
        };
        assert_eq!(dab_polygon(BrushTipShape::Round, d).len(), 0);
        assert_eq!(dab_polygon(BrushTipShape::Square, d).len(), 4);
        assert_eq!(dab_polygon(BrushTipShape::Triangle, d).len(), 3);
        assert_eq!(dab_polygon(BrushTipShape::Pentagon, d).len(), 5);
        assert_eq!(dab_polygon(BrushTipShape::TallRectangle, d).len(), 4);
        assert_eq!(dab_polygon(BrushTipShape::WideRectangle, d).len(), 4);
    }

    #[test]
    fn gpen_single_tap_generates_a_visible_dab() {
        let dabs = sample_pressure_dabs(&[point(4.0, 8.0, 0.5)], 10.0);
        assert_eq!(dabs.len(), 1);
        assert!((dabs[0].radius - 2.5).abs() < 0.01);
    }

    #[test]
    fn interpolation_preserves_start_end_and_varies_width() {
        let dabs = sample_pressure_dabs(&[point(0.0, 0.0, 0.2), point(40.0, 0.0, 1.0)], 10.0);
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
        let dabs = sample_pressure_dabs(&[point(0.0, 0.0, 1.0), point(200_000.0, 0.0, 1.0)], 6.0);
        assert!(dabs.len() <= MAX_DABS_PER_STROKE);
        assert!((dabs.last().unwrap().x - 200_000.0).abs() < 0.01);
        assert!(dabs.windows(2).all(|p| p[1].x - p[0].x <= 11.0));
    }

    #[test]
    fn pressure_spacing_is_independent_of_input_event_density() {
        let sparse = [point(0., 0., 0.25), point(100., 0., 1.0)];
        let dense: Vec<_> = (0..=100)
            .map(|i| point(i as f32, 0., 0.25 + 0.75 * i as f32 / 100.))
            .collect();
        let samples = sample_pressure_dabs(&sparse, 8.);
        let other = sample_pressure_dabs(&dense, 8.);
        assert_eq!(samples.len(), other.len());
        assert!(samples
            .iter()
            .zip(other)
            .all(|(a, b)| (a.x - b.x).abs() < 0.001 && (a.radius - b.radius).abs() < 0.001));
    }
}
