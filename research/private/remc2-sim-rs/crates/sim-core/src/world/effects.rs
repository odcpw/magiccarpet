//! Fire, impact and smoke behavior over the authoritative source arena.
//!
//! This owner has no session access. The source adapter supplies immutable
//! content and retained particle rows; the scheduler applies common cadence,
//! publishes the returned sound/damage operations, and reclaims hidden rows.

pub(crate) mod lighting;

use crate::level_definition::{EffectDefinition, EffectKind};
use crate::native_world::{
    EntitySnapshot, Level2AuthoredModel76Source, Level2Model76Child, ParticleParameter,
    SessionTerrain,
};
use crate::renderable_map_chain::RenderableMapSeed;
use crate::{FlightWorld, Position, campaign::source_arena::SourceArena, move_entity};
use std::collections::BTreeMap;

/// Source descriptor shared by both ordinary model27 projectile births.
/// The typed Fireball origin selects this rule without a duplicate mutable
/// pointer; the raw Lightning row retains its source address.
pub(crate) struct Model27Guidance {
    pub source_index: u16,
    pub yaw_turn: i16,
    pub pitch_turn: i16,
}

pub(crate) const MODEL27_GUIDANCE: Model27Guidance = Model27Guidance {
    source_index: 106,
    yaw_turn: 17,
    pitch_turn: 113,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DamagePulse {
    pub(crate) position: Position,
    pub(crate) owner: u16,
    pub(crate) payload: u16,
    pub(crate) half_width: i16,
    pub(crate) vertical_offset: i16,
    pub(crate) half_height: i16,
    /// sub_11400 writes the source damage mailbox; sub_10C80 also reports hits.
    pub(crate) continuous: bool,
    pub(crate) selector: (u8, u8),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) damage: Vec<DamagePulse>,
    pub(crate) sounds: Vec<u16>,
    pub(crate) nearest_fire: Option<Position>,
    pub(crate) experience: Vec<(u16, u8, u16)>,
}

/// Source-global eruption ownership, reset with the arena, never inferred
/// from presentation or duplicated in the client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    pub(crate) eruption: u16,
    pub(crate) eruption_smoke: u16,
    // Observation provenance only: gameplay state stays in the actual raw
    // effect row. Generation prevents a reused address inheriting its parent.
    death_bursts: BTreeMap<u16, (u32, u16)>,
}

impl State {
    pub(crate) fn from_globals(eruption: u16, eruption_smoke: u16) -> Self {
        Self {
            eruption,
            eruption_smoke,
            ..Self::default()
        }
    }

    pub(crate) fn forget_source_slot(&mut self, slot: u16) {
        self.death_bursts.remove(&slot);
    }

    pub(crate) fn death_burst_origins(&self) -> &BTreeMap<u16, (u32, u16)> {
        &self.death_bursts
    }

    /// A callback can reclaim or replace a burst while this State is
    /// temporarily borrowed out of SourceArena. Reclaim leaves the generation
    /// unchanged until the next allocation, so check the free stack too.
    pub(crate) fn retain_current_generations(&mut self, arena: &SourceArena) {
        self.death_bursts.retain(|slot, (generation, _)| {
            arena.economy.slot_generation(*slot) == *generation
                && !arena.economy.free_source_slots().contains(slot)
        });
    }
}

/// Register only an actual successfully allocated raw 10/1 death product.
/// The source id is read from that live row after its producer initializes it.
pub(crate) fn register_death_burst(
    arena: &mut SourceArena,
    slot: u16,
    parent: u16,
) -> Result<(), String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 1, 1))
        .ok_or_else(|| format!("death burst {slot} is not a live raw 10/1 product"))?;
    let id = row.source_id;
    let generation = arena.economy.slot_generation(slot);
    if arena.effects.death_bursts.contains_key(&slot) {
        return Err(format!("death burst {slot} is already registered"));
    }
    arena
        .effects
        .death_bursts
        .insert(slot, (generation, parent));
    arena.economy.record_raw_death_burst_spawn(slot, parent, id);
    Ok(())
}

/// The combat adapter owns actor carriers and mailboxes. Effects add only
/// 10C80's separate MSPRD-backed class-10/model-45 source-list traversal.
pub(crate) trait DamageWorld: crate::combat::area::AreaWorld {
    fn model45_slots(&self) -> Vec<u16>;
    fn model45_contact(&self, request: DamagePulse, slot: u16) -> Result<bool, String>;
}

fn damage_overlap(request: DamagePulse, target: crate::combat::area::Actor) -> bool {
    let dx = i32::from(target.position.x.wrapping_sub(request.position.x) as i16).abs();
    let dy = i32::from(target.position.y.wrapping_sub(request.position.y) as i16).abs();
    let dz = (i32::from(request.position.z) + i32::from(request.vertical_offset)
        - i32::from(target.position.z)
        - i32::from(target.shape.vertical_offset))
    .abs();
    dx < i32::from(request.half_width) + i32::from(target.shape.half_width)
        && dy < i32::from(request.half_width) + i32::from(target.shape.half_depth)
        && dz < i32::from(request.half_height) + i32::from(target.shape.half_height)
}

