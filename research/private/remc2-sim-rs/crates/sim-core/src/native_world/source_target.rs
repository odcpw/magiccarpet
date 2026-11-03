//! Current fixed-address target reads across arena and external actor owners.
//!
//! These values are borrowed for one read. They are neither an entity mirror
//! nor a generation-bound lifetime handle: NewEvent can replace the address
//! between two reads made by the same callback.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::combat::area::{Actor, Shape};
use crate::goat::GoatFamily;
use crate::house::HouseFamily;
use crate::villager::VillagerFamily;

use super::{EntitySnapshot, Level2DynamicArenaKind as Kind, ParticleParameter, area_damage};

pub(crate) fn bind_castle_upgrade_record(
    arena: &mut SourceArena,
    slot: u16,
    particle: &ParticleParameter,
) -> Result<(), String> {
    let limit = super::campaign_shared_families::particle_animation_limit(particle.render_mode)
        .ok_or_else(|| format!("Castle particle41 render mode {} has no source animation limit", particle.render_mode))?;
    let raw = &mut arena.entities[usize::from(slot)];
    raw.record[94..96].copy_from_slice(&41_u16.to_le_bytes());
    raw.record[96] = 0;
    raw.record[97] = limit;
    Ok(())
}

pub(crate) fn advance_castle_upgrade_frame(arena: &mut SourceArena, slot: u16) {
    let raw = &mut arena.entities[usize::from(slot)];
    if raw.record[96].cast_signed() < raw.record[97].cast_signed() {
        raw.record[96] = raw.record[96].wrapping_add(1);
    }
}

/// 389F0 includes Entities[0] in its direct Castle pointer dereference.
pub(crate) fn castle_upgrade_target(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<(Position, Shape)>, String> {
    let view = if slot == 0 {
        arena.entities.first().map(View::raw)
    } else { read(arena, context, slot)? };
    view.map(|view| {
        view.collision.map(|shape| (view.position, shape))
            .ok_or_else(|| format!("Castle upgrade target {slot} lacks current source axes"))
    }).transpose()
}

pub(crate) fn collector_target(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>, String> {
    let Some(view) = read(arena, context, slot)? else { return Ok(None) };
    let raw = &arena.entities[usize::from(slot)];
    let kind = arena.economy.live_arena_kind(slot);
    let mut mana = if kind.is_none() { raw.mana } else { 0 };
    let mut relation = if kind.is_none() { raw.source_player_entity_index() } else { 0 };
    let mut stage = if kind.is_none() { raw.source_dword_10() } else { 0 };
    let mut shape = view.collision;
    if let Some(row) = arena.economy.spheres.iter()
        .find(|row| kind == Some(Kind::ManaSphere) && row.active && row.source_entity_index == slot)
    {
        mana = row.mana;
        relation = row.player_entity_index;
    } else if kind == Some(Kind::CastleRoot) && let Some(row) = arena.economy.castle_at(slot) {
        mana = row.mana;
        relation = row.player_entity_index;
        stage = i32::from(row.level);
    } else if let Some(row) = arena.economy.castle_children.iter()
        .find(|row| row.active && row.source_entity_index == slot
            && matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary)))
    {
        mana = row.mana;
        relation = row.player_entity_index;
        shape = shape.or_else(|| context.particle_shape(row.visual_index));
    } else if let Some(row) = arena.economy.carriers.iter()
        .find(|row| kind == Some(Kind::ManaCarrier) && row.active && row.source_entity_index == slot)
    {
        mana = row.carried_mana;
        relation = row.owner_entity_index;
    }
    // Zero-relation class10 and nonclass10 branches never test these axes.
    let shape = if view.class == 10 && relation != 0 {
        shape.ok_or_else(|| format!("Castle collector target {slot} lacks current collision axes"))?
    } else { shape.unwrap_or(Shape { half_width: 0, half_depth: 0, vertical_offset: 0, half_height: 0 }) };
    Ok(Some(super::level2_dynamic_economy::Level2CastleCollectorTarget {
        source_slot: slot, source_class: view.class, source_model: view.model, position: view.position,
        player_entity_index: relation, mana, source_dword_10: stage,
        collision_yaw: shape.vertical_offset, collision_pitch: shape.half_width,
        collision_roll: shape.half_depth, collision_fov: shape.half_height,
    }))
}

/// A callback-local pointer view plus the raw record domain. Economy-owned
/// mutations go to their current carrier; retained free rows stay physical.
pub(crate) struct CastleCollectorConsumer<'a> {
    pub(crate) target: Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>,
    pub(crate) entities: &'a mut [EntitySnapshot],
    pub(crate) shared: &'a mut super::Level2SharedSpellResetSession,
}

pub(crate) fn publish_collector_player_resources(
    arena: &SourceArena,
    shared: &mut super::Level2SharedSpellResetSession,
    rivals: &mut super::level2_nyphur::RivalWizardRoster,
    slot: u16,
) {
    if arena.economy.live_arena_kind(slot).is_some() { return; }
    let Some(row) = arena.entities.get(usize::from(slot))
        .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1))
    else { return };
    if let Some(handle) = arena.handle(slot) && let Some(rival) = rivals.get_mut(handle) {
        rival.sync_spell_target_resources(row);
    }
    shared.sync_player_entity_from_authority(row);
}

