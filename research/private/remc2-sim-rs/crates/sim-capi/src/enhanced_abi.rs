//! Additive fixed-record ABI for the Rust-owned Unity Enhanced snapshot.
//!
//! The page function is a deterministic bridge fixture until a NativeSession
//! owns one `EnhancedPresentationLedger` per presentation session.  The ABI
//! shape is real: fixed records, size-query semantics, generation/tick
//! identity, source ordering, pagination, and explicit family capability
//! flags are all validated at the boundary.

use std::{
    cell::RefCell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    slice,
};

use sim_core::enhanced_presentation::{
    ENHANCED_ENTITY_ACTIVE, ENHANCED_ENTITY_VISIBLE, ENHANCED_EVENT_PRESENTATION_ONLY,
    ENHANCED_FAMILY_COUNT, ENHANCED_PROFILE_REVISION, ENHANCED_PROFILE_UNITY,
    EnhancedAudioReference, EnhancedAuthority, EnhancedDirtyRegion, EnhancedEntityRecord,
    EnhancedEventKind, EnhancedEventRecord, EnhancedFamily, EnhancedFrameState,
    EnhancedMinimapMark, EnhancedPresentationError, EnhancedPresentationLedger, EnhancedTransform,
};

pub const MC_ENHANCED_ABI_VERSION: u32 = 1;
pub const MC_ENHANCED_OK: i32 = 0;
pub const MC_ENHANCED_NULL_POINTER: i32 = 1;
pub const MC_ENHANCED_INVALID_ARGUMENT: i32 = 3;
pub const MC_ENHANCED_STRUCT_TOO_SMALL: i32 = 5;
pub const MC_ENHANCED_BUFFER_TOO_SMALL: i32 = 11;
pub const MC_ENHANCED_GENERATION_MISMATCH: i32 = 12;
pub const MC_ENHANCED_INTERNAL_PANIC: i32 = 14;

pub const MC_ENHANCED_FAMILY_FLAG_PRESENTABLE: u32 = 1 << 0;
pub const MC_ENHANCED_FAMILY_FLAG_EXPLICIT_EXCLUSION: u32 = 1 << 1;
pub const MC_ENHANCED_SNAPSHOT_FLAG_FIXTURE: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedFamilyInfoV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub family_code: u8,
    pub flags: u8,
    pub reserved_0: u16,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedSnapshotQueryV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub profile_id: u8,
    pub reserved_profile: [u8; 7],
    pub profile_revision: u64,
    pub entity_cursor: u32,
    pub event_cursor: u32,
    pub max_entities: u32,
    pub max_events: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedEntityStateV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub stable_id: u64,
    pub source_entity_index: u16,
    pub family: u8,
    pub authority: u8,
    pub flags: u32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub selector: i16,
    pub animation_clock: u32,
    pub owner_colour: u8,
    pub life_percent: u8,
    pub reserved_0: u16,
    pub presentation_flags: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedEventStateV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub event_id: u64,
    pub kind: u8,
    pub family: u8,
    pub reserved_0: u16,
    pub source_entity_index: u16,
    pub reserved_1: u16,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub reserved_2: i16,
    pub value_0: i32,
    pub value_1: i32,
    pub flags: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedSnapshotInfoV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub reset_epoch: u64,
    pub entity_cursor: u32,
    pub event_cursor: u32,
    pub next_entity_cursor: u32,
    pub next_event_cursor: u32,
    pub entities_written: u32,
    pub events_written: u32,
    pub entity_bytes: u64,
    pub event_bytes: u64,
    pub state_hash: u64,
    pub event_hash: u64,
    pub receipt_hash: u64,
    pub flags: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedFrameStateV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub reset_epoch: u64,
    pub profile_id: u8,
    pub asset_tier: u8,
    pub quality_flags: u16,
    pub profile_revision: u64,
    pub internal_width: u16,
    pub internal_height: u16,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub terrain_revision: u64,
    pub terrain_hash: u64,
    pub dirty_regions_written: u32,
    pub minimap_marks_written: u32,
    pub audio_references_written: u32,
    pub camera_x: i32,
    pub camera_y: i32,
    pub camera_z: i32,
    pub camera_yaw: i16,
    pub camera_pitch: i16,
    pub camera_roll: i16,
    pub camera_fov: i16,
    pub camera_eye_height: i16,
    pub camera_slope_target_x: i16,
    pub camera_slope_target_y: i16,
    pub player_x: i32,
    pub player_y: i32,
    pub player_z: i32,
    pub player_yaw: i16,
    pub player_pitch: i16,
    pub player_roll: i16,
    pub life_percent: u8,
    pub mana_percent: u8,
    pub selected_spell: u8,
    pub selected_tier: u8,
    pub left_spell: u8,
    pub right_spell: u8,
    pub objective_selector: u8,
    pub objective_textbox_ticks: u8,
    pub hud_flags: u32,
    pub cursor_x: i16,
    pub cursor_y: i16,
    pub menu_state: u8,
    pub reserved_0: [u8; 3],
    pub flags: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedDirtyRegionV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub kind: u8,
    pub flags: u8,
    pub reserved_0: u16,
    pub source_revision: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedMinimapMarkV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub stable_id: u64,
    pub x: i16,
    pub y: i16,
    pub colour: u8,
    pub selector: u8,
    pub flags: u16,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedAudioReferenceV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub event_id: u64,
    pub sound_id: u16,
    pub entity_index: u16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub playback_rate_hz: u32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub play_type: u8,
    pub loop_count: i8,
    pub flags: u16,
    pub reserved: [u32; 3],
}

