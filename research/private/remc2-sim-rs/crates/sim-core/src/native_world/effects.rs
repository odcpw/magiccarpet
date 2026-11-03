//! MC2 header views for retained effect carriers. Timers, RNG, descendants,
//! damage and terrain rules live in world::effects; this adapter only maps
//! the arena's existing typed storage to that one implementation.

use super::{EntitySnapshot, Level2DynamicArenaKind as Kind, ParticleParameter, SessionTerrain};
use crate::{Position, campaign::source_arena::SourceArena, world::effects as rules};

impl super::Level2EconomyState {
    pub(crate) fn record_raw_death_burst_spawn(
        &mut self,
        slot: u16,
        parent: u16,
        id: u16,
    ) {
        self.events.push(super::Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index: slot,
            parent_entity_index: parent,
            id_entity_index: id,
        });
    }

    pub(crate) fn record_raw_death_burst_expiry(&mut self, slot: u16, parent: u16) {
        self.events.push(super::Level2EconomyEvent::DeathBurstExpired {
            source_entity_index: slot,
            parent_entity_index: parent,
        });
    }
}

/// The flyer owns guidance/collision; its terminal constructor returns to
/// SourceArena so raw and typed replaceable rows arbitrate the same NewEvent.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    context: super::source_target::Context<'_>,
    capture_slots: &[u16],
    owner_target_range: u16,
) -> Result<super::level2_dynamic_economy::Level2FireballStep, String> {
    use super::level2_dynamic_economy::FireballBirth;
    let entry = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .copied();
    let bound = entry.map_or(0, |row| row.target_source_entity_index);
    let context = super::source_target::Context {
        particles: Some(particles),
        map_type: Some(terrain.map_type()),
        ..context
    };
    let mut initial_capture = false;
    if let Some(projectile) = entry.filter(|row| {
        matches!((row.source_model, row.action), (0, 0) | (28, 29))
            && row.target_source_entity_index == 0
            && !row.acquisition_attempted
    }) {
        let mut view = crate::world::terrain::cave_features::new_event(
            slot,
            projectile.entity_rng,
            projectile.position,
        );
        view.class = 9;
        view.model = projectile.source_model;
        view.source_id = projectile.owner_entity_index;
        view.set_yaw(projectile.yaw);
        view.set_pitch(projectile.pitch);
        let empty_models = std::array::from_fn(|_| Vec::new());
        initial_capture = super::guided_projectile::acquire_capture(
            arena,
            &mut view,
            &super::guided_projectile::Targets {
                class3: &[],
                class5: &empty_models,
                class10: capture_slots,
                owner_target_range,
            },
            context,
        )?;
        if initial_capture {
            let row = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or("capture acquisition lost its Fireball carrier")?;
            row.target_source_entity_index = view.source_word_96();
            row.roll = view.source_roll();
            row.fov = view.fov;
        }
    }
    let current_bound = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|row| row.source_entity_index == slot)
        .map_or(bound, |row| row.target_source_entity_index);
    // Capture each candidate's current fixed-address owner before borrowing
    // the economy mutably. Only errors for the actual target or visited map
    // candidates are consumed by the flyer callback.
    let views = arena
        .entities
        .iter()
        .map(|row| {
            if row.index == current_bound || arena.economy.map_chains.is_linked(row.index) {
                super::source_target::read(arena, context, row.index)
            } else {
                Ok(None)
            }
        })
        .collect::<Vec<_>>();
    let mut step = arena.economy.advance_fireball_projectile_with_views(
        slot,
        terrain,
        &arena.entities,
        true,
        Some(&views),
        initial_capture,
    )?;
    if let Some(position) = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|row| row.source_entity_index == slot && row.active)
        .map(|row| row.position)
    {
        // Publish both old and new terrain map heads after source movement.
        rules::sync_position(arena, terrain, slot, position, true, false)?;
    }
    let Some(birth) = step.pending_birth.take() else {
        return Ok(step);
    };
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|s| s.source_entity_index == slot)
        .copied()
        .ok_or("terminal flyer lost its current carrier")?;
    // 65C20 asks 68AC0 before its ordinary impact. Magic Mine owns the
    // capture mailbox; the caller commits the current holder's expiry value.
    let capture_target = step.collision_target.filter(|target| {
        matches!(
            (projectile.source_model, projectile.action),
            (0, 0) | (28, 29)
        ) && arena
            .entities
            .get(usize::from(*target))
            .is_some_and(|mine| {
                (mine.class, mine.model) == (10, 78)
                    && mine.word_32() == projectile.owner_entity_index
                    && u16::from_le_bytes([mine.record[58], mine.record[59]]) == u16::MAX
            })
    });
    if let Some(mine_slot) = capture_target {
        let visual = rules::allocate_from_source_position(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            slot,
            projectile.position,
            particles,
        )?;
        // The visual NewEvent runs first. Fixed pointers may now refer to
        // newborn rows; source does not recheck the mine predicate afterward.
        let current = super::source_target::read(arena, context, slot)?
            .ok_or("captured Fireball lost its fixed source address")?;
        let holder_slot = current.relations.attacker.unwrap_or(0);
        if holder_slot != 0 {
            let holder = super::source_target::status_parent(arena, context, holder_slot)?
                .ok_or("captured Fireball holder address is outside source arena")?;
            crate::world::fields::magic_mine::store_captured_spell(
                &mut arena.entities[usize::from(mine_slot)],
                holder.model,
                holder.level_gate as u8,
            );
            step.captured_holder = Some(holder_slot);
        }
        arena.economy.prepare_sound(slot, slot, -1, 26);
        if visual == Some(slot) {
            rules::hide_birth_replacement(arena, slot, visual)?;
        } else if let Some(current) = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            current.active = false;
            current.drawing_disabled = true;
        }
        step.live = false;
        step.impact_slot = visual;
        step.direct_hit_owner = None;
        step.direct_hit_family = None;
        step.accuracy = None;
        return Ok(step);
    }
    let child = if birth == FireballBirth::Impact
        && projectile.origin == super::FireballProjectileOrigin::Eruption
    {
        arena.allocate_source_slot_with_terrain(terrain).map(|id| {
            // The source passes &projectile.position into NewEvent. If that
            // allocation replaces the flyer itself, the pointer now sees the
            // freshly cleared constructor row before AddMeteor copies it.
            let position = if id == slot { Position::default() } else { projectile.position };
            let mut row = arena.economy.initialize_authored_model17(id, u16::MAX, position);
            if id != slot {
                row.entity_id = projectile.owner_entity_index;
                row.yaw = projectile.yaw;
                row.pitch = projectile.pitch;
                row.subspell_index = projectile.source_subspell_index;
                // Eruption's NewEvent/SummonFireball_4D2E0 leaves byte46 zero.
            }
            if let Some(target) = step.collision_target {
                row.target_source_entity_index = target;
            }
            if id == slot {
                // The final DisableEntityDrawing04 acts on the same address.
                row.control_flags |= 0x400;
            }
            *arena
                .economy
                .authored_model17
                .iter_mut()
                .find(|s| s.source_entity_index == id)
                .expect("new Meteor carrier") = row;
            arena.economy.source_owner.insert(id, row.entity_id);
            id
        })
    } else {
        let kind = if birth == FireballBirth::Ash {
            crate::level_definition::EffectKind::Ash
        } else if projectile.source_model == 28 {
            crate::level_definition::EffectKind::FireSpheres
        } else {
            crate::level_definition::EffectKind::Impact
        };
        let definition = sim_definition::adapter::mc2_effect_definition(kind);
        let child = rules::allocate_from_source_position(
            arena,
            terrain,
            definition,
            slot,
            projectile.position,
            particles,
        )?;
        if let Some(id) = child
            && kind == crate::level_definition::EffectKind::FireSpheres
        {
            // 65C20 and 65B50 read the flyer again after the full constructor.
            // A source-position address reused by the root or one sector now
            // carries that newborn's fields, never the previous flyer payload.
            let replacement = arena.economy.live_arena_kind(slot);
            let replaced_root = if replacement == Some(Kind::AuthoredModel76) {
                arena
                    .economy
                    .authored_model76()
                    .iter()
                    .find(|row| row.source_entity_index == slot)
                    .map(|row| {
                        (
                            row.entity_id,
                            row.subspell_index,
                            row.yaw,
                            row.pitch,
                            row.target_entity_index,
                        )
                    })
            } else if replacement == Some(Kind::Model76Child) {
                arena
                    .economy
                    .model76_children()
                    .iter()
                    .find(|row| row.source_entity_index == slot)
                    .map(|row| (row.entity_id, row.subspell_index, row.yaw, row.pitch, 0))
            } else {
                None
            };
            let (owner, damage, yaw, pitch, target) = replaced_root.unwrap_or((
                projectile.owner_entity_index,
                projectile.source_subspell_index,
                projectile.yaw,
                projectile.pitch,
                step.collision_target.unwrap_or(0),
            ));
            // sub_65B50 specializes the successful sub_65C20 result. The
            // constructor's 80-tick children keep their own life; only the
            // root receives the 30-tick lifetime and projectile payload.
            let root = arena
                .economy
                .effect_sphere_roots_mut()
                .iter_mut()
                .find(|row| row.source_entity_index == id)
                .expect("Fire Spheres constructor retains its root");
            root.entity_id = owner;
            root.subspell_index = damage;
            root.yaw = yaw;
            root.pitch = pitch;
            root.max_life = 30;
            root.life = 30;
            root.target_entity_index = target;
            arena.economy.source_owner.insert(id, owner);
            let local_sector = super::source_target::status_parent(arena, context, owner)?
                .is_some_and(|parent| {
                    (parent.class, parent.model) == (3, 0)
                        && matches!(parent.special_relation, super::EntitySpecialRelation::PlayerState(index)
                            if context.spells.is_some_and(|shared| index == shared.local_player.slot))
                });
            for child in arena
                .economy
                .effect_sphere_children_mut()
                .iter_mut()
                .filter(|row| row.parent_entity_index == id)
            {
                child.entity_id = owner;
                if local_sector {
                    child.visual_index = 42;
                }
            }
            // 65B50's final DisableEntityDrawing acts on the current address.
            if replacement == Some(Kind::AuthoredModel76) {
                if let Some(row) = arena
                    .economy
                    .effect_sphere_roots_mut()
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
                {
                    row.control_flags |= 0x400;
                    row.drawing_enabled = false;
                }
            } else if replacement == Some(Kind::Model76Child)
                && let Some(row) = arena
                    .economy
                    .effect_sphere_children_mut()
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
            {
                row.control_flags |= 0x400;
                row.drawing_enabled = false;
            }
        } else if let Some(id) = child {
            let row = &mut arena.entities[usize::from(id)];
            // NewEvent may reuse the projectile's frozen address. In that
            // case the source pointer now reads the initialized impact row.
            let (owner, subspell, yaw, pitch) = if id == slot
                && matches!(
                    kind,
                    crate::level_definition::EffectKind::Impact
                        | crate::level_definition::EffectKind::Ash
                ) {
                (row.source_id, row.subspell_index, row.yaw, row.pitch)
            } else {
                (
                    projectile.owner_entity_index,
                    projectile.source_subspell_index,
                    projectile.yaw,
                    projectile.pitch,
                )
            };
            row.source_id = owner;
            row.record[30..32].copy_from_slice(&row.source_id.to_le_bytes());
            if birth == FireballBirth::Impact {
                row.set_subspell_index(subspell);
                row.set_yaw(yaw);
                row.set_pitch(pitch);
                if let Some(target) = step.collision_target {
                    row.set_source_word_96(target);
                }
            }
        }
        child
    };
    let impact_replaced_source = child == Some(slot)
        && birth == FireballBirth::Impact
        && projectile.origin != super::FireballProjectileOrigin::Eruption
        && projectile.source_model != 28;
    // sub_686D0 publishes a successful hit to the current player caster
    // before the XP tail. Allied actors consume that same word96. A failed
    // impact allocation leaves it unchanged; a replaced flyer address no
    // longer carries the old caster identity.
    if birth == FireballBirth::Impact
        && child.is_some()
        && child != Some(slot)
        && let Some(target) = step.collision_target.filter(|target| *target != 0)
        && arena
            .handle(projectile.owner_entity_index)
            .is_some_and(|handle| {
                matches!(
                    arena.resolve(handle),
                    Some(crate::campaign::source_arena::SourceEntityView::Imported(owner))
                        if owner.index != 0 && owner.class == 3 && matches!(owner.model, 0 | 1)
                )
            })
        && let Some(owner) = arena
            .entities
            .get_mut(usize::from(projectile.owner_entity_index))
    {
        owner.set_source_word_96(target);
    }
    if child.is_some() || birth == FireballBirth::Ash {
        if impact_replaced_source || (birth == FireballBirth::Ash && child == Some(slot)) {
            rules::hide_birth_replacement(arena, slot, child)?;
        } else if let Some(parent) = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|s| s.source_entity_index == slot)
        {
            parent.active = false;
            parent.drawing_disabled = true;
        }
        step.live = false;
    }
    // 65780 reads both target pointers and the current shooter after the
    // impact constructor. It counts successful eligible output even on
    // terrain; a hit additionally needs equal current target identities.
    if child.is_some()
        && matches!(
            (projectile.source_model, projectile.action),
            (0, 0) | (28, 29)
        )
    {
        let current = super::source_target::read(arena, context, slot)?
            .ok_or("Fireball source address is outside source arena")?;
        let shooter = super::source_target::read(arena, context, current.source_id)?;
        if current.class == 9
            && matches!(current.model, 0 | 28)
            && shooter.is_some_and(|owner| (owner.class, owner.model) == (3, 0))
        {
            let hit = if let Some(target) = step.collision_target.filter(|_| bound != 0) {
                let target = super::source_target::read(arena, context, target)?;
                let bound = super::source_target::read(arena, context, bound)?;
                matches!((target, bound), (Some(target), Some(bound))
                    if target.source_id == bound.source_id)
            } else {
                false
            };
            step.accuracy = Some((current.source_id, hit));
        }
    }
    // Fireball's actual 65C20 awards family0 after an actor impact,
    // independently of word26. Other flyers' 65820 reads the holder model
    // after NewEvent; keep that distinct policy for their callbacks.
    if child.is_some()
        && step.collision_target.is_some()
        && matches!(
            (projectile.source_model, projectile.action),
            (0, 0) | (28, 29)
        )
    {
        let current = super::source_target::read(arena, context, slot)?
            .ok_or("Fireball source address is outside source arena")?;
        step.direct_hit_owner = Some(current.source_id);
        step.direct_hit_family = Some(0);
    } else if child.is_some()
        && !impact_replaced_source
        && !(projectile.origin == super::FireballProjectileOrigin::Eruption && child == Some(slot))
        && step.direct_hit_owner.is_some()
        && projectile.holder_entity_index != 0
    {
        let holder = super::source_target::read(arena, context, projectile.holder_entity_index)?
            .ok_or("Fireball holder address is outside source arena")?;
        step.direct_hit_family = Some(holder.model);
    } else {
        step.direct_hit_owner = None;
    }
    step.impact_slot = child;
    Ok(step)
}

