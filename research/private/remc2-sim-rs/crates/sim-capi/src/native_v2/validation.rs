//! Validation, conversion, asset loading, and buffer-marshalling helpers.
//!
//! This family is a pure move from the native_v2 facade. Parent-scoped
//! visibility preserves the existing sibling and test call graph.

use super::*;

pub(super) fn validate_command(command: &McCommandV2) -> AbiResult<()> {
    validate_reserved(&command.reserved, "command")?;
    if command.requested_spell != MC_REQUESTED_SPELL_NONE
        && !(0..=28).contains(&command.requested_spell)
    {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            "requested spell must be -1 or 0..=28",
        ));
    }
    Ok(())
}

pub(super) fn validate_embedded_command(command: &McCommandV2) -> AbiResult<()> {
    if command.abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!(
                "campaign step command ABI version {} is unsupported",
                command.abi_version
            ),
        ));
    }
    let required = struct_size::<McCommandV2>();
    if command.struct_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!(
                "campaign step command struct_size {} is smaller than {required}",
                command.struct_size
            ),
        ));
    }
    validate_command(command)
}

pub(super) fn campaign_event_snapshot(
    generation: u64,
    ordinal: usize,
    event: &Level2DispatchEvent,
) -> McCampaignEventV2 {
    let (pass, projected_kind, family, flags, source_slot) = event.campaign_projection();
    let kind = match event {
        Level2DispatchEvent::Economy { .. } => MC_CAMPAIGN_EVENT_ECONOMY,
        Level2DispatchEvent::Goat { .. } => MC_CAMPAIGN_EVENT_GOAT,
        Level2DispatchEvent::SpellAcquisition { .. } => MC_CAMPAIGN_EVENT_SPELL_ACQUISITION,
        Level2DispatchEvent::AuthoredGroupActivated { .. } => MC_CAMPAIGN_EVENT_AUTHORED_GROUP,
        _ => projected_kind,
    };
    McCampaignEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        kind,
        family,
        flags,
        source_slot,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

pub(super) fn campaign_shield_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::ShieldState>,
) -> McCampaignShieldSnapshotV2 {
    let mut snapshot = McCampaignShieldSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignShieldSnapshotV2>(),
        generation,
        ..McCampaignShieldSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.parent_status_flags = state.parent_status_flags;
        snapshot.experience_awarded = state.experience_awarded;
    }
    snapshot
}

pub(super) fn campaign_invisible_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::InvisibleState>,
) -> McCampaignInvisibleSnapshotV2 {
    let mut snapshot = McCampaignInvisibleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignInvisibleSnapshotV2>(),
        generation,
        ..McCampaignInvisibleSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.parent_visibility_mode = state.parent_visibility_mode;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.mana_regen_requirement = state.mana_regen_requirement;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.parent_status_flags = state.parent_status_flags;
        snapshot.spell_status_flags = state.spell_status_flags;
        snapshot.experience_awarded = state.experience_awarded;
        snapshot.cleanup_requested = u8::from(state.cleanup_requested);
    }
    snapshot
}

pub(super) fn campaign_beyond_sight_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::BeyondSightState>,
) -> McCampaignBeyondSightSnapshotV2 {
    let mut snapshot = McCampaignBeyondSightSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignBeyondSightSnapshotV2>(),
        generation,
        ..McCampaignBeyondSightSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.mana_regen_requirement = state.mana_regen_requirement;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.experience_awarded = state.experience_awarded;
        snapshot.cleanup_requested = u8::from(state.cleanup_requested);
    }
    snapshot
}

pub(super) fn campaign_opponent_snapshot(
    generation: u64,
    participant: sim_core::native_world::Level2NyphurParticipant,
    visibility: sim_core::native_world::Level2NyphurVisibility,
) -> McCampaignOpponentSnapshotV2 {
    let (objective_relation_domain, objective_relation_value) =
        match participant.objective_stage_relation {
            sim_core::native_world::RuntimeRelation::LiveEntity(index) => (2, index),
            _ => (0, 0),
        };
    McCampaignOpponentSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignOpponentSnapshotV2>(),
        generation,
        active: 1,
        player_slot: participant.player_slot,
        is_ai: u8::from(participant.is_ai),
        color_index: participant.color_index,
        entity_index: participant.entity_index,
        entity_class: participant.entity_class,
        entity_model: participant.entity_model,
        entity_action: participant.entity_action,
        entity_source_id: participant.entity_source_id,
        entity_next_relation: participant.entity_next_relation.unwrap_or(u16::MAX),
        entity_next_entity: participant.entity_next_entity,
        entity_control_flags: participant.entity_control_flags,
        entity_life: participant.entity_life,
        entity_max_life: participant.entity_max_life,
        entity_mana: participant.entity_mana,
        entity_max_mana: participant.entity_max_mana,
        fireball_holder_index: participant.fireball_holder_index,
        fireball_action: participant.fireball_action,
        fireball_parent_entity_index: participant.fireball_parent_entity_index,
        fireball_next_entity: participant.fireball_next_entity,
        possession_holder_index: participant.possession_holder_index,
        possession_action: participant.possession_action,
        possession_parent_entity_index: participant.possession_parent_entity_index,
        possession_next_entity: participant.possession_next_entity,
        objective_relation_domain,
        objective_relation_value,
        visibility_flags: u32::from(visibility.visible_to_local_player)
            | (u32::from(visibility.owner_visibility_mode != 0) << 1)
            | (u32::from(visibility.beyond_sight_holder_present) << 2)
            | (u32::from(visibility.beyond_sight_active) << 3)
            | (u32::from(visibility.family4_gate_active) << 4),
        ..McCampaignOpponentSnapshotV2::default()
    }
}

pub(super) fn campaign_day_hud_spell_slot(
    slot: Level2DayHudSpellSlot,
) -> McCampaignDayHudSpellSlotV2 {
    McCampaignDayHudSpellSlotV2 {
        holder_entity_index: slot.holder_entity_index,
        parent_entity_index: slot.parent_entity_index,
        raw_spell_selector: slot.raw_spell_selector,
        color_index: slot.color_index,
        model: slot.model,
        tier: slot.tier,
        action: slot.action,
        action_marker: slot.action_marker,
        active_value: slot.active_value,
        activation_value: slot.activation_value,
        control_flags: slot.control_flags,
        max_mana: slot.max_mana,
        mana_regen: slot.mana_regen,
        caster_mana: slot.caster_mana,
        frame_selector: slot.frame_selector,
        icon_selector: slot.icon_selector,
        level_text_selector: slot.level_text_selector,
        draw_admitted: u8::from(slot.draw_admitted),
        reserved: [0; 2],
    }
}

pub(super) fn campaign_day_hud_snapshot(
    generation: u64,
    snapshot: Level2DayHudSnapshot,
) -> McCampaignDayHudSnapshotV2 {
    let nyphur =
        campaign_opponent_snapshot(generation, snapshot.nyphur, snapshot.nyphur_visibility);
    McCampaignDayHudSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayHudSnapshotV2>(),
        generation,
        tick: snapshot.tick,
        local_player_slot: snapshot.local_player_slot,
        local_player_active: u8::from(snapshot.local_player_active),
        local_player_color_index: snapshot.local_player_color_index,
        hit_frame_time_pre_draw: snapshot.hit_frame_time_pre_draw,
        local_player_entity_index: snapshot.local_player_entity_index,
        color_phase_1: snapshot.color_phase_1,
        color_phase_2: snapshot.color_phase_2,
        local_player_control_flags: snapshot.local_player_control_flags,
        local_player_life: snapshot.local_player_life,
        local_player_max_life: snapshot.local_player_max_life,
        local_player_mana: snapshot.local_player_mana,
        local_player_max_mana: snapshot.local_player_max_mana,
        global_mana_denominator: snapshot.global_mana_denominator,
        minimap_panel_selector: snapshot.minimap_panel_selector,
        castle_panel_selector: snapshot.castle_panel_selector,
        balloon_panel_selector: snapshot.balloon_panel_selector,
        player_panel_selector: snapshot.player_panel_selector,
        player_icon_selector: snapshot.player_icon_selector,
        life_mana_icon_selector: snapshot.life_mana_icon_selector,
        life_palette_index: snapshot.life_palette_index,
        mana_palette_index: snapshot.mana_palette_index,
        mana_max_palette_index: snapshot.mana_max_palette_index,
        reserved_0: 0,
        nyphur,
        left: campaign_day_hud_spell_slot(snapshot.left),
        right: campaign_day_hud_spell_slot(snapshot.right),
    }
}

