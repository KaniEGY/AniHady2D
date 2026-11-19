//! Optional AniHady-inspired exposure-sheet timeline visuals.
//! These helpers do not mutate project data or timeline commands: OLD and
//! NEW share the exact same frames, keyframes, and context-menu actions.
use eframe::egui;
use crate::project::{AniProject, FrameData, FrameMarkerKind, Stroke, TweenMode};

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


/// Select the cheapest correct source for a Clip Timeline thumbnail. Static
/// holds borrow the current exposure, while tween in-betweens are evaluated so
/// the thumbnail matches the actual visible pose. Cells beyond the document
/// never inherit artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipFrameSource {
    None,
    BorrowExposure,
    EvaluateTween,
}

pub fn clip_frame_source(
    out_of_range: bool,
    explicit_keyframe: bool,
    exposure: Option<&FrameData>,
) -> ClipFrameSource {
    if out_of_range || exposure.is_none() {
        return ClipFrameSource::None;
    }
    if explicit_keyframe {
        return ClipFrameSource::BorrowExposure;
    }
    let exposure = exposure.expect("checked above");
    if exposure.tween_mode != TweenMode::None || exposure.motion_tween {
        ClipFrameSource::EvaluateTween
    } else {
        ClipFrameSource::BorrowExposure
    }
}

