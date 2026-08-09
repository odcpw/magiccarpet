//! Additive size-queried presentation ABI for Classic HD and Unity Enhanced.
//!
//! This namespace is intentionally independent from ABI v1/v2.  The frame
//! and snapshot copy functions below use deterministic synthetic fixtures so
//! the contract can be tested before the real Classic and Enhanced backends
//! are bound.  They are not gameplay or owner-data endpoints.

use std::{
    cell::RefCell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    slice,
};

use sim_core::classic_hd::{
    ClassicHdProfileId, ClassicHdRenderProfile, byte_hash, checked_memory_estimate,
};

pub const MC_PRESENTATION_ABI_VERSION: u32 = 1;
pub const MC_PRESENTATION_OK: i32 = 0;
pub const MC_PRESENTATION_NULL_POINTER: i32 = 1;
pub const MC_PRESENTATION_INVALID_ARGUMENT: i32 = 3;
pub const MC_PRESENTATION_STRUCT_TOO_SMALL: i32 = 5;
pub const MC_PRESENTATION_BUFFER_TOO_SMALL: i32 = 11;
pub const MC_PRESENTATION_GENERATION_MISMATCH: i32 = 12;
pub const MC_PRESENTATION_INTERNAL_PANIC: i32 = 14;

pub const MC_CLASSIC_HD_PROFILE_COMPATIBILITY_320: u32 = 0;
pub const MC_CLASSIC_HD_PROFILE_640: u32 = 1;
pub const MC_CLASSIC_HD_PROFILE_960: u32 = 2;
pub const MC_CLASSIC_HD_PROFILE_1280: u32 = 3;
pub const MC_CLASSIC_HD_PROFILE_COUNT: u32 = 4;

pub const MC_CLASSIC_HD_FLAG_COMPATIBILITY_ONLY: u32 = 1 << 0;
pub const MC_CLASSIC_HD_FLAG_PRODUCTION_SUPPORTED: u32 = 1 << 1;
pub const MC_CLASSIC_HD_FLAG_OPTIONAL_HIGH_RES_ASSETS: u32 = 1 << 2;
pub const MC_CLASSIC_HD_FLAG_SYNTHETIC_FIXTURE: u32 = 1 << 3;

pub const MC_ENHANCED_ENTITY_FLAG_ACTIVE: u32 = 1 << 0;
pub const MC_ENHANCED_ENTITY_FLAG_VISIBLE: u32 = 1 << 1;
pub const MC_ENHANCED_EVENT_FLAG_SYNTHETIC: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McClassicHdBufferInfoV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub profile_id: u32,
    pub flags: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub viewport_x: u32,
    pub viewport_y: u32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub index_bytes: u64,
    pub provenance_bytes: u64,
    pub history_bytes: u64,
    pub profile_revision: u64,
    pub memory_bytes: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McClassicHdFrameQueryV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub profile_id: u32,
    pub reserved_0: u32,
    pub generation: u64,
    pub tick: u64,
    pub input_hash: u64,
    pub reset_epoch: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McClassicHdFrameInfoV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub profile_id: u32,
    pub flags: u32,
    pub generation: u64,
    pub tick: u64,
    pub input_hash: u64,
    pub reset_epoch: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub index_bytes: u64,
    pub provenance_bytes: u64,
    pub index_hash: u64,
    pub provenance_hash: u64,
    pub receipt_hash: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedSnapshotQueryV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub cursor: u32,
    pub max_entities: u32,
    pub max_events: u32,
    pub reserved_0: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedSnapshotInfoV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub cursor: u32,
    pub entities_written: u32,
    pub events_written: u32,
    pub entity_bytes: u64,
    pub event_bytes: u64,
    pub state_hash: u64,
    pub receipt_hash: u64,
    pub flags: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedEntityV1 {
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
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEnhancedEventV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub event_id: u64,
    pub kind: u32,
    pub flags: u32,
    pub source_entity_index: u16,
    pub family: u8,
    pub reserved_0: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub value_0: i32,
    pub value_1: i32,
    pub reserved: [u32; 4],
}

