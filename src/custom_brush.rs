//! Persistent imported raster brush library for Stick.
//!
//! Imported vendor brushes are normalized into Stick's raster stamp model.
//! The original file is kept beside the normalized tip(s) so future versions
//! can improve the mapping without making users re-import their libraries.

use crate::brush_engine::visit_pressure_dabs_spaced;
use crate::project::Stroke;
use base64::Engine;
use eframe::egui;
use flate2::read::ZlibDecoder;
use image::{DynamicImage, GrayImage, ImageFormat, RgbaImage};
use rusqlite::{types::Value, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

const MAX_TIPS: usize = 32;
const MAX_IMPORT_BRUSHES: usize = 128;
const MAX_TIP_SIDE: u32 = 512;

fn default_tip_scale() -> f32 { 1.0 }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomBrushMeta {
    pub id: String,
    pub name: String,
    pub source_format: String,
    pub source_path: String,
    pub icon_path: String,
    pub tip_paths: Vec<String>,
    pub spacing: f32,
    pub opacity: f32,
    pub flow: f32,
    pub size_pressure: f32,
    pub opacity_pressure: f32,
    pub angle_deg: f32,
    pub follow_stroke: bool,
    pub scatter: f32,
    pub random_tip: bool,
    #[serde(default = "default_tip_scale")]
    pub tip_scale: f32,
    #[serde(default)]
    pub compatibility_note: String,
}

#[derive(Clone, Debug)]
pub struct ImportedBrushDraft {
    pub suggested_name: String,
    pub source_format: String,
    pub source_path: PathBuf,
    pub tips: Vec<TipMask>,
    pub icon: Option<IconImage>,
    pub spacing: f32,
    pub opacity: f32,
    pub flow: f32,
    pub size_pressure: f32,
    pub opacity_pressure: f32,
    pub angle_deg: f32,
    pub follow_stroke: bool,
    pub scatter: f32,
    pub random_tip: bool,
    pub tip_scale: f32,
    pub compatibility_note: String,
}

#[derive(Clone, Debug)]
pub struct TipMask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone)]
struct RuntimeBrush {
    meta: CustomBrushMeta,
    tips: Vec<TipMask>,
    icon: Option<IconImage>,
}

static RUNTIME: OnceLock<RwLock<HashMap<String, RuntimeBrush>>> = OnceLock::new();

fn runtime() -> &'static RwLock<HashMap<String, RuntimeBrush>> {
    RUNTIME.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn accepted_extensions() -> &'static [&'static str] {
    &[
        "kpp", "bundle", "myb", "gbr", "gih", "abr",
        "sut", "sutg", "tos", "blend",
        "png", "jpg", "jpeg", "webp", "bmp",
    ]
}

pub fn brush_root() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(base).join("StickStudio").join("brushes");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("brushes");
        }
    }
    PathBuf::from("brushes")
}

fn library_path() -> PathBuf {
    brush_root().join("custom_brushes.json")
}

pub fn load_saved_library() -> Vec<CustomBrushMeta> {
    let path = library_path();
    let list = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<CustomBrushMeta>>(&s).ok())
        .unwrap_or_default();
    refresh_runtime(&list);
    list
}

pub fn save_library(list: &[CustomBrushMeta]) -> Result<(), String> {
    let root = brush_root();
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let data = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    let dst = library_path();
    let tmp = dst.with_extension("json.tmp");
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    if dst.exists() {
        let _ = fs::remove_file(&dst);
    }
    fs::rename(tmp, dst).map_err(|e| e.to_string())?;
    refresh_runtime(list);
    Ok(())
}

pub fn delete_brush(id: &str) {
    let _ = fs::remove_dir_all(brush_root().join(id));
    if let Ok(mut map) = runtime().write() {
        map.remove(id);
    }
}

pub fn install_draft(
    draft: &ImportedBrushDraft,
    display_name: &str,
    custom_icon: Option<&Path>,
) -> Result<CustomBrushMeta, String> {
    let clean_name = display_name.trim();
    if clean_name.is_empty() {
        return Err("Brush name cannot be empty".into());
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    let mut h = Sha256::new();
    h.update(draft.source_path.to_string_lossy().as_bytes());
    h.update(clean_name.as_bytes());
    h.update(now.to_le_bytes());
    let id = format!("{:x}", h.finalize());
    let dir = brush_root().join(&id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let ext = draft.source_path.extension().and_then(|s| s.to_str()).unwrap_or("brush");
    let source_dst = dir.join(format!("source.{ext}"));
    fs::copy(&draft.source_path, &source_dst).map_err(|e| e.to_string())?;

    let mut tip_paths = Vec::new();
    let tips = if draft.tips.is_empty() {
        vec![round_tip(96, 0.9)]
    } else {
        draft.tips.clone()
    };
    for (i, tip) in tips.iter().take(MAX_TIPS).enumerate() {
        let path = dir.join(format!("tip_{i}.png"));
        save_tip_png(tip, &path)?;
        tip_paths.push(path.to_string_lossy().to_string());
    }

    let icon_path = dir.join("icon.png");
    let icon = if let Some(path) = custom_icon {
        image::open(path).ok().map(icon_from_image)
    } else {
        draft.icon.clone()
    };
    if let Some(icon) = icon {
        save_icon_png(&icon, &icon_path)?;
    } else {
        save_tip_preview_png(&tips[0], &icon_path)?;
    }

    let meta = CustomBrushMeta {
        id,
        name: clean_name.to_owned(),
        source_format: draft.source_format.clone(),
        source_path: source_dst.to_string_lossy().to_string(),
        icon_path: icon_path.to_string_lossy().to_string(),
        tip_paths,
        spacing: draft.spacing.clamp(0.02, 2.0),
        opacity: draft.opacity.clamp(0.05, 1.0),
        flow: draft.flow.clamp(0.02, 1.0),
        size_pressure: draft.size_pressure.clamp(0.0, 1.0),
        opacity_pressure: draft.opacity_pressure.clamp(0.0, 1.0),
        angle_deg: draft.angle_deg,
        follow_stroke: draft.follow_stroke,
        scatter: draft.scatter.clamp(0.0, 3.0),
        random_tip: draft.random_tip,
        tip_scale: draft.tip_scale.clamp(0.05, 8.0),
        compatibility_note: draft.compatibility_note.clone(),
    };
    register_meta(&meta);
    Ok(meta)
}

fn register_meta(meta: &CustomBrushMeta) {
    let tips: Vec<_> = meta.tip_paths.iter().filter_map(|p| load_tip(Path::new(p))).collect();
    let icon = load_icon(Path::new(&meta.icon_path));
    if let Ok(mut map) = runtime().write() {
        map.insert(meta.id.clone(), RuntimeBrush { meta: meta.clone(), tips, icon });
    }
}

fn refresh_runtime(list: &[CustomBrushMeta]) {
    if let Ok(mut map) = runtime().write() {
        map.clear();
    }
    for meta in list {
        register_meta(meta);
    }
}

pub fn stroke_custom_id(stroke: &Stroke) -> Option<&str> {
    stroke.brush_texture_id.as_deref()?.strip_prefix("custom:")
}

pub fn draw_icon(painter: &egui::Painter, rect: egui::Rect, id: &str) -> bool {
    let Ok(map) = runtime().read() else { return false };
    let Some(brush) = map.get(id) else { return false };
    let Some(icon) = brush.icon.as_ref() else { return false };
    let ctx = painter.ctx();
    let key = egui::Id::new(("stick_custom_brush_icon", id.to_owned()));
    let cached = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key));
    let tex = cached.unwrap_or_else(|| {
        let pixels = icon.rgba.chunks_exact(4)
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0],p[1],p[2],p[3]))
            .collect();
        let handle = ctx.load_texture(
            format!("custom brush icon {id}"),
            egui::ColorImage { size:[icon.width as usize,icon.height as usize], pixels },
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|d| d.insert_temp(key, handle.clone()));
        handle
    });
    painter.image(tex.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0,1.0)), egui::Color32::WHITE);
    true
}

