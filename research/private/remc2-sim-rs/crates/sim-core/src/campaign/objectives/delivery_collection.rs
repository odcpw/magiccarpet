//! Source-compatible target binding and predicates for objective forms 4 and 6.
//!
//! These forms use the authored entity selected by `stage_1`, then the live
//! entity index produced by the source binding pass. Collection state is read
//! from the owning player's eight-slot source list; this leaf does not invent
//! or synthesize collection authority.

use crate::native_world::{RuntimeRelation, RuntimeStage};

/// The source objective conditions implemented by this leaf.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Form {
    /// Source type 4: the active player owns the bound entity at its XY target.
    DeliverOwnedEntity,
    /// Source type 6: the current objective's bound entity is in the player's list.
    CollectEntity,
}

impl Form {
    /// Decode one source stage type owned by this module.
    #[must_use]
    pub(crate) const fn from_source_stage_type(stage_type: i8) -> Option<Self> {
        match stage_type {
            4 => Some(Self::DeliverOwnedEntity),
            6 => Some(Self::CollectEntity),
            _ => None,
        }
    }
}

/// Source `Type_str_0x1AC_428.word_2BDE_12658` has eight signed slots.
pub(crate) const COLLECTED_ENTITY_SLOT_COUNT: usize = 8;

/// The authored row named by a source objective's `stage_1` selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthoredTargetBinding {
    form: Form,
    authored_entity_index: u16,
}

impl AuthoredTargetBinding {
    /// Bind source forms 4/6 to their referenced authored entity row.
    ///
    /// Negative selectors do not identify an authored entity and are rejected.
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

    /// Match `sub_58DA0`'s authored-row to materialized-live-entity binding.
    ///
    /// Runtime entity zero is the null/sentinel slot and cannot satisfy either
    /// predicate, even when the authored row selector itself is zero.
    #[must_use]
    pub(crate) const fn bind_materialized_entity(
        self,
        materialized_authored_entity_index: u16,
        runtime_entity_index: u16,
    ) -> Option<u16> {
        if materialized_authored_entity_index == self.authored_entity_index
            && runtime_entity_index != 0
        {
            Some(runtime_entity_index)
        } else {
            None
        }
    }
}

/// Check source type 4 for one active stage.
///
/// Unlike type 6, type 4 is not gated by the objective text selector. The
/// source uses the bound target's owner at offset `0x28` and signed XY
/// coordinates, each within 768 source units of the stage destination.
#[must_use]
pub(crate) fn delivered_entity_to_point(
    stage: &RuntimeStage,
    expected_runtime_entity_index: u16,
    target_owner_entity_index: u16,
    active_player_entity_index: u16,
    target_position: (i16, i16),
) -> bool {
    stage.kind == 4
        && bound_target_matches(stage, expected_runtime_entity_index)
        && active_player_entity_index != 0
        && target_owner_entity_index == active_player_entity_index
        && (i32::from(stage.axis_x) - i32::from(target_position.0)).abs() <= 768
        && (i32::from(stage.axis_y) - i32::from(target_position.1)).abs() <= 768
}

/// Check source type 6 against the actual per-player eight-slot entity list.
///
/// The source checks only its current text stage for this form. Empty zero
/// slots and runtime indices that cannot fit the source signed word do not
/// match.
#[must_use]
pub(crate) fn collected_entity_at_current_stage(
    stage: &RuntimeStage,
    stage_index: usize,
    text_selector: u8,
    expected_runtime_entity_index: u16,
    collected_entity_indices: &[i16; COLLECTED_ENTITY_SLOT_COUNT],
) -> bool {
    if stage.kind != 6
        || stage_index != usize::from(text_selector)
        || !bound_target_matches(stage, expected_runtime_entity_index)
    {
        return false;
    }
    let Ok(source_entity_index) = i16::try_from(expected_runtime_entity_index) else {
        return false;
    };
    source_entity_index > 0 && collected_entity_indices.contains(&source_entity_index)
}