/// Exact mode-zero 10C80/11400 policies, sharing authoritative combat
/// carriers without treating either policy as 116A0. Only 10C80 returns the
/// hit count consumed by Meteor's experience award.
pub(crate) fn apply_damage(
    world: &mut impl DamageWorld,
    request: DamagePulse,
) -> Result<u16, String> {
    let mut hits = 0_u16;
    for slot in world.class3_slots() {
        if let Some(target) = world.actor(slot)
            && (target.class, target.model) == (3, 2)
            && target.source_id != request.owner
            && damage_overlap(request, target)
        {
            world.ordinary_damage(slot, request.payload, request.owner)?;
            hits = hits.wrapping_add(1);
        }
    }
    if !request.continuous {
        for slot in world.model45_slots() {
            // The source special list deliberately has no owner rejection.
            if world.model45_contact(request, slot)? {
                world.ordinary_damage(slot, request.payload, request.owner)?;
                hits = hits.wrapping_add(1);
            }
        }
    }
    let center_x = (i32::from(request.position.x) + 128) / 256;
    let center_y = (i32::from(request.position.y) + 128) / 256;
    let radius = (i32::from(request.half_width) + 255) / 256;
    for y in -radius..=radius {
        for x in -radius..=radius {
            for slot in world.cell_slots((center_x + x) as u8, (center_y + y) as u8) {
                let Some(target) = world.actor(slot) else {
                    continue;
                };
                let selected = request.selector.0 == u8::MAX
                    || (request.selector.0 == target.class
                        && (request.selector.1 == u8::MAX || request.selector.1 == target.model));
                if target.source_id != request.owner
                    && (target.class, target.model) != (3, 2)
                    && (request.continuous || (target.class, target.model) != (10, 45))
                    && target.damage_mask & 1 != 0
                    && target.control_flags & 8 != 0
                    && selected
                    && damage_overlap(request, target)
                {
                    let damage = if request.continuous && (target.class, target.model) == (2, 0) {
                        request.payload / 10
                    } else {
                        request.payload
                    };
                    world.ordinary_damage(slot, damage, request.owner)?;
                    hits = hits.wrapping_add(1);
                }
            }
        }
    }
    Ok(if request.continuous { 0 } else { hits })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SpheresStep {
    pub(crate) collision: bool,
    pub(crate) tail: bool,
}

fn orbit_spheres(source: &mut Level2AuthoredModel76Source, children: &mut [Level2Model76Child]) {
    source.yaw = source.yaw.wrapping_add(22) & 2047;
    source.pitch = source.pitch.wrapping_add(16) & 2047;
    for child in children
        .iter_mut()
        .filter(|row| row.active && row.parent_entity_index == source.source_entity_index)
    {
        child.yaw = child.yaw.wrapping_add(child.roll) & 2047;
        child.pitch = child.pitch.wrapping_add(child.fov) & 2047;
        let mut position = source.position;
        move_entity(
            &mut position,
            child.yaw.wrapping_add(source.yaw) & 2047,
            child.pitch.wrapping_add(source.pitch) & 2047,
            source.word_0x2c,
        );
        child.position = position;
        // Action84 is disabled. Root movement never advances child byte3E.
    }
}

/// sub_339B0 owns the complete chain. A target becoming invalid during the
/// ordinary 0/1 branch still receives this branch's final orbit/collision;
/// the following callback takes the separate shrinking branch.
pub(crate) fn prepare_spheres(
    source: &mut Level2AuthoredModel76Source,
    children: &mut [Level2Model76Child],
    terrain: &SessionTerrain,
) -> SpheresStep {
    match source.state {
        0 => {
            if source.target_entity_index != 0 {
                source.maximum_speed = (source.target_pitch >> 1).max(128);
                source.minimum_speed = ((6 * i32::from(source.target_pitch)) >> 2) as i16;
                source.minimum_speed = source.minimum_speed.min(640);
            }
            source.state = 1;
        }
        1 => {}
        2 => {
            if source.font_type_index < 0 {
                source.font_type_index = source.font_type_index.wrapping_neg();
            }
            orbit_spheres(source, children);
            source.word_0x2c = source
                .word_0x2c
                .wrapping_sub(i16::from(source.font_type_index));
            let tail = source.word_0x2c < 0;
            return SpheresStep {
                collision: false,
                tail,
            };
        }
        _ => return SpheresStep::default(),
    }
    if source.target_entity_index != 0 {
        source.position = source.target_position;
        source.position.z = source.position.z.wrapping_add(source.target_height_offset);
        if source.target_life < 0 || source.target_control_flags & 0x400 != 0 {
            source.state = 2;
        }
    }
    let minimum = i32::from(terrain.ground_height(source.position)) + i32::from(source.word_0x2c);
    if i32::from(source.position.z) < minimum {
        source.position.z = minimum as i16;
    }
    if let Some(ceiling) = terrain.ceiling_height(source.position) {
        let maximum = i32::from(ceiling) - i32::from(source.word_0x2c);
        if i32::from(source.position.z) > maximum {
            source.position.z = maximum as i16;
        }
    }
    source.word_0x2c = source
        .word_0x2c
        .wrapping_add(i16::from(source.font_type_index));
    if source.word_0x2c > source.minimum_speed {
        source.word_0x2c = source.minimum_speed;
        source.font_type_index = source.font_type_index.wrapping_neg();
    } else if source.word_0x2c < source.maximum_speed {
        source.word_0x2c = source.maximum_speed;
        source.font_type_index = source.font_type_index.wrapping_neg();
    }
    orbit_spheres(source, children);
    SpheresStep {
        collision: true,
        tail: false,
    }
}

pub(crate) fn visit_spheres<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    particles: &[ParticleParameter],
    mut apply: F,
) -> Result<Visit, String>
where
    F: FnMut(&mut SourceArena, DamagePulse) -> Result<u16, String>,
{
    let index = arena
        .economy
        .authored_model76()
        .iter()
        .position(|row| row.active && row.source_entity_index == source_slot)
        .ok_or_else(|| format!("Fire Spheres root {source_slot} is not live"))?;
    let mut source = arena.economy.authored_model76()[index];
    let outcome = prepare_spheres(
        &mut source,
        arena.economy.effect_sphere_children_mut(),
        terrain,
    );
    arena.economy.effect_sphere_roots_mut()[index] = source;
    let children = arena
        .economy
        .model76_children()
        .iter()
        .filter(|row| row.active && row.parent_entity_index == source_slot)
        .copied()
        .collect::<Vec<_>>();
    for child in &children {
        sync_position(
            arena,
            terrain,
            child.source_entity_index,
            child.position,
            true,
            false,
        )?;
    }
    sync_position(arena, terrain, source_slot, source.position, true, false)?;
    let mut feedback = Visit::default();
    if outcome.collision {
        for child in children.iter().filter(|row| row.column_index == 0) {
            if apply(
                arena,
                DamagePulse {
                    position: child.position,
                    owner: child.entity_id,
                    payload: source.subspell_index,
                    half_width: child.collision_pitch,
                    vertical_offset: child.collision_yaw,
                    half_height: child.collision_fov,
                    continuous: false,
                    selector: (u8::MAX, u8::MAX),
                },
            )? != 0
            {
                feedback.sounds.push(3);
            }
        }
        let root = &mut arena.economy.effect_sphere_roots_mut()[index];
        root.life = root.life.wrapping_sub(1);
        if root.life < 1 {
            root.state = 2;
        }
    }
    if outcome.tail {
        // Source does not copy root ownership into this ordinary NewEvent.
        allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
            source.position,
            particles,
        )?;
        let root = &mut arena.economy.effect_sphere_roots_mut()[index];
        root.control_flags |= 0x400;
        root.drawing_enabled = false;
        for child in arena
            .economy
            .effect_sphere_children_mut()
            .iter_mut()
            .filter(|row| row.active && row.parent_entity_index == source_slot)
        {
            child.control_flags |= 0x400;
            child.drawing_enabled = false;
        }
    }
    Ok(feedback)
}

pub(crate) const fn source_model(kind: EffectKind) -> u8 {
    match kind {
        EffectKind::Impact => 0,
        EffectKind::RadialExplosion => 1,
        EffectKind::Expiry => 2,
        EffectKind::ShortVisual => 3,
        EffectKind::DormantGroundMarker => 4,
        EffectKind::Ash => 5,
        EffectKind::GroundFire => 6,
        EffectKind::ImmediateVisual => 7,
        EffectKind::Smoke => 13,
        EffectKind::DarkSmoke => 14,
        EffectKind::BurningDebris => 16,
        EffectKind::Eruption => 18,
        EffectKind::EruptionSmoke => 19,
        EffectKind::LightningEndpoint => 23,
        EffectKind::LightningStorm => 38,
        EffectKind::AmbientSmoke => 60,
        EffectKind::PossessionContact => 70,
        EffectKind::FireSpheres => 76,
        EffectKind::FadingSmoke => 87,
    }
}