pub(super) fn campaign_economy_event_snapshot(
    generation: u64,
    ordinal: usize,
    pass: u8,
    source_slot: u16,
    event: Level2EconomyEvent,
) -> McCampaignEconomyEventV2 {
    let (code, primary_id, secondary_id, entity_0, entity_1, value_0, value_1, flags) = match event
    {
        Level2EconomyEvent::ManaSphereSpawned {
            sphere_id,
            source_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED,
            sphere_id,
            0,
            source_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereClaimed {
            sphere_id,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED,
            sphere_id,
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereCollected {
            sphere_id,
            carrier_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED,
            sphere_id,
            carrier_id,
            0,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED,
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED,
            u32::from(source_entity_index),
            0,
            source_entity_index,
            owner_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierArrived {
            carrier_id,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED,
            carrier_id,
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderArrived {
            defender_slot,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED,
            u32::from(defender_slot),
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCollectedSphere {
            defender_slot,
            sphere_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE,
            u32::from(defender_slot),
            sphere_id,
            defender_slot,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index,
            parent_entity_index,
            id_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            id_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstExpired {
            source_entity_index,
            parent_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_BUILT,
            u32::from(source_entity_index),
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestroyed {
            source_entity_index,
            killer_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED,
            u32::from(source_entity_index),
            0,
            killer_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleConstructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCaveSound {
            defender_slot,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND,
            u32::from(defender_slot),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::FireSphereCollisionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::AuthoredClass5Model3Sound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        // Keep the generic prepared-sound payload on the existing typed
        // source-audio code; family 8 distinguishes it without allocating a
        // new ABI code.
        Level2EconomyEvent::PreparedSound {
            source_entity_index,
            entity_index,
            player_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND,
            u32::from(source_entity_index),
            u32::from(entity_index),
            sound_id,
            0,
            i32::from(player_index),
            0,
            0,
        ),
        Level2EconomyEvent::CastleStageDestroyed {
            source_entity_index,
            previous_level,
            next_level,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED,
            u32::from(source_entity_index),
            0,
            0,
            0,
            i32::from(previous_level),
            i32::from(next_level),
            0,
        ),
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage,
            lethal,
        } => (
            MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED,
            u32::from(target_entity_index),
            0,
            attacker_entity_index,
            0,
            damage,
            0,
            u8::from(lethal),
        ),
    };
    McCampaignEconomyEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEconomyEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        family: sim_core::native_world::level2_economy_event_family(event),
        code,
        flags,
        source_slot,
        reserved_0: 0,
        primary_id,
        secondary_id,
        entity_0,
        entity_1,
        value_0,
        value_1,
        reserved: [0; 4],
    }
}

pub(super) fn campaign_goat_event_snapshot(
    generation: u64,
    ordinal: usize,
    event_index: usize,
    pass: u8,
    source_slot: u16,
    event: GoatStepEvent,
) -> McCampaignGoatEventV2 {
    let mut row = McCampaignGoatEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignGoatEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign Goat ordinal fits u32"),
        pass,
        source_slot,
        event_index: u16::try_from(event_index)
            .expect("bounded campaign Goat event index fits u16"),
        asset_ref_index: MC_ASSET_REF_NONE,
        ..McCampaignGoatEventV2::default()
    };
    match event {
        GoatStepEvent::Bleat {
            entity_index,
            sound_id,
            position,
            action,
            ..
        } => {
            row.code = MC_CAMPAIGN_GOAT_BLEAT;
            row.entity_0 = entity_index;
            row.sound_id = sound_id;
            row.action = action;
            row.x = position.x;
            row.y = position.y;
            row.z = position.z;
        }
        GoatStepEvent::ManaConverted { entity_index, mana } => {
            row.code = MC_CAMPAIGN_GOAT_MANA_CONVERTED;
            row.entity_0 = entity_index;
            row.value_0 = mana;
        }
        GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_EXTRA_MANA_SPHERE_REQUEST;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::RemovalQueued { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_REMOVAL_QUEUED;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::Reclaimed { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_RECLAIMED;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::EngagementBoundaryReached {
            entity_index,
            target_entity_index,
        } => {
            row.code = MC_CAMPAIGN_GOAT_ENGAGEMENT_BOUNDARY;
            row.entity_0 = entity_index;
            row.entity_1 = target_entity_index;
        }
    }
    row
}

pub(super) fn decode_sha256_hex(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    let mut digest = [0_u8; 32];
    if bytes.len() != 64 {
        return digest;
    }
    let mut index = 0;
    while index < 32 {
        let high = hex_nibble(bytes[index * 2]);
        let low = hex_nibble(bytes[index * 2 + 1]);
        let Some(high) = high else { return [0; 32] };
        let Some(low) = low else { return [0; 32] };
        digest[index] = (high << 4) | low;
        index += 1;
    }
    digest
}

pub(super) const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub(super) fn validate_camera_input(camera_input: &McCameraInputV2) -> AbiResult<()> {
    validate_reserved(&camera_input.reserved, "camera input")
}

pub(super) fn validate_reserved(reserved: &[u32], name: &str) -> AbiResult<()> {
    if reserved.iter().any(|value| *value != 0) {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            format!("{name} reserved fields must be zero"),
        ));
    }
    Ok(())
}

pub(super) fn player_command(command: McCommandV2) -> AbiResult<PlayerCommand> {
    let (opcode, argument_1) = if command.requested_spell == MC_REQUESTED_SPELL_NONE {
        (command.opcode, command.argument_1)
    } else {
        (
            6,
            i8::try_from(command.requested_spell)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "requested spell exceeds i8"))?,
        )
    };
    Ok(PlayerCommand {
        opcode,
        argument_1,
        argument_2: command.argument_2,
        roll: command.roll,
        pitch: command.pitch,
        flags: command.flags,
        ..PlayerCommand::default()
    })
}

pub(super) fn world_string(world: &NativeWorld, kind: u32) -> AbiResult<&str> {
    match kind {
        MC_WORLD_STRING_SCHEMA => Ok("mc2-owner-level-export/v2"),
        MC_WORLD_STRING_PACKAGE_SHA256 => Ok(world.package_sha256()),
        MC_WORLD_STRING_CLOSURE_SHA256 => Ok(world.closure_sha256()),
        MC_WORLD_STRING_STATE_HASH_SCHEMA => Ok("mc2-flight-state-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA => Ok("mc2-flight-conformance-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONTEXT_SEAM => {
            Ok("Level1 control_mode=0; regression-proven, not serialized by package v2")
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown world string kind {kind}"),
        )),
    }
}

pub(super) fn validate_plane_kind(planes: &TerrainPlanes, kind: u32) -> AbiResult<()> {
    match kind {
        MC_TERRAIN_TYPES_U8
        | MC_TERRAIN_HEIGHT_U8
        | MC_TERRAIN_SHADE_U8
        | MC_TERRAIN_ANGLE_U8
        | MC_TERRAIN_SECONDARY_U8 => {
            terrain_u8_plane(planes, kind)?;
            Ok(())
        }
        MC_TERRAIN_ENTITY_INDEX_I16 => Ok(()),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown terrain plane kind {kind}"),
        )),
    }
}

pub(super) fn terrain_u8_plane(planes: &TerrainPlanes, kind: u32) -> AbiResult<&[u8]> {
    match kind {
        MC_TERRAIN_TYPES_U8 => Ok(&planes.terrain_types),
        MC_TERRAIN_HEIGHT_U8 => Ok(&planes.ground_heights),
        MC_TERRAIN_SHADE_U8 => Ok(&planes.shading),
        MC_TERRAIN_ANGLE_U8 => Ok(&planes.angles),
        MC_TERRAIN_SECONDARY_U8 => Ok(&planes.secondary_heights),
        MC_TERRAIN_ENTITY_INDEX_I16 => Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "entity-index terrain plane requires the i16 copy API",
        )),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown u8 terrain plane kind {kind}"),
        )),
    }
}

pub(super) fn entity_snapshot(
    entity: &EntitySnapshot,
    state: &FlightState,
    live_world: &LiveWorldState,
    mission: &Level1MissionState,
) -> McEntitySnapshotV2 {
    let (has_parameter_record, parameter_record_index) = entity
        .parameter_relation
        .map_or((0, MC_PARAMETER_RECORD_NONE), |index| (1, index));
    let mut snapshot = McEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEntitySnapshotV2>(),
        entity_index: entity.index,
        parameter_record_index,
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        has_parameter_record,
        reserved_0: 0,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
        reserved: [0; 4],
    };
    if snapshot.active != 0 && !mission.imported_source_slot_is_live(entity.index) {
        snapshot.authority = MC_AUTHORITY_INACTIVE;
        snapshot.active = 0;
    }
    if entity.authority == EntityAuthority::PlayerFlightLive && entity.index == state.entity_index {
        snapshot.x = state.position.x;
        snapshot.y = state.position.y;
        snapshot.z = state.position.z;
        snapshot.yaw = state.yaw;
        snapshot.pitch = state.display_pitch;
        snapshot.fov = state.entity_fov;
        snapshot.actual_speed = state.actual_speed;
        snapshot.phase = state.entity_phase;
    } else if entity.authority == EntityAuthority::GoatLive
        && let Some(goat) = live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_GOAT_LIVE;
        snapshot.active = u8::from(goat.active);
        snapshot.action = goat.action;
        snapshot.phase = goat.phase;
        snapshot.x = goat.position.x;
        snapshot.y = goat.position.y;
        snapshot.z = goat.position.z;
        snapshot.yaw = goat.yaw;
        snapshot.pitch = goat.pitch;
        snapshot.actual_speed = goat.actual_speed;
    } else if entity.authority == EntityAuthority::VillagerLive
        && mission.imported_source_slot_is_live(entity.index)
        && let Some(villager) = live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_VILLAGER_LIVE;
        snapshot.active = u8::from(villager.active);
        snapshot.action = villager.action;
        snapshot.phase = villager.phase;
        snapshot.x = villager.position.x;
        snapshot.y = villager.position.y;
        snapshot.z = villager.position.z;
        snapshot.yaw = villager.yaw;
        snapshot.pitch = villager.pitch;
        snapshot.actual_speed = villager.actual_speed;
    }
    if let Some(resident) = mission
        .dynamic_residents()
        .find(|resident| resident.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(resident.active);
        snapshot.class_id = resident.class;
        snapshot.model = resident.model;
        snapshot.action = resident.action;
        snapshot.phase = resident.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = match resident.kind {
            sim_core::level1_mission::DynamicResidentKind::Archer => 75,
            sim_core::level1_mission::DynamicResidentKind::Builder => 101,
            sim_core::level1_mission::DynamicResidentKind::Villager
            | sim_core::level1_mission::DynamicResidentKind::Trader => 100,
        };
        snapshot.x = resident.position.x;
        snapshot.y = resident.position.y;
        snapshot.z = resident.position.z;
        snapshot.yaw = resident.yaw;
        snapshot.pitch = 0;
        snapshot.actual_speed = resident.actual_speed;
    }
    if let Some(ambient) = mission
        .ambient_entities()
        .find(|ambient| ambient.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(ambient.active);
        snapshot.class_id = ambient.class;
        snapshot.model = ambient.model;
        snapshot.action = ambient.action;
        snapshot.phase = ambient.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX;
        snapshot.x = ambient.position.x;
        snapshot.y = ambient.position.y;
        snapshot.z = ambient.position.z;
        snapshot.yaw = ambient.yaw;
        snapshot.pitch = ambient.pitch;
        snapshot.actual_speed = ambient.actual_speed;
    }
    snapshot
}

pub(super) fn goat_snapshot(goat: &GoatState) -> McGoatSnapshotV2 {
    McGoatSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McGoatSnapshotV2>(),
        entity_index: goat.entity_index,
        target_entity_index: goat.target_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        render_parameter_index: goat.render_parameter_index,
        base_tmap: GOAT_BASE_TMAP,
        authority: MC_AUTHORITY_GOAT_LIVE,
        active: u8::from(goat.active),
        class_id: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        stage_var_1: goat.stage_var_1,
        stage_var_2: goat.stage_var_2,
        proximity_countdown: goat.proximity_countdown,
        proximity_subcounter: goat.proximity_subcounter,
        animation_frame: goat.animation_frame,
        reserved_0: 0,
        x: goat.position.x,
        y: goat.position.y,
        z: goat.position.z,
        yaw: goat.yaw,
        pitch: goat.pitch,
        roll: goat.roll,
        actual_speed: goat.actual_speed,
        minimum_speed: goat.minimum_speed,
        maximum_speed: goat.maximum_speed,
        rng: goat.rng,
        life: goat.life,
        max_life: goat.max_life,
        mana: goat.mana,
        pending_damage: goat.pending_damage,
        control_flags: goat.control_flags,
        reserved: [0; 4],
    }
}

