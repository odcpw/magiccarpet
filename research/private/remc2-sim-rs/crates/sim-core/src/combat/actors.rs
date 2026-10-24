//! Source combat primitives operating on the caller's authoritative carriers.
//!
//! `sub_11900` contact is intentionally different from ordinary projectile and
//! area writers. Damage is a wrapping signed dword; a zero attacker means an
//! unowned payload, not an empty payload. Family callbacks retain that residue.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageWriter {
    /// `sub_10C80` mode 0: add to owned payload, replace unowned residue.
    Ordinary,
    /// `sub_11900`: replace owned payload, add to unowned residue.
    Contact,
    /// `sub_10C80` modes 3/4: only the first writer acquires the lane.
    FirstWriter,
}

pub struct DamageMailboxMut<'a> {
    payload: &'a mut i32,
    attacker: &'a mut u16,
}

impl<'a> DamageMailboxMut<'a> {
    pub fn new(payload: &'a mut i32, attacker: &'a mut u16) -> Self {
        Self { payload, attacker }
    }

    /// Producer admission (overlap, class and ownership) belongs to its owner.
    /// Do not clamp or reject the signed payload here: the source uses all bits.
    pub fn submit(&mut self, payload: i32, attacker: u16, writer: DamageWriter) -> bool {
        if writer == DamageWriter::FirstWriter && *self.attacker != 0 {
            return false;
        }
        *self.payload = match (writer, *self.attacker != 0) {
            (DamageWriter::Ordinary, true) | (DamageWriter::Contact, false) => {
                self.payload.wrapping_add(payload)
            }
            _ => payload,
        };
        *self.attacker = attacker;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DamageHit {
    pub damage: i32,
    pub attacker: u16,
    pub lethal: bool,
}

/// Family `1BF90`/`1C310` consumption. The attacker word owns the mailbox;
/// damage may be zero, and an unowned nonzero payload must not be applied.
pub fn consume_damage(
    life: &mut i32,
    payload: &mut i32,
    pending_attacker: &mut u16,
    current_attacker: &mut u16,
) -> Option<DamageHit> {
    *current_attacker = *pending_attacker;
    if *pending_attacker == 0 {
        return None;
    }
    *life = life.wrapping_sub(*payload);
    *pending_attacker = 0;
    Some(DamageHit {
        damage: *payload,
        attacker: *current_attacker,
        lethal: *life < 0,
    })
}

/// Called in link order, stopping at the first lower life, as in `1BF90`.
/// A lower child is a hit even when its attacker is zero.
pub fn inherit_linked_life(
    life: &mut i32,
    current_attacker: &mut u16,
    child_life: i32,
    child_attacker: u16,
) -> bool {
    if child_life >= *life {
        return false;
    }
    *life = child_life;
    *current_attacker = child_attacker;
    true
}

/// PreKill walks the entire chain: the last nonzero child killer wins.
pub fn inherit_killer(root_killer: &mut u16, child_killer: u16) {
    if child_killer != 0 {
        *root_killer = child_killer;
    }
}

#[must_use]
pub const fn awards_creature_kill(
    victim_model: u8,
    victim_source_id: u16,
    killer_source_id: u16,
    killer_class: u8,
    killer_model: u8,
) -> bool {
    killer_source_id != 0
        && killer_class == 3
        && killer_model == 0
        && victim_source_id != killer_source_id
        && !matches!(victim_model, 9 | 12 | 13 | 14 | 15)
}

/// The source's inner `manaFraction` shadows the outer loop bound. Even the
/// `true` branch emits one sphere containing ALL mana (the final remainder).
#[must_use]
pub const fn death_mana_sphere_count(mana: i32, _use_mana_fraction: bool) -> usize {
    if mana > 0 { 1 } else { 0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerEffectLane {
    StealMana,
    Duel,
}

impl PlayerEffectLane {
    const fn offset(self) -> usize {
        match self {
            Self::StealMana => 18,
            Self::Duel => 24,
        }
    }
}

/// Deliberate repair of pinned remc2 `sub_10C80` / `sub_11900` pointer
/// arithmetic: six-byte lanes within THIS entity's `str_0x5E_94` (36 bytes).
/// No entity offset is ever interpreted as an adjacent entity address. The
/// caller supplies the mailbox only, excluding any exported record header.
pub struct PlayerEffectMailboxesMut<'a> {
    bytes: &'a mut [u8; 36],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerEffectHit {
    pub payload: i32,
    pub attacker: u16,
}

#[must_use]
pub fn pending_player_effect(bytes: &[u8; 36], lane: PlayerEffectLane) -> Option<PlayerEffectHit> {
    let offset = lane.offset();
    let attacker = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap());
    (attacker != 0).then(|| PlayerEffectHit {
        payload: i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
        attacker,
    })
}

impl<'a> PlayerEffectMailboxesMut<'a> {
    pub fn new(bytes: &'a mut [u8; 36]) -> Self {
        Self { bytes }
    }

    /// Mode 3/4 scan admits overlapping class-3 targets with a different source
    /// id. Unlike mode 0 it has no ordinary-damage mask or subtype gate.
    pub fn submit_overlap(
        &mut self,
        lane: PlayerEffectLane,
        payload: u16,
        attacker_source_id: u16,
        target_source_id: u16,
        target_class: u8,
        overlaps: bool,
    ) -> bool {
        if target_class != 3 || attacker_source_id == target_source_id || !overlaps {
            return false;
        }
        let offset = lane.offset();
        if self.bytes[offset + 4..offset + 6] != [0, 0] {
            return false;
        }
        self.bytes[offset..offset + 4].copy_from_slice(&i32::from(payload).to_le_bytes());
        self.bytes[offset + 4..offset + 6].copy_from_slice(&attacker_source_id.to_le_bytes());
        true
    }

    /// Player `5EFA0` consumes Duel before StealMana, before ordinary damage.
    /// A dead player's effects remain pending. Only the owner word is cleared.
    pub fn take(&mut self, lane: PlayerEffectLane, life: i32) -> Option<PlayerEffectHit> {
        let offset = lane.offset();
        if life < 0 {
            return None;
        }
        let hit = pending_player_effect(self.bytes, lane)?;
        self.bytes[offset + 4..offset + 6].fill(0);
        Some(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writers_and_consumer_preserve_ownerless_residue_and_wrapping() {
        let (mut payload, mut attacker, mut life, mut current) = (7, 0, 100, 0);
        assert!(DamageMailboxMut::new(&mut payload, &mut attacker).submit(
            10,
            1,
            DamageWriter::Contact
        ));
        assert_eq!((payload, attacker), (17, 1));
        DamageMailboxMut::new(&mut payload, &mut attacker).submit(20, 2, DamageWriter::Contact);
        assert_eq!(payload, 20);
        DamageMailboxMut::new(&mut payload, &mut attacker).submit(30, 3, DamageWriter::Ordinary);
        let hit = consume_damage(&mut life, &mut payload, &mut attacker, &mut current).unwrap();
        assert_eq!(
            (life, payload, attacker, current, hit.damage),
            (50, 50, 0, 3, 50)
        );
        assert!(consume_damage(&mut life, &mut payload, &mut attacker, &mut current).is_none());
        DamageMailboxMut::new(&mut payload, &mut attacker).submit(
            i32::MAX,
            4,
            DamageWriter::Ordinary,
        );
        DamageMailboxMut::new(&mut payload, &mut attacker).submit(1, 5, DamageWriter::Ordinary);
        assert_eq!(payload, i32::MIN);
        consume_damage(&mut life, &mut payload, &mut attacker, &mut current);
        assert_eq!(life, 50_i32.wrapping_sub(i32::MIN));
    }

    #[test]
    fn linked_damage_and_credit_follow_source_link_order() {
        let (mut life, mut attacker, mut killer) = (100, 3, 3);
        assert!(inherit_linked_life(&mut life, &mut attacker, -1, 0));
        assert_eq!((life, attacker), (-1, 0));
        for child in [4, 0, 7, 0] {
            inherit_killer(&mut killer, child);
        }
        assert_eq!(killer, 7);
        for model in [9, 12, 13, 14, 15] {
            assert!(!awards_creature_kill(model, 8, 7, 3, 0));
        }
        assert!(awards_creature_kill(4, 8, 7, 3, 0));
        assert!(!awards_creature_kill(4, 7, 7, 3, 0));
        assert!(!awards_creature_kill(4, 8, 7, 3, 1));
    }

    #[test]
    fn repaired_effect_lanes_are_bounded_first_writer_and_clear_only_owners() {
        let mut bytes = [0; 36];
        bytes[..6].copy_from_slice(&[4, 3, 2, 1, 9, 0]);
        let ordinary = bytes[..6].to_vec();
        let mut lanes = PlayerEffectMailboxesMut::new(&mut bytes);
        assert!(!lanes.submit_overlap(PlayerEffectLane::Duel, 5, 7, 8, 5, true));
        assert!(!lanes.submit_overlap(PlayerEffectLane::Duel, 5, 7, 7, 3, true));
        assert!(!lanes.submit_overlap(PlayerEffectLane::Duel, 5, 7, 8, 3, false));
        assert!(lanes.submit_overlap(PlayerEffectLane::StealMana, 2, 7, 8, 3, true));
        assert!(lanes.submit_overlap(PlayerEffectLane::Duel, 5, 9, 8, 3, true));
        assert!(!lanes.submit_overlap(PlayerEffectLane::Duel, 6, 10, 8, 3, true));
        assert!(lanes.take(PlayerEffectLane::Duel, -1).is_none());
        assert_eq!(
            lanes.take(PlayerEffectLane::Duel, 0),
            Some(PlayerEffectHit {
                payload: 5,
                attacker: 9
            })
        );
        assert_eq!(
            lanes.take(PlayerEffectLane::StealMana, 0),
            Some(PlayerEffectHit {
                payload: 2,
                attacker: 7
            })
        );
        assert_eq!(&bytes[..6], ordinary.as_slice());
        assert_eq!(i32::from_le_bytes(bytes[24..28].try_into().unwrap()), 5);
        assert_eq!(death_mana_sphere_count(16_000, true), 1);
    }
}