impl super::level2_dynamic_economy::Level2CastleCollectorConsumer for CastleCollectorConsumer<'_> {
    fn record_damage(&mut self) {
        self.shared.record_castle_collector_damage();
    }
    fn read(&self, _economy: &super::Level2EconomyState, slot: u16)
        -> Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>
    {
        self.target.filter(|row| row.source_slot == slot)
    }

    fn set_attraction(&mut self, economy: &mut super::Level2EconomyState, slot: u16, in_range: bool, holder: u16) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::ManaSphere) && let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            if in_range { row.control_flags |= 0x40; row.assigned_defender_slot = holder; }
            else { row.control_flags &= !0x40; }
        } else if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) {
            if in_range { row.control_flags |= 0x40; } else { row.control_flags &= !0x40; }
        } else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            if in_range { row.control_flags |= 0x40; row.target_sphere_slot = holder; }
            else { row.control_flags &= !0x40; }
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            raw.set_control_flags(if in_range { raw.control_flags | 0x40 } else { raw.control_flags & !0x40 });
            if in_range { raw.set_source_word_96(holder); }
        }
    }

    fn add_mana(&mut self, economy: &mut super::Level2EconomyState, slot: u16, delta: i32) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) { row.mana = row.mana.wrapping_add(delta); }
        else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.mana = row.mana.wrapping_add(delta);
        } else if kind == Some(Kind::ManaCarrier) && let Some(row) = economy.carriers.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.carried_mana = row.carried_mana.wrapping_add(delta);
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            let previous = self.target.filter(|row| row.source_slot == slot).map_or(raw.mana, |row| row.mana);
            raw.set_mana(previous.wrapping_add(delta));
        }
    }

    fn hide(&mut self, economy: &mut super::Level2EconomyState, slot: u16) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::ManaSphere) && let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.pending_delete = true;
            row.control_flags |= 0x400;
        } else if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) { row.control_flags |= 0x400; }
        else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.control_flags |= 0x400;
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) { raw.disable_drawing(); }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Context<'a> {
    pub(crate) goats: Option<&'a GoatFamily>,
    pub(crate) houses: Option<&'a HouseFamily>,
    pub(crate) villagers: Option<&'a VillagerFamily>,
    pub(crate) spells: Option<&'a super::Level2SharedSpellResetSession>,
    pub(crate) player: Option<(u16, Position, i32)>,
    pub(crate) player_orientation: Option<(i16, i16)>,
    pub(crate) particles: Option<&'a [ParticleParameter]>,
    pub(crate) map_type: Option<super::MapType>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        goats: &'a GoatFamily,
        houses: &'a HouseFamily,
        villagers: &'a VillagerFamily,
    ) -> Self {
        Self {
            goats: Some(goats),
            houses: Some(houses),
            villagers: Some(villagers),
            ..Self::default()
        }
    }

    /// Split the ordinary owner at its privacy boundary; the returned context
    /// retains only the actual actor domains required by current-slot reads.
    pub(crate) fn from_ordinary(
        goats: &'a GoatFamily,
        ordinary: &'a super::Level2OrdinaryResetSession,
    ) -> Self {
        Self::new(goats, &ordinary.houses, &ordinary.villagers)
    }

    fn external(self, slot: u16) -> Option<View> {
        if let Some(row) = self.goats.and_then(|family| {
            family
                .goats()
                .iter()
                .find(|row| row.active && row.entity_index == slot)
        }) {
            let mut view = View::external(super::goat_source_entity_view(row));
            view.yaw = Some(row.yaw);
            view.roll = Some(row.roll);
            view.relations.parent = Some(row.parent_entity_index);
            view.relations.stage_mode = Some(row.stage_var_2 as u8);
            view.relations.stage_index = Some(row.stage_var_1 as u8);
            view.relations.stage_relation = Some(row.stage_target);
            return Some(view);
        }
        if let Some(row) = self.houses.and_then(|family| {
            family
                .houses()
                .iter()
                .find(|row| row.source_retained() && row.entity_index == slot)
        }) {
            return Some(View::external(super::house_source_entity_view(row)));
        }
        self.villagers
            .and_then(|family| {
                family
                    .villagers()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
            })
            .map(|row| {
                let mut view = View::external(super::villager_source_entity_view(row));
                view.yaw = Some(row.yaw);
                view.roll = Some(row.roll);
                view.relations.parent = Some(row.stage.parent);
                view.relations.stage_mode = Some(row.stage_var_2 as u8);
                view.relations.stage_index = Some(row.stage_var_1 as u8);
                view.relations.stage_relation = Some(row.stage.relation);
                view.relations.next = Some(row.linked_entity_index);
                view
            })
    }

    fn particle_shape(self, index: u16) -> Option<Shape> {
        self.particles
            .and_then(|rows| rows.get(usize::from(index)))
            .map(|particle| Shape {
                half_width: (particle.speed / 2) as i16,
                half_depth: (particle.speed / 2) as i16,
                vertical_offset: (particle.rotation_speed / 2) as i16,
                half_height: (particle.rotation_speed / 2) as i16,
            })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Relations {
    pub(crate) target: Option<u16>,
    pub(crate) attacker: Option<u16>,
    pub(crate) parent: Option<u16>,
    pub(crate) stage_mode: Option<u8>,
    pub(crate) stage_index: Option<u8>,
    pub(crate) stage_relation: Option<u16>,
    /// Source word0x34 child chain, distinct from word0x32 peer following.
    pub(crate) next: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct View {
    pub(crate) slot: u16,
    pub(crate) source_id: u16,
    pub(crate) active: bool,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) position: Position,
    pub(crate) life: i32,
    pub(crate) control_flags: u32,
    pub(crate) yaw: Option<i16>,
    pub(crate) roll: Option<i16>,
    pub(crate) relations: Relations,
    /// Collision axes are present only when the owning carrier or retained
    /// particle binding supplies them. Missing axes never come from old raw
    /// bytes left at a typed occupant's address.
    pub(crate) collision: Option<Shape>,
}

impl View {
    pub(crate) const fn drawing_disabled(self) -> bool {
        self.control_flags & 0x400 != 0
    }

    fn raw(row: &EntitySnapshot) -> Self {
        Self {
            slot: row.index,
            source_id: row.source_id,
            active: row.active,
            class: row.class,
            model: row.model,
            position: row.position,
            life: row.life,
            control_flags: row.control_flags,
            yaw: Some(row.yaw),
            roll: Some(row.source_roll()),
            relations: Relations {
                target: Some(u16::from_le_bytes([row.record[154], row.record[155]])),
                attacker: Some(row.word_26()),
                parent: Some(row.source_spell_parent_entity_index()),
                stage_mode: Some(row.record[77]),
                stage_index: Some(row.record[76]),
                stage_relation: Some(u16::from_le_bytes([row.record[78], row.record[79]])),
                next: Some(u16::from_le_bytes([row.record[56], row.record[57]])),
            },
            collision: Some(Shape {
                half_width: row.array_pitch(),
                half_depth: row.array_roll(),
                vertical_offset: row.array_yaw(),
                half_height: row.array_fov(),
            }),
        }
    }

    fn actor(slot: u16, actor: Actor, life: i32) -> Self {
        Self {
            slot,
            source_id: actor.source_id,
            active: true,
            class: actor.class,
            model: actor.model,
            position: actor.position,
            life,
            control_flags: actor.control_flags,
            yaw: None,
            roll: None,
            relations: Relations::default(),
            collision: Some(actor.shape),
        }
    }

    fn external(row: crate::level1_mission::Level1SourceEntityView) -> Self {
        Self {
            slot: row.source_entity_index,
            source_id: row.source_id,
            active: row.active,
            class: row.class,
            model: row.model,
            position: row.position,
            life: row.life,
            control_flags: row.control_flags,
            yaw: None,
            roll: None,
            relations: Relations {
                target: Some(row.target_entity_index),
                attacker: Some(row.current_attacker),
                ..Relations::default()
            },
            collision: Some(Shape {
                half_width: row.collision_half_width,
                half_depth: row.collision_half_depth,
                vertical_offset: row.collision_vertical_offset,
                half_height: row.collision_half_height,
            }),
        }
    }
}

/// Alliance directly dereferences these addresses even after reclaim. The
/// result supplements pointer reads without changing frozen search lists.
pub(crate) fn alliance_direct_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    if source.record[77] != 14 {
        return Ok(std::collections::BTreeSet::new());
    }
    direct_creature_references(arena, source)
}

