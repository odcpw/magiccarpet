use super::*;

#[cfg(test)]
mod castle_revival_tests {
    use super::*;

    fn fixture() -> (Level2SharedSpellResetSession, Level2EconomyState) {
        let inspection = NativeWorld::inspect_level2(super::super::tests::level_two_fixture_path())
            .expect("qualified source player/arena package");
        let shared = Level2SharedSpellResetSession::from_campaign_parts(
            inspection.entities(),
            inspection.players(),
            inspection.runtime(),
        )
        .unwrap();
        let economy = Level2EconomyState::from_parts(
            SessionTerrain::from_inspection(&inspection),
            inspection.entities(),
            shared.player_entity.mana,
            shared.player_entity.model,
            0,
            shared.local_player.enabled_spell_holders[2],
            inspection.runtime().random_state,
        )
        .unwrap();
        (shared, economy)
    }

    fn dead_actor(shared: &mut Level2SharedSpellResetSession) {
        let mut actor = shared.player_entity.clone();
        actor.set_player_death_state();
        actor.set_life(-1);
        actor.set_mana(-200);
        shared.sync_player_entity_from_authority(&actor);
    }

    #[test]
    fn local_directional_feedback_preserves_lane_and_upkeep_semantics() {
        let (mut shared, _) = fixture();
        assert_eq!(shared.player_hit_effect_cooldown, 0);
        let baseline_hash = shared.reset_hash();
        let mut different = shared.clone();
        different.player_hit_effect_cooldown = 1;
        assert_ne!(baseline_hash, different.reset_hash());

        shared.set_player_hit_frame_time(0);
        shared.player_life_regen_cooldown = 0;
        let mut actor = shared.player_entity.clone();
        actor.record[58..60].copy_from_slice(&0_i16.to_le_bytes());
        assert!(shared.apply_directional_feedback(&mut actor, false, true, false));
        assert_eq!(actor.record[58..60], 100_i16.to_le_bytes());
        assert_eq!(shared.player_entity.record[58..60], actor.record[58..60]);
        assert_eq!(shared.local_player.hit_frame_time, 0);
        assert_eq!(shared.player_life_regen_cooldown, 0);
        assert_eq!(shared.player_hit_effect_cooldown, 0);

        assert!(shared.apply_directional_feedback(&mut actor, false, true, true));
        assert_eq!(shared.local_player.hit_frame_time, 4);
        assert_eq!(shared.player_life_regen_cooldown, 16);
        assert_eq!(shared.player_hit_effect_cooldown, 64);
        assert!(shared.step_local_player_unboosted_upkeep(0, false, None));
        assert_eq!(shared.player_life_regen_cooldown, 15);
        assert_eq!(shared.player_hit_effect_cooldown, 63);

        let mut stale = actor.clone();
        stale.index = stale.index.wrapping_add(1);
        assert!(!shared.apply_directional_feedback(&mut stale, true, false, false));
        assert_eq!(shared.player_hit_effect_cooldown, 63);
        dead_actor(&mut shared);
        assert!(!shared.step_local_player_unboosted_upkeep(0, false, None));
        assert_eq!(shared.player_hit_effect_cooldown, 63);
    }

    #[test]
    fn castle_revival_restores_same_actor_and_rebuilds_holders_without_resetting_progress() {
        let (mut shared, mut economy) = fixture();
        // This raw reset package has no bound starting Fireball. Produce its
        // holder through the ordinary carry constructor before dropping it;
        // from_campaign_parts only imports existing source relations.
        shared
            .materialize_carried_spell_families(&mut economy, &[(0, 0)])
            .expect("ordinary starting Fireball holder is materialized");
        let old_holder = shared
            .pickup(shared.local_player.enabled_spell_holders[0])
            .expect("starting Fireball relation resolves to its constructed holder");
        let mut dropped = old_holder.clone();
        dropped.set_source_action(1);
        dropped.set_control_flags(dropped.control_flags & !1);
        dropped.set_life(217);
        shared.sync_released_holder(&dropped);
        for family in 0..PLAYER_SPELL_COUNT {
            shared.set_enabled_spell_holder_index(family, 0);
        }
        shared.set_enabled_spell_holder_index(0, 1);
        shared.set_enabled_spell_holder_index(3, 1);
        shared.local_player.persistent_spell_experience[0] = 12_000;
        shared.local_player.runtime_spell_experience[0] = 123;
        shared.local_player.family_selected_tiers[0] = 1;
        shared.local_player.runtime_spell_experience[3] = 400;
        shared.local_player.family_selected_tiers[3] = 2;
        shared.local_player.runtime_spell_experience[2] = 99;
        shared.local_player.left_spell_slot_raw = 0;
        shared.local_player.right_spell_slot_raw = 3;
        shared.local_player.left_subspell_index = 2;
        shared.local_player.right_subspell_index = 2;
        shared.local_player.record[345..347].copy_from_slice(&0x5a73_u16.to_le_bytes());
        let before = shared.local_player.clone();
        dead_actor(&mut shared);
        let dead = shared.player_entity.clone();
        let position = Position {
            x: 4352,
            y: 6272,
            z: -123,
        };
        let holders = shared
            .revive_local_player_at_castle(&mut economy, position, false)
            .unwrap();

        let actor = &shared.player_entity;
        assert_eq!(actor.index, dead.index);
        assert_eq!(actor.source_id, dead.source_id);
        assert_eq!(
            actor.position, position,
            "source uses Castle z without an offset"
        );
        assert_eq!(
            (actor.max_life, actor.life, actor.max_mana, actor.mana),
            (10_000, 10_000, 1_000, 1_000)
        );
        assert_eq!(actor.action, 0);
        assert_eq!(actor.control_flags, (dead.control_flags & !0x20) | 1);
        assert_eq!(actor.actual_speed, dead.actual_speed);
        assert_eq!(actor.entity_rng, dead.entity_rng);
        assert_eq!(actor.mana_regen, dead.mana_regen);
        assert_eq!(shared.player_mana, 1_000);
        assert_eq!(
            shared.pickup(dropped.index),
            Some(&dropped),
            "released holder stays in the world"
        );
        assert_eq!(
            holders.iter().map(|row| row.model).collect::<Vec<_>>(),
            [0, 3]
        );
        assert!(holders.iter().all(|row| row.index != dropped.index));
        for holder in &holders {
            assert_eq!(holder.position, position);
            assert_eq!(holder.source_spell_parent_entity_index(), dead.index);
            assert_eq!(holder.action, holder.model * 3);
            assert_eq!(holder.control_flags, 5);
            assert_eq!(holder.level_gate, 1);
            assert_eq!(
                shared.local_player.enabled_spell_holders[usize::from(holder.model)],
                holder.index
            );
        }
        let player = &shared.local_player;
        assert_eq!(
            player.persistent_spell_experience,
            before.persistent_spell_experience
        );
        assert_eq!(
            player.runtime_spell_experience,
            before.runtime_spell_experience
        );
        assert_eq!(
            (
                player.family_selected_tiers[0],
                player.family_selected_tiers[3]
            ),
            (1, 1)
        );
        assert_eq!(
            (
                player.spell_unlocked_levels[0],
                player.spell_unlocked_levels[3]
            ),
            (2, 1)
        );
        assert_eq!(
            (player.left_spell_slot_raw, player.right_spell_slot_raw),
            (0, 3)
        );
        assert_eq!(
            (player.left_subspell_index, player.right_subspell_index),
            (1, 1)
        );
        assert_eq!(player.spell_hand_eligibility, before.spell_hand_eligibility);
        assert_eq!(player.acquired_spell_flags, before.acquired_spell_flags);
        assert_eq!(player.spell_enabled_flags, before.spell_enabled_flags);
        assert_eq!(player.learned_spell_flags, before.learned_spell_flags);
        assert_eq!(player.banishment_state_0x006, before.banishment_state_0x006);
        assert_eq!(
            &player.record[345..347],
            &before.record[345..347],
            "word159 is not a compact record offset"
        );
        assert_eq!(&player.record[20..24], &[0; 4]);
        assert_eq!(&player.record[32..38], &[0; 6]);
        assert_eq!(
            player.type_str_164_auxiliary.unwrap().dword_0x16D_365,
            2_000
        );
        assert!(
            shared.pending_effects.is_empty(),
            "sub_6DB50(0,0) does not announce improvements"
        );
    }

    #[test]
    fn castle_revival_exhaustion_keeps_earlier_holders_and_caps_only_ordinary_castle_xp() {
        let (mut shared, mut economy) = fixture();
        for family in 0..PLAYER_SPELL_COUNT {
            shared.set_enabled_spell_holder_index(family, 0);
        }
        for family in [0, 3, 25] {
            shared.set_enabled_spell_holder_index(family, 1);
        }
        shared.local_player.runtime_spell_experience[2] = 101;
        shared.local_player.persistent_spell_experience[2] = 42;
        let only_slot = *economy.free_slots.last().unwrap();
        economy.free_slots = vec![only_slot];
        dead_actor(&mut shared);
        let holders = shared
            .revive_local_player_at_castle(
                &mut economy,
                Position {
                    x: 1024,
                    y: 2048,
                    z: 1024,
                },
                true,
            )
            .unwrap();
        assert_eq!(holders.len(), 1);
        assert_eq!((holders[0].model, holders[0].index), (0, only_slot));
        assert_eq!(shared.local_player.enabled_spell_holders[0], only_slot);
        assert_eq!(shared.local_player.enabled_spell_holders[3], 0);
        assert_eq!(shared.local_player.enabled_spell_holders[25], 0);
        assert_eq!(economy.free_slot_count(), 0);
        assert_eq!(shared.local_player.runtime_spell_experience[2], 7);
        assert_eq!(shared.local_player.persistent_spell_experience[2], 42);
        assert_eq!(shared.local_player.spell_unlocked_levels[2], 1);
    }

    #[test]
    fn castle_revival_rejects_a_living_actor_before_mutating_inventory_or_arena() {
        let (mut shared, mut economy) = fixture();
        let actor = shared.player_entity.clone();
        let player = shared.local_player.clone();
        let rng = economy.global_rng;
        let slots = economy.free_slots.clone();
        assert!(
            shared
                .revive_local_player_at_castle(&mut economy, actor.position, true)
                .is_err()
        );
        assert_eq!(shared.player_entity, actor);
        assert_eq!(shared.local_player, player);
        assert_eq!(economy.global_rng, rng);
        assert_eq!(economy.free_slots, slots);
    }
}

#[cfg(test)]
mod authored_spell_family_tests {
    use super::*;

    #[test]
    fn authored_starting_army_mine_and_alliance_rows_use_source_constructors_and_acquisition() {
        let inspection = NativeWorld::inspect_level2(super::super::tests::level_two_fixture_path())
            .expect("qualified source player/arena package");
        let mut shared = Level2SharedSpellResetSession::from_campaign_parts(
            inspection.entities(),
            inspection.players(),
            inspection.runtime(),
        )
        .unwrap();
        let terrain = SessionTerrain::from_inspection(&inspection);
        let mut economy = Level2EconomyState::from_parts(
            terrain.clone(),
            inspection.entities(),
            shared.player_entity.mana,
            shared.player_entity.model,
            0,
            shared.local_player.enabled_spell_holders[2],
            inspection.runtime().random_state,
        )
        .unwrap();
        let particle = ParticleParameter {
            base_tmap: 0,
            word_2: 0,
            word_4: 0,
            speed: 0,
            rotation_speed: 0,
            byte_10: 0,
            byte_11: 0,
            render_mode: 20,
            byte_13: 0,
        };
        let particles = vec![particle; 78];

        for family in [0_u8, 1, 19, 23, 24] {
            let authored_index = 50_000 + u16::from(family);
            let feature = crate::level1_mission::AuthoredFeature {
                index: authored_index,
                kind: 15,
                subtype: i16::from(family),
                tile_x: 8,
                tile_y: 9,
                activation_group: 0,
                word_10: 0,
                stage_tag: 1,
                parameter_1: 0,
                parameter_2: 0,
                parameter_3: 0,
            };
            if family == 0 {
                let before_shared = shared.clone();
                let before_economy = economy.clone();
                let invalid = AuthoredFeature {
                    subtype: 26,
                    ..feature
                };
                assert!(
                    shared
                        .materialize_authored_spell_pickup(
                            &mut economy,
                            invalid,
                            &terrain,
                            &particles,
                        )
                        .is_err()
                );
                assert_eq!(shared, before_shared);
                assert_eq!(economy, before_economy);
            }
            let slot = shared
                .materialize_authored_spell_pickup(&mut economy, feature, &terrain, &particles)
                .unwrap()
                .expect("authored source slot allocated");
            let profile = level2_set_spell_profile(family, 0)
                .expect("typed family has its source SetSpell profile");
            let pickup = shared
                .pickup(slot)
                .expect("ordinary source constructor published the pickup row");
            assert_eq!(
                (pickup.class, pickup.model, pickup.action),
                (15, family, family * 3 + 1)
            );
            assert_eq!(pickup.subspell_index, profile.subspell_index);
            assert_eq!(pickup.activation_value, profile.word_0x30);
            assert_eq!(
                pickup.control_flags, 4,
                "AddSpellXX clears bit 8 and AddEventToMap sets map membership bit 4"
            );
            assert!(
                !pickup.drawing_disabled(),
                "an ordinary pickup remains visible until acquisition"
            );
            assert_eq!(
                (pickup.mana, pickup.max_mana, pickup.mana_regen),
                (profile.mana, profile.max_mana, profile.mana_regen)
            );
            let acquisition = shared
                .acquisitions
                .iter()
                .find(|state| state.family == family)
                .expect("ordinary source pickup registered its acquisition state");
            assert_eq!(acquisition.source_slot, slot);
            assert_eq!(acquisition.source_action, family * 3 + 1);
            assert_eq!(acquisition.subspell_index, profile.subspell_index);
            let replacement = economy
                .allocate_spell_replacement(family, slot, pickup.position)
                .expect("AddSpellXX persistent replacement uses the ordinary allocator");
            shared
                .materialize_spell_replacement_pickup(replacement)
                .expect("the real family constructor supplies its retained prototype");
            let replacement_row = shared.pickup(replacement.source_entity_index).unwrap();
            assert_eq!(replacement_row.action, family * 3 + 2);
            assert_eq!(replacement_row.control_flags, 4);
            assert!(
                !replacement_row.drawing_disabled(),
                "the replacement remains collectible"
            );
            if family == 0 {
                // Controlled missing-inventory setup. Collection still runs
                // the real source collision and acquisition transition.
                shared.set_enabled_spell_holder_index(0, 0);
                let state = shared
                    .acquisitions
                    .iter_mut()
                    .find(|state| state.family == 0)
                    .unwrap();
                state.holder_relation = 0;
                let position = shared.pickup(slot).unwrap().position;
                shared
                    .step_level2_spell_acquisition_with_economy(
                        &mut economy,
                        0,
                        shared.player_entity.index,
                        position,
                        &terrain,
                    )
                    .unwrap()
                    .expect("visible source Fireball is acquired through its ordinary callback");
                assert_eq!(shared.local_player.enabled_spell_holders[0], slot);
                assert_eq!(shared.pickup(slot).unwrap().action, 0);
                let phase = shared.pickup(slot).unwrap().phase;
                shared
                    .finish_dynamic_starting_spell_holder(&mut economy, slot)
                    .unwrap();
                assert_eq!(shared.pickup(slot).unwrap().phase, phase.wrapping_add(1));
                assert_eq!(
                    economy
                        .spell_pickups
                        .iter()
                        .find(|row| row.source_entity_index == slot)
                        .unwrap()
                        .phase,
                    phase.wrapping_add(1)
                );
                let before = shared.clone();
                let mut retired = economy.clone();
                retired
                    .spell_pickups
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
                    .unwrap()
                    .active = false;
                assert!(
                    shared
                        .finish_dynamic_starting_spell_holder(&mut retired, slot)
                        .is_err()
                );
                assert_eq!(
                    shared, before,
                    "a retired typed carrier cannot mutate the shared holder"
                );
            }
        }
    }
}

#[cfg(test)]
mod source30_house_action_51_tests {
    use super::*;
    use crate::{
        PlayerCommand,
        campaign::{CampaignSession, spec::CampaignSelection},
        house::{HOUSE_TERRAIN_TO_LIVE_ACTION, HouseState, HouseStepEvent},
        native_world::Mc2DispatchEvent,
    };
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn actual_source30_open_keeps_action51_house_state_and_runs_its_build_consumer() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-paj2-source30-campaign-package");
        let mut session = CampaignSession::open(
            &package,
            CampaignSelection::secret_destination(30).expect("source30 selection"),
        )
        .expect("actual source30 campaign reset opens");

        let source = session
            .inspection()
            .entities()
            .get(4)
            .expect("source30 house entity 4");
        assert!(source.active);
        assert_eq!((source.class, source.model), (10, 45));
        assert_eq!(source.action, HOUSE_TERRAIN_TO_LIVE_ACTION);
        assert_eq!(source.record_bytes()[73], HOUSE_TERRAIN_TO_LIVE_ACTION);

        let house = session
            .simulation()
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == 4)
            .copied()
            .expect("source30 action51 row materialized into HouseFamily");
        assert_eq!(house.action, HOUSE_TERRAIN_TO_LIVE_ACTION);
        assert_eq!(house.source_id, source.source_id);
        assert_eq!(house.control_flags, source.control_flags);
        assert_eq!((house.life, house.max_life), (source.life, source.max_life));
        assert_eq!(house.phase, source.phase);
        assert_eq!(house.position, source.position);
        assert_eq!(house.build_tail, source.record_bytes()[59]);
        assert_eq!(house.building_index, source.record_bytes()[74]);
        assert_eq!(
            session
                .simulation()
                .ordinary()
                .house_parameters
                .get(&house.building_index),
            Some(&house.parameter)
        );
        assert!(
            session
                .simulation()
                .ordinary()
                .house_build_footprints
                .contains_key(&house.building_index)
        );

        let parameters = BTreeMap::from([(house.building_index, house.parameter)]);
        let mut unsupported = (*source).clone();
        unsupported.action = u8::MAX;
        assert!(HouseState::from_campaign_snapshot(&unsupported, &parameters).is_err());

        session.step(PlayerCommand::default(), 0);
        let first_dispatch = session.simulation().dispatch_events();
        assert!(first_dispatch.iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::House { source_slot: 4, events, .. }
                if events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::ConstructionPass { .. }
                        | HouseStepEvent::ConstructionCompleted { .. }
                ))
        )));
        assert!(
            !first_dispatch
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { source_slot: 4, .. }))
        );

        session.step(PlayerCommand::default(), 0);
        let next_source = &session.simulation().entities()[4];
        let next_house = session
            .simulation()
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == 4)
            .expect("source30 house remains joined through the next dispatch pass");
        assert_eq!(next_source.action, next_house.action);
        assert_eq!(
            (next_source.life, next_source.max_life),
            (next_house.life, next_house.max_life)
        );
        assert_eq!(next_source.phase, next_house.phase);
        assert!(
            !session
                .simulation()
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { source_slot: 4, .. }))
        );
    }
}

impl Level2SharedSpellResetSession {
    fn acquisition_from_source(
        source: &EntitySnapshot,
        player: &PlayerSnapshot,
    ) -> SpellHolderAcquisitionState {
        let family_index = usize::from(source.model);
        let mut state = SpellHolderAcquisitionState::new(
            source.model,
            source.index,
            player.entity_index,
            source.action,
            player.family_selected_tiers[family_index],
        );
        state.holder_relation = player.enabled_spell_holders[family_index];
        state.eligibility = player.spell_hand_eligibility[family_index];
        state.learned = player.acquired_spell_flags[family_index];
        state.left_spell_slot = player.left_spell_slot_raw;
        state.right_spell_slot = player.right_spell_slot_raw;
        state.left_subspell_index = player.left_subspell_index;
        state.right_subspell_index = player.right_subspell_index;
        state.subspell_index = source.subspell_index;
        state.word_0x30 = source.source_spell_word_30();
        state.byte_0x3b = source.byte_3b;
        state.byte_0x3c = source.byte_3c;
        state.font_type_index = source.font_type_index;
        state.mana_regen = source.mana_regen;
        state.max_mana = source.max_mana;
        state.mana = source.mana;
        state.source_life = source.life;
        state.source_draw_enabled = source.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        state.return_side = source.source_word_4a();
        state
    }

    fn register_starting_acquisition(&mut self, source: &EntitySnapshot, player: &PlayerSnapshot) {
        self.bind_cold_holder_lifecycle(source, player);
        if self
            .acquisitions
            .iter()
            .any(|state| state.family == source.model)
        {
            return;
        }
        // A carried or direct-start holder can become an ordinary pickup on
        // death even when this mission has no authored pickup for its family.
        // Preserve an existing reset source (notably Speed slot 75).
        let mut acquisitions = std::mem::take(&mut self.acquisitions).into_vec();
        acquisitions.push(Self::acquisition_from_source(source, player));
        self.acquisitions = acquisitions.into_boxed_slice();
    }

