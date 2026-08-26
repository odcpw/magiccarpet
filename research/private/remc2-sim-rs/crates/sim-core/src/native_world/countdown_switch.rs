//! Shared class-11 absence-countdown state machine (`sub_6F300`).

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CountdownSwitchStep {
    Frozen,
    Armed,
    Decremented,
    Fire,
}

/// Advance the source dword-0x10 timer only while the rebuilt family
/// membership is empty. The scheduler owns RNG, phase and following reclaim;
/// the caller owns sound, synchronous authored-group activation and hide.
pub(super) fn step_absence_countdown(
    timer: &mut i32,
    blocking_family_present: bool,
) -> CountdownSwitchStep {
    if blocking_family_present {
        return CountdownSwitchStep::Frozen;
    }
    match *timer {
        0 => {
            *timer = 16;
            CountdownSwitchStep::Armed
        }
        1 => CountdownSwitchStep::Fire,
        _ => {
            *timer = timer.wrapping_sub(1);
            CountdownSwitchStep::Decremented
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cf14_absence_countdown_freezes_arms_and_fires_on_seventeenth_clear_visit() {
        let mut timer = 0;
        assert_eq!(
            step_absence_countdown(&mut timer, true),
            CountdownSwitchStep::Frozen
        );
        assert_eq!(timer, 0);
        assert_eq!(
            step_absence_countdown(&mut timer, false),
            CountdownSwitchStep::Armed
        );
        assert_eq!(timer, 16);
        for expected in (1..=15).rev() {
            assert_eq!(
                step_absence_countdown(&mut timer, false),
                CountdownSwitchStep::Decremented
            );
            assert_eq!(timer, expected);
        }
        assert_eq!(
            step_absence_countdown(&mut timer, true),
            CountdownSwitchStep::Frozen
        );
        assert_eq!(timer, 1);
        assert_eq!(
            step_absence_countdown(&mut timer, false),
            CountdownSwitchStep::Fire
        );
        assert_eq!(timer, 1);
    }
}