pub fn preview(
    painter: &egui::Painter,
    stroke: &Stroke,
    rgba: [u8;4],
    to_screen: impl Fn(f32,f32)->egui::Pos2,
) -> bool {
    let Some(id)=stroke_custom_id(stroke) else { return false };
    let Ok(map)=runtime().read() else { return false };
    let Some(brush)=map.get(id) else { return false };
    if brush.tips.is_empty() { return false; }

    let ctx=painter.ctx();
    let mut meshes=Vec::with_capacity(brush.tips.len());
    for (i,tip) in brush.tips.iter().enumerate() {
        let key=egui::Id::new(("stick_custom_brush_tip",id.to_owned(),i));
        let cached=ctx.data(|d|d.get_temp::<egui::TextureHandle>(key));
        let tex=cached.unwrap_or_else(||{
            let pixels=tip.data.iter()
                .map(|&a|egui::Color32::from_rgba_unmultiplied(255,255,255,a))
                .collect();
            let handle=ctx.load_texture(
                format!("custom brush tip {id}:{i}"),
                egui::ColorImage{size:[tip.width as usize,tip.height as usize],pixels},
                egui::TextureOptions::LINEAR,
            );
            ctx.data_mut(|d|d.insert_temp(key,handle.clone()));
            handle
        });
        meshes.push(egui::Mesh::with_texture(tex.id()));
    }

    emit_stamps(stroke,brush,|stamp|{
        let mesh=&mut meshes[stamp.tip_index];
        let start=mesh.vertices.len() as u32;
        let alpha=(rgba[3] as f32*stamp.flow).round().clamp(0.0,255.0) as u8;
        let color=egui::Color32::from_rgba_unmultiplied(rgba[0],rgba[1],rgba[2],alpha);
        for (u,v) in [(0.,0.),(1.,0.),(1.,1.),(0.,1.)] {
            let [x,y]=stamp.corner(u,v);
            mesh.vertices.push(egui::epaint::Vertex{
                pos:to_screen(x,y),uv:egui::pos2(u,v),color,
            });
        }
        mesh.indices.extend([start,start+1,start+2,start,start+2,start+3]);
    });
    drop(map);
    for mesh in meshes {
        if !mesh.indices.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }
    true
}

pub fn export(
    target:&mut Pixmap,
    stroke:&Stroke,
    alpha:u8,
    transform:Transform,
)->bool {
    let Some(id)=stroke_custom_id(stroke) else {return false};
    let Ok(map)=runtime().read() else {return false};
    let Some(brush)=map.get(id) else {return false};
    if brush.tips.is_empty(){return false;}

    let mut pixmaps=Vec::with_capacity(brush.tips.len());
    for tip in &brush.tips {
        let mut pm=Pixmap::new(tip.width,tip.height).unwrap();
        for (px,&a) in pm.data_mut().chunks_exact_mut(4).zip(&tip.data) {
            px[0]=((stroke.color[0] as u16*a as u16+127)/255) as u8;
            px[1]=((stroke.color[1] as u16*a as u16+127)/255) as u8;
            px[2]=((stroke.color[2] as u16*a as u16+127)/255) as u8;
            px[3]=a;
        }
        pixmaps.push(pm);
    }
    emit_stamps(stroke,brush,|stamp|{
        let tip=&pixmaps[stamp.tip_index];
        let origin=stamp.corner(0.,0.);
        let kx=2.0/tip.width() as f32;
        let ky=2.0/tip.height() as f32;
        let position=Transform::from_row(
            stamp.along[0]*kx,stamp.along[1]*kx,
            stamp.across[0]*ky,stamp.across[1]*ky,
            origin[0],origin[1],
        ).post_concat(transform);
        let paint=PixmapPaint{
            opacity:(alpha as f32*stamp.flow).round()/255.0,
            quality:FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        target.draw_pixmap(0,0,tip.as_ref(),&paint,position,None);
    });
    true
}

#[derive(Clone,Copy)]
struct Stamp {
    center:[f32;2],
    along:[f32;2],
    across:[f32;2],
    flow:f32,
    tip_index:usize,
}
impl Stamp {
    fn corner(self,u:f32,v:f32)->[f32;2]{
        let (u,v)=(2.0*u-1.0,2.0*v-1.0);
        [
            self.center[0]+self.along[0]*u+self.across[0]*v,
            self.center[1]+self.along[1]*u+self.across[1]*v,
        ]
    }
}

fn emit_stamps(stroke:&Stroke,brush:&RuntimeBrush,mut emit:impl FnMut(Stamp)){
    // Vendor spacing is stored as a fraction of the brush DIAMETER while the
    // shared sampler expects a fraction of RADIUS. Preserve that distinction:
    // Krita spacing=0.1 must become 0.2 radius, not 2.0.
    let ratio=(brush.meta.spacing*2.0*brush.meta.tip_scale).clamp(0.02,2.0);
    let mut ordinal=0u32;
    visit_pressure_dabs_spaced(&stroke.points,stroke.base_width,ratio,|dab,[dx,dy]|{
        let p=(2.0*dab.radius/stroke.base_width.max(0.001)).clamp(0.0,1.5);
        let size_factor=(1.0-brush.meta.size_pressure)+brush.meta.size_pressure*p;
        let r=(stroke.base_width*0.5*size_factor*brush.meta.tip_scale).max(0.25);
        let flow=brush.meta.flow*((1.0-brush.meta.opacity_pressure)+brush.meta.opacity_pressure*p)
            .clamp(0.02,1.5);
        let jitter=unit_noise(ordinal.wrapping_add(hash_id(&brush.meta.id)));
        let mut cx=dab.x;
        let mut cy=dab.y;
        if brush.meta.scatter>0.0 {
            let offset=(jitter-0.5)*2.0*brush.meta.scatter*stroke.base_width;
            cx+=-dy*offset;
            cy+=dx*offset;
        }
        let base_angle=brush.meta.angle_deg.to_radians();
        let angle=if brush.meta.follow_stroke {dy.atan2(dx)+base_angle}else{base_angle};
        let (sin,cos)=angle.sin_cos();
        let idx=if brush.meta.random_tip {
            (unit_noise(ordinal.wrapping_mul(1103515245).wrapping_add(12345))*brush.tips.len() as f32)
                .floor() as usize % brush.tips.len()
        } else {
            ordinal as usize%brush.tips.len()
        };
        let tip=&brush.tips[idx];
        let max_side=tip.width.max(tip.height).max(1) as f32;
        let hw=r*tip.width as f32/max_side;
        let hh=r*tip.height as f32/max_side;
        emit(Stamp{
            center:[cx,cy],
            along:[cos*hw,sin*hw],
            across:[-sin*hh,cos*hh],
            flow,
            tip_index:idx,
        });
        ordinal=ordinal.wrapping_add(1);
    });
}

fn hash_id(id:&str)->u32 {
    id.bytes().fold(2166136261u32,|h,b|(h^b as u32).wrapping_mul(16777619))
}
fn unit_noise(mut x:u32)->f32{
    x^=x>>16;x=x.wrapping_mul(0x7feb352d);x^=x>>15;x=x.wrapping_mul(0x846ca68b);
    ((x^(x>>16))>>8) as f32/16_777_216.0
}

// -------------------------------------------------------------------------------------------------
// Importers
// -------------------------------------------------------------------------------------------------

pub fn import_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let ext=path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png"|"jpg"|"jpeg"|"webp"|"bmp"=>import_image(path),
        "gbr"=>import_gbr_file(path),
        "gih"=>import_gih_file(path),
        "abr"=>import_abr_file(path),
        "kpp"=>import_kpp_file(path),
        "bundle"=>import_krita_bundle(path),
        "myb"=>import_myb_file(path),
        "sut"=>import_sut_file(path),
        "sutg"=>import_sutg_file(path),
        "tos"=>Ok(vec![baseline_draft(path,"Clip Studio / ComicStudio .tos",
            "Legacy .tos settings are imported as a safe Stick raster baseline; proprietary dynamics are not exact.")]),
        "blend"=>import_blend_file(path),
        _=>Err(format!("Unsupported brush file: .{ext}")),
    }
}