/// MCR1 retains the source globals at the same reset boundary as the arena.
pub(crate) fn reset_state(runtime: &super::RuntimeSnapshot) -> rules::State {
    let (eruption, eruption_smoke) = runtime.effect_globals();
    rules::State::from_globals(eruption, eruption_smoke)
}

pub(crate) fn hidden_slots(arena: &SourceArena) -> Vec<(u16, Position)> {
    let mut hidden = Vec::new();
    macro_rules! collect {
        ($rows:expr) => {
            hidden.extend(
                $rows
                    .iter()
                    .filter(|s| s.active && s.control_flags & 0x400 != 0)
                    .map(|s| (s.source_entity_index, s.position)),
            );
        };
    }
    collect!(arena.economy.authored_model1);
    collect!(arena.economy.authored_model5_water_splashes);
    collect!(arena.economy.authored_model6);
    collect!(arena.economy.authored_model13);
    collect!(arena.economy.model13_spawned_children);
    collect!(arena.economy.model59_smoke_children);
    collect!(arena.economy.authored_model17);
    collect!(arena.economy.model17_spawned_children);
    collect!(arena.economy.authored_model76);
    collect!(arena.economy.model76_children);
    collect!(arena.economy.player_death_markers);
    collect!(arena.economy.death_bursts);
    hidden.sort_unstable_by_key(|(slot, _)| *slot);
    hidden
}

