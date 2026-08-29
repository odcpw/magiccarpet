//! Source-faithful dynamic-music control envelope.
//!
//! This is presentation control state, not an audio renderer. MC2's
//! `UpdateMusic_99970(mode, multiplier)` schedules `FadeWarMusic_99830` at
//! `30 * multiplier` callbacks per second for multipliers 1..=4 (30 Hz
//! otherwise). Each callback moves the integer control value by exactly one.

pub const DYNAMIC_MUSIC_MIN: u8 = 0;
pub const DYNAMIC_MUSIC_MAX: u8 = 127;
pub const DYNAMIC_MUSIC_IDLE_MODE: u8 = 1;
pub const DYNAMIC_MUSIC_COMBAT_MODE: u8 = 2;
pub const LEVEL1_TIMER_MULTIPLIER: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FadeCallback {
    Inactive,
    Advanced(u8),
    Retired(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicMusicEnvelope {
    pub updates_enabled: bool,
    pub timer_active: bool,
    pub target_mode: u8,
    pub current: u8,
    pub direction: i8,
    pub timer_multiplier: u8,
}

impl Default for DynamicMusicEnvelope {
    fn default() -> Self {
        Self {
            updates_enabled: false,
            timer_active: false,
            target_mode: DYNAMIC_MUSIC_IDLE_MODE,
            current: DYNAMIC_MUSIC_MIN,
            direction: -1,
            timer_multiplier: LEVEL1_TIMER_MULTIPLIER,
        }
    }
}

impl DynamicMusicEnvelope {
    pub fn start(&mut self) {
        *self = Self {
            updates_enabled: true,
            ..Self::default()
        };
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    pub const fn timer_hz(multiplier: u8) -> u16 {
        if multiplier >= 1 && multiplier <= 4 {
            30 * multiplier as u16
        } else {
            30
        }
    }

    /// Mirrors the source guard and direction reversal. Repeated requests for
    /// the current target are idempotent and do not restart the timer.
    pub fn update(&mut self, mode: u8, timer_multiplier: u8) -> bool {
        if !self.updates_enabled
            || (mode != DYNAMIC_MUSIC_IDLE_MODE && mode != DYNAMIC_MUSIC_COMBAT_MODE)
            || self.target_mode == mode
        {
            return false;
        }
        self.target_mode = mode;
        self.direction = -self.direction;
        self.timer_active = true;
        self.timer_multiplier = timer_multiplier;
        true
    }

    /// The source checks the endpoint before applying the next +/-1 step.
    /// Consequently the 127th upward callback reaches 127 and the 128th
    /// callback retires the timer.
    pub fn callback(&mut self) -> FadeCallback {
        if !self.timer_active {
            return FadeCallback::Inactive;
        }
        let at_target = (self.current == DYNAMIC_MUSIC_MAX
            && self.target_mode == DYNAMIC_MUSIC_COMBAT_MODE)
            || (self.current == DYNAMIC_MUSIC_MIN && self.target_mode == DYNAMIC_MUSIC_IDLE_MODE);
        if at_target {
            self.timer_active = false;
            return FadeCallback::Retired(self.current);
        }
        self.current = if self.direction > 0 {
            self.current.wrapping_add(1)
        } else {
            self.current.wrapping_sub(1)
        };
        FadeCallback::Advanced(self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplier_three_is_the_source_90_hz_rate_not_a_group() {
        assert_eq!(DynamicMusicEnvelope::timer_hz(LEVEL1_TIMER_MULTIPLIER), 90);
        assert_eq!(DynamicMusicEnvelope::timer_hz(0), 30);
        assert_eq!(DynamicMusicEnvelope::timer_hz(5), 30);
    }

    #[test]
    fn start_and_stop_restore_the_source_control_state() {
        let mut envelope = DynamicMusicEnvelope::default();
        assert!(!envelope.updates_enabled);
        envelope.start();
        assert!(envelope.updates_enabled);
        assert_eq!(envelope.current, 0);
        assert_eq!(envelope.target_mode, 1);
        assert_eq!(envelope.direction, -1);
        assert!(!envelope.timer_active);
        envelope.stop();
        assert_eq!(envelope, DynamicMusicEnvelope::default());
    }

    #[test]
    fn upward_ramp_reaches_127_then_retires_on_the_following_callback() {
        let mut envelope = DynamicMusicEnvelope::default();
        envelope.start();
        assert!(envelope.update(2, 3));
        assert_eq!(envelope.direction, 1);
        for expected in 1..=127 {
            assert_eq!(envelope.callback(), FadeCallback::Advanced(expected));
        }
        assert!(envelope.timer_active);
        assert_eq!(envelope.callback(), FadeCallback::Retired(127));
        assert!(!envelope.timer_active);
    }

    #[test]
    fn repeat_is_idempotent_and_mid_ramp_reversal_keeps_current_value() {
        let mut envelope = DynamicMusicEnvelope::default();
        envelope.start();
        assert!(envelope.update(2, 3));
        for expected in 1..=19 {
            assert_eq!(envelope.callback(), FadeCallback::Advanced(expected));
        }
        assert!(!envelope.update(2, 3));
        assert_eq!(envelope.current, 19);
        assert!(envelope.update(1, 3));
        assert_eq!(envelope.direction, -1);
        assert_eq!(envelope.callback(), FadeCallback::Advanced(18));
    }

    #[test]
    fn disabled_or_invalid_updates_do_not_arm_a_timer() {
        let mut envelope = DynamicMusicEnvelope::default();
        assert!(!envelope.update(2, 3));
        envelope.start();
        assert!(!envelope.update(0, 3));
        assert!(!envelope.update(3, 3));
        assert_eq!(envelope.callback(), FadeCallback::Inactive);
    }
}