/// sub_1E580 and sub_1E320 read fixed parent/selected-target records in the
/// Army follow and pursuit modes, independently of frozen search membership.
pub(crate) fn summoned_direct_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    if !matches!(source.record[77], 13 | 16 | 17) {
        return Ok(std::collections::BTreeSet::new());
    }
    direct_creature_references(arena, source)
}

fn direct_creature_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    let mut referenced = std::collections::BTreeSet::new();
    let mut pending = vec![
        source.source_spell_parent_entity_index(),
        source.source_word_96(),
        source.word_26(),
        u16::from_le_bytes([source.record[102], source.record[103]]),
        u16::from_le_bytes([source.record[56], source.record[57]]),
    ];
    while let Some(slot) = pending.pop() {
        if slot == 0 || referenced.contains(&slot) {
            continue;
        }
        let Some(target) = read(arena, Context::default(), slot)? else {
            continue;
        };
        referenced.insert(slot);
        pending.extend([
            target.relations.target.unwrap_or(0),
            target.relations.attacker.unwrap_or(0),
            target.relations.next.unwrap_or(0),
        ]);
    }
    Ok(referenced)
}

/// Current sub_10780 map-chain targets for one arrow. Ordinary actor domains
/// and typed arena carriers are resolved at this callback, not at reset.
pub(crate) fn arrow_collision_frame(
    arena: &SourceArena,
    slot: u16,
    context: Context<'_>,
) -> Result<super::creatures::archer::ArrowCollisionFrame, String> {
    use super::creatures::archer::{ArrowCollisionFrame, ArrowCollisionTarget};
    let arrow = arena.economy.authored_class5_archer_children.iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .ok_or_else(|| format!("Arrow {slot} lost its current carrier"))?;
    if arrow.action != super::creatures::archer::ARCHER_CHILD_ACTION {
        return Ok(ArrowCollisionFrame { targets: Vec::new(), owner_class_model: None });
    }
    let mut targets = Vec::new();
    let offsets = super::guided_projectile::collision_search_offsets(arrow.collision_half_width);
    for target_slot in crate::level1_mission::source_map_search_slots(&arena.map_chains, arrow.position, &offsets) {
        let Some(view) = read(arena, context, target_slot)? else { continue; };
        if view.control_flags & 8 == 0 { continue; }
        let shape = view.collision.ok_or_else(|| format!("Arrow target {target_slot} has no current axes"))?;
        let mana = match arena.economy.archer_arrow_target_properties(target_slot, &arena.entities) {
            Some((_, _, _, mana)) => mana,
            None if view.control_flags & 0x8010 == 0 => 0,
            None => return Err(format!("Protected arrow target {target_slot} has no current mana")),
        };
        targets.push(ArrowCollisionTarget {
            target: super::level2_dynamic_economy::Level2FireballCollisionTarget {
                source_slot: target_slot, source_id: view.source_id,
                class: view.class, model: view.model, position: view.position,
                half_width: shape.half_width, vertical_offset: shape.vertical_offset,
                half_height: shape.half_height,
            },
            control_flags: view.control_flags,
            mana,
        });
    }
    let owner_class_model = read(arena, context, arrow.entity_id)?.map(|view| (view.class, view.model));
    Ok(ArrowCollisionFrame { targets, owner_class_model })
}

/// Read the current fixed-address occupant; allocation is separate from a
/// source pointer's validity.
pub(crate) fn read(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<View>, String> {
    if slot == 0 {
        return Ok(None);
    }
    let Some(raw) = arena.entities.get(usize::from(slot)) else {
        return Ok(None);
    };
    if let Some(cursor) = arena.economy.live_arena_cursor_row(slot) {
        // These descriptor rows lend their mutable actor state to the three
        // external families. A different replacement kind cannot inherit it.
        if matches!(cursor.kind, Kind::AuthoredClass5 | Kind::AuthoredModel45)
            && let Some(external) = context.external(slot)
            && arena
                .economy
                .live_arena_class_action(slot, cursor.kind)
                .is_some_and(|(class, _)| class == external.class)
            && match cursor.kind {
                Kind::AuthoredClass5 => arena.economy.authored_class5.iter().any(|row| {
                    row.active && row.source_entity_index == slot && row.model == external.model
                }),
                Kind::AuthoredModel45 => external.model == 45,
                _ => false,
            }
        {
            let mut external = external;
            if let Some(state) = arena
                .economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                external.relations.next = Some(state.linked_head);
            }
            return Ok(Some(external));
        }
        return dynamic(arena, context, slot, cursor.kind).map(Some);
    }
    // The numeric dispatcher stops visiting a hidden typed flyer, but its
    // fixed row is still allocated until the following reclaim prologue.
    // Light and pointer consumers must see that hidden incarnation meanwhile.
    if !arena.economy.free_source_slots().contains(&slot)
        && let Some(projectile) = arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
    {
        return Ok(Some(fireball(projectile, context)));
    }
    if !arena.economy.free_source_slots().contains(&slot) {
        if let Some(row) = arena
            .economy
            .castle_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
        {
            return Ok(Some(castle_projectile(row)));
        }
        if let Some(row) = arena
            .economy
            .possession_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active)
        {
            return Ok(Some(possession_projectile(row)));
        }
    }
    let mut view = if raw.active
        && let Some(external) = context.external(slot)
        && (raw.class, raw.model) == (external.class, external.model)
    {
        external
    } else {
        View::raw(raw)
    };
    if raw.active
        && raw.class == 3
        && let Some((player_slot, position, life)) = context.player
        && player_slot == slot
    {
        view.position = position;
        view.life = life;
        if let Some((yaw, roll)) = context.player_orientation {
            view.yaw = Some(yaw);
            view.roll = Some(roll);
        }
    }
    Ok(Some(view))
}