    /// Starting/carry holders never visit sub_68FF0, but their active callbacks
    /// need the same cold lifecycle that ordinary acquisition installs.
    pub(crate) fn bind_cold_holder_lifecycle(
        &mut self,
        source: &EntitySnapshot,
        player: &PlayerSnapshot,
    ) {
        let parent = &self.player_entity;
        let tier = source.level_gate.cast_unsigned();
        match source.model {
            6 => {
                let mut state = ShieldState::new(
                    tier,
                    source.index,
                    player.entity_index,
                    parent.mana,
                    parent.life,
                )
                .expect("validated starting Shield tier");
                state.parent_mana_regen = parent.mana_regen;
                state.parent_status_flags = parent.control_flags;
                self.shield = Some(state);
            }
            11 => {
                let mut state = InvisibleState::new(
                    tier,
                    source.index,
                    player.entity_index,
                    parent.mana,
                    parent.life,
                )
                .expect("validated starting Invisible tier");
                state.parent_mana_regen = parent.mana_regen;
                state.parent_status_flags = parent.control_flags;
                self.invisible = Some(state);
            }
            12 => {
                let mut state = BeyondSightState::new(
                    tier,
                    source.index,
                    player.entity_index,
                    parent.mana,
                    parent.life,
                )
                .expect("validated starting Beyond Sight tier");
                state.parent_mana_regen = parent.mana_regen;
                self.beyond_sight = Some(state);
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn player_life_regen_for_test(&self) -> i32 {
        self.player_life_regen
    }

    /// Rehydrate the source class-15/player carriers selected by a campaign
    /// descriptor. Missing holder relations remain source nulls (index zero);
    /// no fixed Level-2 slot, allocator, presentation row or RNG draw is used.
    pub(super) fn from_campaign_parts(
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
        runtime: &RuntimeSnapshot,
    ) -> Result<Self> {
        let local_player = players
            .get(usize::from(runtime.local_player_slot))
            .ok_or_else(|| NativeWorldError::new("campaign local player row is absent"))?
            .clone();
        let player_entity = entities
            .get(usize::from(local_player.entity_index))
            .filter(|row| row.active && row.class == 3)
            .ok_or_else(|| NativeWorldError::new("campaign local player entity is absent"))?
            .clone();
        let player_entities = players
            .iter()
            .take(usize::from(runtime.player_count))
            .map(|player| {
                entities
                    .get(usize::from(player.entity_index))
                    .filter(|row| row.active)
                    .cloned()
                    .ok_or_else(|| {
                        NativeWorldError::new(format!(
                            "campaign player entity {} is absent",
                            player.entity_index
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
        let class15 = entities
            .iter()
            .filter(|row| row.active && row.class == 15)
            .cloned()
            .collect::<Vec<_>>();
        let pickups = class15
            .iter()
            .filter(|row| {
                matches!(
                    row.action,
                    10 | 11
                        | 13
                        | 14
                        | 16
                        | 17
                        | 19
                        | 20
                        | 25
                        | 26
                        | 28
                        | 29
                        | 31
                        | 32
                        | 34
                        | 35
                        | 37
                        | 38
                        | 76
                        | 77
                )
            })
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let holders = class15
            .iter()
            .filter(|row| {
                !matches!(
                    row.action,
                    10 | 11
                        | 13
                        | 14
                        | 16
                        | 17
                        | 19
                        | 20
                        | 25
                        | 26
                        | 28
                        | 29
                        | 31
                        | 32
                        | 34
                        | 35
                        | 37
                        | 38
                        | 76
                        | 77
                )
            })
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let to_holder = |row: &EntitySnapshot| SpellActivationHolder {
            entity_index: row.index,
            model: row.model,
            level_gate: row.level_gate,
            active_value: row.active_value,
            activation_value: row.activation_value,
            mana_cost: row.mana_cost,
            action_marker: row.action_marker,
        };
        let null_holder = |family: u8| SpellActivationHolder {
            entity_index: local_player.enabled_spell_holders[usize::from(family)],
            model: family,
            level_gate: 0,
            active_value: 0,
            activation_value: 0,
            mana_cost: 0,
            action_marker: 0,
        };
        let family_holder = |family: u8| {
            holders
                .iter()
                .find(|row| row.model == family)
                .map_or_else(|| null_holder(family), to_holder)
        };
        let acquisitions = pickups
            .iter()
            .chain(holders.iter().filter(|source| {
                matches!(source.model, 3 | 4 | 6 | 7 | 9 | 11..=24)
                    && source.source_spell_parent_entity_index() == local_player.entity_index
                    && !pickups.iter().any(|pickup| pickup.model == source.model)
            }))
            .map(|source| {
                let family = source.model;
                let family_index = usize::from(family);
                let mut state = SpellHolderAcquisitionState::new(
                    family,
                    source.index,
                    player_entity.index,
                    source.action,
                    local_player.family_selected_tiers[family_index],
                );
                state.holder_relation = local_player.enabled_spell_holders[family_index];
                state.eligibility = local_player.spell_hand_eligibility[family_index];
                state.learned = local_player.acquired_spell_flags[family_index];
                state.left_spell_slot = local_player.left_spell_slot_raw;
                state.right_spell_slot = local_player.right_spell_slot_raw;
                state.left_subspell_index = local_player.left_subspell_index;
                state.right_subspell_index = local_player.right_subspell_index;
                state.subspell_index = source.subspell_index;
                state.word_0x30 = source.source_spell_word_30();
                state.byte_0x3b = source.byte_3b;
                state.byte_0x3c = source.byte_3c;
                state.font_type_index = source.font_type_index;
                state.mana_regen = source.mana_regen;
                state.max_mana = source.max_mana;
                state.mana = source.mana;
                state.source_life = source.life;
                state.source_draw_enabled = source.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
                state.return_side = source.source_word_4a();
                state
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let fireball = family_holder(FIREBALL_SPELL_FAMILY);
        let possession = family_holder(POSSESSION_SPELL_FAMILY);
        let speed = family_holder(SPEED_SPELL_FAMILY);
        let caster = SpellCasterState {
            entity_index: player_entity.index,
            model: player_entity.model,
            mana: player_entity.mana,
            control_flags: player_entity.control_flags,
            player_color_index: local_player.player_color_index(),
            control_mode: 0,
        };
        Ok(Self {
            pickups,
            dynamic_pickups: Vec::new(),
            authored_pickup_slots: BTreeMap::new(),
            holders,
            player_mana: player_entity.mana,
            player_life_regen: 0,
            player_life_regen_cooldown: 0,
            player_hit_effect_cooldown: 0,
            player_entity: player_entity.clone(),
            players: players.to_vec().into_boxed_slice(),
            player_entities,
            local_player: local_player.clone(),
            speed_pickup_state: Level2SpeedPickupState::default(),
            caster,
            fireball,
            possession,
            speed,
            shield: None,
            invisible: None,
            global_special_state: GlobalSpecialState::default(),
            beyond_sight: None,
            shield_damage_mailbox: None,
            beyond_sight_auxiliary: Level2PlayerEventAuxiliary {
                entity_index: player_entity.index,
                ..Level2PlayerEventAuxiliary::default()
            },
            acquisitions,
            free_spell_usage: false,
            persistent_spell_experience: local_player.persistent_spell_experience,
            spell_experience: local_player.runtime_spell_experience,
            spell_levels: local_player.spell_unlocked_levels,
            persistent_acquisitions: BTreeMap::new(),
            player_entity_sha256: sha256_hex(player_entity.record_bytes()),
            events: Vec::new(),
            pickup_events: Vec::new(),
            pending_effects: Vec::new(),
        })
    }

    pub(super) fn from_parts_with_starting_spells(
        _terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
        presentation: &PresentationSelectors,
        starting_spells: Option<(&mut Level2EconomyState, &MissionWizardMapSettings)>,
    ) -> Result<Self> {
        const PARAMETER_60: [u8; ENTITY_PARAMETER_BYTES] = [
            1, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255, 255,
            0, 9, 40, 0, 0, 16, 0, 2, 0, 0,
        ];
        const PARTICLE_77: ParticleParameter = ParticleParameter {
            base_tmap: 59,
            word_2: 8492,
            word_4: 42,
            speed: 184,
            rotation_speed: 150,
            byte_10: 0,
            byte_11: -1,
            render_mode: 1,
            byte_13: 0,
        };
        if presentation.entity_parameters.get(60) != Some(&PARAMETER_60)
            || presentation.particles.get(77) != Some(&PARTICLE_77)
            || presentation.level_sprite_selectors.get(59) != Some(&255)
        {
            return Err(NativeWorldError::new(
                "Level 2 shared spell parameter or presentation row changed",
            ));
        }
        let mut rows = Vec::with_capacity(4);
        for (ordinal, slot) in LEVEL2_SHARED_SPELL_PICKUP_SLOTS
            .into_iter()
            .chain(LEVEL2_SHARED_SPELL_HOLDER_SLOTS)
            .enumerate()
        {
            let entity = entities.get(usize::from(slot)).ok_or_else(|| {
                NativeWorldError::new(format!("missing Level 2 shared spell slot {slot}"))
            })?;
            let expected = if ordinal < 2 {
                (
                    15,
                    if slot == 38 { 12 } else { 3 },
                    if slot == 38 { 37 } else { 10 },
                    0x4,
                    (0, 0),
                )
            } else if slot == 113 {
                (15, 0, 0, 0x5, (112, 114))
            } else {
                (15, 1, 3, 0x5, (113, 0))
            };
            if !entity.active
                || (
                    entity.class,
                    entity.model,
                    entity.action,
                    entity.control_flags,
                    (entity.old_map_entity(), entity.next_entity()),
                ) != expected
                || entity.particle_parameter_index != Some(77)
                || entity.parameter_relation != Some(59)
                || sha256_hex(entity.record_bytes()) != LEVEL2_SHARED_SPELL_ENTITY_SHA256[ordinal]
            {
                return Err(NativeWorldError::new(format!(
                    "Level 2 shared spell census row {slot} changed"
                )));
            }
            rows.push(entity.clone());
        }
        let local_player = players
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell local player is absent"))?;
        let baseline_local_player = local_player.clone();
        let player_entity = entities
            .get(usize::from(local_player.entity_index))
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell player entity is absent"))?;
        if !player_entity.active
            || player_entity.class != 3
            || player_entity.model != 0
            || sha256_hex(player_entity.record_bytes()) != LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256
        {
            return Err(NativeWorldError::new(
                "Level 2 shared spell player entity changed",
            ));
        }
        let player_entities = players
            .iter()
            .map(|player| {
                entities
                    .get(usize::from(player.entity_index))
                    .cloned()
                    .ok_or_else(|| {
                        NativeWorldError::new(format!(
                            "Level 2 spell player entity {} is absent",
                            player.entity_index
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
        let pickups = rows[..2].to_vec().into_boxed_slice();
        let holders = rows[2..].to_vec().into_boxed_slice();
        let caster = SpellCasterState {
            entity_index: local_player.entity_index,
            model: player_entity.model,
            mana: player_entity.mana,
            control_flags: player_entity.control_flags,
            player_color_index: 0,
            control_mode: 0,
        };
        let spell_holder = |entity: &EntitySnapshot| SpellActivationHolder {
            entity_index: entity.index,
            model: entity.model,
            level_gate: entity.level_gate,
            active_value: entity.active_value,
            activation_value: entity.activation_value,
            mana_cost: entity.mana_cost,
            action_marker: entity.action_marker,
        };
        let fireball = spell_holder(&holders[0]);
        let possession = spell_holder(&holders[1]);
        let speed = SpellActivationHolder {
            entity_index: 75,
            model: SPEED_SPELL_FAMILY,
            level_gate: 0,
            active_value: 0,
            activation_value: SPEED_SPELL_ACTIVATION_TICKS,
            mana_cost: SPEED_SPELL_MANA_COST,
            action_marker: 0,
        };
        let acquisitions = LEVEL2_GENERIC_SPELL_PICKUPS
            .into_iter()
            .chain(std::iter::once((SPEED_SPELL_FAMILY, 75, 10)))
            .map(|(family, source_slot, source_action)| {
                let family_index = usize::from(family);
                let mut state = SpellHolderAcquisitionState::new(
                    family,
                    source_slot,
                    player_entity.index,
                    source_action,
                    local_player.family_selected_tiers[family_index],
                );
                state.holder_relation = local_player.enabled_spell_holders[family_index];
                state.eligibility = local_player.spell_hand_eligibility[family_index];
                state.learned = local_player.acquired_spell_flags[family_index];
                state.left_spell_slot = local_player.left_spell_slot_raw;
                state.right_spell_slot = local_player.right_spell_slot_raw;
                state.left_subspell_index = local_player.left_subspell_index;
                state.right_subspell_index = local_player.right_subspell_index;
                if let Some(source) = entities.get(usize::from(source_slot)) {
                    state.subspell_index = source.subspell_index;
                    state.word_0x30 = source.source_spell_word_30();
                    state.byte_0x3b = source.byte_3b;
                    state.byte_0x3c = source.byte_3c;
                    state.font_type_index = source.font_type_index;
                    state.mana_regen = source.mana_regen;
                    state.max_mana = source.max_mana;
                    state.mana = source.mana;
                    state.source_life = source.life;
                    state.source_draw_enabled =
                        source.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
                    state.return_side = source.source_word_4a();
                }
                state
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut session = Self {
            pickups,

            dynamic_pickups: Vec::new(),

            authored_pickup_slots: BTreeMap::new(),

            holders,

            player_mana: player_entity.mana,

            player_life_regen: 0,
            player_life_regen_cooldown: 0,
            player_hit_effect_cooldown: 0,

            player_entity: player_entity.clone(),

            players: players.to_vec().into_boxed_slice(),

            player_entities,

            local_player: baseline_local_player,

            speed_pickup_state: Level2SpeedPickupState::default(),

            caster,

            fireball,

            possession,

            speed,

            shield: None,

            invisible: None,
            global_special_state: GlobalSpecialState::default(),

            beyond_sight: None,

            shield_damage_mailbox: None,

            beyond_sight_auxiliary: Level2PlayerEventAuxiliary {
                entity_index: player_entity.index,
                ..Level2PlayerEventAuxiliary::default()
            },

            acquisitions,

            free_spell_usage: false,

            persistent_spell_experience: local_player.persistent_spell_experience,

            spell_experience: local_player.runtime_spell_experience,

            spell_levels: local_player.spell_unlocked_levels,

            persistent_acquisitions: BTreeMap::new(),
            player_entity_sha256: LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256.to_owned(),
            events: Vec::new(),
            pickup_events: Vec::new(),
            pending_effects: Vec::new(),
        };
        if let Some((economy, wizard)) = starting_spells {
            session.materialize_direct_starting_spells(economy, wizard)?;
        }
        Ok(session)
    }

    fn materialize_direct_starting_spells(
        &mut self,
        economy: &mut Level2EconomyState,
        wizard: &MissionWizardMapSettings,
    ) -> Result<()> {
        // The direct-entry branch may seed map gifts; ordinary continuation
        // supplies retained ownership instead. Both then run the same source
        // reset/holder constructor, including allocation-null behavior.
        let learned = (0..PLAYER_SPELL_COUNT)
            .filter(|family| {
                self.local_player.spell_enabled_flags[*family] != 0
                    || (wizard.starting_spells[*family] != 0 && wizard.blocked_spells[*family] == 0)
            })
            .map(|family| family as u8)
            .collect::<Vec<_>>();
        self.materialize_continuation_spell_entry(economy, &learned, &wizard.blocked_spells)
    }

    pub(crate) fn materialize_carried_spell_families(
        &mut self,
        economy: &mut Level2EconomyState,
        families: &[(u8, u8)],
    ) -> Result<()> {
        let economy_before = economy.clone();
        let player_entity_index = self.player_entity.index;
        let local_player_index = self
            .players
            .iter()
            .position(|player| player.entity_index == player_entity_index)
            .ok_or_else(|| NativeWorldError::new("Level 2 local player row is absent"))?;
        let mut rows = Vec::new();
        for &(family, tier) in families {
            let index = usize::from(family);
            if index >= PLAYER_SPELL_COUNT || rows.iter().any(|(seen, _, _)| *seen == family) {
                *economy = economy_before;
                return Err(NativeWorldError::new(format!(
                    "carried spell family {family} is duplicated or out of range"
                )));
            }
            let tier = tier.min(2);
            let stage = Self::castle_stage_for_owner(economy, player_entity_index);
            let Some(profile) =
                self.physical_spell_profile(family, tier, player_entity_index, stage)
            else {
                *economy = economy_before;
                return Err(NativeWorldError::new(format!(
                    "Level 2 direct starting family {family} has no bounded source profile"
                )));
            };
            let Some(source) =
                economy.allocate_starting_spell_holder(family, self.player_entity.position)
            else {
                continue;
            };
            let holder = Self::starting_spell_holder(source, player_entity_index, tier, profile);
            rows.push((family, tier, holder));
        }

        for (_, _, holder) in &mut rows {
            let source_entity_index = holder.index;
            let old_map_entity = economy.map_chains.old_map_entity(source_entity_index);
            let next_entity = economy.map_chains.next_entity(source_entity_index);
            holder.record[26..28].copy_from_slice(&old_map_entity.to_le_bytes());
            holder.record[28..30].copy_from_slice(&next_entity.to_le_bytes());
        }

        let mut player = self.players[local_player_index].clone();
        let mut left = player.left_spell_slot_raw;
        let mut right = player.right_spell_slot_raw;
        for (family, tier, holder) in &rows {
            let index = usize::from(*family);
            player.enabled_spell_holders[index] = holder.index;
            player.acquired_spell_flags[index] = 1;
            player.family_selected_tiers[index] = tier.cast_signed();
            player.spell_unlocked_levels[index] =
                player.spell_unlocked_levels[index].max(tier.cast_signed());
            player.record[268 + index * 2..270 + index * 2]
                .copy_from_slice(&holder.index.to_le_bytes());
            player.record[476 + index] = 1;
            player.record[528 + index] = *tier;
            player.record[502 + index] = player.spell_unlocked_levels[index].cast_unsigned();
            if left == -1 {
                left = i16::from(*family);
                player.left_subspell_index = 0;
            } else if right == -1 {
                right = i16::from(*family);
                player.right_subspell_index = 0;
            }
        }
        player.left_spell_slot_raw = left;
        player.right_spell_slot_raw = right;
        player.left_spell_slot = u8::try_from(left.max(0)).expect("starting left spell fits");
        player.right_spell_slot = u8::try_from(right.max(0)).expect("starting right spell fits");
        player.record[554..556].copy_from_slice(&left.to_le_bytes());
        player.record[556..558].copy_from_slice(&right.to_le_bytes());
        player.record[558] = player.left_subspell_index.cast_unsigned();
        player.record[559] = player.right_subspell_index.cast_unsigned();

        for (_, _, holder) in &rows {
            self.register_starting_acquisition(holder, &player);
        }
        self.dynamic_pickups
            .extend(rows.into_iter().map(|(_, _, holder)| holder));
        self.players[local_player_index] = player.clone();
        self.local_player = player;
        for family in 0..PLAYER_SPELL_COUNT {
            self.spell_levels[family] =
                self.spell_levels[family].max(self.local_player.family_selected_tiers[family]);
        }
        Ok(())
    }

    /// The shared AddSpell constructor used by campaign carry and sub_5CF40.
    /// Start from NewEvent and the allocated slot's RNG, never a dropped
    /// holder's live cooldown, damage or lifecycle state.
    fn starting_spell_holder(
        source: Level2StartingSpellHolderSource,
        player_entity_index: u16,
        tier: u8,
        profile: Level2SetSpellProfile,
    ) -> EntitySnapshot {
        let mut holder = cave_features::new_event(
            source.source_entity_index,
            source.entity_rng,
            source.position,
        );
        holder.index = source.source_entity_index;
        holder.source_id = source.source_entity_index;
        holder.authority = EntityAuthority::SpellActivationLive;
        holder.active = source.active;
        holder.max_life = 0;
        holder.life = 0;
        holder.class = 15;
        holder.model = source.family;
        holder.control_flags = 5;
        holder.entity_rng = source.entity_rng;
        holder.action = source.action;
        holder.action_marker = 0;
        holder.level_gate = tier.cast_signed();
        holder.position = source.position;
        holder.yaw = 75;
        holder.pitch = 768;
        holder.fov = 1280;
        holder.phase = source.phase;
        holder.subspell_index = profile.subspell_index;
        holder.byte_3b = profile.byte_0x3b;
        holder.byte_3c = 0;
        holder.font_type_index = 0;
        holder.active_value = 0;
        holder.activation_value = profile.word_0x30;
        holder.mana_regen = profile.mana_regen;
        holder.max_mana = profile.max_mana;
        holder.mana_cost = profile.max_mana;
        holder.mana = profile.mana;
        holder.particle_parameter_index = Some(77);
        holder.parameter_relation = Some(59);
        holder.next_relation = None;
        holder.special_relation = EntitySpecialRelation::GlobalSpecialState;
        holder.record[0..2].copy_from_slice(&source.source_entity_index.to_le_bytes());
        holder.record[2] = u8::from(source.active);
        holder.record[4..8].fill(0);
        holder.record[8..16].fill(0);
        holder.record[16..20].copy_from_slice(&holder.control_flags.to_le_bytes());
        holder.record[24..26].copy_from_slice(&source.entity_rng.to_le_bytes());
        holder.record[26..30].fill(0);
        holder.record[30..32].copy_from_slice(&source.source_entity_index.to_le_bytes());
        holder.record[32..34].copy_from_slice(&75_i16.to_le_bytes());
        holder.record[34..36].copy_from_slice(&768_i16.to_le_bytes());
        holder.record[36..38].copy_from_slice(&768_i16.to_le_bytes());
        holder.record[38..40].copy_from_slice(&1280_i16.to_le_bytes());
        holder.record[44..46].copy_from_slice(&player_entity_index.to_le_bytes());
        holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
        holder.record[48..50].fill(0);
        holder.record[50..52].fill(0);
        holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
        holder.record[58..60].fill(0);
        holder.record[164..168].copy_from_slice(&60_u32.to_le_bytes());
        holder.record[63] = profile.byte_0x3b;
        holder.record[64] = 0;
        holder.record[65] = 0;
        holder.record[66] = source.phase;
        holder.record[67] = 15;
        holder.record[68] = source.family;
        holder.record[73] = source.action;
        holder.record[74] = tier;
        holder.record[78..80].fill(0);
        holder.record[86..88].copy_from_slice(&75_i16.to_le_bytes());
        holder.record[88..90].copy_from_slice(&768_i16.to_le_bytes());
        holder.record[90..92].copy_from_slice(&768_i16.to_le_bytes());
        holder.record[92..94].copy_from_slice(&1280_i16.to_le_bytes());
        holder.record[94..96].copy_from_slice(&77_i16.to_le_bytes());
        holder.record[96] = 0;
        holder.record[97] = 1; // particle77's source animation limit
        holder.record[80..82].copy_from_slice(&source.position.x.to_le_bytes());
        holder.record[82..84].copy_from_slice(&source.position.y.to_le_bytes());
        holder.record[84..86].copy_from_slice(&source.position.z.to_le_bytes());
        holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
        holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
        holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
        holder
    }

    /// Existing-human-actor branch of sub_5C950/sub_5CF40 (Space action 15).
    /// The caller validates the linked Castle, runs sub_49F90 and relocates
    /// the actor's map-chain entry BEFORE these holder births, then publishes
    /// the new holders and actor into the source arena. No
    /// mission reset, Castle resource write or dropped-holder reclaim occurs.
    pub(crate) fn revive_local_player_at_castle(
        &mut self,
        economy: &mut Level2EconomyState,
        castle_position: Position,
        cap_castle_experience: bool,
    ) -> Result<Vec<EntitySnapshot>> {
        if !self.player_entity.active
            || self.player_entity.life >= 0
            || self.local_player.is_ai
            || self.local_player.entity_index != self.player_entity.index
        {
            return Err(NativeWorldError::new(
                "Castle revival requires the existing dead local human actor",
            ));
        }
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == self.player_entity.index)
            .ok_or_else(|| NativeWorldError::new("Castle revival local player row is absent"))?;

        let mut actor = self.player_entity.clone();
        actor.set_source_action(0);
        actor.set_control_flags((actor.control_flags & !0x20) | 1);
        actor.set_position(castle_position);
        actor.set_max_life(10_000);
        actor.max_mana = 1_000;
        actor.record[144..148].copy_from_slice(&1_000_i32.to_le_bytes());
        // Source fills current life/mana after sub_5CF40. No spell constructor
        // consumes them before this point; cold lifecycle caches need the
        // restored parent values rather than the death projection.
        actor.set_life(10_000);
        actor.set_mana(1_000);
        self.sync_player_entity_from_authority(&actor);
        self.player_mana = actor.mana;
        self.caster.mana = actor.mana;
        self.caster.control_flags = actor.control_flags;

        // These are compact exporter offsets, NOT Type_str_164 raw offsets.
        self.local_player.record[20..24].fill(0); // speed, strafeSpeed
        self.local_player.record[32..38].fill(0); // moveBoost, yaw, fov
        self.local_player.record[565..569].copy_from_slice(&2_048_i32.to_le_bytes());
        self.local_player.record[569..573].copy_from_slice(&2_048_i32.to_le_bytes());
        if let Some(auxiliary) = self.local_player.type_str_164_auxiliary.as_mut() {
            auxiliary.dword_0x16D_365 = 2_000;
            let offset = PLAYER_LEGACY_RECORD_BYTES + 17;
            self.local_player.record[offset..offset + 4].copy_from_slice(&2_000_i32.to_le_bytes());
        }
        self.players[owner_index] = self.local_player.clone();

        // Death leaves a nonzero released sentinel for every surviving
        // family. Allocate ascending families, including sentinel 1. Source
        // allocation failure clears only that relation and continues.
        let mut holders = Vec::new();
        for family in 0..PLAYER_SPELL_COUNT {
            if self.local_player.enabled_spell_holders[family] == 0 {
                continue;
            }
            let family_id = family as u8;
            let Some(source) = economy.allocate_starting_spell_holder(family_id, castle_position)
            else {
                self.set_enabled_spell_holder_index(family, 0);
                continue;
            };
            let stage = Self::castle_stage_for_owner(economy, actor.index);
            let profile = self
                .physical_spell_profile(family_id, 0, actor.index, stage)
                .expect("the source holder allocator validated its base profile");
            let holder = Self::starting_spell_holder(source, actor.index, 0, profile);
            self.set_enabled_spell_holder_index(family, holder.index);
            holders.push(holder);
        }

        // sub_6DB50(0,0): preserve XP except the ordinary Castle cap, refresh
        // unlocked levels and clamp selected tiers without announcing gains.
        if cap_castle_experience && self.local_player.runtime_spell_experience[2] > 7 {
            self.local_player.runtime_spell_experience[2] = 7;
            self.local_player.record[172..176].copy_from_slice(&7_i32.to_le_bytes());
            self.players[owner_index] = self.local_player.clone();
        }
        for family in 0..PLAYER_SPELL_COUNT {
            self.update_spell_experience(actor.index, family as u8, 0, false);
        }
        for (hand, offset) in [
            (self.local_player.left_spell_slot_raw, 558),
            (self.local_player.right_spell_slot_raw, 559),
        ] {
            if let Ok(family) = usize::try_from(hand) {
                if let Some(&tier) = self.local_player.family_selected_tiers.get(family) {
                    self.local_player.record[offset] = tier.cast_unsigned();
                    if offset == 558 {
                        self.local_player.left_subspell_index = tier;
                    } else {
                        self.local_player.right_subspell_index = tier;
                    }
                }
            }
        }
        self.players[owner_index] = self.local_player.clone();
        let player = self.local_player.clone();
        for holder in &mut holders {
            let tier = player.family_selected_tiers[usize::from(holder.model)].cast_unsigned();
            let stage = Self::castle_stage_for_owner(economy, actor.index);
            let profile = self
                .physical_spell_profile(holder.model, tier, actor.index, stage)
                .expect("recomputed selected tier has a source SetSpell profile");
            holder.apply_nyphur_spell_profile(profile, tier);
            holder.set_map_links(
                economy.map_chains.old_map_entity(holder.index),
                economy.map_chains.next_entity(holder.index),
            );
            self.register_starting_acquisition(holder, &player);
            if let Some(existing) = self.pickup_mut(holder.index) {
                *existing = holder.clone();
            } else {
                self.dynamic_pickups.push(holder.clone());
            }
        }
        // sub_5C950 invalidates every other wizard's remembered relationship
        // for this player. It does not change objective banishment flags.
        let relation = 4 + 4 * usize::from(player.player_color_index());
        for other in &mut self.players {
            if self.player_entities.iter().any(|entity| {
                entity.index == other.entity_index
                    && entity.source_id != actor.source_id
                    && matches!(entity.model, 0 | 1)
            }) {
                other.set_type_str_164_array_word(relation, (-24_609_i16).cast_unsigned());
            }
        }
        self.sync_typed_spell_parent_caches_from_player_entity();
        Ok(holders)
    }

    #[must_use]
    pub fn pickups(&self) -> &[EntitySnapshot] {
        &self.pickups
    }

    #[must_use]
    pub fn dynamic_pickups(&self) -> &[EntitySnapshot] {
        &self.dynamic_pickups
    }

    #[must_use]
    pub fn authored_pickup_slots(&self) -> &BTreeMap<u16, u16> {
        &self.authored_pickup_slots
    }

    /// Borrow the source player rows for read-only campaign verification.
    /// Gameplay mutation remains owned by the shared spell route.
    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    /// Borrow the authoritative local player row rather than assuming source
    /// player-array order identifies the command owner.
    #[must_use]
    pub const fn local_player(&self) -> &PlayerSnapshot {
        &self.local_player
    }

    /// Borrow the source local-player entity used by spell activation and
    /// Castle resource upkeep. This is an inspection-only projection.
    #[must_use]
    pub const fn player_entity(&self) -> &EntitySnapshot {
        &self.player_entity
    }

    pub(super) fn pickup(&self, source_slot: u16) -> Option<&EntitySnapshot> {
        self.pickups
            .iter()
            .chain(self.holders.iter())
            .chain(self.dynamic_pickups.iter())
            .find(|entity| entity.index == source_slot)
    }

    pub(super) fn pickup_mut(&mut self, source_slot: u16) -> Option<&mut EntitySnapshot> {
        if let Some(entity) = self
            .pickups
            .iter_mut()
            .find(|entity| entity.index == source_slot)
        {
            return Some(entity);
        }
        if let Some(entity) = self
            .holders
            .iter_mut()
            .find(|entity| entity.index == source_slot)
        {
            return Some(entity);
        }
        self.dynamic_pickups
            .iter_mut()
            .find(|entity| entity.index == source_slot)
    }

    #[must_use]
    pub fn shield_state(&self) -> Option<ShieldState> {
        self.shield
    }

    fn sync_shield_projection(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };

        if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
            holder.action = shield.action;
            holder.active_value = shield.active_value;
            holder.activation_value = shield.activation_value.cast_unsigned();
            holder.record[44..46].copy_from_slice(&shield.parent_entity_index.to_le_bytes());
            holder.record[50..52].copy_from_slice(&shield.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&shield.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&shield.countdown.to_le_bytes());
            holder.record[73] = shield.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != shield.parent_entity_index {
                return;
            }
            entity.life = shield.parent_life;
            entity.mana = shield.parent_mana;
            entity.mana_regen = shield.parent_mana_regen;
            entity.control_flags = shield.parent_status_flags;
            entity.record[12..16].copy_from_slice(&shield.parent_life.to_le_bytes());
            entity.record[16..20].copy_from_slice(&shield.parent_status_flags.to_le_bytes());
            entity.record[140..144].copy_from_slice(&shield.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&shield.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        if shield.parent_entity_index == self.player_entity.index {
            self.player_mana = shield.parent_mana;
        }
    }

    fn apply_shield_cleanup(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };
        let queued_spell = self
            .pickup(shield.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Shield SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(6, tier) {
                if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(shield) = self.shield.as_mut() {
                    shield.tier = tier;
                    shield.activation_value = profile.word_0x30.cast_signed();
                    shield.mana_cost = profile.max_mana;
                }
            } else if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(shield) = self.shield.as_mut() {
            shield.cleanup_requested = false;
        }
    }

    pub fn activate_shield(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(shield) = self.shield.as_mut() else {
            return false;
        };
        let event_start = self.events.len();
        let activated = shield.activate(direction_mask, help_enabled, &mut self.events);
        self.capture_activation_effects(event_start);
        self.sync_shield_projection();
        activated
    }

    /// Submit one source damage mailbox entry for the active family-6 holder.
    /// The outer source pass consumes it at the holder's action-18 callback;
    /// attacker identity is retained for the typed event projection.
    pub fn submit_shield_damage(
        &mut self,
        target_entity_index: u16,
        damage: i32,
        attacker_entity_index: u16,
    ) -> bool {
        if damage <= 0 || attacker_entity_index == 0 {
            return false;
        }
        let Some(shield) = self.shield else {
            return false;
        };
        if shield.parent_entity_index != target_entity_index {
            return false;
        }
        if let Some(pending) = self
            .shield_damage_mailbox
            .as_mut()
            .filter(|pending| pending.target_entity_index == target_entity_index)
        {
            pending.damage = pending.damage.wrapping_add(damage);
            pending.attacker_entity_index = attacker_entity_index;
        } else {
            self.shield_damage_mailbox = Some(Level2ShieldDamageMailbox {
                target_entity_index,
                damage,
                attacker_entity_index,
            });
        }
        true
    }

    /// Consume one source hit through the active family-6 Shield status
    /// branches (`sub_6A480`/`sub_68DE0` consumer projection).  The typed
    /// Shield state remains authoritative, then the holder and parent entity
    /// records are synchronized through the same production projection used
    /// by outer-turn expiry.  A missing Shield is a bounded no-op rather than
    /// a fallback to a neighboring spell family.
    pub fn absorb_shield_damage(&mut self, raw_damage: i32) -> Option<ShieldDamage> {
        let shield = self.shield.as_mut()?;
        let damage = shield.absorb_damage(raw_damage);
        self.sync_shield_projection();
        Some(damage)
    }

    pub(super) fn absorb_player_shield_damage(
        &mut self,
        target_entity_index: u16,
        raw_damage: i32,
        attacker_entity_index: u16,
    ) -> Option<ShieldDamage> {
        let shield = self.shield?;
        if shield.parent_entity_index != target_entity_index
            || raw_damage <= 0
            || shield.parent_status_flags
                & (crate::spell_activation::SHIELD_MANA_STATUS_BIT
                    | crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT)
                == 0
        {
            return None;
        }
        let damage = self.shield.as_mut()?.absorb_damage(raw_damage);
        self.sync_shield_projection();
        self.emit_shared_spell_effect(Level2SharedSpellEvent::ShieldDamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage,
        });
        Some(damage)
    }

    fn consume_shield_damage_mailbox(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };
        let Some(mailbox) = self
            .shield_damage_mailbox
            .filter(|mailbox| mailbox.target_entity_index == shield.parent_entity_index)
        else {
            return;
        };
        self.shield_damage_mailbox = None;
        if let Some(damage) = self.absorb_shield_damage(mailbox.damage) {
            self.emit_shared_spell_effect(Level2SharedSpellEvent::ShieldDamageApplied {
                target_entity_index: mailbox.target_entity_index,
                attacker_entity_index: mailbox.attacker_entity_index,
                damage,
            });
        }
    }

    fn step_shield_source_slot(&mut self, source_slot: u16) {
        if self
            .shield
            .is_none_or(|shield| shield.holder_entity_index != source_slot)
        {
            return;
        }
        // The public ABI adapter retains its accepted holder-side consumer.
        // Automatic collision does not use this side mailbox; it writes the
        // player entity mailbox and is evaluated at the player source visit.
        self.consume_shield_damage_mailbox();
        if let Some(shield) = self.shield.as_mut() {
            let _step: ShieldStep = shield.step();
        }
        self.sync_shield_projection();
        if self.shield.is_some_and(|shield| shield.cleanup_requested) {
            self.apply_shield_cleanup();
            self.sync_shield_projection();
        }
    }

    #[must_use]
    pub fn invisible_state(&self) -> Option<InvisibleState> {
        self.invisible
    }

    pub fn activate_invisible(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(invisible) = self.invisible.as_mut() else {
            return false;
        };
        let event_start = self.events.len();
        let activated = invisible.activate(direction_mask, help_enabled, &mut self.events);
        self.capture_activation_effects(event_start);
        self.sync_invisible_projection();
        activated
    }

    fn sync_invisible_projection(&mut self) {
        let Some(invisible) = self.invisible else {
            return;
        };

        if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
            holder.action = invisible.action;
            holder.active_value = invisible.active_value;
            holder.activation_value = invisible.activation_value.cast_unsigned();
            holder.control_flags = (holder.control_flags & !INVISIBLE_STATUS_BIT)
                | (invisible.spell_status_flags & INVISIBLE_STATUS_BIT);
            holder.record[50..52].copy_from_slice(&invisible.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&invisible.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&invisible.countdown.to_le_bytes());
            holder.record[16..20].copy_from_slice(&holder.control_flags.to_le_bytes());
            holder.record[73] = invisible.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != invisible.parent_entity_index {
                return;
            }
            entity.life = invisible.parent_life;
            entity.mana = invisible.parent_mana;
            entity.mana_regen = invisible.parent_mana_regen;
            entity.control_flags = invisible.parent_status_flags;
            entity.record[12..16].copy_from_slice(&invisible.parent_life.to_le_bytes());
            entity.record[16..20].copy_from_slice(&invisible.parent_status_flags.to_le_bytes());
            entity.record[140..144].copy_from_slice(&invisible.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&invisible.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        if invisible.parent_entity_index == self.player_entity.index {
            self.player_mana = invisible.parent_mana;
        }
    }

    /// Find a generic command mutation that aliases one already-modeled typed
    /// lifecycle. The C ABI command context is local-only, so both holder and
    /// caster identities must agree before it can hand state back to a source
    /// lifecycle.
    fn matching_typed_spell_fire_holder(
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
        parent_entity_index: u16,
        holder_entity_index: u16,
        family: u8,
    ) -> Option<SpellActivationHolder> {
        if caster.entity_index != parent_entity_index {
            return None;
        }
        holders
            .iter()
            .copied()
            .find(|holder| holder.entity_index == holder_entity_index && holder.model == family)
    }

    /// Mirror the physical SetSpell payload, including cold menu assignments.
    /// An active assignment changes only the queued word, so reading the
    /// physical tier here preserves the running spell until cleanup.
    pub(super) fn sync_typed_spell_profile_from_holder(&mut self, holder_index: u16) {
        let Some(holder) = self.pickup(holder_index) else {
            return;
        };
        let Ok(tier @ 0..=2) = u8::try_from(holder.level_gate) else {
            return;
        };
        let family = holder.model;
        let activation_value = holder.activation_value.cast_signed();
        let mana_cost = holder.max_mana;
        let mana_regen_requirement = holder.mana_regen;
        match family {
            SHIELD_FAMILY => {
                if let Some(state) = self
                    .shield
                    .as_mut()
                    .filter(|state| state.holder_entity_index == holder_index)
                {
                    state.tier = tier;
                    state.activation_value = activation_value;
                    state.mana_cost = mana_cost;
                }
            }
            INVISIBLE_FAMILY => {
                if let Some(state) = self
                    .invisible
                    .as_mut()
                    .filter(|state| state.holder_entity_index == holder_index)
                {
                    state.tier = tier;
                    state.activation_value = activation_value;
                    state.mana_cost = mana_cost;
                    state.mana_regen_requirement = mana_regen_requirement;
                }
            }
            BEYOND_SIGHT_FAMILY => {
                if let Some(state) = self
                    .beyond_sight
                    .as_mut()
                    .filter(|state| state.holder_entity_index == holder_index)
                {
                    state.tier = tier;
                    state.activation_value = activation_value;
                    state.mana_cost = mana_cost;
                    state.mana_regen_requirement = mana_regen_requirement;
                }
            }
            _ => {}
        }
    }

    /// `sub_5F660` mutates the shared command holder after the typed source
    /// callback. Reconcile the matching family-6 alias so the next ordinary
    /// `sub_6A480` visit sees the same active state; remote holders never
    /// enter this local command mirror.
    pub(super) fn sync_shield_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(shield) = self.shield else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            shield.parent_entity_index,
            shield.holder_entity_index,
            SHIELD_FAMILY,
        ) else {
            return;
        };
        self.sync_typed_spell_profile_from_holder(holder.entity_index);
        let shield = self.shield.as_mut().expect("Shield state remains present");
        shield.active_value = holder.active_value;
        shield.activation_value = holder.activation_value.cast_signed();
        shield.parent_mana = caster.mana;
        shield.parent_status_flags = caster.control_flags & !0x3;
        if holder.active_value != 0 {
            shield.action = SHIELD_ACTIVE_ACTION;
            shield.cleanup_requested = false;
        }
        self.sync_shield_projection();
    }

    /// `sub_5F660` mutates the shared command holder before the next
    /// family-11 source visit. Keep the existing typed lifecycle as the
    /// authority for that following `sub_6B1C0` pulse by reconciling only the
    /// matching local holder; remote player holders never enter this command
    /// mirror.
    pub(super) fn sync_invisible_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(invisible) = self.invisible else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            invisible.parent_entity_index,
            invisible.holder_entity_index,
            INVISIBLE_FAMILY,
        ) else {
            return;
        };

        self.sync_typed_spell_profile_from_holder(holder.entity_index);
        let invisible = self
            .invisible
            .as_mut()
            .expect("Invisible state remains present");
        invisible.active_value = holder.active_value;
        invisible.activation_value = holder.activation_value.cast_signed();
        invisible.parent_mana = caster.mana;
        invisible.parent_status_flags = caster.control_flags & !0x3;
        if holder.active_value != 0 {
            invisible.action = INVISIBLE_ACTIVE_ACTION;
            invisible.cleanup_requested = false;
        }
        self.sync_invisible_projection();
    }

    fn apply_invisible_cleanup(&mut self) {
        let Some(invisible) = self.invisible else {
            return;
        };
        let queued_spell = self
            .pickup(invisible.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Invisible SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(11, tier) {
                if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(invisible) = self.invisible.as_mut() {
                    invisible.tier = tier;
                    invisible.activation_value = profile.word_0x30.cast_signed();
                    invisible.mana_cost = profile.max_mana;
                    invisible.mana_regen_requirement = profile.mana_regen;
                }
            } else if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(invisible) = self.invisible.as_mut() {
            invisible.cleanup_requested = false;
        }
    }

    pub(super) fn step_invisible_source_slot(
        &mut self,
        source_slot: u16,
        parent_castle_mana: Option<i32>,
    ) {
        if self
            .invisible
            .is_none_or(|invisible| invisible.holder_entity_index != source_slot)
        {
            return;
        }
        if let Some((parent, step)) = self.invisible.as_mut().map(|invisible| {
            (
                invisible.parent_entity_index,
                invisible.step(parent_castle_mana),
            )
        }) {
            if let Some(write) = step.special_state_write {
                if write.clear_protection {
                    self.global_special_state.protection_ticks = 0;
                }
                self.global_special_state.visibility_mode = write.visibility_mode;
            }
            if step.experience_awarded != 0 {
                self.award_spell_experience(parent, 11, step.experience_awarded);
            }
        }
        self.sync_invisible_projection();
        if self
            .invisible
            .is_some_and(|invisible| invisible.cleanup_requested)
        {
            self.apply_invisible_cleanup();
            self.sync_invisible_projection();
        }
    }

    #[must_use]
    pub fn beyond_sight_state(&self) -> Option<BeyondSightState> {
        self.beyond_sight
    }

    #[must_use]
    pub const fn beyond_sight_auxiliary(&self) -> Level2PlayerEventAuxiliary {
        self.beyond_sight_auxiliary
    }

    /// Bind the latched player-input words to the same transient fields that
    /// `PlayerEvents_51BB0` writes before the source entity pass.
    pub fn set_beyond_sight_player_input(
        &mut self,
        entity_index: u16,
        next_entity: u16,
        entity_index2: u16,
    ) {
        self.beyond_sight_auxiliary.entity_index = entity_index;
        self.beyond_sight_auxiliary.next_entity = next_entity.cast_signed();
        self.beyond_sight_auxiliary.entity_index2 = entity_index2.cast_signed();
    }

    pub fn activate_beyond_sight(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(beyond_sight) = self.beyond_sight.as_mut() else {
            return false;
        };
        let event_start = self.events.len();
        let activated = beyond_sight.activate(direction_mask, help_enabled, &mut self.events);
        self.capture_activation_effects(event_start);
        self.sync_beyond_sight_projection();
        activated
    }

    fn sync_beyond_sight_projection(&mut self) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };

        if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
            holder.action = beyond_sight.action;
            holder.active_value = beyond_sight.active_value;
            holder.activation_value = beyond_sight.activation_value.cast_unsigned();
            holder.record[50..52].copy_from_slice(&beyond_sight.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&beyond_sight.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&beyond_sight.countdown.to_le_bytes());
            holder.record[73] = beyond_sight.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != beyond_sight.parent_entity_index {
                return;
            }
            entity.life = beyond_sight.parent_life;
            entity.mana = beyond_sight.parent_mana;
            entity.mana_regen = beyond_sight.parent_mana_regen;
            entity.record[12..16].copy_from_slice(&beyond_sight.parent_life.to_le_bytes());
            entity.record[140..144].copy_from_slice(&beyond_sight.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&beyond_sight.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        if beyond_sight.parent_entity_index == self.player_entity.index {
            self.player_mana = beyond_sight.parent_mana;
        }
    }

    /// Copy the post-upkeep entity-111 values into local typed parent caches
    /// without invoking their projection writers. The next source callback
    /// may legitimately consume the fresh mana/regen, while a projection
    /// here would replay the previous callback's stale cache over the player.
    pub(super) fn sync_typed_spell_parent_caches_from_player_entity(&mut self) {
        let parent_entity_index = self.player_entity.index;
        let parent_life = self.player_entity.life;
        let parent_mana = self.player_entity.mana;
        let parent_mana_regen = self.player_entity.mana_regen;
        let parent_status_flags = self.player_entity.control_flags;
        if let Some(shield) = self
            .shield
            .as_mut()
            .filter(|shield| shield.parent_entity_index == parent_entity_index)
        {
            shield.parent_life = parent_life;
            shield.parent_mana = parent_mana;
            shield.parent_mana_regen = parent_mana_regen;
            shield.parent_status_flags = parent_status_flags;
        }
        if let Some(invisible) = self
            .invisible
            .as_mut()
            .filter(|invisible| invisible.parent_entity_index == parent_entity_index)
        {
            invisible.parent_life = parent_life;
            invisible.parent_mana = parent_mana;
            invisible.parent_mana_regen = parent_mana_regen;
            invisible.parent_status_flags = parent_status_flags;
        }
        if let Some(beyond_sight) = self
            .beyond_sight
            .as_mut()
            .filter(|beyond_sight| beyond_sight.parent_entity_index == parent_entity_index)
        {
            beyond_sight.parent_life = parent_life;
            beyond_sight.parent_mana = parent_mana;
            beyond_sight.parent_mana_regen = parent_mana_regen;
        }
    }

    /// Reconcile the generic `sub_5F660` holder mutation with the modeled
    /// family-12 lifecycle before its following ordinary source pulse. The
    /// typed state remains the authority for the later pulse/effect path.
    pub(super) fn sync_beyond_sight_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            beyond_sight.parent_entity_index,
            beyond_sight.holder_entity_index,
            BEYOND_SIGHT_FAMILY,
        ) else {
            return;
        };
        let beyond_sight = self
            .beyond_sight
            .as_mut()
            .expect("Beyond Sight state remains present");
        beyond_sight.active_value = holder.active_value;
        beyond_sight.activation_value = holder.activation_value.cast_signed();
        beyond_sight.parent_mana = caster.mana;
        if holder.active_value != 0 {
            beyond_sight.action = BEYOND_SIGHT_ACTIVE_ACTION;
            beyond_sight.cleanup_requested = false;
        }
        self.sync_beyond_sight_projection();
    }

    fn apply_beyond_sight_cleanup(&mut self) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };
        let queued_spell = self
            .pickup(beyond_sight.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Beyond Sight SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(BEYOND_SIGHT_FAMILY, tier) {
                if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(beyond_sight) = self.beyond_sight.as_mut() {
                    beyond_sight.tier = tier;
                    beyond_sight.activation_value = profile.word_0x30.cast_signed();
                    beyond_sight.mana_cost = profile.max_mana;
                    beyond_sight.mana_regen_requirement = profile.mana_regen;
                }
            } else if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(beyond_sight) = self.beyond_sight.as_mut() {
            beyond_sight.cleanup_requested = false;
        }
    }

    fn step_beyond_sight_source_slot(&mut self, source_slot: u16, parent_castle_mana: Option<i32>) {
        if self
            .beyond_sight
            .is_none_or(|beyond_sight| beyond_sight.holder_entity_index != source_slot)
        {
            return;
        }
        let Some(mut beyond_sight) = self.beyond_sight.take() else {
            return;
        };
        if beyond_sight.holder_entity_index != source_slot {
            self.beyond_sight = Some(beyond_sight);
            return;
        }
        let parent_entity_index = beyond_sight.parent_entity_index;
        let experience_awarded = beyond_sight.award_experience_if_fresh_pulse();
        if experience_awarded != 0 {
            // `sub_6B310` orders sub_6D8B0 before sub_68D50. Publish the
            // owner row and threshold presentation before the predicate
            // can collapse a failed fresh pulse.
            self.award_beyond_sight_experience(parent_entity_index, experience_awarded);
        }
        let _step =
            beyond_sight.step_after_experience_award(parent_castle_mana, experience_awarded);
        self.beyond_sight = Some(beyond_sight);
        // `sub_6B310` ends after its own predicate/charge/countdown and
        // never dispatches the separate class-10/model-74/action-72
        // machinery.
        self.sync_beyond_sight_projection();
        if self
            .beyond_sight
            .is_some_and(|beyond_sight| beyond_sight.cleanup_requested)
        {
            self.apply_beyond_sight_cleanup();
            self.sync_beyond_sight_projection();
        }
    }

    /// Materialise one authored class-15 row from `sub_4A1E0`'s group pass.
    /// The source constructor is deliberately explicit: the row is absent at
    /// reset, receives a shared-arena slot/RNG, and only then enters the
    /// generic `sub_68FF0` acquisition scan.
    pub fn materialize_authored_spell_pickup(
        &mut self,
        economy: &mut Level2EconomyState,
        authored: AuthoredFeature,
        terrain: &SessionTerrain,
        particles: &[ParticleParameter],
    ) -> Result<Option<u16>> {
        if let Some(source_slot) = self.authored_pickup_slots.get(&authored.index) {
            return Ok(Some(*source_slot));
        }
        let family = match authored.subtype {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            6 => 6,
            7 => 7,
            8 => 8,
            9 => 9,
            10 => 10,
            11 => 11,
            12 => 12,
            13..=24 => authored.subtype as u8,
            25 => 25,
            _ => {
                return Err(NativeWorldError::new(format!(
                    "unsupported authored spell subtype {} at row {}",
                    authored.subtype, authored.index
                )));
            }
        };
        if authored.kind != 15 || !matches!(authored.stage_tag, 1 | 2) {
            return Err(NativeWorldError::new(format!(
                "authored spell row {} is not an ordinary/persistent class-15 source",
                authored.index
            )));
        }
        let ordinary_action =
            physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup)
                .expect("validated authored family has an ordinary action");
        let profile = level2_set_spell_profile(family, 0).ok_or_else(|| {
            NativeWorldError::new(format!(
                "authored spell family {family} has no constructor profile"
            ))
        })?;
        // AddSpellXX starts from NewEvent, including when the mission has
        // no initial spell rows. Its presentation comes from particle77.
        let particle = *particles
            .get(77)
            .ok_or_else(|| NativeWorldError::new("authored spell requires retained particle77"))?;
        let position = authored.world_position(terrain.ground_height(authored.world_position(0)));
        let Some(mut source) =
            economy.allocate_spell_pickup(authored.index, family, ordinary_action, position)
        else {
            // `sub_4A1E0` consumes this authored row and continues when
            // NewEvent returns null; callers distinguish that source result
            // from semantic adapter errors carried by `Err`.
            return Ok(None);
        };
        // sub_4A310 constructs the ordinary row, then source stage 2 adds
        // one to its action. This uses the same NewEvent/RNG/map publication.
        if authored.stage_tag == 2 {
            source.action =
                physical_spell_action(family, PhysicalSpellActionRole::PersistentPickup)
                    .expect("validated family has a persistent action");
            if !economy.sync_spell_pickup_projection(
                source.source_entity_index,
                source.action,
                position,
            ) {
                return Err(NativeWorldError::new(
                    "persistent authored spell lost its allocated row",
                ));
            }
        }
        let mut entity = cave_features::new_event(
            source.source_entity_index,
            source.entity_rng,
            source.position,
        );
        cave_features::bind_particle(&mut entity, 77, particle).map_err(NativeWorldError::new)?;
        entity.index = source.source_entity_index;
        entity.source_id = source.source_entity_index;
        entity.authority = EntityAuthority::SpellActivationLive;
        entity.active = true;
        entity.max_life = 0;
        entity.life = 0;
        entity.class = 15;
        entity.model = source.family;
        entity.control_flags = 4;
        entity.entity_rng = source.entity_rng;
        entity.action = source.action;
        entity.action_marker = 0;
        entity.level_gate = 0;
        entity.position = source.position;
        entity.yaw = 0;
        entity.pitch = 0;
        entity.fov = 0;
        entity.phase = source.phase;
        entity.subspell_index = profile.subspell_index;
        entity.byte_3b = profile.byte_0x3b;
        entity.byte_3c = 0;
        entity.font_type_index = 0;
        entity.active_value = 0;
        entity.activation_value = profile.word_0x30;
        entity.mana_regen = profile.mana_regen;
        entity.max_mana = profile.max_mana;
        entity.mana_cost = profile.max_mana;
        entity.mana = profile.mana;
        entity.particle_parameter_index = Some(77);
        entity.parameter_relation = Some(59);
        entity.next_relation = None;
        entity.special_relation = EntitySpecialRelation::GlobalSpecialState;
        entity.record[0..2].copy_from_slice(&source.source_entity_index.to_le_bytes());
        entity.record[2] = 1;
        entity.record[4..8].fill(0);
        entity.record[8..16].fill(0);
        entity.record[16..20].copy_from_slice(&4_u32.to_le_bytes());
        entity.record[24..26].copy_from_slice(&source.entity_rng.to_le_bytes());
        entity.record[26..30].fill(0);
        entity.record[30..32].copy_from_slice(&source.source_entity_index.to_le_bytes());
        entity.record[32..40].fill(0);
        entity.record[44..46].fill(0);
        entity.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
        entity.record[48..52].fill(0);
        entity.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
        entity.record[58..60].fill(0);
        entity.record[63] = profile.byte_0x3b;
        entity.record[64..66].fill(0);
        entity.record[66] = source.phase;
        entity.record[67] = 15;
        entity.record[68] = source.family;
        entity.record[73] = source.action;
        entity.record[74] = 0;
        entity.record[78..80].fill(0);
        entity.record[80..82].copy_from_slice(&source.position.x.to_le_bytes());
        entity.record[82..84].copy_from_slice(&source.position.y.to_le_bytes());
        entity.record[84..86].copy_from_slice(&source.position.z.to_le_bytes());
        entity.record[88..90].copy_from_slice(&768_i16.to_le_bytes());
        entity.record[90..92].copy_from_slice(&768_i16.to_le_bytes());
        entity.record[92..94].copy_from_slice(&1280_i16.to_le_bytes());
        entity.record[94..96].copy_from_slice(&77_i16.to_le_bytes());
        entity.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
        entity.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
        entity.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
        entity.record[164..168].copy_from_slice(&60_u32.to_le_bytes());
        entity.record[168] = 2;
        entity.record[169..172].fill(0);
        entity.record[172..176].copy_from_slice(&1_u32.to_le_bytes());
        self.dynamic_pickups.push(entity.clone());
        self.authored_pickup_slots
            .insert(authored.index, source.source_entity_index);
        let index = if let Some(index) = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
        {
            index
        } else {
            let family_index = usize::from(family);
            let mut state = SpellHolderAcquisitionState::new(
                family,
                source.source_entity_index,
                self.player_entity.index,
                source.action,
                self.local_player.family_selected_tiers[family_index],
            );
            state.holder_relation = self.local_player.enabled_spell_holders[family_index];
            state.eligibility = self.local_player.spell_hand_eligibility[family_index];
            state.learned = self.local_player.acquired_spell_flags[family_index];
            state.left_spell_slot = self.local_player.left_spell_slot_raw;
            state.right_spell_slot = self.local_player.right_spell_slot_raw;
            state.left_subspell_index = self.local_player.left_subspell_index;
            state.right_subspell_index = self.local_player.right_subspell_index;
            let mut acquisitions = std::mem::take(&mut self.acquisitions).into_vec();
            acquisitions.push(state);
            let index = acquisitions.len() - 1;
            self.acquisitions = acquisitions.into_boxed_slice();
            index
        };
        let state = &mut self.acquisitions[index];
        state.source_slot = source.source_entity_index;
        state.authored_source_index = authored.index;
        state.source_action = source.action;
        state.source_life = entity.life;
        state.source_draw_enabled = true;
        state.subspell_index = entity.subspell_index;
        state.word_0x30 = entity.source_spell_word_30();
        state.byte_0x3b = entity.byte_3b;
        state.byte_0x3c = entity.byte_3c;
        state.font_type_index = entity.font_type_index;
        state.mana_regen = entity.mana_regen;
        state.max_mana = entity.max_mana;
        state.mana = entity.mana;
        state.return_side = entity.source_word_4a();
        Ok(Some(source.source_entity_index))
    }

    /// Publish the source epilogue of an acquired starting spell into its
    /// NewEvent carrier. Its family callback already owns casting and charge.
    pub(crate) fn finish_dynamic_starting_spell_holder(
        &mut self,
        economy: &mut Level2EconomyState,
        source_slot: u16,
    ) -> Result<()> {
        let holder = self
            .pickup(source_slot)
            .filter(|holder| {
                holder.active
                    && holder.class == 15
                    && holder.model <= 2
                    && physical_spell_action(holder.model, PhysicalSpellActionRole::Active)
                        == Some(holder.action)
            })
            .ok_or_else(|| {
                NativeWorldError::new("dynamic starting holder lost its current spell")
            })?;
        let (family, action, position, phase) = (
            holder.model,
            holder.action,
            holder.position,
            holder.phase.wrapping_add(1),
        );
        if let Some(source) = economy.spell_pickups.iter_mut().find(|source| {
            source.active && source.source_entity_index == source_slot && source.family == family
        }) {
            source.action = action;
            source.position = position;
            source.phase = phase;
        } else if let Some(source) = economy.spell_replacements.iter_mut().find(|source| {
            source.active && source.source_entity_index == source_slot && source.family == family
        }) {
            source.action = action;
            source.position = position;
            source.phase = phase;
        } else {
            return Err(NativeWorldError::new(
                "dynamic starting holder lost its NewEvent carrier",
            ));
        }
        let holder = self
            .pickup_mut(source_slot)
            .expect("validated current starting holder");
        holder.phase = phase;
        holder.record[66] = phase;
        Ok(())
    }

    /// Bind an economy-owned `sub_69250` replacement to the shared spell
    /// source projection. In remc2 the newly allocated class-15 row is a live
    /// persistent pickup in the same entity arena; keeping an entity record
    /// here lets the normal collision/hand transition inspect its action and
    /// position on the next cursor pass.
    pub(super) fn materialize_spell_replacement_pickup(
        &mut self,
        replacement: Level2SpellReplacement,
    ) -> Result<()> {
        let family_index = self
            .acquisitions
            .iter()
            .position(|state| state.family == replacement.family)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "unsupported Level 2 persistent spell family {}",
                    replacement.family
                ))
            })?;
        if self
            .dynamic_pickups
            .iter()
            .any(|entity| entity.index == replacement.source_entity_index)
        {
            return Ok(());
        }
        let source = self
            .pickup(self.acquisitions[family_index].source_slot)
            .ok_or_else(|| NativeWorldError::new("persistent spell source is absent"))?;
        // sub_69250 invokes AddSpellXX again, not a copy of an unrelated
        // reset pickup. Retain only particle77's owner-bound presentation.
        let mut entity = cave_features::new_event(
            replacement.source_entity_index,
            replacement.entity_rng,
            replacement.position,
        );
        entity.particle_parameter_index = source.particle_parameter_index;
        entity.record[86..98].copy_from_slice(&source.record[86..98]);
        entity.index = replacement.source_entity_index;
        entity.source_id = replacement.source_entity_index;
        entity.authority = EntityAuthority::SpellActivationLive;
        entity.active = replacement.active;
        entity.max_life = 0;
        entity.life = 0;
        entity.class = 15;
        entity.model = replacement.family;
        entity.control_flags = replacement.control_flags | 4;
        entity.entity_rng = replacement.entity_rng;
        entity.action = replacement.action;
        entity.action_marker = 0;
        entity.level_gate = 0;
        entity.position = replacement.position;
        entity.phase = replacement.phase;
        entity.subspell_index = 0;
        entity.byte_3b = 0;
        entity.byte_3c = 0;
        entity.font_type_index = 0;
        entity.active_value = 0;
        entity.activation_value = 0;
        entity.mana_regen = 0;
        entity.max_mana = 0;
        entity.mana_cost = 0;
        entity.mana = 0;
        entity.next_relation = None;
        entity.special_relation = EntitySpecialRelation::None;
        entity.record[0..2].copy_from_slice(&replacement.source_entity_index.to_le_bytes());
        entity.record[2] = 1;
        entity.record[4..8].fill(0);
        entity.record[8..16].fill(0);
        entity.record[16..20].copy_from_slice(&entity.control_flags.to_le_bytes());
        entity.record[24..26].copy_from_slice(&replacement.entity_rng.to_le_bytes());
        entity.record[26..30].fill(0);
        entity.record[30..32].copy_from_slice(&replacement.source_entity_index.to_le_bytes());
        entity.record[46..54].fill(0);
        entity.record[58..60].fill(0);
        entity.record[63..66].fill(0);
        entity.record[66] = replacement.phase;
        entity.record[67] = 15;
        entity.record[68] = replacement.family;
        entity.record[73] = replacement.action;
        entity.record[74] = 0;
        entity.record[78..80].fill(0);
        entity.record[80..82].copy_from_slice(&replacement.position.x.to_le_bytes());
        entity.record[82..84].copy_from_slice(&replacement.position.y.to_le_bytes());
        entity.record[84..86].copy_from_slice(&replacement.position.z.to_le_bytes());
        entity.record[140..152].fill(0);
        let profile = level2_set_spell_profile(replacement.family, 0).ok_or_else(|| {
            NativeWorldError::new("persistent spell constructor profile is absent")
        })?;
        entity.apply_nyphur_spell_profile(profile, 0);
        self.dynamic_pickups.push(entity.clone());

        let mut state = self.acquisitions[family_index];
        state.source_slot = replacement.source_entity_index;
        state.authored_source_index = 0;
        state.source_action = replacement.action;
        state.source_life = entity.life;
        state.source_draw_enabled = true;
        state.subspell_index = entity.subspell_index;
        state.word_0x30 = entity.source_spell_word_30();
        state.byte_0x3b = entity.byte_3b;
        state.byte_0x3c = entity.byte_3c;
        state.font_type_index = entity.font_type_index;
        state.mana_regen = entity.mana_regen;
        state.max_mana = entity.max_mana;
        state.mana = entity.mana;
        state.return_side = entity.source_word_4a();
        self.persistent_acquisitions
            .insert(replacement.source_entity_index, state);
        Ok(())
    }

    #[must_use]
    pub fn holders(&self) -> &[EntitySnapshot] {
        &self.holders
    }

    #[must_use]
    pub(super) const fn enabled_spell_holder_indices(&self) -> [u16; PLAYER_SPELL_COUNT] {
        self.local_player.enabled_spell_holders
    }

    pub(super) fn set_enabled_spell_holder_index(&mut self, family: usize, value: u16) {
        let Some(slot) = self.local_player.enabled_spell_holders.get_mut(family) else {
            return;
        };
        *slot = value;
        let offset = 268 + family * 2;
        self.local_player.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        if let Some(player) = self
            .players
            .iter_mut()
            .find(|player| player.slot == self.local_player.slot)
        {
            player.enabled_spell_holders[family] = value;
            player.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    /// Replace every shared projection of one imported holder from the
    /// authoritative source-arena row after the player-death release writes.
    pub(super) fn sync_released_holder(&mut self, source: &EntitySnapshot) {
        for entity in self
            .pickups
            .iter_mut()
            .chain(self.dynamic_pickups.iter_mut())
            .chain(self.holders.iter_mut())
            .chain(self.player_entities.iter_mut())
        {
            if entity.index == source.index {
                *entity = source.clone();
            }
        }
    }

    /// Refresh the shared spell projection from the one source-arena row
    /// before dispatch. The projection remains a typed convenience view; it
    /// must never become a second independently mutable entity authority.
    pub(super) fn sync_pickup_from_authority(&mut self, source: &EntitySnapshot) {
        self.sync_released_holder(source);
        for state in &mut self.acquisitions {
            if state.source_slot != source.index {
                continue;
            }
            state.source_action = source.action;
            state.source_life = source.life;
            state.source_draw_enabled = !source.drawing_disabled();
            state.subspell_index = source.subspell_index;
            state.word_0x30 = source.source_spell_word_30();
            state.byte_0x3b = source.byte_3b;
            state.byte_0x3c = source.byte_3c;
            state.font_type_index = source.font_type_index;
            state.mana_regen = source.mana_regen;
            state.max_mana = source.max_mana;
            state.mana = source.mana;
            state.return_side = source.source_word_4a();
        }
    }

    pub(super) fn increment_player_kill_score(&mut self, entity_index: u16, color: usize) -> bool {
        let mut changed = false;
        for player in &mut self.players {
            if player.entity_index == entity_index {
                changed |= player.increment_kill_score_for_color(color);
                if player.slot == self.local_player.slot {
                    self.local_player = player.clone();
                }
            }
        }
        changed
    }

    pub(super) fn sync_player_entity_from_authority(&mut self, source: &EntitySnapshot) {
        if self.player_entity.index == source.index {
            self.player_entity = source.clone();
        }
        for entity in &mut self.player_entities {
            if entity.index == source.index {
                *entity = source.clone();
            }
        }
    }

    pub(super) fn clear_local_player_banished_camera_feedback(&mut self) {
        let local_slot = self.local_player.slot;
        self.local_player.clear_banished_camera_feedback();
        for player in &mut self.players {
            if player.slot == local_slot {
                player.clear_banished_camera_feedback();
            }
        }
    }

    #[must_use]
    pub const fn player_mana(&self) -> i32 {
        self.player_mana
    }

    /// Keep the shared spell projection's local player entity in lockstep
    /// with the Level 2 source damage mailbox.  The raw entity record is
    /// updated because reset/campaign hashes bind the lossless source bytes.
    pub(super) fn set_player_entity_life(&mut self, life: i32) {
        self.player_entity.life = life;
        self.player_entity.record[12..16].copy_from_slice(&life.to_le_bytes());
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.life = life;
                entity.record[12..16].copy_from_slice(&life.to_le_bytes());
            }
        }
    }

    /// Execute the source PlayerEvents phase update once for the complete
    /// outer frame.  The source increments Turn and rewrites its colorIndex
    /// array before any speed-index entity pass; the bounded projection keeps
    /// the lossless turn carrier in both local/player views and returns the
    /// two HUD phases consumed by this slice.
    pub(super) fn step_day_hud_player_events(&mut self) -> (u8, u8) {
        let local_player_slot = self.local_player.slot;
        let turn = self.local_player.turn.wrapping_add(1);
        self.local_player.turn = turn;
        self.local_player.record[8..12].copy_from_slice(&turn.to_le_bytes());
        if let Some(player) = self
            .players
            .iter_mut()
            .find(|player| player.slot == local_player_slot)
        {
            player.turn = turn;
            player.record[8..12].copy_from_slice(&turn.to_le_bytes());
        }
        level2_day_color_phases(turn)
    }

    /// Carry the source PlayerHitFrameTime_406 write through every typed
    /// player view.  The value is decremented only by the post-update HUD
    /// phase, never by a snapshot read.
    pub(super) fn set_player_hit_frame_time(&mut self, value: u8) {
        let local_player_slot = self.local_player.slot;
        self.local_player.hit_frame_time = value;
        if let Some(player) = self
            .players
            .iter_mut()
            .find(|player| player.slot == local_player_slot)
        {
            player.hit_frame_time = value;
        }
    }

    /// Keep the shared spell projection's source damage mailbox in lockstep
    /// with the Level 2 player route.  The exported entity record carries a
    /// four-byte header, so the source payload lives at bytes 98..102 and its
    /// owner at bytes 102..104; bytes 104..108 remain untouched.
    pub(super) fn set_player_entity_damage_mailbox(&mut self, damage: i32, attacker: u16) {
        self.player_entity.record[98..102].copy_from_slice(&damage.to_le_bytes());
        self.player_entity.record[102..104].copy_from_slice(&attacker.to_le_bytes());
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.record[98..102].copy_from_slice(&damage.to_le_bytes());
                entity.record[102..104].copy_from_slice(&attacker.to_le_bytes());
            }
        }
    }

    /// Mirror `sub_5EFA0`'s attacker/killer words and the following player
    /// death action into the shared source projection.
    pub(super) fn set_player_entity_damage_result(
        &mut self,
        killer: u16,
        attacker: u16,
        dead: bool,
    ) {
        self.player_entity.set_damage_result_words(killer, attacker);
        if dead {
            self.player_entity.set_player_death_state();
        }
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.set_damage_result_words(killer, attacker);
                if dead {
                    entity.set_player_death_state();
                }
            }
        }
    }

    /// Execute the qualified local-player portion of remc2's
    /// `AddPlayer03_00_5E010` upkeep. The source adds the previous
    /// `manaRegen` value, clamps mana to the signed `[0,maxMana]` interval,
    /// then selects the Castle/boost or outdoor recharge rate from the
    /// shift-AABB result. The relation alone never selects the Castle rate.
    pub(super) fn step_local_player_unboosted_upkeep(
        &mut self,
        castle_entity_index: u16,
        castle_overlap: bool,
        projected_max_mana: Option<i32>,
    ) -> bool {
        const RECHARGE_BOOST_FLAG: u32 = 0x0000_1000;
        let player_entity_index = self.player_entity.index;
        if self.player_entity.life < 0 {
            return false;
        }

        if let Some(max_mana) = projected_max_mana {
            self.player_entity.max_mana = max_mana;
            self.player_entity.record[144..148].copy_from_slice(&max_mana.to_le_bytes());
            if let Some(entity) = self
                .player_entities
                .iter_mut()
                .find(|entity| entity.index == player_entity_index)
            {
                entity.max_mana = max_mana;
                entity.record[144..148].copy_from_slice(&max_mana.to_le_bytes());
            }
        }

        // The source entity is authoritative for this callback; the caster
        // projection is only the activation gate and is synchronized after
        // the upkeep write.
        let mut life = self.player_entity.life;
        // sub_5EFA0 sets dword_0x18D to 16 before this same source visit's
        // upkeep. Each nonzero visit decrements instead of healing; the
        // visit after the countdown reaches zero applies the old life rate.
        if self.player_life_regen_cooldown != 0 {
            self.player_life_regen_cooldown = self.player_life_regen_cooldown.wrapping_sub(1);
        } else {
            life = life.wrapping_add(self.player_life_regen);
        }
        if self.player_hit_effect_cooldown != 0 {
            self.player_hit_effect_cooldown = self.player_hit_effect_cooldown.wrapping_sub(1);
        }
        life = life.clamp(-1, self.player_entity.max_life);
        let mut mana = self
            .player_entity
            .mana
            .wrapping_add(self.player_entity.mana_regen);
        if mana < 0 {
            mana = 0;
        }
        if mana > self.player_entity.max_mana {
            mana = self.player_entity.max_mana;
        }
        let castle_overlap_active = castle_entity_index != 0 && castle_overlap;
        let recharge_boost_active = self.player_entity.control_flags & RECHARGE_BOOST_FLAG != 0;
        let castle_or_boost = castle_overlap_active || recharge_boost_active;
        let next_mana_regen = if castle_or_boost {
            (self.player_entity.max_mana / 200).max(1_000)
        } else {
            (self.player_entity.max_mana / 2_000).max(100)
        };
        let next_life_regen = if castle_or_boost {
            self.player_entity.max_life / 250
        } else {
            self.player_entity.max_life / 2_000
        };

        self.player_entity.life = life;
        self.player_entity.record[12..16].copy_from_slice(&life.to_le_bytes());
        self.player_entity.mana = mana;
        self.player_entity.mana_regen = next_mana_regen;
        self.player_life_regen = next_life_regen;
        self.player_entity.record[140..144].copy_from_slice(&next_mana_regen.to_le_bytes());
        self.player_entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
        if let Some(entity) = self
            .player_entities
            .iter_mut()
            .find(|entity| entity.index == player_entity_index)
        {
            entity.life = life;
            entity.record[12..16].copy_from_slice(&life.to_le_bytes());
            entity.mana = mana;
            entity.mana_regen = next_mana_regen;
            entity.record[140..144].copy_from_slice(&next_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
        }
        if recharge_boost_active {
            self.player_entity.control_flags &= !RECHARGE_BOOST_FLAG;
            self.player_entity.record[16..20]
                .copy_from_slice(&self.player_entity.control_flags.to_le_bytes());
            self.caster.control_flags = self.player_entity.control_flags;
            for entity in &mut self.player_entities {
                if entity.index == player_entity_index {
                    entity.control_flags = self.player_entity.control_flags;
                    entity.record[16..20].copy_from_slice(&entity.control_flags.to_le_bytes());
                }
            }
        }
        // The ordinary local-player source dispatch already owns word159's
        // Castle protection, full-mailbox clear and same-visit countdown.
        // Life/mana upkeep must not decrement that authoritative counter twice.
        // Preserve the existing publication of the other linked-player fields.
        for player in &self.players {
            if player.entity_index == player_entity_index && player.slot == self.local_player.slot {
                self.local_player = player.clone();
            }
        }
        self.caster.mana = mana;
        self.player_mana = mana;
        true
    }

    /// Execute remc2's shared `sub_5F660` activation primitive for the
    /// qualified Level 2 Fireball holder (model 0) or Possession holder
    /// (model 1). Projectile factories remain owned by their existing Level 1
    /// mission implementation; this seam proves the exact mana/gate,
    /// direction, timer, pointer, and audio-event transition on the Level 2
    /// reset data.
    pub fn activate_fireball(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let event_start = self.events.len();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.fireball),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.capture_activation_effects(event_start);
        self.player_mana = self.caster.mana;
        activated
    }

    pub fn activate_possession(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let event_start = self.events.len();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.possession),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.capture_activation_effects(event_start);
        self.player_mana = self.caster.mana;
        activated
    }

