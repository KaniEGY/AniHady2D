//! Lightweight articulated pose reference for Drawing mode.
//! It is a non-exported guide, intended as a fast posing aid rather than a 3D renderer.

use eframe::egui;
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct PoseGuide {
    pub visible:bool,
    pub x:f32,
    pub y:f32,
    pub scale:f32,
    pub opacity:f32,
    pub torso_tilt_deg:f32,
    pub shoulder_deg:f32,
    pub hip_deg:f32,
    pub left_arm_deg:f32,
    pub right_arm_deg:f32,
    pub left_forearm_deg:f32,
    pub right_forearm_deg:f32,
    pub left_leg_deg:f32,
    pub right_leg_deg:f32,
    pub left_shin_deg:f32,
    pub right_shin_deg:f32,
}
impl Default for PoseGuide{
    fn default()->Self{Self{
        visible:false,x:0.5,y:0.55,scale:1.0,opacity:0.45,torso_tilt_deg:0.0,
        shoulder_deg:0.0,hip_deg:0.0,left_arm_deg:125.0,right_arm_deg:55.0,
        left_forearm_deg:105.0,right_forearm_deg:75.0,left_leg_deg:110.0,
        right_leg_deg:70.0,left_shin_deg:95.0,right_shin_deg:85.0,
    }}
}
fn dir(angle:f32,len:f32)->egui::Vec2{
    let a=angle.to_radians();
    egui::vec2(a.cos()*len,a.sin()*len)
}
pub fn draw(
    painter:&egui::Painter,guide:&PoseGuide,canvas:(f32,f32),
    to_screen:impl Fn(egui::Pos2)->egui::Pos2
){
    if !guide.visible{return}
    let base=canvas.0.min(canvas.1)*0.22*guide.scale.clamp(0.25,3.0);
    let root=egui::pos2(canvas.0*guide.x.clamp(0.0,1.0),canvas.1*guide.y.clamp(0.0,1.0));
    let alpha=(guide.opacity.clamp(0.05,1.0)*255.0) as u8;
    let color=egui::Color32::from_rgba_unmultiplied(70,190,255,alpha);
    let stroke=egui::Stroke::new(2.0,color);
    let torso_dir=dir(-90.0+guide.torso_tilt_deg,base*0.55);
    let chest=root+torso_dir;
    let head=chest+dir(-90.0+guide.torso_tilt_deg,base*0.22);
    let shoulder_half=dir(guide.shoulder_deg,base*0.22);
    let hip_half=dir(guide.hip_deg,base*0.15);
    let ls=chest-shoulder_half; let rs=chest+shoulder_half;
    let lh=root-hip_half; let rh=root+hip_half;
    let le=ls+dir(guide.left_arm_deg,base*0.34);
    let re=rs+dir(guide.right_arm_deg,base*0.34);
    let lw=le+dir(guide.left_forearm_deg,base*0.31);
    let rw=re+dir(guide.right_forearm_deg,base*0.31);
    let lk=lh+dir(guide.left_leg_deg,base*0.44);
    let rk=rh+dir(guide.right_leg_deg,base*0.44);
    let la=lk+dir(guide.left_shin_deg,base*0.42);
    let ra=rk+dir(guide.right_shin_deg,base*0.42);

    let line=|a:egui::Pos2,b:egui::Pos2|painter.line_segment([to_screen(a),to_screen(b)],stroke);
    line(root,chest); line(ls,rs); line(lh,rh);
    line(ls,le);line(le,lw);line(rs,re);line(re,rw);
    line(lh,lk);line(lk,la);line(rh,rk);line(rk,ra);
    painter.circle_stroke(to_screen(head),base*0.105,stroke);
    for p in [root,chest,ls,rs,lh,rh,le,re,lw,rw,lk,rk,la,ra] {
        painter.circle_filled(to_screen(p),3.2,color);
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn default_pose_is_bounded(){
        let p=PoseGuide::default();
        assert!((0.0..=1.0).contains(&p.x));
        assert!((0.0..=1.0).contains(&p.y));
        assert!(p.scale>0.0);
    }
}
