// Moved mechanically from native_world.rs for mc-split-nw-economy-impl-fikw6.
// This child module preserves native_world's prior effective visibility boundary.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthoredBeeSound {
    Action17Acquire(u16),
    Action18Contact(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthoredBeeStepOutcome {
    pub(super) live: bool,
    pub(super) sound: Option<AuthoredBeeSound>,
}

impl AuthoredBeeStepOutcome {
    const INACTIVE: Self = Self {
        live: false,
        sound: None,
    };
}

impl Level2EconomyState {
    #[cfg(test)]
    pub(super) fn from_parts(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        player_mana: i32,
        player_model: u8,
        player_terrain_flags: u8,
        player_spell_slot_2: u16,
        global_rng: u32,
    ) -> Result<Self> {
        Self::from_parts_with_castle_stage_footprints(
            terrain,
            entities,
            player_mana,
            player_model,
            player_terrain_flags,
            player_spell_slot_2,
            global_rng,
            BTreeMap::new(),
        )
    }

    pub(super) fn from_parts_with_castle_stage_footprints(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        player_mana: i32,
        player_model: u8,
        player_terrain_flags: u8,
        player_spell_slot_2: u16,
        global_rng: u32,
        castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
    ) -> Result<Self> {
        let mut free_slots = entities
            .iter()
            .filter(|entity| !entity.active && entity.index != 0)
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        free_slots.sort_unstable_by(|left, right| right.cmp(left));
        let slot_rng = entities
            .iter()
            .filter(|entity| !entity.active && entity.index != 0)
            .map(|entity| (entity.index, entity.entity_rng))
            .collect::<BTreeMap<_, _>>();
        let owner_class = entities
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| (entity.index, entity.class))
            .collect::<BTreeMap<_, _>>();
        let source_owner = entities
            .iter()
            .map(|entity| (entity.index, entity.source_player_entity_index()))
            .collect::<BTreeMap<_, _>>();
        let baseline_owner_max_mana = entities
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| (entity.index, entity.max_mana))
            .collect::<BTreeMap<_, _>>();
        let static_spheres = level2_static_mana_spheres_from_entities(entities, &source_owner);
        let owner_mana_rows = level2_owner_mana_rows_from_entities(entities);
        let owner_max_mana =
            level2_owner_max_mana_projection(&baseline_owner_max_mana, &owner_mana_rows);
        let (owner_mana_aggregate, owner_mana_global) =
            level2_owner_mana_projection(&owner_mana_rows);
        let sentinel_rng = entities
            .iter()
            .find(|entity| entity.index == 0)
            .map_or(0, |entity| entity.entity_rng);
        let source_creature_population = entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.class == 5
                    && entity.action != 232
                    && (entity.model < 12 || (entity.model > 15 && entity.model != 22))
            })
            .count()
            .try_into()
            .expect("source creature population fits u32");
        let raw_map_entries =
            raw_source_map_chain_entries(&terrain.planes().entity_indices, |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            })
            .map_err(NativeWorldError::new)?;
        let map_seeds = raw_map_entries
            .iter()
            .map(|entry| {
                let entity = &entities[usize::from(entry.entity_index)];
                RenderableMapSeed {
                    entity_index: entry.entity_index,
                    active: entity.active,
                    x: entity.position.x,
                    y: entity.position.y,
                }
            })
            .collect::<Vec<_>>();
        let map_chains = RenderableMapChains::from_raw_reset(
            &map_seeds,
            &terrain.planes().entity_indices,
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::next_entity)
            },
        )
        .map_err(NativeWorldError::new)?;
        let (player_spell_holder_action, player_spell_holder_max_action) =
            if let Some(holder_index) = (player_spell_slot_2 != 0).then_some(player_spell_slot_2) {
                entities
                    .get(usize::from(holder_index))
                    .map_or((0, 0), |holder| {
                        (holder.source_word_2e(), holder.source_word_30())
                    })
            } else {
                (0, 0)
            };
        let mut state = Self {
            free_slots,

            replacement_cursor_exclusions: BTreeSet::new(),

            replacement_list_epoch: 0,

            slot_generations: BTreeMap::new(),

            slot_rng,

            global_rng,

            sentinel_rng,

            source_rng: BTreeMap::new(),

            source_phase_sidecars: BTreeMap::new(),

            source_owner,

            authored_house_record_by_source: BTreeMap::new(),

            owner_class,
            owner_max_mana,

            owner_mana_rows,

            owner_mana_aggregate,

            owner_mana_global,

            spheres: static_spheres,

            death_bursts: Vec::new(),

            player_death_markers: Vec::new(),

            spell_replacements: Vec::new(),

            spell_pickups: Vec::new(),

            speed_trails: Vec::new(),

            terrain_strip_close_triggers: Vec::new(),

            authored_model45: Vec::new(),

            authored_model1: Vec::new(),

            authored_model13: Vec::new(),

            bee_creatures: Vec::new(),

            bee_model2_constructor_phase: 0,

            authored_bee_kill_census: BTreeMap::new(),

            source_creature_population,
            deferred_bee_reclaims: BTreeSet::new(),

            authored_switch_producers: Vec::new(),

            authored_trees: Vec::new(),

            authored_model32: Vec::new(),

            authored_model59: Vec::new(),

            model59_smoke_children: Vec::new(),

            authored_model53: Vec::new(),

            authored_model5_water_splashes: Vec::new(),

            model13_spawned_children: Vec::new(),

            authored_model6: Vec::new(),

            model6_spawned_children: Vec::new(),

            authored_model17: Vec::new(),

            model17_spawned_children: Vec::new(),

            authored_model22: Vec::new(),

            model22_spawned_children: Vec::new(),

            authored_model76: Vec::new(),

            model76_children: Vec::new(),

            model76_tail_spawns: Vec::new(),

            authored_class14: Vec::new(),

            authored_class5: Vec::new(),

            authored_class5_runtime: Vec::new(),

            authored_fireflies: Vec::new(),

            authored_dragons: Vec::new(),

            authored_dragon_parts: Vec::new(),

            authored_class5_model3: Vec::new(),

            authored_class5_model3_children: Vec::new(),

            authored_class5_archer: Vec::new(),

            authored_class5_archer_children: Vec::new(),

            house_residents: Vec::new(),

            class5_model0_constructor_phase: 0,

            class5_model1_constructor_phase: 0,

            class5_model3_constructor_phase: 0,

            class5_model9_constructor_phase: 0,

            class5_model14_constructor_phase: 0,

            class5_model4_constructor_phase: 0,

            class5_model19_constructor_phase: 0,

            castle_auxiliary_constructor_phase: 0,

            class14_model3_handle: 0,

            beyond_sight_effects: Vec::new(),

            mana_attraction_fields: Vec::new(),

            possession_projectiles: Vec::new(),

            possession_guidance_source_slots: Vec::new(),

            possession_guidance_stale_records: BTreeMap::new(),

            possession_effects: Vec::new(),
            fireball_projectiles: Vec::new(),

            carriers: Vec::new(),

            castle_projectiles: Vec::new(),

            castle: None,

            castle_children: Vec::new(),

            castle_footprint: None,

            castle_stage_footprints,

            map_chains,

            player_mana,

            player_castle_entity_index: 0,

            player_castle_stage: 0,

            player_castle_aux_flags: 0,

            player_model,

            player_terrain_flags,

            player_spell_slot_2,

            player_spell_holder_action,

            player_spell_holder_max_action,

            castle_stage_coefficients: [256; 8],

            castle_controller_coefficients: [1; 8],
            events: Vec::new(),
        };
        // The imported owner rows and typed static sphere projection use the
        // same source slot. Rebuild once after construction so the
        // install/replacement semantics remain explicit if a future profile
        // includes a source-linked model-39 row outside the baseline filter.
        state.refresh_owner_mana_rows();
        state.publish_owner_mana_projection();
        Ok(state)
    }

    pub(super) fn for_each_live_replaceable_arena_row(
        &self,
        mut visit: impl FnMut(u16, Level2DynamicArenaKind),
    ) {
        for child in self.castle_children.iter().filter(|child| {
            child.active
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        }) {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::CastleAuxiliary,
            );
        }

        macro_rules! visit_flagged_rows {
            ($rows:expr, $kind:ident) => {
                for row in $rows
                    .iter()
                    .filter(|row| row.active && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0)
                {
                    visit(row.source_entity_index, Level2DynamicArenaKind::$kind);
                }
            };
        }

        // These are the explicitly modeled economy-arena carriers whose
        // stored control dword carries byte[2] & 2. This owner-local scan does
        // not claim authority over Level 2 rows outside this economy arena.
        visit_flagged_rows!(self.spheres, ManaSphere);
        visit_flagged_rows!(self.death_bursts, DeathBurst);
        visit_flagged_rows!(self.authored_model1, AuthoredModel1);
        visit_flagged_rows!(self.model59_smoke_children, Model59SmokeChild);
        visit_flagged_rows!(
            self.authored_model5_water_splashes,
            AuthoredModel5WaterSplash
        );
        visit_flagged_rows!(self.authored_model13, AuthoredModel13);
        visit_flagged_rows!(self.authored_model6, AuthoredModel6);
        visit_flagged_rows!(self.model13_spawned_children, Model13SpawnedChild);
        visit_flagged_rows!(self.model17_spawned_children, Model17SpawnedChild);
        visit_flagged_rows!(self.model76_tail_spawns, Model76TailSpawn);
    }

    pub(super) fn allocate_slot(&mut self) -> Option<u16> {
        if let Some(slot) = self.free_slots.pop() {
            // `NewEvent` overwrites the fixed source record at this slot.
            self.possession_guidance_stale_records.remove(&slot);
            self.source_phase_sidecars.remove(&slot);
            let generation = self.slot_generations.entry(slot).or_default();
            *generation = generation.wrapping_add(1);
            return Some(slot);
        }

        // `NewEvent_4A050` falls through to the lowest live replaceable row
        // when `sub_49F90` has no ordinary free slot. Evict the typed
        // projection directly; `release_slot` would incorrectly add this row
        // to the ordinary free stack and would lose the source relation that
        // `sub_5FF50` cleans for Castle auxiliaries.
        let mut candidate = None;
        self.for_each_live_replaceable_arena_row(|slot, kind| {
            if !self.replacement_cursor_exclusions.contains(&slot)
                && candidate.map_or(true, |(current, _)| slot < current)
            {
                candidate = Some((slot, kind));
            }
        });
        let (slot, kind) = candidate?;
        self.possession_guidance_stale_records.remove(&slot);
        self.source_phase_sidecars.remove(&slot);

        macro_rules! remove_flagged_row {
            ($rows:expr) => {{
                let Some(index) = $rows.iter().position(|row| {
                    row.active
                        && row.source_entity_index == slot
                        && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                }) else {
                    return None;
                };
                $rows.remove(index);
            }};
        }

        match kind {
            Level2DynamicArenaKind::CastleAuxiliary => {
                let Some(index) = self.castle_children.iter().position(|child| {
                    child.active
                        && child.source_entity_index == slot
                        && child.source_class == 5
                        && child.model == 15
                        && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                }) else {
                    return None;
                };
                self.castle_children.remove(index);
            }
            Level2DynamicArenaKind::ManaSphere => remove_flagged_row!(self.spheres),
            Level2DynamicArenaKind::DeathBurst => remove_flagged_row!(self.death_bursts),
            Level2DynamicArenaKind::AuthoredModel1 => remove_flagged_row!(self.authored_model1),
            Level2DynamicArenaKind::Model59SmokeChild => {
                remove_flagged_row!(self.model59_smoke_children);
            }
            Level2DynamicArenaKind::AuthoredModel5WaterSplash => {
                remove_flagged_row!(self.authored_model5_water_splashes);
            }
            Level2DynamicArenaKind::AuthoredModel13 => {
                remove_flagged_row!(self.authored_model13);
            }
            Level2DynamicArenaKind::AuthoredModel6 => remove_flagged_row!(self.authored_model6),
            Level2DynamicArenaKind::Model13SpawnedChild => {
                remove_flagged_row!(self.model13_spawned_children);
            }
            Level2DynamicArenaKind::Model17SpawnedChild => {
                remove_flagged_row!(self.model17_spawned_children);
            }
            Level2DynamicArenaKind::Model76TailSpawn => {
                remove_flagged_row!(self.model76_tail_spawns);
            }
            _ => return None,
        }
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: false,
            x: 0,
            y: 0,
        });
        self.slot_rng.remove(&slot);
        self.source_rng.remove(&slot);
        let generation = self.slot_generations.entry(slot).or_default();
        *generation = generation.wrapping_add(1);
        // The fallback branch zeroes every rebuilt model/class list head.
        // A later callback in this same UpdateEntities walk must therefore
        // observe empty membership even when its individual handles did not
        // change generation.
        self.replacement_list_epoch = self.replacement_list_epoch.wrapping_add(1);
        Some(slot)
    }

    #[must_use]
    pub(super) fn new_event_rng(&self, source_slot: u16) -> u16 {
        // Pinned remc2 NewEvent_4A050: rand_0x14 is the source slot plus the
        // global D41A0.rand_0x8 stream, not the inactive row's stale RNG.
        source_slot.wrapping_add(self.global_rng as u16)
    }

    /// Advance the source-global `D41A0.rand_0x8` stream once for each
    /// `ApplyEvents_498A0` invocation. The source keeps this as a full
    /// 32-bit value; NewEvent consumes its low word only after adding the
    /// allocated source slot. Keeping the full stream is required so the
    /// next source pass does not silently repeat after a high-word carry.
    pub(super) fn advance_global_rng(&mut self) {
        self.global_rng = level2_next_rng32(self.global_rng);
    }

    pub(super) fn release_slot(&mut self, slot: u16, entity_rng: Option<u16>) {
        if slot == 0 {
            return;
        }
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: false,
            x: 0,
            y: 0,
        });
        if self.free_slots.contains(&slot) {
            return;
        }
        if let Some(entity_rng) = entity_rng {
            self.slot_rng.insert(slot, entity_rng);
        }
        // `sub_57F20` appends reclaimed rows to the live LIFO stack. Do not
        // rebuild/sort here: the highest numeric row reclaimed later in the
        // ascending cleanup scan must be the next `NewEvent` allocation even
        // when older reset-free rows remain below it. `rebuild_free_arena`
        // owns the separate source-wide descending reconstruction.
        self.free_slots.push(slot);
    }

    /// Rebuild the source free-arena sidecars using the validated reset pool
    /// and the currently live typed rows.  This is the bounded equivalent of
    /// `sub_49F90`: reclaim rows already marked inactive, then retry the
    /// allocator from descending source-slot order without inventing a new
    /// pool or changing any live relation.
    pub(super) fn rebuild_free_arena(&mut self, reset_free_slots: &[u16]) {
        self.replacement_cursor_exclusions.clear();
        self.reclaim_inactive_slots();
        let occupied = self
            .live_arena_cursor_rows()
            .into_iter()
            .map(|row| row.source_slot)
            // Possession callbacks disable their row in the landing pass but
            // source `UpdateEntities_57730` does not free it until the next
            // pass prologue. It is inactive (and therefore absent from the
            // live cursor) yet must remain unavailable to `NewEvent`.
            .chain(
                self.possession_projectiles
                    .iter()
                    .filter(|projectile| !projectile.active)
                    .map(|projectile| projectile.source_entity_index),
            )
            .chain(
                self.possession_effects
                    .iter()
                    .filter(|effect| !effect.active)
                    .map(|effect| effect.source_entity_index),
            )
            .chain(
                self.fireball_projectiles
                    .iter()
                    .filter(|projectile| !projectile.active && projectile.drawing_disabled)
                    .map(|projectile| projectile.source_entity_index),
            )
            .chain(
                self.death_bursts
                    .iter()
                    .filter(|burst| !burst.active)
                    .map(|burst| burst.source_entity_index),
            )
            .chain(self.authored_house_record_by_source.keys().copied())
            .collect::<BTreeSet<_>>();
        // The source scans current class-zero rows. `free_slots` is the
        // canonical Rust projection of that live set, including slots freed
        // since reset; the reset pool remains a compatibility input for rows
        // that have never been allocated in this bounded carrier.
        self.free_slots = reset_free_slots
            .iter()
            .chain(self.free_slots.iter())
            .copied()
            .filter(|slot| !occupied.contains(slot))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.free_slots
            .sort_unstable_by(|left, right| right.cmp(left));
    }

    /// Apply the unconditional `dword_0x11e6--` at the end of remc2's player
    /// death contact. The skipped slot stays unavailable until sub_49F90
    /// rebuilds the candidate array.
    pub(super) fn decrement_replacement_cursor_after_player_contact(&mut self) {
        let mut candidate = None;
        self.for_each_live_replaceable_arena_row(|slot, _| {
            if !self.replacement_cursor_exclusions.contains(&slot)
                && candidate.is_none_or(|current| slot < current)
            {
                candidate = Some(slot);
            }
        });
        if let Some(slot) = candidate {
            self.replacement_cursor_exclusions.insert(slot);
        }
    }

    #[must_use]
    pub fn spheres(&self) -> &[ManaSphere] {
        &self.spheres
    }

    pub(super) fn spheres_mut(&mut self) -> &mut [ManaSphere] {
        &mut self.spheres
    }

    #[must_use]
    pub fn death_bursts(&self) -> &[Level2DeathBurst] {
        &self.death_bursts
    }

    #[must_use]
    pub fn player_death_markers(&self) -> &[Level2PlayerDeathMarker] {
        &self.player_death_markers
    }

    #[must_use]
    pub fn spell_replacements(&self) -> &[Level2SpellReplacement] {
        &self.spell_replacements
    }

    #[must_use]
    pub fn spell_pickups(&self) -> &[Level2SpellPickupSource] {
        &self.spell_pickups
    }

    #[must_use]
    pub fn speed_trails(&self) -> &[SpeedTrailEffect] {
        &self.speed_trails
    }

    #[must_use]
    pub(crate) fn terrain_strip_close_triggers(
        &self,
    ) -> &[terrain_strip::TerrainStripCloseTrigger] {
        &self.terrain_strip_close_triggers
    }

    #[must_use]
    pub fn authored_model45(&self) -> &[Level2AuthoredModel45Source] {
        &self.authored_model45
    }

    #[must_use]
    pub fn authored_model1(&self) -> &[Level2AuthoredModel1Source] {
        &self.authored_model1
    }

    #[must_use]
    pub fn authored_model13(&self) -> &[Level2AuthoredModel13Source] {
        &self.authored_model13
    }

    #[must_use]
    pub fn bee_creatures(&self) -> &[BeeCreatureRuntime] {
        &self.bee_creatures
    }

    /// Stage the terminal hide of already-proven authored Bee roots for a
    /// downstream objective/gate test. Bee combat and death outputs are not
    /// claimed here; the next production source boundary still owns unlink,
    /// slot release, the frozen model-2 census, and objective service.
    #[cfg(test)]
    pub(super) fn stage_authored_bee_terminal_hide_for_test(
        &mut self,
        authored_indices: &[u16],
    ) -> usize {
        let mut hidden = 0;
        for source in &mut self.bee_creatures {
            if source.active
                && source
                    .authored_index()
                    .is_some_and(|index| authored_indices.contains(&index))
            {
                source.action = 21;
                source.drawing_enabled = false;
                source.active = false;
                source.life = -1;
                source.target_entity_index = None;
                self.deferred_bee_reclaims
                    .insert(source.source_entity_index);
                hidden += 1;
            }
        }
        hidden
    }

    #[must_use]
    pub fn authored_switch_producers(&self) -> &[Level2AuthoredSwitchProducerSource] {
        &self.authored_switch_producers
    }

    #[must_use]
    pub fn authored_trees(&self) -> &[Level2AuthoredTreeSource] {
        &self.authored_trees
    }

    #[must_use]
    pub fn authored_model32(&self) -> &[Level2AuthoredModel32Source] {
        &self.authored_model32
    }

    #[must_use]
    pub fn authored_model59(&self) -> &[Level2AuthoredModel59Source] {
        &self.authored_model59
    }

    #[must_use]
    pub fn model59_smoke_children(&self) -> &[Level2Model59SmokeChild] {
        &self.model59_smoke_children
    }

    #[must_use]
    pub fn authored_model53(&self) -> &[Level2AuthoredModel53Source] {
        &self.authored_model53
    }

    #[must_use]
    pub fn authored_model5_water_splashes(&self) -> &[Level2AuthoredModel5WaterSplashSource] {
        &self.authored_model5_water_splashes
    }

    #[must_use]
    pub fn model13_spawned_children(&self) -> &[Level2Model13SpawnedChild] {
        &self.model13_spawned_children
    }

    #[must_use]
    pub fn authored_model6(&self) -> &[Level2AuthoredModel6Source] {
        &self.authored_model6
    }

    #[must_use]
    pub fn model6_spawned_children(&self) -> &[Level2Model6SpawnedChild] {
        &self.model6_spawned_children
    }

    #[must_use]
    pub fn authored_model17(&self) -> &[Level2AuthoredModel17Source] {
        &self.authored_model17
    }

    #[must_use]
    pub fn model17_spawned_children(&self) -> &[Level2Model17SpawnedChild] {
        &self.model17_spawned_children
    }

    #[must_use]
    pub fn authored_model22(&self) -> &[Level2AuthoredModel22Source] {
        &self.authored_model22
    }

    #[must_use]
    pub fn model22_spawned_children(&self) -> &[Level2Model22SpawnedChild] {
        &self.model22_spawned_children
    }

    #[must_use]
    pub fn authored_model76(&self) -> &[Level2AuthoredModel76Source] {
        &self.authored_model76
    }

    #[must_use]
    pub fn model76_children(&self) -> &[Level2Model76Child] {
        &self.model76_children
    }

    /// Bind the exact `array_0x52_82` collision selectors copied by the
    /// source `SetEntityIndexAndRot_49CD0` constructor. The orbit words are
    /// intentionally not overwritten: they are a separate callback state.
    pub fn bind_model76_collision_axes(
        &mut self,
        parent_entity_index: u16,
        collision_yaw: i16,
        collision_pitch: i16,
        collision_roll: i16,
        collision_fov: i16,
    ) -> bool {
        let mut bound = false;
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.parent_entity_index == parent_entity_index)
        {
            child.collision_yaw = collision_yaw;
            child.collision_pitch = collision_pitch;
            child.collision_roll = collision_roll;
            child.collision_fov = collision_fov;
            bound = true;
        }
        bound
    }

    #[must_use]
    pub fn model76_tail_spawns(&self) -> &[Level2Model76TailSpawn] {
        &self.model76_tail_spawns
    }

    #[must_use]
    pub fn authored_class14(&self) -> &[Level2AuthoredClass14Source] {
        &self.authored_class14
    }

    #[must_use]
    pub fn authored_class5(&self) -> &[Level2AuthoredClass5Source] {
        &self.authored_class5
    }

    /// Stage an already-proven class-5 root as hidden for a downstream
    /// scheduler test. This is test-only prerequisite injection: the next
    /// production prepass still owns unlinking, slot release, and rebuilt
    /// family membership.
    #[cfg(test)]
    pub(super) fn hide_authored_class5_rows_for_test(&mut self, authored_indices: &[u16]) -> usize {
        let mut hidden = 0;
        for source in &mut self.authored_class5 {
            if source.active
                && source.drawing_enabled
                && authored_indices.contains(&source.authored_index)
            {
                source.drawing_enabled = false;
                hidden += 1;
            }
        }
        hidden
    }

    /// Stage the disappearance of one already-proven authored multipart
    /// family for a downstream countdown test. Combat is deliberately not
    /// claimed here; the next production source boundary still owns unlink,
    /// slot release, membership rebuild, and all subsequent callbacks.
    #[cfg(test)]
    pub(super) fn retire_authored_multipart_prerequisite_for_test(
        &mut self,
        authored_index: u16,
    ) -> bool {
        let Some(root) = self.authored_class5.iter_mut().find(|source| {
            source.active && source.authored_index == authored_index && source.model == 3
        }) else {
            return false;
        };
        let root_slot = root.source_entity_index;
        root.active = false;
        root.drawing_enabled = false;
        for child in &mut self.authored_class5_model3_children {
            if child.active && child.parent_entity_index == root_slot {
                child.active = false;
                child.drawing_enabled = false;
            }
        }
        true
    }

    /// Stage one source-authored Worm body mailbox for a focused scheduler
    /// lifecycle test. The ordinary action-232 callback still owns following,
    /// damage consumption, attacker propagation, and its common epilogue.
    #[cfg(test)]
    pub(super) fn stage_authored_worm_child_mailbox_for_test(
        &mut self,
        authored_index: u16,
        child_ordinal: u8,
        attacker_slot: u16,
        damage: i32,
    ) -> Option<(u16, u16)> {
        let root = self.authored_class5.iter_mut().find(|source| {
            source.active && source.authored_index == authored_index && source.model == 3
        })?;
        root.action = 25;
        root.pending_word_0x62 = 0;
        root.word_0x26 = 0;
        root.source_byte_3e = 1;
        root.phase = 1;
        let root_slot = root.source_entity_index;
        let mut child_slot = self
            .authored_class5_model3
            .iter()
            .find(|state| state.source_entity_index == root_slot)?
            .word_0x34;
        for _ in 0..child_ordinal {
            child_slot = self
                .authored_class5_model3_children
                .iter()
                .find(|child| child.active && child.source_entity_index == child_slot)?
                .word_0x34;
        }
        let child = self
            .authored_class5_model3_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == child_slot)?;
        child.action = 232;
        child.pending_word_0x62 = attacker_slot;
        child.damage_dword_0x5e_94 = damage;
        child.word_0x26 = 0;
        Some((root_slot, child.source_entity_index))
    }

    /// Stage the already-constructed Worm root on its source action-26
    /// cadence beside a real live target. The production callback still owns
    /// guidance, allocation, feedback, sound, and the common epilogue.
    #[cfg(test)]
    pub(super) fn stage_authored_worm_attack_for_test(
        &mut self,
        authored_index: u16,
        target: EntitySnapshot,
    ) -> Option<u16> {
        if !target.active || target.life < 0 {
            return None;
        }
        let root = self.authored_class5.iter_mut().find(|source| {
            source.active && source.authored_index == authored_index && source.model == 3
        })?;
        root.action = 26;
        root.word_0x96_150 = target.index;
        root.word_0x26 = target.index;
        root.source_byte_3e = 0;
        root.phase = 0;
        root.position = target.position;
        root.position.x = root.position.x.wrapping_sub(0x0100);
        let root_slot = root.source_entity_index;
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: root_slot,
            active: true,
            x: root.position.x,
            y: root.position.y,
        });
        Some(root_slot)
    }

    /// Remove every currently modeled `NewEvent` allocation candidate for a
    /// production failure-path test. The group callback still owns authored
    /// row consumption, producer hiding, and following-pass cleanup.
    #[cfg(test)]
    pub(super) fn exhaust_new_event_capacity_for_test(&mut self) -> Option<u16> {
        let removed_free_slot = self.free_slots.last().copied();
        self.free_slots.clear();
        for child in &mut self.castle_children {
            child.control_flags &= !SOURCE_REPLACEABLE_FLAG;
        }
        macro_rules! make_nonreplaceable {
            ($rows:expr) => {
                for row in &mut $rows {
                    row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
                }
            };
        }
        make_nonreplaceable!(self.spheres);
        make_nonreplaceable!(self.death_bursts);
        make_nonreplaceable!(self.authored_model1);
        make_nonreplaceable!(self.model59_smoke_children);
        make_nonreplaceable!(self.authored_model5_water_splashes);
        make_nonreplaceable!(self.authored_model13);
        make_nonreplaceable!(self.authored_model6);
        make_nonreplaceable!(self.model13_spawned_children);
        make_nonreplaceable!(self.model17_spawned_children);
        make_nonreplaceable!(self.model76_tail_spawns);
        removed_free_slot
    }

    #[cfg(test)]
    pub(super) fn restore_new_event_slot_for_test(&mut self, source_slot: u16) -> bool {
        if source_slot == 0
            || self
                .live_arena_cursor_rows()
                .iter()
                .any(|row| row.source_slot == source_slot)
        {
            return false;
        }
        if !self.free_slots.contains(&source_slot) {
            self.free_slots.push(source_slot);
        }
        true
    }

    #[must_use]
    pub fn authored_class5_runtime(&self) -> &[Level2AuthoredClass5RuntimeState] {
        &self.authored_class5_runtime
    }

    #[must_use]
    pub fn authored_fireflies(&self) -> &[AuthoredFireflyRuntime] {
        &self.authored_fireflies
    }

    #[must_use]
    pub fn authored_dragons(&self) -> &[AuthoredDragonRuntime] {
        &self.authored_dragons
    }

    #[must_use]
    pub fn authored_dragon_parts(&self) -> &[AuthoredDragonPart] {
        &self.authored_dragon_parts
    }

    #[must_use]
    pub fn authored_class5_model3(&self) -> &[Level2AuthoredClass5Model3State] {
        &self.authored_class5_model3
    }

    #[must_use]
    pub fn authored_class5_model3_children(&self) -> &[Level2AuthoredClass5Model3Child] {
        &self.authored_class5_model3_children
    }

    #[must_use]
    pub fn authored_class5_archer(&self) -> &[Level2AuthoredClass5ArcherState] {
        &self.authored_class5_archer
    }

    #[must_use]
    pub fn authored_class5_archer_children(&self) -> &[Level2AuthoredClass5ArcherChild] {
        &self.authored_class5_archer_children
    }

    pub(super) fn authored_class5_runtime_mut(
        &mut self,
        source_entity_index: u16,
    ) -> Option<&mut Level2AuthoredClass5RuntimeState> {
        self.authored_class5_runtime
            .iter_mut()
            .find(|state| state.source_entity_index == source_entity_index)
    }

    #[must_use]
    pub const fn class14_model3_handle(&self) -> u16 {
        self.class14_model3_handle
    }

    #[must_use]
    pub fn beyond_sight_effects(&self) -> &[Level2BeyondSightPulseEffect] {
        &self.beyond_sight_effects
    }

    #[must_use]
    pub fn mana_attraction_fields(&self) -> &[Level2ManaAttractionField] {
        &self.mana_attraction_fields
    }

    /// Allocate the source class-10/model-54 temporary field from an authored
    /// feature. `sub_4A310` binds `dword_0x10` to `(stageTag << 8)^2` and
    /// `maxLife` to `max(8 * stageTag + 16, 128)` before `sub_38D80` runs.
    /// Action 76 is the pinned duplicate dispatch row and is accepted without
    /// changing the source constructor payload.
    pub fn allocate_mana_attraction_field(
        &mut self,
        authored_index: u16,
        position: Position,
        stage_tag: i16,
        action: u8,
    ) -> Option<u16> {
        debug_assert_eq!((MANA_ATTRACTION_CLASS, MANA_ATTRACTION_MODEL), (10, 54));
        if !matches!(
            action,
            MANA_ATTRACTION_ACTION | MANA_ATTRACTION_ALIAS_ACTION
        ) || stage_tag < 0
        {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let stage = u32::try_from(stage_tag).ok()?;
        let radius = stage.checked_mul(256)?;
        let max_life = i16::try_from((8 * stage + 16).max(128)).ok()?;
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.mana_attraction_fields.push(Level2ManaAttractionField {
            source_entity_index,
            entity_rng,
            authored_index,
            position,
            radius_squared: radius.saturating_mul(radius),
            max_life,
            life: max_life,
            action,
            active: true,
        });
        self.source_phase_sidecars
            .insert(source_entity_index, source_entity_index as u8);
        self.source_rng.insert(source_entity_index, entity_rng);
        Some(source_entity_index)
    }

    /// Allocate an authored class-15 source row through the shared
    /// `NewEvent_4A050` arena.  The caller supplies the already decoded
    /// `sub_4A310` family/action and terrain-centred position.
    pub fn allocate_spell_pickup(
        &mut self,
        authored_index: u16,
        family: u8,
        action: u8,
        position: Position,
    ) -> Option<Level2SpellPickupSource> {
        if physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup) != Some(action) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2SpellPickupSource {
            source_entity_index,
            entity_rng,
            authored_index,
            family,
            action,
            phase: source_entity_index as u8,
            position,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spell_pickups.push(source);
        Some(source)
    }

    /// Allocate `NewAdd0A02_4E430` and apply `GetScroll_69DB0`'s immediate
    /// life/owner writes. The constructor does not insert this effect into the
    /// renderable map chain.
    pub(super) fn allocate_speed_trail(
        &mut self,
        position: Position,
        source_id: u16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        self.speed_trails.push(SpeedTrailEffect {
            source_entity_index,
            entity_rng,
            source_id,
            class: 10,
            model: 2,
            action: 2,
            position,
            phase: source_entity_index as u8,
            max_life: 8,
            life: 32,
            control_flags: 0x0002_0001,
            active: true,
        });
        self.source_rng.insert(source_entity_index, entity_rng);
        Some(source_entity_index)
    }

    /// Dispatch `CastSpeedSpell_31100`. The disabled row remains reserved for
    /// the rest of this pass and is reclaimed by the next source boundary.
    pub(super) fn step_speed_trail(&mut self, source_entity_index: u16) -> bool {
        let Some(trail) = self
            .speed_trails
            .iter_mut()
            .find(|trail| trail.active && trail.source_entity_index == source_entity_index)
        else {
            return false;
        };
        step_speed_trail_effect(trail);
        trail.phase = trail.phase.wrapping_add(1);
        trail.active
    }

    /// Allocate the authored close command through remc2 `sub_4F900`.
    pub(crate) fn allocate_terrain_strip_close_trigger(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<terrain_strip::TerrainStripCloseTrigger> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = terrain_strip::TerrainStripCloseTrigger {
            source_entity_index,
            entity_rng,
            authored_index,
            class: 10,
            model: 63,
            action: 68,
            max_life: 1,
            life: 1,
            control_flags: 1,
            source_byte_3e: source_entity_index as u8,
            position,
            drawing_enabled: true,
            active: true,
        };
        self.source_owner.insert(source_entity_index, 0);
        self.source_rng.insert(source_entity_index, entity_rng);
        self.terrain_strip_close_triggers.push(source);
        Some(source)
    }

    /// Retain the command as a drawing-disabled tombstone until the next
    /// source-boundary reclaim, matching `sub_34390` plus the common epilogue.
    pub(crate) fn complete_terrain_strip_close_trigger(
        &mut self,
        source_entity_index: u16,
    ) -> bool {
        let Some(source) = self
            .terrain_strip_close_triggers
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_entity_index)
        else {
            return false;
        };
        source.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        source.drawing_enabled = false;
        source.active = false;
        true
    }

    /// Allocate the class-15 holder made by `AddSpellXX_XX_51120` during the
    /// isolated `sub_5CF40` pass. These rows use the source action `3*family`
    /// rather than the ordinary pickup action and have no authored pickup id.
    pub(super) fn allocate_starting_spell_holder(
        &mut self,
        family: u8,
        position: Position,
    ) -> Option<Level2StartingSpellHolderSource> {
        if family > 3 || self.free_slot_count() == 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2StartingSpellHolderSource {
            source_entity_index,
            entity_rng,
            family,
            action: family.saturating_mul(3),
            phase: source_entity_index as u8,
            position,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        Some(source)
    }

    /// Allocate the authored class-10/model-1/action-1 effect from the
    /// pinned `NewAdd0A01_4E3B0` constructor. The immediate `AddEvent2`
    /// presentation child is intentionally deferred; the root's source
    /// arena identity and Quickfair callback are source-backed here.
    pub fn allocate_authored_model1(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel1Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel1Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model: 1,
            action: 1,
            subspell_index: 400,
            max_life: 1,
            life: 1,
            // NewAdd0A01 clears bits 3/17, sets byte 2 bit 1, then
            // AddEventToMap installs the low-byte map-linked bit.
            control_flags: 0x0002_0004,
            particle_index: 41,
            last_sound_id: 0,
            position,
            // NewEvent seeds byte 0x3E from the allocated source slot; the
            // Quickfair constructor does not overwrite that cursor.
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model1.push(source);
        Some(source)
    }

    /// Allocate the class-10/model-45/action-51 producer row from authored
    /// group 8.  The qualified rows use BLDGPRM/BUILD entry 21; keeping that
    /// source-data binding explicit prevents a generic model-45 shim from
    /// silently accepting a different authored parameter.
    pub fn allocate_authored_model45(
        &mut self,
        authored_index: u16,
        target_group: u16,
        parameter_1: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel45Source> {
        if parameter_1 != 21 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        // BUILD0-0.TAB[21] is raw 15x15 in the qualified owner package.
        // CreateIndexes_6EB90 doubles it to runtime 30x30 under VGA selector
        // 1, then sub_49A30 halves the runtime dimensions back to an
        // effective 15x15 view/terrain footprint.
        let raw_width = 15_u16;
        let raw_height = 15_u16;
        let runtime_width = raw_width * 2;
        let runtime_height = raw_height * 2;
        let width = runtime_width >> 1;
        let height = runtime_height >> 1;
        let mut source_position = position;
        // sub_49A30 first copies the position snapped to 256-unit tile
        // boundaries, then applies its parity adjustment below.
        source_position.x &= 0xff00;
        source_position.y &= 0xff00;
        let tile_x = source_position.x >> 8;
        let tile_y = source_position.y >> 8;
        let mut corner_x = tile_x.wrapping_sub(width / 2);
        let corner_y = tile_y.wrapping_sub(height / 2);
        if (u16::from(corner_x) + u16::from(corner_y)) % 2 != 0 {
            source_position.x = source_position.x.wrapping_add(256);
            corner_x = corner_x.wrapping_add(1);
        }
        let corner_height = |x: u16, y: u16| {
            i16::from(
                terrain.planes.ground_heights
                    [terrain_index(x.to_le_bytes()[0], y.to_le_bytes()[0])],
            )
        };
        let average_height = (corner_height(corner_x, corner_y)
            + corner_height(corner_x.wrapping_add(width), corner_y)
            + corner_height(corner_x.wrapping_add(width), corner_y.wrapping_add(height))
            + corner_height(corner_x, corner_y.wrapping_add(height)))
            >> 2;
        source_position.z = average_height.saturating_mul(32);
        let minimum_speed = i16::try_from((runtime_width * runtime_height) >> 4)
            .expect("qualified BUILD dimensions fit minimum speed");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: source_position.x,
                y: source_position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let subspell_index = 6_u16;
        // NewEvent zeroes mana.  BLDGPRM[21].byte_2 has bit 3 set, so
        // sub_49A30 skips the branch that derives mana from subSpellIndex.
        let mana = 0;
        let source = Level2AuthoredModel45Source {
            source_entity_index,
            entity_rng,
            authored_index,
            target_group,
            parameter_1,
            source_class: 10,
            model: 45,
            action: 51,
            max_life: 30,
            life: 30,
            actual_speed: 16,
            minimum_speed,
            subspell_index,
            dword_0x10: 2,
            yaw: 0,
            font_type_index: 54,
            // AddTerrainModification starts byte 0x38 at 33.  BLDGPRM[21]
            // has bit 3 set, so sub_49A30 skips the branch that would OR 2.
            source_byte_38: 33,
            // The control byte initialized to 9 is not modified by
            // sub_49A30; it is distinct from source byte 0x38.
            control_flags: 9,
            mana,
            max_mana: 0,
            particle_index: 177,
            particle_byte_0x5d: 1,
            view_pitch: 2560,
            view_yaw: 0,
            view_fov: 256,
            view_roll: 2560,
            axis_x: 0,
            axis_z: 0,
            byte_0x46: 21,
            parent_entity_index: 0,
            source_byte_3e: source_entity_index as u8,
            position: source_position,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model45.push(source);
        Some(source)
    }

    /// Allocate an authored Day Bee from the class-5/model-2 constructor
    /// `sub_4B590`.  The caller has already proved the authored kind/subtype
    /// and producer group; this method owns the real source arena slot, RNG,
    /// map link, and constructor defaults.
    pub(super) fn allocate_authored_bee(
        &mut self,
        authored_index: u16,
        position: Position,
        stage_route: Option<Level2BeeStageRoute>,
        max_life: i16,
        terrain: &SessionTerrain,
    ) -> Option<BeeCreatureRuntime> {
        let (action, stage_var1, stage_var2, stage_target) =
            stage_route.map_or((17, 0, 0, None), |route| {
                (
                    8 * 2 + 7,
                    route.stage_var1,
                    route.stage_var2,
                    Some(route.stage_target),
                )
            });
        let mut position = position;
        position.z = terrain.ground_height(position);
        self.allocate_bee_creature(
            BeeCreatureOrigin::AuthoredRecord { authored_index },
            position,
            0,
            0,
            action,
            stage_var1,
            stage_var2,
            stage_target,
            max_life,
            terrain,
        )
    }

    /// Allocate the Day Bee body created by one successful Morph pulse.
    /// Unlike an authored group row, `sub_6A030` spawns at the caster's exact
    /// 3-D position and installs the caster relationship after `sub_4B590`.
    pub(super) fn allocate_morph_bee(
        &mut self,
        holder_slot: u16,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<BeeCreatureRuntime> {
        self.allocate_bee_creature(
            BeeCreatureOrigin::MorphSpell { holder_slot },
            position,
            parent_entity_index,
            parent_entity_id,
            23,
            0,
            12,
            None,
            level2_class5::BEE_MAX_LIFE,
            terrain,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn allocate_bee_creature(
        &mut self,
        origin: BeeCreatureOrigin,
        position: Position,
        parent_entity_index: u16,
        parent_entity_id: u16,
        action: u8,
        stage_var1: u8,
        stage_var2: u8,
        stage_target: Option<Position>,
        max_life: i16,
        terrain: &SessionTerrain,
    ) -> Option<BeeCreatureRuntime> {
        if terrain.map_type != MapType::Day || max_life <= 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        // Pinned `sub_4B590`: NewEvent supplies the initial rand word, the
        // constructor advances it once, and the same low 11 bits seed yaw,
        // roll, and pitch.  The descriptor is `str_D7BD6[73]`.
        let entity_rng = level2_next_rng16(initial_rng);
        let angle = (entity_rng & 0x07ff) as i16 - 1;
        let source_byte_3e = self.bee_model2_constructor_phase;
        self.bee_model2_constructor_phase = source_byte_3e.wrapping_add(1);
        let action_window = 30;
        let source_byte_39 = action_window
            - i16::from(source_byte_3e % u8::try_from(action_window).expect("positive window"))
            + 4;
        let source = BeeCreatureRuntime {
            source_entity_index,
            entity_id: if parent_entity_id == 0 {
                source_entity_index
            } else {
                parent_entity_id
            },
            entity_rng,
            origin,
            parent_entity_index,
            source_class: 5,
            model: 2,
            action,
            minimum_speed: 64,
            maximum_speed: 30,
            actual_speed: 32,
            // NewEvent_4A050 initializes the control dword to 8. The
            // movement helper only consumes the later byte-1 bit 8 (0x800),
            // so the constructor's low-byte marker remains intact here.
            control_flags: 8 | u32::from(parent_entity_index != 0),
            mana: i32::from(max_life) / 2,
            max_life,
            life: max_life,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            damage_attacker_entity_index: 0,
            kill_credit_entity_index: 0,
            subspell_index: 200,
            dword_0x10: i32::from(source_entity_index % 100),
            target_entity_index: None,
            yaw: angle,
            pitch: angle,
            roll: angle,
            source_byte_3e,
            source_byte_39,
            vertical_step: -32,
            action_window,
            stage_var1,
            stage_var2,
            stage_target,
            last_sound_id: 0,
            position,
            particle_index: 3,
            shift_rot_x: 128,
            shift_rot_y: 128,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.bee_creatures.push(source);
        Some(source)
    }

    /// Bind the source `word_0x96` target mailbox for the bounded Bee action
    /// tests and the future objective/AI owner.  The target is intentionally
    /// explicit; no player or objective is inferred here.
    pub fn set_authored_bee_target(&mut self, source_slot: u16, target: Option<u16>) -> bool {
        let Some(source) = self
            .bee_creatures
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.target_entity_index = target;
        true
    }

    /// Run Morph's action-23 selector-12 follower at the Bee's ordinary
    /// numeric source visit. This callback owns no RNG; it follows the live
    /// parent exactly and leaves the common Bee phase advance in this one
    /// authoritative carrier.
    pub(super) fn step_morph_bee_follower(
        &mut self,
        source_slot: u16,
        parent: Option<BeeFollowerParent>,
    ) -> bool {
        let Some(source_index) = self.bee_creatures.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && matches!(source.origin, BeeCreatureOrigin::MorphSpell { .. })
                && source.action == 23
                && source.stage_var2 == 12
        }) else {
            return false;
        };
        let mut source = self.bee_creatures[source_index];
        let parent = parent.filter(|parent| {
            parent.active && parent.life >= 0 && parent.entity_index == source.parent_entity_index
        });
        if let Some(parent) = parent {
            source.position = parent.position;
            source.yaw = parent.yaw;
            source.roll = parent.roll;
        } else {
            source.drawing_enabled = false;
            self.deferred_bee_reclaims.insert(source_slot);
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.bee_creatures[source_index] = source;
        if source.drawing_enabled {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: source.position.x,
                    y: source.position.y,
                })
                .is_ok()
        } else {
            // DisableEntityDrawing retains the map link through the rest of
            // this pass. The following source boundary owns unlink/release.
            true
        }
    }

    /// Apply Morph holder expiry to its related Bee. The disabled row stays
    /// allocated until the following source-boundary reclaim.
    pub(super) fn disable_morph_bee(&mut self, source_slot: u16) -> bool {
        let Some(source) = self.bee_creatures.iter_mut().find(|source| {
            source.active
                && source.source_entity_index == source_slot
                && matches!(source.origin, BeeCreatureOrigin::MorphSpell { .. })
        }) else {
            return false;
        };
        source.drawing_enabled = false;
        self.deferred_bee_reclaims.insert(source_slot);
        true
    }

    #[cfg(test)]
    pub(super) fn replace_authored_bee_for_test(&mut self, updated: BeeCreatureRuntime) -> bool {
        let Some(source) = self
            .bee_creatures
            .iter_mut()
            .find(|source| source.source_entity_index == updated.source_entity_index)
        else {
            return false;
        };
        *source = updated;
        true
    }

    /// Run the source `sub_1BF90` class-3 target scan for an action-17 Bee.
    ///
    /// The caller supplies the live `dword_38519` class-3 list in source
    /// order. This method owns the action-17 `sub_1B8C0` movement prelude,
    /// model-2 descriptor cadence, and two wander RNG draws; the normal
    /// action-17 callback remains responsible for the follow-up sound draw
    /// and action transition.
    pub fn acquire_authored_bee_target(
        &mut self,
        source_slot: u16,
        candidates: &[(u16, Position, u32)],
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        let Some(source_index) = self
            .bee_creatures
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return None;
        };
        let mut source = self.bee_creatures[source_index];
        if source.action != 17 {
            return None;
        }

        // `sub_1BF90(a1, 16)` always runs `sub_1B8C0` first. That movement
        // callback is intentionally before the cadence guard: even a
        // non-cadence action-17 turn changes position/yaw and only then
        // decides whether to consume the two wander draws and scan targets.
        move_level2_bee_source(&mut source, terrain);
        if source.action_window <= 0 {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.bee_creatures[source_index] = source;
            return None;
        }
        if source.source_byte_3e
            % u8::try_from(source.action_window).expect("positive Bee action window")
            != 0
        {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.bee_creatures[source_index] = source;
            return None;
        }

        // `sub_1BF90` wanders before scanning `dword_38519`: both LCG draws
        // are observable even when no target survives the source filters.
        let first = level2_next_rng16(source.entity_rng);
        let second = level2_next_rng16(first);
        let direction = 2 * (u32::from(first % 0x9d) / 79);
        let roll_delta = i32::from((second & 0xff) as u8) + 85;
        let roll_sign = i32::try_from(direction).expect("Bee roll direction fits") - 1;
        source.roll = (i32::from(source.roll) + roll_delta * roll_sign) as i16 & 0x07ff;
        source.entity_rng = second;

        // The source performs the two wander draws before consulting
        // byte-0x39; a zero window suppresses only the class-3 scan.
        if source.source_byte_39 == 0 {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.bee_creatures[source_index] = source;
            return None;
        }

        let selected = select_level2_bee_class3_target(&source, candidates);
        // `sub_1BF90` writes word_0x96 only when a new class-3 target wins.
        // On a gated or empty scan the stale relation remains in the source
        // row; action 17 itself does not consume that relation as a target.
        if let Some(target_index) = selected {
            source.target_entity_index = Some(target_index);
        }
        self.source_rng.insert(source_slot, source.entity_rng);
        self.bee_creatures[source_index] = source;
        selected
    }

    /// Allocate one authored class-11 model-0/model-1/model-12/model-16/model-17 switch producer using
    /// the shared `NewEvent_4A050` source arena.  `stage_tag` is the source
    /// entity id consumed by the next `sub_4A1E0` group scan.
    pub fn allocate_authored_switch_producer(
        &mut self,
        authored_index: u16,
        model: u8,
        stage_tag: u16,
        word_10: i16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredSwitchProducerSource> {
        if !matches!(model, 0 | 1 | 12 | 16 | 17) || stage_tag == 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredSwitchProducerSource {
            source_entity_index,
            entity_id: stage_tag,
            entity_rng,
            authored_index,
            source_class: 11,
            model,
            action: model,
            stage_tag,
            word_10,
            dword_0x10: 0,
            position,
            // NewEvent_4A050 initializes byte_0x3E from the allocated source
            // entity slot. AddSwitchXX_50A90 and sub_4A310 preserve it for
            // class-11 producers, so the typed phase carries that runtime
            // seed rather than an authored-row or zero-based substitute.
            phase: source_entity_index as u8,
            cooldown: 0,
            fired: false,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_switch_producers.push(source);
        Some(source)
    }

    /// Allocate one authored class-2/model-0/action-0 tree using the exact
    /// `AddTree_4AC40` constructor sequence.  The authored position is
    /// already terrain-grounded by `sub_4A310`; only the constructor's two
    /// signed 0..63-minus-32 offsets alter x/y before the map link is made.
    pub fn allocate_authored_tree(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredTreeSource> {
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let life_rng = level2_next_rng16(initial_rng);
        let x_rng = level2_next_rng16(life_rng);
        let y_rng = level2_next_rng16(x_rng);
        let particle_rng = level2_next_rng16(y_rng);
        let x_offset = i16::try_from(x_rng & 0x3f)
            .expect("tree x offset draw fits i16")
            .wrapping_sub(32);
        let y_offset = i16::try_from(y_rng & 0x3f)
            .expect("tree y offset draw fits i16")
            .wrapping_sub(32);
        let particle_index = if particle_rng & 1 != 0 { 84 } else { 83 };
        let position = Position {
            x: position.x.wrapping_add(x_offset as u16),
            y: position.y.wrapping_add(y_offset as u16),
            z: position.z,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredTreeSource {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng: particle_rng,
            authored_index,
            source_class: 2,
            model: 0,
            action: 0,
            dword_0x10: i32::from(source_entity_index % 11),
            source_byte_38: 1,
            // AddTree writes the random 2,500..7,499 life first, then
            // CopyMaxLifeToLife restores NewEvent's default max/life of 300.
            max_life: 300,
            life: 300,
            // NewEvent starts at 8 and AddEventToMap sets the map bit 4.
            // The callback's byte-2 bit is represented by the same source
            // dword and is added only when the row is dispatched.
            control_flags: 0x0000_000c,
            owner_entity_index: 0,
            pending_damage: 0,
            particle_index,
            // Particle rows 83 and 84 both have rotSpeed 800 and speed 0;
            // SetHalfSpeedEntity therefore writes yaw/fov 400 and pitch/roll
            // zero after the fourth constructor draw.
            yaw: 400,
            pitch: 0,
            roll: 0,
            fov: 400,
            position,
            phase: source_entity_index as u8,
            last_sphere_source_entity_index: 0,
            last_sphere_word_0x2c: 0,
            last_sphere_position_z: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, particle_rng);
        self.authored_trees.push(source);
        Some(source)
    }

    /// Allocate an authored class-11/model-32 objective switch through the
    /// pinned `AddSwitch32_50B80` constructor and `sub_4A310` field writes.
    pub fn allocate_authored_model32(
        &mut self,
        authored_index: u16,
        stage_tag: u16,
        objective_stage_index: u8,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel32Source> {
        if stage_tag == 0 || objective_stage_index >= 8 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel32Source {
            source_entity_index,
            entity_id: stage_tag,
            entity_rng,
            authored_index,
            source_class: 11,
            model: 32,
            action: 32,
            stage_tag,
            objective_stage_index,
            // The model-32 branch skips SetEntityShiftRot, so NewEvent's
            // zeroed word_0x10 remains untouched by sub_4A310.
            word_10: 0,
            // NewEvent_4A050 defaults maxLife/life to 300. AddSwitch32 does
            // not alter those words; sub_4A310 copies maxLife to life.
            max_life: 300,
            life: 300,
            // AddSwitchXX clears NewEvent's byte-zero 8 and sets bit 0,
            // yielding the exact little-endian control dword 1.
            control_flags: 1,
            source_byte_3e: source_entity_index as u8,
            position,
            fired: false,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model32.push(source);
        Some(source)
    }

    /// Consume one model-32 source's objective callback. The caller owns the
    /// objective predicate and downstream group scan; this method only
    /// applies the source's one-shot drawing/fired transition.
    pub(super) fn complete_authored_model32(
        &mut self,
        source_slot: u16,
    ) -> Option<Level2AuthoredModel32Source> {
        let source = self.authored_model32.iter_mut().find(|source| {
            source.active && source.source_entity_index == source_slot && !source.fired
        })?;
        source.fired = true;
        source.drawing_enabled = false;
        source.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        source.active = false;
        Some(*source)
    }

    /// Allocate the authored class-10/model-59 checkpoint row from the
    /// generic `sub_4A310` path. The pinned `ArriveCheckpoint_4EB50`
    /// constructor performs two entity-local RNG draws: max-life in
    /// `800..899`, then actual speed in `0..16`; it writes action 64/model 59
    /// and preserves the direct authored position.
    pub fn allocate_authored_model59(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel59Source> {
        // `ArriveCheckpoint_4EB50` checks the shared NewEvent free count
        // before it consumes either constructor RNG draw.
        if self.free_slots.len() < 32 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let life_rng = level2_next_rng16(initial_rng);
        let max_life = i16::try_from(life_rng % 0x64).expect("checkpoint life fits i16") + 800;
        let speed_rng = level2_next_rng16(life_rng);
        let actual_speed = i16::try_from(speed_rng % 0x11).expect("checkpoint speed fits i16");
        let source = Level2AuthoredModel59Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng: speed_rng,
            authored_index,
            source_class: 10,
            model: 59,
            action: 64,
            max_life,
            life: max_life,
            actual_speed,
            // NewEvent starts with dword 8; ArriveCheckpoint applies
            // `(byte0 & 0xf6) | 1`, yielding the little-endian control dword 1.
            control_flags: 1,
            source_byte_3e: source_entity_index as u8,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, speed_rng);
        self.authored_model59.push(source);
        Some(source)
    }

    fn allocate_model13_smoke_child_after_global_draw(
        &mut self,
        position: Position,
        max_life: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let entity_rng = level2_next_rng16(initial_rng);
        let actual_speed =
            i16::try_from(entity_rng % 0x35 + 51).expect("model-59 smoke child speed fits");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model59_smoke_children.push(Level2Model59SmokeChild {
            source_entity_index,
            entity_rng,
            source_class: 10,
            model: 13,
            action: 13,
            max_life,
            life: max_life,
            max_speed: 30,
            actual_speed,
            dword_0x10: 0,
            particle_index: 67,
            yaw: 0,
            control_flags: 0x0002_0000,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        });
        Some(source_entity_index)
    }

    pub(super) fn allocate_model59_smoke_child(&mut self, position: Position) -> Option<u16> {
        // `SetParticleSmoke3B_4E9E0` advances the process-global stream before
        // `SetSmoke4_4EAA0` attempts NewEvent. The draw survives failure.
        self.global_rng = level2_next_rng32(self.global_rng);
        self.allocate_model13_smoke_child_after_global_draw(position, 32)
    }

    /// Construct the class-2 ballistic death smoke through the shared
    /// `SetParticleSmoke3B -> SetSmoke4` path. The global draw occurs before
    /// allocation and therefore remains observable when the arena is full.
    pub(super) fn allocate_ballistic_death_smoke(&mut self, position: Position) -> Option<u16> {
        self.global_rng = level2_next_rng32(self.global_rng);
        let max_life = i16::try_from(self.global_rng % 0x17 + 17)
            .expect("ballistic death-smoke life fits i16");
        self.allocate_model13_smoke_child_after_global_draw(position, max_life)
    }

    /// Advance the authored action-64 checkpoint callback. The parent emits
    /// one model-13/action-13 smoke child per successful callback allocation;
    /// the child then follows the source `sub_32160` motion/expiry path.
    pub fn step_authored_model59_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model59
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let life_before = self.authored_model59[source_index].life;
        let mut source = self.authored_model59[source_index];
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
            source.active = true;
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            source.phase = source.phase.wrapping_add(1);
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_model59[source_index] = source;
            return true;
        }

        let mut smoke_position = source.position;
        source.entity_rng = level2_next_rng16(source.entity_rng);
        smoke_position.x = smoke_position.x.wrapping_add(source.entity_rng % 0xA0);
        source.entity_rng = level2_next_rng16(source.entity_rng);
        smoke_position.z = smoke_position
            .z
            .wrapping_add(i16::try_from(source.entity_rng % 0xA0).expect("smoke z offset"));
        if let Some(child_slot) = self.allocate_model59_smoke_child(smoke_position) {
            // `AddParticleSmoke0A_3D` consumes the parent's third draw only
            // after SetSmoke4 returns a live child.
            source.entity_rng = level2_next_rng16(source.entity_rng);
            if let Some(child) = self
                .model59_smoke_children
                .iter_mut()
                .find(|child| child.source_entity_index == child_slot)
            {
                child.actual_speed = child
                    .actual_speed
                    .wrapping_add(source.actual_speed)
                    .wrapping_add(
                        i16::try_from(source.entity_rng % 0x4D)
                            .expect("model-59 smoke parent speed draw fits"),
                    );
            }
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model59[source_index] = source;
        true
    }

    /// Advance one model-59 smoke child through the pinned `sub_32160`
    /// callback. Its particle ramp differs from the rooted model-13 family:
    /// it rises toward selector 74 and falls back toward selector 67.
    pub fn step_model59_smoke_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self
            .model59_smoke_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model59_smoke_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.drawing_enabled = false;
            child.active = true;
            child.phase = child.phase.wrapping_add(1);
            self.model59_smoke_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-59 smoke child fits the map chain");
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        let mut predicted = child.position;
        predicted.z = predicted
            .z
            .wrapping_add(child.actual_speed)
            .max(terrain.ground_height(predicted));
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut predicted, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 74 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 67 {
            child.particle_index -= 1;
        }
        child.position = predicted;
        child.phase = child.phase.wrapping_add(1);
        self.model59_smoke_children[child_index] = child;
        self.source_rng.insert(source_slot, child.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Allocate the authored class-10/model-53/action-58 row from remc2
    /// `sub_504B0`.
    /// The constructor intentionally does not add the row to the render map;
    /// it writes its authored position directly, sets action 58/model 53,
    /// life 6, subspell 3000, and clears NewEvent's default control bit 0x08.
    /// The remaining NewEvent defaults are retained because `sub_38B90`
    /// consumes the source RNG immediately on its live callback path.
    pub fn allocate_authored_model53(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel53Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let source = Level2AuthoredModel53Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 53,
            action: 58,
            max_life: 6,
            life: 6,
            subspell_index: 3000,
            actual_speed: 16,
            dword_0x10: 0,
            control_flags: 0,
            shift_rot_x: 0,
            shift_rot_y: 0,
            source_byte_39: -6,
            byte_0x43: 10,
            byte_0x44: 0,
            byte_0x46: 0,
            word_0x2c: 0,
            sprite_index: 0,
            last_sound_id: 0,
            water_spawn_requested: false,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model53.push(source);
        Some(source)
    }

    /// Run remc2 class-10/action-58 `sub_38B90` for one live source turn.
    ///
    /// `sub_504B0` is the only pinned constructor for this action and creates
    /// model 53. The callback decrements life before its negative terminal
    /// return, latches sound 30 and shifts, then consumes RNG-L2-0263 before
    /// the three accepted SEARCH.DAT shape-0 cells (0264..0267). Each accepted
    /// child is the existing `NewAdd0A00_4E320` model-0 arena row with the
    /// source's copied identity and action-58 control/shift overrides.
    pub fn step_authored_model53_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model53
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };

        let mut source = self.authored_model53[source_index];
        let life_before = source.life;
        source.life = source.life.wrapping_sub(1);
        if life_before >= 0 {
            if source.control_flags & 0x02 == 0 {
                source.control_flags |= 0x0001_0002;
                source.last_sound_id = 30;
            }
            let signed_term = 768_i32 * source.dword_0x10;
            let sign = signed_term.signum();
            source.shift_rot_x = i16::try_from((signed_term - sign * 5) >> 2)
                .expect("Level 2 action-58 shift rotation fits i16");
            source.shift_rot_y = 512;

            // `sub_10C80(a2 = 0)` is a no-RNG target-mailbox scan. Its
            // cross-entity record writes remain outside this economy-owned
            // carrier; the source transition and RNG-producing child path are
            // kept exact here.
            source.entity_rng = level2_next_rng16(source.entity_rng);
            for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_x = i32::from(source.position.x) - 96
                    + 160 * offset_x
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_y = i32::from(source.position.y) - 96
                    + 160 * offset_y
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                let child_position = Position {
                    x: child_x as u16,
                    y: child_y as u16,
                    z: source.position.z,
                };
                let _ = self.allocate_model0_child(
                    source_slot,
                    source.entity_id,
                    0,
                    child_position,
                    0x0003_0080,
                    512,
                    512,
                );
            }
            source.dword_0x10 = (source.dword_0x10 + 2).rem_euclid(7);
        }
        // ApplyEvents advances byte 0x3E even when sub_38B90 returns its
        // terminal positive result, so the typed phase follows every visit.
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model53[source_index] = source;
        true
    }

    /// Materialize remc2's shared `NewAdd0A05_4E570` constructor. Authored
    /// group rows and model-53's water branch both produce class 10/model 5/
    /// action 5 with max/life 8, subspell 0, control dword 0x20000, grounded,
    /// mapped, and sprite 244.
    pub fn allocate_authored_model5_water_splash(
        &mut self,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel5WaterSplashSource> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut grounded_position = position;
        grounded_position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: grounded_position.x,
                y: grounded_position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel5WaterSplashSource {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            source_class: 10,
            model: 5,
            action: 5,
            max_life: 8,
            life: 8,
            subspell_index: 0,
            dword_0x10: 0,
            control_flags: 0x20_000,
            sprite_index: 244,
            last_sound_id: 0,
            position: grounded_position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model5_water_splashes.push(source);
        Some(source)
    }

    /// Run the bounded action-5 lifecycle from `AddAsh0A_05_318B0`: consume
    /// the eight-life splash window, emit source sound 27 once through the
    /// typed sidecar, then disable drawing and retire the row on the following
    /// callback. The `sub_585A0` animation increment is a no-op for this
    /// constructor's zero animation limit and is therefore represented by the
    /// unchanged sprite. `reclaim_inactive_slots` mirrors the source's same-
    /// pass `ApplyEvents` cleanup after the draw-disable bit is set.
    pub fn step_authored_model5_water_splash_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_model5_water_splashes
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        if source.life >= 0 {
            source.life -= 1;
            if source.control_flags & 0x2 == 0 {
                source.control_flags |= 0x2;
                source.last_sound_id = 27;
            }
        } else {
            source.drawing_enabled = false;
            source.active = false;
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        true
    }

    /// Allocate the authored class-10/subtype-39 consumer from the generic
    /// `sub_4A310` path.  The pinned subtype table selects
    /// `CreateManaSphere512_50080`, whose constructor writes a model-39,
    /// action-41 sphere with a fixed 512 mana payload.  These rows are
    /// dormant authored records, so their source owner/player links remain
    /// zero until a later source-owned claim/possession callback changes
    /// them; no relation is inferred from the activating switch.
    pub fn allocate_authored_model39(
        &mut self,
        _authored_index: u16,
        trigger_source_entity_index: u16,
        position: Position,
    ) -> Option<ManaSphere> {
        const AUTHORED_MANA: i32 = 512;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let sphere = ManaSphere {
            id: u32::from(source_entity_index),
            source_entity_index,
            entity_rng,
            position,
            velocity_x: 0,
            velocity_y: 0,
            // CreateManaSphere_500C0 seeds word_0x2C with 128.  The first
            // ordinary TransformArcherToMana callback therefore rises before
            // gravity can reach the ground; preserving that seed prevents a
            // same-pass source cursor from merging freshly materialized
            // authored spheres before their later source rows are visited.
            vertical_velocity: 128,
            yaw: 0,
            max_life: 300,
            life: 300,
            mana: AUTHORED_MANA,
            owner_entity_index: 0,
            player_entity_index: 0,
            incoming_owner_entity_index: 0,
            incoming_possession_payload: 0,
            attraction_source_entity_index: 0,
            attraction_step: 0,
            attraction_distance_high: 0,
            assigned_carrier_id: 0,
            assigned_defender_slot: 0,
            control_flags: 0,
            // NewEvent seeds byte 0x3E from the allocated arena row.  The
            // typed phase is the corresponding source-visible byte.
            phase: source_entity_index as u8,
            active: true,
            pending_delete: false,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spheres.push(sphere);
        self.refresh_owner_mana_rows();
        self.events.push(Level2EconomyEvent::ManaSphereSpawned {
            sphere_id: sphere.id,
            source_entity_index: trigger_source_entity_index,
            mana: AUTHORED_MANA,
        });
        Some(sphere)
    }

    /// Resolve one live authored action-0/action-1 switch callback. The caller
    /// supplies the source `CompareAxisWithShift` result. Group consumption,
    /// drawing disable, and the common phase epilogue remain separate because
    /// remc2 rebuilds/consumes the authored group before hiding the producer.
    pub fn step_authored_switch_producer_slot(
        &mut self,
        source_slot: u16,
        overlap: bool,
    ) -> Option<u16> {
        let source = self
            .authored_switch_producers
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)?;
        (source.phase & 7 == 0 && overlap).then_some(source.stage_tag)
    }

    /// Disable a one-shot authored switch after its group scan has returned.
    pub fn complete_authored_switch_producer_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_switch_producers
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.fired = true;
        source.drawing_enabled = false;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .ok();
        true
    }

    /// Bounded projection of the generic `sub_1BD90`/`sub_1BF90` awareness
    /// search used by actions 16 and 17.  The pinned descriptor permits an
    /// XY search radius of `word_0x1C_28 == 0x1400`; a mailbox pointing at an
    /// active entity outside that band is not an actionable target for these
    /// transitions.  This keeps the explicit target mailbox from becoming a
    /// synthetic "any target means attack" shortcut.
    pub(super) fn bee_target_in_awareness_band(
        source: &BeeCreatureRuntime,
        target: Position,
    ) -> bool {
        let dx = i32::from(target.x) - i32::from(source.position.x);
        let dy = i32::from(target.y) - i32::from(source.position.y);
        let distance_squared = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
        distance_squared <= 5_120_i64 * 5_120_i64
    }

    /// Run the `byte_0x3E & 3 == 0` steering tail in `sub_1C310`. The source
    /// first points its roll at the live target, then lets the first nearby
    /// same-model peer override that roll. Peer bounds use the entity's
    /// source pitch field exactly as the decompilation does.
    pub(super) fn steer_authored_bee_source(
        source: &mut BeeCreatureRuntime,
        target: Position,
        peers: &[(u16, Position)],
    ) {
        if source.source_byte_3e & 3 != 0 {
            return;
        }
        source.roll = i16::try_from(heading_for_positions(source.position, target))
            .expect("Bee target heading fits i16");
        let peer_bound = i32::from(source.pitch);
        if peer_bound <= 0 {
            return;
        }
        for &(peer_id, peer_position) in peers {
            if peer_id == source.entity_id {
                continue;
            }
            let dx = (i32::from(source.position.x) - i32::from(peer_position.x)).abs();
            let dy = (i32::from(source.position.y) - i32::from(peer_position.y)).abs();
            if dx < peer_bound && dy < peer_bound {
                source.roll = i16::try_from(heading_for_positions(peer_position, source.position))
                    .expect("Bee peer heading fits i16");
                break;
            }
        }
    }

    /// Advance one authored Bee through the live action 16..19 band.  The
    /// generic creature helpers are represented only for the fields this
    /// family actually consumes: the action-18 latch, target-Z tracking,
    /// strict target contact/recoil, and source RNG/phase ordering. The live
    /// outer dispatcher runs action-17's movement/acquisition prelude through
    /// `acquire_authored_bee_target` before calling this state transition.
    pub fn step_authored_bee_slot(
        &mut self,
        source_slot: u16,
        target_position: Option<Position>,
        terrain: &SessionTerrain,
    ) -> bool {
        let action17_entry_dead = self.bee_creatures.iter().any(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.action == 17
                && source.life < 0
        });
        self.step_authored_bee_slot_with_target(
            source_slot,
            target_position.map(|position| (position, 0, true)),
            action17_entry_dead,
            terrain,
        )
        .live
    }

    pub(super) fn step_authored_bee_slot_with_target(
        &mut self,
        source_slot: u16,
        target: Option<(Position, i16, bool)>,
        action17_entry_dead: bool,
        terrain: &SessionTerrain,
    ) -> AuthoredBeeStepOutcome {
        self.step_authored_bee_slot_with_target_and_peers(
            source_slot,
            target,
            &[],
            &[],
            action17_entry_dead,
            terrain,
        )
    }

    pub(super) fn step_authored_bee_slot_with_target_and_peers(
        &mut self,
        source_slot: u16,
        target: Option<(Position, i16, bool)>,
        peers: &[(u16, Position)],
        class3_candidates: &[(u16, Position, u32)],
        action17_entry_dead: bool,
        terrain: &SessionTerrain,
    ) -> AuthoredBeeStepOutcome {
        let Some(source_index) = self
            .bee_creatures
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return AuthoredBeeStepOutcome::INACTIVE;
        };
        let mut source = self.bee_creatures[source_index];
        let mut sound = None;
        // `sub_1F660` consumes the action-17 mailbox before dispatching the
        // ordinary callback body.  A consumed mailbox selects action 18 or
        // action 20 directly, so the target scan and its RNG tail must not
        // run again on this same source visit.
        if self
            .consume_authored_bee_damage_mailbox(&mut source)
            .is_some()
        {
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            source.phase = source.phase.wrapping_add(1);
            self.source_rng.insert(source_slot, source.entity_rng);
            self.bee_creatures[source_index] = source;
            let live = self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: source.active,
                    x: source.position.x,
                    y: source.position.y,
                })
                .is_ok();
            return AuthoredBeeStepOutcome { live, sound };
        }
        // `sub_1BF90` clears word_0x26 when no damage mailbox is pending,
        // then tests the life value captured at callback entry. A Bee killed
        // by the prior action-17 movement prelude therefore enters action 20
        // on this following visit without inheriting an older attacker's
        // credit. The separate entry flag preserves the source distinction
        // from a movement failure produced later in this same callback.
        if source.action == 17 {
            source.damage_attacker_entity_index = 0;
        }
        if action17_entry_dead {
            source.kill_credit_entity_index = source.damage_attacker_entity_index;
            source.action = 20;
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            source.phase = source.phase.wrapping_add(1);
            self.source_rng.insert(source_slot, source.entity_rng);
            self.bee_creatures[source_index] = source;
            let live = self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: source.active,
                    x: source.position.x,
                    y: source.position.y,
                })
                .is_ok();
            return AuthoredBeeStepOutcome { live, sound };
        }
        match source.action {
            16 => {
                if target
                    .map(|(position, _, valid)| {
                        valid && Self::bee_target_in_awareness_band(&source, position)
                    })
                    .unwrap_or(false)
                {
                    source.action = 18;
                    source.dword_0x10 = 1;
                }
            }
            17 => {
                if target
                    .map(|(position, _, valid)| {
                        valid && Self::bee_target_in_awareness_band(&source, position)
                    })
                    .unwrap_or(false)
                {
                    source.action = 18;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    source.last_sound_id = (source.entity_rng & 1) + 12;
                    sound = Some(AuthoredBeeSound::Action17Acquire(source.last_sound_id));
                    source.dword_0x10 = 1;
                }
            }
            18 => {
                let countdown_before = source.dword_0x10;
                if countdown_before != 0 {
                    source.dword_0x10 -= 1;
                    if countdown_before == 1 {
                        source.actual_speed = 5 * source.minimum_speed / 2;
                    }
                }
                if let Some((target, target_height_offset, target_valid)) = target {
                    let desired_z = target_height_offset.wrapping_add(target.z);
                    let delta = source.position.z.wrapping_sub(desired_z);
                    let direction: i16 = if delta == 0 {
                        0
                    } else if delta < 0 {
                        -1
                    } else {
                        1
                    };
                    source.position.z = source
                        .position
                        .z
                        .wrapping_add(direction.wrapping_mul(source.vertical_step));
                    // `sub_1F6D0` updates target-Z before entering
                    // `sub_1C310`; that helper performs movement even when
                    // its subsequent target validity check rejects the
                    // relation, but skips steering/cadence in that case.
                    move_level2_bee_source(&mut source, terrain);
                    if !target_valid {
                        source.action = 17;
                    } else {
                        Self::steer_authored_bee_source(&mut source, target, peers);
                        let on_action_window = source.action_window > 0
                            && source.source_byte_3e
                                % u8::try_from(source.action_window)
                                    .expect("positive Bee action window")
                                == 0;
                        let distance_squared =
                            level2_wrapping_source_distance_3d(source.position, target);
                        let out_of_range = source.action_window > 0
                            && on_action_window
                            && crate::legacy_sqrt(distance_squared)
                                >= u32::try_from(LEVEL2_BEE_AWARENESS_RADIUS)
                                    .expect("Bee awareness radius fits");
                        if out_of_range {
                            // `sub_1C310` drops a live relation only at the
                            // source cadence; the stale word_0x96 remains for
                            // the next action-17 scan to overwrite.
                            source.action = 17;
                        } else if source.action == 18
                            && on_action_window
                            && distance_squared
                                < u32::from(LEVEL2_BEE_CONTACT_RADIUS)
                                    .saturating_mul(u32::from(LEVEL2_BEE_CONTACT_RADIUS))
                        {
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            source.last_sound_id = (source.entity_rng & 1) + 12;
                            sound = Some(AuthoredBeeSound::Action18Contact(source.last_sound_id));
                            source.actual_speed = -source.maximum_speed;
                            source.dword_0x10 = i32::from(3 * source.action_window);
                        }
                    }
                } else {
                    source.action = 17;
                }
                if source.action != 18 {
                    source.actual_speed = source.minimum_speed;
                }
            }
            19 => {
                // `sub_1C560` only re-evaluates its linked-target relation on
                // the descriptor's 30-tick byte-3E window.  A valid mailbox
                // outside that window remains in action 19 until the next
                // helper visit rather than jumping directly to attack.
                if source.action_window > 0
                    && source.source_byte_3e
                        % u8::try_from(source.action_window).expect("positive action window")
                        == 0
                    && target
                        .map(|(position, _, valid)| {
                            valid && Self::bee_target_in_awareness_band(&source, position)
                        })
                        .unwrap_or(false)
                {
                    source.action = 18;
                    source.dword_0x10 = 1;
                }
            }
            20 => {
                // `sub_1F830` -> `PreKillEntity_1C890(a1, 16)` changes the
                // root and any linked children to action 21. Authored Day
                // Bees have no child chain in this bounded arena.
                source.kill_credit_entity_index = source.damage_attacker_entity_index;
                source.action = 21;
            }
            21 => {
                // `sub_1F850` -> `KillEntity_1C930` is phase-gated by the
                // source byte-0x3E.  The class-5 helper keeps phases 1..7
                // live and performs the ordered sphere/burst conversion only
                // at the admitted phase-0 boundary.
                self.step_authored_bee_death_tail(&mut source, terrain);
            }
            23 => {
                // `sub_1F8A0 -> sub_1D5D0(16)` dispatches the bounded
                // authored L2-06 selector-2 callback before its own roll
                // tail. Other selectors remain deliberately fail-closed.
                if source.stage_var2 == 2 {
                    if source.life < 0 {
                        // Both selector-2 helpers select their terminal
                        // branch before `sub_1B8C0`. A helper which itself
                        // exhausts movement retries remains action 23 until
                        // the following callback reaches here.
                        source.action = 20;
                    } else if let Some(stage_target) = source.stage_target {
                        let outside_point_band = (i32::from(stage_target.x as i16)
                            - i32::from(source.position.x as i16))
                        .abs()
                            > 3_072
                            || (i32::from(stage_target.y as i16)
                                - i32::from(source.position.y as i16))
                            .abs()
                                > 3_072;
                        move_level2_bee_source(&mut source, terrain);
                        if outside_point_band {
                            if source.source_byte_3e & 7 == 0 {
                                if source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0 {
                                    source.roll = i16::try_from(heading_for_positions(
                                        source.position,
                                        stage_target,
                                    ))
                                    .expect("Bee StageVar heading fits i16");
                                    if source.source_byte_3e & 0x3f == 0 {
                                        let first = level2_next_rng16(source.entity_rng);
                                        let second = level2_next_rng16(first);
                                        let direction = 2 * (u32::from(first % 0x9d) / 79);
                                        let delta = (i32::from((second & 0xff) as u8) + 85)
                                            * (i32::try_from(direction)
                                                .expect("Bee roll direction fits")
                                                - 1);
                                        source.roll =
                                            ((i32::from(source.roll) + delta) as i16) & 0x07ff;
                                        source.entity_rng = second;
                                    }
                                }
                                let peer_bound = i32::from(source.pitch);
                                if peer_bound > 0 {
                                    for &(peer_id, peer_position) in peers {
                                        if peer_id == source.entity_id {
                                            continue;
                                        }
                                        let dx = (i32::from(source.position.x)
                                            - i32::from(peer_position.x))
                                        .abs();
                                        let dy = (i32::from(source.position.y)
                                            - i32::from(peer_position.y))
                                        .abs();
                                        if dx < peer_bound && dy < peer_bound {
                                            source.roll = i16::try_from(heading_for_positions(
                                                peer_position,
                                                source.position,
                                            ))
                                            .expect("Bee peer heading fits i16");
                                            break;
                                        }
                                    }
                                }
                            }
                        } else if source.source_byte_3e & 0x0f == 0
                            && source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0
                        {
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            source.roll = ((i32::from(source.roll)
                                + i32::from(source.entity_rng % 0x71)
                                + 142) as i16)
                                & 0x07ff;
                        }

                        if source.action == 23
                            && source.stage_var2 == 2
                            && source.source_byte_39 != 0
                            && source.action_window > 0
                            && source.source_byte_3e
                                % u8::try_from(source.action_window)
                                    .expect("positive Bee action window")
                                == 0
                            && let Some(target_index) =
                                select_level2_bee_class3_target(&source, class3_candidates)
                        {
                            source.target_entity_index = Some(target_index);
                            source.stage_var2 = 10;
                            // Model 2's descriptor lacks the bit-8 route, so
                            // `sub_1E040(..., 16)` selects action 18.
                            source.action = 18;
                        }
                    }
                } else if source.stage_var2 == 10 {
                    // Existing selector-10 preservation route.
                    source.action = 18;
                }
                if source.source_byte_3e & 7 == 0 && (1..=9).contains(&source.stage_var2) {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let direction = 2 * (u32::from(source.entity_rng % 0x9d) / 79);
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let delta = i32::from(source.entity_rng % 0x55)
                        * (i32::try_from(direction).expect("Bee roll direction fits") - 1);
                    source.roll = ((i32::from(source.roll) + delta) as i16) & 0x07ff;
                }
                if source.action == 18 {
                    // The source applies the action-18 latch after the
                    // selector callback and action-23 roll tail.
                    source.dword_0x10 = 1;
                }
            }
            22 => {
                // Deliberate missing callback slot: the source body is empty,
                // but the outer ApplyEvents bookkeeping below still advances
                // the per-entity cursor/phase once for the visited row.
            }
            _ => {}
        }
        // `ApplyEvents_498A0` advances byte 0x3E after every live callback;
        // action 19's generic helper uses that advancing cursor for its
        // descriptor-windowed reevaluation.
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.bee_creatures[source_index] = source;
        let live = self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok();
        AuthoredBeeStepOutcome { live, sound }
    }

    /// Allocate the authored class-10/model-13 emitter initialized by the
    /// pinned `sub_4EF90` constructor.  The authored trigger is owned by the
    /// Level 2 activation route; this method owns source-pool allocation,
    /// map linkage, and the exact constructor payload.
    pub fn allocate_authored_model13(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel13Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        // sub_4EF90 clears bits 3 and 17, sets byte 2 bit 1, then sets the
        // drawing bit in byte 0 before copying max-life to life.
        let source = Level2AuthoredModel13Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 19,
            action: 19,
            subspell_index: 200,
            max_life: 240,
            life: 240,
            dword_0x10: 0,
            particle_index: 228,
            control_flags: 0x0002_0001,
            shift_rot_x: 512,
            shift_rot_y: 512,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model13.push(source);
        Some(source)
    }

    /// Allocate the authored class-10/model-6 event initialized by
    /// `NewAdd0A06_4E5F0`. The constructor's fixed payload is source-backed:
    /// action/model 6, subspell 50, max/life 240, particle 228, clear low
    /// control byte, set byte-2 bit 1, shifts 272/1536, and height offset 0.
    pub fn allocate_authored_model6(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel6Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel6Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            model: 6,
            action: 6,
            subspell_index: 50,
            max_life: 240,
            life: 240,
            height_offset: 0,
            dword_0x10: 0,
            particle_index: 228,
            control_flags: 0x0002_0000,
            shift_rot_x: 272,
            shift_rot_y: 1536,
            position,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model6.push(source);
        Some(source)
    }

    /// Allocate the generic class-14/model-3 or model-5 authored event
    /// produced by `sub_4A310`.  The pinned constructor uses action 8 and
    /// particle 338 for model 3 (`sub_51570`), and action 10 and particle 280
    /// for model 5 (`sub_51610`).  No mission trigger is inferred here: the
    /// caller explicitly selects an authored row that has already fired.
    pub fn allocate_authored_class14(
        &mut self,
        authored_index: u16,
        model: u8,
        experience_award: Option<i32>,
        activation_group: i16,
        stage_tag: i16,
        position: Position,
    ) -> Option<Level2AuthoredClass14Source> {
        let (action, particle, control_flags, shift_rot_x, shift_rot_y) = match model {
            3 => (8, 338, 1, 0, 0),
            5 => (10, 280, 0, 768, 1280),
            _ => return None,
        };
        if (model == 5) != experience_award.is_some_and(|award| award > 0) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredClass14Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model,
            experience_award,
            action,
            particle,
            control_flags,
            shift_rot_x,
            shift_rot_y,
            activation_group,
            stage_tag,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_class14.push(source);
        if model == 3 {
            self.class14_model3_handle = source_entity_index;
        }
        Some(source)
    }

    pub(super) fn allocate_house_resident(
        &mut self,
        parent_house_entity_index: u16,
        owner_entity_index: u16,
        request: HouseResidentRequest,
    ) -> Option<u16> {
        let semantic_kind =
            crate::campaign::mc2_level_definition::Mc2LevelDefinition::creature_for_house_resident(
                request.model,
                request.action,
            )?;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: request.position.x,
                y: request.position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2HouseResidentSource {
            source_entity_index,
            entity_rng,
            parent_house_entity_index,
            owner_entity_index,
            attacker_entity_index: request.attacker_entity_index,
            semantic_kind,
            position: request.position,
            model: request.model,
            action: request.action,
            life: 1_000,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner
            .insert(source_entity_index, owner_entity_index);
        self.house_residents.push(source);
        Some(source_entity_index)
    }

    pub(super) fn step_house_resident_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .house_residents
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.phase = source.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    #[must_use]
    pub fn house_residents(&self) -> &[Level2HouseResidentSource] {
        &self.house_residents
    }

    pub(super) fn set_house_owner_relation(&mut self, source_slot: u16, owner_entity_index: u16) {
        self.source_owner.insert(source_slot, owner_entity_index);
    }

    /// Reserve one shared numeric row for a typed authored House. Publication
    /// of the slot, map link, RNG and private semantic identity is atomic.
    pub(super) fn allocate_authored_house_slot(
        &mut self,
        authored_record: u16,
        position: Position,
    ) -> Option<(u16, u16)> {
        let source_slot = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_slot);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_slot, None);
            return None;
        }
        self.source_rng.insert(source_slot, entity_rng);
        self.source_owner.insert(source_slot, 0);
        self.owner_class.insert(source_slot, HOUSE_CLASS);
        self.authored_house_record_by_source
            .insert(source_slot, authored_record);
        Some((source_slot, entity_rng))
    }

    #[must_use]
    pub(super) fn authored_house_record(&self, source_slot: u16) -> Option<u16> {
        self.authored_house_record_by_source
            .get(&source_slot)
            .copied()
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn authored_house_source_slot(&self, authored_record: u16) -> Option<u16> {
        self.authored_house_record_by_source
            .iter()
            .find_map(|(source_slot, record)| (*record == authored_record).then_some(*source_slot))
    }

    pub(super) fn rollback_authored_house_slot(&mut self, source_slot: u16) {
        self.authored_house_record_by_source.remove(&source_slot);
        self.source_owner.remove(&source_slot);
        self.owner_class.remove(&source_slot);
        self.source_rng.remove(&source_slot);
        self.release_slot(source_slot, None);
    }

    pub(super) fn reclaim_house_source_slot_at_source_boundary(
        &mut self,
        source_slot: u16,
        source_rng: u16,
    ) {
        self.authored_house_record_by_source.remove(&source_slot);
        self.source_owner.remove(&source_slot);
        self.owner_class.remove(&source_slot);
        self.source_rng.remove(&source_slot);
        self.release_slot(source_slot, Some(source_rng));
    }

    /// Advance one model-1 root through `AddQuickfair0A_01_30F60`.
    ///
    /// The callback's transition order is life guard, first sound latch,
    /// SEARCH.DAT shape-0 walk, then the independent ApplyEvents phase
    /// increment. A negative pre-decrement return hides the root and consumes
    /// no Quickfair draw. The three accepted shape cells use the existing
    /// model-0 child arena, which is the pinned `NewAdd0A00_4E320` producer
    /// used by `IfSubtypeCallCreatingManaSphere_4A190(10, 0)`.
    pub fn step_authored_model1_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model1
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_model1[source_index];
        let life_before = source.life;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
        } else {
            if source.control_flags & 0x02 == 0 {
                source.control_flags |= 0x02;
                source.last_sound_id = 3;
                self.prepare_sound(source_slot, source_slot, -1, 3);
            }
            for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                if source.entity_rng % 0x9d < 79 {
                    continue;
                }
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_x = i32::from(source.position.x) - 96
                    + 192 * offset_x
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_y = i32::from(source.position.y) - 96
                    + 192 * offset_y
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                let child_position = Position {
                    x: child_x as u16,
                    y: child_y as u16,
                    z: source.position.z,
                };
                let _ = self.allocate_model0_child(
                    source_slot,
                    source_slot,
                    0,
                    child_position,
                    // NewAdd0A00 starts at 0x0002_0000; Quickfair's
                    // producer then applies the source's 0x80 child bit.
                    0x0002_0080,
                    128,
                    128,
                );
            }
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model1[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub fn step_spell_pickup_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .spell_pickups
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.phase = source.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Keep the shared class-15 entity projection and the source-arena cursor
    /// on the same live row after `sub_68FF0` changes an ordinary pickup into
    /// its active holder. The scheduler resolves the next visit from this
    /// arena selector, so retaining the pre-acquisition action would dispatch
    /// a stale callback even though the entity record already says otherwise.
    pub(super) fn sync_spell_pickup_projection(
        &mut self,
        source_slot: u16,
        action: u8,
        position: Position,
    ) -> bool {
        let Some(source) = self
            .spell_pickups
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.action = action;
        source.position = position;
        true
    }

    pub fn step_authored_model45_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_model45
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };

        // `sub_3A650` is class-10/model-45/action-51's callback.  The
        // constructor leaves `word_0x96_150` at zero for these authored rows,
        // so its source scan has no parent target; it only sets the hidden
        // drawing bit. This production route is UpdateEntities: the outer
        // scheduler increments byte 0x3E after callback return, and only the
        // following pass prologue unlinks and returns the row to NewEvent.
        if source.action == 51 {
            source.drawing_enabled = false;
            return true;
        }
        false
    }

    /// Reclaim model-45 rows hidden by the preceding UpdateEntities callback.
    /// `sub_3A650` only disables drawing; `sub_57F20` owns unlink/release at
    /// the next UpdateEntities prologue.
    pub(super) fn reclaim_hidden_authored_model45_rows_at_source_boundary(&mut self) {
        let hidden = self
            .authored_model45
            .iter()
            .filter(|source| source.active && !source.drawing_enabled)
            .map(|source| {
                (
                    source.source_entity_index,
                    source.entity_rng,
                    source.position,
                )
            })
            .collect::<Vec<_>>();
        self.authored_model45
            .retain(|source| source.drawing_enabled);
        for (slot, entity_rng, position) in hidden {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: position.x,
                y: position.y,
            });
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Advance one authored model-13 emitter through the source action-19
    /// callback `sub_32F40`.  The qualified Level 2 row uses SEARCH.DAT
    /// class zero (the four centre cells), preserves the source LCG draw
    /// order, and allocates subtype-14 children only on the source's odd-life
    /// branch.  Cross-entity presentation (`sub_10C80`) is intentionally
    /// outside this typed state seam.
    pub fn step_authored_model13_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self
            .authored_model13
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model13[source_index];
        let life_before = source_before.life;
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
            // `DisableEntityDrawing04_57F10` only sets the hidden-drawing
            // bit. The source row remains in the entity arena and continues
            // through the cursor; reclaim is a separate reset/pool concern.
            source.active = true;
            self.authored_model13[source_index] = source;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-13 source slot fits the map chain");
            return true;
        }

        source.dword_0x10 = 0;
        // SEARCH.DAT class zero is the four-pixel centre block in the exact
        // row-major order produced by sub_10130 after sub_101C0 loading.
        for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1), (1, 1)] {
            source.entity_rng = level2_next_rng16(source.entity_rng);
            if source.entity_rng % 0x9d < 79 {
                continue;
            }
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let child_x = i32::from(source.position.x) - 96
                + 192 * offset_x
                + i32::from(source.entity_rng % 0x81)
                - 64;
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let child_y = i32::from(source.position.y) - 96
                + 192 * offset_y
                + i32::from(source.entity_rng % 0x81)
                - 64;
            if source.life & 1 != 0 {
                let mut yaw = ((source.life / 2) & 1) << 8;
                while yaw < 0x800 {
                    let position = Position {
                        x: child_x as u16,
                        y: child_y as u16,
                        z: source.position.z,
                    };
                    let _ = self.allocate_model13_spawned_child(
                        source_slot,
                        source.entity_id,
                        position,
                        yaw,
                    );
                    yaw = yaw.wrapping_add(0x200);
                }
            }
        }
        source.position.z = terrain.ground_height(source.position);
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model13[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(super) fn allocate_model13_spawned_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
        yaw: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        // SetParticleSmoke3C first advances the process-global RNG for the
        // child's life, then SetSmoke4 advances the newly allocated row's
        // own RNG for its actual speed.
        self.global_rng = level2_next_rng32(self.global_rng);
        let max_life =
            i16::try_from(self.global_rng as u16 % 0x21 + 28).expect("model-13 child life fits");
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let actual_speed =
            i16::try_from(entity_rng % 0x35 + 51).expect("model-13 child speed fits");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model13_spawned_children
            .push(Level2Model13SpawnedChild {
                source_entity_index,
                entity_rng,
                parent_entity_index,
                parent_entity_id,
                source_class: 10,
                model: 14,
                action: 14,
                max_life,
                life: max_life,
                max_speed: 30,
                actual_speed,
                dword_0x10: 0,
                particle_index: 9,
                entity_index: 9,
                subtype: 14,
                yaw,
                control_flags: 0x0002_0000,
                position,
                phase: source_entity_index as u8,
                drawing_enabled: true,
                active: true,
            });
        Some(source_entity_index)
    }

    /// Advance one model-14 child through the bounded source `sub_322A0`
    /// callback: signed life sentinel, upward movement, terrain clamp, and
    /// sprite index cadence. The callback has no cross-entity relations.
    pub fn step_model13_spawned_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self
            .model13_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model13_spawned_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.drawing_enabled = false;
            // The pinned disable helper hides the row but does not unlink or
            // reclaim it from the source arena.
            child.active = true;
            self.model13_spawned_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-13 child slot fits the map chain");
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        let mut predicted = child.position;
        predicted.z = predicted
            .z
            .saturating_add(child.actual_speed)
            .max(terrain.ground_height(predicted));
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut predicted, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 16 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 9 {
            child.particle_index -= 1;
        }
        child.position = predicted;
        child.phase = child.phase.wrapping_add(1);
        self.model13_spawned_children[child_index] = child;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Advance one model-6 row through the pinned `sub_31760` callback.
    /// This preserves the signed life sentinel, the late-life countdown,
    /// source RNG cadence, class-9/model-14 child allocation, terrain-height
    /// projection, water disable, and map-link state. The source's
    /// `sub_5C870` player-distance mailbox and `sub_11400` attraction scan are
    /// intentionally separate cross-entity follow-up surfaces.
    pub fn step_authored_model6_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self
            .authored_model6
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model6[source_index];
        let life_before = source_before.life;
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.active = false;
            self.authored_model6[source_index] = source;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-6 source slot fits the map chain");
            return true;
        }

        // sub_5C870 updates the local player's minimum distance sidecar. The
        // typed economy does not own that nested player record, so preserve
        // the callback's state mutation here and leave the sidecar bridge to
        // the cross-entity Level 2 integration bead.
        if source.life < 12 {
            if source.dword_0x10 > 0 {
                source.dword_0x10 -= 1;
                source.particle_index = source.particle_index.wrapping_sub(1);
                let low_control = i8::from_le_bytes([source.control_flags as u8]);
                if low_control >= 0 {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    if source.entity_rng % 7 == 0 {
                        let child = self.allocate_model6_spawned_child(
                            source_slot,
                            source.entity_id,
                            source.position,
                        );
                        // NewEvent failure consumes no parent state beyond the
                        // source RNG write, matching the pinned branch.
                        let _ = child;
                    }
                }
            }
        } else if source.dword_0x10 <= 6 {
            source.dword_0x10 += 1;
            source.particle_index = source.particle_index.wrapping_add(1);
        }
        source.position.z = source
            .height_offset
            .wrapping_add(terrain.ground_height(source.position));
        if terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK {
            source.active = false;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-6 water disable fits the map chain");
        }
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model6[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(super) fn allocate_model6_spawned_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_phase_sidecars
            .insert(source_entity_index, source_entity_index as u8);
        self.model6_spawned_children.push(Level2Model6SpawnedChild {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            parent_entity_id,
            source_class: 9,
            model: 14,
            action: 15,
            max_life: 32,
            life: 15,
            actual_speed: 128,
            min_speed: 128,
            max_speed: 0,
            yaw: 0,
            height_offset: 0,
            dword_0x10: 100,
            // subtype-14's constructor sets particle 196; the parent then
            // copies that newly-created child's word-0x5A and adds two.
            particle_index: 198,
            control_flags: 0,
            position,
            active: true,
        });
        Some(source_entity_index)
    }

    /// Allocate the authored class-10/model-17 Meteor initialized by
    /// `AddMeteor_4ED70`.  The constructor is explicit in the pinned source:
    /// action/model 17, life/max-life 10, subspell 3000, and a map-linked
    /// position.  The authored parameter selectors are activation metadata,
    /// not constructor fields for this source row.
    pub fn allocate_authored_model17(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel17Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel17Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 17,
            action: 17,
            subspell_index: 3_000,
            max_life: 10,
            life: 10,
            dword_0x10: 0,
            control_flags: 0,
            shift_rot_x: 0,
            shift_rot_y: 0,
            yaw: 0,
            position,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model17.push(source);
        Some(source)
    }

    /// Advance one authored Meteor through pinned `sub_32880`.  SEARCH.DAT
    /// shape 0 is the four-cell 2x2 kernel centered at (15,15); the source's
    /// iterator executes the first three cells because its return value is
    /// tested for `== 1` before each child allocation.  The source RNG is
    /// advanced once before the walk, then once for each child coordinate.
    pub fn step_authored_model17_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model17
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model17[source_index];
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if source.life < 0 {
            source.active = false;
            self.authored_model17[source_index] = source;
            return true;
        }

        if source.control_flags & 0x02 == 0 {
            // `sub_32880` sets byte 0 bit 1 and byte 2 bit 0, then prepares
            // sound 30.  The audio ledger owns presentation dispatch; the
            // raw source write belongs in this state tuple.
            source.control_flags |= 0x0001_0002;
        }
        let signed_term = 768_i32 * source.dword_0x10;
        let sign = signed_term.signum();
        source.shift_rot_x = i16::try_from((signed_term - sign * 5) >> 2)
            .expect("Level 2 Meteor shift rotation fits i16");
        source.shift_rot_y = 512;

        // Source AddE7EE0x( dword, dword ) walks the first three cells of
        // each SEARCH.DAT shape.  For shape zero these are (0,0), (1,0), and
        // (0,1), with a 160-unit cell stride and signed 96/64 offsets.
        source.entity_rng = level2_next_rng16(source.entity_rng);
        for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let x_jitter = i32::from(source.entity_rng % 0x81);
            let child_x = i32::from(source.position.x) - 96 + 160 * offset_x + x_jitter - 64;
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let y_jitter = i32::from(source.entity_rng % 0x81);
            let child_y = i32::from(source.position.y) - 96 + 160 * offset_y + y_jitter - 64;
            let child_position = Position {
                x: child_x as u16,
                y: child_y as u16,
                z: source.position.z,
            };
            let _ = self.allocate_model0_child(
                source_slot,
                source.entity_id,
                source.yaw,
                child_position,
                0x0003_0080,
                512,
                512,
            );
        }
        source.dword_0x10 = (source.dword_0x10 + 2).rem_euclid(11);
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model17[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(super) fn allocate_model0_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        parent_yaw: i16,
        position: Position,
        control_flags: u32,
        shift_rot_x: i16,
        shift_rot_y: i16,
    ) -> Option<u16> {
        self.allocate_class10_model0_effect(
            parent_entity_index,
            parent_entity_id,
            parent_yaw,
            0,
            position,
            400,
            0,
            control_flags,
            shift_rot_x,
            shift_rot_y,
        )
    }

    /// Shared `NewAdd0A00_4E320` constructor carrier. Fireball and the
    /// authored producers allocate the same live class/model/action identity;
    /// only their copied parent, target, orientation, and payload differ.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn allocate_class10_model0_effect(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        parent_yaw: i16,
        parent_pitch: i16,
        position: Position,
        subspell_index: u16,
        target_source_entity_index: u16,
        control_flags: u32,
        shift_rot_x: i16,
        shift_rot_y: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model17_spawned_children
            .push(Level2Model17SpawnedChild {
                source_entity_index,
                entity_rng,
                parent_entity_index,
                parent_entity_id,
                source_class: 10,
                model: 0,
                action: 0,
                subspell_index,
                max_life: 8,
                life: 8,
                dword_0x10: 0,
                control_flags,
                shift_rot_x,
                shift_rot_y,
                yaw: parent_yaw,
                pitch: parent_pitch,
                source_word_2c: 0,
                target_source_entity_index,
                position,
                phase: source_entity_index as u8,
                active: true,
            });
        Some(source_entity_index)
    }

    /// Source `sub_30D50` for the shared class-10/model-0 impact row. The
    /// callback owns its lifetime, one-shot damage/RNG/sound gate, vertical
    /// terrain settling, and drawing disable. The numeric scheduler owns the
    /// common phase increment; following-pass cleanup owns map unlink/free.
    pub(super) fn step_model17_spawned_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> super::level2_dynamic_economy::Level2Class10Model0Step {
        let Some(child_index) = self
            .model17_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return super::level2_dynamic_economy::Level2Class10Model0Step {
                live: false,
                first_impact: false,
                damage_pulse: false,
                owner_entity_index: 0,
                payload: 0,
                position: Position::default(),
            };
        };
        let mut child = self.model17_spawned_children[child_index];
        let mut first_impact = false;
        let mut damage_pulse = false;

        if child.dword_0x10 & 3 != 0 {
            child.dword_0x10 = child.dword_0x10.wrapping_sub(1);
        } else {
            child.life = child.life.wrapping_sub(1);
            if child.life < -1 {
                child.active = false;
            } else {
                child.control_flags &= !1;
                let ground = terrain.ground_height(child.position);
                if child.control_flags & 2 == 0 {
                    first_impact = true;
                    damage_pulse = child.control_flags & 0x0001_0000 == 0;

                    let terrain_type = terrain.terrain_type(child.position);
                    if terrain_type != 0
                        && !matches!(terrain_type, 10 | 11 | 26)
                        && (terrain_type < 6 || terrain_type > 0x22)
                    {
                        let tile = crate::Tile {
                            x: child.position.x.to_be_bytes()[0],
                            y: child.position.y.to_be_bytes()[0],
                        };
                        if terrain.angle_flags(tile) & 7 != 1
                            && child.position.z.wrapping_sub(ground) <= 128
                            && terrain.material_mask(child.position) & 1 == 0
                        {
                            // Source terrain debris consumes this draw before
                            // the mandatory word-0x2c draw. The visual debris
                            // is presentation-only in this authority layer.
                            child.entity_rng = level2_next_rng16(child.entity_rng);
                        }
                    }

                    child.control_flags |= 2;
                    child.entity_rng = level2_next_rng16(child.entity_rng);
                    child.source_word_2c = i16::try_from(child.entity_rng % 0x41)
                        .expect("Fireball impact displacement fits i16")
                        .wrapping_sub(32);
                }

                if child.position.z > ground {
                    child.position.z = child.position.z.wrapping_add(child.source_word_2c);
                }
                if child.position.z <= ground {
                    child.position.z = ground;
                }
                if let Some(ceiling) = terrain.ceiling_height(child.position) {
                    let ceiling_limit = ceiling.wrapping_sub(child.shift_rot_y);
                    if child.position.z > ceiling_limit {
                        child.position.z = ceiling_limit;
                    }
                }
            }
        }
        self.source_rng.insert(source_slot, child.entity_rng);
        self.model17_spawned_children[child_index] = child;
        if child.active {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            });
        }
        super::level2_dynamic_economy::Level2Class10Model0Step {
            live: child.active,
            first_impact,
            damage_pulse,
            owner_entity_index: child.parent_entity_id,
            payload: child.subspell_index,
            position: child.position,
        }
    }

    /// Allocate the authored class-10/model-22 wind parent and its eleven
    /// cloned model-75/action-82 chain rows from `AddWind_4F040`. The source
    /// requires twelve available arena rows before it starts; no synthetic
    /// success is reported for an exhausted constructor.
    pub fn allocate_authored_model22(
        &mut self,
        authored_index: u16,
        axis_position: Position,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel22Source> {
        // `AddWind_4F040` refuses to start unless the source free-list has
        // room for the parent plus all eleven cloned chain rows.
        if self.free_slots.len() < 12 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let mut position = position;
        position.z = terrain.ground_height(position);
        let yaw = i16::try_from((i32::from(entity_rng & 0x07ff) - 1) & 0x07ff)
            .expect("Level 2 wind yaw fits");
        let parent = Level2AuthoredModel22Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 22,
            action: 22,
            word_0x2c: 0,
            word_0x2e: 1,
            byte_0x38: 1,
            actual_speed: 50,
            minimum_speed: 20,
            maximum_speed: 10,
            max_life: 500,
            life: 500,
            subspell_index: 1_000,
            control_flags: 0,
            word_0x30: 0,
            yaw,
            pitch: yaw,
            roll: yaw,
            particle_index: LEVEL2_MODEL22_PARTICLE_INDEXES[0],
            shift_rot_x: LEVEL2_MODEL22_SHIFT_X[0],
            shift_rot_y: LEVEL2_MODEL22_SHIFT_Y[0],
            axis_position,
            position,
            // NewEvent seeds byte 0x3e from the source slot; ApplyEvents
            // increments it after the action callback returns.
            phase: source_entity_index as u8,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model22.push(parent);

        let mut z_offset = 2 * LEVEL2_MODEL22_SHIFT_Y[0];
        for segment_index in 1..=11_u8 {
            let Some(child_slot) = self.allocate_slot() else {
                break;
            };
            let child_z = i32::from(position.z)
                .saturating_add(i32::from(z_offset))
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            let child_position = Position {
                z: child_z,
                ..position
            };
            let child = Level2Model22SpawnedChild {
                source_entity_index: child_slot,
                entity_id: source_entity_index,
                entity_rng,
                max_life: 500,
                life: 500,
                parent_entity_index: source_entity_index,
                parent_entity_id: source_entity_index,
                source_class: 10,
                model: 75,
                action: 82,
                segment_index,
                word_0x2c: i16::from(segment_index),
                word_0x36: z_offset,
                yaw,
                pitch: yaw,
                roll: yaw,
                particle_index: LEVEL2_MODEL22_PARTICLE_INDEXES[usize::from(segment_index)],
                shift_rot_x: LEVEL2_MODEL22_SHIFT_X[usize::from(segment_index)],
                shift_rot_y: LEVEL2_MODEL22_SHIFT_Y[usize::from(segment_index)],
                position: child_position,
                phase: segment_index - 1,
                drawing_enabled: true,
                active: true,
            };
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x: child_position.x,
                    y: child_position.y,
                })
                .is_err()
            {
                self.release_slot(child_slot, None);
                break;
            }
            self.source_rng.insert(child_slot, entity_rng);
            self.model22_spawned_children.push(child);
            z_offset = z_offset.saturating_add(2 * child.shift_rot_y);
        }
        if let Some(parent) = self
            .authored_model22
            .iter_mut()
            .find(|parent| parent.source_entity_index == source_entity_index)
        {
            parent.entity_rng = entity_rng;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model22
            .iter()
            .find(|parent| parent.source_entity_index == source_entity_index)
            .copied()
    }

    /// Advance one authored wind parent through the bounded kinematic portion
    /// of `sub_33110`: source life, parent heading/translation, and linked
    /// chain spacing. The source's cross-entity pulse helpers (`sub_33340`
    /// and `sub_33710`) remain a separate relation-heavy follow-up.
    pub fn step_authored_model22_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(parent_index) = self
            .authored_model22
            .iter()
            .position(|parent| parent.active && parent.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut parent = self.authored_model22[parent_index];
        parent.life = parent.life.wrapping_sub(1);
        if parent.life < 0 {
            parent.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            self.authored_model22[parent_index] = parent;
            let child_slots = self
                .model22_spawned_children
                .iter()
                .filter(|child| child.active && child.parent_entity_index == source_slot)
                .map(|child| child.source_entity_index)
                .collect::<Vec<_>>();
            for child in &mut self.model22_spawned_children {
                if child_slots.contains(&child.source_entity_index) {
                    child.active = false;
                }
            }
            return true;
        }
        parent.word_0x30 = parent.position.z as i16;
        if parent.phase & 0x0f == 0 {
            parent.entity_rng = level2_next_rng16(parent.entity_rng);
            if parent.entity_rng & 1 == 0 {
                parent.word_0x2e = -parent.word_0x2e;
            }
        }
        parent.roll = (parent.roll + 11 * parent.word_0x2e) & 0x07ff;
        let mut predicted = parent.position;
        crate::move_entity(&mut predicted, parent.roll, 0, 32);
        parent.axis_position = predicted;
        parent.yaw = (parent.yaw + 341) & 0x07ff;
        crate::move_entity(&mut predicted, parent.yaw, 0, 120);
        predicted.z = terrain.ground_height(predicted);
        parent.position = predicted;

        let mut previous = parent.position;
        let mut child_sync_rows = Vec::new();
        for child in self
            .model22_spawned_children
            .iter_mut()
            .filter(|child| child.active && child.parent_entity_index == source_slot)
        {
            child.yaw = i16::try_from(heading_for_positions(child.position, previous))
                .expect("Level 2 wind child heading fits");
            let distance = level2_wrapping_source_distance_3d(previous, child.position);
            let minimum_gap = 72 - 4 * (12 - child.word_0x2c);
            let mut child_position = child.position;
            if distance > u32::try_from(minimum_gap).unwrap_or(0) {
                crate::move_entity(
                    &mut child_position,
                    child.yaw,
                    0,
                    i16::try_from(distance - u32::try_from(minimum_gap).unwrap_or(0))
                        .unwrap_or(i16::MAX),
                );
            }
            child_position.z = i32::from(parent.position.z)
                .saturating_add(i32::from(child.word_0x36))
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX))
                as i16;
            child.position = child_position;
            previous = child.position;
            child_sync_rows.push((
                child.source_entity_index,
                child.position.x,
                child.position.y,
            ));
        }
        for (child_slot, x, y) in child_sync_rows {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x,
                    y,
                })
                .expect("Level 2 wind child slot fits the map chain");
        }
        parent.phase = parent.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, parent.entity_rng);
        self.authored_model22[parent_index] = parent;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: parent.position.x,
                y: parent.position.y,
            })
            .is_ok()
    }

    /// Advance one class-10/model-75/action-82 child through `ApplyEvents`.
    /// Model 75 is rejected by the class-10 model-range gate before action
    /// dispatch, so the row is hidden and byte 0x3e advances. Class 10's
    /// action-82 slot is disabled; the similarly numbered class-5 slot is
    /// unrelated. No callback scans players, transfers a spell, draws RNG,
    /// emits audio, changes action/life, or settles terrain.
    pub fn step_model22_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child) = self
            .model22_spawned_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.drawing_enabled = false;
        child.phase = child.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, child.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: child.active,
                x: child.position.x,
                y: child.position.y,
            })
            .expect("Level 2 wind child slot fits the map chain");
        true
    }

    /// Allocate the authored class-10/model-76 Fire Spheres root plus its
    /// exact 25 linked model-77 children. The pinned constructor is atomic:
    /// if any row cannot be reserved or linked, the whole family is rolled
    /// back with no partial live cluster.
    pub fn allocate_authored_model76(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredModel76Source> {
        if self.free_slots.len() < 1 + LEVEL2_MODEL76_CHILD_COUNT {
            return None;
        }
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut root_position = position;
        root_position.z = terrain.ground_height(root_position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: root_position.x,
                y: root_position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        let root = Level2AuthoredModel76Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 76,
            action: 83,
            max_life: 80,
            life: 80,
            subspell_index: 70,
            actual_speed: 40,
            minimum_speed: 480,
            maximum_speed: 192,
            word_0x2c: 192,
            font_type_index: 18,
            state: 0,
            target_entity_index: 0,
            target_position: root_position,
            target_pitch: 0,
            target_height_offset: 0,
            target_control_flags: ENTITY_DRAW_DISABLED_BIT,
            target_life: -1,
            shift_rot_x: 640,
            shift_rot_y: 640,
            yaw: 0,
            pitch: 0,
            position: root_position,
            phase: source_entity_index as u8,
            control_flags: 1,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model76.push(root);
        for index in 0..LEVEL2_MODEL76_CHILD_COUNT {
            let child_slot = self.allocate_slot().unwrap_or_default();
            if child_slot == 0 {
                *self = state_before;
                return None;
            }
            let row_index = u8::try_from(index / 5).expect("model-76 child row fits");
            let column_index = u8::try_from(index % 5).expect("model-76 child column fits");
            let child_rng = level2_next_rng16(entity_rng);
            let orbit = i16::try_from((child_rng & 0x3f) + 84).expect("model-76 orbit fits");
            let (yaw, pitch, roll, fov, control_flags) = match row_index {
                0 => (
                    wrap_source_angle(512 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0,
                    if column_index == 0 {
                        0x0000_0008
                    } else {
                        0x0080_0000
                    },
                ),
                1 => (
                    512,
                    wrap_source_angle(512 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
                2 => (
                    0,
                    wrap_source_angle(-96 * i16::from(column_index)),
                    0,
                    orbit,
                    if column_index == 0 {
                        0x0000_0008
                    } else {
                        0x0080_0000
                    },
                ),
                3 => (
                    256,
                    wrap_source_angle(256 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
                _ => (
                    768,
                    wrap_source_angle(768 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
            };
            let mut child_position = root_position;
            crate::move_entity(&mut child_position, yaw, pitch, root.word_0x2c);
            let child = Level2Model76Child {
                source_entity_index: child_slot,
                entity_id: root.entity_id,
                entity_rng: child_rng,
                parent_entity_index: source_entity_index,
                parent_entity_id: root.entity_id,
                source_class: 10,
                model: 77,
                action: 84,
                max_life: root.max_life,
                life: root.life,
                subspell_index: root.subspell_index,
                actual_speed: root.actual_speed,
                minimum_speed: root.minimum_speed,
                maximum_speed: root.maximum_speed,
                word_0x2c: root.word_0x2c,
                row_index,
                column_index,
                chain_index: u8::try_from(index).expect("model-76 child index fits"),
                yaw,
                pitch,
                roll,
                fov,
                collision_yaw: 0,
                collision_pitch: 0,
                collision_roll: 0,
                collision_fov: 0,
                shift_rot_x: root.shift_rot_x,
                shift_rot_y: root.shift_rot_y,
                position: child_position,
                phase: root.phase,
                control_flags,
                drawing_enabled: true,
                active: true,
            };
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x: child_position.x,
                    y: child_position.y,
                })
                .is_err()
            {
                *self = state_before;
                return None;
            }
            self.source_rng.insert(child_slot, child_rng);
            self.model76_children.push(child);
        }
        self.authored_model76
            .iter()
            .find(|source| source.source_entity_index == source_entity_index)
            .copied()
    }

    pub(super) fn allocate_model76_tail_spawn(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model76_tail_spawns.push(Level2Model76TailSpawn {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            parent_entity_id,
            source_class: 10,
            model: 0,
            action: 0,
            subspell_index: 400,
            max_life: 8,
            life: 8,
            shift_rot_x: 128,
            shift_rot_y: 128,
            particle_index: 7,
            position,
            phase: source_entity_index as u8,
            control_flags: 0x0002_0000,
            active: true,
        });
        Some(source_entity_index)
    }

    pub(super) fn disable_model76_cluster(&mut self, parent_entity_index: u16) {
        if let Some(parent) = self
            .authored_model76
            .iter_mut()
            .find(|source| source.source_entity_index == parent_entity_index)
        {
            parent.drawing_enabled = false;
            parent.active = false;
        }
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.parent_entity_index == parent_entity_index)
        {
            child.drawing_enabled = false;
            child.active = false;
        }
    }

    /// Advance one Fire Spheres root through the pinned `sub_339B0` root
    /// callback: target-follow/clamp, expanding orbit, child movement order,
    /// terminal `word_0x2c` countdown, model-0 tail spawn, and whole-cluster
    /// disable semantics.
    pub fn step_authored_model76_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self
            .authored_model76
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_model76[source_index];
        let target_live = source.target_entity_index != 0
            && source.target_life >= 0
            && source.target_control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        if source.state == 0 {
            if source.target_entity_index != 0 {
                if !target_live {
                    source.state = 2;
                } else {
                    source.maximum_speed = (source.target_pitch >> 1).max(128);
                    source.minimum_speed = ((6 * source.target_pitch) >> 2).min(640);
                    source.position = source.target_position;
                    source.position.z = source.position.z.wrapping_add(source.target_height_offset);
                    source.state = 1;
                }
            } else {
                source.state = 1;
            }
        }
        if source.state != 2 && source.target_entity_index != 0 {
            if !target_live {
                source.state = 2;
            } else {
                source.position = source.target_position;
                source.position.z = source.position.z.wrapping_add(source.target_height_offset);
            }
        }
        if source.state == 2 {
            if source.font_type_index < 0 {
                source.font_type_index = source.font_type_index.wrapping_neg();
            }
            source.word_0x2c = source
                .word_0x2c
                .wrapping_sub(i16::from(source.font_type_index));
            if source.word_0x2c < 0 {
                let _ = self.allocate_model76_tail_spawn(
                    source_slot,
                    source.entity_id,
                    source.position,
                );
                self.disable_model76_cluster(source_slot);
                self.map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: source_slot,
                        active: false,
                        x: source.position.x,
                        y: source.position.y,
                    })
                    .expect("Level 2 model-76 source slot fits the map chain");
                for child in self
                    .model76_children
                    .iter()
                    .filter(|child| child.parent_entity_index == source_slot)
                {
                    self.map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: child.source_entity_index,
                            active: false,
                            x: child.position.x,
                            y: child.position.y,
                        })
                        .expect("Level 2 model-76 child slot fits the map chain");
                }
                return true;
            }
            source.phase = source.phase.wrapping_add(1);
            self.authored_model76[source_index] = source;
            self.source_rng.insert(source_slot, source.entity_rng);
            return true;
        }

        let ground = terrain.ground_height(source.position);
        let minimum_z = i32::from(ground) + i32::from(source.word_0x2c);
        if i32::from(source.position.z) < minimum_z {
            source.position.z = minimum_z.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
        if let Some(ceiling) = terrain.ceiling_height(source.position) {
            let maximum_z = i32::from(ceiling) - i32::from(source.word_0x2c);
            if i32::from(source.position.z) > maximum_z {
                source.position.z =
                    maximum_z.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            }
        }
        let next_word_0x2c = source
            .word_0x2c
            .wrapping_add(i16::from(source.font_type_index));
        if next_word_0x2c > source.minimum_speed {
            source.word_0x2c = source.minimum_speed;
            source.font_type_index = source.font_type_index.wrapping_neg();
        } else if next_word_0x2c < source.maximum_speed {
            source.word_0x2c = source.maximum_speed;
            source.font_type_index = source.font_type_index.wrapping_neg();
        } else {
            source.word_0x2c = next_word_0x2c;
        }

        source.yaw = wrap_source_angle(source.yaw.wrapping_add(22));
        source.pitch = wrap_source_angle(source.pitch.wrapping_add(16));
        let mut child_sync_rows = Vec::new();
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.active && child.parent_entity_index == source_slot)
        {
            child.yaw = wrap_source_angle(child.yaw.wrapping_add(child.roll));
            child.pitch = wrap_source_angle(child.pitch.wrapping_add(child.fov));
            let mut child_position = source.position;
            crate::move_entity(
                &mut child_position,
                wrap_source_angle(child.yaw.wrapping_add(source.yaw)),
                wrap_source_angle(child.pitch.wrapping_add(source.pitch)),
                source.word_0x2c,
            );
            child.position = child_position;
            child.phase = child.phase.wrapping_add(1);
            child_sync_rows.push((
                child.source_entity_index,
                child.position.x,
                child.position.y,
            ));
        }
        for (child_slot, x, y) in child_sync_rows {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x,
                    y,
                })
                .expect("Level 2 model-76 child slot fits the map chain");
        }
        // Pinned `sub_33C00` probes each column-0 child through the shared
        // `sub_10C80` collision/effect owner.  This Level 2 slice has no
        // generic runtime collision owner for authored model-76 rows yet, so
        // it deliberately records only the source movement/map state here;
        // no synthetic hit or damage event is emitted.
        source.life = source.life.wrapping_sub(1);
        if source.life < 1 {
            source.state = 2;
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model76[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// The linked Fire Spheres children are positioned by the root callback.
    /// Their individual cursor visits are still observable in the live arena,
    /// but have no additional bounded behavior beyond phase retention.
    pub fn step_model76_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child) = self
            .model76_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.phase = child.phase.wrapping_add(1);
        true
    }

    /// Advance the Fire Spheres terminal model-0 tail through the same bounded
    /// life/reclaim seam used for other model-0 rows in this Rust campaign
    /// path. Terrain-side effects remain a separate follow-up.
    pub fn step_model76_tail_spawn_slot(&mut self, source_slot: u16) -> bool {
        let Some(tail) = self
            .model76_tail_spawns
            .iter_mut()
            .find(|tail| tail.active && tail.source_entity_index == source_slot)
        else {
            return false;
        };
        tail.life = tail.life.wrapping_sub(1);
        if tail.life < -1 {
            tail.active = false;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: tail.position.x,
                    y: tail.position.y,
                })
                .expect("Level 2 model-76 tail slot fits the map chain");
            return true;
        }
        tail.phase = tail.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: tail.position.x,
                y: tail.position.y,
            })
            .is_ok()
    }

    /// Advance the generated class-9/model-14 child through `sub_322A0`.
    pub fn step_model6_spawned_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self
            .model6_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model6_spawned_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.active = false;
            child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            self.model6_spawned_children[child_index] = child;
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        child.position.z = child.position.z.wrapping_add(child.actual_speed);
        let ground = terrain.ground_height(child.position);
        if child.position.z < ground {
            child.position.z = ground;
        }
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut child.position, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 16 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 9 {
            child.particle_index -= 1;
        }
        self.model6_spawned_children[child_index] = child;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Dispatch the source callbacks selected by the class-14 constructor.
    /// `sub_59C40_getTerrtoZ` (model 3/action 8) and the non-collecting branch
    /// of `UpdateScroll_59C80` (model 5/action 10) both snap the row to the
    /// current terrain height before their mission-specific work.  The
    /// Level-2 economy owns that shared source write; the enclosing reset
    /// session owns the model-5 player scan and experience presentation after
    /// this callback returns.
    pub fn step_authored_class14_slot(&mut self, source_slot: u16, ground_height: i16) -> bool {
        let Some(source_index) = self
            .authored_class14
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class14[source_index];
        source.position.z = ground_height;
        self.authored_class14[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Reveal the captured class-14 exit portal by clearing the constructor's
    /// source no-draw bit. The portal row remains the sole visibility
    /// authority consumed by presentation.
    pub fn reveal_authored_class14_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self.authored_class14.iter_mut().find(|source| {
            source.source_entity_index == source_slot && source.active && source.model == 3
        }) else {
            return false;
        };
        source.control_flags &= !1;
        true
    }

    /// Apply `DisableEntityDrawing04_57F10` to an authored class-14 row. The
    /// source callback changes only the drawing bit; the row stays allocated
    /// and map-linked until the next UpdateEntities reclaim prologue.
    pub fn deactivate_authored_class14_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_class14
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot && source.active)
        else {
            return false;
        };
        source.drawing_enabled = false;
        true
    }

    /// Allocate a separately dispatched class-10/model-74/action-72 effect.
    /// The caller supplies the already source-derived payload; this method
    /// owns the shared NewEvent slot/RNG, map link, and reclaim identity just
    /// like the other dynamic Level 2 rows. Class-15/action-36 never calls it.
    pub fn allocate_beyond_sight_pulse_effect(
        &mut self,
        holder_entity_index: u16,
        parent_entity_index: u16,
        parent_entity_id: u16,
        subspell_index: u16,
        life: i16,
        effect_parameter: u8,
        actual_speed: i16,
        position: Position,
        axis_position: Position,
        yaw: i16,
        pitch: i16,
    ) -> Option<Level2BeyondSightPulseEffect> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let effect = Level2BeyondSightPulseEffect {
            source_entity_index,
            entity_rng,
            holder_entity_index,
            parent_entity_index,
            parent_entity_id,
            source_class: 10,
            model: 74,
            origin_subtype: 25,
            subspell_index,
            life,
            effect_parameter,
            actual_speed,
            position,
            axis_position,
            yaw,
            pitch,
            sound_id: 9,
            active: true,
        };
        self.source_phase_sidecars
            .insert(source_entity_index, source_entity_index as u8);
        self.source_rng.insert(source_entity_index, entity_rng);
        self.beyond_sight_effects.push(effect);
        Some(effect)
    }

    /// The separate action-72/model-74 effect has no generic movement callback
    /// in this bounded slice; keep its payload live and map-linked for a
    /// future admitted dispatcher without mutating constructor fields.
    pub fn step_beyond_sight_pulse_effect_slot(&mut self, source_slot: u16) -> bool {
        let Some(effect) = self
            .beyond_sight_effects
            .iter()
            .find(|effect| effect.active && effect.source_entity_index == source_slot)
            .copied()
        else {
            return false;
        };
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: effect.position.x,
                y: effect.position.y,
            })
            .is_ok()
    }

    /// Advance one class-10/model-54/69 temporary field through
    /// `sub_38D80`. Eligible mana spheres receive one source-linked attraction
    /// mailbox in source order; an already-targeted sphere is left untouched.
    pub fn step_mana_attraction_slot(&mut self, source_slot: u16) -> bool {
        self.step_mana_attraction_slot_with_membership(source_slot, None)
    }

    /// Advance one attraction field using the model-39 membership rebuilt at
    /// the beginning of the ordinary `UpdateEntities_57730` pass. New spheres
    /// allocated during that pass remain eligible for their own higher-slot
    /// callbacks, but cannot become field candidates until the next pass
    /// rebuilds the source-linked membership list.
    pub fn step_mana_attraction_slot_with_sphere_membership(
        &mut self,
        source_slot: u16,
        sphere_membership: &BTreeSet<u16>,
    ) -> bool {
        self.step_mana_attraction_slot_with_membership(source_slot, Some(sphere_membership))
    }

    pub(super) fn step_mana_attraction_slot_with_membership(
        &mut self,
        source_slot: u16,
        sphere_membership: Option<&BTreeSet<u16>>,
    ) -> bool {
        let Some(field_index) = self
            .mana_attraction_fields
            .iter()
            .position(|field| field.active && field.source_entity_index == source_slot)
        else {
            return false;
        };
        let (position, radius_squared, action, life_before) = {
            let field = self.mana_attraction_fields[field_index];
            (
                field.position,
                field.radius_squared,
                field.action,
                field.life,
            )
        };
        debug_assert!(matches!(
            action,
            MANA_ATTRACTION_ACTION | MANA_ATTRACTION_ALIAS_ACTION
        ));

        // `sub_38D80` decrements before testing the signed life sentinel. A
        // zero-life row therefore gets one final producer pass; the next
        // cursor visit disables it and returns the slot to the arena stack.
        if let Some(field) = self.mana_attraction_fields.get_mut(field_index) {
            field.life = field.life.saturating_sub(1);
            if life_before < 0 {
                field.active = false;
                return true;
            }
        }

        for sphere in self.spheres.iter_mut().filter(|sphere| {
            sphere.active
                && sphere.attraction_source_entity_index == 0
                && sphere_membership.map_or(true, |membership| {
                    membership.contains(&sphere.source_entity_index)
                })
        }) {
            let distance_squared = level2_wrapping_xy_distance(position, sphere.position);
            if distance_squared < radius_squared {
                let distance = crate::legacy_sqrt(distance_squared);
                sphere.attraction_step =
                    u16::try_from(distance.min(42)).expect("Level 2 attraction step fits u16");
                sphere.attraction_distance_high = u16::try_from(distance >> 16)
                    .expect("Level 2 attraction distance high word fits u16");
                sphere.attraction_source_entity_index = source_slot;
            }
        }
        true
    }

    /// Allocate the source class-15 row created by `sub_69250`. This uses the
    /// same free stack, constructor RNG, map chain, and failure transaction as
    /// every other Level 2 `NewEvent_4A050` allocation.
    pub fn allocate_spell_replacement(
        &mut self,
        family: u8,
        parent_entity_index: u16,
        position: Position,
    ) -> Option<Level2SpellReplacement> {
        if family >= 26 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let replacement = Level2SpellReplacement {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            position,
            family,
            action: family.saturating_mul(3).saturating_add(2),
            phase: source_entity_index as u8,
            control_flags: 0,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spell_replacements.push(replacement);
        Some(replacement)
    }

    /// Convert one persistent pickup source into its holder action after the
    /// shared `sub_68FF0` transition. The source row remains in the live
    /// arena (the new replacement is the row that carries the persistent
    /// action), so the cursor can still observe its phase/map writes.
    pub fn mark_spell_replacement_acquired(
        &mut self,
        source_slot: u16,
        holder_action: u8,
    ) -> Option<Position> {
        let replacement = self.spell_replacements.iter_mut().find(|replacement| {
            replacement.active && replacement.source_entity_index == source_slot
        })?;
        replacement.action = holder_action;
        Some(replacement.position)
    }

    /// The family callback remains outside this bounded allocator slice. The
    /// source cursor still visits the live row and preserves source phase/map
    /// state; no invented spell effect is emitted here.
    pub fn step_spell_replacement_slot(&mut self, source_slot: u16) -> bool {
        let Some(replacement) = self.spell_replacements.iter_mut().find(|replacement| {
            replacement.active && replacement.source_entity_index == source_slot
        }) else {
            return false;
        };
        replacement.phase = replacement.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: replacement.position.x,
                y: replacement.position.y,
            })
            .is_ok()
    }

    #[must_use]
    pub fn carrier(&self) -> Option<&ManaCarrier> {
        self.carriers.iter().find(|carrier| carrier.active)
    }

    /// Return source-carrier rows in allocation order, including inactive
    /// rows that remain until the source reclaim pass. The legacy `carrier()`
    /// accessor remains as a single-row active compatibility view for callers
    /// that only render the first live carrier.
    #[must_use]
    pub fn carriers(&self) -> &[ManaCarrier] {
        &self.carriers
    }

    #[must_use]
    pub const fn castle(&self) -> Option<Level2CastleState> {
        self.castle
    }

    #[cfg(test)]
    pub(crate) fn set_castle_mana_prerequisite_for_test(
        &mut self,
        castle_slot: u16,
        mana: i32,
    ) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        // This cfg(test)-only seam stages the complete upstream Castle-mana
        // prerequisite. Preserve the injected payload through the ordinary
        // Castle upkeep callback instead of letting a level-one capacity cap
        // turn a downstream objective/gate test into a Castle-upgrade test.
        castle.mana_capacity = castle.mana_capacity.max(mana);
        castle.mana = mana;
        true
    }

    #[must_use]
    pub fn castle_projectiles(&self) -> &[Level2CastleProjectile] {
        &self.castle_projectiles
    }

    #[must_use]
    pub fn possession_projectiles(&self) -> &[Level2PossessionProjectile] {
        &self.possession_projectiles
    }

    #[must_use]
    pub fn fireball_projectiles(&self) -> &[Level2FireballProjectile] {
        &self.fireball_projectiles
    }

    #[must_use]
    pub fn possession_effects(&self) -> &[Level2PossessionEffect] {
        &self.possession_effects
    }

    #[must_use]
    pub fn player_castle_shift_aabb_overlap(&self, player: &EntitySnapshot) -> bool {
        let Some(castle) = self.castle.filter(|castle| {
            castle.active && self.player_castle_entity_index == castle.source_entity_index
        }) else {
            return false;
        };
        level2_model76_sub106c0_matches(
            castle.position,
            castle.shift_yaw,
            castle.shift_pitch,
            castle.shift_roll,
            castle.shift_fov,
            player.position,
            player.array_yaw(),
            player.array_pitch(),
            player.array_roll(),
            player.array_fov(),
        )
    }

    #[must_use]
    pub fn castle_children(&self) -> &[Level2CastleChild] {
        &self.castle_children
    }

    #[must_use]
    pub(crate) fn slot_generation(&self, slot: u16) -> u32 {
        self.slot_generations
            .get(&slot)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn source_phase_sidecar(&self, slot: u16) -> Option<u8> {
        self.source_phase_sidecars.get(&slot).copied()
    }

    pub(crate) fn advance_source_phase_sidecar(&mut self, slot: u16) -> Option<u8> {
        let phase = self.source_phase_sidecars.get_mut(&slot)?;
        *phase = phase.wrapping_add(1);
        Some(*phase)
    }

    pub(super) fn for_each_live_arena_slot(
        &self,
        mut visit: impl FnMut(u16, Level2DynamicArenaKind),
    ) {
        for projectile in self
            .castle_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::CastleProjectile,
            );
        }
        for projectile in self
            .fireball_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::FireballProjectile,
            );
        }
        for projectile in self
            .possession_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::PossessionProjectile,
            );
        }
        for effect in self
            .possession_effects
            .iter()
            .filter(|effect| effect.active)
        {
            visit(
                effect.source_entity_index,
                Level2DynamicArenaKind::PossessionEffect,
            );
        }
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            visit(
                castle.source_entity_index,
                Level2DynamicArenaKind::CastleRoot,
            );
        }
        for child in self.castle_children.iter().filter(|child| child.active) {
            visit(
                child.source_entity_index,
                match child.model {
                    15 => Level2DynamicArenaKind::CastleAuxiliary,
                    3 => Level2DynamicArenaKind::CastleDefender,
                    79 => Level2DynamicArenaKind::CastleController,
                    42 | 41 => Level2DynamicArenaKind::CastleChild,
                    _ => Level2DynamicArenaKind::CastleChild,
                },
            );
        }
        for sphere in self.spheres.iter().filter(|sphere| sphere.active) {
            visit(
                sphere.source_entity_index,
                Level2DynamicArenaKind::ManaSphere,
            );
        }
        for burst in self.death_bursts.iter().filter(|burst| burst.active) {
            visit(
                burst.source_entity_index,
                Level2DynamicArenaKind::DeathBurst,
            );
        }
        for marker in self
            .player_death_markers
            .iter()
            .filter(|marker| marker.active)
        {
            visit(
                marker.source_entity_index,
                Level2DynamicArenaKind::PlayerDeathMarker,
            );
        }
        for carrier in self.carriers.iter().filter(|carrier| carrier.active) {
            visit(
                carrier.source_entity_index,
                Level2DynamicArenaKind::ManaCarrier,
            );
        }
        for replacement in self
            .spell_replacements
            .iter()
            .filter(|replacement| replacement.active)
        {
            visit(
                replacement.source_entity_index,
                Level2DynamicArenaKind::SpellReplacement,
            );
        }
        for source in self.spell_pickups.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::SpellPickup,
            );
        }
        for trail in self.speed_trails.iter().filter(|trail| trail.active) {
            visit(
                trail.source_entity_index,
                Level2DynamicArenaKind::SpeedTrail,
            );
        }
        for source in self
            .terrain_strip_close_triggers
            .iter()
            .filter(|source| source.active)
        {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::TerrainStripCloseTrigger,
            );
        }
        for source in self.authored_model1.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel1,
            );
        }
        for source in self.authored_model45.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel45,
            );
        }
        for source in self.authored_model13.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel13,
            );
        }
        for source in self.bee_creatures.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::BeeCreature,
            );
        }
        for source in self
            .authored_switch_producers
            .iter()
            .filter(|source| source.active)
        {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredSwitchProducer,
            );
        }
        for source in self.authored_trees.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredTree,
            );
        }
        for source in self.authored_model32.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel32,
            );
        }
        for source in self.authored_model59.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel59,
            );
        }
        for child in self
            .model59_smoke_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model59SmokeChild,
            );
        }
        for source in self.authored_model53.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel53,
            );
        }
        for source in self
            .authored_model5_water_splashes
            .iter()
            .filter(|source| source.active)
        {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel5WaterSplash,
            );
        }
        for child in self
            .model13_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model13SpawnedChild,
            );
        }
        for source in self.authored_model6.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel6,
            );
        }
        for child in self
            .model6_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model6SpawnedChild,
            );
        }
        for source in self.authored_model17.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel17,
            );
        }
        for child in self
            .model17_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model17SpawnedChild,
            );
        }
        for source in self.authored_model22.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel22,
            );
        }
        for child in self
            .model22_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model22SpawnedChild,
            );
        }
        for source in self.authored_model76.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel76,
            );
        }
        for child in self.model76_children.iter().filter(|child| child.active) {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model76Child,
            );
        }
        for tail in self.model76_tail_spawns.iter().filter(|tail| tail.active) {
            visit(
                tail.source_entity_index,
                Level2DynamicArenaKind::Model76TailSpawn,
            );
        }
        for source in self.authored_class14.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass14,
            );
        }
        for source in self.authored_class5.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5,
            );
        }
        for child in self
            .authored_dragon_parts
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::AuthoredDragonPart,
            );
        }
        for child in self
            .authored_class5_archer_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5ArcherChild,
            );
        }
        for child in self
            .authored_class5_model3_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5Model3Child,
            );
        }
        for source in self.house_residents.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::HouseResident,
            );
        }
        for effect in self
            .beyond_sight_effects
            .iter()
            .filter(|effect| effect.active)
        {
            visit(
                effect.source_entity_index,
                Level2DynamicArenaKind::BeyondSightPulseEffect,
            );
        }
        for field in self
            .mana_attraction_fields
            .iter()
            .filter(|field| field.active)
        {
            visit(
                field.source_entity_index,
                Level2DynamicArenaKind::ManaAttraction,
            );
        }
    }

    /// Return the currently live dynamic source rows in ascending source-slot
    /// order. This is the bounded arena-cursor seam: allocations and releases
    /// are observed from live projections on every pass rather than captured
    /// once at reset. Per-object movement remains owned by the economy bead.
    #[must_use]
    pub fn live_arena_slots(&self) -> Vec<(u16, Level2DynamicArenaKind)> {
        let mut slots = BTreeMap::new();
        self.for_each_live_arena_slot(|slot, kind| {
            slots.insert(slot, kind);
        });
        slots.into_iter().collect()
    }

    #[must_use]
    pub(super) fn live_arena_cursor_rows(&self) -> Vec<Level2DynamicArenaCursorRow> {
        let mut rows = BTreeMap::new();
        self.for_each_live_arena_slot(|slot, kind| {
            rows.insert(
                slot,
                Level2DynamicArenaCursorRow {
                    source_slot: slot,
                    generation: self.slot_generation(slot),
                    kind,
                },
            );
        });
        rows.into_values().collect()
    }

    /// Read the live source action-table selector for one typed arena row.
    ///
    /// `UpdateEntities_57730` reads class/action from the fixed numeric row at
    /// the moment its cursor reaches that row.  Keeping this lookup beside the
    /// carrier census prevents the campaign scheduler from dispatching a
    /// pass-start classification after an earlier callback changed the row.
    #[must_use]
    pub(super) fn live_arena_class_action(
        &self,
        source_slot: u16,
        kind: Level2DynamicArenaKind,
    ) -> Option<(u8, u8)> {
        macro_rules! selector {
            ($rows:expr, $class:expr) => {
                $rows
                    .iter()
                    .find(|row| row.active && row.source_entity_index == source_slot)
                    .map(|row| ($class, row.action))
            };
            ($rows:expr) => {
                $rows
                    .iter()
                    .find(|row| row.active && row.source_entity_index == source_slot)
                    .map(|row| (row.source_class, row.action))
            };
        }

        match kind {
            Level2DynamicArenaKind::CastleProjectile => self
                .castle_projectiles
                .iter()
                .any(|row| row.active && row.source_entity_index == source_slot)
                .then_some((9, 10)),
            Level2DynamicArenaKind::FireballProjectile => {
                selector!(self.fireball_projectiles)
            }
            Level2DynamicArenaKind::PossessionProjectile => {
                selector!(self.possession_projectiles)
            }
            Level2DynamicArenaKind::PossessionEffect => selector!(self.possession_effects),
            Level2DynamicArenaKind::CastleRoot => self
                .castle
                .filter(|row| row.active && row.source_entity_index == source_slot)
                .map(|row| (row.source_class, row.action)),
            Level2DynamicArenaKind::CastleChild
            | Level2DynamicArenaKind::CastleAuxiliary
            | Level2DynamicArenaKind::CastleDefender
            | Level2DynamicArenaKind::CastleController => selector!(self.castle_children),
            Level2DynamicArenaKind::ManaSphere => self
                .spheres
                .iter()
                .any(|row| row.active && row.source_entity_index == source_slot)
                .then_some((10, 41)),
            Level2DynamicArenaKind::ManaCarrier => self
                .carriers
                .iter()
                .any(|row| row.active && row.source_entity_index == source_slot)
                .then_some((3, 9)),
            Level2DynamicArenaKind::DeathBurst => self
                .death_bursts
                .iter()
                .any(|row| row.active && row.source_entity_index == source_slot)
                .then_some((10, 1)),
            Level2DynamicArenaKind::PlayerDeathMarker => self
                .player_death_markers
                .iter()
                .any(|row| row.active && row.source_entity_index == source_slot)
                .then_some((10, 42)),
            Level2DynamicArenaKind::SpellReplacement => {
                selector!(self.spell_replacements, 15)
            }
            Level2DynamicArenaKind::SpellPickup => selector!(self.spell_pickups, 15),
            Level2DynamicArenaKind::SpeedTrail => self
                .speed_trails
                .iter()
                .find(|row| row.active && row.source_entity_index == source_slot)
                .map(|row| (row.class, row.action)),
            Level2DynamicArenaKind::TerrainStripCloseTrigger => self
                .terrain_strip_close_triggers
                .iter()
                .find(|row| row.active && row.source_entity_index == source_slot)
                .map(|row| (row.class, row.action)),
            Level2DynamicArenaKind::AuthoredModel1 => selector!(self.authored_model1, 10),
            Level2DynamicArenaKind::AuthoredModel45 => selector!(self.authored_model45),
            Level2DynamicArenaKind::AuthoredModel13 => selector!(self.authored_model13),
            Level2DynamicArenaKind::BeeCreature => selector!(self.bee_creatures),
            Level2DynamicArenaKind::Model13SpawnedChild => {
                selector!(self.model13_spawned_children)
            }
            Level2DynamicArenaKind::AuthoredModel6 => selector!(self.authored_model6, 10),
            Level2DynamicArenaKind::Model6SpawnedChild => {
                selector!(self.model6_spawned_children)
            }
            Level2DynamicArenaKind::AuthoredModel17 => selector!(self.authored_model17),
            Level2DynamicArenaKind::Model17SpawnedChild => {
                selector!(self.model17_spawned_children)
            }
            Level2DynamicArenaKind::AuthoredModel22 => selector!(self.authored_model22),
            Level2DynamicArenaKind::Model22SpawnedChild => {
                selector!(self.model22_spawned_children)
            }
            Level2DynamicArenaKind::AuthoredModel76 => selector!(self.authored_model76),
            Level2DynamicArenaKind::Model76Child => selector!(self.model76_children),
            Level2DynamicArenaKind::Model76TailSpawn => {
                selector!(self.model76_tail_spawns)
            }
            Level2DynamicArenaKind::AuthoredClass14 => selector!(self.authored_class14, 14),
            Level2DynamicArenaKind::AuthoredClass5 => selector!(self.authored_class5, 5),
            Level2DynamicArenaKind::AuthoredDragonPart => {
                selector!(self.authored_dragon_parts, 5)
            }
            Level2DynamicArenaKind::AuthoredClass5ArcherChild => {
                selector!(self.authored_class5_archer_children)
            }
            Level2DynamicArenaKind::AuthoredClass5Model3Child => {
                selector!(self.authored_class5_model3_children, 5)
            }
            Level2DynamicArenaKind::HouseResident => selector!(self.house_residents, 5),
            Level2DynamicArenaKind::AuthoredSwitchProducer => {
                selector!(self.authored_switch_producers)
            }
            Level2DynamicArenaKind::AuthoredTree => selector!(self.authored_trees),
            Level2DynamicArenaKind::AuthoredModel32 => selector!(self.authored_model32),
            Level2DynamicArenaKind::AuthoredModel59 => selector!(self.authored_model59),
            Level2DynamicArenaKind::Model59SmokeChild => {
                selector!(self.model59_smoke_children)
            }
            Level2DynamicArenaKind::AuthoredModel53 => selector!(self.authored_model53),
            Level2DynamicArenaKind::AuthoredModel5WaterSplash => {
                selector!(self.authored_model5_water_splashes)
            }
            Level2DynamicArenaKind::BeyondSightPulseEffect => self
                .beyond_sight_effects
                .iter()
                .find(|row| row.active && row.source_entity_index == source_slot)
                .map(|row| (row.source_class, 72)),
            Level2DynamicArenaKind::ManaAttraction => {
                selector!(self.mana_attraction_fields, 10)
            }
        }
    }

    #[must_use]
    pub const fn map_chains(&self) -> &RenderableMapChains {
        &self.map_chains
    }

    #[must_use]
    pub const fn player_mana(&self) -> i32 {
        self.player_mana
    }

    #[must_use]
    pub const fn player_castle_entity_index(&self) -> u16 {
        self.player_castle_entity_index
    }

    #[must_use]
    pub const fn player_castle_stage(&self) -> u8 {
        self.player_castle_stage
    }

    #[must_use]
    pub const fn player_castle_aux_flags(&self) -> u8 {
        self.player_castle_aux_flags
    }

    #[must_use]
    pub const fn player_spell_slot_2(&self) -> u16 {
        self.player_spell_slot_2
    }

    #[must_use]
    pub const fn player_spell_holder_action(&self) -> u16 {
        self.player_spell_holder_action
    }

    /// Bind the source conditions used by `sub_605E0` when the final Castle
    /// stage is removed. The reset package normally leaves this disabled; the
    /// setter lets an imported player/terrain projection opt into the exact
    /// model-1, terrain-bit-4 holder cleanup without conflating it with the
    /// economy owner relation.
    pub fn set_player_spell_cleanup_context(
        &mut self,
        player_model: u8,
        terrain_flags: u8,
        holder_slot: u16,
    ) {
        self.player_model = player_model;
        self.player_terrain_flags = terrain_flags;
        self.player_spell_slot_2 = holder_slot;
        self.player_spell_holder_action = 0;
        self.player_spell_holder_max_action = 1;
    }

    /// Bind the preserved holder `word_0x2E`/`word_0x30` pair when a caller
    /// supplies a synthetic player cleanup context.  The ordinary setter
    /// above remains a compact compatibility helper for callers that only
    /// know the holder relation.
    pub fn set_player_spell_holder_counters(&mut self, action: u16, max_action: u16) {
        self.player_spell_holder_action = action;
        self.player_spell_holder_max_action = max_action;
    }

    pub(super) fn initialize_player_spell_cleanup_context(
        &mut self,
        player_model: u8,
        terrain_flags: u8,
        holder_slot: u16,
        action: u16,
        max_action: u16,
    ) {
        self.player_model = player_model;
        self.player_terrain_flags = terrain_flags;
        self.player_spell_slot_2 = holder_slot;
        self.player_spell_holder_action = action;
        self.player_spell_holder_max_action = max_action;
    }

    #[must_use]
    pub const fn free_slot_count(&self) -> usize {
        self.free_slots.len()
    }

    #[must_use]
    pub fn events(&self) -> &[Level2EconomyEvent] {
        &self.events
    }

    /// Record one source-shaped `PrepareEventSound_6E450` call. The handler
    /// slot and the exact entity argument are separate because house and
    /// switch callbacks prepare sound on an owner/player row rather than on
    /// their triggering source row.
    pub(super) fn prepare_sound(
        &mut self,
        source_entity_index: u16,
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    ) {
        self.events.push(Level2EconomyEvent::PreparedSound {
            source_entity_index,
            entity_index,
            player_index,
            sound_id,
        });
    }

    /// Start a new source turn's economy event stream. Mutations append to
    /// this stream so one turn can be audited in source order.
    pub fn begin_turn(&mut self) {
        self.events.clear();
    }

    /// Override the imported owner capacity used by the source's
    /// `sub_36D50` merge arbitration when two non-neutral owners collide.
    pub fn set_owner_max_mana(&mut self, owner_entity_index: u16, max_mana: i32) {
        self.owner_max_mana.insert(owner_entity_index, max_mana);
    }

    /// Refresh the current class/model-filtered source rows after a typed
    /// economy mutation. Baseline rows retain the last source-boundary
    /// relation; the owning pass below is the only place that adopts newly
    /// written `source_owner` relations. The published aggregate and global
    /// counter remain frozen until that boundary as they do in `sub_60F00`.
    pub(super) fn refresh_owner_mana_rows(&mut self) {
        let imported_source_slots = self
            .owner_mana_rows
            .iter()
            .filter(|row| !self.slot_generations.contains_key(&row.source_entity_index))
            .map(|row| row.source_entity_index)
            .collect::<BTreeSet<_>>();
        let mut active_dynamic_source_slots = BTreeSet::new();
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            active_dynamic_source_slots.insert(castle.source_entity_index);
        }
        active_dynamic_source_slots.extend(
            self.castle_children
                .iter()
                .filter(|child| child.active)
                .map(|child| child.source_entity_index),
        );
        active_dynamic_source_slots.extend(
            self.spheres
                .iter()
                .filter(|sphere| sphere.active)
                .map(|sphere| sphere.source_entity_index),
        );
        active_dynamic_source_slots.extend(
            self.carriers
                .iter()
                .filter(|carrier| carrier.active)
                .map(|carrier| carrier.source_entity_index),
        );
        self.owner_mana_rows.retain(|row| {
            imported_source_slots.contains(&row.source_entity_index)
                || active_dynamic_source_slots.contains(&row.source_entity_index)
        });
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            self.install_owner_mana_row(
                castle.source_entity_index,
                castle.source_class,
                castle.source_model,
                castle.source_id_entity_index,
                castle.mana,
            );
        }
        let children = self
            .castle_children
            .iter()
            .filter(|child| child.active)
            .copied()
            .collect::<Vec<_>>();
        for child in children {
            self.install_owner_mana_row(
                child.source_entity_index,
                child.source_class,
                child.model,
                child.player_entity_index,
                child.mana,
            );
        }
        let spheres = self
            .spheres
            .iter()
            .filter(|sphere| sphere.active)
            .copied()
            .collect::<Vec<_>>();
        for sphere in spheres {
            self.install_owner_mana_row(
                sphere.source_entity_index,
                10,
                39,
                sphere.owner_entity_index,
                sphere.mana,
            );
        }
        let carriers = self
            .carriers
            .iter()
            .filter(|carrier| carrier.active)
            .copied()
            .collect::<Vec<_>>();
        for carrier in carriers {
            self.install_owner_mana_row(
                carrier.source_entity_index,
                3,
                3,
                carrier.owner_entity_index,
                carrier.carried_mana,
            );
        }
        self.owner_mana_rows
            .sort_unstable_by_key(|row| row.source_entity_index);
    }

    pub(super) fn publish_owner_mana_projection(&mut self) {
        let (aggregate, global) = level2_owner_mana_projection(&self.owner_mana_rows);
        self.owner_mana_aggregate = aggregate;
        self.owner_mana_global = global;
    }

    /// Run the source `sub_60F00` owner-row reset at the owning-pass
    /// boundary. House callbacks update `source_owner` during the cursor;
    /// those writes become visible only at this next boundary, while typed
    /// sphere/carrier callbacks continue to refresh their own rows without
    /// retroactively changing the frozen baseline relation for the pass.
    pub(super) fn rebuild_owner_mana_projection_at_source_boundary(&mut self) {
        self.owner_mana_rows = self
            .owner_mana_rows
            .iter()
            .filter(|row| !self.slot_generations.contains_key(&row.source_entity_index))
            .filter_map(|row| {
                self.source_owner
                    .get(&row.source_entity_index)
                    .map(|&owner| Level2OwnerManaRow {
                        player_entity_index: owner,
                        ..*row
                    })
            })
            .collect();
        self.refresh_owner_mana_rows();
        self.publish_owner_mana_projection();
    }

    /// Run the source `sub_60F00` max-mana reset/sink at its owning-pass
    /// boundary. Generic sphere/carrier mutations refresh the underlying rows
    /// but must not publish aggregate/global/capacity earlier than this turn.
    pub(super) fn rebuild_owner_max_mana_projection(
        &mut self,
        reset_owner_max_mana: &BTreeMap<u16, i32>,
    ) {
        self.owner_max_mana =
            level2_owner_max_mana_projection(reset_owner_max_mana, &self.owner_mana_rows);
    }

    #[must_use]
    pub fn owner_mana_aggregate(&self, owner_entity_index: u16) -> i32 {
        self.owner_mana_aggregate
            .get(&owner_entity_index)
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub const fn owner_mana_global(&self) -> i32 {
        self.owner_mana_global
    }

    /// Bind one imported `sub_60810` life coefficient. `256` is the source
    /// identity coefficient; callers with the preserved nested player-state
    /// bytes can provide the exact value for each stage.
    pub fn set_castle_stage_coefficient(&mut self, level: u8, coefficient: u16) -> bool {
        let Some(value) = self.castle_stage_coefficients.get_mut(usize::from(level)) else {
            return false;
        };
        *value = coefficient;
        true
    }

    /// Bind the raw player `array_0x24E[9 + level]` byte copied to each
    /// model-79 controller by source `sub_613D0`.
    pub fn set_castle_controller_coefficient(&mut self, level: u8, coefficient: u8) -> bool {
        let Some(value) = self
            .castle_controller_coefficients
            .get_mut(usize::from(level))
        else {
            return false;
        };
        *value = coefficient;
        true
    }

    /// Bind a defender's source `byte_0x3E_62` cadence phase. The source
    /// stores this byte on the child entity, independently of Castle phase;
    /// callers without an imported byte use the Castle phase at allocation.
    pub fn set_castle_defender_source_phase(&mut self, slot: u16, phase: u8) -> bool {
        let Some(child) = self.castle_children.iter_mut().find(|child| {
            child.active
                && child.source_entity_index == slot
                && child.model == 3
                && child.action == 9
        }) else {
            return false;
        };
        child.source_byte_3e = phase;
        true
    }

    /// Rebuild `UpdateEntities_57730`'s model-39 guidance membership once at
    /// the source-pass boundary. Same-pass births are deliberately absent;
    /// callbacks resolve these fixed slots against the current record.
    pub(super) fn rebuild_possession_guidance_source_slots(&mut self) {
        self.possession_guidance_source_slots = self
            .spheres
            .iter()
            .filter(|sphere| sphere.active || sphere.pending_delete)
            .map(|sphere| sphere.source_entity_index)
            .collect();
        self.possession_guidance_source_slots.sort_unstable();
        self.possession_guidance_source_slots.dedup();
    }

    pub fn reclaim_inactive_slots(&mut self) {
        self.reclaim_inactive_slots_with_policy(true);
    }

    /// Reclaim inactive rows at the following `UpdateEntities_57730`
    /// prologue. Callback-hidden rows remain allocated and map-linked through
    /// the remainder of their dispatch pass.
    pub(super) fn reclaim_inactive_slots_at_source_boundary(&mut self) {
        self.reclaim_inactive_slots_with_policy(true);
    }

    /// Reclaim a phase-zero Bee hidden during the preceding source pass. This
    /// is deliberately before the next cursor snapshot and after the prior
    /// pass's event tail, matching the source's following-pass `sub_57F20`.
    pub(super) fn reclaim_deferred_bee_creatures_at_source_boundary(&mut self) {
        let reclaimed_bees = self
            .bee_creatures
            .iter()
            .filter(|source| {
                !source.drawing_enabled
                    && self
                        .deferred_bee_reclaims
                        .contains(&source.source_entity_index)
            })
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.bee_creatures.retain(|source| {
            source.drawing_enabled
                || !self
                    .deferred_bee_reclaims
                    .contains(&source.source_entity_index)
        });
        for (slot, entity_rng) in reclaimed_bees {
            self.deferred_bee_reclaims.remove(&slot);
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
    }

    #[must_use]
    pub(super) fn is_deferred_bee_reclaim(&self, source_slot: u16) -> bool {
        self.deferred_bee_reclaims.contains(&source_slot)
    }

    /// Reclaim Archer roots and class-9/model-13 children disabled during the
    /// preceding UpdateEntities scan. Invalid child selector 9:37 and the
    /// root KillArcher callback both leave their numeric rows allocated for
    /// the remainder of the pass.
    pub(super) fn reclaim_hidden_archer_rows_at_source_boundary(&mut self) {
        let hidden_roots = self
            .authored_class5
            .iter()
            .filter(|source| {
                source.active
                    && source.model == level2_class5::archer::ARCHER_MODEL
                    && !source.drawing_enabled
            })
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5.retain(|source| {
            source.model != level2_class5::archer::ARCHER_MODEL || source.drawing_enabled
        });
        self.authored_class5_archer.retain(|state| {
            !hidden_roots
                .iter()
                .any(|(source_slot, _)| *source_slot == state.source_entity_index)
        });

        let hidden_children = self
            .authored_class5_archer_children
            .iter()
            .filter(|child| child.active && !child.drawing_enabled)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_archer_children
            .retain(|child| child.drawing_enabled);

        for (slot, entity_rng) in hidden_roots.into_iter().chain(hidden_children) {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Reclaim Worm roots and bodies whose independent action-29 callback (or
    /// invalid predecessor check) disabled drawing during the prior source
    /// pass. The fixed rows remain active and map-linked through their common
    /// epilogue, then this following-boundary service unlinks and releases
    /// them in numeric-slot order.
    pub(super) fn reclaim_hidden_worm_rows_at_source_boundary(&mut self) {
        let hidden_roots = self
            .authored_class5
            .iter()
            .filter(|source| source.active && source.model == 3 && !source.drawing_enabled)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5
            .retain(|source| source.model != 3 || source.drawing_enabled);
        self.authored_class5_model3.retain(|state| {
            !hidden_roots
                .iter()
                .any(|(slot, _)| *slot == state.source_entity_index)
        });

        let hidden_children = self
            .authored_class5_model3_children
            .iter()
            .filter(|child| child.active && !child.drawing_enabled)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_model3_children
            .retain(|child| child.drawing_enabled);

        let mut hidden = hidden_roots
            .into_iter()
            .chain(hidden_children)
            .collect::<Vec<_>>();
        hidden.sort_unstable_by_key(|(slot, _)| *slot);
        for (slot, entity_rng) in hidden {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Apply UpdateEntities' invalid-selector branch to the retained Archer
    /// projectile. There is no callback and therefore no phase/byte-0x3E
    /// epilogue on this visit.
    pub(super) fn hide_authored_class5_archer_child_invalid_selector(
        &mut self,
        source_slot: u16,
    ) -> bool {
        let Some(child) = self
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.drawing_enabled = false;
        true
    }

    fn reclaim_inactive_slots_with_policy(&mut self, defer_bee_creatures: bool) {
        // Castle projectiles are disabled by their source callback and
        // reclaimed only by reclaim_hidden_castle_rows_at_source_boundary at
        // the following UpdateEntities prologue. Same-pass reclamation would
        // erase a blocked flyer before the source cursor can observe the
        // hidden row's reserved slot.
        let reclaimed_carriers = self
            .carriers
            .iter()
            .filter(|carrier| !carrier.active)
            .map(|carrier| (carrier.source_entity_index, carrier.entity_rng))
            .collect::<Vec<_>>();
        self.carriers.retain(|carrier| carrier.active);
        for (slot, entity_rng) in reclaimed_carriers {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed = self
            .spheres
            .iter()
            .filter(|sphere| !sphere.active || sphere.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
            .map(|sphere| {
                (
                    sphere.source_entity_index,
                    sphere.entity_rng,
                    level2_possession_guidance_target_top(sphere),
                )
            })
            .collect::<Vec<_>>();
        self.spheres
            .retain(|sphere| sphere.active && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT == 0);
        for (slot, entity_rng, target_top) in reclaimed {
            // `sub_57F20` frees the slot but leaves its fixed entity record in
            // place until `NewEvent` overwrites it. An already-guided flyer
            // therefore continues `sub_65610` against this stale transform.
            self.possession_guidance_stale_records
                .insert(slot, target_top);
            self.release_slot(slot, Some(entity_rng));
        }
        // A model-1 burst disabled by its callback remains reserved for the
        // rest of that source pass. The next pass prologue owns its unlink
        // and release through reclaim_death_burst_rows_at_source_boundary.
        let reclaimed_spell_replacements = self
            .spell_replacements
            .iter()
            .filter(|replacement| !replacement.active)
            .map(|replacement| (replacement.source_entity_index, replacement.entity_rng))
            .collect::<Vec<_>>();
        self.spell_replacements
            .retain(|replacement| replacement.active);
        for (slot, entity_rng) in reclaimed_spell_replacements {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_spell_pickups = self
            .spell_pickups
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.spell_pickups.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_spell_pickups {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_speed_trails = self
            .speed_trails
            .iter()
            .filter(|trail| !trail.active)
            .map(|trail| (trail.source_entity_index, trail.entity_rng))
            .collect::<Vec<_>>();
        self.speed_trails.retain(|trail| trail.active);
        for (slot, entity_rng) in reclaimed_speed_trails {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_terrain_strip_triggers = self
            .terrain_strip_close_triggers
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.terrain_strip_close_triggers
            .retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_terrain_strip_triggers {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model1 = self
            .authored_model1
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model1.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model1 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model45 = self
            .authored_model45
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model45.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model45 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model13 = self
            .authored_model13
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model13.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model13 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_bees = self
            .bee_creatures
            .iter()
            .filter(|source| {
                !source.active
                    && (!defer_bee_creatures
                        || !self
                            .deferred_bee_reclaims
                            .contains(&source.source_entity_index))
            })
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.bee_creatures.retain(|source| {
            source.active
                || (defer_bee_creatures
                    && self
                        .deferred_bee_reclaims
                        .contains(&source.source_entity_index))
        });
        for (slot, entity_rng) in reclaimed_bees {
            self.deferred_bee_reclaims.remove(&slot);
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_switch_producers = self
            .authored_switch_producers
            .iter()
            .filter(|source| !source.active || !source.drawing_enabled)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_switch_producers
            .retain(|source| source.active && source.drawing_enabled);
        for (slot, entity_rng) in reclaimed_switch_producers {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_trees = self
            .authored_trees
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_trees.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_trees {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model32 = self
            .authored_model32
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model32.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model32 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model59 = self
            .authored_model59
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model59.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model59 {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model59_smoke_children = self
            .model59_smoke_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model59_smoke_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model59_smoke_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model53 = self
            .authored_model53
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model53.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model53 {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model5_water_splashes = self
            .authored_model5_water_splashes
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model5_water_splashes
            .retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model5_water_splashes {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model13_children = self
            .model13_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model13_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model13_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model6 = self
            .authored_model6
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model6.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model6 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model6_children = self
            .model6_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model6_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model6_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model17 = self
            .authored_model17
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model17.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model17 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model17_children = self
            .model17_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model17_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model17_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model22 = self
            .authored_model22
            .iter()
            .filter(|source| !source.active || source.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model22
            .retain(|source| source.active && source.control_flags & ENTITY_DRAW_DISABLED_BIT == 0);
        for (slot, entity_rng) in reclaimed_model22 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model22_children = self
            .model22_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model22_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model22_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76 = self
            .authored_model76
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model76.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model76 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76_children = self
            .model76_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model76_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model76_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76_tails = self
            .model76_tail_spawns
            .iter()
            .filter(|tail| !tail.active)
            .map(|tail| (tail.source_entity_index, tail.entity_rng))
            .collect::<Vec<_>>();
        self.model76_tail_spawns.retain(|tail| tail.active);
        for (slot, entity_rng) in reclaimed_model76_tails {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class14 = self
            .authored_class14
            .iter()
            .filter(|source| !source.active || !source.drawing_enabled)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class14
            .retain(|source| source.active && source.drawing_enabled);
        for (slot, entity_rng) in reclaimed_class14 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let inactive_class5_roots = self
            .authored_class5
            .iter()
            .filter(|source| !source.active)
            .map(|source| source.source_entity_index)
            .collect::<BTreeSet<_>>();
        // Worm body rows own independent action-29 phase gates. A hidden root
        // must not atomically erase body rows which have not yet reached their
        // own KillEntity callback.
        for part in &mut self.authored_dragon_parts {
            if inactive_class5_roots.contains(&part.parent_entity_index) {
                part.active = false;
            }
        }
        for child in &mut self.authored_class5_archer_children {
            if inactive_class5_roots.contains(&child.parent_entity_index) {
                child.active = false;
            }
        }
        let reclaimed_class5_archer_children = self
            .authored_class5_archer_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_archer_children
            .retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_class5_archer_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_dragon_parts = self
            .authored_dragon_parts
            .iter()
            .filter(|part| !part.active)
            .map(|part| (part.source_entity_index, part.entity_rng))
            .collect::<Vec<_>>();
        self.authored_dragon_parts.retain(|part| part.active);
        for (slot, entity_rng) in reclaimed_dragon_parts {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class5_model3_children = self
            .authored_class5_model3_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_model3_children
            .retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_class5_model3_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class5 = self
            .authored_class5
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5.retain(|source| source.active);
        let live_class5_slots = self
            .authored_class5
            .iter()
            .map(|source| source.source_entity_index)
            .collect::<BTreeSet<_>>();
        self.authored_class5_runtime
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_fireflies
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_dragons
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_class5_model3
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_class5_archer
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        for (slot, entity_rng) in reclaimed_class5 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_house_residents = self
            .house_residents
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.house_residents.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_house_residents {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_beyond_sight_effects = self
            .beyond_sight_effects
            .iter()
            .filter(|effect| !effect.active)
            .map(|effect| (effect.source_entity_index, effect.entity_rng))
            .collect::<Vec<_>>();
        self.beyond_sight_effects.retain(|effect| effect.active);
        for (slot, entity_rng) in reclaimed_beyond_sight_effects {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_mana_attraction = self
            .mana_attraction_fields
            .iter()
            .filter(|field| !field.active)
            .map(|field| (field.source_entity_index, field.entity_rng))
            .collect::<Vec<_>>();
        self.mana_attraction_fields.retain(|field| field.active);
        for (slot, entity_rng) in reclaimed_mana_attraction {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_children = self
            .castle_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.castle_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_children {
            self.release_slot(slot, Some(entity_rng));
        }
        if let Some(castle) = self.castle
            && !castle.active
        {
            self.source_rng.remove(&castle.source_entity_index);
            self.release_slot(castle.source_entity_index, None);
            self.castle = None;
        }
        self.refresh_owner_mana_rows();
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        self.reset_hash_inner(None)
    }

    #[must_use]
    pub(crate) fn reset_hash_with_terrain(&self, terrain: &SessionTerrain) -> String {
        self.reset_hash_inner(Some(terrain))
    }

    fn reset_hash_inner(&self, terrain: Option<&SessionTerrain>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        if let Some(terrain) = terrain {
            hasher.update(terrain.deterministic_hash().to_le_bytes());
        }
        hasher.update(self.global_rng.to_le_bytes());
        hasher.update(self.sentinel_rng.to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_castle_entity_index.to_le_bytes());
        hasher.update([self.player_castle_stage, self.player_castle_aux_flags]);
        hasher.update([self.player_model, self.player_terrain_flags]);
        hasher.update(self.player_spell_slot_2.to_le_bytes());
        hasher.update(self.player_spell_holder_action.to_le_bytes());
        hasher.update(self.player_spell_holder_max_action.to_le_bytes());
        for coefficient in self.castle_stage_coefficients {
            hasher.update(coefficient.to_le_bytes());
        }
        hasher.update(self.castle_controller_coefficients);
        for slot in &self.free_slots {
            hasher.update(slot.to_le_bytes());
        }
        for slot in &self.replacement_cursor_exclusions {
            hasher.update(slot.to_le_bytes());
        }
        hasher.update(b"replacement-list-epoch-v1");
        hasher.update(self.replacement_list_epoch.to_le_bytes());
        for (slot, rng) in &self.slot_rng {
            hasher.update(slot.to_le_bytes());
            hasher.update(rng.to_le_bytes());
        }
        for (slot, rng) in &self.source_rng {
            hasher.update(slot.to_le_bytes());
            hasher.update(rng.to_le_bytes());
        }
        hasher.update(b"source-phase-sidecars-v1");
        for (slot, phase) in &self.source_phase_sidecars {
            hasher.update(slot.to_le_bytes());
            hasher.update([*phase]);
        }
        for (slot, owner) in &self.source_owner {
            hasher.update(slot.to_le_bytes());
            hasher.update(owner.to_le_bytes());
        }
        hasher.update(b"authored-house-record-by-source-v1");
        for (slot, authored_record) in &self.authored_house_record_by_source {
            hasher.update(slot.to_le_bytes());
            hasher.update(authored_record.to_le_bytes());
        }
        for (owner, class) in &self.owner_class {
            hasher.update(owner.to_le_bytes());
            hasher.update([*class]);
        }
        for (owner, max_mana) in &self.owner_max_mana {
            hasher.update(owner.to_le_bytes());
            hasher.update(max_mana.to_le_bytes());
        }
        for row in &self.owner_mana_rows {
            hasher.update(row.source_entity_index.to_le_bytes());
            hasher.update([row.source_class, row.source_model]);
            hasher.update(row.player_entity_index.to_le_bytes());
            hasher.update(row.mana.to_le_bytes());
            hasher.update([u8::from(row.active)]);
        }
        for (owner, mana) in &self.owner_mana_aggregate {
            hasher.update(owner.to_le_bytes());
            hasher.update(mana.to_le_bytes());
        }
        hasher.update(self.owner_mana_global.to_le_bytes());
        for sphere in &self.spheres {
            hasher.update(sphere.id.to_le_bytes());
            hasher.update(sphere.source_entity_index.to_le_bytes());
            hasher.update(sphere.entity_rng.to_le_bytes());
            hasher.update(sphere.position.x.to_le_bytes());
            hasher.update(sphere.position.y.to_le_bytes());
            hasher.update(sphere.position.z.to_le_bytes());
            hasher.update(sphere.velocity_x.to_le_bytes());
            hasher.update(sphere.velocity_y.to_le_bytes());
            hasher.update(sphere.vertical_velocity.to_le_bytes());
            hasher.update(sphere.yaw.to_le_bytes());
            hasher.update(sphere.max_life.to_le_bytes());
            hasher.update(sphere.life.to_le_bytes());
            hasher.update(sphere.mana.to_le_bytes());
            hasher.update(sphere.owner_entity_index.to_le_bytes());
            hasher.update(sphere.player_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_owner_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_possession_payload.to_le_bytes());
            hasher.update(sphere.attraction_source_entity_index.to_le_bytes());
            hasher.update(sphere.attraction_step.to_le_bytes());
            hasher.update(sphere.attraction_distance_high.to_le_bytes());
            hasher.update(sphere.assigned_carrier_id.to_le_bytes());
            hasher.update(sphere.assigned_defender_slot.to_le_bytes());
            hasher.update(sphere.control_flags.to_le_bytes());
            hasher.update([sphere.phase]);
            hasher.update([u8::from(sphere.active), u8::from(sphere.pending_delete)]);
        }
        for burst in &self.death_bursts {
            hasher.update(burst.source_entity_index.to_le_bytes());
            hasher.update(burst.entity_rng.to_le_bytes());
            hasher.update(burst.parent_entity_index.to_le_bytes());
            hasher.update(burst.id_entity_index.to_le_bytes());
            hasher.update(burst.position.x.to_le_bytes());
            hasher.update(burst.position.y.to_le_bytes());
            hasher.update(burst.position.z.to_le_bytes());
            hasher.update(burst.max_life.to_le_bytes());
            hasher.update(burst.life_ticks.to_le_bytes());
            hasher.update(burst.subspell_index.to_le_bytes());
            hasher.update(burst.control_flags.to_le_bytes());
            hasher.update([burst.source_byte_3e, u8::from(burst.active)]);
        }
        hasher.update(b"player-death-markers-v1");
        for marker in &self.player_death_markers {
            hasher.update(marker.source_entity_index.to_le_bytes());
            hasher.update(marker.entity_rng.to_le_bytes());
            hasher.update(marker.trigger_player_entity_index.to_le_bytes());
            hasher.update(marker.position.x.to_le_bytes());
            hasher.update(marker.position.y.to_le_bytes());
            hasher.update(marker.position.z.to_le_bytes());
            hasher.update([marker.dispatch_phase]);
            hasher.update(marker.constructor_phase.to_le_bytes());
            hasher.update([marker.byte_38]);
            hasher.update(marker.max_life.to_le_bytes());
            hasher.update(marker.life.to_le_bytes());
            hasher.update(marker.particle_parameter_index.to_le_bytes());
            hasher.update([u8::from(marker.active)]);
        }
        for replacement in &self.spell_replacements {
            hasher.update(replacement.source_entity_index.to_le_bytes());
            hasher.update(replacement.entity_rng.to_le_bytes());
            hasher.update(replacement.parent_entity_index.to_le_bytes());
            hasher.update(replacement.position.x.to_le_bytes());
            hasher.update(replacement.position.y.to_le_bytes());
            hasher.update(replacement.position.z.to_le_bytes());
            hasher.update([replacement.family, replacement.action, replacement.phase]);
            hasher.update(replacement.control_flags.to_le_bytes());
            hasher.update([u8::from(replacement.active)]);
        }
        for source in &self.spell_pickups {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.family, source.action, source.phase]);
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        hasher.update(b"speed-trails-v1");
        for trail in &self.speed_trails {
            hasher.update(trail.source_entity_index.to_le_bytes());
            hasher.update(trail.entity_rng.to_le_bytes());
            hasher.update(trail.source_id.to_le_bytes());
            hasher.update([trail.class, trail.model, trail.action]);
            hasher.update(trail.position.x.to_le_bytes());
            hasher.update(trail.position.y.to_le_bytes());
            hasher.update(trail.position.z.to_le_bytes());
            hasher.update([trail.phase]);
            hasher.update(trail.max_life.to_le_bytes());
            hasher.update(trail.life.to_le_bytes());
            hasher.update(trail.control_flags.to_le_bytes());
            hasher.update([u8::from(trail.active)]);
        }
        hasher.update(b"terrain-strip-close-triggers-v1");
        for source in &self.terrain_strip_close_triggers {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.class,
                source.model,
                source.action,
                source.source_byte_3e,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"authored-model1-v1");
        for source in &self.authored_model1 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action, source.phase]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.drawing_enabled), u8::from(source.active)]);
        }
        for source in &self.authored_model45 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update(source.target_group.to_le_bytes());
            hasher.update(source.parameter_1.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update([source.font_type_index, source.source_byte_38]);
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_mana.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update([source.particle_byte_0x5d]);
            hasher.update(source.view_pitch.to_le_bytes());
            hasher.update(source.view_yaw.to_le_bytes());
            hasher.update(source.view_fov.to_le_bytes());
            hasher.update(source.view_roll.to_le_bytes());
            hasher.update(source.axis_x.to_le_bytes());
            hasher.update(source.axis_z.to_le_bytes());
            hasher.update(source.byte_0x46.to_le_bytes());
            hasher.update(source.parent_entity_index.to_le_bytes());
            hasher.update([source.source_byte_3e]);
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.drawing_enabled), u8::from(source.active)]);
        }
        for source in &self.authored_model13 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
        }
        for source in &self.bee_creatures {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            match source.origin {
                BeeCreatureOrigin::AuthoredRecord { authored_index } => {
                    // Preserve the established authored-Bee hash stream
                    // byte-for-byte. Package identity already proves this
                    // origin; only Morph needs an added discriminator.
                    hasher.update(authored_index.to_le_bytes());
                }
                BeeCreatureOrigin::MorphSpell { holder_slot } => {
                    hasher.update(b"morph-bee-origin-v1");
                    hasher.update(holder_slot.to_le_bytes());
                    hasher.update(source.parent_entity_index.to_le_bytes());
                }
            }
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                source.source_byte_3e,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.pending_damage.to_le_bytes());
            hasher.update(source.pending_attacker_entity_index.to_le_bytes());
            hasher.update(source.damage_attacker_entity_index.to_le_bytes());
            hasher.update(source.kill_credit_entity_index.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.target_entity_index.unwrap_or(u16::MAX).to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.source_byte_39.to_le_bytes());
            hasher.update([source.stage_var1, source.stage_var2]);
            match source.stage_target {
                Some(target) => {
                    hasher.update([1]);
                    hasher.update(target.x.to_le_bytes());
                    hasher.update(target.y.to_le_bytes());
                    hasher.update(target.z.to_le_bytes());
                }
                None => hasher.update([0]),
            }
            hasher.update(source.vertical_step.to_le_bytes());
            hasher.update(source.action_window.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
        }
        hasher.update(b"level2-bee-model2-constructor-phase-v1");
        hasher.update([self.bee_model2_constructor_phase]);
        hasher.update(b"level2-authored-bee-kill-census-v1");
        for (killer_entity_index, count) in &self.authored_bee_kill_census {
            hasher.update(killer_entity_index.to_le_bytes());
            hasher.update(count.to_le_bytes());
        }
        hasher.update(b"level2-authored-bee-deferred-reclaim-v1");
        for source_entity_index in &self.deferred_bee_reclaims {
            hasher.update(source_entity_index.to_le_bytes());
        }
        hasher.update(b"level2-authored-switch-producer-v1");
        for source in &self.authored_switch_producers {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                source.cooldown,
                u8::from(source.fired),
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.word_10.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-tree-v1");
        for source in &self.authored_trees {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.source_byte_38,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.owner_entity_index.to_le_bytes());
            hasher.update(source.pending_damage.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.fov.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update(source.last_sphere_source_entity_index.to_le_bytes());
            hasher.update(source.last_sphere_word_0x2c.to_le_bytes());
            hasher.update(source.last_sphere_position_z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model32-v1");
        for source in &self.authored_model32 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.objective_stage_index,
                source.source_byte_3e,
                u8::from(source.fired),
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.word_10.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model59-v1");
        for source in &self.authored_model59 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.source_byte_3e,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        if !self.model59_smoke_children.is_empty() {
            hasher.update(b"level2-model59-smoke-child-v1");
            for child in &self.model59_smoke_children {
                hasher.update(child.source_entity_index.to_le_bytes());
                hasher.update(child.entity_rng.to_le_bytes());
                hasher.update([
                    child.source_class,
                    child.model,
                    child.action,
                    child.phase,
                    u8::from(child.drawing_enabled),
                    u8::from(child.active),
                ]);
                hasher.update(child.max_life.to_le_bytes());
                hasher.update(child.life.to_le_bytes());
                hasher.update(child.max_speed.to_le_bytes());
                hasher.update(child.actual_speed.to_le_bytes());
                hasher.update(child.dword_0x10.to_le_bytes());
                hasher.update(child.particle_index.to_le_bytes());
                hasher.update(child.yaw.to_le_bytes());
                hasher.update(child.control_flags.to_le_bytes());
                hasher.update(child.position.x.to_le_bytes());
                hasher.update(child.position.y.to_le_bytes());
                hasher.update(child.position.z.to_le_bytes());
            }
        }
        hasher.update(b"level2-authored-model53-v1");
        for source in &self.authored_model53 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update([source.source_byte_39 as u8]);
            hasher.update([
                source.byte_0x43,
                source.byte_0x44,
                source.byte_0x46,
                u8::from(source.water_spawn_requested),
            ]);
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.sprite_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model5-water-splash-v1");
        for source in &self.authored_model5_water_splashes {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.sprite_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        for child in &self.model13_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.max_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.entity_index.to_le_bytes());
            hasher.update([child.subtype]);
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for source in &self.authored_model6 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.height_offset.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        for child in &self.model6_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.min_speed.to_le_bytes());
            hasher.update(child.max_speed.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.height_offset.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([u8::from(child.active)]);
        }
        for source in &self.authored_model17 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        for child in &self.model17_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.source_word_2c.to_le_bytes());
            hasher.update(child.target_source_entity_index.to_le_bytes());
            hasher.update(child.phase.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([u8::from(child.active)]);
        }
        for source in &self.authored_model22 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.word_0x2e.to_le_bytes());
            hasher.update([source.byte_0x38]);
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.word_0x30.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.axis_position.x.to_le_bytes());
            hasher.update(source.axis_position.y.to_le_bytes());
            hasher.update(source.axis_position.z.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([source.phase, u8::from(source.active)]);
        }
        for child in &self.model22_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update([child.segment_index]);
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update(child.word_0x36.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for source in &self.authored_model76 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.font_type_index.to_le_bytes());
            hasher.update([source.state]);
            hasher.update(source.target_entity_index.to_le_bytes());
            hasher.update(source.target_position.x.to_le_bytes());
            hasher.update(source.target_position.y.to_le_bytes());
            hasher.update(source.target_position.z.to_le_bytes());
            hasher.update(source.target_pitch.to_le_bytes());
            hasher.update(source.target_height_offset.to_le_bytes());
            hasher.update(source.target_control_flags.to_le_bytes());
            hasher.update(source.target_life.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
        }
        for child in &self.model76_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update([child.row_index, child.column_index, child.chain_index]);
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.collision_yaw.to_le_bytes());
            hasher.update(child.collision_pitch.to_le_bytes());
            hasher.update(child.collision_roll.to_le_bytes());
            hasher.update(child.collision_fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for tail in &self.model76_tail_spawns {
            hasher.update(tail.source_entity_index.to_le_bytes());
            hasher.update(tail.entity_rng.to_le_bytes());
            hasher.update(tail.parent_entity_index.to_le_bytes());
            hasher.update(tail.parent_entity_id.to_le_bytes());
            hasher.update([tail.source_class, tail.model, tail.action]);
            hasher.update(tail.subspell_index.to_le_bytes());
            hasher.update(tail.max_life.to_le_bytes());
            hasher.update(tail.life.to_le_bytes());
            hasher.update(tail.shift_rot_x.to_le_bytes());
            hasher.update(tail.shift_rot_y.to_le_bytes());
            hasher.update(tail.particle_index.to_le_bytes());
            hasher.update(tail.position.x.to_le_bytes());
            hasher.update(tail.position.y.to_le_bytes());
            hasher.update(tail.position.z.to_le_bytes());
            hasher.update([tail.phase]);
            hasher.update(tail.control_flags.to_le_bytes());
            hasher.update([u8::from(tail.active)]);
        }
        hasher.update(b"authored-class14-v2");
        hasher.update(self.class14_model3_handle.to_le_bytes());
        for source in &self.authored_class14 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action]);
            match source.experience_award {
                Some(award) => {
                    hasher.update([1]);
                    hasher.update(award.to_le_bytes());
                }
                None => hasher.update([0]),
            }
            hasher.update(source.particle.to_le_bytes());
            hasher.update([source.control_flags]);
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.activation_group.to_le_bytes());
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.drawing_enabled)]);
            hasher.update([u8::from(source.active)]);
        }
        hasher.update(b"authored-class5-v5");
        hasher.update(self.source_creature_population.to_le_bytes());
        hasher.update([
            self.class5_model0_constructor_phase,
            self.class5_model1_constructor_phase,
            self.class5_model3_constructor_phase,
            self.class5_model4_constructor_phase,
            self.class5_model9_constructor_phase,
            self.class5_model14_constructor_phase,
            self.class5_model19_constructor_phase,
        ]);
        hasher.update(b"castle-auxiliary-constructor-phase-v1");
        hasher.update([self.castle_auxiliary_constructor_phase]);
        for source in &self.authored_class5 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.model,
                source.action,
                source.source_byte_3e,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_mana.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.pending_word_0x62.to_le_bytes());
            hasher.update(source.word_0x26.to_le_bytes());
            hasher.update(source.word_0x24.to_le_bytes());
            hasher.update([source.byte_0x3b]);
            hasher.update(source.word_0x96_150.to_le_bytes());
            hasher.update([source.byte_0x46_70]);
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.source_byte_39.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.fov.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"authored-firefly-stage-v1");
        for state in &self.authored_fireflies {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.linked_entity_index.to_le_bytes());
            hasher.update(state.control_flags.to_le_bytes());
            hasher.update([state.byte_0x38, state.xtype]);
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update([state.stage_var1, state.stage_var2]);
            hasher.update(state.stage_target.x.to_le_bytes());
            hasher.update(state.stage_target.y.to_le_bytes());
            hasher.update(state.stage_target.z.to_le_bytes());
            hasher.update([u8::from(state.movement_blocked)]);
        }
        hasher.update(b"authored-dragon-v2");
        for state in &self.authored_dragons {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.linked_entity_index.to_le_bytes());
            hasher.update(state.word_0x36.to_le_bytes());
            hasher.update(state.control_flags.to_le_bytes());
            hasher.update([state.font_type_index, u8::from(state.movement_blocked)]);
            hasher.update(state.render_yaw.to_le_bytes());
            hasher.update(state.render_pitch.to_le_bytes());
            hasher.update(state.render_roll.to_le_bytes());
            hasher.update(state.render_fov.to_le_bytes());
        }
        for part in &self.authored_dragon_parts {
            hasher.update(part.source_entity_index.to_le_bytes());
            hasher.update(part.root_authored_index.to_le_bytes());
            hasher.update(part.entity_id.to_le_bytes());
            hasher.update(part.entity_rng.to_le_bytes());
            hasher.update(part.parent_entity_index.to_le_bytes());
            hasher.update(part.previous_entity_index.to_le_bytes());
            hasher.update(part.next_entity_index.to_le_bytes());
            hasher.update(part.word_0x36.to_le_bytes());
            hasher.update(part.pending_word_0x62.to_le_bytes());
            hasher.update(part.pending_damage.to_le_bytes());
            hasher.update(part.word_0x26.to_le_bytes());
            hasher.update([
                part.model,
                part.action,
                part.source_byte_3e,
                part.source_byte_39,
            ]);
            hasher.update(part.max_life.to_le_bytes());
            hasher.update(part.life.to_le_bytes());
            hasher.update(part.mana.to_le_bytes());
            hasher.update(part.subspell_index.to_le_bytes());
            hasher.update(part.particle_index.to_le_bytes());
            hasher.update(part.yaw.to_le_bytes());
            hasher.update(part.pitch.to_le_bytes());
            hasher.update(part.roll.to_le_bytes());
            hasher.update(part.fov.to_le_bytes());
            hasher.update(part.render_yaw.to_le_bytes());
            hasher.update(part.render_pitch.to_le_bytes());
            hasher.update(part.render_roll.to_le_bytes());
            hasher.update(part.render_fov.to_le_bytes());
            hasher.update(part.position.x.to_le_bytes());
            hasher.update(part.position.y.to_le_bytes());
            hasher.update(part.position.z.to_le_bytes());
            hasher.update([u8::from(part.drawing_enabled), u8::from(part.active)]);
        }
        hasher.update(b"authored-class5-model3-v3");
        for state in &self.authored_class5_model3 {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.damage_dword_0x5e_94.to_le_bytes());
            hasher.update(state.word_0x32.to_le_bytes());
            hasher.update(state.word_0x34.to_le_bytes());
            hasher.update(state.word_0x36.to_le_bytes());
            hasher.update(state.control_flags.to_le_bytes());
            hasher.update([state.byte_0x43, state.byte_0x44]);
            hasher.update(state.word_0x2c.to_le_bytes());
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update([state.last_child_root_phase]);
            hasher.update(state.render_yaw.to_le_bytes());
            hasher.update(state.render_pitch.to_le_bytes());
            hasher.update(state.render_roll.to_le_bytes());
            hasher.update(state.render_fov.to_le_bytes());
        }
        for child in &self.authored_class5_model3_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.root_authored_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.player_owner_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.word_0x32.to_le_bytes());
            hasher.update(child.word_0x34.to_le_bytes());
            hasher.update(child.word_0x36.to_le_bytes());
            hasher.update(child.word_0x24.to_le_bytes());
            hasher.update(child.pending_word_0x62.to_le_bytes());
            hasher.update(child.word_0x26.to_le_bytes());
            hasher.update(child.word_0x96.to_le_bytes());
            hasher.update(child.damage_dword_0x5e_94.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.mana.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update([
                child.model,
                child.action,
                child.source_byte_3e,
                child.source_byte_39,
                child.byte_0x3b,
                child.byte_0x43,
                child.byte_0x44,
                child.byte_0x46,
            ]);
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.render_yaw.to_le_bytes());
            hasher.update(child.render_pitch.to_le_bytes());
            hasher.update(child.render_roll.to_le_bytes());
            hasher.update(child.render_fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        hasher.update(b"authored-class5-archer-v1");
        for state in &self.authored_class5_archer {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.slot_generation.to_le_bytes());
            hasher.update(state.word_0x32.to_le_bytes());
            hasher.update(state.word_0x34.to_le_bytes());
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update([state.byte_0x38, state.xtype, state.last_child_root_phase]);
            hasher.update(state.pending_damage.to_le_bytes());
        }
        for child in &self.authored_class5_archer_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update(child.slot_generation.to_le_bytes());
            hasher.update(child.word_0x32.to_le_bytes());
            hasher.update(child.word_0x34.to_le_bytes());
            hasher.update(child.word_0x24.to_le_bytes());
            hasher.update(child.pending_word_0x62.to_le_bytes());
            hasher.update(child.word_0x26.to_le_bytes());
            hasher.update(child.word_0x96.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update([
                child.source_class,
                child.model,
                child.action,
                child.xtype,
                child.xsubtype,
                child.source_byte_3e,
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
        }
        for state in &self.authored_class5_runtime {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update([state.byte_0x38, state.xtype]);
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update(state.pending_damage.to_le_bytes());
            hasher.update(state.linked_head.to_le_bytes());
        }
        hasher.update(b"level2-house-resident-v1");
        for source in &self.house_residents {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.parent_house_entity_index.to_le_bytes());
            hasher.update(source.owner_entity_index.to_le_bytes());
            hasher.update(source.attacker_entity_index.to_le_bytes());
            let semantic_kind = source.semantic_kind.as_str().as_bytes();
            hasher.update(b"semantic-creature-v1");
            hasher.update([u8::try_from(semantic_kind.len())
                .expect("semantic creature name fits one source byte")]);
            hasher.update(semantic_kind);
            hasher.update([
                source.model,
                source.action,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        for effect in &self.beyond_sight_effects {
            hasher.update(effect.source_entity_index.to_le_bytes());
            hasher.update(effect.entity_rng.to_le_bytes());
            hasher.update(effect.holder_entity_index.to_le_bytes());
            hasher.update(effect.parent_entity_index.to_le_bytes());
            hasher.update(effect.parent_entity_id.to_le_bytes());
            hasher.update([effect.source_class, effect.model, effect.origin_subtype]);
            hasher.update(effect.subspell_index.to_le_bytes());
            hasher.update(effect.life.to_le_bytes());
            hasher.update(effect.effect_parameter.to_le_bytes());
            hasher.update(effect.actual_speed.to_le_bytes());
            hasher.update(effect.position.x.to_le_bytes());
            hasher.update(effect.position.y.to_le_bytes());
            hasher.update(effect.position.z.to_le_bytes());
            hasher.update(effect.axis_position.x.to_le_bytes());
            hasher.update(effect.axis_position.y.to_le_bytes());
            hasher.update(effect.axis_position.z.to_le_bytes());
            hasher.update(effect.yaw.to_le_bytes());
            hasher.update(effect.pitch.to_le_bytes());
            hasher.update([effect.sound_id, u8::from(effect.active)]);
        }
        for field in &self.mana_attraction_fields {
            hasher.update(field.source_entity_index.to_le_bytes());
            hasher.update(field.entity_rng.to_le_bytes());
            hasher.update(field.authored_index.to_le_bytes());
            hasher.update(field.position.x.to_le_bytes());
            hasher.update(field.position.y.to_le_bytes());
            hasher.update(field.position.z.to_le_bytes());
            hasher.update(field.radius_squared.to_le_bytes());
            hasher.update(field.max_life.to_le_bytes());
            hasher.update(field.life.to_le_bytes());
            hasher.update([field.action, u8::from(field.active)]);
        }
        for projectile in &self.fireball_projectiles {
            // Preserve the frozen public-spell state hash dialect. Creature
            // producer identity still participates in future-state identity.
            match projectile.origin {
                FireballProjectileOrigin::PlayerSpell => {}
                FireballProjectileOrigin::Firefly => {
                    hasher.update(b"firefly-fireball-origin-v1");
                }
                FireballProjectileOrigin::Dragon => {
                    hasher.update(b"dragon-fireball-origin-v1");
                }
                FireballProjectileOrigin::Worm => {
                    hasher.update(b"worm-fireball-origin-v1");
                }
            }
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.owner_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.source_subspell_index.to_le_bytes());
            hasher.update(projectile.source_mana.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.action,
            ]);
            hasher.update(projectile.payload.to_le_bytes());
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.roll.to_le_bytes());
            hasher.update(projectile.fov.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
            hasher.update(projectile.dword_0x10.to_le_bytes());
            hasher.update(projectile.target_source_entity_index.to_le_bytes());
            hasher.update(projectile.control_flags.to_le_bytes());
            hasher.update(projectile.source_word_2c.to_le_bytes());
            hasher.update([
                u8::from(projectile.acquisition_attempted),
                u8::from(projectile.threat_accounted),
                projectile.phase,
                u8::from(projectile.active),
                u8::from(projectile.drawing_disabled),
            ]);
        }
        hasher.update(b"level2-possession-projectile-v2");
        for projectile in &self.possession_projectiles {
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.source_id_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.target_source_entity_index.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.action,
                projectile.output_class,
                projectile.output_model,
                projectile.phase,
                u8::from(projectile.acquisition_attempted),
                u8::from(projectile.active),
            ]);
            if projectile.threat_accounted {
                // Preserve reset hashes for the source-zero state while
                // making the one-shot prepass transition authoritative.
                hasher.update(b"sub-159e0-threat-accounted");
            }
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.roll.to_le_bytes());
            hasher.update(projectile.fov.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
        }
        hasher.update(b"level2-possession-guidance-source-slots-v1");
        for source_slot in &self.possession_guidance_source_slots {
            hasher.update(source_slot.to_le_bytes());
        }
        hasher.update(b"level2-possession-guidance-stale-records-v1");
        for (source_slot, position) in &self.possession_guidance_stale_records {
            hasher.update(source_slot.to_le_bytes());
            hasher.update(position.x.to_le_bytes());
            hasher.update(position.y.to_le_bytes());
            hasher.update(position.z.to_le_bytes());
        }
        hasher.update(b"level2-possession-effect-v1");
        for effect in &self.possession_effects {
            hasher.update(effect.source_entity_index.to_le_bytes());
            hasher.update(effect.entity_rng.to_le_bytes());
            hasher.update(effect.source_id_entity_index.to_le_bytes());
            hasher.update(effect.projectile_entity_index.to_le_bytes());
            hasher.update([
                effect.source_class,
                effect.source_model,
                effect.action,
                effect.phase,
                u8::from(effect.active),
            ]);
            hasher.update(effect.position.x.to_le_bytes());
            hasher.update(effect.position.y.to_le_bytes());
            hasher.update(effect.position.z.to_le_bytes());
            hasher.update(effect.yaw.to_le_bytes());
            hasher.update(effect.pitch.to_le_bytes());
            hasher.update(effect.life.to_le_bytes());
            hasher.update(effect.max_life.to_le_bytes());
        }
        if self.carriers.is_empty() {
            hasher.update([0]);
        }
        for carrier in &self.carriers {
            hasher.update([1]);
            hasher.update(carrier.id.to_le_bytes());
            hasher.update(carrier.source_entity_index.to_le_bytes());
            hasher.update(carrier.entity_rng.to_le_bytes());
            hasher.update(carrier.owner_entity_index.to_le_bytes());
            hasher.update(carrier.position.x.to_le_bytes());
            hasher.update(carrier.position.y.to_le_bytes());
            hasher.update(carrier.position.z.to_le_bytes());
            hasher.update(carrier.yaw.to_le_bytes());
            hasher.update(carrier.pitch.to_le_bytes());
            hasher.update(carrier.speed.to_le_bytes());
            hasher.update(carrier.carried_mana.to_le_bytes());
            hasher.update(carrier.life.to_le_bytes());
            hasher.update(carrier.max_life.to_le_bytes());
            hasher.update(carrier.max_mana.to_le_bytes());
            hasher.update(carrier.target_sphere_id.to_le_bytes());
            hasher.update(carrier.pending_damage.to_le_bytes());
            hasher.update(carrier.pending_attacker_entity_index.to_le_bytes());
            hasher.update([
                u8::from(carrier.returning_to_castle),
                u8::from(carrier.active),
            ]);
        }
        for projectile in &self.castle_projectiles {
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.owner_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.source_subspell_index.to_le_bytes());
            hasher.update(projectile.source_mana.to_le_bytes());
            hasher.update([projectile.source_a4_byte_154]);
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.target_position.x.to_le_bytes());
            hasher.update(projectile.target_position.y.to_le_bytes());
            hasher.update(projectile.target_position.z.to_le_bytes());
            hasher.update(projectile.target_anchor.x.to_le_bytes());
            hasher.update(projectile.target_anchor.y.to_le_bytes());
            hasher.update(projectile.target_anchor.z.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.source_byte_43,
                projectile.source_byte_44,
                u8::from(projectile.site_rejected),
                u8::from(projectile.active),
                u8::from(projectile.drawing_disabled),
            ]);
            hasher.update(projectile.source_word_96.to_le_bytes());
            hasher.update(projectile.target_castle_slot.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
            hasher.update(projectile.control_flags.to_le_bytes());
        }
        if let Some(castle) = self.castle {
            hasher.update([1]);
            hasher.update(castle.source_entity_index.to_le_bytes());
            hasher.update(castle.source_id_entity_index.to_le_bytes());
            hasher.update([
                castle.source_class,
                castle.source_model,
                castle.source_byte_38,
            ]);
            hasher.update(castle.source_particle_parameter.to_le_bytes());
            hasher.update(castle.owner_entity_index.to_le_bytes());
            hasher.update(castle.position.x.to_le_bytes());
            hasher.update(castle.position.y.to_le_bytes());
            hasher.update(castle.position.z.to_le_bytes());
            hasher.update(castle.anchor_position.x.to_le_bytes());
            hasher.update(castle.anchor_position.y.to_le_bytes());
            hasher.update(castle.anchor_position.z.to_le_bytes());
            hasher.update(castle.mana.to_le_bytes());
            hasher.update(castle.life.to_le_bytes());
            hasher.update(castle.max_life.to_le_bytes());
            hasher.update(castle.mana_capacity.to_le_bytes());
            hasher.update(castle.pending_damage.to_le_bytes());
            hasher.update(castle.pending_attacker_entity_index.to_le_bytes());
            hasher.update(castle.killer_entity_index.to_le_bytes());
            hasher.update(castle.control_flags.to_le_bytes());
            hasher.update([castle.timer]);
            hasher.update([castle.phase]);
            hasher.update(castle.construction_slot.to_le_bytes());
            hasher.update(castle.defense_slot.to_le_bytes());
            hasher.update([
                castle.action,
                castle.build_state,
                castle.level,
                castle.construction_model,
                castle.construction_action,
                castle.defense_model,
                castle.defense_action,
                u8::from(castle.construction_child_active),
                u8::from(castle.defense_child_active),
                u8::from(castle.active),
            ]);
            hasher.update(castle.construction_control_flags.to_le_bytes());
            hasher.update(castle.defense_control_flags.to_le_bytes());
            hasher.update(castle.construction_parent_slot.to_le_bytes());
            hasher.update(castle.construction_owner_entity_index.to_le_bytes());
            hasher.update(castle.construction_id.to_le_bytes());
            hasher.update(castle.construction_position.x.to_le_bytes());
            hasher.update(castle.construction_position.y.to_le_bytes());
            hasher.update(castle.construction_position.z.to_le_bytes());
            hasher.update(castle.defense_parent_slot.to_le_bytes());
            hasher.update(castle.defense_owner_entity_index.to_le_bytes());
            hasher.update(castle.defense_id.to_le_bytes());
            hasher.update(castle.defense_position.x.to_le_bytes());
            hasher.update(castle.defense_position.y.to_le_bytes());
            hasher.update(castle.defense_position.z.to_le_bytes());
            for slot in castle.child_chain_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update(castle.child_chain_models);
            hasher.update([
                u8::from(castle.child_chain_active[0]),
                u8::from(castle.child_chain_active[1]),
            ]);
            for slot in castle.controller_chain_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update(castle.controller_chain_count.to_le_bytes());
            hasher.update(castle.controller_chain_head.to_le_bytes());
            hasher.update(castle.controller_chain_tail.to_le_bytes());
            hasher.update(castle.shift_pitch.to_le_bytes());
            hasher.update(castle.shift_roll.to_le_bytes());
            hasher.update(castle.shift_yaw.to_le_bytes());
            hasher.update(castle.shift_fov.to_le_bytes());
            hasher.update(castle.source_word_2e.to_le_bytes());
            hasher.update(castle.source_word_30.to_le_bytes());
            hasher.update(castle.source_word_7c.to_le_bytes());
            hasher.update(castle.source_word_80.to_le_bytes());
            hasher.update([u8::from(castle.upgrade_holder_clear_pending)]);
            for slot in castle.defender_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update([castle.defender_count]);
            hasher.update(castle.defender_total_mana.to_le_bytes());
            hasher.update(castle.defender_total_max_mana.to_le_bytes());
            for slot in castle.defender_target_spheres {
                hasher.update(slot.to_le_bytes());
            }
            for slot in castle.auxiliary_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update([castle.auxiliary_count, castle.auxiliary_cooldown]);
            for position in castle.auxiliary_positions {
                hasher.update(position.x.to_le_bytes());
                hasher.update(position.y.to_le_bytes());
                hasher.update(position.z.to_le_bytes());
            }
            for yaw in castle.auxiliary_yaws {
                hasher.update(yaw.to_le_bytes());
            }
            for roll in castle.auxiliary_rolls {
                hasher.update(roll.to_le_bytes());
            }
            for player in castle.auxiliary_player_entity_indices {
                hasher.update(player.to_le_bytes());
            }
        } else {
            hasher.update([0]);
        }
        for child in &self.castle_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.id_entity_index.to_le_bytes());
            hasher.update([child.source_class]);
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.owner_entity_index.to_le_bytes());
            hasher.update(child.player_entity_index.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update(child.axis_position.x.to_le_bytes());
            hasher.update(child.axis_position.y.to_le_bytes());
            hasher.update(child.axis_position.z.to_le_bytes());
            hasher.update([child.model, child.action, u8::from(child.active)]);
            hasher.update(child.mana.to_le_bytes());
            hasher.update(child.max_mana.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.speed.to_le_bytes());
            hasher.update(child.target_sphere_slot.to_le_bytes());
            hasher.update(child.pending_damage.to_le_bytes());
            hasher.update(child.pending_attacker_entity_index.to_le_bytes());
            hasher.update(child.damage_attacker_entity_index.to_le_bytes());
            hasher.update(child.killer_entity_index.to_le_bytes());
            hasher.update([
                child.source_byte_3e,
                child.source_byte_39,
                child.source_byte_3b,
                child.cave_collision_cooldown,
            ]);
            hasher.update(child.source_dword_10.to_le_bytes());
            hasher.update(child.source_word_2e.to_le_bytes());
            hasher.update(child.construction_target_height.to_le_bytes());
            hasher.update(child.source_subspell_index.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.visual_index.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.chain_previous_slot.to_le_bytes());
            hasher.update(child.chain_next_slot.to_le_bytes());
            hasher.update([child.source_byte_43]);
            hasher.update(child.source_word_4a.to_le_bytes());
        }
        if let Some(footprint) = &self.castle_footprint {
            hasher.update([1, footprint.index, footprint.width, footprint.height]);
            for cell in footprint.cells() {
                hasher.update(cell);
            }
        } else {
            hasher.update([0]);
        }
        for (stage, footprint) in &self.castle_stage_footprints {
            hasher.update([*stage, footprint.index, footprint.width, footprint.height]);
            for cell in footprint.cells() {
                hasher.update(cell);
            }
        }
        hasher.update([0xff]);
        for source_entity_index in self.map_chains.linked_entity_indices() {
            hasher.update(source_entity_index.to_le_bytes());
            hasher.update(self.map_chains.tile_of(source_entity_index).to_le_bytes());
            hasher.update(
                self.map_chains
                    .old_map_entity(source_entity_index)
                    .to_le_bytes(),
            );
            hasher.update(
                self.map_chains
                    .next_entity(source_entity_index)
                    .to_le_bytes(),
            );
        }
        format!("{:x}", hasher.finalize())
    }
}

fn step_speed_trail_effect(trail: &mut SpeedTrailEffect) {
    let transition = crate::spell_activation::step_speed_trail(trail.life);
    trail.life = transition.life;
    if transition.disable_drawing {
        trail.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        trail.active = false;
    }
}

#[cfg(test)]
mod speed_trail_tests {
    use super::*;

    #[test]
    fn terminal_visit_disables_drawing_but_retains_new_event_identity() {
        let mut trail = SpeedTrailEffect {
            source_entity_index: 178,
            entity_rng: 0x4321,
            source_id: 152,
            class: 10,
            model: 2,
            action: 2,
            position: Position::default(),
            phase: 178,
            max_life: 8,
            life: -1,
            control_flags: 0x0002_0001,
            active: true,
        };

        step_speed_trail_effect(&mut trail);

        assert_eq!(trail.life, -2);
        assert_eq!(trail.control_flags, 0x0002_0401);
        assert!(!trail.active);
        assert_eq!((trail.class, trail.model, trail.action), (10, 2, 2));
        assert_eq!((trail.source_entity_index, trail.source_id), (178, 152));
    }
}