/// The type-2 objective reads this byte only after the current target dies.
/// Resolve it from the same owner as `read`, never a typed row's raw shadow.
pub(crate) fn font_type_index(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<u8>, String> {
    let Some(current) = read(arena, context, slot)? else {
        return Ok(None);
    };
    if (current.class, current.model) == (10, 45)
        && let Some(house) = context.houses.and_then(|family| {
            family
                .houses()
                .iter()
                .find(|row| row.source_retained() && row.entity_index == slot)
        })
    {
        return Ok(Some(house.parameter.destruction_successor));
    }
    let Some(cursor) = arena.economy.live_arena_cursor_row(slot) else {
        return Ok(Some(arena.entities[usize::from(slot)].font_type_index));
    };
    match cursor.kind {
        // These constructors retain NewEvent's zero byte, and their source
        // callbacks do not write fontTypeIndex (4AC40, 4E5F0, 500C0,
        // 64E20, 31760, 30F60, 35940).
        Kind::ManaSphere | Kind::AuthoredTree | Kind::AuthoredModel6 => Ok(Some(0)),
        Kind::AuthoredModel76 => arena
            .economy
            .authored_model76
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map(|row| Some(row.font_type_index as u8))
            .ok_or_else(|| format!("objective target {slot} lost its Fire Spheres carrier")),
        Kind::SpellPickup | Kind::SpellReplacement => context
            .spells
            .and_then(|spells| spells.pickup(slot))
            .filter(|row| (row.class, row.model) == (current.class, current.model))
            .map(|row| Some(row.font_type_index))
            .ok_or_else(|| format!("objective target {slot} lost its spell carrier")),
        kind => Err(format!(
            "objective target {slot} has no font-type binding for current {kind:?}"
        )),
    }
}

fn dynamic(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
    kind: Kind,
) -> Result<View, String> {
    let economy = &arena.economy;
    if matches!(
        kind,
        Kind::ManaSphere
            | Kind::AuthoredTree
            | Kind::CastleRoot
            | Kind::CastleAuxiliary
            | Kind::BeeCreature
            | Kind::HouseResident
            | Kind::AuthoredDragonPart
            | Kind::AuthoredClass5Model3Child
            | Kind::AuthoredClass5
    ) {
        let actor = area_damage::read_arena_actor(arena, slot)
            .ok_or_else(|| format!("target slot {slot} lost its current {kind:?} actor"))?;
        let life = area_damage::read_arena_life(arena, slot)
            .ok_or_else(|| format!("target slot {slot} lost its current {kind:?} life"))?;
        let mut view = View::actor(slot, actor, life);
        (view.yaw, view.roll) = orientation(arena, kind, slot);
        view.relations = relations(arena, kind, slot);
        return Ok(view);
    }
    macro_rules! carrier {
        ($rows:expr, $row:ident, $class:expr, $model:expr, $id:expr, $life:expr, $flags:expr) => {{
            let $row = $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost current {kind:?} carrier"))?;
            View {
                slot,
                source_id: $id,
                active: true,
                class: $class,
                model: $model,
                position: $row.position,
                life: i32::from($life),
                control_flags: $flags,
                yaw: None,
                roll: None,
                relations: Relations::default(),
                collision: None,
            }
        }};
    }
    let mut view = match kind {
        Kind::FireballProjectile => fireball(
            economy
                .fireball_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Fireball carrier"))?,
            context,
        ),
        Kind::CastleProjectile => castle_projectile(
            economy
                .castle_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Castle flyer"))?,
        ),
        Kind::PossessionProjectile => possession_projectile(
            economy
                .possession_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Possession flyer"))?,
        ),
        Kind::PossessionEffect => carrier!(
            economy.possession_effects,
            row,
            row.source_class,
            row.source_model,
            row.source_id_entity_index,
            row.life,
            // NewAdd0A0C clears bits 0 and 3, then sets bits 0 and map-link 2.
            5
        ),
        Kind::ManaCarrier => {
            let mut view = carrier!(
                economy.carriers,
                row,
                crate::level1_mission::MANA_CARRIER_CLASS,
                crate::level1_mission::MANA_CARRIER_MODEL,
                row.owner_entity_index,
                row.life,
                12
            );
            view.collision = context.map_type.and_then(|map_type| {
                context.particle_shape(169).map(|mut shape| {
                    if map_type == super::MapType::Cave {
                        shape.half_width = 256;
                        shape.half_depth = 256;
                        shape.half_height = 768;
                    }
                    shape
                })
            });
            view
        }
        Kind::SpellPickup | Kind::SpellReplacement => {
            // The typed arena row owns allocation/dispatch; the shared spell
            // owner retains this live record's mutable life, flags and axes.
            // The raw arena copy need not have been published yet.
            let family = match kind {
                Kind::SpellPickup => economy
                    .spell_pickups
                    .iter()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .map(|row| row.family),
                _ => economy
                    .spell_replacements
                    .iter()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .map(|row| row.family),
            }
            .ok_or_else(|| format!("target slot {slot} lost its current spell carrier"))?;
            let row = context
                .spells
                .and_then(|spells| spells.pickup(slot))
                .filter(|row| row.active && row.class == 15 && row.model == family)
                .ok_or_else(|| {
                    format!("target slot {slot} lacks its current shared spell record")
                })?;
            View::raw(row)
        }
        Kind::CastleChild | Kind::CastleController | Kind::CastleDefender => {
            let mut view = carrier!(
                economy.castle_children,
                row,
                row.source_class,
                row.model,
                row.id_entity_index,
                row.life,
                row.control_flags
            );
            let row = economy
                .castle_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its Castle child"))?;
            if row.model == 43 && row.action == 45 {
                view.yaw = Some(0);
                view.roll = Some(0);
                view.collision = Some(Shape {
                    vertical_offset: row.yaw.cast_signed(),
                    half_width: row.roll.cast_signed(),
                    half_depth: row.roll.cast_signed(),
                    half_height: row.roll.cast_signed(),
                });
                return Ok(view);
            }
            view.yaw = Some(row.yaw as i16);
            view.roll = Some(row.roll as i16);
            view.relations.target = Some(row.target_sphere_slot);
            view.relations.attacker = Some(row.damage_attacker_entity_index);
            view.relations.parent = Some(row.parent_entity_index);
            if kind == Kind::CastleDefender {
                // This is sub_4ABA0's real class3/model3 balloon carrier;
                // the separate class5/model15 owner is CastleAuxiliary.
                view.collision = context.map_type.and_then(|map_type| {
                    context.particle_shape(row.visual_index).map(|mut shape| {
                        if map_type == super::MapType::Cave {
                            shape.half_width = 256;
                            shape.half_depth = 256;
                            shape.half_height = 768;
                        }
                        shape
                    })
                });
            }
            view
        }
        Kind::DeathBurst => carrier!(
            economy.death_bursts,
            row,
            10,
            1,
            row.id_entity_index,
            row.life_ticks,
            row.control_flags
        ),
        Kind::PlayerDeathMarker => {
            let row = economy
                .player_death_markers
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its player death marker"))?;
            let mut view = carrier!(
                economy.player_death_markers,
                marker,
                10,
                40,
                slot,
                marker.life,
                marker.control_flags
            );
            view.collision = context.particle_shape(row.particle_parameter_index);
            view
        }
        Kind::SpeedTrail => carrier!(
            economy.speed_trails,
            row,
            row.class,
            row.model,
            row.source_id,
            row.life,
            row.control_flags
        ),
        Kind::TerrainStripCloseTrigger => carrier!(
            economy.terrain_strip_close_triggers,
            row,
            row.class,
            row.model,
            slot,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredSwitchProducer => carrier!(
            economy.authored_switch_producers,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            // AddSwitchXX copies NewEvent's maxLife; visits change dword10,
            // not life. Its collision bit is cleared by the constructor.
            300,
            1 | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredClass14 => {
            let row = economy
                .authored_class14
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its class14 carrier"))?;
            let mut view = carrier!(
                economy.authored_class14,
                row,
                14,
                row.model,
                slot,
                // sub_514E0 copies NewEvent's maxLife before SetHalfSpeed.
                300,
                12 | u32::from(row.control_flags) | if row.drawing_enabled { 0 } else { 0x400 }
            );
            view.collision = context.particle_shape(row.particle).map(|mut shape| {
                if row.model == 5 {
                    shape.half_width = row.shift_rot_x as i16;
                    shape.half_depth = row.shift_rot_x as i16;
                    shape.half_height = row.shift_rot_y as i16;
                }
                shape
            });
            view
        }
        Kind::ManaAttraction => carrier!(
            economy.mana_attraction_fields,
            row,
            10,
            54,
            slot,
            row.life,
            // AddAuxiliary_50500 clears bits 0 and 3, then sets bit 0.
            1
        ),
        Kind::AuthoredModel1 => carrier!(
            economy.authored_model1,
            row,
            10,
            row.model,
            slot,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel45 => carrier!(
            economy.authored_model45,
            row,
            row.source_class,
            row.model,
            slot,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel13 => carrier!(
            economy.authored_model13,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model13SpawnedChild => {
            let mut view = carrier!(
                economy.model13_spawned_children,
                row,
                row.source_class,
                row.model,
                row.parent_entity_id,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            let row = economy
                .model13_spawned_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("the current smoke14 carrier was just read");
            view.yaw = Some(row.yaw);
            view.collision = context.particle_shape(row.particle_index);
            view
        }
        Kind::AuthoredModel6 => carrier!(
            economy.authored_model6,
            row,
            10,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags
        ),
        Kind::Model6SpawnedChild => carrier!(
            economy.model6_spawned_children,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredModel17 => {
            let row = economy
                .authored_model17
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost current {kind:?} carrier"))?;
            let mut view = carrier!(
                economy.authored_model17,
                row,
                row.source_class,
                row.model,
                row.entity_id,
                row.life,
                row.control_flags
            );
            view.yaw = Some(row.yaw);
            view.relations.target = Some(row.target_source_entity_index);
            view
        }
        Kind::Model17SpawnedChild => carrier!(
            economy.model17_spawned_children,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredModel22 | Kind::Model22SpawnedChild => economy
            .fields
            .source_rows()
            .iter()
            .find(|row| row.index == slot)
            .map(View::raw)
            .ok_or_else(|| format!("target slot {slot} lost its Wind carrier"))?,
        Kind::AuthoredModel76 => carrier!(
            economy.authored_model76,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model76Child => carrier!(
            economy.model76_children,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model76TailSpawn => carrier!(
            economy.model76_tail_spawns,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredClass5ArcherChild => {
            let row = economy.authored_class5_archer_children.iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("Arrow {slot} lost its current collision axes"))?;
            let mut view = carrier!(
                economy.authored_class5_archer_children,
                row,
                row.source_class,
                row.model,
                row.entity_id,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            view.collision = Some(Shape {
                half_width: row.collision_half_width,
                half_depth: row.collision_half_width,
                vertical_offset: row.collision_vertical_offset,
                half_height: row.collision_half_height,
            });
            view
        }
        Kind::AuthoredModel32 => carrier!(
            economy.authored_model32,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel59 => carrier!(
            economy.authored_model59,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model59SmokeChild => {
            let mut view = carrier!(
                economy.model59_smoke_children,
                row,
                row.source_class,
                row.model,
                slot,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            let row = economy
                .model59_smoke_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("the current smoke13 carrier was just read");
            view.yaw = Some(row.yaw);
            view.collision = context.particle_shape(row.particle_index);
            view
        }
        Kind::AuthoredModel53 => carrier!(
            economy.authored_model53,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel5WaterSplash => carrier!(
            economy.authored_model5_water_splashes,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags
        ),
        _ => {
            return Err(format!(
                "target slot {slot} has no current projection for {kind:?}"
            ));
        }
    };
    let (yaw, roll) = orientation(arena, kind, slot);
    if yaw.is_some() {
        view.yaw = yaw;
    }
    if roll.is_some() {
        view.roll = roll;
    }
    let current_relations = relations(arena, kind, slot);
    view.relations = Relations {
        target: current_relations.target.or(view.relations.target),
        attacker: current_relations.attacker.or(view.relations.attacker),
        parent: current_relations.parent.or(view.relations.parent),
        stage_mode: current_relations.stage_mode.or(view.relations.stage_mode),
        stage_index: current_relations.stage_index.or(view.relations.stage_index),
        stage_relation: current_relations
            .stage_relation
            .or(view.relations.stage_relation),
        next: current_relations.next.or(view.relations.next),
    };
    Ok(view)
}

fn fireball(row: &super::Level2FireballProjectile, context: Context<'_>) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.owner_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: i32::from(row.life),
        control_flags: row.control_flags | if row.drawing_disabled { 0x400 } else { 0 },
        yaw: Some(row.yaw),
        roll: Some(row.roll),
        relations: Relations {
            target: Some(row.target_source_entity_index),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: context.particle_shape(340),
    }
}

fn castle_projectile(row: &super::Level2CastleProjectile) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.owner_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: row.life,
        control_flags: row.control_flags | if row.drawing_disabled { 0x400 } else { 0 },
        yaw: Some(row.yaw),
        roll: Some(row.source_roll),
        relations: Relations {
            target: Some(row.source_word_96),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: Some(row.collision_shape),
    }
}

fn possession_projectile(row: &super::Level2PossessionProjectile) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.source_id_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: i32::from(row.life),
        control_flags: 4
            | u32::from(row.acquisition_attempted) * 2
            | u32::from(row.threat_accounted) * 0x2000
            | if row.active { 0 } else { 0x400 },
        yaw: Some(row.yaw),
        roll: Some(row.roll),
        relations: Relations {
            target: Some(row.target_source_entity_index),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: None,
    }
}

fn relations(arena: &SourceArena, kind: Kind, slot: u16) -> Relations {
    let economy = &arena.economy;
    match kind {
        Kind::BeeCreature => economy
            .bee_creatures
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_entity_index.unwrap_or(0)),
                attacker: Some(row.damage_attacker_entity_index),
                parent: Some(row.parent_entity_index),
                stage_mode: Some(row.stage_var2),
                stage_index: Some(row.stage_var1),
                stage_relation: Some(row.stage_relation),
                next: Some(row.linked_child_entity_index),
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5 => {
            let mut relation = economy
                .authored_class5
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| Relations {
                    target: Some(row.word_0x96_150),
                    attacker: Some(row.word_0x26),
                    ..Relations::default()
                })
                .unwrap_or_default();
            if let Some(state) = economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.next = Some(state.linked_head);
            }
            if let Some(state) = economy
                .authored_class5_archer
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.stage.parent);
                relation.stage_mode = Some(state.stage.mode);
                relation.stage_index = Some(state.stage.index);
                relation.stage_relation = Some(state.stage.relation);
                relation.next = Some(state.word_0x34);
            } else if let Some(state) = economy
                .authored_fireflies
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
            } else if let Some(state) = economy
                .authored_dragons
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
            } else if let Some(state) = economy
                .authored_class5_model3
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
                relation.next = Some(state.word_0x34);
            }
            relation
        }
        Kind::HouseResident => economy
            .house_residents
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_entity_index),
                attacker: Some(row.current_attacker),
                parent: Some(row.stage.parent),
                stage_mode: Some(row.stage_var_2 as u8),
                stage_index: Some(row.stage_var_1 as u8),
                stage_relation: Some(row.stage.relation),
                next: Some(row.linked_entity_index),
            })
            .unwrap_or_default(),
        Kind::AuthoredDragonPart => economy
            .authored_dragon_parts
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.next_entity_index),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5Model3Child => economy
            .authored_class5_model3_children
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.word_0x96),
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.word_0x34),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::FireballProjectile => economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_source_entity_index),
                attacker: Some(row.holder_entity_index),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5ArcherChild => economy
            .authored_class5_archer_children
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.word_0x96),
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.word_0x34),
                ..Relations::default()
            })
            .unwrap_or_default(),
        _ => Relations::default(),
    }
}

