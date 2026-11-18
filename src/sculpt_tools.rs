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
    let strength=strength.clamp(0.0,1.0);
    let mut changed=false;

    for stroke in strokes {
        if stroke.points.is_empty(){continue;}

        // Fast reject: large projects may contain thousands of strokes. Avoid
        // rebuilding or walking point-by-point when the brush is nowhere near
        // this stroke. Include half the stroke width so thick/vector lines are
        // still easy to grab.
        let pad=radius+stroke.base_width.max(1.0)*0.5;
        let mut min_x=f32::INFINITY; let mut max_x=f32::NEG_INFINITY;
        let mut min_y=f32::INFINITY; let mut max_y=f32::NEG_INFINITY;
        for p in &stroke.points {
            min_x=min_x.min(p.x); max_x=max_x.max(p.x);
            min_y=min_y.min(p.y); max_y=max_y.max(p.y);
        }
        if at.0 < min_x-pad || at.0 > max_x+pad || at.1 < min_y-pad || at.1 > max_y+pad {
            continue;
        }

        let was_closed=stroke.points.len()>2
            && stroke.points.first().zip(stroke.points.last())
                .is_some_and(|(a,b)|(a.x-b.x).hypot(a.y-b.y)<=0.001);

        // Vector strokes and bucket/contour fills often have sparse boundary
        // nodes. Refine only when a nearby segment is genuinely too long.
        // The previous implementation rebuilt every stroke on every pointer
        // event, which became expensive in long drawing sessions.
        if stroke.points.len() >= 2 && stroke.points.len() < 6_000 {
            let step=(radius*0.22).clamp(4.0,28.0);
            let needs_refine=stroke.points.windows(2).any(|pair|{
                let a=&pair[0]; let b=&pair[1];
                let length=(b.x-a.x).hypot(b.y-a.y);
                if length<=step*1.2{return false;}
                let mx=(a.x+b.x)*0.5; let my=(a.y+b.y)*0.5;
                (mx-at.0).hypot(my-at.1)<=radius+length*0.5
            });
            if needs_refine {
                let mut dense=Vec::with_capacity((stroke.points.len()*2).min(15_000));
                dense.push(stroke.points[0].clone());
                for pair in stroke.points.windows(2) {
                    let a=&pair[0];let b=&pair[1];
                    let length=(b.x-a.x).hypot(b.y-a.y);
                    let mx=(a.x+b.x)*0.5;let my=(a.y+b.y)*0.5;
                    let close=(mx-at.0).hypot(my-at.1)<=radius+length*0.5;
                    let subdivisions=if close {
                        (length/step).ceil().clamp(1.0,12.0) as usize
                    } else {1};
                    for k in 1..=subdivisions {
                        let t=k as f32/subdivisions as f32;
                        dense.push(StrokePoint{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t,
                            pressure:a.pressure+(b.pressure-a.pressure)*t});
                    }
                    if dense.len()>15_000{break;}
                }
                if dense.len()<=15_000 {stroke.points=dense;}
            }
        }

        for point in &mut stroke.points {
            let d=(point.x-at.0).hypot(point.y-at.1);
            if d>=radius{continue;}
            let t=(1.0-d/radius).clamp(0.0,1.0);
            let w=t*t*(3.0-2.0*t)*strength;
            point.x+=motion.0*w;
            point.y+=motion.1*w;
            changed=true;
        }

        // Filled bucket faces and closed vector contours must remain closed
        // after sculpting or a tiny seam can appear in fill/outline rendering.
        if was_closed && stroke.points.len()>2 {
            let first=stroke.points[0].clone();
            let last=stroke.points.len()-1;
            stroke.points[last]=first;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{BrushTipShape,KritaBrushPreset};
    #[test]
    fn sculpt_deforms_filled_closed_contours_and_keeps_the_seam_closed(){
        let mut s=Stroke{points:vec![
            StrokePoint{x:0.,y:0.,pressure:1.},StrokePoint{x:100.,y:0.,pressure:1.},
            StrokePoint{x:100.,y:100.,pressure:1.},StrokePoint{x:0.,y:100.,pressure:1.},
            StrokePoint{x:0.,y:0.,pressure:1.}],
            color:[0,0,0,0],fill_color:Some([20,40,80,255]),base_width:2.,is_vector:true,
            brush_preset:KritaBrushPreset::GPenInk,opacity:1.0,
            brush_texture_id:None,tip_shape:BrushTipShape::Round};
        assert!(sculpt(std::slice::from_mut(&mut s),(50.,0.),(0.,20.),35.,1.0));
        assert_eq!(s.points.first(),s.points.last());
        assert!(s.points.iter().any(|p|p.y>0.));
    }

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
