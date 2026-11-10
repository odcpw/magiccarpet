//! Shared level-exit behavior used by every package that defines an exit
//! portal and exit-sequence trigger.

use crate::{
    FlightContext, FlightProfile, FlightState, FlightWorld, MovementResolution, Position,
    heading_for_positions, move_entity, resolve_legacy_movement, turn_toward,
};

pub const EXIT_PLAYER_ACTION: u8 = 12;
pub const SECRET_EXIT_PLAYER_ACTION: u8 = 11;
pub const EXIT_SWITCH_SOUND: u16 = 41;
pub const EXIT_ACCELERATION_SOUND: u16 = 19;
pub const EXIT_PALETTE_NONE: u8 = 0;
pub const EXIT_PALETTE_ENTRY_FLASH_REQUEST: u8 = 6;
pub const EXIT_PALETTE_BLACK_REQUEST: u8 = 5;

const EXIT_TARGET_DISTANCE: u32 = 0x180;

/// Destination of an ordinary exit, fixed when the mission is opened.
/// The game adapter derives this from retained campaign progress.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OrdinaryExitRoute {
    #[default]
    CampaignMap,
    UnfinishedSecretContinuation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ExitMovementResolution {
    #[default]
    NotAttempted = 0,
    Accepted = 1,
    Redirected = 2,
    Rejected = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LevelExitEffect {
    PrepareSound {
        sound_id: u16,
        /// The source emission precedes the common altitude epilogue.
        position: Position,
    },
    Completed {
        result_code: u8,
    },
}

/// Canonical state established when a package exit trigger captures its
/// portal. The player transition, sequence body, and presentation phases use
/// this same carrier; there is no level-specific exit session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LevelExitSequence {
    pub ordinary_exit_route: OrdinaryExitRoute,
    pub active: bool,
    pub complete: bool,
    pub phase: u8,
    pub timer: i16,
    pub trigger_entity_index: u16,
    pub portal_entity_index: u16,
    pub portal_valid: bool,
    pub end_sequence: bool,
    pub player_status_flags: u8,
    pub palette_submode: u8,
    pub last_movement: ExitMovementResolution,
    pub display_blank: bool,
    pub blur_mode: u8,
    pub result_code: u8,
    pub camera_fov: i16,
}

impl Default for LevelExitSequence {
    fn default() -> Self {
        Self {
            ordinary_exit_route: OrdinaryExitRoute::CampaignMap,
            active: false,
            complete: false,
            phase: 0,
            timer: 0,
            trigger_entity_index: 0,
            portal_entity_index: 0,
            portal_valid: false,
            end_sequence: false,
            player_status_flags: 0,
            palette_submode: EXIT_PALETTE_NONE,
            last_movement: ExitMovementResolution::NotAttempted,
            display_blank: false,
            blur_mode: 0,
            result_code: 0,
            camera_fov: 128,
        }
    }
}

impl LevelExitSequence {
    pub(crate) fn start(&mut self, trigger_entity_index: u16, portal_entity_index: u16) -> bool {
        if self.active || self.complete {
            return false;
        }
        self.active = true;
        self.phase = 0;
        self.timer = 0;
        self.trigger_entity_index = trigger_entity_index;
        self.portal_entity_index = portal_entity_index;
        self.portal_valid = portal_entity_index != 0;
        true
    }

    /// Advance the source player action-12 callback exactly once.
    pub(crate) fn advance<W: FlightWorld>(
        &mut self,
        world: &W,
        profile: FlightProfile,
        player: &mut FlightState,
        context: &mut FlightContext,
        portal_position: Option<Position>,
    ) -> Vec<LevelExitEffect> {
        self.advance_sequence(
            world,
            profile,
            player,
            context,
            portal_position,
            self.ordinary_exit_route == OrdinaryExitRoute::UnfinishedSecretContinuation,
        )
    }

