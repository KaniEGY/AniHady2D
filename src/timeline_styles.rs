//! Optional AniHady-inspired exposure-sheet timeline visuals.
//! These helpers do not mutate project data or timeline commands: OLD and
//! NEW share the exact same frames, keyframes, and context-menu actions.
use eframe::egui;
use crate::project::{FrameData, FrameMarkerKind, TweenMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameVisualState {
    EmptyBase,
    Copied,
    NewBlank,
    Authored,
}

/// One stable color system shared by OLD, NEW and New mine:
/// red = untouched/default empty frame; green = exact duplicate; yellow =
/// newly created blank keyframe; white = authored/edited drawing.
pub fn frame_visual_state(marker: FrameMarkerKind, has_content: bool) -> FrameVisualState {
    if marker == FrameMarkerKind::CopiedPose {
        return FrameVisualState::Copied;
    }
    if has_content {
        return FrameVisualState::Authored;
    }
    match marker {
        FrameMarkerKind::Blank | FrameMarkerKind::InsertedEmpty => FrameVisualState::NewBlank,
        FrameMarkerKind::Legacy => FrameVisualState::EmptyBase,
        FrameMarkerKind::CopiedPose => FrameVisualState::Copied,
    }
}

pub fn draw_action_marker(
    painter: &egui::Painter, center: egui::Pos2, radius: f32,
    marker: FrameMarkerKind, has_content: bool,
) -> bool {
    let (fill,stroke)=match frame_visual_state(marker,has_content) {
        FrameVisualState::EmptyBase => (
            egui::Color32::from_rgb(232,67,67),
            egui::Stroke::new(1.2,egui::Color32::from_rgb(92,22,22))),
        FrameVisualState::Copied => (
            egui::Color32::from_rgb(55,205,105),
            egui::Stroke::new(1.2,egui::Color32::from_rgb(18,76,43))),
        FrameVisualState::NewBlank => (
            egui::Color32::from_rgb(247,194,66),
            egui::Stroke::new(1.2,egui::Color32::from_rgb(104,74,12))),
        FrameVisualState::Authored => (
            egui::Color32::WHITE,
            egui::Stroke::new(1.4,egui::Color32::BLACK)),
    };
    painter.circle_filled(center,radius,fill);
    painter.circle_stroke(center,radius,stroke);
    true
}

/// Frame 1 is time zero. Display compact minute:second:subframe timing.
pub fn label_for_frame(frame: usize, fps: u32, timecodes: bool) -> String {
    if !timecodes { return format!("{}", frame); }
    let fps = fps.max(1) as usize;
    let index = frame.saturating_sub(1);
    let seconds = index / fps;
    let subframe = index % fps;
    format!("{:02}:{:02}:{:02}", seconds / 60, seconds % 60, subframe)
}

pub fn draw_header(
    painter: &egui::Painter,
    rect: egui::Rect,
    frame: usize,
    fps: u32,
    current: bool,
    in_loop: bool,
    timecodes: bool,
) {
    let muted = egui::Color32::from_rgb(119, 147, 171);
    let major = frame == 1 || (frame - 1) % 5 == 0;
    let bg = if current { egui::Color32::from_rgb(35, 112, 156) }
        else if in_loop { egui::Color32::from_rgb(37, 76, 68) }
        else { egui::Color32::from_rgb(26, 38, 49) };
    painter.rect_filled(rect, 0.0, bg);
    painter.line_segment(
        [egui::pos2(rect.left(), rect.bottom() - if major { 9.0 } else { 5.0 }),
         egui::pos2(rect.left(), rect.bottom())],
        egui::Stroke::new(1.2, if current { egui::Color32::WHITE } else { muted }),
    );
    if major || current {
        let caption = if timecodes && rect.width() >= 60.0 {
            label_for_frame(frame, fps, true)
        } else { frame.to_string() };
        painter.text(rect.center() + egui::vec2(0.0, -3.0),
            egui::Align2::CENTER_CENTER,
            caption, egui::FontId::monospace(if timecodes && rect.width() >= 60.0 { 10.5 } else { 12.0 }),
            if current { egui::Color32::WHITE } else { egui::Color32::from_rgb(198, 218, 233) });
    }
    if current {
        let y = rect.bottom();
        let x = rect.left() + rect.width() * 0.5;
        painter.add(egui::Shape::convex_polygon(
            vec![egui::pos2(x - 6.0, y - 1.0), egui::pos2(x + 6.0, y - 1.0),
                 egui::pos2(x, y + 6.0)],
            egui::Color32::from_rgb(247, 190, 59), egui::Stroke::NONE,
        ));
    }
}

pub fn draw_cell(
    painter: &egui::Painter,
    rect: egui::Rect,
    is_keyframe: bool,
    has_content: bool,
    marker: FrameMarkerKind,
    tween: TweenMode,
    active: bool,
    locked: bool,
    playhead: bool,
) {
    // Exposure is a horizontal bar rather than the individual square cells
    // used by the legacy editor; keyframe diamonds sit above the bar.
    let background = if locked { egui::Color32::from_rgb(28, 35, 40) }
        else if active { egui::Color32::from_rgb(35, 56, 75) }
        else { egui::Color32::from_rgb(31, 38, 49) };
    painter.rect_filled(rect, 0.0, background);
    let bar = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 0.4, rect.center().y - 8.0),
        egui::pos2(rect.right() - 0.4, rect.center().y + 8.0),
    );
    let tone = match tween {
        TweenMode::Classic => egui::Color32::from_rgb(103, 77, 176),
        TweenMode::Shape => egui::Color32::from_rgb(33, 131, 109),
        TweenMode::None if has_content => egui::Color32::from_rgb(63, 94, 129),
        _ => egui::Color32::from_rgb(43, 50, 61),
    };
    painter.rect_filled(bar, if is_keyframe { 3.0 } else { 0.0 }, tone);
    painter.line_segment(
        [egui::pos2(rect.right(), rect.top() + 1.0),
         egui::pos2(rect.right(), rect.bottom() - 1.0)],
        egui::Stroke::new(0.7, egui::Color32::from_rgb(52, 64, 76)),
    );
    if is_keyframe {
        let center=egui::pos2(rect.left() + rect.width().min(17.0) * 0.5,rect.center().y);
        let radius = if rect.width() >= 28.0 { 5.6 } else { 4.3 };
        if !draw_action_marker(painter, center, radius, marker, has_content) {
            let color=if has_content { egui::Color32::from_rgb(247, 194, 66) }
                else { egui::Color32::from_rgb(194, 219, 235) };
            painter.add(egui::Shape::convex_polygon(
                vec![center + egui::vec2(0.0,-radius), center + egui::vec2(radius,0.0),
                     center + egui::vec2(0.0,radius), center + egui::vec2(-radius,0.0)],
                color, egui::Stroke::new(0.6,egui::Color32::from_rgb(23,30,37)),
            ));
        }
    } else if has_content {
        let x=rect.center().x;
        painter.circle_filled(egui::pos2(x,rect.center().y), 1.8,
            egui::Color32::from_rgb(155, 184, 210));
    }
    if playhead {
        let x=rect.left() + rect.width()*0.5;
        painter.line_segment(
            [egui::pos2(x,rect.top()),egui::pos2(x,rect.bottom())],
            egui::Stroke::new(2.2, egui::Color32::from_rgb(249, 189, 63)),
        );
    }
}