pub(crate) fn sprite_sources(arena: &SourceArena) -> Vec<(u16, u16, u16, i16, Position, u32)> {
    let mut rows = Vec::new();
    let visible = |active: bool, flags: u32| active && flags & 0x421 == 0;
    for s in arena
        .economy
        .death_bursts
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.id_entity_index,
            41,
            0,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .authored_model1
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.source_entity_index,
            s.particle_index,
            0,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .authored_model5_water_splashes
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.sprite_index,
            0,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .authored_model6
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.particle_index,
            0,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .authored_model13
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.particle_index,
            0,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .model13_spawned_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.parent_entity_id,
            s.particle_index,
            s.yaw,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .model59_smoke_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.source_entity_index,
            s.particle_index,
            s.yaw,
            s.position,
            s.control_flags,
        ));
    }
    for s in arena
        .economy
        .model76_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.visual_index,
            s.yaw,
            s.position,
            s.control_flags,
        ));
    }
    rows
}

pub(crate) fn animation_frame(arena: &SourceArena, slot: u16, particle: u16) -> u8 {
    if let Some(s) = arena
        .economy
        .model17_spawned_children
        .iter()
        .find(|s| s.active && s.source_entity_index == slot)
    {
        return s.animation_frame;
    }
    if let Some(s) = arena
        .economy
        .authored_model5_water_splashes
        .iter()
        .find(|s| s.active && s.source_entity_index == slot)
    {
        return s.animation_frame;
    }
    if arena
        .economy
        .live_arena_slots()
        .iter()
        .any(|(id, _)| *id == slot)
    {
        return 0;
    }
    arena
        .entities
        .get(usize::from(slot))
        .filter(|s| s.active && s.particle_parameter_index == Some(particle))
        .map_or(0, |s| s.record[96])
}