#[derive(Debug)]
struct PresentationAbiError {
    status: i32,
    message: String,
}

impl PresentationAbiError {
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
    LAST_ERROR.with(|error| {
        *error.borrow_mut() = message.into();
    });
}

fn clear_error() {
    LAST_ERROR.with(|error| error.borrow_mut().clear());
}

fn boundary(operation: impl FnOnce() -> Result<(), PresentationAbiError>) -> i32 {
    clear_error();
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => MC_PRESENTATION_OK,
        Ok(Err(error)) => {
            set_error(error.message);
            error.status
        }
        Err(_) => {
            set_error("panic contained at additive presentation ABI boundary");
            MC_PRESENTATION_INTERNAL_PANIC
        }
    }
}

fn write_output<T: Copy>(
    output: *mut T,
    output_size: u32,
    value: T,
) -> Result<(), PresentationAbiError> {
    validate_output_slot::<T>(output, output_size)?;
    unsafe { output.write_unaligned(value) };
    Ok(())
}

fn validate_output_slot<T>(output: *mut T, output_size: u32) -> Result<(), PresentationAbiError> {
    if output.is_null() {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_NULL_POINTER,
            "presentation output pointer is null",
        ));
    }
    let required = u32::try_from(size_of::<T>()).expect("ABI struct size fits u32");
    if output_size < required {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_STRUCT_TOO_SMALL,
            format!("presentation output requires {required} bytes, got {output_size}"),
        ));
    }
    Ok(())
}

fn read_query<T: Copy>(input: *const T, name: &str) -> Result<T, PresentationAbiError> {
    if input.is_null() {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    Ok(unsafe { input.read_unaligned() })
}

fn validate_header(
    abi_version: u32,
    struct_size: u32,
    expected: usize,
    name: &str,
) -> Result<(), PresentationAbiError> {
    if abi_version != MC_PRESENTATION_ABI_VERSION {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_INVALID_ARGUMENT,
            format!("{name} ABI version {abi_version} is unsupported"),
        ));
    }
    let expected = u32::try_from(expected).expect("ABI struct size fits u32");
    if struct_size < expected {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_STRUCT_TOO_SMALL,
            format!("{name} requires {expected} bytes, got {struct_size}"),
        ));
    }
    Ok(())
}

fn validate_reserved(values: &[u32], name: &str) -> Result<(), PresentationAbiError> {
    if values.iter().any(|value| *value != 0) {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_INVALID_ARGUMENT,
            format!("{name} reserved fields must be zero"),
        ));
    }
    Ok(())
}

fn profile(id: u32) -> Result<ClassicHdRenderProfile, PresentationAbiError> {
    match id {
        MC_CLASSIC_HD_PROFILE_COMPATIBILITY_320 => {
            Ok(ClassicHdRenderProfile::compatibility_320x200())
        }
        MC_CLASSIC_HD_PROFILE_640 => Ok(ClassicHdRenderProfile::hd_640x480()),
        MC_CLASSIC_HD_PROFILE_960 => Ok(ClassicHdRenderProfile::hd_960x720()),
        MC_CLASSIC_HD_PROFILE_1280 => Ok(ClassicHdRenderProfile::hd_1280x960()),
        _ => Err(PresentationAbiError::new(
            MC_PRESENTATION_INVALID_ARGUMENT,
            format!("unknown Classic HD profile id {id}"),
        )),
    }
}