#[derive(Debug)]
struct EnhancedAbiError {
    status: i32,
    message: String,
}

impl EnhancedAbiError {
    fn new(status: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_error(message: impl Into<String>) {
    LAST_ERROR.with(|error| *error.borrow_mut() = message.into());
}

fn clear_error() {
    LAST_ERROR.with(|error| error.borrow_mut().clear());
}

fn boundary(operation: impl FnOnce() -> Result<(), EnhancedAbiError>) -> i32 {
    clear_error();
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => MC_ENHANCED_OK,
        Ok(Err(error)) => {
            set_error(error.message);
            error.status
        }
        Err(_) => {
            set_error("panic contained at Enhanced ABI boundary");
            MC_ENHANCED_INTERNAL_PANIC
        }
    }
}

fn from_core(error: EnhancedPresentationError, status: i32) -> EnhancedAbiError {
    EnhancedAbiError::new(status, error.0)
}

fn validate_output<T>(output: *mut T, output_size: u32) -> Result<(), EnhancedAbiError> {
    if output.is_null() {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_NULL_POINTER,
            "Enhanced output pointer is null",
        ));
    }
    let required = u32::try_from(size_of::<T>()).expect("Enhanced ABI struct size fits u32");
    if output_size < required {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_STRUCT_TOO_SMALL,
            format!("Enhanced output requires {required} bytes, got {output_size}"),
        ));
    }
    Ok(())
}

fn validate_query(query: &McEnhancedSnapshotQueryV1) -> Result<(), EnhancedAbiError> {
    if query.abi_version != MC_ENHANCED_ABI_VERSION {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            format!(
                "Enhanced query ABI version {} is unsupported",
                query.abi_version
            ),
        ));
    }
    if query.struct_size < u32::try_from(size_of::<McEnhancedSnapshotQueryV1>()).unwrap() {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_STRUCT_TOO_SMALL,
            "Enhanced query struct is too small",
        ));
    }
    if query.generation == 0 {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_GENERATION_MISMATCH,
            "Enhanced query generation zero is invalid",
        ));
    }
    if query.profile_id != ENHANCED_PROFILE_UNITY {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            format!("unknown Enhanced profile id {}", query.profile_id),
        ));
    }
    if query.profile_revision != ENHANCED_PROFILE_REVISION {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            format!(
                "Enhanced profile revision {} is unsupported",
                query.profile_revision
            ),
        ));
    }
    if query.reserved_profile.iter().any(|value| *value != 0) {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            "Enhanced profile reserved fields must be zero",
        ));
    }
    if query.reserved.iter().any(|value| *value != 0) {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            "Enhanced query reserved fields must be zero",
        ));
    }
    Ok(())
}

fn write_output<T: Copy>(
    output: *mut T,
    output_size: u32,
    value: T,
) -> Result<(), EnhancedAbiError> {
    validate_output(output, output_size)?;
    unsafe { output.write_unaligned(value) };
    Ok(())
}

