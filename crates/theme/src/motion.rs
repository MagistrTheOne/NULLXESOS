//! NULLXES animation timing — functional, never decorative.

use std::time::Duration;

/// All animation durations in the system.
#[derive(Debug, Clone, Copy)]
pub struct Durations;

impl Durations {
    /// No visual transition — state changes that are instant by design.
    pub const INSTANT:  Duration = Duration::ZERO;
    /// Button press feedback, cursor state change.
    pub const MICRO:    Duration = Duration::from_millis(80);
    /// Panel open/close, notification appear.
    pub const FAST:     Duration = Duration::from_millis(120);
    /// Window open/close, workspace switch.
    pub const STANDARD: Duration = Duration::from_millis(200);
    /// Overlay, modal appearance.
    pub const SLOW:     Duration = Duration::from_millis(350);
}

/// Easing curve discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Things appearing/expanding — fast start, decelerates to rest.
    EaseOutExpo,
    /// Things disappearing/contracting — slow start, fast end.
    EaseInExpo,
    /// Progress bars and loaders only.
    Linear,
}

impl Easing {
    /// Evaluate t ∈ [0, 1] → output ∈ [0, 1].
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOutExpo => {
                if t >= 1.0 { 1.0 } else { 1.0 - 2.0_f32.powf(-10.0 * t) }
            }
            Easing::EaseInExpo => {
                if t <= 0.0 { 0.0 } else { 2.0_f32.powf(10.0 * t - 10.0) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_expo_boundaries() {
        assert_eq!(Easing::EaseOutExpo.eval(0.0), 0.0);
        assert_eq!(Easing::EaseOutExpo.eval(1.0), 1.0);
    }

    #[test]
    fn ease_in_expo_boundaries() {
        assert_eq!(Easing::EaseInExpo.eval(0.0), 0.0);
        assert_eq!(Easing::EaseInExpo.eval(1.0), 1.0);
    }
}