    /// Action 11 shares `sub_5E8C0_endGameSeq` with the ordinary exit, but
    /// phase 12 sets the source secret-transition bit in the player result.
    pub(crate) fn advance_secret<W: FlightWorld>(
        &mut self,
        world: &W,
        profile: FlightProfile,
        player: &mut FlightState,
        context: &mut FlightContext,
        portal_position: Option<Position>,
    ) -> Vec<LevelExitEffect> {
        self.advance_sequence(world, profile, player, context, portal_position, true)
    }

    fn advance_sequence<W: FlightWorld>(
        &mut self,
        world: &W,
        profile: FlightProfile,
        player: &mut FlightState,
        context: &mut FlightContext,
        portal_position: Option<Position>,
        secret_transition: bool,
    ) -> Vec<LevelExitEffect> {
        if !self.active || self.complete {
            return Vec::new();
        }
        self.last_movement = ExitMovementResolution::NotAttempted;
        let mut effects = Vec::new();
        let mut acceleration_sound = false;
        loop {
            match self.phase {
                0 => {
                    effects.push(LevelExitEffect::PrepareSound {
                        sound_id: EXIT_SWITCH_SOUND,
                        position: player.position,
                    });
                    self.palette_submode = EXIT_PALETTE_ENTRY_FLASH_REQUEST;
                    self.player_status_flags |= 0x20;
                    player.immobilize_counter = 0;
                    player.speed_modifier = 0;
                    player.target_speed = 0;
                    player.recoil_speed = 0;
                    player.strafe_speed = 0;
                    if let Some(holder) = context.speed_spell_holder.as_mut() {
                        holder.active_value = 0;
                    }
                    self.portal_valid = self.portal_entity_index != 0 && portal_position.is_some();
                    if !self.portal_valid {
                        self.portal_entity_index = 0;
                    }
                    self.phase = 1;
                }
                1 => {
                    let old_speed = player.actual_speed;
                    if !self.try_move(world, profile, player, player.yaw, old_speed) {
                        player.actual_speed = 0;
                    }
                    if old_speed.unsigned_abs() <= 4 {
                        player.actual_speed = 0;
                    } else if old_speed <= 0 {
                        player.actual_speed = player.actual_speed.wrapping_add(4);
                    } else {
                        player.actual_speed = player.actual_speed.wrapping_sub(4);
                    }
                    if player.actual_speed == 0 {
                        self.phase = if self.portal_entity_index == 0 { 4 } else { 3 };
                    }
                }
                3 => {
                    let Some(target) = portal_position else {
                        self.portal_entity_index = 0;
                        self.portal_valid = false;
                        self.phase = 4;
                        continue;
                    };
                    let target_yaw = i16::try_from(heading_for_positions(player.position, target))
                        .expect("11-bit exit heading");
                    if angle_distance(i32::from(player.yaw), i32::from(target_yaw)) <= 11 {
                        player.yaw = target_yaw;
                        self.phase = 4;
                    } else {
                        player.yaw = player
                            .yaw
                            .wrapping_add(turn_toward(player.yaw, target_yaw, 11))
                            & 0x07ff;
                    }
                }
                4 => {
                    self.timer = 12;
                    self.phase = 5;
                    self.end_sequence = true;
                    continue;
                }
                5 => {
                    self.timer = self.timer.wrapping_sub(1);
                    if self.timer > 4 {
                        self.camera_fov = self.camera_fov.wrapping_sub(1);
                    }
                    if self.timer == 0 {
                        self.phase = if self.portal_entity_index == 0 { 8 } else { 6 };
                    }
                }
                6 => {
                    self.timer = 512;
                    player.actual_speed = 100;
                    self.phase = 7;
                    self.blur_mode = 2;
                    acceleration_sound = true;
                    continue;
                }
                7 => {
                    self.timer = self.timer.wrapping_sub(1);
                    if self.timer == 0 {
                        self.phase = 10;
                    } else if let Some(target) = portal_position {
                        player.yaw = i16::try_from(heading_for_positions(player.position, target))
                            .expect("11-bit exit heading");
                        let speed = player.actual_speed;
                        player.actual_speed = accelerated_speed(player.actual_speed);
                        if !self.try_move(world, profile, player, player.yaw, speed)
                            || wrapping_xy_distance(player.position, target) < EXIT_TARGET_DISTANCE
                        {
                            self.phase = 10;
                        }
                    } else {
                        self.phase = 10;
                    }
                }
                8 => {
                    self.timer = 128;
                    player.actual_speed = 100;
                    self.phase = 9;
                    self.blur_mode = 2;
                    acceleration_sound = true;
                    continue;
                }
                9 => {
                    self.timer = self.timer.wrapping_sub(1);
                    if self.timer == 0 {
                        self.phase = 10;
                    } else {
                        let speed = player.actual_speed;
                        player.actual_speed = accelerated_speed(player.actual_speed);
                        if !self.try_move(world, profile, player, player.yaw, speed) {
                            self.phase = 10;
                        }
                    }
                }
                10 => {
                    self.timer = 32;
                    self.phase = 11;
                    self.palette_submode = EXIT_PALETTE_BLACK_REQUEST;
                    continue;
                }
                11 => {
                    self.timer = self.timer.wrapping_sub(1);
                    if self.timer != 0 && self.palette_submode != EXIT_PALETTE_NONE {
                        let _ = self.try_move(world, profile, player, player.yaw, 2);
                    } else {
                        self.phase = 12;
                    }
                }
                12 => {
                    self.result_code = if secret_transition { 10 | 0x10 } else { 10 };
                    self.player_status_flags = self.result_code;
                    self.display_blank = true;
                    self.blur_mode = 0;
                    self.active = false;
                    self.complete = true;
                    effects.push(LevelExitEffect::Completed {
                        result_code: self.result_code,
                    });
                }
                _ => {}
            }
            break;
        }

        // sub_5E8C0_endGameSeq sets v28 in phase6/8, runs phase7/9 movement,
        // then emits sound19 before the common altitude adjustment.
        if acceleration_sound {
            effects.push(LevelExitEffect::PrepareSound {
                sound_id: EXIT_ACCELERATION_SOUND,
                position: player.position,
            });
        }

        let ground = world.ground_height(player.position);
        if i32::from(player.position.z) <= i32::from(ground) + 256 {
            if player.position.z >= ground {
                player.position.z = ground.wrapping_add(128);
            } else {
                player.position.z = player.position.z.wrapping_add(128);
            }
        } else {
            player.position.z = player.position.z.wrapping_sub(128);
        }
        player.roll_accumulator = relax_axis(player.roll_accumulator, 7, 3);
        player.pitch_accumulator = relax_axis(player.pitch_accumulator, 3, 2);
        effects
    }

