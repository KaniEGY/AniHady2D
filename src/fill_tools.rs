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


/// Vector-friendly raster region detection for complex painted contours.
/// This fallback detects enclosed regions even when a shape is formed from
/// many separate strokes, T-junctions or interior intersections.
///
/// We rasterize only a *bounded* coverage mask (never an unbounded image),
/// flood the connected transparent area, then trace the outermost pixel edge
/// back to a vector polygon. The resulting fill stays BELOW the linework.
/// This is an independently implemented adaptation of the general method
/// described in AniCraft's permissively licensed paint-bucket documentation.
pub fn flood_fill_polygon(
    strokes: &[Stroke], point: (f32, f32), canvas: (f32, f32), gap: f32,
) -> Option<Vec<StrokePoint>> {
    use std::collections::{HashMap, VecDeque};
    let (cw, ch) = canvas;
    let (sx, sy) = point;
    if !cw.is_finite() || !ch.is_finite() || cw <= 0.0 || ch <= 0.0
        || !sx.is_finite() || !sy.is_finite() || sx < 0.0 || sy < 0.0
        || sx >= cw || sy >= ch || strokes.is_empty() { return None; }
    // Limit memory and the number of points independently from canvas size.
    let cell = (cw.max(ch) / 1200.0).max(0.5);
    let w = (cw / cell).ceil() as usize + 4;
    let h = (ch / cell).ceil() as usize + 4;
    if w.checked_mul(h)? > 2_000_000 { return None; }
    let size = w * h;
    let mut wall = vec![false; size];
    let world_to_px = |x: f32, y: f32| -> (f32, f32) {
        (x / cell + 2.0, y / cell + 2.0)
    };
    let mut segment_budget = 0usize;
    for stroke in strokes {
        if stroke.points.len() < 2 || stroke.points.len() > 40_000 { continue; }
        if stroke.points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) { continue; }
        // Ignore fill-only strokes: they should not block a new fill.
        if stroke.color[3] == 0 { continue; }
        segment_budget = segment_budget.saturating_add(stroke.points.len());
        if segment_budget > 250_000 { return None; }
        let r = ((stroke.base_width.abs() * 0.5 + gap.clamp(0.0, 14.0)) / cell)
            .clamp(1.15, 9.0);
        let r2 = r * r;
        for pair in stroke.points.windows(2) {
            let (x0, y0) = world_to_px(pair[0].x, pair[0].y);
            let (x1, y1) = world_to_px(pair[1].x, pair[1].y);
            let steps = (x1-x0).abs().max((y1-y0).abs()).ceil() as usize;
            if steps > 15_000 { continue; }
            for i in 0..=steps.max(1) {
                let t = i as f32 / steps.max(1) as f32;
                let cx = x0 + (x1-x0)*t;
                let cy = y0 + (y1-y0)*t;
                let min_x = (cx-r).floor().max(0.0) as usize;
                let max_x = (cx+r).ceil().min((w-1) as f32) as usize;
                let min_y = (cy-r).floor().max(0.0) as usize;
                let max_y = (cy+r).ceil().min((h-1) as f32) as usize;
                for iy in min_y..=max_y {
                    for ix in min_x..=max_x {
                        let dx = ix as f32 + 0.5 - cx;
                        let dy = iy as f32 + 0.5 - cy;
                        if dx*dx + dy*dy <= r2 { wall[iy*w+ix] = true; }
                    }
                }
            }
        }
    }
    let (px, py) = world_to_px(sx, sy);
    let ix = px.floor() as usize;
    let iy = py.floor() as usize;
    if ix >= w || iy >= h { return None; }
    let seed = iy*w+ix;
    // Clicking the contour itself is ambiguous. Old contour fill handles it.
    if wall[seed] { return None; }
    let mut visited = vec![false; size];
    let mut queue = VecDeque::new();
    visited[seed] = true;
    queue.push_back(seed);
    let mut hits_edge = false;
    let mut count = 0usize;
    while let Some(index) = queue.pop_front() {
        count += 1;
        let x = index % w;
        let y = index / w;
        if x <= 1 || y <= 1 || x >= w - 2 || y >= h - 2 {
            hits_edge = true; break;
        }
        for next in [index-1,index+1,index-w,index+w] {
            if !wall[next] && !visited[next] {
                visited[next] = true;
                queue.push_back(next);
            }
        }
    }
    if hits_edge || count < 8 { return None; }

    // Krita/AniCraft-style fill growth under drawn linework. The wall mask
    // includes a gap-closing dilation. Tracing the bare transparent area
    // leaves a white hairline between the color and the visible brush.
    // Grow the selected component INTO wall cells only, never into another
    // transparent component: a center divider therefore remains a barrier.
    let grow_steps = ((gap.clamp(0.0, 14.0) + 1.5) / cell).ceil()
        .clamp(1.0, 20.0) as usize;
    let mut covered = visited.clone();
    let mut boundary = VecDeque::<(usize, usize)>::new();
    for index in 0..size {
        if !visited[index] { continue; }
        let x = index % w;
        let y = index / w;
        if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h { continue; }
        for next in [index - 1, index + 1, index - w, index + w] {
            if wall[next] && !covered[next] {
                covered[next] = true;
                boundary.push_back((next, 1));
            }
        }
    }
    while let Some((index, distance)) = boundary.pop_front() {
        if distance >= grow_steps { continue; }
        let x = index % w;
        let y = index / w;
        if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h { continue; }
        for next in [index - 1, index + 1, index - w, index + w] {
            if wall[next] && !covered[next] {
                covered[next] = true;
                boundary.push_back((next, distance + 1));
            }
        }
    }

    // Build clockwise edges of the flooded pixels. The topmost edge is on
    // the outside perimeter; secondary loops (holes) remain undisturbed.
    type Corner = (i32, i32);
    let mut edges: HashMap<Corner, Vec<Corner>> = HashMap::new();
    let mut edge_count = 0usize;
    for y in 2..h-2 {
        for x in 2..w-2 {
            if !covered[y*w+x] { continue; }
            let x = x as i32;
            let y = y as i32;
            let pos = (y as usize)*w+(x as usize);
            let mut add = |from:Corner, to:Corner| {
                edges.entry(from).or_default().push(to);
                edge_count += 1;
            };
            if !covered[pos-w] { add((x,y),(x+1,y)); }
            if !covered[pos+1] { add((x+1,y),(x+1,y+1)); }
            if !covered[pos+w] { add((x+1,y+1),(x,y+1)); }
            if !covered[pos-1] { add((x,y+1),(x,y)); }
        }
    }
    if edge_count < 8 || edge_count > 70_000 { return None; }
    let start = *edges.keys().min_by_key(|(x,y)| (*y,*x))?;
    let mut at = start;
    let mut ring = Vec::new();
    for _ in 0..=edge_count {
        ring.push(at);
        let options = edges.get_mut(&at)?;
        let next = options.pop()?;
        if options.is_empty() { edges.remove(&at); }
        at = next;
        if at == start { break; }
    }
    if at != start || ring.len() < 8 { return None; }

    // Remove collinear pixel corners: keep stored vector projects compact.
    let mut clean = Vec::<Corner>::new();
    for i in 0..ring.len() {
        let p = ring[(i+ring.len()-1)%ring.len()];
        let c = ring[i];
        let n = ring[(i+1)%ring.len()];
        if (c.0-p.0)*(n.1-c.1) != (c.1-p.1)*(n.0-c.0) { clean.push(c); }
    }
    if clean.len() < 4 || clean.len() > 14_000 { return None; }
    let mut result: Vec<StrokePoint> = clean.iter().map(|&(x,y)| StrokePoint {
        x: ((x as f32 - 2.0) * cell).clamp(0.0,cw),
        y: ((y as f32 - 2.0) * cell).clamp(0.0,ch),
        pressure: 1.0,
    }).collect();
    result.push(result[0].clone());
    Some(result)
}