fn baseline_draft(path:&Path,format:&str,note:&str)->ImportedBrushDraft{
    ImportedBrushDraft{
        suggested_name:path.file_stem().and_then(|s|s.to_str()).unwrap_or("Imported Brush").to_owned(),
        source_format:format.to_owned(),source_path:path.to_path_buf(),
        tips:vec![round_tip(96,0.9)],icon:None,spacing:0.10,opacity:1.0,flow:1.0,
        size_pressure:1.0,opacity_pressure:0.0,angle_deg:0.0,follow_stroke:false,
        scatter:0.0,random_tip:false,tip_scale:1.0,compatibility_note:note.to_owned(),
    }
}

fn import_image(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let img=image::open(path).map_err(|e|e.to_string())?;
    let tip=tip_from_image(&img);
    Ok(vec![ImportedBrushDraft{
        suggested_name:path.file_stem().and_then(|s|s.to_str()).unwrap_or("Image Brush").to_owned(),
        source_format:"Image brush tip".into(),source_path:path.to_path_buf(),
        tips:vec![tip],icon:Some(icon_from_image(img)),spacing:0.10,opacity:1.0,flow:1.0,
        size_pressure:1.0,opacity_pressure:0.0,angle_deg:0.0,follow_stroke:true,
        scatter:0.0,random_tip:false,tip_scale:1.0,compatibility_note:String::new(),
    }])
}

fn import_abr_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    use brushkit_preview::{preview_abr,PreviewOptions,TipPreview};
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    let set=preview_abr(&bytes,PreviewOptions{max_cell:MAX_TIP_SIDE})
        .map_err(|e|format!("ABR: {e}"))?;
    let mut out=Vec::new();
    for entry in set.entries.into_iter().take(MAX_IMPORT_BRUSHES) {
        if let TipPreview::Available(bitmap)=entry.tip {
            out.push(ImportedBrushDraft{
                suggested_name:if entry.name.trim().is_empty(){format!("ABR Brush {}",entry.index+1)}else{entry.name},
                source_format:"Photoshop / Krita / Clip Studio .abr".into(),
                source_path:path.to_path_buf(),
                tips:vec![TipMask{width:bitmap.width,height:bitmap.height,data:bitmap.data}],
                icon:None,spacing:0.10,opacity:1.0,flow:1.0,size_pressure:1.0,opacity_pressure:0.0,
                angle_deg:0.0,follow_stroke:false,scatter:0.0,random_tip:false,tip_scale:1.0,
                compatibility_note:"ABR tip shape is preserved; vendor-only brush dynamics are mapped to Stick defaults.".into(),
            });
        }
    }
    if out.is_empty(){return Err("ABR contains no readable brush tips".into())}
    Ok(out)
}

fn import_gbr_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let data=fs::read(path).map_err(|e|e.to_string())?;
    let (name,tip,spacing,_)=parse_gbr(&data,0)?;
    Ok(vec![ImportedBrushDraft{
        suggested_name:if name.is_empty(){path.file_stem().and_then(|s|s.to_str()).unwrap_or("GBR Brush").to_owned()}else{name},
        source_format:"Krita / GIMP .gbr".into(),source_path:path.to_path_buf(),
        tips:vec![tip],icon:None,spacing:(spacing as f32/100.0).clamp(0.02,2.0),
        opacity:1.0,flow:1.0,size_pressure:1.0,opacity_pressure:0.0,angle_deg:0.0,
        follow_stroke:false,scatter:0.0,random_tip:false,tip_scale:1.0,compatibility_note:String::new(),
    }])
}

fn import_gih_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let data=fs::read(path).map_err(|e|e.to_string())?;
    parse_gih_bytes(&data,path)
}

fn parse_gih_bytes(data:&[u8],path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let nl1=data.iter().position(|&b|b==b'\n').ok_or("GIH header missing")?;
    let nl2=data[nl1+1..].iter().position(|&b|b==b'\n').map(|i|i+nl1+1).ok_or("GIH header missing")?;
    let name=String::from_utf8_lossy(&data[..nl1]).trim().to_owned();
    let line=String::from_utf8_lossy(&data[nl1+1..nl2]);
    let count=line.split_whitespace().next().and_then(|v|v.parse::<usize>().ok()).ok_or("GIH cell count missing")?;
    let random=line.contains("sel0:random")||line.contains("sel0:pressure")||line.contains("sel0:velocity");
    let mut pos=nl2+1;
    let mut tips=Vec::new();
    let mut spacing=10u32;
    for _ in 0..count.min(MAX_TIPS) {
        let (_,tip,sp,next)=parse_gbr(&data[pos..],0)?;
        spacing=sp;
        tips.push(tip);
        pos=pos.saturating_add(next);
        if pos>=data.len(){break}
    }
    if tips.is_empty(){return Err("GIH contains no readable tips".into())}
    Ok(vec![ImportedBrushDraft{
        suggested_name:if name.is_empty(){path.file_stem().and_then(|s|s.to_str()).unwrap_or("GIH Brush").to_owned()}else{name},
        source_format:"Krita / GIMP .gih".into(),source_path:path.to_path_buf(),tips,icon:None,
        spacing:(spacing as f32/100.0).clamp(0.02,2.0),opacity:1.0,flow:1.0,size_pressure:1.0,
        opacity_pressure:0.0,angle_deg:0.0,follow_stroke:false,scatter:0.0,random_tip:random,tip_scale:1.0,
        compatibility_note:"Multi-tip GIH order is preserved as sequential/random in Stick.".into(),
    }])
}

