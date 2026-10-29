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

    fn external(self, slot: u16) -> Option<crate::level1_mission::Level1SourceEntityView> {
        self.goats
            .and_then(|family| {
                family
                    .goats()
                    .iter()
                    .find(|row| row.active && row.entity_index == slot)
            })
            .map(super::goat_source_entity_view)
            .or_else(|| {
                self.houses
                    .and_then(|family| {
                        family
                            .houses()
                            .iter()
                            .find(|row| row.source_retained() && row.entity_index == slot)
                    })
                    .map(super::house_source_entity_view)
            })
            .or_else(|| {
                self.villagers
                    .and_then(|family| {
                        family
                            .villagers()
                            .iter()
                            .find(|row| row.source_retained() && row.entity_index == slot)
                    })
                    .map(super::villager_source_entity_view)
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

/// Read a source pointer, including retained bytes at a free address. Only
/// the sentinel and addresses outside the source array are absent. A current
/// typed occupant is resolved before considering external sidecars or raw
/// storage. An unsupported typed carrier is an explicit integration error.
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
            return Ok(Some(View::external(external)));
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
        View::external(external)
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
        Kind::AuthoredModel17 => carrier!(
            economy.authored_model17,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags
        ),
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
        Kind::AuthoredClass5ArcherChild => carrier!(
            economy.authored_class5_archer_children,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredTree => carrier!(
            economy.authored_trees,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
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
        roll: None,
        relations: Relations {
            target: Some(row.source_word_96),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: None,
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
                .authored_class5_archer
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.stage.parent);
                relation.stage_mode = Some(state.stage.mode);
            } else if let Some(state) = economy
                .authored_fireflies
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
            } else if let Some(state) = economy
                .authored_dragons
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
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
                parent: Some(row.parent_house_entity_index),
                stage_mode: Some(row.stage_var_2 as u8),
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
        Kind::FireballProjectile => angles!(arena.economy.fireball_projectiles),
        Kind::BeeCreature => angles!(arena.economy.bee_creatures),
        Kind::AuthoredClass5 => angles!(arena.economy.authored_class5),
        Kind::AuthoredDragonPart => angles!(arena.economy.authored_dragon_parts),
        Kind::AuthoredClass5ArcherChild => angles!(arena.economy.authored_class5_archer_children),
        Kind::AuthoredModel22 => angles!(arena.economy.fields.authored_model22),
        Kind::Model22SpawnedChild => angles!(arena.economy.fields.model22_spawned_children),
        Kind::Model76Child => angles!(arena.economy.model76_children),
        Kind::AuthoredTree => angles!(arena.economy.authored_trees),
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
            }
        );

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
