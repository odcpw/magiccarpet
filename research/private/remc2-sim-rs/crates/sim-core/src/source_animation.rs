//! Source TMAP residency and the ordered animation-record pool.
//!
//! This owns the state observed by source gameplay animation-record reads.
//! Decoded files describe available assets; only source preload or admitted
//! render demand makes them resident. It does not emulate animated pixels.

use sha2::{Digest, Sha256};

use crate::native_world::PresentationSelectors;

pub(crate) const SOURCE_TMAP_COUNT: usize = 504;
// sub_83B50's active allocation, less sub_71B40's source descriptor arrays.
// The complete admitted bank must fit: this bounded owner then cannot enter
// MainInitTmaps' pressure-eviction branch, whatever the demand order.
const SOURCE_HEAP_BYTES: u64 = 0x0140_0000;
// Packed engine_support.h descriptors: reserve the larger 64-bit layout
// (18-byte metadata +8-byte pointer per selector, plus38-byte owner header).
// This also bounds the original 32-bit layout used by the source comments.
const SOURCE_DESCRIPTOR_BYTES: u64 = 504 * (18 + 8) + 38;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTmapMetadata {
    pub selector: u16,
    pub group: u16,
    pub allocation_bytes: u32,
    pub encoded_frame_count: Option<u16>,
}

