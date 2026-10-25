use super::{status, transport, wind};
use crate::{
    FlightContext, FlightEffectsContext, FlightProfile, FlightState, FlightWorld, PlayerCommand,
    Position, Tile,
    level_definition::{FieldDefinition, FieldKind, TilePosition},
    native_world::EntitySnapshot,
    step_turn_with_context,
    world::terrain::cave_features::new_event,
};

struct Ground;
impl FlightWorld for Ground {
    fn ground_height(&self, _: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _: Tile) -> u8 {
        0
    }
}

fn player() -> (EntitySnapshot, FlightState) {
    let position = Position {
        x: 10_000,
        y: 10_000,
        z: 2_000,
    };
    let mut body = new_event(100, 77, position);
    body.class = 3;
    body.model = 0;
    body.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    body.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    (
        body,
        FlightState {
            entity_index: 100,
            position,
            actual_speed: 64,
            target_speed: 64,
            ..FlightState::default()
        },
    )
}

fn impact(kind: status::Kind) -> EntitySnapshot {
    let mut row = new_event(120, 13, Position { x: 0, y: 0, z: 0 });
    let (model, action) = kind.source_selector();
    row.class = 10;
    row.model = model;
    row.set_source_action(action);
    row.record[154..156].copy_from_slice(&100_u16.to_le_bytes());
    row
}

fn definition(kind: FieldKind) -> FieldDefinition {
    sim_definition::adapter::mc2_field_definition(kind, TilePosition { x: 0, y: 0 })
}

fn campaign() -> crate::native_world::Level2ResetSession {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        std::path::PathBuf::from,
    );
    crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .unwrap()
    .level2_compatibility()
    .clone()
}

fn scheduled_impact(session: &mut crate::native_world::Level2ResetSession, kind: FieldKind) -> u16 {
    let live = &mut session.session.state.live;
    let player = live.core.state.entity_index;
    let slot = status::construct(
        &mut live.economy,
        &mut live.terrain,
        live.core.state.position,
        definition(kind),
    )
    .unwrap()
    .unwrap();
    // This is the projectile producer's real target relation, not a callback
    // invocation or a replacement player carrier.
    live.economy.entities[usize::from(slot)].record[154..156]
        .copy_from_slice(&player.to_le_bytes());
    assert!(slot > player, "pin impact after the local player visit");
    slot
}

fn campaign_step(session: &mut crate::native_world::Level2ResetSession) {
    assert!(
        !session
            .step_outer_turn(PlayerCommand::default(), 0)
            .is_empty()
    );
}

#[test]
fn scheduled_slowing_reaches_player_upkeep_palette_and_reclaim() {
    let mut session = campaign();
    let slot = scheduled_impact(&mut session, FieldKind::Slowing);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.core.state.speed_modifier, 1);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        8
    );
    assert!(session.session.state.live.economy.entities[usize::from(slot)].drawing_disabled());
    campaign_step(&mut session);
    assert!(!session.session.state.live.economy.entities[usize::from(slot)].active);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        7
    );
    let original = [96, 128, 160].repeat(256);
    let mut tinted = original.clone();
    session.field_palette().apply_rgb8(&mut tinted);
    assert_ne!(tinted, original, "actual frame palette consumes slowing");
    for _ in 0..12 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!((state.speed_modifier, state.speed_modifier_counter), (0, 0));
    let mut recovered = original.clone();
    session.field_palette().apply_rgb8(&mut recovered);
    assert_eq!(
        recovered, original,
        "frame service finishes the recovery fade"
    );
}

#[test]
fn scheduled_web_damages_actual_player_once_and_refresh_only_recovers() {
    let mut session = campaign();
    let life = session.session.state.live.player_life;
    let first = scheduled_impact(&mut session, FieldKind::Web);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life);
    assert_eq!(session.session.state.live.player_pending_damage, 200);
    assert_eq!(session.session.state.live.player_pending_attacker, first);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    let second = scheduled_impact(&mut session, FieldKind::Web);
    let rng = session.session.state.live.economy.entities[usize::from(second)].entity_rng;
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert_eq!(session.session.state.live.player_pending_attacker, 0);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    assert_eq!(
        session.session.state.live.economy.entities[usize::from(second)].entity_rng,
        rng
    );
    for _ in 0..10 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!(
        (state.immobilize_counter, state.immobilize_subcounter),
        (0, 0)
    );
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert!(!session.session.state.live.economy.entities[usize::from(second)].active);
}

#[test]
fn slowing_impacts_stack_once_then_real_flight_recovers_all_stages() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Slowing);
    for (stage, count) in [(1, 142), (2, 199), (3, 256)] {
        let result = status::impact(
            &mut source,
            definition(FieldKind::Slowing),
            Some((&body, &mut state)),
            0,
        )
        .unwrap();
        assert_eq!(state.speed_modifier, stage);
        assert_eq!(state.speed_modifier_counter, 8);
        assert_eq!(result.palette.unwrap().count, count);
        assert_eq!(result.sounds.len(), usize::from(stage == 1));
        assert!(result.contacts.is_empty());
    }
    let rng = source.entity_rng;
    state.recoil_speed = 12;
    let capped = status::impact(
        &mut source,
        definition(FieldKind::Slowing),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(capped.palette.is_none());
    assert_eq!(source.entity_rng, rng);
    assert_eq!(state.recoil_speed, 12);
    assert!(source.drawing_disabled());
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext::default()),
        ..FlightContext::default()
    };
    for tick in 1..=24 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut context,
        );
        assert_eq!(state.speed_modifier, 3 - tick / 8);
    }
    assert_eq!(state.speed_modifier_counter, 0);
    assert_eq!(context.effects.unwrap().palette_submode, 1);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(
        state.position.y < before.y,
        "ordinary translation resumes after expiry"
    );
}

