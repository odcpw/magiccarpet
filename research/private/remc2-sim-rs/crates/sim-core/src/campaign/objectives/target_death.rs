//! Source-compatible predicates and target binding for objective forms 1/2.
//!
//! remc2 stores these stages as authored-entity references. The campaign
//! objective state and the runtime stage relation remain their canonical
//! carriers; this module only translates the authored target and evaluates the
//! narrow observation consumed by the objective service.

use crate::native_world::{EntitySnapshot, RuntimeRelation, RuntimeStage};

/// Read the entity row attached by the source construction/conversion pass.
/// Dead rows remain readable until source reclaim; an unbound or wrongly
/// typed stage cannot borrow another stage form's target.
pub(crate) fn achieved_in_arena(
    form: Form,
    stage: &RuntimeStage,
    entities: &[EntitySnapshot],
) -> bool {
    if Form::from_source_stage_type(stage.kind) != Some(form) || stage.flags & 1 == 0 {
        return false;
    }
    let RuntimeRelation::LiveEntity(slot) = stage.relation else {
        return false;
    };
    achieved(
        form,
        entities
            .get(usize::from(slot))
            .filter(|entity| entity.index == slot)
            .map(|entity| TargetObservation {
                life: entity.life,
                font_type_index: entity.font_type_index,
            }),
    )
}

/// The two remc2 target-death objective forms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Form {
    /// Source stage type 1: the bound target's life reaches -1 or less.
    TargetDeath,
    /// Source stage type 2: the target is dead and its font type is zero.
    TargetDeathWithFontTypeZero,
}

impl Form {
    /// Decode the source stage type used by `InitStages_58940`.
    #[must_use]
    pub(crate) const fn from_source_stage_type(stage_type: i8) -> Option<Self> {
        match stage_type {
            1 => Some(Self::TargetDeath),
            2 => Some(Self::TargetDeathWithFontTypeZero),
            _ => None,
        }
    }

    /// Whether `sub_59760` moves this form's target to a converted entity.
    #[must_use]
    pub(crate) const fn follows_converted_target(self) -> bool {
        matches!(self, Self::TargetDeathWithFontTypeZero)
    }
}

/// Only the two fields read by the source target-death predicates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TargetObservation {
    pub(crate) life: i32,
    pub(crate) font_type_index: u8,
}

/// Evaluate one bound target. `None` represents a target that has not yet
/// been attached to a runtime entity, which cannot satisfy either form.
#[must_use]
pub(crate) const fn achieved(form: Form, target: Option<TargetObservation>) -> bool {
    let Some(target) = target else {
        return false;
    };
    if target.life > -1 {
        return false;
    }
    match form {
        Form::TargetDeath => true,
        Form::TargetDeathWithFontTypeZero => target.font_type_index == 0,
    }
}

/// The target named by an authored source objective row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthoredTargetBinding {
    form: Form,
    authored_entity_index: u16,
}

impl AuthoredTargetBinding {
    /// Bind source stage types 1/2 to the authored entity selected by
    /// `stage_1`. Negative selectors are invalid authored entity references.
    #[must_use]
    pub(crate) fn from_source_row(stage_type: i8, authored_entity_selector: i16) -> Option<Self> {
        Some(Self {
            form: Form::from_source_stage_type(stage_type)?,
            authored_entity_index: u16::try_from(authored_entity_selector).ok()?,
        })
    }

    #[must_use]
    pub(crate) const fn form(self) -> Form {
        self.form
    }

    #[must_use]
    pub(crate) const fn authored_entity_index(self) -> u16 {
        self.authored_entity_index
    }

    /// `sub_58DA0` attaches the runtime entity produced from the referenced
    /// authored entity. Other authored records cannot bind this target.
    #[must_use]
    pub(crate) const fn bind_materialized_entity(
        self,
        materialized_authored_entity_index: u16,
        runtime_entity_index: u16,
    ) -> Option<u16> {
        if materialized_authored_entity_index == self.authored_entity_index {
            Some(runtime_entity_index)
        } else {
            None
        }
    }

    /// `sub_59760` follows a conversion only for an open, nonterminal type-2
    /// objective whose currently bound entity is the converted source.
    #[must_use]
    pub(crate) const fn replacement_runtime_entity(
        self,
        objective_stage_selector: u8,
        objective_is_level_end: bool,
        bound_runtime_entity_index: Option<u16>,
        converted_runtime_entity_index: u16,
        replacement_runtime_entity_index: u16,
    ) -> Option<u16> {
        if self.form.follows_converted_target()
            && !objective_is_level_end
            && objective_stage_selector == 1
            && matches!(
                bound_runtime_entity_index,
                Some(bound_index) if bound_index == converted_runtime_entity_index
            )
        {
            Some(replacement_runtime_entity_index)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthoredTargetBinding, Form, TargetObservation, achieved};

    #[test]
    fn target_death_forms_keep_their_distinct_source_gates() {
        assert!(achieved(
            Form::TargetDeath,
            Some(TargetObservation {
                life: -1,
                font_type_index: 7,
            }),
        ));
        assert!(!achieved(
            Form::TargetDeath,
            Some(TargetObservation {
                life: 0,
                font_type_index: 0,
            }),
        ));
        assert!(achieved(
            Form::TargetDeathWithFontTypeZero,
            Some(TargetObservation {
                life: -2,
                font_type_index: 0,
            }),
        ));
        assert!(!achieved(
            Form::TargetDeathWithFontTypeZero,
            Some(TargetObservation {
                life: -2,
                font_type_index: 1,
            }),
        ));
        assert!(!achieved(Form::TargetDeathWithFontTypeZero, None));
    }

    #[test]
    fn authored_target_binding_and_form_two_replacement_are_exact() {
        let form_one = AuthoredTargetBinding::from_source_row(1, 17).unwrap();
        let form_two = AuthoredTargetBinding::from_source_row(2, 17).unwrap();
        assert_eq!(form_one.form(), Form::TargetDeath);
        assert_eq!(form_two.authored_entity_index(), 17);
        assert_eq!(form_two.bind_materialized_entity(17, 42), Some(42));
        assert_eq!(form_two.bind_materialized_entity(18, 42), None);

        assert_eq!(
            form_two.replacement_runtime_entity(1, false, Some(42), 42, 43),
            Some(43)
        );
        assert_eq!(
            form_two.replacement_runtime_entity(1, false, Some(44), 42, 43),
            None
        );
        assert_eq!(
            form_two.replacement_runtime_entity(2, false, Some(42), 42, 43),
            None
        );
        assert_eq!(
            form_two.replacement_runtime_entity(1, true, Some(42), 42, 43),
            None
        );
        assert_eq!(
            form_one.replacement_runtime_entity(1, false, Some(42), 42, 43),
            None
        );
        assert!(AuthoredTargetBinding::from_source_row(1, -1).is_none());
        assert!(AuthoredTargetBinding::from_source_row(3, 17).is_none());
    }
}
