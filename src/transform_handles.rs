//! Screen-space hit testing for Free Transform handles.
//! Use stage coordinates for geometry but constant screen-pixel hit radii so
//! tiny handles stay easy to click at any zoom level.
use eframe::egui;

pub const RESIZE_HIT_RADIUS_PX: f32 = 22.0;
pub const ROTATE_HIT_RADIUS_PX: f32 = 21.0;
pub const RESIZE_VISUAL_PX: f32 = 13.0;
pub const ROTATE_OFFSET_PX: f32 = 34.0;

pub fn rotation_pivot(bounds: egui::Rect, cursor: egui::Pos2, zoom: f32) -> Option<egui::Pos2> {
    let factor = zoom.max(0.1);
    let point = bounds.center_top() + egui::vec2(0.0, -ROTATE_OFFSET_PX / factor);
    (cursor.distance(point) <= ROTATE_HIT_RADIUS_PX / factor).then_some(bounds.center())
}

pub fn resize_anchor(bounds: egui::Rect, cursor: egui::Pos2, zoom: f32) -> Option<egui::Pos2> {
    let radius = RESIZE_HIT_RADIUS_PX / zoom.max(0.1);
    let corners = [
        (bounds.left_top(), bounds.right_bottom()),
        (bounds.right_top(), bounds.left_bottom()),
        (bounds.left_bottom(), bounds.right_top()),
        (bounds.right_bottom(), bounds.left_top()),
    ];
    // Find the nearest corner, not just bottom-right. The opposite corner
    // is the stationary anchor for scaling a selection from this handle.
    corners.iter()
        .map(|(handle, anchor)| (handle.distance(cursor), *anchor))
        .filter(|(distance, _)| *distance <= radius)
        .min_by(|a,b| a.0.total_cmp(&b.0))
        .map(|(_, anchor)| anchor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_handles_have_clickable_targets() {
        let b = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(210.0, 140.0));
        for (handle, expected) in [
            (b.left_top(), b.right_bottom()),
            (b.right_top(), b.left_bottom()),
            (b.left_bottom(), b.right_top()),
            (b.right_bottom(), b.left_top()),
        ] {
            assert_eq!(resize_anchor(b, handle + egui::vec2(17.0, 0.0), 1.0), Some(expected));
        }
        assert_eq!(resize_anchor(b, egui::pos2(110.0, 80.0), 1.0), None);
    }

    #[test]
    fn handle_targets_remain_screen_pixel_sized_at_different_zoom() {
        let b = egui::Rect::from_min_max(egui::pos2(10.,20.),egui::pos2(210.,140.));
        for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
            let near = b.right_bottom() + egui::vec2(19.0 / zoom, 0.0);
            assert_eq!(resize_anchor(b, near, zoom), Some(b.left_top()));
        }
    }

    #[test]
    fn rotate_hitbox_has_a_safe_miss_margin() {
        let b = egui::Rect::from_min_max(egui::pos2(30.,40.),egui::pos2(230.,180.));
        let rot = b.center_top() + egui::vec2(0.0,-ROTATE_OFFSET_PX);
        assert_eq!(rotation_pivot(b, rot + egui::vec2(18.0,0.0),1.0), Some(b.center()));
        assert_eq!(rotation_pivot(b, rot + egui::vec2(25.0,0.0),1.0), None);
    }
}