fn parse_gbr(data:&[u8],offset:usize)->Result<(String,TipMask,u32,usize),String>{
    let d=data.get(offset..).ok_or("GBR offset out of range")?;
    if d.len()<20{return Err("GBR header truncated".into())}
    let be=|i:usize|u32::from_be_bytes(d[i..i+4].try_into().unwrap());
    let header=be(0) as usize;
    let version=be(4);
    let w=be(8);let h=be(12);let nbytes=be(16);
    if w==0||h==0||w>8192||h>8192{return Err("GBR dimensions are unsafe".into())}
    let (name_start,spacing)=if version==1 {
        (20,25)
    } else if version==2||version==3 {
        if d.len()<28||be(20)!=u32::from_be_bytes(*b"GIMP"){return Err("GBR magic missing".into())}
        (28,be(24))
    } else {return Err(format!("Unsupported GBR version {version}"))};
    if header<name_start||header>d.len(){return Err("GBR header size invalid".into())}
    let name=String::from_utf8_lossy(&d[name_start..header]).trim_end_matches('\0').to_string();
    let pixels=(w as usize).checked_mul(h as usize).ok_or("GBR too large")?;
    let need=match nbytes {1=>pixels,4=>pixels.checked_mul(4).ok_or("GBR too large")?,_=>return Err(format!("GBR depth {nbytes} is unsupported"))};
    if header+need>d.len(){return Err("GBR pixels truncated".into())}
    let raw=&d[header..header+need];
    let mask=if nbytes==1 {raw.to_vec()} else {raw.chunks_exact(4).map(|p|p[3]).collect()};
    Ok((name,downsample_tip(TipMask{width:w,height:h,data:mask}),spacing,header+need))
}

fn import_kpp_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let data=fs::read(path).map_err(|e|e.to_string())?;
    Ok(vec![parse_kpp_bytes(&data,path)?])
}

fn read_krita_resource_near(source_path:&Path,filename:&str)->Option<Vec<u8>>{
    let file=Path::new(filename).file_name()?.to_owned();
    let parent=source_path.parent().unwrap_or(Path::new("."));
    let mut candidates=vec![
        parent.join(&file),
        parent.join("brushes").join(&file),
    ];
    if let Some(up)=parent.parent(){
        candidates.push(up.join("brushes").join(&file));
    }
    if let Some(appdata)=std::env::var_os("APPDATA"){
        candidates.push(PathBuf::from(appdata).join("krita").join("brushes").join(&file));
    }
    if let Some(local)=std::env::var_os("LOCALAPPDATA"){
        candidates.push(PathBuf::from(local).join("krita").join("brushes").join(&file));
    }
    candidates.into_iter().find_map(|p|fs::read(p).ok())
}

fn parse_kpp_bytes(data:&[u8],source_path:&Path)->Result<ImportedBrushDraft,String>{
    let img=image::load_from_memory(data).map_err(|e|format!("KPP preview PNG: {e}"))?;
    let icon=Some(icon_from_image(img));
    let text=png_text_chunks(data);
    let xml=text.get("preset").cloned().ok_or("KPP has no preset metadata")?;
    parse_krita_preset_xml(&xml,icon,source_path,|name|read_krita_resource_near(source_path,name))
}

