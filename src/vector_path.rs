//! Non-destructive vector interpolation shared by live canvas and raster/video export.
//! Control points in the project are never changed by render-time smoothing.
use crate::project::StrokePoint;

const MAX_CONTROL_POINTS: usize = 5_000;
const MAX_SUBDIVISIONS: usize = 24;
const CORNER_COS_THRESHOLD: f32 = 0.55;

type Point = (f32, f32);

fn distance(a: Point, b: Point) -> f32 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// Preserve deliberate corners instead of rounding every node as if it were
/// a smooth Bezier handle. A deflection of roughly 57 degrees is treated as a
/// corner. Coincident neighbors are handled by the duplicate-point filter.
fn sharp_corner(before: Point, at: Point, after: Point) -> bool {
    let incoming = (at.0 - before.0, at.1 - before.1);
    let outgoing = (after.0 - at.0, after.1 - at.1);
    let lengths = distance(before, at) * distance(at, after);
    lengths > 0.0001
        && (incoming.0 * outgoing.0 + incoming.1 * outgoing.1) / lengths
            < CORNER_COS_THRESHOLD
}

fn tangent(before: Point, at: Point, after: Point, segment: Point, segment_len: f32) -> Point {
    if sharp_corner(before, at, after) {
        // Each adjacent segment uses its own chord at a hard corner, so
        // the corner stays at the precise position of the editable node.
        return segment;
    }
    let total = distance(before, at) + distance(at, after);
    if total <= 0.0001 { return segment; }
    let scale = segment_len / total;
    ((after.0 - before.0) * scale, (after.1 - before.1) * scale)
}

/// Interpolate curved vector segments with an adaptive sample count. This
/// avoids obvious flat facets on long curves, without multiplying tiny
/// segments into excessive geometry or modifying any saved vector nodes.
///
/// Closed strokes use cyclic tangents so the seam does not develop a cusp.
/// Straight segments and intentional hard corners remain straight.
pub fn smoothed_path(points: &[StrokePoint]) -> Vec<Point> {
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Vec::new();
    }
    if points.len() < 3 || points.len() > MAX_CONTROL_POINTS {
        return points.iter().map(|p| (p.x, p.y)).collect();
    }

    let closed = points.len() > 3
        && distance(
            (points[0].x, points[0].y),
            (points[points.len() - 1].x, points[points.len() - 1].y),
        ) <= 0.001;
    let end = points.len() - usize::from(closed);
    let mut nodes: Vec<Point> = Vec::with_capacity(end);
    for p in &points[..end] {
        let pos = (p.x, p.y);
        if nodes.last().is_none_or(|&last| distance(last, pos) > 0.001) {
            nodes.push(pos);
        }
    }
    let n = nodes.len();
    if n < 3 {
        let mut path = nodes;
        if closed && !path.is_empty() { path.push(path[0]); }
        return path;
    }

    let edges = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(1 + edges * 6);
    out.push(nodes[0]);
    for i in 0..edges {
        let a = if i > 0 { nodes[i - 1] } else if closed { nodes[n - 1] } else { nodes[0] };
        let b = nodes[i];
        let c = nodes[(i + 1) % n];
        let d = if i + 2 < n { nodes[i + 2] } else if closed {
            nodes[(i + 2) % n]
        } else { nodes[n - 1] };

        let length = distance(b, c);
        if !length.is_finite() || length <= 0.001 {
            out.push(c);
            continue;
        }
        // At a hard corner the outgoing/incoming tangent follows this
        // segment's own chord. Gentle bends use chord-length-weighted Hermite
        // tangents to reduce uneven input-point spacing artifacts.
        let chord = (c.0 - b.0, c.1 - b.1);
        let m0 = tangent(a, b, c, chord, length);
        let m1 = tangent(b, c, d, chord, length);
        let steps = ((length / 6.0).ceil() as usize).clamp(3, MAX_SUBDIVISIONS);

        for j in 1..=steps {
            if j == steps {
                // Keep the original control node exactly, even after float
                // calculations; this also guarantees exact loop closure.
                out.push(c);
                continue;
            }
            let t = j as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let h0 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h1 = t3 - 2.0 * t2 + t;
            let h2 = -2.0 * t3 + 3.0 * t2;
            let h3 = t3 - t2;
            let x = h0 * b.0 + h1 * m0.0 + h2 * c.0 + h3 * m1.0;
            let y = h0 * b.1 + h1 * m0.1 + h2 * c.1 + h3 * m1.1;
            // Safeguard against loops and badly spaced nodes without the
            // axis-by-axis per-segment clamp that flattened smooth bends.
            let margin = length * 0.1;
            let min_x = a.0.min(b.0).min(c.0).min(d.0) - margin;
            let max_x = a.0.max(b.0).max(c.0).max(d.0) + margin;
            let min_y = a.1.min(b.1).min(c.1).min(d.1) - margin;
            let max_y = a.1.max(b.1).max(c.1).max(d.1) + margin;
            out.push((x.clamp(min_x, max_x), y.clamp(min_y, max_y)));
        }
    }
    out
}