fn clip_rgba(color: [u8;4], stroke_opacity: f32, layer_opacity: f32) -> egui::Color32 {
    let alpha = (color[3] as f32
        * stroke_opacity.clamp(0.0, 1.0)
        * layer_opacity.clamp(0.0, 1.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(color[0],color[1],color[2],alpha)
}

fn draw_clip_stroke<F>(
    painter:&egui::Painter,
    preview:egui::Rect,
    stage_size:(f32,f32),
    stroke:&Stroke,
    layer_opacity:f32,
    width_multiplier:f32,
    map:F,
    budget:&mut usize,
) where F: Fn(f32,f32)->(f32,f32) {
    if *budget==0 || stroke.points.is_empty() { return; }
    let sx=preview.width()/stage_size.0.max(1.0);
    let sy=preview.height()/stage_size.1.max(1.0);
    let scale=sx.min(sy);
    let origin=preview.center()-egui::vec2(stage_size.0*scale,stage_size.1*scale)*0.5;
    let to_preview=|x:f32,y:f32| {
        let (x,y)=map(x,y);
        origin+egui::vec2(x*scale,y*scale)
    };

    let line_color=clip_rgba(stroke.color,stroke.opacity,layer_opacity);
    let width=(stroke.base_width*width_multiplier.abs()*scale).clamp(0.7,2.8);

    // A quick pen tap is a valid drawing. The old thumbnail renderer skipped
    // one-point strokes entirely, making some authored frames look blank.
    if stroke.points.len()==1 {
        let p=&stroke.points[0];
        painter.circle_filled(to_preview(p.x,p.y),(width*0.5).max(0.8),line_color);
        *budget=budget.saturating_sub(1);
        return;
    }

    // Fills need a closed thumbnail shape, not just their contour. Cap the
    // number of preview vertices so pathological bucket contours stay cheap.
    if let Some(fill)=stroke.fill_color {
        let step=((stroke.points.len().saturating_sub(1))/256).max(1);
        let polygon:Vec<_>=stroke.points.iter().step_by(step)
            .map(|p|to_preview(p.x,p.y)).collect();
        if polygon.len()>=3 {
            let fill_color=clip_rgba(fill,stroke.opacity,layer_opacity);
            if let Some(mesh)=crate::fill_render::solid_polygon_mesh(&polygon,fill_color) {
                painter.add(egui::Shape::mesh(mesh));
            }
        }
    }

    let step=((stroke.points.len().saturating_sub(1))/(*budget).max(1)).max(1);
    let mut last:Option<egui::Pos2>=None;
    for p in stroke.points.iter().step_by(step) {
        let q=to_preview(p.x,p.y);
        if let Some(prev)=last {
            painter.line_segment([prev,q],egui::Stroke::new(width,line_color));
            *budget=budget.saturating_sub(1);
            if *budget==0 { break; }
        }
        last=Some(q);
    }
}

/// Clip Timeline cell: thumbnail-first presentation inspired by exposure
/// thumbnails in traditional animation software. Unlike the old implementation
/// this paints the actual visible exposure (including held/tweened frames),
/// valid one-point dabs, fills and symbol artwork.
pub fn draw_clip_cell(
    painter:&egui::Painter,
    rect:egui::Rect,
    frame:Option<&FrameData>,
    project:&AniProject,
    global_frame:usize,
    stage_size:(f32,f32),
    layer_opacity:f32,
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

    let mut budget=420usize;
    for stroke in &frame.strokes {
        draw_clip_stroke(
            painter,preview,stage_size,stroke,layer_opacity,1.0,
            |x,y|(x,y),&mut budget);
        if budget==0 { break; }
    }

    // Symbols are real frame artwork too. Use the same nested-frame selection
    // and transform order as export instead of showing an empty thumbnail.
    if budget>0 {
        for instance in &frame.symbol_instances {
            let Some(symbol)=project.library_symbols.get(&instance.symbol_name) else {continue};
            for stroke in project.evaluate_symbol_strokes(symbol,global_frame) {
                draw_clip_stroke(
                    painter,preview,stage_size,stroke,layer_opacity,
                    instance.scale,
                    |x,y|instance.transform_point(x,y),&mut budget);
                if budget==0 { break; }
            }
            if budget==0 { break; }
        }
    }

    // Media bytes live in the editor texture cache, so timeline_styles cannot
    // sample them directly. Still show their real transformed footprint rather
    // than a detached corner badge.
    for media in &frame.media_items {
        let corners=crate::media_transform::corners(media);
        let sx=preview.width()/stage_size.0.max(1.0);
        let sy=preview.height()/stage_size.1.max(1.0);
        let scale=sx.min(sy);
        let origin=preview.center()-egui::vec2(stage_size.0*scale,stage_size.1*scale)*0.5;
        let points:Vec<_>=corners.into_iter()
            .map(|(x,y)|origin+egui::vec2(x*scale,y*scale)).collect();
        if points.len()==4 {
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                egui::Color32::from_rgba_unmultiplied(110,125,145,55),
                egui::Stroke::new(0.7,egui::Color32::from_rgb(85,100,120))));
            painter.line_segment([points[0],points[2]],
                egui::Stroke::new(0.6,egui::Color32::from_rgb(100,112,130)));
            painter.line_segment([points[1],points[3]],
                egui::Stroke::new(0.6,egui::Color32::from_rgb(100,112,130)));
        }
    }

    // Place text indicators at their actual stage position instead of always
    // pinning a T badge to the same corner.
    if !frame.text_items.is_empty() {
        let sx=preview.width()/stage_size.0.max(1.0);
        let sy=preview.height()/stage_size.1.max(1.0);
        let scale=sx.min(sy);
        let origin=preview.center()-egui::vec2(stage_size.0*scale,stage_size.1*scale)*0.5;
        for text in frame.text_items.iter().take(12) {
            painter.text(origin+egui::vec2(text.x*scale,text.y*scale),
                egui::Align2::LEFT_TOP,"T",egui::FontId::proportional(8.0),
                clip_rgba(text.color,1.0,layer_opacity));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clip_timeline_uses_visible_hold_and_evaluates_tween_inbetweens() {
        let static_frame=FrameData::default();
        assert_eq!(
            clip_frame_source(false,false,Some(&static_frame)),
            ClipFrameSource::BorrowExposure
        );
        let tween=FrameData {
            tween_mode:TweenMode::Classic,
            ..Default::default()
        };
        assert_eq!(
            clip_frame_source(false,false,Some(&tween)),
            ClipFrameSource::EvaluateTween
        );
        assert_eq!(
            clip_frame_source(false,true,Some(&tween)),
            ClipFrameSource::BorrowExposure
        );
        assert_eq!(clip_frame_source(true,false,Some(&static_frame)),ClipFrameSource::None);
        assert_eq!(clip_frame_source(false,false,None),ClipFrameSource::None);
    }

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
