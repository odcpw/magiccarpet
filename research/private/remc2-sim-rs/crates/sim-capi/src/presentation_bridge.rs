//! Production bridge from live Level 1 state to typed FINAL MAIN commands.
//!
//! `native_v2` supplies one atomic snapshot of authoritative session state.
//! This module owns the presentation decisions, source ordering, fixed-profile
//! coordinate conversion, and immutable command plan only. Execution receipts
//! come from the compositor, lighting, postprocess, and overlay writers.

use sim_core::{
    FlightState, Position,
    indexed_overlay::{
        FixedOverlayProfile, IndexedOverlayAssets, IndexedOverlayCommand, IndexedOverlayError,
        IndexedOverlayStage, Level1FinalMainState,
    },
    level1_mission::{
        ABILITY_IMPROVED_NAME_SELECTOR_BASE, ABILITY_IMPROVED_NOTIFICATION_STATE,
        ABILITY_IMPROVED_TEMPLATE_SELECTOR, LEVEL1_MINIMAP_ANCHOR_BOTTOM_CENTER,
        LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT, LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
        LEVEL1_MINIMAP_COMMAND_DOT, LEVEL1_MINIMAP_COMMAND_SPRITE, Level1MinimapEntityCommand,
        Level1MinimapEntityInput, Level1MinimapMarks, level1_minimap_entity_command,
        level1_minimap_project,
    },
    native_world::{Level2ResetSession, TerrainPlanes},
};

/// Source-ordered Level 2 minimap entity presentation row.
///
/// The row is deliberately a presentation carrier only: Rust decides
/// admission, visibility, colour, selector, and source order; consumers do
/// not infer any of those values from scene objects or pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Level2MinimapEntity {
    pub source_order: u16,
    pub subject_entity_index: u16,
    pub x: i16,
    pub y: i16,
    pub selector: u16,
    pub palette_index: u8,
    pub kind: u8,
    pub size: u8,
    pub anchor: u8,
    pub help_selector: u8,
    pub visible: u8,
}

/// Source-ordered Level 2 minimap mark presentation row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Level2MinimapMark {
    pub source_order: u16,
    pub subject_entity_index: u16,
    pub x: i16,
    pub y: i16,
    pub palette_index: u8,
    pub kind: u8,
    pub help_selector: u8,
    pub visible: u8,
}

/// Immutable minimap plan for one generation/tick of the Level 2 campaign.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Level2MinimapPlan {
    pub entities: Vec<Level2MinimapEntity>,
    pub marks: Vec<Level2MinimapMark>,
    pub visibility_flags: u32,
}

const LEVEL2_MINIMAP_SIDE: i32 = 128;
const LEVEL2_MINIMAP_KIND_FIXED_MARK: u8 = 2;

// `playersColors_E88E0x` in the pinned Day GameUI source.  The first two
// columns are the source's primary/secondary minimap colours; the third HUD
// column is intentionally not used by DrawMinimapEntities.
const LEVEL2_DAY_PLAYER_COLOURS: [[u8; 2]; 8] = [
    [0x60, 0x64],
    [0x7B, 0x77],
    [0x1C, 0x18],
    [0x5B, 0x57],
    [0x9A, 0x97],
    [0xDB, 0xD8],
    [0x76, 0xA0],
    [0x3D, 0x3A],
];

fn level2_day_true_wizard_number(source_colour: u8) -> Option<usize> {
    // TransformPlayerColorIndex_616D0's single-player mapping.  The Level 2
    // campaign route is the qualified single-player route; multiplayer's
    // lobby permutation is not a carrier in Level2ResetSession.
    Some(match source_colour {
        0 => 0,
        1 => 1,
        2 => 4,
        3 => 3,
        4 => 2,
        5 => 5,
        6 => 7,
        7 => 6,
        _ => return None,
    })
}

fn level2_day_player_colour(
    session: &Level2ResetSession,
    entity_index: u16,
    secondary: bool,
) -> Option<u8> {
    let player = session
        .players()
        .iter()
        .find(|player| player.entity_index == entity_index)?;
    let wizard = level2_day_true_wizard_number(player.player_color_index())?;
    Some(LEVEL2_DAY_PLAYER_COLOURS[wizard][usize::from(secondary)])
}

fn level2_source_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn level2_minimap_input(
    session: &Level2ResetSession,
    entity: &sim_core::native_world::EntitySnapshot,
    local_entity_index: u16,
) -> Option<Level1MinimapEntityInput> {
    let bytes = entity.record_bytes();
    let player_link = entity.source_player_entity_index();
    let relation_is_player = |entity_index: u16| {
        session
            .entities()
            .iter()
            .find(|candidate| candidate.index == entity_index)
            .is_some_and(|candidate| candidate.active && candidate.class == 3)
    };
    Some(Level1MinimapEntityInput {
        source_slot: entity.index,
        class: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        control_flags: entity.control_flags,
        source_id: entity.source_id,
        source_id_is_player: relation_is_player(entity.source_id),
        source_id_is_local_player: entity.source_id == local_entity_index,
        player_link,
        player_link_is_player: relation_is_player(player_link),
        player_link_is_local_player: player_link == local_entity_index,
        parent_id: level2_source_u16(bytes, 0x28)?,
        stage_var_2: *bytes.get(0x49)? as i8,
        word_32: level2_source_u16(bytes, 0x32)?,
        position: entity.position,
        active: entity.active && !entity.drawing_disabled(),
    })
}

fn level2_day_palette_for_entity(
    session: &Level2ResetSession,
    input: Level1MinimapEntityInput,
) -> Option<u8> {
    let phase_2 = (session.state.tick / 2) & 1 != 0;
    let phase_3 = (session.state.tick / 3) & 1 != 0;
    let linked_player = if input.source_id_is_player {
        Some(input.source_id)
    } else if input.player_link_is_player {
        Some(input.player_link)
    } else {
        None
    };
    let player_colour = |secondary: bool| {
        linked_player
            .and_then(|entity_index| level2_day_player_colour(session, entity_index, secondary))
    };

    let palette = match input.class {
        2 => match input.model {
            0 => 0x1C,
            1 => 23,
            2 => {
                if phase_2 {
                    177
                } else {
                    0x1C
                }
            }
            _ => 0x1C,
        },
        5 => {
            if input.source_id != input.source_slot && input.source_id_is_player {
                player_colour(true)?
            } else if (12..=14).contains(&input.model) {
                23
            } else {
                // DrawMinimapEntities uses colorPalette[0] for the Day
                // generic/entity branch.  PALD's qualified entry zero is the
                // direct zero palette index, not a generated fallback.
                0
            }
        }
        9 => {
            if input.source_id_is_player {
                player_colour(false)?
            } else {
                215
            }
        }
        10 => match input.model {
            39 | 57 | 45 => {
                if input.player_link_is_player {
                    player_colour(!phase_3)?
                } else {
                    0xE8
                }
            }
            78 => level2_day_player_colour(session, session.state.entity_index, !phase_3)?,
            _ => {
                if input.source_id_is_player {
                    player_colour(false)?
                } else {
                    215
                }
            }
        },
        12 | 15 => 119,
        14 if input.model == 5 => {
            if phase_3 {
                119
            } else {
                176
            }
        }
        _ => return None,
    };
    Some(palette)
}

fn level2_day_minimap_command(
    session: &Level2ResetSession,
    input: Level1MinimapEntityInput,
    family12_enabled: bool,
) -> Option<Level1MinimapEntityCommand> {
    // DrawMinimapEntities_B_61A00 has no class-3/model-1 entity branch.  The
    // Day Nyphur row is admitted by DrawMinimapMarks_644F0 instead.
    if input.class == 3 && !matches!(input.model, 2 | 3) {
        return None;
    }
    // Day's model-0 class-2 branch is admitted only for action 2; the old
    // Level 1 classifier intentionally did not need this Day action gate.
    if input.class == 2 && input.model == 0 && input.action != 2 {
        return None;
    }

    // The pinned source's v83 is the non-zero local family-12 holder word.
    // The typed player projection exposes the authoritative enabled holder;
    // this keeps the model-3 player branch fail-closed when it is absent.
    let mut classified_input = input;
    if input.class == 3
        && input.model == 3
        && family12_enabled
        && !input.player_link_is_local_player
    {
        classified_input.player_link_is_local_player = true;
    }
    let mut command = level1_minimap_entity_command(&session.state, classified_input)?;
    if input.class == 3 {
        let source_player = if input.source_id_is_player {
            input.source_id
        } else if input.player_link_is_player {
            input.player_link
        } else {
            return None;
        };
        let wizard = level2_day_true_wizard_number(
            session
                .players()
                .iter()
                .find(|player| player.entity_index == source_player)?
                .player_color_index(),
        )?;
        command.selector = command.selector.checked_add(wizard as u16)?;
        command.palette_index = 0;
    } else {
        command.palette_index = level2_day_palette_for_entity(session, input)?;
    }
    Some(command)
}