fn parse_krita_preset_xml<F>(
    xml:&str,
    icon:Option<IconImage>,
    source_path:&Path,
    mut resolve_resource:F,
)->Result<ImportedBrushDraft,String>
where F:FnMut(&str)->Option<Vec<u8>>
{
    let doc=roxmltree::Document::parse(xml).map_err(|e|format!("KPP preset XML: {e}"))?;
    let root=doc.descendants().find(|n|n.has_tag_name("Preset")).ok_or("KPP Preset element missing")?;
    let name=root.attribute("name")
        .unwrap_or_else(||source_path.file_stem().and_then(|s|s.to_str()).unwrap_or("Krita Brush"))
        .replace('_'," ");
    let paintop=root.attribute("paintopid").unwrap_or("paintbrush").to_owned();

    let mut params=HashMap::<String,String>::new();
    for n in doc.descendants().filter(|n|n.has_tag_name("param")) {
        let Some(k)=n.attribute("name") else {continue};
        let raw=n.text().unwrap_or("");
        let value=if n.attribute("type")==Some("bytearray") {
            base64::engine::general_purpose::STANDARD
                .decode(raw.split_whitespace().collect::<String>())
                .ok()
                .map(|bytes|String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default()
        } else {
            raw.to_owned()
        };
        params.insert(k.to_owned(),value);
    }

    // Some modern KPPs embed resource data directly in the preset XML.
    let mut tips=Vec::new();
    for r in doc.descendants().filter(|n|n.has_tag_name("resource")).take(MAX_TIPS) {
        let rtype=r.attribute("type").unwrap_or("");
        if !rtype.to_ascii_lowercase().contains("brush") {continue}
        let filename=r.attribute("filename").or_else(||r.attribute("name")).unwrap_or("");
        let raw=r.text().and_then(|t|
            base64::engine::general_purpose::STANDARD
                .decode(t.split_whitespace().collect::<String>()).ok());
        if let Some(raw)=raw {
            tips.extend(tips_from_resource(filename,&raw));
        }
    }

    let mut spacing=0.10_f32;
    let mut angle_deg=0.0_f32;
    let mut auto_hardness=0.90_f32;
    let mut auto_ratio=1.0_f32;
    let mut tip_scale=1.0_f32;
    let mut brush_type="auto_brush".to_owned();
    let mut definition_resource=String::new();

    if let Some(def)=params.get("brush_definition") {
        let cleaned=def.replace("<!DOCTYPE BrushTip>", "");
        if let Ok(bdoc)=roxmltree::Document::parse(cleaned.trim()) {
            if let Some(brush)=bdoc.descendants().find(|n|n.has_tag_name("Brush")) {
                brush_type=brush.attribute("type").unwrap_or("auto_brush").to_ascii_lowercase();
                // IMPORTANT: this is the real brush-tip spacing. SpacingValue is
                // a dynamics multiplier and must never replace it.
                if let Some(v)=brush.attribute("spacing").and_then(|v|v.parse::<f32>().ok()) {
                    spacing=v.clamp(0.005,2.0);
                }
                tip_scale=brush.attribute("scale").and_then(|v|v.parse::<f32>().ok())
                    .unwrap_or(1.0).clamp(0.05,8.0);
                angle_deg=brush.attribute("angle").and_then(|v|v.parse::<f32>().ok())
                    .map(|r|r.to_degrees()).unwrap_or(0.0);
                definition_resource=brush.attribute("filename").unwrap_or("").trim().to_owned();
                if let Some(mask)=brush.descendants().find(|n|n.has_tag_name("MaskGenerator")) {
                    let h=mask.attribute("hfade").and_then(|v|v.parse::<f32>().ok()).unwrap_or(0.9);
                    let v=mask.attribute("vfade").and_then(|v|v.parse::<f32>().ok()).unwrap_or(h);
                    auto_hardness=((h+v)*0.5).clamp(0.02,0.999);
                    auto_ratio=mask.attribute("ratio").and_then(|v|v.parse::<f32>().ok())
                        .unwrap_or(1.0).clamp(0.05,1.0);
                }
            }
        }
    }

    let opacity=find_number_in_params(&params,&["OpacityValue","opacity"])
        .map(|v|if v>1.0{v/100.0}else{v}).unwrap_or(1.0).clamp(0.05,1.0);
    let flow=find_number_in_params(&params,&["FlowValue","flow"])
        .map(|v|if v>1.0{v/100.0}else{v}).unwrap_or(1.0).clamp(0.02,1.0);

    let pressure_size=params.get("PressureSize").is_some_and(|v|is_truthy(v));
    let size_sensor=params.get("SizeSensor").map(String::as_str).unwrap_or("");
    let size_pressure=pressure_size && size_sensor.to_ascii_lowercase().contains("pressure");

    let pressure_opacity=params.get("PressureOpacity").is_some_and(|v|is_truthy(v));
    let opacity_sensor=params.get("OpacitySensor").map(String::as_str).unwrap_or("");
    let opacity_pressure=if pressure_opacity
        && opacity_sensor.to_ascii_lowercase().contains("pressure"){1.0}else{0.0};

    let pressure_rotation=params.get("PressureRotation").is_some_and(|v|is_truthy(v));
    let rotation_sensor=params.get("RotationSensor").map(String::as_str).unwrap_or("").to_ascii_lowercase();
    let follow_stroke=pressure_rotation && rotation_sensor.contains("drawingangle");

    // Krita's own setPaintOpScatter() writes ScatterValue AND toggles
    // PressureScatter based on whether the value is non-zero. Old presets can
    // retain a stale ScatterValue while PressureScatter=false. Applying that
    // stale value was the cause of imported ink strokes wandering into dots.
    let pressure_scatter=params.get("PressureScatter").is_some_and(|v|is_truthy(v));
    let scatter=if pressure_scatter {
        find_number_in_params(&params,&["ScatterValue"]).unwrap_or(0.0).clamp(0.0,3.0)
    } else {
        0.0
    };

    if tips.is_empty() && brush_type!="auto_brush" {
        let required=params.get("requiredBrushFile")
            .map(|s|s.trim()).filter(|s|!s.is_empty())
            .unwrap_or(definition_resource.trim());
        if !required.is_empty() {
            if let Some(raw)=resolve_resource(required) {
                tips.extend(tips_from_resource(required,&raw));
            }
            if tips.is_empty() {
                return Err(format!(
                    "Krita brush '{name}' needs the external tip '{required}'. \
Put that .gih/.gbr beside the .kpp, keep the original 'brushes' folder next to \
'paintoppresets', or import the complete Krita .bundle. Stick will not replace \
a missing real tip with a fake round brush."
                ));
            }
        } else {
            return Err(format!(
                "Krita brush '{name}' uses '{brush_type}' but contains no readable brush tip. \
Stick refused the old round-tip fallback because it changes the brush."
            ));
        }
    }

    if tips.is_empty() {
        // auto_brush is self-contained: rebuild the actual generated mask from
        // the KPP brush_definition instead of substituting a generic hard dot.
        let tip=if auto_ratio<0.96 {
            elliptical_tip(128,auto_hardness,auto_ratio,angle_deg)
        } else {
            round_tip(128,auto_hardness)
        };
        tips.push(tip);
    }

    let note=if brush_type=="auto_brush" {
        format!(
            "Krita {paintop} auto-brush mapped from brush_definition: spacing {:.1}%, pressure size {}, pressure opacity {}, scatter {:.2}.",
            spacing*100.0,size_pressure,opacity_pressure>0.0,scatter
        )
    } else {
        format!(
            "Krita {paintop} {brush_type} imported with its real brush-tip resource; spacing {:.1}% and scale {:.2} preserved.",
            spacing*100.0,tip_scale
        )
    };

    Ok(ImportedBrushDraft{
        suggested_name:name,source_format:"Krita .kpp".into(),source_path:source_path.to_path_buf(),
        tips,icon,spacing,opacity,flow,size_pressure:if size_pressure{1.0}else{0.0},
        opacity_pressure,angle_deg,follow_stroke,scatter,random_tip:false,tip_scale,
        compatibility_note:note,
    })
}

fn import_krita_bundle(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    let cursor=Cursor::new(bytes);
    let mut zip=zip::ZipArchive::new(cursor).map_err(|e|format!("Krita bundle: {e}"))?;

    // Read the bundle once so every .kpp can resolve its sibling brush
    // resources (GBR/GIH/images) from the same archive.
    let mut entries=Vec::<(String,Vec<u8>)>::new();
    for i in 0..zip.len() {
        let mut file=zip.by_index(i).map_err(|e|e.to_string())?;
        if file.is_dir(){continue}
        let name=file.name().replace('\\',"/");
        let mut data=Vec::new();
        file.read_to_end(&mut data).map_err(|e|e.to_string())?;
        entries.push((name,data));
    }

    let mut out=Vec::new();
    for (entry_name,data) in entries.iter()
        .filter(|(name,_)|name.to_ascii_lowercase().ends_with(".kpp"))
        .take(MAX_IMPORT_BRUSHES)
    {
        let pseudo=PathBuf::from(entry_name);
        let parsed=parse_krita_preset_xml(
            &png_text_chunks(data).get("preset").cloned().unwrap_or_default(),
            image::load_from_memory(data).ok().map(icon_from_image),
            &pseudo,
            |wanted|{
                let wanted_name=Path::new(wanted).file_name()
                    .and_then(|s|s.to_str()).unwrap_or(wanted);
                entries.iter().find(|(name,_)|{
                    Path::new(name).file_name().and_then(|s|s.to_str())
                        .is_some_and(|n|n.eq_ignore_ascii_case(wanted_name))
                }).map(|(_,bytes)|bytes.clone())
            }
        );
        if let Ok(mut draft)=parsed {
            draft.source_path=path.to_path_buf();
            draft.source_format="Krita .bundle".into();
            out.push(draft);
        }
    }
    if out.is_empty(){return Err("Krita bundle contains no readable brush presets/resources".into())}
    Ok(out)
}

fn import_myb_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let text=fs::read_to_string(path).map_err(|e|e.to_string())?;
    let v:serde_json::Value=serde_json::from_str(&text).map_err(|e|format!("MyPaint .myb: {e}"))?;
    let settings=v.get("settings").unwrap_or(&serde_json::Value::Null);
    let base=|name:&str,default:f32|settings.get(name).and_then(|x|x.get("base_value")).and_then(|x|x.as_f64()).map(|x|x as f32).unwrap_or(default);
    let opaque=base("opaque",1.0).clamp(0.05,1.0);
    let hardness=base("hardness",0.8).clamp(0.0,1.0);
    let dabs=base("dabs_per_basic_radius",4.0).max(0.5);
    Ok(vec![ImportedBrushDraft{
        suggested_name:path.file_stem().and_then(|s|s.to_str()).unwrap_or("MyPaint Brush").to_owned(),
        source_format:"Krita / MyPaint .myb".into(),source_path:path.to_path_buf(),
        tips:vec![round_tip(96,hardness)],icon:None,spacing:(0.5/dabs).clamp(0.02,0.5),
        opacity:opaque,flow:1.0,size_pressure:1.0,opacity_pressure:0.0,angle_deg:0.0,
        follow_stroke:false,scatter:0.0,random_tip:false,tip_scale:1.0,
        compatibility_note:"MyPaint core size/opacity/hardness/spacing are mapped; advanced input mappings are approximated.".into(),
    }])
}