pub(crate) const fn source_kind(model: u8) -> Option<EffectKind> {
    Some(match model {
        0 => EffectKind::Impact,
        1 => EffectKind::RadialExplosion,
        2 => EffectKind::Expiry,
        3 => EffectKind::ShortVisual,
        4 => EffectKind::DormantGroundMarker,
        5 => EffectKind::Ash,
        6 => EffectKind::GroundFire,
        7 => EffectKind::ImmediateVisual,
        13 => EffectKind::Smoke,
        14 => EffectKind::DarkSmoke,
        16 => EffectKind::BurningDebris,
        18 => EffectKind::Eruption,
        19 => EffectKind::EruptionSmoke,
        23 => EffectKind::LightningEndpoint,
        38 => EffectKind::LightningStorm,
        60 => EffectKind::AmbientSmoke,
        70 => EffectKind::PossessionContact,
        76 => EffectKind::FireSpheres,
        87 => EffectKind::FadingSmoke,
        _ => return None,
    })
}

fn next_rng(value: u16) -> u16 {
    value.wrapping_mul(9377).wrapping_add(9439)
}

fn tile_index(x: u8, y: u8) -> usize {
    usize::from(x) + 256 * usize::from(y)
}

fn draw(source: &mut EntitySnapshot) -> u16 {
    let rng = next_rng(source.entity_rng);
    source.set_entity_rng(rng);
    rng
}

fn word(source: &EntitySnapshot, offset: usize) -> i16 {
    i16::from_le_bytes([source.record[offset], source.record[offset + 1]])
}

fn write_word(source: &mut EntitySnapshot, offset: usize, value: i16) {
    source.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn bind_particle(
    source: &mut EntitySnapshot,
    index: u16,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let value = particles
        .get(usize::from(index))
        .ok_or_else(|| format!("effect requires retained particle {index}"))?;
    let limit = value.animation_limit().ok_or_else(|| {
        format!(
            "effect particle {index} has unsupported render mode {}",
            value.render_mode
        )
    })?;
    source.particle_parameter_index = Some(index);
    write_word(source, 94, index as i16);
    source.record[96] = 0;
    source.record[97] = limit;
    for (offset, value) in [
        (86, value.rotation_speed / 2),
        (88, value.speed / 2),
        (90, value.speed / 2),
        (92, value.rotation_speed / 2),
    ] {
        write_word(source, offset, value as i16);
    }
    Ok(())
}

fn animate(source: &mut EntitySnapshot) {
    if source.record[96] < source.record[97] {
        source.record[96] += 1;
    }
}

/// Allocate an ordinary effect. Global smoke randomness precedes NewEvent
/// even on failure. Fire Spheres delegates to the same 26-row arena producer.
pub(crate) fn allocate(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    definition: EffectDefinition,
    position: Position,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    allocate_inner(arena, terrain, definition, position, particles, None)
}

/// Construct an impact, radial explosion or ash from a source entity's position
/// address. NewEvent can reuse that same address and zero it before the
/// constructor maps the child or admits its light.
pub(crate) fn allocate_from_source_position(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    definition: EffectDefinition,
    source_slot: u16,
    source_position: Position,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    if !matches!(
        definition.kind(),
        EffectKind::Impact | EffectKind::RadialExplosion | EffectKind::Ash
    ) {
        return Err(
            "source-position effect constructor only supports impact, radial explosion and ash"
                .to_owned(),
        );
    }
    if usize::from(source_slot) >= arena.entities.len() {
        return Err(format!(
            "effect source address {source_slot} is outside the arena"
        ));
    }
    allocate_inner(
        arena,
        terrain,
        definition,
        source_position,
        particles,
        Some(source_slot),
    )
}

/// The common KillEntity 10/1 birth. The caller supplies the current source
/// pose and id after its preceding mana allocation, then owns its original
/// source-pointer hide tail. Success publishes exactly one provenance event.
pub(crate) fn spawn_death_burst(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
    parent_position: Position,
    parent_id: u16,
) -> Result<Option<u16>, String> {
    let Some(slot) = allocate_from_source_position(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(EffectKind::RadialExplosion),
        parent_slot,
        parent_position,
        particles,
    )?
    else {
        return Ok(None);
    };
    // KillEntity reads its parent pointer after NewEvent, including when
    // that pointer now names the newly initialized burst itself.
    let id = if slot == parent_slot { slot } else { parent_id };
    let row = &mut arena.entities[usize::from(slot)];
    row.source_id = id;
    row.record[30..32].copy_from_slice(&id.to_le_bytes());
    register_death_burst(arena, slot, parent_slot)?;
    Ok(Some(slot))
}

/// Hide a source address if this birth, or its preceding mana conversion,
/// replaced the caller. Other typed occupants remain their family's job;
/// returning false never writes through an unrelated stale raw row.
pub(crate) fn hide_birth_replacement(
    arena: &mut SourceArena,
    source_slot: u16,
    newborn: Option<u16>,
) -> Result<bool, String> {
    if usize::from(source_slot) >= arena.entities.len() {
        return Err(format!(
            "effect source address {source_slot} is outside the arena"
        ));
    }
    if newborn == Some(source_slot) {
        let row = &mut arena.entities[usize::from(source_slot)];
        if !row.active || row.class != 10 || !matches!(row.model, 0 | 1 | 5) {
            return Err(format!(
                "effect newborn at {source_slot} changed before source hide"
            ));
        }
        row.disable_drawing();
        return Ok(true);
    }
    if let Some(sphere) = arena
        .economy
        .spheres_mut()
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == source_slot)
    {
        sphere.control_flags |= 0x400;
        sphere.pending_delete = true;
        return Ok(true);
    }
    Ok(false)
}

/// TransformEntityToManaSphere clears playerEntityIndex on the current
/// address after allocation, including when that address is the new sphere.
/// The caller clears its original actor's own relation when this returns false.
pub(crate) fn clear_replacement_mana_owner(
    arena: &mut SourceArena,
    source_slot: u16,
) -> Result<bool, String> {
    if usize::from(source_slot) >= arena.entities.len() {
        return Err(format!(
            "mana conversion source address {source_slot} is outside the arena"
        ));
    }
    if let Some(sphere) = arena
        .economy
        .spheres_mut()
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == source_slot)
    {
        sphere.player_entity_index = 0;
        sphere.owner_entity_index = 0;
        arena.economy.refresh_owner_mana_rows();
        return Ok(true);
    }
    Ok(false)
}