fn entity_fixture(
    generation: u64,
    stable_id: u64,
    source_entity_index: u16,
) -> EnhancedEntityRecord {
    EnhancedEntityRecord {
        generation,
        stable_id,
        source_entity_index,
        family: if source_entity_index % 2 == 0 {
            EnhancedFamily::Goat
        } else {
            EnhancedFamily::ResidentVillager
        },
        authority: EnhancedAuthority::Rust,
        flags: ENHANCED_ENTITY_ACTIVE | ENHANCED_ENTITY_VISIBLE,
        transform: EnhancedTransform {
            x: i32::from(source_entity_index),
            y: i32::try_from(stable_id).unwrap_or(i32::MAX),
            ..EnhancedTransform::default()
        },
        selector: 152,
        animation_clock: 0,
        owner_colour: 0,
        life_percent: 100,
        presentation_flags: 0,
    }
}

fn event_fixture(generation: u64, tick: u64, event_id: u64) -> EnhancedEventRecord {
    EnhancedEventRecord {
        generation,
        tick,
        event_id,
        kind: EnhancedEventKind::Spell,
        family: EnhancedFamily::SpellPickup,
        source_entity_index: 10,
        transform: EnhancedTransform::default(),
        value_0: 1,
        value_1: 2,
        flags: ENHANCED_EVENT_PRESENTATION_ONLY,
    }
}

fn frame_state_fixture(generation: u64, tick: u64) -> EnhancedFrameState {
    let mut frame = EnhancedFrameState::for_frame(generation, tick);
    frame.profile.quality_flags = 0b11;
    frame.terrain_revision = tick + 1;
    frame.terrain_hash = 0x5445_5252_4149_4e31 ^ tick;
    frame.dirty_regions = vec![
        EnhancedDirtyRegion {
            x: 128,
            y: 96,
            width: 256,
            height: 192,
            kind: 1,
            flags: 1,
            source_revision: u32::try_from(tick + 1).unwrap(),
            ..EnhancedDirtyRegion::default()
        },
        EnhancedDirtyRegion {
            x: 768,
            y: 384,
            width: 128,
            height: 128,
            kind: 2,
            flags: 0,
            source_revision: u32::try_from(tick + 1).unwrap(),
            ..EnhancedDirtyRegion::default()
        },
    ]
    .into_boxed_slice();
    frame.camera = sim_core::enhanced_presentation::EnhancedCameraPose {
        transform: EnhancedTransform {
            x: 1024,
            y: 768,
            z: 512,
            yaw: 64,
            pitch: -8,
            roll: 0,
        },
        fov: 70,
        eye_height: 48,
        slope_target_x: 12,
        slope_target_y: -4,
        flags: 1,
    };
    frame.player = EnhancedTransform {
        x: 1024,
        y: 768,
        z: 384,
        yaw: 64,
        ..EnhancedTransform::default()
    };
    frame.hud = sim_core::enhanced_presentation::EnhancedHudState {
        life_percent: 92,
        mana_percent: 67,
        selected_spell: 3,
        selected_tier: 2,
        left_spell: 1,
        right_spell: 4,
        objective_selector: 7,
        objective_textbox_ticks: 12,
        flags: 0b101,
        cursor_x: 640,
        cursor_y: 512,
        menu_state: 0,
        ..sim_core::enhanced_presentation::EnhancedHudState::default()
    };
    frame.minimap_marks = vec![
        EnhancedMinimapMark {
            stable_id: 2,
            x: 24,
            y: -8,
            colour: 12,
            selector: 3,
            flags: 1,
        },
        EnhancedMinimapMark {
            stable_id: 4,
            x: -16,
            y: 20,
            colour: 31,
            selector: 8,
            flags: 0,
        },
    ]
    .into_boxed_slice();
    frame.audio_references = vec![EnhancedAudioReference {
        event_id: (tick << 32) | 1,
        sound_id: 17,
        entity_index: 10,
        volume_q15: 24_000,
        pan_q16: 32_768,
        playback_rate_hz: 44_100,
        position: frame.player,
        play_type: 1,
        loop_count: 0,
        flags: 1,
    }]
    .into_boxed_slice();
    frame
}

