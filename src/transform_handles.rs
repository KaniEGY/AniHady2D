//! Screen-space hit testing for Free Transform handles.
//!
//! All hit targets are compared against the exact screen positions that are
//! painted by the editor. This keeps the mouse target aligned with the visible
//! gizmo after zooming, panning, rotating or flipping the canvas.
use eframe::egui;

pub const RESIZE_HIT_RADIUS_PX: f32 = 22.0;
pub const ROTATE_HIT_RADIUS_PX: f32 = 21.0;
pub const TRANSLATE_HIT_RADIUS_PX: f32 = 14.0;
pub const RESIZE_VISUAL_PX: f32 = 13.0;
pub const ROTATE_OFFSET_PX: f32 = 34.0;
pub const TRANSLATE_OFFSET_PX: f32 = 52.0;

pub fn rotation_handle_hit(handle_screen: egui::Pos2, cursor_screen: egui::Pos2) -> bool {
    cursor_screen.distance(handle_screen) <= ROTATE_HIT_RADIUS_PX
}

/// Return the closest visible resize-handle index. The index order is:
/// left-top, right-top, left-bottom, right-bottom.
pub fn resize_handle_index(
    handles_screen: [egui::Pos2; 4],
    cursor_screen: egui::Pos2,
) -> Option<usize> {
    handles_screen
        .into_iter()
        .enumerate()
        .map(|(index, handle)| (handle.distance(cursor_screen), index))
        .filter(|(distance, _)| *distance <= RESIZE_HIT_RADIUS_PX)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, index)| index)
}

pub fn translation_axis_screen(
    x_tip_screen: egui::Pos2,
    y_tip_screen: egui::Pos2,
    cursor_screen: egui::Pos2,
) -> Option<char> {
    let x = cursor_screen.distance(x_tip_screen);
    let y = cursor_screen.distance(y_tip_screen);
    match (x <= TRANSLATE_HIT_RADIUS_PX, y <= TRANSLATE_HIT_RADIUS_PX) {
        (true, true) => if x <= y { Some('x') } else { Some('y') },
        (true, false) => Some('x'),
        (false, true) => Some('y'),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_visible_resize_handles_have_exact_pixel_targets() {
        let handles = [
            egui::pos2(10.0, 20.0),
            egui::pos2(210.0, 20.0),
            egui::pos2(10.0, 140.0),
            egui::pos2(210.0, 140.0),
        ];
        for (index, handle) in handles.into_iter().enumerate() {
            assert_eq!(
                resize_handle_index(handles, handle + egui::vec2(17.0, 0.0)),
                Some(index)
            );
        }
        assert_eq!(resize_handle_index(handles, egui::pos2(110.0, 80.0)), None);
    }

    #[test]
    fn resize_hitbox_is_screen_pixel_sized_regardless_of_transformed_geometry() {
        let handles = [
            egui::pos2(400.0, 150.0),
            egui::pos2(560.0, 190.0),
            egui::pos2(370.0, 280.0),
            egui::pos2(530.0, 320.0),
        ];
        assert_eq!(
            resize_handle_index(handles, handles[3] + egui::vec2(21.0, 0.0)),
            Some(3)
        );
        assert_eq!(
            resize_handle_index(handles, handles[3] + egui::vec2(23.0, 0.0)),
            None
        );
    }

    #[test]
    fn rotate_hitbox_has_a_safe_miss_margin() {
        let handle = egui::pos2(230.0, 120.0);
        assert!(rotation_handle_hit(handle, handle + egui::vec2(18.0, 0.0)));
        assert!(!rotation_handle_hit(handle, handle + egui::vec2(25.0, 0.0)));
    }

    #[test]
    fn translation_arrows_select_independent_screen_targets() {
        let x = egui::pos2(252.0, 100.0);
        let y = egui::pos2(200.0, 48.0);
        assert_eq!(translation_axis_screen(x, y, x), Some('x'));
        assert_eq!(translation_axis_screen(x, y, y), Some('y'));
        assert_eq!(translation_axis_screen(x, y, egui::pos2(200.0, 100.0)), None);
    }
}
