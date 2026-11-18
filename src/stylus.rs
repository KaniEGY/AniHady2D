//! Read pressure samples forwarded by the current egui backend.
//!
//! egui 0.29 exposes `Event::Touch { force }`, but NOT pressure for plain
//! mouse-pointer events. On a Windows tablet this is therefore best-effort:
//! if the window backend does not forward pen pressure, the brush remains
//! fully usable as a mouse with pressure 1.0.
//! We do not guess pressure from cursor speed or synthesize pen hardware data.

use eframe::egui;

const MAX_TOUCH_POINTER_DISTANCE: f32 = 64.0;
const MAX_STALE_PRESSURE_DISTANCE: f32 = 256.0;
const PRESSURE_NEW_SAMPLE_WEIGHT: f32 = 0.45;

#[derive(Default, Debug)]
pub struct TouchPressureTracker {
    pressure: Option<f32>,
    sample_position: Option<egui::Pos2>,
}

impl TouchPressureTracker {
    pub fn pressure_is_available(&self) -> bool {
        self.pressure.is_some()
    }

    pub fn reset(&mut self) {
        self.pressure = None;
        self.sample_position = None;
    }

    fn apply_touch_event(
        &mut self,
        phase: egui::TouchPhase,
        pos: egui::Pos2,
        force: Option<f32>,
        pointer_pos: egui::Pos2,
    ) {
        // Ignore unrelated touches. The pen is typically also emitted as
        // the primary pointer; pressure may be missing on some platforms.
        if !pos.x.is_finite()
            || !pos.y.is_finite()
            || pos.distance(pointer_pos) > MAX_TOUCH_POINTER_DISTANCE
        {
            return;
        }
        match phase {
            egui::TouchPhase::Start | egui::TouchPhase::Move => match force {
                Some(value) if value.is_finite() && (0.0..=1.0).contains(&value) => {
                    let raw = value.clamp(0.02, 1.0);
                    // Tablet backends can alternate between dense pressure
                    // samples and sparse pointer events. Smooth valid pressure
                    // samples instead of jumping between widths frame-to-frame.
                    let filtered = self.pressure
                        .map(|previous| previous * (1.0 - PRESSURE_NEW_SAMPLE_WEIGHT)
                            + raw * PRESSURE_NEW_SAMPLE_WEIGHT)
                        .unwrap_or(raw);
                    self.pressure = Some(filtered.clamp(0.02, 1.0));
                    self.sample_position = Some(pos);
                }
                _ => self.reset(),
            },
            egui::TouchPhase::End | egui::TouchPhase::Cancel => self.reset(),
        }
    }

    /// Process raw input once per frame, before drawing a brush point.
    /// Only pressure associated with the active primary pointer is accepted.
    /// The fallback remains 1.0 for all mice and unsupported tablets.
    pub fn update(
        &mut self,
        events: &[egui::Event],
        pointer_pos: Option<egui::Pos2>,
        pointer_down: bool,
    ) -> f32 {
        if !pointer_down {
            self.reset();
            return 1.0;
        }
        let Some(pointer) = pointer_pos.filter(|p| p.x.is_finite() && p.y.is_finite()) else {
            self.reset();
            return 1.0;
        };

        for event in events {
            if let egui::Event::Touch { phase, pos, force, .. } = event {
                self.apply_touch_event(*phase, *pos, *force, pointer);
            }
        }

        // If a pen stops delivering touch events while the emulated cursor
        // keeps moving, drop the old pressure instead of drawing stale-width
        // strokes across the canvas.
        if self
            .sample_position
            .is_some_and(|sample| sample.distance(pointer) > MAX_STALE_PRESSURE_DISTANCE)
        {
            self.reset();
        }
        self.pressure.unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(x, y)
    }

    #[test]
    fn mouse_defaults_to_full_pressure() {
        let mut tracker = TouchPressureTracker::default();
        assert_eq!(tracker.update(&[], Some(pos(10.0, 10.0)), true), 1.0);
        assert!(!tracker.pressure_is_available());
    }

    #[test]
    fn nearby_touch_samples_change_pressure() {
        let mut tracker = TouchPressureTracker::default();
        tracker.apply_touch_event(
            egui::TouchPhase::Start,
            pos(10.0, 10.0),
            Some(0.25),
            pos(12.0, 11.0),
        );
        assert_eq!(tracker.update(&[], Some(pos(12.0, 11.0)), true), 0.25);
        tracker.apply_touch_event(
            egui::TouchPhase::Move,
            pos(16.0, 10.0),
            Some(0.75),
            pos(16.0, 10.0),
        );
        assert_eq!(tracker.update(&[], Some(pos(16.0, 10.0)), true), 0.75);
    }

    #[test]
    fn far_or_invalid_touch_cannot_modify_pressure() {
        let mut tracker = TouchPressureTracker::default();
        tracker.apply_touch_event(egui::TouchPhase::Move, pos(200.0, 200.0), Some(0.1), pos(0.0, 0.0));
        assert_eq!(tracker.update(&[], Some(pos(0.0, 0.0)), true), 1.0);
        tracker.apply_touch_event(egui::TouchPhase::Start, pos(0.0, 0.0), Some(f32::NAN), pos(0.0, 0.0));
        assert_eq!(tracker.update(&[], Some(pos(0.0, 0.0)), true), 1.0);
        tracker.apply_touch_event(egui::TouchPhase::Move, pos(0.0, 0.0), Some(1.5), pos(0.0, 0.0));
        assert_eq!(tracker.update(&[], Some(pos(0.0, 0.0)), true), 1.0);
    }

    #[test]
    fn pressure_samples_are_smoothed_instead_of_width_jumping() {
        let mut tracker = TouchPressureTracker::default();
        tracker.apply_touch_event(
            egui::TouchPhase::Start, pos(10.0,10.0), Some(0.2), pos(10.0,10.0));
        tracker.apply_touch_event(
            egui::TouchPhase::Move, pos(12.0,10.0), Some(1.0), pos(12.0,10.0));
        let value = tracker.update(&[], Some(pos(12.0,10.0)), true);
        assert!(value > 0.2 && value < 1.0);
    }

    #[test]
    fn lifting_pen_or_moving_away_resets_pressure() {
        let mut tracker = TouchPressureTracker::default();
        tracker.apply_touch_event(egui::TouchPhase::Start, pos(3.0, 3.0), Some(0.2), pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(50.0, 50.0)), true), 1.0);
        tracker.apply_touch_event(egui::TouchPhase::Move, pos(3.0, 3.0), Some(0.5), pos(3.0, 3.0));
        tracker.apply_touch_event(egui::TouchPhase::End, pos(3.0, 3.0), None, pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(3.0, 3.0)), true), 1.0);
        tracker.apply_touch_event(egui::TouchPhase::Start, pos(3.0, 3.0), Some(0.5), pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(3.0, 3.0)), false), 1.0);
    }
}