/// Extend an open polyline by half its stroke width for square line caps.
/// Closed paths have no endpoints and must never gain artificial extensions.
pub fn extend_square_caps(path: &mut [Point], width: f32) {
    if path.len() < 2 || !width.is_finite() || width <= 0.0
        || path.iter().any(|&(x, y)| !x.is_finite() || !y.is_finite())
    { return; }
    let first = path[0];
    let last_idx = path.len() - 1;
    let last = path[last_idx];
    if path.len() > 2 && distance(first, last) <= 0.001 { return; }

    let first_direction = path.iter().skip(1)
        .find(|&&p| distance(first, p) > 0.001).copied();
    let last_direction = path.iter().rev().skip(1)
        .find(|&&p| distance(last, p) > 0.001).copied();
    let radius = width * 0.5;
    if let Some(next) = first_direction {
        let len = distance(first, next);
        path[0] = (first.0 - (next.0 - first.0) / len * radius,
                   first.1 - (next.1 - first.1) / len * radius);
    }
    if let Some(before) = last_direction {
        let len = distance(before, last);
        path[last_idx] = (last.0 + (last.0 - before.0) / len * radius,
                          last.1 + (last.1 - before.1) / len * radius);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f32, y: f32) -> StrokePoint { StrokePoint { x, y, pressure: 1.0 } }

    #[test]
    fn smooth_path_preserves_ends_and_editable_control_nodes() {
        let nodes = vec![p(0., 0.), p(20., 10.), p(35., 10.), p(45., 30.)];
        let before = nodes.clone();
        let result = smoothed_path(&nodes);
        assert_eq!(nodes, before);
        assert_eq!(result.first(), Some(&(0., 0.)));
        assert_eq!(result.last(), Some(&(45., 30.)));
        for node in nodes {
            assert!(result.contains(&(node.x, node.y)));
        }
        assert!(result.windows(2).all(|w|
            w.iter().all(|&(x, y)| x.is_finite() && y.is_finite())));
    }

    #[test]
    fn a_straight_line_remains_collinear_even_with_many_nodes() {
        let path = smoothed_path(&[p(0., 1.), p(10., 1.), p(100., 1.)]);
        assert!(path.len() > 10);
        assert!(path.iter().all(|&(x, y)| (y - 1.).abs() < 0.0001 && x >= -0.001 && x <= 100.001));
        assert!(path.windows(2).all(|w| w[1].0 + 0.001 >= w[0].0));
    }

    #[test]
    fn fast_corner_is_not_rounded_away_or_overshot() {
        let path = smoothed_path(&[p(0., 0.), p(40., 0.), p(40., 40.)]);
        assert!(path.contains(&(40., 0.)));
        assert!(path.iter().all(|&(x, y)| x >= -0.001 && x <= 40.001 && y >= -0.001 && y <= 40.001));
        assert!(path.iter().all(|&(x, y)| (x - 40.).abs() < 0.001 || y.abs() < 0.001));
    }

    #[test]
    fn a_closed_circle_like_path_has_a_smooth_seam() {
        let points = [p(0., -30.), p(30., 0.), p(0., 30.), p(-30., 0.), p(0., -30.)];
        let path = smoothed_path(&points);
        assert_eq!(path.first(), path.last());
        assert!(path.len() > points.len());
        let mut capped = path.clone();
        extend_square_caps(&mut capped, 8.);
        assert_eq!(capped, path);
    }

    #[test]
    fn duplicate_and_invalid_samples_are_bounded() {
        let path = smoothed_path(&[p(0., 0.), p(0., 0.), p(10., 5.), p(20., 5.)]);
        assert_eq!(path.first(), Some(&(0., 0.)));
        assert_eq!(path.last(), Some(&(20., 5.)));
        assert!(path.len() < 100);
        assert!(smoothed_path(&[p(0., 0.), p(f32::NAN, 1.), p(5., 5.)]).is_empty());
    }

    #[test]
    fn square_caps_extend_both_ends_and_ignore_duplicate_neighbors() {
        let mut path = vec![(0., 0.), (0., 0.), (10., 0.), (10., 0.)];
        extend_square_caps(&mut path, 4.);
        assert_eq!(path[0], (-2., 0.));
        assert_eq!(path[3], (12., 0.));
    }

    #[test]
    fn long_curves_get_more_render_samples_than_short_ones() {
        let short = smoothed_path(&[p(0., 0.), p(10., 5.), p(20., 0.)]);
        let long = smoothed_path(&[p(0., 0.), p(100., 50.), p(200., 0.)]);
        assert!(long.len() > short.len());
        assert!(long.len() <= 1 + 2 * MAX_SUBDIVISIONS);
    }
}
