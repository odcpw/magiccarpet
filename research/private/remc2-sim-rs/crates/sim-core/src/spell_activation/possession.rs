//! Source Possession holder timing, independent of the caster and arena.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Input {
    pub active: i16,
    pub duration: i16,
    pub marker: i8,
    pub tier: i8,
    pub life: u8,
    pub timer: u16,
    pub parent_present: bool,
    pub gate: bool,
    pub parent_regen: i32,
    pub cost: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Output {
    /// Source life selector for the requested shot; zero is a basic shot.
    pub launch: Option<u8>,
    pub active: i16,
    pub marker: i8,
    pub timer: u16,
    pub parent_regen: Option<i32>,
    pub expired: bool,
}

/// `sub_69640`: allocation is attempted before the returned charge is applied.
/// Missing parent still counts down; a failed source gate ends the cast now.
pub(crate) fn step(input: Input) -> Output {
    let mut out = Output {
        launch: None,
        active: input.active,
        marker: input.marker,
        timer: input.timer.saturating_sub(1),
        parent_regen: None,
        expired: false,
    };
    if input.active <= 0 {
        return out;
    }
    if input.parent_present {
        if !input.gate {
            out.active = 1;
        } else {
            let fresh = input.active == input.duration;
            if fresh {
                if input.life <= 3 {
                    out.launch = Some(input.life);
                }
                out.parent_regen = Some(if input.parent_regen >= 0 {
                    input.cost.wrapping_neg()
                } else {
                    input.parent_regen.wrapping_sub(input.cost)
                });
            } else {
                if out.marker != 0 {
                    if input.tier != 0 {
                        if out.marker == 1 {
                            out.launch = Some(0);
                        }
                        out.marker = out.marker.wrapping_add(1);
                        if out.marker > 3 {
                            out.marker = 0;
                        }
                    } else {
                        out.marker = 0;
                    }
                }
                if input.parent_regen > 0 {
                    out.parent_regen = Some(0);
                }
            }
        }
    }
    out.active = out.active.wrapping_sub(1);
    out.expired = out.active == 0;
    out
}

#[cfg(test)]
mod tests {
    use super::{Input, step};

    #[test]
    fn source_pulses_charge_and_count_down_with_missing_parent() {
        let base = Input {
            active: 5,
            duration: 5,
            marker: 0,
            tier: 2,
            life: 2,
            timer: 2,
            parent_present: true,
            gate: true,
            parent_regen: -10,
            cost: 100,
        };
        let first = step(base);
        assert_eq!(
            (first.launch, first.active, first.parent_regen),
            (Some(2), 4, Some(-110))
        );
        let repeat = step(Input {
            active: 4,
            marker: 1,
            parent_regen: 5,
            ..base
        });
        assert_eq!(
            (repeat.launch, repeat.marker, repeat.parent_regen),
            (Some(0), 2, Some(0))
        );
        let no_parent = step(Input {
            active: 1,
            parent_present: false,
            ..base
        });
        assert_eq!(
            (no_parent.launch, no_parent.active, no_parent.expired),
            (None, 0, true)
        );
        let failed_gate = step(Input {
            gate: false,
            ..base
        });
        assert_eq!(
            (
                failed_gate.launch,
                failed_gate.active,
                failed_gate.parent_regen
            ),
            (None, 0, None)
        );
        assert_eq!(step(Input { life: 3, ..base }).launch, Some(3));
        assert_eq!(step(Input { life: 4, ..base }).launch, None);
    }

    #[test]
    fn missing_parent_does_not_freeze_or_charge_holder() {
        let mut input = Input {
            active: 3,
            duration: 3,
            marker: 1,
            tier: 1,
            life: 1,
            timer: 2,
            parent_present: false,
            gate: false,
            parent_regen: 30,
            cost: 250,
        };
        for expected in [2, 1, 0] {
            let output = step(input);
            assert_eq!(output.active, expected);
            assert_eq!(
                (output.launch, output.parent_regen, output.marker),
                (None, None, 1)
            );
            assert_eq!(output.expired, expected == 0);
            input.active = output.active;
            input.timer = output.timer;
        }
        assert_eq!(input.timer, 0);
    }
}