pub(crate) fn renderable_source(row: &EntitySnapshot) -> bool {
    row.class != 10 || rules::source_kind(row.model).is_none() || row.control_flags & 4 != 0
}

fn header(
    arena: &SourceArena,
    slot: u16,
    kind: Kind,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let (owner, rng, model, action, max_life, life, payload, flags, phase, position, base_particle) =
        match kind {
            Kind::DeathBurst => {
                let s = arena
                    .economy
                    .death_bursts
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("death burst carrier is absent")?;
                (
                    s.id_entity_index,
                    s.entity_rng,
                    1,
                    1,
                    s.max_life as i16,
                    s.life_ticks,
                    s.subspell_index,
                    s.control_flags,
                    s.source_byte_3e,
                    s.position,
                    41,
                )
            }
            Kind::AuthoredModel17 => {
                let s = arena
                    .economy
                    .authored_model17
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("Meteor carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    17,
                    17,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    0,
                )
            }
            Kind::Model17SpawnedChild => {
                let s = arena
                    .economy
                    .model17_spawned_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("impact carrier is absent")?;
                (
                    s.parent_entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    7,
                )
            }
            Kind::AuthoredModel1 => {
                let s = arena
                    .economy
                    .authored_model1
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("radial explosion carrier is absent")?;
                (
                    slot,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    41,
                )
            }
            Kind::AuthoredModel5WaterSplash => {
                let s = arena
                    .economy
                    .authored_model5_water_splashes
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("ash carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    244,
                )
            }
            Kind::AuthoredModel6 => {
                let s = arena
                    .economy
                    .authored_model6
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("ground fire carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    228,
                )
            }
            Kind::AuthoredModel13 => {
                let s = arena
                    .economy
                    .authored_model13
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("eruption smoke carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    228,
                )
            }
            Kind::Model13SpawnedChild => {
                let s = arena
                    .economy
                    .model13_spawned_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("dark smoke carrier is absent")?;
                (
                    s.parent_entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    100,
                    s.control_flags,
                    s.phase,
                    s.position,
                    9,
                )
            }
            Kind::Model59SmokeChild => {
                let s = arena
                    .economy
                    .model59_smoke_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("smoke carrier is absent")?;
                (
                    slot,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    100,
                    s.control_flags,
                    s.phase,
                    s.position,
                    67,
                )
            }
            _ => return Err(format!("{kind:?} is not a retained shared effect carrier")),
        };
    let mut row = super::cave_features::new_event(slot, rng, position);
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
    row.class = 10;
    row.record[67] = 10;
    row.model = model;
    row.record[68] = model;
    row.set_source_action(action);
    row.set_max_life(i32::from(max_life));
    row.set_life(i32::from(life));
    row.set_subspell_index(payload);
    row.set_control_flags(
        flags
            | if arena.economy.map_chains.is_linked(slot) {
                4
            } else {
                0
            },
    );
    row.set_phase(phase);
    if kind != Kind::AuthoredModel17 {
        rules::bind_particle(&mut row, base_particle, particles)?;
    }
    match kind {
        Kind::AuthoredModel17 => {
            let s = arena
                .economy
                .authored_model17
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_yaw(s.yaw);
            row.set_pitch(s.pitch);
            row.set_source_word_96(s.target_source_entity_index);
            row.set_source_byte_46(s.source_byte_46);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
            row.record[69] = u8::MAX;
            row.record[70] = u8::MAX;
        }
        Kind::Model17SpawnedChild => {
            let s = arena
                .economy
                .model17_spawned_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_word_2c(s.source_word_2c);
            row.set_yaw(s.yaw);
            row.set_pitch(s.pitch);
            row.set_source_word_96(s.target_source_entity_index);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
            row.record[96] = s.animation_frame;
        }
        Kind::AuthoredModel5WaterSplash => {
            let s = arena
                .economy
                .authored_model5_water_splashes
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.record[96] = s.animation_frame;
        }
        Kind::AuthoredModel6 => {
            let s = arena
                .economy
                .authored_model6
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_word_2c(s.height_offset);
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.set_source_overlap_half_extents(s.shift_rot_x as i16, s.shift_rot_x as i16);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
        }
        Kind::AuthoredModel13 => {
            let s = arena
                .economy
                .authored_model13
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
        }
        Kind::Model13SpawnedChild => {
            let s = arena
                .economy
                .model13_spawned_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_actual_speed(s.actual_speed);
            row.set_yaw(s.yaw);
            row.set_source_word_30(s.source_word_30);
            row.record[98..102].copy_from_slice(&s.pending_damage.to_le_bytes());
            row.record[102..104].copy_from_slice(&s.pending_attacker_entity_index.to_le_bytes());
            row.record[138..140].copy_from_slice(&s.max_speed.to_le_bytes());
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.record[69] = 10;
            row.record[70] = 14;
        }
        Kind::Model59SmokeChild => {
            let s = arena
                .economy
                .model59_smoke_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_actual_speed(s.actual_speed);
            row.set_yaw(s.yaw);
            row.set_source_word_30(s.source_word_30);
            row.record[98..102].copy_from_slice(&s.pending_damage.to_le_bytes());
            row.record[102..104].copy_from_slice(&s.pending_attacker_entity_index.to_le_bytes());
            row.record[138..140].copy_from_slice(&s.max_speed.to_le_bytes());
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.record[69] = 10;
            row.record[70] = 13;
        }
        _ => {}
    }
    Ok(row)
}

