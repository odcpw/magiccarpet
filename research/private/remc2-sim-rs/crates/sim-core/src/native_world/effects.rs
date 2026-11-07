//! MC2 header views for retained effect carriers. Timers, RNG, descendants,
//! damage and terrain rules live in world::effects; this adapter only maps
//! the arena's existing typed storage to that one implementation.

use super::{EntitySnapshot, Level2DynamicArenaKind as Kind, ParticleParameter, SessionTerrain};
use crate::{Position, campaign::source_arena::SourceArena, world::effects as rules};

#[cfg(test)]
use crate::campaign::source_arena::SourceEntityHandle;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FireballTerminalReason {
    Collision,
    Terrain,
    Lifetime,
    Ash,
    Unknown,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireballTerminalReceipt {
    pub(crate) projectile: SourceEntityHandle,
    pub(crate) owner: u16,
    pub(crate) position: Position,
    pub(crate) reason: FireballTerminalReason,
    pub(crate) collision_target: Option<u16>,
    pub(crate) target_handle: Option<SourceEntityHandle>,
    pub(crate) impact: Option<SourceEntityHandle>,
    pub(crate) live_after: bool,
}

#[cfg(test)]
thread_local! {
    static DRAGON_REACTION_TRACE: std::cell::RefCell<Option<Vec<(u16, u32, u16, u16, u32)>>> = const { std::cell::RefCell::new(None) };
    static FIREBALL_TERMINAL_TRACE: std::cell::RefCell<Option<Vec<FireballTerminalReceipt>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_fireball_terminal_trace() {
    FIREBALL_TERMINAL_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_fireball_terminal_trace() -> Vec<FireballTerminalReceipt> {
    FIREBALL_TERMINAL_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
fn trace_fireball_terminal(
    arena: &SourceArena,
    receipt: Option<FireballTerminalReceipt>,
    impact: Option<u16>,
    live_after: bool,
) {
    let Some(mut receipt) = receipt else {
        return;
    };
    receipt.impact = impact.and_then(|slot| arena.handle(slot));
    receipt.live_after = live_after;
    FIREBALL_TERMINAL_TRACE.with(|trace| {
        if let Some(receipts) = trace.borrow_mut().as_mut()
            && receipts.len() < 64
        {
            receipts.push(receipt);
        }
    });
}

#[cfg(test)]
pub(crate) fn start_dragon_reaction_trace() {
    DRAGON_REACTION_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_dragon_reaction_trace() -> Vec<(u16, u32, u16, u16, u32)> {
    DRAGON_REACTION_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

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
    frozen_targets: Option<(&[u16], &[Vec<u16>; 29])>,
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
        let (class3, class5) = frozen_targets.unwrap_or((&[], &empty_models));
        let targets = super::guided_projectile::Targets {
            class3,
            class5,
            class10: capture_slots,
            owner_target_range,
        };
        let mut outcome = super::guided_projectile::StepOutcome::default();
        initial_capture = if frozen_targets.is_some() {
            super::guided_projectile::acquire(
                arena, &mut view, &targets, &mut outcome, context,
            )?
        } else {
            super::guided_projectile::acquire_capture(
            arena,
            &mut view,
            &targets,
            context,
            )?
        };
        #[cfg(test)]
        if initial_capture && super::source_target::read(arena, context, view.source_word_96())?
            .is_some_and(|row| (row.class, row.model) == (5, 0))
        {
            let target = view.source_word_96();
            DRAGON_REACTION_TRACE.with(|trace| {
                if let Some(trace) = trace.borrow_mut().as_mut() {
                    trace.push((slot, arena.economy.slot_generation(slot), projectile.owner_entity_index,
                        target, arena.economy.slot_generation(target)));
                }
            });
        }
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
        } else if frozen_targets.is_some() {
            // The real source performs this search only once, including a miss.
            let row = arena.economy.fireball_projectiles.iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or("ordinary acquisition lost its Fireball carrier")?;
            row.acquisition_attempted = true;
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
            if row.index == current_bound || arena.economy.map_chains.is_linked(row.index)
                || frozen_targets.is_some_and(|(class3, class5)| class3.contains(&row.index)
                    || class5.iter().any(|slots| slots.contains(&row.index))) {
                super::source_target::read(arena, context, row.index)
            } else {
                Ok(None)
            }
        })
        .collect::<Vec<_>>();
    // The source traverses the class heads frozen at the pass boundary, but
    // reads each candidate's current identity and byte39 during acquisition.
    // In particular, do not re-filter the frozen list by current life/action.
    let frozen_guidance = if let Some((class3, class5)) = frozen_targets.filter(|_| {
        entry.is_some_and(|row| !matches!((row.source_model, row.action), (0, 0) | (28, 29))
            && row.target_source_entity_index == 0 && !row.acquisition_attempted)
            && !initial_capture
    }) {
        let owner = entry.map_or(0, |row| row.owner_entity_index);
        let mut candidates = Vec::new();
        for (source_slot, creature) in class3.iter().copied().map(|slot| (slot, false))
            .chain(class5.iter().enumerate().filter(|(model, _)| *model != 22)
                .flat_map(|(_, slots)| slots.iter().copied().map(|slot| (slot, true))))
        {
            if creature && !super::guided_projectile::class5_eligible(arena, source_slot, owner, false) {
                continue;
            }
            let Some(current) = views.get(usize::from(source_slot))
                .ok_or("Fireball frozen target address is outside arena")?
                .as_ref().map_err(Clone::clone)? else { continue; };
            if !creature && current.control_flags & 0x20 != 0 { continue; }
            let top_position = if current.model == 2 {
                current.position
            } else {
                let shape = current.collision.ok_or_else(||
                    format!("Fireball acquisition target {source_slot} lacks current source axes"))?;
                super::guided_projectile::centre(current.position, current.model, shape.vertical_offset)
            };
            candidates.push(super::level2_dynamic_economy::Level2FireballGuidanceTarget {
                source_slot, source_id: current.source_id, class: current.class, model: current.model,
                stage_mode: current.relations.stage_mode.unwrap_or(0),
                parent_entity_index: current.relations.parent.unwrap_or(0), top_position,
            });
        }
        Some(candidates)
    } else { None };
    let mut step = arena.economy.advance_fireball_projectile_with_views(
        slot,
        terrain,
        &arena.entities,
        true,
        Some(&views),
        initial_capture,
        frozen_guidance.as_deref(),
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
    #[cfg(test)]
    let terminal_receipt = FIREBALL_TERMINAL_TRACE.with(|trace| {
        if !trace.borrow().as_ref().is_some_and(|rows| rows.len() < 64) {
            return None;
        }
        // The core checks actor collision, then terrain, then decrements life
        // only in its no-contact branch. Reflection returns before any birth.
        // Observe that actual write, never re-run a geometric collision test.
        let reason = match (step.collision_target, birth, entry) {
            (Some(_), _, _) => FireballTerminalReason::Collision,
            (_, FireballBirth::Ash, _) => FireballTerminalReason::Ash,
            (None, FireballBirth::Impact, Some(before))
                if projectile.life == before.life.wrapping_sub(1) && projectile.life < 0 =>
            {
                FireballTerminalReason::Lifetime
            }
            (None, FireballBirth::Impact, Some(before)) if projectile.life == before.life => {
                FireballTerminalReason::Terrain
            }
            _ => FireballTerminalReason::Unknown,
        };
        Some(FireballTerminalReceipt {
            projectile: SourceEntityHandle {
                source_slot: slot,
                generation: arena.economy.slot_generation(slot),
            },
            owner: projectile.owner_entity_index,
            position: projectile.position,
            reason,
            collision_target: step.collision_target,
            target_handle: step.collision_target.and_then(|target| arena.handle(target)),
            impact: None,
            live_after: step.live,
        })
    });
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
        #[cfg(test)]
        trace_fireball_terminal(arena, terminal_receipt, visual, step.live);
        return Ok(step);
    }
    let impact_sequence=if let Some(before)=entry.filter(|before|
        step.collision_target.is_some()||matches!(birth,FireballBirth::Ash)||projectile.life==before.life) {
        use crate::voxel_world::Point;
        let at=step.voxel_contact.map_or(Point{x:i32::from(projectile.position.x),y:i32::from(projectile.position.y),z:i32::from(projectile.position.z)},|contact|contact.surface);
        let normal=step.voxel_contact.map_or_else(||{
            // Local terrain is a stack of horizontal voxel tops. Source actor
            // collisions expose no surface normal, so leave those unknown.
            if step.collision_target.is_none() && terrain.voxel_world().is_some_and(|world|
                world.terrain_height(at.x,at.y)==Some(at.z)) {
                Point{x:0,y:0,z:1}
            } else { Point{x:0,y:0,z:0} }
        },|contact|contact.normal);
        if let Some(velocity)=step.incoming_velocity {
            terrain.voxel_impact(before.position,at,velocity,normal,slot,
                arena.economy.slot_generation(slot),projectile.source_model,step.collision_target)
        } else {None}
    } else {None};
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
    if let (Some(sequence),Some(id))=(impact_sequence,child)
        && (arena.entities[usize::from(id)].class,arena.entities[usize::from(id)].model)==(10,0) {
        terrain.bind_voxel_impact_child(sequence,id,arena.economy.slot_generation(id));
    }
    #[cfg(test)]
    if let Some(child) = child
        && (arena.entities[usize::from(child)].class, arena.entities[usize::from(child)].model) == (10, 0)
    {
        super::area_damage::trace_fireball_impact_birth(
            &arena.economy, child, slot, arena.entities[usize::from(child)].source_id,
        );
    }
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
    #[cfg(test)]
    trace_fireball_terminal(arena, terminal_receipt, child, step.live);
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
    collect!(arena.economy.authored_model59);
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

pub(super) fn checkpoint_packet(arena: &SourceArena, slot: u16) -> Result<EntitySnapshot, String> {
    header(arena, slot, Kind::AuthoredModel59, &[])
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
                    s.sprite_index,
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
            Kind::AuthoredModel59 => {
                let s = arena.economy.authored_model59.iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("checkpoint smoke emitter is absent")?;
                (s.entity_id, s.entity_rng, s.model, s.action, s.max_life,
                    s.life, 100, s.control_flags, s.phase, s.position, 0)
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
    let mut row = arena.entities[usize::from(slot)].clone();
    row.active = true;
    row.set_entity_rng(rng);
    row.set_position(position);
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
    if !matches!(kind, Kind::AuthoredModel17 | Kind::AuthoredModel59) {
        rules::bind_particle(&mut row, base_particle, particles)?;
    }
    match kind {
        Kind::AuthoredModel59 => {
            let s = arena.economy.authored_model59.iter()
                .find(|s| s.source_entity_index == slot).unwrap();
            row.set_actual_speed(s.actual_speed);
        }
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
            row.set_actual_speed(s.actual_speed);
            row.record[96] = s.animation_frame;
            if let Some(limit) = s.animation_limit {
                row.record[97] = limit;
            }
            if let Some(axes) = s.collision_axes {
                for (offset, value) in [86, 88, 90, 92].into_iter().zip(axes) {
                    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
                }
            }
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
        Kind::AuthoredModel59 => {
            let s = arena.economy.authored_model59.iter_mut()
                .find(|s| s.source_entity_index == slot).unwrap();
            common!(s);
            s.actual_speed = row.actual_speed;
            s.drawing_enabled = !row.drawing_disabled();
        }
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
            s.dword_0x10 = row.source_dword_10();
            s.actual_speed = row.actual_speed;
            s.sprite_index = row.particle_parameter_index.unwrap_or(s.sprite_index);
            s.animation_frame = row.record[96];
            s.animation_limit = Some(row.record[97]);
            if s.collision_axes.is_some() {
                s.collision_axes = Some([
                    row.array_yaw(),
                    row.array_pitch(),
                    row.array_roll(),
                    row.array_fov(),
                ]);
            }
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

/// After hidden map unlink, retain final shared-effect packets before their
/// typed carriers are freed. Model17 also supports external source hiding.
pub(crate) fn prepare_retained_effects_reclaim(arena: &mut SourceArena) -> Result<(), String> {
    let trails = arena.economy.speed_trails.iter()
        .filter(|source| !source.active && source.control_flags & 0x400 != 0)
        .copied().collect::<Vec<_>>();
    for source in trails {
        let slot = source.source_entity_index;
        if arena.economy.free_source_slots().contains(&slot)
            || arena.economy.live_arena_kind(slot).is_some()
        {
            continue;
        }
        // This constructor never map-links. Its final pose/life/phase still
        // belong to the hidden typed row until the following owning sweep.
        let mut row = super::source_target::speed_trail_packet(&source);
        row.reclaim_source_row();
        arena.entities[usize::from(slot)] = row;
    }
    let checkpoints = arena.economy.authored_model59.iter()
        .filter(|source| source.active && source.control_flags & 0x400 != 0)
        .map(|source| source.source_entity_index).collect::<Vec<_>>();
    for slot in checkpoints {
        if arena.economy.live_arena_kind(slot) != Some(Kind::AuthoredModel59) { continue; }
        let mut row = checkpoint_packet(arena, slot)?;
        row.reclaim_source_row();
        arena.entities[usize::from(slot)] = row;
    }
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
        row.set_control_flags(row.control_flags & !4);
        row.reclaim_source_row();
        arena.entities[usize::from(slot)] = row;
    }
    for (slot, kind) in arena.economy.live_arena_slots() {
        if matches!(kind, Kind::DeathBurst | Kind::AuthoredModel1 | Kind::AuthoredModel5WaterSplash
            | Kind::AuthoredModel6 | Kind::AuthoredModel13 | Kind::Model13SpawnedChild
            | Kind::Model59SmokeChild | Kind::Model17SpawnedChild)
        {
            let row = &mut arena.entities[usize::from(slot)];
            if row.class == 0 && row.drawing_disabled() {
                row.set_control_flags(row.control_flags & !4);
            }
        }
    }
    Ok(())
}

/// Publish the last current flyer/contact packet before its owning sweep
/// removes the typed carrier. `sub_57F20` clears class, not the old payload.
pub(crate) fn prepare_spell_flyers_reclaim(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> Result<(), String> {
    let current = |slot, kind| {
        arena.economy.live_arena_kind(slot).is_none()
            && arena.economy.retained_projectile_kind(slot) == Some(kind)
    };
    let castles = arena.economy.castle_projectiles.iter()
        .filter(|row| !row.active && row.drawing_disabled
            && current(row.source_entity_index, Kind::CastleProjectile))
        .copied().collect::<Vec<_>>();
    let possessions = arena.economy.possession_projectiles.iter()
        .filter(|row| !row.active && current(row.source_entity_index, Kind::PossessionProjectile))
        .copied().collect::<Vec<_>>();
    let contacts = arena.economy.possession_effects.iter()
        .filter(|row| !row.active && current(row.source_entity_index, Kind::PossessionEffect))
        .copied().collect::<Vec<_>>();
    for source in castles {
        let slot = source.source_entity_index;
        let links = (arena.economy.map_chains.old_map_entity(slot), arena.economy.map_chains.next_entity(slot));
        rules::sync_position(arena, terrain, slot, source.position, false, false)?;
        let mut row = super::source_target::status_parent(arena, Default::default(), slot)?
            .ok_or_else(|| format!("hidden Castle flyer {slot} lost its current packet"))?;
        row.set_map_links(links.0, links.1);
        row.set_entity_rng(source.entity_rng);
        row.set_source_action(10);
        row.record[68] = source.source_model;
        row.record[30..32].copy_from_slice(&source.owner_entity_index.to_le_bytes());
        row.record[42..44].copy_from_slice(&source.holder_entity_index.to_le_bytes());
        row.set_subspell_index(source.source_subspell_index);
        row.set_source_dword_10(i32::from(source.source_a4_byte_154));
        row.set_max_life(source.max_life);
        if let Some(phase) = arena.economy.source_phase_sidecar(slot) {
            row.set_phase(phase);
        }
        row.set_source_roll(source.source_roll);
        row.fov = source.source_fov;
        row.record[38..40].copy_from_slice(&source.source_fov.to_le_bytes());
        row.record[71] = source.source_byte_43;
        row.record[72] = source.source_byte_44;
        row.set_source_word_96(source.source_word_96);
        for (offset, value) in [(158, source.axis_position.x), (160, source.axis_position.y), (162, source.axis_position.z.cast_unsigned())] {
            row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        row.reclaim_source_row();
        row.set_control_flags(row.control_flags & !4);
        arena.entities[usize::from(slot)] = row;
    }
    for source in possessions {
        let slot = source.source_entity_index;
        let links = (arena.economy.map_chains.old_map_entity(slot), arena.economy.map_chains.next_entity(slot));
        rules::sync_position(arena, terrain, slot, source.position, false, false)?;
        let mut row = super::source_target::status_parent(arena, Default::default(), slot)?
            .ok_or_else(|| format!("hidden Possession flyer {slot} lost its current packet"))?;
        row.set_map_links(links.0, links.1);
        row.set_entity_rng(source.entity_rng);
        row.set_source_action(source.action);
        row.record[68] = source.source_model;
        row.record[30..32].copy_from_slice(&source.source_id_entity_index.to_le_bytes());
        row.record[42..44].copy_from_slice(&source.holder_entity_index.to_le_bytes());
        row.set_mana(source.source_mana);
        row.set_source_dword_10(source.source_dword_10);
        row.set_max_life(i32::from(source.max_life));
        row.set_phase(source.phase);
        row.set_source_roll(source.roll);
        row.fov = source.fov;
        row.record[38..40].copy_from_slice(&source.fov.to_le_bytes());
        row.record[69] = source.output_class;
        row.record[70] = source.output_model;
        row.set_source_word_96(source.target_source_entity_index);
        for (offset, value) in [(158, source.axis_position.x), (160, source.axis_position.y), (162, source.axis_position.z.cast_unsigned())] {
            row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        row.reclaim_source_row();
        row.set_control_flags(row.control_flags & !4);
        arena.entities[usize::from(slot)] = row;
    }
    for source in contacts {
        let slot = source.source_entity_index;
        let flags = super::source_target::read(arena, Default::default(), slot)?
            .ok_or_else(|| format!("hidden Possession contact {slot} lost its current packet"))?
            .control_flags;
        let links = (arena.economy.map_chains.old_map_entity(slot), arena.economy.map_chains.next_entity(slot));
        rules::sync_position(arena, terrain, slot, source.position, false, false)?;
        let row = &mut arena.entities[usize::from(slot)];
        row.set_map_links(links.0, links.1);
        row.model = source.source_model;
        row.record[68] = source.source_model;
        row.source_id = source.source_id_entity_index;
        row.record[30..32].copy_from_slice(&source.source_id_entity_index.to_le_bytes());
        row.set_source_action(source.action);
        row.set_entity_rng(source.entity_rng);
        row.set_position(source.position);
        row.set_yaw(source.yaw);
        row.set_pitch(source.pitch);
        row.set_life(i32::from(source.life));
        row.set_max_life(i32::from(source.max_life));
        row.set_phase(source.phase);
        if matches!(source.source_model, 12 | 70) {
            row.set_source_dword_10(source.source_dword_10);
        }
        row.set_control_flags(flags);
        row.reclaim_source_row();
    }
    Ok(())
}

/// `sub_57F20` clears only class at a hidden Fire Spheres address. Preserve
/// the current root and sector packets until a later NewEvent overwrites them.
pub(crate) fn prepare_fire_spheres_reclaim(arena: &mut SourceArena) -> Result<(), String> {
    let roots = arena.economy.authored_model76().iter()
        .filter(|row| row.active && row.control_flags & 0x400 != 0
            && arena.economy.live_arena_kind(row.source_entity_index) == Some(Kind::AuthoredModel76))
        .copied().collect::<Vec<_>>();
    let children = arena.economy.model76_children().iter()
        .filter(|row| row.active
            && arena.economy.live_arena_kind(row.source_entity_index) == Some(Kind::Model76Child))
        .copied().collect::<Vec<_>>();
    for source in roots {
        let mut row = arena.entities[usize::from(source.source_entity_index)].clone();
        row.set_entity_rng(source.entity_rng);
        row.set_position(source.position);
        row.source_id = source.entity_id;
        row.record[30..32].copy_from_slice(&source.entity_id.to_le_bytes());
        row.model = source.model;
        row.record[68] = source.model;
        row.set_source_action(source.action);
        row.set_max_life(i32::from(source.max_life));
        row.set_life(i32::from(source.life));
        row.set_subspell_index(source.subspell_index);
        row.set_control_flags(source.control_flags & !4);
        row.set_phase(source.phase);
        row.set_yaw(source.yaw);
        row.set_pitch(source.pitch);
        row.set_actual_speed(source.actual_speed);
        row.record[136..138].copy_from_slice(&source.minimum_speed.to_le_bytes());
        row.record[138..140].copy_from_slice(&source.maximum_speed.to_le_bytes());
        row.set_source_word_2c(source.word_0x2c);
        row.font_type_index = source.font_type_index as u8;
        row.record[65] = source.font_type_index as u8;
        row.set_source_byte_46(source.state);
        row.set_source_word_96(source.target_entity_index);
        row.set_source_overlap_half_extents(source.shift_rot_x, source.shift_rot_x);
        row.record[92..94].copy_from_slice(&source.shift_rot_y.to_le_bytes());
        let next = children.iter().filter(|child| child.parent_entity_index == source.source_entity_index)
            .min_by_key(|child| child.chain_index).map_or(0, |child| child.source_entity_index);
        row.record[56..58].copy_from_slice(&next.to_le_bytes());
        row.reclaim_source_row();
        arena.entities[usize::from(source.source_entity_index)] = row;
    }
    for source in children.iter().filter(|row| row.control_flags & 0x400 != 0) {
        let mut row = arena.entities[usize::from(source.source_entity_index)].clone();
        row.set_entity_rng(source.entity_rng);
        row.set_position(source.position);
        row.source_id = source.entity_id;
        row.record[30..32].copy_from_slice(&source.entity_id.to_le_bytes());
        row.model = source.model;
        row.record[68] = source.model;
        row.set_source_action(source.action);
        row.set_max_life(i32::from(source.max_life));
        row.set_life(i32::from(source.life));
        row.set_subspell_index(source.subspell_index);
        row.set_control_flags(source.control_flags & !4);
        row.set_phase(source.phase);
        row.set_yaw(source.yaw);
        row.set_pitch(source.pitch);
        row.set_source_roll(source.roll);
        row.fov = source.fov;
        row.record[38..40].copy_from_slice(&source.fov.to_le_bytes());
        row.set_actual_speed(source.actual_speed);
        row.record[136..138].copy_from_slice(&source.minimum_speed.to_le_bytes());
        row.record[138..140].copy_from_slice(&source.maximum_speed.to_le_bytes());
        row.set_source_word_2c(source.word_0x2c);
        row.record[71] = source.row_index;
        row.record[72] = source.column_index;
        row.record[86..88].copy_from_slice(&source.collision_yaw.to_le_bytes());
        row.record[88..90].copy_from_slice(&source.collision_pitch.to_le_bytes());
        row.record[90..92].copy_from_slice(&source.collision_roll.to_le_bytes());
        row.record[92..94].copy_from_slice(&source.collision_fov.to_le_bytes());
        row.particle_parameter_index = Some(source.visual_index);
        row.record[94..96].copy_from_slice(&source.visual_index.to_le_bytes());
        row.record[98..102].copy_from_slice(&source.pending_damage.to_le_bytes());
        row.record[102..104].copy_from_slice(&source.pending_damage_owner.to_le_bytes());
        let previous = children.iter().filter(|child| child.parent_entity_index == source.parent_entity_index
            && child.chain_index < source.chain_index).max_by_key(|child| child.chain_index)
            .map_or(source.parent_entity_index, |child| child.source_entity_index);
        let next = children.iter().filter(|child| child.parent_entity_index == source.parent_entity_index
            && child.chain_index > source.chain_index).min_by_key(|child| child.chain_index)
            .map_or(0, |child| child.source_entity_index);
        row.record[54..56].copy_from_slice(&previous.to_le_bytes());
        row.record[56..58].copy_from_slice(&next.to_le_bytes());
        row.reclaim_source_row();
        arena.entities[usize::from(source.source_entity_index)] = row;
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
    if kind == Kind::AuthoredModel59 && let Some(child) = visit.smoke_child {
        // SetSmoke4 has already initialized the one canonical source row.
        // Retain that packet in the existing MC2 smoke carrier; no second
        // allocation, RNG draw, constructor or parent-ID write occurs here.
        let child = arena.entities[usize::from(child)].clone();
        arena.economy.adopt_checkpoint_smoke(&child);
    }
    if arena.economy.slot_generation(slot) != generation {
        // NewEvent replaced this typed parent during its own callback. The
        // source pointer now denotes the child; never restore the old carrier.
        rules::commit_entity(arena, terrain, row)?;
    } else {
        store(arena, kind, &row);
        arena.economy.source_rng.insert(slot, row.entity_rng);
        rules::sync_position(
            arena,
            terrain,
            slot,
            row.position,
            row.active && row.control_flags & 4 != 0,
            false,
        )?;
        if row.drawing_disabled() {
            if let Some(parent) = burst_parent {
                arena.economy.record_raw_death_burst_expiry(slot, parent);
            }
            // The retained typed carrier remains authoritative while hidden.
            // Keep its eventual class-zero bytes for fixed-address readers
            // after the following source reclaim removes that carrier.
            let mut reclaimed = row.clone();
            reclaimed.set_map_links(arena.economy.map_chains.old_map_entity(slot),
                arena.economy.map_chains.next_entity(slot));
            reclaimed.reclaim_source_row();
            arena.entities[usize::from(slot)] = reclaimed;
        }
    }
    Ok(visit)
}

#[cfg(test)]
mod tests {
    #[test]
    fn voxel_fireball_wall_breach_actor_and_reflection_controls() {
        use crate::Position;
        use crate::native_world::{FireballProjectileOrigin,level2_dynamic_economy::Level2FireballProjectileLaunchPayload};
        use crate::voxel_world::{Grid,Point,Volume,VoxelWorld};

        let session=super::super::tests::level_two_campaign_session();
        let particles=&session.session.definition.presentation.particles;
        let masks=session.collision_mask_bank().expect("qualified owner-package collision-mask bank");
        let owner=session.state.entity_index;
        let mut terrain=session.canonical_terrain().clone();
        let mut arena=session.source_arena().clone();
        // Explicit staging: choose an unused original raw address for the
        // BUILD owner, and allocate an ordinary current actor in open sky.
        let house=arena.entities.iter().find(|row| row.index!=0 && !row.active
            && arena.economy.slot_generation(row.index)==0
            && arena.economy.live_arena_cursor_row(row.index).is_none()).unwrap().index;
        arena.economy.free_slots.retain(|slot|*slot!=house);
        let actor=arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let wall=Position{x:62056,y:62048,z:8000};
        let behind=Position{x:62180,..wall};
        let mut legacy=crate::world::terrain::cave_features::new_event(house,0,wall);
        legacy.class=10;legacy.model=45;legacy.record[67]=10;legacy.record[68]=45;
        legacy.level_gate=26;legacy.record[74]=26;
        legacy.set_life(1000);legacy.set_control_flags(8);
        legacy.set_source_overlap_half_extents(256,256);
        legacy.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
        arena.entities[usize::from(house)]=legacy;
        crate::world::effects::sync_position(&mut arena,&mut terrain,house,wall,true,true).unwrap();
        let mut target=crate::world::terrain::cave_features::new_event(actor,0,behind);
        target.class=5;target.model=1;target.record[67]=5;target.record[68]=1;
        target.record[60]=1;target.set_life(1000);target.set_control_flags(8);
        target.set_source_overlap_half_extents(16,16);
        target.record[92..94].copy_from_slice(&16_i16.to_le_bytes());
        arena.entities[usize::from(actor)]=target;
        crate::world::effects::sync_position(&mut arena,&mut terrain,actor,behind,true,true).unwrap();
        let pristine_arena=arena.clone();let pristine_terrain=terrain.clone();
        let grid=Grid{origin:Point{x:62048,y:62040,z:7992},cell_size:16,dimensions:[1,1,1]};
        terrain.install_voxel_world(VoxelWorld::new(1,vec![Volume::building(house,grid,[[0,0,0]]).unwrap()]).unwrap());

        fn fire(arena:&mut crate::campaign::source_arena::SourceArena,
                terrain:&mut crate::native_world::SessionTerrain,
                particles:&[crate::native_world::ParticleParameter],owner:u16,speed:i16)
            -> crate::native_world::level2_dynamic_economy::Level2FireballStep {
            let start=Position{x:61950,y:62048,z:8000};
            let flyer=arena.allocate_source_slot_with_terrain(terrain).unwrap();
            arena.economy.initialize_fireball_projectile(flyer,Level2FireballProjectileLaunchPayload{
                origin:FireballProjectileOrigin::PlayerSpell,position:start,yaw:512,pitch:0,axis_position:start,
                source_id_entity_index:owner,holder_entity_index:0,source_subspell_index:250,source_mana:20,
                source_dword_0x10:0,target_source_entity_index:0,target_class:0,target_model:0,
            },0).unwrap();
            let row=arena.economy.fireball_projectiles.iter_mut().find(|row|row.source_entity_index==flyer).unwrap();
            row.actual_speed=speed;row.collision_axes=Some([0,8,8,8]);row.acquisition_attempted=true;
            super::step_projectile(arena,terrain,particles,flyer,Default::default(),&[],0,None).unwrap()
        }

        let blocked=fire(&mut arena,&mut terrain,particles,owner,230);
        assert_eq!(blocked.collision_target,None);
        assert!(blocked.voxel_contact.is_some());
        assert!(terrain.voxel_impacts().last().unwrap().geometry_changed);
        assert_eq!(arena.entities[usize::from(actor)].source_wind_spell_payload(),0);
        let through=fire(&mut arena,&mut terrain,particles,owner,230);
        assert_eq!(through.collision_target,Some(actor),"breach must not retain the legacy BUILD collider");
        let impact=through.impact_slot.expect("actual impact constructor");
        let source=arena.entities[usize::from(impact)].clone();
        let lists=crate::native_world::area_damage::capture_effect_lists(&mut arena,None);
        let mut state=crate::world::effects::State::default();
        crate::world::effects::visit_entity(&mut arena,&mut terrain,source,particles,&mut state,|arena,pulse|
            crate::native_world::area_damage::apply_effect(arena,None,None,Some(masks),&lists,pulse)).unwrap();
        assert!(arena.entities[usize::from(actor)].source_wind_spell_payload()>0,
            "the actual impact callback must write the actor damage mailbox");

        let mut original=pristine_arena.clone();let mut old_terrain=pristine_terrain.clone();
        let original_hit=fire(&mut original,&mut old_terrain,particles,owner,230);
        assert!(original_hit.voxel_contact.is_none(),"disabled mode keeps source behavior");
        assert!(matches!(original_hit.collision_target,Some(slot) if slot==house || slot==actor));
        let mut front=pristine_arena;let mut front_terrain=pristine_terrain;
        front_terrain.install_voxel_world(VoxelWorld::new(1,vec![Volume::building(house,grid,[[0,0,0]]).unwrap()]).unwrap());
        let front_position=Position{x:62020,..wall};
        front.entities[usize::from(actor)].set_position(front_position);
        front.entities[usize::from(actor)].set_control_flags(0x18);
        front.entities[usize::from(actor)].set_mana(1000);
        crate::world::effects::sync_position(&mut front,&mut front_terrain,actor,front_position,true,true).unwrap();
        let reflected=fire(&mut front,&mut front_terrain,particles,owner,70);
        assert!(reflected.reflection.is_some(),"a reflector in front of the wall keeps source precedence");
        assert!(reflected.voxel_contact.is_none());
    }

    #[test]
    fn default_water_splash_animation_uses_retained_particle_binding() {
        let session = super::super::tests::level_two_campaign_session();
        let particles = &session.session.definition.presentation.particles;
        let expected_limit = particles[244].animation_limit().unwrap();
        assert_eq!(expected_limit, 11, "actual retained particle244 animation");
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        // This is an explicit shared-constructor control. Both existing
        // consumers receive the genuine owner particle data; no callback
        // state or lifetime is shortened to exercise the animation.
        let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let source = arena.economy.initialize_authored_model5_water_splash(
            slot,
            session.state.position,
            &terrain,
        ).unwrap();
        assert_eq!(source.animation_limit, None);
        let mut direct = arena.economy.clone();
        let mut state = crate::world::effects::State::default();
        for expected_frame in 1..=2 {
            assert!(direct.step_authored_model5_water_splash_slot(slot, particles));
            super::visit(
                &mut arena,
                &mut terrain,
                slot,
                super::Kind::AuthoredModel5WaterSplash,
                particles,
                &mut state,
                |_, _| Ok(0),
            ).unwrap();
            let scheduled = arena.economy.authored_model5_water_splashes().iter()
                .find(|row| row.source_entity_index == slot).unwrap();
            let fallback = direct.authored_model5_water_splashes().iter()
                .find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!(scheduled.sprite_index, 244);
            assert_eq!(scheduled.animation_limit, Some(expected_limit));
            assert_eq!(scheduled.animation_frame, expected_frame);
            assert_eq!(fallback.animation_frame, expected_frame);
            assert_eq!(scheduled.life, fallback.life);
            assert_eq!(scheduled.actual_speed, 16);
        }
    }

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
                None,
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
