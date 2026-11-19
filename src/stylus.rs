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
    /// Latched only for the current contact. Once a real pressure sample
    /// has appeared, a later missing/zero sample is pen lift, not a mouse.
    sensed_this_contact: bool,
}

impl TouchPressureTracker {
    pub fn pressure_is_available(&self) -> bool {
        self.sensed_this_contact
    }

    pub fn reset(&mut self) {
        self.pressure = None;
        self.sample_position = None;
        self.sensed_this_contact = false;
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
                    self.sensed_this_contact = true;
                    self.sample_position = Some(pos);
                    if value <= 0.001 {
                        // A real zero means the stylus is leaving the surface.
                        // Keep the pressure channel active but do not turn the
                        // missing sample into a full-pressure mouse dab.
                        self.pressure = Some(0.0);
                    } else {
                        let raw = value.clamp(0.02, 1.0);
                        let filtered = self.pressure
                            .filter(|previous| *previous > 0.0)
                            .map(|previous| previous * (1.0 - PRESSURE_NEW_SAMPLE_WEIGHT)
                                + raw * PRESSURE_NEW_SAMPLE_WEIGHT)
                            .unwrap_or(raw);
                        self.pressure = Some(filtered.clamp(0.02, 1.0));
                    }
                }
                // Windows/driver stacks can drop the final pressure reading
                // while the pointer is still technically down. Once this
                // contact has proven it is pressure-capable, absence means
                // lift-off rather than "mouse at pressure 1".
                None if self.sensed_this_contact => {
                    self.pressure = Some(0.0);
                    self.sample_position = Some(pos);
                }
                _ if self.sensed_this_contact => {
                    self.pressure = Some(0.0);
                    self.sample_position = Some(pos);
                }
                _ => {}
            },
            egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                if self.sensed_this_contact {
                    self.pressure = Some(0.0);
                    self.sample_position = Some(pos);
                } else {
                    self.reset();
                }
            },
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
        if self.sensed_this_contact {
            self.pressure.unwrap_or(0.0)
        } else {
            1.0
        }
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
        let smoothed = tracker.update(&[], Some(pos(16.0, 10.0)), true);
        assert!(smoothed > 0.25 && smoothed < 0.75);
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
    fn missing_pressure_after_real_pen_sample_is_lift_not_full_mouse_pressure() {
        let mut tracker=TouchPressureTracker::default();
        tracker.apply_touch_event(
            egui::TouchPhase::Start,pos(10.0,10.0),Some(0.45),pos(10.0,10.0));
        assert!(tracker.pressure_is_available());
        tracker.apply_touch_event(
            egui::TouchPhase::Move,pos(12.0,10.0),None,pos(12.0,10.0));
        assert_eq!(tracker.update(&[],Some(pos(12.0,10.0)),true),0.0);
        assert!(tracker.pressure_is_available());
    }

    #[test]
    fn a_new_mouse_contact_after_pen_lift_returns_to_full_pressure() {
        let mut tracker=TouchPressureTracker::default();
        tracker.apply_touch_event(
            egui::TouchPhase::Start,pos(4.0,4.0),Some(0.3),pos(4.0,4.0));
        tracker.update(&[],Some(pos(4.0,4.0)),false);
        assert_eq!(tracker.update(&[],Some(pos(8.0,8.0)),true),1.0);
        assert!(!tracker.pressure_is_available());
    }

    #[test]
    fn lifting_pen_or_moving_away_resets_pressure() {
        let mut tracker = TouchPressureTracker::default();
        tracker.apply_touch_event(egui::TouchPhase::Start, pos(3.0, 3.0), Some(0.2), pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(400.0, 400.0)), true), 1.0);
        tracker.apply_touch_event(egui::TouchPhase::Move, pos(3.0, 3.0), Some(0.5), pos(3.0, 3.0));
        tracker.apply_touch_event(egui::TouchPhase::End, pos(3.0, 3.0), None, pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(3.0, 3.0)), true), 0.0);
        tracker.apply_touch_event(egui::TouchPhase::Start, pos(3.0, 3.0), Some(0.5), pos(3.0, 3.0));
        assert_eq!(tracker.update(&[], Some(pos(3.0, 3.0)), false), 1.0);
    }
}