fn store(arena: &mut SourceArena, kind: Kind, row: &EntitySnapshot) {
    let slot = row.index;
    macro_rules! common {
        ($s:ident) => {
            $s.entity_rng = row.entity_rng;
            $s.max_life = row.max_life as i16;
            $s.life = row.life as i16;
            $s.control_flags = row.control_flags;
            $s.position = row.position;
            $s.active = row.active;
        };
    }
    match kind {
        Kind::DeathBurst => {
            let s = arena
                .economy
                .death_bursts
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            s.entity_rng = row.entity_rng;
            s.max_life = row.max_life;
            s.life_ticks = row.life as i16;
            s.control_flags = row.control_flags;
            s.position = row.position;
            s.active = row.active;
        }
        Kind::AuthoredModel17 => {
            let s = arena
                .economy
                .authored_model17
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.yaw = row.yaw;
            s.pitch = row.pitch;
            s.target_source_entity_index = row.source_word_96();
            s.source_byte_46 = row.record[74];
            s.shift_rot_x = row.array_pitch();
            s.shift_rot_y = row.array_fov();
        }
        Kind::Model17SpawnedChild => {
            let s = arena
                .economy
                .model17_spawned_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.source_word_2c = row.source_word_2c();
            s.animation_frame = row.record[96];
        }
        Kind::AuthoredModel1 => {
            let s = arena
                .economy
                .authored_model1
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.drawing_enabled = !row.drawing_disabled();
            if row.control_flags & 2 != 0 {
                s.last_sound_id = 3;
            }
        }
        Kind::AuthoredModel5WaterSplash => {
            let s = arena
                .economy
                .authored_model5_water_splashes
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.drawing_enabled = !row.drawing_disabled();
            s.animation_frame = row.record[96];
            if row.control_flags & 2 != 0 {
                s.last_sound_id = 27;
            }
        }
        Kind::AuthoredModel6 => {
            let s = arena
                .economy
                .authored_model6
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.height_offset = row.source_word_2c();
            s.particle_index = row.particle_parameter_index.unwrap_or(228);
        }
        Kind::AuthoredModel13 => {
            let s = arena
                .economy
                .authored_model13
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.drawing_enabled = !row.drawing_disabled();
        }
        Kind::Model13SpawnedChild => {
            let s = arena
                .economy
                .model13_spawned_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.yaw = row.yaw;
            s.source_word_30 = row.source_word_30();
            s.pending_damage = row.source_wind_spell_payload();
            s.pending_attacker_entity_index = row.source_wind_spell_owner();
            s.actual_speed = row.actual_speed;
            s.max_speed = i16::from_le_bytes(row.record[138..140].try_into().unwrap());
            s.particle_index = row.particle_parameter_index.unwrap_or(9);
            s.drawing_enabled = !row.drawing_disabled();
        }
        Kind::Model59SmokeChild => {
            let s = arena
                .economy
                .model59_smoke_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.yaw = row.yaw;
            s.source_word_30 = row.source_word_30();
            s.pending_damage = row.source_wind_spell_payload();
            s.pending_attacker_entity_index = row.source_wind_spell_owner();
            s.actual_speed = row.actual_speed;
            s.max_speed = i16::from_le_bytes(row.record[138..140].try_into().unwrap());
            s.particle_index = row.particle_parameter_index.unwrap_or(67);
            s.drawing_enabled = !row.drawing_disabled();
        }
        _ => unreachable!("header admitted a retained effect kind"),
    }
}