fn fixture_ledger(
    generation: u64,
    tick: u64,
) -> Result<EnhancedPresentationLedger, EnhancedAbiError> {
    if tick > 1_024 {
        return Err(EnhancedAbiError::new(
            MC_ENHANCED_INVALID_ARGUMENT,
            "Enhanced fixture tick exceeds the bounded fixture horizon",
        ));
    }
    let mut ledger = EnhancedPresentationLedger::new(generation, 64)
        .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
    for frame_tick in 0..=tick {
        ledger
            .begin_frame(generation, frame_tick)
            .map_err(|error| from_core(error, MC_ENHANCED_GENERATION_MISMATCH))?;
        if frame_tick == tick {
            for ordinal in 0_u16..4 {
                ledger
                    .upsert_entity(entity_fixture(
                        generation,
                        u64::from(ordinal + 1),
                        10 + ordinal,
                    ))
                    .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
            }
            for ordinal in 0_u64..2 {
                ledger
                    .push_event(event_fixture(generation, tick, ordinal + 1))
                    .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
            }
            ledger
                .set_frame_state(frame_state_fixture(generation, tick))
                .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
        }
        ledger
            .commit()
            .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
    }
    Ok(ledger)
}

fn entity_to_abi(entity: &EnhancedEntityRecord) -> McEnhancedEntityStateV1 {
    McEnhancedEntityStateV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedEntityStateV1>()).unwrap(),
        generation: entity.generation,
        stable_id: entity.stable_id,
        source_entity_index: entity.source_entity_index,
        family: entity.family.code(),
        authority: entity.authority as u8,
        flags: entity.flags,
        x: entity.transform.x,
        y: entity.transform.y,
        z: entity.transform.z,
        yaw: entity.transform.yaw,
        pitch: entity.transform.pitch,
        roll: entity.transform.roll,
        selector: entity.selector,
        animation_clock: entity.animation_clock,
        owner_colour: entity.owner_colour,
        life_percent: entity.life_percent,
        reserved_0: 0,
        presentation_flags: entity.presentation_flags,
        reserved: [0; 3],
    }
}

fn event_to_abi(event: &EnhancedEventRecord) -> McEnhancedEventStateV1 {
    McEnhancedEventStateV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedEventStateV1>()).unwrap(),
        generation: event.generation,
        tick: event.tick,
        event_id: event.event_id,
        kind: event.kind.code(),
        family: event.family.code(),
        reserved_0: 0,
        source_entity_index: event.source_entity_index,
        reserved_1: 0,
        x: event.transform.x,
        y: event.transform.y,
        z: event.transform.z,
        yaw: event.transform.yaw,
        pitch: event.transform.pitch,
        roll: event.transform.roll,
        reserved_2: 0,
        value_0: event.value_0,
        value_1: event.value_1,
        flags: event.flags,
        reserved: [0; 3],
    }
}

fn frame_state_to_abi(frame: &EnhancedFrameState, reset_epoch: u64) -> McEnhancedFrameStateV1 {
    McEnhancedFrameStateV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedFrameStateV1>()).unwrap(),
        generation: frame.generation,
        tick: frame.tick,
        reset_epoch,
        profile_id: frame.profile.mode_id,
        asset_tier: frame.profile.asset_tier,
        quality_flags: frame.profile.quality_flags,
        profile_revision: frame.profile.profile_revision,
        internal_width: frame.profile.internal_width,
        internal_height: frame.profile.internal_height,
        viewport_x: frame.profile.viewport_x,
        viewport_y: frame.profile.viewport_y,
        viewport_width: frame.profile.viewport_width,
        viewport_height: frame.profile.viewport_height,
        terrain_revision: frame.terrain_revision,
        terrain_hash: frame.terrain_hash,
        dirty_regions_written: u32::try_from(frame.dirty_regions.len()).unwrap(),
        minimap_marks_written: u32::try_from(frame.minimap_marks.len()).unwrap(),
        audio_references_written: u32::try_from(frame.audio_references.len()).unwrap(),
        camera_x: frame.camera.transform.x,
        camera_y: frame.camera.transform.y,
        camera_z: frame.camera.transform.z,
        camera_yaw: frame.camera.transform.yaw,
        camera_pitch: frame.camera.transform.pitch,
        camera_roll: frame.camera.transform.roll,
        camera_fov: frame.camera.fov,
        camera_eye_height: frame.camera.eye_height,
        camera_slope_target_x: frame.camera.slope_target_x,
        camera_slope_target_y: frame.camera.slope_target_y,
        player_x: frame.player.x,
        player_y: frame.player.y,
        player_z: frame.player.z,
        player_yaw: frame.player.yaw,
        player_pitch: frame.player.pitch,
        player_roll: frame.player.roll,
        life_percent: frame.hud.life_percent,
        mana_percent: frame.hud.mana_percent,
        selected_spell: frame.hud.selected_spell,
        selected_tier: frame.hud.selected_tier,
        left_spell: frame.hud.left_spell,
        right_spell: frame.hud.right_spell,
        objective_selector: frame.hud.objective_selector,
        objective_textbox_ticks: frame.hud.objective_textbox_ticks,
        hud_flags: frame.hud.flags,
        cursor_x: frame.hud.cursor_x,
        cursor_y: frame.hud.cursor_y,
        menu_state: frame.hud.menu_state,
        reserved_0: [0; 3],
        flags: 0,
        reserved: [0; 3],
    }
}

