//! Deterministic, renderer-free `VoxelMode` structure and damage subsystem.
//!
//! This crate owns the immutable structure contract, mutable detail mirror,
//! damage mapping, collapse analysis, and versioned sparse snapshots. It has
//! no dependency on gameplay authority, native transport, or presentation.

pub mod voxel_collapse;
pub mod voxel_damage;
pub mod voxel_snapshot;
pub mod voxel_state;
pub mod voxel_structure;