pub(super) fn campaign_goat_snapshot(goat: &GoatState) -> McCampaignGoatSnapshotV2 {
    McCampaignGoatSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignGoatSnapshotV2>(),
        entity_index: goat.entity_index,
        target_entity_index: goat.target_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        render_parameter_index: goat.render_parameter_index,
        base_tmap: GOAT_BASE_TMAP,
        authority: MC_AUTHORITY_GOAT_LIVE,
        active: u8::from(goat.active),
        class_id: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        stage_var_1: goat.stage_var_1,
        stage_var_2: goat.stage_var_2,
        proximity_countdown: goat.proximity_countdown,
        proximity_subcounter: goat.proximity_subcounter,
        animation_frame: goat.animation_frame,
        reserved_0: 0,
        x: goat.position.x,
        y: goat.position.y,
        z: goat.position.z,
        yaw: goat.yaw,
        pitch: goat.pitch,
        roll: goat.roll,
        actual_speed: goat.actual_speed,
        minimum_speed: goat.minimum_speed,
        maximum_speed: goat.maximum_speed,
        rng: goat.rng,
        life: goat.life,
        max_life: goat.max_life,
        mana: goat.mana,
        pending_damage: goat.pending_damage,
        control_flags: goat.control_flags,
        partner_entity_index: goat.partner_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        reserved_1: 0,
        reserved_2: [0; 3],
    }
}

pub(super) fn campaign_spell_pickup_snapshot(
    pickup: &sim_core::native_world::Level2SpellPickupSource,
) -> McCampaignSpellPickupSnapshotV2 {
    McCampaignSpellPickupSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSpellPickupSnapshotV2>(),
        source_entity_index: pickup.source_entity_index,
        authored_index: pickup.authored_index,
        family: pickup.family,
        action: pickup.action,
        phase: pickup.phase,
        active: u8::from(pickup.active),
        x: pickup.position.x,
        y: pickup.position.y,
        z: pickup.position.z,
        entity_rng: pickup.entity_rng,
        reserved_0: 0,
        reserved: [0; 9],
    }
}

pub(super) fn level1_mission_snapshot(session: &SessionRecord) -> McLevel1MissionSnapshotV2 {
    let mission = &session.mission;
    let possessed_buildings = || {
        mission
            .possessed_buildings()
            .iter()
            .filter(|building| building.active && building.owner_entity_index != 0)
    };
    McLevel1MissionSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MissionSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        current_stage: mission.current_stage,
        text_selector: mission.current_text_selector,
        objective_textbox_ticks: mission.objective_textbox_ticks,
        tasks_completed: u8::from(mission.level_completed),
        player_dead: u8::from(mission.player_dead()),
        has_fireball: u8::from(mission.has_fireball),
        has_possession: u8::from(mission.has_possession),
        has_castle_spell: u8::from(mission.has_castle_spell),
        has_speed_spell: u8::from(mission.has_speed_spell),
        castle_exists: u8::from(mission.castle_exists),
        castle_level: mission.castle_level,
        fidelity_gap_flags: MC_LEVEL1_CURRENT_FIDELITY_GAPS,
        exit_sequence_active: u8::from(mission.exit_sequence_active),
        exit_sequence_complete: u8::from(mission.exit_sequence_complete),
        exit_result_code: mission.exit_result_code,
        player_life: mission.player_life,
        player_max_life: mission.player_max_life,
        player_mana: mission.player_mana,
        player_max_mana: mission.player_max_mana,
        castle_mana: mission.castle_mana,
        secured_mana_percent: mission.secured_mana_percent(),
        actor_count: u32::try_from(mission.actors().len()).expect("Level 1 actor count fits u32"),
        projectile_count: u32::try_from(mission.projectiles().len())
            .expect("Level 1 projectile count fits u32"),
        impact_count: u32::try_from(mission.impacts().len())
            .expect("Level 1 impact count fits u32"),
        exit_sequence_phase: u32::from(mission.exit_sequence_phase),
        exit_sequence_timer: i32::from(mission.exit_sequence_timer),
        possession_experience: mission.possession_experience,
        last_possessed_building_entity: possessed_buildings()
            .last()
            .map_or(0, |building| building.entity_index),
        last_possessed_building_owner: possessed_buildings()
            .last()
            .map_or(0, |building| building.owner_entity_index),
        possessed_building_count: u16::try_from(possessed_buildings().count())
            .expect("Level 1 possessed building count fits u16"),
        possession_tier: u8::try_from(mission.possession_tier)
            .expect("Level 1 possession tier is non-negative"),
        speed_trail_count: u8::try_from(mission.speed_trails().len()).unwrap_or(u8::MAX),
    }
}

pub(super) fn level1_player_lifecycle_snapshot(
    session: &SessionRecord,
) -> McLevel1PlayerLifecycleSnapshotV2 {
    let mission = &session.mission;
    let marker = mission.player_death_marker();
    let player_entity_index = session.state.entity_index;
    let player_control_flags = mission.player_source_control_flags(player_entity_index);
    McLevel1PlayerLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        lifecycle_hash: mission.player_lifecycle_hash_v2(&session.state, &session.context),
        source_arena_hash: mission.source_arena_hash_v1(),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.player_action,
        player_dead: u8::from(mission.player_dead()),
        input_suppressed: u8::from(
            mission.player_action != sim_core::level1_mission::PLAYER_ACTION_ACTIVE,
        ),
        player_active: u8::from(mission.player_source_active(player_entity_index)),
        player_visible: u8::from(player_control_flags & 0x20 == 0),
        player_targetable: u8::from(
            mission.player_source_active(player_entity_index) && mission.player_life >= 0,
        ),
        menu_close_requested: u8::from(mission.player_menu_close_requested),
        notification_active: u8::from(mission.player_notification_active),
        notification_state: mission.player_notification_state,
        banished_camera_active: u8::from(mission.player_banished_camera_active),
        marker_present: u8::from(marker.is_some_and(|marker| marker.active)),
        fall_velocity: mission.player_fall_velocity,
        notification_timer: mission.player_notification_timer,
        action_timer: mission.player_action_timer,
        replaceable_cursor: mission.source_pool_replaceable_cursor(),
        killer_entity_index: mission.player_killer_entity_index(),
        killer_score_awards: mission.player_score_for(
            mission.player_killer_entity_index(),
            session.state.player_color_index,
        ),
        notification_id: mission.player_notification_id,
        player_entity_rng: mission.player_source_entity_rng(player_entity_index),
        player_control_flags,
        player_entity_roll: mission.player_entity_roll,
        player_yaw: session.state.yaw,
        controller_pitch: session.state.pitch_accumulator,
        controller_roll: session.state.roll_accumulator,
        recoil_speed: session.state.recoil_speed,
        marker_source_entity_index: marker.map_or(0, |marker| marker.source_entity_index),
        marker_x: marker.map_or(0, |marker| marker.position.x),
        marker_y: marker.map_or(0, |marker| marker.position.y),
        marker_z: marker.map_or(0, |marker| marker.position.z),
        marker_trigger_entity_index: marker.map_or(0, |marker| marker.trigger_player_entity_index),
        marker_particle_parameter_index: marker.map_or(0, |marker| marker.particle_parameter_index),
        marker_life: marker.map_or(0, |marker| marker.life),
        marker_max_life: marker.map_or(0, |marker| marker.max_life),
        source_pool_rebuild_count: mission.source_pool_rebuild_count(),
        lifecycle_trace_hash: mission.player_lifecycle_trace_hash(),
        lifecycle_trace_count: u16::try_from(mission.player_lifecycle_trace().len())
            .unwrap_or(u16::MAX),
        marker_dispatch_phase: marker.map_or(0, |marker| marker.dispatch_phase),
        marker_class: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_CLASS),
        marker_model: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_MODEL),
        marker_action: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_ACTION),
        marker_byte_38: marker.map_or(0, |marker| marker.byte_38),
        source_color_phase_6: u8::from(mission.source_color_phase_6()),
        source_turn_counter: mission.source_turn_counter(),
        menu_state: mission.player_menu_state,
        killer_score_victim_color: session.state.player_color_index.min(7),
        effective_pitch: session.state.effective_pitch,
        marker_constructor_phase: marker.map_or(0, |marker| marker.constructor_phase),
        reserved: [0; 4],
    }
}

pub(super) fn level1_hud_spell_slot(
    session: &SessionRecord,
    holder_index: u16,
) -> McLevel1HudSpellSlotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1HudSpellSlotV2::default();
    };
    let Some(holder) = fire
        .holders
        .iter()
        .find(|holder| holder.entity_index == holder_index)
    else {
        return McLevel1HudSpellSlotV2::default();
    };
    let enabled_flags = if matches!(holder.model, 3 | 4 | 6 | 8 | 11 | 12 | 14) {
        4
    } else {
        0
    };
    McLevel1HudSpellSlotV2 {
        holder_entity_index: holder.entity_index,
        parent_entity_index: fire.caster.entity_index,
        model: holder.model,
        // DrawSpellIcon_2E260 reads the shared holder level gate, not the
        // selected tier stored independently for each hand.
        tier: holder.level_gate,
        enabled_flags,
        action_marker: holder.action_marker,
        active_value: holder.active_value,
        activation_value: holder.activation_value,
        max_mana: holder.mana_cost,
        // All four holders reachable in Level 1 have zero source manaRegen.
        mana_regen: 0,
        caster_mana: fire.caster.mana,
        frame_selector: if holder.active_value == 0 { 1 } else { 2 },
        icon_selector: 123 + u16::from(holder.model),
        reserved: [0; 2],
    }
}

pub(super) fn level1_hud_snapshot(session: &SessionRecord) -> McLevel1HudSnapshotV2 {
    let mission = &session.mission;
    let fire = session.context.spell_fire.as_ref();
    let left_index = fire.map_or(0, |fire| fire.left_holder_index);
    let right_index = fire.map_or(0, |fire| fire.right_holder_index);
    let selected_index = fire.map_or(0, |fire| fire.selected_holder_index);
    let selected = level1_hud_spell_slot(session, selected_index);
    let player_color = usize::from(session.state.player_color_index.min(7));
    const NIGHT_COLORS: [[u8; 2]; 8] = [
        [0xA4, 0xAA],
        [0x77, 0x7D],
        [0xC0, 0xC6],
        [0x58, 0x5D],
        [0x97, 0x9D],
        [0xD7, 0xDD],
        [0x69, 0x62],
        [0xC9, 0xCF],
    ];
    McLevel1HudSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1HudSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        mana_color_index: NIGHT_COLORS[player_color][0],
        mana_max_color_index: NIGHT_COLORS[player_color][1],
        life_color_index: 0x7B,
        color_phase_1: u8::from((session.state.tick & 1) != 0),
        color_phase_2: u8::from(((session.state.tick / 2) & 1) != 0),
        player_hit_frame_ticks: mission.player_hit_frame_ticks(),
        castle_hit_frame_ticks: mission.castle_hit_frame_ticks(),
        global_mana_denominator: mission.global_mana_denominator,
        secured_mana_component: mission.secured_mana_component,
        goal_percent: mission.secured_mana_percent(),
        player_status_flags: session
            .context
            .spell_fire
            .as_ref()
            .map_or(0, |fire| fire.caster.control_flags),
        left: level1_hud_spell_slot(session, left_index),
        right: level1_hud_spell_slot(session, right_index),
        selected_holder_entity_index: selected.holder_entity_index,
        selected_icon_selector: selected.icon_selector,
        selected_model: selected.model,
        selected_tier: selected.tier,
        selected_enabled_flags: selected.enabled_flags,
        selected_frame_selector: u8::try_from(selected.frame_selector)
            .expect("spell frame selector is 1 or 2"),
    }
}