#[test]
fn web_refreshes_without_duplicate_contact_and_real_movement_expires_it() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Web);
    source.set_subspell_index(200);
    let first = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert_eq!((first.contacts.len(), first.contacts[0].payload), (1, 200));
    assert_eq!(
        (
            state.immobilize_counter,
            state.immobilize_subcounter,
            state.recoil_speed
        ),
        (1, 10, -80)
    );
    let rng = source.entity_rng;
    let refresh = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(refresh.contacts.is_empty() && refresh.sounds.is_empty());
    assert_eq!(source.entity_rng, rng);
    state.recoil_speed = 0;
    let start = state.position;
    for tick in 1..=10 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut FlightContext::default(),
        );
        assert_eq!(state.immobilize_counter, if tick < 10 { 1 } else { 0 });
        assert_eq!(
            state.position.y, start.y,
            "web consumes the actual translation branch"
        );
    }
    assert_eq!(state.position.z, start.z - 9 * 51 - 16);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(state.position.y < before.y);
}

#[test]
fn transport_uses_facing_and_destination_and_life_one_hides_before_contact() {
    let (mut body, mut state) = player();
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y - 1,
            ..state.position
        },
    );
    source.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    source.record[90..92].copy_from_slice(&256_i16.to_le_bytes());
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());
    let mut rejected_source = source.clone();
    let mut rejected_body = body.clone();
    let mut rejected_player = state.clone();
    rejected_player.yaw = 170;
    let rejected = transport::step(
        &mut rejected_source,
        &mut rejected_body,
        &mut rejected_player,
        &Ground,
        384,
        0,
    );
    assert!(
        !rejected.transported,
        "facing170 is outside the strict gate"
    );
    assert_eq!(rejected_player.position, state.position);
    state.yaw = 169;
    let phase = source.phase;
    let outcome = transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0);
    assert!(outcome.transported);
    assert_eq!(
        state.position,
        Position {
            x: 30_000,
            y: 40_000,
            z: 384
        }
    );
    assert_eq!(body.position, state.position);
    assert_eq!(source.life, 0);
    assert_eq!(source.phase, phase, "common tail owns phase");
    assert_eq!(outcome.sounds, [(120, 21), (120, 22)]);
    source.set_life(1);
    let before = state.position;
    let outcome = transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0);
    assert!(!outcome.transported);
    assert_eq!(
        (source.life, source.drawing_disabled(), state.position),
        (0, true, before)
    );
    assert_eq!(outcome.sounds, [(120, 20)]);
}

#[test]
fn follower_parent_sentinel_exact_life_and_hidden_parent_semantics() {
    let mut child = new_event(
        20,
        7,
        Position {
            x: 10,
            y: 20,
            z: 30,
        },
    );
    child.set_yaw(2040);
    child.set_actual_speed(0);
    let mut parent = new_event(
        10,
        9,
        Position {
            x: 500,
            y: 700,
            z: 900,
        },
    );
    let before = child.clone();
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(child, before);
    child.record[44..46].copy_from_slice(&10_u16.to_le_bytes());
    parent.active = false;
    parent.disable_drawing();
    parent.set_life(-1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(
        (child.position, child.yaw),
        (
            Position {
                x: 500,
                y: 700,
                z: 256
            },
            37
        )
    );
    assert!(!child.drawing_disabled());
    parent.set_life(1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert!(child.drawing_disabled());
}

#[test]
fn wind_motion_reads_and_writes_the_exported_heading_word() {
    let mut source = new_event(10, 0, Position { x: 0, y: 0, z: 0 });
    source.record[48..50].copy_from_slice(&1111_i16.to_le_bytes());
    source.record[52..54].copy_from_slice(&2222_i16.to_le_bytes());
    let actor = wind::Actor::from_source(&source, None);
    assert_eq!(actor.move_heading, 2222);
    let motion = wind::Motion {
        actor: wind::Actor {
            move_heading: 333,
            ..actor
        },
        actual_speed: None,
        player_roll_delta: 0,
        contact: false,
    };
    motion.write_source(&mut source);
    assert_eq!(wind::Actor::from_source(&source, None).move_heading, 333);
    assert_eq!(source.record[48..50], 1111_i16.to_le_bytes());
}

#[test]
fn palette_uses_rgb6_source_math_and_recovers_through_frame_fade() {
    let original = vec![240_u8; 768];
    let mut palette = super::palette::Palette::default();
    palette.request(super::PaletteRequest {
        mode: 3,
        count: 256,
    });
    let mut colors = original.clone();
    palette.apply_rgb8(&mut colors);
    assert_eq!(
        colors, original,
        "pending source request waits for the frame consumer"
    );
    assert!(palette.advance_frame());
    palette.apply_rgb8(&mut colors);
    assert_eq!(&colors[..6], &[240, 240, 240, 16, 240, 16]);
    for _ in 0..5 {
        palette.advance_frame();
    }
    let mut recovered = original.clone();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(recovered, original);
    palette.request(super::PaletteRequest { mode: 6, count: 0 });
    palette.advance_frame();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(&recovered[3..6], &[252, 252, 252]);
}