fn bound_target_matches(stage: &RuntimeStage, expected_runtime_entity_index: u16) -> bool {
    if stage.flags & 1 == 0 || expected_runtime_entity_index == 0 {
        return false;
    }
    match (stage.kind, stage.relation) {
        (4, RuntimeRelation::LiveEntity(index)) | (6, RuntimeRelation::LiveEntityIndex(index)) => {
            index == expected_runtime_entity_index
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::native_world::{RuntimeRelation, RuntimeStage};

    use super::{
        AuthoredTargetBinding, COLLECTED_ENTITY_SLOT_COUNT, Form,
        collected_entity_at_current_stage, delivered_entity_to_point,
    };

    fn stage(kind: i8, flags: i8, relation: RuntimeRelation, x: i16, y: i16) -> RuntimeStage {
        RuntimeStage {
            kind,
            flags,
            axis_x: x,
            axis_y: y,
            relation,
        }
    }

    #[test]
    fn source_forms_bind_only_their_nonnegative_authored_target() {
        let delivery = AuthoredTargetBinding::from_source_row(4, 12).unwrap();
        let collection = AuthoredTargetBinding::from_source_row(6, 17).unwrap();
        assert_eq!(delivery.form(), Form::DeliverOwnedEntity);
        assert_eq!(collection.form(), Form::CollectEntity);
        assert_eq!(delivery.authored_entity_index(), 12);
        assert_eq!(delivery.bind_materialized_entity(12, 42), Some(42));
        assert_eq!(delivery.bind_materialized_entity(13, 42), None);
        assert_eq!(delivery.bind_materialized_entity(12, 0), None);
        assert_eq!(AuthoredTargetBinding::from_source_row(5, 12), None);
        assert_eq!(AuthoredTargetBinding::from_source_row(4, -1), None);
    }

    #[test]
    fn delivery_requires_the_bound_owned_entity_inside_both_signed_xy_limits() {
        let target_index = 42;
        let delivery = stage(
            4,
            1,
            RuntimeRelation::LiveEntity(target_index),
            i16::MIN,
            i16::MAX,
        );

        assert!(delivered_entity_to_point(
            &delivery,
            target_index,
            7,
            7,
            (i16::MIN + 768, i16::MAX - 768),
        ));
        assert!(!delivered_entity_to_point(
            &delivery,
            target_index,
            8,
            7,
            (i16::MIN, i16::MAX),
        ));
        assert!(!delivered_entity_to_point(
            &delivery,
            target_index,
            7,
            7,
            (i16::MIN + 769, i16::MAX),
        ));
        assert!(!delivered_entity_to_point(
            &stage(4, 0, RuntimeRelation::LiveEntity(target_index), 0, 0),
            target_index,
            7,
            7,
            (0, 0),
        ));
        assert!(!delivered_entity_to_point(
            &stage(4, 1, RuntimeRelation::Scalar(u32::from(target_index)), 0, 0),
            target_index,
            7,
            7,
            (0, 0),
        ));
        assert!(!delivered_entity_to_point(
            &stage(4, 1, RuntimeRelation::LiveEntityIndex(target_index), 0, 0,),
            target_index,
            7,
            7,
            (0, 0),
        ));
        assert!(!delivered_entity_to_point(
            &delivery,
            target_index + 1,
            7,
            7,
            (i16::MIN, i16::MAX),
        ));
    }

    #[test]
    fn collection_requires_the_current_bound_target_in_the_actual_eight_slots() {
        let target_index = 51;
        let collection = stage(6, 1, RuntimeRelation::LiveEntityIndex(target_index), 0, 0);
        let mut collected = [0_i16; COLLECTED_ENTITY_SLOT_COUNT];
        collected[7] = i16::try_from(target_index).unwrap();

        assert!(collected_entity_at_current_stage(
            &collection,
            2,
            2,
            target_index,
            &collected,
        ));
        assert!(!collected_entity_at_current_stage(
            &collection,
            2,
            3,
            target_index,
            &collected,
        ));
        assert!(!collected_entity_at_current_stage(
            &collection,
            2,
            2,
            target_index + 1,
            &collected,
        ));
        assert!(!collected_entity_at_current_stage(
            &stage(6, 1, RuntimeRelation::Null, 0, 0),
            2,
            2,
            target_index,
            &collected,
        ));
        assert!(!collected_entity_at_current_stage(
            &stage(6, 1, RuntimeRelation::LiveEntity(target_index), 0, 0),
            2,
            2,
            target_index,
            &collected,
        ));
        assert!(!collected_entity_at_current_stage(
            &collection,
            2,
            2,
            u16::MAX,
            &collected,
        ));
    }
}