pub(super) fn level1_spell_menu_snapshot(session: &SessionRecord) -> McLevel1SpellMenuSnapshotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1SpellMenuSnapshotV2::default();
    };
    let holder_model = |holder_index| {
        fire.holders
            .iter()
            .find(|holder| holder.entity_index == holder_index)
            .map_or(0, |holder| holder.model)
    };
    let mut family_enabled = [0_u8; 4];
    let mut family_unlocked_tier = [0_i8; 4];
    let mut family_selected_tier = [0_i8; 4];
    let mut holder_entity_index = [0_u16; 4];
    for family in 0..4 {
        let holder_index = fire.enabled_holder_indices[family];
        holder_entity_index[family] = holder_index;
        family_enabled[family] = u8::from(holder_index != 0);
        family_unlocked_tier[family] = fire.spell_levels[family].clamp(0, 2);
        family_selected_tier[family] = fire.family_selected_tiers[family];
    }
    McLevel1SpellMenuSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1SpellMenuSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        selected_ui_index: fire.selected_spell_ui_index,
        selected_tier: fire.selected_subspell_index,
        left_family: holder_model(fire.left_holder_index),
        left_tier: fire.left_subspell_index,
        right_family: holder_model(fire.right_holder_index),
        right_tier: fire.right_subspell_index,
        reserved_0: 0,
        family_enabled,
        family_unlocked_tier,
        family_selected_tier,
        family_hand_eligibility: std::array::from_fn(|family| fire.spell_hand_eligibility[family]),
        holder_entity_index,
        reserved: [0; 2],
    }
}

pub(super) fn level1_minimap_marks(session: &SessionRecord) -> McLevel1MinimapMarksV2 {
    let Level1MinimapMarks {
        mark_count,
        mark_pulse,
        arrow_valid,
        arrow_visible,
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
    } = session.mission.minimap_marks(&session.state);
    McLevel1MinimapMarksV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MinimapMarksV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mark_count,
        mark_pulse: u8::from(mark_pulse),
        arrow_valid: u8::from(arrow_valid),
        arrow_visible: u8::from(arrow_visible),
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
        reserved: [0; 4],
    }
}

pub(super) fn level1_exit_presentation(session: &SessionRecord) -> McLevel1ExitPresentationV2 {
    let mission = &session.mission;
    McLevel1ExitPresentationV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ExitPresentationV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.exit_player_action,
        action_phase: mission.exit_sequence_phase,
        palette_submode: mission.exit_palette_submode,
        palette_effect: mission.exit_palette_effect,
        palette_weight: mission.exit_palette_weight,
        palette_denominator: mission.exit_palette_denominator,
        movement_resolution: mission.exit_last_movement_resolution,
        target_source_entity_index: mission.exit_target_source_entity_index,
        target_valid: u8::from(mission.exit_target_valid),
        input_suppressed: u8::from(mission.exit_sequence_owns_player_action()),
        camera_fov: session.camera_fov,
        action_timer: mission.exit_sequence_timer,
        player_status_flags: mission.exit_player_status_flags,
        blur_mode: mission.exit_blur_mode,
        display_blank: u8::from(mission.exit_display_blank),
        result_code: mission.exit_result_code,
        active: u8::from(mission.exit_sequence_active),
        complete: u8::from(mission.exit_sequence_complete),
        flags: MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
            | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
            | if mission.exit_end_sequence_flag {
                MC_LEVEL1_EXIT_FLAG_END_SEQUENCE
            } else {
                0
            },
        reserved_0: 0,
        reserved: [0; 4],
    }
}

pub(super) fn record_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

pub(super) fn level1_minimap_entity_inputs(
    session: &SessionRecord,
) -> [Level1MinimapEntityInput; 1_000] {
    const SOURCE_EXTENT: usize = 1_000;
    let local_player = session.state.entity_index;
    let mut inputs = [Level1MinimapEntityInput::default(); SOURCE_EXTENT];
    let relation_is_player = |entity_index: u16| {
        session
            .world
            .entities()
            .get(usize::from(entity_index))
            .is_some_and(|entity| entity.active && entity.class == 3)
    };

    for source_slot in 1..SOURCE_EXTENT {
        let slot = session.mission.source_slots()[source_slot];
        let reset = &session.world.entities()[source_slot];
        let bytes = reset.record_bytes();
        let source_id = reset.source_id;
        let player_link = record_u16(bytes, 0x94);
        inputs[source_slot] = Level1MinimapEntityInput {
            source_slot: source_slot as u16,
            class: slot.class,
            model: slot.model,
            action: slot.action,
            phase: slot.phase,
            control_flags: slot.control_flags,
            source_id,
            source_id_is_player: relation_is_player(source_id),
            source_id_is_local_player: source_id == local_player,
            player_link,
            player_link_is_player: relation_is_player(player_link),
            player_link_is_local_player: player_link == local_player,
            parent_id: record_u16(bytes, 0x28),
            stage_var_2: bytes[0x49] as i8,
            word_32: record_u16(bytes, 0x32),
            position: reset.position,
            active: slot.active,
        };
    }

    let mut resolve = |source_slot: u16,
                       position: Position,
                       source_id: u16,
                       player_link: u16,
                       parent_id: u16,
                       stage_var_2: i8| {
        if source_slot == 0 || usize::from(source_slot) >= SOURCE_EXTENT {
            return;
        }
        let input = &mut inputs[usize::from(source_slot)];
        input.position = position;
        input.source_id = source_id;
        input.source_id_is_player = relation_is_player(source_id);
        input.source_id_is_local_player = source_id == local_player;
        input.player_link = player_link;
        input.player_link_is_player = relation_is_player(player_link);
        input.player_link_is_local_player = player_link == local_player;
        input.parent_id = parent_id;
        input.stage_var_2 = stage_var_2;
    };

    resolve(
        local_player,
        session.state.position,
        local_player,
        local_player,
        0,
        0,
    );
    for goat in session.live_world.goats() {
        resolve(
            goat.entity_index,
            goat.position,
            goat.source_id,
            0,
            0,
            goat.stage_var_2,
        );
    }
    for actor in session.mission.actors() {
        resolve(
            actor.source_entity_index,
            actor.position,
            actor.owner_id.cast_unsigned(),
            0,
            0,
            actor.stage_var_2,
        );
    }
    for segment in session.mission.multipart_segments() {
        resolve(
            segment.source_entity_index,
            segment.position,
            segment.owner_id.cast_unsigned(),
            0,
            segment.head_source_entity_index,
            0,
        );
    }
    for projectile in session.mission.projectiles() {
        match projectile {
            Level1Projectile::PlayerFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                0,
                0,
                0,
            ),
            Level1Projectile::ArcherArrow(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::FireflyFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::CastleBuild(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
            Level1Projectile::Possession(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
        }
    }
    for impact in session.mission.impacts() {
        resolve(
            impact.source_entity_index,
            impact.position,
            impact.owner_entity_index,
            impact.external_target_entity_index,
            0,
            0,
        );
    }
    for burst in session.mission.death_bursts() {
        resolve(
            burst.source_entity_index,
            burst.position,
            burst.parent_entity_index,
            0,
            burst.parent_entity_index,
            0,
        );
    }
    for sphere in session.mission.mana_spheres() {
        resolve(
            sphere.source_entity_index,
            sphere.position,
            sphere.source_entity_index,
            sphere.owner_entity_index,
            0,
            0,
        );
    }
    if let Some(carrier) = session.mission.mana_carrier() {
        resolve(
            carrier.source_entity_index,
            carrier.position,
            carrier.owner_entity_index,
            carrier.owner_entity_index,
            0,
            0,
        );
    }
    if session.mission.castle_exists {
        resolve(
            session.mission.castle_source_entity_index,
            session.mission.castle_position,
            session.mission.castle_owner_entity_index,
            session.mission.castle_owner_entity_index,
            0,
            0,
        );
    }
    if let Some(construction) = session.mission.castle_construction() {
        if construction.active {
            resolve(
                construction.source_entity_index,
                construction.position,
                construction.parent_entity_index,
                construction.owner_entity_index,
                construction.parent_entity_index,
                0,
            );
        }
    }
    inputs
}

pub(super) fn level1_minimap_entity_commands(
    session: &SessionRecord,
) -> Vec<McLevel1MinimapEntityCommandV2> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        for (order, (x, y)) in
            level1_minimap_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .enumerate()
        {
            commands.push(McLevel1MinimapEntityCommandV2 {
                source_slot: session.mission.castle_source_entity_index,
                x,
                y,
                kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                palette_index: 176,
                anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                order: u16::try_from(order).expect("castle rope command order fits u16"),
                ..McLevel1MinimapEntityCommandV2::default()
            });
        }
    }
    for input in inputs.into_iter().skip(1) {
        let Some(command) = level1_minimap_entity_command(&session.state, input) else {
            continue;
        };
        commands.push(McLevel1MinimapEntityCommandV2 {
            source_slot: command.source_slot,
            x: command.x,
            y: command.y,
            selector: command.selector,
            kind: command.kind,
            palette_index: command.palette_index,
            size: command.size,
            anchor: command.anchor,
            help_selector: command.help_selector,
            flags: 0x07,
            order: u16::try_from(commands.len()).expect("minimap command order fits u16"),
            reserved: [0; 2],
        });
    }
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 minimap command extent"
    );
    commands
}

pub(super) fn level1_map_entity_commands(
    session: &SessionRecord,
) -> Vec<Level1MinimapEntityCommand> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        commands.extend(
            level1_map_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .map(|(x, y)| Level1MinimapEntityCommand {
                    source_slot: session.mission.castle_source_entity_index,
                    x,
                    y,
                    selector: 0,
                    kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                    palette_index: 176,
                    size: 1,
                    anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                    help_selector: 0,
                }),
        );
    }
    commands.extend(
        inputs
            .into_iter()
            .skip(1)
            .filter_map(|input| level1_map_entity_command(&session.state, input)),
    );
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 map entity command extent"
    );
    commands
}

