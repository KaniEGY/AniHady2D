//! Onion Skin frame selection and preview colors; no changes to saved project schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GhostSide { Previous, Next }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GhostFrame {
    pub frame: usize,
    pub side: GhostSide,
    pub distance: usize,
}

pub fn ghost_frames(
    current: usize, total: usize, before: usize, after: usize,
) -> Vec<GhostFrame> {
    if current == 0 || current > total { return Vec::new(); }
    let mut frames = Vec::new();
    // More distant ghost frames first, so the closest preview remains legible.
    for distance in (1..=before.min(12)).rev() {
        if let Some(frame) = current.checked_sub(distance).filter(|&f| f > 0) {
            frames.push(GhostFrame { frame, side: GhostSide::Previous, distance });
        }
    }
    for distance in (1..=after.min(12)).rev() {
        if let Some(frame) = current.checked_add(distance).filter(|&f| f <= total) {
            frames.push(GhostFrame { frame, side: GhostSide::Next, distance });
        }
    }
    frames
}

pub fn ghost_color(base: [u8; 4], distance: usize, fade_percent: f32) -> [u8; 4] {
    let distance = distance.clamp(1, 12);
    let keep = (1.0 - fade_percent.clamp(0.0, 95.0) / 100.0)
        .powi(distance.saturating_sub(1) as i32);
    let alpha = ((base[3] as f32) * keep).round().clamp(0.0, 255.0) as u8;
    [base[0], base[1], base[2], alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_one_only_shows_future_frames() {
        let frames = ghost_frames(1, 10, 2, 2);
        assert_eq!(frames.iter().map(|f| f.frame).collect::<Vec<_>>(), vec![3, 2]);
        assert!(frames.iter().all(|f| f.side == GhostSide::Next));
    }

    #[test]
    fn middle_frame_never_ghosts_itself() {
        let frames = ghost_frames(5, 9, 2, 2);
        assert_eq!(frames.iter().map(|f| f.frame).collect::<Vec<_>>(), vec![3, 4, 7, 6]);
        assert!(!frames.iter().any(|f| f.frame == 5));
    }

    #[test]
    fn end_and_invalid_frame_are_safe() {
        assert_eq!(ghost_frames(4, 4, 2, 3).len(), 2);
        assert!(ghost_frames(0, 10, 2, 2).is_empty());
        assert!(ghost_frames(11, 10, 2, 2).is_empty());
    }

    #[test]
    fn zero_opacity_stays_invisible_at_any_distance() {
        assert_eq!(ghost_color([255, 80, 80, 0], 1, 25.0)[3], 0);
        assert_eq!(ghost_color([255, 80, 80, 0], 12, 25.0)[3], 0);
    }

    #[test]
    fn distant_ghost_is_fainter() {
        assert!(ghost_color([255, 80, 80, 90], 2, 25.0)[3]
            < ghost_color([255, 80, 80, 90], 1, 25.0)[3]);
    }

    #[test]
    fn fade_amount_is_user_controllable() {
        let base=[255,80,80,100];
        assert_eq!(ghost_color(base,4,0.0)[3],100);
        assert!(ghost_color(base,4,60.0)[3] < ghost_color(base,4,20.0)[3]);
    }
}
