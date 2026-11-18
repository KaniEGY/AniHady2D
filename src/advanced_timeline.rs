//! Advanced Timeline editing core and exposure visualization.
//!
//! The original two timeline styles remain unchanged. Every operation here is
//! validated before mutation and uses the existing editor Undo snapshots.
use crate::project::{AniProject, FrameMarkerKind, TweenMode};
use eframe::egui;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CelAddress {
    pub layer: usize,
    pub frame: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocateMode { Move, Copy }

/// Move or duplicate multiple genuine keyframes (not merely held exposure
/// frames), potentially between unlocked layers. Validation is atomic: any
/// collision or out-of-range target rejects the entire operation.
pub fn relocate_keyframes(
    project: &mut AniProject,
    selected: &[CelAddress],
    layer_delta: i32,
    frame_delta: i32,
    mode: RelocateMode,
) -> Result<Vec<CelAddress>, &'static str> {
    if selected.is_empty() { return Err("No keyframes selected."); }
    if selected.len() > 10_000 { return Err("Too many keyframes selected."); }
    if layer_delta == 0 && frame_delta == 0 { return Err("Choose another destination."); }

    let mut sources = HashSet::new();
    for &cell in selected {
        if !sources.insert(cell) { return Err("Duplicate keyframe selection."); }
        if cell.frame == 0 || cell.frame > project.total_frames {
            return Err("Source frame is outside the timeline.");
        }
        let layer = project.layers.get(cell.layer).ok_or("Source layer is missing.")?;
        if layer.locked { return Err("Source layer is locked."); }
        if !layer.keyframes.contains_key(&cell.frame) {
            return Err("Selection contains a held exposure, not a keyframe.");
        }
    }

    let mut targets = Vec::with_capacity(selected.len());
    let mut distinct_targets = HashSet::new();
    for &cell in selected {
        let layer = (cell.layer as i64) + i64::from(layer_delta);
        let frame = (cell.frame as i64) + i64::from(frame_delta);
        if layer < 0 || layer >= project.layers.len() as i64
            || frame < 1 || frame > project.total_frames as i64 {
            return Err("Move is outside the layer or timeline bounds.");
        }
        let destination = CelAddress { layer: layer as usize, frame: frame as usize };
        if !distinct_targets.insert(destination) { return Err("Destination keyframe overlaps."); }
        let target_layer = &project.layers[destination.layer];
        if target_layer.locked { return Err("Destination layer is locked."); }
        if target_layer.keyframes.contains_key(&destination.frame)
            && (mode == RelocateMode::Copy || !sources.contains(&destination)) {
            return Err("Destination already contains another keyframe.");
        }
        targets.push(destination);
    }

    let content: Vec<_> = selected.iter().map(|c|
        project.layers[c.layer].keyframes[&c.frame].clone()).collect();
    if mode == RelocateMode::Move {
        for &cell in selected {
            project.layers[cell.layer].keyframes.remove(&cell.frame);
        }
    }
    for (destination, frame_data) in targets.iter().copied().zip(content) {
        project.layers[destination.layer].keyframes.insert(destination.frame, frame_data);
    }
    if mode == RelocateMode::Move {
        // The project format expects an initial exposure anchor. Preserve it
        // even when moving the old frame-1 key to a later frame.
        for layer in &mut project.layers {
            layer.keyframes.entry(1).or_default();
        }
    }
    Ok(targets)
}

pub fn render_ruler_cell(
    painter: &egui::Painter, rect: egui::Rect, frame: usize, fps: u32,
    current: bool, show_timecodes: bool,
) {
    let major = frame == 1 || (frame-1) % 5 == 0;
    painter.rect_filled(rect, 0.0,
        if current { egui::Color32::from_rgb(81, 57, 29) }
        else { egui::Color32::from_rgb(36, 40, 49) });
    painter.line_segment(
        [egui::pos2(rect.left(),rect.bottom()-if major {12.} else {6.}),
         egui::pos2(rect.left(),rect.bottom())],
        egui::Stroke::new(1.,egui::Color32::from_gray(if major {205} else {96})));
    if major || current {
        let index=frame.saturating_sub(1);
        let caption=if show_timecodes {
            let fps=fps.max(1) as usize;
            format!("{:02}:{:02}:{:02}",index/(fps*60),index/fps%60,index%fps)
        } else {format!("{frame}")};
        painter.text(rect.center_top()+egui::vec2(0.0,8.0),
            egui::Align2::CENTER_TOP,caption,egui::FontId::monospace(9.),
            if current {egui::Color32::from_rgb(255,220,117)}
            else {egui::Color32::from_gray(201)});
    }
    if current {
        painter.line_segment([rect.left_bottom(),rect.right_bottom()],
            egui::Stroke::new(3.,egui::Color32::from_rgb(251,196,62)));
    }
}

