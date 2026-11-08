//! Guarded observations of the authoritative source BUILD carriers.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct McRuntimeBuildingV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub slot_generation: u32,
    pub source_slot: u16,
    pub source_id: u16,
    pub building_index: u8,
    pub action: u8,
    pub active: u8,
    pub drawing: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub reserved: [u16; 3],
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_buildings_copy_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut McRuntimeBuildingV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "building count")?;
        write_output(out_written, 0);
        require_element_size::<McRuntimeBuildingV1>(element_size, "building element")?;
        with_runtime(handle, |record| {
            let session = record.session()?;
            let identity = session.identity();
            if identity.generation != generation || identity.tick != expected_tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale building observation",
                ));
            }
            let rows = session.backend().live_building_observations();
            let count = u32::try_from(rows.len())
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "building count exceeds u32"))?;
            write_output(out_written, count);
            if capacity < count {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "building buffer is too small",
                ));
            }
            if count != 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "building buffer is null",
                ));
            }
            for (index, row) in rows.into_iter().enumerate() {
                let output = McRuntimeBuildingV1 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McRuntimeBuildingV1>(),
                    generation,
                    tick: expected_tick,
                    slot_generation: row.slot_generation,
                    source_slot: row.source_slot,
                    source_id: row.source_id,
                    building_index: row.building_index,
                    action: row.action,
                    active: u8::from(row.active),
                    drawing: u8::from(row.drawing),
                    x: row.x,
                    y: row.y,
                    z: row.z,
                    reserved: [0; 3],
                };
                // SAFETY: the caller supplied count elements of the declared
                // exact size; unaligned writes are supported by this ABI.
                unsafe { buffer.add(index).write_unaligned(output) };
            }
            Ok(())
        })
    })
}