/// The one-sphere 36BA0 birth after the dying carrier's RNG draw. The caller
/// clears the current source owner's relation after this attempt, including
/// allocation failure, and owns the source-specific RNG write before it.
pub(crate) fn spawn_death_mana_sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    source_position: Position,
    source_yaw: i16,
    source_mana: i32,
    source_owner: u16,
) -> Result<Option<u16>, String> {
    if usize::from(source_slot) >= arena.entities.len() {
        return Err(format!(
            "mana conversion source address {source_slot} is outside the arena"
        ));
    }
    if source_mana <= 0 {
        return Ok(None);
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // CreateManaSphere512 sets mana before 36BA0 rereads the parent. If the
    // source address was reused, those reads now see the new sphere itself.
    let replaced_source = slot == source_slot;
    let position = if replaced_source {
        Position::default()
    } else {
        source_position
    };
    arena.economy.initialize_death_mana_sphere_at_slot(
        slot,
        source_slot,
        position,
        if replaced_source { 0 } else { source_yaw },
        if replaced_source { 512 } else { source_mana },
        if replaced_source { 0 } else { source_owner },
        terrain,
    );
    let rng = arena
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
        .ok_or_else(|| format!("mana conversion lost allocated sphere {slot}"))?
        .entity_rng;
    arena.economy.source_rng.insert(slot, rng);
    sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

fn allocate_inner(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    definition: EffectDefinition,
    position: Position,
    particles: &[ParticleParameter],
    source_position_slot: Option<u16>,
) -> Result<Option<u16>, String> {
    let model = source_model(definition.kind());
    if matches!(model, 23 | 38) {
        use crate::native_world::lightning::{self, Product};
        let product = if model == 23 {
            Product::Endpoint
        } else {
            Product::Storm
        };
        let slot = lightning::construct(arena, terrain, particles, product, position)?;
        if let Some(slot) = slot {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_max_life(definition.lifetime_base());
            row.set_life(definition.lifetime_base());
            row.set_subspell_index(definition.damage());
        }
        return Ok(slot);
    }
    if model == 76 {
        let particle = particles
            .get(340)
            .ok_or_else(|| "Fire Spheres requires retained particle340".to_owned())?;
        let lifetime = i16::try_from(definition.lifetime_base())
            .map_err(|_| "Fire Spheres lifetime exceeds its retained carrier".to_owned())?;
        let Some(root) = arena
            .economy
            .allocate_authored_model76(u16::MAX, position, terrain)
        else {
            return Ok(None);
        };
        let slot = root.source_entity_index;
        arena.economy.bind_model76_collision_axes(
            slot,
            (particle.rotation_speed / 2) as i16,
            (particle.speed / 2) as i16,
            (particle.speed / 2) as i16,
            (particle.rotation_speed / 2) as i16,
        );
        if let Some(root) = arena
            .economy
            .effect_sphere_roots_mut()
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            root.max_life = lifetime;
            root.life = lifetime;
            root.subspell_index = definition.damage();
            root.control_flags |= 4;
        }
        for child in arena
            .economy
            .effect_sphere_children_mut()
            .iter_mut()
            .filter(|row| row.parent_entity_index == slot)
        {
            child.max_life = lifetime;
            child.life = lifetime;
            child.subspell_index = definition.damage();
            child.control_flags = if child.column_index == 0 {
                12
            } else {
                0x800004
            };
            child.word_0x2c = 0;
            child.shift_rot_x = child.collision_pitch;
            child.shift_rot_y = child.collision_fov;
        }
        let children = arena
            .economy
            .model76_children()
            .iter()
            .filter(|row| row.parent_entity_index == slot)
            .copied()
            .collect::<Vec<_>>();
        for child in children {
            sync_position(
                arena,
                terrain,
                child.source_entity_index,
                child.position,
                true,
                false,
            )?;
            lighting::admit_source(
                arena,
                terrain,
                child.source_entity_index,
                child.position,
                lighting::SourceLight::Fireball,
            );
        }
        // AddFireSpheres adds the root after all 25 children. Preserve the
        // actual map head even though its root's low drawing bit hides it.
        sync_position(arena, terrain, slot, root.position, true, true)?;
        return Ok(Some(slot));
    }
    let smoke_life = if matches!(model, 13 | 14 | 87) {
        arena.economy.advance_global_rng();
        let span = if model == 14 { 33 } else { 23 };
        (arena.economy.global_rng % span) as i32
    } else {
        0
    };
    if model == 60 && arena.economy.free_slot_count() < 32 {
        return Ok(None);
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if source_position_slot == Some(slot) {
        Position::default()
    } else {
        position
    };
    let mut row = crate::native_world::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    row.class = 10;
    row.record[67] = 10;
    row.model = model;
    row.record[68] = model;
    row.set_source_action(match model {
        60 => 65,
        70 => 77,
        87 => 94,
        _ => model,
    });
    row.set_max_life(definition.lifetime_base());
    row.set_subspell_index(definition.damage());
    row.set_control_flags((row.control_flags & 0xfffd_fff7) | 0x0002_0000);
    let mut linked = !matches!(model, 2 | 4 | 18 | 60);
    let mut index = match model {
        0 => Some(7),
        1 => Some(41),
        3 => Some(36),
        5 => Some(244),
        6 => Some(228),
        7 => Some(78),
        13 | 87 => Some(67),
        14 => Some(9),
        16 => Some(210),
        19 => Some(228),
        70 => Some(41),
        _ => None,
    };
    match model {
        0 | 1 => {}
        2 => {
            row.set_control_flags((row.control_flags & 0xfffd_fff6) | 0x20001);
        }
        3..=5 => {
            row.set_source_dword_10(0);
        }
        6 => {
            row.set_source_word_2c(0);
            row.set_source_dword_10(0);
        }
        7 => {
            let speed = (draw(&mut row) % 20 + 20) as i16;
            row.set_actual_speed(speed);
            row.record[69] = 10;
            row.record[70] = 7;
            row.set_control_flags(row.control_flags | 1);
        }
        13 | 14 | 87 => {
            row.set_max_life(definition.lifetime_base().wrapping_add(smoke_life));
            let speed = (draw(&mut row) % 53 + 51) as i16;
            row.set_actual_speed(speed);
            write_word(&mut row, 138, 30);
            row.record[69] = 10;
            row.record[70] = model;
        }
        16 => {
            let life = definition
                .lifetime_base()
                .wrapping_add(i32::from(draw(&mut row) % 100));
            row.set_max_life(life);
            let speed = (draw(&mut row) % 50 + 52) as i16;
            row.set_actual_speed(speed);
            let yaw = (draw(&mut row) & 2047) as i16;
            row.set_yaw(yaw);
            row.set_source_word_2c(256);
            let mut velocity = Position::default();
            move_entity(&mut velocity, yaw, 0, speed);
            write_word(&mut row, 158, velocity.x as i16);
            write_word(&mut row, 160, velocity.y as i16);
        }
        18 => {
            row.set_control_flags(0);
            row.set_source_dword_10(0);
        }
        19 => {
            row.set_control_flags(row.control_flags | 1);
        }
        60 => {
            let life = definition
                .lifetime_base()
                .wrapping_add(i32::from(draw(&mut row) % 100));
            row.set_max_life(life);
            let speed = (draw(&mut row) % 17) as i16;
            row.set_actual_speed(speed);
            row.set_control_flags(1);
            linked = false;
            index = None;
        }
        70 => {
            row.set_control_flags((row.control_flags & 0xf6) | 1);
        }
        _ => unreachable!("typed effect has a single-row constructor"),
    }
    if matches!(model, 4..=7 | 16) {
        let mut position = row.position;
        position.z = terrain
            .ground_height(position)
            .wrapping_add(if model == 16 { 64 } else { 0 });
        row.set_position(position);
    }
    row.set_life(row.max_life);
    if let Some(index) = index {
        if let Err(error) = bind_particle(&mut row, index, particles) {
            arena.economy.release_slot(slot, Some(row.entity_rng));
            return Err(error);
        }
    }
    if matches!(model, 0 | 6 | 19 | 70) {
        let (width, height) = match model {
            0 => (128, 128),
            6 => (272, 1536),
            _ => (512, 512),
        };
        row.set_source_overlap_half_extents(width, width);
        write_word(&mut row, 92, height);
    }
    if linked {
        if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: true,
            x: row.position.x,
            y: row.position.y,
        }) {
            arena.economy.release_slot(slot, Some(row.entity_rng));
            return Err(error);
        }
        row.set_control_flags(row.control_flags | 4);
        let previous = arena.economy.map_chains.old_map_entity(slot);
        if previous != 0
            && !arena
                .economy
                .live_arena_slots()
                .iter()
                .any(|(id, _)| *id == previous)
        {
            arena.entities[usize::from(previous)].record[28..30]
                .copy_from_slice(&slot.to_le_bytes());
        }
        row.record[26..28].copy_from_slice(&previous.to_le_bytes());
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    sync_map(arena, terrain, slot)?;
    let light = match model {
        0 | 1 => Some(lighting::SourceLight::Impact),
        6 => Some(lighting::SourceLight::GroundFire),
        _ => None,
    };
    if let Some(light) = light {
        let position = arena.entities[usize::from(slot)].position;
        lighting::admit_source(arena, terrain, slot, position, light);
    }
    Ok(Some(slot))
}

fn pulse(source: &EntitySnapshot, continuous: bool) -> DamagePulse {
    DamagePulse {
        position: source.position,
        owner: source.source_id,
        payload: source.subspell_index,
        half_width: source.array_pitch(),
        vertical_offset: source.array_yaw(),
        half_height: source.array_fov(),
        continuous,
        selector: (source.record[69], source.record[70]),
    }
}

fn child(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent: &mut EntitySnapshot,
    kind: EffectKind,
    position: Position,
) -> Result<Option<u16>, String> {
    // NewEvent may evict this very replaceable parent. Publish the current
    // source writes before allocation, then keep using the actual occupant
    // of that address, just as the source pointer does after replacement.
    let parent_slot = parent.index;
    let generation = arena.economy.slot_generation(parent_slot);
    if !arena
        .economy
        .live_arena_slots()
        .iter()
        .any(|(slot, _)| *slot == parent_slot)
    {
        arena.entities[usize::from(parent_slot)] = parent.clone();
    }
    let result = allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(kind),
        position,
        particles,
    )?;
    if arena.economy.slot_generation(parent_slot) != generation {
        *parent = arena.entities[usize::from(parent_slot)].clone();
    }
    if let Some(slot) = result {
        let row = &mut arena.entities[usize::from(slot)];
        row.source_id = parent.source_id;
        row.record[30..32].copy_from_slice(&parent.source_id.to_le_bytes());
    }
    Ok(result)
}