pub fn polygon_fill_stroke(points: Vec<StrokePoint>, color: [u8;4]) -> Stroke {
    Stroke {
        points, color: [0,0,0,0], fill_color: Some(color),
        base_width: 1.0, is_vector: true,
        brush_preset: KritaBrushPreset::GPenInk, opacity: 1.0,
        brush_texture_id: None, tip_shape: BrushTipShape::Round,
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
    fn flood_fill_detects_complex_multi_stroke_square() {
        let lines = vec![
            shape(&[(15.,15.),(90.,15.)]),
            shape(&[(90.,15.),(90.,90.)]),
            shape(&[(90.,90.),(15.,90.)]),
            shape(&[(15.,90.),(15.,15.)]),
            shape(&[(51.,15.),(51.,89.)]),
        ];
        let ring = flood_fill_polygon(&lines,(34.,34.),(120.,120.),1.4).expect("closed left half");
        assert!(ring.len() >= 5);
        let filled = polygon_fill_stroke(ring,[245,80,20,255]);
        assert!(contour_contains(&filled,34.,34.));
        assert!(!contour_contains(&filled,73.,34.));
    }
    #[test]
    fn flood_fill_rejects_unbounded_open_contour() {
        let lines = vec![
            shape(&[(15.,15.),(90.,15.)]),
            shape(&[(90.,15.),(90.,90.)]),
            shape(&[(15.,90.),(15.,15.)]),
        ];
        assert!(flood_fill_polygon(&lines,(34.,34.),(120.,120.),1.0).is_none());
    }
    #[test]
    fn bucket_grows_color_beneath_visible_ink_without_leaking_outside() {
        let mut border = shape(&[(25.,25.),(140.,25.),(140.,140.),
                                  (25.,140.),(25.,25.)]);
        border.base_width = 8.0;
        let polygon = flood_fill_polygon(&[border],(80.,80.),(180.,180.),3.5)
            .expect("enclosed area");
        let filled = polygon_fill_stroke(polygon,[200,80,20,255]);
        // The inside half of the 8px line must be filled *under* the ink.
        assert!(contour_contains(&filled,28.0,70.0),
            "filling should not leave a white halo just inside the outline");
        assert!(!contour_contains(&filled,17.0,70.0),
            "color must not escape outside the contour");
    }

    #[test]
    fn divided_circle_fills_only_the_clicked_half() {
        // The outer closed contour is one stroke. A separate diameter
        // must become a wall; filling the outer contour would be wrong.
        let circle: Vec<(f32,f32)> = (0..=96).map(|i| {
            let t = (i as f32) * std::f32::consts::TAU / 96.0;
            (110.0 + 80.0 * t.cos(), 110.0 + 80.0 * t.sin())
        }).collect();
        let strokes = vec![shape(&circle), shape(&[(110.,30.),(110.,190.)])];
        let ring = flood_fill_polygon(&strokes,(72.,110.),(220.,220.),1.4)
            .expect("left region should be enclosed by circle and diameter");
        let left = polygon_fill_stroke(ring, [220,30,80,255]);
        assert!(contour_contains(&left,72.,110.));
        assert!(!contour_contains(&left,147.,110.),
            "diameter must separate the other half");
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
