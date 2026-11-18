//! Ephemeral stage-space pointers. Each room session owns a fresh set.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const SEND_INTERVAL: Duration = Duration::from_millis(50);
const KEEPALIVE: Duration = Duration::from_secs(2);
const EXPIRE_AFTER: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: f32,
    pub y: f32,
    pub frame: usize,
}

impl CursorPosition {
    pub fn valid(self) -> bool {
        (0.0..=16384.0).contains(&self.x)
            && (0.0..=16384.0).contains(&self.y)
            && (1..=20000).contains(&self.frame)
    }
}

#[derive(Default)]
pub struct CursorSender {
    last: Option<CursorPosition>,
    sent_at: Option<Instant>,
}

impl CursorSender {
    pub fn should_send(&mut self, position: Option<CursorPosition>, now: Instant) -> bool {
        if position.is_some_and(|p| !p.valid()) {
            return false;
        }
        let elapsed = self
            .sent_at
            .map_or(Duration::MAX, |sent| now.saturating_duration_since(sent));
        if elapsed < SEND_INTERVAL
            || (position == self.last && (position.is_none() || elapsed < KEEPALIVE))
        {
            return false;
        }
        self.last = position;
        self.sent_at = Some(now);
        true
    }
}

struct Motion {
    from: CursorPosition,
    target: CursorPosition,
    received: Instant,
    duration: Duration,
}

impl Motion {
    fn sample(&self, now: Instant) -> CursorPosition {
        let elapsed = now.saturating_duration_since(self.received);
        if elapsed >= self.duration {
            return self.target;
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        CursorPosition {
            x: self.from.x + (self.target.x - self.from.x) * t,
            y: self.from.y + (self.target.y - self.from.y) * t,
            frame: self.target.frame,
        }
    }
}

#[derive(Default)]
pub struct RemoteCursors {
    motions: HashMap<u64, Motion>,
}

impl RemoteCursors {
    pub fn update(&mut self, id: u64, position: Option<CursorPosition>, received: Instant) {
        let Some(target) = position.filter(|p| p.valid()) else {
            self.motions.remove(&id);
            return;
        };
        let previous = self.motions.get(&id).filter(|old| {
            target.frame == old.target.frame
                && received.saturating_duration_since(old.received) < EXPIRE_AFTER
        });
        let (from, duration) = previous.map_or((target, Duration::ZERO), |old| {
            (
                old.sample(received),
                received
                    .saturating_duration_since(old.received)
                    .clamp(Duration::from_millis(25), Duration::from_millis(90)),
            )
        });
        self.motions.insert(
            id,
            Motion {
                from,
                target,
                received,
                duration,
            },
        );
    }

    pub fn retain_members(&mut self, mut present: impl FnMut(u64) -> bool) {
        self.motions.retain(|id, _| present(*id));
    }

    pub fn position(&self, id: u64, now: Instant) -> Option<CursorPosition> {
        let motion = self.motions.get(&id)?;
        (now.saturating_duration_since(motion.received) < EXPIRE_AFTER).then(|| motion.sample(now))
    }

    pub fn moving(&self, now: Instant) -> bool {
        self.motions.values().any(|motion| {
            motion.from != motion.target
                && now.saturating_duration_since(motion.received) < motion.duration + SEND_INTERVAL
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, frame: usize) -> CursorPosition {
        CursorPosition { x, y: 4.0, frame }
    }

    #[test]
    fn throttles_moves_and_sends_latest_position_keepalive_and_leave() {
        let now = Instant::now();
        let mut sender = CursorSender::default();
        assert!(!sender.should_send(None, now));
        assert!(sender.should_send(Some(point(1.0, 1)), now));
        assert!(!sender.should_send(Some(point(2.0, 1)), now + SEND_INTERVAL / 2));
        assert!(sender.should_send(Some(point(3.0, 1)), now + SEND_INTERVAL));
        assert!(!sender.should_send(Some(point(3.0, 1)), now + SEND_INTERVAL * 2));
        let heartbeat = now + SEND_INTERVAL + KEEPALIVE;
        assert!(sender.should_send(Some(point(3.0, 1)), heartbeat));
        assert!(sender.should_send(None, heartbeat + SEND_INTERVAL));
        assert!(!sender.should_send(None, heartbeat + KEEPALIVE));
    }

    #[test]
    fn interpolates_without_overshoot_and_clears_stale_or_departed_participants() {
        let now = Instant::now();
        let mut cursors = RemoteCursors::default();
        cursors.update(7, Some(point(1.0, 1)), now);
        let moved = now + SEND_INTERVAL;
        cursors.update(7, Some(point(11.0, 1)), moved);
        assert_eq!(
            cursors.position(7, moved + SEND_INTERVAL / 2),
            Some(point(6.0, 1))
        );
        assert_eq!(
            cursors.position(7, moved + SEND_INTERVAL * 2),
            Some(point(11.0, 1))
        );
        assert!(cursors.position(7, moved + EXPIRE_AFTER).is_none());
        cursors.update(7, Some(point(50.0, 2)), moved + SEND_INTERVAL);
        assert_eq!(
            cursors.position(7, moved + SEND_INTERVAL),
            Some(point(50.0, 2))
        );
        cursors.update(8, Some(point(42.0, 1)), moved);
        cursors.retain_members(|id| id == 8);
        assert!(cursors.position(7, moved + SEND_INTERVAL).is_none());
        cursors.update(8, None, moved + SEND_INTERVAL);
        assert!(cursors.position(8, moved + SEND_INTERVAL).is_none());
        assert!(!point(f32::NAN, 1).valid());
        assert!(!point(10.0, 0).valid());
    }
}
