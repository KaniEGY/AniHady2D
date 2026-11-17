//! Optional AniHady-inspired exposure-sheet timeline visuals.
//! These helpers do not mutate project data or timeline commands: OLD and
//! NEW share the exact same frames, keyframes, and context-menu actions.
use eframe::egui;
use crate::project::TweenMode;

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
        let color=if has_content { egui::Color32::from_rgb(247, 194, 66) }
            else { egui::Color32::from_rgb(194, 219, 235) };
        painter.add(egui::Shape::convex_polygon(
            vec![center + egui::vec2(0.0,-radius), center + egui::vec2(radius,0.0),
                 center + egui::vec2(0.0,radius), center + egui::vec2(-radius,0.0)],
            color, egui::Stroke::new(0.6,egui::Color32::from_rgb(23,30,37)),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_based_frames_generate_correct_timecode() {
        assert_eq!(label_for_frame(1,30,true), "00:00:00");
        assert_eq!(label_for_frame(31,30,true), "00:01:00");
        assert_eq!(label_for_frame(1801,30,true), "01:00:00");
        assert_eq!(label_for_frame(48,24,false), "48");
        assert_eq!(label_for_frame(2,0,true), "00:01:00");
    }
}