impl SourceTmapMetadata {
    pub(crate) fn from_decoded(selector: u16, group: u16, decoded: &[u8]) -> Result<Self, String> {
        let allocation_bytes = u32::try_from((decoded.len() + 13) / 4 * 4)
            .map_err(|_| "source TMAP allocation exceeds u32")?;
        let encoded_frame_count = if decoded == b"0" {
            None
        } else {
            if decoded.len() < 6 {
                return Err(format!("source TMAP{selector} lacks its header"));
            }
            let word = |at| u16::from_le_bytes([decoded[at], decoded[at + 1]]);
            let end = 6 + usize::from(word(2)) * usize::from(word(4));
            if end > decoded.len() {
                return Err(format!("source TMAP{selector} pixels exceed its payload"));
            }
            if word(0) & 1 == 0 {
                None
            } else {
                let bytes = decoded
                    .get(end..end + 2)
                    .ok_or_else(|| format!("source TMAP{selector} lacks its animation header"))?;
                let count = u16::from_le_bytes(bytes.try_into().unwrap());
                if count == 0 {
                    return Err(format!("source TMAP{selector} has zero animation frames"));
                }
                Some(count)
            }
        };
        Ok(Self {
            selector,
            group,
            allocation_bytes,
            encoded_frame_count,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTmapCatalog {
    records: Box<[SourceTmapMetadata]>,
}

impl SourceTmapCatalog {
    pub(crate) fn new(mut records: Vec<SourceTmapMetadata>) -> Result<Self, String> {
        records.sort_unstable_by_key(|row| row.selector);
        if records.len() != SOURCE_TMAP_COUNT
            || records.iter().enumerate().any(|(i, row)| {
                usize::from(row.selector) != i
                    || row.group > row.selector
                    || records
                        .get(usize::from(row.group))
                        .is_none_or(|base| base.group != row.group)
            })
        {
            return Err(
                "source TMAP catalog requires the complete ordered504-entry TAB groups".into(),
            );
        }
        for (index, row) in records.iter().enumerate() {
            if row.allocation_bytes > u32::from(u16::MAX) {
                return Err(format!(
                    "source TMAP{} allocation would truncate at InitTmaps' u16 call",
                    row.selector
                ));
            }
            if index > usize::from(row.group) && records[index - 1].group != row.group {
                return Err(format!(
                    "source TMAP{} has a noncontiguous TAB group",
                    row.selector
                ));
            }
        }
        if records
            .iter()
            .filter(|row| row.encoded_frame_count.is_some())
            .count()
            >= SOURCE_TMAP_COUNT
        {
            return Err("source TMAP bank exceeds the admissible503-record animation pool".into());
        }
        let total: u64 = records
            .iter()
            .map(|row| u64::from(row.allocation_bytes))
            .sum();
        // MainInitTmaps also compares its literal sub_70EF0 group-index sum,
        // plus20, to free bytes before allocating. Bound that separately.
        let largest_probe = records
            .iter()
            .map(|row| {
                records
                    .iter()
                    .filter(|other| other.group == row.group)
                    .map(|other| u64::from(other.group))
                    .sum::<u64>()
                    + 20
            })
            .max()
            .unwrap_or(0);
        if total + SOURCE_DESCRIPTOR_BYTES + largest_probe >= SOURCE_HEAP_BYTES {
            return Err(format!(
                "source TMAP bank needs pressure eviction: allocation={total}, probe={largest_probe}"
            ));
        }
        // LoadTMapMetadata's dword_16x is uint32_t*: its literal addition
        // scales the consumed-byte count by4. Bound physical placement for
        // every demand order separately from the allocator's logical budget.
        if SOURCE_DESCRIPTOR_BYTES + 4 * total > SOURCE_HEAP_BYTES {
            return Err(format!(
                "source TMAP bank has undefined physical heap placement: allocation={total}, displaced={}",
                4 * total
            ));
        }
        Ok(Self {
            records: records.into_boxed_slice(),
        })
    }

    pub(crate) fn record(&self, selector: u16) -> Option<&SourceTmapMetadata> {
        self.records.get(usize::from(selector))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceAnimationResidency {
    catalog: Option<SourceTmapCatalog>,
    resident: Box<[bool]>,
    // sub_72120 initializes dword0 and Particles to zero. sub_721C0 fills
    // the last free record with dword0=1; ResetAnimation clears both fields.
    animation_slots: Box<[Option<u16>]>,
    permanent: Box<[bool]>,
}

impl Default for SourceAnimationResidency {
    fn default() -> Self {
        Self {
            catalog: None,
            resident: vec![false; SOURCE_TMAP_COUNT].into_boxed_slice(),
            animation_slots: vec![None; SOURCE_TMAP_COUNT].into_boxed_slice(),
            permanent: vec![false; SOURCE_TMAP_COUNT].into_boxed_slice(),
        }
    }
}

impl SourceAnimationResidency {
    pub(crate) fn initialize(
        catalog: SourceTmapCatalog,
        presentation: &PresentationSelectors,
        authored: &[(u16, i16)],
    ) -> Result<Self, String> {
        let mut state = Self {
            catalog: Some(catalog),
            ..Self::default()
        };
        // sub_71890/718F0: permanent base selectors, scanned before the
        // authored sub_4A1E0(0) priority preload.
        for particle in presentation
            .particles
            .iter()
            .take_while(|p| p.speed != 0 || p.rotation_speed != 0)
        {
            if particle.byte_11 == -1 {
                let flag = state
                    .permanent
                    .get_mut(usize::from(particle.base_tmap))
                    .ok_or("source permanent preload selector exceeds504")?;
                *flag = true;
            }
        }
        for selector in 0..SOURCE_TMAP_COUNT {
            if state.permanent[selector] {
                state.demand(selector as u16)?;
            }
        }
        let mut priorities = vec![0_u8; SOURCE_TMAP_COUNT];
        let parameters = presentation
            .entity_parameters
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for &(class, model) in authored {
            let mut selected = None;
            for (index, row) in presentation.entity_parameters.iter().enumerate() {
                let kind = i16::from_le_bytes([row[0], row[1]]);
                if kind < 0 {
                    break;
                }
                if kind as u16 == class && i16::from_le_bytes([row[2], row[3]]) == model {
                    selected = Some(index * row.len() + 4);
                }
            }
            if let Some(mut offset) = selected {
                loop {
                    let bytes = parameters
                        .get(offset..offset + 2)
                        .ok_or("source716C0 preload list lacks its terminator")?;
                    let selector = u16::from_le_bytes(bytes.try_into().unwrap());
                    if selector & 0x8000 != 0 {
                        break;
                    }
                    let particle = presentation
                        .particles
                        .get(usize::from(selector))
                        .ok_or("source716C0 preload particle is absent")?;
                    let group = state
                        .catalog
                        .as_ref()
                        .unwrap()
                        .record(particle.base_tmap)
                        .ok_or("source716C0 preload TMAP is absent")?
                        .group;
                    priorities[usize::from(group)] = particle.byte_11 as u8;
                    offset += 2;
                }
            }
        }
        // sub_71780 first removes nonpermanent groups absent from the
        // authored priority set, then loads priorities255..1 in TAB order.
        for selector in 0..SOURCE_TMAP_COUNT {
            if priorities[selector] == 0 {
                state.unload_group(selector as u16)?;
            }
        }
        for priority in (1..=255_u8).rev() {
            for selector in 0..SOURCE_TMAP_COUNT {
                if priorities[selector] == priority && !state.resident[selector] {
                    state.demand(selector as u16)?;
                }
            }
        }
        Ok(state)
    }

    pub(crate) fn matches_catalog(&self, catalog: &SourceTmapCatalog) -> bool {
        self.catalog.as_ref() == Some(catalog)
    }

    pub(crate) fn catalog(&self) -> Option<&SourceTmapCatalog> {
        self.catalog.as_ref()
    }

    pub(crate) fn is_bound(&self) -> bool {
        self.catalog.is_some()
    }

    pub(crate) fn demand(&mut self, selector: u16) -> Result<bool, String> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or("source animation catalog is unbound")?;
        let group = catalog
            .record(selector)
            .ok_or("render TMAP demand exceeds504")?
            .group;
        let members = catalog
            .records
            .iter()
            .filter(|row| row.group == group)
            .cloned()
            .collect::<Vec<_>>();
        let mut changed = false;
        for row in members {
            if self.resident[usize::from(row.selector)] {
                continue;
            }
            self.resident[usize::from(row.selector)] = true;
            changed = true;
            if row.encoded_frame_count.is_some() {
                // Literal721C0 cannot allocate record0 when it is the only
                // free record (v3<=0 chooses the absent v4 fallback).
                let slot = self
                    .animation_slots
                    .iter()
                    .rposition(Option::is_none)
                    .filter(|&slot| slot != 0)
                    .ok_or("source animation pool has no admissible free record")?;
                self.animation_slots[slot] = Some(row.selector);
            }
        }
        Ok(changed)
    }

    fn unload_group(&mut self, selector: u16) -> Result<(), String> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or("source animation catalog is unbound")?;
        let group = catalog
            .record(selector)
            .ok_or("source unload selector exceeds504")?
            .group;
        if self.permanent[usize::from(group)] || !self.resident[usize::from(group)] {
            return Ok(());
        }
        for row in catalog.records.iter().filter(|row| row.group == group) {
            self.resident[usize::from(row.selector)] = false;
            for slot in self
                .animation_slots
                .iter_mut()
                .filter(|slot| **slot == Some(row.selector))
            {
                *slot = None;
            }
        }
        Ok(())
    }

    /// Source2B860 reads the resident record's CountOfFrames after71AB0.
    /// That reset changes pixels/playback state, not this header or residency;
    /// it does not demand an absent asset. This is distinct from221F0's
    /// literal sixteen-record lookahead below.
    pub(crate) fn resident_frame_count_read(&self, base_tmap: u16) -> Result<Option<u16>, String> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or("source frame count requires a qualified complete TMAP catalog")?;
        let resident = self
            .resident
            .get(usize::from(base_tmap))
            .ok_or("source frame count selector exceeds504")?;
        if !resident {
            return Ok(None);
        }
        if !self.animation_slots.contains(&Some(base_tmap)) {
            return Err(format!(
                "source frame count resident TMAP{base_tmap} has no animation record"
            ));
        }
        catalog
            .record(base_tmap)
            .and_then(|record| record.encoded_frame_count)
            .map(Some)
            .ok_or_else(|| format!("source frame count TMAP{base_tmap} lacks its animation header"))
    }

    pub(crate) fn boss_timer_read(&self, base_tmap: u16) -> Result<Option<u16>, String> {
        if self.catalog.is_none() {
            return Err("source221F0 requires a qualified complete TMAP catalog".into());
        }
        let resident = self
            .resident
            .get(usize::from(base_tmap))
            .ok_or("boss animation selector exceeds504")?;
        if !resident {
            return Ok(None);
        }
        let slot = self
            .animation_slots
            .iter()
            .position(|slot| *slot == Some(base_tmap))
            .ok_or_else(|| {
                format!("source221F0 resident TMAP{base_tmap} has no animation record")
            })?;
        // GetAnimationByIndex returns type_animations1*: the pinned +16 is
        // sixteen RECORDS, not the current record's CountOfFrames offset.
        let addressed = self.animation_slots.get(slot + 16).ok_or_else(|| {
            format!(
                "source221F0 undefined animation read: TMAP{base_tmap} record{slot}+16 exceeds504"
            )
        })?;
        Ok(Some(u16::from(addressed.is_some())))
    }

    pub(crate) fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update(b"source-animation-pool-v1");
        hasher.update([u8::from(self.catalog.is_some())]);
        if let Some(catalog) = &self.catalog {
            for row in &catalog.records {
                hasher.update(row.selector.to_le_bytes());
                hasher.update(row.group.to_le_bytes());
                hasher.update(row.allocation_bytes.to_le_bytes());
                hasher.update([u8::from(row.encoded_frame_count.is_some())]);
                hasher.update(row.encoded_frame_count.unwrap_or(0).to_le_bytes());
            }
        }
        for (&resident, (&slot, &permanent)) in self
            .resident
            .iter()
            .zip(self.animation_slots.iter().zip(self.permanent.iter()))
        {
            hasher.update([u8::from(resident), u8::from(permanent)]);
            hasher.update(slot.unwrap_or(u16::MAX).to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> SourceTmapCatalog {
        SourceTmapCatalog::new(
            (0..504_u16)
                .map(|selector| SourceTmapMetadata {
                    selector,
                    group: selector,
                    allocation_bytes: 16,
                    encoded_frame_count: (selector < 32).then_some(5),
                })
                .collect(),
        )
        .unwrap()
    }

    fn empty_pool() -> SourceAnimationResidency {
        SourceAnimationResidency {
            catalog: Some(catalog()),
            ..SourceAnimationResidency::default()
        }
    }

    #[test]
    fn resident_header_read_is_lazy_and_distinct_from_literal_boss_lookahead() {
        let mut state = empty_pool();
        let absent = state.clone();
        assert_eq!(state.resident_frame_count_read(0).unwrap(), None);
        assert_eq!(state, absent, "a gameplay read never loads an absent asset");
        state.demand(0).unwrap();
        assert_eq!(state.animation_slots[503], Some(0));
        let resident = state.clone();
        assert_eq!(state.resident_frame_count_read(0).unwrap(), Some(5));
        assert_eq!(state, resident, "header reads preserve playback allocation");
        assert!(state.boss_timer_read(0).unwrap_err().contains("record503+16"));
        assert!(state.resident_frame_count_read(504).is_err());
        assert!(SourceAnimationResidency::default().resident_frame_count_read(0).is_err());
        state.unload_group(0).unwrap();
        assert_eq!(state.resident_frame_count_read(0).unwrap(), None);
        state.demand(0).unwrap();
        state.animation_slots[503] = None;
        assert!(state.resident_frame_count_read(0).unwrap_err().contains("no animation record"));
    }

    #[test]
    fn literal_pool_read_distinguishes_unresident_active_inactive_and_undefined() {
        // Configured pool controls pin the literal pointer expression. Header
        // frame counts deliberately differ from the queried active word.
        let mut state = empty_pool();
        assert_eq!(state.boss_timer_read(16).unwrap(), None);
        state.demand(0).unwrap();
        assert_eq!(
            state.animation_slots[503],
            Some(0),
            "721C0 chooses last free"
        );
        assert!(
            state
                .boss_timer_read(0)
                .unwrap_err()
                .contains("record503+16")
        );
        for selector in 1..=16 {
            state.demand(selector).unwrap();
        }
        assert_eq!(state.animation_slots[487], Some(16));
        assert_eq!(state.boss_timer_read(16).unwrap(), Some(1));
        state.unload_group(0).unwrap();
        assert_eq!(state.boss_timer_read(16).unwrap(), Some(0));
        assert_eq!(state.boss_timer_read(0).unwrap(), None);
        state.demand(17).unwrap();
        assert_eq!(
            state.animation_slots[503],
            Some(17),
            "freed last slot is reused"
        );
        assert_eq!(state.boss_timer_read(16).unwrap(), Some(1));
        assert_eq!(
            state.boss_timer_read(31).unwrap(),
            None,
            "an unrelated undefined entry is not eagerly queried"
        );
        assert!(state.boss_timer_read(504).is_err());
        assert!(
            SourceAnimationResidency::default()
                .boss_timer_read(16)
                .is_err()
        );
    }

    #[test]
    fn group_demand_uses_tab_order_and_changes_the_owned_hash() {
        let mut rows = catalog().records.into_vec();
        for row in &mut rows[..8] {
            row.group = 0;
        }
        let mut state = SourceAnimationResidency {
            catalog: Some(SourceTmapCatalog::new(rows).unwrap()),
            ..SourceAnimationResidency::default()
        };
        let before = state.clone();
        assert!(state.demand(7).unwrap());
        for selector in 0..8 {
            assert_eq!(state.animation_slots[503 - selector], Some(selector as u16));
        }
        assert!(!state.demand(2).unwrap());
        let digest = |state: &SourceAnimationResidency| {
            let mut hasher = Sha256::new();
            state.hash_into(&mut hasher);
            hasher.finalize()
        };
        assert_ne!(digest(&state), digest(&before));
        assert_eq!(
            before,
            SourceAnimationResidency {
                catalog: Some(state.catalog.clone().unwrap()),
                ..SourceAnimationResidency::default()
            }
        );
    }

    #[test]
    fn catalog_rejects_logically_fitting_but_physically_displaced_bank() {
        let mut rows = catalog().records.into_vec();
        for row in &mut rows {
            row.allocation_bytes = 30_000;
        }
        assert!(
            30_000 * 504 + SOURCE_DESCRIPTOR_BYTES < SOURCE_HEAP_BYTES,
            "configured bank fits the source allocator's logical budget"
        );
        assert!(
            SourceTmapCatalog::new(rows)
                .unwrap_err()
                .contains("undefined physical heap placement"),
            "literal uint32_t pointer displacement must also fit the real20MiB heap"
        );
    }

    #[test]
    fn qualified_level25_owner_headers_startup_clone_and_reset_preserve_pool() {
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| generated.join("mc-bhq10-owner-v3-level25-20261029"));
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                generated.join("dragon-bee-20261105-integration/current-level25-environment")
            });
        let selection = crate::campaign::spec::CampaignSelection::ordinary(25).unwrap();
        let error = crate::campaign::CampaignSession::open(&package, selection).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing gameplay animation assets"),
            "future authored boss production must qualify assets before exposing the session"
        );
        let legacy = generated.join("level1-unity-asset-cache-plan29-final");
        let error =
            crate::campaign::CampaignSession::open_with_owner_cache(&package, &legacy, selection)
                .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing gameplay animation assets"),
            "an incomplete legacy bank cannot stand in for this required gameplay input"
        );
        crate::campaign::CampaignSession::open_with_owner_cache(
            generated.join("mc-bhq8-level1-collision-v5-night"),
            &legacy,
            crate::campaign::spec::CampaignSelection::LEVEL1,
        )
        .expect("ordinary legacy non-boss worlds do not require the new consumer's assets");
        let campaign =
            crate::campaign::CampaignSession::open_with_owner_cache(&package, cache, selection)
                .unwrap();
        assert!(
            !campaign
                .simulation()
                .entities()
                .iter()
                .any(|row| row.active && (row.class, row.model) == (5, 10)),
            "this qualification covers the future group producer, not only live bosses"
        );
        let mut simulation = campaign.simulation().clone();
        let before = simulation.source_animations.clone();
        let catalog = before.catalog.as_ref().unwrap();
        assert_eq!(
            [480, 488, 496].map(|base| catalog.record(base).unwrap().encoded_frame_count),
            [Some(5), Some(15), Some(20)],
            "actual decoded headers, never substituted for221F0"
        );
        assert_eq!(
            before.resident.iter().filter(|&&resident| resident).count(),
            359
        );
        assert_eq!(
            before
                .animation_slots
                .iter()
                .filter(|slot| slot.is_some())
                .count(),
            137
        );
        // Independent source/data replay: permanent preload, then authored
        // row379's type5/model10 list and the complete priority preload.
        for (base, slot, lookahead) in [(480, 390, 403), (488, 382, 412), (496, 374, 480)] {
            assert!(
                before.resident[usize::from(base)],
                "authored model10 preloads TMAP{base}"
            );
            assert_eq!(before.animation_slots[slot], Some(base));
            assert_eq!(before.animation_slots[slot + 16], Some(lookahead));
            assert_eq!(before.boss_timer_read(base).unwrap(), Some(1));
        }
        let selector = (0..504_u16)
            .find(|&selector| {
                !before.resident[usize::from(selector)]
                    && catalog
                        .record(selector)
                        .unwrap()
                        .encoded_frame_count
                        .is_some()
            })
            .unwrap();
        // Mc2Simulation::state_hash is the retained flight-only contract.
        // Campaign/replay consumers fingerprint this authoritative composite.
        let hash = simulation.campaign_state_hash();
        simulation
            .apply_source_render_feedback(simulation.state.tick, &[], &[selector])
            .unwrap();
        assert_ne!(simulation.source_animations, before);
        assert_ne!(simulation.campaign_state_hash(), hash);
        assert_eq!(
            campaign.simulation().source_animations,
            before,
            "clone isolation"
        );
        assert_eq!(
            campaign.state_hash(),
            hash,
            "the original authority remains unchanged"
        );
        simulation.reset();
        assert_eq!(
            simulation.source_animations, before,
            "cold reset restores exact startup allocation order"
        );
        assert_eq!(simulation.campaign_state_hash(), hash);
    }
}