/// sub_32160/sub_322A0. Selector animation changes the retained particle
/// index, without resetting the frame or replacing source collision axes.
pub(crate) fn step_smoke(source: &mut EntitySnapshot, terrain: &SessionTerrain) {
    let life = source.life;
    source.set_life(life.wrapping_sub(1));
    if life < 0 {
        source.disable_drawing();
        return;
    }
    let speed = source.actual_speed.wrapping_sub(4).clamp(64, 128);
    source.set_actual_speed(speed);
    let mut position = source.position;
    position.z = position
        .z
        .wrapping_add(speed)
        .max(terrain.ground_height(source.position));
    let age = source.source_dword_10().wrapping_add(1);
    source.set_source_dword_10(age);
    let (first, last) = if source.model == 14 {
        (9, 16)
    } else {
        (67, 74)
    };
    let mut index = source.particle_parameter_index.unwrap_or(first);
    if age < 16 {
        let drift = word(source, 138);
        move_entity(&mut position, source.yaw, 0, drift);
        write_word(source, 138, drift.wrapping_sub(52).clamp(30, 1024));
        if age & 1 == 0 && index < last {
            index += 1;
        }
    }
    if source.life < 6 && index > first {
        index -= 1;
    }
    source.particle_parameter_index = Some(index);
    write_word(source, 94, index as i16);
    source.set_position(position);
}

/// Ordinary Update callback, before the scheduler's common phase increment.
/// A local row copy is committed to the one arena carrier after child births;
/// there is no second mutable effect collection.
pub(crate) fn visit<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    particles: &[ParticleParameter],
    state: &mut State,
    apply_damage: F,
) -> Result<Visit, String>
where
    F: FnMut(&mut SourceArena, DamagePulse) -> Result<u16, String>,
{
    let source = arena.entities[usize::from(slot)].clone();
    let generation = arena.economy.slot_generation(slot);
    let was_live_burst =
        (source.class, source.model, source.action) == (10, 1, 1) && !source.drawing_disabled();
    let result = visit_entity(arena, terrain, source, particles, state, apply_damage);
    state.retain_current_generations(arena);
    let (source, visit) = result?;
    let expired = was_live_burst
        && arena.economy.slot_generation(slot) == generation
        && (source.class, source.model, source.action) == (10, 1, 1)
        && source.drawing_disabled();
    commit_entity(arena, terrain, source)?;
    if arena.economy.slot_generation(slot) != generation {
        state.forget_source_slot(slot);
    } else if expired
        && let Some((registered_generation, parent)) = state.death_bursts.remove(&slot)
        && registered_generation == generation
    {
        arena.economy.record_raw_death_burst_expiry(slot, parent);
    }
    Ok(visit)
}

pub(crate) fn commit_entity(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source: EntitySnapshot,
) -> Result<(), String> {
    let slot = source.index;
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    sync_map(arena, terrain, slot)
}

