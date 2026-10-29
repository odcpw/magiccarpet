//! Admission policy and family-owned construction for authored Level 2 rows.

use std::collections::BTreeSet;

use crate::{
    campaign::source_arena::SourceArena,
    level1_mission::{AUTHORED_FEATURE_COUNT, AuthoredFeature},
    native_world::{RuntimeSnapshot, SessionTerrain},
};

/// `sub_4A1E0` visits the authored table in index order, beginning at one.
/// Row zero is a sentinel even when it contains a matching nonzero type.
/// Consumed rows correspond to source records whose type has been cleared.
pub(crate) fn source_group_rows(
    features: &[AuthoredFeature],
    group: i16,
    consumed: &BTreeSet<u16>,
) -> Vec<AuthoredFeature> {
    let mut rows = features
        .iter()
        .copied()
        .filter(|feature| {
            feature.index != 0
                && usize::from(feature.index) < AUTHORED_FEATURE_COUNT
                && feature.kind != 0
                && feature.activation_group == group
                && !consumed.contains(&feature.index)
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|feature| feature.index);
    rows
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthoredSpawnFamily {
    Builder,
    Villager,
    CastleDefender,
}

/// Admit class-5 families whose ordinary publisher is wired to a real
/// constructor, typed state binding, and scheduled source callback.
pub(crate) const fn has_family_constructor(class: u16, model: u16) -> bool {
    matches!((class, model), (5, 12) | (5, 13) | (5, 15))
}

pub(crate) const fn family(feature: AuthoredFeature) -> Option<AuthoredSpawnFamily> {
    if feature.kind != 5 {
        return None;
    }
    match feature.subtype {
        12 => Some(AuthoredSpawnFamily::Builder),
        13 => Some(AuthoredSpawnFamily::Villager),
        15 => Some(AuthoredSpawnFamily::CastleDefender),
        _ => None,
    }
}

/// Construct the family-owned source carrier after the publisher has consumed
/// this authored row. The resulting slot is left in the ordinary arena so the
/// remaining numeric source cursor can schedule its existing callback.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    runtime: &mut RuntimeSnapshot,
    feature: AuthoredFeature,
    max_life: u32,
    stage_index: Option<u8>,
) -> Result<Option<u16>, String> {
    let family = family(feature).ok_or_else(|| {
        format!(
            "authored row {} has no class-5 family constructor for {}/{}",
            feature.index, feature.kind, feature.subtype
        )
    })?;
    let source_slot = match family {
        AuthoredSpawnFamily::Builder | AuthoredSpawnFamily::Villager => {
            arena.allocate_authored_resident(feature, max_life, terrain)
        }
        AuthoredSpawnFamily::CastleDefender => {
            crate::native_world::creatures::creature_15::mc2::allocate_authored_route(
                arena,
                runtime,
                feature,
                max_life.cast_signed(),
                terrain,
                stage_index,
            )
        }
    };
    Ok(source_slot)
}
