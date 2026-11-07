//! Bounded presentation journal for native character deaths. This is not game
//! state: neither death detection nor consumers may mutate mana or ownership.
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use super::Mc2Simulation;

pub const VOXEL_DEATH_CAPACITY: usize = 1024;

fn goat_dead(action: u8, life: i32) -> bool { life < 0 || matches!(action, 12 | 13) }
fn archer_dead(action: u8, life: i32) -> bool { life < 0 || action == 37 }

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Identity {
    slot: u16,
    slot_generation: u32,
    source_id: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorDeath {
    pub sequence: u64,
    pub slot: u16,
    pub slot_generation: u32,
    pub source_id: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct JournalData {
    rows: VecDeque<ActorDeath>,
    // Keep currently dying actors separate from the ring, so eviction after
    // many other deaths cannot re-emit an actor which is still dying.
    currently_dead: BTreeSet<Identity>,
    next_sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeathJournal(Arc<JournalData>);

impl DeathJournal {
    #[must_use]
    pub fn rows(&self) -> &VecDeque<ActorDeath> { &self.0.rows }

    fn observe(&mut self, deaths: BTreeSet<Identity>) {
        if self.0.currently_dead == deaths { return; }
        let data = Arc::make_mut(&mut self.0);
        for identity in deaths.difference(&data.currently_dead) {
            // A temporarily absent row in the same lifetime must not cause a
            // second emission while its previous record remains available.
            if data.rows.iter().any(|row| row.slot == identity.slot
                && row.slot_generation == identity.slot_generation && row.source_id == identity.source_id) { continue; }
            data.next_sequence = data.next_sequence.checked_add(1).expect("voxel death sequence overflow");
            if data.rows.len() == VOXEL_DEATH_CAPACITY { data.rows.pop_front(); }
            data.rows.push_back(ActorDeath { sequence: data.next_sequence, slot: identity.slot,
                slot_generation: identity.slot_generation, source_id: identity.source_id });
        }
        data.currently_dead = deaths;
    }
}

impl Mc2Simulation {
    /// Capture immediately after each native damage/dispatch boundary and
    /// before reclamation or reuse can replace the authoritative actor row.
    pub fn capture_voxel_deaths(&mut self) -> Result<(), String> {
        if self.canonical_terrain().voxel_deaths().is_none() { return Ok(()); }
        let live = &self.session.state.live;
        let arena = &live.economy;
        let economy = &arena.economy;
        let context = super::source_target::Context::from_ordinary(&live.goats, &live.ordinary);
        let mut deaths = BTreeSet::new();
        for goat in self.goats().iter().filter(|goat| goat.active && goat_dead(goat.action, goat.life)) {
            let Some(handle) = arena.handle(goat.entity_index) else { continue; };
            let Some(current) = super::source_target::read(arena, context, goat.entity_index)? else { continue; };
            if !current.active || (current.class, current.model, current.source_id) != (5, 1, goat.source_id) { continue; }
            deaths.insert(Identity { slot: goat.entity_index,
                slot_generation: handle.generation, source_id: goat.source_id });
        }
        for source in economy.authored_class5().iter().filter(|source|
            source.active && source.model == 4 && archer_dead(source.action, source.life)) {
            let Some(archer) = economy.authored_class5_archer().iter().find(|archer|
                archer.source_entity_index == source.source_entity_index) else { continue; };
            let Some(handle) = arena.handle(source.source_entity_index) else { continue; };
            let Some(current) = super::source_target::read(arena, context, source.source_entity_index)? else { continue; };
            if !current.active || (current.class, current.model, current.source_id) != (5, 4, archer.entity_id)
                || handle.generation != archer.slot_generation { continue; }
            deaths.insert(Identity { slot: source.source_entity_index,
                slot_generation: archer.slot_generation, source_id: archer.entity_id });
        }
        if let Some(journal) = self.session.state.terrain_mut().voxel_deaths_mut() {
            journal.observe(deaths);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(slot: u16, generation: u32, source_id: u16) -> Identity {
        Identity { slot, slot_generation: generation, source_id }
    }

    #[test]
    fn death_predicates_reject_zero_life_and_living_archer_join() {
        assert!(!goat_dead(0, 0));
        assert!(goat_dead(12, 1));
        assert!(!archer_dead(36, 100));
        assert!(archer_dead(36, -1));
        assert!(archer_dead(37, 1));
    }

    #[test]
    fn lifetime_dedup_reuse_and_clone_isolation() {
        let mut journal = DeathJournal::default();
        let first = identity(8, 1, 12);
        journal.observe([first].into_iter().collect());
        let old = journal.clone();
        journal.observe([first].into_iter().collect());
        journal.observe(BTreeSet::new());
        journal.observe([first].into_iter().collect());
        assert_eq!(journal.rows().len(), 1);
        journal.observe([identity(8, 2, 12)].into_iter().collect());
        assert_eq!(journal.rows().len(), 2);
        assert_eq!(journal.rows().back().unwrap().sequence, 2);
        assert_eq!(old.rows().len(), 1);
        assert!(DeathJournal::default().rows().is_empty());
    }

    #[test]
    fn ring_is_bounded_without_reemitting_a_still_dead_actor() {
        let mut journal = DeathJournal::default();
        let held = identity(1, 1, 1);
        for generation in 1..=VOXEL_DEATH_CAPACITY as u32 + 5 {
            journal.observe([held, identity(2, generation, 2)].into_iter().collect());
        }
        assert_eq!(journal.rows().len(), VOXEL_DEATH_CAPACITY);
        assert!(!journal.rows().iter().any(|row| row.slot == 1));
        let last = journal.rows().back().unwrap().sequence;
        journal.observe([held, identity(2, VOXEL_DEATH_CAPACITY as u32 + 6, 2)].into_iter().collect());
        assert_eq!(journal.rows().back().unwrap().sequence, last + 1);
        assert!(!journal.rows().iter().any(|row| row.slot == 1));
    }

    #[test]
    fn source_goat_damage_records_once_through_following_pass_reclaim() {
        use crate::voxel_world::{Grid, Point, Volume, VoxelWorld};
        let mut session = super::super::tests::level_two_campaign_session();
        // Only enables the journal; this isolated voxel cell is outside the
        // encounter and is not presented as a real terrain/gameplay fixture.
        let grid = Grid { origin: Point { x: -100_000, y: -100_000, z: 0 }, cell_size: 8, dimensions: [1, 1, 1] };
        let world = VoxelWorld::new(11, vec![Volume::terrain(grid, vec![1]).unwrap()]).unwrap();
        session.session.state.terrain_mut().install_voxel_world(world);
        let goat = session.goats().iter().find(|goat| goat.active && goat.life > 0
            && matches!(goat.action, 8 | 9 | 10 | 11 | 14 | 15)).unwrap().clone();
        let generation = session.economy().slot_generation(goat.entity_index);
        let attacker = session.shared_spells().players()[0].entity_index;
        session.submit_goat_damage(goat.entity_index, goat.life + 1, attacker).unwrap();
        let mut reclaimed = false;
        for pass in 0..120_u8 {
            session.step_level2_source_pass(pass);
            if !session.goats().iter().find(|row| row.entity_index == goat.entity_index).unwrap().active {
                reclaimed = true;
                break;
            }
        }
        assert!(reclaimed, "actual source damage must reach following-pass Goat reclaim");
        let rows = session.canonical_terrain().voxel_deaths().unwrap().rows();
        let matching = rows.iter().filter(|row| row.slot == goat.entity_index
            && row.slot_generation == generation && row.source_id == goat.source_id).count();
        assert_eq!(matching, 1, "one death for the native actor incarnation through reclaim");
    }
}