pub fn render_exposure_cell(
    painter: &egui::Painter, rect: egui::Rect,
    keyframe: bool, occupied: bool, marker: FrameMarkerKind, tween: TweenMode,
    current: bool, selected: bool, locked: bool,
) {
    let fill = if locked {egui::Color32::from_rgb(47,47,53)}
        else if occupied {egui::Color32::from_rgb(41,80,92)}
        else {egui::Color32::from_rgb(34,38,46)};
    let body=rect.shrink2(egui::vec2(0.,4.));
    painter.rect_filled(body,1.0,fill);
    if occupied {
        painter.line_segment(
            [egui::pos2(body.left(),body.bottom()-1.),
             egui::pos2(body.right(),body.bottom()-1.)],
             egui::Stroke::new(1.0,egui::Color32::from_rgb(87,178,188)));
    }
    if keyframe {
        let x=rect.center().x;
        let y=rect.center().y;
        let size=if selected {7.} else {5.5};
        if !crate::timeline_styles::draw_action_marker(
            painter, egui::pos2(x,y), size, marker, occupied
        ) {
            let diamond=vec![egui::pos2(x,y-size),egui::pos2(x+size,y),
                egui::pos2(x,y+size),egui::pos2(x-size,y)];
            let color=if tween!=TweenMode::None {
                egui::Color32::from_rgb(215,145,245)
            } else if occupied {egui::Color32::from_rgb(247,206,105)}
            else {egui::Color32::from_rgb(182,194,205)};
            painter.add(egui::Shape::convex_polygon(diamond,color,egui::Stroke::NONE));
        }
    }
    if selected {
        painter.rect_stroke(rect.shrink(1.0),2.0,
            egui::Stroke::new(2.0,egui::Color32::from_rgb(255,205,77)));
    } else if current {
        painter.line_segment([rect.left_top(),rect.left_bottom()],
            egui::Stroke::new(2.0,egui::Color32::from_rgb(255,192,71)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::FrameData;
    fn key(label: &str) -> FrameData {
        FrameData { label: label.into(), ..Default::default() }
    }
    fn project() -> AniProject {
        let mut p=AniProject::default();
        p.total_frames=16;
        p.layers[0].keyframes.insert(3,key("three"));
        p.layers[0].keyframes.insert(5,key("five"));
        let mut second=p.layers[0].clone();
        second.keyframes.clear();
        second.keyframes.insert(1,key("other"));
        p.layers.push(second);
        p
    }
    #[test]
    fn move_multiple_keys_within_a_layer_and_keep_labels() {
        let mut p=project();
        let chosen=[CelAddress{layer:0,frame:3},CelAddress{layer:0,frame:5}];
        let dest=relocate_keyframes(&mut p,&chosen,0,4,RelocateMode::Move).unwrap();
        assert_eq!(dest[0],CelAddress{layer:0,frame:7});
        assert!(!p.layers[0].keyframes.contains_key(&3));
        assert_eq!(p.layers[0].keyframes[&7].label,"three");
        assert_eq!(p.layers[0].keyframes[&9].label,"five");
    }
    #[test]
    fn duplicate_to_other_layer_keeps_original() {
        let mut p=project();
        let selected=[CelAddress{layer:0,frame:3}];
        let dest=relocate_keyframes(&mut p,&selected,1,4,RelocateMode::Copy).unwrap();
        assert_eq!(dest[0],CelAddress{layer:1,frame:7});
        assert_eq!(p.layers[1].keyframes[&7].label,"three");
        assert_eq!(p.layers[0].keyframes[&3].label,"three");
    }
    #[test]
    fn all_collisions_and_locked_layers_are_atomic() {
        let mut p=project();
        let before=serde_json::to_value(&p).unwrap();
        assert!(relocate_keyframes(&mut p,&[CelAddress{layer:0,frame:3}],0,2,RelocateMode::Move).is_err());
        assert_eq!(before,serde_json::to_value(&p).unwrap());
        p.layers[1].locked=true;
        let before=serde_json::to_value(&p).unwrap();
        assert!(relocate_keyframes(&mut p,&[CelAddress{layer:0,frame:3}],1,2,RelocateMode::Move).is_err());
        assert_eq!(before,serde_json::to_value(&p).unwrap());
    }
    #[test]
    fn moving_first_key_preserves_default_anchor() {
        let mut p=project();
        relocate_keyframes(&mut p,&[CelAddress{layer:0,frame:1}],0,1,RelocateMode::Move).unwrap();
        assert!(p.layers[0].keyframes.contains_key(&1));
        assert!(p.layers[0].keyframes.contains_key(&2));
    }
    #[test]
    fn out_of_bounds_and_held_exposures_rejected_without_mutation() {
        let mut p=project();
        let before=serde_json::to_value(&p).unwrap();
        for address in [CelAddress{layer:0,frame:4},CelAddress{layer:12,frame:1}] {
            assert!(relocate_keyframes(&mut p,&[address],0,2,RelocateMode::Move).is_err());
        }
        assert!(relocate_keyframes(&mut p,&[CelAddress{layer:0,frame:3}],0,-4,RelocateMode::Copy).is_err());
        assert_eq!(serde_json::to_value(&p).unwrap(),before);
    }
}