pub(super) fn level1_minimap_command_hash(commands: &[McLevel1MinimapEntityCommandV2]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut append = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    append(b"mc2-level1-minimap-entities-v1\0");
    append(&(commands.len() as u32).to_le_bytes());
    for command in commands {
        append(&command.source_slot.to_le_bytes());
        append(&command.x.to_le_bytes());
        append(&command.y.to_le_bytes());
        append(&command.selector.to_le_bytes());
        append(&[
            command.kind,
            command.palette_index,
            command.size,
            command.anchor,
            command.help_selector,
            command.flags,
        ]);
        append(&command.order.to_le_bytes());
    }
    hash
}

pub(super) fn level1_castle_snapshot(session: &SessionRecord) -> McLevel1CastleSnapshotV2 {
    let mission = &session.mission;
    let carrier = mission.mana_carrier();
    McLevel1CastleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(mission.castle_exists),
        level: mission.castle_level,
        class_id: CASTLE_ROOT_CLASS,
        model: CASTLE_ROOT_MODEL,
        action: CASTLE_ROOT_ACTION,
        // TMAP 96 header `03 00` supplies the runtime mode.
        render_mode: 0,
        reserved_0: 0,
        owner_entity_index: carrier.map_or(0, |carrier| carrier.owner_entity_index),
        render_parameter_index: CASTLE_ROOT_RENDER_INDEX,
        x: mission.castle_position.x,
        y: mission.castle_position.y,
        z: mission.castle_position.z,
        base_tmap: CASTLE_ROOT_BASE_TMAP,
        logical_height: CASTLE_ROOT_LOGICAL_HEIGHT,
        reserved_1: 0,
        life: mission.castle_life,
        max_life: mission.castle_max_life,
        mana: mission.castle_mana,
        mana_capacity: mission.castle_mana_capacity,
        carrier_id: carrier.map_or(0, |carrier| carrier.id),
        reserved: [
            u32::from(mission.castle_source_entity_index),
            mission.castle_presentation_id,
            0,
        ],
    }
}

pub(super) fn terrain_state(session: &SessionRecord) -> McTerrainStateV2 {
    McTerrainStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McTerrainStateV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mutation_sequence: session.terrain.mutation_sequence(),
        terrain_hash: session.terrain.deterministic_hash(),
        width: 256,
        height: 256,
        plane_count: 6,
        hash_schema: 1,
        authority: MC_AUTHORITY_SESSION_TERRAIN_LIVE,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

pub(super) fn level1_castle_lifecycle_snapshot(
    session: &SessionRecord,
) -> McLevel1CastleLifecycleSnapshotV2 {
    let mission = &session.mission;
    let construction = mission.castle_construction();
    McLevel1CastleLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        root_presentation_id: mission.castle_presentation_id,
        root_source_entity_index: mission.castle_source_entity_index,
        root_owner_entity_index: mission.castle_owner_entity_index,
        root_x: mission.castle_position.x,
        root_y: mission.castle_position.y,
        root_z: mission.castle_position.z,
        root_build_state: mission.castle_build_state,
        root_action: mission.castle_action,
        root_level: mission.castle_level,
        root_active: u8::from(mission.castle_exists),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        root_life: mission.castle_life,
        construction_id: construction.map_or(0, |child| child.id),
        construction_source_entity_index: construction.map_or(0, |child| child.source_entity_index),
        construction_parent_entity_index: construction.map_or(0, |child| child.parent_entity_index),
        construction_owner_entity_index: construction.map_or(0, |child| child.owner_entity_index),
        construction_x: construction.map_or(0, |child| child.position.x),
        construction_y: construction.map_or(0, |child| child.position.y),
        construction_z: construction.map_or(0, |child| child.position.z),
        construction_timer: construction.map_or(0, |child| child.timer),
        construction_stage: construction.map_or(0, |child| child.stage),
        construction_present: u8::from(construction.is_some()),
        construction_active: u8::from(construction.is_some_and(|child| child.active)),
        construction_initialized: u8::from(construction.is_some_and(|child| child.initialized)),
        reserved_0: [0; 2],
        terrain_mutation_sequence: session.terrain.mutation_sequence(),
    }
}

pub(super) fn level1_house_completion_effects(
    session: &SessionRecord,
) -> Vec<McLevel1HouseCompletionEffectV2> {
    let completion_effects = session
        .mission
        .house_construction_effects()
        .iter()
        .copied()
        .filter(|effect| {
            matches!(
                effect,
                HouseConstructionSourceEffect::ConstructionAllocated { .. }
                    | HouseConstructionSourceEffect::ConstructionAllocationFailed { .. }
            )
        })
        .collect::<Vec<_>>();
    let effect_count =
        u32::try_from(completion_effects.len()).expect("completion effect count fits u32");
    completion_effects
        .into_iter()
        .enumerate()
        .map(|(ordinal, effect)| {
            let (kind, house_entity_index, parent_entity_index, source_entity_index, selector) =
                match effect {
                    HouseConstructionSourceEffect::ConstructionAllocated {
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED,
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    ),
                    HouseConstructionSourceEffect::ConstructionAllocationFailed {
                        house_entity_index,
                        parent_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED,
                        house_entity_index,
                        parent_entity_index,
                        0,
                        building_index,
                    ),
                    _ => unreachable!("filtered completion effect"),
                };
            let construction = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .house_completion_constructions()
                        .iter()
                        .find(|construction| {
                            construction.source_entity_index == source_entity_index
                        })
                })
                .flatten();
            let source = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .source_slots()
                        .get(usize::from(source_entity_index))
                })
                .flatten();
            let parent_word_2e = session
                .mission
                .raw_house_construction_state(parent_entity_index)
                .map_or(0, |state| state.4);
            let parent_construction_selector = session
                .mission
                .raw_source_construction_selector(parent_entity_index)
                .unwrap_or_default();
            let house_building_index = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .map(|house| house.building_index)
                .or_else(|| {
                    session
                        .mission
                        .authored_houses()
                        .find(|house| house.entity_index == house_entity_index)
                        .map(|house| house.building_index)
                })
                .unwrap_or_default();
            McLevel1HouseCompletionEffectV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1HouseCompletionEffectV2>(),
                generation: session.generation,
                tick: session.state.tick,
                kind,
                house_entity_index,
                parent_entity_index,
                source_entity_index,
                source_id: construction.map_or(0, |value| value.owner_entity_index),
                class_id: source.map_or(0, |value| value.class),
                model: source.map_or(0, |value| value.model),
                action: source.map_or(0, |value| value.action),
                construction_selector: selector,
                house_building_index,
                constructor_latch: construction.map_or(0, |value| value.constructor_latch),
                active: u8::from(construction.is_some_and(|value| value.active)),
                phase: source.map_or(0, |value| value.phase),
                parent_word_2e,
                parent_construction_selector,
                x: construction.map_or(0, |value| value.position.x),
                y: construction.map_or(0, |value| value.position.y),
                z: construction.map_or(0, |value| value.position.z),
                collision_half_width: construction.map_or(0, |value| value.collision_half_width),
                collision_half_depth: construction.map_or(0, |value| value.collision_half_depth),
                yaw: construction.map_or(0, |value| value.yaw),
                fov: construction.map_or(0, |value| value.fov),
                effect_ordinal: u16::try_from(ordinal).expect("completion effect ordinal fits u16"),
                max_life: construction.map_or(0, |value| value.max_life),
                life: construction.map_or(0, |value| value.life),
                control_flags: construction.map_or(0, |value| value.control_flags),
                effect_count,
            }
        })
        .collect()
}

pub(super) fn mana_sphere_snapshot(sphere: &ManaSphere) -> McManaSphereSnapshotV2 {
    let size = usize::from(mana_sphere_size_ordinal(sphere.mana));
    let render_base = if sphere.owner_entity_index == 0 {
        MANA_SPHERE_NEUTRAL_RENDER_BASE
    } else {
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE
    };
    McManaSphereSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaSphereSnapshotV2>(),
        sphere_id: sphere.id,
        source_entity_index: sphere.source_entity_index,
        owner_entity_index: sphere.owner_entity_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(sphere.active && !sphere.pending_delete),
        class_id: MANA_SPHERE_CLASS,
        model: MANA_SPHERE_MODEL,
        action: MANA_SPHERE_ACTION,
        phase: sphere.phase,
        // TMAPs 58/67 both carry runtime mode 21.
        render_mode: 21,
        reserved_0: 0,
        x: sphere.position.x,
        y: sphere.position.y,
        z: sphere.position.z,
        yaw: sphere.yaw,
        velocity_x: sphere.velocity_x,
        velocity_y: sphere.velocity_y,
        vertical_velocity: sphere.vertical_velocity,
        render_parameter_index: render_base
            + u16::try_from(size).expect("eight mana sphere sizes fit u16"),
        base_tmap: if sphere.owner_entity_index == 0 {
            MANA_SPHERE_NEUTRAL_BASE_TMAP
        } else {
            MANA_SPHERE_PLAYER_ONE_BASE_TMAP
        },
        logical_height: u16::try_from((size + 1) * 50)
            .expect("mana sphere logical height fits u16"),
        mana: sphere.mana,
        assigned_carrier_id: sphere.assigned_carrier_id,
        // Additive Level 2 mailbox projection. The public struct remains the
        // sealed 64-byte record; reserved words carry the source attraction
        // relation and the two source radial words.
        reserved: [
            u32::from(sphere.attraction_source_entity_index),
            u32::from(sphere.attraction_step) | (u32::from(sphere.attraction_distance_high) << 16),
            0,
        ],
    }
}

pub(super) fn mana_carrier_snapshot(session: &SessionRecord) -> McManaCarrierSnapshotV2 {
    let carrier = session
        .mission
        .mana_carrier()
        .copied()
        .unwrap_or(ManaCarrier {
            id: 0,
            source_entity_index: 0,
            entity_rng: 0,
            owner_entity_index: 0,
            position: Position::default(),
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: 0,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: false,
        });
    mana_carrier_snapshot_value(&carrier, session.generation, session.state.tick)
}