fn dirty_region_to_abi(region: &EnhancedDirtyRegion) -> McEnhancedDirtyRegionV1 {
    McEnhancedDirtyRegionV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedDirtyRegionV1>()).unwrap(),
        x: region.x,
        y: region.y,
        width: region.width,
        height: region.height,
        kind: region.kind,
        flags: region.flags,
        reserved_0: region.reserved,
        source_revision: region.source_revision,
        reserved: [0; 3],
    }
}

fn minimap_mark_to_abi(mark: &EnhancedMinimapMark) -> McEnhancedMinimapMarkV1 {
    McEnhancedMinimapMarkV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedMinimapMarkV1>()).unwrap(),
        stable_id: mark.stable_id,
        x: mark.x,
        y: mark.y,
        colour: mark.colour,
        selector: mark.selector,
        flags: mark.flags,
        reserved: [0; 3],
    }
}

fn audio_reference_to_abi(reference: &EnhancedAudioReference) -> McEnhancedAudioReferenceV1 {
    McEnhancedAudioReferenceV1 {
        abi_version: MC_ENHANCED_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedAudioReferenceV1>()).unwrap(),
        event_id: reference.event_id,
        sound_id: reference.sound_id,
        entity_index: reference.entity_index,
        volume_q15: reference.volume_q15,
        pan_q16: reference.pan_q16,
        playback_rate_hz: reference.playback_rate_hz,
        x: reference.position.x,
        y: reference.position.y,
        z: reference.position.z,
        yaw: reference.position.yaw,
        pitch: reference.position.pitch,
        roll: reference.position.roll,
        play_type: reference.play_type,
        loop_count: reference.loop_count,
        flags: reference.flags,
        reserved: [0; 3],
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_enhanced_abi_version() -> u32 {
    MC_ENHANCED_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_enhanced_family_count() -> u32 {
    u32::try_from(ENHANCED_FAMILY_COUNT).unwrap()
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_enhanced_family_info(
    family_code: u8,
    output: *mut McEnhancedFamilyInfoV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        let family = EnhancedFamily::try_from_code(family_code)
            .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
        let flags = if family.is_presentable() {
            MC_ENHANCED_FAMILY_FLAG_PRESENTABLE
        } else {
            MC_ENHANCED_FAMILY_FLAG_EXPLICIT_EXCLUSION
        };
        write_output(
            output,
            output_size,
            McEnhancedFamilyInfoV1 {
                abi_version: MC_ENHANCED_ABI_VERSION,
                struct_size: u32::try_from(size_of::<McEnhancedFamilyInfoV1>()).unwrap(),
                family_code,
                flags: u8::try_from(flags).unwrap(),
                reserved_0: 0,
                reserved: [0; 4],
            },
        )
    })
}

/// Copy a bounded deterministic page through the same ledger-backed record
/// contract used by the future live NativeSession bridge.
#[unsafe(no_mangle)]
pub extern "C" fn mc_unity_enhanced_snapshot_page_fixture(
    query_input: *const McEnhancedSnapshotQueryV1,
    entity_buffer: *mut McEnhancedEntityStateV1,
    entity_capacity: u32,
    event_buffer: *mut McEnhancedEventStateV1,
    event_capacity: u32,
    output: *mut McEnhancedSnapshotInfoV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        if query_input.is_null() {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_NULL_POINTER,
                "Enhanced query pointer is null",
            ));
        }
        let query = unsafe { query_input.read_unaligned() };
        validate_query(&query)?;
        let ledger = fixture_ledger(query.generation, query.tick)?;
        let page = ledger
            .snapshot_page(
                query.generation,
                query.tick,
                query.entity_cursor,
                query.event_cursor,
                query.max_entities,
                query.max_events,
            )
            .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
        if entity_capacity < u32::try_from(page.entities.len()).unwrap()
            || event_capacity < u32::try_from(page.events.len()).unwrap()
        {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_BUFFER_TOO_SMALL,
                format!(
                    "Enhanced page needs {} entities and {} events",
                    page.entities.len(),
                    page.events.len()
                ),
            ));
        }
        if (!page.entities.is_empty() && entity_buffer.is_null())
            || (!page.events.is_empty() && event_buffer.is_null())
        {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_NULL_POINTER,
                "Enhanced page record buffer is null",
            ));
        }
        validate_output(output, output_size)?;
        if !page.entities.is_empty() {
            let output_entities =
                unsafe { slice::from_raw_parts_mut(entity_buffer, page.entities.len()) };
            for (destination, source) in output_entities.iter_mut().zip(page.entities.iter()) {
                *destination = entity_to_abi(source);
            }
        }
        if !page.events.is_empty() {
            let output_events =
                unsafe { slice::from_raw_parts_mut(event_buffer, page.events.len()) };
            for (destination, source) in output_events.iter_mut().zip(page.events.iter()) {
                *destination = event_to_abi(source);
            }
        }
        write_output(
            output,
            output_size,
            McEnhancedSnapshotInfoV1 {
                abi_version: MC_ENHANCED_ABI_VERSION,
                struct_size: u32::try_from(size_of::<McEnhancedSnapshotInfoV1>()).unwrap(),
                generation: page.generation,
                tick: page.tick,
                reset_epoch: page.reset_epoch,
                entity_cursor: page.entity_cursor,
                event_cursor: page.event_cursor,
                next_entity_cursor: page.next_entity_cursor,
                next_event_cursor: page.next_event_cursor,
                entities_written: u32::try_from(page.entities.len()).unwrap(),
                events_written: u32::try_from(page.events.len()).unwrap(),
                entity_bytes: u64::try_from(
                    page.entities.len() * size_of::<McEnhancedEntityStateV1>(),
                )
                .unwrap(),
                event_bytes: u64::try_from(page.events.len() * size_of::<McEnhancedEventStateV1>())
                    .unwrap(),
                state_hash: page.state_hash,
                event_hash: page.event_hash,
                receipt_hash: page.receipt_hash,
                flags: MC_ENHANCED_SNAPSHOT_FLAG_FIXTURE,
                reserved: [0; 3],
            },
        )
    })
}

