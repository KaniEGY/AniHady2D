//! Local Drawing-mode assets: quick access commands, palettes and Auto Actions.
//! This is intentionally local/offline and never uploads user content.

use serde::{Deserialize,Serialize};
use crate::project::Stroke;
use std::path::PathBuf;

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct PaletteAsset {
    pub name:String,
    pub colors:Vec<[u8;4]>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct AutoAction {
    pub name:String,
    pub commands:Vec<String>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct StrokeAsset {
    pub name:String,
    pub strokes:Vec<Stroke>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct DrawingAssets {
    #[serde(default="default_quick_access")]
    pub quick_access:Vec<String>,
    #[serde(default="default_palettes")]
    pub palettes:Vec<PaletteAsset>,
    #[serde(default="default_actions")]
    pub auto_actions:Vec<AutoAction>,
    #[serde(default)]
    pub stroke_assets:Vec<StrokeAsset>,
}
fn default_quick_access()->Vec<String>{
    // The old left-dock text strip duplicated actions and consumed a large
    // amount of drawing space. Commands remain available from Ctrl+K, while
    // the dock now exposes the interactive Pose Rig as one visual control.
    Vec::new()
}
fn default_palettes()->Vec<PaletteAsset>{
    vec![
        PaletteAsset{name:"Ink".into(),colors:vec![
            [0,0,0,255],[36,36,40,255],[90,90,96,255],[255,255,255,255]
        ]},
        PaletteAsset{name:"Animation".into(),colors:vec![
            [239,68,68,255],[34,197,94,255],[56,189,248,255],
            [234,179,8,255],[139,92,246,255],[236,72,153,255]
        ]},
    ]
}
fn default_actions()->Vec<AutoAction>{
    vec![
        AutoAction{name:"Clean Vector".into(),commands:vec![
            "vector_smooth".into(),"vector_simplify".into()
        ]},
        AutoAction{name:"Ink Stronger".into(),commands:vec![
            "vector_width_up".into(),"vector_smooth".into()
        ]},
        AutoAction{name:"Perspective Ink".into(),commands:vec![
            "toggle_perspective_snap".into(),"tool_vector_pen".into()
        ]},
    ]
}
impl Default for DrawingAssets {
    fn default()->Self{Self{
        quick_access:default_quick_access(),
        palettes:default_palettes(),
        auto_actions:default_actions(),
        stroke_assets:Vec::new(),
    }}
}
fn path()->Option<PathBuf>{
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p|p.join("StickStudio").join("drawing_assets.json"))
}
impl DrawingAssets {
    pub fn load()->Self{
        let Some(path)=path() else{return Self::default()};
        std::fs::read_to_string(path).ok()
            .and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self)->Result<(),String>{
        let path=path().ok_or("LOCALAPPDATA is unavailable")?;
        if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
        let json=serde_json::to_string_pretty(self).map_err(|e|e.to_string())?;
        std::fs::write(path,json).map_err(|e|e.to_string())
    }
    pub fn pin(&mut self,id:&str){
        if !self.quick_access.iter().any(|x|x==id){self.quick_access.push(id.to_owned());}
    }
    pub fn unpin(&mut self,id:&str){self.quick_access.retain(|x|x!=id);}
    pub fn add_color_to_palette(&mut self,palette:usize,color:[u8;4])->bool{
        let Some(p)=self.palettes.get_mut(palette) else{return false};
        if p.colors.contains(&color){return false}
        if p.colors.len()>=64{return false}
        p.colors.push(color);true
    }
    pub fn push_recorded_action(&mut self,name:String,commands:Vec<String>)->bool{
        let name=name.trim();
        if name.is_empty()||commands.is_empty(){return false}
        self.auto_actions.push(AutoAction{name:name.to_owned(),commands});
        true
    }
    pub fn push_stroke_asset(&mut self,name:String,strokes:Vec<Stroke>)->bool{
        let name=name.trim();
        if name.is_empty()||strokes.is_empty()||strokes.len()>2_000{return false}
        self.stroke_assets.push(StrokeAsset{name:name.to_owned(),strokes});
        true
    }
}

pub fn command_label(id:&str)->(&'static str,&'static str){
    match id {
        "undo"=>("Undo","تراجع"),
        "redo"=>("Redo","إعادة"),
        "tool_brush"=>("Brush","فرشاة"),
        "tool_vector_pen"=>("Vector Ink","قلم فيكتور"),
        "tool_vector_eraser"=>("Vector Eraser","ممحاة فيكتور"),
        "tool_bucket"=>("Fill","تعبئة"),
        "tool_puppet"=>("Puppet Warp","تحريك شبكي"),
        "vector_smooth"=>("Smooth selected vectors","تنعيم الفيكتور المحدد"),
        "vector_simplify"=>("Simplify selected vectors","تقليل نقاط الفيكتور"),
        "vector_width_up"=>("Vector width +10%","زيادة سمك الفيكتور 10%"),
        "vector_width_down"=>("Vector width -10%","تقليل سمك الفيكتور 10%"),
        "toggle_perspective_snap"=>("Perspective Snap","تثبيت على المنظور"),
        "toggle_smart_shape"=>("Smart Shape","التعرف الذكي على الأشكال"),
        "fill_selection"=>("Fill selection","تعبئة التحديد"),
        "flip_h"=>("Mirror canvas","عكس الكانفا"),
        "flip_v"=>("Flip canvas vertically","قلب الكانفا"),
        _=>("Drawing command","أمر رسم"),
    }
}
pub const COMMANDS:[&str;16]=[
    "undo","redo","tool_brush","tool_vector_pen","tool_vector_eraser","tool_bucket",
    "tool_puppet","vector_smooth","vector_simplify","vector_width_up","vector_width_down",
    "toggle_perspective_snap","toggle_smart_shape","fill_selection","flip_h","flip_v",
];

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn defaults_keep_quick_access_clean(){
        let a=DrawingAssets::default();
        assert!(a.quick_access.is_empty());
        assert!(!a.auto_actions.is_empty());
    }
    #[test] fn pin_is_unique(){
        let mut a=DrawingAssets::default();
        let before=a.quick_access.len();
        a.pin("undo");a.pin("undo");
        assert_eq!(a.quick_access.len(),before);
    }
}
