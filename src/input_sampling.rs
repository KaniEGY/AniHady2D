//! Lossless-enough pointer sampling for brush tools.
//!
//! egui exposes every pointer-move event that reached the frame. Reading only
//! `pointer.interact_pos()` drops all but the last one, which makes fast pen
//! strokes polygonal and can leave gaps on a busy frame. Keep the real event
//! path, but bound the amount of work a stalled frame can enqueue.

use eframe::egui;

// A 1000 Hz mouse usually delivers ~16 samples at 60 FPS. 48 leaves ample
// headroom for tablets while preventing a stalled/high-polling frame from
// spending a visible chunk of the next frame replaying old pointer events.
pub const MAX_POINTER_SAMPLES_PER_FRAME: usize = 48;

fn push_distinct(out: &mut Vec<egui::Pos2>, p: egui::Pos2) {
    if !p.x.is_finite() || !p.y.is_finite() {
        return;
    }
    if out.last().is_none_or(|last| last.distance_sq(p) >= 0.04) {
        out.push(p);
    }
}

fn bounded_evenly(mut points: Vec<egui::Pos2>, limit: usize) -> Vec<egui::Pos2> {
    let limit=limit.max(2);
    if points.len()<=limit {
        return points;
    }
    let last=points.len()-1;
    let mut out=Vec::with_capacity(limit);
    for i in 0..limit {
        let index=i*last/(limit-1);
        push_distinct(&mut out,points[index]);
    }
    if out.last().copied()!=points.last().copied() {
        if out.len()==limit {out.pop();}
        out.push(points.pop().unwrap());
    }
    out
}

#[derive(Default)]
pub struct PointerContact {
    pub press: Option<egui::Pos2>,
    pub samples: Vec<egui::Pos2>,
    pub finished: bool,
}

impl PointerContact {
    fn finish(mut self, finished: bool, out: &mut Vec<Self>) {
        self.samples = bounded_evenly(self.samples, MAX_POINTER_SAMPLES_PER_FRAME);
        self.finished = finished;
        out.push(self);
    }
}

/// A busy UI frame can contain the lift of one stroke and the press of another.
/// Keep the release coordinate with its own contact, even when a hover follows.
pub fn primary_contacts(
    events: &[egui::Event],
    continued: bool,
    primary_down: bool,
    current: Option<egui::Pos2>,
) -> Vec<PointerContact> {
    let mut contact = continued.then(PointerContact::default);
    let mut out=Vec::new();
    for event in events {
        match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => {
                if let Some(prior) = contact.take() {
                    prior.finish(true, &mut out);
                }
                let mut next = PointerContact { press: Some(*pos), ..Default::default() };
                push_distinct(&mut next.samples, *pos);
                contact = Some(next);
            }
            egui::Event::PointerMoved(pos) => {
                if let Some(active) = &mut contact {
                    push_distinct(&mut active.samples, *pos);
                }
            }
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(mut active) = contact.take() {
                    push_distinct(&mut active.samples, *pos);
                    active.finish(true, &mut out);
                }
            }
            egui::Event::PointerGone => {
                if let Some(active) = contact.take() {
                    active.finish(true, &mut out);
                }
            }
            _=>{}
        }
    }

    if let Some(mut active) = contact {
        if let Some(pos) = current.filter(|_| primary_down) {
            push_distinct(&mut active.samples, pos);
        }
        active.finish(!primary_down, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x:f32,y:f32)->egui::Pos2 {egui::pos2(x,y)}

    fn button(pos:egui::Pos2,pressed:bool)->egui::Event {
        egui::Event::PointerButton {
            pos,
            button:egui::PointerButton::Primary,
            pressed,
            modifiers:egui::Modifiers::default(),
        }
    }

    #[test]
    fn press_frame_ignores_hover_history_before_the_press() {
        let events=vec![
            egui::Event::PointerMoved(p(2.,2.)),
            egui::Event::PointerMoved(p(5.,5.)),
            button(p(10.,10.),true),
            egui::Event::PointerMoved(p(20.,10.)),
        ];
        let samples=primary_contacts(&events,false,true,Some(p(25.,10.))).remove(0).samples;
        assert_eq!(samples.first().copied(),Some(p(10.,10.)));
        assert_eq!(samples.last().copied(),Some(p(25.,10.)));
        assert!(!samples.contains(&p(2.,2.)));
    }

    #[test]
    fn held_pointer_keeps_all_distinct_moves_in_order() {
        let events=vec![
            egui::Event::PointerMoved(p(10.,0.)),
            egui::Event::PointerMoved(p(20.,1.)),
            egui::Event::PointerMoved(p(30.,1.)),
        ];
        assert_eq!(
            primary_contacts(&events,true,true,Some(p(40.,2.))).remove(0).samples,
            vec![p(10.,0.),p(20.,1.),p(30.,1.),p(40.,2.)]
        );
    }

    #[test]
    fn backlog_is_bounded_but_preserves_first_and_last_samples() {
        let events:Vec<_>=(0..500)
            .map(|i|egui::Event::PointerMoved(p(i as f32,0.0))).collect();
        let samples=primary_contacts(&events,true,true,Some(p(500.,0.))).remove(0).samples;
        assert!(samples.len()<=MAX_POINTER_SAMPLES_PER_FRAME);
        assert_eq!(samples.first().copied(),Some(p(0.,0.)));
        assert_eq!(samples.last().copied(),Some(p(500.,0.)));
    }

    #[test]
    fn duplicate_and_invalid_points_do_not_expand_the_backlog() {
        let events=vec![
            egui::Event::PointerMoved(p(10.,10.)),
            egui::Event::PointerMoved(p(10.01,10.01)),
            egui::Event::PointerMoved(p(f32::NAN,2.)),
        ];
        let samples=primary_contacts(&events,true,true,Some(p(20.,20.))).remove(0).samples;
        assert_eq!(samples,vec![p(10.,10.),p(20.,20.)]);
    }

    #[test]
    fn two_contacts_in_one_frame_ignore_the_hover_between_and_after_them() {
        let events = [
            egui::Event::PointerMoved(p(20.,10.)), button(p(40.,10.), false),
            egui::Event::PointerMoved(p(70.,30.)), button(p(10.,20.), true),
            egui::Event::PointerMoved(p(20.,20.)), button(p(40.,20.), false),
            egui::Event::PointerMoved(p(90.,50.)),
        ];
        let contacts = primary_contacts(&events, true, false, Some(p(90.,50.)));
        assert_eq!(contacts.len(), 2);
        assert!(contacts.iter().all(|c| c.finished));
        assert_eq!(contacts[0].samples, [p(20.,10.), p(40.,10.)]);
        assert_eq!(contacts[1].press, Some(p(10.,20.)));
        assert_eq!(contacts[1].samples, [p(10.,20.), p(20.,20.), p(40.,20.)]);
    }
}