pub(super) fn mana_carrier_snapshot_value(
    carrier: &ManaCarrier,
    generation: u64,
    tick: u64,
) -> McManaCarrierSnapshotV2 {
    McManaCarrierSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaCarrierSnapshotV2>(),
        generation,
        tick,
        carrier_id: carrier.id,
        target_sphere_id: carrier.target_sphere_id,
        owner_entity_index: carrier.owner_entity_index,
        render_parameter_index: MANA_CARRIER_RENDER_INDEX,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(carrier.active),
        class_id: MANA_CARRIER_CLASS,
        model: MANA_CARRIER_MODEL,
        action: MANA_CARRIER_ACTION,
        returning_to_castle: u8::from(carrier.returning_to_castle),
        // TMAP 88 header `02 00` supplies the runtime mode. This family is
        // fixed-facing; it does not use Firefly's mode-17 yaw selection.
        render_mode: 0,
        reserved_0: 0,
        x: carrier.position.x,
        y: carrier.position.y,
        z: carrier.position.z,
        yaw: carrier.yaw,
        speed: carrier.speed,
        base_tmap: MANA_CARRIER_BASE_TMAP,
        logical_height: MANA_CARRIER_LOGICAL_HEIGHT,
        reserved_1: 0,
        carried_mana: carrier.carried_mana,
        // Carrier identity and every carrier/sphere link are source slots.
        // Keep the redundant reserved projection until the next ABI version
        // so existing desktop clients can enforce exact equality.
        reserved: [u32::from(carrier.source_entity_index), 0, 0],
    }
}

pub(super) fn level1_actor_snapshot(actor: &Level1Actor) -> McLevel1ActorSnapshotV2 {
    McLevel1ActorSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ActorSnapshotV2>(),
        authored_index: actor.authored_index,
        activation_group: actor.activation_group,
        kind: actor.kind as u8,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(actor.active),
        collected: u8::from(actor.collected),
        class_id: actor.class,
        model: actor.model,
        action: actor.action,
        render_mode: actor.render_mode.unwrap_or(0),
        x: actor.position.x,
        y: actor.position.y,
        z: actor.position.z,
        render_parameter_index: actor.render_parameter_index.unwrap_or(u16::MAX),
        max_life: actor.max_life,
        life: actor.life,
        pending_damage: actor.pending_damage,
        base_tmap: actor.base_tmap.unwrap_or(u16::MAX),
        logical_height: actor.logical_height.unwrap_or(0),
        yaw: actor.yaw,
        animation_frame: actor.animation_frame,
        visible: u8::from(actor.visible),
        state_value: actor.state_value,
        reserved: 0,
    }
}

pub(super) fn level1_multipart_segment_snapshot(
    segment: &Level1MultipartSegment,
) -> McLevel1MultipartSegmentSnapshotV2 {
    McLevel1MultipartSegmentSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MultipartSegmentSnapshotV2>(),
        source_entity_index: segment.source_entity_index,
        head_authored_index: segment.head_authored_index,
        head_source_entity_index: segment.head_source_entity_index,
        predecessor_source_entity_index: segment.predecessor_source_entity_index,
        successor_source_entity_index: segment.successor_source_entity_index,
        ordinal: segment.ordinal,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(segment.active),
        visible: u8::from(segment.visible),
        class_id: segment.class,
        model: segment.model,
        action: segment.action,
        render_mode: segment.render_mode,
        x: segment.position.x,
        y: segment.position.y,
        z: segment.position.z,
        yaw: segment.yaw,
        pitch: segment.pitch,
        phase: segment.phase,
        reserved_0: 0,
        owner_id: segment.owner_id,
        life: segment.life,
        pending_damage: segment.pending_damage,
        render_parameter_index: segment.render_parameter_index,
        base_tmap: segment.base_tmap,
        logical_height: segment.logical_height,
        collision_half_width: segment.collision_half_width,
        collision_half_height: segment.collision_half_height,
        trailing_distance: segment.trailing_distance,
        reserved: 0,
    }
}

pub(super) fn fireball_projectile_snapshot(
    projectile: &Level1Projectile,
) -> McFireballProjectileSnapshotV2 {
    let (
        projectile_kind,
        id,
        source_entity_index,
        owner_entity_index,
        render_index,
        class_id,
        model,
        position,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        active,
    ) = match projectile {
        Level1Projectile::PlayerFireball(projectile) => (
            1,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::ArcherArrow(projectile) => (
            2,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            ARCHER_ARROW_RENDER_INDEX,
            ARCHER_ARROW_PROJECTILE_CLASS,
            ARCHER_ARROW_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            ARCHER_ARROW_BASE_TMAP,
            projectile.payload,
            ARCHER_ARROW_LOGICAL_HEIGHT,
            ARCHER_ARROW_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::FireflyFireball(projectile) => (
            3,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::CastleBuild(projectile) => (
            4,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            CASTLE_PROJECTILE_RENDER_INDEX,
            CASTLE_PROJECTILE_CLASS,
            CASTLE_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            CASTLE_PROJECTILE_BASE_TMAP,
            0,
            CASTLE_PROJECTILE_LOGICAL_HEIGHT,
            CASTLE_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::Possession(projectile) => (
            5,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            POSSESSION_PROJECTILE_RENDER_INDEX,
            POSSESSION_PROJECTILE_CLASS,
            POSSESSION_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            POSSESSION_PROJECTILE_BASE_TMAP,
            0,
            POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
            POSSESSION_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
    };
    McFireballProjectileSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballProjectileSnapshotV2>(),
        projectile_id: id,
        owner_entity_index,
        render_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(active),
        class_id,
        model,
        x: position.x,
        y: position.y,
        z: position.z,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        animation_frame: 0,
        projectile_kind,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `projectile_id` remains a presentation-stable identifier.
        reserved: [u32::from(source_entity_index), 0],
    }
}

pub(super) fn fireball_impact_snapshot(impact: &FireballImpact) -> McFireballImpactSnapshotV2 {
    let (base_tmap, render_parameter_index, render_mode, logical_height) =
        if impact.model == WATER_SPLASH_MODEL {
            (
                WATER_SPLASH_BASE_TMAP,
                WATER_SPLASH_RENDER_INDEX,
                WATER_SPLASH_RENDER_MODE,
                WATER_SPLASH_LOGICAL_HEIGHT,
            )
        } else if impact.model == GROUND_FIRE_MODEL {
            let (render_parameter_index, logical_height) = ground_fire_presentation(impact.phase);
            (
                GROUND_FIRE_BASE_TMAP,
                render_parameter_index,
                GROUND_FIRE_RENDER_MODE,
                logical_height,
            )
        } else if impact.model == POSSESSION_ATTRACTION_MODEL {
            (
                POSSESSION_ATTRACTION_BASE_TMAP,
                POSSESSION_ATTRACTION_RENDER_INDEX,
                POSSESSION_ATTRACTION_RENDER_MODE,
                POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            )
        } else if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            (
                POSSESSION_IMPACT_BASE_TMAP,
                POSSESSION_IMPACT_RENDER_INDEX,
                POSSESSION_IMPACT_RENDER_MODE,
                POSSESSION_IMPACT_LOGICAL_HEIGHT,
            )
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            let render_parameter_index = u16::from(impact.animation_frame);
            (
                CHECKPOINT_SMOKE_BASE_TMAP,
                render_parameter_index,
                CHECKPOINT_SMOKE_RENDER_MODE,
                render_parameter_index
                    .saturating_sub(CHECKPOINT_SMOKE_FIRST_RENDER_INDEX)
                    .saturating_add(1)
                    * 50,
            )
        } else {
            (
                FIREBALL_IMPACT_BASE_TMAP,
                FIREBALL_IMPACT_RENDER_INDEX,
                FIREBALL_IMPACT_RENDER_MODE,
                FIREBALL_IMPACT_LOGICAL_HEIGHT,
            )
        };
    McFireballImpactSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballImpactSnapshotV2>(),
        impact_id: impact.id,
        source_projectile_id: impact.source_projectile_id,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(impact.active),
        damage_submitted: u8::from(impact.damage_submitted),
        class_id: if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            POSSESSION_IMPACT_CLASS
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            CHECKPOINT_SMOKE_CLASS
        } else {
            FIREBALL_IMPACT_CLASS
        },
        model: impact.model,
        render_mode,
        animation_frame: i8::try_from(impact.animation_frame).unwrap_or(i8::MAX),
        reserved_0: 0,
        x: impact.position.x,
        y: impact.position.y,
        z: impact.position.z,
        life_ticks: impact.life_ticks,
        payload: impact.payload,
        base_tmap,
        render_parameter_index,
        logical_height,
        reserved_1: 0,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `impact_id` remains a presentation-stable identifier.
        reserved: [u32::from(impact.source_entity_index), 0],
    }
}

pub(super) fn validate_mission_snapshot_cursor(
    session: &SessionRecord,
    generation: u64,
    expected_tick: u64,
) -> AbiResult<()> {
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    if expected_tick != session.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission tick {expected_tick} does not match {}",
                session.state.tick
            ),
        ));
    }
    Ok(())
}

pub(super) fn copy_mission_slice<T, U>(
    source: &[T],
    start_ordinal: u32,
    buffer: *mut U,
    capacity: u32,
    out_written: *mut u32,
    convert: impl Fn(&T) -> U,
    label: &str,
) -> AbiResult<()> {
    let start = usize::try_from(start_ordinal)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} start exceeds usize")))?;
    if start > source.len() {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("{label} start is beyond count"),
        ));
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} capacity exceeds usize")))?;
    let count = (source.len() - start).min(capacity);
    if count > 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{label} output buffer is null"),
        ));
    }
    for (offset, item) in source.iter().skip(start).take(count).enumerate() {
        // SAFETY: the caller supplied exact element stride and writable
        // capacity for `count` records.
        unsafe { buffer.add(offset).write_unaligned(convert(item)) };
    }
    write_output(
        out_written,
        u32::try_from(count).expect("mission slice count fits u32"),
    );
    Ok(())
}

pub(super) fn objective_snapshot(
    slot: u8,
    objective: &sim_core::native_world::RuntimeObjective,
) -> McObjectiveSnapshotV2 {
    McObjectiveSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McObjectiveSnapshotV2>(),
        slot,
        is_level_end: objective.is_level_end,
        text_selector: objective.text_selector,
        done: objective.done,
        authority: MC_AUTHORITY_FROZEN_OBSERVED,
        stage_selectors: objective.stage_selectors,
        reserved_0: [0; 3],
        reserved: [0; 4],
    }
}

pub(super) fn campaign_objective_stage_snapshot(
    generation: u64,
    objective_slot: u8,
    stage_slot: u8,
    objective: sim_core::native_world::RuntimeObjective,
    stage: &sim_core::native_world::RuntimeStage,
    stage_state: u8,
) -> McCampaignObjectiveStageSnapshotV2 {
    let (relation_domain, relation_value) = match stage.relation {
        sim_core::native_world::RuntimeRelation::Scalar(value) => (
            0,
            u16::try_from(value).expect("objective scalar relation fits u16"),
        ),
        sim_core::native_world::RuntimeRelation::AuthoredFeature(index) => (1, index),
        sim_core::native_world::RuntimeRelation::LiveEntity(index) => (2, index),
        sim_core::native_world::RuntimeRelation::LiveEntityIndex(index) => (3, index),
        sim_core::native_world::RuntimeRelation::AxisScalar(value) => (
            0,
            u16::try_from(value).expect("objective axis relation fits u16"),
        ),
        sim_core::native_world::RuntimeRelation::Null => (0, 0),
    };
    McCampaignObjectiveStageSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        generation,
        objective_slot,
        stage_slot,
        is_level_end: objective.is_level_end,
        done: objective.done,
        text_selector: objective.text_selector,
        stage_state,
        stage_kind: stage.kind,
        stage_flags: stage.flags,
        axis_x: stage.axis_x,
        axis_y: stage.axis_y,
        relation_domain,
        reserved_0: 0,
        relation_value,
        reserved: [0; 4],
    }
}