fn profile_info(id: u32) -> Result<McClassicHdBufferInfoV1, PresentationAbiError> {
    let profile = profile(id)?;
    let flags = if id == MC_CLASSIC_HD_PROFILE_COMPATIBILITY_320 {
        MC_CLASSIC_HD_FLAG_COMPATIBILITY_ONLY
    } else {
        MC_CLASSIC_HD_FLAG_PRODUCTION_SUPPORTED
            | if profile.id == ClassicHdProfileId::Hd960x720
                || profile.id == ClassicHdProfileId::Hd1280x960
            {
                MC_CLASSIC_HD_FLAG_OPTIONAL_HIGH_RES_ASSETS
            } else {
                0
            }
    };
    let index_bytes = u64::from(profile.stride) * u64::from(profile.height);
    Ok(McClassicHdBufferInfoV1 {
        abi_version: MC_PRESENTATION_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McClassicHdBufferInfoV1>()).unwrap(),
        profile_id: id,
        flags,
        width: profile.width,
        height: profile.height,
        stride: profile.stride,
        viewport_x: profile.viewport.x,
        viewport_y: profile.viewport.y,
        viewport_width: profile.viewport.width,
        viewport_height: profile.viewport.height,
        index_bytes,
        provenance_bytes: index_bytes,
        history_bytes: index_bytes,
        profile_revision: profile.profile_revision,
        memory_bytes: checked_memory_estimate(profile.stride, profile.height).map_err(|error| {
            PresentationAbiError::new(MC_PRESENTATION_INVALID_ARGUMENT, error.0)
        })?,
        reserved: [0; 4],
    })
}

fn validate_frame_query(
    query: &McClassicHdFrameQueryV1,
) -> Result<ClassicHdRenderProfile, PresentationAbiError> {
    validate_header(
        query.abi_version,
        query.struct_size,
        size_of::<McClassicHdFrameQueryV1>(),
        "Classic HD frame query",
    )?;
    if query.reserved_0 != 0 {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_INVALID_ARGUMENT,
            "Classic HD frame query reserved_0 must be zero",
        ));
    }
    validate_reserved(&query.reserved, "Classic HD frame query")?;
    profile(query.profile_id)
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_presentation_abi_version() -> u32 {
    MC_PRESENTATION_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_classic_hd_profile_count() -> u32 {
    MC_CLASSIC_HD_PROFILE_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_classic_hd_profile_info(
    profile_id: u32,
    output: *mut McClassicHdBufferInfoV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        let info = profile_info(profile_id)?;
        write_output(output, output_size, info)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_classic_hd_frame_required_bytes(
    profile_id: u32,
    index_bytes: *mut u64,
    provenance_bytes: *mut u64,
    history_bytes: *mut u64,
) -> i32 {
    boundary(|| {
        let info = profile_info(profile_id)?;
        if index_bytes.is_null() || provenance_bytes.is_null() || history_bytes.is_null() {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_NULL_POINTER,
                "Classic HD required-byte output pointer is null",
            ));
        }
        unsafe {
            index_bytes.write_unaligned(info.index_bytes);
            provenance_bytes.write_unaligned(info.provenance_bytes);
            history_bytes.write_unaligned(info.history_bytes);
        }
        Ok(())
    })
}

/// Copy a deterministic frame-shaped fixture.  This proves caller-owned
/// buffer sizing, reset/generation/tick identity, and per-plane receipts; it
/// must not be presented as a real session renderer.
#[unsafe(no_mangle)]
pub extern "C" fn mc_classic_hd_frame_copy_fixture(
    query_input: *const McClassicHdFrameQueryV1,
    index_buffer: *mut u8,
    index_capacity: u64,
    provenance_buffer: *mut u8,
    provenance_capacity: u64,
    output: *mut McClassicHdFrameInfoV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        let query = read_query(query_input, "Classic HD frame query")?;
        let profile = validate_frame_query(&query)?;
        let required = u64::from(profile.stride) * u64::from(profile.height);
        if index_capacity < required || provenance_capacity < required {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_BUFFER_TOO_SMALL,
                format!("Classic HD frame requires {required} bytes per plane"),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_NULL_POINTER,
                "Classic HD frame plane pointer is null",
            ));
        }
        validate_output_slot::<McClassicHdFrameInfoV1>(output, output_size)?;
        let length = usize::try_from(required).unwrap();
        let indices = unsafe { slice::from_raw_parts_mut(index_buffer, length) };
        let provenance = unsafe { slice::from_raw_parts_mut(provenance_buffer, length) };
        for (offset, (index, origin)) in indices.iter_mut().zip(provenance.iter_mut()).enumerate() {
            *index = query
                .input_hash
                .wrapping_add(query.tick)
                .wrapping_add(offset as u64) as u8;
            *origin = ((offset / usize::try_from(profile.stride).unwrap()) as u8) ^ 0x5a;
        }
        let index_hash = byte_hash(indices);
        let provenance_hash = byte_hash(provenance);
        let receipt_hash = byte_hash(
            &[
                index_hash.to_le_bytes().as_slice(),
                provenance_hash.to_le_bytes().as_slice(),
                query.generation.to_le_bytes().as_slice(),
                query.tick.to_le_bytes().as_slice(),
            ]
            .concat(),
        );
        write_output(
            output,
            output_size,
            McClassicHdFrameInfoV1 {
                abi_version: MC_PRESENTATION_ABI_VERSION,
                struct_size: u32::try_from(size_of::<McClassicHdFrameInfoV1>()).unwrap(),
                profile_id: query.profile_id,
                flags: MC_CLASSIC_HD_FLAG_SYNTHETIC_FIXTURE,
                generation: query.generation,
                tick: query.tick,
                input_hash: query.input_hash,
                reset_epoch: query.reset_epoch,
                width: profile.width,
                height: profile.height,
                stride: profile.stride,
                index_bytes: required,
                provenance_bytes: required,
                index_hash,
                provenance_hash,
                receipt_hash,
                reserved: [0; 4],
            },
        )
    })
}