/// The same callback over a retained typed carrier's temporary header view.
/// The adapter writes the result back to that one carrier, never a mirror.
pub(crate) fn visit_entity<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    mut source: EntitySnapshot,
    particles: &[ParticleParameter],
    state: &mut State,
    mut apply_damage: F,
) -> Result<(EntitySnapshot, Visit), String>
where
    F: FnMut(&mut SourceArena, DamagePulse) -> Result<u16, String>,
{
    let slot = source.index;
    let mut visit = Visit::default();
    match source.action {
        0 => step_impact(arena, &mut source, terrain, &mut visit, &mut apply_damage)?,
        1 => {
            let life = source.life;
            source.set_life(life.wrapping_sub(1));
            if life < 0 {
                source.disable_drawing();
            } else {
                if source.control_flags & 2 == 0 {
                    source.set_control_flags(source.control_flags | 2);
                    visit.sounds.push(3);
                }
                // The source SEARCH.DAT zero bitmap is normalized to its
                // first pixel. Its iterator stops before the last pixel.
                for (x, y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
                    if draw(&mut source) % 157 < 79 {
                        continue;
                    }
                    let dx = i32::from(draw(&mut source) % 129) - 160 + 192 * x;
                    let dy = i32::from(draw(&mut source) % 129) - 160 + 192 * y;
                    let position = Position {
                        x: source.position.x.wrapping_add(dx as u16),
                        y: source.position.y.wrapping_add(dy as u16),
                        z: source.position.z,
                    };
                    if let Some(child) = child(
                        arena,
                        terrain,
                        particles,
                        &mut source,
                        EffectKind::Impact,
                        position,
                    )? {
                        let row = &mut arena.entities[usize::from(child)];
                        row.set_yaw(source.yaw);
                        row.set_control_flags(row.control_flags | 0x80);
                        if child == slot {
                            source = row.clone();
                        }
                    }
                }
            }
        }
        2 => {
            source.set_life(source.life.wrapping_sub(1));
            if source.life < -1 {
                source.disable_drawing();
            }
        }
        3 => {
            let life = source.life;
            source.set_life(life.wrapping_sub(1));
            if life < 0 {
                source.disable_drawing();
            }
        }
        4 => {} // Enabled callback is empty; maxLife is not a countdown.
        5 => {
            let life = source.life;
            source.set_life(life.wrapping_sub(1));
            if life < 0 {
                source.disable_drawing();
            } else {
                animate(&mut source);
                if source.control_flags & 2 == 0 {
                    source.set_control_flags(source.control_flags | 2);
                    visit.sounds.push(27);
                }
            }
        }
        6 => {
            let life = source.life;
            source.set_life(life.wrapping_sub(1));
            if life < 0 {
                source.disable_drawing();
            } else {
                visit.nearest_fire = Some(source.position);
                let age = source.source_dword_10();
                let mut index = source.particle_parameter_index.unwrap_or(228);
                if source.life < 12 && age > 0 {
                    source.set_source_dword_10(age.wrapping_sub(1));
                    index = index.wrapping_sub(1);
                    if source.control_flags & 0x80 == 0 && draw(&mut source) % 7 == 0 {
                        let position = source.position;
                        if let Some(slot) = child(
                            arena,
                            terrain,
                            particles,
                            &mut source,
                            EffectKind::DarkSmoke,
                            position,
                        )? {
                            let row = &mut arena.entities[usize::from(slot)];
                            row.set_source_dword_10(100);
                            row.set_life(15);
                            let index = row.particle_parameter_index.unwrap_or(9).wrapping_add(2);
                            row.particle_parameter_index = Some(index);
                            write_word(row, 94, index as i16);
                            if slot == source.index {
                                source = row.clone();
                            }
                        }
                    }
                } else if source.life >= 12 && age <= 6 {
                    source.set_source_dword_10(age.wrapping_add(1));
                    index = index.wrapping_add(1);
                }
                source.particle_parameter_index = Some(index);
                write_word(&mut source, 94, index as i16);
                let mut position = source.position;
                position.z = terrain
                    .ground_height(position)
                    .wrapping_add(source.source_word_2c());
                source.set_position(position);
                if terrain.terrain_type(position) == 0 {
                    source.disable_drawing();
                }
            }
            // Even the terminating callback writes the continuous mailbox.
            if source.control_flags & 0x0001_0000 == 0 {
                visit.damage.push(pulse(&source, true));
            }
        }
        7 => source.disable_drawing(),
        13 | 14 | 94 => step_smoke(&mut source, terrain),
        16 => step_debris(arena, terrain, particles, &mut source)?,
        17 => {
            source.set_life(source.life.wrapping_sub(1));
            if source.life < 0 {
                source.disable_drawing();
            } else {
                if source.control_flags & 2 == 0 {
                    source.set_control_flags(source.control_flags | 0x10002);
                    visit.sounds.push(30);
                }
                let product = 768_i32.wrapping_mul(source.source_dword_10());
                let width = ((product - (product >> 31) * 5) >> 2) as i16;
                write_word(&mut source, 88, width);
                write_word(&mut source, 90, width);
                write_word(&mut source, 92, 512);
                let mut request = pulse(&source, false);
                request.payload = (i32::from(source.subspell_index) / source.max_life) as u16;
                let hits = apply_damage(arena, request)?;
                if hits != 0 {
                    visit.experience.push((source.source_id, 9, hits));
                }
                for (x, y) in search_shape(
                    source.source_dword_10() as u8,
                    source.source_dword_10() as u8,
                ) {
                    let dx = 160 * i32::from(x) + i32::from(draw(&mut source) % 129) - 160;
                    let dy = 160 * i32::from(y) + i32::from(draw(&mut source) % 129) - 160;
                    let position = Position {
                        x: source.position.x.wrapping_add(dx as u16),
                        y: source.position.y.wrapping_add(dy as u16),
                        z: source.position.z,
                    };
                    if let Some(child_slot) = child(
                        arena,
                        terrain,
                        particles,
                        &mut source,
                        EffectKind::Impact,
                        position,
                    )? {
                        let row = &mut arena.entities[usize::from(child_slot)];
                        row.set_yaw(source.yaw);
                        row.set_control_flags(row.control_flags | 0x10080);
                        write_word(row, 88, 512);
                        write_word(row, 90, 512);
                        write_word(row, 92, 512);
                        row.set_source_dword_10(0);
                        if child_slot == slot {
                            source = row.clone();
                        }
                    }
                }
                source.set_source_dword_10(source.source_dword_10().wrapping_add(2) % 11);
            }
        }
        18 => step_eruption(arena, terrain, particles, state, &mut source)?,
        19 => step_eruption_smoke(arena, terrain, particles, state, &mut source, &mut visit)?,
        65 => {
            let life = source.life;
            source.set_life(life.wrapping_sub(1));
            if life < 0 {
                source.disable_drawing();
            } else {
                let mut position = source.position;
                position.x = position.x.wrapping_add(draw(&mut source) % 160);
                position.z = position.z.wrapping_add((draw(&mut source) % 160) as i16);
                if let Some(slot) = child(
                    arena,
                    terrain,
                    particles,
                    &mut source,
                    EffectKind::DarkSmoke,
                    position,
                )? {
                    let extra = source
                        .actual_speed
                        .wrapping_add((draw(&mut source) % 77) as i16);
                    let row = &mut arena.entities[usize::from(slot)];
                    row.set_max_life(32);
                    row.set_life(32);
                    row.set_actual_speed(row.actual_speed.wrapping_add(extra));
                    if slot == source.index {
                        source = row.clone();
                    }
                }
            }
        }
        77 => step_possession_contact(arena, &mut source),
        action => return Err(format!("effect {slot} has unowned action {action}")),
    }
    for pulse in visit.damage.drain(..) {
        apply_damage(arena, pulse)?;
    }
    Ok((source, visit))
}