/// Builds the bounded Level 2 minimap projection from the live authoritative
/// session. Unknown entity classes and unqualified source subjects are
/// omitted, rather than replaced with a generic presentation.
pub(crate) fn level2_minimap_plan(
    session: &Level2ResetSession,
) -> Result<Level2MinimapPlan, String> {
    let local_player_slot = session.runtime().local_player_slot as usize;
    let local_player = session
        .players()
        .get(local_player_slot)
        .ok_or_else(|| "level2 minimap local player is unavailable".to_owned())?;
    let local_entity_index = local_player.entity_index;
    let nyphur = session
        .nyphur_participant()
        .map_err(|error| error.to_string())?;
    let visibility = session
        .nyphur_visibility()
        .map_err(|error| error.to_string())?;
    let family12_enabled = session
        .players()
        .get(local_player_slot)
        .and_then(|player| player.enabled_spell_holders.get(12))
        .is_some_and(|holder| *holder != 0);

    let mut entities = Vec::new();
    for entity in session.entities() {
        if !entity.active || entity.drawing_disabled() {
            continue;
        }
        let source_player_entity_index = entity.source_player_entity_index();
        let is_nyphur = entity.index == nyphur.entity_index
            || source_player_entity_index == nyphur.entity_index;
        if is_nyphur && !visibility.visible_to_local_player {
            continue;
        }
        let input = level2_minimap_input(session, entity, local_entity_index).ok_or_else(|| {
            format!(
                "level2 minimap entity {} record is incomplete",
                entity.index
            )
        })?;
        let command = level2_day_minimap_command(session, input, family12_enabled);
        let Some(command) = command else {
            continue;
        };
        entities.push(Level2MinimapEntity {
            source_order: command.source_slot,
            subject_entity_index: entity.index,
            x: command.x,
            y: command.y,
            selector: command.selector,
            palette_index: command.palette_index,
            kind: command.kind,
            size: command.size,
            anchor: command.anchor,
            help_selector: command.help_selector,
            visible: 1,
        });
    }

    let objective_index = usize::from(local_player.player_color_index());
    let objective = session
        .runtime()
        .objectives
        .get(objective_index)
        .ok_or_else(|| "level2 minimap local objective is unavailable".to_owned())?;
    let stage_index = usize::from(objective.text_selector);
    let stage = session
        .runtime()
        .stages
        .get(stage_index)
        .ok_or_else(|| "level2 minimap selected objective stage is unavailable".to_owned())?;
    let mut marks = Vec::new();
    if stage.kind == 5 {
        let (x, y) = level1_minimap_project(
            &session.state,
            sim_core::Position {
                x: stage.axis_x as u16,
                y: stage.axis_y as u16,
                z: 0,
            },
            LEVEL2_MINIMAP_SIDE,
            256,
        );
        marks.push(Level2MinimapMark {
            source_order: u16::try_from(stage_index)
                .map_err(|_| "level2 minimap objective stage order exceeds u16".to_owned())?,
            subject_entity_index: 0,
            x,
            y,
            palette_index: 184,
            kind: LEVEL2_MINIMAP_KIND_FIXED_MARK,
            help_selector: 0x51,
            visible: 1,
        });
    }

    let visibility_flags = u32::from(visibility.visible_to_local_player)
        | (u32::from(visibility.owner_visibility_mode != 0) << 1)
        | (u32::from(visibility.beyond_sight_holder_present) << 2)
        | (u32::from(visibility.beyond_sight_active) << 3)
        | (u32::from(visibility.family4_gate_active) << 4);
    Ok(Level2MinimapPlan {
        entities,
        marks,
        visibility_flags,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveSpellSlot {
    pub holder_entity_index: u16,
    pub model: u8,
    pub tier: i8,
    pub enabled_flags: u8,
    pub active_value: i16,
    pub max_mana: i32,
    pub mana_regen: i32,
    pub caster_mana: i32,
    pub frame_selector: u16,
    pub icon_selector: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveHud {
    pub mana_color_index: u8,
    pub mana_max_color_index: u8,
    pub life_color_index: u8,
    pub color_phase_1: u8,
    pub color_phase_2: u8,
    pub player_hit_frame_ticks: u8,
    pub castle_hit_frame_ticks: u8,
    pub global_mana_denominator: i32,
    pub secured_mana_component: i32,
    pub goal_percent: i32,
    pub player_life: i32,
    pub player_max_life: i32,
    pub player_mana: i32,
    pub player_max_mana: i32,
    pub castle_active: bool,
    pub castle_level: u8,
    pub castle_life: i32,
    pub castle_max_life: i32,
    pub castle_mana: i32,
    pub castle_mana_capacity: i32,
    pub player_status_flags: u32,
    pub player_status_word: u16,
    pub balloon_level: u8,
    pub balloon_life: [i32; 3],
    pub balloon_max_life: [i32; 3],
    pub balloon_mana: [i32; 3],
    pub balloon_max_mana: [i32; 3],
    pub left: LiveSpellSlot,
    pub right: LiveSpellSlot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveSpellMenu {
    pub selected_ui_index: u8,
    pub selected_tier: i8,
    pub left_family: u8,
    pub right_family: u8,
    pub family_available: [u8; 26],
    pub holder_entity_index: [u16; 26],
    pub family_unlocked_tier: [i8; 26],
    pub family_selected_tier: [i8; 26],
    pub family_hand_eligibility: [u8; 26],
    pub family_enabled_flags: [u8; 26],
    pub family_active_value: [i16; 26],
    pub family_experience: [i32; 26],
    pub family_mana_cost: [[i32; 3]; 26],
    pub family_max_mana_limit: [[i32; 3]; 26],
    pub castle_mana: i32,
    pub caster_mana: i32,
    pub player_colour_0: u8,
    pub player_colour_1: u8,
    pub active_suppression_phase: bool,
    pub cursor_visible: bool,
    pub cursor_x: i16,
    pub cursor_y: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiveMenuState {
    pub authority_mask: u32,
    pub selected_menu_item: u8,
    pub pause_menu_suppressed: bool,
    pub pause_text_suppressed: bool,
    pub level_end_text_suppressed: bool,
    pub recursive_pause_writer: bool,
    pub load_available: bool,
    pub sound_available: bool,
    pub music_available: bool,
    pub options_enabled_mask: u32,
    pub options_selected_mask: u32,
    pub sound_volume: u8,
    pub music_volume: u8,
    pub volume_target: u8,
    pub chat_recipient: u8,
    pub chat_mode: u8,
    pub chat_player_mask: u8,
    pub local_player_index: u8,
    pub frame_timing_index: u8,
    pub chat_lines: [[u8; 64]; 8],
    pub chat_names: [[u8; 16]; 8],
    pub notification_player_name: [u8; 16],
    pub cursor_visible: bool,
    pub mouse_x: i16,
    pub mouse_y: i16,
    pub joystick: i16,
    pub text_colour_phase: u8,
}

pub(crate) const LIVE_MENU_AUTH_PAUSE: u32 = 1 << 0;
pub(crate) const LIVE_MENU_AUTH_CHAT: u32 = 1 << 1;
pub(crate) const LIVE_MENU_AUTH_OPTIONS: u32 = 1 << 2;
pub(crate) const LIVE_MENU_AUTH_VOLUME: u32 = 1 << 3;
pub(crate) const LIVE_MENU_AUTH_OK_CANCEL: u32 = 1 << 4;
pub(crate) const LIVE_MENU_AUTH_POINTER: u32 = 1 << 5;
pub(crate) const LIVE_MENU_AUTH_NOTIFICATION_NAME: u32 = 1 << 6;
pub(crate) const LIVE_MENU_AUTH_DEBUG_METRICS: u32 = 1 << 7;
pub(crate) const LIVE_MENU_AUTH_ALL: u32 = (1 << 8) - 1;

impl Default for LiveMenuState {
    fn default() -> Self {
        Self {
            authority_mask: 0,
            selected_menu_item: 0,
            pause_menu_suppressed: false,
            pause_text_suppressed: false,
            level_end_text_suppressed: false,
            recursive_pause_writer: false,
            load_available: false,
            sound_available: false,
            music_available: false,
            options_enabled_mask: 0,
            options_selected_mask: 0,
            sound_volume: 0,
            music_volume: 0,
            volume_target: 0,
            chat_recipient: 0,
            chat_mode: 0,
            chat_player_mask: 0,
            local_player_index: 0,
            frame_timing_index: 0,
            chat_lines: [[0; 64]; 8],
            chat_names: [[0; 16]; 8],
            notification_player_name: [0; 16],
            cursor_visible: false,
            mouse_x: 0,
            mouse_y: 0,
            joystick: 0,
            text_colour_phase: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveDebugEntity {
    pub pointer_value: u32,
    pub source_slot: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub life: i32,
    pub max_life: i32,
    pub actual_speed: i16,
    pub min_speed: i16,
    pub max_speed: i16,
    pub position: Position,
    pub source_id: u16,
    pub parent_id: u16,
    pub mana: i32,
    pub max_mana: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PresentationBridgeQuery {
    pub minimap_enabled: bool,
    pub top_bar_enabled: bool,
    pub menu_overlay: u8,
    pub game_paused: bool,
    pub level_end: bool,
    pub objective_selector: Option<u8>,
    pub objective_textbox: bool,
    pub tutorial_or_briefing: bool,
    pub help_popup: bool,
    pub debug_flags: u8,
}

pub(crate) struct Level1PresentationLive<'a> {
    pub flight: FlightState,
    pub menu_state: u8,
    pub player_life: i32,
    pub player_immobilize_ticks: u16,
    pub terrain: &'a TerrainPlanes,
    pub hud: LiveHud,
    pub spell_menu: LiveSpellMenu,
    pub menu: LiveMenuState,
    pub debug_entity: LiveDebugEntity,
    pub minimap_entities: &'a [Level1MinimapEntityCommand],
    pub minimap_marks: Level1MinimapMarks,
    pub map_entities: &'a [Level1MinimapEntityCommand],
    pub map_marks: Level1MinimapMarks,
    pub objective_selector: u8,
    pub objective_ticks: u16,
    pub level_completed: bool,
    pub exit_sequence_active: bool,
    pub exit_display_blank: bool,
    pub exit_blur_mode: u8,
    pub notification_active: bool,
    pub notification_id: u16,
    pub notification_state: u8,
    pub notification_timer: i16,
    pub notification_player_name: [u8; 16],
    pub level_number: u16,
    pub player_turn: i32,
    pub sound_number: i16,
    pub game_timer_turn: i32,
    pub debug_fps: i32,
    pub active_entity_count: u16,
    pub heap_size: u32,
    pub memory_used: u32,
    pub memory_free: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationBridgePlan {
    pub overlay: Level1FinalMainState,
    pub notification_feedback: Option<(bool, u8, i16)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationBridgeError(pub String);

impl From<IndexedOverlayError> for PresentationBridgeError {
    fn from(error: IndexedOverlayError) -> Self {
        Self(error.0)
    }
}

impl std::fmt::Display for PresentationBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PresentationBridgeError {}

pub(crate) fn build_level1_final_main(
    assets: &IndexedOverlayAssets,
    live: Level1PresentationLive<'_>,
    query: PresentationBridgeQuery,
) -> Result<PresentationBridgePlan, PresentationBridgeError> {
    if !matches!(
        live.menu_state,
        0 | 3 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14
    ) {
        return Err(PresentationBridgeError(format!(
            "no fixed Level 1 source branch for menu state {}",
            live.menu_state
        )));
    }
    let player_alive = live.player_life >= 0;
    let map_mode = matches!(live.menu_state, 6 | 7 | 8 | 11 | 12 | 14);
    if !player_alive && map_mode {
        return Err(PresentationBridgeError(
            "dead-player source tree has no map-menu branch".to_owned(),
        ));
    }
    if query.level_end && !live.level_completed {
        return Err(PresentationBridgeError(
            "presentation query cannot override authoritative mission completion".to_owned(),
        ));
    }
    if query.tutorial_or_briefing {
        return Err(PresentationBridgeError(
            "tutorial/briefing is incomplete without the live sentinel-separated text stream"
                .to_owned(),
        ));
    }
    if query.help_popup {
        return Err(PresentationBridgeError(
            "recursive help popup is incomplete without cached E2A74 textbox state".to_owned(),
        ));
    }
    if player_alive && !map_mode && live.player_immobilize_ticks != 0 {
        return Err(PresentationBridgeError(
            "spider-web writer is active but its typed source sprite sheet is not cached"
                .to_owned(),
        ));
    }

    let mut commands = Vec::new();
    let mut notification_feedback = None;
    let mut stages = Vec::new();
    if !player_alive {
        push_dead_menu(assets, &live, query, &mut commands)?;
        stages.push(IndexedOverlayStage::DeadPlayerMenu);
    } else if map_mode {
        push_map_base(assets, &live, &mut commands)?;
        stages.extend([
            IndexedOverlayStage::MapClear,
            IndexedOverlayStage::MapTerrain,
            IndexedOverlayStage::MapEntities,
            IndexedOverlayStage::MapWorldViewport,
            IndexedOverlayStage::MapDividers,
            IndexedOverlayStage::MapMarks,
        ]);
        push_menu(assets, &live, query, true, &mut commands)?;
        stages.push(IndexedOverlayStage::Menu);
        if live.menu_state == 8 {
            push_bottom_spell_menu(assets, live.spell_menu, &mut commands)?;
            stages.push(IndexedOverlayStage::BottomSpellMenu);
        }
        notification_feedback = push_pause_or_end(assets, &live, query, true, &mut commands)?;
        stages.push(IndexedOverlayStage::PauseOrEndText);
        push_optional_spell(
            &mut commands,
            IndexedOverlayStage::LeftSpell,
            255,
            live.hud.left,
            live.hud,
            query.top_bar_enabled,
        );
        stages.push(IndexedOverlayStage::LeftSpell);
        push_optional_spell(
            &mut commands,
            IndexedOverlayStage::RightSpell,
            287,
            live.hud.right,
            live.hud,
            query.top_bar_enabled,
        );
        stages.push(IndexedOverlayStage::RightSpell);
    } else {
        push_decision(
            &mut commands,
            IndexedOverlayStage::SpiderWeb,
            u64::from(live.player_immobilize_ticks),
        );
        stages.push(IndexedOverlayStage::SpiderWeb);
        if !query.minimap_enabled {
            return Err(PresentationBridgeError(
                "ordinary source frame cannot omit its minimap writers".to_owned(),
            ));
        }
        commands.push(minimap_terrain_command(assets, &live)?);
        stages.push(IndexedOverlayStage::MinimapTerrain);
        push_minimap_entities(assets, &live, &mut commands)?;
        stages.push(IndexedOverlayStage::MinimapEntities);
        push_minimap_marks(assets, live.minimap_marks, &mut commands)?;
        stages.push(IndexedOverlayStage::MinimapMarks);
        if !query.top_bar_enabled {
            return Err(PresentationBridgeError(
                "ordinary source frame cannot omit its spell/status writers".to_owned(),
            ));
        }
        push_spell(
            &mut commands,
            IndexedOverlayStage::LeftSpell,
            255,
            live.hud.left,
            live.hud,
        );
        stages.push(IndexedOverlayStage::LeftSpell);
        push_spell(
            &mut commands,
            IndexedOverlayStage::RightSpell,
            287,
            live.hud.right,
            live.hud,
        );
        stages.push(IndexedOverlayStage::RightSpell);
        push_top_status(&mut commands, live.hud);
        stages.push(IndexedOverlayStage::TopStatus);
        push_menu(assets, &live, query, false, &mut commands)?;
        stages.push(IndexedOverlayStage::Menu);
        notification_feedback = push_pause_or_end(assets, &live, query, false, &mut commands)?;
        stages.push(IndexedOverlayStage::PauseOrEndText);
        if live.menu_state == 5 {
            push_bottom_spell_menu(assets, live.spell_menu, &mut commands)?;
            stages.push(IndexedOverlayStage::BottomSpellMenu);
        }
    }
    if let Some(requested) = query.objective_selector
        && requested != live.objective_selector
    {
        return Err(PresentationBridgeError(format!(
            "objective query selector {requested} does not match live selector {}",
            live.objective_selector
        )));
    }
    if query.objective_textbox && live.objective_ticks == 0 {
        return Err(PresentationBridgeError(
            "objective query cannot fabricate a textbox absent from live state".to_owned(),
        ));
    }
    let objective = live.objective_selector;
    if live.objective_ticks != 0 {
        if !(48..=53).contains(&objective) {
            return Err(PresentationBridgeError(format!(
                "objective selector {objective} is outside the Level 1 owner rows"
            )));
        }
        commands.push(IndexedOverlayCommand::Objective {
            stage: IndexedOverlayStage::ObjectiveTextbox,
            selector: objective,
        });
    } else {
        push_decision(
            &mut commands,
            IndexedOverlayStage::ObjectiveTextbox,
            u64::from(objective) << 16,
        );
    }
    stages.push(IndexedOverlayStage::ObjectiveTextbox);
    push_decision(&mut commands, IndexedOverlayStage::TutorialOrBriefing, 0);
    stages.push(IndexedOverlayStage::TutorialOrBriefing);
    push_decision(&mut commands, IndexedOverlayStage::HelpPopup, 0);
    stages.push(IndexedOverlayStage::HelpPopup);
    push_debug(assets, query.debug_flags, &live, &mut commands)?;
    stages.push(IndexedOverlayStage::DebugText);

    let overlay = Level1FinalMainState {
        profile: FixedOverlayProfile {
            menu_state: live.menu_state,
            player_alive,
            ..FixedOverlayProfile::default()
        },
        commands,
        required_stages: stages,
        prior_help_sentinel: 0,
        next_help_sentinel: u8::from(query.help_popup),
    };
    Ok(PresentationBridgePlan {
        overlay,
        notification_feedback,
    })
}

fn push_decision(
    commands: &mut Vec<IndexedOverlayCommand>,
    stage: IndexedOverlayStage,
    state_hash: u64,
) {
    commands.push(IndexedOverlayCommand::Decision { stage, state_hash });
}

fn require_menu_authority(
    menu: LiveMenuState,
    required: u32,
    family: &str,
) -> Result<(), PresentationBridgeError> {
    if menu.authority_mask & required == required {
        Ok(())
    } else {
        Err(PresentationBridgeError(format!(
            "{family} writer is active without authoritative production live state"
        )))
    }
}

fn push_optional_spell(
    commands: &mut Vec<IndexedOverlayCommand>,
    stage: IndexedOverlayStage,
    x: i16,
    slot: LiveSpellSlot,
    hud: LiveHud,
    visible: bool,
) {
    if visible {
        push_spell(commands, stage, x, slot, hud);
    } else {
        push_decision(commands, stage, u64::from(slot.holder_entity_index));
    }
}

fn push_dead_menu(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    query: PresentationBridgeQuery,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let stage = IndexedOverlayStage::DeadPlayerMenu;
    push_menu_writer(assets, live, query, false, stage, commands)
}

fn push_menu(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    query: PresentationBridgeQuery,
    map_mode: bool,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let stage = IndexedOverlayStage::Menu;
    push_menu_writer(assets, live, query, map_mode, stage, commands)
}

fn push_menu_writer(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    query: PresentationBridgeQuery,
    map_mode: bool,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let menu_state = live.menu_state;
    push_decision(
        commands,
        stage,
        u64::from(menu_state)
            | (u64::from(map_mode) << 8)
            | (u64::from(query.menu_overlay) << 16)
            | (u64::from(query.game_paused) << 24),
    );
    match menu_state {
        0 | 5 | 6 | 8 => {
            push_pause_menu(assets, live.menu, query.game_paused, stage, commands)?;
        }
        3 => {
            push_chat_menu(assets, live.menu, stage, commands)?;
            push_pause_menu(assets, live.menu, query.game_paused, stage, commands)?;
        }
        7 => {
            push_sorcerer_scores(assets, live, stage, commands)?;
        }
        9 | 11 => {
            push_options_menu(assets, live.menu, stage, commands)?;
        }
        10 | 12 => {
            push_volume_menu(assets, live.menu, stage, commands)?;
            push_pause_menu(assets, live.menu, query.game_paused, stage, commands)?;
        }
        13 | 14 => {
            push_ok_cancel_menu(assets, live, map_mode, stage, commands)?;
        }
        other => {
            return Err(PresentationBridgeError(format!(
                "no typed source menu writer for state {other}"
            )));
        }
    }
    Ok(())
}

fn source_cursor_visible(menu: LiveMenuState) -> bool {
    menu.cursor_visible && matches!(menu.joystick, 4 | 5 | 6 | 8 | 11 | 12 | 13)
}

fn push_cursor(
    stage: IndexedOverlayStage,
    menu: LiveMenuState,
    _commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    if source_cursor_visible(menu) {
        require_menu_authority(menu, LIVE_MENU_AUTH_POINTER, "cursor")?;
        return Err(PresentationBridgeError(format!(
            "{stage:?} cursor is active but the source selector-9 bank indirection is not proven"
        )));
    }
    Ok(())
}

fn push_blend_rect(
    stage: IndexedOverlayStage,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    blend_index: u8,
    commands: &mut Vec<IndexedOverlayCommand>,
) {
    let pixels = usize::from(width) * usize::from(height);
    commands.push(IndexedOverlayCommand::BlendPixels {
        stage,
        x,
        y,
        width,
        height,
        blend_indices: vec![blend_index; pixels].into_boxed_slice(),
        mask: vec![1; pixels].into_boxed_slice(),
    });
}

fn push_pause_menu(
    assets: &IndexedOverlayAssets,
    menu: LiveMenuState,
    game_paused: bool,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    if !game_paused {
        return Ok(());
    }
    require_menu_authority(menu, LIVE_MENU_AUTH_PAUSE | LIVE_MENU_AUTH_POINTER, "pause")?;
    if menu.recursive_pause_writer {
        return Err(PresentationBridgeError(
            "pause menu recursive bit0x10 writer lacks typed sub_30870 state".to_owned(),
        ));
    }
    if !menu.pause_menu_suppressed {
        let x = 134;
        let mut y = 38;
        let (_save_width, row_height) = assets
            .bitmap_extent(174)
            .ok_or_else(|| PresentationBridgeError("pause selector 174 missing".to_owned()))?;
        commands.push(bitmap(stage, 174, x, y));
        y += i16::try_from(row_height).expect("pause height fits i16");
        commands.push(bitmap(stage, 175, x, y));
        let shade = assets
            .palette_index(0)
            .ok_or_else(|| PresentationBridgeError("CLRN[0] missing".to_owned()))?;
        if !menu.load_available {
            for _ in 0..2 {
                push_blend_rect(stage, x, y, 51, row_height, shade, commands);
            }
        }
        y += i16::try_from(row_height).expect("pause height fits i16");
        commands.push(bitmap(stage, 176, x, y));
        let (sound_width, sound_height) = assets
            .bitmap_extent(176)
            .ok_or_else(|| PresentationBridgeError("pause selector 176 missing".to_owned()))?;
        if !menu.sound_available {
            for _ in 0..2 {
                push_blend_rect(stage, x, y, sound_width, sound_height, shade, commands);
            }
        }
        let (music_width, music_height) = assets
            .bitmap_extent(177)
            .ok_or_else(|| PresentationBridgeError("pause selector 177 missing".to_owned()))?;
        let music_x = 160;
        commands.push(bitmap(stage, 177, music_x, y));
        if !menu.music_available {
            for _ in 0..2 {
                push_blend_rect(
                    stage,
                    music_x,
                    y,
                    music_width,
                    music_height,
                    shade,
                    commands,
                );
            }
        }
        y += i16::try_from(row_height).expect("pause height fits i16");
        commands.push(bitmap(stage, 178, x, y));
    }
    push_cursor(stage, menu, commands)?;
    Ok(())
}

fn push_options_menu(
    assets: &IndexedOverlayAssets,
    menu: LiveMenuState,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    require_menu_authority(
        menu,
        LIVE_MENU_AUTH_OPTIONS | LIVE_MENU_AUTH_POINTER,
        "options",
    )?;
    const ON: [u16; 17] = [
        400, 390, 392, 469, 398, 7, 382, 13, 359, 363, 357, 395, 365, 441, 442, 468, 402,
    ];
    const OFF: [u16; 17] = [
        401, 391, 393, 470, 399, 7, 381, 14, 360, 364, 358, 396, 366, 440, 443, 468, 403,
    ];
    let selected_rows = std::array::from_fn::<_, 17, _>(|ordinal| {
        let bit = 1_u32 << ordinal;
        if menu.options_selected_mask & bit != 0 && menu.options_enabled_mask & bit != 0 {
            ON[ordinal]
        } else {
            OFF[ordinal]
        }
    });
    // byteindex_186 is initialized from the longest frozen English option to
    // source width 202 and is not recomputed from the currently selected rows.
    let width = 101_i16;
    let x = 109_i16;
    let (left_width, _) = assets
        .bitmap_extent(168)
        .ok_or_else(|| PresentationBridgeError("options selector 168 missing".to_owned()))?;
    let (middle_width, _) = assets
        .bitmap_extent(169)
        .ok_or_else(|| PresentationBridgeError("options selector 169 missing".to_owned()))?;
    let (right_width, _) = assets
        .bitmap_extent(170)
        .ok_or_else(|| PresentationBridgeError("options selector 170 missing".to_owned()))?;
    let right_x = x + width - i16::try_from(right_width).expect("panel width fits i16");
    let normal = assets
        .palette_index(4095)
        .ok_or_else(|| PresentationBridgeError("CLRN[4095] missing".to_owned()))?;
    let hover = assets
        .palette_index(3840)
        .ok_or_else(|| PresentationBridgeError("CLRN[3840] missing".to_owned()))?;
    let unavailable = assets
        .palette_index(0x888)
        .ok_or_else(|| PresentationBridgeError("CLRN[0x888] missing".to_owned()))?;
    for (ordinal, selector) in selected_rows.into_iter().enumerate() {
        let y = 33 + i16::try_from(ordinal * 9).expect("options y fits i16");
        commands.push(bitmap(stage, 168, x, y));
        let mut tile_x = x + i16::try_from(left_width).expect("panel width fits i16");
        while tile_x < right_x {
            commands.push(bitmap(stage, 169, tile_x, y));
            tile_x += i16::try_from(middle_width).expect("panel width fits i16");
        }
        commands.push(bitmap(stage, 170, right_x, y));
        let text = assets
            .localization(selector)
            .expect("options localization was validated");
        let text_x = 160 - 2 * i16::try_from(text.len()).expect("option text fits i16");
        let source_mouse_x = menu.mouse_x / 2;
        let source_mouse_y = menu.mouse_y / 2;
        let bit = 1_u32 << ordinal;
        let colour = if menu.options_enabled_mask & bit == 0 {
            unavailable
        } else if (x..x + width).contains(&source_mouse_x) && (y..y + 9).contains(&source_mouse_y) {
            hover
        } else {
            normal
        };
        commands.push(IndexedOverlayCommand::LocalizedText {
            stage,
            selector,
            x: text_x,
            y: y + 1,
            colour,
        });
    }
    let okay_x = 139_i16;
    let okay_y = 188;
    commands.push(bitmap(stage, 168, okay_x, okay_y));
    for tile_x in [147_i16, 153, 159, 165, 171] {
        commands.push(bitmap(stage, 169, tile_x, okay_y));
    }
    let okay_right = 172_i16;
    commands.push(bitmap(stage, 170, okay_right, okay_y));
    let okay_hover = (okay_x
        ..okay_right + i16::try_from(right_width).expect("panel width fits i16"))
        .contains(&(menu.mouse_x / 2))
        && (okay_y..okay_y + 9).contains(&(menu.mouse_y / 2));
    commands.push(IndexedOverlayCommand::Text {
        stage,
        bytes: Box::from(b"OK".as_slice()),
        x: 156,
        y: 189,
        colour: if okay_hover { hover } else { normal },
    });
    push_cursor(stage, menu, commands)?;
    Ok(())
}

fn push_volume_menu(
    assets: &IndexedOverlayAssets,
    menu: LiveMenuState,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    require_menu_authority(menu, LIVE_MENU_AUTH_VOLUME, "volume")?;
    let x = 134;
    let y = 102;
    let width = 51_u16;
    for (rx, ry, rw, rh, colour) in [
        (x, y, width, 12, 0x33),
        (x, y, width, 1, 0x11),
        (x, y + 11, width, 1, 0x3b),
        (x, y, 1, 11, 0x11),
        (x + 50, y, 1, 12, 0x3b),
        (x + 2, y + 2, 47, 8, 0x33),
        (x + 2, y + 2, 47, 1, 0x3b),
        (x + 2, y + 9, 47, 1, 0x11),
        (x + 2, y + 2, 1, 7, 0x3b),
        (x + 48, y + 2, 1, 8, 0x11),
    ] {
        commands.push(IndexedOverlayCommand::Rect {
            stage,
            x: rx,
            y: ry,
            width: rw,
            height: rh,
            colour,
        });
    }
    let black = assets
        .palette_index(0)
        .ok_or_else(|| PresentationBridgeError("CLRN[0] missing".to_owned()))?;
    let fill = assets
        .palette_index(240)
        .ok_or_else(|| PresentationBridgeError("CLRN[240] missing".to_owned()))?;
    commands.push(IndexedOverlayCommand::Rect {
        stage,
        x: x + 3,
        y: y + 3,
        width: 45,
        height: 6,
        colour: black,
    });
    let volume = match menu.volume_target {
        1 => menu.sound_volume,
        2 => menu.music_volume,
        target => {
            return Err(PresentationBridgeError(format!(
                "volume target {target} is outside source sound/music cases"
            )));
        }
    };
    let source_fill = u16::from(volume) * 90 / 127;
    if source_fill != 0 {
        commands.push(IndexedOverlayCommand::Rect {
            stage,
            x: x + 3,
            y: y + 3,
            width: source_fill / 2,
            height: 6,
            colour: fill,
        });
    }
    Ok(())
}

fn push_ok_cancel_menu(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    map_mode: bool,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    require_menu_authority(
        live.menu,
        LIVE_MENU_AUTH_OK_CANCEL | LIVE_MENU_AUTH_POINTER,
        "OK/cancel",
    )?;
    let selector = match live.menu.selected_menu_item {
        0 => return Ok(()),
        1 => 2,
        2 => 423,
        3 => 424,
        item => {
            return Err(PresentationBridgeError(format!(
                "OK/cancel selected item {item} is outside 1..3"
            )));
        }
    };
    let mut text = assets
        .localization(selector)
        .ok_or_else(|| PresentationBridgeError(format!("localization {selector} missing")))?
        .to_vec();
    text.push(b'?');
    let colour_source = 3840 * usize::from(live.menu.text_colour_phase);
    let colour = assets
        .palette_index(colour_source)
        .ok_or_else(|| PresentationBridgeError("OK/cancel colour missing".to_owned()))?;
    commands.push(IndexedOverlayCommand::Text {
        stage,
        bytes: text.into_boxed_slice(),
        x: if map_mode { 3 } else { 66 },
        y: if map_mode { 3 } else { 25 },
        colour,
    });
    commands.push(bitmap(stage, 257, 135, 52));
    commands.push(bitmap(stage, 258, 160, 52));
    push_cursor(stage, live.menu, commands)?;
    Ok(())
}

fn push_chat_menu(
    assets: &IndexedOverlayAssets,
    menu: LiveMenuState,
    stage: IndexedOverlayStage,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    require_menu_authority(menu, LIVE_MENU_AUTH_CHAT | LIVE_MENU_AUTH_POINTER, "chat")?;
    let normal = assets
        .palette_index(4095)
        .ok_or_else(|| PresentationBridgeError("CLRN[4095] missing".to_owned()))?;
    let black = assets
        .palette_index(0)
        .ok_or_else(|| PresentationBridgeError("CLRN[0] missing".to_owned()))?;
    let pulse = assets
        .palette_index(3840)
        .ok_or_else(|| PresentationBridgeError("CLRN[3840] missing".to_owned()))?;
    for (x, y, width, height, colour) in [
        (0, 164, 320, 36, 0x33),
        (0, 164, 320, 1, 0x11),
        (0, 199, 320, 1, 0x3b),
        (0, 164, 1, 35, 0x11),
        (319, 164, 1, 36, 0x3b),
        (2, 166, 186, 10, 0),
        (2, 166, 186, 1, 0x3b),
        (2, 175, 186, 1, 0x11),
        (2, 166, 1, 9, 0x3b),
        (187, 166, 1, 10, 0x11),
    ] {
        commands.push(IndexedOverlayCommand::Rect {
            stage,
            x,
            y,
            width,
            height,
            colour,
        });
    }
    let mouse_x = menu.mouse_x / 2;
    let mouse_y = menu.mouse_y / 2;
    let active_recipient = if (2..34).contains(&mouse_x) && (178..198).contains(&mouse_y) {
        u8::try_from(4 * ((mouse_y - 178) / 10) + (mouse_x - 2) / 8)
            .expect("chat recipient grid fits u8")
    } else {
        menu.chat_recipient
    };
    if active_recipient >= 8 {
        return Err(PresentationBridgeError(format!(
            "chat recipient {active_recipient} is outside the eight source buffers"
        )));
    }
    let chat_line = bytes_until_zero(&menu.chat_lines[usize::from(active_recipient)]);
    commands.push(IndexedOverlayCommand::Text {
        stage,
        bytes: chat_line.to_vec().into_boxed_slice(),
        x: 5,
        y: 167,
        colour: normal,
    });
    commands.push(IndexedOverlayCommand::Rect {
        stage,
        x: 5 + 4 * i16::try_from(chat_line.len()).expect("chat text fits i16"),
        y: 168,
        width: 3,
        height: 6,
        colour: if menu.frame_timing_index & 8 != 0 {
            pulse
        } else {
            0
        },
    });
    for recipient in 0..8_u8 {
        let x = 2 + 8 * i16::from(recipient % 4);
        let y = 178 + 10 * i16::from(recipient / 4);
        commands.push(bitmap(
            stage,
            if recipient == active_recipient {
                155
            } else {
                158
            },
            x,
            y,
        ));
        commands.push(IndexedOverlayCommand::Text {
            stage,
            bytes: (recipient + 1).to_string().into_bytes().into_boxed_slice(),
            x: x + 2,
            y: y + 1,
            colour: black,
        });
    }
    let mode_x = [41_i16, 76, 111, 155];
    for mode in 0..4_u8 {
        let x = mode_x[usize::from(mode)];
        commands.push(bitmap(
            stage,
            if mode == menu.chat_mode { 156 } else { 159 },
            x,
            180,
        ));
        commands.push(bitmap(stage, 154 - u16::from(mode), x + 1, 181));
    }
    let mut hovered = None;
    let mut display_index = 0_i16;
    for player in 0..8_u8 {
        if player == menu.local_player_index {
            continue;
        }
        let x = 192 + 17 * display_index;
        display_index += 1;
        let selected = menu.chat_mode == 3 && menu.chat_player_mask & (1 << player) != 0;
        commands.push(bitmap(stage, if selected { 157 } else { 160 }, x, 177));
        commands.push(bitmap(stage, 74 + u16::from(player), x + 1, 178));
        if (x..x + 16).contains(&(menu.mouse_x / 2)) && (177..188).contains(&(menu.mouse_y / 2)) {
            hovered = Some(player);
        }
    }
    commands.push(bitmap(stage, 259, 192, 166));
    commands.push(bitmap(stage, 261, 237, 166));
    commands.push(bitmap(stage, 260, 282, 166));
    for (x, y, width, height, colour) in [
        (192, 189, 118, 9, 0),
        (192, 189, 118, 1, 0x3b),
        (192, 197, 118, 1, 0x11),
        (192, 189, 1, 8, 0x3b),
        (309, 189, 1, 9, 0x11),
    ] {
        commands.push(IndexedOverlayCommand::Rect {
            stage,
            x,
            y,
            width,
            height,
            colour,
        });
    }
    if let Some(player) = hovered {
        commands.push(IndexedOverlayCommand::Text {
            stage,
            bytes: bytes_until_zero(&menu.chat_names[usize::from(player)])
                .to_vec()
                .into_boxed_slice(),
            x: 195,
            y: 190,
            colour: assets
                .palette_index(240)
                .ok_or_else(|| PresentationBridgeError("CLRN[240] missing".to_owned()))?,
        });
    }
    push_cursor(stage, menu, commands)?;
    Ok(())
}

fn push_sorcerer_scores(
    _assets: &IndexedOverlayAssets,
    _live: &Level1PresentationLive<'_>,
    _stage: IndexedOverlayStage,
    _commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    Err(PresentationBridgeError(
        "score writer is incomplete without the live all-player matrix and help/RNG state"
            .to_owned(),
    ))
}

fn bytes_until_zero(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    &bytes[..end]
}

fn format_single_percent_s(
    template: &[u8],
    argument: &[u8],
) -> Result<Vec<u8>, PresentationBridgeError> {
    let Some(marker) = template.windows(2).position(|window| window == b"%s") else {
        return Err(PresentationBridgeError(
            "ability notification template has no %s marker".to_owned(),
        ));
    };
    if template[marker + 2..]
        .windows(2)
        .any(|window| window == b"%s")
    {
        return Err(PresentationBridgeError(
            "ability notification template has multiple %s markers".to_owned(),
        ));
    }
    let mut formatted = Vec::with_capacity(template.len() - 2 + argument.len());
    formatted.extend_from_slice(&template[..marker]);
    formatted.extend_from_slice(argument);
    formatted.extend_from_slice(&template[marker + 2..]);
    Ok(formatted)
}

fn push_pause_or_end(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    query: PresentationBridgeQuery,
    map_mode: bool,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<Option<(bool, u8, i16)>, PresentationBridgeError> {
    let stage = IndexedOverlayStage::PauseOrEndText;
    let state_hash = u64::from(query.game_paused)
        | (u64::from(live.level_completed) << 8)
        | (u64::from(live.exit_sequence_active) << 9)
        | (u64::from(live.exit_display_blank) << 11)
        | (u64::from(live.exit_blur_mode) << 12)
        | (u64::from(live.notification_active) << 10)
        | (u64::from(live.notification_state) << 16)
        | (u64::from(live.notification_timer.cast_unsigned()) << 24)
        | (u64::from(live.notification_id) << 40);
    push_decision(commands, stage, state_hash);
    if !query.game_paused && !live.level_completed && !live.notification_active {
        return Ok(None);
    }
    require_menu_authority(live.menu, LIVE_MENU_AUTH_PAUSE, "pause/end")?;
    if live.menu.pause_text_suppressed || live.menu.selected_menu_item != 0 {
        return Ok(None);
    }
    let origin_x = if map_mode { 3 } else { 66 };
    let origin_y = if map_mode { 3 } else { 25 };
    let mut x = origin_x;
    let mut y = origin_y;
    if query.game_paused {
        commands.push(IndexedOverlayCommand::LocalizedText {
            stage,
            selector: 425,
            x,
            y,
            colour: assets
                .palette_index(240)
                .ok_or_else(|| PresentationBridgeError("CLRN[240] missing".to_owned()))?,
        });
        let pause_len = assets
            .localization(425)
            .ok_or_else(|| PresentationBridgeError("localization 425 missing".to_owned()))?
            .len();
        x += 4 * i16::try_from(pause_len + 2).expect("paused text width fits i16");
    }
    if live.level_completed && !live.menu.level_end_text_suppressed {
        let mut text = assets
            .localization(378)
            .ok_or_else(|| PresentationBridgeError("localization 378 missing".to_owned()))?
            .to_vec();
        text.extend_from_slice(b". ");
        text.extend_from_slice(
            assets
                .localization(379)
                .ok_or_else(|| PresentationBridgeError("localization 379 missing".to_owned()))?,
        );
        text.push(b'.');
        commands.push(IndexedOverlayCommand::Text {
            stage,
            bytes: text.into_boxed_slice(),
            x,
            y,
            colour: assets
                .palette_index(3840)
                .ok_or_else(|| PresentationBridgeError("CLRN[3840] missing".to_owned()))?,
        });
        x = origin_x;
        y += 7;
    }
    let notification_feedback = if live.notification_active {
        require_menu_authority(live.menu, LIVE_MENU_AUTH_NOTIFICATION_NAME, "notification")?;
        if !(1..=5).contains(&live.notification_state) {
            return Err(PresentationBridgeError(format!(
                "notification state {} is outside source cases 1..5",
                live.notification_state
            )));
        }
        let next_timer = live.notification_timer.wrapping_sub(1);
        if live.notification_timer <= 0 {
            return Ok(Some((false, 0, next_timer)));
        }
        let name = bytes_until_zero(&live.notification_player_name);
        let notification_body = assets.localization(live.notification_id).ok_or_else(|| {
            PresentationBridgeError(format!(
                "notification localization {} missing",
                live.notification_id
            ))
        })?;
        let body = if live.notification_state == ABILITY_IMPROVED_NOTIFICATION_STATE
            && (ABILITY_IMPROVED_NAME_SELECTOR_BASE..ABILITY_IMPROVED_NAME_SELECTOR_BASE + 26)
                .contains(&live.notification_id)
        {
            let template = assets
                .localization(ABILITY_IMPROVED_TEMPLATE_SELECTOR)
                .ok_or_else(|| {
                    PresentationBridgeError(format!(
                        "ability notification template {} missing",
                        ABILITY_IMPROVED_TEMPLATE_SELECTOR
                    ))
                })?;
            format_single_percent_s(template, notification_body)?
        } else {
            notification_body.to_vec()
        };
        let mut text = Vec::new();
        let bracketed = matches!(live.notification_state, 2 | 4);
        if bracketed {
            text.push(b'[');
            text.extend_from_slice(name);
            text.extend_from_slice(b"] ");
        } else if live.notification_state == 1 {
            text.extend_from_slice(name);
            text.push(b' ');
        }
        text.extend_from_slice(&body);
        let colour = match live.notification_state {
            1 | 3 | 5 => assets.palette_index(3840),
            2 if next_timer <= 100 => assets.palette_index(3840),
            2 => assets.palette_index(3840 * usize::from(live.hud.color_phase_2)),
            4 => assets.palette_index(240 * usize::from(live.hud.color_phase_2)),
            _ => None,
        }
        .ok_or_else(|| PresentationBridgeError("notification CLRN index missing".to_owned()))?;
        commands.push(IndexedOverlayCommand::Text {
            stage,
            bytes: text.into_boxed_slice(),
            x,
            y,
            colour,
        });
        Some((true, live.notification_state, next_timer))
    } else {
        None
    };
    Ok(notification_feedback)
}

fn push_map_base(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    commands.push(IndexedOverlayCommand::Rect {
        stage: IndexedOverlayStage::MapClear,
        x: 0,
        y: 0,
        width: 320,
        height: 200,
        colour: 0,
    });
    commands.push(map_terrain_command(assets, live)?);
    push_map_entities(assets, live, commands)?;
    commands.push(IndexedOverlayCommand::SourceViewport {
        stage: IndexedOverlayStage::MapWorldViewport,
        source_x: 192,
        source_y: 0,
        x: 192,
        y: 0,
        width: 128,
        height: 200,
    });
    commands.push(IndexedOverlayCommand::Line {
        stage: IndexedOverlayStage::MapDividers,
        from_x: 192,
        from_y: 0,
        to_x: 192,
        to_y: 199,
        colour: assets.palette_index(0).ok_or_else(|| {
            PresentationBridgeError("CLRN[0] map divider colour is absent".to_owned())
        })?,
    });
    commands.push(IndexedOverlayCommand::Line {
        stage: IndexedOverlayStage::MapDividers,
        from_x: 191,
        from_y: 0,
        to_x: 191,
        to_y: 199,
        colour: assets.palette_index(0).ok_or_else(|| {
            PresentationBridgeError("CLRN[0] map divider colour is absent".to_owned())
        })?,
    });
    push_map_marks(assets, live.map_marks, commands)?;
    Ok(())
}

fn map_terrain_command(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
) -> Result<IndexedOverlayCommand, PresentationBridgeError> {
    const WIDTH: usize = 191;
    const HEIGHT: usize = 200;
    const SCALING: i32 = 408;
    let sine = assets.minimap_sine();
    let angle = usize::from(live.flight.yaw.cast_unsigned() & 0x07ff);
    let scaled_sin = ((i64::from(SCALING) * i64::from(sine[angle])) >> 16) as i32;
    let scaled_cos = ((i64::from(SCALING) * i64::from(sine[angle + 0x200])) >> 16) as i32;
    let row_step_x = (HEIGHT as i32 * scaled_sin) / WIDTH as i32;
    let column_step_x = (HEIGHT as i32 * scaled_cos) / WIDTH as i32;
    let mut row_x = i32::from(live.flight.position.x)
        - (WIDTH as i32 * column_step_x - HEIGHT as i32 * scaled_sin) / 2;
    let mut row_y = i32::from(live.flight.position.y)
        - (HEIGHT as i32 * scaled_cos + WIDTH as i32 * row_step_x) / 2;
    let mut indices = vec![0_u8; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        let mut source_x = row_x;
        let mut source_y = row_y;
        for x in 0..WIDTH {
            let tile_x = ((source_x >> 8) & 255) as usize;
            let tile_y = ((source_y >> 8) & 255) as usize;
            let tile = tile_y * 256 + tile_x;
            indices[y * WIDTH + x] = assets.terrain_shade_index(
                live.terrain.terrain_types[tile],
                live.terrain.shading[tile],
            )?;
            source_x = source_x.wrapping_add(column_step_x);
            source_y = source_y.wrapping_add(row_step_x);
        }
        row_x = row_x.wrapping_sub(scaled_sin);
        row_y = row_y.wrapping_add(scaled_cos);
    }
    Ok(IndexedOverlayCommand::Pixels {
        stage: IndexedOverlayStage::MapTerrain,
        x: 0,
        y: 0,
        width: WIDTH as u16,
        height: HEIGHT as u16,
        indices: indices.into_boxed_slice(),
        mask: vec![1; WIDTH * HEIGHT].into_boxed_slice(),
    })
}

fn push_map_entities(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    for command in live.map_entities {
        match command.kind {
            LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL => {
                commands.push(IndexedOverlayCommand::BlendPixel {
                    stage: IndexedOverlayStage::MapEntities,
                    x: command.x,
                    y: command.y,
                    blend_index: command.palette_index,
                });
            }
            LEVEL1_MINIMAP_COMMAND_DOT => {
                commands.push(IndexedOverlayCommand::Rect {
                    stage: IndexedOverlayStage::MapEntities,
                    x: command.x,
                    y: command.y,
                    width: u16::from(command.size.max(1)),
                    height: u16::from(command.size.max(1)),
                    colour: command.palette_index,
                });
            }
            LEVEL1_MINIMAP_COMMAND_SPRITE => {
                let (width, height) = assets.bitmap_extent(command.selector).ok_or_else(|| {
                    PresentationBridgeError(format!(
                        "live map selector {} is absent from CP1 bank",
                        command.selector
                    ))
                })?;
                let left = if command.anchor == LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT {
                    command.x
                } else {
                    command.x - i16::try_from(width / 2).expect("bitmap width fits i16")
                };
                let top = if command.selector == 83 || command.selector == 84 {
                    command.y - i16::try_from(height / 2).expect("bitmap height fits i16")
                } else {
                    command.y - i16::try_from(height).expect("bitmap height fits i16")
                };
                commands.push(bitmap(
                    IndexedOverlayStage::MapEntities,
                    command.selector,
                    left,
                    top,
                ));
            }
            kind => {
                return Err(PresentationBridgeError(format!(
                    "live map command kind {kind} is not typed"
                )));
            }
        }
    }
    let centre = (94_i16, 100_i16);
    for arm in 0..=15 {
        for (x, y) in [
            (centre.0 - arm, centre.1),
            (centre.0 + arm, centre.1),
            (centre.0, centre.1 - arm),
            (centre.0, centre.1 + arm),
        ] {
            commands.push(IndexedOverlayCommand::BlendPixel {
                stage: IndexedOverlayStage::MapEntities,
                x,
                y,
                blend_index: assets.palette_index(4095).ok_or_else(|| {
                    PresentationBridgeError("CLRN[4095] map cross colour is absent".to_owned())
                })?,
            });
        }
    }
    Ok(())
}

fn push_map_marks(
    assets: &IndexedOverlayAssets,
    marks: Level1MinimapMarks,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    push_marks(assets, marks, IndexedOverlayStage::MapMarks, 1, commands)
}

fn push_debug(
    assets: &IndexedOverlayAssets,
    flags: u8,
    live: &Level1PresentationLive<'_>,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let stage = IndexedOverlayStage::DebugText;
    push_decision(commands, stage, u64::from(flags));
    if flags & !0x48 != 0 {
        return Err(PresentationBridgeError(format!(
            "debug flags 0x{flags:02x} contain an untyped source branch"
        )));
    }
    let label_colour = assets
        .palette_index(3840)
        .ok_or_else(|| PresentationBridgeError("CLRN[3840] missing".to_owned()))?;
    let value_colour = assets
        .palette_index(15)
        .ok_or_else(|| PresentationBridgeError("CLRN[15] missing".to_owned()))?;
    let selected_value_colour = assets
        .palette_index(255)
        .ok_or_else(|| PresentationBridgeError("CLRN[255] missing".to_owned()))?;
    let mut draw = |text: String, x: i16, y: i16, colour: u8| {
        commands.push(IndexedOverlayCommand::Text {
            stage,
            bytes: text.into_bytes().into_boxed_slice(),
            x,
            y,
            colour,
        });
    };
    if flags & 8 != 0 {
        require_menu_authority(live.menu, LIVE_MENU_AUTH_DEBUG_METRICS, "debug metrics")?;
        let rows = [
            ("Product name".to_owned(), label_colour),
            ("Magic Carpet 2 (Netherworlds)".to_owned(), value_colour),
            ("Version number".to_owned(), label_colour),
            ("Beta".to_owned(), value_colour),
            ("Version date".to_owned(), label_colour),
            ("03:42:13 Sep 06 1995".to_owned(), value_colour),
            ("Programmer".to_owned(), label_colour),
            ("Bullfrog, Alan Wright.".to_owned(), value_colour),
            ("Supplied to".to_owned(), label_colour),
            ("PUBLIC".to_owned(), value_colour),
            ("Level Number".to_owned(), label_colour),
            (live.level_number.to_string(), value_colour),
            ("FPS".to_owned(), label_colour),
            (live.debug_fps.to_string(), value_colour),
            ("Sound Number".to_owned(), label_colour),
            (live.sound_number.to_string(), value_colour),
            ("Game turn".to_owned(), label_colour),
            (
                format!("{} {}", live.player_turn, live.game_timer_turn),
                value_colour,
            ),
            ("Thing".to_owned(), label_colour),
            (
                format!("Thing 168, Active {}", live.active_entity_count),
                value_colour,
            ),
            ("Carpet 224791".to_owned(), value_colour),
            ("Tape 38547".to_owned(), value_colour),
            (format!("Heap {}", live.heap_size), value_colour),
            ("Memory (Used/Free)".to_owned(), label_colour),
            (
                format!("{}/{}", live.memory_used, live.memory_free),
                value_colour,
            ),
        ];
        for (ordinal, (text, colour)) in rows.into_iter().enumerate() {
            draw(
                text,
                160,
                i16::try_from(ordinal * 7).expect("debug rows fit i16"),
                colour,
            );
        }
    }
    if flags & 0x40 != 0 {
        let entity = live.debug_entity;
        let rows = [
            ("THING".to_owned(), label_colour),
            (
                format!("{} / {}", entity.pointer_value, entity.source_slot),
                selected_value_colour,
            ),
            ("CLASS / MODEL / STATE".to_owned(), label_colour),
            (
                format!(
                    "{} / {} / {}",
                    entity.class.cast_signed(),
                    entity.model.cast_signed(),
                    entity.action
                ),
                selected_value_colour,
            ),
            ("LIFE / MAX LIFE".to_owned(), label_colour),
            (
                format!("{} / {}", entity.life, entity.max_life),
                selected_value_colour,
            ),
            ("SPEED ACTUAL / MINIMUM / MAXIMUM".to_owned(), label_colour),
            (
                format!(
                    "{} / {} / {}",
                    entity.actual_speed, entity.min_speed, entity.max_speed
                ),
                selected_value_colour,
            ),
            ("ACTUAL X / Y / Z".to_owned(), label_colour),
            (
                format!(
                    "{} / {} / {}",
                    entity.position.x,
                    entity.position.y,
                    entity.position.z.cast_unsigned()
                ),
                selected_value_colour,
            ),
            ("ID / WHO OWNS ME".to_owned(), label_colour),
            (
                format!("{} / {}", entity.source_id, entity.parent_id),
                selected_value_colour,
            ),
            ("MANA ACTUAL / MAXIMUM".to_owned(), label_colour),
            (
                format!("{} / {}", entity.mana, entity.max_mana),
                selected_value_colour,
            ),
        ];
        for (ordinal, (text, colour)) in rows.into_iter().enumerate() {
            draw(
                text,
                5,
                42 + i16::try_from(ordinal * 7).expect("debug rows fit i16"),
                colour,
            );
        }
    }
    Ok(())
}

fn minimap_terrain_command(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
) -> Result<IndexedOverlayCommand, PresentationBridgeError> {
    const SOURCE_SIDE: usize = 128;
    const FIXED_SIDE: usize = 64;
    const SCALING: i32 = 256;
    let sine = assets.minimap_sine();
    if sine.len() < 2_560 {
        return Err(PresentationBridgeError(format!(
            "minimap sine table has {} entries",
            sine.len()
        )));
    }
    if live.terrain.terrain_types.len() != 65_536 || live.terrain.shading.len() != 65_536 {
        return Err(PresentationBridgeError(
            "live minimap terrain planes are not 256x256".to_owned(),
        ));
    }
    let angle = usize::from(live.flight.yaw.cast_unsigned() & 0x07ff);
    let sine_scaled = ((i64::from(SCALING) * i64::from(sine[angle])) >> 16) as i32;
    let cosine_scaled = ((i64::from(SCALING) * i64::from(sine[angle + 0x200])) >> 16) as i32;
    let origin_x = i32::from(live.flight.position.x)
        - (SOURCE_SIDE as i32 * cosine_scaled - SOURCE_SIDE as i32 * sine_scaled) / 2;
    let origin_y = i32::from(live.flight.position.y)
        - (SOURCE_SIDE as i32 * cosine_scaled + SOURCE_SIDE as i32 * sine_scaled) / 2;
    let aperture = assets.minimap_aperture();
    let mut terrain = vec![0_u8; FIXED_SIDE * FIXED_SIDE];
    let mut mask = vec![0_u8; FIXED_SIDE * FIXED_SIDE];
    for fixed_y in 0..FIXED_SIDE {
        for fixed_x in 0..FIXED_SIDE {
            let screen_x = fixed_x * 2;
            let screen_y = fixed_y * 2;
            let output = fixed_y * FIXED_SIDE + fixed_x;
            if aperture[screen_y * SOURCE_SIDE + screen_x] == 0 {
                continue;
            }
            let source_x =
                origin_x - screen_y as i32 * sine_scaled + screen_x as i32 * cosine_scaled;
            let source_y =
                origin_y + screen_y as i32 * cosine_scaled + screen_x as i32 * sine_scaled;
            let tile_x = ((source_x >> 8) & 255) as usize;
            let tile_y = ((source_y >> 8) & 255) as usize;
            let tile = tile_y * 256 + tile_x;
            terrain[output] = assets.terrain_shade_index(
                live.terrain.terrain_types[tile],
                live.terrain.shading[tile],
            )?;
            mask[output] = 1;
        }
    }
    Ok(IndexedOverlayCommand::BlendPixels {
        stage: IndexedOverlayStage::MinimapTerrain,
        x: 0,
        y: 0,
        width: 64,
        height: 64,
        blend_indices: terrain.into_boxed_slice(),
        mask: mask.into_boxed_slice(),
    })
}

fn push_minimap_entities(
    assets: &IndexedOverlayAssets,
    live: &Level1PresentationLive<'_>,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    for command in live.minimap_entities {
        let x = command.x / 2;
        let y = command.y / 2;
        match command.kind {
            LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL => {
                commands.push(IndexedOverlayCommand::BlendPixel {
                    stage: IndexedOverlayStage::MinimapEntities,
                    x,
                    y,
                    blend_index: command.palette_index,
                });
            }
            LEVEL1_MINIMAP_COMMAND_DOT => {
                commands.push(IndexedOverlayCommand::Rect {
                    stage: IndexedOverlayStage::MinimapEntities,
                    x,
                    y,
                    width: u16::from(command.size.max(1)).div_ceil(2),
                    height: u16::from(command.size.max(1)).div_ceil(2),
                    colour: command.palette_index,
                });
            }
            LEVEL1_MINIMAP_COMMAND_SPRITE => {
                let (width, height) = assets.bitmap_extent(command.selector).ok_or_else(|| {
                    PresentationBridgeError(format!(
                        "live minimap selector {} is absent from CP1 bank",
                        command.selector
                    ))
                })?;
                let left = if command.anchor == LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT {
                    x
                } else if command.anchor == LEVEL1_MINIMAP_ANCHOR_BOTTOM_CENTER {
                    x - i16::try_from(width / 2).expect("bitmap width fits i16")
                } else {
                    x
                };
                commands.push(IndexedOverlayCommand::Bitmap {
                    stage: IndexedOverlayStage::MinimapEntities,
                    selector: command.selector,
                    x: left,
                    y: y - i16::try_from(height).expect("bitmap height fits i16"),
                });
            }
            kind => {
                return Err(PresentationBridgeError(format!(
                    "live minimap command kind {kind} is not typed"
                )));
            }
        }
    }
    // DrawMinimapEntities_B_61A00 finishes with the single-blend local cross.
    let centre = (31_i16, 32_i16);
    let centre_cross = assets.palette_index(4095).ok_or_else(|| {
        PresentationBridgeError("CLRN[4095] minimap cross colour is absent".to_owned())
    })?;
    commands.push(IndexedOverlayCommand::BlendPixel {
        stage: IndexedOverlayStage::MinimapEntities,
        x: centre.0,
        y: centre.1,
        blend_index: centre_cross,
    });
    for arm in 1..=5 {
        for (x, y) in [
            (centre.0 - arm, centre.1),
            (centre.0 + arm, centre.1),
            (centre.0, centre.1 - arm),
            (centre.0, centre.1 + arm),
        ] {
            commands.push(IndexedOverlayCommand::BlendPixel {
                stage: IndexedOverlayStage::MinimapEntities,
                x,
                y,
                blend_index: centre_cross,
            });
        }
    }
    Ok(())
}

fn push_minimap_marks(
    assets: &IndexedOverlayAssets,
    marks: Level1MinimapMarks,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    push_marks(
        assets,
        marks,
        IndexedOverlayStage::MinimapMarks,
        2,
        commands,
    )
}

fn push_marks(
    assets: &IndexedOverlayAssets,
    marks: Level1MinimapMarks,
    stage: IndexedOverlayStage,
    divisor: i16,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let command_start = commands.len();
    if marks.mark_pulse {
        for ordinal in 0..usize::from(marks.mark_count.min(16)) {
            let x = marks.mark_x[ordinal] / divisor;
            let y = marks.mark_y[ordinal] / divisor;
            commands.push(IndexedOverlayCommand::Rect {
                stage,
                x: x - 1,
                y: y - 1,
                width: 3,
                height: 3,
                colour: marks.mark_colour[ordinal],
            });
        }
    }
    if marks.arrow_visible {
        for (from, to) in [(0, 1), (1, 3), (3, 4), (4, 5), (5, 6), (6, 2), (2, 0)] {
            commands.push(IndexedOverlayCommand::Line {
                stage,
                from_x: marks.arrow_xy[from * 2] / divisor,
                from_y: marks.arrow_xy[from * 2 + 1] / divisor,
                to_x: marks.arrow_xy[to * 2] / divisor,
                to_y: marks.arrow_xy[to * 2 + 1] / divisor,
                colour: assets.palette_index(3840).ok_or_else(|| {
                    PresentationBridgeError("CLRN[3840] map-arrow colour is absent".to_owned())
                })?,
            });
        }
    }
    if commands.len() == command_start {
        push_decision(
            commands,
            stage,
            u64::from(marks.mark_count)
                | (u64::from(marks.mark_pulse) << 8)
                | (u64::from(marks.arrow_valid) << 9)
                | (u64::from(marks.arrow_visible) << 10),
        );
    }
    Ok(())
}

fn push_spell(
    commands: &mut Vec<IndexedOverlayCommand>,
    stage: IndexedOverlayStage,
    x: i16,
    slot: LiveSpellSlot,
    hud: LiveHud,
) {
    if slot.holder_entity_index == 0
        || slot.enabled_flags & 4 != 0
            && slot.active_value > 0
            && slot.active_value < 32
            && hud.color_phase_1 != 0
    {
        push_decision(
            commands,
            stage,
            u64::from(slot.holder_entity_index)
                | (u64::from(slot.enabled_flags) << 16)
                | (u64::from(slot.active_value.cast_unsigned()) << 24),
        );
        return;
    }
    commands.push(IndexedOverlayCommand::Bitmap {
        stage,
        selector: slot.frame_selector,
        x,
        y: 1,
    });
    commands.push(IndexedOverlayCommand::Bitmap {
        stage,
        selector: slot.icon_selector,
        x,
        y: 1,
    });
    let tier = usize::try_from(slot.tier.clamp(0, 4)).expect("tier is nonnegative");
    let numerals: [&[u8]; 5] = [b"I", b"II", b"III", b"IV", b"V"];
    let text = numerals[tier];
    commands.push(IndexedOverlayCommand::Text {
        stage,
        bytes: text.into(),
        x: x + 32 - 4 * i16::try_from(text.len()).expect("tier text length fits i16") - 1,
        y: 1,
        colour: 61,
    });
    if slot.max_mana <= 0 {
        return;
    }
    let remainder = slot.caster_mana.rem_euclid(slot.max_mana);
    commands.push(IndexedOverlayCommand::Rect {
        stage,
        x: x + 2,
        y: 19,
        width: u16::try_from((28_i64 * i64::from(remainder) / i64::from(slot.max_mana)).max(0))
            .unwrap_or(0),
        height: 2,
        colour: hud.mana_max_color_index,
    });
    let mut charges = slot.caster_mana / slot.max_mana;
    for column in 0..27 {
        for row in 0..2 {
            if charges <= 0 {
                return;
            }
            commands.push(IndexedOverlayCommand::Rect {
                stage,
                x: x + i16::try_from(column + 2).expect("column fits i16"),
                y: 19 + i16::try_from(row).expect("row fits i16"),
                width: 1,
                height: 1,
                colour: hud.mana_color_index,
            });
            charges -= 1;
        }
    }
    if slot.mana_regen != 0 && slot.mana_regen > hud.castle_mana {
        commands.push(IndexedOverlayCommand::BlendPixels {
            stage,
            x,
            y: 1,
            width: 32,
            height: 20,
            blend_indices: vec![16; 32 * 20].into_boxed_slice(),
            mask: vec![1; 32 * 20].into_boxed_slice(),
        });
    }
}

fn push_top_status(commands: &mut Vec<IndexedOverlayCommand>, hud: LiveHud) {
    let stage = IndexedOverlayStage::TopStatus;
    commands.push(bitmap(stage, 40, 1, 1));
    if hud.castle_active && hud.castle_level > 0 {
        commands.push(bitmap(
            stage,
            if hud.castle_hit_frame_ticks > 0 && hud.color_phase_1 != 0 {
                55
            } else {
                41
            },
            63,
            1,
        ));
        commands.push(bitmap(stage, 43 + u16::from(hud.castle_level), 64, 1));
        commands.push(bitmap(stage, 42, 82, 1));
        push_hud_bar(
            commands,
            stage,
            92,
            5,
            hud_width(hud.castle_life, hud.castle_max_life),
            5,
            hud.life_color_index,
        );
        push_hud_bar(
            commands,
            stage,
            92,
            14,
            hud_width(hud.castle_mana_capacity, hud.global_mana_denominator),
            5,
            hud.mana_max_color_index,
        );
        push_hud_bar(
            commands,
            stage,
            92,
            14,
            hud_width(
                hud.castle_mana + hud.secured_mana_component,
                hud.global_mana_denominator,
            ),
            5,
            hud.mana_color_index,
        );
        let goal = 92
            + i16::try_from(hud.goal_percent.clamp(0, 100) * 32 / 100)
                .expect("goal width fits i16");
        let goal_colour = if hud.color_phase_2 == 0 {
            hud.mana_color_index
        } else {
            hud.mana_max_color_index
        };
        push_hud_bar(commands, stage, goal, 13, 1, 1, goal_colour);
        push_hud_bar(commands, stage, goal, 19, 1, 1, goal_colour);
    } else {
        commands.push(bitmap(stage, 54, 63, 1));
    }
    if hud.balloon_level == 0 {
        commands.push(bitmap(stage, 54, 127, 1));
    } else {
        commands.push(bitmap(stage, 41, 127, 1));
        let balloon_count: usize = match hud.balloon_level {
            1..=3 => 1,
            4..=5 => 2,
            6..=7 => 3,
            _ => 0,
        };
        if balloon_count != 0 {
            commands.push(bitmap(
                stage,
                43 + u16::try_from(balloon_count).expect("balloon count fits u16"),
                128,
                1,
            ));
            commands.push(bitmap(stage, 42, 146, 1));
            for balloon in 0..balloon_count {
                push_hud_bar(
                    commands,
                    stage,
                    156,
                    6 + i16::try_from(balloon * 2).expect("balloon row fits i16"),
                    hud_width(hud.balloon_life[balloon], hud.balloon_max_life[balloon]),
                    1,
                    hud.life_color_index,
                );
                push_hud_bar(
                    commands,
                    stage,
                    156,
                    15 + i16::try_from(balloon * 2).expect("balloon row fits i16"),
                    hud_width(hud.balloon_mana[balloon], hud.balloon_max_mana[balloon]),
                    1,
                    hud.mana_color_index,
                );
            }
        }
    }
    commands.push(bitmap(
        stage,
        if hud.player_hit_frame_ticks > 0 && hud.color_phase_1 != 0 {
            55
        } else {
            41
        },
        191,
        1,
    ));
    if (hud.player_status_flags & 0x20 != 0 || hud.player_status_word != 0)
        && hud.color_phase_2 != 0
    {
        commands.push(IndexedOverlayCommand::TransparentBitmap {
            stage,
            selector: 43,
            x: 192,
            y: 1,
        });
        commands.push(IndexedOverlayCommand::TransparentBitmap {
            stage,
            selector: 42,
            x: 210,
            y: 1,
        });
    } else {
        commands.push(bitmap(stage, 43, 192, 1));
        commands.push(bitmap(stage, 42, 210, 1));
    }
    push_hud_bar(
        commands,
        stage,
        220,
        5,
        hud_width(hud.player_life, hud.player_max_life),
        5,
        hud.life_color_index,
    );
    push_hud_bar(
        commands,
        stage,
        220,
        14,
        hud_width(hud.player_max_mana, hud.global_mana_denominator),
        5,
        hud.mana_max_color_index,
    );
    push_hud_bar(
        commands,
        stage,
        220,
        14,
        hud_width(hud.player_mana, hud.global_mana_denominator),
        5,
        hud.mana_color_index,
    );
}

fn push_bottom_spell_menu(
    assets: &IndexedOverlayAssets,
    menu: LiveSpellMenu,
    commands: &mut Vec<IndexedOverlayCommand>,
) -> Result<(), PresentationBridgeError> {
    let stage = IndexedOverlayStage::BottomSpellMenu;
    commands.push(bitmap(stage, 88, 0, 164));
    let (edge_width, _) = assets
        .bitmap_extent(88)
        .ok_or_else(|| PresentationBridgeError("spell-menu edge selector 88 missing".to_owned()))?;
    commands.push(bitmap(
        stage,
        88,
        320 - i16::try_from(edge_width).expect("edge width fits i16"),
        164,
    ));
    for family in 0..26 {
        let x = 4 + i16::try_from((family % 13) * 24).expect("menu x fits i16");
        let y = 164 + i16::try_from((family / 13) * 18).expect("menu y fits i16");
        if menu.family_available[family] == 0 {
            commands.push(bitmap(stage, 89, x, y));
            continue;
        }
        let holder = menu.holder_entity_index[family];
        let selected = usize::from(menu.selected_ui_index) == family;
        if holder == 0 {
            commands.push(bitmap(stage, 89, x, y));
            if menu.family_experience[family] != 0 || menu.family_unlocked_tier[family] != 0 {
                commands.push(IndexedOverlayCommand::ColourizedBitmap {
                    stage,
                    selector: 97 + u16::try_from(family).expect("family fits u16"),
                    x,
                    y,
                    colour: 0xa6,
                });
            }
            continue;
        }
        let tier = usize::try_from(menu.family_selected_tier[family].clamp(0, 2))
            .expect("tier is nonnegative");
        let suppress_active = menu.family_enabled_flags[family] & 4 != 0
            && (1..32).contains(&menu.family_active_value[family])
            && menu.active_suppression_phase;
        if suppress_active {
            if selected || menu.family_hand_eligibility[family] != 0 {
                if menu.family_hand_eligibility[family] == 1 {
                    commands.push(IndexedOverlayCommand::TransparentBitmap {
                        stage,
                        selector: 149,
                        x,
                        y,
                    });
                } else if menu.family_hand_eligibility[family] == 2 {
                    commands.push(IndexedOverlayCommand::TransparentBitmap {
                        stage,
                        selector: 150,
                        x: x + 18,
                        y,
                    });
                }
            }
            continue;
        }
        let limit = menu.family_max_mana_limit[family][tier];
        let can_summon = limit == 0 || menu.castle_mana >= limit;
        if selected && can_summon {
            commands.push(bitmap(stage, 87, x, y));
            let cost = menu.family_mana_cost[family][tier];
            if cost > 0 {
                let remainder_width = u16::try_from(
                    (18_i64 * i64::from(menu.caster_mana.rem_euclid(cost)) / i64::from(cost))
                        .clamp(0, 18),
                )
                .expect("spell-menu mana width fits u16");
                push_hud_bar(
                    commands,
                    stage,
                    x + 3,
                    y + 14,
                    remainder_width,
                    2,
                    menu.player_colour_1,
                );
                let charges = (menu.caster_mana / cost).clamp(0, 36);
                for charge in 0..charges {
                    commands.push(IndexedOverlayCommand::Rect {
                        stage,
                        x: x + 3 + i16::try_from(charge / 2).expect("charge x fits i16"),
                        y: y + 14 + i16::try_from(charge % 2).expect("charge y fits i16"),
                        width: 1,
                        height: 1,
                        colour: menu.player_colour_0,
                    });
                }
            }
        } else {
            commands.push(bitmap(stage, if can_summon { 89 } else { 91 }, x, y));
        }
        if can_summon {
            commands.push(bitmap(
                stage,
                97 + u16::try_from(family).expect("family fits u16"),
                x,
                y,
            ));
        } else {
            commands.push(IndexedOverlayCommand::TransparentBitmap {
                stage,
                selector: 97 + u16::try_from(family).expect("family fits u16"),
                x,
                y,
            });
        }
        if selected {
            if menu.family_hand_eligibility[family] == 1 {
                commands.push(IndexedOverlayCommand::TransparentBitmap {
                    stage,
                    selector: 149,
                    x,
                    y,
                });
            } else if menu.family_hand_eligibility[family] == 2 {
                commands.push(IndexedOverlayCommand::TransparentBitmap {
                    stage,
                    selector: 150,
                    x: x + 18,
                    y,
                });
            }
        }
    }
    let selected = usize::from(menu.selected_ui_index);
    if selected < 26 {
        let x = 4 + i16::try_from((selected % 13) * 24).expect("menu x fits i16");
        let y = 164 + i16::try_from((selected / 13) * 18).expect("menu y fits i16");
        commands.push(bitmap(stage, 90, x, y));
    }
    if selected < 26
        && menu.family_available[selected] != 0
        && menu.holder_entity_index[selected] != 0
    {
        let column = i16::try_from(selected % 13).expect("menu column fits i16");
        let tier_start_x = (-67 + 48 * column).clamp(0, 442) / 2;
        let unlocked = menu.family_unlocked_tier[selected].clamp(0, 2);
        let profile = assets.spell_menu_profile(selected).ok_or_else(|| {
            PresentationBridgeError(format!("spell-menu profile {selected} is absent"))
        })?;
        for tier in 0..3_i8 {
            let tier_x = tier_start_x + i16::from(tier) * 33;
            let tier_index = usize::try_from(tier).expect("tier is nonnegative");
            let limit = menu.family_max_mana_limit[selected][tier_index];
            let can_sub_summon = limit == 0 || menu.castle_mana >= limit;
            let cost = menu.family_mana_cost[selected][tier_index];
            let mana_part = if can_sub_summon && cost > 0 {
                menu.caster_mana / cost
            } else {
                0
            };
            commands.push(bitmap(
                stage,
                if tier > unlocked {
                    163
                } else if can_sub_summon && mana_part > 0 {
                    161
                } else {
                    162
                },
                tier_x,
                147,
            ));
            if tier <= unlocked {
                commands.push(bitmap(
                    stage,
                    165 + u16::try_from(tier).expect("tier fits u16"),
                    tier_x + 3,
                    152,
                ));
                let icon_selector = 179
                    + u16::try_from(selected * 3).expect("family tier fits u16")
                    + u16::try_from(tier).expect("tier fits u16");
                if can_sub_summon {
                    commands.push(bitmap(stage, icon_selector, tier_x + 9, 150));
                } else {
                    commands.push(IndexedOverlayCommand::TransparentBitmap {
                        stage,
                        selector: icon_selector,
                        x: tier_x + 9,
                        y: 150,
                    });
                }
                if menu.selected_tier == tier {
                    commands.push(bitmap(stage, 164, tier_x, 147));
                }
                if tier <= unlocked && tier < 2 {
                    let experience = menu.family_experience[selected].max(0);
                    let threshold = profile.single_player_experience[tier_index];
                    let next = profile.single_player_experience[tier_index + 1];
                    let width = if tier < unlocked {
                        27
                    } else if next > threshold {
                        u16::try_from(
                            (27_i64 * i64::from(experience.saturating_sub(threshold))
                                / i64::from(next - threshold))
                            .clamp(0, 27),
                        )
                        .expect("spell experience width fits u16")
                    } else {
                        0
                    };
                    commands.push(IndexedOverlayCommand::Rect {
                        stage,
                        x: tier_x + 3,
                        y: 161,
                        width: 27,
                        height: 1,
                        colour: assets.palette_index(0).ok_or_else(|| {
                            PresentationBridgeError("CLRN[0] XP background missing".to_owned())
                        })?,
                    });
                    if width != 0 {
                        commands.push(IndexedOverlayCommand::Rect {
                            stage,
                            x: tier_x + 3,
                            y: 161,
                            width,
                            height: 1,
                            colour: assets.palette_index(3840).ok_or_else(|| {
                                PresentationBridgeError("CLRN[3840] XP fill missing".to_owned())
                            })?,
                        });
                    }
                }
            }
        }
    }
    if menu.cursor_visible {
        return Err(PresentationBridgeError(
            "spell-menu cursor is active but selector-9 sprite-bank provenance is not proven"
                .to_owned(),
        ));
    }
    Ok(())
}

const fn bitmap(
    stage: IndexedOverlayStage,
    selector: u16,
    x: i16,
    y: i16,
) -> IndexedOverlayCommand {
    IndexedOverlayCommand::Bitmap {
        stage,
        selector,
        x,
        y,
    }
}

fn push_hud_bar(
    commands: &mut Vec<IndexedOverlayCommand>,
    stage: IndexedOverlayStage,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    colour: u8,
) {
    if width == 0 || height == 0 {
        return;
    }
    commands.push(IndexedOverlayCommand::Rect {
        stage,
        x,
        y,
        width: width.min(32),
        height,
        colour,
    });
}

fn hud_width(value: i32, denominator: i32) -> u16 {
    if denominator <= 0 {
        return 0;
    }
    u16::try_from((i64::from(value) * 32 / i64::from(denominator)).clamp(0, 32))
        .expect("clamped HUD width fits u16")
}

#[cfg(test)]
pub(crate) fn assert_independent_source_vectors(assets: &IndexedOverlayAssets) {
    use sim_core::level1_mission::ABILITY_IMPROVED_NOTIFICATION_TIMER;

    // These vectors are a separate transcription from pinned remc2
    // 6806c720 GameUI.cpp 4591-5195 and EventsFunctions.cpp
    // 21647-22688/44696-44883. They intentionally do not read the cache
    // generator's source_semantic_vectors ledger.
    let stage = IndexedOverlayStage::Menu;
    let mut authoritative = LiveMenuState {
        authority_mask: LIVE_MENU_AUTH_ALL,
        load_available: true,
        sound_available: true,
        music_available: true,
        options_enabled_mask: 0x1ffff,
        sound_volume: 127,
        music_volume: 127,
        volume_target: 1,
        text_colour_phase: 1,
        ..LiveMenuState::default()
    };
    let mut commands = Vec::new();

    authoritative.load_available = false;
    push_pause_menu(assets, authoritative, true, stage, &mut commands)
        .expect("independent pause vector builds");
    assert_eq!(
        commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap { selector, x, y, .. } => {
                    Some((*selector, *x, *y))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (174, 134, 38),
            (175, 134, 54),
            (176, 134, 70),
            (177, 160, 70),
            (178, 134, 86),
        ],
        "pinned GameUI pause selector/coordinate vector"
    );
    assert_eq!(
        commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::BlendPixels {
                    x,
                    y,
                    width,
                    height,
                    blend_indices,
                    ..
                } => Some((*x, *y, *width, *height, blend_indices[0])),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [(134, 54, 51, 16, 61), (134, 54, 51, 16, 61)],
        "source width102 minus2 becomes two fixed 51x16 CLRN0 shade passes"
    );

    authoritative.chat_lines[5][..5].copy_from_slice(b"HELLO");
    authoritative.chat_mode = 3;
    authoritative.chat_player_mask = 1 << 1;
    authoritative.mouse_x = 20;
    authoritative.mouse_y = 376;
    commands.clear();
    push_chat_menu(assets, authoritative, stage, &mut commands)
        .expect("independent chat vector builds");
    assert_eq!(
        commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap { selector, x, y, .. }
                    if matches!(*selector, 259..=261) =>
                {
                    Some((*selector, *x, *y))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        [(259, 192, 166), (261, 237, 166), (260, 282, 166)],
        "pinned chat button order is independent of the cache ledger"
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Text {
            bytes,
            x: 5,
            y: 167,
            colour: 176,
            ..
        } if bytes.as_ref() == b"HELLO"
    )));

    authoritative.options_selected_mask = 1;
    authoritative.options_enabled_mask = 0x1fffe;
    authoritative.mouse_x = 240;
    authoritative.mouse_y = 72;
    commands.clear();
    push_options_menu(assets, authoritative, stage, &mut commands)
        .expect("independent options vector builds");
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::LocalizedText {
            selector: 401,
            x: 134,
            y: 34,
            colour: 177,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::LocalizedText {
            selector: 403,
            y: 178,
            colour: 176,
            ..
        }
    )));

    authoritative.volume_target = 2;
    authoritative.music_volume = 63;
    commands.clear();
    push_volume_menu(assets, authoritative, stage, &mut commands)
        .expect("independent volume vector builds");
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Rect {
            x: 137,
            y: 105,
            width: 22,
            height: 6,
            colour: 171,
            ..
        }
    )));

    authoritative.selected_menu_item = 2;
    let terrain = TerrainPlanes {
        terrain_types: vec![0; 256 * 256].into_boxed_slice(),
        ground_heights: vec![0; 256 * 256].into_boxed_slice(),
        shading: vec![0; 256 * 256].into_boxed_slice(),
        angles: vec![0; 256 * 256].into_boxed_slice(),
        secondary_heights: vec![0; 256 * 256].into_boxed_slice(),
        entity_indices: vec![0; 256 * 256].into_boxed_slice(),
    };
    let mut live = Level1PresentationLive {
        flight: FlightState::default(),
        menu_state: 13,
        player_life: 1,
        player_immobilize_ticks: 0,
        terrain: &terrain,
        hud: LiveHud::default(),
        spell_menu: LiveSpellMenu::default(),
        menu: authoritative,
        debug_entity: LiveDebugEntity {
            pointer_value: 1234,
            source_slot: 9,
            class: 0x83,
            model: 0xfe,
            action: 250,
            life: 100,
            max_life: 200,
            actual_speed: -123,
            min_speed: -2,
            max_speed: i16::MAX,
            position: Position {
                x: 65_534,
                y: 1,
                z: -3,
            },
            source_id: u16::MAX,
            parent_id: 32_768,
            mana: 6,
            max_mana: 7,
        },
        minimap_entities: &[],
        minimap_marks: Level1MinimapMarks::default(),
        map_entities: &[],
        map_marks: Level1MinimapMarks::default(),
        objective_selector: 48,
        objective_ticks: 0,
        level_completed: false,
        exit_sequence_active: false,
        exit_display_blank: false,
        exit_blur_mode: 0,
        notification_active: false,
        notification_id: 0,
        notification_state: 0,
        notification_timer: 0,
        notification_player_name: [0; 16],
        level_number: 1,
        player_turn: 123,
        sound_number: 46,
        game_timer_turn: 456,
        debug_fps: 17,
        active_entity_count: 177,
        heap_size: 20_971_520,
        memory_used: 0,
        memory_free: 0,
    };
    commands.clear();
    push_ok_cancel_menu(assets, &live, false, stage, &mut commands)
        .expect("independent OK/cancel vector builds");
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Text {
            bytes,
            x: 66,
            y: 25,
            colour: 119,
            ..
        } if bytes.as_ref() == b"Load Level?"
    )));

    live.menu.selected_menu_item = 0;
    live.notification_active = true;
    live.notification_id = ABILITY_IMPROVED_NAME_SELECTOR_BASE + 16;
    live.notification_state = ABILITY_IMPROVED_NOTIFICATION_STATE;
    live.notification_timer = ABILITY_IMPROVED_NOTIFICATION_TIMER;
    commands.clear();
    let notification_feedback = push_pause_or_end(
        assets,
        &live,
        PresentationBridgeQuery::default(),
        false,
        &mut commands,
    )
    .expect("independent ability-improved notification vector builds");
    let expected_notification = format_single_percent_s(
        assets
            .localization(ABILITY_IMPROVED_TEMPLATE_SELECTOR)
            .expect("ability-improved template localization"),
        assets
            .localization(ABILITY_IMPROVED_NAME_SELECTOR_BASE + 16)
            .expect("ability-improved name localization"),
    )
    .expect("ability-improved template has one name placeholder");
    assert_eq!(
        notification_feedback,
        Some((
            true,
            ABILITY_IMPROVED_NOTIFICATION_STATE,
            ABILITY_IMPROVED_NOTIFICATION_TIMER - 1,
        )),
        "pinned notification state 5 remains active and decrements timer 200 to 199"
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Text { bytes, .. }
            if bytes.as_ref() == expected_notification.as_slice()
    )));
    live.notification_active = false;
    live.notification_id = 0;
    live.notification_state = 0;
    live.notification_timer = 0;

    let mut spell = LiveSpellMenu {
        selected_ui_index: 1,
        castle_mana: 0,
        caster_mana: 100,
        ..LiveSpellMenu::default()
    };
    spell.family_available[0] = 1;
    spell.holder_entity_index[0] = 99;
    spell.family_hand_eligibility[0] = 1;
    commands.clear();
    push_bottom_spell_menu(assets, spell, &mut commands)
        .expect("independent unselected-eligibility vector builds");
    assert!(
        !commands.iter().any(|command| matches!(
            command,
            IndexedOverlayCommand::TransparentBitmap {
                selector: 149 | 150,
                x: 4,
                y: 164,
                ..
            }
        )),
        "normal unselected eligibility does not enter the hand-corner branch"
    );

    spell.family_available[4] = 1;
    spell.holder_entity_index[4] = 0;
    spell.family_experience[4] = 1;
    commands.clear();
    push_bottom_spell_menu(assets, spell, &mut commands)
        .expect("independent acquired vector builds");
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::ColourizedBitmap {
            selector: 101,
            x: 100,
            y: 164,
            colour: 166,
            ..
        }
    )));
    spell.holder_entity_index[4] = 99;
    spell.family_experience[4] = 0;
    spell.family_max_mana_limit[4][0] = 1;
    commands.clear();
    push_bottom_spell_menu(assets, spell, &mut commands)
        .expect("independent cannot-summon vector builds");
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Bitmap {
            selector: 91,
            x: 100,
            y: 164,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::TransparentBitmap {
            selector: 101,
            x: 100,
            y: 164,
            ..
        }
    )));

    live.menu.authority_mask = LIVE_MENU_AUTH_DEBUG_METRICS;
    commands.clear();
    push_debug(assets, 0x48, &live, &mut commands).expect("independent debug vector builds");
    let debug_rows = commands
        .iter()
        .filter_map(|command| match command {
            IndexedOverlayCommand::Text {
                bytes,
                x: 160,
                y,
                colour,
                ..
            } => Some((bytes.as_ref(), *y, *colour)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(debug_rows.len(), 25);
    assert_eq!(debug_rows[3], (b"Beta".as_slice(), 21, 23));
    assert_eq!(debug_rows[13], (b"17".as_slice(), 91, 23));
    assert_eq!(debug_rows[15], (b"46".as_slice(), 105, 23));
    assert_eq!(debug_rows[17], (b"123 456".as_slice(), 119, 23));
    assert_eq!(
        debug_rows[19],
        (b"Thing 168, Active 177".as_slice(), 133, 23)
    );
    assert_eq!(debug_rows[22], (b"Heap 20971520".as_slice(), 154, 23));
    assert_eq!(debug_rows[24], (b"0/0".as_slice(), 168, 23));
    assert!(commands.iter().any(|command| matches!(
        command,
        IndexedOverlayCommand::Text {
            bytes,
            x: 5,
            y: 49,
            colour: 87,
            ..
        } if bytes.as_ref() == b"1234 / 9"
    )));
    let entity_rows = commands
        .iter()
        .filter_map(|command| match command {
            IndexedOverlayCommand::Text {
                bytes,
                x: 5,
                y,
                colour,
                ..
            } => Some((bytes.as_ref(), *y, *colour)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        entity_rows,
        [
            (b"THING".as_slice(), 42, 119),
            (b"1234 / 9".as_slice(), 49, 87),
            (b"CLASS / MODEL / STATE".as_slice(), 56, 119),
            (b"-125 / -2 / 250".as_slice(), 63, 87),
            (b"LIFE / MAX LIFE".as_slice(), 70, 119),
            (b"100 / 200".as_slice(), 77, 87),
            (b"SPEED ACTUAL / MINIMUM / MAXIMUM".as_slice(), 84, 119,),
            (b"-123 / -2 / 32767".as_slice(), 91, 87),
            (b"ACTUAL X / Y / Z".as_slice(), 98, 119),
            (b"65534 / 1 / 65533".as_slice(), 105, 87),
            (b"ID / WHO OWNS ME".as_slice(), 112, 119),
            (b"65535 / 32768".as_slice(), 119, 87),
            (b"MANA ACTUAL / MAXIMUM".as_slice(), 126, 119),
            (b"6 / 7".as_slice(), 133, 87),
        ],
        "pinned %hhd/%hhu/%hd/%hu debug rows preserve C width and sign semantics"
    );

    let mutations: &[(fn(&mut LiveDebugEntity), &[u8])] = &[
        (
            |entity: &mut LiveDebugEntity| entity.class = 0x7f,
            b"127 / -2 / 250".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.model = 0x80,
            b"-125 / -128 / 250".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.action = 0,
            b"-125 / -2 / 0".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.min_speed = i16::MIN,
            b"-123 / -32768 / 32767".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.max_speed = -1,
            b"-123 / -2 / -1".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.position.x = 0,
            b"0 / 1 / 65533".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.position.y = u16::MAX,
            b"65534 / 65535 / 65533".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.position.z = 2,
            b"65534 / 1 / 2".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.source_id = 42,
            b"42 / 32768".as_slice(),
        ),
        (
            |entity: &mut LiveDebugEntity| entity.parent_id = u16::MAX,
            b"65535 / 65535".as_slice(),
        ),
    ];
    for &(mutate, expected) in mutations {
        let original = live.debug_entity;
        mutate(&mut live.debug_entity);
        commands.clear();
        push_debug(assets, 0x40, &live, &mut commands)
            .expect("independent mutated debug vector builds");
        assert!(
            commands.iter().any(|command| matches!(
                command,
                IndexedOverlayCommand::Text { bytes, .. }
                    if bytes.as_ref() == expected
            )),
            "mutated source debug vector must produce {expected:?}"
        );
        live.debug_entity = original;
    }

    live.menu_state = 0;
    live.menu = authoritative;
    live.menu.selected_menu_item = 0;
    live.menu.load_available = false;
    live.menu.pause_menu_suppressed = false;
    live.menu.recursive_pause_writer = false;
    let pause_plan = build_level1_final_main(
        assets,
        live,
        PresentationBridgeQuery {
            minimap_enabled: true,
            top_bar_enabled: true,
            game_paused: true,
            ..PresentationBridgeQuery::default()
        },
    )
    .expect("independent pause raster vector builds");
    let mut pause_indices = vec![17_u8; 320 * 200];
    let mut pause_provenance = vec![0_u8; 320 * 200];
    assets
        .apply(
            &pause_plan.overlay,
            &mut pause_indices,
            &mut pause_provenance,
            None,
        )
        .expect("independent pause raster vector executes");
    let row_start = 54 * 320 + 134;
    assert_eq!(
        &pause_indices[row_start..row_start + 51],
        [
            49, 49, 50, 50, 52, 53, 53, 53, 53, 53, 51, 51, 51, 51, 51, 52, 51, 52, 53, 53, 53, 53,
            53, 53, 53, 53, 53, 53, 53, 53, 53, 53, 53, 53, 53, 52, 51, 52, 53, 53, 53, 53, 53, 53,
            53, 53, 53, 53, 53, 53, 57,
        ],
        "pinned GameUI.cpp 4925-4936 selector-175 plus two CLRN[0] shade passes"
    );
    assert_eq!(
        &pause_provenance[row_start..row_start + 51],
        [8; 51],
        "all 51 fixed shade pixels carry executed overlay provenance"
    );
}

#[allow(dead_code)]
const fn _position_is_live(_: Position) {}

#[cfg(test)]
mod ability_notification_tests {
    use super::*;

    #[test]
    fn ability_notification_formats_exactly_one_name_placeholder() {
        assert_eq!(
            format_single_percent_s(b"ABILITY %s IMPROVED", b"CASTLE").unwrap(),
            b"ABILITY CASTLE IMPROVED"
        );
        assert!(format_single_percent_s(b"ABILITY IMPROVED", b"CASTLE").is_err());
        assert!(format_single_percent_s(b"%s %s", b"CASTLE").is_err());
    }
}

#[cfg(test)]
mod level2_minimap_projection_tests {
    use sim_core::{
        FlightState, Position,
        level1_mission::{
            Level1MinimapEntityInput, level1_minimap_entity_command, level1_minimap_project,
        },
    };

    #[test]
    fn level2_minimap_reuses_source_rotation_palette_and_admission_vectors() {
        let mut player = FlightState {
            position: Position {
                x: 1_000,
                y: 1_000,
                z: 0,
            },
            tick: 3,
            ..FlightState::default()
        };
        let input = Level1MinimapEntityInput {
            source_slot: 1,
            class: 2,
            model: 1,
            action: 2,
            position: Position {
                x: 1_256,
                y: 1_000,
                z: 0,
            },
            active: true,
            ..Level1MinimapEntityInput::default()
        };
        let day_marker = level1_minimap_entity_command(&player, input)
            .expect("source phase admits the marker at tick three");
        assert_eq!(day_marker.palette_index, 23);
        assert!(day_marker.x > 64);

        player.yaw = 1_024;
        let rotated = level1_minimap_entity_command(&player, input)
            .expect("rotated source phase remains admitted");
        assert_ne!((day_marker.x, day_marker.y), (rotated.x, rotated.y));

        let blocked = Level1MinimapEntityInput { class: 4, ..input };
        assert!(level1_minimap_entity_command(&player, blocked).is_none());

        let off_map = level1_minimap_project(
            &player,
            Position {
                x: 20_000,
                y: 1_000,
                z: 0,
            },
            128,
            256,
        );
        assert!(off_map.0 >= 128 || off_map.0 < 0 || off_map.1 >= 128 || off_map.1 < 0);
    }
}