/// After hidden map unlink, retain the source's class-zero bytes at each
/// current 10/17 address before the economy frees its typed carrier.
pub(crate) fn prepare_model17_reclaim(arena: &mut SourceArena) -> Result<(), String> {
    let hidden = arena
        .economy
        .authored_model17
        .iter()
        .filter(|source| source.active && source.control_flags & 0x400 != 0)
        .filter_map(|source| {
            let current = arena.economy.live_arena_cursor_row(source.source_entity_index)?;
            (current.kind == Kind::AuthoredModel17)
                .then_some((source.source_entity_index, current.generation))
        })
        .collect::<Vec<_>>();
    for (slot, generation) in hidden {
        let current = arena.economy.live_arena_cursor_row(slot);
        if !current
            .is_some_and(|row| row.kind == Kind::AuthoredModel17 && row.generation == generation)
        {
            continue;
        }
        // This header has no particle binding. The typed carrier is still the
        // current authority, including its last callback and phase bytes.
        let mut row = header(arena, slot, Kind::AuthoredModel17, &[])?;
        row.reclaim_source_row();
        arena.entities[usize::from(slot)] = row;
    }
    Ok(())
}

pub(crate) fn visit<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    kind: Kind,
    particles: &[ParticleParameter],
    state: &mut rules::State,
    apply: F,
) -> Result<rules::Visit, String>
where
    F: FnMut(&mut SourceArena, rules::DamagePulse) -> Result<u16, String>,
{
    let generation = arena.economy.slot_generation(slot);
    let row = header(arena, slot, kind, particles)?;
    let burst_parent = if kind == Kind::DeathBurst {
        arena
            .economy
            .death_bursts
            .iter()
            .find(|s| s.source_entity_index == slot && s.active)
            .map(|s| s.parent_entity_index)
    } else {
        None
    };
    let (row, visit) = rules::visit_entity(arena, terrain, row, particles, state, apply)?;
    if arena.economy.slot_generation(slot) != generation {
        // NewEvent replaced this typed parent during its own callback. The
        // source pointer now denotes the child; never restore the old carrier.
        rules::commit_entity(arena, terrain, row)?;
    } else {
        store(arena, kind, &row);
        if row.drawing_disabled() && let Some(parent) = burst_parent {
            arena.economy.record_raw_death_burst_expiry(slot, parent);
            // The retained typed carrier remains authoritative while hidden.
            // Keep its eventual class-zero bytes for fixed-address readers
            // after the following source reclaim removes that carrier.
            let mut reclaimed = row.clone();
            reclaimed.reclaim_source_row();
            arena.entities[usize::from(slot)] = reclaimed;
        }
        arena.economy.source_rng.insert(slot, row.entity_rng);
        rules::sync_position(
            arena,
            terrain,
            slot,
            row.position,
            row.active && row.control_flags & 4 != 0,
            false,
        )?;
    }
    Ok(visit)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fireball_collision_reaches_the_existing_magic_mine_capture_consumer() {
        use crate::Position;
        use crate::native_world::{
            FireballProjectileOrigin, level2_dynamic_economy::Level2FireballProjectileLaunchPayload,
        };
        let session = super::super::tests::level_two_campaign_session();
        for (visual_space, has_holder) in [(true, true), (false, true), (true, false)] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let particles = &session.session.definition.presentation.particles;
            let player = session.state.entity_index;
            let holder = session.shared_spells().local_player().enabled_spell_holders[0];
            assert_ne!(holder, 0);
            let position = Position {
                x: 2_048,
                y: 2_048,
                z: 8_000,
            };
            let mine = crate::world::fields::magic_mine::construct(
                &mut arena,
                &mut terrain,
                position,
                particles[66],
            )
            .unwrap()
            .expect("the real mine constructor");
            // The mine owner's source relation is explicit setup. Collision,
            // visual allocation and capture use the actual shared callbacks.
            arena.entities[usize::from(mine)].record[54..56].copy_from_slice(&player.to_le_bytes());
            let flyer = arena
                .allocate_source_slot_with_terrain(&mut terrain)
                .unwrap();
            arena
                .economy
                .initialize_fireball_projectile(
                    flyer,
                    Level2FireballProjectileLaunchPayload {
                        origin: FireballProjectileOrigin::PlayerSpell,
                        position: Position {
                            y: position.y + 384,
                            ..position
                        },
                        yaw: 0,
                        pitch: 0,
                        axis_position: position,
                        source_id_entity_index: player,
                        holder_entity_index: if has_holder { holder } else { 0 },
                        source_subspell_index: 250,
                        source_mana: 20,
                        source_dword_0x10: 0,
                        target_source_entity_index: 0,
                        target_class: 10,
                        target_model: 78,
                    },
                    0,
                )
                .unwrap();
            if !visual_space {
                arena.economy.free_slots.clear();
                arena.finish_source_group();
            }
            let outcome = super::step_projectile(
                &mut arena,
                &mut terrain,
                particles,
                flyer,
                crate::native_world::source_target::Context {
                    spells: Some(session.shared_spells()),
                    player: Some((player, position, 10_000)),
                    ..crate::native_world::source_target::Context::default()
                },
                &[mine],
                5_120,
            )
            .unwrap();
            assert_eq!(outcome.collision_target, Some(mine));
            assert!(!outcome.live);
            assert_eq!(outcome.impact_slot.is_some(), visual_space);
            assert_eq!(outcome.captured_holder, has_holder.then_some(holder));
            assert_eq!(outcome.accuracy, None);
            assert_eq!(outcome.direct_hit_owner, None);
            let captured = &arena.entities[usize::from(mine)];
            assert_eq!(
                u16::from_le_bytes([captured.record[58], captured.record[59]]),
                if has_holder { 0 } else { u16::MAX }
            );
            assert!(
                arena
                    .economy
                    .fireball_projectiles()
                    .iter()
                    .any(|row| row.source_entity_index == flyer
                        && row.drawing_disabled
                        && !row.active)
            );
        }
    }

    #[test]
    fn fire_spheres_clear_retained_raw_rows_and_keep_source_constructor_chain() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let particles = &session.session.definition.presentation.particles;
        let slot = *arena.economy.free_source_slots().last().unwrap();
        // A retained packet at an ordinary free address must be cleared by
        // canonical NewEvent, rather than survive a typed-only allocation.
        let stale = &mut arena.entities[usize::from(slot)];
        stale.active = true;
        stale.class = 9;
        stale.model = 28;
        stale.set_life(-321);
        let generation = arena.economy.slot_generation(slot);
        let root = crate::world::effects::allocate_from_source_position(
            &mut arena,
            &mut terrain,
            sim_definition::adapter::mc2_effect_definition(
                sim_definition::level_definition::EffectKind::FireSpheres,
            ),
            slot,
            session.state.position,
            particles,
        )
        .unwrap()
        .expect("the ordinary free stack has 26 source rows");
        assert_eq!(root, slot);
        assert!(arena.economy.slot_generation(slot) > generation);
        assert_eq!(
            arena.entities[usize::from(slot)].class,
            0,
            "the old raw incarnation is retired with the typed constructor"
        );
        let source = arena
            .economy
            .authored_model76()
            .iter()
            .find(|row| row.source_entity_index == root)
            .unwrap();
        assert_eq!(
            source.position,
            crate::Position::default(),
            "the aliased source-position pointer sees NewEvent's zero position"
        );
        assert_eq!((source.model, source.action, source.life), (76, 83, 80));
        let children = arena
            .economy
            .model76_children()
            .iter()
            .filter(|row| row.parent_entity_index == root)
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 25);
        assert!(children.iter().all(|row| row.model == 77
            && row.action == 84
            && row.entity_id == root
            && row.entity_rng == children[0].entity_rng));
        assert_eq!(
            children.iter().filter(|row| row.column_index == 0).count(),
            5
        );
        assert!(
            children
                .iter()
                .all(|row| arena.entities[usize::from(row.source_entity_index)].class == 0)
        );
    }
}