fn orientation(arena: &SourceArena, kind: Kind, slot: u16) -> (Option<i16>, Option<i16>) {
    macro_rules! angles {
        ($rows:expr) => {
            $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (Some(row.yaw as i16), Some(row.roll as i16)))
                .unwrap_or((None, None))
        };
    }
    match kind {
        Kind::ManaSphere => arena.economy.spheres.iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| (Some(row.yaw), None))
            .unwrap_or((None, None)),
        Kind::FireballProjectile => angles!(arena.economy.fireball_projectiles),
        Kind::BeeCreature => angles!(arena.economy.bee_creatures),
        Kind::AuthoredClass5 => angles!(arena.economy.authored_class5),
        Kind::AuthoredDragonPart => angles!(arena.economy.authored_dragon_parts),
        Kind::AuthoredClass5ArcherChild => angles!(arena.economy.authored_class5_archer_children),
        Kind::AuthoredModel22 => angles!(arena.economy.fields.authored_model22),
        Kind::Model22SpawnedChild => angles!(arena.economy.fields.model22_spawned_children),
        Kind::Model76Child => angles!(arena.economy.model76_children),
        // AddTree and actions 0..2 retain NewEvent's pose. The typed Tree
        // yaw/roll fields are array_0x52 collision axes, not pose angles.
        Kind::AuthoredTree => (Some(0), Some(0)),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_typed_creature_overrides_retained_raw_target_bytes() {
        let mut session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let position = session.state.position;
        let bee = arena
            .economy
            .allocate_authored_bee(1, position, None, 3000, &session.session.state.live.terrain)
            .expect("ordinary authored Bee constructor");
        let slot = bee.source_entity_index;
        let raw = &mut arena.entities[usize::from(slot)];
        raw.class = 9;
        raw.model = 9;
        raw.set_position(Position { x: 3, y: 4, z: 5 });
        raw.set_life(-400);
        raw.set_control_flags(0x400);
        raw.record[154..156].copy_from_slice(&944_u16.to_le_bytes());
        raw.record[76] = 71;
        raw.record[78..80].copy_from_slice(&988_u16.to_le_bytes());
        raw.record[56..58].copy_from_slice(&977_u16.to_le_bytes());
        let current = arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        current.target_entity_index = Some(111);
        current.damage_attacker_entity_index = 222;
        current.parent_entity_index = 333;
        current.stage_var2 = 14;
        current.stage_var1 = 6;
        current.stage_relation = 444;
        current.linked_child_entity_index = 555;

        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model), (bee.source_class, bee.model));
        assert_eq!((target.position, target.life), (bee.position, bee.life));
        assert_eq!(target.control_flags, bee.control_flags);
        assert!(target.collision.is_some());
        assert_eq!(
            target.relations,
            Relations {
                target: Some(111),
                attacker: Some(222),
                parent: Some(333),
                stage_mode: Some(14),
                stage_index: Some(6),
                stage_relation: Some(444),
                next: Some(555),
            }
        );

        // The objective consumer must use this same current carrier. A dead
        // retained packet cannot kill a live Bee; a live packet cannot hide
        // the current Bee's death from a type-1 objective.
        use crate::campaign::objectives::target_death::{Form, achieved_in_arena};
        let stage = super::super::RuntimeStage {
            kind: 1,
            flags: 1,
            axis_x: 0,
            axis_y: 0,
            relation: super::super::RuntimeRelation::LiveEntity(slot),
        };
        assert!(!achieved_in_arena(Form::TargetDeath, &stage, &arena, Context::default()).unwrap());
        arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap()
            .life = -1;
        arena.entities[usize::from(slot)].set_life(400);
        assert!(achieved_in_arena(Form::TargetDeath, &stage, &arena, Context::default()).unwrap());
        arena.entities[usize::from(slot)].set_life(-400);

        // Removing the typed occupant changes the same address read. No old
        // value or lifetime handle can keep selecting the former Bee.
        arena
            .economy
            .bee_creatures
            .retain(|row| row.source_entity_index != slot);
        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model, target.life), (9, 9, -400));
        assert!(target.drawing_disabled());
        assert_eq!(target.relations.target, Some(944));
        assert_eq!(target.relations.stage_index, Some(71));
        assert_eq!(target.relations.stage_relation, Some(988));
        assert_eq!(target.relations.next, Some(977));

        // A death can allocate a sphere at a Dragon predecessor address.
        // The following segment must read that new carrier's actual yaw,
        // not the old fixed-address bytes left by its predecessor.
        let sphere_slot = arena.economy.spawn_mana_sphere_from_death(
            999, position, 57, 0x1234, 500, 0, &session.session.state.live.terrain,
        ).expect("real death constructor allocates a mana sphere") as u16;
        let sphere = arena.economy.spheres.iter_mut()
            .find(|row| row.active && row.source_entity_index == sphere_slot).unwrap();
        sphere.yaw = 753;
        arena.entities[usize::from(sphere_slot)].set_yaw(-321);
        let target = read(&arena, Context::default(), sphere_slot).unwrap().unwrap();
        assert_eq!((target.class, target.model, target.yaw), (10, 39, Some(753)));
        arena.economy.spheres.iter_mut().find(|row| row.source_entity_index == sphere_slot).unwrap().yaw = 754;
        assert_eq!(read(&arena, Context::default(), sphere_slot).unwrap().unwrap().yaw, Some(754));

        let mut sphere_stage = stage.clone();
        sphere_stage.kind = 2;
        sphere_stage.relation = super::super::RuntimeRelation::LiveEntity(sphere_slot);
        arena.entities[usize::from(sphere_slot)].font_type_index = 7;
        arena.entities[usize::from(sphere_slot)].set_life(400);
        arena
            .economy
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .unwrap()
            .life = -1;
        assert!(
            achieved_in_arena(
                Form::TargetDeathWithFontTypeZero,
                &sphere_stage,
                &arena,
                Context::default(),
            )
            .unwrap(),
            "the current sphere retains its constructor's zero font byte"
        );
        arena
            .economy
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .unwrap()
            .life = 0;
        assert!(
            !achieved_in_arena(
                Form::TargetDeathWithFontTypeZero,
                &sphere_stage,
                &arena,
                Context::default(),
            )
            .unwrap(),
            "zero life is not source death"
        );

        let tree = arena
            .economy
            .allocate_authored_tree(260, position, &session.session.definition.presentation.particles)
            .expect("ordinary Tree constructor");
        let current_tree = arena
            .economy
            .authored_tree_mut(tree.source_entity_index)
            .unwrap();
        current_tree.yaw = 612;
        current_tree.roll = 345;
        arena.entities[usize::from(tree.source_entity_index)].set_yaw(999);
        arena.entities[usize::from(tree.source_entity_index)].set_source_roll(-111);
        let target = read(&arena, Context::default(), tree.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.yaw, target.roll), (Some(0), Some(0)));
        let shape = target.collision.expect("current Tree collision axes");
        assert_eq!((shape.vertical_offset, shape.half_depth), (612, 345));

        let live = &session.session.state.live;
        let house = live
            .ordinary
            .houses()
            .iter()
            .find(|row| row.source_retained())
            .expect("ordinary Level2 House");
        arena.entities[usize::from(house.entity_index)].font_type_index =
            house.parameter.destruction_successor.wrapping_add(1);
        assert_eq!(
            font_type_index(
                &arena,
                Context::from_ordinary(&live.goats, &live.ordinary),
                house.entity_index,
            )
            .unwrap(),
            Some(house.parameter.destruction_successor)
        );
        let goat = live
            .goats
            .goats()
            .iter()
            .find(|goat| goat.active)
            .expect("ordinary Level2 Goat");
        let raw = &mut arena.entities[usize::from(goat.entity_index)];
        raw.record[76] = (goat.stage_var_1 as u8).wrapping_add(1);
        raw.record[78..80].copy_from_slice(&goat.stage_target.wrapping_add(1).to_le_bytes());
        raw.set_yaw(goat.yaw.wrapping_add(1));
        raw.set_source_roll(goat.roll.wrapping_add(1));
        let target = read(
            &arena,
            Context::from_ordinary(&live.goats, &live.ordinary),
            goat.entity_index,
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.yaw, Some(goat.yaw));
        assert_eq!(target.roll, Some(goat.roll));
        assert_eq!(target.relations.stage_index, Some(goat.stage_var_1 as u8));
        assert_eq!(target.relations.stage_relation, Some(goat.stage_target));
        assert_eq!(target.relations.parent, Some(goat.parent_entity_index));

        // Castle model3 defenders are castle_children, whereas model15
        // auxiliaries use the separate castle_defenders collection.
        let owner = session.state.entity_index;
        let castle = session.economy_mut().build_castle(position, owner).unwrap();
        session.economy_mut().maintain_castle_defenders(castle);
        let defender = session
            .economy()
            .castle_children()
            .iter()
            .find(|row| row.active && (row.source_class, row.model) == (3, 3))
            .copied()
            .expect("ordinary Castle maintenance creates a model3 defender");
        let mut arena = session.source_arena().clone();
        let raw = &mut arena.entities[usize::from(defender.source_entity_index)];
        raw.active = true;
        raw.class = 9;
        raw.model = 9;
        raw.set_life(-400);
        raw.record[76] = 88;
        let context = Context {
            particles: Some(&session.session.definition.presentation.particles),
            map_type: Some(super::super::MapType::Day),
            ..Context::default()
        };
        let target = read(&arena, context, defender.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.class, target.model), (3, 3));
        assert_eq!(
            (target.source_id, target.life),
            (defender.id_entity_index, defender.life)
        );
        assert_eq!(target.position, defender.position);
        assert!(target.collision.is_some());

        assert_eq!(
            target.relations.stage_index, None,
            "an unavailable typed stage never inherits an old raw byte"
        );

        arena
            .economy
            .castle_children
            .iter_mut()
            .find(|row| row.source_entity_index == defender.source_entity_index)
            .unwrap()
            .active = false;
        let target = read(&arena, context, defender.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.class, target.model, target.life), (9, 9, -400));

        // A House birth retains its provenance after a StageVar/Alliance
        // parent changes. Only the actual source parent belongs in this view.
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let resident_slot = live
            .economy
            .allocate_house_resident(
                10,
                0,
                crate::level1_mission::HouseResidentRequest {
                    position,
                    model: 13,
                    action: 105,
                    attacker_entity_index: 0,
                },
                &mut live.terrain,
            )
            .expect("ordinary Villager constructor");
        live.economy
            .economy
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == resident_slot)
            .unwrap()
            .stage
            .parent = parent;
        live.economy.entities[usize::from(resident_slot)].record[44..46]
            .copy_from_slice(&900_u16.to_le_bytes());
        let target = read(&live.economy, Context::default(), resident_slot)
            .unwrap()
            .unwrap();
        assert_eq!(target.relations.parent, Some(parent));
        let resident = live
            .economy
            .economy
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == resident_slot)
            .unwrap();
        assert_eq!(resident.parent_house_entity_index, 10);
        resident.stage.parent = 0;
        let target = read(&live.economy, Context::default(), resident_slot)
            .unwrap()
            .unwrap();
        assert_eq!(target.relations.parent, Some(0));
    }

    #[test]
    fn current_spell_carrier_uses_shared_owner_before_raw_publication() {
        let mut session = super::super::tests::level_two_campaign_session();
        let feature = *session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 156)
            .expect("retained Level2 family11 authored producer");
        assert_eq!((feature.kind, feature.subtype), (15, 11));
        session
            .activate_authored_group_prerequisite_for_test(
                u16::try_from(feature.activation_group).unwrap(),
            )
            .unwrap();
        let parent = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.authored_index == feature.index)
            .expect("real group constructor publishes family11")
            .source_entity_index;
        assert_eq!(session.shared_spells().pickup(parent).unwrap().model, 11);
        let replacement = session
            .allocate_level2_spell_replacement(11, parent, session.state.position)
            .expect("ordinary persistent spell replacement constructor");
        let slot = replacement.source_entity_index;
        let mut arena = session.source_arena().clone();
        let mut spells = session.shared_spells().clone();
        let current = spells.pickup_mut(slot).unwrap();
        current.set_position(Position {
            x: 111,
            y: 222,
            z: 333,
        });
        current.set_life(-7);
        current.set_control_flags(0x404);
        current.record[154..156].copy_from_slice(&944_u16.to_le_bytes());
        current.record[44..46].copy_from_slice(&parent.to_le_bytes());
        let expected = View::raw(current);

        // Even a plausible same-family arena copy is not the shared owner's
        // current mutable carrier before publication to the next dispatch.
        let raw = &mut arena.entities[usize::from(slot)];
        raw.class = 15;
        raw.model = 11;
        raw.set_position(Position::default());
        raw.set_life(700);
        raw.set_control_flags(8);
        let context = Context {
            spells: Some(&spells),
            ..Context::default()
        };
        assert_eq!(read(&arena, context, slot).unwrap(), Some(expected));
        assert!(read(&arena, Context::default(), slot).is_err());

        // A record for another family at the same numeric address cannot
        // satisfy this typed occupant's current-owner requirement.
        spells.pickup_mut(slot).unwrap().model = 12;
        let context = Context {
            spells: Some(&spells),
            ..Context::default()
        };
        assert!(read(&arena, context, slot).is_err());
    }

    #[test]
    fn hidden_fireball_remains_current_until_the_source_reclaim_boundary() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = arena
            .economy
            .spawn_fireball_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: super::super::FireballProjectileOrigin::PlayerSpell,
                    position: session.state.position,
                    yaw: 21,
                    pitch: 22,
                    axis_position: Position::default(),
                    source_id_entity_index: session.state.entity_index,
                    holder_entity_index: 0,
                    source_subspell_index: 100,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                },
                0,
            )
            .expect("ordinary Fireball constructor");
        assert!(
            !read(&arena, Context::default(), slot)
                .unwrap()
                .unwrap()
                .drawing_disabled()
        );
        let row = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        row.active = false;
        row.drawing_disabled = true;
        assert!(arena.economy.live_arena_cursor_row(slot).is_none());
        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model), (9, 0));
        assert_eq!(target.position, session.state.position);
        assert!(!target.active && target.drawing_disabled());

        let castle = arena
            .economy
            .spawn_castle_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
                    position: session.state.position,
                    axis_position: session.state.position,
                    yaw: session.state.yaw,
                    pitch: 0,
                    source_id_entity_index: session.state.entity_index,
                    source_subspell_index: 0,
                    source_mana: session.shared_spells().player_mana(),
                    source_a4_byte_154: 0,
                },
                0,
                0,
                &session.session.state.live.terrain,
            )
            .expect("ordinary Castle flyer constructor");
        let possession = arena
            .economy
            .spawn_possession_projectile(
                session.state.position,
                session.state.entity_index,
                0,
                session.state.yaw,
                0,
                0,
                session.state.position,
            )
            .expect("ordinary Possession flyer constructor");
        let row = arena
            .economy
            .castle_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == castle)
            .unwrap();
        row.active = false;
        row.drawing_disabled = true;
        arena
            .economy
            .possession_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == possession)
            .unwrap()
            .active = false;
        for (slot, model) in [(castle, 10), (possession, 1)] {
            // A retained packet at the same address cannot resurrect the
            // previous occupant while its typed flyer awaits reclaim.
            let raw = &mut arena.entities[usize::from(slot)];
            raw.class = 2;
            raw.model = 7;
            raw.set_life(-900);
            let current = read(&arena, Context::default(), slot).unwrap().unwrap();
            assert_eq!((current.class, current.model), (9, model));
            assert_eq!(current.position, session.state.position);
            assert_eq!(current.source_id, session.state.entity_index);
            assert!(!current.active && current.drawing_disabled());
        }
        arena.economy.reclaim_inactive_slots_at_source_boundary();
        for slot in [castle, possession] {
            assert!(arena.economy.free_source_slots().contains(&slot));
            let retained = read(&arena, Context::default(), slot).unwrap().unwrap();
            assert_eq!(
                (retained.class, retained.model, retained.life),
                (2, 7, -900)
            );
        }
    }

    #[test]
    fn live_player_override_does_not_hide_retained_free_address_bytes() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let current = Position {
            x: 1000,
            y: 2000,
            z: 300,
        };
        let context = Context {
            player: Some((slot, current, 17)),
            player_orientation: Some((21, 22)),
            ..Context::default()
        };
        let target = read(&arena, context, slot).unwrap().unwrap();
        assert_eq!((target.position, target.life), (current, 17));
        assert_eq!((target.yaw, target.roll), (Some(21), Some(22)));
        let raw = &mut arena.entities[usize::from(slot)];
        let retained = (raw.position, raw.life);
        let retained_orientation = (Some(raw.yaw), Some(raw.source_roll()));
        raw.active = false;
        let target = read(&arena, context, slot).unwrap().unwrap();
        assert!(!target.active);
        assert_eq!((target.position, target.life), retained);
        assert_eq!((target.yaw, target.roll), retained_orientation);
        assert!(read(&arena, context, 0).unwrap().is_none());
        assert!(read(&arena, context, u16::MAX).unwrap().is_none());
    }
}
