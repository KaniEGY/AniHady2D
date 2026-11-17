//! Pressure-aware continuous brush shape shared between canvas and exporters.
//! The previous renderer painted each dab separately; semi-transparent dabs
//! composited on top of each other as dark overlapping circles.
use crate::project::StrokePoint;

/// A single filled outline, with round caps and averaged corner tangents.
/// Reject out-of-range input and cap the work to keep drawing interactive.
pub fn pressure_outline(points:&[StrokePoint], width:f32)->Vec<(f32,f32)> {
    if !width.is_finite() || width <= 0.0 || points.is_empty() || points.len()>12_000 {
        return Vec::new();
    }
    let clean:Vec<_>=points.iter().filter(|p|p.x.is_finite()&&p.y.is_finite()&&p.pressure.is_finite())
        .filter(|p|p.pressure > 0.0).collect();
    if clean.len()<2 {return Vec::new();}
    let mut samples:Vec<&StrokePoint>=Vec::with_capacity(clean.len());
    for p in clean {
        if samples.last().is_none_or(|q|
            (q.x-p.x).hypot(q.y-p.y)>0.3) {samples.push(p);}
    }
    if samples.len()<2 {return Vec::new();}
    let n=samples.len();
    let mut left=Vec::with_capacity(n);
    let mut right=Vec::with_capacity(n);
    let mut tangents=Vec::with_capacity(n);
    let mut radii=Vec::with_capacity(n);
    for i in 0..n {
        let prev=samples[i.saturating_sub(1)];
        let next=samples[(i+1).min(n-1)];
        let dx=next.x-prev.x;
        let dy=next.y-prev.y;
        let len=dx.hypot(dy).max(0.0001);
        let nx=-dy/len;let ny=dx/len;
        let r=(width*0.5*samples[i].pressure.clamp(0.05,2.0)).clamp(0.25,512.0);
        radii.push(r);
        tangents.push((dx/len,dy/len));
        left.push((samples[i].x+nx*r,samples[i].y+ny*r));
        right.push((samples[i].x-nx*r,samples[i].y-ny*r));
    }
    let mut outline=Vec::with_capacity(2*n+28);
    outline.extend(left);
    // Semicircle at the end, from the left side to the right side.
    let e=samples[n-1];let (tx,ty)=tangents[n-1];
    let base=ty.atan2(tx)+std::f32::consts::FRAC_PI_2;
    for i in 1..=12 {
        let angle=base-std::f32::consts::PI*i as f32/12.0;
        outline.push((e.x+angle.cos()*radii[n-1],e.y+angle.sin()*radii[n-1]));
    }
    outline.extend(right.into_iter().rev().skip(1));
    // Round start cap, continuing from right to left.
    let s=samples[0];let (tx,ty)=tangents[0];
    let base=ty.atan2(tx)-std::f32::consts::FRAC_PI_2;
    for i in 1..=11 {
        let angle=base-std::f32::consts::PI*i as f32/12.0;
        outline.push((s.x+angle.cos()*radii[0],s.y+angle.sin()*radii[0]));
    }
    outline
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x:f32,y:f32,pressure:f32)->StrokePoint{StrokePoint{x,y,pressure}}
    #[test]
    fn continuous_stroke_has_one_outline_with_pressure_width(){
        let v=pressure_outline(&[p(0.,0.,1.),p(50.,0.,0.5),p(100.,0.,1.)],20.);
        assert!(v.len()>20);
        assert!(v.iter().all(|(x,y)|x.is_finite()&&y.is_finite()));
        assert!((v[0].1-v[1].1).abs()>3.0);
    }
    #[test]
    fn repeated_or_invalid_points_do_not_panic(){
        assert!(pressure_outline(&[p(1.,1.,1.),p(1.,1.,1.)],10.).is_empty());
        assert!(pressure_outline(&[p(1.,1.,1.),p(f32::NAN,2.,1.)],10.).is_empty());
    }
}