    fn try_move<W: FlightWorld>(
        &mut self,
        world: &W,
        profile: FlightProfile,
        player: &mut FlightState,
        yaw: i16,
        speed: i16,
    ) -> bool {
        let from = player.position;
        let mut proposed = from;
        move_entity(&mut proposed, yaw, 0, speed);
        match resolve_legacy_movement(player, from, proposed, profile, world) {
            MovementResolution::Accept(position) => {
                player.position = position;
                self.last_movement = ExitMovementResolution::Accepted;
                true
            }
            MovementResolution::Redirect(position) => {
                player.position = position;
                self.last_movement = ExitMovementResolution::Redirected;
                true
            }
            MovementResolution::Reject | MovementResolution::RejectAndStop => {
                self.last_movement = ExitMovementResolution::Rejected;
                false
            }
        }
    }
}

fn angle_distance(left: i32, right: i32) -> i16 {
    let direct = (left - right).unsigned_abs() & 0x07ff;
    let wrapped = 2_048_u32 - direct;
    i16::try_from(direct.min(wrapped)).expect("11-bit angle distance fits i16")
}

fn accelerated_speed(value: i16) -> i16 {
    let value = value.wrapping_add(8);
    if value < 0 { 0 } else { value.min(200) }
}

fn relax_axis(value: i16, negative_bias: i16, shift: u32) -> i16 {
    let source_sign = if value < 0 { -1_i32 } else { 0 };
    let delta = (i32::from(value) - source_sign * i32::from(negative_bias)) >> shift;
    value.wrapping_sub(i16::try_from(delta).expect("exit axis relaxation fits i16"))
}