    pub fn activate_speed(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let event_start = self.events.len();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.speed),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.capture_activation_effects(event_start);
        self.player_mana = self.caster.mana;
        activated
    }

    #[must_use]
    pub fn fireball_holder(&self) -> SpellActivationHolder {
        self.fireball
    }

    #[must_use]
    pub fn possession_holder(&self) -> SpellActivationHolder {
        self.possession
    }

    #[must_use]
    pub fn speed_holder(&self) -> SpellActivationHolder {
        self.speed
    }

    #[must_use]
    pub fn activation_events(&self) -> &[FlightEvent] {
        &self.events
    }

    #[must_use]
    pub const fn speed_pickup_state(&self) -> Level2SpeedPickupState {
        self.speed_pickup_state
    }

    #[must_use]
    pub fn pickup_events(&self) -> &[Level2SharedSpellEvent] {
        &self.pickup_events
    }

    pub(super) fn drain_pending_effects(
        &mut self,
    ) -> Vec<crate::campaign::journal::CampaignEffect> {
        std::mem::take(&mut self.pending_effects)
    }

    pub(super) fn emit_flight_effect(&mut self, event: FlightEvent) {
        self.pending_effects
            .push(crate::campaign::journal::CampaignEffect::Flight(event));
        Vec::push(&mut self.events, event);
    }

    fn emit_shared_spell_effect(&mut self, event: Level2SharedSpellEvent) {
        self.pending_effects
            .push(crate::campaign::journal::CampaignEffect::SharedSpell(event));
        Vec::push(&mut self.pickup_events, event);
    }

    fn capture_activation_effects(&mut self, event_start: usize) {
        self.pending_effects.extend(
            self.events[event_start..]
                .iter()
                .copied()
                .map(crate::campaign::journal::CampaignEffect::Flight),
        );
    }

    #[must_use]
    pub fn spell_experience(&self) -> &[i32; PLAYER_SPELL_COUNT] {
        &self.spell_experience
    }

    #[must_use]
    pub fn spell_levels(&self) -> &[i8; PLAYER_SPELL_COUNT] {
        &self.spell_levels
    }

    /// Apply the source `sub_6D8B0(parent, 12, 1)` family-12 award and
    /// `sub_6D9C0` projection to the exact owning player row. The persistent
    /// and runtime words are summed with source i32 wrapping; a newly
    /// computed unlocked level can clamp a selected tier downward, but never
    /// raises it automatically. The convenience arrays are local-player
    /// projections only.
    fn award_beyond_sight_experience(&mut self, parent_entity_index: u16, amount: i32) {
        let family = usize::from(BEYOND_SIGHT_FAMILY);
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == parent_entity_index)
            .expect("Beyond Sight parent remains in the source player rows");
        let mut owner = self.players[owner_index].clone();
        let previous_unlocked = owner.spell_unlocked_levels[family];
        let runtime = owner.runtime_spell_experience[family].wrapping_add(amount);
        let total = owner.persistent_spell_experience[family].wrapping_add(runtime);
        let unlocked = beyond_sight_unlocked_tier(total);
        owner.runtime_spell_experience[family] = runtime;
        owner.spell_unlocked_levels[family] = unlocked;
        owner.record[164 + family * std::mem::size_of::<i32>()
            ..164 + (family + 1) * std::mem::size_of::<i32>()]
            .copy_from_slice(&runtime.to_le_bytes());
        owner.record[502 + family] = unlocked.cast_unsigned();
        if owner.family_selected_tiers[family] > unlocked {
            owner.family_selected_tiers[family] = unlocked;
            owner.record[528 + family] = unlocked.cast_unsigned();
        }
        let source_family_enabled =
            owner.spell_enabled_flags[family] != 0 || owner.enabled_spell_holders[family] != 0;
        let owner_is_local = owner.entity_index == self.local_player.entity_index;
        self.players[owner_index] = owner.clone();
        if owner_is_local {
            self.spell_experience[family] = runtime;
            self.spell_levels[family] = unlocked;
            self.local_player = owner;
        }
        if unlocked != previous_unlocked && source_family_enabled {
            mark_source_audio_call(SourceAudioCallTag::AbilityImprovedTier);
            self.emit_shared_spell_effect(Level2SharedSpellEvent::AbilityImproved {
                source_family: BEYOND_SIGHT_FAMILY,
                tier: unlocked,
            });
            self.emit_flight_effect(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::AbilityImproved,
                entity_index: 0,
                player_index: -1,
                sound_id: 61,
            });
        }
    }

    /// Apply `sub_6D8B0(parent, 3, 1)` for the active Speed holder.
    pub(super) fn award_speed_experience(&mut self, parent_entity_index: u16, amount: i32) {
        self.award_spell_experience(parent_entity_index, 3, amount);
    }

    /// Apply the shared `sub_6D8B0` spell-experience transition for a
    /// package-backed physical spell family.
    pub(crate) fn award_spell_experience(
        &mut self,
        parent_entity_index: u16,
        family: u8,
        amount: i32,
    ) {
        self.update_spell_experience(parent_entity_index, family, amount, true);
    }

    fn update_spell_experience(
        &mut self,
        parent_entity_index: u16,
        family: u8,
        amount: i32,
        announce_improvement: bool,
    ) {
        let family = usize::from(family);
        assert!(family < PLAYER_SPELL_COUNT, "spell family is in range");
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == parent_entity_index)
            .expect("spell holder parent remains in the source player rows");
        let mut owner = self.players[owner_index].clone();
        let previous_unlocked = owner.spell_unlocked_levels[family];
        let runtime = owner.runtime_spell_experience[family].wrapping_add(amount);
        let total = owner.persistent_spell_experience[family].wrapping_add(runtime);
        let unlocked = LEVEL2_SPELL_TIER_THRESHOLDS[family]
            .iter()
            .rposition(|threshold| total >= *threshold)
            .unwrap_or(0) as i8;
        owner.runtime_spell_experience[family] = runtime;
        owner.spell_unlocked_levels[family] = unlocked;
        owner.record[164 + family * std::mem::size_of::<i32>()
            ..164 + (family + 1) * std::mem::size_of::<i32>()]
            .copy_from_slice(&runtime.to_le_bytes());
        owner.record[502 + family] = unlocked.cast_unsigned();
        if owner.family_selected_tiers[family] > unlocked {
            owner.family_selected_tiers[family] = unlocked;
            owner.record[528 + family] = unlocked.cast_unsigned();
        }
        let family_enabled =
            owner.spell_enabled_flags[family] != 0 || owner.enabled_spell_holders[family] != 0;
        let owner_is_local = owner.entity_index == self.local_player.entity_index;
        self.players[owner_index] = owner.clone();
        if owner_is_local {
            self.spell_experience[family] = runtime;
            self.spell_levels[family] = unlocked;
            self.local_player = owner;
        }
        if announce_improvement && unlocked != previous_unlocked && family_enabled {
            mark_source_audio_call(SourceAudioCallTag::AbilityImprovedTier);
            self.emit_shared_spell_effect(Level2SharedSpellEvent::AbilityImproved {
                source_family: family as u8,
                tier: unlocked,
            });
            self.emit_flight_effect(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::AbilityImproved,
                entity_index: 0,
                player_index: -1,
                sound_id: 61,
            });
        }
    }

    /// Run the source `UpdateScroll_59C80` callback for a live model-5 row.
    /// The source scans every live model-0 player in linked-list order, but
    /// only the local LevelIndex player receives XP and tier updates. A
    /// remote collision still consumes the scroll, so the scan deliberately
    /// does not return early after the first overlap.
    pub fn collect_level2_experience_scroll(
        &mut self,
        source_slot: u16,
        experience_award: i32,
        scroll_position: Position,
        local_player_slot: u8,
        local_player_entity_index: u16,
        local_player_position: Position,
        local_player_life: i32,
        entities: &[EntitySnapshot],
    ) -> bool {
        let mut first_collision_entity = None;
        let mut local_collision = false;
        let mut local_player_index = None;
        for (index, player) in self.players.iter().enumerate() {
            let is_local = player.player_color_index() == local_player_slot
                && player.entity_index == local_player_entity_index;
            if is_local {
                local_player_index = Some(index);
            }
            let (active, class, model, life, position) = if is_local {
                (true, 3, 0, local_player_life, local_player_position)
            } else if let Some(entity) = entities
                .iter()
                .find(|entity| entity.index == player.entity_index)
            {
                (
                    entity.active,
                    entity.class,
                    entity.model,
                    entity.life,
                    entity.position,
                )
            } else {
                continue;
            };
            if !active
                || class != 3
                || model != 0
                || life < 0
                || !level2_experience_scroll_overlap(scroll_position, position)
            {
                continue;
            }
            first_collision_entity.get_or_insert(player.entity_index);
            local_collision |= is_local;
        }
        let Some(first_collision_entity) = first_collision_entity else {
            return false;
        };
        self.emit_shared_spell_effect(Level2SharedSpellEvent::ExperienceScrollCollected {
            source_slot,
            player_entity_index: first_collision_entity,
        });
        if local_collision {
            let owner_index =
                local_player_index.expect("local player is present for a local collision");
            let enabled_holders = self.players[owner_index].enabled_spell_holders;
            let mut improved = Vec::new();
            let mut owner = self.players[owner_index].clone();
            for family in 0..PLAYER_SPELL_COUNT {
                if enabled_holders[family] == 0 {
                    continue;
                }
                let mut experience =
                    owner.runtime_spell_experience[family].wrapping_add(experience_award);
                if family == usize::from(CASTLE_SPELL_FAMILY) {
                    experience = experience.min(7);
                }
                owner.runtime_spell_experience[family] = experience;
                owner.record[164 + family * std::mem::size_of::<i32>()
                    ..164 + (family + 1) * std::mem::size_of::<i32>()]
                    .copy_from_slice(&experience.to_le_bytes());
                self.spell_experience[family] = experience;
                let previous_level = owner.spell_unlocked_levels[family];
                let total = owner.persistent_spell_experience[family].wrapping_add(experience);
                let level = LEVEL2_SPELL_TIER_THRESHOLDS[family]
                    .iter()
                    .rposition(|threshold| total >= *threshold)
                    .unwrap_or(0) as i8;
                owner.spell_unlocked_levels[family] = level;
                owner.record[502 + family] = level.cast_unsigned();
                self.spell_levels[family] = level;
                if owner.family_selected_tiers[family] > level {
                    owner.family_selected_tiers[family] = level;
                    owner.record[528 + family] = level.cast_unsigned();
                }
                if level != previous_level {
                    improved.push((family as u8, level));
                }
            }
            self.players[owner_index] = owner.clone();
            mark_source_audio_call(SourceAudioCallTag::ExperienceScrollPickup);
            self.emit_flight_effect(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::ExperienceScroll,
                entity_index: local_player_entity_index,
                player_index: i16::from(local_player_slot),
                sound_id: 63,
            });
            for (family, tier) in improved {
                mark_source_audio_call(SourceAudioCallTag::AbilityImprovedTier);
                self.emit_shared_spell_effect(Level2SharedSpellEvent::AbilityImproved {
                    source_family: family,
                    tier,
                });
                self.emit_flight_effect(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::AbilityImproved,
                    entity_index: local_player_entity_index,
                    player_index: i16::from(local_player_slot),
                    sound_id: 61,
                });
            }
            self.local_player = owner;
        }
        true
    }

    #[must_use]
    pub fn acquisition_states(&self) -> &[SpellHolderAcquisitionState] {
        &self.acquisitions
    }

    #[must_use]
    pub fn persistent_acquisition_states(&self) -> &BTreeMap<u16, SpellHolderAcquisitionState> {
        &self.persistent_acquisitions
    }

    /// Reach the pinned generic `sub_68FF0` acquisition helper from the
    /// production Level 2 reset projection.  The caller owns collision
    /// detection (the source `sub_106C0` query); this method applies the
    /// source relation, hand, timer, action, player-record, and presentation
    /// writes once that predicate is true.
    pub fn acquire_level2_spell_holder(
        &mut self,
        family: u8,
        role: PhysicalSpellActionRole,
        overlaps: bool,
    ) -> Result<SpellHolderAcquisitionOutcome> {
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == self.player_entity.index)
            .unwrap_or(0);
        self.acquire_level2_spell_holder_for_player(family, role, overlaps, owner_index)
    }

    pub(super) fn acquire_level2_spell_holder_for_player(
        &mut self,
        family: u8,
        role: PhysicalSpellActionRole,
        overlaps: bool,
        owner_index: usize,
    ) -> Result<SpellHolderAcquisitionOutcome> {
        let index = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unsupported Level 2 spell family {family}"))
            })?;
        let owner = self.players[owner_index].clone();
        let family_index = usize::from(family);
        let left_spell_slot = owner.left_spell_slot_raw;
        let right_spell_slot = owner.right_spell_slot_raw;
        let left_subspell_index = owner.left_subspell_index;
        let right_subspell_index = owner.right_subspell_index;
        self.acquisitions[index].owner_entity_index = owner.entity_index;
        self.acquisitions[index].holder_relation = owner.enabled_spell_holders[usize::from(family)];
        // SetSpell reads the owner's current array_0x437 entry on every
        // acquisition; do not keep the reset-time tier when gameplay changes
        // the selected tier between source pulses.
        self.acquisitions[index].selected_tier = owner.family_selected_tiers[family_index];
        self.acquisitions[index].left_spell_slot = left_spell_slot;
        self.acquisitions[index].right_spell_slot = right_spell_slot;
        self.acquisitions[index].left_subspell_index = left_subspell_index;
        self.acquisitions[index].right_subspell_index = right_subspell_index;
        if let Some(pickup) = self.pickup(self.acquisitions[index].source_slot) {
            self.acquisitions[index].return_side = pickup.source_word_4a();
        }
        // `SetSpell_6D5E0` has a distinct active-holder branch: it only
        // rewrites word_0x2C and must leave the cold payload/hash projection
        // untouched.  Preserve those fields across the generic owner/hand
        // transition so the typed state follows the same branch.
        let active_setspell_projection = self
            .pickup(self.acquisitions[index].source_slot)
            .and_then(|pickup| {
                (u16::from_le_bytes(
                    pickup.record[50..52]
                        .try_into()
                        .expect("entity word 0x2e record extent"),
                ) != 0)
                    .then_some(self.acquisitions[index])
            });
        let return_side = self.acquisitions[index].return_side;
        let result = acquire_level2_spell_holder_with_return_side(
            &mut self.acquisitions[index],
            role,
            overlaps,
            return_side,
        );
        let outcome = match result {
            SpellHolderAcquisitionResult::Acquired(outcome) => outcome,
            SpellHolderAcquisitionResult::Rejected(failure) => {
                return Err(NativeWorldError::new(format!(
                    "Level 2 spell family {family} acquisition rejected: {failure:?}"
                )));
            }
        };
        if let Some(previous) = active_setspell_projection {
            let state = &mut self.acquisitions[index];
            state.subspell_index = previous.subspell_index;
            state.word_0x30 = previous.word_0x30;
            state.byte_0x3b = previous.byte_0x3b;
            state.byte_0x3c = previous.byte_0x3c;
            state.font_type_index = previous.font_type_index;
            state.mana_regen = previous.mana_regen;
            state.max_mana = previous.max_mana;
            state.mana = previous.mana;
        } else if self.free_spell_usage {
            // Cold `SetSpell_6D5E0` applies the global free-spell option only
            // after calculating the ordinary profile. `max_mana` remains the
            // source mana-cost result; only these two fields are overridden.
            let state = &mut self.acquisitions[index];
            state.mana_regen = 0;
            state.mana = 1;
        }
        let state = self.acquisitions[index];
        let owner_entity_index = self.players[owner_index].entity_index;
        {
            let owner = &mut self.players[owner_index];
            owner.enabled_spell_holders[family_index] = outcome.source_slot;
            owner.acquired_spell_flags[family_index] = state.learned;
            owner.record[268 + family_index * 2..270 + family_index * 2]
                .copy_from_slice(&outcome.source_slot.to_le_bytes());
            owner.record[476 + family_index] = state.learned;
            match outcome.hand {
                crate::spell_activation::SpellHand::Left => {
                    owner.left_spell_slot_raw = state.left_spell_slot;
                    owner.left_spell_slot = family;
                    owner.left_subspell_index = state.left_subspell_index;
                    owner.record[554..556].copy_from_slice(&state.left_spell_slot.to_le_bytes());
                    owner.record[558] = state.left_subspell_index.to_le_bytes()[0];
                }
                crate::spell_activation::SpellHand::Right => {
                    owner.right_spell_slot_raw = state.right_spell_slot;
                    owner.right_spell_slot = family;
                    owner.right_subspell_index = state.right_subspell_index;
                    owner.record[556..558].copy_from_slice(&state.right_spell_slot.to_le_bytes());
                    owner.record[559] = state.right_subspell_index.to_le_bytes()[0];
                }
            }
            if owner_index == 0 {
                self.local_player = owner.clone();
            }
        }
        if let Some(pickup) = self.pickup_mut(outcome.source_slot) {
            pickup.control_flags |= 1;
            pickup.record[16] |= 1;
            pickup.record[19] &= !2;
            pickup.control_flags = u32::from_le_bytes(
                pickup.record[16..20]
                    .try_into()
                    .expect("entity control flag record extent"),
            );
            pickup.record[44..46].copy_from_slice(&state.parent_entity_index.to_le_bytes());
            pickup.action = outcome.holder_action;
            pickup.record[73] = outcome.holder_action;
            pickup.record[58..60].copy_from_slice(&state.presentation_timer.to_le_bytes());
            // SetSpell_6D5E0's source-visible writes. An already-active
            // holder updates only word_0x2C; a cold holder receives the full
            // subspell/font/mana payload.
            let active_word_2e = u16::from_le_bytes(
                pickup.record[50..52]
                    .try_into()
                    .expect("entity word 0x2e record extent"),
            );
            if active_word_2e != 0 {
                let loc_spell_id = u16::from(
                    level2_applied_tier(state.selected_tier)
                        .expect("successful SetSpell keeps a non-negative tier"),
                ) + 1;
                pickup.record[48..50].copy_from_slice(&loc_spell_id.to_le_bytes());
            } else {
                pickup.subspell_index = state.subspell_index;
                pickup.record[46..48].copy_from_slice(&state.subspell_index.to_le_bytes());
                pickup.record[52..54].copy_from_slice(&state.word_0x30.to_le_bytes());
                pickup.activation_value = state.word_0x30;
                pickup.byte_3b = state.byte_0x3b;
                pickup.record[63] = state.byte_0x3b;
                pickup.byte_3c = 0;
                pickup.record[64] = 0;
                pickup.action_marker = 0;
                pickup.font_type_index = 0;
                pickup.record[65] = 0;
                pickup.mana_regen = state.mana_regen;
                pickup.record[140..144].copy_from_slice(&state.mana_regen.to_le_bytes());
                pickup.max_mana = state.max_mana;
                pickup.mana_cost = state.max_mana;
                pickup.record[144..148].copy_from_slice(&state.max_mana.to_le_bytes());
                pickup.mana = state.mana;
                pickup.record[148..152].copy_from_slice(&state.mana.to_le_bytes());
                let applied_tier = level2_applied_tier(state.selected_tier)
                    .expect("successful SetSpell keeps a non-negative tier");
                pickup.record[74] = applied_tier;
                pickup.level_gate = applied_tier.cast_signed();
            }
            // Successful `sub_68FF0` sets the pickup ownership bit; it does
            // not call DisableEntityDrawing. The latter is only applied by
            // the positive-life prefix when life decrements from one to zero.
            pickup.record[78..80].copy_from_slice(&0_i16.to_le_bytes());
        }
        // The active SetSpell branch only queues the newly selected tier in
        // word_0x2E; the running holder keeps its current word_0x30/mana
        // payload until cleanup. Read that post-branch projection before
        // seeding a typed active lifecycle so it cannot jump tiers early.
        let active_holder_projection = self.pickup(outcome.source_slot).cloned();
        if family == 6 {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut shield = ShieldState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Shield SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Shield tier");
            shield.parent_mana_regen = parent.mana_regen;
            shield.parent_status_flags = parent.control_flags;
            self.shield = Some(shield);
            self.sync_shield_projection();
        } else if family == 11 {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut invisible = InvisibleState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Invisible SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Invisible tier");
            invisible.parent_mana_regen = parent.mana_regen;
            invisible.parent_status_flags = parent.control_flags;
            self.invisible = Some(invisible);
            self.sync_invisible_projection();
        } else if family == BEYOND_SIGHT_FAMILY {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut beyond_sight = BeyondSightState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Beyond Sight SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Beyond Sight tier");
            beyond_sight.parent_mana_regen = parent.mana_regen;
            if let Some(holder) = active_holder_projection.as_ref() {
                let active_word_2e = u16::from_le_bytes(
                    holder.record[50..52]
                        .try_into()
                        .expect("Beyond Sight active word extent"),
                );
                if active_word_2e != 0 {
                    beyond_sight.active_value = active_word_2e.cast_signed();
                    beyond_sight.activation_value = u16::from_le_bytes(
                        holder.record[52..54]
                            .try_into()
                            .expect("Beyond Sight activation word extent"),
                    )
                    .cast_signed();
                    beyond_sight.mana_cost = holder.max_mana;
                    beyond_sight.mana_regen_requirement = holder.mana_regen;
                    for tier in 0..=2 {
                        let Some(profile) = level2_set_spell_profile(BEYOND_SIGHT_FAMILY, tier)
                        else {
                            continue;
                        };
                        if beyond_sight.activation_value == profile.word_0x30.cast_signed()
                            && holder.max_mana == profile.max_mana
                            && holder.mana_regen == profile.mana_regen
                        {
                            beyond_sight.tier = tier;
                            break;
                        }
                    }
                }
            }
            self.beyond_sight = Some(beyond_sight);
            self.sync_beyond_sight_projection();
        }
        // `sub_68FF0` prepares sound 18 before writing the holder/player
        // transition. The state mutation is transactional here, but the
        // externally observable journal must retain that source chronology.
        mark_source_audio_call(SourceAudioCallTag::SpellPickup);
        self.emit_flight_effect(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::SpellPickup,
            entity_index: owner_entity_index,
            player_index: -1,
            sound_id: 18,
        });
        self.emit_shared_spell_effect(Level2SharedSpellEvent::SpellAcquired {
            source_slot: outcome.source_slot,
            player_entity_index: owner_entity_index,
            family,
            holder_action: outcome.holder_action,
        });
        Ok(outcome)
    }

    /// Source-pass adapter for the authored Level 2 pickup rows. This follows
    /// the `sub_68FF0` prefix before calling the owner transition: life is
    /// decremented first, the pickup is terrain-relaxed, and the source map
    /// scan considers only live model-0 players with an empty SpellEnabled
    /// relation. Rows 155/156 are admitted only after their authored event
    /// constructor materialises a live map entity.
    pub fn step_level2_spell_acquisition(
        &mut self,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            terrain,
            None,
            &source_player_order,
        )
    }

    fn step_level2_spell_acquisition_with_castle_mana(
        &mut self,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
        parent_castle_mana: Option<i32>,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        if player_entity_index != self.player_entity.index {
            return Err(NativeWorldError::new(format!(
                "Level 2 spell acquisition received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            )));
        }
        self.player_entity.position = player_position;
        self.player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
        self.player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
        self.player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());
        if let Some(player_entity) = self
            .player_entities
            .iter_mut()
            .find(|entity| entity.index == player_entity_index)
        {
            player_entity.position = player_position;
            player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
            player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
            player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());
        }
        let Some(index) = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
        else {
            return Err(NativeWorldError::new(format!(
                "unsupported Level 2 spell family {family}"
            )));
        };
        let source_slot = self.acquisitions[index].source_slot;
        let source_action = self
            .pickup(source_slot)
            .map(|pickup| pickup.action)
            .unwrap_or(self.acquisitions[index].source_action);
        let role = if physical_spell_action(family, PhysicalSpellActionRole::PersistentPickup)
            == Some(source_action)
        {
            PhysicalSpellActionRole::PersistentPickup
        } else if physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup)
            == Some(source_action)
        {
            PhysicalSpellActionRole::OrdinaryPickup
        } else if physical_spell_action(family, PhysicalSpellActionRole::Active)
            == Some(source_action)
        {
            if family == 6 && source_action == SHIELD_ACTIVE_ACTION {
                self.step_shield_source_slot(source_slot);
            } else if family == 11 && source_action == INVISIBLE_ACTIVE_ACTION {
                self.step_invisible_source_slot(source_slot, parent_castle_mana);
            } else if family == BEYOND_SIGHT_FAMILY && source_action == BEYOND_SIGHT_ACTIVE_ACTION {
                self.step_beyond_sight_source_slot(source_slot, parent_castle_mana);
            }
            // The source cursor still reaches the live holder row, but its
            // pickup transition has already completed. The active family
            // lifecycles advance above; other active families remain
            // deliberately bounded to their acquisition seam.
            return Ok(None);
        } else {
            return Err(NativeWorldError::new(format!(
                "Level 2 spell family {family} has unsupported source action {source_action}"
            )));
        };
        if !self.acquisitions[index].step_source_life() {
            // `sub_68FF0` disables drawing exactly when the positive source
            // life reaches zero. Keep the raw entity byte17 bit2/dword 0x400
            // in lock-step with the typed cadence state.
            let source_life = self.acquisitions[index].source_life;
            if self.acquisitions[index].source_life == 0 {
                let source_slot = self.acquisitions[index].source_slot;
                if let Some(pickup) = self.pickup_mut(source_slot) {
                    pickup.life = source_life;
                    pickup.record[12..16].copy_from_slice(&source_life.to_le_bytes());
                    pickup.disable_drawing();
                }
                self.acquisitions[index].source_draw_enabled = false;
            } else {
                let source_slot = self.acquisitions[index].source_slot;
                if let Some(pickup) = self.pickup_mut(source_slot) {
                    pickup.life = source_life;
                    pickup.record[12..16].copy_from_slice(&source_life.to_le_bytes());
                }
            }
            return Ok(None);
        }
        if self.pickup(source_slot).is_none() {
            return Ok(None);
        }
        let mut pickup = self
            .pickup(source_slot)
            .cloned()
            .expect("validated Level 2 spell source row remains live");
        let ground = terrain.ground_height(pickup.position);
        if pickup.position.z > ground {
            pickup.position.z = pickup.position.z.wrapping_sub(128);
        }
        if pickup.position.z <= ground {
            pickup.position.z = ground;
        }
        pickup.record[84..86].copy_from_slice(&pickup.position.z.to_le_bytes());
        // Source `byte_0x3E_62` is the action cadence byte at record 66,
        // distinct from the byte0 control flags at record 16.
        if pickup.phase & 3 != 0 {
            if let Some(live) = self.pickup_mut(source_slot) {
                live.clone_from(&pickup);
            }
            return Ok(None);
        }
        // First source-list pass: the active level/player-colour owner marks
        // the pickup's byte0 flag before the collision owner pass. Level 2's
        // local source colour is zero (the same `LevelIndex_0xc` selector
        // used by the pinned routine); this branch is intentionally kept
        // separate from the SpellEnabled collision gate below.
        if pickup.control_flags & 1 == 0
            && source_player_order.iter().copied().any(|player_index| {
                let Some(player) = self.players.get(player_index) else {
                    return false;
                };
                player.enabled_spell_holders[usize::from(family)] != 0
                    && player.player_color_index() == 0
                    && self
                        .player_entities
                        .get(player_index)
                        .is_some_and(|entity| {
                            entity.active && entity.model == 0 && entity.life >= 0
                        })
            })
        {
            pickup.control_flags |= 1;
            pickup.record[16] |= 1;
        }
        let mut owner_index = None;
        for &player_index in source_player_order {
            let Some(player) = self.players.get(player_index) else {
                continue;
            };
            if player.enabled_spell_holders[usize::from(family)] != 0 {
                continue;
            }
            let Some(entity) = self.player_entities.get(player_index) else {
                continue;
            };
            // The source linked-list filter is model==0 and life>=0. The
            // collision query itself is exactly `sub_106C0`.
            if entity.active
                && entity.model == 0
                && entity.life >= 0
                && level2_source_intersects(&pickup, entity)
            {
                owner_index = Some(player_index);
                break;
            }
        }
        let Some(owner_index) = owner_index else {
            if let Some(live) = self.pickup_mut(source_slot) {
                live.clone_from(&pickup);
            }
            return Ok(None);
        };
        if let Some(live) = self.pickup_mut(source_slot) {
            live.clone_from(&pickup);
        }
        self.acquire_level2_spell_holder_for_player(family, role, true, owner_index)
            .map(Some)
    }

    /// Production source-dispatch wrapper for `sub_69250`: when the live
    /// source carries a persistent action, the successful holder transition
    /// creates the next replacement in the shared economy arena and binds it
    /// back into this same entity projection for the run-again cursor.
    pub fn step_level2_spell_acquisition_with_economy(
        &mut self,
        economy: &mut Level2EconomyState,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_acquisition_with_economy_in_source_order(
            economy,
            family,
            player_entity_index,
            player_position,
            terrain,
            &source_player_order,
        )
    }

    pub(super) fn step_level2_spell_acquisition_with_economy_in_source_order(
        &mut self,
        economy: &mut Level2EconomyState,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let active_parent = self
            .invisible
            .and_then(|invisible| {
                (invisible.holder_entity_index
                    == self
                        .acquisitions
                        .iter()
                        .find(|state| state.family == family)
                        .map_or(0, |state| state.source_slot))
                .then_some(invisible.parent_entity_index)
            })
            .or_else(|| {
                self.beyond_sight.and_then(|beyond_sight| {
                    (beyond_sight.holder_entity_index
                        == self
                            .acquisitions
                            .iter()
                            .find(|state| state.family == family)
                            .map_or(0, |state| state.source_slot))
                    .then_some(beyond_sight.parent_entity_index)
                })
            });
        let parent_castle_mana = economy
            .castle()
            .filter(|castle| {
                castle.active
                    && matches!(family, 11 | BEYOND_SIGHT_FAMILY)
                    && active_parent == Some(castle.owner_entity_index)
            })
            .map(|castle| castle.mana);
        let result = self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            terrain,
            parent_castle_mana,
            source_player_order,
        )?;
        let Some(outcome) = result else {
            return Ok(None);
        };
        self.reprice_acquired_castle_holder(economy, outcome);
        if outcome.replacement_action.is_none() {
            return Ok(Some(outcome));
        }
        let economy_before = economy.clone();
        let position = economy
            .mark_spell_replacement_acquired(outcome.source_slot, outcome.holder_action)
            .or_else(|| {
                let position = self.pickup(outcome.source_slot)?.position;
                // Authored/reset persistent sources are not replacement
                // products yet; their first holder conversion is in place.
                let _ = economy.sync_spell_pickup_projection(
                    outcome.source_slot,
                    outcome.holder_action,
                    position,
                );
                Some(position)
            })
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 persistent source {} is absent from the economy arena",
                    outcome.source_slot
                ))
            })?;
        let Some(replacement) =
            economy.allocate_spell_replacement(family, outcome.parent_entity_index, position)
        else {
            // `sub_69250` keeps the successful holder transition even when
            // NewEvent has no free row; the next replacement is simply
            // absent until a later source creates one.
            return Ok(Some(outcome));
        };
        if let Err(error) = self.materialize_spell_replacement_pickup(replacement) {
            *economy = economy_before;
            return Err(error);
        }
        Ok(Some(outcome))
    }

    /// Dispatch one economy-owned persistent source without aliasing the
    /// authored ordinary source of the same family. The source game can keep
    /// both rows live at once, so the temporary family slot is restored after
    /// this source-local transition and the replacement state is keyed by its
    /// own arena slot.
    pub fn step_level2_spell_replacement_acquisition_with_economy(
        &mut self,
        economy: &mut Level2EconomyState,
        source_slot: u16,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_replacement_acquisition_with_economy_in_source_order(
            economy,
            source_slot,
            family,
            player_entity_index,
            player_position,
            terrain,
            &source_player_order,
        )
    }

    pub(super) fn step_level2_spell_replacement_acquisition_with_economy_in_source_order(
        &mut self,
        economy: &mut Level2EconomyState,
        source_slot: u16,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let family_index = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unsupported Level 2 spell family {family}"))
            })?;
        let pickup = self.pickup(source_slot).cloned().ok_or_else(|| {
            NativeWorldError::new(format!(
                "Level 2 persistent source row {source_slot} is absent"
            ))
        })?;
        let base_state = self.acquisitions[family_index];
        let mut source_state = self
            .persistent_acquisitions
            .remove(&source_slot)
            .unwrap_or_else(|| Self::acquisition_from_source(&pickup, &self.local_player));
        source_state.source_slot = source_slot;
        source_state.authored_source_index = 0;
        source_state.source_action = pickup.action;
        source_state.source_life = pickup.life;
        source_state.source_draw_enabled = pickup.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        self.acquisitions[family_index] = source_state;
        let result = self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            terrain,
            None,
            source_player_order,
        );
        if let Ok(Some(outcome)) = result.as_ref() {
            self.reprice_acquired_castle_holder(economy, *outcome);
        }
        let result_state = self.acquisitions[family_index];
        self.acquisitions[family_index] = base_state;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                self.persistent_acquisitions
                    .insert(source_slot, result_state);
                return Err(error);
            }
        };
        let Some(outcome) = outcome else {
            self.persistent_acquisitions
                .insert(source_slot, result_state);
            return Ok(None);
        };
        if outcome.replacement_action.is_none() {
            return Ok(Some(outcome));
        }

        let economy_before = economy.clone();
        let position = economy
            .mark_spell_replacement_acquired(source_slot, outcome.holder_action)
            .or_else(|| {
                let position = self.pickup(source_slot)?.position;
                let _ = economy.sync_spell_pickup_projection(
                    source_slot,
                    outcome.holder_action,
                    position,
                );
                Some(position)
            })
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 persistent source {source_slot} is absent from the economy arena"
                ))
            })?;
        let Some(replacement) =
            economy.allocate_spell_replacement(family, outcome.parent_entity_index, position)
        else {
            // The holder transition has succeeded; a full source arena simply
            // means there is no next persistent row to materialise.
            return Ok(Some(outcome));
        };
        if let Err(error) = self.materialize_spell_replacement_pickup(replacement) {
            *economy = economy_before;
            self.persistent_acquisitions
                .insert(source_slot, result_state);
            return Err(error);
        }
        let mut next_state = result_state;
        next_state.source_slot = replacement.source_entity_index;
        next_state.authored_source_index = 0;
        next_state.source_action = replacement.action;
        next_state.source_life = 0;
        next_state.source_draw_enabled = true;
        self.persistent_acquisitions
            .insert(replacement.source_entity_index, next_state);
        Ok(Some(outcome))
    }

    /// Dispatch the authored Level 2 Speed pickup at source slot 75.
    ///
    /// This follows the source order of `GetScroll_69DB0`: advance the pickup
    /// cadence byte, reject non-overlap or an already-owned family, then
    /// convert the world pickup into its holder and prepare sound 18. The
    /// model-12 row at slot 38 is intentionally not routed through this
    /// Level 1 family primitive; its later Level 2 family owns that behavior.
    pub fn step_speed_pickup(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Level2SpeedPickupStep> {
        self.step_speed_pickup_inner(player_entity_index, player_position, terrain, true)
    }

    /// Source `UpdateEntities` keeps one presentation/event stream across
    /// ascending entity slots. The outer dispatcher therefore uses this
    /// preserving variant; isolated direct calls retain the public method's
    /// per-call event reset.
    fn step_speed_pickup_in_source_order(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Level2SpeedPickupStep> {
        self.step_speed_pickup_inner(player_entity_index, player_position, terrain, false)
    }

    fn step_speed_pickup_inner(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
        clear_events: bool,
    ) -> Result<Level2SpeedPickupStep> {
        if player_entity_index != self.player_entity.index {
            return Err(NativeWorldError::new(format!(
                "Level 2 Speed pickup received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            )));
        }
        if clear_events {
            self.events.clear();
        }
        let pickup_index = self
            .pickups
            .iter()
            .position(|entity| entity.index == LEVEL2_SHARED_SPELL_PICKUP_SLOTS[1])
            .ok_or_else(|| NativeWorldError::new("Level 2 Speed pickup slot 75 is absent"))?;
        let pickup = &mut self.pickups[pickup_index];
        let previous_action = pickup.action;
        let previous_phase = pickup.phase;
        // `sub_68FF0` begins by relaxing the pickup body against the shared
        // terrain query. The surrounding Level2 session owns map-chain
        // surgery; this adapter deliberately claims only the row-local Z
        // write and records that boundary in the typed state below.
        let ground = terrain.ground_height(pickup.position);
        if pickup.position.z > ground {
            pickup.position.z = pickup.position.z.wrapping_sub(128);
        }
        if pickup.position.z <= ground {
            pickup.position.z = ground;
        }
        pickup.record[84..86].copy_from_slice(&pickup.position.z.to_le_bytes());
        pickup.phase = pickup.phase.wrapping_add(1);
        pickup.record[66] = pickup.phase;
        self.player_entity.position = player_position;
        self.player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
        self.player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
        self.player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());

        let overlaps = level2_source_intersects(pickup, &self.player_entity);
        let already_owned = self.speed.active_value != 0 || pickup.action == 9;
        let in_cadence = previous_phase & 3 == 0;
        if pickup.action != 10 || !overlaps || already_owned || !in_cadence {
            return Ok(Level2SpeedPickupStep {
                source_slot: pickup.index,
                player_entity_index,
                previous_action,
                action: pickup.action,
                previous_phase,
                phase: pickup.phase,
                collected: false,
            });
        }

        // `sub_68FF0` preserves the class-15 source row as the holder. The
        // fixed Speed action/cost are the Level 1 family constants already
        // bound by this Level 2 census.
        pickup.record[16] |= 1;
        pickup.record[19] &= !2;
        pickup.control_flags |= 1;
        pickup.control_flags &= !0x0200_0000;
        pickup.record[44..46].copy_from_slice(&player_entity_index.to_le_bytes());
        pickup.action = 9;
        pickup.record[73] = pickup.action;
        pickup.active_value = 0;
        pickup.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
        pickup.record[58..60].copy_from_slice(&64_i16.to_le_bytes());
        // The player state stores the holder relation and subspell selection
        // separately from the entity row. Level 2's source uses family 3 and
        // the first Speed subspell for this authored pickup.
        let family = usize::from(SPEED_SPELL_FAMILY);
        self.local_player.enabled_spell_holders[family] = pickup.index;
        self.local_player.record[268 + family * 2..270 + family * 2]
            .copy_from_slice(&pickup.index.to_le_bytes());
        self.local_player.acquired_spell_flags[family] = 1;
        self.local_player.record[476 + family] = 1;
        self.local_player.family_selected_tiers[family] = 0;
        self.local_player.record[528 + family] = 0;
        if self.local_player.left_spell_slot_raw < 0 || self.local_player.right_spell_slot_raw != -1
        {
            self.local_player.left_spell_slot_raw = i16::from(SPEED_SPELL_FAMILY);
            self.local_player.left_spell_slot = SPEED_SPELL_FAMILY;
            self.local_player.left_subspell_index = 0;
            self.local_player.record[554..556]
                .copy_from_slice(&i16::from(SPEED_SPELL_FAMILY).to_le_bytes());
            self.local_player.record[558] = 0;
        } else {
            self.local_player.right_spell_slot_raw = i16::from(SPEED_SPELL_FAMILY);
            self.local_player.right_spell_slot = SPEED_SPELL_FAMILY;
            self.local_player.right_subspell_index = 0;
            self.local_player.record[556..558]
                .copy_from_slice(&i16::from(SPEED_SPELL_FAMILY).to_le_bytes());
            self.local_player.record[559] = 0;
        }
        self.speed_pickup_state = Level2SpeedPickupState {
            owner_entity_index: player_entity_index,
            collected: true,
            visible: false,
            state_timer: 64,
        };
        self.speed.entity_index = pickup.index;
        self.speed.model = SPEED_SPELL_FAMILY;
        self.speed.activation_value = SPEED_SPELL_ACTIVATION_TICKS;
        self.speed.mana_cost = SPEED_SPELL_MANA_COST;
        let source_slot = pickup.index;
        let action = pickup.action;
        let phase = pickup.phase;
        self.emit_shared_spell_effect(Level2SharedSpellEvent::SpellCollected {
            source_slot,
            player_entity_index,
            family: SPEED_SPELL_FAMILY,
        });
        mark_source_audio_call(SourceAudioCallTag::SpellPickup);
        self.emit_flight_effect(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::SpellPickup,
            entity_index: player_entity_index,
            player_index: 0,
            sound_id: 18,
        });
        Ok(Level2SpeedPickupStep {
            source_slot,
            player_entity_index,
            previous_action,
            action,
            previous_phase,
            phase,
            collected: true,
        })
    }

    /// Dispatch one of the two reset-census pickup rows in source order.
    ///
    /// Slot 38 is returned as a mechanical exclusion (`None`) because its
    /// model-12 family is not implemented here. Unknown slots fail
    /// closed instead of being treated as a nearby Level 1 family.
    pub fn step_shared_pickup(
        &mut self,
        source_slot: u16,
        player_entity_index: u16,
        player_position: Position,
        terrain: &SessionTerrain,
    ) -> Result<Option<Level2SpeedPickupStep>> {
        match source_slot {
            38 if player_entity_index == self.player_entity.index => Ok(None),
            38 => Err(NativeWorldError::new(format!(
                "Level 2 shared pickup received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            ))),
            75 => self
                .step_speed_pickup_in_source_order(player_entity_index, player_position, terrain)
                .map(Some),
            _ => Err(NativeWorldError::new(format!(
                "unknown Level 2 shared pickup source slot {source_slot}"
            ))),
        }
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        self.reset_hash_without_terrain()
    }

    #[must_use]
    pub(crate) fn reset_hash_with_terrain(&self, terrain: &SessionTerrain) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        for entity in self.pickups.iter().chain(self.holders.iter()) {
            hasher.update(entity.record_bytes());
        }
        for entity in &self.dynamic_pickups {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.player_entity.record_bytes());
        for player in &self.players {
            hasher.update(player.record_bytes());
            player.append_omitted_state_hash(&mut hasher);
        }
        for entity in &self.player_entities {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.local_player.record_bytes());
        self.local_player.append_omitted_state_hash(&mut hasher);
        hasher.update(self.player_entity_sha256.as_bytes());
        hasher.update(terrain.deterministic_hash().to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_life_regen.to_le_bytes());
        hasher.update(self.player_life_regen_cooldown.to_le_bytes());
        if self.player_hit_effect_cooldown != 0 {
            hasher.update(b"source-player-hit-effect-cooldown-v1");
            hasher.update(self.player_hit_effect_cooldown.to_le_bytes());
        }
        self.global_special_state.append_hash(&mut hasher);
        for experience in self.persistent_spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for experience in self.spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for level in self.spell_levels {
            hasher.update(level.to_le_bytes());
        }
        hasher.update(self.speed.entity_index.to_le_bytes());
        hasher.update(self.speed.active_value.to_le_bytes());
        hasher.update(self.speed.activation_value.to_le_bytes());
        hasher.update(self.speed.mana_cost.to_le_bytes());
        if let Some(shield) = self.shield {
            hasher.update([1]);
            hasher.update(shield.holder_entity_index.to_le_bytes());
            hasher.update(shield.parent_entity_index.to_le_bytes());
            hasher.update([shield.tier, shield.action]);
            hasher.update(shield.active_value.to_le_bytes());
            hasher.update(shield.activation_value.to_le_bytes());
            hasher.update(shield.mana_cost.to_le_bytes());
            hasher.update(shield.parent_mana.to_le_bytes());
            hasher.update(shield.parent_life.to_le_bytes());
            hasher.update(shield.parent_mana_regen.to_le_bytes());
            hasher.update(shield.parent_status_flags.to_le_bytes());
            hasher.update(shield.countdown.to_le_bytes());
            hasher.update(shield.experience_awarded.to_le_bytes());
            hasher.update([u8::from(shield.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(mailbox) = self.shield_damage_mailbox {
            hasher.update(b"shield-damage-mailbox-v1");
            hasher.update(mailbox.target_entity_index.to_le_bytes());
            hasher.update(mailbox.damage.to_le_bytes());
            hasher.update(mailbox.attacker_entity_index.to_le_bytes());
        }
        if let Some(invisible) = self.invisible {
            hasher.update([1]);
            hasher.update(invisible.holder_entity_index.to_le_bytes());
            hasher.update(invisible.parent_entity_index.to_le_bytes());
            hasher.update([invisible.tier, invisible.action]);
            hasher.update(invisible.active_value.to_le_bytes());
            hasher.update(invisible.activation_value.to_le_bytes());
            hasher.update(invisible.mana_cost.to_le_bytes());
            hasher.update(invisible.mana_regen_requirement.to_le_bytes());
            hasher.update(invisible.parent_mana.to_le_bytes());
            hasher.update(invisible.parent_life.to_le_bytes());
            hasher.update(invisible.parent_mana_regen.to_le_bytes());
            hasher.update(invisible.parent_status_flags.to_le_bytes());
            hasher.update(invisible.spell_status_flags.to_le_bytes());
            hasher.update(invisible.countdown.to_le_bytes());
            hasher.update(invisible.experience_awarded.to_le_bytes());
            hasher.update([u8::from(invisible.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(beyond_sight) = self.beyond_sight {
            hasher.update([1]);
            hasher.update(beyond_sight.holder_entity_index.to_le_bytes());
            hasher.update(beyond_sight.parent_entity_index.to_le_bytes());
            hasher.update([beyond_sight.tier, beyond_sight.action]);
            hasher.update(beyond_sight.active_value.to_le_bytes());
            hasher.update(beyond_sight.activation_value.to_le_bytes());
            hasher.update(beyond_sight.mana_cost.to_le_bytes());
            hasher.update(beyond_sight.mana_regen_requirement.to_le_bytes());
            hasher.update(beyond_sight.parent_mana.to_le_bytes());
            hasher.update(beyond_sight.parent_life.to_le_bytes());
            hasher.update(beyond_sight.parent_mana_regen.to_le_bytes());
            hasher.update(beyond_sight.countdown.to_le_bytes());
            hasher.update(beyond_sight.experience_awarded.to_le_bytes());
            hasher.update([u8::from(beyond_sight.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        hasher.update(self.beyond_sight_auxiliary.entity_index.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.next_entity.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.entity_index2.to_le_bytes());
        hasher.update([self.beyond_sight_auxiliary.effect_parameter]);
        // Preserve the established false-state hash while still binding the
        // newly reachable true state. No prior valid session could carry the
        // source option, so the tagged true branch is collision-free with the
        // old state space without needlessly invalidating existing baselines.
        if self.free_spell_usage {
            hasher.update(b"level2-free-spell-usage-v1");
            hasher.update([1]);
        }
        for state in &self.acquisitions {
            hasher.update([state.family]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.authored_source_index.to_le_bytes());
            hasher.update(state.owner_entity_index.to_le_bytes());
            hasher.update([state.source_action, state.holder_action]);
            hasher.update(state.replacement_action.unwrap_or(u8::MAX).to_le_bytes());
            hasher.update(state.parent_entity_index.to_le_bytes());
            hasher.update(state.presentation_timer.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility]);
            hasher.update([state.learned]);
            hasher.update(state.left_spell_slot.to_le_bytes());
            hasher.update(state.right_spell_slot.to_le_bytes());
            hasher.update([
                state.left_subspell_index.to_le_bytes()[0],
                state.right_subspell_index.to_le_bytes()[0],
                state.selected_tier.to_le_bytes()[0],
                u8::from(state.source_draw_enabled),
            ]);
            hasher.update(state.subspell_index.to_le_bytes());
            hasher.update(state.word_0x30.to_le_bytes());
            hasher.update([state.byte_0x3b]);
            hasher.update([state.byte_0x3c, state.font_type_index]);
            hasher.update(state.mana_regen.to_le_bytes());
            hasher.update(state.max_mana.to_le_bytes());
            hasher.update(state.mana.to_le_bytes());
            hasher.update(state.source_life.to_le_bytes());
            hasher.update(state.return_side.to_le_bytes());
        }
        for (source_slot, state) in &self.persistent_acquisitions {
            hasher.update([0xfe]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update([state.family, state.source_action, state.holder_action]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility, state.learned]);
            hasher.update(state.source_life.to_le_bytes());
            hasher.update([u8::from(state.source_draw_enabled)]);
        }
        hasher.update(self.speed_pickup_state.owner_entity_index.to_le_bytes());
        hasher.update([u8::from(self.speed_pickup_state.collected)]);
        hasher.update([u8::from(self.speed_pickup_state.visible)]);
        hasher.update([self.speed_pickup_state.state_timer]);
        format!("{:x}", hasher.finalize())
    }

    fn reset_hash_without_terrain(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        for entity in self.pickups.iter().chain(self.holders.iter()) {
            hasher.update(entity.record_bytes());
        }
        for entity in &self.dynamic_pickups {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.player_entity.record_bytes());
        for player in &self.players {
            hasher.update(player.record_bytes());
            player.append_omitted_state_hash(&mut hasher);
        }
        for entity in &self.player_entities {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.local_player.record_bytes());
        self.local_player.append_omitted_state_hash(&mut hasher);
        hasher.update(self.player_entity_sha256.as_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_life_regen.to_le_bytes());
        hasher.update(self.player_life_regen_cooldown.to_le_bytes());
        if self.player_hit_effect_cooldown != 0 {
            hasher.update(b"source-player-hit-effect-cooldown-v1");
            hasher.update(self.player_hit_effect_cooldown.to_le_bytes());
        }
        self.global_special_state.append_hash(&mut hasher);
        for experience in self.persistent_spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for experience in self.spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for level in self.spell_levels {
            hasher.update(level.to_le_bytes());
        }
        hasher.update(self.speed.entity_index.to_le_bytes());
        hasher.update(self.speed.active_value.to_le_bytes());
        hasher.update(self.speed.activation_value.to_le_bytes());
        hasher.update(self.speed.mana_cost.to_le_bytes());
        if let Some(shield) = self.shield {
            hasher.update([1]);
            hasher.update(shield.holder_entity_index.to_le_bytes());
            hasher.update(shield.parent_entity_index.to_le_bytes());
            hasher.update([shield.tier, shield.action]);
            hasher.update(shield.active_value.to_le_bytes());
            hasher.update(shield.activation_value.to_le_bytes());
            hasher.update(shield.mana_cost.to_le_bytes());
            hasher.update(shield.parent_mana.to_le_bytes());
            hasher.update(shield.parent_life.to_le_bytes());
            hasher.update(shield.parent_mana_regen.to_le_bytes());
            hasher.update(shield.parent_status_flags.to_le_bytes());
            hasher.update(shield.countdown.to_le_bytes());
            hasher.update(shield.experience_awarded.to_le_bytes());
            hasher.update([u8::from(shield.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(mailbox) = self.shield_damage_mailbox {
            hasher.update(b"shield-damage-mailbox-v1");
            hasher.update(mailbox.target_entity_index.to_le_bytes());
            hasher.update(mailbox.damage.to_le_bytes());
            hasher.update(mailbox.attacker_entity_index.to_le_bytes());
        }
        if let Some(invisible) = self.invisible {
            hasher.update([1]);
            hasher.update(invisible.holder_entity_index.to_le_bytes());
            hasher.update(invisible.parent_entity_index.to_le_bytes());
            hasher.update([invisible.tier, invisible.action]);
            hasher.update(invisible.active_value.to_le_bytes());
            hasher.update(invisible.activation_value.to_le_bytes());
            hasher.update(invisible.mana_cost.to_le_bytes());
            hasher.update(invisible.mana_regen_requirement.to_le_bytes());
            hasher.update(invisible.parent_mana.to_le_bytes());
            hasher.update(invisible.parent_life.to_le_bytes());
            hasher.update(invisible.parent_mana_regen.to_le_bytes());
            hasher.update(invisible.parent_status_flags.to_le_bytes());
            hasher.update(invisible.spell_status_flags.to_le_bytes());
            hasher.update(invisible.countdown.to_le_bytes());
            hasher.update(invisible.experience_awarded.to_le_bytes());
            hasher.update([u8::from(invisible.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(beyond_sight) = self.beyond_sight {
            hasher.update([1]);
            hasher.update(beyond_sight.holder_entity_index.to_le_bytes());
            hasher.update(beyond_sight.parent_entity_index.to_le_bytes());
            hasher.update([beyond_sight.tier, beyond_sight.action]);
            hasher.update(beyond_sight.active_value.to_le_bytes());
            hasher.update(beyond_sight.activation_value.to_le_bytes());
            hasher.update(beyond_sight.mana_cost.to_le_bytes());
            hasher.update(beyond_sight.mana_regen_requirement.to_le_bytes());
            hasher.update(beyond_sight.parent_mana.to_le_bytes());
            hasher.update(beyond_sight.parent_life.to_le_bytes());
            hasher.update(beyond_sight.parent_mana_regen.to_le_bytes());
            hasher.update(beyond_sight.countdown.to_le_bytes());
            hasher.update(beyond_sight.experience_awarded.to_le_bytes());
            hasher.update([u8::from(beyond_sight.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        hasher.update(self.beyond_sight_auxiliary.entity_index.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.next_entity.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.entity_index2.to_le_bytes());
        hasher.update([self.beyond_sight_auxiliary.effect_parameter]);
        if self.free_spell_usage {
            hasher.update(b"level2-free-spell-usage-v1");
            hasher.update([1]);
        }
        for state in &self.acquisitions {
            hasher.update([state.family]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.authored_source_index.to_le_bytes());
            hasher.update(state.owner_entity_index.to_le_bytes());
            hasher.update([state.source_action, state.holder_action]);
            hasher.update(state.replacement_action.unwrap_or(u8::MAX).to_le_bytes());
            hasher.update(state.parent_entity_index.to_le_bytes());
            hasher.update(state.presentation_timer.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility]);
            hasher.update([state.learned]);
            hasher.update(state.left_spell_slot.to_le_bytes());
            hasher.update(state.right_spell_slot.to_le_bytes());
            hasher.update([
                state.left_subspell_index.to_le_bytes()[0],
                state.right_subspell_index.to_le_bytes()[0],
                state.selected_tier.to_le_bytes()[0],
                u8::from(state.source_draw_enabled),
            ]);
            hasher.update(state.subspell_index.to_le_bytes());
            hasher.update(state.word_0x30.to_le_bytes());
            hasher.update([state.byte_0x3b]);
            hasher.update([state.byte_0x3c, state.font_type_index]);
            hasher.update(state.mana_regen.to_le_bytes());
            hasher.update(state.max_mana.to_le_bytes());
            hasher.update(state.mana.to_le_bytes());
            hasher.update(state.source_life.to_le_bytes());
            hasher.update(state.return_side.to_le_bytes());
        }
        for (source_slot, state) in &self.persistent_acquisitions {
            hasher.update([0xfe]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update([state.family, state.source_action, state.holder_action]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility, state.learned]);
            hasher.update(state.source_life.to_le_bytes());
            hasher.update([u8::from(state.source_draw_enabled)]);
        }
        hasher.update(self.speed_pickup_state.owner_entity_index.to_le_bytes());
        hasher.update([u8::from(self.speed_pickup_state.collected)]);
        hasher.update([u8::from(self.speed_pickup_state.visible)]);
        hasher.update([self.speed_pickup_state.state_timer]);
        format!("{:x}", hasher.finalize())
    }
}

impl Level2OrdinaryResetSession {
    pub(super) fn from_campaign_parts(
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
        runtime: &RuntimeSnapshot,
        presentation: &PresentationSelectors,
        terrain_planes: &TerrainPlanes,
        house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
        house_parameters: &BTreeMap<u8, HouseParameter>,
    ) -> Result<Self> {
        let houses = HouseFamily::from_campaign_entities(entities, house_parameters)
            .map_err(NativeWorldError::new)?;
        let villagers = if entities.iter().any(|entity| {
            entity.active && entity.class == VILLAGER_CLASS && entity.model == VILLAGER_MODEL
        }) {
            let config = VillagerBehaviorConfig::from_parameter_record(
                presentation.entity_parameters.get(100).ok_or_else(|| {
                    NativeWorldError::new("campaign Villager parameter row 100 is absent")
                })?,
            )
            .map_err(NativeWorldError::new)?;
            VillagerFamily::from_campaign_entities(entities, config)
                .map_err(NativeWorldError::new)?
        } else {
            VillagerFamily::empty(VillagerBehaviorConfig::LEVEL_ONE)
        };
        let dolmens = entities
            .iter()
            .filter(|entity| {
                entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
            })
            .map(|entity| Level2Dolmen::from_campaign_entity(entity, terrain_planes))
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
        let player_entities = players
            .iter()
            .take(usize::from(runtime.player_count))
            .filter_map(|player| entities.get(usize::from(player.entity_index)))
            .filter(|entity| entity.active)
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            house_build_footprints,
            house_parameters: house_parameters.clone(),
            houses,
            villagers,
            dolmens,
            players: player_entities,
            population_census: u32::try_from(
                entities
                    .iter()
                    .filter(|entity| {
                        entity.active
                            && entity.class == VILLAGER_CLASS
                            && entity.model == VILLAGER_MODEL
                    })
                    .count(),
            )
            .expect("campaign Villager count fits u32"),
        })
    }

    pub(super) fn from_parts(
        terrain: SessionTerrain,
        entities: Box<[EntitySnapshot]>,
        players: Box<[PlayerSnapshot]>,
        presentation: PresentationSelectors,
        house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
    ) -> Result<Self> {
        let houses =
            HouseFamily::from_level_two_entities(&entities).map_err(NativeWorldError::new)?;
        let villager_config = VillagerBehaviorConfig::from_parameter_record(
            presentation.entity_parameters.get(100).ok_or_else(|| {
                NativeWorldError::new("missing Level 2 villager parameter row 101")
            })?,
        )
        .map_err(NativeWorldError::new)?;
        if villager_config != VillagerBehaviorConfig::LEVEL_TWO {
            return Err(NativeWorldError::new(
                "Level 2 villager parameter row 101 does not match the shared movement config",
            ));
        }
        let villagers = VillagerFamily::from_level_two_entities(&entities, villager_config)
            .map_err(NativeWorldError::new)?;
        let dolmens = [108_u16, 109, 110]
            .iter()
            .map(|slot| {
                Level2Dolmen::from_entity(
                    entities.get(usize::from(*slot)).ok_or_else(|| {
                        NativeWorldError::new(format!("missing Level 2 dolmen {slot}"))
                    })?,
                    terrain.planes(),
                )
            })
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
        validate_level2_ordinary_census(&entities, &presentation)?;
        let player_entities = players
            .iter()
            .filter_map(|player| entities.get(usize::from(player.entity_index)))
            .filter(|entity| entity.active)
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let population_census = u32::try_from(
            villagers
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .count(),
        )
        .expect("Level 2 ordinary population census fits u32");
        Ok(Self {
            house_build_footprints,
            house_parameters: (0..=u8::MAX)
                .filter_map(|index| {
                    HouseParameter::level_two(index)
                        .or_else(|| HouseParameter::level_one(index))
                        .map(|parameter| (index, parameter))
                })
                .collect(),
            houses,

            villagers,

            dolmens,

            players: player_entities,

            population_census,
        })
    }

    #[must_use]
    pub fn houses(&self) -> &[HouseState] {
        self.houses.houses()
    }

    pub(crate) fn take_inactive_houses(&mut self) -> Vec<HouseState> {
        self.houses.reclaim_inactive()
    }

    pub(crate) fn reclaim_inactive_villagers(&mut self) {
        self.villagers.reclaim_inactive();
    }

    /// Skeleton conversion hides the imported arena row and its typed owner
    /// together. Allocation failure does not undo the source hide.
    pub(crate) fn disable_villager_source(&mut self, source_slot: u16) -> bool {
        self.villagers.disable_source_slot(source_slot)
    }

    pub(crate) fn step_villager_source(
        &mut self,
        arena: &mut crate::campaign::source_arena::SourceArena,
        terrain: &mut SessionTerrain,
        source_slot: u16,
        frozen_building_slots: &[u16],
        runtime: &mut RuntimeSnapshot,
        models: &[Vec<u16>; 29],
        players: &[u16],
        particles: &[ParticleParameter],
    ) -> std::result::Result<creatures::settlements::SettlementVillagerStep, String> {
        let mut views = arena.settlement_source_views(
            &self.houses,
            self.villagers.villagers(),
            &self.house_parameters,
        );
        for view in &mut views {
            // A current tracked target still resolves through the arena;
            // only the search traverses the pass-start building membership.
            view.villager_entry_eligible &=
                frozen_building_slots.contains(&view.source_entity_index);
        }
        let targets =
            creatures::settlements::stage::targets(arena, &views, self.villagers.villagers());
        let mut stage = creatures::settlements::stage::StageContext {
            runtime: Some(runtime),
            targets: &targets,
            models,
            players,
            effects: Vec::new(),
        };
        let effects = self.villagers.step_source_slot_with_entry(
            source_slot,
            terrain,
            &views,
            Some(&mut stage),
            |target| arena.try_enter_settlement_house(&mut self.houses, target),
        );
        if let Some(villager) = self
            .villagers
            .villagers()
            .iter()
            .find(|row| row.entity_index == source_slot)
        {
            let source = arena
                .entities
                .get_mut(usize::from(source_slot))
                .filter(|row| row.active && row.class == 5 && row.model == villager.model)
                .ok_or_else(|| format!("Villager {source_slot} lost its imported arena header"))?;
            villager.publish_source(source);
            arena
                .economy
                .map_chains
                .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: villager.position.x,
                    y: villager.position.y,
                })
                .map_err(|error| format!("Villager {source_slot} map publication: {error}"))?;
            if let Some(target) = villager.target_entity_index
                && let Some(house) = self
                    .houses
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == target)
            {
                house.publish_source(&mut arena.entities[usize::from(target)]);
            }
        }
        let spell_awards = creatures::settlements::runtime::apply_effects(
            arena,
            &mut self.villagers,
            terrain,
            particles,
            stage.effects,
        )?;
        Ok(creatures::settlements::SettlementVillagerStep {
            effects,
            spell_awards,
        })
    }

    pub(crate) fn step_settlement_source(
        &mut self,
        arena: &mut crate::campaign::source_arena::SourceArena,
        terrain: &mut SessionTerrain,
        source_slot: u16,
        frozen_building_slots: &[u16],
        runtime: &mut RuntimeSnapshot,
        models: &[Vec<u16>; 29],
        players: &[u16],
        particles: &[ParticleParameter],
    ) -> std::result::Result<Option<creatures::settlements::SettlementPopulationStep>, String> {
        arena.step_settlement_resident_slot(
            source_slot,
            terrain,
            &mut self.houses,
            &mut self.villagers,
            frozen_building_slots,
            &self.house_parameters,
            &self.house_build_footprints,
            runtime,
            models,
            players,
            particles,
        )
    }

    pub(crate) fn settlement_building_slots(
        &self,
        arena: &crate::campaign::source_arena::SourceArena,
    ) -> Vec<u16> {
        arena
            .settlement_source_views(
                &self.houses,
                self.villagers.villagers(),
                &self.house_parameters,
            )
            .into_iter()
            .filter(|row| row.active && row.class == 10 && row.model == 45)
            .map(|row| row.source_entity_index)
            .collect()
    }

    pub(crate) fn current_building_rows(
        &self,
        arena: &crate::campaign::source_arena::SourceArena,
    ) -> Vec<(u16, u8, Position)> {
        let mut rows = arena
            .live_model45_buildings()
            .into_iter()
            .map(|(slot, selector, position)| (slot, (selector, position)))
            .collect::<BTreeMap<_, _>>();
        let dynamic = arena
            .live_arena_slots()
            .into_iter()
            .map(|(slot, _)| slot)
            .collect::<BTreeSet<_>>();
        for house in self.houses.houses() {
            if house.active {
                rows.insert(house.entity_index, (house.building_index, house.position));
            } else if !dynamic.contains(&house.entity_index) {
                rows.remove(&house.entity_index);
            }
        }
        rows.into_iter()
            .map(|(slot, (selector, position))| (slot, selector, position))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn house_mut(&mut self, source_slot: u16) -> Option<&mut HouseState> {
        self.houses.house_mut(source_slot)
    }

    pub(crate) fn advance_house_source_phase(&mut self, source_slot: u16) -> Option<u8> {
        self.houses.advance_source_phase(source_slot)
    }

    #[must_use]
    pub fn villagers(&self) -> &[VillagerState] {
        self.villagers.villagers()
    }

    pub(crate) fn bind_frame_zero_villager_variant(
        &mut self,
        entity_index: u16,
        package_position: Position,
        authored_position: Position,
        package_max_life: i32,
        authored_max_life: i32,
    ) -> Result<()> {
        self.villagers
            .bind_frame_zero_semantic_variant(
                entity_index,
                package_position,
                authored_position,
                package_max_life,
                authored_max_life,
            )
            .map_err(NativeWorldError::new)
    }

    #[must_use]
    pub fn dolmens(&self) -> &[Level2Dolmen] {
        &self.dolmens
    }

    #[must_use]
    pub fn player_interaction_flags(&self) -> Vec<(u16, u32)> {
        self.players
            .iter()
            .map(|entity| (entity.index, entity.control_flags))
            .collect()
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(self.population_census.to_le_bytes());
        hasher.update(b"settlement-construction-bank-v1");
        for (selector, parameter) in &self.house_parameters {
            hasher.update([*selector, parameter.flags, parameter.destruction_successor]);
            hasher.update(parameter.life_mana_scalar.to_le_bytes());
        }
        hasher
            .update(format!("{:?}{:?}{:?}", self.houses, self.villagers, self.dolmens).as_bytes());
        for entity in &self.players {
            hasher.update(entity.record_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Source `sub_1B8C0` decrements the class-5 population census when a
    /// movement attempt exhausts all headings and kills the resident. Keep
    /// this counter in the reset-scoped ordinary package; the Level 2 mission
    /// allocator is a separate, later sink and must not be inferred here.
    #[must_use]
    pub const fn population_census(&self) -> u32 {
        self.population_census
    }

    pub(super) fn apply_villager_effects(&mut self, effects: &[VillagerStepEffect]) {
        for effect in effects {
            if matches!(effect, VillagerStepEffect::PopulationCensusDecrement { .. }) {
                self.population_census = self.population_census.saturating_sub(1);
            }
        }
    }

    pub fn step_house_action_52(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> Vec<HouseStepEvent> {
        self.houses.step_source_slot(source_slot, terrain)
    }

    pub(crate) fn bind_frame_zero_house_position(
        &mut self,
        entity_index: u16,
        package_position: Position,
        authored_position: Position,
    ) -> Result<()> {
        self.houses
            .bind_frame_zero_semantic_position(entity_index, package_position, authored_position)
            .map_err(NativeWorldError::new)
    }

    pub(super) fn apply_house_terrain_events(
        &mut self,
        economy: &mut crate::campaign::source_arena::SourceArena,
        goats: &mut crate::goat::GoatFamily,
        events: &[HouseStepEvent],
        terrain: &mut SessionTerrain,
    ) -> Result<()> {
        for event in events {
            match *event {
                HouseStepEvent::ConstructionPass {
                    entity_index,
                    source_id,
                    position,
                    building_index,
                    remaining,
                    first_pass,
                } => {
                    if first_pass {
                        let footprint = self
                            .house_build_footprints
                            .get(&building_index)
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "Level 2 House BUILD footprint {building_index} is absent"
                                ))
                            })?;
                        let (width, height) = (footprint.width, footprint.height);
                        crate::world::terrain::disasters::mc2::burial::footprint(
                            economy,
                            Some(super::area_damage::ActorDomains::new(goats, self)),
                            position,
                            width,
                            height,
                            source_id,
                        )
                        .map_err(NativeWorldError::new)?;
                    }
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    terrain
                        .apply_house_construction_with_clearance(
                            position,
                            Some(remaining),
                            footprint,
                            self.houses
                                .houses()
                                .iter()
                                .find(|house| house.entity_index == entity_index)
                                .ok_or_else(|| {
                                    NativeWorldError::new("construction lost its House carrier")
                                })?
                                .parameter
                                .flags
                                & 4
                                == 0,
                        )
                        .map_err(NativeWorldError::new)?;
                }
                HouseStepEvent::ConstructionCompleted {
                    entity_index,
                    position,
                    building_index,
                    ..
                } => {
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    terrain
                        .apply_house_construction_with_clearance(
                            position,
                            None,
                            footprint,
                            self.houses
                                .houses()
                                .iter()
                                .find(|house| house.entity_index == entity_index)
                                .ok_or_else(|| {
                                    NativeWorldError::new(
                                        "construction completion lost its House carrier",
                                    )
                                })?
                                .parameter
                                .flags
                                & 4
                                == 0,
                        )
                        .map_err(NativeWorldError::new)?;
                }
                HouseStepEvent::RemoveFootprint {
                    entity_index,
                    attacker_entity_index,
                    position,
                    building_index,
                    occupants,
                    rng,
                    ..
                } => {
                    let produces_residents = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .is_some_and(|house| house.parameter.produces_villagers());
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    let outcome = terrain
                        .apply_house_removal(
                            position,
                            rng,
                            occupants,
                            produces_residents,
                            footprint,
                        )
                        .map_err(NativeWorldError::new)?;
                    let owner_entity_index = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .map_or(0, |house| house.owner_entity_index);
                    let requests = outcome
                        .residents
                        .iter()
                        .map(|resident| {
                            (
                                entity_index,
                                owner_entity_index,
                                HouseResidentRequest {
                                    position: resident.position,
                                    model: resident.model,
                                    action: resident.action,
                                    attacker_entity_index,
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    // `RemoveCastleStage_385C0` attempts each NewEvent in
                    // row-major order. A null allocation skips only that
                    // resident; earlier products, House RNG/occupants, and
                    // terrain mutations remain committed.
                    for (parent_house_entity_index, owner_entity_index, request) in requests {
                        if let Some(slot) = economy.allocate_house_resident(
                            parent_house_entity_index,
                            owner_entity_index,
                            request,
                            terrain,
                        ) {
                            economy.initialize_demolition_resident_mailbox(
                                slot,
                                attacker_entity_index,
                            );
                        }
                    }
                    self.houses
                        .commit_removal_state(
                            entity_index,
                            outcome.final_rng,
                            outcome.final_occupants,
                        )
                        .map_err(NativeWorldError::new)?;
                }
                HouseStepEvent::ResidentRequested {
                    entity_index,
                    attacker_entity_index,
                    position,
                    model,
                    action,
                } => {
                    let owner_entity_index = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .map_or(0, |house| house.owner_entity_index);
                    let request = HouseResidentRequest {
                        position,
                        model,
                        action,
                        attacker_entity_index,
                    };
                    // `AddHouse0A_2D_38330` mutates the House before
                    // NewEvent. A null result simply produces no resident.
                    let _ = economy.allocate_house_resident(
                        entity_index,
                        owner_entity_index,
                        request,
                        terrain,
                    );
                }
                HouseStepEvent::Possessed {
                    entity_index,
                    owner_entity_index,
                    payload: _,
                } => {
                    economy.prepare_sound(entity_index, owner_entity_index, -1, 4);
                    economy.set_house_owner_relation(entity_index, owner_entity_index);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {}
            }
        }
        Ok(())
    }

    pub fn step_villager_action_105(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> Vec<VillagerStepEffect> {
        let mut source_views = self
            .houses
            .houses()
            .iter()
            .map(house_source_entity_view)
            .chain(
                self.villagers
                    .villagers()
                    .iter()
                    .map(villager_source_entity_view),
            )
            .collect::<Vec<_>>();
        source_views.sort_unstable_by_key(|view| view.source_entity_index);
        self.villagers
            .step_source_slot(source_slot, terrain, &source_views, &mut self.houses)
    }

    pub fn inject_house_damage(
        &mut self,
        source_slot: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<()> {
        self.houses
            .inject_damage(source_slot, damage, attacker)
            .map_err(NativeWorldError::new)
    }

    pub fn submit_house_possession(
        &mut self,
        source_slot: u16,
        owner_entity_index: u16,
        payload: u32,
    ) -> Result<()> {
        self.houses
            .submit_possession(source_slot, owner_entity_index, payload)
            .map_err(NativeWorldError::new)
    }

    pub fn inject_villager_damage(
        &mut self,
        source_slot: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<()> {
        self.villagers
            .inject_damage(source_slot, damage, attacker)
            .map_err(NativeWorldError::new)
    }

    pub fn step_dolmen_action_6(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> Result<Level2DolmenStep> {
        let index = self
            .dolmens
            .iter()
            .position(|dolmen| dolmen.entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unknown Level 2 dolmen {source_slot}"))
            })?;
        let dolmen_entity = self.dolmens[index].entity.clone();
        for player in &mut self.players {
            if player.active && player.life >= 0 && level2_source_intersects(&dolmen_entity, player)
            {
                player.record[17] |= 0x10;
                player.control_flags = u32::from_le_bytes([
                    player.record[16],
                    player.record[17],
                    player.record[18],
                    player.record[19],
                ]);
            }
        }
        Ok(self.dolmens[index].step_action_6(terrain))
    }
}

pub(super) fn validate_level2_ordinary_census(
    entities: &[EntitySnapshot],
    presentation: &PresentationSelectors,
) -> Result<()> {
    const PARAMETER_59: [u8; ENTITY_PARAMETER_BYTES] = [
        0, 0, 56, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255, 255, 0,
        12, 50, 0, 0, 16, 0, 2, 0, 0,
    ];
    const PARAMETER_100: [u8; ENTITY_PARAMETER_BYTES] = [
        41, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 1, 0, 0, 0, 255, 15, 0, 0, 2, 254, 254, 255, 255, 0, 9,
        40, 0, 0, 14, 0, 2, 9, 0,
    ];
    if presentation.entity_parameters.get(59) != Some(&PARAMETER_59)
        || presentation.entity_parameters.get(100) != Some(&PARAMETER_100)
    {
        return Err(NativeWorldError::new(
            "Level 2 ordinary resident/static parameter rows changed",
        ));
    }
    let particle = |index: u16| presentation.particles.get(usize::from(index)).copied();
    if particle(177)
        != Some(ParticleParameter {
            base_tmap: 96,
            word_2: 8492,
            word_4: 42,
            speed: 389,
            rotation_speed: 400,
            byte_10: 0,
            byte_11: -1,
            render_mode: 0,
            byte_13: 0,
        })
    {
        return Err(NativeWorldError::new(
            "Level 2 ordinary house particle row 177 changed",
        ));
    }
    if presentation.level_sprite_selectors.get(96) != Some(&255) {
        return Err(NativeWorldError::new(
            "Level 2 ordinary house particle selector 96 changed",
        ));
    }
    let expected_particles = [
        (
            39_u16,
            ParticleParameter {
                base_tmap: 79,
                word_2: 8492,
                word_4: 42,
                speed: 666,
                rotation_speed: 400,
                byte_10: 0,
                byte_11: 1,
                render_mode: 0,
                byte_13: 0,
            },
            0_u8,
        ),
        (
            239,
            ParticleParameter {
                base_tmap: 155,
                word_2: 8492,
                word_4: 42,
                speed: 90,
                rotation_speed: 200,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            241,
            ParticleParameter {
                base_tmap: 180,
                word_2: 8492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            242,
            ParticleParameter {
                base_tmap: 183,
                word_2: 8492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            271,
            ParticleParameter {
                base_tmap: 199,
                word_2: 8492,
                word_4: 42,
                speed: 96,
                rotation_speed: 300,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
    ];
    for (index, expected, selector) in expected_particles {
        if particle(index) != Some(expected)
            || presentation
                .level_sprite_selectors
                .get(usize::from(expected.base_tmap))
                != Some(&selector)
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary presentation particle {index} changed"
            )));
        }
    }
    for (ordinal, index) in (1_u16..=22).enumerate() {
        let entity = entities
            .get(usize::from(index))
            .ok_or_else(|| NativeWorldError::new(format!("missing Level 2 house {index}")))?;
        if !entity.active
            || entity.class != 10
            || entity.model != 45
            || entity.action != 52
            || entity.source_word_10() != 2
            || entity.control_flags != 0x0000_000d
            || entity.particle_parameter_index != Some(177)
            || entity.parameter_relation != Some(59)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
            || sha256_hex(entity.record_bytes()) != LEVEL2_ORDINARY_HOUSE_RECORD_SHA256[ordinal]
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary house census row {index} changed"
            )));
        }
    }
    for (ordinal, index) in LEVEL_TWO_VILLAGER_INDICES.iter().copied().enumerate() {
        let entity = entities
            .get(usize::from(index))
            .ok_or_else(|| NativeWorldError::new(format!("missing Level 2 villager {index}")))?;
        let expected_links = match index {
            77 => (0, 105),
            105 => (77, 0),
            97 => (0, 106),
            106 => (97, 0),
            _ => (0, 0),
        };
        let census_mismatch = !entity.active
            || entity.class != 5
            || entity.model != 13
            || entity.action != 105
            || entity.source_word_10() != 2
            || entity.control_flags != 0x0000_000c
            || entity.max_life != 1_000
            || entity.life != 1_000
            || entity.actual_speed != if index == 77 { 30 } else { 18 }
            || entity.parameter_relation != Some(100)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != expected_links.0
            || entity.next_entity() != expected_links.1
            || !matches!(entity.particle_parameter_index, Some(239 | 241 | 242 | 271))
            || sha256_hex(entity.record_bytes()) != LEVEL2_ORDINARY_VILLAGER_RECORD_SHA256[ordinal];
        if census_mismatch {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary villager census row {index} changed"
            )));
        }
    }
    Ok(())
}