pub(super) fn campaign_player_snapshot(
    player: &sim_core::native_world::PlayerSnapshot,
    active: bool,
) -> McCampaignPlayerSnapshotV2 {
    McCampaignPlayerSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignPlayerSnapshotV2>(),
        slot: player.slot,
        is_ai: u8::from(player.is_ai),
        active: u8::from(active),
        reserved_0: 0,
        entity_index: player.entity_index,
        active_player_index: player.active_player_index,
        turn: player.turn,
        left_spell_slot_raw: player.left_spell_slot_raw,
        right_spell_slot_raw: player.right_spell_slot_raw,
        left_subspell_index: player.left_subspell_index,
        right_subspell_index: player.right_subspell_index,
        selected_spell_ui_index: player.selected_spell_ui_index,
        selected_subspell_index: player.selected_subspell_index,
        reserved_1: [0; 2],
        reserved: [0; 4],
    }
}

pub(super) fn campaign_entity_snapshot(entity: &EntitySnapshot) -> McCampaignEntitySnapshotV2 {
    McCampaignEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEntitySnapshotV2>(),
        entity_index: entity.index,
        source_id: entity.source_id,
        next_relation: entity.next_relation.unwrap_or(u16::MAX),
        parameter_relation: entity.parameter_relation.unwrap_or(u16::MAX),
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
        reserved: [0; 4],
    }
}

pub(super) fn asset_ref(world: &NativeWorld, index: u32) -> AbiResult<&AssetRef> {
    let index = usize::try_from(index)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "asset index does not fit usize"))?;
    world.asset_refs().get(index).ok_or_else(|| {
        AbiError::new(
            MC_STATUS_BOUNDS,
            format!("asset index {index} is out of bounds"),
        )
    })
}

pub(super) fn asset_string(asset: &AssetRef, kind: u32, owner_index: u32) -> AbiResult<&str> {
    match kind {
        MC_ASSET_STRING_ID => Ok(&asset.asset_id),
        MC_ASSET_STRING_CONTENT_ADDRESS => Ok(&asset.content_address),
        MC_ASSET_STRING_CATEGORY => Ok(&asset.observation.category),
        MC_ASSET_STRING_HOOK => Ok(&asset.observation.hook),
        MC_ASSET_STRING_STAGE => Ok(asset.observation.stage.as_deref().unwrap_or("")),
        MC_ASSET_STRING_OWNER_ID => {
            let owner_index = usize::try_from(owner_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "owner index does not fit usize"))?;
            asset
                .owner_asset_ids
                .get(owner_index)
                .map(String::as_str)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("owner asset index {owner_index} is out of bounds"),
                    )
                })
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown asset string kind {kind}"),
        )),
    }
}

pub(super) fn resolution_code(evidence: &StepEvidence) -> u32 {
    match evidence.movement_resolution {
        "accepted" => 0,
        "redirected" => 1,
        "rejected" => 2,
        "rejected-and-stopped" => 3,
        "recovered-after-reject" => 4,
        "skipped" => 5,
        _ => u32::MAX,
    }
}

pub(super) fn profile_code(cave_mode: bool) -> u8 {
    if cave_mode {
        MC_PROFILE_CAVE
    } else {
        MC_PROFILE_OUTDOOR
    }
}

pub(super) fn next_nonzero(value: u64) -> u64 {
    let next = value.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

pub(super) fn load_optional_terrain_frame_assets() -> AbiResult<Option<TerrainFrameAssets>> {
    let Some(root) = std::env::var_os("MC_ASSET_CACHE_PATH") else {
        return Ok(None);
    };
    load_terrain_frame_assets(&PathBuf::from(root)).map(Some)
}

pub(super) fn load_optional_level2_day_frame_assets(
    explicit_root: Option<&str>,
) -> AbiResult<Option<DayIndexedFrameAssets>> {
    let root = explicit_root
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from));
    let Some(root) = root else {
        return Ok(None);
    };
    let material_root = if root.join("components/material/manifest.json").is_file() {
        root.join("components/material")
    } else {
        root
    };
    DayIndexedFrameAssets::load(material_root)
        .map(Some)
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load qualified Level 2 Day indexed-frame cache: {error}"),
            )
        })
}

pub(super) fn load_optional_level2_day_music(
    explicit_root: Option<&str>,
) -> AbiResult<Option<CampaignDayMusicLifecycle>> {
    let root = explicit_root
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from));
    let Some(root) = root else {
        return Ok(None);
    };
    let audio_root = if root.join("components/audio/manifest.json").is_file() {
        root.join("components/audio")
    } else {
        root
    };
    let _ = read_qualified_cache_file(
        &audio_root,
        "audio/descriptor.json",
        171_379,
        "53b04308fe28b218d122fb515c12e902259eab60c1b91bc031bd154ca8aa477a",
    )?;
    let _ = read_qualified_cache_file(
        &audio_root,
        "audio/music/C2GAME1.xmi",
        10_352,
        "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d",
    )?;
    Ok(Some(CampaignDayMusicLifecycle::start_day()))
}

pub(super) fn load_terrain_frame_assets(root: &Path) -> AbiResult<TerrainFrameAssets> {
    let terrain_atlas = read_qualified_cache_file(
        root,
        "terrain/bl32n.atlas.indices.u8",
        LEVEL1_TERRAIN_ATLAS_BYTES,
        TERRAIN_ATLAS_SHA256,
    )?;
    let tablesn = read_qualified_cache_file(
        root,
        "palette/tablesn.u8",
        sim_core::mode26_raster::NIGHT_TABLES_BYTES,
        TABLESN_SHA256,
    )?;
    let sky =
        read_qualified_cache_file(root, "sky/skyn.indices.u8", LEVEL1_SKY_BYTES, SKYN_SHA256)?;
    let palette_rgb6 = read_qualified_cache_file(root, "palette/paln.rgb6", 768, PALN_RGB6_SHA256)?;
    let archer_tmap_bank = load_qualified_archer_tmap_bank(root)?;
    let firefly_tmap_bank = load_qualified_firefly_tmap_bank(root)?;
    let frozen_actor_tmap_bank = load_qualified_frozen_actor_tmap_bank(root)?;
    let combat_tmap_bank = load_qualified_combat_tmap_bank(root)?;
    let static_world_tmap_bank = load_qualified_static_world_tmap_bank(root)?;
    let economy_tmap_bank = load_qualified_economy_tmap_bank(root)?;
    let indexed_overlays = IndexedOverlayAssets::load(root)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
    Ok(TerrainFrameAssets {
        terrain_atlas: terrain_atlas.into_boxed_slice(),
        tablesn: tablesn.into_boxed_slice(),
        sky: sky.into_boxed_slice(),
        palette_rgb6: palette_rgb6.into_boxed_slice(),
        archer_tmap_bank,
        firefly_tmap_bank,
        frozen_actor_tmap_bank,
        combat_tmap_bank,
        static_world_tmap_bank,
        economy_tmap_bank,
        indexed_overlays,
    })
}

