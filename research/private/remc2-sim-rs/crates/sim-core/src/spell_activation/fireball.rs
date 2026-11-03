//! Source `sub_693F0` timing. Arena allocation precedes the returned charge.

#[derive(Clone, Copy, Debug)]
pub(crate) struct Input {
    pub active: i16,
    pub duration: i16,
    pub timer: u16,
    pub parent_present: bool,
    pub gate: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Output {
    pub launch: bool,
    pub charge: bool,
    pub active: i16,
    pub timer: u16,
    pub expired: bool,
}

pub(crate) fn step(input: Input) -> Output {
    let admitted = input.active > 0 && input.parent_present && input.gate;
    let mut active = input.active;
    if active > 0 {
        if input.parent_present && !input.gate {
            active = 1;
        }
        active = active.wrapping_sub(1);
    }
    Output {
        launch: admitted && input.active == input.duration,
        charge: admitted,
        active,
        timer: input.timer.saturating_sub(1),
        expired: input.active > 0 && active == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{Input, step};

    #[test]
    fn missing_parent_counts_down_and_rejected_parent_expires_without_launch() {
        let base = Input {
            active: 5,
            duration: 5,
            timer: 2,
            parent_present: false,
            gate: true,
        };
        let missing = step(base);
        assert_eq!(
            (
                missing.active,
                missing.timer,
                missing.launch,
                missing.charge
            ),
            (4, 1, false, false)
        );
        let rejected = step(Input {
            parent_present: true,
            gate: false,
            ..base
        });
        assert_eq!(
            (
                rejected.active,
                rejected.launch,
                rejected.charge,
                rejected.expired
            ),
            (0, false, false, true)
        );
        assert!(
            step(Input {
                parent_present: true,
                ..base
            })
            .launch
        );
        assert!(
            !step(Input {
                active: 4,
                parent_present: true,
                ..base
            })
            .launch
        );
    }
}