fn step_impact<F>(
    arena: &mut SourceArena,
    source: &mut EntitySnapshot,
    terrain: &mut SessionTerrain,
    visit: &mut Visit,
    apply_damage: &mut F,
) -> Result<(), String>
where
    F: FnMut(&mut SourceArena, DamagePulse) -> Result<u16, String>,
{
    let age = source.source_dword_10();
    if age & 3 != 0 {
        source.set_source_dword_10(age.wrapping_sub(1));
        return Ok(());
    }
    source.set_life(source.life.wrapping_sub(1));
    if source.life < -1 {
        source.disable_drawing();
        return Ok(());
    }
    source.set_control_flags(source.control_flags & !1);
    // sub_30D50 caches the ground before its material/depression operation.
    let ground = terrain.ground_height(source.position);
    if source.control_flags & 2 == 0 {
        if source.control_flags & 0x0001_0000 == 0 {
            apply_damage(arena, pulse(source, false))?;
        }
        let x = (source.position.x.wrapping_add(128) >> 8) as u8;
        let y = (source.position.y.wrapping_add(128) >> 8) as u8;
        let index = tile_index(x, y);
        let material = terrain.planes().terrain_types[index];
        match material {
            26 | 10 | 11 => {
                let target = match material {
                    26 => 20,
                    10 => 21,
                    _ => 22,
                };
                let (planes, map_type, _, sequence) = terrain.disaster_parts();
                crate::native_world::apply_ambient_material(
                    planes,
                    x,
                    y,
                    (source.control_flags >> 16) as u8,
                    target,
                    map_type,
                )?;
                *sequence = sequence.wrapping_add(1);
            }
            0 => {}
            value
                if (value < 6 || value > 34)
                    && terrain.planes().angles[index] & 7 != 1
                    && i32::from(source.position.z) - i32::from(ground) <= 128
                    && terrain.planes().angles[tile_index(
                        (source.position.x >> 8) as u8,
                        (source.position.y >> 8) as u8,
                    )] & 15
                        != 0 =>
            {
                let delta = -((draw(source) % 7) as i16);
                crate::world::terrain::disasters::deform_band(
                    terrain,
                    source.position,
                    source.array_pitch(),
                    0,
                    0,
                    delta,
                    true,
                );
            }
            _ => {}
        }
        source.set_control_flags(source.control_flags | 2);
        let shift = (draw(source) % 65) as i16 - 32;
        source.set_source_word_2c(shift);
        visit.sounds.push(3);
    }
    let mut position = source.position;
    if position.z > ground {
        position.z = position.z.wrapping_add(source.source_word_2c());
    }
    position.z = position.z.max(ground);
    if let Some(ceiling) = terrain.ceiling_height(position) {
        position.z = position.z.min(ceiling.wrapping_sub(source.array_fov()));
    }
    source.set_position(position);
    animate(source);
    Ok(())
}

fn sync_map(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let row = &arena.entities[usize::from(slot)];
    if row.control_flags & 4 == 0 {
        return Ok(());
    }
    let position = row.position;
    let active = row.active;
    sync_position(arena, terrain, slot, position, active, false)
}

pub(crate) fn sync_position(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    position: Position,
    active: bool,
    reinsert: bool,
) -> Result<(), String> {
    let before = [
        arena.economy.map_chains.old_map_entity(slot),
        arena.economy.map_chains.next_entity(slot),
    ];
    let old_tile = arena.economy.map_chains.tile_of(slot);
    if reinsert {
        arena.economy.map_chains.set_map_entity_unlink(slot);
    }
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active,
        x: position.x,
        y: position.y,
    })?;
    let after = [
        arena.economy.map_chains.old_map_entity(slot),
        arena.economy.map_chains.next_entity(slot),
    ];
    let new_tile = arena.economy.map_chains.tile_of(slot);
    for tile in [old_tile, new_tile] {
        terrain.disaster_parts().0.entity_indices[usize::from(tile)] =
            arena.economy.map_chains.tile_head(tile) as i16;
    }
    for id in before
        .into_iter()
        .chain(after)
        .chain([slot])
        .filter(|id| *id != 0)
    {
        if arena
            .economy
            .live_arena_slots()
            .iter()
            .any(|(slot, _)| *slot == id)
        {
            continue;
        }
        let links = (
            arena.economy.map_chains.old_map_entity(id),
            arena.economy.map_chains.next_entity(id),
        );
        arena.entities[usize::from(id)].record[26..28].copy_from_slice(&links.0.to_le_bytes());
        arena.entities[usize::from(id)].record[28..30].copy_from_slice(&links.1.to_le_bytes());
    }
    Ok(())
}

fn step_debris(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source: &mut EntitySnapshot,
) -> Result<(), String> {
    let life = source.life;
    source.set_life(life.wrapping_sub(1));
    if life < 0 {
        source.disable_drawing();
        return Ok(());
    }
    source.set_control_flags(source.control_flags | 2);
    let vx = word(source, 158).clamp(-80, 80);
    let vy = word(source, 160).clamp(-80, 80);
    write_word(source, 158, vx);
    write_word(source, 160, vy);
    let mut position = source.position;
    position.x = position.x.wrapping_add(vx as u16);
    position.y = position.y.wrapping_add(vy as u16);
    position.z = position.z.wrapping_add(source.source_word_2c());
    source.set_source_word_2c(source.source_word_2c().wrapping_sub(28).clamp(-384, 256));
    let ground = terrain.ground_height(position);
    if ground > position.z {
        position.z = ground;
        // Source's sign correction truncates the negative divide toward zero.
        source.set_source_word_2c(-(source.source_word_2c() / 4));
        if terrain.terrain_type(source.position) == 0 {
            if child(arena, terrain, particles, source, EffectKind::Ash, position)?.is_some() {
                source.disable_drawing();
            }
        } else {
            let cell = u16::from_le_bytes([(position.x >> 8) as u8, (position.y >> 8) as u8]);
            let already_burning = crate::level1_mission::source_map_search_slots(
                &arena.economy.map_chains,
                position,
                &[(0, 0)],
            )
            .into_iter()
            .any(|id| {
                let row = &arena.entities[usize::from(id)];
                (row.active && row.class == 10 && row.model == 6)
                    || arena.economy.authored_model6().iter().any(|row| {
                        row.active
                            && row.source_entity_index == id
                            && u16::from_le_bytes([
                                (row.position.x >> 8) as u8,
                                (row.position.y >> 8) as u8,
                            ]) == cell
                    })
            });
            if !already_burning {
                if let Some(id) = child(
                    arena,
                    terrain,
                    particles,
                    source,
                    EffectKind::GroundFire,
                    position,
                )? {
                    let row = &mut arena.entities[usize::from(id)];
                    row.set_life(30);
                    row.set_subspell_index(row.subspell_index.wrapping_mul(3));
                    if id == source.index {
                        *source = row.clone();
                    }
                    source.set_source_dword_10(0);
                }
            }
            if source.source_word_2c() <= 28 {
                source.set_source_word_2c(0);
            }
        }
    }
    source.set_source_dword_10(source.source_dword_10().wrapping_add(1));
    source.set_position(position);
    if position.z == ground {
        let x = (position.x >> 8) as u8;
        let y = (position.y >> 8) as u8;
        let heights = terrain.ground_heights();
        let at = |x: u8, y: u8| i16::from(heights[tile_index(x, y)]);
        let a = at(x, y);
        let b = at(x.wrapping_add(1), y);
        let c = at(x.wrapping_add(1), y.wrapping_add(1));
        let d = at(x, y.wrapping_add(1));
        let damp = |v: i16| ((250 * i32::from(v)) / 256) as i16;
        write_word(source, 158, damp(vx.wrapping_add(a - b - c + d)));
        write_word(source, 160, damp(vy.wrapping_add(a + b - c - d)));
    }
    Ok(())
}