fn import_sut_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let flags=OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let con=Connection::open_with_flags(path,flags).map_err(|e|format!("Clip Studio .sut: {e}"))?;
    let tables:Vec<String>={
        let mut st=con.prepare("SELECT name FROM sqlite_master WHERE type='table'").map_err(|e|e.to_string())?;
        let rows=st.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        let mut names=Vec::new();
        for row in rows {
            if let Ok(name)=row { names.push(name); }
        }
        names
    };
    if !tables.iter().any(|x|x=="Variant"){return Err("Clip Studio file has no Variant table".into())}
    let variant=first_row_map(&con,"Variant")?;
    let name=if tables.iter().any(|x|x=="Node"){
        con.query_row("SELECT NodeName FROM Node WHERE NodeName IS NOT NULL AND NodeName<>'' LIMIT 1",[],|r|r.get::<_,String>(0)).ok()
    }else{None}.unwrap_or_else(||path.file_stem().and_then(|s|s.to_str()).unwrap_or("Clip Studio Brush").to_owned());

    let mut tips=Vec::new();
    let mut icon=None;
    if tables.iter().any(|x|x=="MaterialFile") {
        if let Ok(mut st)=con.prepare("SELECT FileData FROM MaterialFile WHERE FileData IS NOT NULL") {
            if let Ok(rows)=st.query_map([],|r|r.get::<_,Vec<u8>>(0)) {
                for row in rows.filter_map(Result::ok).take(MAX_TIPS) {
                    if let Some(png)=find_first_png(&row) {
                        if let Ok(img)=image::load_from_memory(png) {
                            if icon.is_none(){icon=Some(icon_from_image(img.clone()));}
                            tips.push(tip_from_image(&img));
                        }
                    }
                }
            }
        }
    }
    let num=|k:&str,d:f32|value_f32(variant.get(k)).unwrap_or(d);
    let size=num("BrushSize",30.0);
    let _=size;
    let opacity=(num("Opacity",100.0)/100.0).clamp(0.05,1.0);
    let flow=(num("BrushFlow",100.0)/100.0).clamp(0.02,1.0);
    let auto=num("BrushAutoIntervalType",0.0) as i32;
    let spacing=if auto==0{(num("BrushInterval",10.0)/100.0).clamp(0.02,2.0)}
        else{match auto{1=>0.05,2=>0.10,3=>0.25,_=>0.10}};
    let rotation=num("BrushRotation",0.0);
    let scatter=if num("BrushUseSpray",0.0)>0.5{0.75}else{0.0};
    if tips.is_empty(){tips.push(round_tip(96,num("BrushHardness",100.0)/100.0));}
    Ok(vec![ImportedBrushDraft{
        suggested_name:name,source_format:"Clip Studio Paint .sut".into(),source_path:path.to_path_buf(),
        tips,icon,spacing,opacity,flow,size_pressure:1.0,opacity_pressure:0.0,angle_deg:rotation,
        follow_stroke:false,scatter,random_tip:num("BrushPatternOrderType",0.0) as i32==3,tip_scale:1.0,
        compatibility_note:"CSP size/opacity/flow/spacing/rotation and readable embedded tip thumbnails are mapped. CSP-only mixing, encrypted material internals and some dynamics are approximated.".into(),
    }])
}

fn import_sutg_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    if fs::read(path).map(|b|b.starts_with(b"SQLite format 3\0")).unwrap_or(false) {
        return import_sut_file(path);
    }
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    if let Ok(mut zip)=zip::ZipArchive::new(Cursor::new(bytes)) {
        let mut out=Vec::new();
        for i in 0..zip.len() {
            if out.len()>=MAX_IMPORT_BRUSHES{break}
            let mut f=zip.by_index(i).map_err(|e|e.to_string())?;
            if !f.name().to_ascii_lowercase().ends_with(".sut"){continue}
            let mut buf=Vec::new();f.read_to_end(&mut buf).map_err(|e|e.to_string())?;
            let temp=std::env::temp_dir().join(format!("stick_sutg_{}_{}.sut",std::process::id(),i));
            fs::write(&temp,&buf).map_err(|e|e.to_string())?;
            if let Ok(mut brushes)=import_sut_file(&temp) {
                for b in &mut brushes {b.source_path=path.to_path_buf();b.source_format="Clip Studio Paint .sutg".into();}
                out.extend(brushes);
            }
            let _=fs::remove_file(temp);
        }
        if !out.is_empty(){return Ok(out)}
    }
    Ok(vec![baseline_draft(path,"Clip Studio Paint .sutg",
        "This tool-group container was accepted, but its vendor-specific grouping could not be decoded; Stick created a safe raster baseline.")])
}

fn first_row_map(con:&Connection,table:&str)->Result<HashMap<String,Value>,String>{
    let sql=format!("SELECT * FROM \"{}\" LIMIT 1",table.replace('"',""));
    let mut st=con.prepare(&sql).map_err(|e|e.to_string())?;
    let names:Vec<String>=st.column_names().iter().map(|s|s.to_string()).collect();
    let mut rows=st.query([]).map_err(|e|e.to_string())?;
    let Some(row)=rows.next().map_err(|e|e.to_string())? else{return Ok(HashMap::new())};
    let mut out=HashMap::new();
    for (i,name) in names.into_iter().enumerate() {
        if let Ok(v)=row.get::<_,Value>(i){out.insert(name,v);}
    }
    Ok(out)
}
fn value_f32(v:Option<&Value>)->Option<f32>{
    match v?{Value::Integer(x)=>Some(*x as f32),Value::Real(x)=>Some(*x as f32),Value::Text(s)=>s.parse().ok(),_=>None}
}