/// Copy the fixed frame contract and its bounded dirty/minimap/audio sidecars.
/// Unity consumes these records after the Rust session has committed a frame;
/// none of the returned fields are writable inputs to simulation.
#[unsafe(no_mangle)]
pub extern "C" fn mc_unity_enhanced_frame_state_fixture(
    query_input: *const McEnhancedSnapshotQueryV1,
    dirty_region_buffer: *mut McEnhancedDirtyRegionV1,
    dirty_region_capacity: u32,
    minimap_buffer: *mut McEnhancedMinimapMarkV1,
    minimap_capacity: u32,
    audio_buffer: *mut McEnhancedAudioReferenceV1,
    audio_capacity: u32,
    output: *mut McEnhancedFrameStateV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        if query_input.is_null() {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_NULL_POINTER,
                "Enhanced frame query pointer is null",
            ));
        }
        let query = unsafe { query_input.read_unaligned() };
        validate_query(&query)?;
        let ledger = fixture_ledger(query.generation, query.tick)?;
        let snapshot = ledger
            .snapshot()
            .map_err(|error| from_core(error, MC_ENHANCED_INVALID_ARGUMENT))?;
        let frame = &snapshot.frame;
        let dirty_count = u32::try_from(frame.dirty_regions.len()).unwrap();
        let minimap_count = u32::try_from(frame.minimap_marks.len()).unwrap();
        let audio_count = u32::try_from(frame.audio_references.len()).unwrap();
        if dirty_region_capacity < dirty_count
            || minimap_capacity < minimap_count
            || audio_capacity < audio_count
        {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_BUFFER_TOO_SMALL,
                format!(
                    "Enhanced frame needs {dirty_count} dirty regions, {minimap_count} minimap marks, and {audio_count} audio references"
                ),
            ));
        }
        if (dirty_count > 0 && dirty_region_buffer.is_null())
            || (minimap_count > 0 && minimap_buffer.is_null())
            || (audio_count > 0 && audio_buffer.is_null())
        {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_NULL_POINTER,
                "Enhanced frame sidecar buffer is null",
            ));
        }
        validate_output(output, output_size)?;
        if dirty_count > 0 {
            let destination = unsafe {
                slice::from_raw_parts_mut(
                    dirty_region_buffer,
                    usize::try_from(dirty_count).unwrap(),
                )
            };
            for (slot, region) in destination.iter_mut().zip(frame.dirty_regions.iter()) {
                *slot = dirty_region_to_abi(region);
            }
        }
        if minimap_count > 0 {
            let destination = unsafe {
                slice::from_raw_parts_mut(minimap_buffer, usize::try_from(minimap_count).unwrap())
            };
            for (slot, mark) in destination.iter_mut().zip(frame.minimap_marks.iter()) {
                *slot = minimap_mark_to_abi(mark);
            }
        }
        if audio_count > 0 {
            let destination = unsafe {
                slice::from_raw_parts_mut(audio_buffer, usize::try_from(audio_count).unwrap())
            };
            for (slot, reference) in destination.iter_mut().zip(frame.audio_references.iter()) {
                *slot = audio_reference_to_abi(reference);
            }
        }
        write_output(
            output,
            output_size,
            frame_state_to_abi(frame, snapshot.reset_epoch),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_enhanced_last_error_length() -> u32 {
    LAST_ERROR.with(|error| u32::try_from(error.borrow().len()).unwrap_or(u32::MAX))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_enhanced_last_error_copy(
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        if out_written.is_null() {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_NULL_POINTER,
                "Enhanced last-error output pointer is null",
            ));
        }
        let bytes = LAST_ERROR.with(|error| error.borrow().as_bytes().to_vec());
        let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        if capacity < length || (length > 0 && buffer.is_null()) {
            return Err(EnhancedAbiError::new(
                MC_ENHANCED_BUFFER_TOO_SMALL,
                format!("Enhanced last error requires {length} bytes"),
            ));
        }
        unsafe {
            if length > 0 {
                buffer.copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
            }
            out_written.write_unaligned(length);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_descriptor_is_complete_and_exclusions_are_explicit() {
        assert_eq!(mc_enhanced_abi_version(), 1);
        assert_eq!(mc_enhanced_family_count(), 36);
        for code in 0..mc_enhanced_family_count() as u8 {
            let mut info = McEnhancedFamilyInfoV1::default();
            assert_eq!(
                mc_enhanced_family_info(
                    code,
                    &mut info,
                    size_of::<McEnhancedFamilyInfoV1>() as u32
                ),
                MC_ENHANCED_OK
            );
            assert_eq!(info.family_code, code);
            assert_ne!(info.flags, 0);
        }
        let mut invalid = McEnhancedFamilyInfoV1::default();
        assert_eq!(
            mc_enhanced_family_info(
                255,
                &mut invalid,
                size_of::<McEnhancedFamilyInfoV1>() as u32
            ),
            MC_ENHANCED_INVALID_ARGUMENT
        );
    }

    #[test]
    fn page_bridge_is_source_ordered_deterministic_and_size_queried() {
        let query = McEnhancedSnapshotQueryV1 {
            abi_version: MC_ENHANCED_ABI_VERSION,
            struct_size: size_of::<McEnhancedSnapshotQueryV1>() as u32,
            generation: 9,
            tick: 2,
            profile_id: ENHANCED_PROFILE_UNITY,
            profile_revision: ENHANCED_PROFILE_REVISION,
            entity_cursor: 1,
            event_cursor: 0,
            max_entities: 2,
            max_events: 1,
            ..McEnhancedSnapshotQueryV1::default()
        };
        let mut entities = vec![McEnhancedEntityStateV1::default(); 2];
        let mut events = vec![McEnhancedEventStateV1::default(); 1];
        let mut info = McEnhancedSnapshotInfoV1::default();
        let status = mc_unity_enhanced_snapshot_page_fixture(
            &query,
            entities.as_mut_ptr(),
            entities.len() as u32,
            events.as_mut_ptr(),
            events.len() as u32,
            &mut info,
            size_of::<McEnhancedSnapshotInfoV1>() as u32,
        );
        assert_eq!(status, MC_ENHANCED_OK);
        assert_eq!(info.entities_written, 2);
        assert_eq!(info.events_written, 1);
        assert_eq!(entities[0].source_entity_index, 11);
        assert_eq!(entities[1].source_entity_index, 12);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(info.flags, MC_ENHANCED_SNAPSHOT_FLAG_FIXTURE);
        assert_ne!(info.receipt_hash, 0);
        let mut second_entities = vec![McEnhancedEntityStateV1::default(); 2];
        let mut second_events = vec![McEnhancedEventStateV1::default(); 1];
        let mut second_info = McEnhancedSnapshotInfoV1::default();
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                &query,
                second_entities.as_mut_ptr(),
                second_entities.len() as u32,
                second_events.as_mut_ptr(),
                second_events.len() as u32,
                &mut second_info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_OK
        );
        assert_eq!(entities, second_entities);
        assert_eq!(events, second_events);
        assert_eq!(info.state_hash, second_info.state_hash);
        assert_eq!(info.receipt_hash, second_info.receipt_hash);
    }

    #[test]
    fn frame_bridge_carries_profile_terrain_camera_hud_minimap_and_audio_state() {
        let query = McEnhancedSnapshotQueryV1 {
            abi_version: MC_ENHANCED_ABI_VERSION,
            struct_size: size_of::<McEnhancedSnapshotQueryV1>() as u32,
            generation: 9,
            tick: 2,
            profile_id: ENHANCED_PROFILE_UNITY,
            profile_revision: ENHANCED_PROFILE_REVISION,
            max_entities: 0,
            max_events: 0,
            ..McEnhancedSnapshotQueryV1::default()
        };
        let mut dirty = vec![McEnhancedDirtyRegionV1::default(); 2];
        let mut minimap = vec![McEnhancedMinimapMarkV1::default(); 2];
        let mut audio = vec![McEnhancedAudioReferenceV1::default(); 1];
        let mut output = McEnhancedFrameStateV1::default();
        assert_eq!(
            mc_unity_enhanced_frame_state_fixture(
                &query,
                dirty.as_mut_ptr(),
                dirty.len() as u32,
                minimap.as_mut_ptr(),
                minimap.len() as u32,
                audio.as_mut_ptr(),
                audio.len() as u32,
                &mut output,
                size_of::<McEnhancedFrameStateV1>() as u32,
            ),
            MC_ENHANCED_OK
        );
        assert_eq!(output.profile_id, ENHANCED_PROFILE_UNITY);
        assert_eq!(output.profile_revision, ENHANCED_PROFILE_REVISION);
        assert_eq!(output.terrain_revision, 3);
        assert_eq!(output.dirty_regions_written, 2);
        assert_eq!(output.minimap_marks_written, 2);
        assert_eq!(output.audio_references_written, 1);
        assert_eq!(output.camera_x, 1024);
        assert_eq!(output.player_z, 384);
        assert_eq!(output.life_percent, 92);
        assert_eq!(dirty[0].source_revision, 3);
        assert_eq!(minimap[1].stable_id, 4);
        assert_eq!(audio[0].sound_id, 17);
    }

    #[test]
    fn page_bridge_rejects_generation_zero_stale_boundaries_and_small_buffers() {
        let mut query = McEnhancedSnapshotQueryV1 {
            abi_version: MC_ENHANCED_ABI_VERSION,
            struct_size: size_of::<McEnhancedSnapshotQueryV1>() as u32,
            generation: 1,
            tick: 0,
            profile_id: ENHANCED_PROFILE_UNITY,
            profile_revision: ENHANCED_PROFILE_REVISION,
            max_entities: 4,
            max_events: 2,
            ..McEnhancedSnapshotQueryV1::default()
        };
        let mut entities = vec![McEnhancedEntityStateV1::default(); 4];
        let mut events = vec![McEnhancedEventStateV1::default(); 2];
        let mut info = McEnhancedSnapshotInfoV1::default();
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                std::ptr::null(),
                entities.as_mut_ptr(),
                4,
                events.as_mut_ptr(),
                2,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_NULL_POINTER
        );
        query.generation = 0;
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                &query,
                entities.as_mut_ptr(),
                4,
                events.as_mut_ptr(),
                2,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_GENERATION_MISMATCH
        );
        query.generation = 1;
        query.profile_id = 255;
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                &query,
                entities.as_mut_ptr(),
                4,
                events.as_mut_ptr(),
                2,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_INVALID_ARGUMENT
        );
        query.profile_id = ENHANCED_PROFILE_UNITY;
        query.entity_cursor = 9;
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                &query,
                entities.as_mut_ptr(),
                4,
                events.as_mut_ptr(),
                2,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_INVALID_ARGUMENT
        );
        query.entity_cursor = 0;
        assert_eq!(
            mc_unity_enhanced_snapshot_page_fixture(
                &query,
                entities.as_mut_ptr(),
                1,
                events.as_mut_ptr(),
                2,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_ENHANCED_BUFFER_TOO_SMALL
        );
    }
}