fn validate_snapshot_query(query: &McEnhancedSnapshotQueryV1) -> Result<(), PresentationAbiError> {
    validate_header(
        query.abi_version,
        query.struct_size,
        size_of::<McEnhancedSnapshotQueryV1>(),
        "Enhanced snapshot query",
    )?;
    if query.reserved_0 != 0 {
        return Err(PresentationAbiError::new(
            MC_PRESENTATION_INVALID_ARGUMENT,
            "Enhanced snapshot query reserved_0 must be zero",
        ));
    }
    validate_reserved(&query.reserved, "Enhanced snapshot query")
}

fn synthetic_entity(generation: u64, tick: u64, ordinal: u32) -> McEnhancedEntityV1 {
    McEnhancedEntityV1 {
        abi_version: MC_PRESENTATION_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedEntityV1>()).unwrap(),
        generation,
        stable_id: (u64::from(ordinal) << 32) | 0x454e_5449,
        source_entity_index: u16::try_from(ordinal).unwrap(),
        family: 1,
        authority: 0,
        flags: MC_ENHANCED_ENTITY_FLAG_ACTIVE | MC_ENHANCED_ENTITY_FLAG_VISIBLE,
        x: i32::try_from(tick).unwrap_or(i32::MAX),
        y: i32::try_from(ordinal).unwrap_or(i32::MAX),
        z: 0,
        yaw: 0,
        pitch: 0,
        roll: 0,
        selector: 0,
        animation_clock: u32::try_from(tick).unwrap_or(u32::MAX),
        owner_colour: 0,
        life_percent: 100,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn synthetic_event(generation: u64, tick: u64, ordinal: u32) -> McEnhancedEventV1 {
    McEnhancedEventV1 {
        abi_version: MC_PRESENTATION_ABI_VERSION,
        struct_size: u32::try_from(size_of::<McEnhancedEventV1>()).unwrap(),
        generation,
        tick,
        event_id: (tick << 32) | u64::from(ordinal),
        kind: 1,
        flags: MC_ENHANCED_EVENT_FLAG_SYNTHETIC,
        source_entity_index: u16::try_from(ordinal).unwrap(),
        family: 1,
        reserved_0: 0,
        x: 0,
        y: 0,
        z: 0,
        value_0: 0,
        value_1: 0,
        reserved: [0; 4],
    }
}

/// Copy a deterministic fixed-record snapshot fixture.  Real snapshots will
/// bind this exact size-query and generation contract in GFX-Q.
#[unsafe(no_mangle)]
pub extern "C" fn mc_unity_enhanced_snapshot_copy_fixture(
    query_input: *const McEnhancedSnapshotQueryV1,
    entity_buffer: *mut McEnhancedEntityV1,
    entity_capacity: u32,
    event_buffer: *mut McEnhancedEventV1,
    event_capacity: u32,
    output: *mut McEnhancedSnapshotInfoV1,
    output_size: u32,
) -> i32 {
    boundary(|| {
        let query = read_query(query_input, "Enhanced snapshot query")?;
        validate_snapshot_query(&query)?;
        if query.generation == 0 {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_GENERATION_MISMATCH,
                "Enhanced snapshot generation zero is invalid",
            ));
        }
        let entity_count = query.max_entities.min(4);
        let event_count = query.max_events.min(4);
        if entity_capacity < entity_count || event_capacity < event_count {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_BUFFER_TOO_SMALL,
                format!(
                    "Enhanced snapshot requires {entity_count} entities and {event_count} events"
                ),
            ));
        }
        if (entity_count > 0 && entity_buffer.is_null())
            || (event_count > 0 && event_buffer.is_null())
        {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_NULL_POINTER,
                "Enhanced snapshot record buffer is null",
            ));
        }
        validate_output_slot::<McEnhancedSnapshotInfoV1>(output, output_size)?;
        let entities: &mut [McEnhancedEntityV1] = if entity_count == 0 {
            &mut []
        } else {
            unsafe { slice::from_raw_parts_mut(entity_buffer, entity_count as usize) }
        };
        let events: &mut [McEnhancedEventV1] = if event_count == 0 {
            &mut []
        } else {
            unsafe { slice::from_raw_parts_mut(event_buffer, event_count as usize) }
        };
        for (ordinal, entity) in entities.iter_mut().enumerate() {
            *entity = synthetic_entity(query.generation, query.tick, query.cursor + ordinal as u32);
        }
        for (ordinal, event) in events.iter_mut().enumerate() {
            *event = synthetic_event(query.generation, query.tick, query.cursor + ordinal as u32);
        }
        let entity_bytes =
            u64::from(entity_count) * u64::try_from(size_of::<McEnhancedEntityV1>()).unwrap();
        let event_bytes =
            u64::from(event_count) * u64::try_from(size_of::<McEnhancedEventV1>()).unwrap();
        let state_hash = if entities.is_empty() {
            0
        } else {
            byte_hash(unsafe {
                slice::from_raw_parts(
                    entities.as_ptr().cast::<u8>(),
                    usize::try_from(entity_bytes).unwrap(),
                )
            })
        };
        let receipt_hash = if events.is_empty() {
            0
        } else {
            byte_hash(unsafe {
                slice::from_raw_parts(
                    events.as_ptr().cast::<u8>(),
                    usize::try_from(event_bytes).unwrap(),
                )
            })
        };
        write_output(
            output,
            output_size,
            McEnhancedSnapshotInfoV1 {
                abi_version: MC_PRESENTATION_ABI_VERSION,
                struct_size: u32::try_from(size_of::<McEnhancedSnapshotInfoV1>()).unwrap(),
                generation: query.generation,
                tick: query.tick,
                cursor: query.cursor,
                entities_written: entity_count,
                events_written: event_count,
                entity_bytes,
                event_bytes,
                state_hash,
                receipt_hash,
                flags: 1,
                reserved: [0; 4],
            },
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_presentation_last_error_length() -> u32 {
    LAST_ERROR.with(|error| u32::try_from(error.borrow().len()).unwrap_or(u32::MAX))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_presentation_last_error_copy(
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        if out_written.is_null() {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_NULL_POINTER,
                "presentation last-error length pointer is null",
            ));
        }
        let message = LAST_ERROR.with(|error| error.borrow().as_bytes().to_vec());
        let length = u32::try_from(message.len()).unwrap_or(u32::MAX);
        if capacity < length || (length > 0 && buffer.is_null()) {
            return Err(PresentationAbiError::new(
                MC_PRESENTATION_BUFFER_TOO_SMALL,
                format!("presentation last error requires {length} bytes"),
            ));
        }
        unsafe {
            if length > 0 {
                buffer.copy_from_nonoverlapping(message.as_ptr(), message.len());
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
    fn profile_matrix_is_size_queried_and_namespaced() {
        assert_eq!(mc_presentation_abi_version(), 1);
        assert_eq!(mc_classic_hd_profile_count(), 4);
        for id in 0..mc_classic_hd_profile_count() {
            let mut info = McClassicHdBufferInfoV1::default();
            assert_eq!(
                mc_classic_hd_profile_info(
                    id,
                    &mut info,
                    size_of::<McClassicHdBufferInfoV1>() as u32
                ),
                MC_PRESENTATION_OK
            );
            assert_eq!(info.abi_version, MC_PRESENTATION_ABI_VERSION);
            assert_eq!(
                info.struct_size as usize,
                size_of::<McClassicHdBufferInfoV1>()
            );
            assert_eq!(
                info.index_bytes,
                u64::from(info.stride) * u64::from(info.height)
            );
            assert_eq!(info.provenance_bytes, info.index_bytes);
            assert_eq!(info.history_bytes, info.index_bytes);
        }
    }

    #[test]
    fn frame_fixture_proves_capacity_generation_and_receipt_contract() {
        let profile = profile_info(MC_CLASSIC_HD_PROFILE_640).unwrap();
        let mut query = McClassicHdFrameQueryV1 {
            abi_version: MC_PRESENTATION_ABI_VERSION,
            struct_size: size_of::<McClassicHdFrameQueryV1>() as u32,
            profile_id: MC_CLASSIC_HD_PROFILE_640,
            generation: 7,
            tick: 3,
            input_hash: 0x55,
            reset_epoch: 2,
            ..McClassicHdFrameQueryV1::default()
        };
        let mut indices = vec![0; profile.index_bytes as usize];
        let mut provenance = vec![0; profile.provenance_bytes as usize];
        let mut info = McClassicHdFrameInfoV1::default();
        assert_eq!(
            mc_classic_hd_frame_copy_fixture(
                &query,
                indices.as_mut_ptr(),
                indices.len() as u64,
                provenance.as_mut_ptr(),
                provenance.len() as u64,
                &mut info,
                size_of::<McClassicHdFrameInfoV1>() as u32,
            ),
            MC_PRESENTATION_OK
        );
        assert_eq!(info.generation, 7);
        assert_eq!(info.tick, 3);
        assert_eq!(info.index_hash, byte_hash(&indices));
        assert_ne!(info.receipt_hash, 0);
        query.tick = 4;
        assert_eq!(
            mc_classic_hd_frame_copy_fixture(
                &query,
                indices.as_mut_ptr(),
                1,
                provenance.as_mut_ptr(),
                provenance.len() as u64,
                &mut info,
                size_of::<McClassicHdFrameInfoV1>() as u32,
            ),
            MC_PRESENTATION_BUFFER_TOO_SMALL
        );
    }

    #[test]
    fn enhanced_fixture_uses_fixed_records_and_zero_reserved_fields() {
        let query = McEnhancedSnapshotQueryV1 {
            abi_version: MC_PRESENTATION_ABI_VERSION,
            struct_size: size_of::<McEnhancedSnapshotQueryV1>() as u32,
            generation: 2,
            tick: 9,
            max_entities: 2,
            max_events: 1,
            ..McEnhancedSnapshotQueryV1::default()
        };
        let mut entities = vec![McEnhancedEntityV1::default(); 2];
        let mut events = vec![McEnhancedEventV1::default(); 1];
        let mut info = McEnhancedSnapshotInfoV1::default();
        assert_eq!(
            mc_unity_enhanced_snapshot_copy_fixture(
                &query,
                entities.as_mut_ptr(),
                2,
                events.as_mut_ptr(),
                1,
                &mut info,
                size_of::<McEnhancedSnapshotInfoV1>() as u32,
            ),
            MC_PRESENTATION_OK
        );
        assert_eq!(info.entities_written, 2);
        assert_eq!(info.events_written, 1);
        assert_eq!(entities[0].generation, 2);
        assert_ne!(info.state_hash, 0);
        assert_eq!(entities[0].reserved, [0; 4]);
        assert_eq!(events[0].reserved, [0; 4]);
    }
}
