use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransitionDirection {
    Opening,
    Closing,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Transition {
    pub(crate) started_at: Instant,
    pub(crate) duration: Duration,
    pub(crate) direction: TransitionDirection,
}

impl Transition {
    pub(crate) fn opening(duration: Duration, started_at: Instant) -> Self {
        Self {
            started_at,
            duration,
            direction: TransitionDirection::Opening,
        }
    }

    pub(crate) fn closing(duration: Duration, started_at: Instant) -> Self {
        Self {
            started_at,
            duration,
            direction: TransitionDirection::Closing,
        }
    }

    pub(crate) fn sample_at(self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started_at);
        let raw = if self.duration.is_zero() {
            1.0
        } else {
            (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
        };
        let eased = ease_out_cubic(raw);
        match self.direction {
            TransitionDirection::Opening => eased,
            TransitionDirection::Closing => 1.0 - eased,
        }
    }

    pub(crate) fn is_complete_at(self, now: Instant) -> bool {
        now.saturating_duration_since(self.started_at) >= self.duration
    }
}

pub(crate) fn ease_out_cubic(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    1.0 - (1.0 - value).powi(3)
}
