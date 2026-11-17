//! Finger/sculpt brush for editing vector-backed and freehand strokes.
//! Displaces points with a smooth falloff and inserts local intermediate
//! vertices on long segments, so large brush radii can pull an edge rather
//! than moving only sparse endpoint nodes.
use crate::project::{Stroke,StrokePoint};

pub fn sculpt(strokes:&mut [Stroke],at:(f32,f32),motion:(f32,f32),radius:f32,strength:f32)->bool {
    if !at.0.is_finite()||!at.1.is_finite()
        ||!motion.0.is_finite()||!motion.1.is_finite()
        ||!radius.is_finite()||radius<2.0
        ||!strength.is_finite()||strength<=0.0{return false;}
    let radius=radius.clamp(2.0,500.0);
    let mut changed=false;
    for stroke in strokes {
        // Do not deform closed fill-only faces without rebuilding their
        // topology; limit sculpt to actual linework.
        if stroke.points.is_empty() || stroke.fill_color.is_some() {continue;}
        if stroke.points.len() >= 2 && stroke.points.len() < 6_000 {
            let mut dense=Vec::with_capacity(stroke.points.len()*2);
            dense.push(stroke.points[0].clone());
            for pair in stroke.points.windows(2) {
                let a=&pair[0];let b=&pair[1];
                let length=(b.x-a.x).hypot(b.y-a.y);
                let mx=(a.x+b.x)*0.5;let my=(a.y+b.y)*0.5;
                // Only refine segments near the sculpt zone.
                let close=(mx-at.0).hypot(my-at.1)<=radius+length*0.5;
                let step=(radius*0.28).clamp(5.0,35.0);
                let subdivisions=if close {(length/step).ceil().clamp(1.0,16.0) as usize}else{1};
                for k in 1..=subdivisions {
                    let t=k as f32/subdivisions as f32;
                    dense.push(StrokePoint{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t,
                        pressure:a.pressure+(b.pressure-a.pressure)*t});
                }
                if dense.len()>15_000 {return changed;}
            }
            stroke.points=dense;
        }
        for point in &mut stroke.points {
            let d=(point.x-at.0).hypot(point.y-at.1);
            if d>=radius{continue;}
            let t=(1.0-d/radius).clamp(0.0,1.0);
            let w=t*t*(3.0-2.0*t)*strength.clamp(0.0,1.0);
            point.x+=motion.0*w;
            point.y+=motion.1*w;
            changed=true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{BrushTipShape,KritaBrushPreset};
    #[test]
    fn finger_moves_local_midsegment_but_not_far_points(){
        let mut s=Stroke{points:vec![StrokePoint{x:0.,y:10.,pressure:1.},
                                     StrokePoint{x:200.,y:10.,pressure:1.}],
            color:[0,0,0,255],fill_color:None,base_width:4.,is_vector:true,
            brush_preset:KritaBrushPreset::GPenInk,opacity:1.0,
            brush_texture_id:None,tip_shape:BrushTipShape::Round};
        assert!(sculpt(std::slice::from_mut(&mut s),(100.,10.),(0.,30.),40.,0.8));
        assert!(s.points.iter().any(|p|p.y>10.));
        assert_eq!(s.points.first().unwrap().y,10.);
        assert_eq!(s.points.last().unwrap().y,10.);
    }
}