fn wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .cast_unsigned()
            .wrapping_add(dy.wrapping_mul(dy).cast_unsigned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FlatWorld;

    impl FlightWorld for FlatWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }
        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }
        fn material_mask(&self, _position: Position) -> u32 {
            0
        }
        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    #[test]
    fn secret_exit_sets_transition_bit_without_changing_ordinary_result() {
        for secret in [false, true] {
            let mut sequence = LevelExitSequence {
                active: true,
                phase: 12,
                ..LevelExitSequence::default()
            };
            let mut player = FlightState::default();
            let mut context = FlightContext::default();
            let effects = if secret {
                sequence.advance_secret(
                    &FlatWorld,
                    FlightProfile::OUTDOOR,
                    &mut player,
                    &mut context,
                    None,
                )
            } else {
                sequence.advance(
                    &FlatWorld,
                    FlightProfile::OUTDOOR,
                    &mut player,
                    &mut context,
                    None,
                )
            };
            let expected = if secret { 26 } else { 10 };
            assert_eq!(
                effects,
                vec![LevelExitEffect::Completed {
                    result_code: expected
                }]
            );
            assert_eq!(sequence.player_status_flags, expected);
            assert!(sequence.complete);
            assert!(!sequence.active);
            assert!(
                sequence
                    .advance_secret(
                        &FlatWorld,
                        FlightProfile::OUTDOOR,
                        &mut player,
                        &mut context,
                        None,
                    )
                    .is_empty()
            );
        }
    }

    #[test]
    fn ordinary_exit_result_follows_the_retained_parent_route() {
        for (route, expected) in [
            (OrdinaryExitRoute::CampaignMap, 10),
            (OrdinaryExitRoute::UnfinishedSecretContinuation, 26),
        ] {
            let mut sequence = LevelExitSequence {
                ordinary_exit_route: route,
                active: true,
                phase: 12,
                ..LevelExitSequence::default()
            };
            let effects = sequence.advance(
                &FlatWorld,
                FlightProfile::OUTDOOR,
                &mut FlightState::default(),
                &mut FlightContext::default(),
                None,
            );
            assert_eq!(
                effects,
                vec![LevelExitEffect::Completed {
                    result_code: expected
                }]
            );
            assert_eq!(sequence.result_code, expected);
        }
    }

    #[test]
    fn exit_sound_pose_precedes_altitude_epilogue_and_follows_acceleration_motion() {
        let start = Position {
            x: 10_000,
            y: 10_000,
            z: 1_024,
        };
        for phase in [0, 6, 8] {
            let mut sequence = LevelExitSequence {
                active: true,
                phase,
                portal_entity_index: 1,
                ..LevelExitSequence::default()
            };
            let mut player = FlightState {
                position: start,
                ..FlightState::default()
            };
            let portal = Position {
                x: 10_000,
                y: 20_000,
                z: 1_024,
            };
            let effects = sequence.advance(
                &FlatWorld,
                FlightProfile::OUTDOOR,
                &mut player,
                &mut FlightContext::default(),
                Some(portal),
            );
            assert_eq!(player.position.z, start.z - 128);
            let sound_position = Position {
                z: start.z,
                ..player.position
            };
            assert_eq!(
                effects,
                vec![LevelExitEffect::PrepareSound {
                    sound_id: if phase == 0 {
                        EXIT_SWITCH_SOUND
                    } else {
                        EXIT_ACCELERATION_SOUND
                    },
                    position: sound_position,
                }]
            );
            if phase == 0 {
                assert_eq!(sound_position, start);
            } else {
                assert_ne!((sound_position.x, sound_position.y), (start.x, start.y));
            }
            assert_ne!(sound_position, player.position);
            // The next ordinary phase callback still moves/settles the player,
            // but neither phase1 nor phase7/9 emits another sound.
            let next = sequence.advance(
                &FlatWorld,
                FlightProfile::OUTDOOR,
                &mut player,
                &mut FlightContext::default(),
                Some(portal),
            );
            assert!(next.is_empty());
        }
    }
}
