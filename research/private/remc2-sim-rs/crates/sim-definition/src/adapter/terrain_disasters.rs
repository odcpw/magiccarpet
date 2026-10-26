//! Pinned MC2 terrain constructor and physical spell-tier bindings.

use crate::level_definition::{
    ActivationGroupId, CRATER_PROPERTIES, EARTHQUAKE_PROPERTIES, LevelEntityDefinition,
    LevelEntityId, LevelEntityKind, LevelEntityPositionAccess, MechanicKind, TREMOR_PROPERTIES,
    TerrainDisasterDefinition, TerrainDisasterKind, TilePosition, VOLCANO_PROPERTIES,
};

#[must_use]
pub const fn mc2_terrain_disaster_definition(
    kind: TerrainDisasterKind,
) -> TerrainDisasterDefinition {
    let (max_life, lifetime, damage, width, height) = match kind {
        TerrainDisasterKind::Volcano => (11, 17, 2000, 7, 16384),
        TerrainDisasterKind::Impact => (1, 1, 100, 128, 128),
        TerrainDisasterKind::LoweringRing => (40, 40, 200, 2304, 8192),
        TerrainDisasterKind::Earthquake => (128, 128, 100, 1024, 16384),
        TerrainDisasterKind::RaisedRibbon => (2, 2, 0, 0, 0),
        TerrainDisasterKind::MarkedPath | TerrainDisasterKind::CraterTrail => (0, 0, 0, 0, 0),
        TerrainDisasterKind::RisingTrail => (0, 0, 0, 768, 768),
        TerrainDisasterKind::Tremor => (120, 120, 20000, 1280, 2048),
    };
    TerrainDisasterDefinition::new(kind, max_life, lifetime, damage, width, height)
}

/// Initial PrepareEvents49540 overrides only models9,11,15. In particular,
/// initial authored Tremor71 retains its constructor's120/20000 defaults.
pub fn mc2_authored_terrain_disaster_definition(
    kind: TerrainDisasterKind,
    tier: u16,
) -> Result<TerrainDisasterDefinition, String> {
    let defaults = mc2_terrain_disaster_definition(kind);
    if !matches!(
        kind,
        TerrainDisasterKind::Volcano
            | TerrainDisasterKind::LoweringRing
            | TerrainDisasterKind::Earthquake
    ) {
        return Ok(defaults);
    }
    let tier = usize::from(tier);
    if tier > 2 {
        return Err(format!(
            "authored {} has invalid physical spell tier {tier}",
            kind.as_str()
        ));
    }
    let (max_life, lifetime, damage) = match kind {
        TerrainDisasterKind::Volcano => {
            let rule = VOLCANO_PROPERTIES.tiers[tier];
            (i32::from(rule.impact_parameter), 17, rule.damage)
        }
        TerrainDisasterKind::LoweringRing => {
            let rule = CRATER_PROPERTIES.tiers[tier];
            (40, i32::from(rule.impact_parameter), rule.damage)
        }
        TerrainDisasterKind::Earthquake => {
            let rule = EARTHQUAKE_PROPERTIES.tiers[tier];
            (128, i32::from(rule.impact_parameter), rule.damage)
        }
        _ => unreachable!("only tier-bound constructors reached"),
    };
    Ok(TerrainDisasterDefinition::new(
        kind,
        max_life,
        lifetime,
        damage,
        defaults.half_width(),
        defaults.half_height(),
    ))
}

/// Live sub4A310 also binds model71's lifetime/payload from physical15,
/// while preserving the constructor maxLife120 for its envelope division.
pub fn mc2_grouped_terrain_disaster_definition(
    kind: TerrainDisasterKind,
    tier: u16,
) -> Result<TerrainDisasterDefinition, String> {
    if kind != TerrainDisasterKind::Tremor {
        return mc2_authored_terrain_disaster_definition(kind, tier);
    }
    let tier = usize::from(tier);
    if tier > 2 {
        return Err(format!(
            "authored tremor has invalid physical spell tier {tier}"
        ));
    }
    let rule = TREMOR_PROPERTIES.tiers[tier];
    Ok(TerrainDisasterDefinition::new(
        kind,
        120,
        i32::from(rule.impact_parameter),
        rule.damage,
        1280,
        2048,
    ))
}

#[must_use]
pub const fn terrain_disaster_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    config: TerrainDisasterDefinition,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Mechanic(MechanicKind::TerrainDisaster {
            kind: config.kind(),
        }),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        None,
    )
    .with_terrain_disaster(config)
}