pub(super) fn load_qualified_frozen_actor_tmap_bank(root: &Path) -> AbiResult<FrozenActorTmapBank> {
    let dolmen = read_qualified_cache_file(
        root,
        "terrain/tmaps1/079.decoded.bin",
        DOLMEN_PAYLOAD_BYTES,
        DOLMEN_PAYLOAD_SHA256,
    )?;
    let payloads = VILLAGER_ANIMATED_SELECTORS
        .iter()
        .enumerate()
        .map(|(ordinal, selector)| {
            read_qualified_cache_file(
                root,
                &format!("terrain/tmaps1/{selector:03}.decoded.bin"),
                VILLAGER_PAYLOAD_BYTES[ordinal],
                VILLAGER_PAYLOAD_SHA256[ordinal],
            )
        })
        .collect::<AbiResult<Vec<_>>>()?;
    FrozenActorTmapBank::decode_qualified(
        &dolmen,
        std::array::from_fn(|ordinal| payloads[ordinal].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

pub(super) fn load_qualified_economy_tmap_bank(root: &Path) -> AbiResult<EconomyTmapBank> {
    let neutral_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/058.decoded.bin",
        MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
        MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256,
    )?;
    let player_one_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/067.decoded.bin",
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
    )?;
    let carrier = read_qualified_cache_file(
        root,
        "terrain/tmaps1/088.decoded.bin",
        MANA_CARRIER_PAYLOAD_BYTES,
        MANA_CARRIER_PAYLOAD_SHA256,
    )?;
    let castle = read_qualified_cache_file(
        root,
        "terrain/tmaps1/096.decoded.bin",
        CASTLE_ROOT_PAYLOAD_BYTES,
        CASTLE_ROOT_PAYLOAD_SHA256,
    )?;
    EconomyTmapBank::decode_qualified(&neutral_mana, &player_one_mana, &carrier, &castle)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

pub(super) fn load_qualified_static_world_tmap_bank(root: &Path) -> AbiResult<StaticWorldTmapBank> {
    let spell_jar = read_qualified_cache_file(
        root,
        "terrain/tmaps1/059.decoded.bin",
        SPELL_JAR_PAYLOAD_BYTES,
        SPELL_JAR_PAYLOAD_SHA256,
    )?;
    let exit_target = read_qualified_cache_file(
        root,
        "terrain/tmaps1/461.decoded.bin",
        EXIT_TARGET_PAYLOAD_BYTES,
        EXIT_TARGET_PAYLOAD_SHA256,
    )?;
    let secondary = [30_u16, 31, 32, 33, 34, 35, 36, 37, 66, 259]
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let scenery = LEVEL1_SCENERY_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let ambient = LEVEL1_AMBIENT_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    StaticWorldTmapBank::decode_with_level1_secondary_scenery_and_ambient(
        &spell_jar,
        &exit_target,
        std::array::from_fn(|index| secondary[index].as_slice()),
        std::array::from_fn(|index| scenery[index].as_slice()),
        std::array::from_fn(|index| ambient[index].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

pub(super) fn load_qualified_combat_tmap_bank(root: &Path) -> AbiResult<CombatTmapBank> {
    let fireball = read_qualified_cache_file(
        root,
        "terrain/tmaps1/463.decoded.bin",
        4_346,
        FIREBALL_PAYLOAD_SHA256,
    )?;
    let possession = read_qualified_cache_file(
        root,
        "terrain/tmaps1/144.decoded.bin",
        7_186,
        POSSESSION_PAYLOAD_SHA256,
    )?;
    let mut static_pixels = Vec::with_capacity(COMBAT_STATIC_SELECTORS.len());
    for ordinal in 0..COMBAT_STATIC_SELECTORS.len() {
        let selector = COMBAT_STATIC_SELECTORS[ordinal];
        let (width, height) = COMBAT_STATIC_EXTENTS[ordinal];
        static_pixels.push(read_qualified_cache_file(
            root,
            &format!("terrain/tmaps1/{selector:03}.indices.u8"),
            usize::from(width) * usize::from(height),
            COMBAT_STATIC_SHA256[ordinal],
        )?);
    }
    let references: Vec<&[u8]> = static_pixels.iter().map(Vec::as_slice).collect();
    let references: [&[u8]; COMBAT_STATIC_SELECTORS.len()] = references
        .try_into()
        .map_err(|_| AbiError::new(MC_STATUS_VALIDATION_FAILED, "combat TMAP count changed"))?;
    CombatTmapBank::decode_qualified(&fireball, &possession, references)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

pub(super) fn load_qualified_archer_tmap_bank(root: &Path) -> AbiResult<ArcherTmapBank> {
    let archer_animated = [
        (
            "terrain/tmaps1/008.decoded.bin",
            6_300,
            "084b65352a3123fa3d5dd8fb344ba56bb4e78f387b1026dd3cdfe590b56c0691",
        ),
        (
            "terrain/tmaps1/009.decoded.bin",
            6_958,
            "96747ee124997f694b384bdd1acf7d896f609ef5c5ef6f1eca9fa9875c57c4f1",
        ),
        (
            "terrain/tmaps1/010.decoded.bin",
            6_048,
            "f07373bd03faec45604b1ad16ae016c005cec11584d39fa76a4769e04c940cf7",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_static = [
        (
            "terrain/tmaps1/011.indices.u8",
            1_938,
            "e41cac6d975a7cd02e50a1f63aba2ab51fbf54587f2ddb5af89e9d4257385a1a",
        ),
        (
            "terrain/tmaps1/012.indices.u8",
            2_090,
            "82ad42cd719d21c2422632a44361b6c918a805efbd4bca8bffc998099048f77a",
        ),
        (
            "terrain/tmaps1/013.indices.u8",
            1_760,
            "d81fe313bcc95ea110fee6bfd4e0716a8eef91737d964be3dee173292f87b44e",
        ),
        (
            "terrain/tmaps1/121.indices.u8",
            1_440,
            "da5633de0e9f4a0591c5f11a6648bb0f6c5caf1b586ed1b13b11d00480c35501",
        ),
        (
            "terrain/tmaps1/122.indices.u8",
            1_710,
            "7b0a2db4ae60b748857a356deed89a6698950371ad564fc5b51ff1985da58fc9",
        ),
        (
            "terrain/tmaps1/123.indices.u8",
            1_440,
            "ce6eb45e6e8d07c45de984c1cb78d574a5ae2f4357df0318f3f3ce9bf37bd3df",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_tmap_bank = ArcherTmapBank::decode_qualified(
        [
            &archer_animated[0],
            &archer_animated[1],
            &archer_animated[2],
        ],
        [
            &archer_static[0],
            &archer_static[1],
            &archer_static[2],
            &archer_static[3],
            &archer_static[4],
            &archer_static[5],
        ],
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
    Ok(archer_tmap_bank)
}

pub(super) fn load_qualified_firefly_tmap_bank(root: &Path) -> AbiResult<FireflyTmapBank> {
    let payloads = [
        (
            "terrain/tmaps1/279.decoded.bin",
            8_378,
            "d4ffec8ae18edb62f68441ecd29cf375b6873af3cf98a780bc3539f11b5b1387",
        ),
        (
            "terrain/tmaps1/280.decoded.bin",
            9_828,
            "c334d13b8adf0d274452643760d58a2bcb5527241cd5d8a9ed0ae3550a77b7c9",
        ),
        (
            "terrain/tmaps1/281.decoded.bin",
            10_290,
            "fbfb64916d1d8e7b3c460b0235d47f227603e434d8b6bed808d2d4079986a9e7",
        ),
        (
            "terrain/tmaps1/282.decoded.bin",
            10_292,
            "f91014fba453529488ca0e49f3643622eb8485c1e2d45e631a8bfd1d18d0c720",
        ),
        (
            "terrain/tmaps1/283.decoded.bin",
            10_902,
            "3566b01e55bc04862eb40cfd20150e7eb03cf04f842d37078a245521d225df8d",
        ),
        (
            "terrain/tmaps1/284.decoded.bin",
            11_018,
            "8f75ffc75929f75364445a32c59e04a9b335e3f4d9d96388a106939987f0b4d0",
        ),
        (
            "terrain/tmaps1/285.decoded.bin",
            11_344,
            "ae00740ed6c6d188d369aad1ef95d062ec3d301ecdf7f0413d8b24c9086c367b",
        ),
        (
            "terrain/tmaps1/286.decoded.bin",
            11_294,
            "8095182d4842a4b04f2f02e3937225599bcd2fc50d73ef0ea1170024d5b6b283",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    FireflyTmapBank::decode_qualified([
        &payloads[0],
        &payloads[1],
        &payloads[2],
        &payloads[3],
        &payloads[4],
        &payloads[5],
        &payloads[6],
        &payloads[7],
    ])
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

pub(super) fn read_qualified_cache_file(
    root: &Path,
    relative: &str,
    expected_len: usize,
    expected_sha256: &str,
) -> AbiResult<Vec<u8>> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!(
                "cannot resolve owner asset cache {}: {error}",
                root.display()
            ),
        )
    })?;
    if fs::symlink_metadata(&canonical_root)
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_IO_ERROR,
                format!(
                    "cannot inspect owner asset cache {}: {error}",
                    canonical_root.display()
                ),
            )
        })?
        .file_type()
        .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            "owner asset cache root must not be a symlink",
        ));
    }
    let candidate = canonical_root.join(relative);
    let canonical_candidate = fs::canonicalize(&candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot resolve owner cache file {relative}: {error}"),
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_root)
        || fs::symlink_metadata(&candidate)
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("cannot inspect owner cache file {relative}: {error}"),
                )
            })?
            .file_type()
            .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} escapes the cache or is a symlink"),
        ));
    }
    let bytes = fs::read(&canonical_candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot read owner cache file {relative}: {error}"),
        )
    })?;
    if bytes.len() != expected_len {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!(
                "owner cache file {relative} has {} bytes, expected {expected_len}",
                bytes.len()
            ),
        ));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha256 != expected_sha256 {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} hash mismatch"),
        ));
    }
    Ok(bytes)
}

pub(super) fn struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("ABI struct size fits u32")
}

pub(super) fn string_length(value: &str) -> AbiResult<u32> {
    u32::try_from(value.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "string length exceeds u32"))
}

pub(super) fn copy_string(
    value: &str,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> AbiResult<()> {
    let payload_length = string_length(value)?;
    write_output(out_length, payload_length);
    let required = payload_length
        .checked_add(1)
        .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "string capacity overflow"))?;
    if capacity < required {
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("string buffer requires {required} bytes, got {capacity}"),
        ));
    }
    if buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "string output buffer is null",
        ));
    }
    // SAFETY: the caller supplied `capacity >= value.len() + 1` writable bytes.
    unsafe {
        buffer.copy_from_nonoverlapping(value.as_ptr(), value.len());
        *buffer.add(value.len()) = 0;
    }
    Ok(())
}

pub(super) fn copy_slice<T: Copy>(
    values: &[T],
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> AbiResult<()> {
    let required = u32::try_from(values.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "slice length exceeds u32"))?;
    if capacity < required {
        write_output(out_written, 0);
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("copy requires {required} elements, got {capacity}"),
        ));
    }
    if !values.is_empty() && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "copy output buffer is null",
        ));
    }
    // SAFETY: the caller supplied enough writable T elements and source and
    // destination cannot overlap across the FFI ownership boundary.
    unsafe { buffer.copy_from_nonoverlapping(values.as_ptr(), values.len()) };
    write_output(out_written, required);
    Ok(())
}

pub(super) fn utf8_input(pointer: *const u8, length: u32, name: &str) -> AbiResult<String> {
    let length = usize::try_from(length)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{name} length exceeds usize")))?;
    if length > 0 && pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    if length == 0 {
        return Ok(String::new());
    }
    // SAFETY: the caller promises `length` readable bytes at the non-null
    // pointer for the duration of this call.
    let bytes = unsafe { slice::from_raw_parts(pointer, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|error| {
        AbiError::new(
            MC_STATUS_INVALID_UTF8,
            format!("{name} is not valid UTF-8: {error}"),
        )
    })
}

pub(super) fn read_versioned_input<T: Copy>(pointer: *const T, name: &str) -> AbiResult<T> {
    if pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    // SAFETY: every versioned ABI input begins with two u32 fields. Reading
    // only that fixed header lets us reject short callers without first
    // reading the full current Rust structure.
    let bytes = pointer.cast::<u8>();
    let abi_version = unsafe { bytes.cast::<u32>().read_unaligned() };
    let declared_size = unsafe { bytes.add(size_of::<u32>()).cast::<u32>().read_unaligned() };
    if abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!("{name} ABI version {abi_version} is unsupported"),
        ));
    }
    let required = struct_size::<T>();
    if declared_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} struct_size {declared_size} is smaller than {required}"),
        ));
    }
    // SAFETY: after declaring at least the current prefix size, the caller is
    // required by the ABI contract to provide that readable prefix.
    Ok(unsafe { pointer.read_unaligned() })
}

pub(super) fn require_output<T>(pointer: *mut T, name: &str) -> AbiResult<()> {
    if pointer.is_null() {
        Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ))
    } else {
        Ok(())
    }
}

pub(super) fn require_struct_output<T>(
    pointer: *mut T,
    output_size: u32,
    name: &str,
) -> AbiResult<()> {
    require_output(pointer, name)?;
    let required = struct_size::<T>();
    if output_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {output_size} is smaller than {required}"),
        ));
    }
    Ok(())
}

pub(super) fn require_element_size<T>(element_size: u32, name: &str) -> AbiResult<()> {
    let required = struct_size::<T>();
    if element_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {element_size} is smaller than {required}"),
        ));
    }
    if element_size > required {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("{name} size {element_size} must equal {required}"),
        ));
    }
    Ok(())
}

pub(super) fn write_output<T>(pointer: *mut T, value: T) {
    // SAFETY: all callers validated the output pointer before reaching this
    // helper. Unaligned writes support conservative P/Invoke marshaling.
    unsafe { pointer.write_unaligned(value) };
}