fn import_blend_file(path:&Path)->Result<Vec<ImportedBrushDraft>,String>{
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let blend=blend::Blend::from_path(path).map_err(|e|format!("Blender .blend: {e:?}"))?;
        let mut out=Vec::new();
        for (index,brush) in blend.instances_with_code(*b"BR").take(MAX_IMPORT_BRUSHES).enumerate() {
            let safe_f32=|field:&str,default:f32| -> f32 {
                if !brush.is_valid(field){return default}
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||brush.get_f32(field)))
                    .unwrap_or(default)
            };
            let safe_i32=|field:&str,default:i32| -> i32 {
                if !brush.is_valid(field){return default}
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||brush.get_i32(field)))
                    .unwrap_or(default)
            };
            let safe_u32=|field:&str,default:u32| -> u32 {
                if !brush.is_valid(field){return default}
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||brush.get_u32(field)))
                    .unwrap_or_else(|_|safe_i32(field,default as i32) as u32)
            };
            let raw_name=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
                brush.get("id").get_string("name")
            })).unwrap_or_else(|_|format!("BRBrush {}",index+1));
            let name=raw_name.strip_prefix("BR").unwrap_or(&raw_name).trim().to_owned();
            let size=safe_i32("size",30).clamp(1,5000);
            let alpha=safe_f32("alpha",1.0).clamp(0.05,1.0);
            let raw_flow=safe_f32("flow",1.0);
            let flow=if raw_flow<=0.001{1.0}else{raw_flow.clamp(0.02,1.0)};
            let spacing=(safe_i32("spacing",10) as f32/100.0).clamp(0.02,2.0);
            let jitter=safe_f32("jitter",0.0).clamp(0.0,3.0);
            let hardness=safe_f32("hardness",0.95).clamp(0.0,0.999);
            let smooth=safe_f32("smooth_stroke_factor",0.0).clamp(0.0,1.0);
            let flags=safe_u32("flag",0);
            let size_pressure=if flags&(1<<3)!=0{1.0}else{0.0};
            let opacity_pressure=if flags&(1<<2)!=0{1.0}else{0.0};
            let gp_type=safe_i32("gpencil_brush_type",0);
            out.push(ImportedBrushDraft{
                suggested_name:if name.is_empty(){format!("Blender Brush {}",index+1)}else{name},
                source_format:"Blender Grease Pencil .blend/.asset.blend".into(),
                source_path:path.to_path_buf(),tips:vec![round_tip(96,hardness)],icon:None,
                spacing,opacity:alpha,flow,size_pressure,opacity_pressure,angle_deg:0.0,
                follow_stroke:false,scatter:jitter,random_tip:false,tip_scale:1.0,
                compatibility_note:format!(
                    "Blender Brush settings mapped: size {size}px, spacing {:.0}%, opacity {:.0}%, jitter {:.2}, smooth {:.2}, Grease Pencil type {}. Asset materials/textures without a Stick equivalent are approximated.",
                    spacing*100.0,alpha*100.0,jitter,smooth,gp_type),
            });
        }
        Ok::<_,String>(out)
    })).map_err(|_|"Blender asset parser rejected this .blend safely".to_string())??;
    if result.is_empty() {
        Ok(vec![baseline_draft(path,"Blender Grease Pencil .blend/.asset.blend",
            "No Brush datablocks were found; Stick created a safe raster baseline.")])
    } else {
        Ok(result)
    }
}

fn tips_from_resource(filename:&str,data:&[u8])->Vec<TipMask>{
    let ext=Path::new(filename).extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str(){
        "gbr"=>parse_gbr(data,0).ok().map(|x|vec![x.1]).unwrap_or_default(),
        "gih"=>parse_gih_bytes(data,Path::new(filename)).ok()
            .and_then(|v|v.into_iter().next()).map(|d|d.tips).unwrap_or_default(),
        "png"|"jpg"|"jpeg"|"webp"|"bmp"=>image::load_from_memory(data)
            .ok().map(|i|vec![tip_from_image(&i)]).unwrap_or_default(),
        _=>Vec::new(),
    }
}

fn png_text_chunks(data:&[u8])->HashMap<String,String>{
    let mut out=HashMap::new();
    if !data.starts_with(b"\x89PNG\r\n\x1a\n"){return out}
    let mut pos=8usize;
    while pos+12<=data.len() {
        let len=u32::from_be_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        if pos+12+len>data.len(){break}
        let typ=&data[pos+4..pos+8];
        let chunk=&data[pos+8..pos+8+len];
        if typ==b"tEXt" {
            if let Some(z)=chunk.iter().position(|&b|b==0) {
                out.insert(String::from_utf8_lossy(&chunk[..z]).to_string(),String::from_utf8_lossy(&chunk[z+1..]).to_string());
            }
        } else if typ==b"zTXt" {
            if let Some(z)=chunk.iter().position(|&b|b==0) {
                if chunk.get(z+1)==Some(&0) {
                    let mut dec=ZlibDecoder::new(&chunk[z+2..]);
                    let mut text=String::new();
                    if dec.read_to_string(&mut text).is_ok(){out.insert(String::from_utf8_lossy(&chunk[..z]).to_string(),text);}
                }
            }
        } else if typ==b"iTXt" {
            if let Some(z)=chunk.iter().position(|&b|b==0) {
                let key=String::from_utf8_lossy(&chunk[..z]).to_string();
                if z+3<chunk.len() {
                    let compressed=chunk[z+1]==1;
                    let mut p=z+3;
                    for _ in 0..2 {
                        if let Some(n)=chunk[p..].iter().position(|&b|b==0){p+=n+1}else{p=chunk.len();break}
                    }
                    if p<=chunk.len() {
                        if compressed {
                            let mut dec=ZlibDecoder::new(&chunk[p..]);let mut text=String::new();
                            if dec.read_to_string(&mut text).is_ok(){out.insert(key,text);}
                        } else {out.insert(key,String::from_utf8_lossy(&chunk[p..]).to_string());}
                    }
                }
            }
        }
        pos+=12+len;
    }
    out
}

fn find_number_in_params(map:&HashMap<String,String>,names:&[&str])->Option<f32>{
    for wanted in names {
        for (k,v) in map {
            if k.eq_ignore_ascii_case(wanted)||k.to_ascii_lowercase().ends_with(&wanted.to_ascii_lowercase()) {
                if let Some(x)=first_number(v){return Some(x)}
            }
        }
    }
    None
}
fn first_number(s:&str)->Option<f32>{
    let mut buf=String::new();let mut started=false;
    for c in s.chars() {
        if c.is_ascii_digit()||c=='.'||c=='-'||c=='+' {
            buf.push(c);started=true;
        } else if started {break}
    }
    buf.parse().ok()
}
fn is_truthy(v:&str)->bool{
    matches!(v.trim().to_ascii_lowercase().as_str(),"1"|"true"|"yes"|"on")
}

