//! Source projectile lights over the authoritative arena and terrain planes.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::dynamic_lighting::{DynamicLightLinkedEntity, DynamicLightingState};
use crate::native_world::{MapType, SessionTerrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectileLight {
    Fireball,
    Lightning,
}

fn night_or_cave(terrain: &SessionTerrain) -> bool {
    matches!(terrain.map_type(), MapType::Night | MapType::Cave)
}

/// The caller invokes this at the constructor's AddEvent2 site, before any
/// subsequent source callback can allocate another light. A successful
/// admission must set the source row's light-linked flag at that same site.
pub(crate) fn admit_projectile(
    lighting: &mut DynamicLightingState,
    enabled: bool,
    terrain: &SessionTerrain,
    slot: u16,
    position: Position,
    kind: ProjectileLight,
) -> bool {
    let jitter = match kind {
        ProjectileLight::Fireball => 1,
        ProjectileLight::Lightning => 9,
    };
    lighting
        .admit(
            enabled,
            night_or_cave(terrain),
            slot,
            position,
            128,
            jitter,
            false,
        )
        .is_some()
}

/// Remove the previous footprint before the first UpdateEntities pass.
pub(crate) fn before_updates(lighting: &DynamicLightingState, terrain: &mut SessionTerrain) {
    let active_map = night_or_cave(terrain);
    let (_, shading) = terrain.dynamic_lighting_planes();
    lighting.subtract_previous(shading, active_map);
}

/// Recompute after the last pass. Fixed source addresses deliberately resolve
/// their current occupant, including a recycled typed Fireball carrier.
pub(crate) fn after_updates(
    lighting: &mut DynamicLightingState,
    arena: &SourceArena,
    terrain: &mut SessionTerrain,
) {
    let active_map = night_or_cave(terrain);
    let (ground, shading) = terrain.dynamic_lighting_planes();
    lighting.service_recompute(
        shading,
        active_map,
        |slot| {
            if let Some(projectile) = arena
                .economy
                .fireball_projectiles()
                .iter()
                .find(|row| row.source_entity_index == slot && row.active)
            {
                return Some(DynamicLightLinkedEntity {
                    position: projectile.position,
                    life: i32::from(projectile.life),
                    disabled: projectile.drawing_disabled,
                });
            }
            arena.entities.get(usize::from(slot)).and_then(|row| {
                (row.active && row.class != 0).then_some(DynamicLightLinkedEntity {
                    position: row.position,
                    life: row.life,
                    disabled: row.control_flags & 0x400 != 0,
                })
            })
        },
        |x, y| crate::legacy_terrain::interpolate_ground(ground, x, y) as i16,
    );
}