fn search_shape(first: u8, last: u8) -> Vec<(i8, i8)> {
    let mut cells = crate::level1_mission::source_search_offsets(last);
    if first > 0 {
        let previous = crate::level1_mission::source_search_offsets(first - 1).len();
        cells.drain(..previous);
    }
    // sub_10130 writes the final pair but returns2; while(result==1)
    // deliberately excludes that last pair from the callback body.
    cells.pop();
    cells
}

fn step_eruption(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    state: &mut State,
    source: &mut EntitySnapshot,
) -> Result<(), String> {
    let mut age = source.source_dword_10();
    if age > 2500 && draw(source) % 100 == 0 && state.eruption == 0 {
        let mut position = source.position;
        position.z = terrain.ground_height(position);
        let moved = position.z != source.position.z;
        source.set_position(position);
        if moved {
            source.disable_drawing();
            return Ok(());
        }
        source.set_source_dword_10(0);
        age = 0;
    }
    let selected = (age < 128 && age & 15 != 0 && draw(source) % 5 == 0) || age == 0;
    if selected {
        let mut position = source.position;
        position.z = terrain.ground_height(position);
        let moved = position.z != source.position.z;
        source.set_position(position);
        if moved {
            source.disable_drawing();
            state.eruption = 0;
            return Ok(());
        }
        if age == 0 {
            if state.eruption != 0
                && let Some(previous) = arena.entities.get_mut(usize::from(state.eruption))
            {
                if previous.active {
                    previous.set_source_dword_10(250);
                }
            }
            state.eruption = source.index;
            let position = source.position;
            if let Some(id) = child(
                arena,
                terrain,
                particles,
                source,
                EffectKind::EruptionSmoke,
                position,
            )? {
                if state.eruption_smoke != 0
                    && let Some(previous) =
                        arena.entities.get_mut(usize::from(state.eruption_smoke))
                {
                    if previous.active {
                        previous.disable_drawing();
                    }
                }
                state.eruption_smoke = id;
            }
        }
        let position = source.position;
        if let Some(id) = child(
            arena,
            terrain,
            particles,
            source,
            EffectKind::BurningDebris,
            position,
        )? {
            let rng = draw(source);
            arena.entities[usize::from(id)].set_entity_rng(rng);
        }
        source.set_yaw(source.yaw.wrapping_add(1280));
        if age == 0 {
            spawn_eruption_fireball(arena, terrain, source);
        }
        if age >= 127 {
            source.disable_drawing();
            state.eruption = 0;
        }
    }
    source.set_source_dword_10(source.source_dword_10().wrapping_add(1));
    Ok(())
}

fn step_eruption_smoke(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    state: &mut State,
    source: &mut EntitySnapshot,
    visit: &mut Visit,
) -> Result<(), String> {
    let life = source.life;
    source.set_life(life.wrapping_sub(1));
    if life < 0 {
        source.disable_drawing();
        state.eruption_smoke = 0;
    } else {
        source.set_source_dword_10(0);
        for (x, y) in search_shape(0, 0) {
            if draw(source) % 157 < 79 {
                continue;
            }
            let dx = i32::from(draw(source) % 129) - 160 + 192 * i32::from(x);
            let dy = i32::from(draw(source) % 129) - 160 + 192 * i32::from(y);
            let position = Position {
                x: source.position.x.wrapping_add(dx as u16),
                y: source.position.y.wrapping_add(dy as u16),
                z: source.position.z,
            };
            if source.life & 1 != 0 {
                let start = ((source.life / 2) & 1) << 8;
                for angle in (start..2048).step_by(512) {
                    if let Some(id) = child(
                        arena,
                        terrain,
                        particles,
                        source,
                        EffectKind::DarkSmoke,
                        position,
                    )? {
                        arena.entities[usize::from(id)].set_yaw(angle as i16);
                        if id == source.index {
                            *source = arena.entities[usize::from(id)].clone();
                        }
                    }
                }
            }
        }
        let mut position = source.position;
        position.z = terrain.ground_height(position);
        source.set_position(position);
    }
    visit.damage.push(pulse(source, false));
    Ok(())
}

fn step_possession_contact(arena: &mut SourceArena, source: &mut EntitySnapshot) {
    source.set_source_dword_10(source.source_dword_10().wrapping_add(1));
    let life = source.life;
    source.set_life(life.wrapping_sub(1));
    if life < 0 {
        source.disable_drawing();
        return;
    }
    animate(source);
    let center_x = (i32::from(source.position.x) + 128) >> 8;
    let center_y = (i32::from(source.position.y) + 128) >> 8;
    let width = i32::from(source.array_pitch()) + 255;
    let radius = (width - (width >> 31) * 255) >> 8;
    let mut slots = Vec::new();
    for y in -radius..=radius {
        for x in -radius..=radius {
            slots.extend(
                arena
                    .economy
                    .map_chains
                    .entity_chain((center_x + x) as u8, (center_y + y) as u8),
            );
        }
    }
    let selector = (source.record[69], source.record[70]);
    for slot in &slots {
        let target = &mut arena.entities[usize::from(*slot)];
        if target.active
            && target.source_id != source.source_id
            && target.class != 0
            && target.control_flags & 8 != 0
            && target.record[60] & 2 != 0
            && (selector.0 == u8::MAX
                || (selector.0 == target.class
                    && (selector.1 == u8::MAX || selector.1 == target.model)))
            && crate::level1_mission::source_entities_overlap(
                source.position,
                source.array_pitch(),
                source.array_yaw(),
                source.array_fov(),
                target.position,
                target.array_pitch(),
                target.array_yaw(),
                target.array_fov(),
            )
        {
            target.record[104..108].copy_from_slice(&1_u32.to_le_bytes());
            target.record[108..110].copy_from_slice(&source.source_id.to_le_bytes());
        }
    }
    for sphere in arena.economy.spheres_mut() {
        if slots.contains(&sphere.source_entity_index)
            && sphere.active
            && !sphere.pending_delete
            && sphere.source_entity_index != source.source_id
            && (selector.0 == u8::MAX
                || (selector.0 == 10 && (selector.1 == u8::MAX || selector.1 == 39)))
        {
            if sphere.overlaps_source_axes(
                source.position,
                source.array_pitch(),
                source.array_roll(),
                source.array_yaw(),
                source.array_fov(),
            ) {
                sphere.incoming_owner_entity_index = source.source_id;
                sphere.incoming_possession_payload = 1;
            }
        }
    }
}

fn spawn_eruption_fireball(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source: &EntitySnapshot,
) {
    let yaw = source.yaw & 2047;
    let mut destination = source.position;
    move_entity(&mut destination, yaw, 0, 1536);
    destination.z = terrain.ground_height(destination);
    arena.spawn_eruption_projectile(
        terrain,
        source.position,
        yaw,
        destination,
        source.source_id,
        source.index,
    );
}