/// Clip Timeline cell: thumbnail-first presentation inspired by exposure
/// thumbnails in traditional animation software. It deliberately ignores the
/// red/green/yellow/white marker system so the drawing itself is the focus.
pub fn draw_clip_cell(
    painter:&egui::Painter,
    rect:egui::Rect,
    frame:Option<&FrameData>,
    stage_size:(f32,f32),
    current:bool,
    selected:bool,
    locked:bool,
) {
    let body=rect.shrink2(egui::vec2(2.0,3.0));
    let border=if selected {egui::Color32::from_rgb(255,205,63)}
        else if current {egui::Color32::from_rgb(70,190,245)}
        else {egui::Color32::from_rgb(92,99,110)};
    painter.rect_filled(body,3.0,
        if locked {egui::Color32::from_rgb(45,45,48)}
        else {egui::Color32::from_rgb(238,239,241)});
    painter.rect_stroke(body,3.0,egui::Stroke::new(if selected{2.2}else{1.0},border));
    let Some(frame)=frame else{return};
    let preview=body.shrink2(egui::vec2(4.0,4.0));
    let sx=preview.width()/stage_size.0.max(1.0);
    let sy=preview.height()/stage_size.1.max(1.0);
    let scale=sx.min(sy);
    let origin=preview.center()-egui::vec2(stage_size.0*scale,stage_size.1*scale)*0.5;

    let mut budget=420usize;
    for stroke in &frame.strokes {
        if budget==0{break;}
        if stroke.points.len()<2{continue;}
        let color=egui::Color32::from_rgba_unmultiplied(
            stroke.color[0],stroke.color[1],stroke.color[2],
            ((stroke.color[3] as f32)*stroke.opacity.clamp(0.0,1.0)) as u8);
        let step=((stroke.points.len().saturating_sub(1))/budget.max(1)).max(1);
        let mut last:Option<egui::Pos2>=None;
        for p in stroke.points.iter().step_by(step) {
            let q=origin+egui::vec2(p.x*scale,p.y*scale);
            if let Some(prev)=last {
                painter.line_segment([prev,q],egui::Stroke::new(
                    (stroke.base_width*scale).clamp(0.7,2.4),color));
                budget=budget.saturating_sub(1);
                if budget==0{break;}
            }
            last=Some(q);
        }
    }
    if !frame.text_items.is_empty() {
        painter.text(preview.right_bottom()-egui::vec2(3.0,2.0),
            egui::Align2::RIGHT_BOTTOM,"T",egui::FontId::proportional(9.0),
            egui::Color32::from_rgb(35,70,120));
    }
    if !frame.media_items.is_empty() {
        painter.text(preview.left_bottom()+egui::vec2(3.0,-2.0),
            egui::Align2::LEFT_BOTTOM,"▣",egui::FontId::proportional(9.0),
            egui::Color32::from_rgb(90,75,35));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_frame_color_states_match_editor_rules() {
        assert_eq!(frame_visual_state(FrameMarkerKind::Legacy,false),FrameVisualState::EmptyBase);
        assert_eq!(frame_visual_state(FrameMarkerKind::CopiedPose,true),FrameVisualState::Copied);
        assert_eq!(frame_visual_state(FrameMarkerKind::Blank,false),FrameVisualState::NewBlank);
        assert_eq!(frame_visual_state(FrameMarkerKind::InsertedEmpty,false),FrameVisualState::NewBlank);
        assert_eq!(frame_visual_state(FrameMarkerKind::Legacy,true),FrameVisualState::Authored);
    }

    #[test]
    fn one_based_frames_generate_correct_timecode() {
        assert_eq!(label_for_frame(1,30,true), "00:00:00");
        assert_eq!(label_for_frame(31,30,true), "00:01:00");
        assert_eq!(label_for_frame(1801,30,true), "01:00:00");
        assert_eq!(label_for_frame(48,24,false), "48");
        assert_eq!(label_for_frame(2,0,true), "00:01:00");
    }
}
