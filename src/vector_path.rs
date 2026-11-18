//! Smooth, nondestructive vector rendering path. Saved control points remain
//! untouched so vector node editing and legacy .stick files stay compatible.
use crate::project::StrokePoint;

pub fn smoothed_path(points: &[StrokePoint]) -> Vec<(f32, f32)> {
    if points.len() < 3 || points.len() > 5_000 {
        return points.iter().map(|p| (p.x,p.y)).collect();
    }
    let subdivisions = 3;
    let mut out = Vec::with_capacity((points.len() - 1) * subdivisions + 1);
    for i in 0..points.len() - 1 {
        let a = &points[i.saturating_sub(1)];
        let b = &points[i];
        let c = &points[i + 1];
        let d = &points[(i + 2).min(points.len() - 1)];
        if i == 0 { out.push((b.x,b.y)); }
        for j in 1..=subdivisions {
            let t = j as f32 / subdivisions as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let interpolate = |p0:f32,p1:f32,p2:f32,p3:f32| {
                let value = 0.5 * ((2.0*p1) + (-p0+p2)*t
                    + (2.0*p0-5.0*p1+4.0*p2-p3)*t2
                    + (-p0+3.0*p1-3.0*p2+p3)*t3);
                // Avoid overshooting a sharp turn or drawing a loop that
                // wasn't present in the vector control points.
                value.clamp(p1.min(p2),p1.max(p2))
            };
            out.push((interpolate(a.x,b.x,c.x,d.x),
                interpolate(a.y,b.y,c.y,d.y)));
        }
    }
    out
}

pub fn extend_square_caps(path: &mut [(f32, f32)], width: f32) {
    if path.len() < 2 || !width.is_finite() || width <= 0.0 { return; }
    let distance = width * 0.5;
    let first = path[0];
    let second = path[1];
    let last_idx = path.len() - 1;
    let last = path[last_idx];
    let before = path[last_idx - 1];
    let extend = |a:(f32,f32), b:(f32,f32), backward:bool| {
        let (dx,dy) = (b.0-a.0,b.1-a.1);
        let len = dx.hypot(dy);
        if len > 0.001 {
            let sign = if backward { -1.0 } else { 1.0 };
            (a.0 + sign * dx/len*distance, a.1 + sign*dy/len*distance)
        } else { a }
    };
    path[0] = extend(first, second, true);
    path[last_idx] = extend(last, before, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x:f32,y:f32)->StrokePoint { StrokePoint {x,y,pressure:1.0} }
    #[test]
    fn smooth_path_preserves_ends_and_does_not_mutate_edit_nodes() {
        let nodes=vec![p(0.,0.),p(20.,10.),p(35.,10.),p(45.,30.)];
        let before=nodes.clone();
        let smoothed=smoothed_path(&nodes);
        assert_eq!(nodes,before);
        assert_eq!(smoothed.first(),Some(&(0.,0.)));
        assert_eq!(smoothed.last(),Some(&(45.,30.)));
        assert_eq!(smoothed.len(),10);
        assert!(smoothed.windows(2).all(|w| w.iter().all(|&(x,y)| x.is_finite()&&y.is_finite())));
    }
    #[test]
    fn a_straight_segment_is_unchanged() {
        assert_eq!(smoothed_path(&[p(0.,1.),p(10.,1.)]),vec![(0.,1.),(10.,1.)]);
    }
    #[test]
    fn square_caps_extend_both_ends() {
        let mut path=vec![(0.,0.),(10.,0.)];
        extend_square_caps(&mut path,4.);
        assert_eq!(path,vec![(-2.,0.),(12.,0.)]);
    }
}