fn find_first_png(data:&[u8])->Option<&[u8]>{
    let sig=b"\x89PNG\r\n\x1a\n";
    let start=data.windows(sig.len()).position(|w|w==sig)?;
    let mut p=start+8;
    while p+12<=data.len() {
        let len=u32::from_be_bytes(data[p..p+4].try_into().ok()?) as usize;
        if p+12+len>data.len(){return None}
        let typ=&data[p+4..p+8];
        p+=12+len;
        if typ==b"IEND"{return Some(&data[start..p])}
    }
    None
}

fn tip_from_image(img:&DynamicImage)->TipMask{
    let rgba=img.to_rgba8();
    let has_alpha=rgba.pixels().any(|p|p[3]<250);
    let data=if has_alpha {
        rgba.pixels().map(|p|p[3]).collect()
    } else {
        img.to_luma8().pixels().map(|p|255u8.saturating_sub(p[0])).collect()
    };
    downsample_tip(TipMask{width:img.width(),height:img.height(),data})
}

fn downsample_tip(tip:TipMask)->TipMask{
    if tip.width<=MAX_TIP_SIDE&&tip.height<=MAX_TIP_SIDE{return tip}
    let Some(gray)=GrayImage::from_raw(tip.width,tip.height,tip.data) else{return round_tip(96,0.9)};
    let scale=MAX_TIP_SIDE as f32/tip.width.max(tip.height) as f32;
    let w=(tip.width as f32*scale).round().max(1.0) as u32;
    let h=(tip.height as f32*scale).round().max(1.0) as u32;
    let resized=image::imageops::resize(&gray,w,h,image::imageops::FilterType::Triangle);
    TipMask{width:w,height:h,data:resized.into_raw()}
}

fn icon_from_image(img:DynamicImage)->IconImage{
    let rgba=img.thumbnail(200,200).to_rgba8();
    IconImage{width:rgba.width(),height:rgba.height(),rgba:rgba.into_raw()}
}
fn load_icon(path:&Path)->Option<IconImage>{image::open(path).ok().map(icon_from_image)}
fn load_tip(path:&Path)->Option<TipMask>{
    let img=image::open(path).ok()?.to_luma8();
    Some(TipMask{width:img.width(),height:img.height(),data:img.into_raw()})
}

fn elliptical_tip(size:u32,hardness:f32,ratio:f32,angle_deg:f32)->TipMask{
    let size=size.max(8);let r=size as f32/2.0;let hard=hardness.clamp(0.0,0.999);
    let ratio=ratio.clamp(0.05,1.0);let a=angle_deg.to_radians();let (sin,cos)=a.sin_cos();
    let mut data=Vec::with_capacity((size*size) as usize);
    for y in 0..size {for x in 0..size {
        let dx=(x as f32+0.5-r)/r;let dy=(y as f32+0.5-r)/r;
        let xr=dx*cos+dy*sin;let yr=(-dx*sin+dy*cos)/ratio;
        let d=(xr*xr+yr*yr).sqrt();
        let alpha=if d<=hard{1.0}else{((1.0-d)/(1.0-hard)).clamp(0.0,1.0)};
        data.push((alpha*255.0).round() as u8);
    }}
    TipMask{width:size,height:size,data}
}

fn round_tip(size:u32,hardness:f32)->TipMask{
    let size=size.max(8);let r=size as f32/2.0;let hard=hardness.clamp(0.0,0.999);
    let mut data=Vec::with_capacity((size*size) as usize);
    for y in 0..size {for x in 0..size {
        let dx=(x as f32+0.5-r)/r;let dy=(y as f32+0.5-r)/r;let d=(dx*dx+dy*dy).sqrt();
        let a=if d<=hard{1.0}else{((1.0-d)/(1.0-hard)).clamp(0.0,1.0)};
        data.push((a*255.0).round() as u8);
    }}
    TipMask{width:size,height:size,data}
}

fn save_tip_png(tip:&TipMask,path:&Path)->Result<(),String>{
    let gray=GrayImage::from_raw(tip.width,tip.height,tip.data.clone()).ok_or("Invalid tip image")?;
    gray.save_with_format(path,ImageFormat::Png).map_err(|e|e.to_string())
}
fn save_tip_preview_png(tip:&TipMask,path:&Path)->Result<(),String>{
    let mut rgba=Vec::with_capacity(tip.data.len()*4);
    for &a in &tip.data {rgba.extend_from_slice(&[20,20,20,a]);}
    let img=RgbaImage::from_raw(tip.width,tip.height,rgba).ok_or("Invalid tip preview")?;
    img.save_with_format(path,ImageFormat::Png).map_err(|e|e.to_string())
}
fn save_icon_png(icon:&IconImage,path:&Path)->Result<(),String>{
    let img=RgbaImage::from_raw(icon.width,icon.height,icon.rgba.clone()).ok_or("Invalid icon")?;
    img.save_with_format(path,ImageFormat::Png).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_extensions_cover_requested_apps() {
        for ext in ["kpp","bundle","myb","gbr","gih","abr","sut","sutg","blend"] {
            assert!(accepted_extensions().contains(&ext),"{ext}");
        }
    }

    #[test]
    fn custom_id_prefix_is_project_safe() {
        let stroke=Stroke{
            points:vec![],color:[0,0,0,255],fill_color:None,base_width:10.0,is_vector:false,
            brush_preset:crate::project::KritaBrushPreset::GPenInk,opacity:1.0,
            brush_texture_id:Some("custom:abc123".into()),tip_shape:crate::project::BrushTipShape::Round,
        };
        assert_eq!(stroke_custom_id(&stroke),Some("abc123"));
    }

    #[test]
    fn gbr_v2_gray_tip_decodes() {
        let name=b"test\0";
        let header=(28+name.len()) as u32;
        let mut data=Vec::new();
        for v in [header,2,2,1,1,u32::from_be_bytes(*b"GIMP"),25] {data.extend(v.to_be_bytes());}
        data.extend(name);data.extend([10,240]);
        let (n,tip,spacing,_)=parse_gbr(&data,0).unwrap();
        assert_eq!(n,"test");assert_eq!((tip.width,tip.height),(2,1));
        assert_eq!(tip.data,vec![10,240]);assert_eq!(spacing,25);
    }

    #[test]
    fn round_tip_is_bounded_and_visible() {
        let t=round_tip(64,0.8);
        assert_eq!(t.data.len(),64*64);
        assert!(t.data.iter().any(|&a|a==255));
        assert!(t.data.iter().any(|&a|a==0));
    }
}
