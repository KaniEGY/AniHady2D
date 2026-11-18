//! Shared non-destructive transforms for still images and decoded video frames.
//! All coordinates are stage-space; no transform changes source media files.
use crate::project::MediaItem;

pub fn corners(item: &MediaItem) -> [(f32, f32); 4] {
    let center = (item.x + item.width * 0.5, item.y + item.height * 0.5);
    let (sin, cos) = item.rotation_deg.to_radians().sin_cos();
    let hw = item.width * 0.5;
    let hh = item.height * 0.5;
    [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(dx, dy)| {
        (center.0 + dx * cos - dy * sin, center.1 + dx * sin + dy * cos)
    })
}

pub fn hit_test(item: &MediaItem, x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() || !item.width.is_finite()
        || !item.height.is_finite() || item.width <= 0.0 || item.height <= 0.0
        || !item.rotation_deg.is_finite() { return false; }
    let center_x = item.x + item.width * 0.5;
    let center_y = item.y + item.height * 0.5;
    let (sin, cos) = item.rotation_deg.to_radians().sin_cos();
    let (dx, dy) = (x - center_x, y - center_y);
    let unrotated_x = dx * cos + dy * sin;
    let unrotated_y = -dx * sin + dy * cos;
    unrotated_x.abs() <= item.width * 0.5 && unrotated_y.abs() <= item.height * 0.5
}

pub fn translate(item: &mut MediaItem, dx: f32, dy: f32) {
    if dx.is_finite() && dy.is_finite() {
        item.x = (item.x + dx).clamp(-32768.0, 32768.0);
        item.y = (item.y + dy).clamp(-32768.0, 32768.0);
    }
}

pub fn scale_around(item: &mut MediaItem, anchor: (f32, f32), factor: f32) {
    if !factor.is_finite() || factor <= 0.0 { return; }
    let new_w = (item.width * factor).clamp(4.0, 16_384.0);
    let new_h = (item.height * factor).clamp(4.0, 16_384.0);
    let fx = new_w / item.width.max(0.001);
    let fy = new_h / item.height.max(0.001);
    item.x = anchor.0 + (item.x - anchor.0) * fx;
    item.y = anchor.1 + (item.y - anchor.1) * fy;
    item.width = new_w;
    item.height = new_h;
}

pub fn rotate_around(item: &mut MediaItem, pivot: (f32, f32), radians: f32) {
    if !radians.is_finite() { return; }
    let center_x = item.x + item.width * 0.5;
    let center_y = item.y + item.height * 0.5;
    let (sin, cos) = radians.sin_cos();
    let (dx, dy) = (center_x - pivot.0, center_y - pivot.1);
    let new_cx = pivot.0 + dx * cos - dy * sin;
    let new_cy = pivot.1 + dx * sin + dy * cos;
    item.x = new_cx - item.width * 0.5;
    item.y = new_cy - item.height * 0.5;
    item.rotation_deg = (item.rotation_deg + radians.to_degrees()).rem_euclid(360.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{MediaType, MediaItem};

    fn sample() -> MediaItem {
        MediaItem { path: "local.png".into(), media_type: MediaType::Image,
            x: 100.0, y: 80.0, width: 200.0, height: 100.0,
            rotation_deg: 0.0, flip_h: false, flip_v: false }
    }

    #[test]
    fn rotation_hit_testing_and_corners_share_geometry() {
        let mut item = sample();
        assert!(hit_test(&item, 101.0, 100.0));
        item.rotation_deg = 90.0;
        assert!(!hit_test(&item, 101.0, 100.0));
        assert!(hit_test(&item, 200.0, 175.0));
        let points = corners(&item);
        assert!((points[0].0 - 250.0).abs() < 0.01);
        assert!((points[0].1 - 30.0).abs() < 0.01);
    }

    #[test]
    fn moving_scaling_and_rotating_keep_media_non_destructive() {
        let mut item = sample();
        translate(&mut item, 20.0, -5.0);
        assert_eq!((item.x,item.y),(120.0,75.0));
        scale_around(&mut item,(220.0,125.0),1.5);
        assert_eq!((item.width,item.height),(300.0,150.0));
        assert_eq!((item.x,item.y),(70.0,50.0));
        rotate_around(&mut item,(220.0,125.0),std::f32::consts::FRAC_PI_2);
        assert!((item.rotation_deg-90.0).abs()<0.01);
        assert_eq!(item.path,"local.png");
    }
}
